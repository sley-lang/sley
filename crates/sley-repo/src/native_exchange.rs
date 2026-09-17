//! Native repository exchange transport v1 (`docs/spec/NATIVE_TEST_ADMISSION_V1.md`
//! Appendix B, reservations in `docs/spec/NATIVE_TEST_RESERVATIONS_V1.md`).
//!
//! This is a transport profile under the existing repository exchange domain,
//! not an epoch-1 SSMC envelope or a new program semantic epoch. The outer
//! envelope is magic-disjoint (`SLEYXCH2`): old `SLEYSCB1` contract-540
//! decoders refuse these bytes on the magic, and their registry, schema, and
//! decoder stay unchanged. The exchanged identifier stays in the existing
//! typed family: the exchange ID is a `RepositoryExchangeId` derived over the
//! `SLEYXCH2` preimage variant, exactly as the reservations ledger permits for
//! reviewed magic-disjoint versions. The transport descriptor
//! `NativeExchangeProfileV1` (`SLEYNTP1`) is the one fixed record from
//! Appendix B; its ID binds the exact transport rules and is checked on both
//! the envelope and the record before any payload is trusted.
//!
//! Record tags 1..8 retain the v1 logical fields with tag 1 fixed at 2.
//! Receipt elements carry canonical chunks instead of whole bytes, field 9
//! carries the exact sorted union of trust policy IDs referenced by every
//! native measurement and acceptance statement, and field 10 carries the
//! transport profile ID. The digest tree reuses the exact v1 leaf and node
//! construction over the reconstructed receipt bytes and appends the section-5
//! transport leaf last. Trust IDs never install receiver trust; they are an
//! exact-match claim the importer checks against caller-supplied manifests.
//!
//! This slice owns the profile descriptor, canonical chunking, and the
//! record/envelope codecs with byte vectors. Export, import preflight with
//! the Appendix B counters, promotion, and replay arrive in later N6 slices.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;

use crate::refs::{BranchError, BranchRepository, MAX_BRANCHES, ResolvedBranch};
use sley_id::{
    EntityId, NativeExchangeProfileId, ObjectId, ReceiptId, RepositoryExchangeId, StateRoot,
    TestReportId, TransactionId, WorkspaceId,
};
use sley_scb1::{ScbErrorCode, encode_list, encode_record, encode_union, encode_uvar};
use sley_store::{CanonicalVerifier, ObjectStore};
use sley_tests::{
    HistoricalTrustPolicyV1, MeasuredTestAttestationV1, NativeTestPlanV1, NativeTestReportV1,
};
use sley_txn::{
    CommitError, ImportedNativeTransactionReceipt, ImportedReceipt, NATIVE_RECEIPT_MAGIC,
    NativeCommitError, NativeTestExecutor, RECEIPT_MAGIC, RepositoryMaintenanceGuard,
    TransactionCodecError, TransactionKind, TransactionRepository,
    acquire_exclusive_repository_maintenance_nonblocking, acquire_shared_repository_maintenance,
    check_execution_coverage, import_native_transaction_receipt, import_transaction_receipt,
    initialize_repository_maintenance, native_receipt_trust_policy_ids,
    verify_acceptance_statement, verify_any_receipt_against_objects,
    verify_measurement_attestation,
};

use crate::exchange::{
    BRANCH_SECTION, EXCHANGE_DIRECTORY, EXCHANGE_VERSION_DIRECTORY, ExchangeBranchEntry,
    ExchangeError, ExchangeErrorCode, ExchangeHeadEntry, HEAD_LEN, HEAD_SECTION, INDEX_DIRECTORY,
    MAX_EMBEDDED_PACK_BYTES, MAX_EXCHANGE_ALLOCATION, MAX_EXCHANGE_BRANCHES, MAX_EXCHANGE_BYTES,
    MAX_EXCHANGE_RECEIPTS, MAX_PREFLIGHT_BINDING_VISITS, MAX_PREFLIGHT_OBJECT_BYTES,
    MAX_PREFLIGHT_OBJECT_VERIFICATIONS, MAX_PREFLIGHT_RECEIPT_BYTES, ORIGIN_SUFFIX, PACK_SECTION,
    RECEIPT_SECTION, RECEIPT_SUFFIX, REF_SUFFIX, REPOSITORY_LAYOUT_ENTRIES, STAGE_SUFFIX,
    STAGE_TEMPORARY_SUFFIX, branch_name_key, collect_files_with_suffix, content_leaf,
    create_real_directory, decode_head_bytes, embedded_pack_header_is_tag_170,
    encode_branch_element, encode_bytes, hex_id, merkle_root, read_regular_file,
    real_directory_metadata, stored_head_bytes, sync_directory,
};
use crate::refs::{
    BRANCH_STAGE_PREFIX, BranchErrorCode, BranchName, ImportedBranchRecord, ImportedBranchRef,
    ensure_key_path, import_branch_record, import_branch_ref, persist_expected_ref,
    persist_no_overwrite, validate_origin_ref_binding,
};
use crate::{
    PackError, PreflightedPack, Reader, RecordReader, decode_absent_signature, decode_list,
    exact_array, export_conformance_pack, preflight_conformance_pack, promote_pack_objects,
    read_single_uvar, scb_error,
};

const ID_LEN: usize = 32;
/// Disjoint native exchange envelope magic.
const ENVELOPE_MAGIC: &[u8; 8] = b"SLEYXCH2";
/// Native exchange record version carried by tag 1.
const RECORD_VERSION: u64 = 2;
/// Transport descriptor magic.
const PROFILE_MAGIC: &[u8; 8] = b"SLEYNTP1";
/// Transport descriptor version.
const PROFILE_VERSION: u64 = 1;
/// Canonical receipt chunk size.
pub const NATIVE_CHUNK_BYTES: usize = 65_536;
/// Maximum chunks per receipt (64 MiB of receipt bytes).
pub const MAX_NATIVE_RECEIPT_CHUNKS: usize = 1_024;
/// Maximum digest leaves: pack, head, and transport leaf plus entries.
pub const MAX_NATIVE_EXCHANGE_LEAVES: usize = 8_195;
/// Final digest-tree section binding the transport profile and trust union.
const TRANSPORT_SECTION: u64 = 5;
/// Preflight ceiling on signature checks: 4,096 receipts times 256
/// measurement attestations plus one acceptance statement each.
pub const MAX_NATIVE_SIGNATURE_CHECKS: u64 = 1_052_672;
/// Preflight ceiling on native test visits across all receipts.
pub const MAX_NATIVE_TEST_VISITS: u64 = 1_048_576;
/// Preflight ceiling on decoded evidence bytes across all receipts.
pub const MAX_NATIVE_EVIDENCE_BYTES: u64 = 1_073_741_824;
const TREE_ALGORITHM_TAG: u64 = 1;
const COMPRESSION_NONE: u64 = 0;
/// Native exchange marker directory: separate identity from the v1 marker.
pub const NATIVE_EXCHANGE_VERSION_DIRECTORY: &str = "v2";
/// Fixed descriptor fields in canonical order: `version`,
/// `exchange_record_version`, `chunk_bytes`, `max_exchange_bytes`,
/// `max_receipts`, `max_branches`, `max_signatures`, `max_test_visits`.
const PROFILE_FIELDS: [u64; 8] = [
    PROFILE_VERSION,
    RECORD_VERSION,
    NATIVE_CHUNK_BYTES as u64,
    MAX_EXCHANGE_BYTES as u64,
    MAX_EXCHANGE_RECEIPTS as u64,
    MAX_EXCHANGE_BRANCHES as u64,
    MAX_NATIVE_SIGNATURE_CHECKS,
    MAX_NATIVE_TEST_VISITS,
];

/// Native-only exchange failure code.
///
/// Shared repository-exchange failures (digest, ancestry, branch, target)
/// keep their exact v1 codes and symbols through the wrapped
/// [`ExchangeError`]: the same failure carries the same code. Only failures
/// with no v1 counterpart live here, each mapped to its reserved `29xxx`
/// symbol from `docs/spec/NATIVE_TEST_RESERVATIONS_V1.md`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeExchangeErrorCode {
    /// `NATIVE_TEST_PROFILE_UNSUPPORTED`.
    ProfileMismatch,
    /// `NATIVE_TEST_ENCODING_INVALID`.
    EncodingInvalid,
    /// `NATIVE_TEST_ENFORCER_UNAVAILABLE`.
    ExecutorUnavailable,
    /// `NATIVE_TEST_HISTORICAL_TRUST_UNAVAILABLE`.
    TrustUnavailable,
    /// `NATIVE_TEST_HISTORICAL_TRUST_REJECTED`.
    TrustRejected,
    /// `NATIVE_TEST_REPLAY_INCONCLUSIVE`.
    ReplayInconclusive,
    /// `NATIVE_TEST_RESOURCE_LIMIT`.
    CounterLimit,
    /// `NATIVE_TEST_INTERNAL_INVARIANT`.
    InternalInvariant,
}

impl NativeExchangeErrorCode {
    /// Every native-only code in frozen numeric order.
    pub const ALL: [Self; 8] = [
        Self::ProfileMismatch,
        Self::EncodingInvalid,
        Self::ExecutorUnavailable,
        Self::TrustUnavailable,
        Self::TrustRejected,
        Self::ReplayInconclusive,
        Self::CounterLimit,
        Self::InternalInvariant,
    ];

    /// Returns the exact stable symbol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProfileMismatch => "NATIVE_TEST_PROFILE_UNSUPPORTED",
            Self::EncodingInvalid => "NATIVE_TEST_ENCODING_INVALID",
            Self::ExecutorUnavailable => "NATIVE_TEST_ENFORCER_UNAVAILABLE",
            Self::TrustUnavailable => "NATIVE_TEST_HISTORICAL_TRUST_UNAVAILABLE",
            Self::TrustRejected => "NATIVE_TEST_HISTORICAL_TRUST_REJECTED",
            Self::ReplayInconclusive => "NATIVE_TEST_REPLAY_INCONCLUSIVE",
            Self::CounterLimit => "NATIVE_TEST_RESOURCE_LIMIT",
            Self::InternalInvariant => "NATIVE_TEST_INTERNAL_INVARIANT",
        }
    }

    /// Returns the frozen numeric code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::ProfileMismatch => 29_200,
            Self::EncodingInvalid => 29_201,
            Self::ExecutorUnavailable => 29_212,
            Self::TrustUnavailable => 29_215,
            Self::TrustRejected => 29_216,
            Self::ReplayInconclusive => 29_223,
            Self::CounterLimit => 29_224,
            Self::InternalInvariant => 29_225,
        }
    }
}

/// Exact native exchange failure preserving lower-layer codes.
#[derive(Debug)]
pub enum NativeExchangeError {
    /// Shared repository-exchange failure with its exact v1 code.
    Exchange(ExchangeError),
    /// Exact embedded-pack, object-store, or SCB1 failure.
    Pack(PackError),
    /// Native-only failure with its reserved code.
    Native(NativeExchangeErrorCode),
}

impl NativeExchangeError {
    const fn native(code: NativeExchangeErrorCode) -> Self {
        Self::Native(code)
    }

    const fn exchange(code: ExchangeErrorCode) -> Self {
        Self::Exchange(ExchangeError::Exchange(code))
    }

    /// Returns the exact stable source symbol without collapsing namespaces.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Exchange(error) => error.code(),
            Self::Pack(error) => error.symbol(),
            Self::Native(code) => code.as_str(),
        }
    }

    /// Returns the owning numeric code when one is frozen.
    #[must_use]
    pub fn numeric_code(&self) -> Option<u32> {
        match self {
            Self::Exchange(error) => error.numeric_code(),
            Self::Native(code) => Some(code.numeric()),
            Self::Pack(_) => None,
        }
    }
}

impl fmt::Display for NativeExchangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for NativeExchangeError {}

impl From<ExchangeError> for NativeExchangeError {
    fn from(error: ExchangeError) -> Self {
        Self::Exchange(error)
    }
}

impl From<PackError> for NativeExchangeError {
    fn from(error: PackError) -> Self {
        Self::Pack(error)
    }
}

impl From<CommitError> for NativeExchangeError {
    fn from(error: CommitError) -> Self {
        Self::Exchange(ExchangeError::from(error))
    }
}

impl From<BranchError> for NativeExchangeError {
    fn from(error: BranchError) -> Self {
        Self::Exchange(ExchangeError::from(error))
    }
}

impl From<TransactionCodecError> for NativeExchangeError {
    fn from(error: TransactionCodecError) -> Self {
        Self::Exchange(ExchangeError::from(error))
    }
}

impl From<std::io::Error> for NativeExchangeError {
    fn from(error: std::io::Error) -> Self {
        Self::Exchange(ExchangeError::from(error))
    }
}

type Result<T> = core::result::Result<T, NativeExchangeError>;

fn pack(error: PackError) -> NativeExchangeError {
    NativeExchangeError::Pack(error)
}

fn scb(error: &sley_scb1::ScbError) -> NativeExchangeError {
    pack(scb_error(error))
}

/// The one fixed native exchange transport descriptor (Appendix B).
///
/// There is exactly one valid descriptor, so the constructor derives the
/// profile ID internally and never accepts a caller-chosen value. The ID is
/// the BLAKE3 of the exchange-profile domain over the magic-prefixed
/// canonical record, per the identifier rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeExchangeProfileV1 {
    id: NativeExchangeProfileId,
}

impl NativeExchangeProfileV1 {
    /// Returns the fixed transport descriptor with its derived ID.
    #[must_use]
    pub fn fixed() -> Self {
        Self {
            id: NativeExchangeProfileId::derive(profile_record_bytes()),
        }
    }

    /// Returns the derived transport profile ID.
    #[must_use]
    pub const fn id(&self) -> NativeExchangeProfileId {
        self.id
    }

    /// Checks a wire transport profile ID against the fixed descriptor.
    ///
    /// # Errors
    ///
    /// Returns `NATIVE_TEST_PROFILE_UNSUPPORTED` when the ID is not the
    /// fixed descriptor's.
    pub fn check_id(id: NativeExchangeProfileId) -> Result<()> {
        if id != Self::fixed().id {
            return Err(NativeExchangeError::native(
                NativeExchangeErrorCode::ProfileMismatch,
            ));
        }
        Ok(())
    }
}

/// Returns the exact canonical transport descriptor record:
///
/// `SLEYNTP1` followed by the eight descriptor fields as minimal `uvar`s in
/// Appendix B order.
fn profile_record_bytes() -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 8 * 4);
    out.extend_from_slice(PROFILE_MAGIC);
    for field in PROFILE_FIELDS {
        out.extend_from_slice(&encode_uvar(field));
    }
    out
}

/// Encodes the canonical trust-ID set: `uvar` count followed by the IDs.
///
/// Callers must pass the IDs sorted ascending and deduplicated; the decoder
/// enforces the same shape.
fn encode_trust_id_set(ids: &[[u8; ID_LEN]]) -> Vec<u8> {
    let mut out = encode_uvar(ids.len() as u64);
    for id in ids {
        out.extend_from_slice(id);
    }
    out
}

fn decode_trust_id_set(input: &[u8]) -> Result<Vec<[u8; ID_LEN]>> {
    let mut reader = Reader::new(input);
    let count = reader.read_len(MAX_EXCHANGE_RECEIPTS).map_err(pack)?;
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        ids.push(reader.take_array::<ID_LEN>().map_err(pack)?);
    }
    if !reader.is_finished() {
        return Err(pack(PackError::upstream(
            ScbErrorCode::TrailingBytes.as_str(),
        )));
    }
    let mut previous: Option<[u8; ID_LEN]> = None;
    for id in &ids {
        if let Some(before) = previous {
            if before == *id {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::DuplicateEntry,
                ));
            }
            if before > *id {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::CanonicalOrder,
                ));
            }
        }
        previous = Some(*id);
    }
    Ok(ids)
}

/// Splits exact receipt bytes into canonical transport chunks.
///
/// Every nonfinal chunk is exactly 65,536 bytes, the final chunk holds
/// 1..=65,536 bytes, and at most 1,024 chunks are produced.
///
/// # Errors
///
/// Returns `NATIVE_TEST_ENCODING_INVALID` for empty input or input needing
/// more than 1,024 chunks: such bytes are not transportable.
pub fn chunk_native_receipt(bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    if bytes.is_empty() {
        return Err(NativeExchangeError::native(
            NativeExchangeErrorCode::EncodingInvalid,
        ));
    }
    let count = bytes.len().div_ceil(NATIVE_CHUNK_BYTES);
    if count > MAX_NATIVE_RECEIPT_CHUNKS {
        return Err(NativeExchangeError::native(
            NativeExchangeErrorCode::EncodingInvalid,
        ));
    }
    Ok(bytes
        .chunks(NATIVE_CHUNK_BYTES)
        .map(<[u8]>::to_vec)
        .collect())
}

/// Checks the canonical chunk shape without reassembling bytes.
///
/// # Errors
///
/// Returns `NATIVE_TEST_ENCODING_INVALID` under the exact partition rules
/// of [`assemble_native_receipt`].
pub fn check_native_chunks(chunks: &[Vec<u8>]) -> Result<()> {
    if chunks.is_empty() || chunks.len() > MAX_NATIVE_RECEIPT_CHUNKS {
        return Err(NativeExchangeError::native(
            NativeExchangeErrorCode::EncodingInvalid,
        ));
    }
    let mut nonempty = false;
    for (index, chunk) in chunks.iter().enumerate() {
        let final_chunk = index + 1 == chunks.len();
        if final_chunk {
            if chunk.is_empty() || chunk.len() > NATIVE_CHUNK_BYTES {
                return Err(NativeExchangeError::native(
                    NativeExchangeErrorCode::EncodingInvalid,
                ));
            }
        } else if chunk.len() != NATIVE_CHUNK_BYTES {
            return Err(NativeExchangeError::native(
                NativeExchangeErrorCode::EncodingInvalid,
            ));
        }
        nonempty = nonempty || !chunk.is_empty();
    }
    if !nonempty {
        return Err(NativeExchangeError::native(
            NativeExchangeErrorCode::EncodingInvalid,
        ));
    }
    Ok(())
}

/// Reassembles canonical chunks into the exact stored receipt bytes.
///
/// # Errors
///
/// Returns `NATIVE_TEST_ENCODING_INVALID` under the exact partition rules
/// of [`check_native_chunks`]: no alternative partition and no extra empty
/// chunk is accepted.
pub fn assemble_native_receipt(chunks: &[Vec<u8>]) -> Result<Vec<u8>> {
    check_native_chunks(chunks)?;
    let mut out = Vec::new();
    for chunk in chunks {
        out.extend_from_slice(chunk);
    }
    Ok(out)
}

/// One exchanged receipt as its canonical chunks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeExchangeReceiptEntry {
    /// Declared transaction identity.
    pub transaction_id: TransactionId,
    /// Declared receipt identity.
    pub receipt_id: ReceiptId,
    /// Canonical receipt chunks in order.
    pub chunks: Vec<Vec<u8>>,
}

impl NativeExchangeReceiptEntry {
    /// Reassembles the exact stored receipt bytes these chunks carry.
    ///
    /// # Errors
    ///
    /// Returns `NATIVE_TEST_ENCODING_INVALID` when the chunks are not the
    /// canonical partition.
    pub fn reconstructed(&self) -> Result<Vec<u8>> {
        assemble_native_receipt(&self.chunks)
    }
}

/// Decoded native exchange record: tags 1..10 with tag 1 fixed at 2.
#[derive(Clone, Debug)]
pub struct DecodedNativeExchange {
    /// Exact embedded S20-170 pack bytes.
    pub object_pack: Vec<u8>,
    /// Receipt entries in canonical transaction order.
    pub receipts: Vec<NativeExchangeReceiptEntry>,
    /// The accepted head.
    pub accepted_head: ExchangeHeadEntry,
    /// Branch entries in canonical set order.
    pub branches: Vec<ExchangeBranchEntry>,
    /// Exact sorted trust-ID union referenced by native evidence.
    pub required_trust_policy_ids: Vec<[u8; ID_LEN]>,
    /// Wire transport profile ID (checked against the fixed descriptor).
    pub transport_profile_id: NativeExchangeProfileId,
    /// Digest-tree leaves in canonical order.
    pub leaves: Vec<[u8; ID_LEN]>,
    /// Verified digest-tree root.
    pub digest_tree_root: [u8; ID_LEN],
}

fn encode_receipt_element(entry: &NativeExchangeReceiptEntry) -> Result<Vec<u8>> {
    check_native_chunks(&entry.chunks)?;
    let chunk_elements = entry
        .chunks
        .iter()
        .map(|chunk| encode_bytes(chunk))
        .collect::<Vec<_>>();
    encode_record(&[
        (1, entry.transaction_id.as_bytes().to_vec()),
        (2, entry.receipt_id.as_bytes().to_vec()),
        (
            3,
            encode_list(&chunk_elements).map_err(|error| scb(&error))?,
        ),
    ])
    .map_err(|error| scb(&error))
}

fn decode_receipt_entries(input: &[u8]) -> Result<Vec<NativeExchangeReceiptEntry>> {
    let elements = decode_list(input, MAX_EXCHANGE_RECEIPTS).map_err(pack)?;
    let mut entries = Vec::with_capacity(elements.len());
    for element in elements {
        let mut record = RecordReader::new(element).map_err(pack)?;
        let transaction_id = TransactionId::from_bytes(
            exact_array(record.required(1).map_err(pack)?).map_err(pack)?,
        );
        let receipt_id =
            ReceiptId::from_bytes(exact_array(record.required(2).map_err(pack)?).map_err(pack)?);
        let chunk_slices =
            decode_list(record.required(3).map_err(pack)?, MAX_NATIVE_RECEIPT_CHUNKS)
                .map_err(pack)?;
        record.finish().map_err(pack)?;
        let mut chunks = Vec::with_capacity(chunk_slices.len());
        for element in chunk_slices {
            let mut reader = Reader::new(element);
            let len = reader.read_len(NATIVE_CHUNK_BYTES).map_err(pack)?;
            let chunk = reader.take_exact(len).map_err(pack)?.to_vec();
            if !reader.is_finished() {
                return Err(pack(PackError::upstream(
                    ScbErrorCode::TrailingBytes.as_str(),
                )));
            }
            chunks.push(chunk);
        }
        let entry = NativeExchangeReceiptEntry {
            transaction_id,
            receipt_id,
            chunks,
        };
        // The canonical partition is enforced at decode: only canonical
        // chunks ever enter the record.
        entry.reconstructed()?;
        entries.push(entry);
    }
    let mut previous: Option<TransactionId> = None;
    for entry in &entries {
        if let Some(before) = previous {
            if before == entry.transaction_id {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::DuplicateEntry,
                ));
            }
            if before.as_bytes() > entry.transaction_id.as_bytes() {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::CanonicalOrder,
                ));
            }
        }
        previous = Some(entry.transaction_id);
    }
    Ok(entries)
}

fn decode_head_entry(input: &[u8]) -> Result<ExchangeHeadEntry> {
    let mut record = RecordReader::new(input).map_err(pack)?;
    let transaction_id =
        TransactionId::from_bytes(exact_array(record.required(1).map_err(pack)?).map_err(pack)?);
    let receipt_id =
        ReceiptId::from_bytes(exact_array(record.required(2).map_err(pack)?).map_err(pack)?);
    record.finish().map_err(pack)?;
    Ok(ExchangeHeadEntry {
        transaction_id,
        receipt_id,
    })
}

fn decode_branch_entries(input: &[u8]) -> Result<Vec<ExchangeBranchEntry>> {
    let elements = decode_list(input, MAX_EXCHANGE_BRANCHES).map_err(pack)?;
    let mut entries = Vec::with_capacity(elements.len());
    let mut previous: Option<&[u8]> = None;
    for element in elements {
        if let Some(before) = previous {
            if before == element {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::DuplicateEntry,
                ));
            }
            if before > element {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::CanonicalOrder,
                ));
            }
        }
        previous = Some(element);
        let mut record = RecordReader::new(element).map_err(pack)?;
        let branch_name = record.required(1).map_err(pack)?.to_vec();
        let stored_origin = record.required(2).map_err(pack)?.to_vec();
        let stored_ref = record.required(3).map_err(pack)?.to_vec();
        record.finish().map_err(pack)?;
        entries.push(ExchangeBranchEntry {
            branch_name,
            stored_origin,
            stored_ref,
        });
    }
    Ok(entries)
}

fn decode_tree(input: &[u8]) -> Result<(Vec<[u8; ID_LEN]>, [u8; ID_LEN])> {
    let mut record = RecordReader::new(input).map_err(pack)?;
    if read_single_uvar(record.required(1).map_err(pack)?).map_err(pack)? != TREE_ALGORITHM_TAG {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::DigestTreeMismatch,
        ));
    }
    let count = usize::try_from(read_single_uvar(record.required(2).map_err(pack)?).map_err(pack)?)
        .map_err(|_| pack(PackError::upstream(ScbErrorCode::LengthOverflow.as_str())))?;
    let leaf_slices = decode_list(
        record.required(3).map_err(pack)?,
        MAX_NATIVE_EXCHANGE_LEAVES,
    )
    .map_err(pack)?;
    let root = exact_array(record.required(4).map_err(pack)?).map_err(pack)?;
    record.finish().map_err(pack)?;
    // Pack, head, and transport leaves are the structural minimum: an
    // exchange with no receipt is refused as open ancestry, never as a
    // well-formed empty tree.
    if count != leaf_slices.len() || leaf_slices.len() < 4 {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::DigestTreeMismatch,
        ));
    }
    let mut leaves = Vec::with_capacity(leaf_slices.len());
    for slice in leaf_slices {
        leaves.push(exact_array(slice).map_err(pack)?);
    }
    Ok((leaves, root))
}

/// Computes the canonical digest-tree leaves for a native exchange.
///
/// Sections 1..4 and the node construction reuse the exact v1 exchange
/// build: receipt leaves cover the reconstructed receipt bytes. The final
/// section-5 leaf binds the transport profile ID to the canonical trust-ID
/// set. Leaf order is pack, receipts, branches, head, transport.
///
/// # Errors
///
/// Returns `EXCHANGE_RESOURCE_LIMIT` when the leaf count exceeds 8,195, and
/// `NATIVE_TEST_ENCODING_INVALID` when any receipt chunks are not the
/// canonical partition.
pub fn compute_native_leaves(
    pack_id: sley_id::RepositoryPackId,
    object_pack: &[u8],
    receipts: &[NativeExchangeReceiptEntry],
    accepted_head: ExchangeHeadEntry,
    branches: &[ExchangeBranchEntry],
    transport_profile_id: NativeExchangeProfileId,
    required_trust_policy_ids: &[[u8; ID_LEN]],
) -> Result<Vec<[u8; ID_LEN]>> {
    let count = receipts
        .len()
        .checked_add(branches.len())
        .and_then(|count| count.checked_add(3))
        .ok_or_else(|| NativeExchangeError::exchange(ExchangeErrorCode::ResourceLimit))?;
    if count > MAX_NATIVE_EXCHANGE_LEAVES {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::ResourceLimit,
        ));
    }
    let mut leaves = Vec::with_capacity(count);
    leaves.push(content_leaf(PACK_SECTION, pack_id.as_bytes(), object_pack));
    for entry in receipts {
        leaves.push(content_leaf(
            RECEIPT_SECTION,
            entry.transaction_id.as_bytes(),
            &entry.reconstructed()?,
        ));
    }
    for entry in branches {
        let mut bytes = encode_bytes(&entry.stored_origin);
        bytes.extend_from_slice(&encode_bytes(&entry.stored_ref));
        leaves.push(content_leaf(
            BRANCH_SECTION,
            &branch_name_key(entry).map_err(NativeExchangeError::from)?,
            &bytes,
        ));
    }
    let mut head_bytes = stored_head_bytes(accepted_head.transaction_id);
    head_bytes.extend_from_slice(accepted_head.receipt_id.as_bytes());
    leaves.push(content_leaf(
        HEAD_SECTION,
        accepted_head.transaction_id.as_bytes(),
        &head_bytes,
    ));
    leaves.push(content_leaf(
        TRANSPORT_SECTION,
        transport_profile_id.as_bytes(),
        &encode_trust_id_set(required_trust_policy_ids),
    ));
    Ok(leaves)
}

/// Encodes the native exchange record: tags 1..10 with tag 1 fixed at 2.
///
/// # Errors
///
/// Returns `EXCHANGE_ANCESTRY_OPEN` for an empty receipt history,
/// `EXCHANGE_RESOURCE_LIMIT` for over-limit entry counts, and
/// `NATIVE_TEST_ENCODING_INVALID` for noncanonical receipt chunks.
#[allow(clippy::too_many_arguments)]
pub fn encode_native_payload(
    object_pack: &[u8],
    receipts: &[NativeExchangeReceiptEntry],
    accepted_head: ExchangeHeadEntry,
    branches: &[ExchangeBranchEntry],
    leaves: &[[u8; ID_LEN]],
    tree_root: [u8; ID_LEN],
    required_trust_policy_ids: &[[u8; ID_LEN]],
    transport_profile_id: NativeExchangeProfileId,
) -> Result<Vec<u8>> {
    if receipts.is_empty() {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::AncestryOpen,
        ));
    }
    if receipts.len() > MAX_EXCHANGE_RECEIPTS {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::ResourceLimit,
        ));
    }
    if branches.len() > MAX_EXCHANGE_BRANCHES {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::ResourceLimit,
        ));
    }
    let receipt_elements = receipts
        .iter()
        .map(encode_receipt_element)
        .collect::<Result<Vec<_>>>()?;
    let head = encode_record(&[
        (1, accepted_head.transaction_id.as_bytes().to_vec()),
        (2, accepted_head.receipt_id.as_bytes().to_vec()),
    ])
    .map_err(|error| scb(&error))?;
    let branch_elements = branches
        .iter()
        .map(encode_branch_element)
        .collect::<core::result::Result<Vec<_>, _>>()
        .map_err(NativeExchangeError::from)?;
    let leaf_elements = leaves.iter().map(|leaf| leaf.to_vec()).collect::<Vec<_>>();
    let tree = encode_record(&[
        (1, encode_uvar(TREE_ALGORITHM_TAG)),
        (2, encode_uvar(leaves.len() as u64)),
        (3, encode_list(&leaf_elements).map_err(|error| scb(&error))?),
        (4, tree_root.to_vec()),
    ])
    .map_err(|error| scb(&error))?;
    let signature = encode_union(0, &[]).map_err(|error| scb(&error))?;
    encode_record(&[
        (1, encode_uvar(RECORD_VERSION)),
        (2, object_pack.to_vec()),
        (
            3,
            encode_list(&receipt_elements).map_err(|error| scb(&error))?,
        ),
        (4, head),
        (
            5,
            encode_list(&branch_elements).map_err(|error| scb(&error))?,
        ),
        (6, encode_uvar(COMPRESSION_NONE)),
        (7, tree),
        (8, signature),
        (9, encode_trust_id_set(required_trust_policy_ids)),
        (10, transport_profile_id.as_bytes().to_vec()),
    ])
    .map_err(|error| scb(&error))
}

/// Decodes the native exchange record with every structural rule.
///
/// Tag 1 must be 2, receipt elements must be canonically ordered with
/// canonical chunks, the trust set must be strictly sorted, and the leaf
/// count must be exactly 3 plus the receipt and branch counts. Receipt or
/// tree bytes are not verified against objects here; that is preflight.
///
/// # Errors
///
/// Returns the precise structural refusal: `EXCHANGE_VERSION_UNSUPPORTED`,
/// `EXCHANGE_ANCESTRY_OPEN`, `EXCHANGE_CANONICAL_ORDER`,
/// `EXCHANGE_DUPLICATE_ENTRY`, `NATIVE_TEST_ENCODING_INVALID`,
/// `EXCHANGE_DIGEST_TREE_MISMATCH`, `EXCHANGE_COMPRESSION_UNSUPPORTED`,
/// `EXCHANGE_PROFILE_UNSUPPORTED`, or `EXCHANGE_RESOURCE_LIMIT`.
pub fn decode_native_payload(input: &[u8]) -> Result<DecodedNativeExchange> {
    let mut record = RecordReader::new(input).map_err(pack)?;
    if read_single_uvar(record.required(1).map_err(pack)?).map_err(pack)? != RECORD_VERSION {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::VersionUnsupported,
        ));
    }
    let object_pack = record.required(2).map_err(pack)?.to_vec();
    let receipts = decode_receipt_entries(record.required(3).map_err(pack)?)?;
    if receipts.is_empty() {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::AncestryOpen,
        ));
    }
    let accepted_head = decode_head_entry(record.required(4).map_err(pack)?)?;
    let branches = decode_branch_entries(record.required(5).map_err(pack)?)?;
    if read_single_uvar(record.required(6).map_err(pack)?).map_err(pack)? != COMPRESSION_NONE {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::CompressionUnsupported,
        ));
    }
    let (leaves, digest_tree_root) = decode_tree(record.required(7).map_err(pack)?)?;
    decode_absent_signature(record.required(8).map_err(pack)?).map_err(|error| {
        if error.symbol() == crate::PackErrorCode::ProfileUnsupported.as_str() {
            NativeExchangeError::exchange(ExchangeErrorCode::ProfileUnsupported)
        } else {
            pack(error)
        }
    })?;
    let required_trust_policy_ids = decode_trust_id_set(record.required(9).map_err(pack)?)?;
    let transport_profile_id = NativeExchangeProfileId::from_bytes(
        exact_array(record.required(10).map_err(pack)?).map_err(pack)?,
    );
    record.finish().map_err(pack)?;
    if leaves.len() != receipts.len() + branches.len() + 3 {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::DigestTreeMismatch,
        ));
    }
    let mut expanded = object_pack.len();
    for entry in &receipts {
        expanded = expanded
            .checked_add(entry.reconstructed()?.len())
            .ok_or_else(|| NativeExchangeError::exchange(ExchangeErrorCode::ResourceLimit))?;
    }
    for entry in &branches {
        expanded = expanded
            .checked_add(entry.stored_origin.len())
            .and_then(|total| total.checked_add(entry.stored_ref.len()))
            .ok_or_else(|| NativeExchangeError::exchange(ExchangeErrorCode::ResourceLimit))?;
    }
    if expanded > MAX_EXCHANGE_ALLOCATION {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::ResourceLimit,
        ));
    }
    Ok(DecodedNativeExchange {
        object_pack,
        receipts,
        accepted_head,
        branches,
        required_trust_policy_ids,
        transport_profile_id,
        leaves,
        digest_tree_root,
    })
}

/// Encodes the disjoint native exchange envelope.
///
/// Layout: `SLEYXCH2`, record version 2, transport profile ID, payload
/// length, payload, and the exchange-ID trailer. The trailer binds every
/// preceding byte; the profile ID is additionally checked against the fixed
/// descriptor at decode.
///
/// # Errors
///
/// Returns `NATIVE_TEST_PROFILE_UNSUPPORTED` for a non-fixed transport
/// profile ID and `EXCHANGE_RESOURCE_LIMIT` for an oversized envelope.
pub fn encode_native_envelope(
    payload: &[u8],
    transport_profile_id: NativeExchangeProfileId,
) -> Result<(Vec<u8>, RepositoryExchangeId)> {
    NativeExchangeProfileV1::check_id(transport_profile_id)?;
    let mut preimage = Vec::with_capacity(payload.len() + 8 + 8 + ID_LEN + 8);
    preimage.extend_from_slice(ENVELOPE_MAGIC);
    preimage.extend_from_slice(&encode_uvar(RECORD_VERSION));
    preimage.extend_from_slice(transport_profile_id.as_bytes());
    preimage.extend_from_slice(&encode_uvar(payload.len() as u64));
    preimage.extend_from_slice(payload);
    let exchange_id = RepositoryExchangeId::derive(&preimage);
    preimage.extend_from_slice(exchange_id.as_bytes());
    if preimage.len() > MAX_EXCHANGE_BYTES {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::ResourceLimit,
        ));
    }
    Ok((preimage, exchange_id))
}

/// Decodes the native exchange envelope without trusting the payload.
///
/// Returns the exchange ID, the wire transport profile ID, and the raw
/// record payload. Old `SLEYSCB1` bytes refuse on the magic before any
/// other work.
///
/// # Errors
///
/// Returns `EXCHANGE_RESOURCE_LIMIT` for an oversized envelope,
/// `EXCHANGE_VERSION_UNSUPPORTED` for a non-2 version,
/// `EXCHANGE_DIGEST_MISMATCH` for a trailer that does not bind the bytes,
/// and `NATIVE_TEST_PROFILE_UNSUPPORTED` for a non-fixed profile ID.
pub fn decode_native_envelope(
    input: &[u8],
) -> Result<(RepositoryExchangeId, NativeExchangeProfileId, Vec<u8>)> {
    if input.len() > MAX_EXCHANGE_BYTES {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::ResourceLimit,
        ));
    }
    let mut reader = Reader::new(input);
    if reader.take_exact(ENVELOPE_MAGIC.len()).map_err(pack)? != ENVELOPE_MAGIC {
        return Err(pack(PackError::upstream(
            ScbErrorCode::MagicInvalid.as_str(),
        )));
    }
    if reader.read_uvar().map_err(pack)? != RECORD_VERSION {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::VersionUnsupported,
        ));
    }
    let transport_profile_id =
        NativeExchangeProfileId::from_bytes(reader.take_array::<ID_LEN>().map_err(pack)?);
    let payload_len = reader.read_len(MAX_EXCHANGE_BYTES).map_err(pack)?;
    let payload = reader.take_exact(payload_len).map_err(pack)?.to_vec();
    let trailer = reader.take_array::<ID_LEN>().map_err(pack)?;
    if !reader.is_finished() {
        return Err(pack(PackError::upstream(
            ScbErrorCode::TrailingBytes.as_str(),
        )));
    }
    let exchange_id = RepositoryExchangeId::derive(&input[..input.len() - ID_LEN]);
    if trailer != *exchange_id.as_bytes() {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::DigestMismatch,
        ));
    }
    NativeExchangeProfileV1::check_id(transport_profile_id)?;
    Ok((exchange_id, transport_profile_id, payload))
}

/// Walks the mixed-history closure from the accepted head and every
/// visible branch head, verifying each revision of either receipt format
/// before trusting its bytes.
///
/// # Errors
///
/// Returns the first revision verification failure or
/// `EXCHANGE_RESOURCE_LIMIT` for a closure beyond 4,096 receipts.
fn collect_native_receipts(
    transactions: &TransactionRepository,
    maintenance: &RepositoryMaintenanceGuard,
    head: &ImportedReceipt,
    visible: &[ResolvedBranch],
) -> Result<BTreeMap<TransactionId, ImportedReceipt>> {
    let mut receipts: BTreeMap<TransactionId, ImportedReceipt> = BTreeMap::new();
    let mut pending: Vec<TransactionId> = vec![head.transaction_id()];
    pending.extend(
        visible
            .iter()
            .map(|branch| branch.reference.record.head_transaction_id),
    );
    while let Some(transaction_id) = pending.pop() {
        if receipts.contains_key(&transaction_id) {
            continue;
        }
        if receipts.len() >= MAX_EXCHANGE_RECEIPTS {
            return Err(NativeExchangeError::exchange(
                ExchangeErrorCode::ResourceLimit,
            ));
        }
        let revision =
            transactions.verified_revision_any_with_maintenance(maintenance, transaction_id)?;
        pending.extend(revision.parent_transaction_ids().iter().copied());
        receipts.insert(transaction_id, revision);
    }
    Ok(receipts)
}

/// Canonical accepted native repository exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedNativeExchange {
    /// Derived exchange identifier.
    pub exchange_id: RepositoryExchangeId,
    /// Exact standalone exchange bytes.
    pub stored_bytes: Vec<u8>,
    /// Identifier of the embedded S20-170 pack.
    pub pack_id: sley_id::RepositoryPackId,
    /// Exact embedded S20-170 pack bytes.
    pub object_pack: Vec<u8>,
    /// Exact ordered chunked receipt entries.
    pub receipts: Vec<NativeExchangeReceiptEntry>,
    /// The accepted head.
    pub accepted_head: ExchangeHeadEntry,
    /// Exact ordered branch entries.
    pub branches: Vec<ExchangeBranchEntry>,
    /// Exact sorted trust-ID union referenced by native evidence.
    pub required_trust_policy_ids: Vec<[u8; ID_LEN]>,
    /// Verified digest-tree root.
    pub digest_tree_root: [u8; ID_LEN],
}

/// Exports the complete mixed-history repository as a native exchange.
///
/// The walk holds shared repository maintenance and verifies every
/// revision of either receipt format before trusting its bytes: format 1
/// through the frozen v1 loader, native receipts through the native loader
/// with its evidence, relationship, object, inventory, and pin checks.
/// Receipt bytes are exact stored bytes (reconstructed identically from
/// canonical chunks at decode), branch records are exact stored bytes, and
/// the trust union collects every native receipt's acceptance and
/// measurement manifests without installing any receiver trust.
///
/// # Errors
///
/// Returns the first maintenance, verification, pack, chunking, or
/// encoding failure; `EXCHANGE_RESOURCE_LIMIT` for over-limit closures,
/// packs, or envelopes.
pub fn export_native_exchange<V: CanonicalVerifier>(
    root: &Path,
    verifier: &V,
) -> Result<AcceptedNativeExchange> {
    let maintenance = acquire_shared_repository_maintenance(root)?;
    let transactions = TransactionRepository::new(root);
    let head = transactions.accepted_head_any_with_maintenance(&maintenance)?;
    let branch_repository = BranchRepository::new(root);
    let visible = {
        let _refs_lock = branch_repository.acquire_refs_lock()?;
        branch_repository.list_branches_locked(&maintenance, MAX_BRANCHES)?
    };
    if visible.len() > MAX_EXCHANGE_BRANCHES {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::ResourceLimit,
        ));
    }

    let receipts = collect_native_receipts(&transactions, &maintenance, &head, &visible)?;

    let roots = receipts
        .values()
        .map(|receipt| receipt.state_root().clone())
        .collect::<Vec<sley_state_root::AcceptedStateRoot>>();
    let store = ObjectStore::new(root);
    let pack = export_conformance_pack(&store, &roots, verifier)?;
    if pack.stored_bytes.len() > MAX_EMBEDDED_PACK_BYTES {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::ResourceLimit,
        ));
    }

    let mut trust_ids = BTreeSet::new();
    let mut receipt_entries = Vec::with_capacity(receipts.len());
    for (transaction_id, receipt) in &receipts {
        if let ImportedReceipt::V2(native) = receipt {
            trust_ids.extend(native_receipt_trust_policy_ids(native)?);
        }
        receipt_entries.push(NativeExchangeReceiptEntry {
            transaction_id: *transaction_id,
            receipt_id: receipt.receipt_id(),
            chunks: chunk_native_receipt(receipt.stored_bytes())?,
        });
    }
    let required_trust_policy_ids = trust_ids.into_iter().collect::<Vec<_>>();
    let accepted_head = ExchangeHeadEntry {
        transaction_id: head.transaction_id(),
        receipt_id: head.receipt_id(),
    };
    let mut branch_entries = visible
        .iter()
        .map(|branch| ExchangeBranchEntry {
            branch_name: branch.origin.record.branch_name.as_bytes().to_vec(),
            stored_origin: branch.origin.stored_bytes.clone(),
            stored_ref: branch.reference.stored_bytes.clone(),
        })
        .collect::<Vec<_>>();
    let mut keyed = branch_entries
        .drain(..)
        .map(|entry| encode_branch_element(&entry).map(|encoded| (encoded, entry)))
        .collect::<core::result::Result<Vec<_>, ExchangeError>>()
        .map_err(NativeExchangeError::from)?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    keyed.dedup_by(|left, right| left.0 == right.0);
    let branch_entries = keyed
        .into_iter()
        .map(|(_, entry)| entry)
        .collect::<Vec<_>>();
    drop(maintenance);
    let profile = NativeExchangeProfileV1::fixed();
    let leaves = compute_native_leaves(
        pack.pack_id,
        &pack.stored_bytes,
        &receipt_entries,
        accepted_head,
        &branch_entries,
        profile.id(),
        &required_trust_policy_ids,
    )?;
    let digest_tree_root =
        merkle_root(&leaves, MAX_NATIVE_EXCHANGE_LEAVES).map_err(NativeExchangeError::from)?;
    let payload = encode_native_payload(
        &pack.stored_bytes,
        &receipt_entries,
        accepted_head,
        &branch_entries,
        &leaves,
        digest_tree_root,
        &required_trust_policy_ids,
        profile.id(),
    )?;
    let (stored_bytes, exchange_id) = encode_native_envelope(&payload, profile.id())?;
    Ok(AcceptedNativeExchange {
        exchange_id,
        stored_bytes,
        pack_id: pack.pack_id,
        object_pack: pack.stored_bytes,
        receipts: receipt_entries,
        accepted_head,
        branches: branch_entries,
        required_trust_policy_ids,
        digest_tree_root,
    })
}

/// Caller-supplied trust manifests for native import preflight.
///
/// The manifests are read-only resolution input: every trust ID the
/// exchange references must resolve to exactly one supplied manifest, and
/// import never installs receiver trust.
#[derive(Clone, Copy, Debug)]
pub struct NativeExchangeTrust<'a> {
    /// Trust policy manifests the receiver already holds.
    pub manifests: &'a [HistoricalTrustPolicyV1],
}

/// Facts proven by a complete native preflight without any write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeExchangePreflightReport {
    /// Exchange identifier bound by the trailer.
    pub exchange_id: RepositoryExchangeId,
    /// Identifier of the embedded S20-170 pack.
    pub pack_id: sley_id::RepositoryPackId,
    /// Accepted head named by the exchange.
    pub accepted_head: ExchangeHeadEntry,
    /// Receipt entries.
    pub receipts: usize,
    /// Visible branch entries.
    pub branches: usize,
    /// Digest leaves.
    pub leaves: usize,
    /// Signature checks charged (measurements plus one acceptance each).
    pub signatures_checked: u64,
    /// Native test envelopes visited (executions plus measurements).
    pub test_visits: u64,
    /// Decoded native evidence bytes charged.
    pub evidence_bytes: u64,
}

/// Successful native clone report.
#[derive(Debug)]
pub struct NativeExchangeImportReport {
    /// Imported exchange identifier.
    pub exchange_id: RepositoryExchangeId,
    /// Reconstructed accepted head identities.
    pub accepted_head: ExchangeHeadEntry,
    /// Receipts durable after the import.
    pub receipts: usize,
    /// Visible branches installed.
    pub branches: usize,
    /// Newly promoted object count.
    pub promoted_objects: usize,
    /// Already-present verified object count.
    pub present_objects: usize,
}

struct NativeVerifiedBranch {
    name: BranchName,
    origin: ImportedBranchRecord,
    reference: ImportedBranchRef,
}

struct NativePreflight {
    exchange_id: RepositoryExchangeId,
    pack: PreflightedPack,
    receipts: BTreeMap<TransactionId, ImportedReceipt>,
    branches: Vec<NativeVerifiedBranch>,
    accepted_head: ExchangeHeadEntry,
    leaves: usize,
    counters: NativePreflightCounters,
}

/// Appendix B preflight counters: every counter fails before exceeding its
/// ceiling, never after. The v1 receipt/object/binding ceilings keep their
/// exact v1 codes; only the three native evidence counters report the
/// native resource limit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct NativePreflightCounters {
    signatures: u64,
    visits: u64,
    evidence_bytes: u64,
}

impl NativePreflightCounters {
    fn add_signatures(&mut self, count: u64) -> Result<()> {
        self.signatures = self
            .signatures
            .checked_add(count)
            .filter(|total| *total <= MAX_NATIVE_SIGNATURE_CHECKS)
            .ok_or_else(|| NativeExchangeError::native(NativeExchangeErrorCode::CounterLimit))?;
        Ok(())
    }

    fn add_visits(&mut self, count: u64) -> Result<()> {
        self.visits = self
            .visits
            .checked_add(count)
            .filter(|total| *total <= MAX_NATIVE_TEST_VISITS)
            .ok_or_else(|| NativeExchangeError::native(NativeExchangeErrorCode::CounterLimit))?;
        Ok(())
    }

    fn add_evidence_bytes(&mut self, count: u64) -> Result<()> {
        self.evidence_bytes = self
            .evidence_bytes
            .checked_add(count)
            .filter(|total| *total <= MAX_NATIVE_EVIDENCE_BYTES)
            .ok_or_else(|| NativeExchangeError::native(NativeExchangeErrorCode::CounterLimit))?;
        Ok(())
    }
}

fn native_topological_order(
    receipts: &BTreeMap<TransactionId, ImportedReceipt>,
) -> Result<Vec<TransactionId>> {
    let mut order = Vec::with_capacity(receipts.len());
    let mut placed: BTreeSet<TransactionId> = BTreeSet::new();
    let mut pending: Vec<TransactionId> = receipts.keys().copied().collect();
    while !pending.is_empty() {
        let mut remaining = Vec::with_capacity(pending.len());
        let mut progressed = false;
        for transaction_id in pending {
            let ready = receipts[&transaction_id]
                .parent_transaction_ids()
                .iter()
                .all(|parent| placed.contains(parent));
            if ready {
                placed.insert(transaction_id);
                order.push(transaction_id);
                progressed = true;
            } else {
                remaining.push(transaction_id);
            }
        }
        pending = remaining;
        if !progressed && !pending.is_empty() {
            return Err(NativeExchangeError::exchange(
                ExchangeErrorCode::AncestryCycle,
            ));
        }
    }
    Ok(order)
}

fn native_parent_roots(receipt: &ImportedReceipt) -> &[StateRoot] {
    match receipt {
        ImportedReceipt::V1(inner) => &inner.transaction.record.parent_roots,
        ImportedReceipt::V2(inner) => &inner.transaction.record.parent_roots,
    }
}

fn native_verify_closure_rules(
    decoded: &DecodedNativeExchange,
    receipts: &BTreeMap<TransactionId, ImportedReceipt>,
    pack_roots: &[sley_state_root::AcceptedStateRoot],
) -> Result<Vec<TransactionId>> {
    let mut genesis = 0_usize;
    let mut workspace: Option<WorkspaceId> = None;
    for receipt in receipts.values() {
        for parent in receipt.parent_transaction_ids() {
            if !receipts.contains_key(parent) {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::AncestryOpen,
                ));
            }
        }
        match receipt.transaction_kind() {
            TransactionKind::TrustedGenesis => {
                if !receipt.parent_transaction_ids().is_empty() {
                    return Err(NativeExchangeError::exchange(
                        ExchangeErrorCode::AncestryOpen,
                    ));
                }
                genesis += 1;
            }
            TransactionKind::OrdinaryCandidate => {
                // No v2 genesis is a wire state, so every native receipt
                // carries exactly one parent like an ordinary v1 receipt.
                if receipt.parent_transaction_ids().len() != 1
                    || native_parent_roots(receipt).len() != 1
                {
                    return Err(NativeExchangeError::exchange(
                        ExchangeErrorCode::AncestryOpen,
                    ));
                }
            }
        }
        match workspace {
            None => workspace = Some(receipt.workspace_id()),
            Some(expected) if expected == receipt.workspace_id() => {}
            Some(_) => {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::WorkspaceMismatch,
                ));
            }
        }
    }
    if genesis != 1 {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::AncestryOpen,
        ));
    }
    let order = native_topological_order(receipts)?;

    let head = decoded.accepted_head;
    match receipts.get(&head.transaction_id) {
        Some(receipt) if receipt.receipt_id() == head.receipt_id => {}
        _ => {
            return Err(NativeExchangeError::exchange(
                ExchangeErrorCode::HeadInvalid,
            ));
        }
    }

    let committed: BTreeSet<StateRoot> = receipts
        .values()
        .map(ImportedReceipt::committed_root)
        .collect();
    let by_root: BTreeMap<StateRoot, &sley_state_root::AcceptedStateRoot> =
        pack_roots.iter().map(|root| (root.root, root)).collect();
    let mut closure = committed;
    loop {
        let mut added = false;
        for root in closure.clone() {
            if let Some(accepted) = by_root.get(&root) {
                for dependency in &accepted.record.dependency_roots {
                    if closure.insert(*dependency) {
                        added = true;
                    }
                }
            }
        }
        if !added {
            break;
        }
    }
    let pack_set: BTreeSet<StateRoot> = by_root.keys().copied().collect();
    if pack_set != closure {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::RootClosure,
        ));
    }
    Ok(order)
}

fn native_fast_forward_reachable(
    receipts: &BTreeMap<TransactionId, ImportedReceipt>,
    origin: TransactionId,
    head: TransactionId,
) -> bool {
    let mut cursor = Some(head);
    let mut visited = 0_usize;
    while let Some(transaction_id) = cursor {
        if transaction_id == origin {
            return true;
        }
        visited += 1;
        if visited > MAX_EXCHANGE_RECEIPTS {
            return false;
        }
        cursor = receipts
            .get(&transaction_id)
            .and_then(|receipt| receipt.parent_transaction_ids().first().copied());
    }
    false
}

fn native_verify_branch_entry(
    entry: &ExchangeBranchEntry,
    receipts: &BTreeMap<TransactionId, ImportedReceipt>,
    workspace: WorkspaceId,
) -> Result<NativeVerifiedBranch> {
    let name = BranchName::parse(&entry.branch_name)
        .map_err(|_| NativeExchangeError::exchange(ExchangeErrorCode::BranchInvalid))?;
    let origin = import_branch_record(&entry.stored_origin).map_err(NativeExchangeError::from)?;
    let reference = import_branch_ref(&entry.stored_ref).map_err(NativeExchangeError::from)?;
    if origin.record.branch_name != name || reference.record.branch_name != name {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::BranchInvalid,
        ));
    }
    validate_origin_ref_binding(&origin, &reference)
        .map_err(|_| NativeExchangeError::exchange(ExchangeErrorCode::BranchInvalid))?;
    let origin_receipt = receipts
        .get(&origin.record.origin_transaction_id)
        .ok_or_else(|| NativeExchangeError::exchange(ExchangeErrorCode::BranchInvalid))?;
    let head_receipt = receipts
        .get(&reference.record.head_transaction_id)
        .ok_or_else(|| NativeExchangeError::exchange(ExchangeErrorCode::BranchInvalid))?;
    if origin.record.workspace_id != workspace || reference.record.workspace_id != workspace {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::WorkspaceMismatch,
        ));
    }
    let origin_facts_match = origin.record.origin_state_root == origin_receipt.committed_root()
        && origin.record.schema_epoch_id == origin_receipt.state_root().record.schema_epoch_id
        && origin.record.policy_root_id == origin_receipt.policy_root().root()
        && origin.record.dependency_roots == origin_receipt.state_root().record.dependency_roots;
    let ref_facts_match = reference.record.head_state_root == head_receipt.committed_root()
        && reference.record.schema_epoch_id == head_receipt.state_root().record.schema_epoch_id
        && reference.record.policy_root_id == head_receipt.policy_root().root()
        && reference.record.dependency_roots == head_receipt.state_root().record.dependency_roots;
    if !origin_facts_match || !ref_facts_match {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::BranchInvalid,
        ));
    }
    if !native_fast_forward_reachable(
        receipts,
        origin.record.origin_transaction_id,
        reference.record.head_transaction_id,
    ) {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::BranchNotFastForward,
        ));
    }
    Ok(NativeVerifiedBranch {
        name,
        origin,
        reference,
    })
}

fn native_verify_no_surplus(
    receipts: &BTreeMap<TransactionId, ImportedReceipt>,
    head: TransactionId,
    branches: &[NativeVerifiedBranch],
) -> Result<()> {
    let mut reachable: BTreeSet<TransactionId> = BTreeSet::new();
    let mut pending = vec![head];
    for branch in branches {
        pending.push(branch.reference.record.head_transaction_id);
        pending.push(branch.origin.record.origin_transaction_id);
    }
    while let Some(transaction_id) = pending.pop() {
        if !reachable.insert(transaction_id) {
            continue;
        }
        if let Some(receipt) = receipts.get(&transaction_id) {
            pending.extend(receipt.parent_transaction_ids().iter().copied());
        }
    }
    if reachable.len() != receipts.len() {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::AncestrySurplus,
        ));
    }
    Ok(())
}

fn map_trust_error(error: NativeCommitError) -> NativeExchangeError {
    match error {
        NativeCommitError::TrustUnavailable => {
            NativeExchangeError::native(NativeExchangeErrorCode::TrustUnavailable)
        }
        NativeCommitError::TrustRejected => {
            NativeExchangeError::native(NativeExchangeErrorCode::TrustRejected)
        }
        _ => NativeExchangeError::native(NativeExchangeErrorCode::InternalInvariant),
    }
}

fn resolve_trust_manifest<'a>(
    id: &[u8; 32],
    trust: &'a NativeExchangeTrust<'a>,
) -> Result<&'a HistoricalTrustPolicyV1> {
    // The manifest ID binds the full manifest record (nonce plus entries),
    // so same-ID manifests are interchangeable and the first match wins
    // deterministically.
    trust
        .manifests
        .iter()
        .find(|manifest| manifest.id().as_bytes() == id)
        .ok_or_else(|| NativeExchangeError::native(NativeExchangeErrorCode::TrustUnavailable))
}

/// Grant checks proven for one native receipt: every statement and
/// attestation carries its role, scope, profile, and interval grants from
/// the resolved caller-supplied manifests.
struct TrustGrantCounts {
    signatures: u64,
    visits: u64,
}

/// Checks one native receipt's acceptance and measurement grants without
/// charging counters or touching the union.
///
/// Preflight and explicit replay share this: both resolve the receipt's
/// own referenced manifests and grant-check every statement and
/// attestation, and neither installs receiver trust.
///
/// # Errors
///
/// Returns `NATIVE_TEST_HISTORICAL_TRUST_UNAVAILABLE` for an unresolvable
/// manifest, `NATIVE_TEST_HISTORICAL_TRUST_REJECTED` for a failed grant,
/// or the exact nested parse failure.
fn check_receipt_trust_grants(
    receipt: &ImportedNativeTransactionReceipt,
    trust: &NativeExchangeTrust<'_>,
) -> Result<TrustGrantCounts> {
    let statement = receipt.statement.parts();
    let acceptance_manifest =
        resolve_trust_manifest(statement.acceptance_trust_policy_id.as_bytes(), trust)?;
    verify_acceptance_statement(
        &receipt.statement,
        receipt.transaction.record.workspace_id,
        statement.admission_profile,
        acceptance_manifest,
    )
    .map_err(map_trust_error)?;
    let plan = NativeTestPlanV1::parse(receipt.bundle.plan_stored())
        .map_err(|error| NativeExchangeError::Pack(scb_error(&error)))?;
    for embedded in receipt.bundle.measurements() {
        let attestation = MeasuredTestAttestationV1::parse(&embedded.stored)
            .map_err(|error| NativeExchangeError::Pack(scb_error(&error)))?;
        let measurement_manifest = resolve_trust_manifest(&attestation.trust_policy_id(), trust)?;
        verify_measurement_attestation(
            &attestation,
            receipt.transaction.record.workspace_id,
            plan.execution_profile(),
            measurement_manifest,
        )
        .map_err(map_trust_error)?;
    }
    Ok(TrustGrantCounts {
        signatures: receipt.bundle.measurements().len() as u64 + 1,
        visits: (receipt.bundle.executions().len() + receipt.bundle.measurements().len()) as u64,
    })
}

/// Verifies every native receipt's acceptance and measurement trust against
/// caller-supplied manifests while charging the Appendix B counters.
///
/// The wire union must equal the exact collected union first: a referenced
/// ID without a declaration is missing trust, and a declaration no receipt
/// references is not a valid encoding of the exchange contents. Nothing
/// here installs receiver trust.
fn verify_native_receipt_trust(
    receipts: &BTreeMap<TransactionId, ImportedReceipt>,
    declared_union: &[[u8; ID_LEN]],
    trust: &NativeExchangeTrust<'_>,
    counters: &mut NativePreflightCounters,
) -> Result<()> {
    let mut collected = BTreeSet::new();
    let mut native: Vec<&ImportedNativeTransactionReceipt> = Vec::new();
    for receipt in receipts.values() {
        if let ImportedReceipt::V2(inner) = receipt {
            collected.extend(
                native_receipt_trust_policy_ids(inner)
                    .map_err(NativeExchangeError::from)?
                    .into_iter(),
            );
            native.push(inner);
        }
    }
    for id in &collected {
        if !declared_union.contains(id) {
            return Err(NativeExchangeError::native(
                NativeExchangeErrorCode::TrustUnavailable,
            ));
        }
    }
    if declared_union.len() != collected.len() {
        return Err(NativeExchangeError::native(
            NativeExchangeErrorCode::EncodingInvalid,
        ));
    }
    for receipt in native {
        let counts = check_receipt_trust_grants(receipt, trust)?;
        counters.add_signatures(counts.signatures)?;
        counters.add_visits(counts.visits)?;
        counters.add_evidence_bytes(receipt.stored_bytes.len() as u64)?;
    }
    Ok(())
}

fn import_native_receipts(
    decoded: &DecodedNativeExchange,
) -> Result<BTreeMap<TransactionId, ImportedReceipt>> {
    let mut receipts = BTreeMap::new();
    for entry in &decoded.receipts {
        let reconstructed = entry.reconstructed()?;
        // Acceptance dispatches by exact receipt magic, never by caller
        // assertion or position.
        let receipt = if reconstructed.starts_with(&RECEIPT_MAGIC) {
            let imported =
                import_transaction_receipt(&reconstructed).map_err(NativeExchangeError::from)?;
            if imported.transaction.transaction_id != entry.transaction_id
                || imported.receipt_id != entry.receipt_id
            {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::ReceiptInvalid,
                ));
            }
            ImportedReceipt::V1(Box::new(imported))
        } else if reconstructed.starts_with(&NATIVE_RECEIPT_MAGIC) {
            let imported = import_native_transaction_receipt(&reconstructed)
                .map_err(NativeExchangeError::from)?;
            if imported.transaction.transaction_id != entry.transaction_id
                || imported.receipt_id != entry.receipt_id
            {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::ReceiptInvalid,
                ));
            }
            ImportedReceipt::V2(Box::new(imported))
        } else {
            return Err(NativeExchangeError::exchange(
                ExchangeErrorCode::ReceiptInvalid,
            ));
        };
        if receipts.insert(entry.transaction_id, receipt).is_some() {
            return Err(NativeExchangeError::exchange(
                ExchangeErrorCode::DuplicateEntry,
            ));
        }
    }
    Ok(receipts)
}

#[allow(clippy::too_many_lines)]
fn verify_native_receipts_against_pack(
    order: &[TransactionId],
    receipts: &BTreeMap<TransactionId, ImportedReceipt>,
    pack: &PreflightedPack,
) -> Result<()> {
    let objects: BTreeMap<ObjectId, &[u8]> = pack
        .decoded
        .objects
        .iter()
        .map(|object| (object.object_id, object.stored_bytes.as_slice()))
        .collect();
    let mut binding_visits = 0_u64;
    let mut receipt_bytes = 0_u64;
    let mut verified_objects: BTreeSet<ObjectId> = BTreeSet::new();
    let mut object_bytes = 0_u64;
    for transaction_id in order {
        let receipt = &receipts[transaction_id];
        receipt_bytes = receipt_bytes
            .checked_add(receipt.stored_bytes().len() as u64)
            .filter(|total| *total <= MAX_PREFLIGHT_RECEIPT_BYTES)
            .ok_or_else(|| NativeExchangeError::exchange(ExchangeErrorCode::ResourceLimit))?;
        binding_visits = binding_visits
            .checked_add(receipt.state_root().record.entity_bindings.len() as u64)
            .filter(|total| *total <= MAX_PREFLIGHT_BINDING_VISITS)
            .ok_or_else(|| NativeExchangeError::exchange(ExchangeErrorCode::ResourceLimit))?;
        for (_, object_id) in &receipt.state_root().record.entity_bindings {
            if verified_objects.insert(*object_id) {
                if verified_objects.len() as u64 > MAX_PREFLIGHT_OBJECT_VERIFICATIONS {
                    return Err(NativeExchangeError::exchange(
                        ExchangeErrorCode::ResourceLimit,
                    ));
                }
                let length = objects.get(object_id).map_or(0, |bytes| bytes.len() as u64);
                object_bytes = object_bytes
                    .checked_add(length)
                    .filter(|total| *total <= MAX_PREFLIGHT_OBJECT_BYTES)
                    .ok_or_else(|| {
                        NativeExchangeError::exchange(ExchangeErrorCode::ResourceLimit)
                    })?;
            }
        }
        let parent = receipt
            .parent_transaction_ids()
            .first()
            .and_then(|parent| receipts.get(parent));
        verify_any_receipt_against_objects(receipt, parent, &objects)
            .map_err(NativeExchangeError::from)?;
    }
    Ok(())
}

fn native_preflight<V: CanonicalVerifier>(
    input: &[u8],
    verifier: &V,
    trust: &NativeExchangeTrust<'_>,
) -> Result<NativePreflight> {
    let (exchange_id, _, payload) = decode_native_envelope(input)?;
    let decoded = decode_native_payload(&payload)?;
    if !embedded_pack_header_is_tag_170(&decoded.object_pack) {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::PackInvalid,
        ));
    }
    let pack = preflight_conformance_pack(&decoded.object_pack, verifier)
        .map_err(NativeExchangeError::from)?;
    let expected_leaves = compute_native_leaves(
        pack.pack_id,
        &decoded.object_pack,
        &decoded.receipts,
        decoded.accepted_head,
        &decoded.branches,
        decoded.transport_profile_id,
        &decoded.required_trust_policy_ids,
    )?;
    if decoded.leaves != expected_leaves
        || merkle_root(&expected_leaves, MAX_NATIVE_EXCHANGE_LEAVES)
            .map_err(NativeExchangeError::from)?
            != decoded.digest_tree_root
    {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::DigestTreeMismatch,
        ));
    }
    let receipts = import_native_receipts(&decoded)?;
    let order = native_verify_closure_rules(&decoded, &receipts, &pack.roots)?;
    let workspace = receipts
        .values()
        .next()
        .map(ImportedReceipt::workspace_id)
        .ok_or_else(|| NativeExchangeError::exchange(ExchangeErrorCode::AncestryOpen))?;
    let mut branches = Vec::with_capacity(decoded.branches.len());
    for entry in &decoded.branches {
        branches.push(native_verify_branch_entry(entry, &receipts, workspace)?);
    }
    native_verify_no_surplus(&receipts, decoded.accepted_head.transaction_id, &branches)?;
    let mut counters = NativePreflightCounters::default();
    verify_native_receipt_trust(
        &receipts,
        &decoded.required_trust_policy_ids,
        trust,
        &mut counters,
    )?;
    verify_native_receipts_against_pack(&order, &receipts, &pack)?;
    Ok(NativePreflight {
        exchange_id,
        pack,
        receipts,
        branches,
        accepted_head: decoded.accepted_head,
        leaves: decoded.leaves.len(),
        counters,
    })
}

/// Runs the complete native exchange preflight without any write.
///
/// Every receipt of either format imports with its declared identities
/// bound, the mixed ancestry closes over the pack roots with the head
/// matched, branches verify with fast-forward reachability and no surplus,
/// the trust union matches the evidence exactly with every statement and
/// attestation grant-checked against caller-supplied manifests, and every
/// receipt verifies against the pack objects under the shared v1 work
/// maxima plus the three native evidence counters. No worker runs, no
/// trust installs, no byte is written.
///
/// # Errors
///
/// Returns the first precise structural, ancestry, trust, counter, or
/// object failure.
pub fn preflight_native_exchange<V: CanonicalVerifier>(
    input: &[u8],
    verifier: &V,
    trust: &NativeExchangeTrust<'_>,
) -> Result<NativeExchangePreflightReport> {
    let preflight = native_preflight(input, verifier, trust)?;
    Ok(NativeExchangePreflightReport {
        exchange_id: preflight.exchange_id,
        pack_id: preflight.pack.pack_id,
        accepted_head: preflight.accepted_head,
        receipts: preflight.receipts.len(),
        branches: preflight.branches.len(),
        leaves: preflight.leaves,
        signatures_checked: preflight.counters.signatures,
        test_visits: preflight.counters.visits,
        evidence_bytes: preflight.counters.evidence_bytes,
    })
}

enum NativeTarget {
    Fresh,
    IncompleteClone,
}

struct InstalledNativeMarker {
    path: std::path::PathBuf,
    created: bool,
}

impl InstalledNativeMarker {
    fn sync_parent(&self) -> Result<()> {
        let parent = self.path.parent().ok_or_else(|| {
            NativeExchangeError::native(NativeExchangeErrorCode::InternalInvariant)
        })?;
        sync_directory(parent).map_err(NativeExchangeError::from)
    }
}

/// Installs the separate native exchange marker identity.
///
/// The marker lives under `exchange/v2`, never under the v1 marker
/// directory: a native import and a v1 import never share resume state.
/// The stage, sync, link discipline mirrors the v1 marker exactly.
//
// Native clone durability cuts mirror the v1 marker rows for the N8
// qualification slice: same one-shot thread-local discipline, `#[cfg(test)]`
// only, never a production fault-selector API.
#[cfg(test)]
enum NativeCloneDurabilityCut {
    Nclone01DuringMarkerTempWrite,
    Nclone02VerifiedMarkerTempBeforeRename,
    Nclone03MarkerRenameBeforeFirstSync,
}

#[cfg(test)]
std::thread_local! {
    static SELECTED_NATIVE_CLONE_CUT: std::cell::RefCell<Option<NativeCloneDurabilityCut>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
struct NativeCloneCutSelection;

#[cfg(test)]
impl NativeCloneCutSelection {
    fn install(cut: NativeCloneDurabilityCut) -> Self {
        SELECTED_NATIVE_CLONE_CUT.with(|selected| {
            let previous = selected.replace(Some(cut));
            assert!(
                previous.is_none(),
                "native clone durability selection is not nested"
            );
        });
        Self
    }
}

#[cfg(test)]
impl Drop for NativeCloneCutSelection {
    fn drop(&mut self) {
        SELECTED_NATIVE_CLONE_CUT.with(|selected| {
            selected.replace(None);
        });
    }
}

#[cfg(test)]
fn take_selected_native_clone_cut(
    predicate: impl FnOnce(&NativeCloneDurabilityCut) -> bool,
) -> bool {
    SELECTED_NATIVE_CLONE_CUT.with(|selected| {
        let take = selected.borrow().as_ref().is_some_and(predicate);
        if take {
            selected.borrow_mut().take();
        }
        take
    })
}

#[cfg(test)]
fn fail_selected_native_clone_cut(
    predicate: impl FnOnce(&NativeCloneDurabilityCut) -> bool,
) -> Result<()> {
    if take_selected_native_clone_cut(predicate) {
        return Err(NativeExchangeError::exchange(ExchangeErrorCode::Io));
    }
    Ok(())
}
fn install_native_stage_marker(
    target: &Path,
    exchange_id: RepositoryExchangeId,
) -> Result<InstalledNativeMarker> {
    create_real_directory(target).map_err(NativeExchangeError::from)?;
    let exchange_dir = target.join(EXCHANGE_DIRECTORY);
    create_real_directory(&exchange_dir).map_err(NativeExchangeError::from)?;
    let versioned = exchange_dir.join(NATIVE_EXCHANGE_VERSION_DIRECTORY);
    create_real_directory(&versioned).map_err(NativeExchangeError::from)?;
    let hex = hex_id(exchange_id.as_bytes());
    let marker = versioned.join(format!("{hex}{STAGE_SUFFIX}"));
    if let Ok(metadata) = fs::symlink_metadata(&marker) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(NativeExchangeError::exchange(ExchangeErrorCode::Io));
        }
        if read_regular_file(&marker, ID_LEN).map_err(NativeExchangeError::from)?
            == exchange_id.as_bytes()
        {
            return Ok(InstalledNativeMarker {
                path: marker,
                created: false,
            });
        }
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::TargetIncompleteMismatch,
        ));
    }
    let temporary = versioned.join(format!("{hex}{STAGE_TEMPORARY_SUFFIX}"));
    if fs::symlink_metadata(&temporary).is_ok() {
        fs::remove_file(&temporary).map_err(NativeExchangeError::from)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(NativeExchangeError::from)?;
    #[cfg(test)]
    if take_selected_native_clone_cut(|cut| {
        matches!(cut, NativeCloneDurabilityCut::Nclone01DuringMarkerTempWrite)
    }) {
        let half = exchange_id.as_bytes().len() / 2;
        file.write_all(&exchange_id.as_bytes()[..half])
            .map_err(NativeExchangeError::from)?;
        file.flush().map_err(NativeExchangeError::from)?;
        file.sync_all().map_err(NativeExchangeError::from)?;
        drop(file);
        return Err(NativeExchangeError::exchange(ExchangeErrorCode::Io));
    }
    file.write_all(exchange_id.as_bytes())
        .map_err(NativeExchangeError::from)?;
    file.sync_all().map_err(NativeExchangeError::from)?;
    drop(file);
    #[cfg(test)]
    fail_selected_native_clone_cut(|cut| {
        matches!(
            cut,
            NativeCloneDurabilityCut::Nclone02VerifiedMarkerTempBeforeRename
        )
    })?;
    fs::rename(&temporary, &marker).map_err(NativeExchangeError::from)?;
    #[cfg(test)]
    fail_selected_native_clone_cut(|cut| {
        matches!(
            cut,
            NativeCloneDurabilityCut::Nclone03MarkerRenameBeforeFirstSync
        )
    })?;
    sync_directory(&versioned).map_err(NativeExchangeError::from)?;
    sync_directory(&exchange_dir).map_err(NativeExchangeError::from)?;
    sync_directory(target).map_err(NativeExchangeError::from)?;
    Ok(InstalledNativeMarker {
        path: marker,
        created: true,
    })
}

/// Proves that everything already installed in a natively marked target
/// belongs to this exchange: the head (if present), every receipt by exact
/// file bytes, and every branch origin and ref by exact bytes.
fn verify_native_incomplete_clone(target: &Path, preflight: &NativePreflight) -> Result<()> {
    let head_path = target.join("heads").join("accepted");
    match fs::symlink_metadata(&head_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(NativeExchangeError::exchange(ExchangeErrorCode::Io));
            }
            let bytes =
                read_regular_file(&head_path, HEAD_LEN).map_err(NativeExchangeError::from)?;
            match decode_head_bytes(&bytes) {
                Some(head) if head == preflight.accepted_head.transaction_id => {}
                _ => {
                    return Err(NativeExchangeError::exchange(
                        ExchangeErrorCode::TargetIncompleteMismatch,
                    ));
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(NativeExchangeError::from(error)),
    }

    let receipt_bytes: BTreeMap<String, &[u8]> = preflight
        .receipts
        .iter()
        .map(|(transaction_id, receipt)| {
            (
                format!("{}{RECEIPT_SUFFIX}", hex_id(transaction_id.as_bytes())),
                receipt.stored_bytes(),
            )
        })
        .collect();
    for path in collect_files_with_suffix(&target.join("transactions"), RECEIPT_SUFFIX)
        .map_err(NativeExchangeError::from)?
    {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        let expected = name.as_deref().and_then(|name| receipt_bytes.get(name));
        let bytes =
            read_regular_file(&path, MAX_EXCHANGE_BYTES).map_err(NativeExchangeError::from)?;
        if expected.is_none_or(|expected| *expected != bytes.as_slice()) {
            return Err(NativeExchangeError::exchange(
                ExchangeErrorCode::TargetIncompleteMismatch,
            ));
        }
    }
    let origin_bytes: BTreeSet<&[u8]> = preflight
        .branches
        .iter()
        .map(|branch| branch.origin.stored_bytes.as_slice())
        .collect();
    for path in collect_files_with_suffix(&target.join("branches"), ORIGIN_SUFFIX)
        .map_err(NativeExchangeError::from)?
    {
        let bytes =
            read_regular_file(&path, MAX_EXCHANGE_BYTES).map_err(NativeExchangeError::from)?;
        if !origin_bytes.contains(bytes.as_slice()) {
            return Err(NativeExchangeError::exchange(
                ExchangeErrorCode::TargetIncompleteMismatch,
            ));
        }
    }
    let ref_bytes: BTreeSet<&[u8]> = preflight
        .branches
        .iter()
        .map(|branch| branch.reference.stored_bytes.as_slice())
        .collect();
    for path in collect_files_with_suffix(&target.join("refs"), REF_SUFFIX)
        .map_err(NativeExchangeError::from)?
    {
        let bytes =
            read_regular_file(&path, MAX_EXCHANGE_BYTES).map_err(NativeExchangeError::from)?;
        if !ref_bytes.contains(bytes.as_slice()) {
            return Err(NativeExchangeError::exchange(
                ExchangeErrorCode::TargetIncompleteMismatch,
            ));
        }
    }
    Ok(())
}

/// Scans the native marker directory for this exchange's stage marker.
///
/// Returns whether the exact marker is present and durable. Any foreign,
/// malformed, or mismatched marker state refuses; a leftover temporary
/// from a crashed marker install is tolerated like the v1 discipline.
fn scan_native_markers(
    versioned: &Path,
    marker_name: &str,
    temporary_name: &str,
    exchange_id: &RepositoryExchangeId,
) -> Result<bool> {
    let mut markers = 0_usize;
    let mut present = false;
    for entry in fs::read_dir(versioned).map_err(NativeExchangeError::from)? {
        let entry = entry.map_err(NativeExchangeError::from)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry.file_type().map_err(NativeExchangeError::from)?;
        if file_type.is_symlink() {
            return Err(NativeExchangeError::exchange(ExchangeErrorCode::Io));
        }
        if name == temporary_name {
            if !file_type.is_file() {
                return Err(NativeExchangeError::exchange(ExchangeErrorCode::Io));
            }
            continue;
        }
        if name.ends_with(STAGE_SUFFIX) {
            if !file_type.is_file() {
                return Err(NativeExchangeError::exchange(ExchangeErrorCode::Io));
            }
            markers += 1;
            if name != marker_name {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::TargetIncompleteMismatch,
                ));
            }
            let contents =
                read_regular_file(&entry.path(), ID_LEN).map_err(NativeExchangeError::from)?;
            if contents != exchange_id.as_bytes() {
                return Err(NativeExchangeError::exchange(
                    ExchangeErrorCode::TargetIncompleteMismatch,
                ));
            }
            present = true;
            continue;
        }
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::TargetNotEmpty,
        ));
    }
    if markers > 1 {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::TargetIncompleteMismatch,
        ));
    }
    Ok(present)
}

fn classify_native_target(target: &Path, preflight: &NativePreflight) -> Result<NativeTarget> {
    let Some(_) = real_directory_metadata(target).map_err(NativeExchangeError::from)? else {
        return Ok(NativeTarget::Fresh);
    };
    let hex = hex_id(preflight.exchange_id.as_bytes());
    let marker_name = format!("{hex}{STAGE_SUFFIX}");
    let temporary_name = format!("{hex}{STAGE_TEMPORARY_SUFFIX}");
    let mut entries: Vec<String> = Vec::new();
    for entry in fs::read_dir(target).map_err(NativeExchangeError::from)? {
        let entry = entry.map_err(NativeExchangeError::from)?;
        entries.push(entry.file_name().to_string_lossy().into_owned());
    }
    if entries.is_empty() {
        return Ok(NativeTarget::Fresh);
    }
    let exchange_dir = target.join(EXCHANGE_DIRECTORY);
    let mut marker_present = false;
    if real_directory_metadata(&exchange_dir)
        .map_err(NativeExchangeError::from)?
        .is_some()
    {
        let mut exchange_entries = Vec::new();
        for entry in fs::read_dir(&exchange_dir).map_err(NativeExchangeError::from)? {
            exchange_entries.push(
                entry
                    .map_err(NativeExchangeError::from)?
                    .file_name()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        // A v1 marker directory beside the native one is foreign state, not
        // resume state: the two transports never share an import.
        if exchange_entries
            .iter()
            .any(|name| name != NATIVE_EXCHANGE_VERSION_DIRECTORY)
        {
            return Err(NativeExchangeError::exchange(
                ExchangeErrorCode::TargetNotEmpty,
            ));
        }
        let versioned = exchange_dir.join(NATIVE_EXCHANGE_VERSION_DIRECTORY);
        if real_directory_metadata(&versioned)
            .map_err(NativeExchangeError::from)?
            .is_some()
        {
            marker_present = scan_native_markers(
                &versioned,
                &marker_name,
                &temporary_name,
                &preflight.exchange_id,
            )?;
        }
    }
    let only_exchange = entries.iter().all(|name| name == EXCHANGE_DIRECTORY);
    if !marker_present {
        return if only_exchange {
            Ok(NativeTarget::Fresh)
        } else {
            Err(NativeExchangeError::exchange(
                ExchangeErrorCode::TargetNotEmpty,
            ))
        };
    }
    if entries
        .iter()
        .any(|name| !REPOSITORY_LAYOUT_ENTRIES.contains(&name.as_str()))
    {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::TargetNotEmpty,
        ));
    }
    verify_native_incomplete_clone(target, preflight)?;
    Ok(NativeTarget::IncompleteClone)
}

fn purge_native_index_cache(target: &Path) -> Result<()> {
    let path = target.join(INDEX_DIRECTORY);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(NativeExchangeError::from(error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(NativeExchangeError::exchange(ExchangeErrorCode::Io));
    }
    fs::remove_dir_all(&path).map_err(NativeExchangeError::from)?;
    sync_directory(target).map_err(NativeExchangeError::from)
}

/// Installs verified branch origins and refs into the shared live layout.
///
/// Branch records are format-agnostic S20-500 bytes, so a native import
/// installs them under the same `branches/v1` and `refs/v1` directories a
/// v1 import uses — never a forked layout. Only preflight-verified
/// branches arrive here; the collision checks still compare exact bytes.
fn install_native_branches(
    target: &Path,
    branches: &[NativeVerifiedBranch],
    branch_repository: &BranchRepository,
) -> Result<()> {
    branch_repository
        .ensure_layout_under_maintenance()
        .map_err(NativeExchangeError::from)?;
    let _refs_lock = branch_repository
        .acquire_refs_lock()
        .map_err(NativeExchangeError::from)?;
    let branches_dir = target.join("branches").join(EXCHANGE_VERSION_DIRECTORY);
    let refs_dir = target.join("refs").join(EXCHANGE_VERSION_DIRECTORY);
    for branch in branches {
        let origin_path = ensure_key_path(&branches_dir, &branch.name, ORIGIN_SUFFIX, 2, 3)
            .map_err(NativeExchangeError::from)?;
        let ref_path = ensure_key_path(&refs_dir, &branch.name, REF_SUFFIX, 0, 0)
            .map_err(NativeExchangeError::from)?;
        let expected_origin = branch.origin.clone();
        persist_no_overwrite(
            &origin_path,
            &branch.origin.stored_bytes,
            BRANCH_STAGE_PREFIX,
            BranchErrorCode::BranchOriginMismatch,
            move |bytes| {
                if import_branch_record(bytes)? == expected_origin {
                    Ok(())
                } else {
                    Err(crate::refs::BranchError::Branch(
                        BranchErrorCode::BranchOriginMismatch,
                    ))
                }
            },
        )
        .map_err(map_native_install_collision)?;
        persist_expected_ref(&ref_path, &branch.reference).map_err(map_native_install_collision)?;
    }
    Ok(())
}

fn map_native_install_collision(error: crate::refs::BranchError) -> NativeExchangeError {
    match &error {
        crate::refs::BranchError::Branch(
            BranchErrorCode::BranchOriginMismatch | BranchErrorCode::RefAlreadyExists,
        ) => NativeExchangeError::exchange(ExchangeErrorCode::Io),
        _ => NativeExchangeError::Exchange(crate::exchange::ExchangeError::Branch(error)),
    }
}

/// Imports a fully preflighted native exchange into a fresh or matching
/// incomplete target.
///
/// Promotion order mirrors the v1 clone exactly: stage marker, exclusive
/// maintenance with owned reclassification, derived-cache purge, pack
/// objects, mixed receipts, branch records, accepted head last, marker
/// removal. Everything above was verified before the first write; the head
/// write still lands last with the receipt-before-head durability the
/// shared installer owns.
///
/// # Errors
///
/// Returns the first target, promotion, or durability failure; a target
/// that changed between the advisory and owned classifications aborts
/// without adopting foreign state.
pub fn import_native_exchange<V: CanonicalVerifier>(
    target: &Path,
    input: &[u8],
    verifier: &V,
    trust: &NativeExchangeTrust<'_>,
) -> Result<NativeExchangeImportReport> {
    let preflight = native_preflight(input, verifier, trust)?;
    let _advisory = classify_native_target(target, &preflight)?;
    let marker = install_native_stage_marker(target, preflight.exchange_id)?;
    initialize_repository_maintenance(target).map_err(NativeExchangeError::from)?;
    let maintenance = acquire_exclusive_repository_maintenance_nonblocking(target)
        .map_err(NativeExchangeError::from)?;
    let transactions = TransactionRepository::new(target);
    if !matches!(
        classify_native_target(target, &preflight)?,
        NativeTarget::IncompleteClone
    ) {
        if marker.created {
            fs::remove_file(&marker.path).map_err(NativeExchangeError::from)?;
            marker.sync_parent()?;
        }
        drop(maintenance);
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::TargetIncompleteMismatch,
        ));
    }
    purge_native_index_cache(target)?;
    let store = ObjectStore::new(target);
    let (promoted_objects, present_objects) =
        promote_pack_objects(&store, &preflight.pack.decoded.objects, verifier)
            .map_err(NativeExchangeError::from)?;
    let order = native_topological_order(&preflight.receipts)?;
    let mut staged: Vec<&[u8]> = Vec::with_capacity(preflight.receipts.len());
    for transaction_id in &order {
        staged.push(preflight.receipts[transaction_id].stored_bytes());
    }
    transactions
        .initialize_trusted_mixed_clone_receipts_with_maintenance(
            &maintenance,
            preflight.accepted_head.transaction_id,
            &staged,
        )
        .map_err(NativeExchangeError::from)?;
    let branch_repository = BranchRepository::new(target);
    install_native_branches(target, &preflight.branches, &branch_repository)?;
    let head = transactions
        .initialize_trusted_mixed_clone_head_with_maintenance(
            &maintenance,
            preflight.accepted_head.transaction_id,
        )
        .map_err(NativeExchangeError::from)?;
    if head.transaction_id() != preflight.accepted_head.transaction_id
        || head.receipt_id() != preflight.accepted_head.receipt_id
    {
        return Err(NativeExchangeError::native(
            NativeExchangeErrorCode::InternalInvariant,
        ));
    }
    fs::remove_file(&marker.path).map_err(NativeExchangeError::from)?;
    marker.sync_parent()?;
    drop(maintenance);
    Ok(NativeExchangeImportReport {
        exchange_id: preflight.exchange_id,
        accepted_head: preflight.accepted_head,
        receipts: preflight.receipts.len(),
        branches: preflight.branches.len(),
        promoted_objects,
        present_objects,
    })
}

/// Explicit replay outcome: local wire tags mirroring the 606 response
/// statuses, not global error codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeReplayStatus {
    /// Re-execution reproduces the exact evidence bytes and counters.
    Matched,
    /// Re-execution diverges from the recorded evidence.
    Mismatch,
    /// The executor, host, or replay ceilings could not complete the run.
    InconclusiveResource,
    /// The named history does not verify or its trust does not grant.
    UntrustedHistory,
}

impl NativeReplayStatus {
    /// Returns the 606 response status tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Matched => 1,
            Self::Mismatch => 2,
            Self::InconclusiveResource => 3,
            Self::UntrustedHistory => 4,
        }
    }
}

/// Explicit replay request over pinned repository history.
#[derive(Clone, Copy)]
pub struct NativeReplayRequest<'a> {
    /// Native transaction to replay.
    pub transaction_id: TransactionId,
    /// Exact committed root the replay must reproduce.
    pub expected_root: StateRoot,
    /// Re-execution backend; `None` refuses before any work.
    pub executor: Option<&'a dyn NativeTestExecutor>,
    /// Trust manifests the replayed history must grant against.
    pub trust: NativeExchangeTrust<'a>,
}

/// Explicit replay report. No accepted transaction and no replacement
/// historical attestation is created; `replay_report_id` stays `None`
/// locally (a future session layer may serve replayed bytes under its own
/// capabilities without persisting them).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeReplayReport {
    /// Replayed transaction.
    pub transaction_id: TransactionId,
    /// Committed root the replay ran against.
    pub root: StateRoot,
    /// Replay outcome.
    pub status: NativeReplayStatus,
    /// Original deterministic test report.
    pub original_report_id: TestReportId,
    /// Never populated by the local service.
    pub replay_report_id: Option<TestReportId>,
}

/// Compares original and replayed execution evidence by test entity.
///
/// Both sides must name the same test entities with byte-identical
/// execution reports. Order-independent: coverage already binds the
/// replayed side to plan order, and the original side is keyed by entity.
fn replay_executions_match(
    original: &[(EntityId, Vec<u8>)],
    replayed: &[(EntityId, Vec<u8>)],
) -> bool {
    if original.len() != replayed.len() {
        return false;
    }
    let mut expected = original.to_vec();
    expected.sort_by(|left, right| left.0.cmp(&right.0));
    let mut actual = replayed.to_vec();
    actual.sort_by(|left, right| left.0.cmp(&right.0));
    expected == actual
}

/// Replays one native transaction against its pinned history.
///
/// The shared maintenance guard pins the history for the whole call: the
/// receipt imports, re-verifies against the live store, and re-checks its
/// trust grants before any re-execution. The stored plan re-executes under
/// the recorded execution profile through the supplied executor, which
/// sees the plan and the pinned object bytes but never the original
/// evidence and never a live validation context (replay is secret-free by
/// construction). Coverage binds the returned evidence to the plan, and
/// exact byte equality per test entity decides matched versus mismatch.
/// Executor, host, coverage, or ceiling failures are inconclusive, never
/// historical verdicts.
///
/// # Errors
///
/// Returns the first host, configuration, or malformed-history failure:
/// maintenance acquisition, a missing executor, a non-native or
/// codec-malformed named transaction. Unverifiable, mismatched-root,
/// untrusted, divergent, and inconclusive histories arrive as statuses.
/// A replay history failure: either a verdict about the named history or
/// a host/configuration error that aborts the call.
enum ReplayFailure {
    Status(NativeReplayStatus),
    Error(NativeExchangeError),
}

impl From<NativeExchangeError> for ReplayFailure {
    fn from(error: NativeExchangeError) -> Self {
        Self::Error(error)
    }
}

/// Verified replay inputs: the pinned objects and the stored plan.
struct VerifiedReplayHistory {
    objects: Vec<(ObjectId, Vec<u8>)>,
    plan: NativeTestPlanV1,
}

fn host_unreadable(message: &'static str) -> ReplayFailure {
    ReplayFailure::Error(NativeExchangeError::Exchange(ExchangeError::Io(
        std::io::Error::other(message),
    )))
}

/// Loads the objects a replay verification binds, straight from the live
/// store with identity checks on read.
///
/// A host I/O failure aborts the call; any other store failure means the
/// pinned history does not verify.
fn load_replay_objects<V: CanonicalVerifier>(
    store: &ObjectStore,
    verifier: &V,
    bindings: &[(sley_id::EntityId, ObjectId)],
) -> core::result::Result<Vec<(ObjectId, Vec<u8>)>, ReplayFailure> {
    let mut objects: Vec<(ObjectId, Vec<u8>)> = Vec::new();
    for (_, object_id) in bindings {
        match store.read(*object_id, verifier) {
            Ok(bytes) => objects.push((*object_id, bytes)),
            Err(error) if error.code() == sley_store::StoreErrorCode::StoreIo => {
                return Err(host_unreadable(error.symbol()));
            }
            Err(_) => {
                return Err(ReplayFailure::Status(NativeReplayStatus::UntrustedHistory));
            }
        }
    }
    Ok(objects)
}

/// Verifies the named native history against the live store and its trust
/// grants without executing anything.
///
/// Every history verdict (untrusted root, object, or grant) arrives as a
/// status; only host I/O aborts as an error. The original report must
/// already have parsed: this runs after the caller holds its identity.
#[allow(clippy::too_many_arguments)]
fn verify_replay_history<V: CanonicalVerifier>(
    transactions: &TransactionRepository,
    maintenance: &RepositoryMaintenanceGuard,
    store: &ObjectStore,
    verifier: &V,
    receipt: &ImportedReceipt,
    native: &ImportedNativeTransactionReceipt,
    expected_root: StateRoot,
    trust: &NativeExchangeTrust<'_>,
) -> core::result::Result<VerifiedReplayHistory, ReplayFailure> {
    if native.transaction.record.committed_root != expected_root {
        return Err(ReplayFailure::Status(NativeReplayStatus::UntrustedHistory));
    }
    let objects = load_replay_objects(store, verifier, &native.state_root.record.entity_bindings)?;
    let by_id: BTreeMap<ObjectId, &[u8]> = objects
        .iter()
        .map(|(id, bytes)| (*id, bytes.as_slice()))
        .collect();
    let parent = match native.transaction.record.parent_transaction_ids.first() {
        Some(parent_id) => {
            match transactions.imported_receipt_any_with_maintenance(maintenance, *parent_id) {
                Ok(parent) => Some(parent),
                Err(CommitError::Io(_)) => {
                    return Err(host_unreadable("replay parent history unreadable"));
                }
                Err(_) => {
                    return Err(ReplayFailure::Status(NativeReplayStatus::UntrustedHistory));
                }
            }
        }
        None => None,
    };
    if let Err(error) = verify_any_receipt_against_objects(receipt, parent.as_ref(), &by_id) {
        match error {
            CommitError::Io(_) => {
                return Err(host_unreadable("replay history unreadable"));
            }
            _ => {
                return Err(ReplayFailure::Status(NativeReplayStatus::UntrustedHistory));
            }
        }
    }
    let counts = match check_receipt_trust_grants(native, trust) {
        Ok(counts) => counts,
        Err(
            NativeExchangeError::Native(
                NativeExchangeErrorCode::TrustUnavailable | NativeExchangeErrorCode::TrustRejected,
            )
            | NativeExchangeError::Pack(_),
        ) => {
            return Err(ReplayFailure::Status(NativeReplayStatus::UntrustedHistory));
        }
        Err(error) => return Err(ReplayFailure::Error(error)),
    };
    let mut counters = NativePreflightCounters::default();
    for step in [
        counters.add_signatures(counts.signatures),
        counters.add_visits(counts.visits),
        counters.add_evidence_bytes(native.stored_bytes.len() as u64),
    ] {
        if let Err(error) = step {
            match error {
                NativeExchangeError::Native(NativeExchangeErrorCode::CounterLimit) => {
                    return Err(ReplayFailure::Status(
                        NativeReplayStatus::InconclusiveResource,
                    ));
                }
                _ => return Err(ReplayFailure::Error(error)),
            }
        }
    }
    // The stored plan must parse for re-execution; a native receipt whose
    // plan no longer parses is untrusted history, never a malformed
    // request: the outer receipt already imported.
    let Ok(plan) = NativeTestPlanV1::parse(native.bundle.plan_stored()) else {
        return Err(ReplayFailure::Status(NativeReplayStatus::UntrustedHistory));
    };
    Ok(VerifiedReplayHistory { objects, plan })
}

/// Replays one native transaction against its pinned history.
///
/// The shared maintenance guard pins the history for the whole call: the
/// receipt imports, re-verifies against the live store, and re-checks its
/// trust grants before any re-execution. The stored plan re-executes under
/// the recorded execution profile through the supplied executor, which
/// sees the plan and the pinned object bytes but never the original
/// evidence and never a live validation context (replay is secret-free by
/// construction). Coverage binds the returned evidence to the plan, and
/// exact byte equality per test entity decides matched versus mismatch.
/// Executor, host, coverage, or ceiling failures are inconclusive, never
/// historical verdicts.
///
/// # Errors
///
/// Returns the first host, configuration, or malformed-history failure:
/// maintenance acquisition, a missing executor, a non-native or
/// codec-malformed named transaction. Unverifiable, mismatched-root,
/// untrusted, divergent, and inconclusive histories arrive as statuses.
pub fn replay_native_commit<V: CanonicalVerifier>(
    root: &Path,
    request: &NativeReplayRequest<'_>,
    verifier: &V,
) -> Result<NativeReplayReport> {
    let maintenance = acquire_shared_repository_maintenance(root)?;
    let transactions = TransactionRepository::new(root);
    let receipt =
        transactions.imported_receipt_any_with_maintenance(&maintenance, request.transaction_id)?;
    let ImportedReceipt::V2(native) = &receipt else {
        return Err(NativeExchangeError::exchange(
            ExchangeErrorCode::ReceiptInvalid,
        ));
    };
    // Malformed nested bytes are a request error; every later history
    // verdict carries the parsed report identity.
    let original_report_id = NativeTestReportV1::parse(native.bundle.test_report_stored())
        .map_err(|error| NativeExchangeError::Pack(scb_error(&error)))?
        .report_id();
    let report_with = |status| NativeReplayReport {
        transaction_id: request.transaction_id,
        root: native.transaction.record.committed_root,
        status,
        original_report_id,
        replay_report_id: None,
    };
    let store = ObjectStore::new(root);
    let history = match verify_replay_history(
        &transactions,
        &maintenance,
        &store,
        verifier,
        &receipt,
        native,
        request.expected_root,
        &request.trust,
    ) {
        Ok(history) => history,
        Err(ReplayFailure::Status(status)) => return Ok(report_with(status)),
        Err(ReplayFailure::Error(error)) => return Err(error),
    };
    let by_id: BTreeMap<ObjectId, &[u8]> = history
        .objects
        .iter()
        .map(|(id, bytes)| (*id, bytes.as_slice()))
        .collect();
    let executor = request
        .executor
        .ok_or_else(|| NativeExchangeError::native(NativeExchangeErrorCode::ExecutorUnavailable))?;
    let Ok(executions) = executor.execute_replay(&history.plan, &by_id) else {
        return Ok(report_with(NativeReplayStatus::InconclusiveResource));
    };
    if check_execution_coverage(&history.plan, &executions).is_err() {
        return Ok(report_with(NativeReplayStatus::InconclusiveResource));
    }
    let original: Vec<(EntityId, Vec<u8>)> = native
        .bundle
        .executions()
        .iter()
        .map(|embedded| (embedded.test_entity, embedded.stored.clone()))
        .collect();
    let replayed: Vec<(EntityId, Vec<u8>)> = executions
        .iter()
        .map(|execution| (execution.test_entity, execution.execution_stored.clone()))
        .collect();
    if replay_executions_match(&original, &replayed) {
        Ok(report_with(NativeReplayStatus::Matched))
    } else {
        Ok(report_with(NativeReplayStatus::Mismatch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::tests::{TempDir, candidate_for, fixed, namespace_body, verifier};
    use crate::refs::BranchRepository;

    use std::cell::Cell;
    use std::fs;

    use sley_id::{EntityId, NativeAdmissionProfileId, PrincipalId, SchemaEpochId, WorkspaceId};
    use sley_mutate::{EntityObjectRecord, MutationClass, build_entity_object};
    use sley_policy::{
        CandidateValidationLimits, PolicyResourceCeilings, PolicyRootBuilder,
        PrincipalGrantBuilder, ValidatedCandidatePlan, conformance_registry as policy_registry,
        fixed_native_admission_profile,
    };
    use sley_state_root::{
        StateRootBuilder, conformance_epoch_id as state_epoch_id,
        conformance_registry as state_registry,
    };
    use sley_store::ObjectStore;
    use sley_tests::{
        HistoricalTrustPolicyParts, HistoricalTrustPolicyV1, NativeAggregateLimits,
        NativeImplementationLimits, ROLE_ACCEPTANCE, ROLE_MEASUREMENT, TrustEntry,
        native_execution_profile_id,
    };
    use sley_txn::{
        CommitInput, Ed25519AcceptanceSigner, ExecutedNativeTest, NativeAcceptanceSigner,
        NativeAttemptId, NativeCommitError, NativeCommitInput, NativeCommitOutcome,
        NativeTestExecutor, TransactionRepository, TrustedGenesisInput,
    };
    const NOW: u64 = 1_000;
    const MEASUREMENT_KEY: [u8; 32] = [0xB2; 32];
    const ACCEPTANCE_SECRET: [u8; 32] = [0xA1; 32];

    struct TestSigner(Ed25519AcceptanceSigner);

    impl NativeAcceptanceSigner for TestSigner {
        fn key_id(&self) -> [u8; 32] {
            self.0.key_id()
        }

        fn sign(&self, preimage: &[u8]) -> [u8; 64] {
            self.0.sign(preimage)
        }
    }

    fn acceptance_key() -> [u8; 32] {
        Ed25519AcceptanceSigner::from_secret_bytes(ACCEPTANCE_SECRET).key_id()
    }

    struct CountingExecutor {
        invocations: Cell<usize>,
    }

    impl NativeTestExecutor for CountingExecutor {
        fn execute(
            &self,
            _plan: &sley_tests::NativeTestPlanV1,
            _validated: &ValidatedCandidatePlan,
        ) -> core::result::Result<Vec<ExecutedNativeTest>, NativeCommitError> {
            self.invocations.set(self.invocations.get() + 1);
            Ok(Vec::new())
        }
    }

    fn test_trust(
        key: [u8; 32],
        role: u32,
        workspace: WorkspaceId,
        profile: [u8; 32],
    ) -> HistoricalTrustPolicyV1 {
        HistoricalTrustPolicyV1::build(HistoricalTrustPolicyParts {
            policy_nonce: [0x11; 32],
            entries: vec![TrustEntry {
                key_id: key,
                role,
                workspaces: vec![*workspace.as_bytes()],
                profiles: vec![profile],
                valid_from_unix_millis: 0,
                valid_until_unix_millis: u64::MAX,
            }],
        })
        .expect("test trust builds")
    }

    struct NativeHarness {
        measurement_trust: HistoricalTrustPolicyV1,
        acceptance_trust: HistoricalTrustPolicyV1,
        signer: TestSigner,
        admission_profile: NativeAdmissionProfileId,
    }

    impl NativeHarness {
        fn new(workspace: WorkspaceId) -> Self {
            let admission_profile = fixed_native_admission_profile()
                .expect("fixed descriptor builds")
                .id();
            let measurement_trust = test_trust(
                MEASUREMENT_KEY,
                ROLE_MEASUREMENT,
                workspace,
                *native_execution_profile_id().as_bytes(),
            );
            let acceptance_trust = test_trust(
                acceptance_key(),
                ROLE_ACCEPTANCE,
                workspace,
                *admission_profile.as_bytes(),
            );
            Self {
                measurement_trust,
                acceptance_trust,
                signer: TestSigner(Ed25519AcceptanceSigner::from_secret_bytes(
                    ACCEPTANCE_SECRET,
                )),
                admission_profile,
            }
        }

        fn input<'a>(
            &'a self,
            expected_parent: TransactionId,
            candidate: &'a [u8],
            principal: PrincipalId,
            attempt: NativeAttemptId,
            executor: Option<&'a dyn NativeTestExecutor>,
        ) -> NativeCommitInput<'a> {
            NativeCommitInput {
                expected_parent,
                stored_candidate: candidate,
                principal_id: principal,
                capabilities: &[],
                now_unix_millis: NOW,
                limits: CandidateValidationLimits::full_v1(),
                attempt_id: attempt,
                admission_profile_id: self.admission_profile,
                implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
                aggregate: NativeAggregateLimits::HARD_MAXIMA,
                executor,
                acceptance_signer: &self.signer,
                measurement_trust: &self.measurement_trust,
                acceptance_trust: &self.acceptance_trust,
            }
        }
    }

    fn id(byte: u8) -> [u8; ID_LEN] {
        [byte; ID_LEN]
    }

    /// A mixed-history source: genesis, one v1 commit, one empty-selection
    /// native commit, branch `main` at the v1 commit and branch `aux` at the
    /// v1 commit. Branches stay on format-1 heads (branch advance to native
    /// heads is N6c); the native head enters the closure through the head
    /// seed, exactly like an unbranched tip.
    struct NativeSource {
        temp: TempDir,
        root: std::path::PathBuf,
        epoch: SchemaEpochId,
        genesis: TransactionId,
        v1_head: TransactionId,
        head: TransactionId,
        acceptance_id: [u8; 32],
        trust_manifests: Vec<HistoricalTrustPolicyV1>,
    }

    /// Genesis plus one v1 commit with the policy, objects, and roots the
    /// native commit builds on.
    struct V1Base {
        temp: TempDir,
        root: std::path::PathBuf,
        epoch: SchemaEpochId,
        genesis: TransactionId,
        v1_head: TransactionId,
        workspace_id: WorkspaceId,
        principal_id: PrincipalId,
        policy: sley_policy::AcceptedPolicyRoot,
    }

    impl NativeSource {
        fn new(label: &str) -> Self {
            Self::build(label, true)
        }

        fn v1_only(label: &str) -> Self {
            Self::build(label, false)
        }

        fn v1_base(label: &str) -> V1Base {
            let temp = TempDir::new(label);
            let root = temp.child("source");
            fs::create_dir(&root).unwrap();
            let transactions = TransactionRepository::new(&root);
            let workspace_id = fixed(1, WorkspaceId::from_bytes);
            let principal_id = fixed(2, PrincipalId::from_bytes);
            let base_entity = fixed(10, EntityId::from_bytes);
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
                        entity_id: fixed(byte, EntityId::from_bytes),
                        body: namespace_body(),
                        label: None,
                        semantic_fingerprint: None,
                    },
                )
                .unwrap();
                store
                    .put(object.object_id(), object.stored_bytes(), &verifier(epoch))
                    .unwrap();
                object.object_id()
            });
            let base_state =
                StateRootBuilder::new(workspace_id, anchors[0], anchors[1], policy.root())
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
            let candidate = candidate_for(
                workspace_id,
                principal_id,
                genesis,
                &base_state,
                &policy,
                30,
            );
            let v1_head = transactions
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
            V1Base {
                temp,
                root,
                epoch,
                genesis,
                v1_head,
                workspace_id,
                principal_id,
                policy,
            }
        }

        fn build(label: &str, with_native: bool) -> Self {
            let V1Base {
                temp,
                root,
                epoch,
                genesis,
                v1_head,
                workspace_id,
                principal_id,
                policy,
            } = Self::v1_base(label);
            let transactions = TransactionRepository::new(&root);
            let branches = BranchRepository::new(&root);
            branches.create_branch("main", genesis).unwrap();
            branches.advance_branch("main", genesis, v1_head).unwrap();
            branches.create_branch("aux", v1_head).unwrap();
            if !with_native {
                return Self {
                    temp,
                    root,
                    epoch,
                    genesis,
                    v1_head,
                    head: v1_head,
                    acceptance_id: [0; 32],
                    trust_manifests: Vec::new(),
                };
            }
            // The native commit carries a plain candidate with no TestCase:
            // the empty selection commits live through the test executor.
            let harness = NativeHarness::new(workspace_id);
            let executor = CountingExecutor {
                invocations: Cell::new(0),
            };
            let v1_state = transactions
                .verified_revision(v1_head)
                .unwrap()
                .state_root()
                .clone();
            let native_candidate =
                candidate_for(workspace_id, principal_id, v1_head, &v1_state, &policy, 31);
            let outcome = transactions
                .commit_native(&harness.input(
                    v1_head,
                    &native_candidate.stored_bytes,
                    principal_id,
                    NativeAttemptId([0xC1; 16]),
                    Some(&executor),
                ))
                .expect("empty native commit succeeds");
            let NativeCommitOutcome::Committed(output) = outcome else {
                panic!("empty selection must commit");
            };
            let acceptance_id = *harness.acceptance_trust.id().as_bytes();
            let trust_manifests = vec![
                harness.measurement_trust.clone(),
                harness.acceptance_trust.clone(),
            ];
            Self {
                temp,
                root,
                epoch,
                genesis,
                v1_head,
                head: output.transaction_id(),
                acceptance_id,
                trust_manifests,
            }
        }

        fn target(&self, name: &str) -> std::path::PathBuf {
            self.temp.child(name)
        }

        fn trust(&self) -> NativeExchangeTrust<'_> {
            NativeExchangeTrust {
                manifests: &self.trust_manifests,
            }
        }
    }

    /// Exact receipt file bytes for one transaction, through the
    /// deterministic fanout layout both formats share.
    fn receipt_file_bytes(root: &std::path::Path, transaction: TransactionId) -> Vec<u8> {
        let hex = const_hex(transaction.as_bytes());
        fs::read(
            root.join("transactions")
                .join("v1")
                .join(&hex[0..2])
                .join(&hex[2..4])
                .join(format!("{hex}.receipt.scb1")),
        )
        .unwrap()
    }

    fn const_hex(bytes: &[u8; 32]) -> String {
        let mut out = String::with_capacity(64);
        for byte in bytes {
            out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap());
            out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap());
        }
        out
    }

    #[test]
    fn native_error_codes_map_to_the_reserved_symbols_and_numerics() {
        let cases = [
            (
                NativeExchangeErrorCode::ProfileMismatch,
                "NATIVE_TEST_PROFILE_UNSUPPORTED",
                29_200,
            ),
            (
                NativeExchangeErrorCode::EncodingInvalid,
                "NATIVE_TEST_ENCODING_INVALID",
                29_201,
            ),
            (
                NativeExchangeErrorCode::ExecutorUnavailable,
                "NATIVE_TEST_ENFORCER_UNAVAILABLE",
                29_212,
            ),
            (
                NativeExchangeErrorCode::TrustUnavailable,
                "NATIVE_TEST_HISTORICAL_TRUST_UNAVAILABLE",
                29_215,
            ),
            (
                NativeExchangeErrorCode::TrustRejected,
                "NATIVE_TEST_HISTORICAL_TRUST_REJECTED",
                29_216,
            ),
            (
                NativeExchangeErrorCode::ReplayInconclusive,
                "NATIVE_TEST_REPLAY_INCONCLUSIVE",
                29_223,
            ),
            (
                NativeExchangeErrorCode::CounterLimit,
                "NATIVE_TEST_RESOURCE_LIMIT",
                29_224,
            ),
            (
                NativeExchangeErrorCode::InternalInvariant,
                "NATIVE_TEST_INTERNAL_INVARIANT",
                29_225,
            ),
        ];
        assert_eq!(NativeExchangeErrorCode::ALL.len(), cases.len());
        for (code, symbol, numeric) in cases {
            assert!(NativeExchangeErrorCode::ALL.contains(&code));
            assert_eq!(code.as_str(), symbol);
            assert_eq!(code.numeric(), numeric);
            let error = NativeExchangeError::Native(code);
            assert_eq!(error.code(), symbol);
            assert_eq!(error.numeric_code(), Some(numeric));
        }
    }

    #[test]
    fn shared_exchange_failures_keep_their_exact_v1_codes() {
        let shared = [
            (ExchangeErrorCode::VersionUnsupported, 54_000),
            (ExchangeErrorCode::DigestMismatch, 54_001),
            (ExchangeErrorCode::DigestTreeMismatch, 54_002),
            (ExchangeErrorCode::CanonicalOrder, 54_003),
            (ExchangeErrorCode::DuplicateEntry, 54_004),
            (ExchangeErrorCode::AncestryOpen, 54_007),
            (ExchangeErrorCode::ResourceLimit, 54_015),
            (ExchangeErrorCode::CompressionUnsupported, 54_016),
        ];
        for (code, numeric) in shared {
            let error = NativeExchangeError::Exchange(ExchangeError::Exchange(code));
            assert_eq!(error.code(), code.as_str());
            assert_eq!(error.numeric_code(), Some(numeric));
        }
    }

    #[test]
    fn transport_profile_id_is_derived_and_fixed() {
        let profile = NativeExchangeProfileV1::fixed();
        assert_eq!(profile.id(), NativeExchangeProfileV1::fixed().id());
        NativeExchangeProfileV1::check_id(profile.id()).unwrap();
        let mut wrong = *profile.id().as_bytes();
        wrong[0] ^= 1;
        let error = NativeExchangeProfileV1::check_id(NativeExchangeProfileId::from_bytes(wrong))
            .unwrap_err();
        assert_eq!(error.code(), "NATIVE_TEST_PROFILE_UNSUPPORTED");
        assert_eq!(error.numeric_code(), Some(29_200));
        // The descriptor pins every Appendix B value in order.
        let mut expected = Vec::from(&b"SLEYNTP1"[..]);
        for field in [
            1_u64, 2, 65_536, 67_108_864, 4_096, 4_096, 1_052_672, 1_048_576,
        ] {
            expected.extend_from_slice(&encode_uvar(field));
        }
        assert_eq!(profile.id(), NativeExchangeProfileId::derive(expected));
    }

    #[test]
    fn chunk_boundaries_are_canonical() {
        // Empty receipts are not transportable.
        assert_eq!(
            chunk_native_receipt(&[]).unwrap_err().code(),
            "NATIVE_TEST_ENCODING_INVALID"
        );
        // One byte, exact chunk, and one-over populate 1, 1, and 2 chunks.
        assert_eq!(chunk_native_receipt(&[7]).unwrap(), vec![vec![7]]);
        let exact = vec![9_u8; NATIVE_CHUNK_BYTES];
        assert_eq!(chunk_native_receipt(&exact).unwrap(), vec![exact.clone()]);
        let over = vec![9_u8; NATIVE_CHUNK_BYTES + 1];
        let chunks = chunk_native_receipt(&over).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), NATIVE_CHUNK_BYTES);
        assert_eq!(chunks[1], vec![9_u8; 1]);
        assert_eq!(assemble_native_receipt(&chunks).unwrap(), over);
        // Nonfinal short chunks, empty finals, empty lists, and oversized
        // counts all refuse.
        assert_eq!(
            assemble_native_receipt(&[vec![1_u8; 10], vec![2_u8; 5]])
                .unwrap_err()
                .code(),
            "NATIVE_TEST_ENCODING_INVALID"
        );
        assert_eq!(
            assemble_native_receipt(&[vec![1_u8; NATIVE_CHUNK_BYTES], Vec::new()])
                .unwrap_err()
                .code(),
            "NATIVE_TEST_ENCODING_INVALID"
        );
        assert_eq!(
            assemble_native_receipt(&[]).unwrap_err().code(),
            "NATIVE_TEST_ENCODING_INVALID"
        );
        let too_many = vec![vec![3_u8; NATIVE_CHUNK_BYTES]; MAX_NATIVE_RECEIPT_CHUNKS + 1];
        assert_eq!(
            assemble_native_receipt(&too_many).unwrap_err().code(),
            "NATIVE_TEST_ENCODING_INVALID"
        );
        // A maximal canonical partition round-trips exactly.
        let maximal = vec![5_u8; NATIVE_CHUNK_BYTES * MAX_NATIVE_RECEIPT_CHUNKS];
        let chunks = chunk_native_receipt(&maximal).unwrap();
        assert_eq!(chunks.len(), MAX_NATIVE_RECEIPT_CHUNKS);
        assert_eq!(assemble_native_receipt(&chunks).unwrap(), maximal);
        // One byte past the maximum is not transportable.
        let oversized = vec![5_u8; NATIVE_CHUNK_BYTES * MAX_NATIVE_RECEIPT_CHUNKS + 1];
        assert_eq!(
            chunk_native_receipt(&oversized).unwrap_err().code(),
            "NATIVE_TEST_ENCODING_INVALID"
        );
    }

    #[test]
    fn trust_id_sets_require_strict_order() {
        assert_eq!(
            decode_trust_id_set(&encode_trust_id_set(&[])).unwrap(),
            Vec::<[u8; ID_LEN]>::new()
        );
        let sorted = [id(1), id(2), id(3)];
        assert_eq!(
            decode_trust_id_set(&encode_trust_id_set(&sorted)).unwrap(),
            sorted
        );
        let unsorted = [id(2), id(1)];
        assert_eq!(
            decode_trust_id_set(&encode_trust_id_set(&unsorted))
                .unwrap_err()
                .code(),
            "EXCHANGE_CANONICAL_ORDER"
        );
        let duplicated = [id(4), id(4)];
        assert_eq!(
            decode_trust_id_set(&encode_trust_id_set(&duplicated))
                .unwrap_err()
                .code(),
            "EXCHANGE_DUPLICATE_ENTRY"
        );
        let mut trailing = encode_trust_id_set(&sorted);
        trailing.push(0);
        assert!(decode_trust_id_set(&trailing).is_err());
    }

    fn receipt_entry(
        transaction: u8,
        receipt: u8,
        chunks: Vec<Vec<u8>>,
    ) -> NativeExchangeReceiptEntry {
        NativeExchangeReceiptEntry {
            transaction_id: TransactionId::from_bytes([transaction; ID_LEN]),
            receipt_id: ReceiptId::from_bytes([receipt; ID_LEN]),
            chunks,
        }
    }

    fn branch_entry(name: &[u8]) -> ExchangeBranchEntry {
        ExchangeBranchEntry {
            branch_name: name.to_vec(),
            stored_origin: vec![0x0a],
            stored_ref: vec![0x0b],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_record(
        tag1: u64,
        receipts: &[NativeExchangeReceiptEntry],
        head: ExchangeHeadEntry,
        trust: &[[u8; ID_LEN]],
        profile: NativeExchangeProfileV1,
        pack_id: sley_id::RepositoryPackId,
        receipt_list: &[Vec<u8>],
        trust_value: Vec<u8>,
        tree_value: Option<Vec<u8>>,
    ) -> Vec<u8> {
        let leaves =
            compute_native_leaves(pack_id, &[0xcc], receipts, head, &[], profile.id(), trust)
                .unwrap();
        let root = merkle_root(&leaves, MAX_NATIVE_EXCHANGE_LEAVES).unwrap();
        let leaf_elements = leaves.iter().map(|leaf| leaf.to_vec()).collect::<Vec<_>>();
        let tree = tree_value.unwrap_or_else(|| {
            encode_record(&[
                (1, encode_uvar(1)),
                (2, encode_uvar(leaves.len() as u64)),
                (3, encode_list(&leaf_elements).unwrap()),
                (4, root.to_vec()),
            ])
            .unwrap()
        });
        encode_record(&[
            (1, encode_uvar(tag1)),
            (2, vec![0xcc]),
            (3, encode_list(receipt_list).unwrap()),
            (
                4,
                encode_record(&[
                    (1, head.transaction_id.as_bytes().to_vec()),
                    (2, head.receipt_id.as_bytes().to_vec()),
                ])
                .unwrap(),
            ),
            (5, encode_list(&[]).unwrap()),
            (6, encode_uvar(0)),
            (7, tree),
            (8, encode_union(0, &[]).unwrap()),
            (9, trust_value),
            (10, profile.id().as_bytes().to_vec()),
        ])
        .unwrap()
    }

    struct RefusalFixture {
        profile: NativeExchangeProfileV1,
        receipts: Vec<NativeExchangeReceiptEntry>,
        head: ExchangeHeadEntry,
        trust: [[u8; ID_LEN]; 1],
        pack_id: sley_id::RepositoryPackId,
        good_element: Vec<u8>,
    }

    fn refusal_fixture() -> RefusalFixture {
        let profile = NativeExchangeProfileV1::fixed();
        let receipts = vec![receipt_entry(1, 10, vec![vec![1_u8; 4]])];
        let head = ExchangeHeadEntry {
            transaction_id: receipts[0].transaction_id,
            receipt_id: receipts[0].receipt_id,
        };
        let trust = [id(9)];
        let pack_id = sley_id::RepositoryPackId::from_bytes(id(11));
        let good_element = encode_record(&[
            (1, receipts[0].transaction_id.as_bytes().to_vec()),
            (2, receipts[0].receipt_id.as_bytes().to_vec()),
            (3, encode_list(&[encode_bytes(&[1_u8; 4])]).unwrap()),
        ])
        .unwrap();
        RefusalFixture {
            profile,
            receipts,
            head,
            trust,
            pack_id,
            good_element,
        }
    }

    fn decode_build(
        tag1: u64,
        receipt_list: &[Vec<u8>],
        trust_value: Vec<u8>,
        tree_value: Option<Vec<u8>>,
    ) -> String {
        let fixture = refusal_fixture();
        decode_native_payload(&build_record(
            tag1,
            &fixture.receipts,
            fixture.head,
            &fixture.trust,
            fixture.profile,
            fixture.pack_id,
            receipt_list,
            trust_value,
            tree_value,
        ))
        .unwrap_err()
        .code()
        .to_string()
    }

    #[test]
    fn record_round_trip_preserves_every_field() {
        let profile = NativeExchangeProfileV1::fixed();
        let receipts = vec![
            receipt_entry(1, 10, vec![vec![1_u8; 100]]),
            receipt_entry(2, 20, vec![vec![2_u8; NATIVE_CHUNK_BYTES], vec![3_u8; 7]]),
        ];
        let head = ExchangeHeadEntry {
            transaction_id: receipts[1].transaction_id,
            receipt_id: receipts[1].receipt_id,
        };
        let branches = vec![branch_entry(b"main")];
        let trust = [id(9), id(10)];
        let pack_id = sley_id::RepositoryPackId::from_bytes(id(11));
        let leaves = compute_native_leaves(
            pack_id,
            &[0xcc],
            &receipts,
            head,
            &branches,
            profile.id(),
            &trust,
        )
        .unwrap();
        assert_eq!(leaves.len(), 3 + receipts.len() + branches.len());
        let root = merkle_root(&leaves, MAX_NATIVE_EXCHANGE_LEAVES).unwrap();
        let payload = encode_native_payload(
            &[0xcc],
            &receipts,
            head,
            &branches,
            &leaves,
            root,
            &trust,
            profile.id(),
        )
        .unwrap();
        let decoded = decode_native_payload(&payload).unwrap();
        assert_eq!(decoded.object_pack, vec![0xcc]);
        assert_eq!(decoded.receipts, receipts);
        assert_eq!(decoded.accepted_head, head);
        assert_eq!(decoded.branches, branches);
        assert_eq!(decoded.required_trust_policy_ids, trust);
        assert_eq!(decoded.transport_profile_id, profile.id());
        assert_eq!(decoded.leaves, leaves);
        assert_eq!(decoded.digest_tree_root, root);
        // The receipt leaves cover reconstructed bytes, not chunks.
        assert_eq!(
            decoded.receipts[1].reconstructed().unwrap().len(),
            NATIVE_CHUNK_BYTES + 7
        );
        // Recomputing the leaves over decoded bytes reproduces the root.
        let recomputed = compute_native_leaves(
            pack_id,
            &decoded.object_pack,
            &decoded.receipts,
            decoded.accepted_head,
            &decoded.branches,
            decoded.transport_profile_id,
            &decoded.required_trust_policy_ids,
        )
        .unwrap();
        assert_eq!(
            merkle_root(&recomputed, MAX_NATIVE_EXCHANGE_LEAVES).unwrap(),
            root
        );
    }

    #[test]
    fn record_version_and_order_refusals_are_precise() {
        let fixture = refusal_fixture();
        let trust_set = encode_trust_id_set(&fixture.trust);
        // Tag 1 carries the v1 version.
        assert_eq!(
            decode_build(
                1,
                std::slice::from_ref(&fixture.good_element),
                trust_set.clone(),
                None
            ),
            "EXCHANGE_VERSION_UNSUPPORTED"
        );
        // Empty receipt history is open ancestry.
        assert_eq!(
            decode_build(2, &[], trust_set.clone(), None),
            "EXCHANGE_ANCESTRY_OPEN"
        );
        // Unordered and duplicated receipt elements refuse.
        let second = encode_record(&[
            (1, TransactionId::from_bytes(id(2)).as_bytes().to_vec()),
            (2, ReceiptId::from_bytes(id(20)).as_bytes().to_vec()),
            (3, encode_list(&[encode_bytes(&[2_u8; 4])]).unwrap()),
        ])
        .unwrap();
        assert_eq!(
            decode_build(
                2,
                &[second.clone(), fixture.good_element.clone()],
                trust_set.clone(),
                None
            ),
            "EXCHANGE_CANONICAL_ORDER"
        );
        assert_eq!(
            decode_build(
                2,
                &[fixture.good_element.clone(), fixture.good_element.clone()],
                trust_set.clone(),
                None
            ),
            "EXCHANGE_DUPLICATE_ENTRY"
        );
    }

    #[test]
    fn record_chunk_and_tree_refusals_are_precise() {
        let fixture = refusal_fixture();
        let trust_set = encode_trust_id_set(&fixture.trust);
        // Noncanonical chunks refuse at decode.
        let bad_chunks = encode_record(&[
            (1, fixture.receipts[0].transaction_id.as_bytes().to_vec()),
            (2, fixture.receipts[0].receipt_id.as_bytes().to_vec()),
            (
                3,
                encode_list(&[encode_bytes(&[1_u8; 4]), encode_bytes(&[2_u8; 4])]).unwrap(),
            ),
        ])
        .unwrap();
        assert_eq!(
            decode_build(2, &[bad_chunks], trust_set.clone(), None),
            "NATIVE_TEST_ENCODING_INVALID"
        );
        // A leaf count that disagrees with the entries refuses.
        let short_tree = encode_record(&[
            (1, encode_uvar(1)),
            (2, encode_uvar(4)),
            (
                3,
                encode_list(
                    &compute_native_leaves(
                        fixture.pack_id,
                        &[0xcc],
                        &fixture.receipts,
                        fixture.head,
                        &[],
                        fixture.profile.id(),
                        &fixture.trust,
                    )
                    .unwrap()[..3]
                        .iter()
                        .map(|leaf| leaf.to_vec())
                        .collect::<Vec<_>>(),
                )
                .unwrap(),
            ),
            (4, [0x55; ID_LEN].to_vec()),
        ])
        .unwrap();
        assert_eq!(
            decode_build(
                2,
                std::slice::from_ref(&fixture.good_element),
                trust_set.clone(),
                Some(short_tree)
            ),
            "EXCHANGE_DIGEST_TREE_MISMATCH"
        );
    }

    #[test]
    fn record_shape_refusals_precede_any_object_work() {
        let fixture = refusal_fixture();
        let intact = || {
            build_record(
                2,
                &fixture.receipts,
                fixture.head,
                &fixture.trust,
                fixture.profile,
                fixture.pack_id,
                std::slice::from_ref(&fixture.good_element),
                encode_trust_id_set(&fixture.trust),
                None,
            )
        };
        // Missing, surplus, and reordered fields refuse before any object work.
        let mut missing = intact();
        missing.pop();
        assert!(decode_native_payload(&missing).is_err());
        let mut surplus = intact();
        surplus.extend_from_slice(&[0x99]);
        assert!(decode_native_payload(&surplus).is_err());
    }

    #[test]
    fn mixed_history_export_round_trips_through_the_codecs() {
        let source = NativeSource::new("native-export-mixed");
        let exchange =
            export_native_exchange(&source.root, &verifier(source.epoch)).expect("export succeeds");
        // The closure holds genesis, the v1 commit, and the native commit
        // in canonical transaction-ID order under the native head.
        assert_eq!(exchange.receipts.len(), 3);
        let mut expected = vec![source.genesis, source.v1_head, source.head];
        expected.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        let ordered = exchange
            .receipts
            .iter()
            .map(|entry| entry.transaction_id)
            .collect::<Vec<_>>();
        assert_eq!(ordered, expected);
        assert_eq!(exchange.accepted_head.transaction_id, source.head);
        assert_eq!(exchange.branches.len(), 2);
        // The trust union is exactly the acceptance manifest: the committed
        // empty selection carries no measurement attestations.
        assert_eq!(
            exchange.required_trust_policy_ids,
            vec![source.acceptance_id]
        );
        // Every entry reassembles to the exact receipt file bytes.
        for entry in &exchange.receipts {
            assert_eq!(
                entry.reconstructed().unwrap(),
                receipt_file_bytes(&source.root, entry.transaction_id)
            );
        }
        // The envelope decodes to the same exchange ID and profile, and the
        // record decodes with a leaf set that recomputes to the same root.
        let (exchange_id, profile_id, payload) =
            decode_native_envelope(&exchange.stored_bytes).expect("envelope decodes");
        assert_eq!(exchange_id, exchange.exchange_id);
        assert_eq!(profile_id, NativeExchangeProfileV1::fixed().id());
        let decoded = decode_native_payload(&payload).expect("record decodes");
        assert_eq!(decoded.receipts, exchange.receipts);
        assert_eq!(
            decoded.required_trust_policy_ids,
            vec![source.acceptance_id]
        );
        let recomputed = compute_native_leaves(
            exchange.pack_id,
            &exchange.object_pack,
            &decoded.receipts,
            decoded.accepted_head,
            &decoded.branches,
            decoded.transport_profile_id,
            &decoded.required_trust_policy_ids,
        )
        .unwrap();
        assert_eq!(
            merkle_root(&recomputed, MAX_NATIVE_EXCHANGE_LEAVES).unwrap(),
            exchange.digest_tree_root
        );
        assert_eq!(decoded.digest_tree_root, exchange.digest_tree_root);
        let _ = source.target("unused");
    }

    #[test]
    fn native_export_is_byte_deterministic() {
        let source = NativeSource::new("native-export-deterministic");
        let first = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let second = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        assert_eq!(first.stored_bytes, second.stored_bytes);
        assert_eq!(first.exchange_id, second.exchange_id);
    }

    #[test]
    fn v1_history_exports_with_an_empty_trust_union() {
        let source = NativeSource::v1_only("native-export-v1-only");
        let exchange =
            export_native_exchange(&source.root, &verifier(source.epoch)).expect("export succeeds");
        assert_eq!(exchange.receipts.len(), 2);
        assert_eq!(exchange.accepted_head.transaction_id, source.v1_head);
        assert!(exchange.required_trust_policy_ids.is_empty());
        let (exchange_id, profile_id, payload) =
            decode_native_envelope(&exchange.stored_bytes).expect("envelope decodes");
        assert_eq!(exchange_id, exchange.exchange_id);
        assert_eq!(profile_id, NativeExchangeProfileV1::fixed().id());
        let decoded = decode_native_payload(&payload).unwrap();
        assert_eq!(decoded.receipts, exchange.receipts);
        assert!(decoded.required_trust_policy_ids.is_empty());
        for entry in &decoded.receipts {
            assert_eq!(
                entry.reconstructed().unwrap(),
                receipt_file_bytes(&source.root, entry.transaction_id)
            );
        }
    }

    fn branch_listing(root: &std::path::Path) -> Vec<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let mut listed = BranchRepository::new(root)
            .list_branches(crate::refs::MAX_BRANCHES)
            .unwrap()
            .into_iter()
            .map(|branch| {
                (
                    branch.origin.record.branch_name.as_bytes().to_vec(),
                    branch.origin.stored_bytes,
                    branch.reference.stored_bytes,
                )
            })
            .collect::<Vec<_>>();
        listed.sort();
        listed
    }

    #[test]
    fn counters_fail_before_exceeding_any_ceiling() {
        let mut counters = NativePreflightCounters::default();
        counters
            .add_signatures(MAX_NATIVE_SIGNATURE_CHECKS)
            .unwrap();
        assert_eq!(
            counters.add_signatures(1).unwrap_err().code(),
            "NATIVE_TEST_RESOURCE_LIMIT"
        );
        let mut counters = NativePreflightCounters::default();
        counters.add_visits(MAX_NATIVE_TEST_VISITS).unwrap();
        assert_eq!(
            counters.add_visits(1).unwrap_err().code(),
            "NATIVE_TEST_RESOURCE_LIMIT"
        );
        let mut counters = NativePreflightCounters::default();
        counters
            .add_evidence_bytes(MAX_NATIVE_EVIDENCE_BYTES)
            .unwrap();
        assert_eq!(
            counters.add_evidence_bytes(1).unwrap_err().code(),
            "NATIVE_TEST_RESOURCE_LIMIT"
        );
        // Overflow without wrapping also refuses.
        let mut counters = NativePreflightCounters::default();
        assert_eq!(
            counters.add_signatures(u64::MAX).unwrap_err().code(),
            "NATIVE_TEST_RESOURCE_LIMIT"
        );
    }

    #[test]
    fn mixed_preflight_charges_counters_without_writing() {
        let source = NativeSource::new("native-preflight-mixed");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let report = preflight_native_exchange(
            &exchange.stored_bytes,
            &verifier(source.epoch),
            &source.trust(),
        )
        .expect("mixed preflight succeeds");
        assert_eq!(report.exchange_id, exchange.exchange_id);
        assert_eq!(report.accepted_head, exchange.accepted_head);
        assert_eq!(report.receipts, 3);
        assert_eq!(report.branches, 2);
        assert_eq!(report.leaves, 3 + 3 + 2);
        // The committed empty selection carries no measurements: one
        // acceptance signature, zero visits, and exactly its receipt bytes.
        assert_eq!(report.signatures_checked, 1);
        assert_eq!(report.test_visits, 0);
        assert_eq!(
            report.evidence_bytes,
            receipt_file_bytes(&source.root, source.head).len() as u64
        );
    }

    #[test]
    fn native_import_is_clone_equivalent_and_reexports_identically() {
        let source = NativeSource::new("native-import-round-trip");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let target = source.target("clone");
        let report = import_native_exchange(
            &target,
            &exchange.stored_bytes,
            &verifier(source.epoch),
            &source.trust(),
        )
        .expect("native import succeeds");
        assert_eq!(report.exchange_id, exchange.exchange_id);
        assert_eq!(report.accepted_head, exchange.accepted_head);
        assert_eq!(report.receipts, 3);
        assert_eq!(report.branches, 2);
        // Receipt file bytes are identical on both sides.
        for entry in &exchange.receipts {
            assert_eq!(
                receipt_file_bytes(&target, entry.transaction_id),
                receipt_file_bytes(&source.root, entry.transaction_id)
            );
        }
        // Branches resolve to the same records on both sides.
        assert_eq!(branch_listing(&target), branch_listing(&source.root));
        // The accepted head verifies through the mixed loader.
        let transactions = TransactionRepository::new(&target);
        let maintenance = sley_txn::acquire_shared_repository_maintenance(&target).unwrap();
        let head = transactions
            .accepted_head_any_with_maintenance(&maintenance)
            .expect("cloned head loads");
        assert_eq!(head.transaction_id(), source.head);
        drop(maintenance);
        // A re-export of the clone is byte-identical: the clone is
        // exchange-closed with no surplus or missing state.
        let reexported = export_native_exchange(&target, &verifier(source.epoch)).unwrap();
        assert_eq!(reexported.stored_bytes, exchange.stored_bytes);
        assert_eq!(reexported.exchange_id, exchange.exchange_id);
        // The native marker is gone: a finished import leaves no resume
        // state behind.
        assert!(
            !target
                .join("exchange")
                .join(NATIVE_EXCHANGE_VERSION_DIRECTORY)
                .join(format!("{}.stage", hex_id(exchange.exchange_id.as_bytes())))
                .exists()
        );
    }

    #[test]
    fn post_head_crash_resumes_to_the_same_clone() {
        let source = NativeSource::new("native-import-resume");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let target = source.target("resumed");
        import_native_exchange(
            &target,
            &exchange.stored_bytes,
            &verifier(source.epoch),
            &source.trust(),
        )
        .expect("first import succeeds");
        // Simulate a crash after the head write before marker removal: the
        // complete clone with its marker restored must converge on retry.
        fs::create_dir_all(target.join("exchange").join("v2")).unwrap();
        fs::write(
            target
                .join("exchange")
                .join("v2")
                .join(format!("{}.stage", hex_id(exchange.exchange_id.as_bytes()))),
            exchange.exchange_id.as_bytes(),
        )
        .unwrap();
        let report = import_native_exchange(
            &target,
            &exchange.stored_bytes,
            &verifier(source.epoch),
            &source.trust(),
        )
        .expect("resumed import converges");
        assert_eq!(report.accepted_head, exchange.accepted_head);
        assert_eq!(report.receipts, 3);
        let reexported = export_native_exchange(&target, &verifier(source.epoch)).unwrap();
        assert_eq!(reexported.stored_bytes, exchange.stored_bytes);
    }

    #[test]
    fn nclone01_marker_temp_write_fails_retry_converges() {
        let source = NativeSource::new("native-clone-cut-01");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let target = source.target("cut01");
        let error = {
            let _selection = NativeCloneCutSelection::install(
                NativeCloneDurabilityCut::Nclone01DuringMarkerTempWrite,
            );
            import_native_exchange(
                &target,
                &exchange.stored_bytes,
                &verifier(source.epoch),
                &source.trust(),
            )
            .expect_err("marker temp-write cut must fail")
        };
        assert_eq!(error.code(), "EXCHANGE_IO");
        // Retry removes the torn temporary and converges to the same
        // clone an uninterrupted import would produce.
        let report = import_native_exchange(
            &target,
            &exchange.stored_bytes,
            &verifier(source.epoch),
            &source.trust(),
        )
        .expect("retry converges");
        assert_eq!(report.accepted_head, exchange.accepted_head);
        assert_eq!(report.receipts, 3);
        assert_eq!(report.branches, 2);
        let reexported = export_native_exchange(&target, &verifier(source.epoch)).unwrap();
        assert_eq!(reexported.stored_bytes, exchange.stored_bytes);
    }

    #[test]
    fn nclone02_marker_temp_before_rename_fails_retry_converges() {
        let source = NativeSource::new("native-clone-cut-02");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let target = source.target("cut02");
        let error = {
            let _selection = NativeCloneCutSelection::install(
                NativeCloneDurabilityCut::Nclone02VerifiedMarkerTempBeforeRename,
            );
            import_native_exchange(
                &target,
                &exchange.stored_bytes,
                &verifier(source.epoch),
                &source.trust(),
            )
            .expect_err("pre-rename cut must fail")
        };
        assert_eq!(error.code(), "EXCHANGE_IO");
        // No marker was promoted: the retry starts clean and converges.
        let report = import_native_exchange(
            &target,
            &exchange.stored_bytes,
            &verifier(source.epoch),
            &source.trust(),
        )
        .expect("retry converges");
        assert_eq!(report.accepted_head, exchange.accepted_head);
        assert_eq!(report.receipts, 3);
        let reexported = export_native_exchange(&target, &verifier(source.epoch)).unwrap();
        assert_eq!(reexported.stored_bytes, exchange.stored_bytes);
    }

    #[test]
    fn nclone03_marker_rename_before_sync_fails_retry_converges() {
        let source = NativeSource::new("native-clone-cut-03");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let target = source.target("cut03");
        let error = {
            let _selection = NativeCloneCutSelection::install(
                NativeCloneDurabilityCut::Nclone03MarkerRenameBeforeFirstSync,
            );
            import_native_exchange(
                &target,
                &exchange.stored_bytes,
                &verifier(source.epoch),
                &source.trust(),
            )
            .expect_err("post-rename cut must fail")
        };
        assert_eq!(error.code(), "EXCHANGE_IO");
        // The marker renamed but never synced: the retry reuses the
        // matching marker identity and converges to the same clone.
        let report = import_native_exchange(
            &target,
            &exchange.stored_bytes,
            &verifier(source.epoch),
            &source.trust(),
        )
        .expect("retry converges");
        assert_eq!(report.accepted_head, exchange.accepted_head);
        assert_eq!(report.receipts, 3);
        let reexported = export_native_exchange(&target, &verifier(source.epoch)).unwrap();
        assert_eq!(reexported.stored_bytes, exchange.stored_bytes);
    }

    #[test]
    fn reimport_of_a_finished_clone_is_not_empty() {
        let source = NativeSource::new("native-import-finished");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let target = source.target("finished");
        import_native_exchange(
            &target,
            &exchange.stored_bytes,
            &verifier(source.epoch),
            &source.trust(),
        )
        .expect("first import succeeds");
        // No marker remains, so the finished clone refuses as a non-empty
        // target rather than silently re-promoting.
        assert_eq!(
            import_native_exchange(
                &target,
                &exchange.stored_bytes,
                &verifier(source.epoch),
                &source.trust(),
            )
            .unwrap_err()
            .code(),
            "EXCHANGE_TARGET_NOT_EMPTY"
        );
    }

    #[test]
    fn import_without_manifests_refuses_before_any_write() {
        let source = NativeSource::new("native-import-no-trust");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let empty = NativeExchangeTrust { manifests: &[] };
        assert_eq!(
            preflight_native_exchange(&exchange.stored_bytes, &verifier(source.epoch), &empty)
                .unwrap_err()
                .code(),
            "NATIVE_TEST_HISTORICAL_TRUST_UNAVAILABLE"
        );
        let target = source.target("untouched");
        assert_eq!(
            import_native_exchange(
                &target,
                &exchange.stored_bytes,
                &verifier(source.epoch),
                &empty
            )
            .unwrap_err()
            .code(),
            "NATIVE_TEST_HISTORICAL_TRUST_UNAVAILABLE"
        );
        // Preflight precedes the marker: nothing was written.
        assert!(!target.exists());
    }

    #[test]
    fn import_with_an_unknown_key_is_unavailable_and_duplicates_tolerated() {
        let source = NativeSource::new("native-import-untrusted");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let workspace = fixed(1, WorkspaceId::from_bytes);
        // A manifest that never mentions the acceptance key is unavailable.
        let stranger = test_trust(
            [0xE1; 32],
            ROLE_ACCEPTANCE,
            workspace,
            *fixed_native_admission_profile().unwrap().id().as_bytes(),
        );
        let trust = NativeExchangeTrust {
            manifests: &[stranger],
        };
        assert_eq!(
            preflight_native_exchange(&exchange.stored_bytes, &verifier(source.epoch), &trust)
                .unwrap_err()
                .code(),
            "NATIVE_TEST_HISTORICAL_TRUST_UNAVAILABLE"
        );
        // A duplicated manifest resolves deterministically to the same
        // grant: the ID binds the full record, so same-ID manifests are
        // interchangeable.
        let trust = NativeExchangeTrust {
            manifests: &[
                source.trust_manifests[0].clone(),
                source.trust_manifests[1].clone(),
                source.trust_manifests[1].clone(),
            ],
        };
        let report =
            preflight_native_exchange(&exchange.stored_bytes, &verifier(source.epoch), &trust)
                .expect("duplicate manifests still preflight");
        assert_eq!(report.receipts, 3);
    }

    #[test]
    fn trust_checker_failures_map_to_the_reserved_symbols() {
        // The underlying structural checkers emit these variants at commit
        // time (N5b vectors); import maps them without collapsing.
        assert_eq!(
            map_trust_error(NativeCommitError::TrustUnavailable).code(),
            "NATIVE_TEST_HISTORICAL_TRUST_UNAVAILABLE"
        );
        assert_eq!(
            map_trust_error(NativeCommitError::TrustUnavailable).numeric_code(),
            Some(29_215)
        );
        assert_eq!(
            map_trust_error(NativeCommitError::TrustRejected).code(),
            "NATIVE_TEST_HISTORICAL_TRUST_REJECTED"
        );
        assert_eq!(
            map_trust_error(NativeCommitError::TrustRejected).numeric_code(),
            Some(29_216)
        );
    }

    /// Rebuilds an exchange with a substituted trust union, recomputing the
    /// transport leaf, tree, record, and envelope around it.
    fn rebuild_with_trust(
        exchange: &AcceptedNativeExchange,
        pack_id: sley_id::RepositoryPackId,
        trust: &[[u8; ID_LEN]],
    ) -> Vec<u8> {
        let (_, _, payload) = decode_native_envelope(&exchange.stored_bytes).unwrap();
        let decoded = decode_native_payload(&payload).unwrap();
        let profile = NativeExchangeProfileV1::fixed();
        let leaves = compute_native_leaves(
            pack_id,
            &decoded.object_pack,
            &decoded.receipts,
            decoded.accepted_head,
            &decoded.branches,
            profile.id(),
            trust,
        )
        .unwrap();
        let root = merkle_root(&leaves, MAX_NATIVE_EXCHANGE_LEAVES).unwrap();
        let payload = encode_native_payload(
            &decoded.object_pack,
            &decoded.receipts,
            decoded.accepted_head,
            &decoded.branches,
            &leaves,
            root,
            trust,
            profile.id(),
        )
        .unwrap();
        encode_native_envelope(&payload, profile.id()).unwrap().0
    }

    #[test]
    fn trust_union_exactness_is_checked_in_both_directions() {
        let source = NativeSource::new("native-import-union");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        // A declaration no receipt references is not a valid encoding.
        let surplus = vec![source.acceptance_id, [0xF1; 32]];
        let rebuilt = rebuild_with_trust(&exchange, exchange.pack_id, &surplus);
        assert_eq!(
            preflight_native_exchange(&rebuilt, &verifier(source.epoch), &source.trust())
                .unwrap_err()
                .code(),
            "NATIVE_TEST_ENCODING_INVALID"
        );
        // A referenced ID without a declaration is missing trust.
        let rebuilt = rebuild_with_trust(&exchange, exchange.pack_id, &[]);
        assert_eq!(
            preflight_native_exchange(&rebuilt, &verifier(source.epoch), &source.trust())
                .unwrap_err()
                .code(),
            "NATIVE_TEST_HISTORICAL_TRUST_UNAVAILABLE"
        );
    }

    #[test]
    fn tampered_receipt_bytes_fail_preflight_before_promotion() {
        let source = NativeSource::new("native-import-tamper");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let (_, _, payload) = decode_native_envelope(&exchange.stored_bytes).unwrap();
        let mut decoded = decode_native_payload(&payload).unwrap();
        // Flip one byte of the native receipt, then recompute the tree so
        // the failure lands on the receipt itself rather than the digest.
        let native = decoded
            .receipts
            .iter_mut()
            .find(|entry| entry.transaction_id == source.head)
            .expect("native receipt present");
        native.chunks[0][0] ^= 1;
        let profile = NativeExchangeProfileV1::fixed();
        let leaves = compute_native_leaves(
            exchange.pack_id,
            &decoded.object_pack,
            &decoded.receipts,
            decoded.accepted_head,
            &decoded.branches,
            profile.id(),
            &decoded.required_trust_policy_ids,
        )
        .unwrap();
        let root = merkle_root(&leaves, MAX_NATIVE_EXCHANGE_LEAVES).unwrap();
        let payload = encode_native_payload(
            &decoded.object_pack,
            &decoded.receipts,
            decoded.accepted_head,
            &decoded.branches,
            &leaves,
            root,
            &decoded.required_trust_policy_ids,
            profile.id(),
        )
        .unwrap();
        let (tampered, _) = encode_native_envelope(&payload, profile.id()).unwrap();
        assert_eq!(
            preflight_native_exchange(&tampered, &verifier(source.epoch), &source.trust())
                .unwrap_err()
                .code(),
            "EXCHANGE_RECEIPT_INVALID"
        );
        let target = source.target("untampered");
        assert_eq!(
            import_native_exchange(&target, &tampered, &verifier(source.epoch), &source.trust())
                .unwrap_err()
                .code(),
            "EXCHANGE_RECEIPT_INVALID"
        );
        assert!(!target.exists());
    }

    #[test]
    fn cloned_native_history_recovers_without_secrets() {
        let source = NativeSource::new("native-import-recover");
        let exchange = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let target = source.target("recovered");
        import_native_exchange(
            &target,
            &exchange.stored_bytes,
            &verifier(source.epoch),
            &source.trust(),
        )
        .expect("import succeeds");
        // Recovery proves the accepted state with no trust manifests, no
        // executor, and no secret export: the evidence is self-sufficient.
        let transactions = TransactionRepository::new(&target);
        let report = transactions.recover().expect("clone recovers");
        let maintenance = sley_txn::acquire_shared_repository_maintenance(&target).unwrap();
        let head = transactions
            .accepted_head_any_with_maintenance(&maintenance)
            .expect("recovered head loads");
        assert_eq!(head.transaction_id(), source.head);
        drop((maintenance, report));
    }

    #[test]
    fn replay_status_tags_match_the_606_contract() {
        assert_eq!(NativeReplayStatus::Matched.tag(), 1);
        assert_eq!(NativeReplayStatus::Mismatch.tag(), 2);
        assert_eq!(NativeReplayStatus::InconclusiveResource.tag(), 3);
        assert_eq!(NativeReplayStatus::UntrustedHistory.tag(), 4);
    }

    #[test]
    fn replay_pairing_compares_entities_not_positions() {
        let left = vec![(EntityId::from_bytes([1; 32]), vec![0x0a])];
        assert!(replay_executions_match(&left, &left));
        assert!(!replay_executions_match(&left, &[]));
        assert!(!replay_executions_match(&[], &left));
        let divergent = vec![(EntityId::from_bytes([1; 32]), vec![0x0b])];
        assert!(!replay_executions_match(&left, &divergent));
        let renamed = vec![(EntityId::from_bytes([2; 32]), vec![0x0a])];
        assert!(!replay_executions_match(&left, &renamed));
        // Order-independent: coverage already binds positions to the plan.
        let two = vec![
            (EntityId::from_bytes([1; 32]), vec![0x0a]),
            (EntityId::from_bytes([2; 32]), vec![0x0b]),
        ];
        let swapped = vec![
            (EntityId::from_bytes([2; 32]), vec![0x0b]),
            (EntityId::from_bytes([1; 32]), vec![0x0a]),
        ];
        assert!(replay_executions_match(&two, &swapped));
    }

    use sley_id::ObjectId as ReplayObjectId;
    use std::collections::BTreeMap as ReplayMap;

    /// Test-only replay executor reproducing the empty selection exactly.
    struct EmptyReplayExecutor;

    impl NativeTestExecutor for EmptyReplayExecutor {
        fn execute(
            &self,
            _plan: &sley_tests::NativeTestPlanV1,
            _validated: &ValidatedCandidatePlan,
        ) -> core::result::Result<Vec<ExecutedNativeTest>, NativeCommitError> {
            Ok(Vec::new())
        }

        fn execute_replay(
            &self,
            _plan: &sley_tests::NativeTestPlanV1,
            _objects: &ReplayMap<ReplayObjectId, &[u8]>,
        ) -> core::result::Result<Vec<ExecutedNativeTest>, NativeCommitError> {
            Ok(Vec::new())
        }
    }

    /// Test-only replay executor whose host run fails outright.
    struct FailingReplayExecutor;

    impl NativeTestExecutor for FailingReplayExecutor {
        fn execute(
            &self,
            _plan: &sley_tests::NativeTestPlanV1,
            _validated: &ValidatedCandidatePlan,
        ) -> core::result::Result<Vec<ExecutedNativeTest>, NativeCommitError> {
            Err(NativeCommitError::ExecutorUnavailable)
        }

        fn execute_replay(
            &self,
            _plan: &sley_tests::NativeTestPlanV1,
            _objects: &ReplayMap<ReplayObjectId, &[u8]>,
        ) -> core::result::Result<Vec<ExecutedNativeTest>, NativeCommitError> {
            Err(NativeCommitError::ExecutorUnavailable)
        }
    }
    fn committed_root_of(root: &std::path::Path, transaction: TransactionId) -> StateRoot {
        let transactions = TransactionRepository::new(root);
        let maintenance = sley_txn::acquire_shared_repository_maintenance(root).unwrap();
        let receipt = transactions
            .verified_revision_any_with_maintenance(&maintenance, transaction)
            .unwrap();
        receipt.committed_root()
    }

    /// Every regular file under a root as sorted relative path and length.
    fn file_tree(root: &std::path::Path) -> Vec<(String, u64)> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(path) = stack.pop() {
            for entry in fs::read_dir(&path).unwrap() {
                let entry = entry.unwrap();
                let file_type = entry.file_type().unwrap();
                if file_type.is_dir() {
                    stack.push(entry.path());
                } else {
                    out.push((
                        entry
                            .path()
                            .strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                        entry.metadata().unwrap().len(),
                    ));
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn replay_of_the_empty_selection_matches_without_writing() {
        let source = NativeSource::new("native-replay-matched");
        let before = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        let executor = EmptyReplayExecutor;
        let request = NativeReplayRequest {
            transaction_id: source.head,
            expected_root: committed_root_of(&source.root, source.head),
            executor: Some(&executor),
            trust: source.trust(),
        };
        let report = replay_native_commit(&source.root, &request, &verifier(source.epoch))
            .expect("replay runs");
        assert_eq!(report.transaction_id, source.head);
        assert_eq!(report.status, NativeReplayStatus::Matched);
        assert_eq!(report.status.tag(), 1);
        assert!(report.replay_report_id.is_none());
        // Replay writes nothing: the re-export is byte-identical and the
        // file tree is unchanged (the attempt journal below predates the
        // replay: the fixture's commit left it).
        let before_tree = file_tree(&source.root);
        let after = export_native_exchange(&source.root, &verifier(source.epoch)).unwrap();
        assert_eq!(after.stored_bytes, before.stored_bytes);
        assert_eq!(file_tree(&source.root), before_tree);
    }

    #[test]
    fn replay_without_an_executor_is_an_honest_refusal() {
        let source = NativeSource::new("native-replay-no-executor");
        let request = NativeReplayRequest {
            transaction_id: source.head,
            expected_root: committed_root_of(&source.root, source.head),
            executor: None,
            trust: source.trust(),
        };
        assert_eq!(
            replay_native_commit(&source.root, &request, &verifier(source.epoch))
                .unwrap_err()
                .code(),
            "NATIVE_TEST_ENFORCER_UNAVAILABLE"
        );
    }

    #[test]
    fn replay_refuses_a_format_1_transaction() {
        let source = NativeSource::new("native-replay-v1");
        let request = NativeReplayRequest {
            transaction_id: source.v1_head,
            expected_root: committed_root_of(&source.root, source.v1_head),
            executor: None,
            trust: source.trust(),
        };
        // Replay applies to native history only; a format-1 receipt is a
        // request error, never a history verdict.
        assert_eq!(
            replay_native_commit(&source.root, &request, &verifier(source.epoch))
                .unwrap_err()
                .code(),
            "EXCHANGE_RECEIPT_INVALID"
        );
    }

    #[test]
    fn replay_of_a_wrong_root_is_untrusted_history() {
        let source = NativeSource::new("native-replay-root");
        let executor = EmptyReplayExecutor;
        let request = NativeReplayRequest {
            transaction_id: source.head,
            expected_root: StateRoot::from_bytes([0xFF; 32]),
            executor: Some(&executor),
            trust: source.trust(),
        };
        let report = replay_native_commit(&source.root, &request, &verifier(source.epoch))
            .expect("replay reports");
        assert_eq!(report.status, NativeReplayStatus::UntrustedHistory);
        assert_eq!(report.status.tag(), 4);
    }

    #[test]
    fn replay_without_manifests_is_untrusted_history() {
        let source = NativeSource::new("native-replay-untrusted");
        let executor = EmptyReplayExecutor;
        let empty = NativeExchangeTrust { manifests: &[] };
        let request = NativeReplayRequest {
            transaction_id: source.head,
            expected_root: committed_root_of(&source.root, source.head),
            executor: Some(&executor),
            trust: empty,
        };
        let report = replay_native_commit(&source.root, &request, &verifier(source.epoch))
            .expect("replay reports");
        assert_eq!(report.status, NativeReplayStatus::UntrustedHistory);
    }

    #[test]
    fn replay_with_a_failing_host_is_inconclusive_not_mismatch() {
        let source = NativeSource::new("native-replay-inconclusive");
        let executor = FailingReplayExecutor;
        let request = NativeReplayRequest {
            transaction_id: source.head,
            expected_root: committed_root_of(&source.root, source.head),
            executor: Some(&executor),
            trust: source.trust(),
        };
        let report = replay_native_commit(&source.root, &request, &verifier(source.epoch))
            .expect("replay reports");
        // A host/executor failure is inconclusive, never a historical
        // failure or success verdict.
        assert_eq!(report.status, NativeReplayStatus::InconclusiveResource);
        assert_eq!(report.status.tag(), 3);
    }

    #[test]
    fn replay_with_a_commit_only_executor_is_inconclusive() {
        let source = NativeSource::new("native-replay-default-refusal");
        // CountingExecutor never implemented replay: the default refusal is
        // explicit and maps to inconclusive, proving the default is wired.
        let executor = CountingExecutor {
            invocations: Cell::new(0),
        };
        let request = NativeReplayRequest {
            transaction_id: source.head,
            expected_root: committed_root_of(&source.root, source.head),
            executor: Some(&executor),
            trust: source.trust(),
        };
        let report = replay_native_commit(&source.root, &request, &verifier(source.epoch))
            .expect("replay reports");
        assert_eq!(report.status, NativeReplayStatus::InconclusiveResource);
    }

    #[test]
    fn envelope_round_trip_binds_profile_and_trailer() {
        let profile = NativeExchangeProfileV1::fixed();
        let (bytes, exchange_id) = encode_native_envelope(&[0x01, 0x02], profile.id()).unwrap();
        assert!(bytes.starts_with(ENVELOPE_MAGIC));
        let (decoded_id, decoded_profile, payload) = decode_native_envelope(&bytes).unwrap();
        assert_eq!(decoded_id, exchange_id);
        assert_eq!(decoded_profile, profile.id());
        assert_eq!(payload, vec![0x01, 0x02]);
        // The exchange ID is the existing family over the disjoint preimage.
        let mut preimage = Vec::from(&ENVELOPE_MAGIC[..]);
        preimage.extend_from_slice(&encode_uvar(RECORD_VERSION));
        preimage.extend_from_slice(profile.id().as_bytes());
        preimage.extend_from_slice(&encode_uvar(2));
        preimage.extend_from_slice(&[0x01, 0x02]);
        assert_eq!(exchange_id, RepositoryExchangeId::derive(preimage));
        // A tampered trailer refuses as a digest mismatch.
        let mut tampered = bytes.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert_eq!(
            decode_native_envelope(&tampered).unwrap_err().code(),
            "EXCHANGE_DIGEST_MISMATCH"
        );
        // A wrong profile refuses even before any payload is trusted.
        let mut wrong = *profile.id().as_bytes();
        wrong[5] ^= 1;
        let wrong_id = NativeExchangeProfileId::from_bytes(wrong);
        assert_eq!(
            encode_native_envelope(&[0x01], wrong_id)
                .unwrap_err()
                .code(),
            "NATIVE_TEST_PROFILE_UNSUPPORTED"
        );
    }

    #[test]
    fn old_v1_bytes_refuse_on_the_disjoint_magic() {
        // A v1 SLEYSCB1 envelope is not a native exchange.
        let mut v1 = Vec::from(&b"SLEYSCB1"[..]);
        v1.extend_from_slice(&[0x01]);
        assert_eq!(
            decode_native_envelope(&v1).unwrap_err().code(),
            "SCB_MAGIC_INVALID"
        );
        // A native envelope is not a v1 exchange record either.
        let profile = NativeExchangeProfileV1::fixed();
        let (bytes, _) = encode_native_envelope(&[0x09], profile.id()).unwrap();
        assert_ne!(&bytes[..8], b"SLEYSCB1");
    }
}
