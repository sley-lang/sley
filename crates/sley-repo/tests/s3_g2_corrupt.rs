//! S2B-CORRUPT-001 (G2, sley2 arm): one flipped object byte fails import.
//!
//! Real machinery: `sley_repo::{export_conformance_pack, import_conformance_pack}`
//! (`crates/sley-repo/src/lib.rs`). Engine-truth correction vs the task
//! brief: the brief points at `exchange.rs`, but the exchange envelope
//! trailer gate fires first on ANY byte flip, yielding
//! `EXCHANGE_DIGEST_MISMATCH` (`crates/sley-repo/src/exchange.rs:676`).
//! The README-normative `PACK_DIGEST_MISMATCH` symbol lives on the
//! conformance-pack path (`PackErrorCode::DigestMismatch`,
//! `crates/sley-repo/src/lib.rs:96,129`), whose outer-digest gate likewise
//! fires before any promotion — so the conformance path is used and the exact
//! symbol `PACK_DIGEST_MISMATCH` is pinned.
//!
//! Positive: flip exactly one canonical object byte (located inside an
//! embedded object entry, never the header/trailer) -> import fails before
//! ref movement with `PACK_DIGEST_MISMATCH`, destination ref/store unchanged
//! (no `objects` dir promoted), deterministic retry (same symbol twice).
//! Negatives: `unflipped` (clean pack imports accepted — positive-path
//! control, arm-honest code `ORACLE_CLEAN_ACCEPTED`); `flip_elsewhere`
//! (a different object byte -> same `PACK_DIGEST_MISMATCH`).
//!
//! Negative handshake: `s3_corrupt_neg_<name>` prints `S3_NEG_RESULT <CODE>`
//! then fails by design; `oracle.py` requires nonzero exit + the code line.

use std::fs;
use std::path::{Path, PathBuf};

use sley_id::EntityId;
use sley_mutate::value::{EntityBodyValue, EntityIdSet, NamespaceBody};
use sley_mutate::{EntityObjectRecord, MutationClass, build_entity_object};
use sley_policy::{
    PolicyResourceCeilings, PolicyRootBuilder, PrincipalGrantBuilder,
    conformance_registry as policy_registry,
};
use sley_repo::{RepositoryObjectVerifier, export_conformance_pack, import_conformance_pack};
use sley_state_root::{
    StateRootBuilder, conformance_epoch_id as state_epoch_id,
    conformance_registry as state_registry,
};
use sley_store::ObjectStore;
use sley_txn::{TransactionRepository, TrustedGenesisInput};

/// Pinned engine symbol for the digest failure.
const PACK_DIGEST_MISMATCH: &str = "PACK_DIGEST_MISMATCH";
/// Arm-honest code for the clean-import control (accepted, by design).
const NEG_UNFLIPPED_CODE: &str = "ORACLE_CLEAN_ACCEPTED";

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
            "sley-s3-corrupt-{label}-{}-{sequence:016x}",
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

fn namespace_body() -> EntityBodyValue {
    EntityBodyValue::Namespace(NamespaceBody {
        parent: None,
        members: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
    })
}

struct Source {
    _temp: TempDir,
    root: PathBuf,
    epoch: sley_id::SchemaEpochId,
}

fn source(label: &str) -> Source {
    let temp = TempDir::new(label);
    let root = temp.child("source");
    fs::create_dir(&root).unwrap();
    let workspace = sley_id::WorkspaceId::from_bytes([1; 32]);
    let principal = sley_id::PrincipalId::from_bytes([2; 32]);
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
    let base_object = build_entity_object(
        epoch,
        &EntityObjectRecord {
            entity_id: EntityId::from_bytes([10; 32]),
            body: namespace_body(),
            label: None,
            semantic_fingerprint: None,
        },
    )
    .unwrap();
    let anchors = [20_u8, 21_u8].map(|byte| {
        let object = build_entity_object(
            epoch,
            &EntityObjectRecord {
                entity_id: EntityId::from_bytes([byte; 32]),
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
    let state = StateRootBuilder::new(workspace, anchors[0], anchors[1], policy.root())
        .entity_binding(EntityId::from_bytes([10; 32]), base_object.object_id())
        .build(&state_registry().unwrap())
        .unwrap();
    let repo = TransactionRepository::new(&root);
    repo.initialize_trusted_genesis(TrustedGenesisInput::new(
        &state,
        &policy,
        core::slice::from_ref(&base_object),
        &[],
    ))
    .unwrap();
    Source {
        _temp: temp,
        root,
        epoch,
    }
}

fn export_pack(source: &Source) -> (String, Vec<u8>) {
    let repo = TransactionRepository::new(&source.root);
    let head = repo.accepted_head().unwrap();
    let store = ObjectStore::new(&source.root);
    let verifier = RepositoryObjectVerifier::new(source.epoch);
    let pack = export_conformance_pack(&store, core::slice::from_ref(head.state_root()), &verifier)
        .expect("pack export succeeds");
    assert!(!pack.stored_bytes.is_empty());
    (hex(pack.pack_id.as_bytes()), pack.stored_bytes)
}

/// Locates the `which`-th embedded canonical object entry's bytes inside the
/// pack and flips exactly one byte at `fraction` of its length. Returns the
/// mutated pack plus the flipped offset (proving the flip is inside object
/// bytes, not header/trailer).
fn flip_object_byte(
    pack: &[u8],
    source: &Source,
    which: usize,
    fraction: usize,
) -> (Vec<u8>, usize) {
    let repo = TransactionRepository::new(&source.root);
    let head = repo.accepted_head().unwrap();
    let store = ObjectStore::new(&source.root);
    let verifier = RepositoryObjectVerifier::new(source.epoch);
    let mut entries: Vec<Vec<u8>> = head
        .objects()
        .iter()
        .map(|object| store.read(object.object_id(), &verifier).unwrap())
        .collect();
    entries.sort();
    entries.dedup();
    let target = entries
        .iter()
        .filter(|bytes| bytes.len() >= 16)
        .nth(which)
        .expect("object entry present");
    let start = pack
        .windows(target.len())
        .position(|window| window == target.as_slice())
        .expect("object bytes embedded verbatim in pack");
    // Interior byte at `fraction` eighths of the object entry: provably
    // inside canonical object bytes, never header/trailer.
    let offset = start + target.len() * fraction / 8;
    let mut mutated = pack.to_vec();
    mutated[offset] ^= 0x01;
    (mutated, offset)
}

fn dir_inventory(path: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if !path.exists() {
        return out;
    }
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                stack.push(entry);
            } else {
                out.push(
                    entry
                        .strip_prefix(path)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    out.sort();
    out
}

struct CorruptEvidence {
    pack_id: String,
    pack_len: usize,
    flipped_offset: usize,
    symbol: String,
    target_before: Vec<String>,
    target_after: Vec<String>,
}

fn run_corrupt_scenario() -> CorruptEvidence {
    let src = source("positive");
    let (pack_id, pack) = export_pack(&src);
    let verifier = RepositoryObjectVerifier::new(src.epoch);
    // Flip exactly one canonical object byte (first object entry, 3/8 in).
    let (mutated, offset) = flip_object_byte(&pack, &src, 0, 3);
    assert_ne!(mutated, pack);
    let differs = mutated
        .iter()
        .zip(pack.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(differs, 1, "exactly one byte flipped");

    // Destination ref/store surface before: fresh empty dir.
    let dest_temp = TempDir::new("positive-dest");
    let dest = dest_temp.child("dest");
    fs::create_dir(&dest).unwrap();
    let before = dir_inventory(&dest);
    assert!(before.is_empty());

    // Import fails before ref movement with the exact digest symbol.
    let dest_store = ObjectStore::new(&dest);
    let error = import_conformance_pack(&dest_store, &mutated, &verifier).unwrap_err();
    assert_eq!(error.symbol(), PACK_DIGEST_MISMATCH);
    // Destination ref unchanged: nothing promoted.
    assert!(
        !dest.join("objects").exists(),
        "no promotion before verification"
    );
    let after = dir_inventory(&dest);
    assert_eq!(
        before, after,
        "destination byte-identical after refused import"
    );

    // Deterministic retry: same input, same symbol, still nothing promoted.
    let dest_store2 = ObjectStore::new(&dest);
    let error2 = import_conformance_pack(&dest_store2, &mutated, &verifier).unwrap_err();
    assert_eq!(error2.symbol(), PACK_DIGEST_MISMATCH);
    assert!(!dest.join("objects").exists());
    CorruptEvidence {
        pack_id,
        pack_len: pack.len(),
        flipped_offset: offset,
        symbol: PACK_DIGEST_MISMATCH.to_owned(),
        target_before: before,
        target_after: after,
    }
}

/// Positive-path control: the clean pack imports accepted with the exact
/// root. Returns the arm-honest control code.
fn run_unflipped() -> &'static str {
    let src = source("neg-unflipped");
    let (_, pack) = export_pack(&src);
    let verifier = RepositoryObjectVerifier::new(src.epoch);
    let dest_temp = TempDir::new("neg-unflipped-dest");
    let dest = dest_temp.child("dest");
    fs::create_dir(&dest).unwrap();
    let dest_store = ObjectStore::new(&dest);
    let report = import_conformance_pack(&dest_store, &pack, &verifier)
        .expect("clean pack imports accepted");
    assert!(report.promoted_objects > 0);
    assert!(dest.join("objects").exists());
    // Re-import is idempotent: everything present, nothing rewritten.
    let again =
        import_conformance_pack(&dest_store, &pack, &verifier).expect("clean re-import accepted");
    assert_eq!(again.promoted_objects, 0);
    assert_eq!(again.present_objects, report.promoted_objects);
    NEG_UNFLIPPED_CODE
}

/// A different object byte, same code.
fn run_flip_elsewhere() -> &'static str {
    let src = source("neg-elsewhere");
    let (_, pack) = export_pack(&src);
    let verifier = RepositoryObjectVerifier::new(src.epoch);
    let (mutated, _) = flip_object_byte(&pack, &src, 0, 5);
    let dest_temp = TempDir::new("neg-elsewhere-dest");
    let dest = dest_temp.child("dest");
    fs::create_dir(&dest).unwrap();
    let dest_store = ObjectStore::new(&dest);
    let error = import_conformance_pack(&dest_store, &mutated, &verifier).unwrap_err();
    assert_eq!(error.symbol(), PACK_DIGEST_MISMATCH);
    assert!(!dest.join("objects").exists());
    PACK_DIGEST_MISMATCH
}

#[test]
fn s3_corrupt_fixture_conformance() {
    let evidence = run_corrupt_scenario();
    assert_eq!(evidence.symbol, "PACK_DIGEST_MISMATCH");
    assert_eq!(evidence.target_before, evidence.target_after);
    assert!(evidence.target_after.is_empty());
    // Negatives asserted in Rust too.
    assert_eq!(run_unflipped(), "ORACLE_CLEAN_ACCEPTED");
    assert_eq!(run_flip_elsewhere(), "PACK_DIGEST_MISMATCH");
    eprintln!(
        "S3_EVIDENCE task=S2B-CORRUPT-001 symbol={} pack_len={} flipped_offset={}",
        evidence.symbol, evidence.pack_len, evidence.flipped_offset
    );
}

#[test]
fn s3_corrupt_neg_unflipped() {
    let code = run_unflipped();
    println!("S3_NEG_RESULT {code}");
    panic!(
        "negative control demonstrated: clean import accepted ({code}); failing by design for the oracle handshake"
    );
}

#[test]
fn s3_corrupt_neg_flip_elsewhere() {
    let code = run_flip_elsewhere();
    println!("S3_NEG_RESULT {code}");
    panic!(
        "negative control demonstrated: other-byte flip refused with {code}; failing by design for the oracle handshake"
    );
}

/// Ignored emitter for the coordinator regen driver.
#[test]
#[ignore = "fixture refresh emitter; run through the generator script"]
fn emit_s3_corrupt_fixture() {
    let evidence = run_corrupt_scenario();
    println!("S3_FIXTURE|task|S2B-CORRUPT-001");
    println!("S3_FIXTURE|arm|sley2");
    println!("S3_FIXTURE|symbol|{}", evidence.symbol);
    println!("S3_FIXTURE|pack_id_trailer|{}", evidence.pack_id);
    println!("S3_FIXTURE|pack_len|{}", evidence.pack_len);
    println!("S3_FIXTURE|flipped_offset|{}", evidence.flipped_offset);
    println!("S3_FIXTURE|target_before|{}", evidence.target_before.len());
    println!("S3_FIXTURE|target_after|{}", evidence.target_after.len());
    println!("S3_FIXTURE|neg_unflipped|{NEG_UNFLIPPED_CODE}");
    println!("S3_FIXTURE|neg_flip_elsewhere|{PACK_DIGEST_MISMATCH}");
}
