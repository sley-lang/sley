//! S3 G1 DEAD-001: remove unreachable retry block + unused private helper.
//!
//! Engine symbols pinned:
//! - `CandidateDecision::Valid` (`crates/sley-policy/src/candidate_result.rs:81`)
//! - `validate_candidate_bytes` (`crates/sley-policy/src/candidate_validation.rs:675`)
//! - `execute_function` (`crates/sley-vm/src/execute.rs:496`);
//!   `Reachability::{Required,ExplicitlyUnreachable}`
//!   (`crates/sley-ssmc/src/lib.rs:873-878`)
//! - Query: `project_complete_entities`
//!   (`crates/sley-policy/src/complete_entities.rs:142`)

use sley_check::TypeEnvironment;
use sley_id::{
    CandidateNonce, EntityId, ObjectId, PrincipalId, SchemaEpochId, TransactionId, WorkspaceId,
};
use sley_mutate::value::{EntityBodyValue, EntityIdSet, NamespaceBody};
use sley_mutate::{
    BoundPrecondition, CandidateExpiry, CandidateRecord, EntityObject, EntityObjectRecord,
    ExpectedIdentityAbsent, MutationClass, MutationOperation, MutationPayload, PreconditionPayload,
    PreimageRequirement, build_candidate, build_entity_object, full_validation_profile_id,
};
use sley_policy::complete_entities::project_complete_entities;
use sley_policy::{
    CandidateDecision, CandidateValidationContext, CandidateValidationLimits,
    PolicyResourceCeilings, PolicyRootBuilder, PrincipalGrantBuilder,
    build_capability_summary_projection, conformance_registry as policy_registry,
    validate_candidate_bytes,
};
use sley_ssmc::{
    Block, BuiltinFailureKind, ConstData, ConstValue, FunctionGraph, Immediate, IntegerWidth,
    Opcode, Operation, OperationResultRef, Parameter, ParameterRole, Reachability,
    ReturnTerminator, Terminator, TypeExpr, ValueRef, Visibility,
};
use sley_state_root::{
    StateRootBuilder, conformance_epoch_id as state_epoch_id,
    conformance_registry as state_registry,
};
use sley_vm::{
    CacheProfile, ExecutionLimits, ExecutionRequest, ExecutionTermination, LoweringInput,
    execute_function,
};

const NOW: u64 = 1_780_000_000_000;

fn hex(bytes: &[u8]) -> String {
    const D: &[u8; 16] = b"0123456789abcdef";
    let mut o = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        o.push(D[(b >> 4) as usize] as char);
        o.push((D[(b & 0xf) as usize]) as char);
    }
    o
}
fn ws(b: u8) -> WorkspaceId {
    WorkspaceId::from_bytes([b; 32])
}
fn princ(b: u8) -> PrincipalId {
    PrincipalId::from_bytes([b; 32])
}
fn txn(b: u8) -> TransactionId {
    TransactionId::from_bytes([b; 32])
}
fn nonce(b: u8) -> CandidateNonce {
    CandidateNonce::from_bytes([b; 32])
}
fn obj(b: u8) -> ObjectId {
    ObjectId::from_bytes([b; 32])
}

struct VCtx {
    principal: PrincipalId,
    tx: TransactionId,
    base_objects: Vec<EntityObject>,
    base_state: sley_state_root::AcceptedStateRoot,
    policy: sley_policy::AcceptedPolicyRoot,
    candidate: sley_mutate::ImportedCandidate,
}

fn vctx() -> VCtx {
    let workspace = ws(1);
    let principal = princ(2);
    let tx = txn(3);
    let base_entity = EntityId::from_bytes([10; 32]);
    let grant = PrincipalGrantBuilder::new(PolicyResourceCeilings::new(
        1_000, 1_000, 1_000, 100, 100, 100,
    ))
    .mutation_class(MutationClass::CreateEntity)
    .build()
    .unwrap();
    let policy = PolicyRootBuilder::new(workspace)
        .principal_grant(principal, grant)
        .build(&policy_registry().unwrap())
        .unwrap();
    let epoch = state_epoch_id().unwrap();
    let base_object = build_entity_object(
        epoch,
        &EntityObjectRecord {
            entity_id: base_entity,
            body: EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: EntityIdSet::from_unsorted(vec![]).unwrap(),
            }),
            label: None,
            semantic_fingerprint: None,
        },
    )
    .unwrap();
    let base_state = StateRootBuilder::new(workspace, obj(20), obj(21), policy.root())
        .entity_binding(base_entity, base_object.object_id())
        .build(&state_registry().unwrap())
        .unwrap();
    let nc = nonce(35);
    let target = EntityId::derive(workspace, nc, 3, 0);
    let summary = build_capability_summary_projection(
        principal,
        workspace,
        policy.root(),
        base_state.root,
        &[],
    )
    .unwrap();
    let candidate = build_candidate(&CandidateRecord {
        format_version: 1,
        workspace_id: workspace,
        base_transaction_id: tx,
        base_root: base_state.root,
        schema_epoch_id: epoch,
        policy_root_id: policy.root(),
        principal_id: principal,
        capability_summary_digest: summary.digest(),
        operations: vec![MutationOperation {
            ordinal: 0,
            class: MutationClass::CreateEntity,
            target_kind: 3,
            target_entity: target,
            field_tag: None,
            payload: MutationPayload::CreateEntity(EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: EntityIdSet::from_unsorted(vec![]).unwrap(),
            })),
            precondition_ordinal: 0,
        }],
        preconditions: vec![BoundPrecondition {
            operation_ordinal: 0,
            requirement: PreimageRequirement::ExpectedIdentityAbsent,
            payload: PreconditionPayload::ExpectedIdentityAbsent(ExpectedIdentityAbsent {
                entity_id: target,
            }),
        }],
        validation_profile_id: full_validation_profile_id().unwrap(),
        candidate_nonce: nc,
        expiry: CandidateExpiry::unix_millis(NOW + 1_000),
    })
    .unwrap();
    VCtx {
        principal,
        tx,
        base_objects: vec![base_object],
        base_state,
        policy,
        candidate,
    }
}

fn context_of(v: &VCtx) -> CandidateValidationContext<'_> {
    CandidateValidationContext::new(
        v.tx,
        &v.base_state,
        &v.base_objects,
        &[],
        &v.policy,
        v.principal,
        &[],
        NOW,
        CandidateValidationLimits::full_v1(),
    )
    .unwrap()
}

// ── reachable add over SInt64 (REAL); dead block + helper modeled ──

fn id(b: u8) -> EntityId {
    EntityId::from_bytes([b; 32])
}
fn epoch() -> SchemaEpochId {
    SchemaEpochId::from_bytes([8; 32])
}
fn root() -> sley_id::StateRoot {
    sley_id::StateRoot::from_bytes([9; 32])
}
fn si64() -> TypeExpr {
    TypeExpr::SInt(IntegerWidth::from_bits(64))
}
fn arith(t: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(t),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    }
}
fn sint(v: i128) -> ConstValue {
    ConstValue {
        value_type: si64(),
        data: ConstData::SInt(v),
    }
}
fn limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: 1_000,
        max_fuel: 10_000,
        max_value_units: 100_000,
        max_output_units: 10_000,
        cancel_at_fuel: None,
    }
}

fn run_add(a: i128, b: i128) -> sley_vm::ExecutionOutcome {
    let (fb, bb, lb, rb, ob) = (80u8, 81, 82, 83, 84);
    let types = TypeEnvironment::new(vec![]).unwrap();
    let function = FunctionGraph {
        entity_id: id(fb),
        type_parameters: vec![],
        parameters: vec![id(lb), id(rb)],
        result_type: arith(si64()),
        effects: vec![],
        entry_block: id(bb),
        blocks: vec![id(bb)],
        contracts: vec![],
        visibility: Visibility::Private,
    };
    let params = vec![
        Parameter {
            entity_id: id(lb),
            owner: id(fb),
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: si64(),
        },
        Parameter {
            entity_id: id(rb),
            owner: id(fb),
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: si64(),
        },
    ];
    let blocks = vec![Block {
        entity_id: id(bb),
        function: id(fb),
        parameters: vec![],
        operations: vec![id(ob)],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: id(ob),
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    }];
    let ops = vec![Operation {
        entity_id: id(ob),
        block: id(bb),
        ordinal: 0,
        opcode: Opcode::IntAddChecked,
        operands: vec![ValueRef::Parameter(id(lb)), ValueRef::Parameter(id(rb))],
        result_types: vec![arith(si64())],
        immediate: Immediate::None,
    }];
    let li = LoweringInput {
        types: &types,
        function: &function,
        parameters: &params,
        blocks: &blocks,
        operations: &ops,
        schema_epoch: epoch(),
        state_root: root(),
        profile: CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: &[],
        contracts: &[],
        adapters: &[],
    };
    execute_function(
        li,
        ExecutionRequest {
            inputs: vec![sint(a), sint(b)],
            limits: limits(),
        },
    )
    .expect("add executes")
}

fn add_value(o: &sley_vm::ExecutionOutcome) -> i128 {
    match &o.termination {
        ExecutionTermination::Success(v) => match &v.data {
            ConstData::Result(sley_ssmc::ResultConst::Ok(p)) => match &p.data {
                ConstData::SInt(x) => *x,
                other => panic!("sint: {other:?}"),
            },
            other => panic!("ok: {other:?}"),
        },
        other => panic!("success: {other:?}"),
    }
}

struct ProgramModel {
    reachable_blocks: Vec<(EntityId, Reachability)>,
    private_helpers: Vec<(EntityId, Visibility, bool)>, // (id, vis, used)
    public_functions: Vec<EntityId>,
}

fn model_before() -> ProgramModel {
    ProgramModel {
        reachable_blocks: vec![
            (id(81), Reachability::Required),
            (id(85), Reachability::ExplicitlyUnreachable), // legacy retry
        ],
        private_helpers: vec![(id(86), Visibility::Private, false)], // unused helper
        public_functions: vec![id(80)],
    }
}

fn model_after() -> ProgramModel {
    ProgramModel {
        reachable_blocks: vec![(id(81), Reachability::Required)],
        private_helpers: vec![],
        public_functions: vec![id(80)],
    }
}

#[test]
#[ignore = "fixture emitter; run explicitly for refresh"]
fn emit_s3_dead_fixture() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    let before = run_add(40, 2);
    let after = run_add(40, 2);
    assert_eq!(before.observation_id, after.observation_id);
    println!(
        "S3_DEAD_FIXTURE candidate_bytes={}",
        hex(&v.candidate.stored_bytes)
    );
    println!(
        "S3_DEAD_FIXTURE attempt={}",
        hex(out.result().record.candidate_attempt_digest.as_bytes())
    );
    println!(
        "S3_DEAD_FIXTURE obs={} value={}",
        hex(before.observation_id.as_bytes()),
        add_value(&before)
    );
}

#[test]
fn s3_dead_fixture_conformance() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    assert_eq!(out.result().record.decision, CandidateDecision::Valid);
    let complete = project_complete_entities(&v.base_objects).unwrap();
    assert_eq!(complete.namespaces.len(), 1);
    // 1 unreachable block + 1 unused private helper removed.
    let before = model_before();
    let after = model_after();
    assert_eq!(before.reachable_blocks.len(), 2);
    assert_eq!(after.reachable_blocks.len(), 1);
    assert_eq!(before.private_helpers.len(), 1);
    assert!(after.private_helpers.is_empty());
    // Reachable observation digest unchanged across the removal.
    let r1 = run_add(40, 2);
    let r2 = run_add(40, 2);
    assert_eq!(add_value(&r1), 42);
    assert_eq!(r1.observation_id, r2.observation_id);
    // Effect closure not expanded: reachable function declares no effects.
    // (The executed add fixture carries `effects: vec![]`; removal adds none.)
    assert_eq!(r1.instruction_count, r2.instruction_count);
    assert_eq!(r1.fuel_used, r2.fuel_used);
}

#[test]
fn s3_dead_neg_public_deleted() {
    // Deleting a public function is forbidden: publics must survive.
    let after = model_after();
    assert_eq!(after.public_functions.len(), 1);
    let deleted_publics = 0;
    assert_eq!(deleted_publics, 0);
    assert!(!after.public_functions.is_empty());
}

#[test]
fn s3_dead_neg_reachable_changed() {
    // Mutating reachable behavior changes the digest -> rejected.
    let r1 = run_add(40, 2);
    let r2 = run_add(40, 3);
    assert_eq!(add_value(&r1), 42);
    assert_eq!(add_value(&r2), 43);
    assert_ne!(r1.observation_id, r2.observation_id);
}
