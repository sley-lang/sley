//! Verified-repository adapter for bounded entity and signature reads
//! (AT-MW-02, contract `docs/spec/ENTITY_READ_PROFILE_V2.md` sections 3-5).
//!
//! Production trusted-input adapter only: it borrows the single
//! `VerifiedRevision` the response is read from and hands it to the pure
//! S20-310 owner. It supplies no caller-owned bindings, performs no
//! whole-root extraction, and owns no session admission or transport debit.

use sley_id::SessionId;
use sley_query::{
    EntityReadCeilings, EntityReadError, EntityReadMethod, EntityReadOutcome, EntityReadPlan,
    EntityReadRequest, EntityReadRevision, decode_entity_read_request,
    encode_entity_read_response, prepare_entity_read,
};
use sley_txn::VerifiedRevision;

fn view(revision: &VerifiedRevision) -> EntityReadRevision<'_> {
    let record = &revision.state_root().record;
    EntityReadRevision {
        workspace: record.workspace_id,
        root: revision.state_root().root,
        epoch: record.schema_epoch_id,
        bindings: &record.entity_bindings,
        objects: revision.objects(),
        tombstones: revision.tombstoned_entities(),
    }
}

/// Decodes the request and runs the ordered owner checks against one
/// verified revision, without allocating object-sized output.
///
/// The returned plan borrows the revision; it carries the copied revision
/// view, request, and ceilings, and only it can drive encoding.
///
/// # Errors
///
/// Returns the first failing contract check.
pub fn prepare_verified_entity_read<'rev>(
    revision: &'rev VerifiedRevision,
    method: EntityReadMethod,
    request_bytes: &[u8],
    selected: &EntityReadCeilings,
) -> Result<(EntityReadRequest, EntityReadPlan<'rev>), EntityReadError> {
    let request = decode_entity_read_request(request_bytes)?;
    let plan = prepare_entity_read(method, &view(revision), &request, selected)?;
    Ok((request, plan))
}

/// Encodes the complete response for a prepared plan.
///
/// # Errors
///
/// Returns `InternalInvariant` when the written bytes drift from the
/// preflight length.
pub fn encode_verified_entity_read_response(
    plan: &EntityReadPlan<'_>,
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
            let (request, plan) = prepare_verified_entity_read(
                &revision,
                EntityReadMethod::Version,
                &encode_request(root, entity),
                &ceilings(),
            )
            .unwrap();
            assert_eq!(request.entity, entity);
            let outcome =
                encode_verified_entity_read_response(&plan, session).unwrap();
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
        let (_request, plan) = prepare_verified_entity_read(
            &revision,
            EntityReadMethod::Signature,
            &encode_request(root, id(30)),
            &ceilings(),
        )
        .unwrap();
        let outcome = encode_verified_entity_read_response(&plan, session).unwrap();
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
}
