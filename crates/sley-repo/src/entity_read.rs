//! Verified-repository adapter for bounded entity and signature reads
//! (AT-MW-02, contract `docs/spec/ENTITY_READ_PROFILE_V2.md` sections 3-5).
//!
//! Production trusted-input adapter only: it borrows the single
//! `VerifiedRevision` the response is read from and hands it to the pure
//! S20-310 owner. It supplies no caller-owned bindings, performs no
//! whole-root extraction, and owns no session admission or transport debit.

use sley_id::SessionId;
use sley_mutate::{value::EntityBodyValue, EntityObject};
use sley_query::{
    decode_entity_read_request, encode_entity_read_response, prepare_entity_read, EntityReadBody,
    EntityReadCeilings, EntityReadError, EntityReadMethod, EntityReadObject, EntityReadOutcome,
    EntityReadPlan, EntityReadRequest, EntityReadRevision, EntityReadSelection,
};
use sley_txn::VerifiedRevision;

pub use sley_query::capture_entity_read_selection;

fn view(revision: &VerifiedRevision) -> EntityReadRevision<'_> {
    let record = &revision.state_root().record;
    EntityReadRevision {
        workspace: record.workspace_id,
        root: revision.state_root().root,
        epoch: record.schema_epoch_id,
        bindings: &record.entity_bindings,
        tombstones: revision.tombstoned_entities(),
    }
}

/// Projects one stored object onto the query-owned narrow view.
///
/// Only the identity, kind tag, object identity, epoch, stored bytes, and
/// the Function/Parameter relationship facts cross the owner boundary;
/// every other body crosses as an opaque tag. The projection borrows the
/// verified revision, so the owner touches only the selected indices and
/// no whole-root view is ever built.
fn adapter_view_at(objects: &[EntityObject], index: usize) -> Option<EntityReadObject<'_>> {
    objects.get(index).map(view_object)
}

fn view_object(object: &EntityObject) -> EntityReadObject<'_> {
    let record = object.record();
    let body = match &record.body {
        EntityBodyValue::Function(function) => EntityReadBody::Function {
            parameters: &function.parameters,
        },
        EntityBodyValue::Parameter(parameter) => EntityReadBody::Parameter {
            owner: parameter.owner,
            role: parameter.role,
            ordinal: parameter.ordinal,
        },
        other => EntityReadBody::Other {
            kind: other.kind_tag(),
        },
    };
    EntityReadObject {
        entity: record.entity_id,
        kind: record.body.kind_tag(),
        object_id: object.object_id(),
        epoch: object.schema_epoch_id(),
        stored_bytes: object.stored_bytes(),
        body,
    }
}

/// Decodes the request and runs the ordered owner checks against one
/// verified revision, returning the borrowed selection without allocating
/// object-sized output.
///
/// The caller runs the frame preflight and the reservation against the
/// selection, then copies with
/// [`sley_query::capture_entity_read_selection`]; only the captured plan
/// can drive encoding.
///
/// # Errors
///
/// Returns the first failing contract check.
pub fn prepare_verified_entity_read<'a>(
    revision: &'a VerifiedRevision,
    method: EntityReadMethod,
    request_bytes: &[u8],
    selected: &EntityReadCeilings,
) -> Result<(EntityReadRequest, EntityReadSelection<'a>), EntityReadError> {
    let request = decode_entity_read_request(request_bytes)?;
    let objects = revision.objects();
    let selection = prepare_entity_read(
        method,
        &view(revision),
        &request,
        selected,
        objects,
        adapter_view_at,
    )?;
    Ok((request, selection))
}

/// Encodes the complete response for a prepared plan, consuming it.
///
/// # Errors
///
/// Returns `InternalInvariant` when the written bytes drift from the
/// preflight length.
pub fn encode_verified_entity_read_response(
    plan: EntityReadPlan,
    session: SessionId,
) -> Result<EntityReadOutcome, EntityReadError> {
    encode_entity_read_response(plan, session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sley_id::{EntityId, StateRoot};
    use sley_query::decode_entity_read_response;
    use sley_scb1::{encode_record, encode_uvar};

    use crate::test_support::{epoch, executable_bodies, genesis, id};

    fn ceilings() -> EntityReadCeilings {
        EntityReadCeilings {
            max_entities: 65_535,
            max_response_bytes: 67_108_864,
            max_work: 100_000_000,
            budget_before_dispatch: 100_000_000,
        }
    }

    fn encode_request(root: StateRoot, entity: EntityId) -> Vec<u8> {
        encode_record(&[
            (1, root.as_bytes().to_vec()),
            (2, entity.as_bytes().to_vec()),
            (3, encode_uvar(65_535)),
            (4, encode_uvar(67_108_864)),
            (5, encode_uvar(100_000_000)),
        ])
        .unwrap()
    }

    #[test]
    fn adapter_reads_exact_version_bytes_over_a_verified_revision() {
        let (_temp, transactions, genesis_id) =
            genesis("entity-read-version", executable_bodies(), &[]);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let root = revision.state_root().root;
        let session = SessionId::from_bytes([0x20; 32]);
        for object in revision.objects() {
            let entity = object.record().entity_id;
            let (request, selection) = prepare_verified_entity_read(
                &revision,
                EntityReadMethod::Version,
                &encode_request(root, entity),
                &ceilings(),
            )
            .unwrap();
            assert_eq!(request.entity, entity);
            let plan = sley_query::capture_entity_read_selection(selection).unwrap();
            let outcome =
                encode_verified_entity_read_response(plan, session).unwrap();
            assert_eq!(outcome.returned_entities, 1);
            let response = decode_entity_read_response(&outcome.body).unwrap();
            assert_eq!(response.root, root);
            assert_eq!(response.session, session);
            assert_eq!(response.requested_entity, entity);
            assert_eq!(response.objects.len(), 1);
            assert_eq!(response.objects[0].entity, entity);
            assert_eq!(response.objects[0].object_id, object.object_id());
            assert_eq!(
                response.objects[0].kind,
                u64::from(object.record().body.kind_tag())
            );
            assert_eq!(response.objects[0].stored_bytes, object.stored_bytes());
        }
    }

    #[test]
    fn adapter_signature_returns_ordered_parameters() {
        let (_temp, transactions, genesis_id) =
            genesis("entity-read-signature", executable_bodies(), &[]);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let root = revision.state_root().root;
        let session = SessionId::from_bytes([0x20; 32]);
        let (_request, selection) = prepare_verified_entity_read(
            &revision,
            EntityReadMethod::Signature,
            &encode_request(root, id(30)),
            &ceilings(),
        )
        .unwrap();
        let plan = sley_query::capture_entity_read_selection(selection).unwrap();
        let outcome = encode_verified_entity_read_response(plan, session).unwrap();
        assert_eq!(outcome.returned_entities, 3);
        let response = decode_entity_read_response(&outcome.body).unwrap();
        let entities: Vec<EntityId> = response
            .objects
            .iter()
            .map(|object| object.entity)
            .collect();
        assert_eq!(entities, vec![id(30), id(31), id(32)]);
        let kinds: Vec<u64> = response.objects.iter().map(|object| object.kind).collect();
        assert_eq!(kinds, vec![5, 6, 6]);
    }

    #[test]
    fn adapter_reports_owner_failures_against_verified_state() {
        let (_temp, transactions, genesis_id) =
            genesis("entity-read-failures", executable_bodies(), &[]);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let root = revision.state_root().root;
        assert_eq!(
            prepare_verified_entity_read(
                &revision,
                EntityReadMethod::Signature,
                &encode_request(root, id(6)),
                &ceilings(),
            )
            .unwrap_err(),
            EntityReadError::ClassNotApplicable
        );
        assert_eq!(
            prepare_verified_entity_read(
                &revision,
                EntityReadMethod::Version,
                &encode_request(root, id(99)),
                &ceilings(),
            )
            .unwrap_err(),
            EntityReadError::UnresolvedEntity
        );
        assert_eq!(
            prepare_verified_entity_read(
                &revision,
                EntityReadMethod::Version,
                &encode_request(StateRoot::from_bytes([0x77; 32]), id(6)),
                &ceilings(),
            )
            .unwrap_err(),
            EntityReadError::RootMismatch
        );
        assert_eq!(
            prepare_verified_entity_read(
                &revision,
                EntityReadMethod::Version,
                &[0x00],
                &ceilings(),
            )
            .unwrap_err(),
            EntityReadError::NotCanonical
        );
    }

    #[test]
    fn adapter_epoch_comes_from_the_verified_revision() {
        let (_temp, transactions, genesis_id) =
            genesis("entity-read-epoch", executable_bodies(), &[]);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        assert_eq!(revision.state_root().record.schema_epoch_id, epoch());
    }

    #[test]
    fn projection_exposes_function_parameter_and_opaque_views() {
        use sley_mutate::value::{FunctionBody, ParameterBody};
        use sley_mutate::{build_entity_object, EntityObjectRecord};
        use sley_query::EntityReadBody;
        use sley_ssmc::{ParameterRole, TypeExpr, Visibility};

        use crate::test_support::set;
        let empty = set(&[]);
        let function = build_entity_object(
            epoch(),
            &EntityObjectRecord {
                entity_id: id(40),
                body: EntityBodyValue::Function(FunctionBody {
                    type_parameters: Vec::new(),
                    parameters: vec![id(41), id(42)],
                    result_type: TypeExpr::Bool,
                    effects: empty.clone(),
                    entry_block: id(44),
                    blocks: vec![id(44)],
                    contracts: empty.clone(),
                    visibility: Visibility::Private,
                }),
                label: None,
                semantic_fingerprint: None,
            },
        )
        .unwrap();
        let view = view_object(&function);
        assert_eq!(view.entity, id(40));
        assert_eq!(view.kind, 5);
        assert_eq!(view.object_id, function.object_id());
        assert_eq!(view.epoch, epoch());
        assert_eq!(view.stored_bytes, function.stored_bytes());
        match view.body {
            EntityReadBody::Function { parameters } => {
                assert_eq!(parameters, &[id(41), id(42)]);
            }
            other => panic!("function projected as {other:?}"),
        }

        let parameter = build_entity_object(
            epoch(),
            &EntityObjectRecord {
                entity_id: id(41),
                body: EntityBodyValue::Parameter(ParameterBody {
                    owner: id(40),
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: TypeExpr::Bool,
                }),
                label: None,
                semantic_fingerprint: None,
            },
        )
        .unwrap();
        let view = view_object(&parameter);
        assert_eq!(view.kind, 6);
        match view.body {
            EntityReadBody::Parameter {
                owner,
                role,
                ordinal,
            } => {
                assert_eq!(owner, id(40));
                assert_eq!(role, ParameterRole::Function);
                assert_eq!(ordinal, 0);
            }
            other => panic!("parameter projected as {other:?}"),
        }

        let namespace = build_entity_object(
            epoch(),
            &EntityObjectRecord {
                entity_id: id(3),
                body: crate::test_support::namespace_body(),
                label: None,
                semantic_fingerprint: None,
            },
        )
        .unwrap();
        let view = view_object(&namespace);
        assert_eq!(view.kind, 3);
        match view.body {
            EntityReadBody::Other { kind } => assert_eq!(kind, 3),
            other => panic!("namespace projected as {other:?}"),
        }
    }

    #[test]
    fn projection_covers_every_body_variant_without_interpretation() {
        use sley_mutate::{build_entity_object, EntityObjectRecord};
        use sley_query::EntityReadBody;
        for (byte, body) in crate::complete_root::tests::complete_bodies() {
            let expected_kind = body.kind_tag();
            let object = build_entity_object(
                epoch(),
                &EntityObjectRecord {
                    entity_id: id(byte),
                    body: body.clone(),
                    label: None,
                    semantic_fingerprint: None,
                },
            )
            .unwrap();
            let view = view_object(&object);
            assert_eq!(view.entity, id(byte), "byte {byte} identity");
            assert_eq!(view.kind, expected_kind, "byte {byte} tag");
            assert_eq!(view.object_id, object.object_id(), "byte {byte} object id");
            assert_eq!(view.epoch, epoch(), "byte {byte} epoch");
            assert_eq!(
                view.stored_bytes,
                object.stored_bytes(),
                "byte {byte} bytes"
            );
            match (&object.record().body, view.body) {
                (EntityBodyValue::Function(expected), EntityReadBody::Function { parameters }) => {
                    assert_eq!(parameters, expected.parameters, "byte {byte} parameters");
                }
                (
                    EntityBodyValue::Parameter(expected),
                    EntityReadBody::Parameter {
                        owner,
                        role,
                        ordinal,
                    },
                ) => {
                    assert_eq!(owner, expected.owner, "byte {byte} owner");
                    assert_eq!(role, expected.role, "byte {byte} role");
                    assert_eq!(ordinal, expected.ordinal, "byte {byte} ordinal");
                }
                (_, EntityReadBody::Other { kind }) => {
                    assert_eq!(kind, expected_kind, "byte {byte} opaque tag");
                }
                (_, other) => panic!("byte {byte} projected as {other:?}"),
            }
        }
    }
}
