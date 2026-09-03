//! S20-540 Repository Exchange v1: clone-equivalent exchange of one complete
//! repository (`docs/spec/REPOSITORY_EXCHANGE_V1.md`, ADR-0025).
//!
//! An exchange embeds one exact S20-170 pack and adds the S20-390 receipts of
//! the exported ancestry, the fixed accepted head, and every visible S20-500
//! branch as an origin and ref pair. Import targets only a fresh location or a
//! marked incomplete clone of the same exchange, verifies everything before
//! any write, persists under exclusive maintenance ownership with the head
//! written last, and converges on retry.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sley_id::{
    ObjectId, ReceiptId, RepositoryExchangeId, RepositoryPackId, SchemaEpochId, StateRoot,
    TransactionId, WorkspaceId,
};
use sley_scb1::{ScbError, ScbErrorCode, encode_list, encode_record, encode_union, encode_uvar};
use sley_schema::{
    ContractDescriptor, EpochDecoder, EpochLimits, RegistryEntry, SchemaEpochRecordV1,
    SchemaEpochRegistry, UnicodeVersion,
};
use sley_state_root::AcceptedStateRoot;
use sley_store::{CanonicalVerifier, ObjectStore};
use sley_txn::{
    AcceptedHead, CommitError, ImportedTransactionReceipt, TransactionCodecError, TransactionKind,
    TransactionRepository, acquire_exclusive_repository_maintenance_nonblocking,
    acquire_shared_repository_maintenance, import_transaction_receipt,
    initialize_repository_maintenance, verify_receipt_against_objects,
};

use crate::refs::{
    BRANCH_STAGE_PREFIX, BranchError, BranchErrorCode, BranchName, BranchRepository,
    ImportedBranchRecord, ImportedBranchRef, MAX_BRANCHES, ensure_key_path, import_branch_record,
    import_branch_ref, persist_expected_ref, persist_no_overwrite, validate_origin_ref_binding,
};
use crate::{
    PackError, PackErrorCode, Reader, RecordReader, decode_absent_signature, decode_list,
    exact_array, export_conformance_pack, preflight_conformance_pack, promote_pack_objects,
    read_single_uvar, scb_error,
};

const MAGIC: &[u8; 8] = b"SLEYSCB1";
const PACK_CONTRACT_TAG: u64 = 170;
const FORMAT_VERSION: u64 = 1;
const CONTRACT_TAG: u32 = 540;
const DIGEST_DOMAIN_TAG: u32 = 19;
const KIND_TAG: u32 = 540;
const ID_LEN: usize = 32;
const TREE_ALGORITHM_TAG: u64 = 1;
const COMPRESSION_NONE: u64 = 0;
const PACK_SECTION: u64 = 1;
const RECEIPT_SECTION: u64 = 2;
const BRANCH_SECTION: u64 = 3;
const HEAD_SECTION: u64 = 4;
const LEAF_DOMAIN: &[u8] = b"sley2.repository-exchange-leaf.v1";
const NODE_DOMAIN: &[u8] = b"sley2.repository-exchange-node.v1";
const HEAD_MAGIC: &[u8; 8] = b"SLEYHD01";
const HEAD_DOMAIN: &[u8] = b"sley2.accepted-head.v1";
const HEAD_LEN: usize = 73;
const EXCHANGE_DIRECTORY: &str = "exchange";
const EXCHANGE_VERSION_DIRECTORY: &str = "v1";
const STAGE_SUFFIX: &str = ".stage";
const STAGE_TEMPORARY_SUFFIX: &str = ".stage.tmp";
const RECEIPT_SUFFIX: &str = ".receipt.scb1";
const ORIGIN_SUFFIX: &str = ".branch.scb1";
const REF_SUFFIX: &str = ".ref.scb1";

/// Maximum stored exchange bytes.
pub const MAX_EXCHANGE_BYTES: usize = 67_108_864;
/// Maximum embedded S20-170 pack bytes (the epoch-1 `Bytes` ceiling).
pub const MAX_EMBEDDED_PACK_BYTES: usize = 16_777_216;
/// Maximum receipt entries.
pub const MAX_EXCHANGE_RECEIPTS: usize = 4_096;
/// Maximum visible branch entries.
pub const MAX_EXCHANGE_BRANCHES: usize = 4_096;
/// Maximum digest leaves (`1 + 4,096 + 4,096 + 1`).
pub const MAX_EXCHANGE_LEAVES: usize = 8_194;
/// Shared decoder allocation budget.
pub const MAX_EXCHANGE_ALLOCATION: usize = 134_217_728;
/// Preflight ceiling on distinct object verifications.
pub const MAX_PREFLIGHT_OBJECT_VERIFICATIONS: u64 = 2_097_152;
/// Preflight ceiling on entity-binding visits.
pub const MAX_PREFLIGHT_BINDING_VISITS: u64 = 4_194_304;
/// Preflight ceiling on verified object bytes.
pub const MAX_PREFLIGHT_OBJECT_BYTES: u64 = 1_073_741_824;
/// Preflight ceiling on verified receipt bytes.
pub const MAX_PREFLIGHT_RECEIPT_BYTES: u64 = 1_073_741_824;

const FIELD_SCHEMA_HASH: [u8; ID_LEN] = [
    0xa8, 0x43, 0x40, 0x5b, 0xe5, 0xe3, 0x4d, 0x97, 0x9b, 0xb4, 0x88, 0x9b, 0x0e, 0x98, 0xc1, 0x52,
    0xdd, 0x01, 0xf8, 0x6d, 0x9c, 0x61, 0x4c, 0x1a, 0xfd, 0xa4, 0xcf, 0x88, 0xdd, 0x88, 0x4e, 0x2c,
];
const DECODER_LIMITS_HASH: [u8; ID_LEN] = [
    0x80, 0x8e, 0xab, 0xa9, 0x36, 0xf0, 0x9b, 0x2a, 0x93, 0x83, 0x06, 0xc5, 0x38, 0xe0, 0xdf, 0xf6,
    0x36, 0xa1, 0xa6, 0xd2, 0x61, 0x7e, 0xd0, 0xb4, 0x29, 0x8d, 0x92, 0x98, 0x91, 0xdb, 0x9d, 0x09,
];

/// Stable repository-exchange failure code (`54000` through `54021`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExchangeErrorCode {
    /// `EXCHANGE_VERSION_UNSUPPORTED`.
    VersionUnsupported,
    /// `EXCHANGE_DIGEST_MISMATCH`.
    DigestMismatch,
    /// `EXCHANGE_DIGEST_TREE_MISMATCH`.
    DigestTreeMismatch,
    /// `EXCHANGE_CANONICAL_ORDER`.
    CanonicalOrder,
    /// `EXCHANGE_DUPLICATE_ENTRY`.
    DuplicateEntry,
    /// `EXCHANGE_PACK_INVALID`.
    PackInvalid,
    /// `EXCHANGE_RECEIPT_INVALID`.
    ReceiptInvalid,
    /// `EXCHANGE_ANCESTRY_OPEN`.
    AncestryOpen,
    /// `EXCHANGE_ANCESTRY_CYCLE`.
    AncestryCycle,
    /// `EXCHANGE_ANCESTRY_SURPLUS`.
    AncestrySurplus,
    /// `EXCHANGE_HEAD_INVALID`.
    HeadInvalid,
    /// `EXCHANGE_BRANCH_INVALID`.
    BranchInvalid,
    /// `EXCHANGE_BRANCH_NOT_FAST_FORWARD`.
    BranchNotFastForward,
    /// `EXCHANGE_TARGET_NOT_EMPTY`.
    TargetNotEmpty,
    /// `EXCHANGE_TARGET_INCOMPLETE_MISMATCH`.
    TargetIncompleteMismatch,
    /// `EXCHANGE_RESOURCE_LIMIT`.
    ResourceLimit,
    /// `EXCHANGE_COMPRESSION_UNSUPPORTED`.
    CompressionUnsupported,
    /// `EXCHANGE_PROFILE_UNSUPPORTED`.
    ProfileUnsupported,
    /// `EXCHANGE_IO`.
    Io,
    /// `EXCHANGE_INTERNAL_INVARIANT`.
    InternalInvariant,
    /// `EXCHANGE_ROOT_CLOSURE`.
    RootClosure,
    /// `EXCHANGE_WORKSPACE_MISMATCH`.
    WorkspaceMismatch,
}

impl ExchangeErrorCode {
    /// Every code in frozen numeric order.
    pub const ALL: [Self; 22] = [
        Self::VersionUnsupported,
        Self::DigestMismatch,
        Self::DigestTreeMismatch,
        Self::CanonicalOrder,
        Self::DuplicateEntry,
        Self::PackInvalid,
        Self::ReceiptInvalid,
        Self::AncestryOpen,
        Self::AncestryCycle,
        Self::AncestrySurplus,
        Self::HeadInvalid,
        Self::BranchInvalid,
        Self::BranchNotFastForward,
        Self::TargetNotEmpty,
        Self::TargetIncompleteMismatch,
        Self::ResourceLimit,
        Self::CompressionUnsupported,
        Self::ProfileUnsupported,
        Self::Io,
        Self::InternalInvariant,
        Self::RootClosure,
        Self::WorkspaceMismatch,
    ];

    /// Returns the exact stable symbol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VersionUnsupported => "EXCHANGE_VERSION_UNSUPPORTED",
            Self::DigestMismatch => "EXCHANGE_DIGEST_MISMATCH",
            Self::DigestTreeMismatch => "EXCHANGE_DIGEST_TREE_MISMATCH",
            Self::CanonicalOrder => "EXCHANGE_CANONICAL_ORDER",
            Self::DuplicateEntry => "EXCHANGE_DUPLICATE_ENTRY",
            Self::PackInvalid => "EXCHANGE_PACK_INVALID",
            Self::ReceiptInvalid => "EXCHANGE_RECEIPT_INVALID",
            Self::AncestryOpen => "EXCHANGE_ANCESTRY_OPEN",
            Self::AncestryCycle => "EXCHANGE_ANCESTRY_CYCLE",
            Self::AncestrySurplus => "EXCHANGE_ANCESTRY_SURPLUS",
            Self::HeadInvalid => "EXCHANGE_HEAD_INVALID",
            Self::BranchInvalid => "EXCHANGE_BRANCH_INVALID",
            Self::BranchNotFastForward => "EXCHANGE_BRANCH_NOT_FAST_FORWARD",
            Self::TargetNotEmpty => "EXCHANGE_TARGET_NOT_EMPTY",
            Self::TargetIncompleteMismatch => "EXCHANGE_TARGET_INCOMPLETE_MISMATCH",
            Self::ResourceLimit => "EXCHANGE_RESOURCE_LIMIT",
            Self::CompressionUnsupported => "EXCHANGE_COMPRESSION_UNSUPPORTED",
            Self::ProfileUnsupported => "EXCHANGE_PROFILE_UNSUPPORTED",
            Self::Io => "EXCHANGE_IO",
            Self::InternalInvariant => "EXCHANGE_INTERNAL_INVARIANT",
            Self::RootClosure => "EXCHANGE_ROOT_CLOSURE",
            Self::WorkspaceMismatch => "EXCHANGE_WORKSPACE_MISMATCH",
        }
    }

    /// Returns the frozen numeric code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::VersionUnsupported => 54_000,
            Self::DigestMismatch => 54_001,
            Self::DigestTreeMismatch => 54_002,
            Self::CanonicalOrder => 54_003,
            Self::DuplicateEntry => 54_004,
            Self::PackInvalid => 54_005,
            Self::ReceiptInvalid => 54_006,
            Self::AncestryOpen => 54_007,
            Self::AncestryCycle => 54_008,
            Self::AncestrySurplus => 54_009,
            Self::HeadInvalid => 54_010,
            Self::BranchInvalid => 54_011,
            Self::BranchNotFastForward => 54_012,
            Self::TargetNotEmpty => 54_013,
            Self::TargetIncompleteMismatch => 54_014,
            Self::ResourceLimit => 54_015,
            Self::CompressionUnsupported => 54_016,
            Self::ProfileUnsupported => 54_017,
            Self::Io => 54_018,
            Self::InternalInvariant => 54_019,
            Self::RootClosure => 54_020,
            Self::WorkspaceMismatch => 54_021,
        }
    }
}

/// Exact repository-exchange failure preserving lower-layer codes.
#[derive(Debug)]
pub enum ExchangeError {
    /// S20-540-owned failure.
    Exchange(ExchangeErrorCode),
    /// Exact embedded-pack or object-store failure.
    Pack(PackError),
    /// Exact transaction, receipt, root, policy, or SCB1 failure.
    Transaction(CommitError),
    /// Exact branch-record or ref failure.
    Branch(BranchError),
    /// Local host I/O failure.
    Io(io::Error),
}

impl ExchangeError {
    const fn exchange(code: ExchangeErrorCode) -> Self {
        Self::Exchange(code)
    }

    /// Returns the exact stable source symbol without collapsing namespaces.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Exchange(code) => code.as_str(),
            Self::Pack(error) => error.symbol(),
            Self::Transaction(error) => error.code(),
            Self::Branch(error) => error.code(),
            Self::Io(_) => ExchangeErrorCode::Io.as_str(),
        }
    }

    /// Returns the owning numeric code when one is frozen.
    #[must_use]
    pub fn numeric_code(&self) -> Option<u32> {
        match self {
            Self::Exchange(code) => Some(code.numeric()),
            Self::Io(_) => Some(ExchangeErrorCode::Io.numeric()),
            Self::Transaction(error) => error.numeric_code(),
            Self::Branch(error) => error.numeric_code(),
            Self::Pack(_) => None,
        }
    }
}

impl fmt::Display for ExchangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for ExchangeError {}

impl From<PackError> for ExchangeError {
    fn from(error: PackError) -> Self {
        Self::Pack(error)
    }
}

impl From<CommitError> for ExchangeError {
    fn from(error: CommitError) -> Self {
        Self::Transaction(error)
    }
}

impl From<TransactionCodecError> for ExchangeError {
    fn from(error: TransactionCodecError) -> Self {
        Self::Transaction(CommitError::Codec(error))
    }
}

impl From<BranchError> for ExchangeError {
    fn from(error: BranchError) -> Self {
        Self::Branch(error)
    }
}

impl From<io::Error> for ExchangeError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

type Result<T> = core::result::Result<T, ExchangeError>;

/// One exported receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExchangeReceiptEntry {
    /// Declared transaction identity.
    pub transaction_id: TransactionId,
    /// Declared receipt identity.
    pub receipt_id: ReceiptId,
    /// Exact stored S20-390 receipt bytes.
    pub stored_bytes: Vec<u8>,
}

/// The exporter's fixed accepted head.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExchangeHeadEntry {
    /// Head transaction identity.
    pub transaction_id: TransactionId,
    /// Head receipt identity.
    pub receipt_id: ReceiptId,
}

/// One exported visible branch as its exact origin and ref records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExchangeBranchEntry {
    /// Exact canonical branch name bytes.
    pub branch_name: Vec<u8>,
    /// Exact stored S20-500 branch-origin record.
    pub stored_origin: Vec<u8>,
    /// Exact stored S20-500 branch-ref record.
    pub stored_ref: Vec<u8>,
}

/// Canonical accepted repository exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedRepositoryExchange {
    /// Derived exchange identifier.
    pub exchange_id: RepositoryExchangeId,
    /// Exact standalone exchange bytes.
    pub stored_bytes: Vec<u8>,
    /// Identifier of the embedded S20-170 pack.
    pub pack_id: RepositoryPackId,
    /// Exact embedded S20-170 pack bytes.
    pub object_pack: Vec<u8>,
    /// Exact ordered receipt entries.
    pub receipts: Vec<ExchangeReceiptEntry>,
    /// The accepted head.
    pub accepted_head: ExchangeHeadEntry,
    /// Exact ordered branch entries.
    pub branches: Vec<ExchangeBranchEntry>,
    /// Verified digest-tree root.
    pub digest_tree_root: [u8; ID_LEN],
}

/// Successful clone report.
#[derive(Debug)]
pub struct ExchangeImportReport {
    /// Imported exchange identifier.
    pub exchange_id: RepositoryExchangeId,
    /// Reconstructed accepted head.
    pub accepted_head: AcceptedHead,
    /// Receipts durable after the import.
    pub receipts: usize,
    /// Visible branches installed.
    pub branches: usize,
    /// Newly promoted object count.
    pub promoted_objects: usize,
    /// Already-present verified object count.
    pub present_objects: usize,
}

#[derive(Clone, Debug)]
struct DecodedExchange {
    object_pack: Vec<u8>,
    receipts: Vec<ExchangeReceiptEntry>,
    accepted_head: ExchangeHeadEntry,
    branches: Vec<ExchangeBranchEntry>,
    leaves: Vec<[u8; ID_LEN]>,
    digest_tree_root: [u8; ID_LEN],
}

/// Preserved decoder for the exact S20-540 exchange schema epoch.
#[derive(Clone, Debug)]
pub struct ExchangeEpoch1Decoder {
    epoch_id: SchemaEpochId,
}

impl EpochDecoder for ExchangeEpoch1Decoder {
    fn epoch_id(&self) -> SchemaEpochId {
        self.epoch_id
    }

    fn decode_contract(
        &self,
        contract_tag: u32,
        input: &[u8],
    ) -> core::result::Result<(), ScbError> {
        if contract_tag != CONTRACT_TAG {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        decode_payload(input).map(|_| ()).map_err(|error| {
            let code = match error.code() {
                "EXCHANGE_RESOURCE_LIMIT" => ScbErrorCode::ResourceLimit,
                "EXCHANGE_CANONICAL_ORDER" => ScbErrorCode::MapOrder,
                "EXCHANGE_VERSION_UNSUPPORTED" => ScbErrorCode::VersionUnsupported,
                _ => ScbErrorCode::FieldUnknown,
            };
            ScbError::new(code)
        })
    }
}

/// Returns the frozen exchange schema-epoch record.
#[must_use]
pub fn exchange_epoch_record() -> SchemaEpochRecordV1 {
    SchemaEpochRecordV1 {
        epoch_number: 1,
        scb_format_version: 1,
        hash_algorithm_tag: 1,
        unicode_nfc_version: UnicodeVersion::EPOCH_1,
        limits: EpochLimits::EPOCH_1,
        contracts: vec![ContractDescriptor {
            contract_tag: CONTRACT_TAG,
            digest_domain_tag: DIGEST_DOMAIN_TAG,
            kind_tag: KIND_TAG,
            field_schema_hash: FIELD_SCHEMA_HASH,
            required_fields: (1..=8).collect(),
            optional_fields: Vec::new(),
            variant_tags: Vec::new(),
            decoder_limits_hash: DECODER_LIMITS_HASH,
        }],
        extensions: Vec::new(),
        predecessor: None,
        migration_contracts: Vec::new(),
    }
}

/// Returns the exact exchange schema epoch ID.
///
/// # Errors
///
/// Returns the frozen schema-layer error if the descriptor record drifts.
pub fn exchange_epoch_id() -> Result<SchemaEpochId> {
    exchange_epoch_record()
        .schema_epoch_id()
        .map_err(|error| ExchangeError::Pack(PackError::upstream(error.code().as_str())))
}

fn exchange_registry() -> Result<SchemaEpochRegistry<ExchangeEpoch1Decoder>> {
    let record = exchange_epoch_record();
    let epoch_id = record
        .schema_epoch_id()
        .map_err(|error| ExchangeError::Pack(PackError::upstream(error.code().as_str())))?;
    let entry = RegistryEntry::new(epoch_id, record, ExchangeEpoch1Decoder { epoch_id })
        .map_err(|error| ExchangeError::Pack(PackError::upstream(error.code().as_str())))?;
    SchemaEpochRegistry::new(vec![entry])
        .map_err(|error| ExchangeError::Pack(PackError::upstream(error.code().as_str())))
}

fn exchange_error(code: ExchangeErrorCode) -> ExchangeError {
    ExchangeError::exchange(code)
}

fn encode_bytes(value: &[u8]) -> Vec<u8> {
    let mut out = encode_uvar(value.len() as u64);
    out.extend_from_slice(value);
    out
}

/// Computes the exact 73-byte S20-390 fixed-head value for a head.
fn stored_head_bytes(transaction_id: TransactionId) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(HEAD_LEN);
    prefix.extend_from_slice(HEAD_MAGIC);
    prefix.extend_from_slice(&encode_uvar(1));
    prefix.extend_from_slice(transaction_id.as_bytes());
    let mut hasher = blake3::Hasher::new();
    hasher.update(HEAD_DOMAIN);
    hasher.update(&prefix);
    prefix.extend_from_slice(hasher.finalize().as_bytes());
    prefix
}

fn decode_head_bytes(bytes: &[u8]) -> Option<TransactionId> {
    if bytes.len() != HEAD_LEN || &bytes[..8] != HEAD_MAGIC || bytes[8] != 1 {
        return None;
    }
    let transaction_id = TransactionId::from_bytes(bytes[9..41].try_into().ok()?);
    if stored_head_bytes(transaction_id) == bytes {
        Some(transaction_id)
    } else {
        None
    }
}

fn hex_id(bytes: &[u8; ID_LEN]) -> String {
    use fmt::Write as _;

    let mut output = String::with_capacity(ID_LEN * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap_or_default();
    }
    output
}

fn scb(error: &ScbError) -> ExchangeError {
    ExchangeError::Pack(scb_error(error))
}

fn pack_to_exchange(error: PackError) -> ExchangeError {
    ExchangeError::Pack(error)
}

fn encode_branch_element(entry: &ExchangeBranchEntry) -> Result<Vec<u8>> {
    encode_record(&[
        (1, entry.branch_name.clone()),
        (2, entry.stored_origin.clone()),
        (3, entry.stored_ref.clone()),
    ])
    .map_err(|error| scb(&error))
}

fn encode_payload(
    object_pack: &[u8],
    receipts: &[ExchangeReceiptEntry],
    accepted_head: ExchangeHeadEntry,
    branches: &[ExchangeBranchEntry],
    leaves: &[[u8; ID_LEN]],
    tree_root: [u8; ID_LEN],
) -> Result<Vec<u8>> {
    let receipt_elements = receipts
        .iter()
        .map(|entry| {
            encode_record(&[
                (1, entry.transaction_id.as_bytes().to_vec()),
                (2, entry.receipt_id.as_bytes().to_vec()),
                (3, entry.stored_bytes.clone()),
            ])
        })
        .collect::<core::result::Result<Vec<_>, _>>()
        .map_err(|error| scb(&error))?;
    let head = encode_record(&[
        (1, accepted_head.transaction_id.as_bytes().to_vec()),
        (2, accepted_head.receipt_id.as_bytes().to_vec()),
    ])
    .map_err(|error| scb(&error))?;
    let branch_elements = branches
        .iter()
        .map(encode_branch_element)
        .collect::<Result<Vec<_>>>()?;
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
        (1, encode_uvar(FORMAT_VERSION)),
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
    ])
    .map_err(|error| scb(&error))
}

fn stored_exchange_bytes(payload: &[u8]) -> Result<(Vec<u8>, RepositoryExchangeId)> {
    let epoch_id = exchange_epoch_id()?;
    let mut preimage = Vec::with_capacity(payload.len() + 96);
    preimage.extend_from_slice(MAGIC);
    preimage.extend_from_slice(&encode_uvar(FORMAT_VERSION));
    preimage.extend_from_slice(&encode_uvar(u64::from(CONTRACT_TAG)));
    preimage.extend_from_slice(epoch_id.as_bytes());
    preimage.extend_from_slice(&encode_uvar(payload.len() as u64));
    preimage.extend_from_slice(payload);
    let exchange_id = RepositoryExchangeId::derive(&preimage);
    preimage.extend_from_slice(exchange_id.as_bytes());
    if preimage.len() > MAX_EXCHANGE_BYTES {
        return Err(exchange_error(ExchangeErrorCode::ResourceLimit));
    }
    Ok((preimage, exchange_id))
}

fn decode_envelope(input: &[u8]) -> Result<(SchemaEpochId, &[u8], RepositoryExchangeId)> {
    if input.len() > MAX_EXCHANGE_BYTES {
        return Err(exchange_error(ExchangeErrorCode::ResourceLimit));
    }
    let mut reader = Reader::new(input);
    if reader.take_exact(MAGIC.len()).map_err(pack_to_exchange)? != MAGIC {
        return Err(ExchangeError::Pack(PackError::upstream(
            ScbErrorCode::MagicInvalid.as_str(),
        )));
    }
    if reader.read_uvar().map_err(pack_to_exchange)? != FORMAT_VERSION {
        return Err(exchange_error(ExchangeErrorCode::VersionUnsupported));
    }
    if reader.read_uvar().map_err(pack_to_exchange)? != u64::from(CONTRACT_TAG) {
        return Err(ExchangeError::Pack(PackError::upstream(
            ScbErrorCode::ContractUnknown.as_str(),
        )));
    }
    let epoch_id = SchemaEpochId::from_bytes(reader.take_array().map_err(pack_to_exchange)?);
    let payload_len = reader
        .read_len(MAX_EXCHANGE_BYTES)
        .map_err(pack_to_exchange)?;
    let payload = reader.take_exact(payload_len).map_err(pack_to_exchange)?;
    let trailer = reader.take_array::<ID_LEN>().map_err(pack_to_exchange)?;
    if !reader.is_finished() {
        return Err(ExchangeError::Pack(PackError::upstream(
            ScbErrorCode::TrailingBytes.as_str(),
        )));
    }
    let exchange_id = RepositoryExchangeId::derive(&input[..input.len() - ID_LEN]);
    if trailer != *exchange_id.as_bytes() {
        return Err(exchange_error(ExchangeErrorCode::DigestMismatch));
    }
    Ok((epoch_id, payload, exchange_id))
}

fn decode_receipt_entries(input: &[u8]) -> Result<Vec<ExchangeReceiptEntry>> {
    let elements = decode_list(input, MAX_EXCHANGE_RECEIPTS).map_err(pack_to_exchange)?;
    let mut entries = Vec::with_capacity(elements.len());
    for element in elements {
        let mut record = RecordReader::new(element).map_err(pack_to_exchange)?;
        let transaction_id = TransactionId::from_bytes(
            exact_array(record.required(1).map_err(pack_to_exchange)?).map_err(pack_to_exchange)?,
        );
        let receipt_id = ReceiptId::from_bytes(
            exact_array(record.required(2).map_err(pack_to_exchange)?).map_err(pack_to_exchange)?,
        );
        let stored_bytes = record.required(3).map_err(pack_to_exchange)?.to_vec();
        record.finish().map_err(pack_to_exchange)?;
        entries.push(ExchangeReceiptEntry {
            transaction_id,
            receipt_id,
            stored_bytes,
        });
    }
    let mut previous: Option<TransactionId> = None;
    for entry in &entries {
        if let Some(before) = previous {
            if before == entry.transaction_id {
                return Err(exchange_error(ExchangeErrorCode::DuplicateEntry));
            }
            if before.as_bytes() > entry.transaction_id.as_bytes() {
                return Err(exchange_error(ExchangeErrorCode::CanonicalOrder));
            }
        }
        previous = Some(entry.transaction_id);
    }
    Ok(entries)
}

fn decode_head_entry(input: &[u8]) -> Result<ExchangeHeadEntry> {
    let mut record = RecordReader::new(input).map_err(pack_to_exchange)?;
    let transaction_id = TransactionId::from_bytes(
        exact_array(record.required(1).map_err(pack_to_exchange)?).map_err(pack_to_exchange)?,
    );
    let receipt_id = ReceiptId::from_bytes(
        exact_array(record.required(2).map_err(pack_to_exchange)?).map_err(pack_to_exchange)?,
    );
    record.finish().map_err(pack_to_exchange)?;
    Ok(ExchangeHeadEntry {
        transaction_id,
        receipt_id,
    })
}

fn decode_branch_entries(input: &[u8]) -> Result<Vec<ExchangeBranchEntry>> {
    let elements = decode_list(input, MAX_EXCHANGE_BRANCHES).map_err(pack_to_exchange)?;
    let mut entries = Vec::with_capacity(elements.len());
    let mut previous: Option<&[u8]> = None;
    for element in elements {
        if let Some(before) = previous {
            if before == element {
                return Err(exchange_error(ExchangeErrorCode::DuplicateEntry));
            }
            if before > element {
                return Err(exchange_error(ExchangeErrorCode::CanonicalOrder));
            }
        }
        previous = Some(element);
        let mut record = RecordReader::new(element).map_err(pack_to_exchange)?;
        let branch_name = record.required(1).map_err(pack_to_exchange)?.to_vec();
        let stored_origin = record.required(2).map_err(pack_to_exchange)?.to_vec();
        let stored_ref = record.required(3).map_err(pack_to_exchange)?.to_vec();
        record.finish().map_err(pack_to_exchange)?;
        entries.push(ExchangeBranchEntry {
            branch_name,
            stored_origin,
            stored_ref,
        });
    }
    Ok(entries)
}

fn decode_tree(input: &[u8]) -> Result<(Vec<[u8; ID_LEN]>, [u8; ID_LEN])> {
    let mut record = RecordReader::new(input).map_err(pack_to_exchange)?;
    if read_single_uvar(record.required(1).map_err(pack_to_exchange)?).map_err(pack_to_exchange)?
        != TREE_ALGORITHM_TAG
    {
        return Err(exchange_error(ExchangeErrorCode::DigestTreeMismatch));
    }
    let count = read_single_uvar(record.required(2).map_err(pack_to_exchange)?)
        .map_err(pack_to_exchange)?;
    let elements = decode_list(
        record.required(3).map_err(pack_to_exchange)?,
        MAX_EXCHANGE_LEAVES,
    )
    .map_err(pack_to_exchange)?;
    let root =
        exact_array(record.required(4).map_err(pack_to_exchange)?).map_err(pack_to_exchange)?;
    record.finish().map_err(pack_to_exchange)?;
    if count != elements.len() as u64 || elements.len() < 3 {
        return Err(exchange_error(ExchangeErrorCode::DigestTreeMismatch));
    }
    let leaves = elements
        .into_iter()
        .map(|element| exact_array(element).map_err(pack_to_exchange))
        .collect::<Result<Vec<_>>>()?;
    Ok((leaves, root))
}

fn decode_payload(input: &[u8]) -> Result<DecodedExchange> {
    let mut record = RecordReader::new(input).map_err(pack_to_exchange)?;
    let version_bytes = record.required(1).map_err(pack_to_exchange)?;
    let pack_bytes = record.required(2).map_err(pack_to_exchange)?;
    let receipts_bytes = record.required(3).map_err(pack_to_exchange)?;
    let head_bytes = record.required(4).map_err(pack_to_exchange)?;
    let branches_bytes = record.required(5).map_err(pack_to_exchange)?;
    let compression_bytes = record.required(6).map_err(pack_to_exchange)?;
    let tree_bytes = record.required(7).map_err(pack_to_exchange)?;
    let signature_bytes = record.required(8).map_err(pack_to_exchange)?;
    record.finish().map_err(pack_to_exchange)?;

    if read_single_uvar(version_bytes).map_err(pack_to_exchange)? != FORMAT_VERSION {
        return Err(exchange_error(ExchangeErrorCode::VersionUnsupported));
    }
    if pack_bytes.len() > MAX_EMBEDDED_PACK_BYTES {
        return Err(exchange_error(ExchangeErrorCode::ResourceLimit));
    }
    if read_single_uvar(compression_bytes).map_err(pack_to_exchange)? != COMPRESSION_NONE {
        return Err(exchange_error(ExchangeErrorCode::CompressionUnsupported));
    }
    decode_absent_signature(signature_bytes).map_err(|error| {
        if error.symbol() == PackErrorCode::ProfileUnsupported.as_str() {
            exchange_error(ExchangeErrorCode::ProfileUnsupported)
        } else {
            ExchangeError::Pack(error)
        }
    })?;
    let receipts = decode_receipt_entries(receipts_bytes)?;
    if receipts.is_empty() {
        return Err(exchange_error(ExchangeErrorCode::AncestryOpen));
    }
    let accepted_head = decode_head_entry(head_bytes)?;
    let branches = decode_branch_entries(branches_bytes)?;
    let (leaves, digest_tree_root) = decode_tree(tree_bytes)?;
    if leaves.len() != receipts.len() + branches.len() + 2 {
        return Err(exchange_error(ExchangeErrorCode::DigestTreeMismatch));
    }
    let expanded = pack_bytes
        .len()
        .checked_add(receipts.iter().map(|entry| entry.stored_bytes.len()).sum())
        .and_then(|total| {
            total.checked_add(
                branches
                    .iter()
                    .map(|entry| entry.stored_origin.len() + entry.stored_ref.len())
                    .sum(),
            )
        })
        .ok_or_else(|| exchange_error(ExchangeErrorCode::ResourceLimit))?;
    if expanded > MAX_EXCHANGE_ALLOCATION {
        return Err(exchange_error(ExchangeErrorCode::ResourceLimit));
    }
    Ok(DecodedExchange {
        object_pack: pack_bytes.to_vec(),
        receipts,
        accepted_head,
        branches,
        leaves,
        digest_tree_root,
    })
}

fn content_leaf(section: u64, id: &[u8; ID_LEN], bytes: &[u8]) -> [u8; ID_LEN] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(LEAF_DOMAIN);
    hasher.update(&encode_uvar(section));
    hasher.update(id);
    hasher.update(&encode_uvar(bytes.len() as u64));
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn branch_name_key(entry: &ExchangeBranchEntry) -> Result<[u8; ID_LEN]> {
    let name = BranchName::parse(&entry.branch_name)
        .map_err(|_| exchange_error(ExchangeErrorCode::BranchInvalid))?;
    Ok(name.path_key())
}

fn compute_leaves(
    pack_id: RepositoryPackId,
    object_pack: &[u8],
    receipts: &[ExchangeReceiptEntry],
    accepted_head: ExchangeHeadEntry,
    branches: &[ExchangeBranchEntry],
) -> Result<Vec<[u8; ID_LEN]>> {
    let count = receipts
        .len()
        .checked_add(branches.len())
        .and_then(|count| count.checked_add(2))
        .ok_or_else(|| exchange_error(ExchangeErrorCode::ResourceLimit))?;
    if count > MAX_EXCHANGE_LEAVES {
        return Err(exchange_error(ExchangeErrorCode::ResourceLimit));
    }
    let mut leaves = Vec::with_capacity(count);
    leaves.push(content_leaf(PACK_SECTION, pack_id.as_bytes(), object_pack));
    for entry in receipts {
        leaves.push(content_leaf(
            RECEIPT_SECTION,
            entry.transaction_id.as_bytes(),
            &entry.stored_bytes,
        ));
    }
    for entry in branches {
        let mut bytes = encode_bytes(&entry.stored_origin);
        bytes.extend_from_slice(&encode_bytes(&entry.stored_ref));
        leaves.push(content_leaf(
            BRANCH_SECTION,
            &branch_name_key(entry)?,
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
    Ok(leaves)
}

fn merkle_root(leaves: &[[u8; ID_LEN]]) -> Result<[u8; ID_LEN]> {
    if leaves.is_empty() || leaves.len() > MAX_EXCHANGE_LEAVES {
        return Err(exchange_error(ExchangeErrorCode::DigestTreeMismatch));
    }
    let mut level = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            if pair.len() == 1 {
                next.push(pair[0]);
            } else {
                let mut hasher = blake3::Hasher::new();
                hasher.update(NODE_DOMAIN);
                hasher.update(&pair[0]);
                hasher.update(&pair[1]);
                next.push(*hasher.finalize().as_bytes());
            }
        }
        level = next;
    }
    Ok(level[0])
}

fn embedded_pack_header_is_tag_170(bytes: &[u8]) -> bool {
    let mut reader = Reader::new(bytes);
    let Ok(magic) = reader.take_exact(MAGIC.len()) else {
        return false;
    };
    if magic != MAGIC {
        return false;
    }
    let Ok(version) = reader.read_uvar() else {
        return false;
    };
    let Ok(contract) = reader.read_uvar() else {
        return false;
    };
    version == FORMAT_VERSION && contract == PACK_CONTRACT_TAG
}

/// Exports a canonical S20-540 exchange from a complete local repository.
///
/// # Errors
///
/// Fails closed with `EXCHANGE_RESOURCE_LIMIT` beyond any clone-profile
/// ceiling and otherwise preserves the exact transaction, branch, pack, or
/// store failure.
pub fn export_repository_exchange<V: CanonicalVerifier>(
    root: &Path,
    verifier: &V,
) -> Result<AcceptedRepositoryExchange> {
    let maintenance = acquire_shared_repository_maintenance(root)?;
    let transactions = TransactionRepository::new(root);
    let head = transactions.accepted_head_with_maintenance(&maintenance)?;
    let branch_repository = BranchRepository::new(root);
    let visible = {
        let _refs_lock = branch_repository.acquire_refs_lock()?;
        branch_repository.list_branches_locked(&maintenance, MAX_BRANCHES)?
    };
    if visible.len() > MAX_EXCHANGE_BRANCHES {
        return Err(exchange_error(ExchangeErrorCode::ResourceLimit));
    }

    let mut receipts: BTreeMap<TransactionId, ImportedTransactionReceipt> = BTreeMap::new();
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
            return Err(exchange_error(ExchangeErrorCode::ResourceLimit));
        }
        let revision =
            transactions.verified_revision_with_maintenance(&maintenance, transaction_id)?;
        pending.extend(
            revision
                .receipt()
                .transaction
                .record
                .parent_transaction_ids
                .iter()
                .copied(),
        );
        receipts.insert(transaction_id, revision.receipt().clone());
    }

    let roots = receipts
        .values()
        .map(|receipt| receipt.state_root.clone())
        .collect::<Vec<AcceptedStateRoot>>();
    let store = ObjectStore::new(root);
    let pack = export_conformance_pack(&store, &roots, verifier)?;
    if pack.stored_bytes.len() > MAX_EMBEDDED_PACK_BYTES {
        return Err(exchange_error(ExchangeErrorCode::ResourceLimit));
    }

    let receipt_entries = receipts
        .iter()
        .map(|(transaction_id, receipt)| ExchangeReceiptEntry {
            transaction_id: *transaction_id,
            receipt_id: receipt.receipt_id,
            stored_bytes: receipt.stored_bytes.clone(),
        })
        .collect::<Vec<_>>();
    let accepted_head = ExchangeHeadEntry {
        transaction_id: head.transaction_id(),
        receipt_id: head.receipt().receipt_id,
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
        .collect::<Result<Vec<_>>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    keyed.dedup_by(|left, right| left.0 == right.0);
    let branch_entries = keyed
        .into_iter()
        .map(|(_, entry)| entry)
        .collect::<Vec<_>>();
    drop(maintenance);
    build_exchange(
        pack.pack_id,
        pack.stored_bytes,
        receipt_entries,
        accepted_head,
        branch_entries,
    )
}

fn build_exchange(
    pack_id: RepositoryPackId,
    object_pack: Vec<u8>,
    receipts: Vec<ExchangeReceiptEntry>,
    accepted_head: ExchangeHeadEntry,
    branches: Vec<ExchangeBranchEntry>,
) -> Result<AcceptedRepositoryExchange> {
    if receipts.is_empty()
        || receipts.len() > MAX_EXCHANGE_RECEIPTS
        || branches.len() > MAX_EXCHANGE_BRANCHES
    {
        return Err(exchange_error(ExchangeErrorCode::ResourceLimit));
    }
    let leaves = compute_leaves(pack_id, &object_pack, &receipts, accepted_head, &branches)?;
    let digest_tree_root = merkle_root(&leaves)?;
    let payload = encode_payload(
        &object_pack,
        &receipts,
        accepted_head,
        &branches,
        &leaves,
        digest_tree_root,
    )?;
    let (stored_bytes, exchange_id) = stored_exchange_bytes(&payload)?;
    Ok(AcceptedRepositoryExchange {
        exchange_id,
        stored_bytes,
        pack_id,
        object_pack,
        receipts,
        accepted_head,
        branches,
        digest_tree_root,
    })
}

struct VerifiedBranch {
    name: BranchName,
    origin: ImportedBranchRecord,
    reference: ImportedBranchRef,
}

struct Preflight {
    exchange_id: RepositoryExchangeId,
    decoded: DecodedExchange,
    pack: crate::PreflightedPack,
    receipts: BTreeMap<TransactionId, ImportedTransactionReceipt>,
    branches: Vec<VerifiedBranch>,
}

fn topological_order(
    receipts: &BTreeMap<TransactionId, ImportedTransactionReceipt>,
) -> Result<Vec<TransactionId>> {
    let mut order = Vec::with_capacity(receipts.len());
    let mut placed: BTreeSet<TransactionId> = BTreeSet::new();
    let mut pending: Vec<TransactionId> = receipts.keys().copied().collect();
    while !pending.is_empty() {
        let mut remaining = Vec::with_capacity(pending.len());
        let mut progressed = false;
        for transaction_id in pending {
            let ready = receipts[&transaction_id]
                .transaction
                .record
                .parent_transaction_ids
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
            return Err(exchange_error(ExchangeErrorCode::AncestryCycle));
        }
    }
    Ok(order)
}

fn verify_closure_rules(
    decoded: &DecodedExchange,
    receipts: &BTreeMap<TransactionId, ImportedTransactionReceipt>,
    pack_roots: &[AcceptedStateRoot],
) -> Result<()> {
    let mut genesis = 0_usize;
    let mut workspace: Option<WorkspaceId> = None;
    for receipt in receipts.values() {
        let record = &receipt.transaction.record;
        for parent in &record.parent_transaction_ids {
            if !receipts.contains_key(parent) {
                return Err(exchange_error(ExchangeErrorCode::AncestryOpen));
            }
        }
        match record.transaction_kind {
            TransactionKind::TrustedGenesis => {
                if !record.parent_transaction_ids.is_empty() {
                    return Err(exchange_error(ExchangeErrorCode::AncestryOpen));
                }
                genesis += 1;
            }
            TransactionKind::OrdinaryCandidate => {
                if record.parent_transaction_ids.len() != 1 || record.parent_roots.len() != 1 {
                    return Err(exchange_error(ExchangeErrorCode::AncestryOpen));
                }
            }
        }
        match workspace {
            None => workspace = Some(record.workspace_id),
            Some(expected) if expected == record.workspace_id => {}
            Some(_) => return Err(exchange_error(ExchangeErrorCode::WorkspaceMismatch)),
        }
    }
    if genesis != 1 {
        return Err(exchange_error(ExchangeErrorCode::AncestryOpen));
    }
    topological_order(receipts)?;

    let head = decoded.accepted_head;
    match receipts.get(&head.transaction_id) {
        Some(receipt) if receipt.receipt_id == head.receipt_id => {}
        _ => return Err(exchange_error(ExchangeErrorCode::HeadInvalid)),
    }

    let committed: BTreeSet<StateRoot> = receipts
        .values()
        .map(|receipt| receipt.transaction.record.committed_root)
        .collect();
    let by_root: BTreeMap<StateRoot, &AcceptedStateRoot> =
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
        return Err(exchange_error(ExchangeErrorCode::RootClosure));
    }
    Ok(())
}

fn fast_forward_reachable(
    receipts: &BTreeMap<TransactionId, ImportedTransactionReceipt>,
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
        cursor = receipts.get(&transaction_id).and_then(|receipt| {
            receipt
                .transaction
                .record
                .parent_transaction_ids
                .first()
                .copied()
        });
    }
    false
}

fn verify_branch_entry(
    entry: &ExchangeBranchEntry,
    receipts: &BTreeMap<TransactionId, ImportedTransactionReceipt>,
    workspace: WorkspaceId,
) -> Result<VerifiedBranch> {
    let name = BranchName::parse(&entry.branch_name)
        .map_err(|_| exchange_error(ExchangeErrorCode::BranchInvalid))?;
    let origin = import_branch_record(&entry.stored_origin)?;
    let reference = import_branch_ref(&entry.stored_ref)?;
    if origin.record.branch_name != name || reference.record.branch_name != name {
        return Err(exchange_error(ExchangeErrorCode::BranchInvalid));
    }
    validate_origin_ref_binding(&origin, &reference)
        .map_err(|_| exchange_error(ExchangeErrorCode::BranchInvalid))?;
    let origin_receipt = receipts
        .get(&origin.record.origin_transaction_id)
        .ok_or_else(|| exchange_error(ExchangeErrorCode::BranchInvalid))?;
    let head_receipt = receipts
        .get(&reference.record.head_transaction_id)
        .ok_or_else(|| exchange_error(ExchangeErrorCode::BranchInvalid))?;
    if origin.record.workspace_id != workspace || reference.record.workspace_id != workspace {
        return Err(exchange_error(ExchangeErrorCode::WorkspaceMismatch));
    }
    let origin_facts_match = origin.record.origin_state_root == origin_receipt.state_root.root
        && origin.record.schema_epoch_id == origin_receipt.state_root.record.schema_epoch_id
        && origin.record.policy_root_id == origin_receipt.policy_root.root()
        && origin.record.dependency_roots == origin_receipt.state_root.record.dependency_roots;
    let ref_facts_match = reference.record.head_state_root == head_receipt.state_root.root
        && reference.record.schema_epoch_id == head_receipt.state_root.record.schema_epoch_id
        && reference.record.policy_root_id == head_receipt.policy_root.root()
        && reference.record.dependency_roots == head_receipt.state_root.record.dependency_roots;
    if !origin_facts_match || !ref_facts_match {
        return Err(exchange_error(ExchangeErrorCode::BranchInvalid));
    }
    if !fast_forward_reachable(
        receipts,
        origin.record.origin_transaction_id,
        reference.record.head_transaction_id,
    ) {
        return Err(exchange_error(ExchangeErrorCode::BranchNotFastForward));
    }
    Ok(VerifiedBranch {
        name,
        origin,
        reference,
    })
}

fn verify_no_surplus(
    receipts: &BTreeMap<TransactionId, ImportedTransactionReceipt>,
    head: TransactionId,
    branches: &[VerifiedBranch],
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
            pending.extend(
                receipt
                    .transaction
                    .record
                    .parent_transaction_ids
                    .iter()
                    .copied(),
            );
        }
    }
    if reachable.len() != receipts.len() {
        return Err(exchange_error(ExchangeErrorCode::AncestrySurplus));
    }
    Ok(())
}

fn verify_receipts_against_pack(
    order: &[TransactionId],
    receipts: &BTreeMap<TransactionId, ImportedTransactionReceipt>,
    pack: &crate::PreflightedPack,
) -> Result<()> {
    let objects = pack
        .decoded
        .objects
        .iter()
        .map(|object| (object.object_id, object.stored_bytes.as_slice()))
        .collect::<Vec<(ObjectId, &[u8])>>();
    let mut binding_visits = 0_u64;
    let mut receipt_bytes = 0_u64;
    let mut verified_objects: BTreeSet<ObjectId> = BTreeSet::new();
    let mut object_bytes = 0_u64;
    for transaction_id in order {
        let receipt = &receipts[transaction_id];
        receipt_bytes = receipt_bytes
            .checked_add(receipt.stored_bytes.len() as u64)
            .filter(|total| *total <= MAX_PREFLIGHT_RECEIPT_BYTES)
            .ok_or_else(|| exchange_error(ExchangeErrorCode::ResourceLimit))?;
        binding_visits = binding_visits
            .checked_add(receipt.state_root.record.entity_bindings.len() as u64)
            .filter(|total| *total <= MAX_PREFLIGHT_BINDING_VISITS)
            .ok_or_else(|| exchange_error(ExchangeErrorCode::ResourceLimit))?;
        for (_, object_id) in &receipt.state_root.record.entity_bindings {
            if verified_objects.insert(*object_id) {
                if verified_objects.len() as u64 > MAX_PREFLIGHT_OBJECT_VERIFICATIONS {
                    return Err(exchange_error(ExchangeErrorCode::ResourceLimit));
                }
                let length = pack
                    .decoded
                    .objects
                    .iter()
                    .find(|object| object.object_id == *object_id)
                    .map_or(0, |object| object.stored_bytes.len() as u64);
                object_bytes = object_bytes
                    .checked_add(length)
                    .filter(|total| *total <= MAX_PREFLIGHT_OBJECT_BYTES)
                    .ok_or_else(|| exchange_error(ExchangeErrorCode::ResourceLimit))?;
            }
        }
        let parent = receipt
            .transaction
            .record
            .parent_transaction_ids
            .first()
            .and_then(|parent| receipts.get(parent));
        verify_receipt_against_objects(receipt, parent, &objects)?;
    }
    Ok(())
}

fn preflight<V: CanonicalVerifier>(input: &[u8], verifier: &V) -> Result<Preflight> {
    let (epoch_id, payload, exchange_id) = decode_envelope(input)?;
    if epoch_id != exchange_epoch_id()? {
        return Err(ExchangeError::Pack(PackError::upstream(
            ScbErrorCode::ContractUnknown.as_str(),
        )));
    }
    let decoded = decode_payload(payload)?;
    let registry = exchange_registry()?;
    registry
        .lookup_contract(epoch_id, CONTRACT_TAG)
        .map_err(|error| ExchangeError::Pack(PackError::upstream(error.code().as_str())))?;
    registry
        .decode_contract(epoch_id, CONTRACT_TAG, payload)
        .map_err(|error| match error {
            sley_schema::EpochDecodeError::Schema(schema) => {
                ExchangeError::Pack(PackError::upstream(schema.code().as_str()))
            }
            sley_schema::EpochDecodeError::Scb(scb) => {
                ExchangeError::Pack(PackError::upstream(scb.code().as_str()))
            }
        })?;

    if !embedded_pack_header_is_tag_170(&decoded.object_pack) {
        return Err(exchange_error(ExchangeErrorCode::PackInvalid));
    }
    let pack = preflight_conformance_pack(&decoded.object_pack, verifier)?;

    let expected_leaves = compute_leaves(
        pack.pack_id,
        &decoded.object_pack,
        &decoded.receipts,
        decoded.accepted_head,
        &decoded.branches,
    )?;
    if decoded.leaves != expected_leaves
        || merkle_root(&expected_leaves)? != decoded.digest_tree_root
    {
        return Err(exchange_error(ExchangeErrorCode::DigestTreeMismatch));
    }

    let mut receipts: BTreeMap<TransactionId, ImportedTransactionReceipt> = BTreeMap::new();
    for entry in &decoded.receipts {
        let receipt = import_transaction_receipt(&entry.stored_bytes)?;
        if receipt.transaction.transaction_id != entry.transaction_id
            || receipt.receipt_id != entry.receipt_id
        {
            return Err(exchange_error(ExchangeErrorCode::ReceiptInvalid));
        }
        receipts.insert(entry.transaction_id, receipt);
    }
    verify_closure_rules(&decoded, &receipts, &pack.roots)?;
    let workspace = receipts
        .values()
        .next()
        .map(|receipt| receipt.transaction.record.workspace_id)
        .ok_or_else(|| exchange_error(ExchangeErrorCode::AncestryOpen))?;
    let branches = decoded
        .branches
        .iter()
        .map(|entry| verify_branch_entry(entry, &receipts, workspace))
        .collect::<Result<Vec<_>>>()?;
    verify_no_surplus(&receipts, decoded.accepted_head.transaction_id, &branches)?;
    let order = topological_order(&receipts)?;
    verify_receipts_against_pack(&order, &receipts, &pack)?;
    Ok(Preflight {
        exchange_id,
        decoded,
        pack,
        receipts,
        branches,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Target {
    Fresh,
    IncompleteClone,
}

fn real_directory_metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(exchange_error(ExchangeErrorCode::Io));
            }
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn read_regular_file(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(exchange_error(ExchangeErrorCode::Io));
    }
    if metadata.len() > maximum as u64 {
        return Err(exchange_error(ExchangeErrorCode::Io));
    }
    Ok(fs::read(path)?)
}

fn collect_files_with_suffix(root: &Path, suffix: &str) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        if real_directory_metadata(&directory)?.is_none() {
            continue;
        }
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let path = entry.path();
            if file_type.is_symlink() {
                return Err(exchange_error(ExchangeErrorCode::Io));
            }
            if file_type.is_dir() {
                pending.push(path);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(suffix))
            {
                found.push(path);
            }
        }
    }
    Ok(found)
}

fn classify_target(target: &Path, preflight: &Preflight) -> Result<Target> {
    let Some(_) = real_directory_metadata(target)? else {
        return Ok(Target::Fresh);
    };
    let hex = hex_id(preflight.exchange_id.as_bytes());
    let marker_name = format!("{hex}{STAGE_SUFFIX}");
    let temporary_name = format!("{hex}{STAGE_TEMPORARY_SUFFIX}");
    let mut entries: Vec<String> = Vec::new();
    for entry in fs::read_dir(target)? {
        let entry = entry?;
        entries.push(entry.file_name().to_string_lossy().into_owned());
    }
    if entries.is_empty() {
        return Ok(Target::Fresh);
    }
    let exchange_dir = target.join(EXCHANGE_DIRECTORY);
    let mut marker_present = false;
    if real_directory_metadata(&exchange_dir)?.is_some() {
        let mut exchange_entries = Vec::new();
        for entry in fs::read_dir(&exchange_dir)? {
            exchange_entries.push(entry?.file_name().to_string_lossy().into_owned());
        }
        if exchange_entries
            .iter()
            .any(|name| name != EXCHANGE_VERSION_DIRECTORY)
        {
            return Err(exchange_error(ExchangeErrorCode::TargetNotEmpty));
        }
        let versioned = exchange_dir.join(EXCHANGE_VERSION_DIRECTORY);
        if real_directory_metadata(&versioned)?.is_some() {
            let mut markers = 0_usize;
            for entry in fs::read_dir(&versioned)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let file_type = entry.file_type()?;
                if file_type.is_symlink() {
                    return Err(exchange_error(ExchangeErrorCode::Io));
                }
                if name == temporary_name {
                    if !file_type.is_file() {
                        return Err(exchange_error(ExchangeErrorCode::Io));
                    }
                    continue;
                }
                if name.ends_with(STAGE_SUFFIX) {
                    if !file_type.is_file() {
                        return Err(exchange_error(ExchangeErrorCode::Io));
                    }
                    markers += 1;
                    if name != marker_name {
                        return Err(exchange_error(ExchangeErrorCode::TargetIncompleteMismatch));
                    }
                    let contents = read_regular_file(&entry.path(), ID_LEN)?;
                    if contents != preflight.exchange_id.as_bytes() {
                        return Err(exchange_error(ExchangeErrorCode::TargetIncompleteMismatch));
                    }
                    marker_present = true;
                    continue;
                }
                return Err(exchange_error(ExchangeErrorCode::TargetNotEmpty));
            }
            if markers > 1 {
                return Err(exchange_error(ExchangeErrorCode::TargetIncompleteMismatch));
            }
        }
    }
    let only_exchange = entries.iter().all(|name| name == EXCHANGE_DIRECTORY);
    if !marker_present {
        return if only_exchange {
            Ok(Target::Fresh)
        } else {
            Err(exchange_error(ExchangeErrorCode::TargetNotEmpty))
        };
    }
    verify_incomplete_clone(target, preflight)?;
    Ok(Target::IncompleteClone)
}

/// Proves that everything already installed in a marked target belongs to
/// this exchange: the head (if present), every receipt, every origin record
/// (including one without a visible ref), and every ref.
fn verify_incomplete_clone(target: &Path, preflight: &Preflight) -> Result<()> {
    let head_path = target.join("heads").join("accepted");
    match fs::symlink_metadata(&head_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(exchange_error(ExchangeErrorCode::Io));
            }
            let bytes = read_regular_file(&head_path, HEAD_LEN)?;
            match decode_head_bytes(&bytes) {
                Some(head) if head == preflight.decoded.accepted_head.transaction_id => {}
                _ => return Err(exchange_error(ExchangeErrorCode::TargetIncompleteMismatch)),
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let receipt_bytes: BTreeMap<String, &[u8]> = preflight
        .decoded
        .receipts
        .iter()
        .map(|entry| {
            (
                format!(
                    "{}{RECEIPT_SUFFIX}",
                    hex_id(entry.transaction_id.as_bytes())
                ),
                entry.stored_bytes.as_slice(),
            )
        })
        .collect();
    for path in collect_files_with_suffix(&target.join("transactions"), RECEIPT_SUFFIX)? {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        let expected = name.as_deref().and_then(|name| receipt_bytes.get(name));
        let bytes = read_regular_file(&path, MAX_EXCHANGE_BYTES)?;
        if expected.is_none_or(|expected| *expected != bytes.as_slice()) {
            return Err(exchange_error(ExchangeErrorCode::TargetIncompleteMismatch));
        }
    }
    let origin_bytes: BTreeSet<&[u8]> = preflight
        .decoded
        .branches
        .iter()
        .map(|entry| entry.stored_origin.as_slice())
        .collect();
    for path in collect_files_with_suffix(&target.join("branches"), ORIGIN_SUFFIX)? {
        let bytes = read_regular_file(&path, MAX_EXCHANGE_BYTES)?;
        if !origin_bytes.contains(bytes.as_slice()) {
            return Err(exchange_error(ExchangeErrorCode::TargetIncompleteMismatch));
        }
    }
    let ref_bytes: BTreeSet<&[u8]> = preflight
        .decoded
        .branches
        .iter()
        .map(|entry| entry.stored_ref.as_slice())
        .collect();
    for path in collect_files_with_suffix(&target.join("refs"), REF_SUFFIX)? {
        let bytes = read_regular_file(&path, MAX_EXCHANGE_BYTES)?;
        if !ref_bytes.contains(bytes.as_slice()) {
            return Err(exchange_error(ExchangeErrorCode::TargetIncompleteMismatch));
        }
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

fn create_real_directory(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    if real_directory_metadata(path)?.is_none() {
        return Err(exchange_error(ExchangeErrorCode::Io));
    }
    Ok(())
}

fn install_stage_marker(target: &Path, exchange_id: RepositoryExchangeId) -> Result<PathBuf> {
    create_real_directory(target)?;
    let exchange_dir = target.join(EXCHANGE_DIRECTORY);
    create_real_directory(&exchange_dir)?;
    let versioned = exchange_dir.join(EXCHANGE_VERSION_DIRECTORY);
    create_real_directory(&versioned)?;
    let hex = hex_id(exchange_id.as_bytes());
    let marker = versioned.join(format!("{hex}{STAGE_SUFFIX}"));
    if let Ok(metadata) = fs::symlink_metadata(&marker) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(exchange_error(ExchangeErrorCode::Io));
        }
        if read_regular_file(&marker, ID_LEN)? == exchange_id.as_bytes() {
            return Ok(marker);
        }
        return Err(exchange_error(ExchangeErrorCode::TargetIncompleteMismatch));
    }
    let temporary = versioned.join(format!("{hex}{STAGE_TEMPORARY_SUFFIX}"));
    if fs::symlink_metadata(&temporary).is_ok() {
        fs::remove_file(&temporary)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(exchange_id.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, &marker)?;
    sync_directory(&versioned)?;
    sync_directory(&exchange_dir)?;
    sync_directory(target)?;
    Ok(marker)
}

fn install_branches(
    target: &Path,
    branches: &[VerifiedBranch],
    branch_repository: &BranchRepository,
) -> Result<()> {
    branch_repository.ensure_layout_under_maintenance()?;
    let _refs_lock = branch_repository.acquire_refs_lock()?;
    let branches_dir = target.join("branches").join(EXCHANGE_VERSION_DIRECTORY);
    let refs_dir = target.join("refs").join(EXCHANGE_VERSION_DIRECTORY);
    for branch in branches {
        let origin_path = ensure_key_path(&branches_dir, &branch.name, ORIGIN_SUFFIX, 2, 3)?;
        let ref_path = ensure_key_path(&refs_dir, &branch.name, REF_SUFFIX, 0, 0)?;
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
                    Err(BranchError::Branch(BranchErrorCode::BranchOriginMismatch))
                }
            },
        )
        .map_err(map_install_collision)?;
        persist_expected_ref(&ref_path, &branch.reference).map_err(map_install_collision)?;
    }
    Ok(())
}

fn map_install_collision(error: BranchError) -> ExchangeError {
    match &error {
        BranchError::Branch(
            BranchErrorCode::BranchOriginMismatch | BranchErrorCode::RefAlreadyExists,
        ) => exchange_error(ExchangeErrorCode::Io),
        _ => ExchangeError::Branch(error),
    }
}

/// Imports an S20-540 exchange into a fresh target or a same-exchange
/// incomplete clone, producing a clone-equivalent repository.
///
/// # Errors
///
/// Fails closed before any write with the exact exchange, pack, transaction,
/// branch, or store code named by the contract, and preserves every
/// lower-layer code during persistence.
#[allow(clippy::too_many_lines)]
pub fn import_repository_exchange<V: CanonicalVerifier>(
    target: &Path,
    input: &[u8],
    verifier: &V,
) -> Result<ExchangeImportReport> {
    let preflight = preflight(input, verifier)?;
    let advisory = classify_target(target, &preflight)?;
    let _ = advisory;

    let marker = install_stage_marker(target, preflight.exchange_id)?;
    initialize_repository_maintenance(target)?;
    let maintenance = acquire_exclusive_repository_maintenance_nonblocking(target)
        .map_err(|_| exchange_error(ExchangeErrorCode::Io))?;
    if classify_target(target, &preflight)? != Target::IncompleteClone {
        return Err(exchange_error(ExchangeErrorCode::TargetIncompleteMismatch));
    }

    let store = ObjectStore::new(target);
    let (promoted_objects, present_objects) =
        promote_pack_objects(&store, &preflight.pack.decoded.objects, verifier)?;

    let transactions = TransactionRepository::new(target);
    let order = topological_order(&preflight.receipts)?;
    let receipt_bytes = order
        .iter()
        .map(|transaction_id| preflight.receipts[transaction_id].stored_bytes.as_slice())
        .collect::<Vec<&[u8]>>();
    let receipts = transactions.initialize_trusted_clone_receipts_with_maintenance(
        &maintenance,
        preflight.decoded.accepted_head.transaction_id,
        &receipt_bytes,
    )?;

    let branch_repository = BranchRepository::new(target);
    install_branches(target, &preflight.branches, &branch_repository)?;

    let accepted_head = transactions.initialize_trusted_clone_head_with_maintenance(
        &maintenance,
        preflight.decoded.accepted_head.transaction_id,
    )?;

    fs::remove_file(&marker)?;
    let versioned = target
        .join(EXCHANGE_DIRECTORY)
        .join(EXCHANGE_VERSION_DIRECTORY);
    sync_directory(&versioned)?;
    drop(maintenance);
    let _ = preflight.decoded.leaves.len();
    Ok(ExchangeImportReport {
        exchange_id: preflight.exchange_id,
        accepted_head,
        receipts,
        branches: preflight.branches.len(),
        promoted_objects,
        present_objects,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use sley_id::{
        CandidateNonce, EntityId, ObjectId, PrincipalId, SchemaEpochId, TransactionId, WorkspaceId,
    };
    use sley_mutate::{
        BoundPrecondition, CandidateExpiry, CandidateRecord, EntityObjectRecord,
        ExpectedIdentityAbsent, ImportedCandidate, MutationClass, MutationOperation,
        MutationPayload, PreconditionPayload, PreimageRequirement, build_candidate,
        build_entity_object, full_validation_profile_id, import_entity_object,
        value::{EntityBodyValue, EntityIdSet, NamespaceBody},
    };
    use sley_policy::{
        AcceptedPolicyRoot, CandidateValidationLimits, PolicyResourceCeilings, PolicyRootBuilder,
        PrincipalGrantBuilder, build_capability_summary_projection,
        conformance_registry as policy_registry,
    };
    use sley_state_root::{
        AcceptedStateRoot, StateRootBuilder, conformance_epoch_id as state_epoch_id,
        conformance_registry as state_registry,
    };
    use sley_store::ObjectStore;
    use sley_txn::{CommitInput, TransactionRepository, TrustedGenesisInput};

    use super::*;
    use crate::refs::BranchRepository;

    const NOW: u64 = 1_000;
    static TEMP_DIR_COUNTER: ::std::sync::atomic::AtomicU64 =
        ::std::sync::atomic::AtomicU64::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let sequence = TEMP_DIR_COUNTER.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed);
            let path = ::std::env::temp_dir().join(format!(
                "sley-exchange-{label}-{}-{sequence:016x}",
                ::std::process::id()
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

    fn fixed<T>(byte: u8, constructor: impl FnOnce([u8; 32]) -> T) -> T {
        constructor([byte; 32])
    }

    fn namespace_body() -> EntityBodyValue {
        EntityBodyValue::Namespace(NamespaceBody {
            parent: None,
            members: EntityIdSet::from_unsorted(vec![]).unwrap(),
        })
    }

    fn verifier(
        epoch: SchemaEpochId,
    ) -> impl Fn(&[u8]) -> core::result::Result<ObjectId, ScbError> {
        move |bytes| import_entity_object(epoch, bytes).map(|object| object.object_id())
    }

    fn candidate_for(
        workspace_id: WorkspaceId,
        principal_id: PrincipalId,
        base_transaction_id: TransactionId,
        base_state: &AcceptedStateRoot,
        policy: &AcceptedPolicyRoot,
        nonce_byte: u8,
    ) -> ImportedCandidate {
        let nonce = fixed(nonce_byte, CandidateNonce::from_bytes);
        let target = EntityId::derive(workspace_id, nonce, 3, 0);
        let summary = build_capability_summary_projection(
            principal_id,
            workspace_id,
            policy.root(),
            base_state.root,
            &[],
        )
        .unwrap();
        build_candidate(&CandidateRecord {
            format_version: 1,
            workspace_id,
            base_transaction_id,
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
        .unwrap()
    }

    /// A source repository: genesis, one commit, branch `main` advanced to the
    /// commit, branch `aux` at the commit, an orphan origin, an unreachable
    /// receipt file, and an unreferenced object.
    struct Source {
        temp: TempDir,
        root: PathBuf,
        epoch: SchemaEpochId,
        genesis: TransactionId,
        head: TransactionId,
    }

    impl Source {
        fn new(label: &str) -> Self {
            Self::new_with_nonce(label, 30)
        }

        /// A source whose commit uses `nonce_byte`, so two sources with
        /// different nonces export different exchanges.
        fn new_with_nonce(label: &str, nonce_byte: u8) -> Self {
            let temp = TempDir::new(label);
            let root = temp.child("source");
            fs::create_dir(&root).unwrap();
            let transactions = TransactionRepository::new(&root);
            let branches = BranchRepository::new(&root);
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
                nonce_byte,
            );
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
            branches.create_branch("aux", head).unwrap();
            branches.create_branch("orphan", genesis).unwrap();
            let orphan = BranchName::parse("orphan").unwrap();
            let orphan_ref = root
                .join("refs")
                .join("v1")
                .join(&hex_id(&orphan.path_key())[0..2])
                .join(&hex_id(&orphan.path_key())[2..4])
                .join(format!("{}{REF_SUFFIX}", hex_id(&orphan.path_key())));
            fs::remove_file(orphan_ref).unwrap();
            let head_receipt = transactions.verified_revision(head).unwrap();
            let unreachable = root.join("transactions").join("v1").join("zz").join("zz");
            fs::create_dir_all(&unreachable).unwrap();
            fs::write(
                unreachable.join(format!("{}{RECEIPT_SUFFIX}", "ff".repeat(32))),
                &head_receipt.receipt().stored_bytes,
            )
            .unwrap();
            let extra_object = build_entity_object(
                epoch,
                &EntityObjectRecord {
                    entity_id: fixed(77, EntityId::from_bytes),
                    body: namespace_body(),
                    label: None,
                    semantic_fingerprint: None,
                },
            )
            .unwrap();
            store
                .put(
                    extra_object.object_id(),
                    extra_object.stored_bytes(),
                    &verifier(epoch),
                )
                .unwrap();
            Self {
                temp,
                root,
                epoch,
                genesis,
                head,
            }
        }

        fn export(&self) -> AcceptedRepositoryExchange {
            export_repository_exchange(&self.root, &verifier(self.epoch)).unwrap()
        }

        fn target(&self, name: &str) -> PathBuf {
            self.temp.child(name)
        }
    }

    fn branch_listing(root: &Path) -> Vec<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let mut listed = BranchRepository::new(root)
            .list_branches(MAX_BRANCHES)
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

    fn mark(root: &Path, exchange_id: RepositoryExchangeId) {
        let versioned = root
            .join(EXCHANGE_DIRECTORY)
            .join(EXCHANGE_VERSION_DIRECTORY);
        fs::create_dir_all(&versioned).unwrap();
        fs::write(
            versioned.join(format!("{}{STAGE_SUFFIX}", hex_id(exchange_id.as_bytes()))),
            exchange_id.as_bytes(),
        )
        .unwrap();
    }

    fn assert_clone_equivalent(
        source: &Source,
        target: &Path,
        exchange: &AcceptedRepositoryExchange,
    ) {
        let source_transactions = TransactionRepository::new(&source.root);
        let target_transactions = TransactionRepository::new(target);
        let source_head = source_transactions.accepted_head().unwrap();
        let target_head = target_transactions.accepted_head().unwrap();
        assert_eq!(target_head.transaction_id(), source_head.transaction_id());
        assert_eq!(
            target_head.receipt().stored_bytes,
            source_head.receipt().stored_bytes
        );
        for entry in &exchange.receipts {
            let expected = source_transactions
                .verified_revision(entry.transaction_id)
                .unwrap();
            let actual = target_transactions
                .verified_revision(entry.transaction_id)
                .unwrap();
            assert_eq!(
                actual.receipt().stored_bytes,
                expected.receipt().stored_bytes
            );
            assert_eq!(actual.state_root().root, expected.state_root().root);
            assert_eq!(actual.policy_root().root(), expected.policy_root().root());
            assert_eq!(actual.objects().len(), expected.objects().len());
            assert_eq!(actual.tombstoned_entities(), expected.tombstoned_entities());
        }
        let exported: Vec<(Vec<u8>, Vec<u8>, Vec<u8>)> = {
            let mut entries = exchange
                .branches
                .iter()
                .map(|entry| {
                    (
                        entry.branch_name.clone(),
                        entry.stored_origin.clone(),
                        entry.stored_ref.clone(),
                    )
                })
                .collect::<Vec<_>>();
            entries.sort();
            entries
        };
        assert_eq!(branch_listing(target), exported);
        assert_eq!(exported.len(), 2);
        assert!(!exported.iter().any(|(name, _, _)| name == b"orphan"));
        let receipts =
            collect_files_with_suffix(&target.join("transactions"), RECEIPT_SUFFIX).unwrap();
        assert_eq!(receipts.len(), exchange.receipts.len());
        let origins = collect_files_with_suffix(&target.join("branches"), ORIGIN_SUFFIX).unwrap();
        assert_eq!(origins.len(), 2);
        assert!(
            !target
                .join(EXCHANGE_DIRECTORY)
                .join(EXCHANGE_VERSION_DIRECTORY)
                .join(format!(
                    "{}{STAGE_SUFFIX}",
                    hex_id(exchange.exchange_id.as_bytes())
                ))
                .exists()
        );
        let re_exported = export_repository_exchange(target, &verifier(source.epoch)).unwrap();
        assert_eq!(re_exported.stored_bytes, exchange.stored_bytes);
        assert_eq!(re_exported.exchange_id, exchange.exchange_id);
    }

    #[test]
    fn export_import_is_clone_equivalent_and_re_export_is_byte_identical() {
        let source = Source::new("round-trip");
        let exchange = source.export();
        assert_eq!(exchange.receipts.len(), 2);
        assert_eq!(exchange.branches.len(), 2);
        assert_eq!(exchange.accepted_head.transaction_id, source.head);
        assert_eq!(
            exchange.receipts[0]
                .transaction_id
                .min(exchange.receipts[1].transaction_id),
            exchange.receipts[0].transaction_id
        );
        let second = source.export();
        assert_eq!(second.stored_bytes, exchange.stored_bytes);

        let target = source.target("clone");
        let report =
            import_repository_exchange(&target, &exchange.stored_bytes, &verifier(source.epoch))
                .unwrap();
        assert_eq!(report.exchange_id, exchange.exchange_id);
        assert_eq!(report.receipts, 2);
        assert_eq!(report.branches, 2);
        assert_eq!(report.accepted_head.transaction_id(), source.head);
        assert_clone_equivalent(&source, &target, &exchange);

        let again =
            import_repository_exchange(&target, &exchange.stored_bytes, &verifier(source.epoch))
                .unwrap_err();
        assert_eq!(again.code(), "EXCHANGE_TARGET_NOT_EMPTY");
        let extra_object = ObjectStore::new(&target);
        let unreferenced = collect_files_with_suffix(&target.join("objects"), ".scb1").unwrap();
        let source_objects =
            collect_files_with_suffix(&source.root.join("objects"), ".scb1").unwrap();
        assert_eq!(unreferenced.len() + 1, source_objects.len());
        drop(extra_object);
        let _ = source.genesis;
    }

    #[test]
    fn interrupted_clones_converge_on_retry_with_identical_bytes() {
        let source = Source::new("retry");
        let exchange = source.export();
        let target = source.target("clone");

        // X-02: marker present, nothing else.
        mark(&target, exchange.exchange_id);
        import_repository_exchange(&target, &exchange.stored_bytes, &verifier(source.epoch))
            .unwrap();
        assert_clone_equivalent(&source, &target, &exchange);

        // X-07: complete clone whose marker was not yet removed.
        mark(&target, exchange.exchange_id);
        let report =
            import_repository_exchange(&target, &exchange.stored_bytes, &verifier(source.epoch))
                .unwrap();
        assert_eq!(report.receipts, 2);
        assert_clone_equivalent(&source, &target, &exchange);

        // Different bytes into an incomplete clone.
        mark(&target, exchange.exchange_id);
        let other = Source::new_with_nonce("retry-other", 31);
        let other_exchange = other.export();
        let error = import_repository_exchange(
            &target,
            &other_exchange.stored_bytes,
            &verifier(source.epoch),
        )
        .unwrap_err();
        assert_eq!(error.code(), "EXCHANGE_TARGET_INCOMPLETE_MISMATCH");
    }

    #[test]
    fn target_rules_fail_closed_before_any_write() {
        let source = Source::new("targets");
        let exchange = source.export();
        let verify = verifier(source.epoch);

        let occupied = source.target("occupied");
        fs::create_dir(&occupied).unwrap();
        fs::write(occupied.join("stray"), b"x").unwrap();
        let error =
            import_repository_exchange(&occupied, &exchange.stored_bytes, &verify).unwrap_err();
        assert_eq!(error.code(), "EXCHANGE_TARGET_NOT_EMPTY");
        assert!(!occupied.join(EXCHANGE_DIRECTORY).exists());

        let file_target = source.target("file");
        fs::write(&file_target, b"x").unwrap();
        let error =
            import_repository_exchange(&file_target, &exchange.stored_bytes, &verify).unwrap_err();
        assert_eq!(error.code(), "EXCHANGE_IO");

        let linked = source.target("linked");
        std::os::unix::fs::symlink(source.target("nowhere"), &linked).unwrap();
        let error =
            import_repository_exchange(&linked, &exchange.stored_bytes, &verify).unwrap_err();
        assert_eq!(error.code(), "EXCHANGE_IO");

        let foreign = source.target("foreign-marker");
        let other = Source::new_with_nonce("targets-other", 31);
        mark(&foreign, other.export().exchange_id);
        let error =
            import_repository_exchange(&foreign, &exchange.stored_bytes, &verify).unwrap_err();
        assert_eq!(error.code(), "EXCHANGE_TARGET_INCOMPLETE_MISMATCH");

        let malformed = source.target("malformed-marker");
        mark(&malformed, exchange.exchange_id);
        let marker = malformed
            .join(EXCHANGE_DIRECTORY)
            .join(EXCHANGE_VERSION_DIRECTORY)
            .join(format!(
                "{}{STAGE_SUFFIX}",
                hex_id(exchange.exchange_id.as_bytes())
            ));
        fs::write(&marker, b"short").unwrap();
        let error =
            import_repository_exchange(&malformed, &exchange.stored_bytes, &verify).unwrap_err();
        assert_eq!(error.code(), "EXCHANGE_TARGET_INCOMPLETE_MISMATCH");

        let existing = source.target("existing-repository");
        import_repository_exchange(&existing, &exchange.stored_bytes, &verify).unwrap();
        let error =
            import_repository_exchange(&existing, &exchange.stored_bytes, &verify).unwrap_err();
        assert_eq!(error.code(), "EXCHANGE_TARGET_NOT_EMPTY");
        assert_eq!(
            TransactionRepository::new(&existing)
                .accepted_head()
                .unwrap()
                .transaction_id(),
            source.head
        );

        // A marked clone carrying a branch outside the exchange is rejected.
        let extra = source.target("extra-branch");
        import_repository_exchange(&extra, &exchange.stored_bytes, &verify).unwrap();
        BranchRepository::new(&extra)
            .create_branch("extra", source.genesis)
            .unwrap();
        mark(&extra, exchange.exchange_id);
        let error =
            import_repository_exchange(&extra, &exchange.stored_bytes, &verify).unwrap_err();
        assert_eq!(error.code(), "EXCHANGE_TARGET_INCOMPLETE_MISMATCH");
    }

    #[test]
    fn frozen_write_paths_fail_closed_on_a_marked_root() {
        let source = Source::new("guards");
        let exchange = source.export();
        mark(&source.root, exchange.exchange_id);
        let branches = BranchRepository::new(&source.root);
        let error = branches
            .create_branch("blocked", source.genesis)
            .unwrap_err();
        assert_eq!(error.code(), "TXN_INCOMPLETE_CLONE");
        assert_eq!(error.numeric_code(), Some(39_022));
        let error = branches
            .advance_branch("aux", source.head, source.genesis)
            .unwrap_err();
        assert_eq!(error.code(), "TXN_INCOMPLETE_CLONE");
        let error = branches.recover_refs().unwrap_err();
        assert_eq!(error.code(), "TXN_INCOMPLETE_CLONE");
        let error = crate::acquire_exclusive_gc(&ObjectStore::new(&source.root)).unwrap_err();
        assert_eq!(error.symbol(), "TXN_INCOMPLETE_CLONE");
        let transactions = TransactionRepository::new(&source.root);
        let error = transactions.recover().unwrap_err();
        assert_eq!(error.code(), "TXN_INCOMPLETE_CLONE");
        // Read paths stay available and establish no acceptance.
        assert_eq!(
            transactions.accepted_head().unwrap().transaction_id(),
            source.head
        );
        assert_eq!(branches.list_branches(MAX_BRANCHES).unwrap().len(), 2);
    }

    fn tamper(exchange: &AcceptedRepositoryExchange, mutate: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
        let mut bytes = exchange.stored_bytes.clone();
        mutate(&mut bytes);
        bytes
    }

    #[test]
    fn envelope_payload_and_nesting_rejections_precede_any_write() {
        let source = Source::new("rejections");
        let exchange = source.export();
        let verify = verifier(source.epoch);
        let target = source.target("never-written");

        let trailer = tamper(&exchange, |bytes| {
            let last = bytes.len() - 1;
            bytes[last] ^= 0x01;
        });
        assert_eq!(
            import_repository_exchange(&target, &trailer, &verify)
                .unwrap_err()
                .code(),
            "EXCHANGE_DIGEST_MISMATCH"
        );
        assert!(!target.exists());

        let nested = build_exchange(
            exchange.pack_id,
            exchange.stored_bytes.clone(),
            exchange.receipts.clone(),
            exchange.accepted_head,
            exchange.branches.clone(),
        )
        .unwrap();
        assert_eq!(
            import_repository_exchange(&target, &nested.stored_bytes, &verify)
                .unwrap_err()
                .code(),
            "EXCHANGE_PACK_INVALID"
        );

        let wrong_leaf = {
            let mut receipts = exchange.receipts.clone();
            receipts[0].stored_bytes.push(0);
            build_exchange(
                exchange.pack_id,
                exchange.object_pack.clone(),
                receipts,
                exchange.accepted_head,
                exchange.branches.clone(),
            )
            .unwrap()
        };
        let error =
            import_repository_exchange(&target, &wrong_leaf.stored_bytes, &verify).unwrap_err();
        assert!(
            error.code().starts_with("TXN_")
                || error.code().starts_with("SCB_")
                || error.code() == "EXCHANGE_RECEIPT_INVALID",
            "{}",
            error.code()
        );

        let reversed_branches = {
            let mut branches = exchange.branches.clone();
            branches.reverse();
            build_exchange(
                exchange.pack_id,
                exchange.object_pack.clone(),
                exchange.receipts.clone(),
                exchange.accepted_head,
                branches,
            )
            .unwrap()
        };
        assert_eq!(
            import_repository_exchange(&target, &reversed_branches.stored_bytes, &verify)
                .unwrap_err()
                .code(),
            "EXCHANGE_CANONICAL_ORDER"
        );

        let foreign_head = build_exchange(
            exchange.pack_id,
            exchange.object_pack.clone(),
            exchange.receipts.clone(),
            ExchangeHeadEntry {
                transaction_id: fixed(9, TransactionId::from_bytes),
                receipt_id: exchange.accepted_head.receipt_id,
            },
            exchange.branches.clone(),
        )
        .unwrap();
        assert_eq!(
            import_repository_exchange(&target, &foreign_head.stored_bytes, &verify)
                .unwrap_err()
                .code(),
            "EXCHANGE_HEAD_INVALID"
        );

        let surplus_free_but_open = build_exchange(
            exchange.pack_id,
            exchange.object_pack.clone(),
            vec![
                exchange
                    .receipts
                    .iter()
                    .find(|entry| entry.transaction_id == source.head)
                    .cloned()
                    .unwrap(),
            ],
            exchange.accepted_head,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(
            import_repository_exchange(&target, &surplus_free_but_open.stored_bytes, &verify)
                .unwrap_err()
                .code(),
            "EXCHANGE_ANCESTRY_OPEN"
        );

        let no_branches = build_exchange(
            exchange.pack_id,
            exchange.object_pack.clone(),
            exchange.receipts.clone(),
            exchange.accepted_head,
            Vec::new(),
        )
        .unwrap();
        let clone = source.target("no-branches");
        let report =
            import_repository_exchange(&clone, &no_branches.stored_bytes, &verify).unwrap();
        assert_eq!(report.branches, 0);
        assert!(branch_listing(&clone).is_empty());
        assert!(!target.exists());
    }

    #[test]
    fn branch_entries_follow_canonical_set_order_over_the_encoded_element() {
        let name = |length: usize| {
            let mut bytes = vec![b'a'; length];
            if length > 1 {
                bytes[length - 1] = b'z';
            }
            bytes
        };
        let entry = |branch_name: Vec<u8>| ExchangeBranchEntry {
            branch_name,
            stored_origin: vec![1],
            stored_ref: vec![2],
        };
        let mut entries = [
            entry(name(253)),
            entry(b"aa".to_vec()),
            entry(name(128)),
            entry(name(255)),
            entry(b"b".to_vec()),
            entry(name(127)),
            entry(name(254)),
        ]
        .into_iter()
        .map(|entry| (encode_branch_element(&entry).unwrap(), entry))
        .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        let lengths = entries
            .iter()
            .map(|(_, entry)| entry.branch_name.len())
            .collect::<Vec<_>>();
        assert_eq!(lengths, vec![1, 2, 127, 128, 253, 254, 255]);
    }

    #[test]
    fn fixed_head_bytes_match_the_transaction_model() {
        let source = Source::new("head-bytes");
        let stored = fs::read(source.root.join("heads").join("accepted")).unwrap();
        assert_eq!(stored, stored_head_bytes(source.head));
        assert_eq!(decode_head_bytes(&stored), Some(source.head));
        assert_eq!(decode_head_bytes(&stored[..HEAD_LEN - 1]), None);
    }

    #[test]
    fn exchange_codes_are_closed_and_contiguous() {
        for (offset, code) in ExchangeErrorCode::ALL.into_iter().enumerate() {
            assert_eq!(code.numeric(), 54_000 + u32::try_from(offset).unwrap());
            assert!(code.as_str().starts_with("EXCHANGE_"));
        }
        assert_eq!(
            exchange_epoch_record().contracts[0].contract_tag,
            CONTRACT_TAG
        );
        assert_eq!(
            exchange_epoch_record().contracts[0].digest_domain_tag,
            DIGEST_DOMAIN_TAG
        );
    }
}
