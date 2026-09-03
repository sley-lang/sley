//! Complete-root extraction adapter (S20-250 full, contract
//! `docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md` section 6).
//!
//! `sley-repo` already owns verified revisions, whose objects are loaded and
//! inventory-checked by `sley-txn`, and it depends on the validator's public
//! projection. The adapter therefore lives here: it projects a verified
//! revision's objects onto the normative model, copies the record's three
//! root facts, and hands borrowed bodies to the pure `sley-query` judgment.
//! It grants no root, commit, or runtime authority.

use core::fmt;

use sley_id::EntityId;
use sley_policy::complete_entities::{
    CompleteEntities, CompleteProjectionError, project_complete_entities,
};
use sley_query::{
    CompleteRootFacts, CompleteRootIndex, ImpactEntity, ImpactError, ImpactErrorCode,
    MAX_IMPACT_ENTITIES, judge_complete_root,
};
use sley_txn::VerifiedRevision;

/// Complete-root extraction or judgment failure with its exact source code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompleteRootError {
    /// The validator's projection failed (`GRAPH_*` or `SSMC_*`).
    Projection(CompleteProjectionError),
    /// The S20-250 judgment failed (`IMPACT_*`).
    Impact(ImpactError),
}

impl CompleteRootError {
    /// Returns the exact stable symbolic code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Projection(error) => error.symbol(),
            Self::Impact(error) => error.code().as_str(),
        }
    }

    /// Returns the exact stable numeric code.
    #[must_use]
    pub const fn numeric(&self) -> u32 {
        match self {
            Self::Projection(error) => error.numeric(),
            Self::Impact(error) => error.code().numeric(),
        }
    }
}

impl fmt::Display for CompleteRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for CompleteRootError {}

impl From<CompleteProjectionError> for CompleteRootError {
    fn from(value: CompleteProjectionError) -> Self {
        Self::Projection(value)
    }
}

impl From<ImpactError> for CompleteRootError {
    fn from(value: ImpactError) -> Self {
        Self::Impact(value)
    }
}

/// Complete-root request extracted from one verified revision.
#[derive(Clone, Debug)]
pub struct CompleteRootRequest {
    entities: CompleteEntities,
    bound_entities: Vec<EntityId>,
    entry_points: Vec<EntityId>,
    dependency_roots: Vec<sley_id::StateRoot>,
}

impl CompleteRootRequest {
    /// Extracts the request from a verified revision without judging it.
    ///
    /// Step 1 of the contract fails closed above `65,535` bindings; steps 2
    /// and 3 are already established by the revision loader, and the
    /// adapter re-checks that every object sits at its binding with its
    /// bound identity (`IMPACT_ROOT_BINDING_MISMATCH` otherwise); step 4 is
    /// the validator's projection.
    ///
    /// # Errors
    ///
    /// Returns the resource, binding, or projection failure.
    pub fn extract(revision: &VerifiedRevision) -> Result<Self, CompleteRootError> {
        let record = &revision.state_root().record;
        if record.entity_bindings.len() > MAX_IMPACT_ENTITIES {
            return Err(ImpactError::new(ImpactErrorCode::ResourceLimit).into());
        }
        let objects = revision.objects();
        let aligned = objects.len() == record.entity_bindings.len()
            && objects.iter().zip(&record.entity_bindings).all(
                |(object, (entity_id, object_id))| {
                    object.record().entity_id == *entity_id && object.object_id() == *object_id
                },
            );
        if !aligned {
            return Err(ImpactError::new(ImpactErrorCode::RootBindingMismatch).into());
        }
        let entities = project_complete_entities(objects)?;
        Ok(Self {
            entities,
            bound_entities: record.entity_bindings.iter().map(|(id, _)| *id).collect(),
            entry_points: record.entry_points.clone(),
            dependency_roots: record.dependency_roots.clone(),
        })
    }

    /// Returns the projected definitions.
    #[must_use]
    pub const fn entities(&self) -> &CompleteEntities {
        &self.entities
    }

    /// Returns the raw-ID-sorted borrowed request over all eighteen kinds.
    #[must_use]
    pub fn borrowed(&self) -> Vec<ImpactEntity<'_>> {
        borrow_entities(&self.entities)
    }

    /// Returns the three root facts.
    #[must_use]
    pub fn facts(&self) -> CompleteRootFacts<'_> {
        CompleteRootFacts {
            bound_entities: &self.bound_entities,
            entry_points: &self.entry_points,
            dependency_roots: &self.dependency_roots,
        }
    }

    /// Runs the pure closure judgment and builds the exact index.
    ///
    /// # Errors
    ///
    /// Returns the first S20-250 failure.
    pub fn judge(&self) -> Result<CompleteRootIndex, CompleteRootError> {
        judge_complete_root(&self.borrowed(), self.facts()).map_err(Into::into)
    }
}

/// Extracts and judges the complete root of one verified revision.
///
/// # Errors
///
/// Returns the first extraction, projection, or judgment failure.
pub fn judge_complete_root_revision(
    revision: &VerifiedRevision,
) -> Result<CompleteRootIndex, CompleteRootError> {
    CompleteRootRequest::extract(revision)?.judge()
}

/// Borrows every projected definition as a raw-ID-sorted impact request.
#[must_use]
pub fn borrow_entities(entities: &CompleteEntities) -> Vec<ImpactEntity<'_>> {
    let mut borrowed = Vec::with_capacity(entities.len());
    borrowed.extend(entities.workspaces.iter().map(ImpactEntity::Workspace));
    borrowed.extend(entities.packages.iter().map(ImpactEntity::Package));
    borrowed.extend(entities.namespaces.iter().map(ImpactEntity::Namespace));
    borrowed.extend(entities.type_definitions.iter().map(ImpactEntity::TypeDef));
    borrowed.extend(entities.functions.iter().map(ImpactEntity::Function));
    borrowed.extend(entities.parameters.iter().map(ImpactEntity::Parameter));
    borrowed.extend(entities.blocks.iter().map(ImpactEntity::Block));
    borrowed.extend(entities.operations.iter().map(ImpactEntity::Operation));
    borrowed.extend(entities.constants.iter().map(ImpactEntity::Constant));
    borrowed.extend(entities.globals.iter().map(ImpactEntity::GlobalValue));
    borrowed.extend(entities.effects.iter().map(ImpactEntity::EffectDef));
    borrowed.extend(
        entities
            .requirements
            .iter()
            .map(ImpactEntity::CapabilityRequirement),
    );
    borrowed.extend(entities.contracts.iter().map(ImpactEntity::Contract));
    borrowed.extend(entities.tests.iter().map(ImpactEntity::TestCase));
    borrowed.extend(entities.adapters.iter().map(ImpactEntity::AdapterImport));
    borrowed.extend(entities.entry_points.iter().map(ImpactEntity::EntryPoint));
    borrowed.extend(
        entities
            .policy_bindings
            .iter()
            .map(ImpactEntity::PolicyBinding),
    );
    borrowed.extend(
        entities
            .dependency_bindings
            .iter()
            .map(ImpactEntity::DependencyBinding),
    );
    borrowed.sort_by_key(|entity| entity.entity_id());
    borrowed
}

#[cfg(test)]
mod tests {
    use std::fs;

    use sley_id::{EntityId, PrincipalId, StateRoot, WorkspaceId};
    use sley_mutate::{
        EntityObject, EntityObjectRecord, MutationClass, build_entity_object,
        value::{
            DependencyBindingBody, EntityBodyValue, EntityIdSet, NamespaceBody, PackageBody,
            PolicyBindingBody, TypeDefBody, WorkspaceBody,
        },
    };
    use sley_policy::{
        PolicyResourceCeilings, PolicyRootBuilder, PrincipalGrantBuilder,
        conformance_registry as policy_registry,
    };
    use sley_ssmc::{TypeDefForm, Visibility};
    use sley_state_root::{
        StateRootBuilder, conformance_epoch_id as state_epoch_id,
        conformance_registry as state_registry,
    };
    use sley_store::ObjectStore;
    use sley_txn::{TransactionRepository, TrustedGenesisInput};

    use super::*;
    use crate::exchange::tests::{TempDir, fixed, namespace_body, verifier};

    fn set(ids: &[u8]) -> EntityIdSet {
        EntityIdSet::from_unsorted(
            ids.iter()
                .map(|byte| fixed(*byte, EntityId::from_bytes))
                .collect(),
        )
        .unwrap()
    }

    fn id(byte: u8) -> EntityId {
        fixed(byte, EntityId::from_bytes)
    }

    /// A trusted genesis whose root binds the given bodies (by entity byte)
    /// and carries the given dependency roots.
    fn genesis(
        label: &str,
        bodies: Vec<(u8, EntityBodyValue)>,
        dependency_roots: &[StateRoot],
    ) -> (TempDir, TransactionRepository, sley_id::TransactionId) {
        let temp = TempDir::new(label);
        let root = temp.child("repo");
        fs::create_dir(&root).unwrap();
        let workspace_id = fixed(1, WorkspaceId::from_bytes);
        let principal_id = fixed(2, PrincipalId::from_bytes);
        let grant = PrincipalGrantBuilder::new(PolicyResourceCeilings::new(
            1_000, 1_000, 1_000, 100, 100, 100,
        ))
        .mutation_class(MutationClass::CreateEntity)
        .build()
        .unwrap();
        let policy = PolicyRootBuilder::new(workspace_id)
            .principal_grant(principal_id, grant)
            .build(&policy_registry().unwrap())
            .unwrap();
        let epoch = state_epoch_id().unwrap();
        let store = ObjectStore::new(&root);
        let anchors = [20_u8, 21_u8].map(|byte| {
            let object = build_entity_object(
                epoch,
                &EntityObjectRecord {
                    entity_id: id(byte),
                    body: namespace_body(),
                    label: None,
                    semantic_fingerprint: None,
                },
            )
            .unwrap();
            store
                .put(object.object_id(), object.stored_bytes(), &verifier(epoch))
                .unwrap();
            object.object_id()
        });
        let mut objects: Vec<EntityObject> = bodies
            .into_iter()
            .map(|(byte, body)| {
                build_entity_object(
                    epoch,
                    &EntityObjectRecord {
                        entity_id: id(byte),
                        body,
                        label: None,
                        semantic_fingerprint: None,
                    },
                )
                .unwrap()
            })
            .collect();
        objects.sort_by_key(|object| object.record().entity_id);
        let mut builder =
            StateRootBuilder::new(workspace_id, anchors[0], anchors[1], policy.root());
        for object in &objects {
            builder = builder.entity_binding(object.record().entity_id, object.object_id());
        }
        for dependency_root in dependency_roots {
            builder = builder.dependency_root(*dependency_root);
        }
        let state = builder.build(&state_registry().unwrap()).unwrap();
        let transactions = TransactionRepository::new(&root);
        let genesis = transactions
            .initialize_trusted_genesis(TrustedGenesisInput::new(&state, &policy, &objects, &[]))
            .unwrap()
            .transaction_id();
        (temp, transactions, genesis)
    }

    fn complete_bodies() -> Vec<(u8, EntityBodyValue)> {
        vec![
            (
                1,
                EntityBodyValue::Workspace(WorkspaceBody {
                    packages: set(&[3]),
                    root_namespace: id(2),
                    capability_requirements: set(&[]),
                    contracts: set(&[]),
                    tests: set(&[]),
                }),
            ),
            (
                2,
                EntityBodyValue::Namespace(NamespaceBody {
                    parent: None,
                    members: set(&[]),
                }),
            ),
            (
                3,
                EntityBodyValue::Package(PackageBody {
                    workspace: id(1),
                    root_namespace: id(4),
                    dependencies: set(&[17]),
                    exports: set(&[6]),
                }),
            ),
            (
                4,
                EntityBodyValue::Namespace(NamespaceBody {
                    parent: None,
                    members: set(&[6, 16]),
                }),
            ),
            (
                6,
                EntityBodyValue::TypeDef(TypeDefBody {
                    type_parameters: Vec::new(),
                    form: TypeDefForm::Record(Vec::new()),
                    invariants: set(&[]),
                    visibility: Visibility::Private,
                }),
            ),
            (
                16,
                EntityBodyValue::PolicyBinding(PolicyBindingBody {
                    subject: id(6),
                    requirements: set(&[]),
                }),
            ),
            (
                17,
                EntityBodyValue::DependencyBinding(DependencyBindingBody {
                    dependency_root: fixed(9, StateRoot::from_bytes),
                    external_package: id(99),
                    local_namespace: id(4),
                }),
            ),
        ]
    }

    #[test]
    fn complete_root_of_a_verified_revision_is_judged_and_indexed() {
        let (_temp, transactions, genesis) = genesis(
            "complete-root",
            complete_bodies(),
            &[fixed(9, StateRoot::from_bytes)],
        );
        let revision = transactions.verified_revision(genesis).unwrap();
        let judged = judge_complete_root_revision(&revision).unwrap();
        assert_eq!(judged.workspace(), id(1));
        assert_eq!(judged.bound_entities(), 7);
        assert_eq!(judged.packages(), 1);
        assert_eq!(judged.namespaces(), 2);
        let request = CompleteRootRequest::extract(&revision).unwrap();
        assert_eq!(request.entities().len(), 7);
        assert_eq!(request.facts().entry_points, &[] as &[EntityId]);
        assert_eq!(
            request.facts().dependency_roots,
            &[fixed(9, StateRoot::from_bytes)]
        );
        // Determinism over repeated extraction.
        assert_eq!(request.judge().unwrap(), judged);
    }

    #[test]
    fn validator_reference_graph_and_impact_index_agree_edge_for_edge() {
        let (_temp, transactions, genesis) = genesis(
            "edge-agreement",
            complete_bodies(),
            &[fixed(9, StateRoot::from_bytes)],
        );
        let revision = transactions.verified_revision(genesis).unwrap();
        let request = CompleteRootRequest::extract(&revision).unwrap();
        let judged = request.judge().unwrap();
        let impact: Vec<(EntityId, EntityId, u32)> = judged
            .index()
            .direct_edges()
            .iter()
            .map(|edge| (edge.dependent, edge.dependency, edge.kind.tag()))
            .collect();
        let mut reference = request.entities().reference_edges.clone();
        reference.sort_unstable();
        assert_eq!(impact, reference, "S20-360 graph and S20-250 index differ");
        assert_eq!(impact.len(), 10);
    }

    #[test]
    fn revision_without_a_workspace_fails_closed_with_the_closure_code() {
        let (_temp, transactions, genesis) = genesis(
            "no-workspace",
            vec![(
                2,
                EntityBodyValue::Namespace(NamespaceBody {
                    parent: None,
                    members: set(&[]),
                }),
            )],
            &[],
        );
        let revision = transactions.verified_revision(genesis).unwrap();
        let error = judge_complete_root_revision(&revision).unwrap_err();
        assert_eq!(error.code(), "IMPACT_ROOT_WORKSPACE_MISSING");
        assert_eq!(error.numeric(), 25_015);
    }

    #[test]
    fn revision_whose_dependency_roots_disagree_with_bindings_fails_closed() {
        let (_temp, transactions, genesis) = genesis("root-mismatch", complete_bodies(), &[]);
        let revision = transactions.verified_revision(genesis).unwrap();
        let error = judge_complete_root_revision(&revision).unwrap_err();
        assert_eq!(error.code(), "IMPACT_ROOT_DEPENDENCY_ROOTS_MISMATCH");
    }
}
