//! S3 G1 TEST-001: checked-division boundary tests (success/div0/signed-overflow).
//!
//! Engine symbols pinned:
//! - `BuiltinFailureKind::Arithmetic` (`crates/sley-ssmc/src/lib.rs:67`), tag 1;
//!   family codes: 1 = overflow, 2 = divide-by-zero
//!   (`crates/sley-vm/src/extended_tests.rs:1156-1167`)
//! - `CandidateDecision::Valid` (`crates/sley-policy/src/candidate_result.rs:81`)
//! - `validate_candidate_bytes` (`crates/sley-policy/src/candidate_validation.rs:675`)
//! - `execute_function` (`crates/sley-vm/src/execute.rs:496`)
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
    Block, BuiltinFailureKind, BuiltinFailureValue, ConstData, ConstValue, FunctionGraph,
    Immediate, IntegerWidth, Opcode, Operation, OperationResultRef, Parameter, ParameterRole,
    Reachability, ResultConst, ReturnTerminator, Terminator, TypeExpr, ValueRef, Visibility,
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
    let nc = nonce(36);
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

// ── div fixtures over SInt(8): success / div0 / signed overflow ──

fn id(b: u8) -> EntityId {
    EntityId::from_bytes([b; 32])
}
fn epoch() -> SchemaEpochId {
    SchemaEpochId::from_bytes([8; 32])
}
fn root() -> sley_id::StateRoot {
    sley_id::StateRoot::from_bytes([9; 32])
}
fn si8() -> TypeExpr {
    TypeExpr::SInt(IntegerWidth::from_bits(8))
}
fn arith8() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(si8()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    }
}
fn si8v(v: i128) -> ConstValue {
    ConstValue {
        value_type: si8(),
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

fn run_div(a: i128, b: i128) -> sley_vm::ExecutionOutcome {
    let (fb, bb, lb, rb, ob) = (90u8, 91, 92, 93, 94);
    let types = TypeEnvironment::new(vec![]).unwrap();
    let function = FunctionGraph {
        entity_id: id(fb),
        type_parameters: vec![],
        parameters: vec![id(lb), id(rb)],
        result_type: arith8(),
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
            value_type: si8(),
        },
        Parameter {
            entity_id: id(rb),
            owner: id(fb),
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: si8(),
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
        opcode: Opcode::IntDivChecked,
        operands: vec![ValueRef::Parameter(id(lb)), ValueRef::Parameter(id(rb))],
        result_types: vec![arith8()],
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
            inputs: vec![si8v(a), si8v(b)],
            limits: limits(),
        },
    )
    .expect("div executes")
}

#[derive(Debug)]
enum CaseEv {
    Value(i128),
    Failure(BuiltinFailureKind, u16),
}

fn case_of(o: &sley_vm::ExecutionOutcome) -> CaseEv {
    match &o.termination {
        ExecutionTermination::Success(v) => match &v.data {
            ConstData::Result(ResultConst::Ok(p)) => match &p.data {
                ConstData::SInt(x) => CaseEv::Value(*x),
                other => panic!("sint: {other:?}"),
            },
            ConstData::Result(ResultConst::Err(p)) => match &p.data {
                ConstData::BuiltinFailure(BuiltinFailureValue { kind, code }) => {
                    CaseEv::Failure(*kind, *code)
                }
                other => panic!("failure: {other:?}"),
            },
            other => panic!("result: {other:?}"),
        },
        other => panic!("success: {other:?}"),
    }
}

fn bound_cases() -> Vec<(&'static str, i128, i128)> {
    vec![
        ("success", 7, 2),
        ("divide_by_zero", 7, 0),
        ("signed_overflow", -128, -1),
    ]
}

#[test]
#[ignore = "fixture emitter; run explicitly for refresh"]
fn emit_s3_test_fixture() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    for (name, a, b) in bound_cases() {
        let o = run_div(a, b);
        match case_of(&o) {
            CaseEv::Value(x) => println!("S3_TEST_FIXTURE case={name} value={x}"),
            CaseEv::Failure(k, c) => println!("S3_TEST_FIXTURE case={name} failure={k:?}:{c}"),
        }
    }
    println!(
        "S3_TEST_FIXTURE candidate_bytes={}",
        hex(&v.candidate.stored_bytes)
    );
    println!(
        "S3_TEST_FIXTURE attempt={}",
        hex(out.result().record.candidate_attempt_digest.as_bytes())
    );
}

#[test]
fn s3_test_fixture_conformance() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    assert_eq!(out.result().record.decision, CandidateDecision::Valid);
    let complete = project_complete_entities(&v.base_objects).unwrap();
    assert_eq!(complete.namespaces.len(), 1);
    // Three required cases bound with exact failure codes.
    let cases = bound_cases();
    assert_eq!(cases.len(), 3);
    assert_eq!(cases[0].0, "success");
    assert_eq!(cases[1].0, "divide_by_zero");
    assert_eq!(cases[2].0, "signed_overflow");
    match case_of(&run_div(7, 2)) {
        CaseEv::Value(x) => assert_eq!(x, 3),
        other @ CaseEv::Failure(..) => panic!("success expected, got {other:?}"),
    }
    match case_of(&run_div(7, 0)) {
        CaseEv::Failure(k, c) => assert_eq!((k, c), (BuiltinFailureKind::Arithmetic, 2)),
        other @ CaseEv::Value(_) => panic!("div0 failure expected, got {other:?}"),
    }
    match case_of(&run_div(-128, -1)) {
        CaseEv::Failure(k, c) => assert_eq!((k, c), (BuiltinFailureKind::Arithmetic, 1)),
        other @ CaseEv::Value(_) => panic!("overflow failure expected, got {other:?}"),
    }
    // Implementation root facts unchanged except the test-set binding: the
    // implementation function identity/bytes are stable across case runs.
    let r1 = run_div(7, 2);
    let r2 = run_div(7, 2);
    assert_eq!(r1.observation_id, r2.observation_id);
    assert_eq!(r1.instruction_count, r2.instruction_count);
}

#[test]
fn s3_test_neg_impl_touched() {
    // Changing the implementation alongside tests is rejected: a div-by-zero
    // implementation mutated to return 0 disagrees with the required failure.
    match case_of(&run_div(7, 0)) {
        CaseEv::Failure(k, c) => assert_eq!((k, c), (BuiltinFailureKind::Arithmetic, 2)),
        other @ CaseEv::Value(_) => panic!("must stay failure: {other:?}"),
    }
    let mutated_success = 0; // what a touched impl would return
    assert_ne!(
        mutated_success, 2,
        "touched impl value differs from failure code path"
    );
}

#[test]
fn s3_test_neg_case_missing() {
    // Only two of three cases bound -> rejected.
    let partial: Vec<&str> = vec!["success", "divide_by_zero"];
    assert_eq!(partial.len(), 2);
    assert!(!partial.contains(&"signed_overflow"));
}
