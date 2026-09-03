//! Test-support mirror of the repository fixtures (feature `test-support`).
//!
//! Downstream crates (the SMP1 server, the CLI) build trusted genesis
//! repositories in their tests through this module. It mirrors the private
//! fixtures of the `complete_root` and `exchange` test modules and grants no
//! runtime authority: it exists only under `cfg(test)` or the explicit
//! `test-support` feature.

#![allow(missing_docs, clippy::missing_panics_doc, clippy::must_use_candidate)]

use std::fs;
use std::path::PathBuf;

use sley_id::{EntityId, ObjectId, PrincipalId, SchemaEpochId, StateRoot, WorkspaceId};
use sley_mutate::{
    EntityObject, EntityObjectRecord, MutationClass, build_entity_object, import_entity_object,
    value::{
        DependencyBindingBody, EntityBodyValue, EntityIdSet, NamespaceBody, PackageBody,
        PolicyBindingBody, TypeDefBody, WorkspaceBody,
    },
};
use sley_policy::{
    PolicyResourceCeilings, PolicyRootBuilder, PrincipalGrantBuilder,
    conformance_registry as policy_registry,
};
use sley_scb1::ScbError;
use sley_ssmc::{TypeDefForm, Visibility};
use sley_state_root::{
    StateRootBuilder, conformance_epoch_id as state_epoch_id,
    conformance_registry as state_registry,
};
use sley_store::ObjectStore;
use sley_txn::{TransactionRepository, TrustedGenesisInput};

static TEMP_DIR_COUNTER: ::std::sync::atomic::AtomicU64 = ::std::sync::atomic::AtomicU64::new(0);

/// A process-unique temporary directory removed on drop.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> Self {
        let sequence = TEMP_DIR_COUNTER.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed);
        let path = ::std::env::temp_dir().join(format!(
            "sley-support-{label}-{}-{sequence:016x}",
            ::std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }

    pub fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn fixed<T>(byte: u8, constructor: impl FnOnce([u8; 32]) -> T) -> T {
    constructor([byte; 32])
}

pub fn id(byte: u8) -> EntityId {
    fixed(byte, EntityId::from_bytes)
}

pub fn set(ids: &[u8]) -> EntityIdSet {
    EntityIdSet::from_unsorted(ids.iter().map(|byte| id(*byte)).collect()).unwrap()
}

pub fn namespace_body() -> EntityBodyValue {
    EntityBodyValue::Namespace(NamespaceBody {
        parent: None,
        members: EntityIdSet::from_unsorted(vec![]).unwrap(),
    })
}

/// The canonical object verifier over one schema epoch.
pub fn verifier(
    epoch: SchemaEpochId,
) -> impl Fn(&[u8]) -> core::result::Result<ObjectId, ScbError> {
    move |bytes| import_entity_object(epoch, bytes).map(|object| object.object_id())
}

/// The frozen state schema epoch identity.
pub fn epoch() -> SchemaEpochId {
    state_epoch_id().unwrap()
}

/// A trusted genesis whose root binds the given bodies (by entity byte) and
/// carries the given dependency roots.
pub fn genesis(
    label: &str,
    bodies: Vec<(u8, EntityBodyValue)>,
    dependency_roots: &[StateRoot],
) -> (TempDir, TransactionRepository, sley_id::TransactionId) {
    genesis_in_workspace(label, bodies, dependency_roots, 1)
}

/// A trusted genesis under a chosen workspace identity byte.
pub fn genesis_in_workspace(
    label: &str,
    bodies: Vec<(u8, EntityBodyValue)>,
    dependency_roots: &[StateRoot],
    workspace_byte: u8,
) -> (TempDir, TransactionRepository, sley_id::TransactionId) {
    let temp = TempDir::new(label);
    let root = temp.child("repo");
    fs::create_dir(&root).unwrap();
    let workspace_id = fixed(workspace_byte, WorkspaceId::from_bytes);
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
    let epoch = epoch();
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
    let mut builder = StateRootBuilder::new(workspace_id, anchors[0], anchors[1], policy.root());
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

/// The seven-body complete root used across repository tests.
pub fn complete_bodies() -> Vec<(u8, EntityBodyValue)> {
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

/// The dependency root the complete bodies bind.
pub fn complete_dependency_root() -> StateRoot {
    fixed(9, StateRoot::from_bytes)
}

/// The complete bodies plus one executable Function (entity 30: `BoolAnd` over
/// two Bool parameters 31 and 32 in block 33 with operation 34), for the
/// SMP1 `execute` path.
#[must_use]
pub fn executable_bodies() -> Vec<(u8, EntityBodyValue)> {
    use sley_mutate::value::{BlockBody, EntityIdSet, FunctionBody, OperationBody, ParameterBody};
    use sley_ssmc::{
        Immediate, Opcode, OperationResultRef, ParameterRole, Reachability, ReturnTerminator,
        Terminator, TypeExpr, ValueRef,
    };
    let mut bodies = complete_bodies();
    let function = id(30);
    let left = id(31);
    let right = id(32);
    let block = id(33);
    let operation = id(34);
    let empty = EntityIdSet::from_unsorted(Vec::new()).unwrap();
    bodies.push((
        30,
        EntityBodyValue::Function(FunctionBody {
            type_parameters: Vec::new(),
            parameters: vec![left, right],
            result_type: TypeExpr::Bool,
            effects: empty.clone(),
            entry_block: block,
            blocks: vec![block],
            contracts: empty,
            visibility: Visibility::Private,
        }),
    ));
    for (byte, ordinal) in [(31, 0), (32, 1)] {
        bodies.push((
            byte,
            EntityBodyValue::Parameter(ParameterBody {
                owner: function,
                role: ParameterRole::Function,
                ordinal,
                value_type: TypeExpr::Bool,
            }),
        ));
    }
    bodies.push((
        33,
        EntityBodyValue::Block(BlockBody {
            function,
            parameters: Vec::new(),
            operations: vec![operation],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        }),
    ));
    bodies.push((
        34,
        EntityBodyValue::Operation(OperationBody {
            block,
            ordinal: 0,
            opcode: Opcode::BoolAnd.tag(),
            operands: vec![ValueRef::Parameter(left), ValueRef::Parameter(right)],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        }),
    ));
    bodies
}
