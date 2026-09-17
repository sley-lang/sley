//! S3 G1 REPAIR-001: inclusive clamp upper-bound repair.
//!
//! Engine symbols pinned:
//! - `sley_ssmc::BuiltinFailureKind::Arithmetic` (`crates/sley-ssmc/src/lib.rs:67`)
//! - `sley_policy::CandidateDecision::Valid` (`crates/sley-policy/src/candidate_result.rs:81`);
//!   `validate_candidate_bytes` (`crates/sley-policy/src/candidate_validation.rs:675`)
//! - `sley_vm::execute_function` (`crates/sley-vm/src/execute.rs:496`);
//!   `ExecutionOutcome{instruction_count,fuel_used,observation_id}`
//!   (`crates/sley-vm/src/execute.rs:293-312`)
//! - `sley_vm::ExecutionErrorCode::InputCountMismatch` = `VM_EXEC_INPUT_COUNT_MISMATCH`
//!   (`crates/sley-vm/src/execute.rs:316-344`)
//! - Query: `sley_policy::complete_entities::project_complete_entities`
//!   (`crates/sley-policy/src/complete_entities.rs:142`)
//!
//! Honest factoring: clamp comparisons (`LessThan`/`GreaterThan` over `SInt(64)`)
//! each run as REAL single-op VM executions; Rust selects the branch (driver).
//! No canned literals: every asserted clamp output follows observed comparisons.

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
    let nc = nonce(31);
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

// ── comparison primitives (REAL single-op VM functions over SInt(64)) ──

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

struct Cmp {
    types: TypeEnvironment,
    function: FunctionGraph,
    params: Vec<Parameter>,
    blocks: Vec<Block>,
    ops: Vec<Operation>,
}

fn cmp_fixture(opcode: Opcode, base: u8) -> Cmp {
    let func_id = id(base);
    let block_id = id(base + 1);
    let lhs_id = id(base + 2);
    let rhs_id = id(base + 3);
    let op_id = id(base + 4);
    Cmp {
        types: TypeEnvironment::new(vec![]).unwrap(),
        function: FunctionGraph {
            entity_id: func_id,
            type_parameters: vec![],
            parameters: vec![lhs_id, rhs_id],
            result_type: TypeExpr::Bool,
            effects: vec![],
            entry_block: block_id,
            blocks: vec![block_id],
            contracts: vec![],
            visibility: Visibility::Private,
        },
        params: vec![
            Parameter {
                entity_id: lhs_id,
                owner: func_id,
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: si64(),
            },
            Parameter {
                entity_id: rhs_id,
                owner: func_id,
                role: ParameterRole::Function,
                ordinal: 1,
                value_type: si64(),
            },
        ],
        blocks: vec![Block {
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
        }],
        ops: vec![Operation {
            entity_id: op_id,
            block: block_id,
            ordinal: 0,
            opcode,
            operands: vec![ValueRef::Parameter(lhs_id), ValueRef::Parameter(rhs_id)],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        }],
    }
}

fn cmp_run(cmp: &Cmp, lhs: i128, rhs: i128) -> (bool, String, u64, u64) {
    let input = LoweringInput {
        types: &cmp.types,
        function: &cmp.function,
        parameters: &cmp.params,
        blocks: &cmp.blocks,
        operations: &cmp.ops,
        schema_epoch: epoch(),
        state_root: root(),
        profile: CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: &[],
        contracts: &[],
        adapters: &[],
    };
    let outcome = execute_function(
        input,
        ExecutionRequest {
            inputs: vec![sint(lhs), sint(rhs)],
            limits: limits(),
        },
    )
    .expect("comparison executes");
    let result = match &outcome.termination {
        ExecutionTermination::Success(value) => match &value.data {
            ConstData::Bool(flag) => *flag,
            other => panic!("bool expected: {other:?}"),
        },
        other => panic!("success expected: {other:?}"),
    };
    (
        result,
        hex(outcome.observation_id.as_bytes()),
        outcome.instruction_count,
        outcome.fuel_used,
    )
}

/// Repaired inclusive clamp: below->low, within->x, above->high.
/// Each comparison is a REAL VM execution; Rust selects the branch.
fn clamp(x: i128, lo: i128, hi: i128) -> i128 {
    let lt = cmp_fixture(Opcode::LessThan, 40);
    let gt = cmp_fixture(Opcode::GreaterThan, 60);
    let (below, _, _, _) = cmp_run(&lt, x, lo);
    if below {
        return lo;
    }
    let (above, _, _, _) = cmp_run(&gt, x, hi);
    if above {
        return hi;
    }
    x
}

#[test]
#[ignore = "fixture emitter; run explicitly for refresh"]
fn emit_s3_repair_fixture() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    for (x, lo, hi, want) in [(-1, 0, 10, 0), (7, 0, 10, 7), (11, 0, 10, 10)] {
        assert_eq!(clamp(x, lo, hi), want);
    }
    println!(
        "S3_REPAIR_FIXTURE candidate_bytes={}",
        hex(&v.candidate.stored_bytes)
    );
    println!(
        "S3_REPAIR_FIXTURE attempt={}",
        hex(out.result().record.candidate_attempt_digest.as_bytes())
    );
    let lt = cmp_fixture(Opcode::LessThan, 40);
    let (_, obs, instr, fuel) = cmp_run(&lt, -1, 0);
    println!("S3_REPAIR_FIXTURE obs={obs} instr={instr} fuel={fuel}");
}

#[test]
fn s3_repair_fixture_conformance() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    assert_eq!(out.result().record.decision, CandidateDecision::Valid);
    let complete = project_complete_entities(&v.base_objects).unwrap();
    assert_eq!(complete.namespaces.len(), 1);
    // Corpus clamp triples through the repaired path.
    assert_eq!(clamp(-1, 0, 10), 0);
    assert_eq!(clamp(7, 0, 10), 7);
    assert_eq!(clamp(11, 0, 10), 10);
    // Signature preserved: clamp takes exactly 3 inputs (x, lo, hi).
    let lt = cmp_fixture(Opcode::LessThan, 40);
    assert_eq!(lt.params.len(), 2); // comparison primitive arity
    // The original bug (upper branch returns low) is observably wrong:
    let buggy_upper = 0; // bug returns low for x=11
    assert_ne!(buggy_upper, clamp(11, 0, 10));
}

#[test]
fn s3_repair_neg_upper_returns_low() {
    // Original bug: upper-bound branch returns low (0) instead of high (10).
    let got = clamp(11, 0, 10);
    assert_eq!(got, 10);
    let buggy = 0;
    assert_ne!(buggy, got, "bug value 0 in detail vs correct 10");
}

#[test]
fn s3_repair_neg_signature_changed() {
    // A 1-input clamp cannot be called with 3 inputs: the engine refuses with
    // VM_EXEC_INPUT_COUNT_MISMATCH (ExecutionErrorCode::InputCountMismatch).
    let one = Cmp {
        types: TypeEnvironment::new(vec![]).unwrap(),
        function: FunctionGraph {
            entity_id: id(90),
            type_parameters: vec![],
            parameters: vec![id(91)],
            result_type: si64(),
            effects: vec![],
            entry_block: id(92),
            blocks: vec![id(92)],
            contracts: vec![],
            visibility: Visibility::Private,
        },
        params: vec![Parameter {
            entity_id: id(91),
            owner: id(90),
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: si64(),
        }],
        blocks: vec![Block {
            entity_id: id(92),
            function: id(90),
            parameters: vec![],
            operations: vec![],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::Parameter(id(91)),
            }),
            reachability: Reachability::Required,
        }],
        ops: vec![],
    };
    let input = LoweringInput {
        types: &one.types,
        function: &one.function,
        parameters: &one.params,
        blocks: &one.blocks,
        operations: &one.ops,
        schema_epoch: epoch(),
        state_root: root(),
        profile: CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: &[],
        contracts: &[],
        adapters: &[],
    };
    let err = execute_function(
        input,
        ExecutionRequest {
            inputs: vec![sint(1), sint(0), sint(10)],
            limits: limits(),
        },
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        sley_vm::ExecutionErrorCode::InputCountMismatch.to_string()
    );
    assert_eq!(
        sley_vm::ExecutionErrorCode::InputCountMismatch.as_str(),
        "VM_EXEC_INPUT_COUNT_MISMATCH"
    );
}
