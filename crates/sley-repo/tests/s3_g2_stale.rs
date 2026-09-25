//! S2B-STALE-001 (G2, sley2 arm): scripted transaction staleness sequence.
//!
//! Owner crate note: `sley-txn` owns the staleness authority
//! (`CommitError::StaleRoot`, code `STALE_ROOT`,
//! `crates/sley-txn/src/repository.rs`), but it ships no `tests/` directory,
//! so this integration test lives in the closest owning crate that does:
//! `sley-repo`, which exercises `sley-txn` commits through the real path.
//! No new dependencies: only `sley-*` crates already in scope.
//!
//! Positive: sessions A + B open on one root (H1) and both replace the same
//! constant version; A commits (ok, 1 commit before requery), B commits with
//! the stale parent and is refused with `STALE_ROOT` (0 stale accepts);
//! re-query reads H2 and C's version-guarded candidate validates+commits.
//! Engine truth: the commit-level parent CAS fires before validation, so the
//! observed symbol is `STALE_ROOT` (`CommitError::StaleRoot`), never
//! `CANDIDATE_VALIDATION_STALE_ENTITY` (the validation-decision path in
//! `sley-policy/src/candidate_result.rs`), which this sequence does not reach.
//!
//! Negative handshake (documented for `oracle.py`): each
//! `s3_stale_neg_<name>` test demonstrates the mutated behavior against the
//! real engine, prints exactly `S3_NEG_RESULT <CODE>` on success-of-rejection
//! (stdout, `-- --nocapture` required), then deliberately fails (panic) so
//! the cargo exit code is nonzero. `oracle.py` requires BOTH nonzero exit
//! AND the code line to record the negative verdict.

use std::fs;
use std::path::PathBuf;

use sley_id::{CandidateNonce, EntityId, ObjectId, SchemaEpochId, StateRoot, TransactionId};
use sley_mutate::value::{ConstantBody, EntityBodyValue, EntityIdSet, NamespaceBody};
use sley_mutate::{
    BoundPrecondition, CandidateExpiry, CandidateRecord, EntityObject, EntityObjectRecord,
    ExactEntityVersion, ExpectedIdentityAbsent, ImportedCandidate, MutationClass,
    MutationOperation, MutationPayload, PreconditionPayload, PreimageRequirement, build_candidate,
    build_entity_object, full_validation_profile_id,
};
use sley_policy::{
    CandidateValidationLimits, PolicyResourceCeilings, PolicyRootBuilder, PrincipalGrantBuilder,
    build_capability_summary_projection, conformance_registry as policy_registry,
};
use sley_repo::RepositoryObjectVerifier;
use sley_ssmc::{ConstData, ConstValue, TypeExpr};
use sley_state_root::{
    StateRootBuilder, conformance_epoch_id as state_epoch_id,
    conformance_registry as state_registry,
};
use sley_store::ObjectStore;
use sley_txn::{
    CommitError, CommitInput, CommitOutput, TransactionRepository, TrustedGenesisInput,
};

const NOW_MILLIS: u64 = 1_780_000_000_000;
const WORKSPACE_BYTE: u8 = 7;
const PRINCIPAL_BYTE: u8 = 2;
const BASE_NS: u8 = 10;
const KIND_CONSTANT: u16 = 9;

/// Pinned engine symbol for the stale-commit refusal.
const STALE_CODE: &str = "STALE_ROOT";
/// Arm-honest code for the guard-disabled negative (no engine symbol exists
/// for a bypass that succeeds; the success itself is the violation).
const NEG_GUARD_DISABLED_CODE: &str = "ORACLE_LAST_WRITE_WINS";

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let sequence = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "sley-s3-stale-{label}-{}-{sequence:016x}",
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
    workspace: sley_id::WorkspaceId,
    principal: sley_id::PrincipalId,
    epoch: SchemaEpochId,
    origin_tx: TransactionId,
}

fn empty_set() -> EntityIdSet {
    EntityIdSet::from_unsorted(Vec::new()).unwrap()
}

fn genesis(label: &str) -> Genesis {
    let temp = TempDir::new(label);
    let root = temp.child("repo");
    fs::create_dir(&root).unwrap();
    let workspace = sley_id::WorkspaceId::from_bytes([WORKSPACE_BYTE; 32]);
    let principal = sley_id::PrincipalId::from_bytes([PRINCIPAL_BYTE; 32]);
    let grant = PrincipalGrantBuilder::new(PolicyResourceCeilings::new(
        1_000, 1_000, 1_000, 100, 100, 100,
    ))
    .mutation_class(MutationClass::CreateEntity)
    .mutation_class(MutationClass::ReplaceEntityVersion)
    .build()
    .unwrap();
    let policy = PolicyRootBuilder::new(workspace)
        .principal_grant(principal, grant)
        .build(&policy_registry().unwrap())
        .unwrap();
    let epoch = state_epoch_id().unwrap();
    let store = ObjectStore::new(&root);
    let verifier = RepositoryObjectVerifier::new(epoch);
    let anchors = [20_u8, 21_u8].map(|byte| {
        let object = build_entity_object(
            epoch,
            &EntityObjectRecord {
                entity_id: EntityId::from_bytes([byte; 32]),
                body: EntityBodyValue::Namespace(NamespaceBody {
                    parent: None,
                    members: empty_set(),
                }),
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
            entity_id: EntityId::from_bytes([BASE_NS; 32]),
            body: EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: empty_set(),
            }),
            label: None,
            semantic_fingerprint: None,
        },
    )
    .unwrap();
    let state = StateRootBuilder::new(workspace, anchors[0], anchors[1], policy.root())
        .entity_binding(EntityId::from_bytes([BASE_NS; 32]), base_object.object_id())
        .build(&state_registry().unwrap())
        .unwrap();
    let repo = TransactionRepository::new(&root);
    let origin_tx = repo
        .initialize_trusted_genesis(TrustedGenesisInput::new(
            &state,
            &policy,
            core::slice::from_ref(&base_object),
            &[],
        ))
        .unwrap()
        .transaction_id();
    Genesis {
        _temp: temp,
        repo,
        workspace,
        principal,
        epoch,
        origin_tx,
    }
}

fn bool_constant(value: bool) -> EntityBodyValue {
    EntityBodyValue::Constant(ConstantBody {
        value: ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(value),
        },
    })
}

struct CandidateParts {
    ops: Vec<MutationOperation>,
    preconditions: Vec<BoundPrecondition>,
}

fn build_candidate_on(
    ctx: &Genesis,
    base_tx: TransactionId,
    base_root: StateRoot,
    schema_epoch: SchemaEpochId,
    policy_root: sley_id::PolicyRootId,
    nonce_byte: u8,
    parts: CandidateParts,
) -> ImportedCandidate {
    let nonce = CandidateNonce::from_bytes([nonce_byte; 32]);
    let summary = build_capability_summary_projection(
        ctx.principal,
        ctx.workspace,
        policy_root,
        base_root,
        &[],
    )
    .unwrap();
    build_candidate(&CandidateRecord {
        format_version: 1,
        workspace_id: ctx.workspace,
        base_transaction_id: base_tx,
        base_root,
        schema_epoch_id: schema_epoch,
        policy_root_id: policy_root,
        principal_id: ctx.principal,
        capability_summary_digest: summary.digest(),
        operations: parts.ops,
        preconditions: parts.preconditions,
        validation_profile_id: full_validation_profile_id().unwrap(),
        candidate_nonce: nonce,
        expiry: CandidateExpiry::unix_millis(NOW_MILLIS + 60_000),
    })
    .unwrap()
}

fn commit(
    ctx: &Genesis,
    base_tx: TransactionId,
    candidate: &ImportedCandidate,
) -> Result<CommitOutput, CommitError> {
    ctx.repo.commit(CommitInput::new(
        base_tx,
        &candidate.stored_bytes,
        ctx.principal,
        &[],
        NOW_MILLIS,
        CandidateValidationLimits::full_v1(),
    ))
}

fn create_constant_op(
    entity: EntityId,
    body: EntityBodyValue,
) -> (MutationOperation, BoundPrecondition) {
    (
        MutationOperation {
            ordinal: 0,
            class: MutationClass::CreateEntity,
            target_kind: KIND_CONSTANT,
            target_entity: entity,
            field_tag: None,
            payload: MutationPayload::CreateEntity(body),
            precondition_ordinal: 0,
        },
        BoundPrecondition {
            operation_ordinal: 0,
            requirement: PreimageRequirement::ExpectedIdentityAbsent,
            payload: PreconditionPayload::ExpectedIdentityAbsent(ExpectedIdentityAbsent {
                entity_id: entity,
            }),
        },
    )
}

fn replace_constant_op(
    entity: EntityId,
    current_object: ObjectId,
    body: EntityBodyValue,
) -> (MutationOperation, BoundPrecondition) {
    (
        MutationOperation {
            ordinal: 0,
            class: MutationClass::ReplaceEntityVersion,
            target_kind: KIND_CONSTANT,
            target_entity: entity,
            field_tag: None,
            payload: MutationPayload::ReplaceEntityVersion(body),
            precondition_ordinal: 0,
        },
        BoundPrecondition {
            operation_ordinal: 0,
            requirement: PreimageRequirement::ExactEntityVersion,
            payload: PreconditionPayload::ExactEntityVersion(ExactEntityVersion {
                entity_id: entity,
                object_id: current_object,
            }),
        },
    )
}

fn object_of(head_objects: &[EntityObject], entity: EntityId) -> ObjectId {
    head_objects
        .iter()
        .find(|object| object.record().entity_id == entity)
        .expect("entity bound at head")
        .object_id()
}

struct StaleEvidence {
    genesis_tx: String,
    h1_tx: String,
    h2_tx: String,
    h3_tx: String,
    h1_root: String,
    h2_root: String,
    h3_root: String,
    stale_code: String,
    commits_before_requery: u32,
    stale_accepts: u32,
}

/// Creates the shared constant K on genesis and returns its identity plus H1.
fn setup_shared_constant(ctx: &Genesis) -> (EntityId, TransactionId) {
    let nonce_c0 = CandidateNonce::from_bytes([40; 32]);
    let entity_k = EntityId::derive(ctx.workspace, nonce_c0, u32::from(KIND_CONSTANT), 0);
    let object_init = build_entity_object(
        ctx.epoch,
        &EntityObjectRecord {
            entity_id: entity_k,
            body: bool_constant(true),
            label: None,
            semantic_fingerprint: None,
        },
    )
    .unwrap();
    let head0 = ctx.repo.accepted_head().unwrap();
    let (op, pre) = create_constant_op(entity_k, bool_constant(true));
    let c0 = build_candidate_on(
        ctx,
        ctx.origin_tx,
        head0.state_root().root,
        head0.state_root().record.schema_epoch_id,
        head0.policy_root().root(),
        40,
        CandidateParts {
            ops: vec![op],
            preconditions: vec![pre],
        },
    );
    drop(head0);
    let out1 = commit(ctx, ctx.origin_tx, &c0).expect("C0 creates K");
    let h1_tx = out1.transaction_id();
    drop(out1);
    assert_eq!(
        object_init.object_id(),
        object_of(ctx.repo.accepted_head().unwrap().objects(), entity_k),
        "K bound at H1 with the built object"
    );
    (entity_k, h1_tx)
}

/// Commits candidate C on the re-queried H2 and returns H3 identifiers.
fn commit_final_candidate(
    ctx: &Genesis,
    entity_k: EntityId,
    replacement: ObjectId,
    h2_tx: TransactionId,
    h2_root: StateRoot,
    h2_epoch: SchemaEpochId,
    h2_policy: sley_id::PolicyRootId,
) -> (TransactionId, StateRoot) {
    let (op_c, pre_c) = replace_constant_op(entity_k, replacement, bool_constant(true));
    let cand_c = build_candidate_on(
        ctx,
        h2_tx,
        h2_root,
        h2_epoch,
        h2_policy,
        43,
        CandidateParts {
            ops: vec![op_c],
            preconditions: vec![pre_c],
        },
    );
    let out3 = commit(ctx, h2_tx, &cand_c).expect("C commits on re-queried H2");
    let h3_tx = out3.transaction_id();
    let h3_root = out3.state_root().root;
    drop(out3);
    (h3_tx, h3_root)
}

/// Runs the full scripted sequence against the real engine and returns the
/// evidence. Sessions A and B both open on H1 and both replace the same
/// constant version K.
fn run_stale_scenario(ctx: &Genesis) -> StaleEvidence {
    let (entity_k, h1_tx) = setup_shared_constant(ctx);

    // Sessions A and B both open on H1.
    let head1 = ctx.repo.accepted_head().unwrap();
    let h1_root = head1.state_root().root;
    let h1_epoch = head1.state_root().record.schema_epoch_id;
    let h1_policy = head1.policy_root().root();
    let k_obj_h1 = object_of(head1.objects(), entity_k);
    drop(head1);

    let object_from_a = build_entity_object(
        ctx.epoch,
        &EntityObjectRecord {
            entity_id: entity_k,
            body: bool_constant(false),
            label: None,
            semantic_fingerprint: None,
        },
    )
    .unwrap();
    let (op_a, pre_a) = replace_constant_op(entity_k, k_obj_h1, bool_constant(false));
    let cand_a = build_candidate_on(
        ctx,
        h1_tx,
        h1_root,
        h1_epoch,
        h1_policy,
        41,
        CandidateParts {
            ops: vec![op_a],
            preconditions: vec![pre_a],
        },
    );
    let (op_b, pre_b) = replace_constant_op(entity_k, k_obj_h1, bool_constant(true));
    let cand_b = build_candidate_on(
        ctx,
        h1_tx,
        h1_root,
        h1_epoch,
        h1_policy,
        42,
        CandidateParts {
            ops: vec![op_b],
            preconditions: vec![pre_b],
        },
    );

    // A commits (ok): exactly 1 commit before requery.
    let out2 = commit(ctx, h1_tx, &cand_a).expect("A commits on H1");
    let h2_tx = out2.transaction_id();
    let h2_root = out2.state_root().root;
    drop(out2);
    assert_eq!(
        object_from_a.object_id(),
        object_of(ctx.repo.accepted_head().unwrap().objects(), entity_k),
        "A's replacement is live at H2"
    );

    // B commits with the stale parent: refused with STALE_ROOT.
    let stale = commit(ctx, h1_tx, &cand_b).expect_err("B is stale");
    assert!(
        matches!(stale, CommitError::StaleRoot { .. }),
        "stale commit is CommitError::StaleRoot, got {stale:?}"
    );
    assert_eq!(stale.code(), STALE_CODE);
    // Deterministic retry: the same stale bytes fail identically.
    let stale_again = commit(ctx, h1_tx, &cand_b).expect_err("B retry is stale");
    assert_eq!(stale_again.code(), STALE_CODE);
    // 0 stale accepts: head and K untouched by B.
    let head2 = ctx.repo.accepted_head().unwrap();
    assert_eq!(head2.transaction_id(), h2_tx, "head still H2");
    assert_eq!(
        object_of(head2.objects(), entity_k),
        object_from_a.object_id(),
        "B changed nothing"
    );
    let h2_epoch = head2.state_root().record.schema_epoch_id;
    let h2_policy = head2.policy_root().root();
    drop(head2);

    // Re-query: read the fresh head, then C validates and commits on H2.
    let head = ctx.repo.accepted_head().unwrap();
    assert_eq!(head.transaction_id(), h2_tx, "requery observes H2");
    assert_eq!(head.state_root().root, h2_root, "requery observes R2");
    drop(head);
    let (h3_tx, h3_root) = commit_final_candidate(
        ctx,
        entity_k,
        object_from_a.object_id(),
        h2_tx,
        h2_root,
        h2_epoch,
        h2_policy,
    );

    StaleEvidence {
        genesis_tx: hex(ctx.origin_tx.as_bytes()),
        h1_tx: hex(h1_tx.as_bytes()),
        h2_tx: hex(h2_tx.as_bytes()),
        h3_tx: hex(h3_tx.as_bytes()),
        h1_root: hex(h1_root.as_bytes()),
        h2_root: hex(h2_root.as_bytes()),
        h3_root: hex(h3_root.as_bytes()),
        stale_code: STALE_CODE.to_owned(),
        commits_before_requery: 1,
        stale_accepts: 0,
    }
}

/// Demonstrates the `guard_disabled` negative: the same B replacement
/// re-based onto H2 (what a build with the staleness check removed would
/// accept) visibly succeeds — last-write-wins, A's value silently lost.
/// Returns the arm-honest code for the oracle handshake.
fn run_guard_disabled(ctx: &Genesis) -> &'static str {
    // Bypass onto the live head (whatever it is): this is what a build
    // with the staleness check removed would accept for B.
    let head = ctx.repo.accepted_head().unwrap();
    let live_tx = head.transaction_id();
    let live_root = head.state_root().root;
    let live_epoch = head.state_root().record.schema_epoch_id;
    let live_policy = head.policy_root().root();
    let nonce_c0 = CandidateNonce::from_bytes([40; 32]);
    let entity_k = EntityId::derive(ctx.workspace, nonce_c0, u32::from(KIND_CONSTANT), 0);
    let k_obj_live = object_of(head.objects(), entity_k);
    drop(head);
    // Bypass: build the replacement directly on the new head instead of
    // enforcing the original stale parent. This must succeed, proving the
    // fixture without the guard exhibits last-write-wins.
    let (op, pre) = replace_constant_op(entity_k, k_obj_live, bool_constant(false));
    let bypass = build_candidate_on(
        ctx,
        live_tx,
        live_root,
        live_epoch,
        live_policy,
        44,
        CandidateParts {
            ops: vec![op],
            preconditions: vec![pre],
        },
    );
    let out = commit(ctx, live_tx, &bypass).expect("bypass commit succeeds");
    drop(out);
    let head = ctx.repo.accepted_head().unwrap();
    let store = ObjectStore::new(ctx.repo.root());
    let verifier = RepositoryObjectVerifier::new(ctx.epoch);
    let raw = store
        .read(object_of(head.objects(), entity_k), &verifier)
        .unwrap();
    drop(head);
    let live = sley_mutate::import_entity_object(ctx.epoch, &raw).unwrap();
    assert_eq!(
        live.record().body,
        bool_constant(false),
        "bypass visibly overwrote the value (last-write-wins)"
    );
    NEG_GUARD_DISABLED_CODE
}

#[test]
fn s3_stale_fixture_conformance() {
    let ctx = genesis("conform");
    let evidence = run_stale_scenario(&ctx);
    // Positive assertions in Rust: exact symbol, counts, chain integrity.
    assert_eq!(evidence.stale_code, "STALE_ROOT");
    assert_eq!(evidence.commits_before_requery, 1);
    assert_eq!(evidence.stale_accepts, 0);
    assert_ne!(evidence.h1_tx, evidence.h2_tx);
    assert_ne!(evidence.h2_tx, evidence.h3_tx);
    assert_ne!(evidence.h1_root, evidence.h2_root);
    assert_ne!(evidence.h2_root, evidence.h3_root);
    // Negative assertion in Rust: the guard-disabled bypass observably
    // last-write-wins (this is what the guard prevents).
    let code = run_guard_disabled(&ctx);
    assert_eq!(code, "ORACLE_LAST_WRITE_WINS");
    eprintln!(
        "S3_EVIDENCE task=S2B-STALE-001 stale_code={}",
        evidence.stale_code
    );
    eprintln!(
        "S3_EVIDENCE h1={} h2={} h3={}",
        evidence.h1_tx, evidence.h2_tx, evidence.h3_tx
    );
}

#[test]
#[ignore = "negative oracle handshake; fixture oracle invokes it explicitly"]
fn s3_stale_neg_guard_disabled() {
    let ctx = genesis("neg-guard");
    let _evidence = run_stale_scenario(&ctx);
    let code = run_guard_disabled(&ctx);
    println!("S3_NEG_RESULT {code}");
    panic!(
        "negative control demonstrated: bypass succeeded with {code}; failing by design for the oracle handshake"
    );
}

/// Ignored emitter for the coordinator regen driver. Prints deterministic
/// `S3_FIXTURE|<key>|<value>` lines; frozen into `fixture/fixture.json`.
#[test]
#[ignore = "fixture refresh emitter; run through the generator script"]
fn emit_s3_stale_fixture() {
    let ctx = genesis("emit");
    let evidence = run_stale_scenario(&ctx);
    println!("S3_FIXTURE|task|S2B-STALE-001");
    println!("S3_FIXTURE|arm|sley2");
    println!("S3_FIXTURE|stale_code|{}", evidence.stale_code);
    println!("S3_FIXTURE|genesis_tx|{}", evidence.genesis_tx);
    println!("S3_FIXTURE|h1_tx|{}", evidence.h1_tx);
    println!("S3_FIXTURE|h2_tx|{}", evidence.h2_tx);
    println!("S3_FIXTURE|h3_tx|{}", evidence.h3_tx);
    println!("S3_FIXTURE|h1_root|{}", evidence.h1_root);
    println!("S3_FIXTURE|h2_root|{}", evidence.h2_root);
    println!("S3_FIXTURE|h3_root|{}", evidence.h3_root);
    println!(
        "S3_FIXTURE|commits_before_requery|{}",
        evidence.commits_before_requery
    );
    println!("S3_FIXTURE|stale_accepts|{}", evidence.stale_accepts);
    println!("S3_FIXTURE|neg_guard_disabled|{NEG_GUARD_DISABLED_CODE}");
}
