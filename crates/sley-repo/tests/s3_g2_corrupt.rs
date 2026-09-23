//! S2B-CORRUPT-001 (G2, sley2 arm): one flipped object byte fails import.
//!
//! Real machinery: `sley_repo::{export_conformance_pack, import_conformance_pack}`
//! (`crates/sley-repo/src/lib.rs`). Engine-truth note vs the task brief: the
//! brief points at `exchange.rs`. An UNRESEALED byte flip anywhere in an
//! exchange trips the exchange envelope trailer gate first, yielding
//! `EXCHANGE_DIGEST_MISMATCH` (`crates/sley-repo/src/exchange.rs:674-677`).
//! That holds only for an unresealed flip: the exchange trailer is an
//! unkeyed digest (`RepositoryExchangeId::derive`), so a flip inside the
//! embedded pack with the exchange trailer resealed passes that gate and
//! reaches the PACK owner's own preflight (`preflight_conformance_pack`,
//! `exchange.rs:1384`), whose code is returned verbatim (`exchange.rs:273-281`):
//! exact `PACK_DIGEST_MISMATCH` before any write. That exchange-route vector
//! is pinned by `s3_corrupt_exchange_resealed_embedded_pack` below, against
//! the staged live-trial exchange. The frozen fixture conformance test keeps
//! the conformance-pack path (`PackErrorCode::DigestMismatch`,
//! `crates/sley-repo/src/lib.rs:96,129`), whose outer-digest gate likewise
//! fires before any promotion, and pins the same exact symbol there.
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
use sley_repo::{
    ExchangeError, RepositoryObjectVerifier, decode_conformance_pack_entries_for_testing,
    export_conformance_pack, import_conformance_pack, import_repository_exchange,
    preflight_repository_exchange,
};
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
#[ignore = "negative oracle handshake; fixture oracle invokes it explicitly"]
fn s3_corrupt_neg_unflipped() {
    let code = run_unflipped();
    println!("S3_NEG_RESULT {code}");
    panic!(
        "negative control demonstrated: clean import accepted ({code}); failing by design for the oracle handshake"
    );
}

#[test]
#[ignore = "negative oracle handshake; fixture oracle invokes it explicitly"]
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

// ---------------------------------------------------------------------------
// Exchange-route vector: one canonical object byte flipped inside the
// embedded pack of the staged live-trial exchange, exchange trailer resealed
// with the owner's own digest (`RepositoryExchangeId::derive`, the exact
// function `decode_envelope` checks at `exchange.rs:674`). Driven through
// `import_repository_exchange`, the function the serve protocol's single
// import method `exchange.import` calls. No production API is added: the
// embedded pack's object entries are located with the existing test-only
// structural decode `decode_conformance_pack_entries_for_testing`.
// ---------------------------------------------------------------------------

/// The staged live-trial exchange every `sley_2_0` CORRUPT trial seeds from.
const STAGED_EXCHANGE: &str = "../../bench/fixtures/sley2/S2B-CORRUPT-001/base.pack";
/// Frozen S20-540 exchange contract tag and S20-170 pack contract tag.
const EXCHANGE_TAG: u64 = 540;
const PACK_TAG: u64 = 170;
const SCB_MAGIC: &[u8] = b"SLEYSCB1";
const TRAILER_LEN: usize = 32;

fn read_uvar(bytes: &[u8], position: &mut usize) -> u64 {
    let mut value = 0_u64;
    let mut shift = 0_u32;
    loop {
        let byte = bytes[*position];
        *position += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return value;
        }
        shift += 7;
        assert!(shift < 64, "uvar overflow");
    }
}

/// `(payload_start, payload_len)` of an SCB1 envelope with the given tag;
/// asserts the envelope spans the input exactly (payload + 32-byte trailer).
fn envelope(bytes: &[u8], tag: u64) -> (usize, usize) {
    assert_eq!(&bytes[..SCB_MAGIC.len()], SCB_MAGIC, "SCB1 magic");
    let mut position = SCB_MAGIC.len();
    assert_eq!(read_uvar(bytes, &mut position), 1, "format version");
    assert_eq!(read_uvar(bytes, &mut position), tag, "contract tag");
    position += 32; // schema epoch id
    let len = usize::try_from(read_uvar(bytes, &mut position)).unwrap();
    assert_eq!(position + len + TRAILER_LEN, bytes.len(), "envelope span");
    (position, len)
}

/// Absolute `(start, len)` of record field `tag` within `bytes[start..start+len]`.
fn record_field(bytes: &[u8], start: usize, len: usize, tag: u64) -> (usize, usize) {
    let end = start + len;
    let mut position = start;
    let count = read_uvar(bytes, &mut position);
    let mut found = None;
    for _ in 0..count {
        let field = read_uvar(bytes, &mut position);
        let size = usize::try_from(read_uvar(bytes, &mut position)).unwrap();
        if field == tag {
            found = Some((position, size));
        }
        position += size;
    }
    assert_eq!(position, end, "record span");
    found.expect("record field present")
}

struct StagedExchange {
    bytes: Vec<u8>,
    /// Absolute range of the embedded tag-170 pack.
    pack_start: usize,
    pack_end: usize,
    /// Each embedded canonical object's stored bytes, absolute start + len.
    objects: Vec<(usize, usize)>,
}

fn staged_exchange() -> StagedExchange {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(STAGED_EXCHANGE);
    let bytes = fs::read(&path).expect("staged CORRUPT exchange readable");
    let (payload_start, payload_len) = envelope(&bytes, EXCHANGE_TAG);
    // Exchange payload field 2 is the embedded object pack (`exchange.rs:788`).
    let (pack_start, pack_len) = record_field(&bytes, payload_start, payload_len, 2);
    let pack_end = pack_start + pack_len;
    let pack = &bytes[pack_start..pack_end];
    envelope(pack, PACK_TAG);
    // The embedded pack's own trailer is the owner's pack digest.
    let pack_id = sley_id::RepositoryPackId::derive(&pack[..pack.len() - TRAILER_LEN]);
    assert_eq!(&pack[pack.len() - TRAILER_LEN..], pack_id.as_bytes());
    // Object entries via the owner's existing test-only structural decode.
    let (_epochs, _roots, entries) =
        decode_conformance_pack_entries_for_testing(pack).expect("embedded pack decodes");
    assert!(entries.len() >= 2, "at least two canonical objects");
    let objects = entries
        .iter()
        .map(|entry| {
            let stored = entry.stored_bytes.as_slice();
            let hits: Vec<usize> = pack
                .windows(stored.len())
                .enumerate()
                .filter(|(_, window)| *window == stored)
                .map(|(index, _)| index)
                .collect();
            assert_eq!(hits.len(), 1, "object bytes embedded exactly once");
            (pack_start + hits[0], stored.len())
        })
        .collect();
    StagedExchange {
        bytes,
        pack_start,
        pack_end,
        objects,
    }
}

/// Recompute the exchange trailer exactly as the owner does.
fn reseal(bytes: &mut [u8]) {
    let body = bytes.len() - TRAILER_LEN;
    let id = sley_id::RepositoryExchangeId::derive(&bytes[..body]);
    bytes[body..].copy_from_slice(id.as_bytes());
}

/// Flip one interior byte of canonical object `which` (at `eighths`/8 of its
/// stored bytes). Returns the resealed exchange and the absolute offset.
fn resealed_object_flip(staged: &StagedExchange, which: usize, eighths: usize) -> (Vec<u8>, usize) {
    let (start, len) = staged.objects[which];
    let offset = start + len * eighths / 8;
    assert!(
        offset > start && offset < start + len - 1,
        "interior object byte"
    );
    assert!(offset > staged.pack_start && offset < staged.pack_end);
    let mut mutated = staged.bytes.clone();
    mutated[offset] ^= 0x01;
    reseal(&mut mutated);
    (mutated, offset)
}

/// Every file under `path` with its exact bytes (sorted).
fn tree_snapshot(path: &Path) -> Vec<(String, Vec<u8>)> {
    dir_inventory(path)
        .into_iter()
        .map(|name| {
            let bytes = fs::read(path.join(&name)).unwrap();
            (name, bytes)
        })
        .collect()
}

fn head_facts(target: &Path) -> (sley_id::TransactionId, usize) {
    let head = TransactionRepository::new(target).accepted_head().unwrap();
    (head.transaction_id(), head.objects().len())
}

fn import_code(target: &Path, input: &[u8], verifier: &RepositoryObjectVerifier) -> String {
    let error = import_repository_exchange(target, input, verifier).unwrap_err();
    assert!(
        matches!(error, ExchangeError::Pack(_)),
        "PACK owner error, not an exchange-layer remap: {error:?}"
    );
    error.code().to_owned()
}

#[test]
fn s3_corrupt_exchange_resealed_embedded_pack() {
    let staged = staged_exchange();
    let epoch = state_epoch_id().unwrap();
    let verifier = RepositoryObjectVerifier::new(epoch);

    // Owner-digest equivalence: resealing the untouched exchange reproduces
    // its trailer byte for byte, and the clean exchange preflights.
    let mut unflipped = staged.bytes.clone();
    reseal(&mut unflipped);
    assert_eq!(
        unflipped, staged.bytes,
        "reseal reproduces the owner trailer"
    );
    preflight_repository_exchange(&staged.bytes, &verifier).expect("clean exchange preflights");

    // Two independent flips: different canonical objects, different offsets.
    let last = staged.objects.len() - 1;
    let (flip_a, offset_a) = resealed_object_flip(&staged, 0, 3);
    let (flip_b, offset_b) = resealed_object_flip(&staged, last, 5);
    assert_ne!(offset_a, offset_b);
    for (vector, offset) in [(&flip_a, offset_a), (&flip_b, offset_b)] {
        let differs: Vec<usize> = vector
            .iter()
            .zip(staged.bytes.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(index, _)| index)
            .collect();
        // Exactly the one object byte plus (some of) the resealed trailer.
        assert_eq!(differs[0], offset);
        assert!(
            differs[1..]
                .iter()
                .all(|index| *index >= staged.bytes.len() - TRAILER_LEN),
            "only the object byte and the exchange trailer differ"
        );
        // Target-free preflight reaches the same exact PACK code.
        let error = preflight_repository_exchange(vector, &verifier).unwrap_err();
        assert_eq!(error.code(), PACK_DIGEST_MISMATCH);
    }

    let temp = TempDir::new("exchange-resealed");

    // (1) Populated destination: clean clone, then both vectors refused with
    // the exact PACK code, twice each, head tx / live objects / every file
    // byte-identical.
    let populated = temp.child("populated");
    let clean_report =
        import_repository_exchange(&populated, &staged.bytes, &verifier).expect("clean clone");
    let (head_tx, head_objects) = head_facts(&populated);
    assert_eq!(clean_report.accepted_head.transaction_id(), head_tx);
    assert!(head_objects > 0);
    let before = tree_snapshot(&populated);
    for vector in [&flip_a, &flip_b] {
        for _attempt in 0..2 {
            assert_eq!(
                import_code(&populated, vector, &verifier),
                PACK_DIGEST_MISMATCH
            );
            assert_eq!(head_facts(&populated), (head_tx, head_objects));
            assert!(tree_snapshot(&populated) == before, "no write on refusal");
        }
    }
    // Regression (distinct layer): the same flip WITHOUT resealing stops at
    // the exchange trailer gate, never remapped to the PACK code.
    let mut unresealed = staged.bytes.clone();
    unresealed[offset_a] ^= 0x01;
    let error = import_repository_exchange(&populated, &unresealed, &verifier).unwrap_err();
    assert_eq!(error.code(), "EXCHANGE_DIGEST_MISMATCH");
    assert!(tree_snapshot(&populated) == before);

    // (2) Fresh destination: refusal writes nothing (the target is never
    // created), deterministic on retry; then the clean exchange imports
    // into the same destination with no cleanup and lands on the same head.
    let fresh = temp.child("fresh");
    for vector in [&flip_a, &flip_b, &flip_a] {
        assert_eq!(import_code(&fresh, vector, &verifier), PACK_DIGEST_MISMATCH);
        assert!(!fresh.exists(), "refused import created nothing");
    }
    let report = import_repository_exchange(&fresh, &staged.bytes, &verifier)
        .expect("clean re-import accepted after refusals");
    assert_eq!(report.accepted_head.transaction_id(), head_tx);
    assert_eq!(head_facts(&fresh), (head_tx, head_objects));
    eprintln!(
        "S3_EVIDENCE task=S2B-CORRUPT-001 route=exchange.import resealed=true \
         symbol={PACK_DIGEST_MISMATCH} exchange_len={} flipped_offsets={offset_a},{offset_b} \
         head_objects={head_objects} unresealed_symbol=EXCHANGE_DIGEST_MISMATCH",
        staged.bytes.len()
    );
}
