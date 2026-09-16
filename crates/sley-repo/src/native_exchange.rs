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
use std::path::Path;

use crate::refs::{BranchError, BranchRepository, MAX_BRANCHES, ResolvedBranch};
use sley_id::{NativeExchangeProfileId, ReceiptId, RepositoryExchangeId, TransactionId};
use sley_scb1::{ScbErrorCode, encode_list, encode_record, encode_union, encode_uvar};
use sley_store::{CanonicalVerifier, ObjectStore};
use sley_txn::{
    CommitError, ImportedReceipt, RepositoryMaintenanceGuard, TransactionCodecError,
    TransactionRepository, acquire_shared_repository_maintenance, native_receipt_trust_policy_ids,
};

use crate::exchange::{
    BRANCH_SECTION, ExchangeBranchEntry, ExchangeError, ExchangeErrorCode, ExchangeHeadEntry,
    HEAD_SECTION, MAX_EMBEDDED_PACK_BYTES, MAX_EXCHANGE_ALLOCATION, MAX_EXCHANGE_BRANCHES,
    MAX_EXCHANGE_BYTES, MAX_EXCHANGE_RECEIPTS, PACK_SECTION, RECEIPT_SECTION, branch_name_key,
    content_leaf, encode_branch_element, encode_bytes, merkle_root, stored_head_bytes,
};
use crate::{
    PackError, Reader, RecordReader, decode_absent_signature, decode_list, exact_array,
    export_conformance_pack, read_single_uvar, scb_error,
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
    ChunkInvalid,
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
    pub const ALL: [Self; 7] = [
        Self::ProfileMismatch,
        Self::ChunkInvalid,
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
            Self::ChunkInvalid => "NATIVE_TEST_ENCODING_INVALID",
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
            Self::ChunkInvalid => 29_201,
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
            NativeExchangeErrorCode::ChunkInvalid,
        ));
    }
    let count = bytes.len().div_ceil(NATIVE_CHUNK_BYTES);
    if count > MAX_NATIVE_RECEIPT_CHUNKS {
        return Err(NativeExchangeError::native(
            NativeExchangeErrorCode::ChunkInvalid,
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
            NativeExchangeErrorCode::ChunkInvalid,
        ));
    }
    let mut nonempty = false;
    for (index, chunk) in chunks.iter().enumerate() {
        let final_chunk = index + 1 == chunks.len();
        if final_chunk {
            if chunk.is_empty() || chunk.len() > NATIVE_CHUNK_BYTES {
                return Err(NativeExchangeError::native(
                    NativeExchangeErrorCode::ChunkInvalid,
                ));
            }
        } else if chunk.len() != NATIVE_CHUNK_BYTES {
            return Err(NativeExchangeError::native(
                NativeExchangeErrorCode::ChunkInvalid,
            ));
        }
        nonempty = nonempty || !chunk.is_empty();
    }
    if !nonempty {
        return Err(NativeExchangeError::native(
            NativeExchangeErrorCode::ChunkInvalid,
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
        CommitInput, ExecutedNativeTest, NativeAcceptanceSigner, NativeAttemptId,
        NativeCommitError, NativeCommitInput, NativeCommitOutcome, NativeTestExecutor,
        TransactionRepository, TrustedGenesisInput,
    };

    const NOW: u64 = 1_000;
    const MEASUREMENT_KEY: [u8; 32] = [0xB2; 32];
    const ACCEPTANCE_KEY: [u8; 32] = [0xA1; 32];

    struct TestSigner {
        key: [u8; 32],
    }

    impl NativeAcceptanceSigner for TestSigner {
        fn key_id(&self) -> [u8; 32] {
            self.key
        }

        fn sign(&self, _preimage: &[u8]) -> [u8; 64] {
            [0x5A; 64]
        }
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
                ACCEPTANCE_KEY,
                ROLE_ACCEPTANCE,
                workspace,
                *admission_profile.as_bytes(),
            );
            Self {
                measurement_trust,
                acceptance_trust,
                signer: TestSigner {
                    key: ACCEPTANCE_KEY,
                },
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
            Self {
                temp,
                root,
                epoch,
                genesis,
                v1_head,
                head: output.transaction_id(),
                acceptance_id,
            }
        }

        fn target(&self, name: &str) -> std::path::PathBuf {
            self.temp.child(name)
        }

        /// Exact receipt file bytes for one closure transaction, through the
        /// deterministic fanout layout both formats share.
        fn receipt_file_bytes(&self, transaction: TransactionId) -> Vec<u8> {
            let hex = const_hex(transaction.as_bytes());
            fs::read(
                self.root
                    .join("transactions")
                    .join("v1")
                    .join(&hex[0..2])
                    .join(&hex[2..4])
                    .join(format!("{hex}.receipt.scb1")),
            )
            .unwrap()
        }
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
                NativeExchangeErrorCode::ChunkInvalid,
                "NATIVE_TEST_ENCODING_INVALID",
                29_201,
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
                source.receipt_file_bytes(entry.transaction_id)
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
                source.receipt_file_bytes(entry.transaction_id)
            );
        }
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
