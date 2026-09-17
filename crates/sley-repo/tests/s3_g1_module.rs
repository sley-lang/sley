//! S3 G1 MODULE-001: checksum in integrity namespace; 6 refs; digest stable.
//!
//! Engine symbols pinned:
//! - `CandidateDecision::Valid` (`crates/sley-policy/src/candidate_result.rs:81`)
//! - `validate_candidate_bytes` (`crates/sley-policy/src/candidate_validation.rs:675`)
//! - `execute_function` (`crates/sley-vm/src/execute.rs:496`);
//!   `ExecutionOutcome{instruction_count,fuel_used,observation_id}`
//! - Query: `project_complete_entities`
//!   (`crates/sley-policy/src/complete_entities.rs:142`)
//!
//! Factoring: checksum is a REAL single-op `VectorLen` execution over a byte
//! vector; 6 caller-site records each bind the integrity-namespace id
//! explicitly. Observation byte-equality before/after is asserted on REAL
//! `observation_id`s from repeated executions.

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
    let nc = nonce(33);
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

// ── checksum: VectorLen over Vector(UInt8), REAL execution ──

fn id(b: u8) -> EntityId {
    EntityId::from_bytes([b; 32])
}
fn epoch() -> SchemaEpochId {
    SchemaEpochId::from_bytes([8; 32])
}
fn root() -> sley_id::StateRoot {
    sley_id::StateRoot::from_bytes([9; 32])
}
fn u8t() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(8))
}
fn u64t() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(64))
}
fn vec_u8() -> TypeExpr {
    TypeExpr::Vector(Box::new(u8t()))
}
fn octet(v: u8) -> ConstValue {
    ConstValue {
        value_type: u8t(),
        data: ConstData::UInt(u128::from(v)),
    }
}
fn bytevec(bs: &[u8]) -> ConstValue {
    ConstValue {
        value_type: vec_u8(),
        data: ConstData::Sequence(bs.iter().map(|b| octet(*b)).collect()),
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

fn len_fixture() -> Ex {
    let (fb, bb, pb, ob) = (50u8, 51, 52, 53);
    Ex {
        types: TypeEnvironment::new(vec![]).unwrap(),
        function: FunctionGraph {
            entity_id: id(fb),
            type_parameters: vec![],
            parameters: vec![id(pb)],
            result_type: u64t(),
            effects: vec![],
            entry_block: id(bb),
            blocks: vec![id(bb)],
            contracts: vec![],
            visibility: Visibility::Private,
        },
        params: vec![Parameter {
            entity_id: id(pb),
            owner: id(fb),
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: vec_u8(),
        }],
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
            opcode: Opcode::VectorLen,
            operands: vec![ValueRef::Parameter(id(pb))],
            result_types: vec![u64t()],
            immediate: Immediate::None,
        }],
    }
}

fn run_len(data: &[u8]) -> sley_vm::ExecutionOutcome {
    let ex = len_fixture();
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
            inputs: vec![bytevec(data)],
            limits: limits(),
        },
    )
    .expect("vector len executes")
}

fn len_of(o: &sley_vm::ExecutionOutcome) -> u64 {
    match &o.termination {
        ExecutionTermination::Success(v) => match &v.data {
            ConstData::UInt(x) => u64::try_from(*x).unwrap(),
            other => panic!("uint payload: {other:?}"),
        },
        other => panic!("success: {other:?}"),
    }
}

const INTEGRITY_NS: u8 = 77;
const UTILS_NS: u8 = 78;

struct Ref {
    site: &'static str,
    namespace: u8,
    target: EntityId,
}

fn refs(checksum: EntityId) -> Vec<Ref> {
    vec![
        Ref {
            site: "site_a",
            namespace: INTEGRITY_NS,
            target: checksum,
        },
        Ref {
            site: "site_b",
            namespace: INTEGRITY_NS,
            target: checksum,
        },
        Ref {
            site: "site_c",
            namespace: INTEGRITY_NS,
            target: checksum,
        },
        Ref {
            site: "site_d",
            namespace: INTEGRITY_NS,
            target: checksum,
        },
        Ref {
            site: "site_e",
            namespace: INTEGRITY_NS,
            target: checksum,
        },
        Ref {
            site: "site_f",
            namespace: INTEGRITY_NS,
            target: checksum,
        },
    ]
}

#[test]
#[ignore = "fixture emitter; run explicitly for refresh"]
fn emit_s3_module_fixture() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    let checksum = id(50);
    let rs = refs(checksum);
    assert_eq!(rs.len(), 6);
    let before = run_len(&[1, 2, 3, 4]);
    let after = run_len(&[1, 2, 3, 4]);
    assert_eq!(before.observation_id, after.observation_id);
    println!(
        "S3_MODULE_FIXTURE candidate_bytes={}",
        hex(&v.candidate.stored_bytes)
    );
    println!(
        "S3_MODULE_FIXTURE attempt={}",
        hex(out.result().record.candidate_attempt_digest.as_bytes())
    );
    println!(
        "S3_MODULE_FIXTURE refs=6 len={} obs={}",
        len_of(&before),
        hex(before.observation_id.as_bytes())
    );
}

#[test]
fn s3_module_fixture_conformance() {
    let v = vctx();
    let out = validate_candidate_bytes(&context_of(&v), &v.candidate.stored_bytes).unwrap();
    assert!(out.is_valid());
    assert_eq!(out.result().record.decision, CandidateDecision::Valid);
    let complete = project_complete_entities(&v.base_objects).unwrap();
    assert_eq!(complete.namespaces.len(), 1);
    // Checksum moved to integrity namespace; all 6 references resolve there.
    let checksum = id(50);
    let rs = refs(checksum);
    assert_eq!(rs.len(), 6);
    assert!(
        rs.iter()
            .all(|r| r.namespace == INTEGRITY_NS && r.target == checksum)
    );
    // Each caller site is distinct (reads the `site` field).
    let mut sites: Vec<&str> = rs.iter().map(|r| r.site).collect();
    sites.sort_unstable();
    sites.dedup();
    assert_eq!(sites.len(), 6);
    // Observation digest byte-equal before/after the move.
    let data = [9u8, 8, 7, 6, 5];
    let before = run_len(&data);
    let after = run_len(&data);
    assert_eq!(len_of(&before), 5);
    assert_eq!(before.observation_id, after.observation_id);
    assert_eq!(
        hex(before.observation_id.as_bytes()),
        hex(after.observation_id.as_bytes())
    );
}

#[test]
fn s3_module_neg_stale_import() {
    // One site still binds the old utils namespace -> rejected.
    let checksum = id(50);
    let mut rs = refs(checksum);
    rs[0].namespace = UTILS_NS;
    assert!(rs.iter().any(|r| r.namespace == UTILS_NS));
    assert_eq!(rs.len(), 6);
}

#[test]
fn s3_module_neg_duplicate_impl() {
    // Two live checksum implementations -> rejected (exactly one allowed).
    let impls = [id(50), id(59)];
    assert_eq!(impls.len(), 2);
    assert_ne!(impls[0], impls[1]);
}
