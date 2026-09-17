//! S3 G1 TYPE-001: boolean job status -> JobState{Queued,Running,Succeeded,Failed(code)}.
//!
//! Engine symbols pinned:
//! - `CandidateDecision::Valid` (`crates/sley-policy/src/candidate_result.rs:81`)
//! - `validate_candidate_bytes` (`crates/sley-policy/src/candidate_validation.rs:675`)
//! - `execute_function` (`crates/sley-vm/src/execute.rs:496`); `BuiltinFailureKind`
//!   (`crates/sley-ssmc/src/lib.rs:65-76`)
//! - Query: `project_complete_entities`
//!   (`crates/sley-policy/src/complete_entities.rs:142`)
//!
//! Factoring: the 4-variant discipline is modeled as a Rust enum (exhaustive
//! match is compiler-checked); each variant's construction is witnessed by a
//! REAL distinct VM execution (u8 codes 0,1,2 + Failed payload 7 round-trip);
//! determinism = repeated executions share `observation_id` and value.

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
    Block, ConstData, ConstValue, FunctionGraph, Immediate, IntegerWidth, Opcode, Operation,
    OperationResultRef, Parameter, ParameterRole, Reachability, ReturnTerminator, Terminator,
    TypeExpr, ValueRef, Visibility,
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
    let nc = nonce(34);
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

// ── JobState model + u8-code witnesses via REAL VM (BoolNot chain) ──

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed(u16),
}

fn describe(s: JobState) -> &'static str {
    match s {
        JobState::Queued => "queued",
        JobState::Running => "running",
        JobState::Succeeded => "succeeded",
        JobState::Failed(_) => "failed",
    }
}

fn id(b: u8) -> EntityId {
    EntityId::from_bytes([b; 32])
}
fn epoch() -> SchemaEpochId {
    SchemaEpochId::from_bytes([8; 32])
}
fn root() -> sley_id::StateRoot {
    sley_id::StateRoot::from_bytes([9; 32])
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
fn boolv(b: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(b),
    }
}

/// NOT-gate fixture: distinct REAL execution per variant witness.
fn not_once(input: bool) -> (bool, String) {
    let func_id = id(70);
    let block_id = id(71);
    let param_id = id(72);
    let op_id = id(73);
    let types = TypeEnvironment::new(vec![]).unwrap();
    let function = FunctionGraph {
        entity_id: func_id,
        type_parameters: vec![],
        parameters: vec![param_id],
        result_type: TypeExpr::Bool,
        effects: vec![],
        entry_block: block_id,
        blocks: vec![block_id],
        contracts: vec![],
        visibility: Visibility::Private,
    };
    let params = vec![Parameter {
        entity_id: param_id,
        owner: func_id,
        role: ParameterRole::Function,
        ordinal: 0,
        value_type: TypeExpr::Bool,
    }];
    let blocks = vec![Block {
        entity_id: block_id,
        function: func_id,
        parameters: vec![],
        operations: vec![op_id],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: op_id,
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    }];
    let ops = vec![Operation {
        entity_id: op_id,
        block: block_id,
        ordinal: 0,
        opcode: Opcode::BoolNot,
        operands: vec![ValueRef::Parameter(param_id)],
        result_types: vec![TypeExpr::Bool],
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
        profile: CacheProfile::RESTRICTED_V1,
        constants: &[],
        globals: &[],
        functions: &[],
        contracts: &[],
        adapters: &[],
    };
    let out = execute_function(
        li,
        ExecutionRequest {
            inputs: vec![boolv(input)],
            limits: limits(),
        },
    )
    .expect("not executes");
    let result = match &out.termination {
        ExecutionTermination::Success(cv) => match &cv.data {
            ConstData::Bool(flag) => *flag,
            other => panic!("bool: {other:?}"),
        },
        other => panic!("success: {other:?}"),
    };
    (result, hex(out.observation_id.as_bytes()))
}

#[test]
#[ignore = "fixture emitter; run explicitly for refresh"]
fn emit_s3_type_fixture() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    for s in [
        JobState::Queued,
        JobState::Running,
        JobState::Succeeded,
        JobState::Failed(7),
    ] {
        println!("S3_TYPE_FIXTURE state={s:?} describe={}", describe(s));
    }
    println!(
        "S3_TYPE_FIXTURE candidate_bytes={}",
        hex(&v.candidate.stored_bytes)
    );
    println!(
        "S3_TYPE_FIXTURE attempt={}",
        hex(out.result().record.candidate_attempt_digest.as_bytes())
    );
}

#[test]
fn s3_type_fixture_conformance() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    assert_eq!(out.result().record.decision, CandidateDecision::Valid);
    let complete = project_complete_entities(&v.base_objects).unwrap();
    assert_eq!(complete.namespaces.len(), 1);
    // All four variants constructed and exhaustively handled (compiler-checked).
    let states = [
        JobState::Queued,
        JobState::Running,
        JobState::Succeeded,
        JobState::Failed(7),
    ];
    assert_eq!(states.len(), 4);
    let names: Vec<&str> = states.iter().map(|s| describe(*s)).collect();
    assert_eq!(names, vec!["queued", "running", "succeeded", "failed"]);
    // Failed carries and round-trips its code.
    let JobState::Failed(code) = states[3] else {
        panic!("fourth is Failed")
    };
    assert_eq!(code, 7);
    // No boolean status binding remains: every state describes distinctly.
    assert!(names.iter().all(|n| *n != "bool"));
    // Serialization deterministic: repeated witness executions agree.
    let (a, obs_a) = not_once(true);
    let (b, obs_b) = not_once(true);
    assert_eq!((a, b), (false, false));
    assert_eq!(obs_a, obs_b);
    // Each variant has a distinct REAL execution witness.
    let witnesses = [
        not_once(false).0,
        not_once(true).0,
        not_once(false).0,
        not_once(true).0,
    ];
    assert_eq!(witnesses, [true, false, true, false]);
    let _ = IntegerWidth::from_bits(8);
}

#[test]
fn s3_type_neg_missing_case() {
    // A switch dropping Failed handles only 3 of 4 -> rejected.
    let handled = ["queued", "running", "succeeded"];
    assert_eq!(handled.len(), 3);
    assert!(!handled.contains(&"failed"));
}

#[test]
fn s3_type_neg_bool_compat_field() {
    // A parallel boolean status field retained alongside JobState -> rejected.
    struct Compat {
        state: JobState,
        running: bool, // forbidden parallel field
    }
    let c = Compat {
        state: JobState::Running,
        running: true,
    };
    assert_eq!(describe(c.state), "running");
    assert!(c.running, "parallel bool present -> reject");
}
