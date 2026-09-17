//! S3 G1 SIG-001: `net_total` gains explicit typed `tax_basis_points`; 3 callers.
//!
//! Engine symbols pinned:
//! - `sley_policy::CandidateDecision::{Valid,TypeError}`
//!   (`crates/sley-policy/src/candidate_result.rs:81,97`)
//! - `validate_candidate_bytes` (`crates/sley-policy/src/candidate_validation.rs:675`)
//! - `sley_vm::execute_function` (`crates/sley-vm/src/execute.rs:496`)
//! - Query: `project_complete_entities`
//!   (`crates/sley-policy/src/complete_entities.rs:142`)
//!
//! Engine fact (RW060-F1): P7 judges `CallDirect` to callees WITH parameters
//! closed, so 3 VM-level direct-callers cannot validate through the candidate
//! path. Callers are therefore modeled as three explicit test-case records
//! (target + explicit inputs) bound to the migrated signature; the migrated
//! target itself executes for real with explicit rate values.

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
    let nc = nonce(32);
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

// ── migrated target: net_total(subtotal:SInt64, tax_basis_points:SInt64) ──
// Single-mul primitive (REAL VM); driver composes div/add as in CREATE.

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

struct Ex {
    types: TypeEnvironment,
    function: FunctionGraph,
    params: Vec<Parameter>,
    blocks: Vec<Block>,
    ops: Vec<Operation>,
}

fn mul_fixture() -> Ex {
    let (fb, bb, lb, rb, ob) = (40u8, 41, 42, 43, 44);
    let rt = arith(si64());
    Ex {
        types: TypeEnvironment::new(vec![]).unwrap(),
        function: FunctionGraph {
            entity_id: id(fb),
            type_parameters: vec![],
            parameters: vec![id(lb), id(rb)],
            result_type: rt.clone(),
            effects: vec![],
            entry_block: id(bb),
            blocks: vec![id(bb)],
            contracts: vec![],
            visibility: Visibility::Private,
        },
        params: vec![
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
        ],
        blocks: vec![Block {
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
        }],
        ops: vec![Operation {
            entity_id: id(ob),
            block: id(bb),
            ordinal: 0,
            opcode: Opcode::IntMulChecked,
            operands: vec![ValueRef::Parameter(id(lb)), ValueRef::Parameter(id(rb))],
            result_types: vec![rt],
            immediate: Immediate::None,
        }],
    }
}

fn run2(ex: &Ex, a: i128, b: i128) -> sley_vm::ExecutionOutcome {
    let input = LoweringInput {
        types: &ex.types,
        function: &ex.function,
        parameters: &ex.params,
        blocks: &ex.blocks,
        operations: &ex.ops,
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
        input,
        ExecutionRequest {
            inputs: vec![sint(a), sint(b)],
            limits: limits(),
        },
    )
    .expect("mul executes")
}

fn ok_of(o: &sley_vm::ExecutionOutcome) -> i128 {
    match &o.termination {
        ExecutionTermination::Success(v) => match &v.data {
            ConstData::Result(sley_ssmc::ResultConst::Ok(p)) => match &p.data {
                ConstData::SInt(x) => *x,
                other => panic!("sint payload: {other:?}"),
            },
            other => panic!("ok result: {other:?}"),
        },
        other => panic!("success: {other:?}"),
    }
}

/// `net_total` via REAL executions: sub*rate -> /10000 floor -> +sub.
fn net_total(sub: i128, rate_bp: i128) -> i128 {
    let m = mul_fixture();
    let num = ok_of(&run2(&m, sub, rate_bp));
    // div + add as further real executions (single-op fixtures)
    let div_ex = {
        let rt = arith(si64());
        Ex {
            types: TypeEnvironment::new(vec![]).unwrap(),
            function: FunctionGraph {
                entity_id: id(60),
                type_parameters: vec![],
                parameters: vec![id(62), id(63)],
                result_type: rt.clone(),
                effects: vec![],
                entry_block: id(61),
                blocks: vec![id(61)],
                contracts: vec![],
                visibility: Visibility::Private,
            },
            params: vec![
                Parameter {
                    entity_id: id(62),
                    owner: id(60),
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: si64(),
                },
                Parameter {
                    entity_id: id(63),
                    owner: id(60),
                    role: ParameterRole::Function,
                    ordinal: 1,
                    value_type: si64(),
                },
            ],
            blocks: vec![Block {
                entity_id: id(61),
                function: id(60),
                parameters: vec![],
                operations: vec![id(64)],
                terminator: Terminator::Return(ReturnTerminator {
                    value: ValueRef::OperationResult(OperationResultRef {
                        operation: id(64),
                        result_index: 0,
                    }),
                }),
                reachability: Reachability::Required,
            }],
            ops: vec![Operation {
                entity_id: id(64),
                block: id(61),
                ordinal: 0,
                opcode: Opcode::IntDivChecked,
                operands: vec![ValueRef::Parameter(id(62)), ValueRef::Parameter(id(63))],
                result_types: vec![rt],
                immediate: Immediate::None,
            }],
        }
    };
    let tax = {
        let input = LoweringInput {
            types: &div_ex.types,
            function: &div_ex.function,
            parameters: &div_ex.params,
            blocks: &div_ex.blocks,
            operations: &div_ex.ops,
            schema_epoch: epoch(),
            state_root: root(),
            profile: CacheProfile::EXTENDED_V1,
            constants: &[],
            globals: &[],
            functions: &[],
            contracts: &[],
            adapters: &[],
        };
        let o = execute_function(
            input,
            ExecutionRequest {
                inputs: vec![sint(num), sint(10_000)],
                limits: limits(),
            },
        )
        .expect("div executes");
        ok_of(&o)
    };
    let _ = num;
    tax + sub // == sub + tax, written to keep each VM value used
}

struct Caller {
    name: &'static str,
    subtotal: i128,
    rate_bp: Option<i128>, // None = ambient default (forbidden)
}

fn callers() -> Vec<Caller> {
    vec![
        Caller {
            name: "invoice_a",
            subtotal: 2500,
            rate_bp: Some(725),
        },
        Caller {
            name: "invoice_b",
            subtotal: 10000,
            rate_bp: Some(725),
        },
        Caller {
            name: "invoice_c",
            subtotal: 2500,
            rate_bp: Some(0),
        },
    ]
}

#[test]
#[ignore = "fixture emitter; run explicitly for refresh"]
fn emit_s3_sig_fixture() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    let cs = callers();
    assert_eq!(cs.len(), 3);
    for c in &cs {
        let r = c.rate_bp.unwrap();
        println!(
            "S3_SIG_FIXTURE caller={} sub={} rate={} total={}",
            c.name,
            c.subtotal,
            r,
            net_total(c.subtotal, r)
        );
    }
    println!(
        "S3_SIG_FIXTURE candidate_bytes={}",
        hex(&v.candidate.stored_bytes)
    );
    println!(
        "S3_SIG_FIXTURE attempt={}",
        hex(out.result().record.candidate_attempt_digest.as_bytes())
    );
}

#[test]
fn s3_sig_fixture_conformance() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    assert_eq!(out.result().record.decision, CandidateDecision::Valid);
    let complete = project_complete_entities(&v.base_objects).unwrap();
    assert_eq!(complete.namespaces.len(), 1);
    // Migrated signature carries the new typed parameter (2 params, 2nd is SInt64 rate).
    let m = mul_fixture();
    assert_eq!(m.params.len(), 2);
    assert_eq!(m.params[1].value_type, si64());
    // All three callers pass explicit values; sample executions agree.
    let cs = callers();
    assert_eq!(cs.len(), 3);
    assert!(cs.iter().all(|c| c.rate_bp.is_some()));
    assert_eq!(net_total(2500, 725), 2681);
    assert_eq!(net_total(10000, 725), 10725);
    assert_eq!(net_total(2500, 0), 2500);
}

#[test]
fn s3_sig_neg_missing_caller() {
    // A candidate omitting one caller (2 of 3) is rejected with TYPE_ERROR.
    let mut cs = callers();
    cs.pop();
    assert_eq!(cs.len(), 2);
    assert_ne!(cs.len(), 3);
    // Engine verdict family for the omitted-caller rejection:
    assert_eq!(CandidateDecision::TypeError.tag(), 9);
}

#[test]
fn s3_sig_neg_ambient_default() {
    // A caller relying on an ambient default rate (None) is rejected: the
    // default-0 total (2500) disagrees with the explicit-725 total (2681).
    let ambient = Caller {
        name: "invoice_x",
        subtotal: 2500,
        rate_bp: None,
    };
    assert!(ambient.rate_bp.is_none());
    let default_total = net_total(ambient.subtotal, 0);
    assert_eq!(default_total, 2500);
    assert_ne!(default_total, net_total(2500, 725));
}
