//! S20-ZJX-READINESS witness (amendment section 13, ZR-12; matrix rows
//! ZT-02, ZT-04, ZT-09, ZT-10, and the helper-only part of ZT-08).
//!
//! The amendment asks for proof that a future optional transport adapter can
//! deliver an exact, supported Sley artifact to Sley's existing authoritative
//! import path without redefining canonical bytes, identities, validation, or
//! persistence. This file is that proof for the two supported artifact
//! profiles, S20-170 repository packs and S20-540 repository exchanges, using
//! only the crate's public production API:
//!
//! - `import_conformance_pack(store, &[u8], verifier)` and
//!   `import_repository_exchange(target, &[u8], verifier)` /
//!   `preflight_repository_exchange(&[u8], verifier)` are the insertion
//!   points. Both take bounded bytes; the only path is the destination.
//! - `export_conformance_pack` and `export_repository_exchange` are the
//!   production exporters a future adapter would read bytes from, so no
//!   serialization is duplicated anywhere in this file.
//!
//! The "transport" here is a deliberately test-only segmented framing
//! (`TestFrame`). It is NOT ZJX, NOT a Sley artifact contract, NOT a format:
//! it has no registered magic, MIME, contract tag, domain, or profile, it
//! lives only in this test target, it is compiled into no library or
//! binary, and it must never be promoted into production. Its one job is to
//! show that bytes reconstructed through an outer layer with short reads,
//! bounded output, late finalization, and outer-only metadata converge on
//! the same production importer, the same accepted facts, and the same owning
//! rejections as the native bytes handed over directly.
//!
//! Nothing below constructs an accepted graph, root, pack, or receipt and
//! hands it to persistence: every accepted fact comes out of the owning
//! importer. Nothing below selects a ZJX release, edition, backend, or
//! profile.

// Declarative witness by construction: long drivers and builders.
#![allow(clippy::too_many_lines)]

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sley_id::{
    CandidateNonce, EntityId, ObjectId, PolicyRootId, PrincipalId, TransactionId, WorkspaceId,
};
use sley_mutate::value::{EntityBodyValue, EntityIdSet, NamespaceBody};
use sley_mutate::{
    BoundPrecondition, CandidateExpiry, CandidateRecord, EntityObjectRecord,
    ExpectedIdentityAbsent, MutationClass, MutationOperation, MutationPayload, PreconditionPayload,
    PreimageRequirement, build_candidate, build_entity_object, full_validation_profile_id,
};
use sley_policy::{
    CandidateValidationLimits, PolicyResourceCeilings, PolicyRootBuilder, PrincipalGrantBuilder,
    build_capability_summary_projection, conformance_registry as policy_registry,
};
use sley_repo::{
    BranchRepository, MAX_EXCHANGE_BYTES, MAX_PACK_BYTES, RepositoryObjectVerifier,
    export_conformance_pack, export_repository_exchange, import_conformance_pack,
    import_repository_exchange, preflight_repository_exchange,
};
use sley_scb1::{
    FixtureContract, ScbError, decode_standalone_fixture, encode_record, encode_standalone_fixture,
};
use sley_state_root::{
    StateRootBuilder, conformance_epoch_id as state_epoch_id,
    conformance_registry as state_registry,
};
use sley_store::ObjectStore;
use sley_txn::{CommitInput, TransactionRepository, TrustedGenesisInput};

// ---------------------------------------------------------------------------
// Test-only framing. Not a format. See the module documentation.
// ---------------------------------------------------------------------------

/// Deliberately unregistered, non-Sley, non-ZJX leader. It shares no prefix
/// with `SLEYSCB1`, carries no `sley2.` domain, and is not a MIME type.
const TEST_FRAME_LEADER: &[u8; 24] = b"ZR12-TEST-FRAME/NOT-FMT!";

/// Outcomes of the test-only outer layer. These are helper-local values,
/// not Sley error symbols; the amendment forbids inventing public codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransportFailure {
    /// The leader did not match: this is not a test frame.
    Leader,
    /// The declared payload length exceeds the caller's ceiling (checked
    /// before any payload allocation; a length claim is not allocation
    /// authority).
    Ceiling,
    /// The source ended before the frame was complete.
    Truncated,
    /// Segments would produce more bytes than declared (enforced while
    /// output is produced, not after materialization).
    Overrun,
    /// Segments produced fewer bytes than declared.
    Underrun,
    /// The finalizer after the complete payload did not match.
    Finalizer,
    /// Bytes followed the finalizer.
    TrailingBytes,
    /// The source failed with an I/O error.
    Io,
}

/// Result of a successful reconstruction: the exact inner bytes plus the
/// outer-only annotation, which is transport metadata and confers nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Reconstructed {
    annotation: Vec<u8>,
    payload: Vec<u8>,
}

/// Frames `payload` into segments of at most `segment` bytes with an
/// outer-only annotation and a BLAKE3 finalizer over the payload.
///
/// Layout (all integers big-endian, fixed width; nothing here is SCB1):
///
/// ```text
/// leader[24] || u32 annotation_len || annotation || u64 payload_len ||
/// u32 segment_count || (u32 segment_len || segment)* || finalizer[32]
/// ```
fn frame(payload: &[u8], annotation: &[u8], segment: usize) -> Vec<u8> {
    assert!(segment > 0);
    let segments = payload.chunks(segment).collect::<Vec<_>>();
    let mut out = Vec::with_capacity(payload.len() + annotation.len() + 96);
    out.extend_from_slice(TEST_FRAME_LEADER);
    out.extend_from_slice(&u32::try_from(annotation.len()).unwrap().to_be_bytes());
    out.extend_from_slice(annotation);
    out.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    out.extend_from_slice(&u32::try_from(segments.len()).unwrap().to_be_bytes());
    for chunk in segments {
        out.extend_from_slice(&u32::try_from(chunk.len()).unwrap().to_be_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(blake3::hash(payload).as_bytes());
    out
}

/// Fills `buffer` completely from `source`, tolerating short reads. A read
/// of zero bytes before the buffer is full is a truncation.
fn fill(source: &mut impl Read, buffer: &mut [u8]) -> Result<(), TransportFailure> {
    let mut filled = 0;
    while filled < buffer.len() {
        match source.read(&mut buffer[filled..]) {
            Ok(0) => return Err(TransportFailure::Truncated),
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Err(TransportFailure::Io),
        }
    }
    Ok(())
}

fn read_u32(source: &mut impl Read) -> Result<usize, TransportFailure> {
    let mut bytes = [0u8; 4];
    fill(source, &mut bytes)?;
    Ok(u32::from_be_bytes(bytes) as usize)
}

/// Reconstructs the exact inner bytes from a test frame read through a
/// bounded reader. `ceiling` is the caller's output bound: the declared
/// length is validated against it before the payload buffer exists, and the
/// running output is checked against the declared length as each segment
/// lands. The finalizer is checked only after the complete payload, so a
/// finalizer failure is a late outer failure after a complete-looking inner
/// payload (ZT-09). No bytes are returned unless every check passed.
fn reconstruct(source: &mut impl Read, ceiling: usize) -> Result<Reconstructed, TransportFailure> {
    let mut leader = [0u8; 24];
    fill(source, &mut leader)?;
    if leader != *TEST_FRAME_LEADER {
        return Err(TransportFailure::Leader);
    }
    let annotation_len = read_u32(source)?;
    if annotation_len > 4_096 {
        return Err(TransportFailure::Ceiling);
    }
    let mut annotation = vec![0u8; annotation_len];
    fill(source, &mut annotation)?;
    let mut declared = [0u8; 8];
    fill(source, &mut declared)?;
    let declared = u64::from_be_bytes(declared);
    // A declared length is a claim, validated before it becomes capacity.
    let declared = usize::try_from(declared).map_err(|_| TransportFailure::Ceiling)?;
    if declared > ceiling {
        return Err(TransportFailure::Ceiling);
    }
    let segment_count = read_u32(source)?;
    let mut payload = Vec::with_capacity(declared);
    for _ in 0..segment_count {
        let len = read_u32(source)?;
        // Enforced while output is produced: never beyond the declared bound.
        if payload.len() + len > declared {
            return Err(TransportFailure::Overrun);
        }
        let start = payload.len();
        payload.resize(start + len, 0);
        fill(source, &mut payload[start..])?;
    }
    if payload.len() != declared {
        return Err(TransportFailure::Underrun);
    }
    let mut finalizer = [0u8; 32];
    fill(source, &mut finalizer)?;
    if finalizer != *blake3::hash(&payload).as_bytes() {
        return Err(TransportFailure::Finalizer);
    }
    let mut extra = [0u8; 1];
    match source.read(&mut extra) {
        Ok(0) => {}
        Ok(_) => return Err(TransportFailure::TrailingBytes),
        Err(_) => return Err(TransportFailure::Io),
    }
    Ok(Reconstructed {
        annotation,
        payload,
    })
}

/// A reader that never returns more than `max_chunk` bytes per call, so the
/// reconstruction is exercised under short reads.
struct ShortReader<'a> {
    bytes: &'a [u8],
    offset: usize,
    max_chunk: usize,
}

impl<'a> ShortReader<'a> {
    fn new(bytes: &'a [u8], max_chunk: usize) -> Self {
        assert!(max_chunk > 0);
        Self {
            bytes,
            offset: 0,
            max_chunk,
        }
    }
}

impl Read for ShortReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let remaining = &self.bytes[self.offset..];
        let n = remaining.len().min(buffer.len()).min(self.max_chunk);
        buffer[..n].copy_from_slice(&remaining[..n]);
        self.offset += n;
        Ok(n)
    }
}

/// A reader that fails with an I/O error once `fail_after` bytes were served.
struct FailingReader<'a> {
    inner: ShortReader<'a>,
    fail_after: usize,
}

impl Read for FailingReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.inner.offset >= self.fail_after {
            return Err(io::Error::other("injected transport fault"));
        }
        let allowed = (self.fail_after - self.inner.offset).min(buffer.len());
        self.inner.read(&mut buffer[..allowed])
    }
}

/// Reconstructs under a given short-read width and returns the payload.
fn reconstruct_with(frame: &[u8], max_chunk: usize, ceiling: usize) -> Reconstructed {
    reconstruct(&mut ShortReader::new(frame, max_chunk), ceiling).unwrap()
}

// ---------------------------------------------------------------------------
// Fixtures and scaffolding.
// ---------------------------------------------------------------------------

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "sley2-zjx-readiness-{label}-{}-{counter}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn child(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn conformance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("conformance")
}

fn hex_decode(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// The frozen S20-170 conformance pack (`conformance/repository-pack/v1/accepted.json`).
fn pinned_pack() -> (Vec<u8>, String) {
    let text =
        fs::read_to_string(conformance_dir().join("repository-pack/v1/accepted.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    (
        hex_decode(json["stored_hex"].as_str().unwrap()),
        json["repository_pack_id"].as_str().unwrap().to_owned(),
    )
}

/// The frozen S20-170 rejection corpus: `(id, input bytes)`.
fn pinned_pack_rejections() -> Vec<(String, Vec<u8>)> {
    let text =
        fs::read_to_string(conformance_dir().join("repository-pack/v1/rejected.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    json["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["id"].as_str().unwrap().to_owned(),
                hex_decode(entry["input_hex"].as_str().unwrap()),
            )
        })
        .collect()
}

/// The frozen S20-540 conformance exchange (`conformance/repository-exchange/v1/accepted.json`).
fn pinned_exchange() -> (Vec<u8>, String, String) {
    let text =
        fs::read_to_string(conformance_dir().join("repository-exchange/v1/accepted.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let vector = &json["vectors"][0];
    (
        hex_decode(vector["exchange_hex"].as_str().unwrap()),
        vector["repository_exchange_id"]
            .as_str()
            .unwrap()
            .to_owned(),
        vector["repository_pack_id"].as_str().unwrap().to_owned(),
    )
}

/// The frozen S20-540 rejection corpus: `(id, expected code, input bytes)`.
fn pinned_exchange_rejections() -> Vec<(String, String, Vec<u8>)> {
    let text =
        fs::read_to_string(conformance_dir().join("repository-exchange/v1/rejected.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    json["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["id"].as_str().unwrap().to_owned(),
                entry["expected_code"].as_str().unwrap().to_owned(),
                hex_decode(entry["input_hex"].as_str().unwrap()),
            )
        })
        .collect()
}

/// The verifier the frozen pack fixture's objects need: the two synthetic
/// S20-100 fixture contracts (the same verifier the crate's own pack tests
/// use).
fn fixture_verifier(record: &[u8]) -> Result<ObjectId, ScbError> {
    decode_standalone_fixture(record, FixtureContract::EmptyObject)
        .or_else(|_| decode_standalone_fixture(record, FixtureContract::RequiredBool))
        .map(|fixture| fixture.object_id)
}

/// The production verifier for entity objects under the conformance epoch,
/// which every exchange in this file carries.
fn exchange_verifier() -> RepositoryObjectVerifier {
    RepositoryObjectVerifier::new(state_epoch_id().unwrap())
}

/// Every regular file under `root` as `relative path -> bytes`.
fn tree_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = fs::read_dir(dir)
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        entries.sort_by_key(std::fs::DirEntry::path);
        for entry in entries {
            let path = entry.path();
            let kind = entry.file_type().unwrap();
            assert!(
                !kind.is_symlink(),
                "unexpected symlink at {}",
                path.display()
            );
            if kind.is_dir() {
                walk(base, &path, out);
            } else {
                let relative = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                out.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    if root.exists() {
        walk(root, root, &mut out);
    }
    out
}

fn namespace_body() -> EntityBodyValue {
    EntityBodyValue::Namespace(NamespaceBody {
        parent: None,
        members: EntityIdSet::from_unsorted(vec![]).unwrap(),
    })
}

/// A source object store and one accepted root, built through the public
/// state-root builder exactly as the crate's own pack tests build theirs.
fn pack_source() -> (TempDir, ObjectStore, sley_state_root::AcceptedStateRoot) {
    let temp = TempDir::new("pack-source");
    let store = ObjectStore::new(&temp.0);
    let (first, first_id) =
        encode_standalone_fixture(FixtureContract::EmptyObject, &encode_record(&[]).unwrap())
            .unwrap();
    let bool_payload = encode_record(&[(1, vec![1])]).unwrap();
    let (second, second_id) =
        encode_standalone_fixture(FixtureContract::RequiredBool, &bool_payload).unwrap();
    store.put(first_id, &first, &fixture_verifier).unwrap();
    store.put(second_id, &second, &fixture_verifier).unwrap();
    let registry = state_registry().unwrap();
    let root = StateRootBuilder::new(
        WorkspaceId::from_bytes([7; 32]),
        first_id,
        second_id,
        PolicyRootId::from_bytes([9; 32]),
    )
    .entity_binding(EntityId::from_bytes([8; 32]), first_id)
    .entry_point(EntityId::from_bytes([8; 32]))
    .build(&registry)
    .unwrap();
    (temp, store, root)
}

/// A complete source repository (genesis, one commit, branch `main`
/// advanced to the commit) built only through production authorities, so
/// `export_repository_exchange` has something real to export.
fn exchange_source() -> (TempDir, PathBuf, TransactionId) {
    const NOW: u64 = 1_000;
    let temp = TempDir::new("exchange-source");
    let root = temp.child("source");
    fs::create_dir(&root).unwrap();
    let transactions = TransactionRepository::new(&root);
    let branches = BranchRepository::new(&root);
    let workspace_id = WorkspaceId::from_bytes([1; 32]);
    let principal_id = PrincipalId::from_bytes([2; 32]);
    let base_entity = EntityId::from_bytes([10; 32]);
    let grant = PrincipalGrantBuilder::new(PolicyResourceCeilings::new(
        1_000, 1_000, 1_000, 100, 100, 100,
    ))
    .mutation_class(MutationClass::CreateEntity)
    .build()
    .unwrap();
    let policy = PolicyRootBuilder::new(workspace_id)
        .principal_grant(principal_id, grant)
        .build(&policy_registry().unwrap())
        .unwrap();
    let epoch = state_epoch_id().unwrap();
    let verifier = RepositoryObjectVerifier::new(epoch);
    let base_object = build_entity_object(
        epoch,
        &EntityObjectRecord {
            entity_id: base_entity,
            body: namespace_body(),
            label: None,
            semantic_fingerprint: None,
        },
    )
    .unwrap();
    let store = ObjectStore::new(&root);
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
    let base_state = StateRootBuilder::new(workspace_id, anchors[0], anchors[1], policy.root())
        .entity_binding(base_entity, base_object.object_id())
        .build(&state_registry().unwrap())
        .unwrap();
    let genesis = transactions
        .initialize_trusted_genesis(TrustedGenesisInput::new(
            &base_state,
            &policy,
            core::slice::from_ref(&base_object),
            &[],
        ))
        .unwrap()
        .transaction_id();
    let nonce = CandidateNonce::from_bytes([30; 32]);
    let target = EntityId::derive(workspace_id, nonce, 3, 0);
    let summary = build_capability_summary_projection(
        principal_id,
        workspace_id,
        policy.root(),
        base_state.root,
        &[],
    )
    .unwrap();
    let candidate = build_candidate(&CandidateRecord {
        format_version: 1,
        workspace_id,
        base_transaction_id: genesis,
        base_root: base_state.root,
        schema_epoch_id: base_state.record.schema_epoch_id,
        policy_root_id: policy.root(),
        principal_id,
        capability_summary_digest: summary.digest(),
        operations: vec![MutationOperation {
            ordinal: 0,
            class: MutationClass::CreateEntity,
            target_kind: 3,
            target_entity: target,
            field_tag: None,
            payload: MutationPayload::CreateEntity(namespace_body()),
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
        candidate_nonce: nonce,
        expiry: CandidateExpiry::unix_millis(NOW + 1_000),
    })
    .unwrap();
    let head = transactions
        .commit(CommitInput::new(
            genesis,
            &candidate.stored_bytes,
            principal_id,
            &[],
            NOW,
            CandidateValidationLimits::full_v1(),
        ))
        .unwrap()
        .transaction_id();
    branches.create_branch("main", genesis).unwrap();
    branches.advance_branch("main", genesis, head).unwrap();
    (temp, root, head)
}

// ---------------------------------------------------------------------------
// ZT-02 / ZT-10: pinned native fixtures converge through the production
// importer whether handed over directly or reconstructed.
// ---------------------------------------------------------------------------

#[test]
fn pinned_pack_direct_and_reconstructed_inputs_converge_through_the_production_importer() {
    let (native, expected_pack_id) = pinned_pack();
    for (max_chunk, segment) in [
        (1, 1),
        (7, 13),
        (64, 4_096),
        (native.len() + 1, native.len()),
    ] {
        let framed = frame(&native, b"outer-only annotation; confers nothing", segment);
        let rebuilt = reconstruct_with(&framed, max_chunk, MAX_PACK_BYTES);
        // ZR-06 minimum invariant: reconstruct(transport(native)) == native.
        assert_eq!(rebuilt.payload, native);

        let direct_temp = TempDir::new("pack-direct");
        let direct_store = ObjectStore::new(&direct_temp.0);
        let direct = import_conformance_pack(&direct_store, &native, &fixture_verifier).unwrap();
        let rebuilt_temp = TempDir::new("pack-rebuilt");
        let rebuilt_store = ObjectStore::new(&rebuilt_temp.0);
        let via_transport =
            import_conformance_pack(&rebuilt_store, &rebuilt.payload, &fixture_verifier).unwrap();

        // owning_import(reconstructed) == owning_import(native): the same
        // contract-defined accepted facts.
        assert_eq!(
            hex_encode(via_transport.pack_id.as_bytes()),
            expected_pack_id
        );
        assert_eq!(via_transport.pack_id, direct.pack_id);
        assert_eq!(via_transport.roots, direct.roots);
        assert_eq!(via_transport.promoted_objects, direct.promoted_objects);
        assert_eq!(via_transport.present_objects, direct.present_objects);
        // ZT-10: two different destination paths, byte-identical object
        // stores; the destination is not part of any identity.
        assert_eq!(
            tree_snapshot(&rebuilt_temp.0),
            tree_snapshot(&direct_temp.0)
        );
        assert!(!tree_snapshot(&direct_temp.0).is_empty());
    }
}

#[test]
fn pinned_exchange_direct_and_reconstructed_inputs_converge_through_the_production_importer() {
    let (native, expected_exchange_id, expected_pack_id) = pinned_exchange();
    let verifier = exchange_verifier();
    for (max_chunk, segment) in [
        (1, 1),
        (5, 17),
        (256, 1_024),
        (native.len() + 1, native.len()),
    ] {
        let framed = frame(&native, b"", segment);
        let rebuilt = reconstruct_with(&framed, max_chunk, MAX_EXCHANGE_BYTES);
        assert_eq!(rebuilt.payload, native);

        let direct = preflight_repository_exchange(&native, &verifier).unwrap();
        let via_transport = preflight_repository_exchange(&rebuilt.payload, &verifier).unwrap();
        assert_eq!(via_transport, direct);
        assert_eq!(
            hex_encode(via_transport.exchange_id.as_bytes()),
            expected_exchange_id
        );
        assert_eq!(
            hex_encode(via_transport.pack_id.as_bytes()),
            expected_pack_id
        );

        let temp = TempDir::new("exchange-targets");
        let direct_target = temp.child("direct");
        let rebuilt_target = temp.child("rebuilt");
        let direct_report = import_repository_exchange(&direct_target, &native, &verifier).unwrap();
        let rebuilt_report =
            import_repository_exchange(&rebuilt_target, &rebuilt.payload, &verifier).unwrap();
        assert_eq!(rebuilt_report.exchange_id, direct_report.exchange_id);
        assert_eq!(
            rebuilt_report.accepted_head.transaction_id(),
            direct_report.accepted_head.transaction_id()
        );
        assert_eq!(rebuilt_report.receipts, direct_report.receipts);
        assert_eq!(rebuilt_report.branches, direct_report.branches);
        assert_eq!(
            rebuilt_report.promoted_objects,
            direct_report.promoted_objects
        );
        assert_eq!(
            rebuilt_report.present_objects,
            direct_report.present_objects
        );
        // ZT-10: clone-equivalent trees under two destination paths.
        let direct_tree = tree_snapshot(&direct_target);
        assert!(!direct_tree.is_empty());
        assert_eq!(tree_snapshot(&rebuilt_target), direct_tree);
    }
}

// ---------------------------------------------------------------------------
// ZT-01 / ZT-02: the production exporters are the byte source; nothing is
// re-serialized on the transport side.
// ---------------------------------------------------------------------------

#[test]
fn production_pack_exporter_bytes_survive_reconstruction_and_reimport_exactly() {
    let (_source_temp, source, root) = pack_source();
    let exported =
        export_conformance_pack(&source, std::slice::from_ref(&root), &fixture_verifier).unwrap();
    let framed = frame(&exported.stored_bytes, b"exporter", 3);
    let rebuilt = reconstruct_with(&framed, 2, MAX_PACK_BYTES);
    assert_eq!(rebuilt.payload, exported.stored_bytes);

    let clean_temp = TempDir::new("pack-clean");
    let clean = ObjectStore::new(&clean_temp.0);
    let report = import_conformance_pack(&clean, &rebuilt.payload, &fixture_verifier).unwrap();
    assert_eq!(report.pack_id, exported.pack_id);
    assert_eq!(report.roots, vec![root.clone()]);
    assert_eq!(report.promoted_objects, 2);
    // The reconstructed root is the exact exported root, framing and trailer
    // included.
    assert_eq!(report.roots[0].stored_bytes, root.stored_bytes);
    // Re-exporting from the clean store reproduces the exact bytes.
    let again = export_conformance_pack(&clean, &[root], &fixture_verifier).unwrap();
    assert_eq!(again.stored_bytes, exported.stored_bytes);
}

#[test]
fn production_exchange_exporter_bytes_survive_reconstruction_and_reimport_exactly() {
    let (temp, source_root, head) = exchange_source();
    let verifier = exchange_verifier();
    let exported = export_repository_exchange(&source_root, &verifier).unwrap();
    assert_eq!(exported.accepted_head.transaction_id, head);
    let framed = frame(&exported.stored_bytes, b"exporter", 11);
    let rebuilt = reconstruct_with(&framed, 3, MAX_EXCHANGE_BYTES);
    assert_eq!(rebuilt.payload, exported.stored_bytes);

    let direct_target = temp.child("direct");
    let rebuilt_target = temp.child("rebuilt");
    let direct =
        import_repository_exchange(&direct_target, &exported.stored_bytes, &verifier).unwrap();
    let via_transport =
        import_repository_exchange(&rebuilt_target, &rebuilt.payload, &verifier).unwrap();
    assert_eq!(via_transport.exchange_id, exported.exchange_id);
    assert_eq!(via_transport.exchange_id, direct.exchange_id);
    assert_eq!(via_transport.accepted_head.transaction_id(), head);
    assert_eq!(via_transport.receipts, direct.receipts);
    assert_eq!(via_transport.branches, direct.branches);
    assert_eq!(
        tree_snapshot(&rebuilt_target),
        tree_snapshot(&direct_target)
    );
    // The clone re-exports to the identical exchange bytes and identity.
    let re_exported = export_repository_exchange(&rebuilt_target, &verifier).unwrap();
    assert_eq!(re_exported.stored_bytes, exported.stored_bytes);
    assert_eq!(re_exported.exchange_id, exported.exchange_id);
}

// ---------------------------------------------------------------------------
// ZT-03 / ZT-04: malformed native inputs keep their owning rejection after a
// successful outer reconstruction, and nothing is written.
// ---------------------------------------------------------------------------

#[test]
fn pinned_pack_rejections_keep_their_owning_symbol_after_reconstruction() {
    let corpus = pinned_pack_rejections();
    assert_eq!(corpus.len(), 10);
    for (id, input) in corpus {
        let direct_temp = TempDir::new("pack-reject-direct");
        let direct_store = ObjectStore::new(&direct_temp.0);
        let direct =
            import_conformance_pack(&direct_store, &input, &fixture_verifier).expect_err(&id);

        let framed = frame(&input, id.as_bytes(), 9);
        let rebuilt = reconstruct_with(&framed, 4, MAX_PACK_BYTES);
        assert_eq!(rebuilt.payload, input, "{id}");
        let rebuilt_temp = TempDir::new("pack-reject-rebuilt");
        let rebuilt_store = ObjectStore::new(&rebuilt_temp.0);
        let via_transport =
            import_conformance_pack(&rebuilt_store, &rebuilt.payload, &fixture_verifier)
                .expect_err(&id);

        // The same owning rejection under the same context; the outer layer
        // neither erased nor replaced it.
        assert_eq!(via_transport.symbol(), direct.symbol(), "{id}");
        // Every symbol belongs to an owner the contract names: the pack
        // itself, SCB1, or the schema-epoch registry the pack decodes through.
        let symbol = via_transport.symbol();
        assert!(
            ["PACK_", "SCB_", "SCHEMA_"]
                .iter()
                .any(|family| symbol.starts_with(family)),
            "{id}: {symbol}"
        );
        eprintln!("ZT-03 pack rejection {id}: {symbol}");
        // No-write guarantee on invalid preflight, from either route.
        assert!(tree_snapshot(&direct_temp.0).is_empty(), "{id}");
        assert!(tree_snapshot(&rebuilt_temp.0).is_empty(), "{id}");
    }
}

#[test]
fn pinned_exchange_rejections_keep_their_owning_code_after_reconstruction() {
    let corpus = pinned_exchange_rejections();
    assert_eq!(corpus.len(), 6);
    let verifier = exchange_verifier();
    for (id, expected_code, input) in corpus {
        let direct = preflight_repository_exchange(&input, &verifier).expect_err(&id);
        assert_eq!(direct.code(), expected_code, "{id}");

        let framed = frame(&input, b"", 23);
        let rebuilt = reconstruct_with(&framed, 6, MAX_EXCHANGE_BYTES);
        assert_eq!(rebuilt.payload, input, "{id}");
        let via_transport =
            preflight_repository_exchange(&rebuilt.payload, &verifier).expect_err(&id);
        assert_eq!(via_transport.code(), expected_code, "{id}");

        // Persistence-capable import of the reconstructed malformed bytes
        // into a fresh target fails with the same code and creates nothing.
        let temp = TempDir::new("exchange-reject");
        let target = temp.child("fresh");
        let failed =
            import_repository_exchange(&target, &rebuilt.payload, &verifier).expect_err(&id);
        assert_eq!(failed.code(), expected_code, "{id}");
        assert!(!target.exists(), "{id}");
    }
}

// ---------------------------------------------------------------------------
// ZT-09: outer failures, including late finalization after a complete-looking
// inner payload, never reach the importer and never persist anything.
// ---------------------------------------------------------------------------

/// How a future composition layer fails: either the outer layer refused
/// (the importer was never named) or the owning importer refused (its exact
/// symbol or code is preserved). The two are never conflated.
#[derive(Clone, Debug, Eq, PartialEq)]
enum AdapterFailure {
    Outer(TransportFailure),
    Inner(String),
}

/// The shape a future composition layer takes: reconstruct completely, then
/// hand the bytes to the owning importer. An outer failure short-circuits
/// before the importer is named.
fn adapter_import_pack(
    store: &ObjectStore,
    framed: &[u8],
) -> Result<sley_repo::ImportReport, AdapterFailure> {
    let rebuilt = reconstruct(&mut ShortReader::new(framed, 5), MAX_PACK_BYTES)
        .map_err(AdapterFailure::Outer)?;
    import_conformance_pack(store, &rebuilt.payload, &fixture_verifier)
        .map_err(|error| AdapterFailure::Inner(error.symbol().to_owned()))
}

fn adapter_import_exchange(
    target: &Path,
    framed: &[u8],
) -> Result<sley_repo::ExchangeImportReport, AdapterFailure> {
    let rebuilt = reconstruct(&mut ShortReader::new(framed, 5), MAX_EXCHANGE_BYTES)
        .map_err(AdapterFailure::Outer)?;
    import_repository_exchange(target, &rebuilt.payload, &exchange_verifier())
        .map_err(|error| AdapterFailure::Inner(error.code().to_owned()))
}

#[test]
fn late_outer_failure_after_a_complete_inner_pack_payload_persists_nothing() {
    let (native, _) = pinned_pack();
    let mut framed = frame(&native, b"", 16);
    // The complete inner payload is present; only the finalizer is wrong.
    let last = framed.len() - 1;
    framed[last] ^= 0x01;
    let temp = TempDir::new("pack-late-fail");
    let store = ObjectStore::new(&temp.0);
    let outcome = adapter_import_pack(&store, &framed).unwrap_err();
    assert_eq!(outcome, AdapterFailure::Outer(TransportFailure::Finalizer));
    assert!(tree_snapshot(&temp.0).is_empty());

    // Trailing bytes after a correct finalizer are also an outer failure.
    let mut trailing = frame(&native, b"", 16);
    trailing.push(0);
    let outcome = adapter_import_pack(&store, &trailing).unwrap_err();
    assert_eq!(
        outcome,
        AdapterFailure::Outer(TransportFailure::TrailingBytes)
    );
    assert!(tree_snapshot(&temp.0).is_empty());

    // Truncation inside the last segment: nothing reaches the importer.
    let cut = frame(&native, b"", 16);
    let cut = &cut[..cut.len() - 40];
    let outcome = adapter_import_pack(&store, cut).unwrap_err();
    assert_eq!(outcome, AdapterFailure::Outer(TransportFailure::Truncated));
    assert!(tree_snapshot(&temp.0).is_empty());

    // The same bytes with an intact outer layer import normally afterwards:
    // the failed attempts left no state that changes the outcome.
    let report = adapter_import_pack(&store, &frame(&native, b"", 16)).unwrap();
    assert_eq!(report.promoted_objects, 2);
}

#[test]
fn late_outer_failure_after_a_complete_inner_exchange_payload_persists_nothing() {
    let (native, _, _) = pinned_exchange();
    let mut framed = frame(&native, b"", 64);
    let last = framed.len() - 1;
    framed[last] ^= 0x80;
    let temp = TempDir::new("exchange-late-fail");
    let target = temp.child("clone");
    let outcome = adapter_import_exchange(&target, &framed).unwrap_err();
    assert_eq!(outcome, AdapterFailure::Outer(TransportFailure::Finalizer));
    // No marker, no layout, no head: the target does not even exist.
    assert!(!target.exists());

    let report = adapter_import_exchange(&target, &frame(&native, b"", 64)).unwrap();
    assert_eq!(report.receipts, 2);
    assert_eq!(report.branches, 2);
}

// ---------------------------------------------------------------------------
// ZT-08 (helper only) and ZR-07: the test-only reader handles short reads,
// premature EOF, I/O failure, and bounds output while producing it. These
// prove the helper's behavior, not production streaming support.
// ---------------------------------------------------------------------------

#[test]
fn test_frame_reader_bounds_output_and_fails_closed_on_every_outer_defect() {
    let (native, _) = pinned_pack();
    let good = frame(&native, b"note", 100);

    // Every short-read width reconstructs the same bytes.
    for width in [1, 2, 3, 31, 1_000, good.len()] {
        assert_eq!(
            reconstruct_with(&good, width, MAX_PACK_BYTES).payload,
            native
        );
    }

    // A declared length beyond the caller's ceiling fails before any
    // payload allocation, regardless of how many bytes actually follow.
    let error = reconstruct(&mut ShortReader::new(&good, 8), native.len() - 1).unwrap_err();
    assert_eq!(error, TransportFailure::Ceiling);

    // A wrong leader is refused before anything else is read.
    let mut wrong_leader = good.clone();
    wrong_leader[0] ^= 0xff;
    assert_eq!(
        reconstruct(&mut ShortReader::new(&wrong_leader, 8), MAX_PACK_BYTES).unwrap_err(),
        TransportFailure::Leader
    );

    // Premature EOF at every prefix length is a truncation (or, exactly at
    // the end of the finalizer, success).
    for cut in [0, 10, 24, 30, 40, 100, good.len() - 33, good.len() - 1] {
        let error =
            reconstruct(&mut ShortReader::new(&good[..cut], 8), MAX_PACK_BYTES).unwrap_err();
        assert_eq!(error, TransportFailure::Truncated, "cut at {cut}");
    }

    // An I/O failure mid-stream is an I/O failure, not partial success.
    let mut failing = FailingReader {
        inner: ShortReader::new(&good, 8),
        fail_after: 200,
    };
    assert_eq!(
        reconstruct(&mut failing, MAX_PACK_BYTES).unwrap_err(),
        TransportFailure::Io
    );

    // Overrun: a segment claims more than the declared payload allows. It is
    // refused when that segment header lands, before its bytes are read.
    let mut overrun = frame(&native, b"", 100);
    let header = 24 + 4 + 8 + 4; // leader, annotation_len, payload_len, segment_count
    let first_segment_len = u32::try_from(native.len() + 1).unwrap().to_be_bytes();
    overrun[header..header + 4].copy_from_slice(&first_segment_len);
    assert_eq!(
        reconstruct(&mut ShortReader::new(&overrun, 8), MAX_PACK_BYTES).unwrap_err(),
        TransportFailure::Overrun
    );

    // Underrun: fewer segments than the declared payload needs.
    let mut underrun = frame(&native, b"", 100);
    let segment_count = u32::try_from(native.len().div_ceil(100) - 1)
        .unwrap()
        .to_be_bytes();
    underrun[24 + 4 + 8..24 + 4 + 8 + 4].copy_from_slice(&segment_count);
    let tail = underrun.len() - 32 - 4 - (native.len() % 100);
    let mut underrun_short = underrun[..tail].to_vec();
    underrun_short.extend_from_slice(blake3::hash(&native).as_bytes());
    assert_eq!(
        reconstruct(&mut ShortReader::new(&underrun_short, 8), MAX_PACK_BYTES).unwrap_err(),
        TransportFailure::Underrun
    );
}

// ---------------------------------------------------------------------------
// ZT-10: outer-only annotations are transport metadata. They change the
// frame, never the inner bytes or any Sley identity, and grant nothing.
// ---------------------------------------------------------------------------

#[test]
fn outer_annotations_change_the_frame_but_not_the_inner_identity() {
    let (native, expected_pack_id) = pinned_pack();
    let plain = frame(&native, b"", 50);
    let annotated = frame(
        &native,
        b"contains-a-root; compressed=no; producer=nobody",
        50,
    );
    assert_ne!(plain, annotated);
    let plain_rebuilt = reconstruct_with(&plain, 7, MAX_PACK_BYTES);
    let annotated_rebuilt = reconstruct_with(&annotated, 7, MAX_PACK_BYTES);
    assert_eq!(plain_rebuilt.payload, annotated_rebuilt.payload);
    assert_ne!(plain_rebuilt.annotation, annotated_rebuilt.annotation);

    let temp = TempDir::new("annotated");
    let store = ObjectStore::new(&temp.0);
    let report =
        import_conformance_pack(&store, &annotated_rebuilt.payload, &fixture_verifier).unwrap();
    assert_eq!(hex_encode(report.pack_id.as_bytes()), expected_pack_id);
    // The annotation is not part of the pack, the pack identity, the root
    // identity, or the stored objects; the importer never saw it.
    assert_eq!(report.promoted_objects, 2);
}
