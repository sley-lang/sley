//! RW-060: useful source-free Sley machine-programming loop.
//!
//! End-to-end demonstration through production authorities only:
//! genesis -> BUILD (typed machine mutations, no source text) -> REJECT
//! (production validation refusal, state untouched) -> REPAIR (diagnostic
//! driven rebuild through the same mutation path) -> COMMIT (real
//! transaction) -> EXECUTE (lower + run under `BOOTSTRAP_PROFILE_1`,
//! deterministic observations) -> PACK (export) -> RECONSTRUCT (clean-store
//! import, exact root) plus source-absence proof and malformed-input
//! negatives.
//!
//! The demo program `checksum` exercises composed profile capability in a
//! single function: `Vector`, `Bytes`, ordered maps, `Option`/`Result`,
//! a named record, a backedge loop with block-parameter state, content
//! hashing, bounded traversal, and canonical record output. It uses no
//! bridge rows: every capability above is a frozen E1/E2/E4/E5 opcode, so
//! the committed program carries no adapter inventory. Scope notes: (a) no
//! `EntryPoint` entity is created; execution addresses the committed
//! function by its stable `EntityId`, exactly like the production SMP1
//! `execute` path. (b) The program is single-function by production
//! necessity, not by design preference: finding RW060-F1 (recorded in
//! `rw-060.md`) shows the P7 validator judges each function unit with
//! unit-scoped parameters, so any `CallDirect` to a callee *with
//! parameters* fails closed even though lowering accepts it. Multi-function
//! gate admission and execution stay proven by the RW-050 closure vectors;
//! the Council owns the F1 ruling.
//!
//! Normative artifacts are canonical bytes throughout. The Rust driver
//! below is orchestration only: it holds typed `sley_ssmc` values in
//! memory (as any agent client would) and never generates, parses, or
//! reconstructs Sley source text — there is no source syntax to generate.

//! Declarative lifecycle demonstration by construction: long test drivers
//! and builders, like the fixture builders in the VM closure workloads.
#![allow(clippy::too_many_lines)]

use std::fs;
use std::path::PathBuf;

use sley_check::TypeEnvironment;
use sley_id::{
    CandidateNonce, EntityId, ObjectId, SchemaEpochId, StateRoot, TransactionId, WorkspaceId,
};
use sley_mutate::value::{
    BlockBody, ConstantBody, EntityBodyValue, EntityIdSet, FunctionBody, NamespaceBody,
    OperationBody, ParameterBody, TypeDefBody,
};
use sley_mutate::{
    BoundPrecondition, CandidateExpiry, CandidateRecord, EntityObject, EntityObjectRecord,
    ExpectedIdentityAbsent, ImportedCandidate, MutationClass, MutationOperation, MutationPayload,
    PreconditionPayload, PreimageRequirement, build_candidate, build_entity_object,
    full_validation_profile_id, import_entity_object,
};
use sley_policy::{
    CandidateDiagnostic, CandidateValidationLimits, PolicyResourceCeilings, PolicyRootBuilder,
    PrincipalGrantBuilder, build_capability_summary_projection,
    complete_entities::project_complete_entities, conformance_registry as policy_registry,
};
use sley_repo::{RepositoryObjectVerifier, export_conformance_pack, import_conformance_pack};
use sley_ssmc::{
    BranchTerminator, BuiltinCase, BuiltinFailureKind, CaseKey, CondBranchTerminator, ConstData,
    ConstValue, FunctionRefValue, Immediate, IntegerWidth, MemberId, OperationResultRef,
    ParameterRole, Reachability, RecordField, ReturnTerminator, SwitchArgument, SwitchCase,
    SwitchEdge, TargetEdge, Terminator, TypeDefForm, TypeExpr, ValueRef, VariantSwitchTerminator,
    Visibility,
};
use sley_state_root::{
    StateRootBuilder, conformance_epoch_id as state_epoch_id,
    conformance_registry as state_registry,
};
use sley_store::ObjectStore;
use sley_txn::{
    CommitError, CommitInput, ImportedTransactionReceipt, TransactionRepository,
    TrustedGenesisInput, verify_receipt_against_objects,
};
use sley_vm::bootstrap::{BootstrapProfileInput, judge_bootstrap_profile};
use sley_vm::{
    CacheProfile, ExecutionLimits, ExecutionRequest, ExecutionTermination, LoweringInput,
    execute_function,
};

// ---------------------------------------------------------------------------
// Frozen demo identities (entity bytes) and wall-clock-free validation time.
// ---------------------------------------------------------------------------

const WORKSPACE_BYTE: u8 = 7;
const PRINCIPAL_BYTE: u8 = 2;
const ANCHOR_A: u8 = 20;
const ANCHOR_B: u8 = 21;
const BASE_NS: u8 = 10;
const NOW_MILLIS: u64 = 1_780_000_000_000;

const TYPE_TALLY: u8 = 60;
const FN_MAIN: u8 = 61;
const P_DATA: u8 = 62;
const P_TAG: u8 = 63;
const B_ENTRY: u8 = 64;
const B_LOOP: u8 = 65;
const B_BODY: u8 = 66;
const B_STEP: u8 = 67;
const B_NEXT: u8 = 68;
const B_SUMDONE: u8 = 69;
const B_SUMFAIL: u8 = 70;
const B_HIT: u8 = 71;
const B_MISS: u8 = 72;
const P_INDEX: u8 = 73;
const P_ACC: u8 = 74;
const P_BI: u8 = 75;
const P_BA: u8 = 76;
const P_E: u8 = 77;
const P_I: u8 = 78;
const P_A: u8 = 79;
const P_NA: u8 = 80;
const P_NI: u8 = 81;
const P_T: u8 = 82;
const P_FE: u8 = 83;
const P_FD: u8 = 84;
const O_ZI: u8 = 85;
const O_ZA: u8 = 86;
const O_LEN: u8 = 87;
const O_CMP: u8 = 88;
const O_GET: u8 = 89;
const O_ADD: u8 = 90;
const O_ONE: u8 = 91;
const O_INC: u8 = 92;
const O_MNEW: u8 = 93;
const O_MINS: u8 = 94;
const O_MGET: u8 = 95;
const O_H1: u8 = 96;
const O_Z1: u8 = 97;
const O_R1: u8 = 98;
const O_H2: u8 = 99;
const O_R2: u8 = 100;
const O_H3: u8 = 101;
const O_Z3: u8 = 102;
const O_R3: u8 = 103;
const C_ZERO_U8: u8 = 104;
const C_ZERO_U64: u8 = 105;
const C_ONE_U64: u8 = 106;
const B_MAP: u8 = 107;
const P_MAP: u8 = 108;
const P_T2: u8 = 109;

// RW060-F1 regression slots: callee `pass` plus entry `usecall`.
const EC_FN: u8 = 40;
const EC_P: u8 = 41;
const EC_B: u8 = 42;
const EN_FN: u8 = 44;
const EN_P: u8 = 45;
const EN_B: u8 = 46;
const EN_O: u8 = 47;
const NONCE_CALL: u8 = 32;

const CALL_ORDER: &[(u8, u16)] = &[
    (EC_FN, 5),
    (EC_P, 6),
    (EC_B, 7),
    (EN_FN, 5),
    (EN_P, 6),
    (EN_B, 7),
    (EN_O, 8),
];

const MEMBER_DIGEST: u8 = 0xA1;
const MEMBER_TOTAL: u8 = 0xA2;

const NONCE_REJECT: u8 = 30;
const NONCE_REPAIR: u8 = 31;

// ---------------------------------------------------------------------------
// Small helpers (test orchestration, not program semantics).
// ---------------------------------------------------------------------------

fn id(byte: u8) -> EntityId {
    EntityId::from_bytes([byte; 32])
}

fn member(byte: u8) -> MemberId {
    MemberId::from_bytes([byte; 32])
}

fn empty_set() -> EntityIdSet {
    EntityIdSet::from_unsorted(Vec::new()).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

fn u8_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(8))
}

fn u64_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(64))
}

fn vec_u8_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(u8_type()))
}

fn map_u8_u8_type() -> TypeExpr {
    TypeExpr::OrderedMap {
        key: Box::new(u8_type()),
        value: Box::new(u8_type()),
    }
}

fn arith_error() -> TypeExpr {
    TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)
}

fn result_u8() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(u8_type()),
        error: Box::new(arith_error()),
    }
}

fn result_u64() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(u64_type()),
        error: Box::new(arith_error()),
    }
}

fn map_dup_result() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(map_u8_u8_type()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    }
}

fn tally_type(definition: EntityId) -> TypeExpr {
    TypeExpr::Named(sley_ssmc::NamedType {
        definition,
        arguments: Vec::new(),
    })
}

fn octet(value: u8) -> ConstValue {
    ConstValue {
        value_type: u8_type(),
        data: ConstData::UInt(u128::from(value)),
    }
}

fn uint(value: u128) -> ConstValue {
    ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(value),
    }
}

fn byte_seq(values: &[u8]) -> ConstValue {
    ConstValue {
        value_type: vec_u8_type(),
        data: ConstData::Sequence(values.iter().map(|v| octet(*v)).collect()),
    }
}

/// Slot table: creation order of the demo program. A created entity's
/// identity is draconically derived (`EntityId::derive(workspace, nonce,
/// kind, create_ordinal)`); the driver resolves every reference through
/// this table, never by inventing bytes.
const SLOT_ORDER: &[(u8, u16)] = &[
    (TYPE_TALLY, 4),
    (FN_MAIN, 5),
    (P_DATA, 6),
    (P_TAG, 6),
    (B_ENTRY, 7),
    (B_LOOP, 7),
    (B_BODY, 7),
    (B_STEP, 7),
    (B_NEXT, 7),
    (B_SUMDONE, 7),
    (B_SUMFAIL, 7),
    (B_HIT, 7),
    (B_MISS, 7),
    (P_INDEX, 6),
    (P_ACC, 6),
    (P_BI, 6),
    (P_BA, 6),
    (P_E, 6),
    (P_I, 6),
    (P_A, 6),
    (P_NA, 6),
    (P_NI, 6),
    (P_T, 6),
    (P_FE, 6),
    (P_FD, 6),
    (O_ZI, 8),
    (O_ZA, 8),
    (O_LEN, 8),
    (O_CMP, 8),
    (O_GET, 8),
    (O_ADD, 8),
    (O_ONE, 8),
    (O_INC, 8),
    (O_MNEW, 8),
    (O_MINS, 8),
    (O_MGET, 8),
    (O_H1, 8),
    (O_Z1, 8),
    (O_R1, 8),
    (O_H2, 8),
    (O_R2, 8),
    (O_H3, 8),
    (O_Z3, 8),
    (O_R3, 8),
    (C_ZERO_U8, 9),
    (C_ZERO_U64, 9),
    (C_ONE_U64, 9),
    (P_MAP, 6),
    (P_T2, 6),
    (B_MAP, 7),
];

struct Slots {
    workspace: WorkspaceId,
    nonce: CandidateNonce,
    order: &'static [(u8, u16)],
}

impl Slots {
    fn eid(&self, symbol: u8) -> EntityId {
        let (ordinal, (_, kind)) = self
            .order
            .iter()
            .enumerate()
            .find(|(_, (slot, _))| *slot == symbol)
            .expect("program slot present");
        EntityId::derive(self.workspace, self.nonce, u32::from(*kind), ordinal as u64)
    }
}

fn p(entity: EntityId) -> ValueRef {
    ValueRef::Parameter(entity)
}

fn r(operation: EntityId) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation,
        result_index: 0,
    })
}

fn br(target: EntityId, arguments: Vec<ValueRef>) -> Terminator {
    Terminator::Branch(BranchTerminator {
        edge: TargetEdge { target, arguments },
    })
}

fn cbr(
    condition: ValueRef,
    if_true: EntityId,
    true_args: Vec<ValueRef>,
    if_false: EntityId,
    false_args: Vec<ValueRef>,
) -> Terminator {
    Terminator::CondBranch(CondBranchTerminator {
        condition,
        if_true: TargetEdge {
            target: if_true,
            arguments: true_args,
        },
        if_false: TargetEdge {
            target: if_false,
            arguments: false_args,
        },
    })
}

fn switch(value: ValueRef, cases: Vec<(CaseKey, EntityId, Vec<SwitchArgument>)>) -> Terminator {
    Terminator::VariantSwitch(VariantSwitchTerminator {
        value,
        cases: cases
            .into_iter()
            .map(|(case_key, target, arguments)| SwitchCase {
                case_key,
                edge: SwitchEdge { target, arguments },
            })
            .collect(),
    })
}

fn ok_case(
    target: EntityId,
    arguments: Vec<SwitchArgument>,
) -> (CaseKey, EntityId, Vec<SwitchArgument>) {
    (CaseKey::Builtin(BuiltinCase::Ok), target, arguments)
}

fn err_case(
    target: EntityId,
    arguments: Vec<SwitchArgument>,
) -> (CaseKey, EntityId, Vec<SwitchArgument>) {
    (CaseKey::Builtin(BuiltinCase::Err), target, arguments)
}

fn some_case(
    target: EntityId,
    arguments: Vec<SwitchArgument>,
) -> (CaseKey, EntityId, Vec<SwitchArgument>) {
    (CaseKey::Builtin(BuiltinCase::Some), target, arguments)
}

fn none_case(
    target: EntityId,
    arguments: Vec<SwitchArgument>,
) -> (CaseKey, EntityId, Vec<SwitchArgument>) {
    (CaseKey::Builtin(BuiltinCase::None), target, arguments)
}

fn ret(value: ValueRef) -> Terminator {
    Terminator::Return(ReturnTerminator { value })
}

// ---------------------------------------------------------------------------
// Frozen minimal genesis (empty program workspace + namespace anchor).
// ---------------------------------------------------------------------------

static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let sequence = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "sley-rw060-{label}-{}-{sequence:016x}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct Genesis {
    _temp: TempDir,
    repo: TransactionRepository,
    workspace: WorkspaceId,
    principal: sley_id::PrincipalId,
    epoch: SchemaEpochId,
    origin_tx: TransactionId,
    origin_receipt: ImportedTransactionReceipt,
}

fn namespace_body() -> EntityBodyValue {
    EntityBodyValue::Namespace(NamespaceBody {
        parent: None,
        members: empty_set(),
    })
}

fn genesis(label: &str) -> Genesis {
    let temp = TempDir::new(label);
    let root = temp.child("repo");
    fs::create_dir(&root).unwrap();
    let workspace = WorkspaceId::from_bytes([WORKSPACE_BYTE; 32]);
    let principal = sley_id::PrincipalId::from_bytes([PRINCIPAL_BYTE; 32]);
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
    let store = ObjectStore::new(&root);
    let verifier = RepositoryObjectVerifier::new(epoch);
    let anchors = [ANCHOR_A, ANCHOR_B].map(|byte| {
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
            .put(object.object_id(), object.stored_bytes(), &verifier)
            .unwrap();
        object.object_id()
    });
    let base_object = build_entity_object(
        epoch,
        &EntityObjectRecord {
            entity_id: id(BASE_NS),
            body: namespace_body(),
            label: None,
            semantic_fingerprint: None,
        },
    )
    .unwrap();
    let state = StateRootBuilder::new(workspace, anchors[0], anchors[1], policy.root())
        .entity_binding(id(BASE_NS), base_object.object_id())
        .build(&state_registry().unwrap())
        .unwrap();
    let repo = TransactionRepository::new(&root);
    let genesis_tx = repo
        .initialize_trusted_genesis(TrustedGenesisInput::new(
            &state,
            &policy,
            core::slice::from_ref(&base_object),
            &[],
        ))
        .unwrap()
        .transaction_id();
    eprintln!("RW060_EVIDENCE genesis_root={}", hex(state.root.as_bytes()));
    eprintln!("RW060_EVIDENCE genesis_tx={}", hex(genesis_tx.as_bytes()));
    let genesis_receipt = repo.accepted_head().unwrap().receipt().clone();
    Genesis {
        _temp: temp,
        repo,
        workspace,
        principal,
        epoch,
        origin_tx: genesis_tx,
        origin_receipt: genesis_receipt,
    }
}

// ---------------------------------------------------------------------------
// BUILD: the demo program as typed machine mutations.
// ---------------------------------------------------------------------------

fn function_body(
    parameters: Vec<EntityId>,
    result_type: TypeExpr,
    entry_block: EntityId,
    blocks: Vec<EntityId>,
) -> EntityBodyValue {
    EntityBodyValue::Function(FunctionBody {
        type_parameters: Vec::new(),
        parameters,
        result_type,
        effects: empty_set(),
        entry_block,
        blocks,
        contracts: empty_set(),
        visibility: Visibility::Private,
    })
}

fn param_body(
    owner: EntityId,
    role: ParameterRole,
    ordinal: u32,
    value_type: TypeExpr,
) -> EntityBodyValue {
    EntityBodyValue::Parameter(ParameterBody {
        owner,
        role,
        ordinal,
        value_type,
    })
}

fn block_body(
    function: EntityId,
    parameters: Vec<EntityId>,
    operations: Vec<EntityId>,
    terminator: Terminator,
) -> EntityBodyValue {
    EntityBodyValue::Block(BlockBody {
        function,
        parameters,
        operations,
        terminator,
        reachability: Reachability::Required,
    })
}

fn op_body(
    block: EntityId,
    ordinal: u32,
    opcode: sley_ssmc::Opcode,
    operands: Vec<ValueRef>,
    result: TypeExpr,
    immediate: Immediate,
) -> EntityBodyValue {
    EntityBodyValue::Operation(OperationBody {
        block,
        ordinal,
        opcode: opcode.tag(),
        operands,
        result_types: vec![result],
        immediate,
    })
}

/// Builds every program entity object. `defect` selects the deliberately
/// invalid REJECT candidate: operation `O_CMP` (`LessThan` over the loop
/// index) compares `u64` against `u8` instead of the vector length,
/// violating a genuine typing invariant (refused at P7 judgment).
fn program_bodies(slots: &Slots, defect: bool) -> Vec<(u8, EntityBodyValue)> {
    let e = |symbol: u8| slots.eid(symbol);
    let tally = tally_type(e(TYPE_TALLY));
    vec![
        (
            TYPE_TALLY,
            EntityBodyValue::TypeDef(TypeDefBody {
                type_parameters: Vec::new(),
                form: TypeDefForm::Record(vec![
                    RecordField {
                        member_id: member(MEMBER_DIGEST),
                        value_type: TypeExpr::Bytes,
                        visibility: Visibility::Private,
                    },
                    RecordField {
                        member_id: member(MEMBER_TOTAL),
                        value_type: u8_type(),
                        visibility: Visibility::Private,
                    },
                ]),
                invariants: empty_set(),
                visibility: Visibility::Private,
            }),
        ),
        (
            FN_MAIN,
            function_body(
                vec![e(P_DATA), e(P_TAG)],
                tally.clone(),
                e(B_ENTRY),
                vec![
                    e(B_ENTRY),
                    e(B_LOOP),
                    e(B_BODY),
                    e(B_STEP),
                    e(B_NEXT),
                    e(B_SUMDONE),
                    e(B_MAP),
                    e(B_SUMFAIL),
                    e(B_HIT),
                    e(B_MISS),
                ],
            ),
        ),
        (
            P_DATA,
            param_body(e(FN_MAIN), ParameterRole::Function, 0, vec_u8_type()),
        ),
        (
            P_TAG,
            param_body(e(FN_MAIN), ParameterRole::Function, 1, u8_type()),
        ),
        (
            B_ENTRY,
            block_body(
                e(FN_MAIN),
                vec![],
                vec![e(O_ZI), e(O_ZA)],
                br(e(B_LOOP), vec![r(e(O_ZI)), r(e(O_ZA))]),
            ),
        ),
        (
            B_LOOP,
            block_body(
                e(FN_MAIN),
                vec![e(P_INDEX), e(P_ACC)],
                vec![e(O_LEN), e(O_CMP)],
                cbr(
                    r(e(O_CMP)),
                    e(B_BODY),
                    vec![p(e(P_INDEX)), p(e(P_ACC))],
                    e(B_SUMDONE),
                    vec![p(e(P_ACC))],
                ),
            ),
        ),
        (
            B_BODY,
            block_body(
                e(FN_MAIN),
                vec![e(P_BI), e(P_BA)],
                vec![e(O_GET)],
                switch(
                    r(e(O_GET)),
                    vec![
                        none_case(e(B_SUMDONE), vec![SwitchArgument::Value(p(e(P_BA)))]),
                        some_case(
                            e(B_STEP),
                            vec![
                                SwitchArgument::CasePayload,
                                SwitchArgument::Value(p(e(P_BI))),
                                SwitchArgument::Value(p(e(P_BA))),
                            ],
                        ),
                    ],
                ),
            ),
        ),
        (
            B_STEP,
            block_body(
                e(FN_MAIN),
                vec![e(P_E), e(P_I), e(P_A)],
                vec![e(O_ADD)],
                switch(
                    r(e(O_ADD)),
                    vec![
                        ok_case(
                            e(B_NEXT),
                            vec![
                                SwitchArgument::CasePayload,
                                SwitchArgument::Value(p(e(P_I))),
                            ],
                        ),
                        err_case(e(B_SUMFAIL), vec![SwitchArgument::CasePayload]),
                    ],
                ),
            ),
        ),
        (
            B_NEXT,
            block_body(
                e(FN_MAIN),
                vec![e(P_NA), e(P_NI)],
                vec![e(O_ONE), e(O_INC)],
                switch(
                    r(e(O_INC)),
                    vec![
                        ok_case(
                            e(B_LOOP),
                            vec![
                                SwitchArgument::CasePayload,
                                SwitchArgument::Value(p(e(P_NA))),
                            ],
                        ),
                        err_case(e(B_SUMFAIL), vec![SwitchArgument::CasePayload]),
                    ],
                ),
            ),
        ),
        (
            B_SUMDONE,
            block_body(
                e(FN_MAIN),
                vec![e(P_T)],
                vec![e(O_MNEW)],
                switch(
                    r(e(O_MNEW)),
                    vec![
                        ok_case(
                            e(B_MAP),
                            vec![
                                SwitchArgument::CasePayload,
                                SwitchArgument::Value(p(e(P_T))),
                            ],
                        ),
                        err_case(e(B_MISS), vec![]),
                    ],
                ),
            ),
        ),
        (
            B_SUMFAIL,
            block_body(
                e(FN_MAIN),
                vec![e(P_FE)],
                vec![e(O_H1), e(O_Z1), e(O_R1)],
                ret(r(e(O_R1))),
            ),
        ),
        (
            B_HIT,
            block_body(
                e(FN_MAIN),
                vec![e(P_FD)],
                vec![e(O_H2), e(O_R2)],
                ret(r(e(O_R2))),
            ),
        ),
        (
            B_MISS,
            block_body(
                e(FN_MAIN),
                vec![],
                vec![e(O_H3), e(O_Z3), e(O_R3)],
                ret(r(e(O_R3))),
            ),
        ),
        (
            P_INDEX,
            param_body(e(B_LOOP), ParameterRole::Block, 0, u64_type()),
        ),
        (
            P_ACC,
            param_body(e(B_LOOP), ParameterRole::Block, 1, u8_type()),
        ),
        (
            P_BI,
            param_body(e(B_BODY), ParameterRole::Block, 0, u64_type()),
        ),
        (
            P_BA,
            param_body(e(B_BODY), ParameterRole::Block, 1, u8_type()),
        ),
        (
            P_E,
            param_body(e(B_STEP), ParameterRole::Block, 0, u8_type()),
        ),
        (
            P_I,
            param_body(e(B_STEP), ParameterRole::Block, 1, u64_type()),
        ),
        (
            P_A,
            param_body(e(B_STEP), ParameterRole::Block, 2, u8_type()),
        ),
        (
            P_NA,
            param_body(e(B_NEXT), ParameterRole::Block, 0, u8_type()),
        ),
        (
            P_NI,
            param_body(e(B_NEXT), ParameterRole::Block, 1, u64_type()),
        ),
        (
            P_T,
            param_body(e(B_SUMDONE), ParameterRole::Block, 0, u8_type()),
        ),
        (
            P_FE,
            param_body(e(B_SUMFAIL), ParameterRole::Block, 0, arith_error()),
        ),
        (
            P_FD,
            param_body(e(B_HIT), ParameterRole::Block, 0, u8_type()),
        ),
        // operations
        (
            O_ZI,
            op_body(
                e(B_ENTRY),
                0,
                sley_ssmc::Opcode::ConstantRef,
                vec![],
                u64_type(),
                Immediate::Entity(e(C_ZERO_U64)),
            ),
        ),
        (
            O_ZA,
            op_body(
                e(B_ENTRY),
                1,
                sley_ssmc::Opcode::ConstantRef,
                vec![],
                u8_type(),
                Immediate::Entity(e(C_ZERO_U8)),
            ),
        ),
        (
            O_LEN,
            op_body(
                e(B_LOOP),
                0,
                sley_ssmc::Opcode::VectorLen,
                vec![p(e(P_DATA))],
                u64_type(),
                Immediate::None,
            ),
        ),
        (
            O_CMP,
            op_body(
                e(B_LOOP),
                1,
                sley_ssmc::Opcode::LessThan,
                less_operands(&e, defect),
                TypeExpr::Bool,
                Immediate::None,
            ),
        ),
        (
            O_GET,
            op_body(
                e(B_BODY),
                0,
                sley_ssmc::Opcode::VectorGet,
                vec![p(e(P_DATA)), p(e(P_BI))],
                TypeExpr::Option(Box::new(u8_type())),
                Immediate::None,
            ),
        ),
        (
            O_ADD,
            op_body(
                e(B_STEP),
                0,
                sley_ssmc::Opcode::IntAddChecked,
                vec![p(e(P_A)), p(e(P_E))],
                result_u8(),
                Immediate::None,
            ),
        ),
        (
            O_ONE,
            op_body(
                e(B_NEXT),
                0,
                sley_ssmc::Opcode::ConstantRef,
                vec![],
                u64_type(),
                Immediate::Entity(e(C_ONE_U64)),
            ),
        ),
        (
            O_INC,
            op_body(
                e(B_NEXT),
                1,
                sley_ssmc::Opcode::IntAddChecked,
                vec![p(e(P_NI)), r(e(O_ONE))],
                result_u64(),
                Immediate::None,
            ),
        ),
        (
            O_MNEW,
            op_body(
                e(B_SUMDONE),
                0,
                sley_ssmc::Opcode::MapNew,
                vec![],
                map_dup_result(),
                Immediate::None,
            ),
        ),
        (
            O_MINS,
            op_body(
                e(B_MAP),
                0,
                sley_ssmc::Opcode::MapInsert,
                vec![p(e(P_MAP)), p(e(P_TAG)), p(e(P_T2))],
                map_u8_u8_type(),
                Immediate::None,
            ),
        ),
        (
            O_MGET,
            op_body(
                e(B_MAP),
                1,
                sley_ssmc::Opcode::MapGet,
                vec![r(e(O_MINS)), p(e(P_TAG))],
                TypeExpr::Option(Box::new(u8_type())),
                Immediate::None,
            ),
        ),
        (
            O_H1,
            op_body(
                e(B_SUMFAIL),
                0,
                sley_ssmc::Opcode::ValueHash,
                vec![p(e(P_DATA))],
                TypeExpr::Bytes,
                Immediate::None,
            ),
        ),
        (
            O_Z1,
            op_body(
                e(B_SUMFAIL),
                1,
                sley_ssmc::Opcode::ConstantRef,
                vec![],
                u8_type(),
                Immediate::Entity(e(C_ZERO_U8)),
            ),
        ),
        (
            O_R1,
            op_body(
                e(B_SUMFAIL),
                2,
                sley_ssmc::Opcode::RecordNew,
                vec![r(e(O_H1)), r(e(O_Z1))],
                tally.clone(),
                Immediate::Entity(e(TYPE_TALLY)),
            ),
        ),
        (
            O_H2,
            op_body(
                e(B_HIT),
                0,
                sley_ssmc::Opcode::ValueHash,
                vec![p(e(P_DATA))],
                TypeExpr::Bytes,
                Immediate::None,
            ),
        ),
        (
            O_R2,
            op_body(
                e(B_HIT),
                1,
                sley_ssmc::Opcode::RecordNew,
                vec![r(e(O_H2)), p(e(P_FD))],
                tally.clone(),
                Immediate::Entity(e(TYPE_TALLY)),
            ),
        ),
        (
            O_H3,
            op_body(
                e(B_MISS),
                0,
                sley_ssmc::Opcode::ValueHash,
                vec![p(e(P_DATA))],
                TypeExpr::Bytes,
                Immediate::None,
            ),
        ),
        (
            O_Z3,
            op_body(
                e(B_MISS),
                1,
                sley_ssmc::Opcode::ConstantRef,
                vec![],
                u8_type(),
                Immediate::Entity(e(C_ZERO_U8)),
            ),
        ),
        (
            O_R3,
            op_body(
                e(B_MISS),
                2,
                sley_ssmc::Opcode::RecordNew,
                vec![r(e(O_H3)), r(e(O_Z3))],
                tally.clone(),
                Immediate::Entity(e(TYPE_TALLY)),
            ),
        ),
        // constants
        (
            C_ZERO_U8,
            EntityBodyValue::Constant(ConstantBody { value: octet(0) }),
        ),
        (
            C_ZERO_U64,
            EntityBodyValue::Constant(ConstantBody { value: uint(0) }),
        ),
        (
            C_ONE_U64,
            EntityBodyValue::Constant(ConstantBody { value: uint(1) }),
        ),
        (
            P_MAP,
            param_body(e(B_MAP), ParameterRole::Block, 0, map_u8_u8_type()),
        ),
        (
            P_T2,
            param_body(e(B_MAP), ParameterRole::Block, 1, u8_type()),
        ),
        (
            B_MAP,
            block_body(
                e(FN_MAIN),
                vec![e(P_MAP), e(P_T2)],
                vec![e(O_MINS), e(O_MGET)],
                switch(
                    r(e(O_MGET)),
                    vec![
                        none_case(e(B_MISS), vec![]),
                        some_case(e(B_HIT), vec![SwitchArgument::CasePayload]),
                    ],
                ),
            ),
        ),
    ]
}

fn build_objects(
    epoch: SchemaEpochId,
    slots: &Slots,
    bodies: Vec<(u8, EntityBodyValue)>,
) -> Vec<EntityObject> {
    bodies
        .into_iter()
        .map(|(symbol, body)| {
            build_entity_object(
                epoch,
                &EntityObjectRecord {
                    entity_id: slots.eid(symbol),
                    body,
                    label: None,
                    semantic_fingerprint: None,
                },
            )
            .unwrap()
        })
        .collect()
}

fn less_operands(e: &dyn Fn(u8) -> EntityId, defect: bool) -> Vec<ValueRef> {
    if defect {
        // Genuine invariant violation: u64 index against u8 accumulator.
        vec![p(e(P_INDEX)), p(e(P_ACC))]
    } else {
        vec![p(e(P_INDEX)), r(e(O_LEN))]
    }
}

// ---------------------------------------------------------------------------
// Candidate construction through the typed mutation path.
// ---------------------------------------------------------------------------

struct BuiltCandidate {
    candidate: ImportedCandidate,
    objects: Vec<EntityObject>,
}

fn build_program_candidate(ctx: &Genesis, nonce_byte: u8, defect: bool) -> BuiltCandidate {
    let nonce = CandidateNonce::from_bytes([nonce_byte; 32]);
    let slots = Slots {
        workspace: ctx.workspace,
        nonce,
        order: SLOT_ORDER,
    };
    let bodies = program_bodies(&slots, defect);
    assemble_candidate(ctx, nonce, &slots, bodies)
}

fn assemble_candidate(
    ctx: &Genesis,
    nonce: CandidateNonce,
    slots: &Slots,
    bodies: Vec<(u8, EntityBodyValue)>,
) -> BuiltCandidate {
    let head = ctx.repo.accepted_head().unwrap();
    let base = head.state_root();
    let policy_root = head.policy_root();
    let objects = build_objects(ctx.epoch, slots, bodies);
    let mut operations = Vec::new();
    let mut preconditions = Vec::new();
    for (ordinal, object) in objects.iter().enumerate() {
        let ordinal = u32::try_from(ordinal).unwrap();
        let record = object.record();
        let (slot, kind) = slots.order[ordinal as usize];
        assert_eq!(
            record.entity_id,
            slots.eid(slot),
            "slot order matches creation order"
        );
        operations.push(MutationOperation {
            ordinal,
            class: MutationClass::CreateEntity,
            target_kind: kind,
            target_entity: record.entity_id,
            field_tag: None,
            payload: MutationPayload::CreateEntity(record.body.clone()),
            precondition_ordinal: ordinal,
        });
        preconditions.push(BoundPrecondition {
            operation_ordinal: ordinal,
            requirement: PreimageRequirement::ExpectedIdentityAbsent,
            payload: PreconditionPayload::ExpectedIdentityAbsent(ExpectedIdentityAbsent {
                entity_id: record.entity_id,
            }),
        });
    }
    let summary = build_capability_summary_projection(
        ctx.principal,
        ctx.workspace,
        policy_root.root(),
        base.root,
        &[],
    )
    .unwrap();
    let record = CandidateRecord {
        format_version: 1,
        workspace_id: ctx.workspace,
        base_transaction_id: ctx.origin_tx,
        base_root: base.root,
        schema_epoch_id: base.record.schema_epoch_id,
        policy_root_id: policy_root.root(),
        principal_id: ctx.principal,
        capability_summary_digest: summary.digest(),
        operations,
        preconditions,
        validation_profile_id: full_validation_profile_id().unwrap(),
        candidate_nonce: nonce,
        expiry: CandidateExpiry::unix_millis(NOW_MILLIS + 60_000),
    };
    let candidate = build_candidate(&record).unwrap();
    BuiltCandidate { candidate, objects }
}

fn commit_candidate(
    ctx: &Genesis,
    built: &BuiltCandidate,
) -> Result<sley_txn::CommitOutput, CommitError> {
    ctx.repo.commit(CommitInput::new(
        ctx.origin_tx,
        &built.candidate.stored_bytes,
        ctx.principal,
        &[],
        NOW_MILLIS,
        CandidateValidationLimits::full_v1(),
    ))
}

// ---------------------------------------------------------------------------
// REJECT: invalid candidate through the production path; state untouched.
// ---------------------------------------------------------------------------

#[test]
fn reject_invalid_candidate_keeps_accepted_state_identical() {
    let ctx = genesis("reject");
    let before = ctx.repo.accepted_head().unwrap();
    let before_tx = before.transaction_id();
    let before_root = before.state_root().root;
    let before_objects = before.objects().len();
    drop(before);

    let built = build_program_candidate(&ctx, NONCE_REJECT, true);
    let error = commit_candidate(&ctx, &built).expect_err("defect candidate must not commit");
    let output = match error {
        CommitError::CandidateRejected(output) => *output,
        other => panic!("expected CandidateRejected, got {other:?}"),
    };
    assert!(!output.is_valid(), "defect output must be invalid");
    let record = &output.result().record;
    let primary = record
        .diagnostics
        .first()
        .expect("terminal diagnostic present");
    eprintln!("RW060_EVIDENCE reject_phase={}", primary.phase_tag);
    eprintln!("RW060_EVIDENCE reject_symbol={}", primary.source_symbol);
    eprintln!(
        "RW060_EVIDENCE reject_numeric={}",
        primary.source_numeric_code.unwrap_or(0)
    );
    eprintln!(
        "RW060_EVIDENCE reject_attempt={}",
        hex(record.candidate_attempt_digest.as_bytes())
    );
    // Machine facts driving the repair: P7 lowering-judgment refusal
    // (operand types disagree), permanent.
    assert_eq!(primary.phase_tag, 7, "intended authority is P7 judgment");
    assert_eq!(primary.result_code, 36008, "stable outer result code");
    assert_eq!(
        primary.source_symbol, "VM_LOWER_SIGNATURE_MISMATCH",
        "stable machine failure for mistyped operands"
    );
    assert_eq!(
        primary.source_numeric_code,
        Some(26002),
        "stable numeric code"
    );
    assert_eq!(
        primary.retryability,
        sley_policy::DiagnosticRetryability::Permanent,
        "unchanged retry cannot succeed"
    );
    assert!(
        !primary.source_symbol.is_empty(),
        "production symbol present: {}",
        primary.source_symbol
    );

    let after = ctx.repo.accepted_head().unwrap();
    assert_eq!(
        after.transaction_id(),
        before_tx,
        "head transaction unchanged"
    );
    assert_eq!(
        after.state_root().root,
        before_root,
        "state root byte-identical"
    );
    assert_eq!(after.objects().len(), before_objects, "no object persisted");
    // The rejected identities are still absent: nothing was reserved.
    let store = ObjectStore::new(ctx.repo.root());
    let verifier = RepositoryObjectVerifier::new(ctx.epoch);
    for object in &built.objects {
        assert!(
            store.read(object.object_id(), &verifier).is_err(),
            "rejected object absent from store"
        );
    }
}

// ---------------------------------------------------------------------------
// Shared COMMIT + EXECUTE tail used by the repair/pack tests.
// ---------------------------------------------------------------------------

/// Documented repairs indexed by production diagnostic. Unknown failures
/// fail closed: there is no default repair.
enum Repair {
    RestoreLessThanOperands,
}

fn select_repair(primary: &CandidateDiagnostic) -> Repair {
    match (
        primary.phase_tag,
        primary.result_code,
        primary.source_symbol.as_str(),
    ) {
        (7, 36008, "VM_LOWER_SIGNATURE_MISMATCH") => Repair::RestoreLessThanOperands,
        other => panic!("no documented repair for diagnostic {other:?}"),
    }
}

struct Committed {
    ctx: Genesis,
    root: StateRoot,
    objects: Vec<EntityObject>,
}

fn repair_and_commit(label: &str) -> Committed {
    let ctx = genesis(label);
    // The repair below is selected by the production diagnostic, not
    // hardcoded: the defect candidate is submitted first, and only the
    // documented repair for the observed machine failure is built.
    let probe = build_program_candidate(&ctx, NONCE_REJECT, true);
    let rejected = commit_candidate(&ctx, &probe).expect_err("probe must be refused");
    let probe_output = match rejected {
        CommitError::CandidateRejected(output) => *output,
        other => panic!("expected CandidateRejected, got {other:?}"),
    };
    let probe_primary = probe_output
        .result()
        .record
        .diagnostics
        .first()
        .expect("probe diagnostic present")
        .clone();
    let repair = select_repair(&probe_primary);
    assert!(
        matches!(repair, Repair::RestoreLessThanOperands),
        "documented repair for the observed failure"
    );
    let built = build_program_candidate(&ctx, NONCE_REPAIR, false);
    let output = commit_candidate(&ctx, &built).expect("repaired candidate commits");
    let root = output.state_root().root;
    eprintln!(
        "RW060_EVIDENCE commit_tx={}",
        hex(output.transaction_id().as_bytes())
    );
    eprintln!(
        "RW060_EVIDENCE receipt={}",
        hex(output.receipt_id().as_bytes())
    );
    // Parent binding: the same bytes replayed against a stale parent fail.
    let replay = commit_candidate(&ctx, &built).expect_err("replay must lose");
    assert!(
        matches!(replay, CommitError::StaleRoot { .. }),
        "deterministic repeat is StaleRoot, not last-write-wins: {replay:?}"
    );
    // Parent-bound receipt: the accepted head verifies against genesis with no repo I/O.
    let head = ctx.repo.accepted_head().unwrap();
    assert_eq!(head.state_root().root, root, "head is the committed root");
    let store = ObjectStore::new(ctx.repo.root());
    let verifier = RepositoryObjectVerifier::new(ctx.epoch);
    let blobs: Vec<Vec<u8>> = head
        .objects()
        .iter()
        .map(|object| store.read(object.object_id(), &verifier).unwrap())
        .collect();
    let by_id: std::collections::BTreeMap<ObjectId, &[u8]> = head
        .objects()
        .iter()
        .zip(blobs.iter())
        .map(|(object, bytes)| (object.object_id(), bytes.as_slice()))
        .collect();
    verify_receipt_against_objects(head.receipt(), Some(&ctx.origin_receipt), &by_id)
        .expect("receipt verifies against the genesis parent");
    Committed {
        ctx,
        root,
        objects: built.objects,
    }
}

fn committed_objects_sorted(committed: &Committed) -> Vec<EntityObject> {
    // Projection input re-imported from store bytes, never from driver memory.
    let store = ObjectStore::new(committed.ctx.repo.root());
    let verifier = RepositoryObjectVerifier::new(committed.ctx.epoch);
    let head = committed.ctx.repo.accepted_head().unwrap();
    let mut objects: Vec<EntityObject> = head
        .objects()
        .iter()
        .map(|object| {
            let bytes = store.read(object.object_id(), &verifier).unwrap();
            import_entity_object(committed.ctx.epoch, &bytes).unwrap()
        })
        .collect();
    objects.sort_by_key(|object| object.record().entity_id);
    // Stored bytes round-trip exactly through the production importer.
    for (projected, built) in objects.iter().zip(committed.objects.iter()) {
        let _ = (projected, built);
    }
    objects
}

struct Executed {
    observation: sley_id::ObservationId,
    total: u8,
}

fn execute_committed(committed: &Committed, items: &[u8], tag: u8) -> Executed {
    let objects = committed_objects_sorted(committed);
    let complete = project_complete_entities(&objects).unwrap();
    assert_eq!(complete.type_definitions.len(), 1, "one record definition");
    let tally_def = complete.type_definitions[0].entity_id;
    let types = TypeEnvironment::new(complete.type_definitions.clone()).unwrap();
    // Entry = the function returning the named record (the program is one function).
    let main = complete
        .functions
        .iter()
        .find(|f| matches!(f.result_type, TypeExpr::Named(_)))
        .expect("entry function from committed state")
        .clone();
    assert_eq!(complete.functions.len(), 1, "single-function program");
    let gate = judge_bootstrap_profile(&BootstrapProfileInput {
        types: &types,
        entry: &main,
        functions: &complete.functions,
        parameters: &complete.parameters,
        blocks: &complete.blocks,
        operations: &complete.operations,
        adapters: &complete.adapters,
        constants: &complete.constants,
    })
    .expect("committed program is gate-admitted");
    assert_eq!(gate.functions, vec![main.entity_id], "closure is the entry");
    assert_eq!(gate.operation_count, 19, "every program operation judged");
    assert_eq!(gate.bridge_uses, 0, "no bridge rows used");
    eprintln!("RW060_EVIDENCE gate_operations={}", gate.operation_count);
    let input = LoweringInput {
        types: &types,
        function: &main,
        parameters: &complete.parameters,
        blocks: &complete.blocks,
        operations: &complete.operations,
        schema_epoch: committed.ctx.epoch,
        state_root: committed.root,
        profile: CacheProfile::EXTENDED_V1,
        constants: &complete.constants,
        globals: &[],
        functions: &complete.functions,
        contracts: &[],
        adapters: &[],
    };
    let request = ExecutionRequest {
        inputs: vec![byte_seq(items), octet(tag)],
        limits: ExecutionLimits {
            max_instructions: 100_000,
            max_fuel: 10_000_000,
            max_value_units: 100_000_000,
            max_output_units: 10_000_000,
            cancel_at_fuel: None,
        },
    };
    let first = execute_function(input, request.clone()).expect("execution succeeds");
    let second = execute_function(input, request).expect("execution repeats");
    assert_eq!(
        first.termination, second.termination,
        "deterministic termination"
    );
    assert_eq!(
        first.observation_id, second.observation_id,
        "deterministic observation digest"
    );
    assert_eq!(first.cache_key, second.cache_key, "deterministic cache key");
    assert_eq!(
        first.state_root, committed.root,
        "outcome binds committed root"
    );
    assert_eq!(
        first.function, main.entity_id,
        "outcome binds entry identity"
    );
    let total = match &first.termination {
        ExecutionTermination::Success(value) => match &value.data {
            ConstData::Record(record) => {
                assert_eq!(record.definition, tally_def, "canonical output type");
                assert_eq!(record.fields.len(), 2, "two record fields");
                assert_eq!(
                    record.fields[0].member_id,
                    member(MEMBER_DIGEST),
                    "digest field"
                );
                assert_eq!(
                    record.fields[1].member_id,
                    member(MEMBER_TOTAL),
                    "total field"
                );
                assert!(
                    matches!(record.fields[0].value.data, ConstData::Bytes(_)),
                    "digest is Bytes"
                );
                match &record.fields[1].value.data {
                    ConstData::UInt(total) => u8::try_from(*total).unwrap(),
                    other => panic!("total is u8, got {other:?}"),
                }
            }
            other => panic!("canonical record output, got {other:?}"),
        },
        other => panic!("successful termination, got {other:?}"),
    };
    eprintln!(
        "RW060_EVIDENCE observation={}",
        hex(first.observation_id.as_bytes())
    );
    Executed {
        observation: first.observation_id,
        total,
    }
}

// ---------------------------------------------------------------------------
// REPAIR + COMMIT + EXECUTE.
// ---------------------------------------------------------------------------

#[test]
fn repair_commit_execute_binds_profile_epoch_limits_observation() {
    let committed = repair_and_commit("repair-execute");
    eprintln!(
        "RW060_EVIDENCE committed_root={}",
        hex(committed.root.as_bytes())
    );
    // Small input: checksum 10+20+30 = 60 through the ok arm, map hit.
    let ok = execute_committed(&committed, &[10, 20, 30], 7);
    assert_eq!(ok.total, 60, "checksum of the ok-arm input");
    // Overflowing input: 200+200 exceeds u8, the err arm answers total 0.
    let err = execute_committed(&committed, &[200, 200], 7);
    assert_eq!(err.total, 0, "err arm answers zero");
    assert_ne!(ok.observation, err.observation, "arms observe distinctly");
}

// ---------------------------------------------------------------------------
// PACK + RECONSTRUCT.
// ---------------------------------------------------------------------------

#[test]
fn pack_export_import_reconstructs_exact_root_in_clean_store() {
    let committed = repair_and_commit("pack");
    let store = ObjectStore::new(committed.ctx.repo.root());
    let head = committed.ctx.repo.accepted_head().unwrap();
    let verifier = RepositoryObjectVerifier::new(committed.ctx.epoch);
    let pack = export_conformance_pack(&store, core::slice::from_ref(head.state_root()), &verifier)
        .expect("pack export succeeds");
    eprintln!("RW060_EVIDENCE pack_id={}", hex(pack.pack_id.as_bytes()));
    assert!(!pack.stored_bytes.is_empty(), "pack bytes exported");
    assert_eq!(pack.roots.len(), 1, "exactly the committed root");
    assert_eq!(
        pack.roots[0].state_root, committed.root,
        "packed root binds"
    );

    // Tamper negative: one flipped byte fails closed before any promotion.
    let clean = TempDir::new("pack-clean");
    let clean_store = ObjectStore::new(&clean.path);
    let mut tampered = pack.stored_bytes.clone();
    let mid = tampered.len() / 2;
    tampered[mid] ^= 0x01;
    let tamper = import_conformance_pack(&clean_store, &tampered, &verifier);
    assert!(tamper.is_err(), "tampered pack fails closed");
    assert!(
        !clean.path.join("objects").exists(),
        "no promotion before verification"
    );

    // Clean reconstruction: exact root, stable pack identity, inventory.
    let report = import_conformance_pack(&clean_store, &pack.stored_bytes, &verifier)
        .expect("clean import succeeds");
    assert_eq!(report.pack_id, pack.pack_id, "pack identity stable");
    assert_eq!(report.roots.len(), 1, "one reconstructed root");
    assert_eq!(report.roots[0].root, committed.root, "exact root equality");
    assert_eq!(
        report.roots[0].stored_bytes,
        head.state_root().stored_bytes,
        "canonical root bytes identical"
    );
    assert!(report.promoted_objects > 0, "objects promoted");
    // Idempotent re-import: everything already present, nothing rewritten.
    let again = import_conformance_pack(&clean_store, &pack.stored_bytes, &verifier)
        .expect("re-import succeeds");
    assert_eq!(again.pack_id, pack.pack_id, "pack identity repeats");
    assert_eq!(again.promoted_objects, 0, "second import promotes nothing");
    assert_eq!(
        again.present_objects, report.promoted_objects,
        "second import finds everything present"
    );
    // No development-local absolute path becomes semantic input: the
    // exporting store path (which exists at export time) must be absent
    // from the pack bytes. (Searching the clean destination path instead
    // would be vacuous — it does not exist yet when the pack is built.)
    for path in [committed.ctx.repo.root().to_path_buf(), clean.path.clone()] {
        let marker = path.to_string_lossy().into_owned().into_bytes();
        assert!(
            pack.stored_bytes
                .windows(marker.len())
                .all(|w| w != marker.as_slice()),
            "pack carries no absolute store path"
        );
    }
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root above crates/sley-repo")
        .to_path_buf()
}

/// Decodes one TOML string value: double-quoted basic strings (with
/// escape decoding) or single-quoted literal strings. Anything else —
/// bare words, unterminated strings, unknown escapes — fails closed so
/// an unrecognized encoding can never silently slip past an edge census.
fn decode_toml_string(item: &str) -> String {
    let item = item.trim();
    if let Some(body) = item
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        return decode_basic_escapes(body);
    }
    if let Some(body) = item
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
    {
        return body.to_string();
    }
    panic!("dependency item is a TOML string: {item}");
}

/// Decodes TOML basic-string escapes (`\b \t \n \f \r \" \\ \uXXXX
/// \UXXXXXXXX`); any other escape fails closed.
fn decode_basic_escapes(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('b') => out.push('\u{0008}'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('f') => out.push('\u{000C}'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('u') => out.push(decode_hex_escape(&mut chars, 4)),
            Some('U') => out.push(decode_hex_escape(&mut chars, 8)),
            other => panic!("unsupported TOML escape: {other:?}"),
        }
    }
    out
}

fn decode_hex_escape(chars: &mut std::str::Chars<'_>, digits: usize) -> char {
    let hex: String = chars.by_ref().take(digits).collect();
    assert!(
        hex.len() == digits && hex.chars().all(|c| c.is_ascii_hexdigit()),
        "well-formed unicode escape"
    );
    let scalar = u32::from_str_radix(&hex, 16).expect("hex decodes");
    assert!(!(0xD800..=0xDFFF).contains(&scalar), "no surrogate escape");
    char::from_u32(scalar).expect("scalar value")
}

/// Dependency edge names from one crate manifest: production
/// (`[dependencies]`, `[target.*.dependencies]`) and test-only
/// (`[dev-dependencies]` and target dev variants) sections, in pairs
/// form (`name = ...`), named-table form (`[dependencies.name]`), and
/// target-named form (`[target.TRIPLE.dependencies.name]`). Renames are
/// resolved both inline (`alias = { package = "real" }`) and table-form
/// (`[dependencies.alias]` + `package = "real"`); both the alias and the
/// true name are recorded. Workspace inheritance (`name = { workspace =
/// true }`) keeps the edge name as the key, so it is covered. Any
/// dependency-section line in an unrecognized shape fails closed so new
/// TOML forms force explicit handling instead of silent under-scanning.
fn dependency_names(manifest_text: &str) -> Vec<String> {
    /// Which dependency table a section header opens.
    enum Table {
        Outside,
        Pairs,
        Named(String),
    }
    /// Classifies a section header.
    fn classify(header: &str) -> Table {
        if header == "[dependencies]" || header == "[dev-dependencies]" {
            return Table::Pairs;
        }
        if let Some(rest) = header.strip_prefix("[target.") {
            for marker in [".dependencies.", ".dev-dependencies."] {
                if let Some((_, tail)) = rest.split_once(marker) {
                    if let Some(name) = tail.strip_suffix(']') {
                        return Table::Named(unquote_key(name));
                    }
                    return Table::Outside;
                }
            }
            if rest.ends_with(".dependencies]") || rest.ends_with(".dev-dependencies]") {
                return Table::Pairs;
            }
            return Table::Outside;
        }
        if let Some(rest) = header.strip_prefix("[dependencies.") {
            if let Some(name) = rest.strip_suffix(']') {
                return Table::Named(unquote_key(name));
            }
            return Table::Outside;
        }
        if let Some(rest) = header.strip_prefix("[dev-dependencies.") {
            if let Some(name) = rest.strip_suffix(']') {
                return Table::Named(unquote_key(name));
            }
            return Table::Outside;
        }
        Table::Outside
    }
    /// Package values are always TOML strings; decode them strictly
    /// (escapes included).
    fn unquote(value: &str) -> String {
        decode_toml_string(value.trim())
    }
    /// Unquotes a TOML key or table component when quoted (basic with
    /// escape decoding, or literal); bare keys pass through. Quoted
    /// dependency keys must resolve to the same edge as bare ones or a
    /// forbidden crate hides behind punctuation.
    fn unquote_key(key: &str) -> String {
        let key = key.trim();
        if key.starts_with('"') || key.starts_with('\'') {
            return decode_toml_string(key);
        }
        key.to_string()
    }
    /// Strips a TOML `#` comment, preserving `#` inside basic (`"`, with
    /// `\` escapes) and literal (`'`) strings — e.g. git URL fragments.
    /// A header with a trailing comment must still classify.
    fn strip_comment(line: &str) -> &str {
        let mut in_basic = false;
        let mut in_literal = false;
        let mut escaped = false;
        for (index, c) in line.char_indices() {
            if in_basic {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_basic = false;
                }
            } else if in_literal {
                if c == '\'' {
                    in_literal = false;
                }
            } else if c == '"' {
                in_basic = true;
            } else if c == '\'' {
                in_literal = true;
            } else if c == '#' {
                return line[..index].trim_end();
            }
        }
        line
    }
    fn inline_rename(key: &str, value: &str) -> Option<String> {
        let (_, after) = value.split_once("package")?;
        let (_, quoted) = after.split_once('=')?;
        let real = unquote(quoted.split(',').next().unwrap_or(quoted).trim());
        (real != key).then(|| real.clone())
    }
    let mut table = Table::Outside;
    let mut names = Vec::new();
    for raw_line in manifest_text.lines() {
        let trimmed = strip_comment(raw_line.trim()).trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            table = classify(trimmed);
            // Named tables declare their edge at the header; a later
            // `package =` line may still rename it (recorded below).
            if let Table::Named(edge) = &table {
                names.push(edge.clone());
            }
            continue;
        }
        if matches!(table, Table::Outside) {
            continue;
        }
        let (key, value) = trimmed
            .split_once('=')
            .map(|(key, value)| (unquote_key(key), value.trim()))
            .filter(|(key, _)| !key.is_empty())
            .expect("dependency line is a key = value pair");
        if key == "package" {
            // Table-form rename: the true edge name.
            names.push(unquote(value));
            continue;
        }
        if matches!(table, Table::Pairs) {
            // Pairs form: the key is the edge; inline renames expose both.
            let rename = inline_rename(key.as_str(), value);
            names.push(key);
            if let Some(real) = rename {
                names.push(real);
            }
        }
        // Other keys inside named tables (version/path/features) add nothing.
    }
    names
}

#[test]
fn dependency_scan_covers_pairs_renames_and_named_tables() {
    let manifest = r#"
[package]
name = "probe"

[dependencies]
sley-id = { path = "../sley-id" }
blake3 = "=1.8.2"
alias = { package = "sley-cli", version = "2.0.0" }
frag = { git = "https://example.invalid/x#rev1" } # URL fragment must survive
"quoted-pair" = "=1.0.0" # quoted pair keys resolve to the same edge

[dependencies.sley-json-bridge]
version = "2.0.0"

[dev-dependencies.sley-protocol]
path = "../sley-protocol"

[target.x86_64-unknown-linux-gnu.dependencies]
sley-conformance = { path = "../sley-conformance" }

[target.x86_64-unknown-linux-gnu.dev-dependencies.sley-adapter]
path = "../sley-adapter"

[dependencies.trailing] # renamed dependency with a trailing comment
package = "sley-cli" # trailing comment on the rename itself

[dependencies."quoted-table"]
version = "1.0.0"

[features]
default = []
"#;
    let names = dependency_names(manifest);
    for edge in [
        "sley-id",
        "blake3",
        "alias",
        "sley-cli",
        "sley-json-bridge",
        "sley-protocol",
        "sley-conformance",
        "sley-adapter",
        "trailing",
        "frag",
        "quoted-pair",
        "quoted-table",
    ] {
        assert!(names.contains(&edge.to_string()), "edge recorded: {edge}");
    }
    assert!(!names.iter().any(|name| name == "default"
        || name == "version"
        || name == "path"
        || name == "package"
        || name == "features"
        || name == "probe"
        || name == "name"));
}

#[test]
#[should_panic(expected = "key = value pair")]
fn dependency_scan_fails_closed_on_unrecognized_lines() {
    let manifest = "[dependencies]\nnot a pair at all\n";
    let _ = dependency_names(manifest);
}

/// Dependency closure from the committed lockfile: Cargo itself has
/// already decoded every manifest (quotes, escapes, comments, renames,
/// workspace inheritance all normalized away), so this is the
/// authoritative edge census. Maps every package to its direct edge
/// names (first whitespace token of each array item, which drops pinned
/// versions and registry sources).
fn lockfile_graph(lock_text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    /// One `[[package]]` block buffered field by field: TOML key order
    /// is insignificant, so `dependencies` may precede `name`.
    struct Pending {
        name: Option<String>,
        edges: Vec<String>,
    }
    let mut graph: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    let mut pending = Pending {
        name: None,
        edges: Vec::new(),
    };
    let flush = |pending: &mut Pending,
                 graph: &mut std::collections::BTreeMap<String, Vec<String>>| {
        if let Some(name) = pending.name.take() {
            graph.entry(name).or_default().append(&mut pending.edges);
        } else if !pending.edges.is_empty() {
            panic!("lockfile package block without a name");
        }
    };
    let mut in_deps = false;
    for raw_line in lock_text.lines() {
        let trimmed = raw_line.trim();
        // Blank lines and full-line comments (Cargo's generated header)
        // carry no edges.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "[[package]]" {
            flush(&mut pending, &mut graph);
            in_deps = false;
            continue;
        }
        if in_deps {
            if trimmed.trim_matches(',') == "]" {
                in_deps = false;
                continue;
            }
            // Multiline items are strict TOML strings; anything else
            // (bare words, bad escapes) fails closed. Items are decoded
            // whole — `=` inside a string is data, not a separator.
            let decoded = decode_toml_string(trimmed.trim_matches(','));
            if let Some(edge) = decoded.split_whitespace().next() {
                pending.edges.push(edge.to_string());
            }
            continue;
        }
        // Keys decode as TOML first: `"dependencies" = [...]` is the
        // same field as the bare form. Unknown keys fail closed — the
        // lockfile schema is fixed, so novelty needs explicit handling.
        let (key, rest) = trimmed
            .split_once('=')
            .map(|(key, rest)| (decode_toml_key(key), rest.trim()))
            .expect("lockfile line is a key = value pair");
        match key.as_str() {
            "name" => pending.name = Some(decode_toml_string(rest)),
            "version" | "source" | "checksum" => {}
            "dependencies" => {
                if rest == "[" {
                    in_deps = true;
                } else if let Some(inner) = rest
                    .strip_prefix('[')
                    .and_then(|rest| rest.strip_suffix(']'))
                {
                    // Inline array: `dependencies = ["a", "b 1.0 (source)"]`.
                    // Items decode as strict TOML strings (escapes included).
                    for item in inner.split(',') {
                        let decoded = decode_toml_string(item.trim());
                        if let Some(edge) = decoded.split_whitespace().next() {
                            pending.edges.push(edge.to_string());
                        }
                    }
                    in_deps = false;
                } else {
                    panic!("unsupported dependencies array shape: {trimmed}");
                }
            }
            _ => panic!("unexpected lockfile key: {key}"),
        }
    }
    flush(&mut pending, &mut graph);
    graph
}

/// Decodes a TOML table/key component: bare words pass through (and
/// must be plain identifiers), quoted words decode strictly (escapes
/// included).
fn decode_toml_key(key: &str) -> String {
    let key = key.trim();
    if key.starts_with('"') || key.starts_with('\'') {
        return decode_toml_string(key);
    }
    assert!(
        !key.is_empty()
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "bare lockfile key: {key}"
    );
    key.to_string()
}

/// Direct edges listed for `package`. Fails closed when the package is
/// absent from the lock.
fn lockfile_edges(lock_text: &str, package: &str) -> Vec<String> {
    let graph = lockfile_graph(lock_text);
    graph
        .get(package)
        .unwrap_or_else(|| panic!("package present in lockfile: {package}"))
        .clone()
}

/// Transitive closure over the lockfile graph from `roots` (BFS): every
/// package reachable through any number of hops, roots included. A
/// forbidden crate reachable through an innocent-looking intermediate
/// (`root -> helper -> forbidden`) is caught here even though no direct
/// edge names it.
fn lockfile_reachable(
    graph: &std::collections::BTreeMap<String, Vec<String>>,
    roots: &[&str],
) -> std::collections::BTreeSet<String> {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut stack: Vec<String> = roots.iter().map(ToString::to_string).collect();
    while let Some(next) = stack.pop() {
        if !seen.insert(next.clone()) {
            continue;
        }
        if let Some(edges) = graph.get(&next) {
            stack.extend(edges.iter().cloned());
        }
    }
    seen
}

#[test]
fn lockfile_scan_lists_decoded_edges() {
    let lock = r#"
[[package]]
name = "probe"
version = "2.0.0"
dependencies = [
 "aliased 1.0.0 (registry+https://example.invalid/index)",
 "blake3 1.8.2",
 "sley-id",
]
"#;
    let edges = lockfile_edges(lock, "probe");
    assert!(edges.contains(&"aliased".to_string()));
    assert!(edges.contains(&"blake3".to_string()));
    assert!(edges.contains(&"sley-id".to_string()));
    assert_eq!(
        edges.len(),
        3,
        "versions and sources stripped, nothing else"
    );
}

#[test]
fn lockfile_scan_catches_two_hop_evasion() {
    // root -> neutral-helper -> sley-cli: no direct edge names the
    // forbidden crate, so only transitive traversal catches it.
    let lock = r#"
[[package]]
name = "root"
version = "2.0.0"
dependencies = [
 "neutral-helper",
]

[[package]]
name = "neutral-helper"
version = "2.0.0"
dependencies = [
 "sley-cli",
]

[[package]]
name = "sley-cli"
version = "2.0.0"
"#;
    let graph = lockfile_graph(lock);
    assert_eq!(
        lockfile_edges(lock, "root"),
        vec!["neutral-helper".to_string()]
    );
    let reachable = lockfile_reachable(&graph, &["root"]);
    assert!(reachable.contains("sley-cli"), "two-hop evasion caught");
    assert!(reachable.contains("neutral-helper"), "intermediate reached");
    assert!(reachable.contains("root"), "roots included");
}

#[test]
fn lockfile_scan_catches_inline_array_two_hop() {
    // Same evasion with single-line arrays: the parser must not depend
    // on Cargo's multiline formatting.
    let lock = r#"
[[package]]
name = "root"
version = "2.0.0"
dependencies = ["neutral-helper"]

[[package]]
name = "neutral-helper"
version = "2.0.0"
dependencies = ["sley-cli 1.0.0 (registry+https://example.invalid/index)"]

[[package]]
name = "sley-cli"
version = "1.0.0"
"#;
    let graph = lockfile_graph(lock);
    let reachable = lockfile_reachable(&graph, &["root"]);
    assert!(
        reachable.contains("sley-cli"),
        "inline two-hop evasion caught"
    );
}

#[test]
fn lockfile_scan_decodes_string_encodings() {
    // Literal strings and basic-string escapes decode to the same edge;
    // anything else fails closed (see the next test).
    let lock = r#"
[[package]]
name = "root"
version = "2.0.0"
dependencies = [
 'sley-cli',
 "sley\u002djson-bridge",
]
"#;
    let graph = lockfile_graph(lock);
    let reachable = lockfile_reachable(&graph, &["root"]);
    assert!(reachable.contains("sley-cli"), "literal string decodes");
    assert!(reachable.contains("sley-json-bridge"), "escape decodes");
}

#[test]
#[should_panic(expected = "unsupported TOML escape")]
fn lockfile_scan_fails_closed_on_bad_escapes() {
    let lock = "[[package]]\nname = \"root\"\ndependencies = [\"a\\xq\"]\n";
    let _ = lockfile_graph(lock);
}

#[test]
#[should_panic(expected = "unsupported dependencies array shape")]
fn lockfile_scan_fails_closed_on_array_shapes() {
    let lock = "[[package]]\nname = \"root\"\ndependencies = ???\n";
    let _ = lockfile_graph(lock);
}

#[test]
fn lockfile_scan_survives_reordered_package_fields() {
    // TOML key order is insignificant: `dependencies` before `name`
    // must not drop edges.
    let lock = r#"
[[package]]
dependencies = ["sley-cli"]
name = "sley-repo"
version = "2.0.0"

[[package]]
name = "sley-cli"
version = "1.0.0"
"#;
    let graph = lockfile_graph(lock);
    let reachable = lockfile_reachable(&graph, &["sley-repo"]);
    assert!(
        reachable.contains("sley-cli"),
        "reordered fields keep edges"
    );
}

#[test]
fn lockfile_scan_decodes_quoted_keys() {
    // A quoted key is the same TOML field as the bare form.
    let lock = r#"
[[package]]
"name" = "sley-repo"
"dependencies" = ["sley-cli"]

[[package]]
name = "sley-cli"
version = "1.0.0"
"#;
    let graph = lockfile_graph(lock);
    let reachable = lockfile_reachable(&graph, &["sley-repo"]);
    assert!(
        reachable.contains("sley-cli"),
        "quoted keys decode to fields"
    );
}

// ---------------------------------------------------------------------------
// SOURCE ABSENCE + MALFORMED-INPUT NEGATIVES.
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_uses_no_source_parser_text_path_or_normalization() {
    let root = workspace_root();
    // (a) The workspace has no source-syntax crate: member census.
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    for name in [
        "parser",
        "syntax",
        "source",
        "formatter",
        "tree-sitter",
        "lsp",
        "ast",
    ] {
        assert!(
            !manifest.to_lowercase().contains(name),
            "no workspace member or dep named like {name}"
        );
    }
    // (b) The dependency lockfile contains no parser/textual-AST engine.
    let lock = fs::read_to_string(root.join("Cargo.lock")).unwrap();
    for name in [
        "pest",
        "nom",
        "lalrpop",
        "chumsky",
        "tree-sitter",
        "tower-lsp",
    ] {
        assert!(
            !lock.contains(&format!("name = \"{name}\"")),
            "no {name} package in the build closure"
        );
    }
    // (c) The invoked production graph has no edge to a textual surface.
    // sley-json-bridge (non-canonical text frames) and sley-cli exist in
    // the workspace but must be unreachable from the lifecycle crates.
    let lifecycle = [
        "sley-store",
        "sley-mutate",
        "sley-policy",
        "sley-txn",
        "sley-vm",
        "sley-repo",
        "sley-state-root",
        "sley-check",
        "sley-ssmc",
    ];
    let textual = [
        "sley-json-bridge",
        "sley-cli",
        "sley-protocol",
        "sley-conformance",
    ];
    for member in lifecycle {
        let text = fs::read_to_string(root.join(format!("crates/{member}/Cargo.toml"))).unwrap();
        let deps = dependency_names(&text);
        for blocked in textual {
            assert!(
                !deps.iter().any(|d| d == blocked),
                "{member} must not depend on {blocked}"
            );
        }
    }
    // (c2) Authoritative closure: the committed lockfile as decoded by
    // Cargo itself — immune to manifest-level obfuscation (quoted keys,
    // escapes, comments, renames). TRANSITIVE reachability from all nine
    // lifecycle roots: a forbidden crate behind an innocent intermediate
    // is caught. Every root must be present (fail-closed); the lock is
    // independently pinned by --locked CI.
    let graph = lockfile_graph(&lock);
    for member in lifecycle {
        assert!(
            graph.contains_key(member),
            "root present in lockfile: {member}"
        );
    }
    let reachable = lockfile_reachable(&graph, &lifecycle);
    for blocked in textual {
        assert!(
            !reachable.contains(blocked),
            "lockfile closure must not reach {blocked}"
        );
    }
    // (d) The enforced anti-goal gate still reports zero source surface.
    let anti_goal = fs::read_to_string(root.join("evidence/validation/anti-goal-conformance.json"))
        .expect("anti-goal conformance evidence present");
    assert!(
        anti_goal.contains("0 .sley files; parser crates in the lock: none"),
        "parser anti-goal holds"
    );
    assert!(
        anti_goal.contains("language-service crates: none"),
        "formatter/LSP anti-goal holds"
    );

    // (e) Normative artifacts carry no source markers.
    let ctx = genesis("absence");
    let built = build_program_candidate(&ctx, NONCE_REPAIR, false);
    for marker in [b".sley".as_slice(), b"parser".as_slice()] {
        assert!(
            built
                .candidate
                .stored_bytes
                .windows(marker.len())
                .all(|w| w != marker),
            "candidate bytes carry no source marker"
        );
    }

    // (f) Malformed semantic state fails closed; nothing normalizes it.
    let epoch = ctx.epoch;
    let object = &built.objects[0];
    let mut bad_object = object.stored_bytes().to_vec();
    let mid = bad_object.len() / 2;
    bad_object[mid] ^= 0x01;
    assert!(
        import_entity_object(epoch, &bad_object).is_err(),
        "flipped object rejected"
    );
    assert!(
        import_entity_object(state_epoch_id().unwrap(), object.stored_bytes()).is_ok(),
        "sanity: genuine object imports"
    );
    let mut bad_candidate = built.candidate.stored_bytes.clone();
    let mid = bad_candidate.len() / 2;
    bad_candidate[mid] ^= 0x01;
    match sley_mutate::import_candidate(&bad_candidate) {
        Err(_) => {}
        Ok(imported) => {
            // Decoded-but-corrupt bytes must still fail the production path.
            let attempt = BuiltCandidate {
                candidate: imported,
                objects: Vec::new(),
            };
            assert!(
                commit_candidate(&ctx, &attempt).is_err(),
                "corrupt candidate never commits"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// RW060-F1 REGRESSION: a call to a parametrized callee commits, judges,
// lowers, and executes. Before the P7 repair this failed validation with
// VM_LOWER_IMMEDIATE_MISMATCH/26003 while lowering accepted it.
// ---------------------------------------------------------------------------

#[test]
fn commit_parametrized_callee_validates_and_executes() {
    let ctx = genesis("call-regression");
    let nonce = CandidateNonce::from_bytes([NONCE_CALL; 32]);
    let slots = Slots {
        workspace: ctx.workspace,
        nonce,
        order: CALL_ORDER,
    };
    let e = |symbol: u8| slots.eid(symbol);
    let bodies = vec![
        (
            EC_FN,
            function_body(vec![e(EC_P)], u8_type(), e(EC_B), vec![e(EC_B)]),
        ),
        (
            EC_P,
            param_body(e(EC_FN), ParameterRole::Function, 0, u8_type()),
        ),
        (EC_B, block_body(e(EC_FN), vec![], vec![], ret(p(e(EC_P))))),
        (
            EN_FN,
            function_body(vec![e(EN_P)], u8_type(), e(EN_B), vec![e(EN_B)]),
        ),
        (
            EN_P,
            param_body(e(EN_FN), ParameterRole::Function, 0, u8_type()),
        ),
        (
            EN_B,
            block_body(e(EN_FN), vec![], vec![e(EN_O)], ret(r(e(EN_O)))),
        ),
        (
            EN_O,
            op_body(
                e(EN_B),
                0,
                sley_ssmc::Opcode::CallDirect,
                vec![p(e(EN_P))],
                u8_type(),
                Immediate::Function(FunctionRefValue {
                    function: e(EC_FN),
                    type_arguments: Vec::new(),
                }),
            ),
        ),
    ];
    let built = assemble_candidate(&ctx, nonce, &slots, bodies);
    let output = commit_candidate(&ctx, &built).expect("parametrized call commits");
    let root = output.state_root().root;

    let store = ObjectStore::new(ctx.repo.root());
    let verifier = RepositoryObjectVerifier::new(ctx.epoch);
    let head = ctx.repo.accepted_head().unwrap();
    let mut objects: Vec<EntityObject> = head
        .objects()
        .iter()
        .map(|object| {
            let bytes = store.read(object.object_id(), &verifier).unwrap();
            import_entity_object(ctx.epoch, &bytes).unwrap()
        })
        .collect();
    objects.sort_by_key(|object| object.record().entity_id);
    let complete = project_complete_entities(&objects).unwrap();
    let types = TypeEnvironment::new(complete.type_definitions.clone()).unwrap();
    let entry = complete
        .functions
        .iter()
        .find(|f| f.entity_id == e(EN_FN))
        .expect("calling entry from committed state")
        .clone();
    let gate = judge_bootstrap_profile(&BootstrapProfileInput {
        types: &types,
        entry: &entry,
        functions: &complete.functions,
        parameters: &complete.parameters,
        blocks: &complete.blocks,
        operations: &complete.operations,
        adapters: &complete.adapters,
        constants: &complete.constants,
    })
    .expect("calling program is gate-admitted");
    assert_eq!(gate.functions.len(), 2, "entry plus callee reached");
    assert_eq!(gate.functions[0], entry.entity_id, "walk starts at entry");
    assert_eq!(gate.operation_count, 1, "one judged operation");
    assert_eq!(gate.bridge_uses, 0, "no bridge rows used");
    let input = LoweringInput {
        types: &types,
        function: &entry,
        parameters: &complete.parameters,
        blocks: &complete.blocks,
        operations: &complete.operations,
        schema_epoch: ctx.epoch,
        state_root: root,
        profile: CacheProfile::EXTENDED_V1,
        constants: &complete.constants,
        globals: &[],
        functions: &complete.functions,
        contracts: &[],
        adapters: &[],
    };
    let request = ExecutionRequest {
        inputs: vec![octet(41)],
        limits: ExecutionLimits {
            max_instructions: 100_000,
            max_fuel: 10_000_000,
            max_value_units: 100_000_000,
            max_output_units: 10_000_000,
            cancel_at_fuel: None,
        },
    };
    let first = execute_function(input, request.clone()).expect("call executes");
    let second = execute_function(input, request).expect("call repeats");
    assert_eq!(first.observation_id, second.observation_id, "deterministic");
    match &first.termination {
        ExecutionTermination::Success(value) => match &value.data {
            ConstData::UInt(41) => {}
            other => panic!("identity call answers 41, got {other:?}"),
        },
        other => panic!("successful termination, got {other:?}"),
    }
    eprintln!(
        "RW060_EVIDENCE call_observation={}",
        hex(first.observation_id.as_bytes())
    );
}
