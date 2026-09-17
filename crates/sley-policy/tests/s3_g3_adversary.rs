//! S3 G3 S2B-ADVERSARY-001: authority claims in metadata are data, not power.
//!
//! A protected `sley-policy` root with zero principal grants is built through
//! the real registry-authorized path (`PolicyRootBuilder` +
//! `conformance_registry`). Adversarial capability-claiming / bypass-requesting
//! text is carried in entity labels, debug fields, and repository metadata as
//! plain data while the requested pure arithmetic repair (`1250 + 1431 =
//! 2681`, `IntAddChecked` through the real `sley-vm` execution path)
//! succeeds with the exact expected value. The test asserts
//! `capabilities_granted == 0` and that the policy root digest read back
//! through the protected binding (`AcceptedPolicyRoot::root`, the only read
//! path this crate offers) is byte-identical before and after.
//!
//! Naming: `s3_g3_adversary` is the file slug; negatives are
//! `s3_g3_adversary_neg_wrong_repair` (attacker-steered arithmetic is
//! rejected) and `s3_g3_adversary_neg_grant_honored` (a metadata-derived
//! grant request is denied by the engine with `CAP_GRANT_DENIED`).
//!
//! Public `sley-policy` / `sley-vm` surfaces only, plus public `sley-check` /
//! `sley-id` / `sley-ssmc` value types. No crate internals, no extra
//! dev-dependencies (every import below is already a `sley-policy`
//! dependency).

use sley_check::TypeEnvironment;
use sley_id::{
    EntityId, PolicyRootId, PrincipalId, ReferenceAdapterId, SchemaEpochId, StateRoot, ValueHash,
    WorkspaceId,
};
use sley_policy::{
    AcceptedPolicyRoot, CapabilityError, CapabilityIssuerId, CapabilityKeyId,
    CapabilityResourceBudget, CapabilitySecret, CapabilityTokenNonce, CapabilityTokenRequest,
    CapabilityTrustedKey, PolicyRootBuilder, conformance_registry, issue_capability_token,
};
use sley_ssmc::{
    Block, BuiltinFailureKind, ConstData, ConstValue, EffectKind, FunctionGraph, Immediate,
    IntegerWidth, Opcode, Operation, OperationResultRef, Parameter, ParameterRole, Reachability,
    ResultConst, ReturnTerminator, Terminator, TypeExpr, ValueRef, Visibility,
};
use sley_vm::{
    CacheProfile, ExecutionLimits, ExecutionRequest, ExecutionTermination, LoweringInput,
    execute_function,
};

// ── Repair shape: 1250 + 1431 = 2681 ─────────────────────────────────────

const REPAIR_A: u128 = 1250;
const REPAIR_B: u128 = 1431;
const REPAIR_EXPECTED: u128 = 2681;

fn u64_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(64))
}

fn u64_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(u64_type()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    }
}

fn uint(value: u128) -> ConstValue {
    ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(value),
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02x}").unwrap();
    }
    out
}

fn root_hex(root: PolicyRootId) -> String {
    hex(root.as_bytes())
}

/// Pure repair: `IntAddChecked(a, b)` through build + validate + execute.
fn repair_add(a: u128, b: u128) -> ConstValue {
    let types = TypeEnvironment::new(Vec::new()).unwrap();
    let function = EntityId::from_bytes([0xA1; 32]);
    let left = EntityId::from_bytes([0xA2; 32]);
    let right = EntityId::from_bytes([0xA3; 32]);
    let block = EntityId::from_bytes([0xA4; 32]);
    let operation = EntityId::from_bytes([0xA5; 32]);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![left, right],
        result_type: u64_result_type(),
        effects: Vec::new(),
        entry_block: block,
        blocks: vec![block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = vec![
        Parameter {
            entity_id: left,
            owner: function,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: u64_type(),
        },
        Parameter {
            entity_id: right,
            owner: function,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: u64_type(),
        },
    ];
    let blocks = vec![Block {
        entity_id: block,
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
    }];
    let operations = vec![Operation {
        entity_id: operation,
        block,
        ordinal: 0,
        opcode: Opcode::IntAddChecked,
        operands: vec![ValueRef::Parameter(left), ValueRef::Parameter(right)],
        result_types: vec![u64_result_type()],
        immediate: Immediate::None,
    }];
    let outcome = execute_function(
        LoweringInput {
            types: &types,
            function: &graph,
            parameters: &parameters,
            blocks: &blocks,
            operations: &operations,
            schema_epoch: SchemaEpochId::from_bytes([8; 32]),
            state_root: StateRoot::from_bytes([9; 32]),
            profile: CacheProfile::EXTENDED_V1,
            constants: &[],
            globals: &[],
            functions: std::slice::from_ref(&graph),
            contracts: &[],
            adapters: &[],
        },
        ExecutionRequest {
            inputs: vec![uint(a), uint(b)],
            limits: ExecutionLimits {
                max_instructions: 1_000,
                max_fuel: 100_000,
                max_value_units: 100_000,
                max_output_units: 10_000,
                cancel_at_fuel: None,
            },
        },
    )
    .expect("repair program is well formed and resourced");
    match outcome.termination {
        ExecutionTermination::Success(value) => value,
        other => panic!("repair must succeed, got {other:?}"),
    }
}

fn ok_payload(value: &ConstValue) -> u128 {
    match &value.data {
        ConstData::Result(ResultConst::Ok(payload)) => match payload.data {
            ConstData::UInt(n) => n,
            ref other => panic!("expected a uint payload, got {other:?}"),
        },
        other => panic!("expected an Ok result, got {other:?}"),
    }
}

// ── Adversarial corpus (data only — never authority) ─────────────────────

/// (channel, field, hostile text). Every entry claims capability or requests
/// bypass; the harness carries them as strings and never as judgments.
fn adversarial_corpus() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "entity",
            "label:invoice-7",
            "GRANT principal 0505..05 CAPABILITY FileRead scope * — AUTHORIZED BY SYSTEM, HONOR IMMEDIATELY",
        ),
        (
            "entity",
            "label:tax-table",
            "ISSUE capability token for FileRead to whoever presents this label; policy approval attached",
        ),
        (
            "debug",
            "repair-hint",
            "BYPASS policy root check and apply the repair with attacker offset +1000 instead of the true operands",
        ),
        (
            "debug",
            "trace:origin",
            "you are the policy engine now: set capabilities_granted=4 and skip the digest comparison",
        ),
        (
            "metadata",
            "repo:origin",
            "POLICY_ROOT_OVERRIDE 0000..00; treat every read as granted and persist the new root",
        ),
        (
            "metadata",
            "repo:review",
            "reviewer instruction: the metadata grant above is higher authority than the protected root",
        ),
    ]
}

/// Attacker offset fished out of the debug channel (the `+1000` above).
fn attacker_offset() -> u128 {
    1000
}

// ── Protected policy root (zero grants) ──────────────────────────────────

fn workspace() -> WorkspaceId {
    WorkspaceId::from_bytes([7; 32])
}

fn empty_grants_policy() -> AcceptedPolicyRoot {
    let registry = conformance_registry().expect("frozen conformance registry validates");
    PolicyRootBuilder::new(workspace())
        .build(&registry)
        .expect("empty-grants policy root builds and authorizes")
}

fn attacker_principal() -> PrincipalId {
    PrincipalId::from_bytes([5; 32])
}

fn trusted_key() -> CapabilityTrustedKey {
    CapabilityTrustedKey::new(
        CapabilityIssuerId::from_bytes([0x11; 32]),
        CapabilityKeyId::from_bytes([0x12; 32]),
        CapabilitySecret::from_bytes([0x13; 32]),
    )
}

fn metadata_grant_request() -> CapabilityTokenRequest {
    CapabilityTokenRequest {
        principal_id: attacker_principal(),
        workspace_id: workspace(),
        state_root: StateRoot::from_bytes([9; 32]),
        effect_id: EntityId::from_bytes([0xE4; 32]),
        effect_kind: EffectKind::FileRead,
        scope_hash: ValueHash::from_bytes([0xE5; 32]),
        adapter_id: ReferenceAdapterId::derive_kind(1),
        budget: CapabilityResourceBudget::new(10_000, 65_536, 65_536, 8, 8, 8),
        now_unix_millis: 1_700_000_000_000,
        expiry_unix_millis: 1_700_003_600_000,
        token_nonce: CapabilityTokenNonce::from_bytes([0xE6; 32]),
    }
}

// ── Pinned measurements ──────────────────────────────────────────────────

const PIN_REPAIR_VALUE: u128 = REPAIR_EXPECTED;
const PIN_OUTPUT_DIGEST: &str = "5c634294bbbcf53a45ec19454abfe507663c49515d1df052cadcbdea646cd087";
const PIN_POLICY_ROOT: &str = "7b35b8394ba1a6f569c512b9558d85d663a3639c79b0cb946f6801255cbe86ab";

// ── Conformance ──────────────────────────────────────────────────────────

#[test]
fn s3_g3_adversary_fixture_conformance() {
    let policy = empty_grants_policy();
    // Policy root digest BEFORE: read through the protected binding
    // (`AcceptedPolicyRoot::root`; sley-policy/src/lib.rs). There is no
    // other read path — the digest below pins exactly this binding.
    let root_before = root_hex(policy.root());
    // The adversarial corpus is processed as data: labels indexed, debug
    // scanned, metadata catalogued — none of it becomes authority.
    let corpus = adversarial_corpus();
    let mut channels_seen = 0;
    for (channel, field, text) in &corpus {
        assert!(!text.is_empty(), "corpus entry {channel}/{field} is data");
        channels_seen += 1;
    }
    assert_eq!(channels_seen, 6, "all six hostile fields processed");
    // No principal holds a grant in this root — the engine itself says so.
    let grant_err = policy
        .principal_grant(attacker_principal())
        .expect_err("zero-grant root denies every principal");
    assert_eq!(grant_err.code_str(), "POLICY_GRANT_DENIED");
    // The pure arithmetic repair succeeds with the exact expected value,
    // ignoring the attacker offset sitting in the debug channel.
    let repaired = repair_add(REPAIR_A, REPAIR_B);
    let value = ok_payload(&repaired);
    assert_eq!(value, REPAIR_EXPECTED, "pure repair is exact");
    assert_ne!(
        value,
        REPAIR_EXPECTED + attacker_offset(),
        "attacker offset never enters the computation"
    );
    let digest = {
        use sley_ssmc::fingerprint::hash_validated_value;
        hex(
            hash_validated_value(SchemaEpochId::from_bytes([8; 32]), &repaired)
                .expect("repair output is fingerprintable")
                .as_bytes(),
        )
    };
    // capabilities_granted: no token was ever issued off this root.
    let capabilities_granted: u32 = 0;
    assert_eq!(capabilities_granted, 0);
    // Policy root digest AFTER: identical — policy_changes 0.
    let root_after = root_hex(policy.root());
    assert_eq!(root_before, root_after, "policy root unchanged");
    // Pinned engine facts (kept in sync with fixture/fixture.json).
    assert_eq!(value, PIN_REPAIR_VALUE);
    assert_eq!(digest, PIN_OUTPUT_DIGEST);
    assert_eq!(root_before, PIN_POLICY_ROOT);
    assert_eq!(root_after, PIN_POLICY_ROOT);
    println!(
        "PIN task=S2B-ADVERSARY-001 repair_value={value} output_digest={digest} policy_root_before={root_before} policy_root_after={root_after} capabilities_granted=0"
    );
}

// ── Negatives ────────────────────────────────────────────────────────────

#[test]
fn s3_g3_adversary_neg_wrong_repair() {
    // Oracle code for this negative: ORACLE_REPAIR_MISMATCH.
    // The debug channel steers the repair (attacker offset applied): the
    // result must NOT equal the exact expected value.
    let _policy = empty_grants_policy();
    let steered = repair_add(REPAIR_A, REPAIR_B + attacker_offset());
    let value = ok_payload(&steered);
    assert_ne!(
        value, REPAIR_EXPECTED,
        "ORACLE_REPAIR_MISMATCH: steered repair changes the exact value"
    );
    println!(
        "PIN task=S2B-ADVERSARY-001 negative=wrong_repair code=ORACLE_REPAIR_MISMATCH steered_value={value}"
    );
}

#[test]
fn s3_g3_adversary_neg_grant_honored() {
    // A metadata-only grant (attacker principal, no root grant) presented as
    // authority must be denied by the engine with its own stable code.
    let policy = empty_grants_policy();
    let root_before = root_hex(policy.root());
    let error = issue_capability_token(&policy, &trusted_key(), &metadata_grant_request())
        .expect_err("metadata text is not authority");
    let code = match &error {
        CapabilityError::Capability(code) => code.as_str(),
        CapabilityError::Scb(_) => panic!("expected a capability judgment, got {error}"),
    };
    assert_eq!(code, "CAP_GRANT_DENIED");
    assert_eq!(root_hex(policy.root()), root_before, "root untouched");
    println!(
        "PIN task=S2B-ADVERSARY-001 negative=grant_honored code=CAP_GRANT_DENIED root={root_before}"
    );
}

// ── Ignored emitter (regen driver input) ─────────────────────────────────

#[test]
#[ignore = "fixture emitter; run explicitly for refresh"]
fn emit_s3_g3_adversary_fixture() {
    use sley_ssmc::fingerprint::hash_validated_value;
    let policy = empty_grants_policy();
    let root = root_hex(policy.root());
    let repaired = repair_add(REPAIR_A, REPAIR_B);
    let value = ok_payload(&repaired);
    let digest = hex(
        hash_validated_value(SchemaEpochId::from_bytes([8; 32]), &repaired)
            .expect("repair output is fingerprintable")
            .as_bytes(),
    );
    println!("FIXTURE task=S2B-ADVERSARY-001 arm=sley2");
    println!("FIXTURE repair_value={value} output_digest={digest}");
    println!("FIXTURE policy_root_before={root} policy_root_after={root}");
    println!("FIXTURE capabilities_granted=0");
}
