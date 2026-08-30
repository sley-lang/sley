//! Native S20-500 named refs over fully verified durable transactions.

use core::fmt;
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sley_id::{PolicyRootId, SchemaEpochId, StateRoot, TransactionId, WorkspaceId};
use sley_scb1::{
    MAX_COLLECTION_ELEMENTS, MAX_STANDALONE_BYTES, ScbErrorCode, ScbValueCursor, encode_bytes,
    encode_list, encode_record, encode_uvar,
};
use sley_txn::{CommitError, RepositoryMaintenanceGuard, TransactionRepository, VerifiedRevision};
use sley_txn::{RecoveryAncestryError, RecoveryAncestryRequest, RecoveryRevisionClaim};

const BRANCH_MAGIC: [u8; 8] = *b"SLEYBR01";
const REF_MAGIC: [u8; 8] = *b"SLEYRF01";
const NAME_KEY_MAGIC: [u8; 8] = *b"SLEYBNM1";
const ENVELOPE_VERSION: u64 = 1;
const RECORD_VERSION: u32 = 1;
const BRANCH_FIELD_COUNT: u64 = 8;
const REF_FIELD_COUNT: u64 = 9;
const BRANCH_DIGEST_DOMAIN: &[u8] = b"sley2.branch-record.v1";
const REF_DIGEST_DOMAIN: &[u8] = b"sley2.branch-ref.v1";
const NAME_KEY_DOMAIN: &[u8] = b"sley2.branch-name-path.v1";
const DIGEST_LEN: usize = 32;
const MAX_BRANCH_NAME_BYTES: usize = 255;
const MAX_BRANCH_COMPONENTS: usize = 8;
const MAX_BRANCH_COMPONENT_BYTES: usize = 63;
const MAX_BRANCHES: usize = 4_096;
const MAX_BRANCH_ORIGINS: usize = 65_536;
const MAX_ANCESTRY_NODES: usize = 65_536;
const MAX_STAGE_ATTEMPTS: u64 = 1_024;
const BRANCH_STAGE_PREFIX: &str = ".sley-branch-stage-";
const REF_STAGE_PREFIX: &str = ".sley-ref-stage-";
const STAGE_SUFFIX: &str = ".tmp";
const ORIGIN_RECOVERY_MAX_FANOUT_DIRECTORIES: u64 = 65_792;
const ORIGIN_RECOVERY_MAX_LEAF_ENTRIES: u64 = 131_072;
const ORIGIN_RECOVERY_MAX_FINAL_RECORDS: u64 = 65_536;
const ORIGIN_RECOVERY_MAX_REMOVABLE_STAGES: u64 = 65_536;
const ORIGIN_RECOVERY_MAX_RECORD_BYTES: u64 = 1_073_741_824;
const REF_RECOVERY_MAX_FANOUT_DIRECTORIES: u64 = 65_792;
const REF_RECOVERY_MAX_LEAF_ENTRIES: u64 = 69_632;
const REF_RECOVERY_MAX_REMOVABLE_STAGES: u64 = 65_536;
const REF_RECOVERY_MAX_RECORD_BYTES: u64 = 268_435_456;
const REF_RECOVERY_MAX_ORPHAN_ORIGINS: u64 = 65_536;
const REF_RECOVERY_MAX_VISIBLE_BRANCHES: u64 = 4_096;

/// Closed per-invocation ref-recovery scan and verification ceilings.
#[derive(Clone, Copy)]
struct RefRecoveryLimits {
    origin_fanout_directories: u64,
    origin_leaf_entries: u64,
    final_origins: u64,
    origin_stages: u64,
    origin_record_bytes: u64,
    ref_fanout_directories: u64,
    ref_leaf_entries: u64,
    ref_stages: u64,
    visible_ref_record_bytes: u64,
    orphan_origins: u64,
    visible_branches: u64,
}

const fn ref_recovery_limits() -> RefRecoveryLimits {
    RefRecoveryLimits {
        origin_fanout_directories: ORIGIN_RECOVERY_MAX_FANOUT_DIRECTORIES,
        origin_leaf_entries: ORIGIN_RECOVERY_MAX_LEAF_ENTRIES,
        final_origins: ORIGIN_RECOVERY_MAX_FINAL_RECORDS,
        origin_stages: ORIGIN_RECOVERY_MAX_REMOVABLE_STAGES,
        origin_record_bytes: ORIGIN_RECOVERY_MAX_RECORD_BYTES,
        ref_fanout_directories: REF_RECOVERY_MAX_FANOUT_DIRECTORIES,
        ref_leaf_entries: REF_RECOVERY_MAX_LEAF_ENTRIES,
        ref_stages: REF_RECOVERY_MAX_REMOVABLE_STAGES,
        visible_ref_record_bytes: REF_RECOVERY_MAX_RECORD_BYTES,
        orphan_origins: REF_RECOVERY_MAX_ORPHAN_ORIGINS,
        visible_branches: REF_RECOVERY_MAX_VISIBLE_BRANCHES,
    }
}

/// Owned ref-recovery scan and verification usage tally.
#[derive(Default)]
struct RefRecoveryUsage {
    origin_fanout_directories: u64,
    origin_leaf_entries: u64,
    final_origins: u64,
    origin_stages: u64,
    origin_record_bytes: u64,
    ref_fanout_directories: u64,
    ref_leaf_entries: u64,
    ref_stages: u64,
    visible_ref_record_bytes: u64,
    orphan_origins: u64,
    visible_branches: u64,
}

fn ensure_ref_recovery_limit(value: u64, limit: u64) -> Result<(), BranchError> {
    #[cfg(test)]
    tests::record_s20_530_limit_probe(value, limit);
    if value > limit {
        return Err(branch_error(BranchErrorCode::BranchResourceLimit));
    }
    Ok(())
}

/// Closed classification of one validated recovery leaf entry.
enum RefRecoveryLeafKind {
    Final,
    OwnedStage,
    Unknown,
}

/// Bounded read-only inventory over one owned recovery tree.
struct RefRecoveryScanPlan {
    final_paths: Vec<PathBuf>,
    leaf_directories: Vec<PathBuf>,
    removal_plan: Vec<PathBuf>,
}

struct RecoveryRecordPath {
    path: PathBuf,
}

struct RecoveryVisibleBranch {
    origin: ImportedBranchRecord,
    reference: ImportedBranchRef,
}

struct RefRecordPreflight {
    visible: Vec<RecoveryVisibleBranch>,
    orphan_origins: Vec<OrphanBranchOrigin>,
}

static STAGE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
enum Cross05OwnerSignal {
    OwnerHeld,
    ReleaseOwner,
}

#[cfg(test)]
enum Cross05Blocked {
    Commit(bool),
    Ref(bool),
    Gc(bool),
}

#[cfg(test)]
enum Cross05Completed {
    Commit(bool),
    Ref(bool),
    Gc(bool),
}

#[cfg(test)]
struct Cross05RecoveryHold {
    root: PathBuf,
    owner_tx: ::std::sync::mpsc::SyncSender<Cross05OwnerSignal>,
    release_rx: ::std::sync::mpsc::Receiver<Cross05OwnerSignal>,
}

#[cfg(test)]
::std::thread_local! {
    static CROSS05_RECOVERY_HOLD:
        ::std::cell::RefCell<Option<Cross05RecoveryHold>> =
        const { ::std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn install_ref_recovery_hold(
    root: &Path,
    owner_tx: ::std::sync::mpsc::SyncSender<Cross05OwnerSignal>,
    release_rx: ::std::sync::mpsc::Receiver<Cross05OwnerSignal>,
) {
    CROSS05_RECOVERY_HOLD.with(|slot| {
        let mut slot = slot.borrow_mut();
        ::core::assert!(slot.is_none());
        *slot = Some(Cross05RecoveryHold {
            root: root.to_path_buf(),
            owner_tx,
            release_rx,
        });
    });
}

#[cfg(test)]
fn hold_ref_recovery_before_exclusive_drop(root: &Path, maintenance: &RepositoryMaintenanceGuard) {
    CROSS05_RECOVERY_HOLD.with(|slot| {
        let Some(hold) = slot.borrow_mut().take() else {
            return;
        };
        ::core::assert_eq!(hold.root, root);
        ::core::assert!(maintenance.is_exclusive());
        ::core::assert!(maintenance.covers(root));
        ::core::assert!(hold.owner_tx.send(Cross05OwnerSignal::OwnerHeld).is_ok());
        let release = hold.release_rx.recv().unwrap();
        ::core::assert!(::core::matches!(release, Cross05OwnerSignal::ReleaseOwner));
    });
}

const RESERVED_COMPONENTS: &[&[u8]] = &[
    b"accepted",
    b"branch",
    b"branches",
    b"head",
    b"heads",
    b"lock",
    b"locks",
    b"object",
    b"objects",
    b"ref",
    b"refs",
    b"tag",
    b"tags",
    b"transaction",
    b"transactions",
];

/// Stable S20-500 native-ref failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchErrorCode {
    /// `REF_FORMAT_VERSION`.
    RefFormatVersion,
    /// `REF_NAME_INVALID`.
    RefNameInvalid,
    /// `REF_NAME_RESERVED`.
    RefNameReserved,
    /// `REF_DIGEST_MISMATCH`.
    RefDigestMismatch,
    /// `REF_FIELD_SHAPE`.
    RefFieldShape,
    /// `REF_BRANCH_BINDING_MISMATCH`.
    RefBranchBindingMismatch,
    /// `REF_NOT_FOUND`.
    RefNotFound,
    /// `REF_ALREADY_EXISTS`.
    RefAlreadyExists,
    /// `REF_NAME_COLLISION`.
    RefNameCollision,
    /// `REF_TARGET_MISMATCH`.
    RefTargetMismatch,
    /// `REF_NAMED_CAS_STALE`.
    RefNamedCasStale,
    /// `BRANCH_RECORD_FORMAT_VERSION`.
    BranchRecordFormatVersion,
    /// `BRANCH_RECORD_DIGEST_MISMATCH`.
    BranchRecordDigestMismatch,
    /// `BRANCH_RECORD_FIELD_SHAPE`.
    BranchRecordFieldShape,
    /// `BRANCH_ORIGIN_MISMATCH`.
    BranchOriginMismatch,
    /// `BRANCH_NOT_FAST_FORWARD`.
    BranchNotFastForward,
    /// `BRANCH_ANCESTRY_CYCLE`.
    BranchAncestryCycle,
    /// `BRANCH_RESOURCE_LIMIT`.
    BranchResourceLimit,
    /// `RECOVERY_NAMED_REF_INCOMPLETE`.
    RecoveryNamedRefIncomplete,
    /// `REF_IO`.
    RefIo,
    /// `REF_INTERNAL_INVARIANT`.
    RefInternalInvariant,
}

impl BranchErrorCode {
    /// Every code in numeric order.
    pub const ALL: [Self; 21] = [
        Self::RefFormatVersion,
        Self::RefNameInvalid,
        Self::RefNameReserved,
        Self::RefDigestMismatch,
        Self::RefFieldShape,
        Self::RefBranchBindingMismatch,
        Self::RefNotFound,
        Self::RefAlreadyExists,
        Self::RefNameCollision,
        Self::RefTargetMismatch,
        Self::RefNamedCasStale,
        Self::BranchRecordFormatVersion,
        Self::BranchRecordDigestMismatch,
        Self::BranchRecordFieldShape,
        Self::BranchOriginMismatch,
        Self::BranchNotFastForward,
        Self::BranchAncestryCycle,
        Self::BranchResourceLimit,
        Self::RecoveryNamedRefIncomplete,
        Self::RefIo,
        Self::RefInternalInvariant,
    ];

    /// Returns the exact stable symbol.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::RefFormatVersion => "REF_FORMAT_VERSION",
            Self::RefNameInvalid => "REF_NAME_INVALID",
            Self::RefNameReserved => "REF_NAME_RESERVED",
            Self::RefDigestMismatch => "REF_DIGEST_MISMATCH",
            Self::RefFieldShape => "REF_FIELD_SHAPE",
            Self::RefBranchBindingMismatch => "REF_BRANCH_BINDING_MISMATCH",
            Self::RefNotFound => "REF_NOT_FOUND",
            Self::RefAlreadyExists => "REF_ALREADY_EXISTS",
            Self::RefNameCollision => "REF_NAME_COLLISION",
            Self::RefTargetMismatch => "REF_TARGET_MISMATCH",
            Self::RefNamedCasStale => "REF_NAMED_CAS_STALE",
            Self::BranchRecordFormatVersion => "BRANCH_RECORD_FORMAT_VERSION",
            Self::BranchRecordDigestMismatch => "BRANCH_RECORD_DIGEST_MISMATCH",
            Self::BranchRecordFieldShape => "BRANCH_RECORD_FIELD_SHAPE",
            Self::BranchOriginMismatch => "BRANCH_ORIGIN_MISMATCH",
            Self::BranchNotFastForward => "BRANCH_NOT_FAST_FORWARD",
            Self::BranchAncestryCycle => "BRANCH_ANCESTRY_CYCLE",
            Self::BranchResourceLimit => "BRANCH_RESOURCE_LIMIT",
            Self::RecoveryNamedRefIncomplete => "RECOVERY_NAMED_REF_INCOMPLETE",
            Self::RefIo => "REF_IO",
            Self::RefInternalInvariant => "REF_INTERNAL_INVARIANT",
        }
    }

    /// Returns the exact stable numeric code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::RefFormatVersion => 50_000,
            Self::RefNameInvalid => 50_001,
            Self::RefNameReserved => 50_002,
            Self::RefDigestMismatch => 50_003,
            Self::RefFieldShape => 50_004,
            Self::RefBranchBindingMismatch => 50_005,
            Self::RefNotFound => 50_006,
            Self::RefAlreadyExists => 50_007,
            Self::RefNameCollision => 50_008,
            Self::RefTargetMismatch => 50_009,
            Self::RefNamedCasStale => 50_010,
            Self::BranchRecordFormatVersion => 50_011,
            Self::BranchRecordDigestMismatch => 50_012,
            Self::BranchRecordFieldShape => 50_013,
            Self::BranchOriginMismatch => 50_014,
            Self::BranchNotFastForward => 50_015,
            Self::BranchAncestryCycle => 50_016,
            Self::BranchResourceLimit => 50_017,
            Self::RecoveryNamedRefIncomplete => 50_018,
            Self::RefIo => 50_019,
            Self::RefInternalInvariant => 50_020,
        }
    }
}

/// Native-ref failure preserving transaction-owned errors.
#[derive(Debug)]
pub enum BranchError {
    /// S20-500-owned semantic or codec failure.
    Branch(BranchErrorCode),
    /// Exact upstream transaction, receipt, root, policy, SCB1, or store failure.
    Transaction(CommitError),
    /// Local host I/O failure.
    Io(io::Error),
}

impl BranchError {
    /// Returns the exact stable source symbol.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Branch(code) => code.symbol(),
            Self::Transaction(error) => error.code(),
            Self::Io(_) => BranchErrorCode::RefIo.symbol(),
        }
    }

    /// Returns the exact numeric code when the owning layer froze one.
    #[must_use]
    pub fn numeric_code(&self) -> Option<u32> {
        match self {
            Self::Branch(code) => Some(code.numeric()),
            Self::Transaction(error) => error.numeric_code(),
            Self::Io(_) => Some(BranchErrorCode::RefIo.numeric()),
        }
    }
}

impl fmt::Display for BranchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for BranchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transaction(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Branch(_) => None,
        }
    }
}

impl From<CommitError> for BranchError {
    fn from(value: CommitError) -> Self {
        Self::Transaction(value)
    }
}

impl From<io::Error> for BranchError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Exact validated lowercase ASCII branch name.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BranchName(Vec<u8>);

impl BranchName {
    /// Validates exact bytes without rewriting them.
    ///
    /// # Errors
    ///
    /// Returns `REF_NAME_RESERVED` for an otherwise-valid reserved component
    /// and `REF_NAME_INVALID` for every other grammar failure.
    pub fn parse(bytes: impl AsRef<[u8]>) -> Result<Self, BranchError> {
        let bytes = bytes.as_ref();
        if bytes.is_empty() || bytes.len() > MAX_BRANCH_NAME_BYTES {
            return Err(branch_error(BranchErrorCode::RefNameInvalid));
        }
        let components = bytes.split(|byte| *byte == b'/').collect::<Vec<_>>();
        if components.is_empty() || components.len() > MAX_BRANCH_COMPONENTS {
            return Err(branch_error(BranchErrorCode::RefNameInvalid));
        }
        for component in &components {
            if component.is_empty() || component.len() > MAX_BRANCH_COMPONENT_BYTES {
                return Err(branch_error(BranchErrorCode::RefNameInvalid));
            }
            if !component.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(*byte, b'.' | b'_' | b'-')
            }) || !component.first().is_some_and(u8::is_ascii_alphanumeric)
                || !component.last().is_some_and(u8::is_ascii_alphanumeric)
                || component.ends_with(b".lock")
            {
                return Err(branch_error(BranchErrorCode::RefNameInvalid));
            }
        }
        if components
            .iter()
            .any(|component| RESERVED_COMPONENTS.contains(component))
        {
            return Err(branch_error(BranchErrorCode::RefNameReserved));
        }
        Ok(Self(bytes.to_vec()))
    }

    /// Returns the exact canonical bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Returns the canonical ASCII spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        // Construction admits ASCII only, so this fallback is unreachable
        // through the public API and keeps this accessor panic-free.
        std::str::from_utf8(&self.0).unwrap_or_default()
    }

    /// Returns the domain-separated host-path key.
    #[must_use]
    pub fn path_key(&self) -> [u8; DIGEST_LEN] {
        let mut preimage = Vec::with_capacity(NAME_KEY_MAGIC.len() + 12 + self.0.len());
        preimage.extend_from_slice(&NAME_KEY_MAGIC);
        preimage.extend_from_slice(&encode_uvar(ENVELOPE_VERSION));
        preimage.extend_from_slice(&encode_uvar(self.0.len() as u64));
        preimage.extend_from_slice(&self.0);
        digest(NAME_KEY_DOMAIN, &preimage)
    }
}

impl fmt::Debug for BranchName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BranchName")
            .field(&self.as_str())
            .finish()
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Integrity digest for one immutable branch-origin record.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BranchRecordDigest([u8; DIGEST_LEN]);

impl BranchRecordDigest {
    /// Constructs a digest from exact raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; DIGEST_LEN]) -> Self {
        Self(bytes)
    }

    /// Returns exact raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; DIGEST_LEN] {
        &self.0
    }
}

impl fmt::Debug for BranchRecordDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BranchRecordDigest")
            .field(&hex_digest(&self.0))
            .finish()
    }
}

/// Canonical immutable branch-origin facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchRecord {
    /// Exact record version.
    pub format_version: u32,
    /// Exact canonical branch name.
    pub branch_name: BranchName,
    /// Immutable origin workspace.
    pub workspace_id: WorkspaceId,
    /// Fully verified origin transaction.
    pub origin_transaction_id: TransactionId,
    /// Origin semantic state root.
    pub origin_state_root: StateRoot,
    /// Origin schema epoch.
    pub schema_epoch_id: SchemaEpochId,
    /// Origin protected policy root.
    pub policy_root_id: PolicyRootId,
    /// Exact sorted dependency roots at origin.
    pub dependency_roots: Vec<StateRoot>,
}

/// Strictly imported immutable branch-origin record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedBranchRecord {
    /// Strict canonical record.
    pub record: BranchRecord,
    /// Domain-separated integrity digest.
    pub digest: BranchRecordDigest,
    /// Exact digest preimage.
    pub preimage: Vec<u8>,
    /// Exact stored envelope bytes including digest trailer.
    pub stored_bytes: Vec<u8>,
}

/// Canonical mutable visible-ref facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchRefRecord {
    /// Exact record version.
    pub format_version: u32,
    /// Exact canonical branch name.
    pub branch_name: BranchName,
    /// Digest of the immutable origin record.
    pub branch_record_digest: BranchRecordDigest,
    /// Current verified workspace.
    pub workspace_id: WorkspaceId,
    /// Current verified transaction.
    pub head_transaction_id: TransactionId,
    /// Current semantic root.
    pub head_state_root: StateRoot,
    /// Current schema epoch.
    pub schema_epoch_id: SchemaEpochId,
    /// Current protected policy root.
    pub policy_root_id: PolicyRootId,
    /// Current exact sorted dependency roots.
    pub dependency_roots: Vec<StateRoot>,
}

/// Strictly imported visible branch ref.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedBranchRef {
    /// Strict canonical record.
    pub record: BranchRefRecord,
    /// Domain-separated ref integrity digest.
    pub digest: [u8; DIGEST_LEN],
    /// Exact digest preimage.
    pub preimage: Vec<u8>,
    /// Exact stored envelope bytes including digest trailer.
    pub stored_bytes: Vec<u8>,
}

/// Closed native-ref mutation result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchUpdateStatus {
    /// Origin and visible ref were durably created by this call.
    Created,
    /// Visible ref was durably advanced by this call.
    Advanced,
    /// The exact requested visible state was already durable and verified.
    Present,
}

impl BranchUpdateStatus {
    /// Returns the frozen wire tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Created => 1,
            Self::Advanced => 2,
            Self::Present => 3,
        }
    }

    /// Resolves one exact frozen wire tag.
    #[must_use]
    pub const fn from_tag(tag: u32) -> Option<Self> {
        match tag {
            1 => Some(Self::Created),
            2 => Some(Self::Advanced),
            3 => Some(Self::Present),
            _ => None,
        }
    }

    /// Returns the frozen result symbol.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::Advanced => "ADVANCED",
            Self::Present => "PRESENT",
        }
    }
}

/// Encodes and derives one immutable branch-origin record.
///
/// # Errors
///
/// Returns the exact S20-500 version, field, or resource failure.
pub fn build_branch_record(record: &BranchRecord) -> Result<ImportedBranchRecord, BranchError> {
    if record.format_version != RECORD_VERSION {
        return Err(branch_error(BranchErrorCode::BranchRecordFormatVersion));
    }
    validate_dependencies(
        &record.dependency_roots,
        BranchErrorCode::BranchRecordFieldShape,
    )?;
    let dependencies = encode_dependencies(&record.dependency_roots)?;
    let payload = encode_record(&[
        (1, encode_uvar(u64::from(record.format_version))),
        (
            2,
            encode_bytes(record.branch_name.as_bytes())
                .map_err(|_| branch_error(BranchErrorCode::BranchRecordFieldShape))?,
        ),
        (3, record.workspace_id.as_bytes().to_vec()),
        (4, record.origin_transaction_id.as_bytes().to_vec()),
        (5, record.origin_state_root.as_bytes().to_vec()),
        (6, record.schema_epoch_id.as_bytes().to_vec()),
        (7, record.policy_root_id.as_bytes().to_vec()),
        (8, dependencies),
    ])
    .map_err(|_| branch_error(BranchErrorCode::BranchRecordFieldShape))?;
    let (preimage, digest, stored_bytes) = build_envelope(
        BRANCH_MAGIC,
        BRANCH_DIGEST_DOMAIN,
        &payload,
        BranchErrorCode::BranchResourceLimit,
    )?;
    Ok(ImportedBranchRecord {
        record: record.clone(),
        digest: BranchRecordDigest::from_bytes(digest),
        preimage,
        stored_bytes,
    })
}

/// Strictly imports one immutable branch-origin record.
///
/// # Errors
///
/// Returns the first exact envelope, digest, name, field, or resource failure.
pub fn import_branch_record(stored: &[u8]) -> Result<ImportedBranchRecord, BranchError> {
    let (payload, preimage, digest) = decode_envelope(
        stored,
        BRANCH_MAGIC,
        BRANCH_DIGEST_DOMAIN,
        BranchErrorCode::BranchRecordFormatVersion,
        BranchErrorCode::BranchRecordDigestMismatch,
        BranchErrorCode::BranchRecordFieldShape,
    )?;
    let fields = decode_required_record(
        payload,
        BRANCH_FIELD_COUNT,
        BranchErrorCode::BranchRecordFieldShape,
    )?;
    let format_version = read_u32(fields[0], BranchErrorCode::BranchRecordFieldShape)?;
    if format_version != RECORD_VERSION {
        return Err(branch_error(BranchErrorCode::BranchRecordFormatVersion));
    }
    let branch_name = BranchName::parse(read_bytes(
        fields[1],
        BranchErrorCode::BranchRecordFieldShape,
    )?)?;
    let record = BranchRecord {
        format_version,
        branch_name,
        workspace_id: WorkspaceId::from_bytes(read_fixed(
            fields[2],
            BranchErrorCode::BranchRecordFieldShape,
        )?),
        origin_transaction_id: TransactionId::from_bytes(read_fixed(
            fields[3],
            BranchErrorCode::BranchRecordFieldShape,
        )?),
        origin_state_root: StateRoot::from_bytes(read_fixed(
            fields[4],
            BranchErrorCode::BranchRecordFieldShape,
        )?),
        schema_epoch_id: SchemaEpochId::from_bytes(read_fixed(
            fields[5],
            BranchErrorCode::BranchRecordFieldShape,
        )?),
        policy_root_id: PolicyRootId::from_bytes(read_fixed(
            fields[6],
            BranchErrorCode::BranchRecordFieldShape,
        )?),
        dependency_roots: decode_dependencies(fields[7], BranchErrorCode::BranchRecordFieldShape)?,
    };
    Ok(ImportedBranchRecord {
        record,
        digest: BranchRecordDigest::from_bytes(digest),
        preimage,
        stored_bytes: stored.to_vec(),
    })
}

/// Encodes and derives one visible branch ref.
///
/// # Errors
///
/// Returns the exact S20-500 version, field, or resource failure.
pub fn build_branch_ref(record: &BranchRefRecord) -> Result<ImportedBranchRef, BranchError> {
    if record.format_version != RECORD_VERSION {
        return Err(branch_error(BranchErrorCode::RefFormatVersion));
    }
    validate_dependencies(&record.dependency_roots, BranchErrorCode::RefFieldShape)?;
    let dependencies = encode_dependencies(&record.dependency_roots)?;
    let payload = encode_record(&[
        (1, encode_uvar(u64::from(record.format_version))),
        (
            2,
            encode_bytes(record.branch_name.as_bytes())
                .map_err(|_| branch_error(BranchErrorCode::RefFieldShape))?,
        ),
        (3, record.branch_record_digest.as_bytes().to_vec()),
        (4, record.workspace_id.as_bytes().to_vec()),
        (5, record.head_transaction_id.as_bytes().to_vec()),
        (6, record.head_state_root.as_bytes().to_vec()),
        (7, record.schema_epoch_id.as_bytes().to_vec()),
        (8, record.policy_root_id.as_bytes().to_vec()),
        (9, dependencies),
    ])
    .map_err(|_| branch_error(BranchErrorCode::RefFieldShape))?;
    let (preimage, digest, stored_bytes) = build_envelope(
        REF_MAGIC,
        REF_DIGEST_DOMAIN,
        &payload,
        BranchErrorCode::BranchResourceLimit,
    )?;
    Ok(ImportedBranchRef {
        record: record.clone(),
        digest,
        preimage,
        stored_bytes,
    })
}

/// Strictly imports one visible branch ref.
///
/// # Errors
///
/// Returns the first exact envelope, digest, name, field, or resource failure.
pub fn import_branch_ref(stored: &[u8]) -> Result<ImportedBranchRef, BranchError> {
    let (payload, preimage, digest) = decode_envelope(
        stored,
        REF_MAGIC,
        REF_DIGEST_DOMAIN,
        BranchErrorCode::RefFormatVersion,
        BranchErrorCode::RefDigestMismatch,
        BranchErrorCode::RefFieldShape,
    )?;
    let fields = decode_required_record(payload, REF_FIELD_COUNT, BranchErrorCode::RefFieldShape)?;
    let format_version = read_u32(fields[0], BranchErrorCode::RefFieldShape)?;
    if format_version != RECORD_VERSION {
        return Err(branch_error(BranchErrorCode::RefFormatVersion));
    }
    let record = BranchRefRecord {
        format_version,
        branch_name: BranchName::parse(read_bytes(fields[1], BranchErrorCode::RefFieldShape)?)?,
        branch_record_digest: BranchRecordDigest::from_bytes(read_fixed(
            fields[2],
            BranchErrorCode::RefFieldShape,
        )?),
        workspace_id: WorkspaceId::from_bytes(read_fixed(
            fields[3],
            BranchErrorCode::RefFieldShape,
        )?),
        head_transaction_id: TransactionId::from_bytes(read_fixed(
            fields[4],
            BranchErrorCode::RefFieldShape,
        )?),
        head_state_root: StateRoot::from_bytes(read_fixed(
            fields[5],
            BranchErrorCode::RefFieldShape,
        )?),
        schema_epoch_id: SchemaEpochId::from_bytes(read_fixed(
            fields[6],
            BranchErrorCode::RefFieldShape,
        )?),
        policy_root_id: PolicyRootId::from_bytes(read_fixed(
            fields[7],
            BranchErrorCode::RefFieldShape,
        )?),
        dependency_roots: decode_dependencies(fields[8], BranchErrorCode::RefFieldShape)?,
    };
    Ok(ImportedBranchRef {
        record,
        digest,
        preimage,
        stored_bytes: stored.to_vec(),
    })
}

/// One fully resolved visible branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBranch {
    /// Strict immutable origin record.
    pub origin: ImportedBranchRecord,
    /// Strict current visible ref.
    pub reference: ImportedBranchRef,
    /// Fully verified current durable revision.
    pub revision: VerifiedRevision,
}

/// One deterministic head-first ancestry entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchAncestryEntry {
    /// Exact transaction identity.
    pub transaction_id: TransactionId,
    /// Ancestry-independent semantic root.
    pub state_root: StateRoot,
    /// Ordered direct parent transaction identities.
    pub parent_transaction_ids: Vec<TransactionId>,
}

/// One immutable origin record with no visible ref.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrphanBranchOrigin {
    /// Exact canonical branch name.
    pub branch_name: BranchName,
    /// Immutable origin-record digest.
    pub branch_record_digest: BranchRecordDigest,
    /// Fully recorded origin transaction.
    pub origin_transaction_id: TransactionId,
}

/// Idempotent S20-500-owned recovery summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefRecoveryReport {
    /// Removed immutable-origin staging remnants.
    pub removed_branch_stages: u64,
    /// Removed mutable-ref staging remnants.
    pub removed_ref_stages: u64,
    /// Number of visible refs fully verified after cleanup.
    pub visible_branches: u64,
    /// Sorted immutable origins without a visible ref.
    pub orphan_origins: Vec<OrphanBranchOrigin>,
    /// Unique transactions verified across all visible branch ancestries.
    pub verified_ancestry_transactions: u64,
}

/// Repository-rooted native branch/ref owner.
#[derive(Clone, Debug)]
pub struct BranchRepository {
    root: PathBuf,
    transactions: TransactionRepository,
}

impl BranchRepository {
    /// Creates a repository handle. The explicit root must already exist.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            transactions: TransactionRepository::new(root.clone()),
            root,
        }
    }

    /// Returns the configured repository root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Creates one named branch at an already durable verified transaction.
    ///
    /// # Errors
    ///
    /// Returns the first exact name, layout, origin, conflict, persistence, or
    /// upstream transaction failure.
    pub fn create_branch(
        &self,
        name: impl AsRef<[u8]>,
        origin_transaction_id: TransactionId,
    ) -> Result<BranchUpdateStatus, BranchError> {
        let maintenance = self.acquire_shared_maintenance()?;
        self.create_branch_with_maintenance(name, origin_transaction_id, &maintenance)
    }

    fn create_branch_with_maintenance(
        &self,
        name: impl AsRef<[u8]>,
        origin_transaction_id: TransactionId,
        maintenance: &RepositoryMaintenanceGuard,
    ) -> Result<BranchUpdateStatus, BranchError> {
        self.create_branch_with_maintenance_inner(name, origin_transaction_id, maintenance)
    }

    fn create_branch_with_maintenance_inner(
        &self,
        name: impl AsRef<[u8]>,
        origin_transaction_id: TransactionId,
        maintenance: &RepositoryMaintenanceGuard,
    ) -> Result<BranchUpdateStatus, BranchError> {
        self.validate_maintenance(maintenance)?;
        let name = BranchName::parse(name)?;
        self.ensure_layout_under_maintenance()?;
        let _lock = self.acquire_refs_lock()?;
        let branch_path = ensure_key_path(&self.branches_dir(), &name, ".branch.scb1", 2, 3)?;
        let ref_path = ensure_key_path(&self.refs_dir(), &name, ".ref.scb1", 0, 0)?;
        let branch_exists = path_exists(&branch_path)?;
        let ref_exists = path_exists(&ref_path)?;

        if ref_exists && !branch_exists {
            return Err(branch_error(BranchErrorCode::RecoveryNamedRefIncomplete));
        }

        let existing_origin = branch_exists
            .then(|| self.read_branch_at(&branch_path))
            .transpose()?;
        let existing_ref = ref_exists
            .then(|| self.read_ref_at(&ref_path))
            .transpose()?;

        if let (Some(origin), Some(reference)) = (existing_origin.as_ref(), existing_ref.as_ref()) {
            return self.resolve_existing_create(
                maintenance,
                origin_transaction_id,
                &branch_path,
                &ref_path,
                origin,
                reference,
            );
        }

        if let Some(origin) = existing_origin {
            return self.finish_orphan_create(
                maintenance,
                &branch_path,
                &ref_path,
                &name,
                origin_transaction_id,
                &origin,
            );
        }

        self.create_fresh(
            maintenance,
            &branch_path,
            &ref_path,
            &name,
            origin_transaction_id,
        )
    }

    fn resolve_existing_create(
        &self,
        maintenance: &RepositoryMaintenanceGuard,
        requested_origin: TransactionId,
        branch_path: &Path,
        ref_path: &Path,
        origin: &ImportedBranchRecord,
        reference: &ImportedBranchRef,
    ) -> Result<BranchUpdateStatus, BranchError> {
        validate_origin_ref_binding(origin, reference)?;
        let origin_revision = self
            .transactions
            .verified_revision_with_maintenance(maintenance, origin.record.origin_transaction_id)?;
        verify_origin_target(&origin.record, &origin_revision)?;
        let current = self.transactions.verified_revision_with_maintenance(
            maintenance,
            reference.record.head_transaction_id,
        )?;
        verify_ref_target(&reference.record, &current)?;
        if reference.record.workspace_id != origin.record.workspace_id
            || origin.record.origin_transaction_id != requested_origin
        {
            return Err(branch_error(BranchErrorCode::BranchOriginMismatch));
        }
        if reference.record.head_transaction_id != requested_origin {
            return Err(branch_error(BranchErrorCode::RefAlreadyExists));
        }
        redurabilize_branch(branch_path, origin)?;
        redurabilize_ref(ref_path, reference)?;
        Ok(BranchUpdateStatus::Present)
    }

    fn finish_orphan_create(
        &self,
        maintenance: &RepositoryMaintenanceGuard,
        branch_path: &Path,
        ref_path: &Path,
        name: &BranchName,
        requested_origin: TransactionId,
        origin: &ImportedBranchRecord,
    ) -> Result<BranchUpdateStatus, BranchError> {
        let revision = self
            .transactions
            .verified_revision_with_maintenance(maintenance, origin.record.origin_transaction_id)?;
        verify_origin_target(&origin.record, &revision)?;
        if origin.record.origin_transaction_id != requested_origin {
            return Err(branch_error(BranchErrorCode::BranchOriginMismatch));
        }
        self.validate_new_visible_capacity()?;
        redurabilize_branch(branch_path, origin)?;
        let desired_ref = build_branch_ref(&ref_record(name, origin.digest, &revision))?;
        persist_expected_ref(ref_path, &desired_ref)?;
        let visible = self.resolve_locked(maintenance, name)?;
        if visible.reference != desired_ref {
            return Err(branch_error(BranchErrorCode::RefInternalInvariant));
        }
        Ok(BranchUpdateStatus::Created)
    }

    fn create_fresh(
        &self,
        maintenance: &RepositoryMaintenanceGuard,
        branch_path: &Path,
        ref_path: &Path,
        name: &BranchName,
        origin_transaction_id: TransactionId,
    ) -> Result<BranchUpdateStatus, BranchError> {
        self.create_fresh_with_origin_limit(
            maintenance,
            branch_path,
            ref_path,
            name,
            origin_transaction_id,
            MAX_BRANCH_ORIGINS,
        )
    }

    fn create_fresh_with_origin_limit(
        &self,
        maintenance: &RepositoryMaintenanceGuard,
        branch_path: &Path,
        ref_path: &Path,
        name: &BranchName,
        origin_transaction_id: TransactionId,
        maximum_origins: usize,
    ) -> Result<BranchUpdateStatus, BranchError> {
        let revision = self
            .transactions
            .verified_revision_with_maintenance(maintenance, origin_transaction_id)?;
        let desired_origin = build_branch_record(&origin_record(name, &revision))?;
        let desired_ref = build_branch_ref(&ref_record(name, desired_origin.digest, &revision))?;
        self.validate_new_origin_capacity(maximum_origins)?;
        self.validate_new_visible_capacity()?;
        persist_no_overwrite(
            branch_path,
            &desired_origin.stored_bytes,
            BRANCH_STAGE_PREFIX,
            BranchErrorCode::BranchOriginMismatch,
            |bytes| {
                if import_branch_record(bytes)? == desired_origin {
                    Ok(())
                } else {
                    Err(branch_error(BranchErrorCode::BranchOriginMismatch))
                }
            },
        )?;
        persist_expected_ref(ref_path, &desired_ref)?;
        let visible = self.resolve_locked(maintenance, name)?;
        if visible.reference != desired_ref {
            return Err(branch_error(BranchErrorCode::RefInternalInvariant));
        }
        Ok(BranchUpdateStatus::Created)
    }

    fn validate_new_origin_capacity(&self, maximum: usize) -> Result<(), BranchError> {
        let origin_count = enumerate_record_paths(
            &self.branches_dir(),
            ".branch.scb1",
            maximum,
            Some(BRANCH_STAGE_PREFIX),
        )?
        .len();
        validate_origin_capacity(origin_count, maximum)
    }

    fn validate_new_visible_capacity(&self) -> Result<(), BranchError> {
        let visible_count = enumerate_record_paths(
            &self.refs_dir(),
            ".ref.scb1",
            MAX_BRANCHES,
            Some(REF_STAGE_PREFIX),
        )?
        .len();
        validate_visible_capacity(visible_count)
    }

    /// Resolves one visible branch through immutable records and durable
    /// transaction evidence.
    ///
    /// # Errors
    ///
    /// Returns no partial branch and preserves the first exact owning failure.
    pub fn resolve_branch(&self, name: impl AsRef<[u8]>) -> Result<ResolvedBranch, BranchError> {
        let name = BranchName::parse(name)?;
        let maintenance = self.prepare_operation()?;
        let _lock = self.acquire_refs_lock()?;
        self.resolve_locked(&maintenance, &name)
    }

    /// Lists one deterministic, fully verified snapshot of visible branches.
    ///
    /// # Errors
    ///
    /// Fails closed on an invalid limit, unknown path, malformed record, or
    /// any unresolved visible branch.
    pub fn list_branches(&self, limit: usize) -> Result<Vec<ResolvedBranch>, BranchError> {
        validate_branch_limit(limit)?;
        let maintenance = self.prepare_operation()?;
        let _lock = self.acquire_refs_lock()?;
        self.list_branches_locked(&maintenance, limit)
    }

    /// Advances one visible branch by an exact direct-parent compare-and-swap.
    ///
    /// # Errors
    ///
    /// Returns exact stale and non-fast-forward failures without
    /// last-write-wins behavior.
    pub fn advance_branch(
        &self,
        name: impl AsRef<[u8]>,
        expected_head: TransactionId,
        new_head: TransactionId,
    ) -> Result<BranchUpdateStatus, BranchError> {
        let maintenance = self.acquire_shared_maintenance()?;
        self.advance_branch_with_maintenance(name, expected_head, new_head, &maintenance)
    }

    fn advance_branch_with_maintenance(
        &self,
        name: impl AsRef<[u8]>,
        expected_head: TransactionId,
        new_head: TransactionId,
        maintenance: &RepositoryMaintenanceGuard,
    ) -> Result<BranchUpdateStatus, BranchError> {
        self.validate_maintenance(maintenance)?;
        let name = BranchName::parse(name)?;
        self.ensure_layout_under_maintenance()?;
        let _lock = self.acquire_refs_lock()?;
        let current = self.resolve_locked(maintenance, &name)?;
        if current.reference.record.head_transaction_id == new_head {
            redurabilize_branch(&self.checked_branch_path(&name)?, &current.origin)?;
            redurabilize_ref(&self.checked_ref_path(&name)?, &current.reference)?;
            return Ok(BranchUpdateStatus::Present);
        }
        if current.reference.record.head_transaction_id != expected_head {
            return Err(branch_error(BranchErrorCode::RefNamedCasStale));
        }
        let revision = self
            .transactions
            .verified_revision_with_maintenance(maintenance, new_head)?;
        if revision.state_root().record.workspace_id != current.origin.record.workspace_id {
            return Err(branch_error(BranchErrorCode::BranchOriginMismatch));
        }
        let parents = &revision.receipt().transaction.record.parent_transaction_ids;
        if parents
            .iter()
            .filter(|parent| **parent == expected_head)
            .count()
            != 1
        {
            return Err(branch_error(BranchErrorCode::BranchNotFastForward));
        }
        let desired = build_branch_ref(&ref_record(&name, current.origin.digest, &revision))?;
        let path = self.checked_ref_path(&name)?;
        replace_ref(&path, &desired)?;
        let visible = self.resolve_locked(maintenance, &name)?;
        if visible.reference != desired {
            return Err(branch_error(BranchErrorCode::RefInternalInvariant));
        }
        Ok(BranchUpdateStatus::Advanced)
    }

    /// Traverses deterministic head-first verified transaction ancestry.
    ///
    /// # Errors
    ///
    /// Returns a hard resource or cycle failure with no partial success and
    /// preserves exact upstream revision failures.
    pub fn branch_ancestry(
        &self,
        name: impl AsRef<[u8]>,
        max_nodes: usize,
    ) -> Result<Vec<BranchAncestryEntry>, BranchError> {
        validate_ancestry_limit(max_nodes)?;
        let name = BranchName::parse(name)?;
        let maintenance = self.prepare_operation()?;
        let _lock = self.acquire_refs_lock()?;
        let branch = self.resolve_locked(&maintenance, &name)?;
        walk_ancestry(
            branch.reference.record.head_transaction_id,
            max_nodes,
            |transaction_id| {
                let revision = self
                    .transactions
                    .verified_revision_with_maintenance(&maintenance, transaction_id)?;
                Ok(BranchAncestryEntry {
                    transaction_id,
                    state_root: revision.state_root().root,
                    parent_transaction_ids: revision
                        .receipt()
                        .transaction
                        .record
                        .parent_transaction_ids
                        .clone(),
                })
            },
        )
    }

    /// Removes only S20-500-owned staging remnants, verifies every visible
    /// branch, and reports immutable orphan origins without deleting them.
    ///
    /// # Errors
    ///
    /// Returns the first exact confinement, cleanup, record, or target failure.
    pub fn recover_refs(&self) -> Result<RefRecoveryReport, BranchError> {
        let maintenance = self.acquire_exclusive_maintenance()?;
        let result = self.recover_refs_with_maintenance(&maintenance);
        #[cfg(test)]
        hold_ref_recovery_before_exclusive_drop(&self.root, &maintenance);
        result
    }

    /// Recovers ref-owned state while the caller holds exclusive same-root
    /// repository maintenance.
    ///
    /// # Errors
    ///
    /// Returns `REF_IO` for a shared or wrong-root guard, or the first exact
    /// confinement, limit, record, ancestry, or cleanup failure.
    pub fn recover_refs_with_maintenance(
        &self,
        maintenance: &RepositoryMaintenanceGuard,
    ) -> Result<RefRecoveryReport, BranchError> {
        self.recover_refs_with_maintenance_and_limits(maintenance, ref_recovery_limits())
    }

    #[allow(clippy::too_many_lines)]
    fn recover_refs_with_maintenance_and_limits(
        &self,
        maintenance: &RepositoryMaintenanceGuard,
        limits: RefRecoveryLimits,
    ) -> Result<RefRecoveryReport, BranchError> {
        self.validate_exclusive_maintenance(maintenance)?;
        #[cfg(test)]
        let _recovery_ancestry_operation =
            ::sley_txn::recovery_ancestry_test_hook::begin_ref_operation(
                &self.transactions,
                maintenance,
            );
        self.ensure_layout_under_maintenance()?;
        let _lock = self.acquire_refs_lock()?;

        let mut usage = RefRecoveryUsage::default();
        let mut pending_origin_directories = vec![(self.branches_dir(), 0_usize)];
        let mut origin_leaf_directories = Vec::new();
        let mut final_origins = Vec::new();
        let mut origin_removal_plan = Vec::new();
        while let Some((directory, depth)) = pending_origin_directories.pop() {
            ensure_existing_directory(&directory)?;
            if depth == 2 {
                origin_leaf_directories.push(directory.clone());
            }
            for entry in fs::read_dir(&directory)? {
                let entry = entry?;
                if depth < 2 {
                    let classified_origin_fanout_path =
                        classify_record_recovery_fanout(&entry, depth)?;
                    let fanout_path = classified_origin_fanout_path;
                    let one = 1_u64;
                    let next_origin_fanout_directories = usage
                        .origin_fanout_directories
                        .checked_add(one)
                        .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
                    ensure_ref_recovery_limit(
                        next_origin_fanout_directories,
                        limits.origin_fanout_directories,
                    )?;
                    usage.origin_fanout_directories = next_origin_fanout_directories;
                    pending_origin_directories.push(fanout_path);
                    continue;
                }
                let next_origin_leaf_path = entry.path();
                let leaf_path = next_origin_leaf_path;
                let one = 1_u64;
                let next_origin_leaf_entries = usage
                    .origin_leaf_entries
                    .checked_add(one)
                    .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
                ensure_ref_recovery_limit(next_origin_leaf_entries, limits.origin_leaf_entries)?;
                usage.origin_leaf_entries = next_origin_leaf_entries;
                classify_origin_recovery_leaf(&leaf_path)?;
                match origin_recovery_leaf_kind(&leaf_path)? {
                    RefRecoveryLeafKind::Final => {
                        let classified_final_origin = leaf_path;
                        let final_origin = classified_final_origin;
                        let one = 1_u64;
                        let next_final_origins = usage
                            .final_origins
                            .checked_add(one)
                            .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
                        ensure_ref_recovery_limit(next_final_origins, limits.final_origins)?;
                        usage.final_origins = next_final_origins;
                        final_origins.push(final_origin);
                    }
                    RefRecoveryLeafKind::OwnedStage => {
                        let classified_origin_stage_path = leaf_path;
                        let origin_stage_path = classified_origin_stage_path;
                        let one = 1_u64;
                        let next_origin_stages = usage
                            .origin_stages
                            .checked_add(one)
                            .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
                        ensure_ref_recovery_limit(next_origin_stages, limits.origin_stages)?;
                        usage.origin_stages = next_origin_stages;
                        origin_removal_plan.push(origin_stage_path);
                    }
                    RefRecoveryLeafKind::Unknown => {}
                }
            }
        }
        final_origins.sort();
        origin_leaf_directories.sort();
        origin_removal_plan.sort();
        let origin_plan = RefRecoveryScanPlan {
            final_paths: final_origins,
            leaf_directories: origin_leaf_directories,
            removal_plan: origin_removal_plan,
        };

        let mut pending_ref_directories = vec![(self.refs_dir(), 0_usize)];
        let mut ref_leaf_directories = Vec::new();
        let mut final_refs = Vec::new();
        let mut ref_removal_plan = Vec::new();
        while let Some((directory, depth)) = pending_ref_directories.pop() {
            ensure_existing_directory(&directory)?;
            if depth == 2 {
                ref_leaf_directories.push(directory.clone());
            }
            for entry in fs::read_dir(&directory)? {
                let entry = entry?;
                if depth < 2 {
                    let classified_ref_fanout_path =
                        classify_record_recovery_fanout(&entry, depth)?;
                    let fanout_path = classified_ref_fanout_path;
                    let one = 1_u64;
                    let next_ref_fanout_directories = usage
                        .ref_fanout_directories
                        .checked_add(one)
                        .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
                    ensure_ref_recovery_limit(
                        next_ref_fanout_directories,
                        limits.ref_fanout_directories,
                    )?;
                    usage.ref_fanout_directories = next_ref_fanout_directories;
                    pending_ref_directories.push(fanout_path);
                    continue;
                }
                let next_ref_leaf_path = entry.path();
                let leaf_path = next_ref_leaf_path;
                let one = 1_u64;
                let next_ref_leaf_entries = usage
                    .ref_leaf_entries
                    .checked_add(one)
                    .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
                ensure_ref_recovery_limit(next_ref_leaf_entries, limits.ref_leaf_entries)?;
                usage.ref_leaf_entries = next_ref_leaf_entries;
                classify_ref_recovery_leaf(&leaf_path)?;
                match ref_recovery_leaf_kind(&leaf_path)? {
                    RefRecoveryLeafKind::Final => {
                        final_refs.push(leaf_path);
                    }
                    RefRecoveryLeafKind::OwnedStage => {
                        let classified_ref_stage_path = leaf_path;
                        let ref_stage_path = classified_ref_stage_path;
                        let one = 1_u64;
                        let next_ref_stages = usage
                            .ref_stages
                            .checked_add(one)
                            .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
                        ensure_ref_recovery_limit(next_ref_stages, limits.ref_stages)?;
                        usage.ref_stages = next_ref_stages;
                        ref_removal_plan.push(ref_stage_path);
                    }
                    RefRecoveryLeafKind::Unknown => {}
                }
            }
        }
        final_refs.sort();
        ref_leaf_directories.sort();
        ref_removal_plan.sort();
        let ref_plan = RefRecoveryScanPlan {
            final_paths: final_refs,
            leaf_directories: ref_leaf_directories,
            removal_plan: ref_removal_plan,
        };

        let record_preflight = self.preflight_ref_records_with_limits(
            &origin_plan.final_paths,
            &ref_plan.final_paths,
            &limits,
            &mut usage,
        )?;
        let visible = &record_preflight.visible;
        let ancestry_requests = visible
            .iter()
            .map(|branch| {
                RecoveryAncestryRequest::with_claims(
                    RecoveryRevisionClaim::new(
                        branch.origin.record.origin_transaction_id,
                        branch.origin.record.workspace_id,
                        branch.origin.record.origin_state_root,
                        branch.origin.record.schema_epoch_id,
                        branch.origin.record.policy_root_id,
                        branch.origin.record.dependency_roots.clone(),
                    ),
                    RecoveryRevisionClaim::new(
                        branch.reference.record.head_transaction_id,
                        branch.reference.record.workspace_id,
                        branch.reference.record.head_state_root,
                        branch.reference.record.schema_epoch_id,
                        branch.reference.record.policy_root_id,
                        branch.reference.record.dependency_roots.clone(),
                    ),
                )
            })
            .collect::<r#Vec<_>>();
        let ancestry_report = self
            .transactions
            .verify_branch_recovery_ancestries_with_maintenance(maintenance, &ancestry_requests)
            .map_err(map_recovery_ancestry_error)?;

        let removed_branch_stages = remove_planned_origin_stages(&origin_plan.removal_plan)?;
        for directory in &origin_plan.leaf_directories {
            ensure_existing_directory(directory)?;
            sync_dir(directory)?;
        }
        let removed_ref_stages = remove_planned_ref_stages(&ref_plan.removal_plan)?;
        for directory in &ref_plan.leaf_directories {
            ensure_existing_directory(directory)?;
            sync_dir(directory)?;
        }
        Ok(RefRecoveryReport {
            removed_branch_stages,
            removed_ref_stages,
            visible_branches: usize_to_u64(record_preflight.visible.len())?,
            orphan_origins: record_preflight.orphan_origins,
            verified_ancestry_transactions: ancestry_report.verified_transactions,
        })
    }

    #[allow(clippy::unused_self)]
    fn read_recovery_visible_ref_with_limits(
        &self,
        visible_ref: &RecoveryRecordPath,
        limits: &RefRecoveryLimits,
        usage: &mut RefRecoveryUsage,
    ) -> Result<ImportedBranchRef, BranchError> {
        let record_path = visible_ref.path.clone();
        let metadata = ::std::fs::symlink_metadata(&record_path).map_err(BranchError::from)?;
        ensure_recovery_regular_file(&metadata)?;
        let metadata_bytes = metadata.len();
        let one = 1_u64;
        let next_visible_branches = usage
            .visible_branches
            .checked_add(one)
            .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
        let next_visible_ref_record_bytes = usage
            .visible_ref_record_bytes
            .checked_add(metadata_bytes)
            .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
        ensure_ref_recovery_limit(next_visible_branches, limits.visible_branches)?;
        ensure_ref_recovery_limit(
            next_visible_ref_record_bytes,
            limits.visible_ref_record_bytes,
        )?;
        usage.visible_branches = next_visible_branches;
        usage.visible_ref_record_bytes = next_visible_ref_record_bytes;
        let decoded_ref = read_recovery_ref_record(&record_path)?;
        Ok(decoded_ref)
    }

    #[allow(clippy::unused_self)]
    fn read_recovery_visible_origin_with_limits(
        &self,
        visible_origin: &RecoveryRecordPath,
        limits: &RefRecoveryLimits,
        usage: &mut RefRecoveryUsage,
    ) -> Result<ImportedBranchRecord, BranchError> {
        let origin_path = visible_origin.path.clone();
        let metadata = ::std::fs::symlink_metadata(&origin_path).map_err(BranchError::from)?;
        ensure_recovery_regular_file(&metadata)?;
        let metadata_bytes = metadata.len();
        let next_visible_origin_record_bytes = usage
            .origin_record_bytes
            .checked_add(metadata_bytes)
            .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
        ensure_ref_recovery_limit(next_visible_origin_record_bytes, limits.origin_record_bytes)?;
        usage.origin_record_bytes = next_visible_origin_record_bytes;
        let decoded_origin = read_recovery_origin_record(&origin_path)?;
        Ok(decoded_origin)
    }

    #[allow(clippy::unused_self)]
    fn read_recovery_orphan_origin_with_limits(
        &self,
        orphan_origin: &RecoveryRecordPath,
        limits: &RefRecoveryLimits,
        usage: &mut RefRecoveryUsage,
        orphan_origins: &mut Vec<ImportedBranchRecord>,
    ) -> Result<(), BranchError> {
        let origin_path = orphan_origin.path.clone();
        let metadata = ::std::fs::symlink_metadata(&origin_path).map_err(BranchError::from)?;
        ensure_recovery_regular_file(&metadata)?;
        let metadata_bytes = metadata.len();
        let one = 1_u64;
        let next_orphan_origins = usage
            .orphan_origins
            .checked_add(one)
            .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
        let next_orphan_origin_record_bytes = usage
            .origin_record_bytes
            .checked_add(metadata_bytes)
            .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
        ensure_ref_recovery_limit(next_orphan_origins, limits.orphan_origins)?;
        ensure_ref_recovery_limit(next_orphan_origin_record_bytes, limits.origin_record_bytes)?;
        usage.orphan_origins = next_orphan_origins;
        usage.origin_record_bytes = next_orphan_origin_record_bytes;
        let decoded_origin = read_recovery_origin_record(&origin_path)?;
        orphan_origins.push(decoded_origin);
        Ok(())
    }

    fn preflight_ref_records_with_limits(
        &self,
        origin_paths: &[PathBuf],
        ref_paths: &[PathBuf],
        limits: &RefRecoveryLimits,
        usage: &mut RefRecoveryUsage,
    ) -> Result<RefRecordPreflight, BranchError> {
        let mut references = Vec::new();
        for path in ref_paths {
            let visible_ref = RecoveryRecordPath { path: path.clone() };
            references.push(self.read_recovery_visible_ref_with_limits(
                &visible_ref,
                limits,
                usage,
            )?);
        }
        references.sort_by(|left, right| left.record.branch_name.cmp(&right.record.branch_name));
        if references
            .windows(2)
            .any(|pair| pair[0].record.branch_name == pair[1].record.branch_name)
        {
            return Err(branch_error(BranchErrorCode::RefNameCollision));
        }
        let origin_paths = origin_paths.iter().cloned().collect::<BTreeSet<_>>();
        let mut consumed_origins = BTreeSet::new();
        let mut visible = Vec::with_capacity(references.len());
        for reference in references {
            let origin_path = self.checked_branch_path(&reference.record.branch_name)?;
            if !origin_paths.contains(&origin_path) {
                return Err(branch_error(BranchErrorCode::RecoveryNamedRefIncomplete));
            }
            let visible_origin = RecoveryRecordPath {
                path: origin_path.clone(),
            };
            let origin =
                self.read_recovery_visible_origin_with_limits(&visible_origin, limits, usage)?;
            if origin.record.branch_name != reference.record.branch_name {
                return Err(branch_error(BranchErrorCode::RefNameCollision));
            }
            validate_origin_ref_binding(&origin, &reference)?;
            consumed_origins.insert(origin_path);
            visible.push(RecoveryVisibleBranch { origin, reference });
        }
        let mut orphan_origin_records = Vec::new();
        for path in origin_paths.difference(&consumed_origins) {
            let orphan_origin = RecoveryRecordPath { path: path.clone() };
            self.read_recovery_orphan_origin_with_limits(
                &orphan_origin,
                limits,
                usage,
                &mut orphan_origin_records,
            )?;
        }
        let mut orphan_origins = Vec::with_capacity(orphan_origin_records.len());
        for origin in orphan_origin_records {
            orphan_origins.push(OrphanBranchOrigin {
                branch_name: origin.record.branch_name,
                branch_record_digest: origin.digest,
                origin_transaction_id: origin.record.origin_transaction_id,
            });
        }
        orphan_origins.sort_by(|left, right| left.branch_name.cmp(&right.branch_name));
        Ok(RefRecordPreflight {
            visible,
            orphan_origins,
        })
    }

    fn resolve_locked(
        &self,
        maintenance: &RepositoryMaintenanceGuard,
        name: &BranchName,
    ) -> Result<ResolvedBranch, BranchError> {
        let ref_path = self.checked_ref_path(name)?;
        let branch_path = self.checked_branch_path(name)?;
        if !path_exists(&ref_path)? {
            return Err(branch_error(BranchErrorCode::RefNotFound));
        }
        if !path_exists(&branch_path)? {
            return Err(branch_error(BranchErrorCode::RecoveryNamedRefIncomplete));
        }
        let origin = self.read_branch_at(&branch_path)?;
        let reference = self.read_ref_at(&ref_path)?;
        if origin.record.branch_name != *name || reference.record.branch_name != *name {
            return Err(branch_error(BranchErrorCode::RefNameCollision));
        }
        validate_origin_ref_binding(&origin, &reference)?;
        let origin_revision = self
            .transactions
            .verified_revision_with_maintenance(maintenance, origin.record.origin_transaction_id)?;
        verify_origin_target(&origin.record, &origin_revision)?;
        let revision = self.transactions.verified_revision_with_maintenance(
            maintenance,
            reference.record.head_transaction_id,
        )?;
        verify_ref_target(&reference.record, &revision)?;
        if reference.record.workspace_id != origin.record.workspace_id {
            return Err(branch_error(BranchErrorCode::BranchOriginMismatch));
        }
        Ok(ResolvedBranch {
            origin,
            reference,
            revision,
        })
    }

    fn list_branches_locked(
        &self,
        maintenance: &RepositoryMaintenanceGuard,
        limit: usize,
    ) -> Result<Vec<ResolvedBranch>, BranchError> {
        let paths = enumerate_record_paths(&self.refs_dir(), ".ref.scb1", MAX_BRANCHES, None)?;
        if paths.len() > limit {
            return Err(branch_error(BranchErrorCode::BranchResourceLimit));
        }
        let mut names = Vec::with_capacity(paths.len());
        for path in paths {
            names.push(self.read_ref_at(&path)?.record.branch_name);
        }
        names.sort();
        if names.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(branch_error(BranchErrorCode::RefNameCollision));
        }
        names
            .iter()
            .map(|name| self.resolve_locked(maintenance, name))
            .collect()
    }

    fn read_branch_at(&self, path: &Path) -> Result<ImportedBranchRecord, BranchError> {
        let imported = import_branch_record(&bounded_read(path, MAX_STANDALONE_BYTES)?)?;
        if self.branch_path(&imported.record.branch_name) != path {
            return Err(branch_error(BranchErrorCode::RefNameCollision));
        }
        Ok(imported)
    }

    fn read_ref_at(&self, path: &Path) -> Result<ImportedBranchRef, BranchError> {
        let imported = import_branch_ref(&bounded_read(path, MAX_STANDALONE_BYTES)?)?;
        if self.ref_path(&imported.record.branch_name) != path {
            return Err(branch_error(BranchErrorCode::RefNameCollision));
        }
        Ok(imported)
    }

    fn prepare_operation(&self) -> Result<RepositoryMaintenanceGuard, BranchError> {
        let maintenance = self.acquire_shared_maintenance()?;
        self.ensure_layout_under_maintenance()?;
        Ok(maintenance)
    }

    fn acquire_shared_maintenance(&self) -> Result<RepositoryMaintenanceGuard, BranchError> {
        Ok(self.transactions.acquire_shared_maintenance()?)
    }

    fn acquire_exclusive_maintenance(&self) -> Result<RepositoryMaintenanceGuard, BranchError> {
        Ok(self.transactions.acquire_exclusive_maintenance()?)
    }

    fn validate_maintenance(
        &self,
        maintenance: &RepositoryMaintenanceGuard,
    ) -> Result<(), BranchError> {
        if !maintenance.covers(&self.root) {
            return Err(BranchError::Io(io::Error::other(
                "repository maintenance guard does not cover the branch repository root",
            )));
        }
        Ok(())
    }

    fn validate_exclusive_maintenance(
        &self,
        maintenance: &RepositoryMaintenanceGuard,
    ) -> Result<(), BranchError> {
        self.validate_maintenance(maintenance)?;
        if !maintenance.is_exclusive() {
            return Err(BranchError::Io(io::Error::other(
                "branch recovery requires exclusive repository maintenance ownership",
            )));
        }
        Ok(())
    }

    #[cfg(test)]
    fn ensure_layout(&self) -> Result<(), BranchError> {
        let _maintenance = self.prepare_operation()?;
        Ok(())
    }

    fn ensure_layout_under_maintenance(&self) -> Result<(), BranchError> {
        ensure_existing_directory(&self.root)?;
        let branches = create_dir_component(&self.root, "branches", 0)?;
        create_dir_component(&branches, "v1", 1)?;
        let refs = create_dir_component(&self.root, "refs", 0)?;
        create_dir_component(&refs, "v1", 0)?;
        create_dir_component(&self.root, "locks", 0)?;
        Ok(())
    }

    fn acquire_refs_lock(&self) -> Result<File, BranchError> {
        let path = self.root.join("locks").join("refs.lock");
        reject_symlink_if_present(&path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(branch_error(BranchErrorCode::RefIo));
        }
        file.lock()?;
        Ok(file)
    }

    #[cfg(test)]
    fn ensure_branch_path(&self, name: &BranchName) -> Result<PathBuf, BranchError> {
        ensure_key_path(&self.branches_dir(), name, ".branch.scb1", 0, 0)
    }

    #[cfg(test)]
    fn ensure_ref_path(&self, name: &BranchName) -> Result<PathBuf, BranchError> {
        ensure_key_path(&self.refs_dir(), name, ".ref.scb1", 0, 0)
    }

    fn checked_branch_path(&self, name: &BranchName) -> Result<PathBuf, BranchError> {
        checked_key_path(&self.branches_dir(), name, ".branch.scb1")
    }

    fn checked_ref_path(&self, name: &BranchName) -> Result<PathBuf, BranchError> {
        checked_key_path(&self.refs_dir(), name, ".ref.scb1")
    }

    fn branch_path(&self, name: &BranchName) -> PathBuf {
        key_path(&self.branches_dir(), name, ".branch.scb1")
    }

    fn ref_path(&self, name: &BranchName) -> PathBuf {
        key_path(&self.refs_dir(), name, ".ref.scb1")
    }

    fn branches_dir(&self) -> PathBuf {
        self.root.join("branches").join("v1")
    }

    fn refs_dir(&self) -> PathBuf {
        self.root.join("refs").join("v1")
    }

    #[cfg(test)]
    fn ensure_layout_with_native_ref_durability_cut(
        &self,
        cut: NativeRefDurabilityCut,
    ) -> Result<(), BranchError> {
        let _selection = NativeRefCutSelection::install(cut);
        self.ensure_layout()
    }

    #[cfg(test)]
    fn create_branch_with_native_ref_durability_cut(
        &self,
        name: impl AsRef<[u8]>,
        origin_transaction_id: TransactionId,
        cut: NativeRefDurabilityCut,
    ) -> Result<BranchUpdateStatus, BranchError> {
        let _selection = NativeRefCutSelection::install(cut);
        self.create_branch(name, origin_transaction_id)
    }

    #[cfg(test)]
    fn advance_branch_with_native_ref_durability_cut(
        &self,
        name: impl AsRef<[u8]>,
        expected_head: TransactionId,
        new_head: TransactionId,
        cut: NativeRefDurabilityCut,
    ) -> Result<BranchUpdateStatus, BranchError> {
        let _selection = NativeRefCutSelection::install(cut);
        self.advance_branch(name, expected_head, new_head)
    }

    #[cfg(test)]
    fn recover_refs_with_native_ref_durability_cut(
        &self,
        cut: NativeRefDurabilityCut,
    ) -> Result<RefRecoveryReport, BranchError> {
        let _selection = NativeRefCutSelection::install(cut);
        self.recover_refs()
    }
}

fn origin_record(name: &BranchName, revision: &VerifiedRevision) -> BranchRecord {
    BranchRecord {
        format_version: RECORD_VERSION,
        branch_name: name.clone(),
        workspace_id: revision.state_root().record.workspace_id,
        origin_transaction_id: revision.transaction_id(),
        origin_state_root: revision.state_root().root,
        schema_epoch_id: revision.state_root().record.schema_epoch_id,
        policy_root_id: revision.policy_root().root(),
        dependency_roots: revision.state_root().record.dependency_roots.clone(),
    }
}

fn ref_record(
    name: &BranchName,
    branch_record_digest: BranchRecordDigest,
    revision: &VerifiedRevision,
) -> BranchRefRecord {
    BranchRefRecord {
        format_version: RECORD_VERSION,
        branch_name: name.clone(),
        branch_record_digest,
        workspace_id: revision.state_root().record.workspace_id,
        head_transaction_id: revision.transaction_id(),
        head_state_root: revision.state_root().root,
        schema_epoch_id: revision.state_root().record.schema_epoch_id,
        policy_root_id: revision.policy_root().root(),
        dependency_roots: revision.state_root().record.dependency_roots.clone(),
    }
}

fn verify_origin_target(
    record: &BranchRecord,
    revision: &VerifiedRevision,
) -> Result<(), BranchError> {
    let expected = origin_record(&record.branch_name, revision);
    if *record == expected {
        Ok(())
    } else {
        Err(branch_error(BranchErrorCode::BranchOriginMismatch))
    }
}

fn verify_ref_target(
    record: &BranchRefRecord,
    revision: &VerifiedRevision,
) -> Result<(), BranchError> {
    let expected = ref_record(&record.branch_name, record.branch_record_digest, revision);
    if *record == expected {
        Ok(())
    } else {
        Err(branch_error(BranchErrorCode::RefTargetMismatch))
    }
}

fn validate_origin_ref_binding(
    origin: &ImportedBranchRecord,
    reference: &ImportedBranchRef,
) -> Result<(), BranchError> {
    if origin.record.branch_name != reference.record.branch_name
        || origin.digest != reference.record.branch_record_digest
    {
        return Err(branch_error(BranchErrorCode::RefBranchBindingMismatch));
    }
    Ok(())
}

fn walk_ancestry<F>(
    head: TransactionId,
    max_nodes: usize,
    mut load: F,
) -> Result<Vec<BranchAncestryEntry>, BranchError>
where
    F: FnMut(TransactionId) -> Result<BranchAncestryEntry, BranchError>,
{
    let mut active = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut output = Vec::new();
    let mut stack = vec![(head, false)];
    while let Some((transaction_id, exiting)) = stack.pop() {
        if exiting {
            active.remove(&transaction_id);
            completed.insert(transaction_id);
            continue;
        }
        if completed.contains(&transaction_id) {
            continue;
        }
        if active.contains(&transaction_id) {
            return Err(branch_error(BranchErrorCode::BranchAncestryCycle));
        }
        if output.len() >= max_nodes {
            return Err(branch_error(BranchErrorCode::BranchResourceLimit));
        }
        let entry = load(transaction_id)?;
        if entry.transaction_id != transaction_id {
            return Err(branch_error(BranchErrorCode::RefInternalInvariant));
        }
        active.insert(transaction_id);
        stack.push((transaction_id, true));
        for parent in entry.parent_transaction_ids.iter().rev() {
            stack.push((*parent, false));
        }
        output.push(entry);
    }
    Ok(output)
}

type BuiltEnvelope = (Vec<u8>, [u8; DIGEST_LEN], Vec<u8>);
type DecodedEnvelope<'a> = (&'a [u8], Vec<u8>, [u8; DIGEST_LEN]);

fn build_envelope(
    magic: [u8; 8],
    domain: &[u8],
    payload: &[u8],
    limit_code: BranchErrorCode,
) -> Result<BuiltEnvelope, BranchError> {
    let mut preimage = Vec::with_capacity(magic.len() + payload.len() + 12);
    preimage.extend_from_slice(&magic);
    preimage.extend_from_slice(&encode_uvar(ENVELOPE_VERSION));
    preimage.extend_from_slice(&encode_uvar(
        u64::try_from(payload.len()).map_err(|_| branch_error(limit_code))?,
    ));
    preimage.extend_from_slice(payload);
    let digest = digest(domain, &preimage);
    let total = preimage
        .len()
        .checked_add(DIGEST_LEN)
        .ok_or_else(|| branch_error(limit_code))?;
    if total > MAX_STANDALONE_BYTES {
        return Err(branch_error(limit_code));
    }
    let mut stored = preimage.clone();
    stored.extend_from_slice(&digest);
    Ok((preimage, digest, stored))
}

fn decode_envelope<'a>(
    stored: &'a [u8],
    magic: [u8; 8],
    domain: &[u8],
    version_code: BranchErrorCode,
    digest_code: BranchErrorCode,
    shape_code: BranchErrorCode,
) -> Result<DecodedEnvelope<'a>, BranchError> {
    if stored.len() > MAX_STANDALONE_BYTES {
        return Err(branch_error(BranchErrorCode::BranchResourceLimit));
    }
    let mut cursor = ScbValueCursor::new(stored).map_err(|_| branch_error(shape_code))?;
    let actual_magic = cursor
        .read_fixed_bytes::<8>()
        .map_err(|_| branch_error(shape_code))?;
    if actual_magic != magic {
        return Err(branch_error(shape_code));
    }
    let version = cursor.read_uvar(64).map_err(|_| branch_error(shape_code))?;
    if version != ENVELOPE_VERSION {
        return Err(branch_error(version_code));
    }
    let payload = cursor
        .read_sized_payload()
        .map_err(|_| branch_error(shape_code))?;
    let preimage_end = cursor.position();
    let stored_digest = cursor
        .read_fixed_bytes::<DIGEST_LEN>()
        .map_err(|_| branch_error(shape_code))?;
    cursor
        .check_finished()
        .map_err(|_| branch_error(shape_code))?;
    let expected = digest(domain, &stored[..preimage_end]);
    if stored_digest != expected {
        return Err(branch_error(digest_code));
    }
    Ok((payload, stored[..preimage_end].to_vec(), stored_digest))
}

fn decode_required_record(
    input: &[u8],
    expected_count: u64,
    code: BranchErrorCode,
) -> Result<Vec<&[u8]>, BranchError> {
    let mut cursor = ScbValueCursor::new(input).map_err(|_| branch_error(code))?;
    let count = cursor
        .read_record_field_count()
        .map_err(|_| branch_error(code))?;
    if count != expected_count {
        return Err(branch_error(code));
    }
    let mut fields =
        Vec::with_capacity(usize::try_from(expected_count).map_err(|_| branch_error(code))?);
    for expected_tag in 1..=expected_count {
        let tag = cursor.read_uvar(32).map_err(|_| branch_error(code))?;
        if tag != expected_tag {
            return Err(branch_error(code));
        }
        fields.push(
            cursor
                .read_sized_payload()
                .map_err(|_| branch_error(code))?,
        );
    }
    cursor.check_finished().map_err(|_| branch_error(code))?;
    Ok(fields)
}

fn read_u32(input: &[u8], code: BranchErrorCode) -> Result<u32, BranchError> {
    let mut cursor = ScbValueCursor::new(input).map_err(|_| branch_error(code))?;
    let value = cursor.read_uvar(32).map_err(|_| branch_error(code))?;
    cursor.check_finished().map_err(|_| branch_error(code))?;
    u32::try_from(value).map_err(|_| branch_error(code))
}

fn read_fixed(input: &[u8], code: BranchErrorCode) -> Result<[u8; DIGEST_LEN], BranchError> {
    let mut cursor = ScbValueCursor::new(input).map_err(|_| branch_error(code))?;
    let value = cursor
        .read_fixed_bytes::<DIGEST_LEN>()
        .map_err(|_| branch_error(code))?;
    cursor.check_finished().map_err(|_| branch_error(code))?;
    Ok(value)
}

fn read_bytes(input: &[u8], code: BranchErrorCode) -> Result<&[u8], BranchError> {
    let mut cursor = ScbValueCursor::new(input).map_err(|_| branch_error(code))?;
    let value = cursor.read_bytes().map_err(|_| branch_error(code))?;
    cursor.check_finished().map_err(|_| branch_error(code))?;
    Ok(value)
}

fn encode_dependencies(values: &[StateRoot]) -> Result<Vec<u8>, BranchError> {
    if u64::try_from(values.len()).map_or(true, |count| count > MAX_COLLECTION_ELEMENTS) {
        return Err(branch_error(BranchErrorCode::BranchResourceLimit));
    }
    encode_list(
        &values
            .iter()
            .map(|value| value.as_bytes().to_vec())
            .collect::<Vec<_>>(),
    )
    .map_err(|_| branch_error(BranchErrorCode::BranchResourceLimit))
}

fn decode_dependencies(input: &[u8], code: BranchErrorCode) -> Result<Vec<StateRoot>, BranchError> {
    let mut cursor = ScbValueCursor::new(input).map_err(|_| branch_error(code))?;
    let count = cursor.read_list_count().map_err(|error| {
        if error.code() == ScbErrorCode::ResourceLimit {
            branch_error(BranchErrorCode::BranchResourceLimit)
        } else {
            branch_error(code)
        }
    })?;
    let count =
        usize::try_from(count).map_err(|_| branch_error(BranchErrorCode::BranchResourceLimit))?;
    let minimum_remaining = count
        .checked_mul(DIGEST_LEN + 1)
        .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
    if minimum_remaining > input.len().saturating_sub(cursor.position()) {
        return Err(branch_error(code));
    }
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let payload = cursor
            .read_sized_payload()
            .map_err(|_| branch_error(code))?;
        values.push(StateRoot::from_bytes(read_fixed(payload, code)?));
    }
    cursor.check_finished().map_err(|_| branch_error(code))?;
    validate_dependencies(&values, code)?;
    Ok(values)
}

fn validate_dependencies(values: &[StateRoot], code: BranchErrorCode) -> Result<(), BranchError> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(branch_error(code));
    }
    Ok(())
}

fn digest(domain: &[u8], preimage: &[u8]) -> [u8; DIGEST_LEN] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(preimage);
    *hasher.finalize().as_bytes()
}

fn hex_digest(bytes: &[u8; DIGEST_LEN]) -> String {
    use fmt::Write as _;
    let mut output = String::with_capacity(DIGEST_LEN * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn validate_branch_limit(limit: usize) -> Result<(), BranchError> {
    if limit == 0 || limit > MAX_BRANCHES {
        Err(branch_error(BranchErrorCode::BranchResourceLimit))
    } else {
        Ok(())
    }
}

fn validate_visible_capacity(current: usize) -> Result<(), BranchError> {
    if current >= MAX_BRANCHES {
        Err(branch_error(BranchErrorCode::BranchResourceLimit))
    } else {
        Ok(())
    }
}

fn validate_origin_capacity(current: usize, maximum: usize) -> Result<(), BranchError> {
    if current >= maximum {
        Err(branch_error(BranchErrorCode::BranchResourceLimit))
    } else {
        Ok(())
    }
}

fn validate_ancestry_limit(limit: usize) -> Result<(), BranchError> {
    if limit == 0 || limit > MAX_ANCESTRY_NODES {
        Err(branch_error(BranchErrorCode::BranchResourceLimit))
    } else {
        Ok(())
    }
}

fn key_path(root: &Path, name: &BranchName, suffix: &str) -> PathBuf {
    let hex = hex_digest(&name.path_key());
    root.join(&hex[0..2])
        .join(&hex[2..4])
        .join(format!("{hex}{suffix}"))
}

fn ensure_key_path(
    root: &Path,
    name: &BranchName,
    suffix: &str,
    first_component_index: usize,
    second_component_index: usize,
) -> Result<PathBuf, BranchError> {
    let hex = hex_digest(&name.path_key());
    let first = create_dir_component(root, &hex[0..2], first_component_index)?;
    let second = create_dir_component(&first, &hex[2..4], second_component_index)?;
    Ok(second.join(format!("{hex}{suffix}")))
}

fn checked_key_path(root: &Path, name: &BranchName, suffix: &str) -> Result<PathBuf, BranchError> {
    let hex = hex_digest(&name.path_key());
    let final_path = root
        .join(&hex[0..2])
        .join(&hex[2..4])
        .join(format!("{hex}{suffix}"));
    ensure_existing_directory(root)?;
    let mut current = root.to_path_buf();
    for component in [&hex[0..2], &hex[2..4]] {
        let next = current.join(component);
        match fs::symlink_metadata(&next) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(branch_error(BranchErrorCode::RefIo));
            }
            Ok(_) => current = next,
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(final_path)
}

fn enumerate_record_paths(
    root: &Path,
    suffix: &str,
    maximum: usize,
    allowed_stage_prefix: Option<&str>,
) -> Result<Vec<PathBuf>, BranchError> {
    ensure_existing_directory(root)?;
    let mut paths = Vec::new();
    for first in fs::read_dir(root)? {
        let first = first?;
        let first_type = first.file_type()?;
        let first_name = exact_utf8_name(&first)?;
        if !first_type.is_dir() || !is_fanout_component(&first_name) {
            return Err(branch_error(BranchErrorCode::RefIo));
        }
        ensure_existing_directory(&first.path())?;
        for second in fs::read_dir(first.path())? {
            let second = second?;
            let second_type = second.file_type()?;
            let second_name = exact_utf8_name(&second)?;
            if !second_type.is_dir() || !is_fanout_component(&second_name) {
                return Err(branch_error(BranchErrorCode::RefIo));
            }
            ensure_existing_directory(&second.path())?;
            for record in fs::read_dir(second.path())? {
                let record = record?;
                if !record.file_type()?.is_file() {
                    return Err(branch_error(BranchErrorCode::RefIo));
                }
                let filename = exact_utf8_name(&record)?;
                if allowed_stage_prefix.is_some_and(|prefix| is_owned_stage_name(&filename, prefix))
                {
                    continue;
                }
                let Some(hex) = filename.strip_suffix(suffix) else {
                    return Err(branch_error(BranchErrorCode::RefIo));
                };
                if hex.len() != DIGEST_LEN * 2
                    || !hex.as_bytes().iter().all(u8::is_ascii_hexdigit)
                    || hex.as_bytes().iter().any(u8::is_ascii_uppercase)
                    || hex[0..2] != first_name
                    || hex[2..4] != second_name
                {
                    return Err(branch_error(BranchErrorCode::RefIo));
                }
                paths.push(record.path());
                if paths.len() > maximum {
                    return Err(branch_error(BranchErrorCode::BranchResourceLimit));
                }
            }
        }
    }
    paths.sort();
    Ok(paths)
}

fn exact_utf8_name(entry: &fs::DirEntry) -> Result<String, BranchError> {
    entry
        .file_name()
        .into_string()
        .map_err(|_| branch_error(BranchErrorCode::RefIo))
}

fn is_fanout_component(value: &str) -> bool {
    value.len() == 2
        && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
        && !value.as_bytes().iter().any(u8::is_ascii_uppercase)
}

fn ensure_existing_directory(path: &Path) -> Result<(), BranchError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(branch_error(BranchErrorCode::RefIo));
    }
    Ok(())
}

fn create_dir_component(
    parent: &Path,
    component: &str,
    component_index: usize,
) -> Result<PathBuf, BranchError> {
    let _ = component_index;
    ensure_existing_directory(parent)?;
    let path = parent.join(component);
    match fs::create_dir(&path) {
        Ok(()) => {
            #[cfg(test)]
            fail_selected_native_ref_layout_cut(component_index)?;
            sync_dir(parent)?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            ensure_existing_directory(&path)?;
            #[cfg(test)]
            fail_selected_native_ref_layout_cut(component_index)?;
            sync_dir(parent)?;
        }
        Err(error) => return Err(error.into()),
    }
    ensure_existing_directory(&path)?;
    Ok(path)
}

fn reject_symlink_if_present(path: &Path) -> Result<(), BranchError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(branch_error(BranchErrorCode::RefIo))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn path_exists(path: &Path) -> Result<bool, BranchError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(branch_error(BranchErrorCode::RefIo))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn bounded_read(path: &Path, maximum: usize) -> Result<Vec<u8>, BranchError> {
    reject_symlink_if_present(path)?;
    let file = File::open(path)?;
    let length = usize::try_from(file.metadata()?.len())
        .map_err(|_| branch_error(BranchErrorCode::BranchResourceLimit))?;
    if length > maximum {
        return Err(branch_error(BranchErrorCode::BranchResourceLimit));
    }
    let mut bytes = Vec::with_capacity(length);
    file.take(u64::try_from(maximum.saturating_add(1)).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(branch_error(BranchErrorCode::BranchResourceLimit));
    }
    Ok(bytes)
}

fn reserve_stage(dir: &Path, prefix: &str) -> Result<(PathBuf, File), BranchError> {
    ensure_existing_directory(dir)?;
    for _ in 0..MAX_STAGE_ATTEMPTS {
        let token = STAGE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!("{prefix}{}-{token:016x}{STAGE_SUFFIX}", std::process::id());
        let path = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(branch_error(BranchErrorCode::RefIo))
}

fn persist_no_overwrite<F>(
    final_path: &Path,
    bytes: &[u8],
    stage_prefix: &str,
    collision_code: BranchErrorCode,
    verify: F,
) -> Result<(), BranchError>
where
    F: Fn(&[u8]) -> Result<(), BranchError>,
{
    let final_dir = final_path
        .parent()
        .ok_or_else(|| branch_error(BranchErrorCode::RefIo))?;
    let (stage_path, mut stage) = reserve_stage(final_dir, stage_prefix)?;
    #[cfg(test)]
    fail_selected_stage_write_cut(&mut stage, bytes, stage_prefix)?;
    stage.write_all(bytes)?;
    stage.flush()?;
    stage.sync_all()?;
    drop(stage);
    verify(&bounded_read(&stage_path, MAX_STANDALONE_BYTES)?)?;
    #[cfg(test)]
    fail_selected_stage_verified_cut(stage_prefix)?;
    match fs::hard_link(&stage_path, final_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let existing = bounded_read(final_path, MAX_STANDALONE_BYTES)?;
            if existing != bytes {
                return Err(branch_error(collision_code));
            }
            verify(&existing)?;
            File::open(final_path)?.sync_all()?;
            remove_file_if_exists(&stage_path)?;
            sync_dir(final_dir)?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    }
    #[cfg(test)]
    fail_selected_record_link_cut(stage_prefix)?;
    sync_dir(final_dir)?;
    #[cfg(test)]
    fail_selected_first_leaf_sync_cut(stage_prefix)?;
    remove_file_if_exists(&stage_path)?;
    #[cfg(test)]
    fail_selected_stage_unlink_cut(stage_prefix)?;
    sync_dir(final_dir)?;
    let final_bytes = bounded_read(final_path, MAX_STANDALONE_BYTES)?;
    if final_bytes != bytes {
        return Err(branch_error(collision_code));
    }
    verify(&final_bytes)
}

fn persist_expected_ref(path: &Path, expected: &ImportedBranchRef) -> Result<(), BranchError> {
    persist_no_overwrite(
        path,
        &expected.stored_bytes,
        REF_STAGE_PREFIX,
        BranchErrorCode::RefAlreadyExists,
        |bytes| {
            if import_branch_ref(bytes)? == *expected {
                Ok(())
            } else {
                Err(branch_error(BranchErrorCode::RefAlreadyExists))
            }
        },
    )
}

fn redurabilize_branch(path: &Path, expected: &ImportedBranchRecord) -> Result<(), BranchError> {
    redurabilize_record(path, &expected.stored_bytes, |bytes| {
        if import_branch_record(bytes)? == *expected {
            Ok(())
        } else {
            Err(branch_error(BranchErrorCode::BranchOriginMismatch))
        }
    })
}

fn redurabilize_ref(path: &Path, expected: &ImportedBranchRef) -> Result<(), BranchError> {
    redurabilize_record(path, &expected.stored_bytes, |bytes| {
        if import_branch_ref(bytes)? == *expected {
            Ok(())
        } else {
            Err(branch_error(BranchErrorCode::RefTargetMismatch))
        }
    })
}

fn redurabilize_record<F>(path: &Path, expected: &[u8], verify: F) -> Result<(), BranchError>
where
    F: Fn(&[u8]) -> Result<(), BranchError>,
{
    let first = bounded_read(path, MAX_STANDALONE_BYTES)?;
    if first != expected {
        return Err(branch_error(BranchErrorCode::RefInternalInvariant));
    }
    verify(&first)?;
    File::open(path)?.sync_all()?;
    sync_dir(
        path.parent()
            .ok_or_else(|| branch_error(BranchErrorCode::RefIo))?,
    )?;
    let second = bounded_read(path, MAX_STANDALONE_BYTES)?;
    if second != expected {
        return Err(branch_error(BranchErrorCode::RefInternalInvariant));
    }
    verify(&second)
}

fn replace_ref(path: &Path, expected: &ImportedBranchRef) -> Result<(), BranchError> {
    reject_symlink_if_present(path)?;
    let directory = path
        .parent()
        .ok_or_else(|| branch_error(BranchErrorCode::RefIo))?;
    let (stage_path, mut stage) = reserve_stage(directory, REF_STAGE_PREFIX)?;
    #[cfg(test)]
    fail_selected_advance_stage_write_cut(&mut stage, &expected.stored_bytes)?;
    stage.write_all(&expected.stored_bytes)?;
    stage.flush()?;
    stage.sync_all()?;
    drop(stage);
    if import_branch_ref(&bounded_read(&stage_path, MAX_STANDALONE_BYTES)?)? != *expected {
        return Err(branch_error(BranchErrorCode::RefInternalInvariant));
    }
    #[cfg(test)]
    fail_selected_native_ref_cut(|cut| {
        matches!(
            cut,
            NativeRefDurabilityCut::Ref12VerifiedAdvanceRefStageBeforeRename
        )
    })?;
    reject_symlink_if_present(path)?;
    fs::rename(&stage_path, path)?;
    #[cfg(test)]
    fail_selected_native_ref_cut(|cut| {
        matches!(
            cut,
            NativeRefDurabilityCut::Ref13AdvanceRefRenameBeforeLeafSync
        )
    })?;
    sync_dir(directory)?;
    #[cfg(test)]
    fail_selected_native_ref_cut(|cut| {
        matches!(
            cut,
            NativeRefDurabilityCut::Ref14AdvanceRefLeafSyncBeforeResponse
        )
    })?;
    if import_branch_ref(&bounded_read(path, MAX_STANDALONE_BYTES)?)? != *expected {
        return Err(branch_error(BranchErrorCode::RefInternalInvariant));
    }
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), BranchError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<(), BranchError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn map_recovery_ancestry_error(error: RecoveryAncestryError) -> BranchError {
    match error {
        RecoveryAncestryError::Cycle => branch_error(BranchErrorCode::BranchAncestryCycle),
        RecoveryAncestryError::LimitExceeded => branch_error(BranchErrorCode::BranchResourceLimit),
        RecoveryAncestryError::ClaimMismatch { claim_index: 0, .. } => {
            branch_error(BranchErrorCode::BranchOriginMismatch)
        }
        RecoveryAncestryError::ClaimMismatch { claim_index: 1, .. } => {
            branch_error(BranchErrorCode::RefTargetMismatch)
        }
        RecoveryAncestryError::ClaimMismatch { .. } => {
            branch_error(BranchErrorCode::RefInternalInvariant)
        }
        RecoveryAncestryError::Verification(error) => error.into(),
    }
}

/// Rejects a symlink or non-regular recovery record entry as `REF_IO`.
fn ensure_recovery_regular_file(metadata: &fs::Metadata) -> Result<(), BranchError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(branch_error(BranchErrorCode::RefIo));
    }
    Ok(())
}

fn read_recovery_ref_record(path: &Path) -> Result<ImportedBranchRef, BranchError> {
    import_branch_ref(&bounded_read(path, MAX_STANDALONE_BYTES)?)
}

fn read_recovery_origin_record(path: &Path) -> Result<ImportedBranchRecord, BranchError> {
    import_branch_record(&bounded_read(path, MAX_STANDALONE_BYTES)?)
}

fn is_recovery_fanout_name(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(is_fanout_component)
}

fn classify_record_recovery_fanout(
    entry: &fs::DirEntry,
    depth: usize,
) -> Result<(PathBuf, usize), BranchError> {
    if depth >= 2 || !entry.file_type()?.is_dir() || !is_recovery_fanout_name(&entry.file_name()) {
        return Err(branch_error(BranchErrorCode::RefIo));
    }
    Ok((entry.path(), depth + 1))
}

fn classify_origin_recovery_leaf(path: &Path) -> Result<(), BranchError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(branch_error(BranchErrorCode::RefIo));
    }
    Ok(())
}

fn classify_ref_recovery_leaf(path: &Path) -> Result<(), BranchError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(branch_error(BranchErrorCode::RefIo));
    }
    Ok(())
}

fn is_final_record_name(name: &std::ffi::OsStr, suffix: &str) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(hex) = name.strip_suffix(suffix) else {
        return false;
    };
    hex.len() == DIGEST_LEN * 2
        && hex.as_bytes().iter().all(u8::is_ascii_hexdigit)
        && !hex.as_bytes().iter().any(u8::is_ascii_uppercase)
}

fn is_final_record_name_for_dir(name: &std::ffi::OsStr, suffix: &str, directory: &Path) -> bool {
    if !is_final_record_name(name, suffix) {
        return false;
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    let hex = &name[..name.len() - suffix.len()];
    let Some(second) = directory.file_name().and_then(std::ffi::OsStr::to_str) else {
        return false;
    };
    let Some(first) = directory
        .parent()
        .and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
    else {
        return false;
    };
    first == &hex[0..2] && second == &hex[2..4]
}

fn origin_recovery_leaf_kind(path: &Path) -> Result<RefRecoveryLeafKind, BranchError> {
    record_recovery_leaf_kind(path, ".branch.scb1", BRANCH_STAGE_PREFIX)
}

fn ref_recovery_leaf_kind(path: &Path) -> Result<RefRecoveryLeafKind, BranchError> {
    record_recovery_leaf_kind(path, ".ref.scb1", REF_STAGE_PREFIX)
}

fn record_recovery_leaf_kind(
    path: &Path,
    suffix: &str,
    stage_prefix: &str,
) -> Result<RefRecoveryLeafKind, BranchError> {
    let name = path
        .file_name()
        .ok_or_else(|| branch_error(BranchErrorCode::RefIo))?;
    let directory = path
        .parent()
        .ok_or_else(|| branch_error(BranchErrorCode::RefIo))?;
    if is_final_record_name(name, suffix) {
        if !is_final_record_name_for_dir(name, suffix, directory) {
            return Err(branch_error(BranchErrorCode::RefIo));
        }
        return Ok(RefRecoveryLeafKind::Final);
    }
    let Some(name) = name.to_str() else {
        return Ok(RefRecoveryLeafKind::Unknown);
    };
    if is_owned_stage_name(name, stage_prefix) {
        return Ok(RefRecoveryLeafKind::OwnedStage);
    }
    Ok(RefRecoveryLeafKind::Unknown)
}

/// Removes planned branch-origin stages after an exact re-stat of every entry.
fn remove_planned_origin_stages(origin_removal_plan: &[PathBuf]) -> Result<u64, BranchError> {
    let mut removed = 0_u64;
    for stage_path in origin_removal_plan {
        let metadata = match fs::symlink_metadata(stage_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if !metadata.file_type().is_file() {
            return Err(branch_error(BranchErrorCode::RefIo));
        }
        fs::remove_file(stage_path)?;
        #[cfg(test)]
        fail_selected_origin_recovery_stage_cut(stage_path)?;
        removed = removed
            .checked_add(1)
            .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
    }
    Ok(removed)
}

/// Removes planned visible-ref stages after an exact re-stat of every entry.
fn remove_planned_ref_stages(ref_removal_plan: &[PathBuf]) -> Result<u64, BranchError> {
    let mut removed = 0_u64;
    for stage_path in ref_removal_plan {
        let metadata = match fs::symlink_metadata(stage_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if !metadata.file_type().is_file() {
            return Err(branch_error(BranchErrorCode::RefIo));
        }
        fs::remove_file(stage_path)?;
        #[cfg(test)]
        fail_selected_ref_recovery_stage_cut(stage_path)?;
        removed = removed
            .checked_add(1)
            .ok_or_else(|| branch_error(BranchErrorCode::BranchResourceLimit))?;
    }
    Ok(removed)
}

fn is_owned_stage_name(name: &str, prefix: &str) -> bool {
    let Some(body) = name
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(STAGE_SUFFIX))
    else {
        return false;
    };
    let Some((pid, token)) = body.split_once('-') else {
        return false;
    };
    let Ok(pid_value) = pid.parse::<u32>() else {
        return false;
    };
    pid_value > 0
        && pid_value.to_string() == pid
        && token.len() == 16
        && token.as_bytes().iter().all(u8::is_ascii_hexdigit)
        && !token.as_bytes().iter().any(u8::is_ascii_uppercase)
}

fn usize_to_u64(value: usize) -> Result<u64, BranchError> {
    u64::try_from(value).map_err(|_| branch_error(BranchErrorCode::BranchResourceLimit))
}

fn branch_error(code: BranchErrorCode) -> BranchError {
    BranchError::Branch(code)
}

#[cfg(test)]
enum NativeRefDurabilityCut {
    Rlay01BranchesV1CreateBeforeBranchesSync,
    Rlay02FirstOriginFanoutCreateBeforeParentSync,
    Rlay03SecondOriginFanoutCreateBeforeParentSync,
    Ref01DuringOriginStageWrite,
    Ref02VerifiedOriginStageBeforeFinalLink,
    Ref03OriginLinkBeforeFirstLeafSync,
    Ref04FirstOriginLeafSyncBeforeStageUnlink,
    Ref05OriginStageUnlinkBeforeSecondLeafSync,
    Ref06DuringInitialRefStageWrite,
    Ref07VerifiedInitialRefStageBeforeFinalLink,
    Ref08InitialRefLinkBeforeFirstLeafSync,
    Ref09FirstInitialRefLeafSyncBeforeStageUnlink,
    Ref10InitialRefStageUnlinkBeforeSecondLeafSync,
    Ref11DuringAdvanceRefStageWrite,
    Ref12VerifiedAdvanceRefStageBeforeRename,
    Ref13AdvanceRefRenameBeforeLeafSync,
    Ref14AdvanceRefLeafSyncBeforeResponse,
    Ref15OriginRecoveryStageUnlinkBeforeLeafSync { branch_name: BranchName },
    Ref16VisibleRefRecoveryStageUnlinkBeforeLeafSync { branch_name: BranchName },
}

#[cfg(test)]
std::thread_local! {
    static SELECTED_NATIVE_REF_CUT: std::cell::RefCell<Option<NativeRefDurabilityCut>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
struct NativeRefCutSelection;

#[cfg(test)]
impl NativeRefCutSelection {
    fn install(cut: NativeRefDurabilityCut) -> Self {
        SELECTED_NATIVE_REF_CUT.with(|selected| {
            let previous = selected.replace(Some(cut));
            assert!(
                previous.is_none(),
                "native ref durability selection is not nested"
            );
        });
        Self
    }
}

#[cfg(test)]
impl Drop for NativeRefCutSelection {
    fn drop(&mut self) {
        SELECTED_NATIVE_REF_CUT.with(|selected| {
            selected.replace(None);
        });
    }
}

#[cfg(test)]
fn take_selected_native_ref_cut(predicate: impl FnOnce(&NativeRefDurabilityCut) -> bool) -> bool {
    SELECTED_NATIVE_REF_CUT.with(|selected| {
        let take = selected.borrow().as_ref().is_some_and(predicate);
        if take {
            selected.borrow_mut().take();
        }
        take
    })
}

#[cfg(test)]
fn fail_selected_native_ref_cut(
    predicate: impl FnOnce(&NativeRefDurabilityCut) -> bool,
) -> Result<(), BranchError> {
    if take_selected_native_ref_cut(predicate) {
        return Err(branch_error(BranchErrorCode::RefIo));
    }
    Ok(())
}

#[cfg(test)]
fn fail_selected_native_ref_layout_cut(component_index: usize) -> Result<(), BranchError> {
    fail_selected_native_ref_cut(|cut| {
        matches!(
            (component_index, cut),
            (
                1,
                NativeRefDurabilityCut::Rlay01BranchesV1CreateBeforeBranchesSync
            ) | (
                2,
                NativeRefDurabilityCut::Rlay02FirstOriginFanoutCreateBeforeParentSync
            ) | (
                3,
                NativeRefDurabilityCut::Rlay03SecondOriginFanoutCreateBeforeParentSync
            )
        )
    })
}

#[cfg(test)]
fn fail_selected_stage_write_cut(
    stage: &mut File,
    bytes: &[u8],
    stage_prefix: &str,
) -> Result<(), BranchError> {
    if take_selected_native_ref_cut(|cut| match cut {
        NativeRefDurabilityCut::Ref01DuringOriginStageWrite => stage_prefix == BRANCH_STAGE_PREFIX,
        NativeRefDurabilityCut::Ref06DuringInitialRefStageWrite => stage_prefix == REF_STAGE_PREFIX,
        _ => false,
    }) {
        let split = bytes.len() / 2;
        stage.write_all(&bytes[..split])?;
        stage.flush()?;
        stage.sync_all()?;
        return Err(branch_error(BranchErrorCode::RefIo));
    }
    Ok(())
}

#[cfg(test)]
fn fail_selected_stage_verified_cut(stage_prefix: &str) -> Result<(), BranchError> {
    fail_selected_native_ref_cut(|cut| match cut {
        NativeRefDurabilityCut::Ref02VerifiedOriginStageBeforeFinalLink => {
            stage_prefix == BRANCH_STAGE_PREFIX
        }
        NativeRefDurabilityCut::Ref07VerifiedInitialRefStageBeforeFinalLink => {
            stage_prefix == REF_STAGE_PREFIX
        }
        _ => false,
    })
}

#[cfg(test)]
fn fail_selected_record_link_cut(stage_prefix: &str) -> Result<(), BranchError> {
    fail_selected_native_ref_cut(|cut| match cut {
        NativeRefDurabilityCut::Ref03OriginLinkBeforeFirstLeafSync => {
            stage_prefix == BRANCH_STAGE_PREFIX
        }
        NativeRefDurabilityCut::Ref08InitialRefLinkBeforeFirstLeafSync => {
            stage_prefix == REF_STAGE_PREFIX
        }
        _ => false,
    })
}

#[cfg(test)]
fn fail_selected_first_leaf_sync_cut(stage_prefix: &str) -> Result<(), BranchError> {
    fail_selected_native_ref_cut(|cut| match cut {
        NativeRefDurabilityCut::Ref04FirstOriginLeafSyncBeforeStageUnlink => {
            stage_prefix == BRANCH_STAGE_PREFIX
        }
        NativeRefDurabilityCut::Ref09FirstInitialRefLeafSyncBeforeStageUnlink => {
            stage_prefix == REF_STAGE_PREFIX
        }
        _ => false,
    })
}

#[cfg(test)]
fn fail_selected_stage_unlink_cut(stage_prefix: &str) -> Result<(), BranchError> {
    fail_selected_native_ref_cut(|cut| match cut {
        NativeRefDurabilityCut::Ref05OriginStageUnlinkBeforeSecondLeafSync => {
            stage_prefix == BRANCH_STAGE_PREFIX
        }
        NativeRefDurabilityCut::Ref10InitialRefStageUnlinkBeforeSecondLeafSync => {
            stage_prefix == REF_STAGE_PREFIX
        }
        _ => false,
    })
}

#[cfg(test)]
fn fail_selected_advance_stage_write_cut(
    stage: &mut File,
    bytes: &[u8],
) -> Result<(), BranchError> {
    if take_selected_native_ref_cut(|cut| {
        matches!(cut, NativeRefDurabilityCut::Ref11DuringAdvanceRefStageWrite)
    }) {
        let split = bytes.len() / 2;
        stage.write_all(&bytes[..split])?;
        stage.flush()?;
        stage.sync_all()?;
        return Err(branch_error(BranchErrorCode::RefIo));
    }
    Ok(())
}

#[cfg(test)]
fn stage_path_matches_branch_fanout(stage_path: &Path, branch_name: &BranchName) -> bool {
    let hex = hex_digest(&branch_name.path_key());
    let Some(second) = stage_path
        .parent()
        .and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
    else {
        return false;
    };
    let Some(first) = stage_path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
    else {
        return false;
    };
    first == &hex[0..2] && second == &hex[2..4]
}

#[cfg(test)]
fn fail_selected_origin_recovery_stage_cut(stage_path: &Path) -> Result<(), BranchError> {
    fail_selected_native_ref_cut(|cut| {
        let NativeRefDurabilityCut::Ref15OriginRecoveryStageUnlinkBeforeLeafSync { branch_name } =
            cut
        else {
            return false;
        };
        stage_path_matches_branch_fanout(stage_path, branch_name)
    })
}

#[cfg(test)]
fn fail_selected_ref_recovery_stage_cut(stage_path: &Path) -> Result<(), BranchError> {
    fail_selected_native_ref_cut(|cut| {
        let NativeRefDurabilityCut::Ref16VisibleRefRecoveryStageUnlinkBeforeLeafSync {
            branch_name,
        } = cut
        else {
            return false;
        };
        stage_path_matches_branch_fanout(stage_path, branch_name)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Barrier, mpsc};
    use std::time::Duration;

    use sley_id::{CandidateNonce, EntityId, ObjectId, PrincipalId, ReceiptId};
    use sley_mutate::{
        BoundPrecondition, CandidateExpiry, CandidateRecord, EntityObjectRecord,
        ExactEntityVersion, ExpectedIdentityAbsent, ImportedCandidate, MutationClass,
        MutationOperation,
        MutationPayload, PreconditionPayload, PreimageRequirement, build_candidate,
        build_entity_object, full_validation_profile_id, import_entity_object,
        value::{EntityBodyValue, EntityIdSet, NamespaceBody},
    };
    use sley_policy::{
        AcceptedPolicyRoot, CandidateValidationContext, CandidateValidationLimits,
        PolicyResourceCeilings, PolicyRootBuilder, PrincipalGrantBuilder,
        build_capability_summary_projection, conformance_registry as policy_registry,
        validate_candidate_bytes,
    };
    use sley_state_root::{
        AcceptedStateRoot, StateRootBuilder, conformance_epoch_id as state_epoch_id,
        conformance_registry as state_registry,
    };
    use sley_store::{CanonicalVerifier, ObjectStore};
    use sley_txn::{
        CommitInput, TrustedGenesisInput, build_transaction, build_transaction_receipt,
    };

    use crate::{
        GcObjectVerifier, RetentionAnchor, RetentionKind, RetentionSnapshot, RetentionTarget,
        acquire_exclusive_gc,
    };

    use super::*;

    const NOW: u64 = 1_000;
    static TEMP_DIR_COUNTER: ::std::sync::atomic::AtomicU64 =
        ::std::sync::atomic::AtomicU64::new(0);

    struct TempDir {
        path: ::std::path::PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let sequence = TEMP_DIR_COUNTER.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed);
            let path = ::std::env::temp_dir().join(::std::format!(
                "sley-refs-{label}-{}-{sequence:016x}",
                ::std::process::id()
            ));
            ::std::fs::create_dir(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = ::std::fs::remove_dir_all(&self.path);
        }
    }

    struct Fixture {
        temp: TempDir,
        transactions: ::sley_txn::TransactionRepository,
        branches: super::BranchRepository,
        principal_id: PrincipalId,
        genesis_transaction_id: TransactionId,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            Self::new_with_workspace(label, 1)
        }

        fn new_with_workspace(label: &str, workspace_byte: u8) -> Self {
            let temp = TempDir::new(label);
            let transactions = ::sley_txn::TransactionRepository::new(&temp.path);
            let branches = super::BranchRepository::new(&temp.path);
            let workspace_id = fixed(workspace_byte, WorkspaceId::from_bytes);
            let principal_id = fixed(2, PrincipalId::from_bytes);
            let base_entity = fixed(10, EntityId::from_bytes);
            let grant = PrincipalGrantBuilder::new(PolicyResourceCeilings::new(
                1_000, 1_000, 1_000, 100, 100, 100,
            ))
            .mutation_class(MutationClass::CreateEntity)
            .mutation_class(MutationClass::DeleteEntityBinding)
            .build()
            .unwrap();
            let policy = PolicyRootBuilder::new(workspace_id)
                .principal_grant(principal_id, grant)
                .build(&policy_registry().unwrap())
                .unwrap();
            let schema_epoch_id = state_epoch_id().unwrap();
            let base_object = build_entity_object(
                schema_epoch_id,
                &EntityObjectRecord {
                    entity_id: base_entity,
                    body: namespace_body(),
                    label: None,
                    semantic_fingerprint: None,
                },
            )
            .unwrap();
            let base_state = StateRootBuilder::new(
                workspace_id,
                fixed(20, ObjectId::from_bytes),
                fixed(21, ObjectId::from_bytes),
                policy.root(),
            )
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
                .unwrap();
            Self {
                temp,
                transactions,
                branches,
                principal_id,
                genesis_transaction_id: genesis.transaction_id(),
            }
        }

        fn path(&self) -> &::std::path::Path {
            &self.temp.path
        }

        fn commit_child(&self, nonce_byte: u8) -> TransactionId {
            let head = self.transactions.accepted_head().unwrap();
            let candidate = candidate_for(
                head.state_root().record.workspace_id,
                self.principal_id,
                head.transaction_id(),
                head.state_root(),
                head.policy_root(),
                nonce_byte,
            );
            self.transactions
                .commit(CommitInput::new(
                    head.transaction_id(),
                    &candidate.stored_bytes,
                    self.principal_id,
                    &[],
                    NOW,
                    CandidateValidationLimits::full_v1(),
                ))
                .unwrap()
                .transaction_id()
        }

        fn delete_entity_on_head(
            &self,
            entity_id: EntityId,
            object_id: ObjectId,
            nonce_byte: u8,
        ) -> TransactionId {
            let head = self.transactions.accepted_head().unwrap();
            let nonce = fixed(nonce_byte, CandidateNonce::from_bytes);
            let state_root = head.state_root();
            let policy_root = head.policy_root();
            let summary = build_capability_summary_projection(
                self.principal_id,
                state_root.record.workspace_id,
                policy_root.root(),
                state_root.root,
                &[],
            )
            .unwrap();
            let candidate = build_candidate(&CandidateRecord {
                format_version: 1,
                workspace_id: state_root.record.workspace_id,
                base_transaction_id: head.transaction_id(),
                base_root: state_root.root,
                schema_epoch_id: state_root.record.schema_epoch_id,
                policy_root_id: policy_root.root(),
                principal_id: self.principal_id,
                capability_summary_digest: summary.digest(),
                operations: ::std::vec![MutationOperation {
                    ordinal: 0,
                    class: MutationClass::DeleteEntityBinding,
                    target_kind: 3,
                    target_entity: entity_id,
                    field_tag: None,
                    payload: MutationPayload::DeleteEntityBinding,
                    precondition_ordinal: 0,
                }],
                preconditions: ::std::vec![BoundPrecondition {
                    operation_ordinal: 0,
                    requirement: PreimageRequirement::ExactEntityVersion,
                    payload: PreconditionPayload::ExactEntityVersion(ExactEntityVersion {
                        entity_id,
                        object_id,
                    }),
                }],
                validation_profile_id: full_validation_profile_id().unwrap(),
                candidate_nonce: nonce,
                expiry: CandidateExpiry::unix_millis(NOW + 1_000),
            })
            .unwrap();
            self.transactions
                .commit(CommitInput::new(
                    head.transaction_id(),
                    &candidate.stored_bytes,
                    self.principal_id,
                    &[],
                    NOW,
                    CandidateValidationLimits::full_v1(),
                ))
                .unwrap()
                .transaction_id()
        }
    }

    #[rustfmt::skip]
    type ExactPathSnapshot = (&'static str, u32, ::std::vec::Vec<u8>, ::core::option::Option<::std::path::PathBuf>);
    type ExactTreeSnapshot = ::std::vec::Vec<(::std::path::PathBuf, ExactPathSnapshot)>;
    #[allow(dead_code)]
    type ExactOptionalPathSnapshot = ::core::option::Option<ExactPathSnapshot>;
    #[allow(dead_code)]
    #[rustfmt::skip]
    type ExactTreeDeltaPaths = (::std::vec::Vec<::std::path::PathBuf>, ::std::vec::Vec<::std::path::PathBuf>, ::std::vec::Vec<::std::path::PathBuf>);

    fn exact_path_snapshot(path: &::std::path::Path) -> ExactPathSnapshot {
        let metadata = ::std::fs::symlink_metadata(path).expect("snapshot metadata");
        let file_type = metadata.file_type();
        let kind = if file_type.is_symlink() {
            "symlink"
        } else if file_type.is_file() {
            "regular"
        } else if file_type.is_dir() {
            "directory"
        } else {
            "non_regular"
        };
        let mode = ::std::os::unix::fs::MetadataExt::mode(&metadata);
        let bytes = if file_type.is_file() {
            ::std::fs::read(path).expect("snapshot file bytes")
        } else {
            ::std::vec::Vec::new()
        };
        let target = if file_type.is_symlink() {
            ::core::option::Option::Some(
                ::std::fs::read_link(path).expect("snapshot symlink target"),
            )
        } else {
            ::core::option::Option::None
        };
        (kind, mode, bytes, target)
    }

    fn exact_tree_snapshot(root: &::std::path::Path) -> ExactTreeSnapshot {
        fn visit(
            root: &::std::path::Path,
            path: &::std::path::Path,
            entries: &mut ExactTreeSnapshot,
        ) {
            let relative = path
                .strip_prefix(root)
                .expect("snapshot path under root")
                .to_path_buf();
            let snapshot = exact_path_snapshot(path);
            let is_directory = snapshot.0 == "directory";
            entries.push((relative, snapshot));
            if is_directory {
                let mut children = ::std::fs::read_dir(path)
                    .expect("snapshot directory")
                    .map(|entry| entry.expect("snapshot directory entry").path())
                    .collect::<::std::vec::Vec<_>>();
                children.sort();
                for child in children {
                    visit(root, &child, entries);
                }
            }
        }
        let mut entries = ::std::vec::Vec::new();
        visit(root, root, &mut entries);
        entries
    }

    #[allow(dead_code)]
    fn exact_optional_path_snapshot(path: &::std::path::Path) -> ExactOptionalPathSnapshot {
        match ::std::fs::symlink_metadata(path) {
            ::core::result::Result::Ok(_) => {
                ::core::option::Option::Some(exact_path_snapshot(path))
            }
            ::core::result::Result::Err(error)
                if error.kind() == ::std::io::ErrorKind::NotFound =>
            {
                ::core::option::Option::None
            }
            ::core::result::Result::Err(error) => {
                ::core::panic!("optional snapshot metadata: {}", error)
            }
        }
    }

    #[allow(dead_code)]
    fn exact_tree_delta_paths(
        before: &ExactTreeSnapshot,
        after: &ExactTreeSnapshot,
    ) -> ExactTreeDeltaPaths {
        let before_by_path = before
            .iter()
            .map(|(path, snapshot)| (path.clone(), snapshot))
            .collect::<::std::collections::BTreeMap<_, _>>();
        let after_by_path = after
            .iter()
            .map(|(path, snapshot)| (path.clone(), snapshot))
            .collect::<::std::collections::BTreeMap<_, _>>();
        let added = after_by_path
            .keys()
            .filter(|path| !before_by_path.contains_key(*path))
            .cloned()
            .collect::<::std::vec::Vec<_>>();
        let changed = after_by_path
            .iter()
            .filter_map(|(path, after_snapshot)| match before_by_path.get(path) {
                ::core::option::Option::Some(before_snapshot)
                    if *before_snapshot != *after_snapshot =>
                {
                    ::core::option::Option::Some(path.clone())
                }
                _ => ::core::option::Option::None,
            })
            .collect::<::std::vec::Vec<_>>();
        let removed = before_by_path
            .keys()
            .filter(|path| !after_by_path.contains_key(*path))
            .cloned()
            .collect::<::std::vec::Vec<_>>();
        (added, changed, removed)
    }

    fn exact_error_source_chain<const N: ::core::primitive::usize>(
        error: &(dyn ::std::error::Error + 'static),
    ) -> [&'static str; N] {
        let mut chain = ::std::vec::Vec::new();
        let mut source = ::std::error::Error::source(error);
        while let ::core::option::Option::Some(current) = source {
            let label = if current.is::<::sley_txn::CommitError>() {
                "CommitError"
            } else if current.is::<::sley_txn::TransactionCodecError>() {
                "TransactionCodecError"
            } else if current.is::<::sley_store::StoreError>() {
                "StoreError"
            } else if let ::core::option::Option::Some(io_error) =
                current.downcast_ref::<::std::io::Error>()
            {
                match io_error.kind() {
                    ::std::io::ErrorKind::Other => "io::Error(Other)",
                    kind => ::core::panic!("unexpected exact I/O error source kind: {kind:?}"),
                }
            } else {
                ::core::panic!(
                    "unexpected exact error source type: {}",
                    ::std::any::type_name_of_val(current)
                )
            };
            chain.push(label);
            source = ::std::error::Error::source(current);
        }
        chain.try_into().unwrap_or_else(|chain: ::std::vec::Vec<_>| {
            ::core::panic!(
                "expected {N} exact error source labels, observed {}",
                chain.len()
            )
        })
    }

    struct S20LimitFixtureObservation {
        _temp: ::core::option::Option<TempDir>,
        _fixture: ::core::option::Option<::std::boxed::Box<Fixture>>,
        cardinality: u64,
        qualified_field: &'static str,
        event_sites: ::std::vec::Vec<&'static str>,
        target_usage: u64,
    }

    struct S20LimitRuntimeObservation {
        qualified_field: &'static str,
        event_sites: ::std::vec::Vec<&'static str>,
        injected_limit: u64,
        fixture_cardinality: u64,
        scanned_peak: u64,
        retained_peak: u64,
        rejected_target_usage: ::core::option::Option<u64>,
    }

    struct S20ActiveLimitProbe {
        qualified_field: &'static str,
        event_sites: ::std::vec::Vec<&'static str>,
        injected_limit: u64,
        fixture_cardinality: u64,
        scanned_peak: u64,
        retained_peak: u64,
        rejected_target_usage: ::core::option::Option<u64>,
    }

    struct S20LimitProbe;

    ::std::thread_local! {
        static S20_LIMIT_FIXTURE_CARDINALITIES: ::std::cell::RefCell<
            ::std::vec::Vec<(::std::path::PathBuf, u64)>,
        > = const { ::std::cell::RefCell::new(::std::vec::Vec::new()) };
        static S20_ACTIVE_LIMIT_PROBE: ::std::cell::RefCell<
            ::core::option::Option<S20ActiveLimitProbe>,
        > = const { ::std::cell::RefCell::new(::core::option::Option::None) };
    }

    fn register_s20_530_limit_fixture(root: &::std::path::Path, cardinality: u64) {
        S20_LIMIT_FIXTURE_CARDINALITIES.with(|registry| {
            registry
                .borrow_mut()
                .push((root.to_path_buf(), cardinality));
        });
    }

    fn begin_s20_530_limit_probe(
        owner_root: &::std::path::Path,
        qualified_field: &'static str,
        injected_limit: u64,
        event_sites: &[&'static str],
    ) -> S20LimitProbe {
        let fixture_cardinality = S20_LIMIT_FIXTURE_CARDINALITIES.with(|registry| {
            registry
                .borrow()
                .iter()
                .find(|(root, _)| root == owner_root)
                .map(|(_, cardinality)| *cardinality)
                .expect("limit probe owner root is registered")
        });
        S20_ACTIVE_LIMIT_PROBE.with(|active| {
            let previous = active.borrow_mut().replace(S20ActiveLimitProbe {
                qualified_field,
                event_sites: event_sites.to_vec(),
                injected_limit,
                fixture_cardinality,
                scanned_peak: 0,
                retained_peak: 0,
                rejected_target_usage: ::core::option::Option::None,
            });
            assert!(previous.is_none(), "limit probes are not nested");
        });
        S20LimitProbe
    }

    pub(crate) fn record_s20_530_limit_probe(value: u64, limit: u64) {
        S20_ACTIVE_LIMIT_PROBE.with(|active| {
            let mut active = active.borrow_mut();
            let ::core::option::Option::Some(probe) = active.as_mut() else {
                return;
            };
            if limit != probe.injected_limit {
                return;
            }
            if value > limit {
                probe.rejected_target_usage = ::core::option::Option::Some(value);
            } else {
                probe.scanned_peak = probe.scanned_peak.max(value);
                probe.retained_peak = probe.retained_peak.max(value);
            }
        });
    }

    fn finish_s20_530_limit_probe(probe: S20LimitProbe) -> S20LimitRuntimeObservation {
        let S20LimitProbe = probe;
        S20_ACTIVE_LIMIT_PROBE.with(|active| {
            let state = active.borrow_mut().take().expect("limit probe is active");
            S20LimitRuntimeObservation {
                qualified_field: state.qualified_field,
                event_sites: state.event_sites,
                injected_limit: state.injected_limit,
                fixture_cardinality: state.fixture_cardinality,
                scanned_peak: state.scanned_peak,
                retained_peak: state.retained_peak,
                rejected_target_usage: state.rejected_target_usage,
            }
        })
    }

    fn prepare_s20_530_limit_03_bare_repository(
        label: &str,
        cardinality: u64,
        qualified_field: &'static str,
        event_site: &'static str,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let temp = TempDir::new(label);
        let repository = super::BranchRepository::new(&temp.path);
        register_s20_530_limit_fixture(repository.root(), cardinality);
        let maintenance = repository.acquire_exclusive_maintenance().unwrap();
        repository
            .recover_refs_with_maintenance(&maintenance)
            .expect("settle ref-recovery layout");
        let observation = S20LimitFixtureObservation {
            _temp: ::core::option::Option::Some(temp),
            _fixture: ::core::option::Option::None,
            cardinality,
            qualified_field,
            event_sites: ::std::vec![event_site],
            target_usage: cardinality,
        };
        (repository, maintenance, observation)
    }

    fn s20_530_limit_03_origin_leaf_dir(
        repository: &super::BranchRepository,
    ) -> ::std::path::PathBuf {
        let leaf = repository.branches_dir().join("00").join("00");
        ::std::fs::create_dir_all(&leaf).expect("limit fixture origin leaf dir");
        leaf
    }

    fn s20_530_limit_03_ref_leaf_dir(
        repository: &super::BranchRepository,
    ) -> ::std::path::PathBuf {
        let leaf = repository.refs_dir().join("00").join("00");
        ::std::fs::create_dir_all(&leaf).expect("limit fixture ref leaf dir");
        leaf
    }

    fn prepare_s20_530_limit_03_origin_leaf_entries_limit_fixture(
        cardinality: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let (repository, maintenance, observation) = prepare_s20_530_limit_03_bare_repository(
            "limit03-origin-leaf",
            cardinality,
            "ref_recovery_limits::origin_leaf_entries",
            "ref.origin_scan_leaf",
        );
        let leaf = s20_530_limit_03_origin_leaf_dir(&repository);
        for index in 0..cardinality {
            ::std::fs::write(
                leaf.join(::std::format!("unknown-origin-entry-{index:02}")),
                b"unknown",
            )
            .expect("limit fixture origin leaf entry");
        }
        (repository, maintenance, observation)
    }

    fn prepare_s20_530_limit_03_origin_stages_limit_fixture(
        cardinality: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let (repository, maintenance, observation) = prepare_s20_530_limit_03_bare_repository(
            "limit03-origin-stages",
            cardinality,
            "ref_recovery_limits::origin_stages",
            "ref.retain_origin_stage",
        );
        let leaf = s20_530_limit_03_origin_leaf_dir(&repository);
        for index in 0..cardinality {
            ::std::fs::write(
                leaf.join(::std::format!(".sley-branch-stage-7-{index:016x}.tmp")),
                b"stage",
            )
            .expect("limit fixture origin stage");
        }
        (repository, maintenance, observation)
    }

    fn prepare_s20_530_limit_03_ref_fanout_directories_limit_fixture(
        cardinality: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let (repository, maintenance, observation) = prepare_s20_530_limit_03_bare_repository(
            "limit03-ref-fanout",
            cardinality,
            "ref_recovery_limits::ref_fanout_directories",
            "ref.record_scan_fanout",
        );
        let first_level = repository.refs_dir().join("00");
        ::std::fs::create_dir(&first_level).expect("limit fixture first-level ref fanout");
        for index in 0..cardinality.saturating_sub(1) {
            ::std::fs::create_dir(first_level.join(::std::format!("{index:02x}")))
                .expect("limit fixture second-level ref fanout");
        }
        (repository, maintenance, observation)
    }

    fn prepare_s20_530_limit_03_ref_leaf_entries_limit_fixture(
        cardinality: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let (repository, maintenance, observation) = prepare_s20_530_limit_03_bare_repository(
            "limit03-ref-leaf",
            cardinality,
            "ref_recovery_limits::ref_leaf_entries",
            "ref.record_scan_leaf",
        );
        let leaf = s20_530_limit_03_ref_leaf_dir(&repository);
        for index in 0..cardinality {
            ::std::fs::write(
                leaf.join(::std::format!("unknown-ref-entry-{index:02}")),
                b"unknown",
            )
            .expect("limit fixture ref leaf entry");
        }
        (repository, maintenance, observation)
    }

    fn prepare_s20_530_limit_03_ref_stages_limit_fixture(
        cardinality: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let (repository, maintenance, observation) = prepare_s20_530_limit_03_bare_repository(
            "limit03-ref-stages",
            cardinality,
            "ref_recovery_limits::ref_stages",
            "ref.retain_record_stage",
        );
        let leaf = s20_530_limit_03_ref_leaf_dir(&repository);
        for index in 0..cardinality {
            ::std::fs::write(
                leaf.join(::std::format!(".sley-ref-stage-7-{index:016x}.tmp")),
                b"stage",
            )
            .expect("limit fixture ref stage");
        }
        (repository, maintenance, observation)
    }

    fn finish_s20_530_limit_03_record_fixture(
        fixture: Fixture,
        cardinality: u64,
        qualified_field: &'static str,
        event_sites: ::std::vec::Vec<&'static str>,
        target_usage: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let repository = super::BranchRepository::new(fixture.path());
        register_s20_530_limit_fixture(repository.root(), cardinality);
        let maintenance = repository.acquire_exclusive_maintenance().unwrap();
        repository
            .recover_refs_with_maintenance(&maintenance)
            .expect("validate ref-recovery record fixture");
        let observation = S20LimitFixtureObservation {
            _temp: ::core::option::Option::None,
            _fixture: ::core::option::Option::Some(::std::boxed::Box::new(fixture)),
            cardinality,
            qualified_field,
            event_sites,
            target_usage,
        };
        (repository, maintenance, observation)
    }

    fn s20_530_limit_03_visible_fixture(
        label: &str,
        cardinality: u64,
    ) -> (Fixture, ::std::vec::Vec<BranchName>) {
        let fixture = Fixture::new(label);
        let mut names = ::std::vec::Vec::new();
        for index in 0..cardinality {
            let name = BranchName::parse(::std::format!("limit03-visible-{index:02}"))
                .expect("limit fixture visible branch name");
            fixture
                .branches
                .create_branch(name.as_bytes(), fixture.genesis_transaction_id)
                .expect("limit fixture visible branch");
            names.push(name);
        }
        (fixture, names)
    }

    fn s20_530_limit_03_write_orphan_origin(
        fixture: &Fixture,
        name: &BranchName,
    ) -> ::std::path::PathBuf {
        fixture.branches.ensure_layout().unwrap();
        let revision = fixture
            .transactions
            .verified_revision(fixture.genesis_transaction_id)
            .expect("limit fixture genesis revision");
        let origin = build_branch_record(&origin_record(name, &revision))
            .expect("limit fixture origin record");
        let path = fixture
            .branches
            .ensure_branch_path(name)
            .expect("limit fixture origin path");
        ::std::fs::write(&path, origin.stored_bytes).expect("limit fixture origin bytes");
        path
    }

    fn s20_530_limit_03_orphan_fixture(
        label: &str,
        cardinality: u64,
    ) -> (Fixture, ::std::vec::Vec<::std::path::PathBuf>) {
        let fixture = Fixture::new(label);
        let mut paths = ::std::vec::Vec::new();
        for index in 0..cardinality {
            let name = BranchName::parse(::std::format!("limit03-orphan-{index:02}"))
                .expect("limit fixture orphan branch name");
            paths.push(s20_530_limit_03_write_orphan_origin(&fixture, &name));
        }
        (fixture, paths)
    }

    fn s20_530_limit_03_total_file_bytes(
        paths: &[::std::path::PathBuf],
    ) -> u64 {
        paths
            .iter()
            .map(|path| {
                ::std::fs::symlink_metadata(path)
                    .expect("limit fixture record metadata")
                    .len()
            })
            .try_fold(0_u64, |total, length| total.checked_add(length))
            .expect("limit fixture byte total")
    }

    fn prepare_s20_530_limit_03_final_origins_limit_fixture(
        cardinality: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let (fixture, _paths) = s20_530_limit_03_orphan_fixture(
            "limit03-final-origins",
            cardinality,
        );
        finish_s20_530_limit_03_record_fixture(
            fixture,
            cardinality,
            "ref_recovery_limits::final_origins",
            ::std::vec!["ref.retain_origin"],
            cardinality,
        )
    }

    fn prepare_s20_530_limit_03_origin_record_bytes_limit_fixture(
        cardinality: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let fixture = Fixture::new("limit03-origin-record-bytes");
        let visible_name = BranchName::parse("limit03-visible-origin")
            .expect("limit fixture visible origin name");
        fixture
            .branches
            .create_branch(visible_name.as_bytes(), fixture.genesis_transaction_id)
            .expect("limit fixture visible origin");
        let mut paths = ::std::vec![fixture
            .branches
            .checked_branch_path(&visible_name)
            .expect("limit fixture visible origin path")];
        for index in 1..cardinality {
            let name = BranchName::parse(::std::format!("limit03-byte-origin-{index:02}"))
                .expect("limit fixture byte origin name");
            paths.push(s20_530_limit_03_write_orphan_origin(&fixture, &name));
        }
        let target_usage = s20_530_limit_03_total_file_bytes(&paths);
        finish_s20_530_limit_03_record_fixture(
            fixture,
            cardinality,
            "ref_recovery_limits::origin_record_bytes",
            ::std::vec!["ref.visible_origin_read", "ref.orphan_origin_read"],
            target_usage,
        )
    }

    fn prepare_s20_530_limit_03_visible_ref_record_bytes_limit_fixture(
        cardinality: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let (fixture, names) =
            s20_530_limit_03_visible_fixture("limit03-visible-ref-bytes", cardinality);
        let paths = names
            .iter()
            .map(|name| {
                fixture
                    .branches
                    .checked_ref_path(name)
                    .expect("limit fixture visible ref path")
            })
            .collect::<::std::vec::Vec<_>>();
        let target_usage = s20_530_limit_03_total_file_bytes(&paths);
        finish_s20_530_limit_03_record_fixture(
            fixture,
            cardinality,
            "ref_recovery_limits::visible_ref_record_bytes",
            ::std::vec!["ref.visible_record_read"],
            target_usage,
        )
    }

    fn prepare_s20_530_limit_03_orphan_origins_limit_fixture(
        cardinality: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let (fixture, _paths) =
            s20_530_limit_03_orphan_fixture("limit03-orphan-origins", cardinality);
        finish_s20_530_limit_03_record_fixture(
            fixture,
            cardinality,
            "ref_recovery_limits::orphan_origins",
            ::std::vec!["ref.orphan_origin_read"],
            cardinality,
        )
    }

    fn prepare_s20_530_limit_03_visible_branches_limit_fixture(
        cardinality: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let (fixture, _names) =
            s20_530_limit_03_visible_fixture("limit03-visible-branches", cardinality);
        finish_s20_530_limit_03_record_fixture(
            fixture,
            cardinality,
            "ref_recovery_limits::visible_branches",
            ::std::vec!["ref.visible_record_read"],
            cardinality,
        )
    }

    fn prepare_s20_530_limit_03_origin_fanout_directories_limit_fixture(
        cardinality: u64,
    ) -> (
        super::BranchRepository,
        super::RepositoryMaintenanceGuard,
        S20LimitFixtureObservation,
    ) {
        let temp = TempDir::new("limit03-origin-fanout");
        let repository = super::BranchRepository::new(&temp.path);
        register_s20_530_limit_fixture(repository.root(), cardinality);
        let maintenance = repository.acquire_exclusive_maintenance().unwrap();
        repository
            .recover_refs_with_maintenance(&maintenance)
            .expect("settle ref-recovery layout");
        let first_level = repository.branches_dir().join("00");
        ::std::fs::create_dir(&first_level).expect("limit fixture first-level origin fanout");
        for index in 0..cardinality.saturating_sub(1) {
            ::std::fs::create_dir(first_level.join(::std::format!("{index:02x}")))
                .expect("limit fixture second-level origin fanout");
        }
        let observation = S20LimitFixtureObservation {
            _temp: ::core::option::Option::Some(temp),
            _fixture: ::core::option::Option::None,
            cardinality,
            qualified_field: "ref_recovery_limits::origin_fanout_directories",
            event_sites: ::std::vec!["ref.origin_scan_fanout"],
            target_usage: cardinality,
        };
        (repository, maintenance, observation)
    }

    #[test]
    fn limit03_origin_fanout_directories_exact_and_plus_one() {
        let (exact_branch_repository, exact_maintenance, exact_fixture_observation) = prepare_s20_530_limit_03_origin_fanout_directories_limit_fixture(2_u64);
        let (plus_one_branch_repository, plus_one_maintenance, plus_one_fixture_observation) = prepare_s20_530_limit_03_origin_fanout_directories_limit_fixture(3_u64);
        ::core::assert_eq!(exact_fixture_observation.cardinality, 2_u64);
        ::core::assert_eq!(plus_one_fixture_observation.cardinality, 3_u64);
        ::core::assert_eq!(exact_fixture_observation.qualified_field, "ref_recovery_limits::origin_fanout_directories");
        ::core::assert_eq!(plus_one_fixture_observation.qualified_field, "ref_recovery_limits::origin_fanout_directories");
        ::core::assert_eq!(exact_fixture_observation.event_sites.as_slice(), &["ref.origin_scan_fanout"]);
        ::core::assert_eq!(plus_one_fixture_observation.event_sites.as_slice(), &["ref.origin_scan_fanout"]);
        let injected_limit = exact_fixture_observation.target_usage;
        ::core::assert!(injected_limit > 0_u64);
        ::core::assert!(injected_limit < ORIGIN_RECOVERY_MAX_FANOUT_DIRECTORIES);
        ::core::assert!(plus_one_fixture_observation.target_usage > injected_limit);
        ::core::assert_eq!(super::ORIGIN_RECOVERY_MAX_FANOUT_DIRECTORIES, 65_792);
        let default_limits = ref_recovery_limits();
        let mut exact_limits = ref_recovery_limits();
        let mut plus_one_limits = ref_recovery_limits();
        exact_limits.origin_fanout_directories = injected_limit;
        plus_one_limits.origin_fanout_directories = injected_limit;
        ::core::assert_eq!(default_limits.origin_fanout_directories, ORIGIN_RECOVERY_MAX_FANOUT_DIRECTORIES);
        ::core::assert_eq!(exact_limits.origin_fanout_directories, injected_limit);
        ::core::assert_eq!(plus_one_limits.origin_fanout_directories, injected_limit);
        ::core::assert_ne!(injected_limit, default_limits.origin_leaf_entries);
        ::core::assert_eq!(exact_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_eq!(plus_one_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.final_origins);
        ::core::assert_eq!(exact_limits.final_origins, default_limits.final_origins);
        ::core::assert_eq!(plus_one_limits.final_origins, default_limits.final_origins);
        ::core::assert_ne!(injected_limit, default_limits.origin_stages);
        ::core::assert_eq!(exact_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_eq!(plus_one_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_ne!(injected_limit, default_limits.origin_record_bytes);
        ::core::assert_eq!(exact_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_eq!(plus_one_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.ref_fanout_directories);
        ::core::assert_eq!(exact_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_eq!(plus_one_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.ref_leaf_entries);
        ::core::assert_eq!(exact_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_eq!(plus_one_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.ref_stages);
        ::core::assert_eq!(exact_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_eq!(plus_one_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_ne!(injected_limit, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(exact_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(plus_one_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.orphan_origins);
        ::core::assert_eq!(exact_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_eq!(plus_one_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_ne!(injected_limit, default_limits.visible_branches);
        ::core::assert_eq!(exact_limits.visible_branches, default_limits.visible_branches);
        ::core::assert_eq!(plus_one_limits.visible_branches, default_limits.visible_branches);
        let exact_owner_root = exact_branch_repository.root();
        let plus_one_owner_root = plus_one_branch_repository.root();
        ::core::assert_ne!(exact_owner_root, plus_one_owner_root);
        let plus_one_owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        let exact_probe = begin_s20_530_limit_probe(exact_owner_root, "ref_recovery_limits::origin_fanout_directories", injected_limit, &["ref.origin_scan_fanout"]);
        let exact_result = exact_branch_repository.recover_refs_with_maintenance_and_limits(&exact_maintenance, exact_limits);
        let exact_runtime_observation = finish_s20_530_limit_probe(exact_probe);
        ::core::assert!(exact_result.is_ok());
        ::core::assert_eq!(exact_runtime_observation.qualified_field, "ref_recovery_limits::origin_fanout_directories");
        ::core::assert_eq!(exact_runtime_observation.event_sites.as_slice(), &["ref.origin_scan_fanout"]);
        ::core::assert_eq!(exact_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.fixture_cardinality, exact_fixture_observation.cardinality);
        ::core::assert_eq!(exact_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.rejected_target_usage, ::core::option::Option::None);
        let plus_one_probe = begin_s20_530_limit_probe(plus_one_owner_root, "ref_recovery_limits::origin_fanout_directories", injected_limit, &["ref.origin_scan_fanout"]);
        let plus_one_result = plus_one_branch_repository.recover_refs_with_maintenance_and_limits(&plus_one_maintenance, plus_one_limits);
        let plus_one_runtime_observation = finish_s20_530_limit_probe(plus_one_probe);
        ::core::assert!(plus_one_result.is_err());
        let limit_plus_one_error = plus_one_result.expect_err("expected limit-plus-one error");
        let plus_one_owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        ::core::assert_eq!(plus_one_owner_tree_before_snapshot, plus_one_owner_tree_after_snapshot);
        ::core::assert_eq!(limit_plus_one_error.code(), "BRANCH_RESOURCE_LIMIT");
        ::core::assert_eq!(plus_one_runtime_observation.qualified_field, "ref_recovery_limits::origin_fanout_directories");
        ::core::assert_eq!(plus_one_runtime_observation.event_sites.as_slice(), &["ref.origin_scan_fanout"]);
        ::core::assert_eq!(plus_one_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.fixture_cardinality, plus_one_fixture_observation.cardinality);
        ::core::assert_eq!(plus_one_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.rejected_target_usage, ::core::option::Option::Some(plus_one_fixture_observation.target_usage));
    }

    #[test]
    fn limit03_origin_leaf_entries_exact_and_plus_one() {
        let (exact_branch_repository, exact_maintenance, exact_fixture_observation) = prepare_s20_530_limit_03_origin_leaf_entries_limit_fixture(2_u64);
        let (plus_one_branch_repository, plus_one_maintenance, plus_one_fixture_observation) = prepare_s20_530_limit_03_origin_leaf_entries_limit_fixture(3_u64);
        ::core::assert_eq!(exact_fixture_observation.cardinality, 2_u64);
        ::core::assert_eq!(plus_one_fixture_observation.cardinality, 3_u64);
        ::core::assert_eq!(exact_fixture_observation.qualified_field, "ref_recovery_limits::origin_leaf_entries");
        ::core::assert_eq!(plus_one_fixture_observation.qualified_field, "ref_recovery_limits::origin_leaf_entries");
        ::core::assert_eq!(exact_fixture_observation.event_sites.as_slice(), &["ref.origin_scan_leaf"]);
        ::core::assert_eq!(plus_one_fixture_observation.event_sites.as_slice(), &["ref.origin_scan_leaf"]);
        let injected_limit = exact_fixture_observation.target_usage;
        ::core::assert!(injected_limit > 0_u64);
        ::core::assert!(injected_limit < ORIGIN_RECOVERY_MAX_LEAF_ENTRIES);
        ::core::assert!(plus_one_fixture_observation.target_usage > injected_limit);
        ::core::assert_eq!(super::ORIGIN_RECOVERY_MAX_LEAF_ENTRIES, 131_072);
        let default_limits = ref_recovery_limits();
        let mut exact_limits = ref_recovery_limits();
        let mut plus_one_limits = ref_recovery_limits();
        exact_limits.origin_leaf_entries = injected_limit;
        plus_one_limits.origin_leaf_entries = injected_limit;
        ::core::assert_ne!(injected_limit, default_limits.origin_fanout_directories);
        ::core::assert_eq!(exact_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_eq!(plus_one_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_eq!(default_limits.origin_leaf_entries, ORIGIN_RECOVERY_MAX_LEAF_ENTRIES);
        ::core::assert_eq!(exact_limits.origin_leaf_entries, injected_limit);
        ::core::assert_eq!(plus_one_limits.origin_leaf_entries, injected_limit);
        ::core::assert_ne!(injected_limit, default_limits.final_origins);
        ::core::assert_eq!(exact_limits.final_origins, default_limits.final_origins);
        ::core::assert_eq!(plus_one_limits.final_origins, default_limits.final_origins);
        ::core::assert_ne!(injected_limit, default_limits.origin_stages);
        ::core::assert_eq!(exact_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_eq!(plus_one_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_ne!(injected_limit, default_limits.origin_record_bytes);
        ::core::assert_eq!(exact_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_eq!(plus_one_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.ref_fanout_directories);
        ::core::assert_eq!(exact_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_eq!(plus_one_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.ref_leaf_entries);
        ::core::assert_eq!(exact_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_eq!(plus_one_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.ref_stages);
        ::core::assert_eq!(exact_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_eq!(plus_one_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_ne!(injected_limit, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(exact_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(plus_one_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.orphan_origins);
        ::core::assert_eq!(exact_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_eq!(plus_one_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_ne!(injected_limit, default_limits.visible_branches);
        ::core::assert_eq!(exact_limits.visible_branches, default_limits.visible_branches);
        ::core::assert_eq!(plus_one_limits.visible_branches, default_limits.visible_branches);
        let exact_owner_root = exact_branch_repository.root();
        let plus_one_owner_root = plus_one_branch_repository.root();
        ::core::assert_ne!(exact_owner_root, plus_one_owner_root);
        let plus_one_owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        let exact_probe = begin_s20_530_limit_probe(exact_owner_root, "ref_recovery_limits::origin_leaf_entries", injected_limit, &["ref.origin_scan_leaf"]);
        let exact_result = exact_branch_repository.recover_refs_with_maintenance_and_limits(&exact_maintenance, exact_limits);
        let exact_runtime_observation = finish_s20_530_limit_probe(exact_probe);
        ::core::assert!(exact_result.is_ok());
        ::core::assert_eq!(exact_runtime_observation.qualified_field, "ref_recovery_limits::origin_leaf_entries");
        ::core::assert_eq!(exact_runtime_observation.event_sites.as_slice(), &["ref.origin_scan_leaf"]);
        ::core::assert_eq!(exact_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.fixture_cardinality, exact_fixture_observation.cardinality);
        ::core::assert_eq!(exact_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.rejected_target_usage, ::core::option::Option::None);
        let plus_one_probe = begin_s20_530_limit_probe(plus_one_owner_root, "ref_recovery_limits::origin_leaf_entries", injected_limit, &["ref.origin_scan_leaf"]);
        let plus_one_result = plus_one_branch_repository.recover_refs_with_maintenance_and_limits(&plus_one_maintenance, plus_one_limits);
        let plus_one_runtime_observation = finish_s20_530_limit_probe(plus_one_probe);
        ::core::assert!(plus_one_result.is_err());
        let limit_plus_one_error = plus_one_result.expect_err("expected limit-plus-one error");
        let plus_one_owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        ::core::assert_eq!(plus_one_owner_tree_before_snapshot, plus_one_owner_tree_after_snapshot);
        ::core::assert_eq!(limit_plus_one_error.code(), "BRANCH_RESOURCE_LIMIT");
        ::core::assert_eq!(plus_one_runtime_observation.qualified_field, "ref_recovery_limits::origin_leaf_entries");
        ::core::assert_eq!(plus_one_runtime_observation.event_sites.as_slice(), &["ref.origin_scan_leaf"]);
        ::core::assert_eq!(plus_one_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.fixture_cardinality, plus_one_fixture_observation.cardinality);
        ::core::assert_eq!(plus_one_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.rejected_target_usage, ::core::option::Option::Some(plus_one_fixture_observation.target_usage));
    }

    #[test]
    fn limit03_origin_stages_exact_and_plus_one() {
        let (exact_branch_repository, exact_maintenance, exact_fixture_observation) = prepare_s20_530_limit_03_origin_stages_limit_fixture(2_u64);
        let (plus_one_branch_repository, plus_one_maintenance, plus_one_fixture_observation) = prepare_s20_530_limit_03_origin_stages_limit_fixture(3_u64);
        ::core::assert_eq!(exact_fixture_observation.cardinality, 2_u64);
        ::core::assert_eq!(plus_one_fixture_observation.cardinality, 3_u64);
        ::core::assert_eq!(exact_fixture_observation.qualified_field, "ref_recovery_limits::origin_stages");
        ::core::assert_eq!(plus_one_fixture_observation.qualified_field, "ref_recovery_limits::origin_stages");
        ::core::assert_eq!(exact_fixture_observation.event_sites.as_slice(), &["ref.retain_origin_stage"]);
        ::core::assert_eq!(plus_one_fixture_observation.event_sites.as_slice(), &["ref.retain_origin_stage"]);
        let injected_limit = exact_fixture_observation.target_usage;
        ::core::assert!(injected_limit > 0_u64);
        ::core::assert!(injected_limit < ORIGIN_RECOVERY_MAX_REMOVABLE_STAGES);
        ::core::assert!(plus_one_fixture_observation.target_usage > injected_limit);
        ::core::assert_eq!(super::ORIGIN_RECOVERY_MAX_REMOVABLE_STAGES, 65_536);
        let default_limits = ref_recovery_limits();
        let mut exact_limits = ref_recovery_limits();
        let mut plus_one_limits = ref_recovery_limits();
        exact_limits.origin_stages = injected_limit;
        plus_one_limits.origin_stages = injected_limit;
        ::core::assert_ne!(injected_limit, default_limits.origin_fanout_directories);
        ::core::assert_eq!(exact_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_eq!(plus_one_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.origin_leaf_entries);
        ::core::assert_eq!(exact_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_eq!(plus_one_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.final_origins);
        ::core::assert_eq!(exact_limits.final_origins, default_limits.final_origins);
        ::core::assert_eq!(plus_one_limits.final_origins, default_limits.final_origins);
        ::core::assert_eq!(default_limits.origin_stages, ORIGIN_RECOVERY_MAX_REMOVABLE_STAGES);
        ::core::assert_eq!(exact_limits.origin_stages, injected_limit);
        ::core::assert_eq!(plus_one_limits.origin_stages, injected_limit);
        ::core::assert_ne!(injected_limit, default_limits.origin_record_bytes);
        ::core::assert_eq!(exact_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_eq!(plus_one_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.ref_fanout_directories);
        ::core::assert_eq!(exact_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_eq!(plus_one_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.ref_leaf_entries);
        ::core::assert_eq!(exact_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_eq!(plus_one_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.ref_stages);
        ::core::assert_eq!(exact_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_eq!(plus_one_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_ne!(injected_limit, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(exact_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(plus_one_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.orphan_origins);
        ::core::assert_eq!(exact_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_eq!(plus_one_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_ne!(injected_limit, default_limits.visible_branches);
        ::core::assert_eq!(exact_limits.visible_branches, default_limits.visible_branches);
        ::core::assert_eq!(plus_one_limits.visible_branches, default_limits.visible_branches);
        let exact_owner_root = exact_branch_repository.root();
        let plus_one_owner_root = plus_one_branch_repository.root();
        ::core::assert_ne!(exact_owner_root, plus_one_owner_root);
        let plus_one_owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        let exact_probe = begin_s20_530_limit_probe(exact_owner_root, "ref_recovery_limits::origin_stages", injected_limit, &["ref.retain_origin_stage"]);
        let exact_result = exact_branch_repository.recover_refs_with_maintenance_and_limits(&exact_maintenance, exact_limits);
        let exact_runtime_observation = finish_s20_530_limit_probe(exact_probe);
        ::core::assert!(exact_result.is_ok());
        ::core::assert_eq!(exact_runtime_observation.qualified_field, "ref_recovery_limits::origin_stages");
        ::core::assert_eq!(exact_runtime_observation.event_sites.as_slice(), &["ref.retain_origin_stage"]);
        ::core::assert_eq!(exact_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.fixture_cardinality, exact_fixture_observation.cardinality);
        ::core::assert_eq!(exact_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.rejected_target_usage, ::core::option::Option::None);
        let plus_one_probe = begin_s20_530_limit_probe(plus_one_owner_root, "ref_recovery_limits::origin_stages", injected_limit, &["ref.retain_origin_stage"]);
        let plus_one_result = plus_one_branch_repository.recover_refs_with_maintenance_and_limits(&plus_one_maintenance, plus_one_limits);
        let plus_one_runtime_observation = finish_s20_530_limit_probe(plus_one_probe);
        ::core::assert!(plus_one_result.is_err());
        let limit_plus_one_error = plus_one_result.expect_err("expected limit-plus-one error");
        let plus_one_owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        ::core::assert_eq!(plus_one_owner_tree_before_snapshot, plus_one_owner_tree_after_snapshot);
        ::core::assert_eq!(limit_plus_one_error.code(), "BRANCH_RESOURCE_LIMIT");
        ::core::assert_eq!(plus_one_runtime_observation.qualified_field, "ref_recovery_limits::origin_stages");
        ::core::assert_eq!(plus_one_runtime_observation.event_sites.as_slice(), &["ref.retain_origin_stage"]);
        ::core::assert_eq!(plus_one_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.fixture_cardinality, plus_one_fixture_observation.cardinality);
        ::core::assert_eq!(plus_one_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.rejected_target_usage, ::core::option::Option::Some(plus_one_fixture_observation.target_usage));
    }

    #[test]
    fn limit03_ref_fanout_directories_exact_and_plus_one() {
        let (exact_branch_repository, exact_maintenance, exact_fixture_observation) = prepare_s20_530_limit_03_ref_fanout_directories_limit_fixture(2_u64);
        let (plus_one_branch_repository, plus_one_maintenance, plus_one_fixture_observation) = prepare_s20_530_limit_03_ref_fanout_directories_limit_fixture(3_u64);
        ::core::assert_eq!(exact_fixture_observation.cardinality, 2_u64);
        ::core::assert_eq!(plus_one_fixture_observation.cardinality, 3_u64);
        ::core::assert_eq!(exact_fixture_observation.qualified_field, "ref_recovery_limits::ref_fanout_directories");
        ::core::assert_eq!(plus_one_fixture_observation.qualified_field, "ref_recovery_limits::ref_fanout_directories");
        ::core::assert_eq!(exact_fixture_observation.event_sites.as_slice(), &["ref.record_scan_fanout"]);
        ::core::assert_eq!(plus_one_fixture_observation.event_sites.as_slice(), &["ref.record_scan_fanout"]);
        let injected_limit = exact_fixture_observation.target_usage;
        ::core::assert!(injected_limit > 0_u64);
        ::core::assert!(injected_limit < REF_RECOVERY_MAX_FANOUT_DIRECTORIES);
        ::core::assert!(plus_one_fixture_observation.target_usage > injected_limit);
        ::core::assert_eq!(super::REF_RECOVERY_MAX_FANOUT_DIRECTORIES, 65_792);
        let default_limits = ref_recovery_limits();
        let mut exact_limits = ref_recovery_limits();
        let mut plus_one_limits = ref_recovery_limits();
        exact_limits.ref_fanout_directories = injected_limit;
        plus_one_limits.ref_fanout_directories = injected_limit;
        ::core::assert_ne!(injected_limit, default_limits.origin_fanout_directories);
        ::core::assert_eq!(exact_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_eq!(plus_one_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.origin_leaf_entries);
        ::core::assert_eq!(exact_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_eq!(plus_one_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.final_origins);
        ::core::assert_eq!(exact_limits.final_origins, default_limits.final_origins);
        ::core::assert_eq!(plus_one_limits.final_origins, default_limits.final_origins);
        ::core::assert_ne!(injected_limit, default_limits.origin_stages);
        ::core::assert_eq!(exact_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_eq!(plus_one_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_ne!(injected_limit, default_limits.origin_record_bytes);
        ::core::assert_eq!(exact_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_eq!(plus_one_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_eq!(default_limits.ref_fanout_directories, REF_RECOVERY_MAX_FANOUT_DIRECTORIES);
        ::core::assert_eq!(exact_limits.ref_fanout_directories, injected_limit);
        ::core::assert_eq!(plus_one_limits.ref_fanout_directories, injected_limit);
        ::core::assert_ne!(injected_limit, default_limits.ref_leaf_entries);
        ::core::assert_eq!(exact_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_eq!(plus_one_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.ref_stages);
        ::core::assert_eq!(exact_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_eq!(plus_one_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_ne!(injected_limit, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(exact_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(plus_one_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.orphan_origins);
        ::core::assert_eq!(exact_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_eq!(plus_one_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_ne!(injected_limit, default_limits.visible_branches);
        ::core::assert_eq!(exact_limits.visible_branches, default_limits.visible_branches);
        ::core::assert_eq!(plus_one_limits.visible_branches, default_limits.visible_branches);
        let exact_owner_root = exact_branch_repository.root();
        let plus_one_owner_root = plus_one_branch_repository.root();
        ::core::assert_ne!(exact_owner_root, plus_one_owner_root);
        let plus_one_owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        let exact_probe = begin_s20_530_limit_probe(exact_owner_root, "ref_recovery_limits::ref_fanout_directories", injected_limit, &["ref.record_scan_fanout"]);
        let exact_result = exact_branch_repository.recover_refs_with_maintenance_and_limits(&exact_maintenance, exact_limits);
        let exact_runtime_observation = finish_s20_530_limit_probe(exact_probe);
        ::core::assert!(exact_result.is_ok());
        ::core::assert_eq!(exact_runtime_observation.qualified_field, "ref_recovery_limits::ref_fanout_directories");
        ::core::assert_eq!(exact_runtime_observation.event_sites.as_slice(), &["ref.record_scan_fanout"]);
        ::core::assert_eq!(exact_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.fixture_cardinality, exact_fixture_observation.cardinality);
        ::core::assert_eq!(exact_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.rejected_target_usage, ::core::option::Option::None);
        let plus_one_probe = begin_s20_530_limit_probe(plus_one_owner_root, "ref_recovery_limits::ref_fanout_directories", injected_limit, &["ref.record_scan_fanout"]);
        let plus_one_result = plus_one_branch_repository.recover_refs_with_maintenance_and_limits(&plus_one_maintenance, plus_one_limits);
        let plus_one_runtime_observation = finish_s20_530_limit_probe(plus_one_probe);
        ::core::assert!(plus_one_result.is_err());
        let limit_plus_one_error = plus_one_result.expect_err("expected limit-plus-one error");
        let plus_one_owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        ::core::assert_eq!(plus_one_owner_tree_before_snapshot, plus_one_owner_tree_after_snapshot);
        ::core::assert_eq!(limit_plus_one_error.code(), "BRANCH_RESOURCE_LIMIT");
        ::core::assert_eq!(plus_one_runtime_observation.qualified_field, "ref_recovery_limits::ref_fanout_directories");
        ::core::assert_eq!(plus_one_runtime_observation.event_sites.as_slice(), &["ref.record_scan_fanout"]);
        ::core::assert_eq!(plus_one_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.fixture_cardinality, plus_one_fixture_observation.cardinality);
        ::core::assert_eq!(plus_one_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.rejected_target_usage, ::core::option::Option::Some(plus_one_fixture_observation.target_usage));
    }

    #[test]
    fn limit03_ref_leaf_entries_exact_and_plus_one() {
        let (exact_branch_repository, exact_maintenance, exact_fixture_observation) = prepare_s20_530_limit_03_ref_leaf_entries_limit_fixture(2_u64);
        let (plus_one_branch_repository, plus_one_maintenance, plus_one_fixture_observation) = prepare_s20_530_limit_03_ref_leaf_entries_limit_fixture(3_u64);
        ::core::assert_eq!(exact_fixture_observation.cardinality, 2_u64);
        ::core::assert_eq!(plus_one_fixture_observation.cardinality, 3_u64);
        ::core::assert_eq!(exact_fixture_observation.qualified_field, "ref_recovery_limits::ref_leaf_entries");
        ::core::assert_eq!(plus_one_fixture_observation.qualified_field, "ref_recovery_limits::ref_leaf_entries");
        ::core::assert_eq!(exact_fixture_observation.event_sites.as_slice(), &["ref.record_scan_leaf"]);
        ::core::assert_eq!(plus_one_fixture_observation.event_sites.as_slice(), &["ref.record_scan_leaf"]);
        let injected_limit = exact_fixture_observation.target_usage;
        ::core::assert!(injected_limit > 0_u64);
        ::core::assert!(injected_limit < REF_RECOVERY_MAX_LEAF_ENTRIES);
        ::core::assert!(plus_one_fixture_observation.target_usage > injected_limit);
        ::core::assert_eq!(super::REF_RECOVERY_MAX_LEAF_ENTRIES, 69_632);
        let default_limits = ref_recovery_limits();
        let mut exact_limits = ref_recovery_limits();
        let mut plus_one_limits = ref_recovery_limits();
        exact_limits.ref_leaf_entries = injected_limit;
        plus_one_limits.ref_leaf_entries = injected_limit;
        ::core::assert_ne!(injected_limit, default_limits.origin_fanout_directories);
        ::core::assert_eq!(exact_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_eq!(plus_one_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.origin_leaf_entries);
        ::core::assert_eq!(exact_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_eq!(plus_one_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.final_origins);
        ::core::assert_eq!(exact_limits.final_origins, default_limits.final_origins);
        ::core::assert_eq!(plus_one_limits.final_origins, default_limits.final_origins);
        ::core::assert_ne!(injected_limit, default_limits.origin_stages);
        ::core::assert_eq!(exact_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_eq!(plus_one_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_ne!(injected_limit, default_limits.origin_record_bytes);
        ::core::assert_eq!(exact_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_eq!(plus_one_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.ref_fanout_directories);
        ::core::assert_eq!(exact_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_eq!(plus_one_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_eq!(default_limits.ref_leaf_entries, REF_RECOVERY_MAX_LEAF_ENTRIES);
        ::core::assert_eq!(exact_limits.ref_leaf_entries, injected_limit);
        ::core::assert_eq!(plus_one_limits.ref_leaf_entries, injected_limit);
        ::core::assert_ne!(injected_limit, default_limits.ref_stages);
        ::core::assert_eq!(exact_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_eq!(plus_one_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_ne!(injected_limit, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(exact_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(plus_one_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.orphan_origins);
        ::core::assert_eq!(exact_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_eq!(plus_one_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_ne!(injected_limit, default_limits.visible_branches);
        ::core::assert_eq!(exact_limits.visible_branches, default_limits.visible_branches);
        ::core::assert_eq!(plus_one_limits.visible_branches, default_limits.visible_branches);
        let exact_owner_root = exact_branch_repository.root();
        let plus_one_owner_root = plus_one_branch_repository.root();
        ::core::assert_ne!(exact_owner_root, plus_one_owner_root);
        let plus_one_owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        let exact_probe = begin_s20_530_limit_probe(exact_owner_root, "ref_recovery_limits::ref_leaf_entries", injected_limit, &["ref.record_scan_leaf"]);
        let exact_result = exact_branch_repository.recover_refs_with_maintenance_and_limits(&exact_maintenance, exact_limits);
        let exact_runtime_observation = finish_s20_530_limit_probe(exact_probe);
        ::core::assert!(exact_result.is_ok());
        ::core::assert_eq!(exact_runtime_observation.qualified_field, "ref_recovery_limits::ref_leaf_entries");
        ::core::assert_eq!(exact_runtime_observation.event_sites.as_slice(), &["ref.record_scan_leaf"]);
        ::core::assert_eq!(exact_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.fixture_cardinality, exact_fixture_observation.cardinality);
        ::core::assert_eq!(exact_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.rejected_target_usage, ::core::option::Option::None);
        let plus_one_probe = begin_s20_530_limit_probe(plus_one_owner_root, "ref_recovery_limits::ref_leaf_entries", injected_limit, &["ref.record_scan_leaf"]);
        let plus_one_result = plus_one_branch_repository.recover_refs_with_maintenance_and_limits(&plus_one_maintenance, plus_one_limits);
        let plus_one_runtime_observation = finish_s20_530_limit_probe(plus_one_probe);
        ::core::assert!(plus_one_result.is_err());
        let limit_plus_one_error = plus_one_result.expect_err("expected limit-plus-one error");
        let plus_one_owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        ::core::assert_eq!(plus_one_owner_tree_before_snapshot, plus_one_owner_tree_after_snapshot);
        ::core::assert_eq!(limit_plus_one_error.code(), "BRANCH_RESOURCE_LIMIT");
        ::core::assert_eq!(plus_one_runtime_observation.qualified_field, "ref_recovery_limits::ref_leaf_entries");
        ::core::assert_eq!(plus_one_runtime_observation.event_sites.as_slice(), &["ref.record_scan_leaf"]);
        ::core::assert_eq!(plus_one_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.fixture_cardinality, plus_one_fixture_observation.cardinality);
        ::core::assert_eq!(plus_one_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.rejected_target_usage, ::core::option::Option::Some(plus_one_fixture_observation.target_usage));
    }

    #[test]
    fn limit03_ref_stages_exact_and_plus_one() {
        let (exact_branch_repository, exact_maintenance, exact_fixture_observation) = prepare_s20_530_limit_03_ref_stages_limit_fixture(2_u64);
        let (plus_one_branch_repository, plus_one_maintenance, plus_one_fixture_observation) = prepare_s20_530_limit_03_ref_stages_limit_fixture(3_u64);
        ::core::assert_eq!(exact_fixture_observation.cardinality, 2_u64);
        ::core::assert_eq!(plus_one_fixture_observation.cardinality, 3_u64);
        ::core::assert_eq!(exact_fixture_observation.qualified_field, "ref_recovery_limits::ref_stages");
        ::core::assert_eq!(plus_one_fixture_observation.qualified_field, "ref_recovery_limits::ref_stages");
        ::core::assert_eq!(exact_fixture_observation.event_sites.as_slice(), &["ref.retain_record_stage"]);
        ::core::assert_eq!(plus_one_fixture_observation.event_sites.as_slice(), &["ref.retain_record_stage"]);
        let injected_limit = exact_fixture_observation.target_usage;
        ::core::assert!(injected_limit > 0_u64);
        ::core::assert!(injected_limit < REF_RECOVERY_MAX_REMOVABLE_STAGES);
        ::core::assert!(plus_one_fixture_observation.target_usage > injected_limit);
        ::core::assert_eq!(super::REF_RECOVERY_MAX_REMOVABLE_STAGES, 65_536);
        let default_limits = ref_recovery_limits();
        let mut exact_limits = ref_recovery_limits();
        let mut plus_one_limits = ref_recovery_limits();
        exact_limits.ref_stages = injected_limit;
        plus_one_limits.ref_stages = injected_limit;
        ::core::assert_ne!(injected_limit, default_limits.origin_fanout_directories);
        ::core::assert_eq!(exact_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_eq!(plus_one_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.origin_leaf_entries);
        ::core::assert_eq!(exact_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_eq!(plus_one_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.final_origins);
        ::core::assert_eq!(exact_limits.final_origins, default_limits.final_origins);
        ::core::assert_eq!(plus_one_limits.final_origins, default_limits.final_origins);
        ::core::assert_ne!(injected_limit, default_limits.origin_stages);
        ::core::assert_eq!(exact_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_eq!(plus_one_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_ne!(injected_limit, default_limits.origin_record_bytes);
        ::core::assert_eq!(exact_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_eq!(plus_one_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.ref_fanout_directories);
        ::core::assert_eq!(exact_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_eq!(plus_one_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.ref_leaf_entries);
        ::core::assert_eq!(exact_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_eq!(plus_one_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_eq!(default_limits.ref_stages, REF_RECOVERY_MAX_REMOVABLE_STAGES);
        ::core::assert_eq!(exact_limits.ref_stages, injected_limit);
        ::core::assert_eq!(plus_one_limits.ref_stages, injected_limit);
        ::core::assert_ne!(injected_limit, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(exact_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(plus_one_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.orphan_origins);
        ::core::assert_eq!(exact_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_eq!(plus_one_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_ne!(injected_limit, default_limits.visible_branches);
        ::core::assert_eq!(exact_limits.visible_branches, default_limits.visible_branches);
        ::core::assert_eq!(plus_one_limits.visible_branches, default_limits.visible_branches);
        let exact_owner_root = exact_branch_repository.root();
        let plus_one_owner_root = plus_one_branch_repository.root();
        ::core::assert_ne!(exact_owner_root, plus_one_owner_root);
        let plus_one_owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        let exact_probe = begin_s20_530_limit_probe(exact_owner_root, "ref_recovery_limits::ref_stages", injected_limit, &["ref.retain_record_stage"]);
        let exact_result = exact_branch_repository.recover_refs_with_maintenance_and_limits(&exact_maintenance, exact_limits);
        let exact_runtime_observation = finish_s20_530_limit_probe(exact_probe);
        ::core::assert!(exact_result.is_ok());
        ::core::assert_eq!(exact_runtime_observation.qualified_field, "ref_recovery_limits::ref_stages");
        ::core::assert_eq!(exact_runtime_observation.event_sites.as_slice(), &["ref.retain_record_stage"]);
        ::core::assert_eq!(exact_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.fixture_cardinality, exact_fixture_observation.cardinality);
        ::core::assert_eq!(exact_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.rejected_target_usage, ::core::option::Option::None);
        let plus_one_probe = begin_s20_530_limit_probe(plus_one_owner_root, "ref_recovery_limits::ref_stages", injected_limit, &["ref.retain_record_stage"]);
        let plus_one_result = plus_one_branch_repository.recover_refs_with_maintenance_and_limits(&plus_one_maintenance, plus_one_limits);
        let plus_one_runtime_observation = finish_s20_530_limit_probe(plus_one_probe);
        ::core::assert!(plus_one_result.is_err());
        let limit_plus_one_error = plus_one_result.expect_err("expected limit-plus-one error");
        let plus_one_owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        ::core::assert_eq!(plus_one_owner_tree_before_snapshot, plus_one_owner_tree_after_snapshot);
        ::core::assert_eq!(limit_plus_one_error.code(), "BRANCH_RESOURCE_LIMIT");
        ::core::assert_eq!(plus_one_runtime_observation.qualified_field, "ref_recovery_limits::ref_stages");
        ::core::assert_eq!(plus_one_runtime_observation.event_sites.as_slice(), &["ref.retain_record_stage"]);
        ::core::assert_eq!(plus_one_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.fixture_cardinality, plus_one_fixture_observation.cardinality);
        ::core::assert_eq!(plus_one_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.rejected_target_usage, ::core::option::Option::Some(plus_one_fixture_observation.target_usage));
    }

    #[test]
    fn limit03_final_origins_exact_and_plus_one() {
        let (exact_branch_repository, exact_maintenance, exact_fixture_observation) = prepare_s20_530_limit_03_final_origins_limit_fixture(2_u64);
        let (plus_one_branch_repository, plus_one_maintenance, plus_one_fixture_observation) = prepare_s20_530_limit_03_final_origins_limit_fixture(3_u64);
        ::core::assert_eq!(exact_fixture_observation.cardinality, 2_u64);
        ::core::assert_eq!(plus_one_fixture_observation.cardinality, 3_u64);
        ::core::assert_eq!(exact_fixture_observation.qualified_field, "ref_recovery_limits::final_origins");
        ::core::assert_eq!(plus_one_fixture_observation.qualified_field, "ref_recovery_limits::final_origins");
        ::core::assert_eq!(exact_fixture_observation.event_sites.as_slice(), &["ref.retain_origin"]);
        ::core::assert_eq!(plus_one_fixture_observation.event_sites.as_slice(), &["ref.retain_origin"]);
        let injected_limit = exact_fixture_observation.target_usage;
        ::core::assert!(injected_limit > 0_u64);
        ::core::assert!(injected_limit < ORIGIN_RECOVERY_MAX_FINAL_RECORDS);
        ::core::assert!(plus_one_fixture_observation.target_usage > injected_limit);
        ::core::assert_eq!(super::ORIGIN_RECOVERY_MAX_FINAL_RECORDS, 65_536);
        let default_limits = ref_recovery_limits();
        let mut exact_limits = ref_recovery_limits();
        let mut plus_one_limits = ref_recovery_limits();
        exact_limits.final_origins = injected_limit;
        plus_one_limits.final_origins = injected_limit;
        ::core::assert_ne!(injected_limit, default_limits.origin_fanout_directories);
        ::core::assert_eq!(exact_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_eq!(plus_one_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.origin_leaf_entries);
        ::core::assert_eq!(exact_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_eq!(plus_one_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_eq!(default_limits.final_origins, ORIGIN_RECOVERY_MAX_FINAL_RECORDS);
        ::core::assert_eq!(exact_limits.final_origins, injected_limit);
        ::core::assert_eq!(plus_one_limits.final_origins, injected_limit);
        ::core::assert_ne!(injected_limit, default_limits.origin_stages);
        ::core::assert_eq!(exact_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_eq!(plus_one_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_ne!(injected_limit, default_limits.origin_record_bytes);
        ::core::assert_eq!(exact_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_eq!(plus_one_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.ref_fanout_directories);
        ::core::assert_eq!(exact_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_eq!(plus_one_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.ref_leaf_entries);
        ::core::assert_eq!(exact_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_eq!(plus_one_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.ref_stages);
        ::core::assert_eq!(exact_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_eq!(plus_one_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_ne!(injected_limit, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(exact_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(plus_one_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.orphan_origins);
        ::core::assert_eq!(exact_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_eq!(plus_one_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_ne!(injected_limit, default_limits.visible_branches);
        ::core::assert_eq!(exact_limits.visible_branches, default_limits.visible_branches);
        ::core::assert_eq!(plus_one_limits.visible_branches, default_limits.visible_branches);
        let exact_owner_root = exact_branch_repository.root();
        let plus_one_owner_root = plus_one_branch_repository.root();
        ::core::assert_ne!(exact_owner_root, plus_one_owner_root);
        let plus_one_owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        let exact_probe = begin_s20_530_limit_probe(exact_owner_root, "ref_recovery_limits::final_origins", injected_limit, &["ref.retain_origin"]);
        let exact_result = exact_branch_repository.recover_refs_with_maintenance_and_limits(&exact_maintenance, exact_limits);
        let exact_runtime_observation = finish_s20_530_limit_probe(exact_probe);
        ::core::assert!(exact_result.is_ok());
        ::core::assert_eq!(exact_runtime_observation.qualified_field, "ref_recovery_limits::final_origins");
        ::core::assert_eq!(exact_runtime_observation.event_sites.as_slice(), &["ref.retain_origin"]);
        ::core::assert_eq!(exact_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.fixture_cardinality, exact_fixture_observation.cardinality);
        ::core::assert_eq!(exact_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.rejected_target_usage, ::core::option::Option::None);
        let plus_one_probe = begin_s20_530_limit_probe(plus_one_owner_root, "ref_recovery_limits::final_origins", injected_limit, &["ref.retain_origin"]);
        let plus_one_result = plus_one_branch_repository.recover_refs_with_maintenance_and_limits(&plus_one_maintenance, plus_one_limits);
        let plus_one_runtime_observation = finish_s20_530_limit_probe(plus_one_probe);
        ::core::assert!(plus_one_result.is_err());
        let limit_plus_one_error = plus_one_result.expect_err("expected limit-plus-one error");
        let plus_one_owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        ::core::assert_eq!(plus_one_owner_tree_before_snapshot, plus_one_owner_tree_after_snapshot);
        ::core::assert_eq!(limit_plus_one_error.code(), "BRANCH_RESOURCE_LIMIT");
        ::core::assert_eq!(plus_one_runtime_observation.qualified_field, "ref_recovery_limits::final_origins");
        ::core::assert_eq!(plus_one_runtime_observation.event_sites.as_slice(), &["ref.retain_origin"]);
        ::core::assert_eq!(plus_one_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.fixture_cardinality, plus_one_fixture_observation.cardinality);
        ::core::assert_eq!(plus_one_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.rejected_target_usage, ::core::option::Option::Some(plus_one_fixture_observation.target_usage));
    }

    #[test]
    fn limit03_origin_record_bytes_exact_and_plus_one() {
        let (exact_branch_repository, exact_maintenance, exact_fixture_observation) = prepare_s20_530_limit_03_origin_record_bytes_limit_fixture(2_u64);
        let (plus_one_branch_repository, plus_one_maintenance, plus_one_fixture_observation) = prepare_s20_530_limit_03_origin_record_bytes_limit_fixture(3_u64);
        ::core::assert_eq!(exact_fixture_observation.cardinality, 2_u64);
        ::core::assert_eq!(plus_one_fixture_observation.cardinality, 3_u64);
        ::core::assert_eq!(exact_fixture_observation.qualified_field, "ref_recovery_limits::origin_record_bytes");
        ::core::assert_eq!(plus_one_fixture_observation.qualified_field, "ref_recovery_limits::origin_record_bytes");
        ::core::assert_eq!(exact_fixture_observation.event_sites.as_slice(), &["ref.visible_origin_read","ref.orphan_origin_read"]);
        ::core::assert_eq!(plus_one_fixture_observation.event_sites.as_slice(), &["ref.visible_origin_read","ref.orphan_origin_read"]);
        let injected_limit = exact_fixture_observation.target_usage;
        ::core::assert!(injected_limit > 0_u64);
        ::core::assert!(injected_limit < ORIGIN_RECOVERY_MAX_RECORD_BYTES);
        ::core::assert!(plus_one_fixture_observation.target_usage > injected_limit);
        ::core::assert_eq!(super::ORIGIN_RECOVERY_MAX_RECORD_BYTES, 1_073_741_824);
        let default_limits = ref_recovery_limits();
        let mut exact_limits = ref_recovery_limits();
        let mut plus_one_limits = ref_recovery_limits();
        exact_limits.origin_record_bytes = injected_limit;
        plus_one_limits.origin_record_bytes = injected_limit;
        ::core::assert_ne!(injected_limit, default_limits.origin_fanout_directories);
        ::core::assert_eq!(exact_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_eq!(plus_one_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.origin_leaf_entries);
        ::core::assert_eq!(exact_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_eq!(plus_one_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.final_origins);
        ::core::assert_eq!(exact_limits.final_origins, default_limits.final_origins);
        ::core::assert_eq!(plus_one_limits.final_origins, default_limits.final_origins);
        ::core::assert_ne!(injected_limit, default_limits.origin_stages);
        ::core::assert_eq!(exact_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_eq!(plus_one_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_eq!(default_limits.origin_record_bytes, ORIGIN_RECOVERY_MAX_RECORD_BYTES);
        ::core::assert_eq!(exact_limits.origin_record_bytes, injected_limit);
        ::core::assert_eq!(plus_one_limits.origin_record_bytes, injected_limit);
        ::core::assert_ne!(injected_limit, default_limits.ref_fanout_directories);
        ::core::assert_eq!(exact_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_eq!(plus_one_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.ref_leaf_entries);
        ::core::assert_eq!(exact_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_eq!(plus_one_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.ref_stages);
        ::core::assert_eq!(exact_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_eq!(plus_one_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_ne!(injected_limit, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(exact_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(plus_one_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.orphan_origins);
        ::core::assert_eq!(exact_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_eq!(plus_one_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_ne!(injected_limit, default_limits.visible_branches);
        ::core::assert_eq!(exact_limits.visible_branches, default_limits.visible_branches);
        ::core::assert_eq!(plus_one_limits.visible_branches, default_limits.visible_branches);
        let exact_owner_root = exact_branch_repository.root();
        let plus_one_owner_root = plus_one_branch_repository.root();
        ::core::assert_ne!(exact_owner_root, plus_one_owner_root);
        let plus_one_owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        let exact_probe = begin_s20_530_limit_probe(exact_owner_root, "ref_recovery_limits::origin_record_bytes", injected_limit, &["ref.visible_origin_read","ref.orphan_origin_read"]);
        let exact_result = exact_branch_repository.recover_refs_with_maintenance_and_limits(&exact_maintenance, exact_limits);
        let exact_runtime_observation = finish_s20_530_limit_probe(exact_probe);
        ::core::assert!(exact_result.is_ok());
        ::core::assert_eq!(exact_runtime_observation.qualified_field, "ref_recovery_limits::origin_record_bytes");
        ::core::assert_eq!(exact_runtime_observation.event_sites.as_slice(), &["ref.visible_origin_read","ref.orphan_origin_read"]);
        ::core::assert_eq!(exact_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.fixture_cardinality, exact_fixture_observation.cardinality);
        ::core::assert_eq!(exact_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.rejected_target_usage, ::core::option::Option::None);
        let plus_one_probe = begin_s20_530_limit_probe(plus_one_owner_root, "ref_recovery_limits::origin_record_bytes", injected_limit, &["ref.visible_origin_read","ref.orphan_origin_read"]);
        let plus_one_result = plus_one_branch_repository.recover_refs_with_maintenance_and_limits(&plus_one_maintenance, plus_one_limits);
        let plus_one_runtime_observation = finish_s20_530_limit_probe(plus_one_probe);
        ::core::assert!(plus_one_result.is_err());
        let limit_plus_one_error = plus_one_result.expect_err("expected limit-plus-one error");
        let plus_one_owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        ::core::assert_eq!(plus_one_owner_tree_before_snapshot, plus_one_owner_tree_after_snapshot);
        ::core::assert_eq!(limit_plus_one_error.code(), "BRANCH_RESOURCE_LIMIT");
        ::core::assert_eq!(plus_one_runtime_observation.qualified_field, "ref_recovery_limits::origin_record_bytes");
        ::core::assert_eq!(plus_one_runtime_observation.event_sites.as_slice(), &["ref.visible_origin_read","ref.orphan_origin_read"]);
        ::core::assert_eq!(plus_one_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.fixture_cardinality, plus_one_fixture_observation.cardinality);
        ::core::assert_eq!(plus_one_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.rejected_target_usage, ::core::option::Option::Some(plus_one_fixture_observation.target_usage));
    }

    #[test]
    fn limit03_visible_ref_record_bytes_exact_and_plus_one() {
        let (exact_branch_repository, exact_maintenance, exact_fixture_observation) = prepare_s20_530_limit_03_visible_ref_record_bytes_limit_fixture(2_u64);
        let (plus_one_branch_repository, plus_one_maintenance, plus_one_fixture_observation) = prepare_s20_530_limit_03_visible_ref_record_bytes_limit_fixture(3_u64);
        ::core::assert_eq!(exact_fixture_observation.cardinality, 2_u64);
        ::core::assert_eq!(plus_one_fixture_observation.cardinality, 3_u64);
        ::core::assert_eq!(exact_fixture_observation.qualified_field, "ref_recovery_limits::visible_ref_record_bytes");
        ::core::assert_eq!(plus_one_fixture_observation.qualified_field, "ref_recovery_limits::visible_ref_record_bytes");
        ::core::assert_eq!(exact_fixture_observation.event_sites.as_slice(), &["ref.visible_record_read"]);
        ::core::assert_eq!(plus_one_fixture_observation.event_sites.as_slice(), &["ref.visible_record_read"]);
        let injected_limit = exact_fixture_observation.target_usage;
        ::core::assert!(injected_limit > 0_u64);
        ::core::assert!(injected_limit < REF_RECOVERY_MAX_RECORD_BYTES);
        ::core::assert!(plus_one_fixture_observation.target_usage > injected_limit);
        ::core::assert_eq!(super::REF_RECOVERY_MAX_RECORD_BYTES, 268_435_456);
        let default_limits = ref_recovery_limits();
        let mut exact_limits = ref_recovery_limits();
        let mut plus_one_limits = ref_recovery_limits();
        exact_limits.visible_ref_record_bytes = injected_limit;
        plus_one_limits.visible_ref_record_bytes = injected_limit;
        ::core::assert_ne!(injected_limit, default_limits.origin_fanout_directories);
        ::core::assert_eq!(exact_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_eq!(plus_one_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.origin_leaf_entries);
        ::core::assert_eq!(exact_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_eq!(plus_one_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.final_origins);
        ::core::assert_eq!(exact_limits.final_origins, default_limits.final_origins);
        ::core::assert_eq!(plus_one_limits.final_origins, default_limits.final_origins);
        ::core::assert_ne!(injected_limit, default_limits.origin_stages);
        ::core::assert_eq!(exact_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_eq!(plus_one_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_ne!(injected_limit, default_limits.origin_record_bytes);
        ::core::assert_eq!(exact_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_eq!(plus_one_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.ref_fanout_directories);
        ::core::assert_eq!(exact_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_eq!(plus_one_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.ref_leaf_entries);
        ::core::assert_eq!(exact_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_eq!(plus_one_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.ref_stages);
        ::core::assert_eq!(exact_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_eq!(plus_one_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_eq!(default_limits.visible_ref_record_bytes, REF_RECOVERY_MAX_RECORD_BYTES);
        ::core::assert_eq!(exact_limits.visible_ref_record_bytes, injected_limit);
        ::core::assert_eq!(plus_one_limits.visible_ref_record_bytes, injected_limit);
        ::core::assert_ne!(injected_limit, default_limits.orphan_origins);
        ::core::assert_eq!(exact_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_eq!(plus_one_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_ne!(injected_limit, default_limits.visible_branches);
        ::core::assert_eq!(exact_limits.visible_branches, default_limits.visible_branches);
        ::core::assert_eq!(plus_one_limits.visible_branches, default_limits.visible_branches);
        let exact_owner_root = exact_branch_repository.root();
        let plus_one_owner_root = plus_one_branch_repository.root();
        ::core::assert_ne!(exact_owner_root, plus_one_owner_root);
        let plus_one_owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        let exact_probe = begin_s20_530_limit_probe(exact_owner_root, "ref_recovery_limits::visible_ref_record_bytes", injected_limit, &["ref.visible_record_read"]);
        let exact_result = exact_branch_repository.recover_refs_with_maintenance_and_limits(&exact_maintenance, exact_limits);
        let exact_runtime_observation = finish_s20_530_limit_probe(exact_probe);
        ::core::assert!(exact_result.is_ok());
        ::core::assert_eq!(exact_runtime_observation.qualified_field, "ref_recovery_limits::visible_ref_record_bytes");
        ::core::assert_eq!(exact_runtime_observation.event_sites.as_slice(), &["ref.visible_record_read"]);
        ::core::assert_eq!(exact_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.fixture_cardinality, exact_fixture_observation.cardinality);
        ::core::assert_eq!(exact_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.rejected_target_usage, ::core::option::Option::None);
        let plus_one_probe = begin_s20_530_limit_probe(plus_one_owner_root, "ref_recovery_limits::visible_ref_record_bytes", injected_limit, &["ref.visible_record_read"]);
        let plus_one_result = plus_one_branch_repository.recover_refs_with_maintenance_and_limits(&plus_one_maintenance, plus_one_limits);
        let plus_one_runtime_observation = finish_s20_530_limit_probe(plus_one_probe);
        ::core::assert!(plus_one_result.is_err());
        let limit_plus_one_error = plus_one_result.expect_err("expected limit-plus-one error");
        let plus_one_owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        ::core::assert_eq!(plus_one_owner_tree_before_snapshot, plus_one_owner_tree_after_snapshot);
        ::core::assert_eq!(limit_plus_one_error.code(), "BRANCH_RESOURCE_LIMIT");
        ::core::assert_eq!(plus_one_runtime_observation.qualified_field, "ref_recovery_limits::visible_ref_record_bytes");
        ::core::assert_eq!(plus_one_runtime_observation.event_sites.as_slice(), &["ref.visible_record_read"]);
        ::core::assert_eq!(plus_one_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.fixture_cardinality, plus_one_fixture_observation.cardinality);
        ::core::assert_eq!(plus_one_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.rejected_target_usage, ::core::option::Option::Some(plus_one_fixture_observation.target_usage));
    }

    #[test]
    fn limit03_orphan_origins_exact_and_plus_one() {
        let (exact_branch_repository, exact_maintenance, exact_fixture_observation) = prepare_s20_530_limit_03_orphan_origins_limit_fixture(2_u64);
        let (plus_one_branch_repository, plus_one_maintenance, plus_one_fixture_observation) = prepare_s20_530_limit_03_orphan_origins_limit_fixture(3_u64);
        ::core::assert_eq!(exact_fixture_observation.cardinality, 2_u64);
        ::core::assert_eq!(plus_one_fixture_observation.cardinality, 3_u64);
        ::core::assert_eq!(exact_fixture_observation.qualified_field, "ref_recovery_limits::orphan_origins");
        ::core::assert_eq!(plus_one_fixture_observation.qualified_field, "ref_recovery_limits::orphan_origins");
        ::core::assert_eq!(exact_fixture_observation.event_sites.as_slice(), &["ref.orphan_origin_read"]);
        ::core::assert_eq!(plus_one_fixture_observation.event_sites.as_slice(), &["ref.orphan_origin_read"]);
        let injected_limit = exact_fixture_observation.target_usage;
        ::core::assert!(injected_limit > 0_u64);
        ::core::assert!(injected_limit < REF_RECOVERY_MAX_ORPHAN_ORIGINS);
        ::core::assert!(plus_one_fixture_observation.target_usage > injected_limit);
        ::core::assert_eq!(super::REF_RECOVERY_MAX_ORPHAN_ORIGINS, 65_536);
        let default_limits = ref_recovery_limits();
        let mut exact_limits = ref_recovery_limits();
        let mut plus_one_limits = ref_recovery_limits();
        exact_limits.orphan_origins = injected_limit;
        plus_one_limits.orphan_origins = injected_limit;
        ::core::assert_ne!(injected_limit, default_limits.origin_fanout_directories);
        ::core::assert_eq!(exact_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_eq!(plus_one_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.origin_leaf_entries);
        ::core::assert_eq!(exact_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_eq!(plus_one_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.final_origins);
        ::core::assert_eq!(exact_limits.final_origins, default_limits.final_origins);
        ::core::assert_eq!(plus_one_limits.final_origins, default_limits.final_origins);
        ::core::assert_ne!(injected_limit, default_limits.origin_stages);
        ::core::assert_eq!(exact_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_eq!(plus_one_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_ne!(injected_limit, default_limits.origin_record_bytes);
        ::core::assert_eq!(exact_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_eq!(plus_one_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.ref_fanout_directories);
        ::core::assert_eq!(exact_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_eq!(plus_one_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.ref_leaf_entries);
        ::core::assert_eq!(exact_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_eq!(plus_one_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.ref_stages);
        ::core::assert_eq!(exact_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_eq!(plus_one_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_ne!(injected_limit, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(exact_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(plus_one_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(default_limits.orphan_origins, REF_RECOVERY_MAX_ORPHAN_ORIGINS);
        ::core::assert_eq!(exact_limits.orphan_origins, injected_limit);
        ::core::assert_eq!(plus_one_limits.orphan_origins, injected_limit);
        ::core::assert_ne!(injected_limit, default_limits.visible_branches);
        ::core::assert_eq!(exact_limits.visible_branches, default_limits.visible_branches);
        ::core::assert_eq!(plus_one_limits.visible_branches, default_limits.visible_branches);
        let exact_owner_root = exact_branch_repository.root();
        let plus_one_owner_root = plus_one_branch_repository.root();
        ::core::assert_ne!(exact_owner_root, plus_one_owner_root);
        let plus_one_owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        let exact_probe = begin_s20_530_limit_probe(exact_owner_root, "ref_recovery_limits::orphan_origins", injected_limit, &["ref.orphan_origin_read"]);
        let exact_result = exact_branch_repository.recover_refs_with_maintenance_and_limits(&exact_maintenance, exact_limits);
        let exact_runtime_observation = finish_s20_530_limit_probe(exact_probe);
        ::core::assert!(exact_result.is_ok());
        ::core::assert_eq!(exact_runtime_observation.qualified_field, "ref_recovery_limits::orphan_origins");
        ::core::assert_eq!(exact_runtime_observation.event_sites.as_slice(), &["ref.orphan_origin_read"]);
        ::core::assert_eq!(exact_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.fixture_cardinality, exact_fixture_observation.cardinality);
        ::core::assert_eq!(exact_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.rejected_target_usage, ::core::option::Option::None);
        let plus_one_probe = begin_s20_530_limit_probe(plus_one_owner_root, "ref_recovery_limits::orphan_origins", injected_limit, &["ref.orphan_origin_read"]);
        let plus_one_result = plus_one_branch_repository.recover_refs_with_maintenance_and_limits(&plus_one_maintenance, plus_one_limits);
        let plus_one_runtime_observation = finish_s20_530_limit_probe(plus_one_probe);
        ::core::assert!(plus_one_result.is_err());
        let limit_plus_one_error = plus_one_result.expect_err("expected limit-plus-one error");
        let plus_one_owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        ::core::assert_eq!(plus_one_owner_tree_before_snapshot, plus_one_owner_tree_after_snapshot);
        ::core::assert_eq!(limit_plus_one_error.code(), "BRANCH_RESOURCE_LIMIT");
        ::core::assert_eq!(plus_one_runtime_observation.qualified_field, "ref_recovery_limits::orphan_origins");
        ::core::assert_eq!(plus_one_runtime_observation.event_sites.as_slice(), &["ref.orphan_origin_read"]);
        ::core::assert_eq!(plus_one_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.fixture_cardinality, plus_one_fixture_observation.cardinality);
        ::core::assert_eq!(plus_one_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.rejected_target_usage, ::core::option::Option::Some(plus_one_fixture_observation.target_usage));
    }

    #[test]
    fn limit03_visible_branches_exact_and_plus_one() {
        let (exact_branch_repository, exact_maintenance, exact_fixture_observation) = prepare_s20_530_limit_03_visible_branches_limit_fixture(2_u64);
        let (plus_one_branch_repository, plus_one_maintenance, plus_one_fixture_observation) = prepare_s20_530_limit_03_visible_branches_limit_fixture(3_u64);
        ::core::assert_eq!(exact_fixture_observation.cardinality, 2_u64);
        ::core::assert_eq!(plus_one_fixture_observation.cardinality, 3_u64);
        ::core::assert_eq!(exact_fixture_observation.qualified_field, "ref_recovery_limits::visible_branches");
        ::core::assert_eq!(plus_one_fixture_observation.qualified_field, "ref_recovery_limits::visible_branches");
        ::core::assert_eq!(exact_fixture_observation.event_sites.as_slice(), &["ref.visible_record_read"]);
        ::core::assert_eq!(plus_one_fixture_observation.event_sites.as_slice(), &["ref.visible_record_read"]);
        let injected_limit = exact_fixture_observation.target_usage;
        ::core::assert!(injected_limit > 0_u64);
        ::core::assert!(injected_limit < REF_RECOVERY_MAX_VISIBLE_BRANCHES);
        ::core::assert!(plus_one_fixture_observation.target_usage > injected_limit);
        ::core::assert_eq!(super::REF_RECOVERY_MAX_VISIBLE_BRANCHES, 4_096);
        let default_limits = ref_recovery_limits();
        let mut exact_limits = ref_recovery_limits();
        let mut plus_one_limits = ref_recovery_limits();
        exact_limits.visible_branches = injected_limit;
        plus_one_limits.visible_branches = injected_limit;
        ::core::assert_ne!(injected_limit, default_limits.origin_fanout_directories);
        ::core::assert_eq!(exact_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_eq!(plus_one_limits.origin_fanout_directories, default_limits.origin_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.origin_leaf_entries);
        ::core::assert_eq!(exact_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_eq!(plus_one_limits.origin_leaf_entries, default_limits.origin_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.final_origins);
        ::core::assert_eq!(exact_limits.final_origins, default_limits.final_origins);
        ::core::assert_eq!(plus_one_limits.final_origins, default_limits.final_origins);
        ::core::assert_ne!(injected_limit, default_limits.origin_stages);
        ::core::assert_eq!(exact_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_eq!(plus_one_limits.origin_stages, default_limits.origin_stages);
        ::core::assert_ne!(injected_limit, default_limits.origin_record_bytes);
        ::core::assert_eq!(exact_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_eq!(plus_one_limits.origin_record_bytes, default_limits.origin_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.ref_fanout_directories);
        ::core::assert_eq!(exact_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_eq!(plus_one_limits.ref_fanout_directories, default_limits.ref_fanout_directories);
        ::core::assert_ne!(injected_limit, default_limits.ref_leaf_entries);
        ::core::assert_eq!(exact_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_eq!(plus_one_limits.ref_leaf_entries, default_limits.ref_leaf_entries);
        ::core::assert_ne!(injected_limit, default_limits.ref_stages);
        ::core::assert_eq!(exact_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_eq!(plus_one_limits.ref_stages, default_limits.ref_stages);
        ::core::assert_ne!(injected_limit, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(exact_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_eq!(plus_one_limits.visible_ref_record_bytes, default_limits.visible_ref_record_bytes);
        ::core::assert_ne!(injected_limit, default_limits.orphan_origins);
        ::core::assert_eq!(exact_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_eq!(plus_one_limits.orphan_origins, default_limits.orphan_origins);
        ::core::assert_eq!(default_limits.visible_branches, REF_RECOVERY_MAX_VISIBLE_BRANCHES);
        ::core::assert_eq!(exact_limits.visible_branches, injected_limit);
        ::core::assert_eq!(plus_one_limits.visible_branches, injected_limit);
        let exact_owner_root = exact_branch_repository.root();
        let plus_one_owner_root = plus_one_branch_repository.root();
        ::core::assert_ne!(exact_owner_root, plus_one_owner_root);
        let plus_one_owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        let exact_probe = begin_s20_530_limit_probe(exact_owner_root, "ref_recovery_limits::visible_branches", injected_limit, &["ref.visible_record_read"]);
        let exact_result = exact_branch_repository.recover_refs_with_maintenance_and_limits(&exact_maintenance, exact_limits);
        let exact_runtime_observation = finish_s20_530_limit_probe(exact_probe);
        ::core::assert!(exact_result.is_ok());
        ::core::assert_eq!(exact_runtime_observation.qualified_field, "ref_recovery_limits::visible_branches");
        ::core::assert_eq!(exact_runtime_observation.event_sites.as_slice(), &["ref.visible_record_read"]);
        ::core::assert_eq!(exact_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.fixture_cardinality, exact_fixture_observation.cardinality);
        ::core::assert_eq!(exact_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(exact_runtime_observation.rejected_target_usage, ::core::option::Option::None);
        let plus_one_probe = begin_s20_530_limit_probe(plus_one_owner_root, "ref_recovery_limits::visible_branches", injected_limit, &["ref.visible_record_read"]);
        let plus_one_result = plus_one_branch_repository.recover_refs_with_maintenance_and_limits(&plus_one_maintenance, plus_one_limits);
        let plus_one_runtime_observation = finish_s20_530_limit_probe(plus_one_probe);
        ::core::assert!(plus_one_result.is_err());
        let limit_plus_one_error = plus_one_result.expect_err("expected limit-plus-one error");
        let plus_one_owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(plus_one_owner_root);
        ::core::assert_eq!(plus_one_owner_tree_before_snapshot, plus_one_owner_tree_after_snapshot);
        ::core::assert_eq!(limit_plus_one_error.code(), "BRANCH_RESOURCE_LIMIT");
        ::core::assert_eq!(plus_one_runtime_observation.qualified_field, "ref_recovery_limits::visible_branches");
        ::core::assert_eq!(plus_one_runtime_observation.event_sites.as_slice(), &["ref.visible_record_read"]);
        ::core::assert_eq!(plus_one_runtime_observation.injected_limit, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.fixture_cardinality, plus_one_fixture_observation.cardinality);
        ::core::assert_eq!(plus_one_runtime_observation.scanned_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.retained_peak, injected_limit);
        ::core::assert_eq!(plus_one_runtime_observation.rejected_target_usage, ::core::option::Option::Some(plus_one_fixture_observation.target_usage));
    }

    #[allow(dead_code)]
    fn expected_recovery_ancestry_test_plan_digest(
        owner_root: &::std::path::Path,
        epochs: ::sley_txn::recovery_ancestry_test_hook::RecoveryAncestryTestEpochs,
        left: ::sley_id::TransactionId,
        right: ::sley_id::TransactionId,
    ) -> [u8; 32] {
        let epoch_budget = match epochs {
            ::sley_txn::recovery_ancestry_test_hook::RecoveryAncestryTestEpochs::One => 1_u64,
            ::sley_txn::recovery_ancestry_test_hook::RecoveryAncestryTestEpochs::Two => 2_u64,
        };
        let root_bytes = owner_root.as_os_str().as_encoded_bytes();
        let root_len = u64::try_from(root_bytes.len()).expect("test root length");
        let mut hasher = ::blake3::Hasher::new();
        hasher.update(b"sley2.s20-530.recovery-ancestry-test-plan.v1");
        hasher.update(&root_len.to_le_bytes());
        hasher.update(root_bytes);
        hasher.update(&epoch_budget.to_le_bytes());
        hasher.update(left.as_bytes());
        hasher.update(right.as_bytes());
        *hasher.finalize().as_bytes()
    }

    struct MappedBranchName(BranchName);

    impl AsRef<[u8]> for MappedBranchName {
        fn as_ref(&self) -> &[u8] {
            self.0.as_bytes()
        }
    }

    impl ::core::ops::Deref for MappedBranchName {
        type Target = BranchName;

        fn deref(&self) -> &BranchName {
            &self.0
        }
    }

    type MappedRevisionClaim = (
        TransactionId,
        WorkspaceId,
        StateRoot,
        SchemaEpochId,
        PolicyRootId,
        ::std::vec::Vec<StateRoot>,
    );

    fn mapped_origin_claim(record: &BranchRecord) -> MappedRevisionClaim {
        (
            record.origin_transaction_id,
            record.workspace_id,
            record.origin_state_root,
            record.schema_epoch_id,
            record.policy_root_id,
            record.dependency_roots.clone(),
        )
    }

    fn mapped_ref_claim(record: &BranchRefRecord) -> MappedRevisionClaim {
        (
            record.head_transaction_id,
            record.workspace_id,
            record.head_state_root,
            record.schema_epoch_id,
            record.policy_root_id,
            record.dependency_roots.clone(),
        )
    }

    fn mapped_verified_claim(revision: &VerifiedRevision) -> MappedRevisionClaim {
        (
            revision.transaction_id(),
            revision.state_root().record.workspace_id,
            revision.state_root().root,
            revision.state_root().record.schema_epoch_id,
            revision.policy_root().root(),
            revision.state_root().record.dependency_roots.clone(),
        )
    }

    struct Anc05Provenance {
        genesis_identity: TransactionId,
        deep_missing_identity: TransactionId,
        direct_parent_identity: TransactionId,
        head_identity: TransactionId,
        pointer_identity: TransactionId,
        head_parent_identity: TransactionId,
        direct_parent_parent_identity: TransactionId,
        deep_missing_parent_identity: TransactionId,
        deep_missing_depth: u64,
        derived_deep_missing_identity: TransactionId,
        deep_missing_receipt_path: PathBuf,
        derived_deep_missing_receipt_path: PathBuf,
        genesis_receipt_kind: &'static str,
        direct_parent_receipt_kind: &'static str,
        head_receipt_kind: &'static str,
        origin_identity: TransactionId,
        ref_identity: TransactionId,
        origin_claim: MappedRevisionClaim,
        verified_origin_claim: MappedRevisionClaim,
        head_claim: MappedRevisionClaim,
        verified_head_claim: MappedRevisionClaim,
    }

    fn anc05_provenance(fixture: &Fixture) -> (Anc05Provenance, PathBuf, PathBuf) {
        let root = fixture.path().to_path_buf();
        let genesis_identity = fixture.genesis_transaction_id;
        let deep_missing = fixture.commit_child(71);
        let direct_parent = fixture.commit_child(72);
        let head = fixture.commit_child(73);
        let branch_name = BranchName::parse("anc05-branch").unwrap();
        fixture
            .branches
            .create_branch("anc05-branch", genesis_identity)
            .unwrap();
        fixture
            .branches
            .advance_branch("anc05-branch", genesis_identity, deep_missing)
            .unwrap();
        fixture
            .branches
            .advance_branch("anc05-branch", deep_missing, direct_parent)
            .unwrap();
        fixture
            .branches
            .advance_branch("anc05-branch", direct_parent, head)
            .unwrap();
        let resolved = fixture.branches.resolve_branch("anc05-branch").unwrap();
        let pointer_path = fixture.branches.checked_ref_path(&branch_name).unwrap();
        let head_revision = fixture.transactions.verified_revision(head).unwrap();
        let head_parent_identity = head_revision
            .receipt()
            .transaction
            .record
            .parent_transaction_ids[0];
        let direct_parent_revision = fixture.transactions.verified_revision(direct_parent).unwrap();
        let direct_parent_parent_identity = direct_parent_revision
            .receipt()
            .transaction
            .record
            .parent_transaction_ids[0];
        let deep_missing_revision = fixture.transactions.verified_revision(deep_missing).unwrap();
        let deep_missing_parent_identity = deep_missing_revision
            .receipt()
            .transaction
            .record
            .parent_transaction_ids[0];
        let origin_revision = fixture
            .transactions
            .verified_revision(resolved.origin.record.origin_transaction_id)
            .unwrap();
        let derived_deep_missing_identity = direct_parent_parent_identity;
        let deep_missing_receipt_path = transaction_receipt_path(&root, deep_missing);
        let derived_deep_missing_receipt_path =
            transaction_receipt_path(&root, derived_deep_missing_identity);
        let genesis_receipt_kind =
            exact_path_snapshot(&transaction_receipt_path(&root, genesis_identity)).0;
        let direct_parent_receipt_kind =
            exact_path_snapshot(&transaction_receipt_path(&root, direct_parent)).0;
        let head_receipt_kind = exact_path_snapshot(&transaction_receipt_path(&root, head)).0;
        fs::remove_file(&deep_missing_receipt_path).unwrap();
        let provenance = Anc05Provenance {
            genesis_identity,
            deep_missing_identity: deep_missing,
            direct_parent_identity: direct_parent,
            head_identity: head,
            pointer_identity: resolved.reference.record.head_transaction_id,
            head_parent_identity,
            direct_parent_parent_identity,
            deep_missing_parent_identity,
            deep_missing_depth: 2_u64,
            derived_deep_missing_identity,
            deep_missing_receipt_path: deep_missing_receipt_path.clone(),
            derived_deep_missing_receipt_path,
            genesis_receipt_kind,
            direct_parent_receipt_kind,
            head_receipt_kind,
            origin_identity: resolved.origin.record.origin_transaction_id,
            ref_identity: resolved.reference.record.head_transaction_id,
            origin_claim: mapped_origin_claim(&resolved.origin.record),
            verified_origin_claim: mapped_verified_claim(&origin_revision),
            head_claim: mapped_ref_claim(&resolved.reference.record),
            verified_head_claim: mapped_verified_claim(&head_revision),
        };
        (provenance, pointer_path, deep_missing_receipt_path)
    }

    struct RefGuardProvenance {
        direct_target_identity: TransactionId,
        authority_target_identity: TransactionId,
        direct_target_receipt_path: PathBuf,
        expected_direct_target_receipt_path: PathBuf,
        owner_origin_stage_relative_path: PathBuf,
        owner_origin_stage_expected_bytes: Vec<u8>,
        owner_ref_stage_relative_path: PathBuf,
        owner_ref_stage_expected_bytes: Vec<u8>,
        guard_origin_stage_relative_path: PathBuf,
        guard_origin_stage_expected_bytes: Vec<u8>,
        guard_ref_stage_relative_path: PathBuf,
        guard_ref_stage_expected_bytes: Vec<u8>,
    }

    struct RefGuardTopology {
        provenance: RefGuardProvenance,
        pointer_path: PathBuf,
        direct_target_receipt_path: PathBuf,
        owner_origin_stage_path: PathBuf,
        owner_ref_stage_path: PathBuf,
        guard_origin_stage_path: PathBuf,
        guard_ref_stage_path: PathBuf,
    }

    fn plant_ref_guard_canaries(root: &Path, tag: u8) -> (PathBuf, PathBuf, Vec<u8>) {
        let bytes = ::std::vec![tag; 9];
        let origin_leaf = root.join("branches").join("v1").join("aa").join("bb");
        fs::create_dir_all(&origin_leaf).unwrap();
        let origin_stage = origin_leaf.join(".sley-branch-stage-7-0000000000000000.tmp");
        fs::write(&origin_stage, &bytes).unwrap();
        let ref_leaf = root.join("refs").join("v1").join("aa").join("bb");
        fs::create_dir_all(&ref_leaf).unwrap();
        let ref_stage = ref_leaf.join(".sley-ref-stage-7-0000000000000000.tmp");
        fs::write(&ref_stage, &bytes).unwrap();
        (origin_stage, ref_stage, bytes)
    }

    fn ref_guard_topology(
        fixture: &Fixture,
        guard_fixture: Option<&Fixture>,
        label: &str,
        tag: u8,
    ) -> RefGuardTopology {
        let root = fixture.path().to_path_buf();
        let guard_root = guard_fixture
            .map_or_else(|| root.clone(), |guard| guard.path().to_path_buf());
        let genesis = fixture.genesis_transaction_id;
        let head = fixture.commit_child(tag);
        fixture.branches.create_branch(label, genesis).unwrap();
        fixture.branches.advance_branch(label, genesis, head).unwrap();
        let branch_name = BranchName::parse(label).unwrap();
        let resolved = fixture.branches.resolve_branch(label).unwrap();
        let authority_target_identity = resolved.reference.record.head_transaction_id;
        let pointer_path = fixture.branches.checked_ref_path(&branch_name).unwrap();
        let direct_target_receipt_path = transaction_receipt_path(&root, head);
        let expected_direct_target_receipt_path =
            transaction_receipt_path(&root, authority_target_identity);
        fs::remove_file(&direct_target_receipt_path).unwrap();
        let (owner_origin_stage_path, owner_ref_stage_path, owner_bytes) =
            plant_ref_guard_canaries(&root, tag);
        let (guard_origin_stage_path, guard_ref_stage_path, guard_bytes) =
            match guard_fixture {
                Some(guard) => plant_ref_guard_canaries(guard.path(), tag ^ 0x0f),
                None => (
                    owner_origin_stage_path.clone(),
                    owner_ref_stage_path.clone(),
                    owner_bytes.clone(),
                ),
            };
        let provenance = RefGuardProvenance {
            direct_target_identity: head,
            authority_target_identity,
            direct_target_receipt_path: direct_target_receipt_path.clone(),
            expected_direct_target_receipt_path,
            owner_origin_stage_relative_path: owner_origin_stage_path
                .strip_prefix(&root)
                .unwrap()
                .to_path_buf(),
            owner_origin_stage_expected_bytes: owner_bytes.clone(),
            owner_ref_stage_relative_path: owner_ref_stage_path
                .strip_prefix(&root)
                .unwrap()
                .to_path_buf(),
            owner_ref_stage_expected_bytes: owner_bytes,
            guard_origin_stage_relative_path: guard_origin_stage_path
                .strip_prefix(&guard_root)
                .unwrap()
                .to_path_buf(),
            guard_origin_stage_expected_bytes: guard_bytes.clone(),
            guard_ref_stage_relative_path: guard_ref_stage_path
                .strip_prefix(&guard_root)
                .unwrap()
                .to_path_buf(),
            guard_ref_stage_expected_bytes: guard_bytes,
        };
        RefGuardTopology {
            provenance,
            pointer_path,
            direct_target_receipt_path,
            owner_origin_stage_path,
            owner_ref_stage_path,
            guard_origin_stage_path,
            guard_ref_stage_path,
        }
    }

    struct Anc08Provenance {
        genesis_identity: TransactionId,
        left_identity: TransactionId,
        right_identity: TransactionId,
        pointer_identity: TransactionId,
        durable_left_parents: Vec<TransactionId>,
        durable_right_parents: Vec<TransactionId>,
        plan_owner_root: PathBuf,
        logical_left_parents: Vec<TransactionId>,
        logical_right_parents: Vec<TransactionId>,
        plan_consumption_counts: Vec<(TransactionId, u64)>,
        origin_identity: TransactionId,
        ref_identity: TransactionId,
        origin_claim: MappedRevisionClaim,
        verified_origin_claim: MappedRevisionClaim,
        left_claim: MappedRevisionClaim,
        verified_left_claim: MappedRevisionClaim,
    }

    type MappedCycleObservation = (TransactionId, Vec<TransactionId>, Vec<TransactionId>);

    fn anc08_provenance(fixture: &Fixture) -> (Anc08Provenance, PathBuf) {
        let genesis_identity = fixture.genesis_transaction_id;
        let right = fixture.commit_child(81);
        let left = fixture.commit_child(82);
        fixture
            .branches
            .create_branch("anc08-branch", genesis_identity)
            .unwrap();
        fixture
            .branches
            .advance_branch("anc08-branch", genesis_identity, right)
            .unwrap();
        fixture
            .branches
            .advance_branch("anc08-branch", right, left)
            .unwrap();
        let branch_name = BranchName::parse("anc08-branch").unwrap();
        let resolved = fixture.branches.resolve_branch("anc08-branch").unwrap();
        let pointer_path = fixture.branches.checked_ref_path(&branch_name).unwrap();
        let left_revision = fixture.transactions.verified_revision(left).unwrap();
        let right_revision = fixture.transactions.verified_revision(right).unwrap();
        let origin_revision = fixture
            .transactions
            .verified_revision(resolved.origin.record.origin_transaction_id)
            .unwrap();
        let provenance = Anc08Provenance {
            genesis_identity,
            left_identity: left,
            right_identity: right,
            pointer_identity: resolved.reference.record.head_transaction_id,
            durable_left_parents: left_revision
                .receipt()
                .transaction
                .record
                .parent_transaction_ids
                .clone(),
            durable_right_parents: right_revision
                .receipt()
                .transaction
                .record
                .parent_transaction_ids
                .clone(),
            plan_owner_root: PathBuf::new(),
            logical_left_parents: vec![right],
            logical_right_parents: vec![left],
            plan_consumption_counts: Vec::new(),
            origin_identity: resolved.origin.record.origin_transaction_id,
            ref_identity: resolved.reference.record.head_transaction_id,
            origin_claim: mapped_origin_claim(&resolved.origin.record),
            verified_origin_claim: mapped_verified_claim(&origin_revision),
            left_claim: mapped_ref_claim(&resolved.reference.record),
            verified_left_claim: mapped_verified_claim(&left_revision),
        };
        (provenance, pointer_path)
    }

    fn install_recovery_ancestry_l_r_l_test_plan(
        transactions: &::sley_txn::TransactionRepository,
        maintenance: &RepositoryMaintenanceGuard,
        left: TransactionId,
        right: TransactionId,
    ) -> ::sley_txn::recovery_ancestry_test_hook::RecoveryAncestryTestPlanIdentity {
        let identity = ::sley_txn::recovery_ancestry_test_hook::install(
            transactions,
            maintenance,
            ::sley_txn::recovery_ancestry_test_hook::RecoveryAncestryTestEpochs::One,
            left,
            right,
        )
        .unwrap();
        drop(::sley_txn::recovery_ancestry_test_hook::begin_ref_operation(transactions, maintenance));
        identity
    }

    fn consumed_l_r_l_cycle_observations(
        transactions: &::sley_txn::TransactionRepository,
        maintenance: &RepositoryMaintenanceGuard,
        identity: ::sley_txn::recovery_ancestry_test_hook::RecoveryAncestryTestPlanIdentity,
        provenance: &Anc08Provenance,
    ) -> (Vec<MappedCycleObservation>, Vec<(TransactionId, u64)>) {
        let observations = ::sley_txn::recovery_ancestry_test_hook::take(transactions, maintenance, identity).unwrap();
        assert_eq!(observations.epoch_budget(), 1);
        assert_eq!(observations.remaining_epochs(), 0);
        assert_eq!(observations.operation_1().claims(), 0);
        assert_eq!(observations.operation_1().edge_counts(), (0, 0));
        assert_eq!(observations.operation_2().claims(), 1);
        let (left_edges, right_edges) = observations.operation_2().edge_counts();
        assert_eq!((left_edges, right_edges), (1, 1));
        (
            vec![
                (
                    provenance.left_identity,
                    provenance.durable_left_parents.clone(),
                    provenance.logical_left_parents.clone(),
                ),
                (
                    provenance.right_identity,
                    provenance.durable_right_parents.clone(),
                    provenance.logical_right_parents.clone(),
                ),
            ],
            vec![
                (provenance.left_identity, left_edges),
                (provenance.right_identity, right_edges),
            ],
        )
    }

    fn cross05_recovery_fixture() -> Fixture {
        Fixture::new("cross05-ref-wrapper")
    }

    struct Cross05CompositeFixture {
        fixture: Fixture,
        commit_expected_parent: TransactionId,
        commit_candidate_bytes: Vec<u8>,
        commit_principal_id: PrincipalId,
        branch_name: Vec<u8>,
        branch_target: TransactionId,
        gc_snapshot: RetentionSnapshot,
        gc_verifier: Cross05GcVerifier,
    }

    struct Cross05GcVerifier {
        schema_epoch_id: SchemaEpochId,
    }

    impl CanonicalVerifier for Cross05GcVerifier {
        fn verify(&self, record: &[u8]) -> core::result::Result<ObjectId, sley_scb1::ScbError> {
            import_entity_object(self.schema_epoch_id, record).map(|object| object.object_id())
        }
    }

    impl GcObjectVerifier for Cross05GcVerifier {
        fn references(
            &self,
            record: &[u8],
        ) -> core::result::Result<Vec<ObjectId>, sley_scb1::ScbError> {
            let _ = <Self as CanonicalVerifier>::verify(self, record)?;
            Ok(Vec::new())
        }
    }

    fn cross05_composite_fixture() -> Cross05CompositeFixture {
        let fixture = Fixture::new("cross05-caller-held");
        let accepted = fixture.transactions.accepted_head().unwrap();
        let commit_expected_parent = accepted.transaction_id();
        let commit_principal_id = fixture.principal_id;
        let candidate = candidate_for(
            accepted.state_root().record.workspace_id,
            fixture.principal_id,
            accepted.transaction_id(),
            accepted.state_root(),
            accepted.policy_root(),
            60,
        );
        let context = CandidateValidationContext::new(
            accepted.transaction_id(),
            accepted.state_root(),
            accepted.objects(),
            accepted.tombstoned_entities(),
            accepted.policy_root(),
            fixture.principal_id,
            &[],
            NOW,
            CandidateValidationLimits::full_v1(),
        )
        .unwrap();
        let validation = validate_candidate_bytes(&context, &candidate.stored_bytes).unwrap();
        let proposed_objects = validation
            .validated_plan()
            .unwrap()
            .proposed_state()
            .entities()
            .to_vec();
        let gc_verifier = Cross05GcVerifier {
            schema_epoch_id: accepted.state_root().record.schema_epoch_id,
        };
        let object_store = ObjectStore::new(fixture.path());
        for object in &proposed_objects {
            object_store
                .put(object.object_id(), object.stored_bytes(), &gc_verifier)
                .unwrap();
        }
        let targets = proposed_objects
            .iter()
            .map(|object| RetentionTarget::Object(object.object_id()))
            .collect();
        let gc_snapshot = RetentionSnapshot::new(
            vec![RetentionAnchor::new(
                RetentionKind::ProtectedRoot,
                [0xc5; 32],
                targets,
            )],
            Vec::new(),
        )
        .unwrap();
        drop(validation);
        drop(context);
        drop(accepted);
        Cross05CompositeFixture {
            fixture,
            commit_expected_parent,
            commit_candidate_bytes: candidate.stored_bytes,
            commit_principal_id,
            branch_name: b"cross05".to_vec(),
            branch_target: commit_expected_parent,
            gc_snapshot,
            gc_verifier,
        }
    }

    fn fixed<T>(byte: u8, constructor: impl FnOnce([u8; 32]) -> T) -> T {
        constructor([byte; 32])
    }

    fn copy_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let destination_path = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &destination_path);
            } else {
                fs::copy(entry.path(), destination_path).unwrap();
            }
        }
    }

    fn import_fixture_revisions(source: &Fixture, destination: &Fixture) {
        for relative in [Path::new("transactions/v1"), Path::new("objects/scb1")] {
            copy_tree(
                &source.temp.path.join(relative),
                &destination.temp.path.join(relative),
            );
        }
    }

    fn transaction_receipt_path(root: &Path, transaction_id: TransactionId) -> PathBuf {
        let hex = hex_digest(transaction_id.as_bytes());
        root.join("transactions")
            .join("v1")
            .join(&hex[0..2])
            .join(&hex[2..4])
            .join(format!("{hex}.receipt.scb1"))
    }

    fn corrupt_nested_receipt(
        receipt: &sley_txn::ImportedTransactionReceipt,
        nested: &[u8],
    ) -> Vec<u8> {
        let mut bytes = receipt.stored_bytes.clone();
        let offset = bytes
            .windows(nested.len())
            .position(|window| window == nested)
            .unwrap();
        bytes[offset + nested.len() - 1] ^= 1;
        let preimage_len = bytes.len() - DIGEST_LEN;
        let digest = ReceiptId::derive(&bytes[..preimage_len]);
        bytes[preimage_len..].copy_from_slice(digest.as_bytes());
        bytes
    }

    type RefCodecOriginObservation = (
        TransactionId,
        TransactionId,
        ::std::collections::BTreeSet<TransactionId>,
    );

    struct RefNestedCodecOriginFixture {
        m2_primary_locator: ::std::string::String,
        m2_primary_nested_state_root_bytes: ::std::vec::Vec<u8>,
        m2_primary_path: ::std::path::PathBuf,
        m2_pristine_nested_state_root_bytes: ::std::vec::Vec<u8>,
        m2_pristine_primary_bytes: ::std::vec::Vec<u8>,
        m2_pristine_primary_observation: ExactPathSnapshot,
        m2_ref_head_receipt_path: ::std::path::PathBuf,
        m2_secondary_head_transaction_id: TransactionId,
        m2_secondary_locator: ::std::string::String,
        m2_secondary_only_observation: RefCodecOriginObservation,
        m2_secondary_only_owner_tree: ExactTreeSnapshot,
        m2_secondary_origin_transaction_id: TransactionId,
        m2_secondary_reachable_transaction_ids:
            ::std::collections::BTreeSet<TransactionId>,
        m2_arguments_1: (),
        m2_arguments_2: (),
    }

    type GroupedOriginFormatSecondaryObservation = (
        TransactionId,
        TransactionId,
        ::std::collections::BTreeSet<TransactionId>,
    );

    struct GroupedOriginFormatNonancestorFixture {
        m2_branch_origin_path: ::std::path::PathBuf,
        m2_primary_carrier_path: ::std::path::PathBuf,
        m2_primary_origin_format_version: u32,
        m2_primary_path: ::std::path::PathBuf,
        m2_pristine_primary_bytes: ::std::vec::Vec<u8>,
        m2_pristine_primary_observation: ExactPathSnapshot,
        m2_secondary_canonical_origin_record_bytes: ::std::vec::Vec<u8>,
        m2_secondary_carrier_path: ::std::path::PathBuf,
        m2_secondary_only_observation: GroupedOriginFormatSecondaryObservation,
        m2_secondary_only_owner_tree: ExactTreeSnapshot,
        m2_secondary_origin_record_bytes: ::std::vec::Vec<u8>,
        m2_secondary_origin_transaction_id: TransactionId,
        m2_secondary_reachable_transaction_ids:
            ::std::collections::BTreeSet<TransactionId>,
        m2_supported_origin_format_version: u32,
        m2_arguments_1: (),
        m2_arguments_2: (),
    }

    trait BranchOriginAncestryFixture {
        fn secondary_origin_transaction_id(&self) -> TransactionId;

        fn secondary_reachable_transaction_ids(
            &self,
        ) -> &::std::collections::BTreeSet<TransactionId>;
    }

    impl BranchOriginAncestryFixture for RefNestedCodecOriginFixture {
        fn secondary_origin_transaction_id(&self) -> TransactionId {
            self.m2_secondary_origin_transaction_id
        }

        fn secondary_reachable_transaction_ids(
            &self,
        ) -> &::std::collections::BTreeSet<TransactionId> {
            &self.m2_secondary_reachable_transaction_ids
        }
    }

    impl BranchOriginAncestryFixture for GroupedOriginFormatNonancestorFixture {
        fn secondary_origin_transaction_id(&self) -> TransactionId {
            self.m2_secondary_origin_transaction_id
        }

        fn secondary_reachable_transaction_ids(
            &self,
        ) -> &::std::collections::BTreeSet<TransactionId> {
            &self.m2_secondary_reachable_transaction_ids
        }
    }

    struct RefNestedStoreCycleFixture {
        selected_branch_name: BranchName,
        m2_primary_object_id: ObjectId,
        m2_primary_path: ::std::path::PathBuf,
        m2_schema_epoch_id: SchemaEpochId,
        m2_secondary_locator: ::std::string::String,
        m2_arguments_1: (),
        m2_arguments_2: (),
    }

    struct RefNestedStoreCyclePrimaryActivation {
        m2_primary_locator: ::std::string::String,
        m2_primary_object_bytes: ::std::vec::Vec<u8>,
    }

    #[derive(Debug)]
    struct MappedStoreProbeError(::sley_store::StoreError);

    impl MappedStoreProbeError {
        fn code(&self) -> &'static str {
            self.0.code().symbol()
        }
    }

    struct MappedRecoveryCycleEdges([(TransactionId, TransactionId); 2]);

    impl MappedRecoveryCycleEdges {
        fn as_slice(&self) -> [(TransactionId, TransactionId); 2] {
            self.0
        }
    }

    trait MappedRecoveryCycleDescriptorExt {
        fn to_vec(self) -> MappedRecoveryCycleEdges;
    }

    impl MappedRecoveryCycleDescriptorExt for [(TransactionId, TransactionId); 2] {
        fn to_vec(self) -> MappedRecoveryCycleEdges {
            MappedRecoveryCycleEdges(self)
        }
    }

    fn prepare_ref_nested_codec_origin_fixture(
        fixture: &Fixture,
    ) -> RefNestedCodecOriginFixture {
        let origin_source = Fixture::new("anc06-codec-origin-source");
        ::core::assert_eq!(
            origin_source.genesis_transaction_id,
            fixture.genesis_transaction_id
        );
        let origin_transaction_id = origin_source.commit_child(82);
        import_fixture_revisions(&origin_source, fixture);
        let head_transaction_id = fixture.commit_child(83);
        let branch_name = "anc06-codec-origin";
        fixture
            .branches
            .create_branch(branch_name, origin_transaction_id)
            .unwrap();
        let resolved_origin = fixture.branches.resolve_branch(branch_name).unwrap();
        let parsed_name = BranchName::parse(branch_name).unwrap();
        let head_revision = fixture
            .transactions
            .verified_revision(head_transaction_id)
            .unwrap();
        let replacement_ref = build_branch_ref(&ref_record(
            &parsed_name,
            resolved_origin.origin.digest,
            &head_revision,
        ))
        .unwrap();
        let ref_path = fixture.branches.checked_ref_path(&parsed_name).unwrap();
        ::std::fs::write(&ref_path, &replacement_ref.stored_bytes).unwrap();
        ::std::fs::File::open(&ref_path)
            .unwrap()
            .sync_all()
            .unwrap();
        super::sync_dir(ref_path.parent().unwrap()).unwrap();

        let m2_primary_path =
            transaction_receipt_path(fixture.path(), head_transaction_id);
        let m2_pristine_primary_bytes = ::std::fs::read(&m2_primary_path).unwrap();
        let imported = ::sley_txn::import_transaction_receipt(&m2_pristine_primary_bytes)
            .expect("multifault pristine ref-head receipt");
        let m2_pristine_nested_state_root_bytes = imported.record.stored_state_root.clone();
        let nested_offsets = m2_pristine_primary_bytes
            .windows(m2_pristine_nested_state_root_bytes.len())
            .enumerate()
            .filter_map(|(offset, window)| {
                (window == m2_pristine_nested_state_root_bytes.as_slice()).then_some(offset)
            })
            .collect::<::std::vec::Vec<_>>();
        ::core::assert_eq!(nested_offsets.len(), 1);
        let nested_start = nested_offsets[0];
        let nested_end = nested_start
            .checked_add(m2_pristine_nested_state_root_bytes.len())
            .unwrap();
        let m2_pristine_primary_observation = exact_path_snapshot(&m2_primary_path);
        let m2_secondary_only_owner_tree = exact_tree_snapshot(fixture.path());
        let corrupted = corrupt_nested_receipt(
            &imported,
            &m2_pristine_nested_state_root_bytes,
        );
        let m2_primary_nested_state_root_bytes = corrupted[nested_start..nested_end].to_vec();
        ::std::fs::write(&m2_primary_path, corrupted).unwrap();
        ::std::fs::File::open(&m2_primary_path)
            .unwrap()
            .sync_all()
            .unwrap();
        super::sync_dir(m2_primary_path.parent().unwrap()).unwrap();
        let m2_secondary_reachable_transaction_ids =
            ::std::collections::BTreeSet::from([
                fixture.genesis_transaction_id,
                head_transaction_id,
            ]);
        let m2_secondary_only_observation = (
            origin_transaction_id,
            head_transaction_id,
            m2_secondary_reachable_transaction_ids.clone(),
        );
        RefNestedCodecOriginFixture {
            m2_primary_locator: m2_primary_path.display().to_string(),
            m2_primary_nested_state_root_bytes,
            m2_primary_path: m2_primary_path.clone(),
            m2_pristine_nested_state_root_bytes,
            m2_pristine_primary_bytes,
            m2_pristine_primary_observation,
            m2_ref_head_receipt_path: m2_primary_path,
            m2_secondary_head_transaction_id: head_transaction_id,
            m2_secondary_locator: ::std::format!(
                "origin-relation:{origin_transaction_id:?}:{head_transaction_id:?}"
            ),
            m2_secondary_only_observation,
            m2_secondary_only_owner_tree,
            m2_secondary_origin_transaction_id: origin_transaction_id,
            m2_secondary_reachable_transaction_ids,
            m2_arguments_1: (),
            m2_arguments_2: (),
        }
    }

    fn prepare_ref_nested_store_cycle_fixture(
        fixture: &Fixture,
    ) -> RefNestedStoreCycleFixture {
        let right_transaction_id = fixture.commit_child(84);
        let right_revision = fixture
            .transactions
            .verified_revision(right_transaction_id)
            .unwrap();
        let right_changes = right_revision
            .receipt()
            .transaction
            .record
            .changed_entity_bindings
            .iter()
            .filter_map(|binding| binding.postimage.map(|postimage| (binding.entity_id, postimage)))
            .collect::<::std::vec::Vec<_>>();
        ::core::assert_eq!(right_changes.len(), 1);
        let (right_entity_id, right_object_id) = right_changes[0];
        let m2_schema_epoch_id = right_revision.receipt().state_root.record.schema_epoch_id;
        let left_transaction_id =
            fixture.delete_entity_on_head(right_entity_id, right_object_id, 85);
        let branch_name = "anc06-store-cycle";
        fixture
            .branches
            .create_branch(branch_name, fixture.genesis_transaction_id)
            .unwrap();
        fixture
            .branches
            .advance_branch(
                branch_name,
                fixture.genesis_transaction_id,
                right_transaction_id,
            )
            .unwrap();
        fixture
            .branches
            .advance_branch(branch_name, right_transaction_id, left_transaction_id)
            .unwrap();
        let selected_branch_name = BranchName::parse(branch_name).unwrap();
        RefNestedStoreCycleFixture {
            selected_branch_name,
            m2_primary_object_id: right_object_id,
            m2_primary_path: ObjectStore::new(fixture.path()).object_path(right_object_id),
            m2_schema_epoch_id,
            m2_secondary_locator: ::std::format!(
                "logical-cycle:{left_transaction_id:?}:{right_transaction_id:?}"
            ),
            m2_arguments_1: (),
            m2_arguments_2: (),
        }
    }

    fn prepare_grouped_origin_format_nonancestor_fixture(
        fixture: &Fixture,
    ) -> GroupedOriginFormatNonancestorFixture {
        let origin_source = Fixture::new("cor07-origin-format-source");
        ::core::assert_eq!(
            origin_source.genesis_transaction_id,
            fixture.genesis_transaction_id
        );
        let origin_transaction_id = origin_source.commit_child(86);
        import_fixture_revisions(&origin_source, fixture);
        let head_transaction_id = fixture.commit_child(87);
        let branch_name = "cor07-origin-format";
        fixture
            .branches
            .create_branch(branch_name, origin_transaction_id)
            .unwrap();
        let resolved_origin = fixture.branches.resolve_branch(branch_name).unwrap();
        let parsed_name = BranchName::parse(branch_name).unwrap();
        let head_revision = fixture
            .transactions
            .verified_revision(head_transaction_id)
            .unwrap();
        let replacement_ref = build_branch_ref(&ref_record(
            &parsed_name,
            resolved_origin.origin.digest,
            &head_revision,
        ))
        .unwrap();
        let ref_path = fixture.branches.checked_ref_path(&parsed_name).unwrap();
        ::std::fs::write(&ref_path, &replacement_ref.stored_bytes).unwrap();
        ::std::fs::File::open(&ref_path)
            .unwrap()
            .sync_all()
            .unwrap();
        super::sync_dir(ref_path.parent().unwrap()).unwrap();

        let m2_primary_path = fixture.branches.branch_path(&parsed_name);
        let m2_pristine_primary_bytes = ::std::fs::read(&m2_primary_path).unwrap();
        let m2_pristine_primary_observation = exact_path_snapshot(&m2_primary_path);
        let m2_secondary_only_owner_tree = exact_tree_snapshot(fixture.path());
        let m2_secondary_reachable_transaction_ids =
            ::std::collections::BTreeSet::from([
                fixture.genesis_transaction_id,
                head_transaction_id,
            ]);
        let m2_secondary_only_observation = (
            origin_transaction_id,
            head_transaction_id,
            m2_secondary_reachable_transaction_ids.clone(),
        );

        let mut corrupted = m2_pristine_primary_bytes.clone();
        let payload = payload_offset(&corrupted);
        corrupted[payload + 3] = 2;
        rehash(&mut corrupted, BRANCH_DIGEST_DOMAIN);
        ::std::fs::write(&m2_primary_path, corrupted).unwrap();
        ::std::fs::File::open(&m2_primary_path)
            .unwrap()
            .sync_all()
            .unwrap();
        super::sync_dir(m2_primary_path.parent().unwrap()).unwrap();

        GroupedOriginFormatNonancestorFixture {
            m2_branch_origin_path: m2_primary_path.clone(),
            m2_primary_carrier_path: m2_primary_path.clone(),
            m2_primary_origin_format_version: 2,
            m2_primary_path: m2_primary_path.clone(),
            m2_pristine_primary_bytes: m2_pristine_primary_bytes.clone(),
            m2_pristine_primary_observation,
            m2_secondary_canonical_origin_record_bytes: m2_pristine_primary_bytes.clone(),
            m2_secondary_carrier_path: m2_primary_path,
            m2_secondary_only_observation,
            m2_secondary_only_owner_tree,
            m2_secondary_origin_record_bytes: m2_pristine_primary_bytes,
            m2_secondary_origin_transaction_id: origin_transaction_id,
            m2_secondary_reachable_transaction_ids,
            m2_supported_origin_format_version: RECORD_VERSION,
            m2_arguments_1: (),
            m2_arguments_2: (),
        }
    }

    fn observe_ref_nested_codec_origin_fixture_primary(
        _fixture: &Fixture,
        m2_fixture: &RefNestedCodecOriginFixture,
    ) -> ExactPathSnapshot {
        exact_path_snapshot(&m2_fixture.m2_primary_path)
    }

    fn observe_ref_nested_codec_origin_fixture_secondary(
        _fixture: &Fixture,
        m2_fixture: &RefNestedCodecOriginFixture,
    ) -> RefCodecOriginObservation {
        m2_fixture.m2_secondary_only_observation.clone()
    }

    fn probe_ref_nested_codec_origin_receipt_error(
        _fixture: &Fixture,
        m2_fixture: &RefNestedCodecOriginFixture,
    ) -> ::core::result::Result<
        ::sley_txn::ImportedTransactionReceipt,
        ::sley_txn::TransactionCodecError,
    > {
        let bytes = ::std::fs::read(&m2_fixture.m2_primary_path).unwrap();
        ::sley_txn::import_transaction_receipt(&bytes)
    }

    fn probe_branch_origin_ancestry<T: BranchOriginAncestryFixture>(
        _fixture: &Fixture,
        m2_fixture: &T,
    ) -> ::core::result::Result<(), super::BranchError> {
        if m2_fixture
            .secondary_reachable_transaction_ids()
            .contains(&m2_fixture.secondary_origin_transaction_id())
        {
            ::core::result::Result::Ok(())
        } else {
            ::core::result::Result::Err(super::branch_error(
                super::BranchErrorCode::BranchOriginMismatch,
            ))
        }
    }

    fn observe_grouped_origin_format_nonancestor_fixture_primary(
        _fixture: &Fixture,
        m2_fixture: &GroupedOriginFormatNonancestorFixture,
    ) -> ExactPathSnapshot {
        exact_path_snapshot(&m2_fixture.m2_primary_path)
    }

    fn observe_grouped_origin_format_nonancestor_fixture_secondary(
        _fixture: &Fixture,
        m2_fixture: &GroupedOriginFormatNonancestorFixture,
    ) -> GroupedOriginFormatSecondaryObservation {
        m2_fixture.m2_secondary_only_observation.clone()
    }

    fn import_branch_record_error(
        _fixture: &Fixture,
        m2_fixture: &GroupedOriginFormatNonancestorFixture,
    ) -> ::core::result::Result<ImportedBranchRecord, super::BranchError> {
        let bytes = ::std::fs::read(&m2_fixture.m2_primary_path).map_err(super::BranchError::Io)?;
        import_branch_record(&bytes)
    }

    #[test]
    fn cor07_origin_format_branch_record_format_version() {
        let fixture = Fixture::new("s20-530-m2-cor-07-origin-format-branch-record-format-version");
        let transaction_repository: &::sley_txn::TransactionRepository = &fixture.transactions;
        let branch_repository: &super::BranchRepository = &fixture.branches;
        let owner_root = branch_repository.root();
        ::core::assert_eq!(owner_root, fixture.path());
        let m2_fixture = prepare_grouped_origin_format_nonancestor_fixture(&fixture);
        let m2_branch_origin_path = m2_fixture.m2_branch_origin_path.clone();
        let m2_primary_carrier_path = m2_fixture.m2_primary_carrier_path.clone();
        let m2_primary_origin_format_version = m2_fixture.m2_primary_origin_format_version.clone();
        let m2_primary_path = m2_fixture.m2_primary_path.clone();
        let m2_pristine_primary_bytes = m2_fixture.m2_pristine_primary_bytes.clone();
        let m2_pristine_primary_observation = m2_fixture.m2_pristine_primary_observation.clone();
        let m2_secondary_canonical_origin_record_bytes = m2_fixture.m2_secondary_canonical_origin_record_bytes.clone();
        let m2_secondary_carrier_path = m2_fixture.m2_secondary_carrier_path.clone();
        let m2_secondary_only_observation = m2_fixture.m2_secondary_only_observation.clone();
        let m2_secondary_only_owner_tree = m2_fixture.m2_secondary_only_owner_tree.clone();
        let m2_secondary_origin_record_bytes = m2_fixture.m2_secondary_origin_record_bytes.clone();
        let m2_secondary_origin_transaction_id = m2_fixture.m2_secondary_origin_transaction_id.clone();
        let m2_secondary_reachable_transaction_ids = m2_fixture.m2_secondary_reachable_transaction_ids.clone();
        let m2_supported_origin_format_version = m2_fixture.m2_supported_origin_format_version.clone();
        let maintenance = branch_repository.acquire_exclusive_maintenance().unwrap();
        ::core::assert_eq!(("primary_origin_path_from_branch", m2_primary_path.as_path()), ("primary_origin_path_from_branch", m2_branch_origin_path.as_path()));
        ::core::assert_ne!(("primary_origin_version_corrupt", m2_primary_origin_format_version), ("primary_origin_version_corrupt", m2_supported_origin_format_version));
        ::core::assert_eq!(("secondary_nonancestor_origin_canonical", m2_secondary_origin_record_bytes.as_slice()), ("secondary_nonancestor_origin_canonical", m2_secondary_canonical_origin_record_bytes.as_slice()));
        ::core::assert!(!m2_secondary_reachable_transaction_ids.contains(&m2_secondary_origin_transaction_id), "secondary_origin_not_reachable_from_head");
        ::core::assert_eq!(("same_carrier_origin_semantics", m2_primary_carrier_path.as_path()), ("same_carrier_origin_semantics", m2_secondary_carrier_path.as_path()));
        let m2_primary_probe_before = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_probe_result = import_branch_record_error(&fixture, &m2_fixture);
        let m2_primary_probe_error = m2_primary_probe_result.expect_err("expected primary multifault probe error");
        let m2_primary_probe_after = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(("primary_probe_branch_record_format", m2_primary_probe_error.code()), ("primary_probe_branch_record_format", "BRANCH_RECORD_FORMAT_VERSION"));
        ::core::assert_eq!(m2_primary_probe_before, m2_primary_probe_after);
        let m2_secondary_probe_before = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_secondary_probe_result = probe_branch_origin_ancestry(&fixture, &m2_fixture);
        let m2_secondary_probe_error = m2_secondary_probe_result.expect_err("expected secondary multifault probe error");
        let m2_secondary_probe_after = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(("secondary_probe_branch_origin_mismatch", m2_secondary_probe_error.code()), ("secondary_probe_branch_origin_mismatch", "BRANCH_ORIGIN_MISMATCH"));
        ::core::assert_eq!(m2_secondary_probe_before, m2_secondary_probe_after);
        let m2_receiver_identity_1 = branch_repository.root().to_path_buf();
        let m2_owner_root_1 = owner_root.to_path_buf();
        let m2_guard_identity_1 = maintenance.repository_root().to_path_buf();
        let m2_arguments_1 = m2_fixture.m2_arguments_1.clone();
        let m2_before_1 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_before_1 = observe_grouped_origin_format_nonancestor_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_before_1 = observe_grouped_origin_format_nonancestor_fixture_secondary(&fixture, &m2_fixture);
        let m2_result_1 = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(m2_result_1.is_err());
        let m2_error_1 = m2_result_1.expect_err("expected multifault precedence winner");
        let m2_after_1 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_after_1 = observe_grouped_origin_format_nonancestor_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_after_1 = observe_grouped_origin_format_nonancestor_fixture_secondary(&fixture, &m2_fixture);
        ::core::assert_eq!(("m2_operation_1_code", m2_error_1.code()), ("m2_operation_1_code", "BRANCH_RECORD_FORMAT_VERSION"));
        ::core::assert!(::core::matches!(&m2_error_1, super::BranchError::Branch(_)), "m2_operation_1_variant");
        ::core::assert_eq!(("m2_operation_1_source_chain", crate::refs::tests::exact_error_source_chain(&m2_error_1)), ("m2_operation_1_source_chain", []));
        ::core::assert_eq!(m2_before_1, m2_after_1);
        ::core::assert_eq!(m2_primary_before_1, m2_primary_after_1);
        ::core::assert_eq!(m2_secondary_before_1, m2_secondary_after_1);
        ::std::fs::write(&m2_primary_path, &m2_pristine_primary_bytes).unwrap();
        ::std::fs::File::open(&m2_primary_path).unwrap().sync_all().unwrap();
        super::sync_dir(m2_primary_path.parent().unwrap()).unwrap();
        let m2_primary_after_repair = observe_grouped_origin_format_nonancestor_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_after_repair = observe_grouped_origin_format_nonancestor_fixture_secondary(&fixture, &m2_fixture);
        let m2_after_repair = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(m2_primary_after_repair, m2_pristine_primary_observation);
        ::core::assert_eq!(m2_secondary_after_repair, m2_secondary_only_observation);
        ::core::assert_eq!(m2_after_repair, m2_secondary_only_owner_tree);
        let m2_receiver_identity_2 = branch_repository.root().to_path_buf();
        let m2_owner_root_2 = owner_root.to_path_buf();
        let m2_guard_identity_2 = maintenance.repository_root().to_path_buf();
        let m2_arguments_2 = m2_fixture.m2_arguments_2.clone();
        let m2_before_2 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_before_2 = observe_grouped_origin_format_nonancestor_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_before_2 = observe_grouped_origin_format_nonancestor_fixture_secondary(&fixture, &m2_fixture);
        ::core::assert_eq!(m2_receiver_identity_1, m2_receiver_identity_2);
        ::core::assert_eq!(m2_owner_root_1, m2_owner_root_2);
        ::core::assert_eq!(m2_guard_identity_1, m2_guard_identity_2);
        ::core::assert_eq!(m2_arguments_1, m2_arguments_2);
        let m2_result_2 = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(m2_result_2.is_err());
        let m2_error_2 = m2_result_2.expect_err("expected multifault precedence loser");
        let m2_after_2 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_after_2 = observe_grouped_origin_format_nonancestor_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_after_2 = observe_grouped_origin_format_nonancestor_fixture_secondary(&fixture, &m2_fixture);
        ::core::assert_eq!(("m2_operation_2_code", m2_error_2.code()), ("m2_operation_2_code", "BRANCH_ORIGIN_MISMATCH"));
        ::core::assert!(::core::matches!(&m2_error_2, super::BranchError::Branch(_)), "m2_operation_2_variant");
        ::core::assert_eq!(("m2_operation_2_source_chain", crate::refs::tests::exact_error_source_chain(&m2_error_2)), ("m2_operation_2_source_chain", []));
        ::core::assert_eq!(m2_before_2, m2_after_2);
        ::core::assert_eq!(m2_primary_before_2, m2_primary_after_2);
        ::core::assert_eq!(m2_secondary_before_2, m2_secondary_after_2);
    }

    fn activate_ref_nested_store_cycle_fixture_primary(
        _fixture: &Fixture,
        _m2_fixture: &RefNestedStoreCycleFixture,
        m2_primary_path: &::std::path::Path,
    ) -> RefNestedStoreCyclePrimaryActivation {
        let mut m2_primary_object_bytes = ::std::fs::read(m2_primary_path).unwrap();
        *m2_primary_object_bytes
            .last_mut()
            .expect("multifault object digest byte") ^= 1;
        ::std::fs::write(m2_primary_path, &m2_primary_object_bytes).unwrap();
        ::std::fs::File::open(m2_primary_path)
            .unwrap()
            .sync_all()
            .unwrap();
        super::sync_dir(m2_primary_path.parent().unwrap()).unwrap();
        RefNestedStoreCyclePrimaryActivation {
            m2_primary_locator: m2_primary_path.display().to_string(),
            m2_primary_object_bytes,
        }
    }

    fn observe_ref_nested_store_cycle_fixture_primary(
        _fixture: &Fixture,
        m2_fixture: &RefNestedStoreCycleFixture,
    ) -> ExactPathSnapshot {
        exact_path_snapshot(&m2_fixture.m2_primary_path)
    }

    fn observe_ref_nested_store_cycle_fixture_secondary(
        _fixture: &Fixture,
        m2_fixture: &RefNestedStoreCycleFixture,
    ) -> ::std::string::String {
        m2_fixture.m2_secondary_locator.clone()
    }

    fn probe_ref_nested_store_cycle_object_error(
        fixture: &Fixture,
        m2_fixture: &RefNestedStoreCycleFixture,
    ) -> ::core::result::Result<::std::vec::Vec<u8>, MappedStoreProbeError> {
        let verifier = |bytes: &[u8]| {
            import_entity_object(m2_fixture.m2_schema_epoch_id, bytes)
                .map(|object| object.object_id())
        };
        ObjectStore::new(fixture.path())
            .read(m2_fixture.m2_primary_object_id, &verifier)
            .map_err(MappedStoreProbeError)
    }

    fn namespace_body() -> EntityBodyValue {
        EntityBodyValue::Namespace(NamespaceBody {
            parent: None,
            members: EntityIdSet::from_unsorted(vec![]).unwrap(),
        })
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

    #[test]
    fn exclusive_recovery_ownership_blocks_shared_operations() {
        let Cross05CompositeFixture {
            fixture,
            commit_expected_parent,
            commit_candidate_bytes,
            commit_principal_id,
            branch_name,
            branch_target,
            gc_snapshot,
            gc_verifier,
        } = cross05_composite_fixture();
        let canonical_root = ::std::fs::canonicalize(fixture.path()).unwrap();
        let transaction_repository = ::sley_txn::TransactionRepository::new(canonical_root.clone());
        let branch_repository = super::BranchRepository::new(canonical_root.clone());
        let object_store = ::sley_store::ObjectStore::new(canonical_root.clone());
        let maintenance = transaction_repository
            .acquire_exclusive_maintenance()
            .unwrap();
        let transaction_recovery_result =
            transaction_repository.recover_with_maintenance(&maintenance);
        let ref_recovery_result = branch_repository.recover_refs_with_maintenance(&maintenance);
        let gc_recovery_result = crate::gc::recover_gc_witness(&object_store, &maintenance);
        let exclusive_observed = maintenance.is_exclusive() && maintenance.covers(&canonical_root);
        let (cross05_blocked_tx, cross05_blocked_rx) =
            ::std::sync::mpsc::sync_channel::<Cross05Blocked>(0);
        let (cross05_completed_tx, cross05_completed_rx) =
            ::std::sync::mpsc::sync_channel::<Cross05Completed>(0);
        let commit_blocked_tx = cross05_blocked_tx.clone();
        let commit_completed_tx = cross05_completed_tx.clone();
        let commit_root = canonical_root.clone();
        let commit_repository = transaction_repository.clone();
        let commit_handle = ::std::thread::spawn(move || {
            let maintenance_probe = ::std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(commit_root.join("locks").join("maintenance.lock"))
                .unwrap();
            let blocked = ::core::matches!(
                ::std::fs::File::try_lock_shared(&maintenance_probe),
                Err(::std::fs::TryLockError::WouldBlock)
            );
            ::core::assert!(
                commit_blocked_tx
                    .send(Cross05Blocked::Commit(blocked))
                    .is_ok()
            );
            let commit_input = ::sley_txn::CommitInput::new(
                commit_expected_parent,
                &commit_candidate_bytes,
                commit_principal_id,
                &[],
                NOW,
                ::sley_policy::CandidateValidationLimits::full_v1(),
            );
            let operation_result = commit_repository.commit(commit_input);
            ::core::assert!(
                commit_completed_tx
                    .send(Cross05Completed::Commit(operation_result.is_ok()))
                    .is_ok()
            );
        });
        let ref_blocked_tx = cross05_blocked_tx.clone();
        let ref_completed_tx = cross05_completed_tx.clone();
        let ref_root = canonical_root.clone();
        let ref_repository = branch_repository.clone();
        let ref_handle = ::std::thread::spawn(move || {
            let maintenance_probe = ::std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(ref_root.join("locks").join("maintenance.lock"))
                .unwrap();
            let blocked = ::core::matches!(
                ::std::fs::File::try_lock_shared(&maintenance_probe),
                Err(::std::fs::TryLockError::WouldBlock)
            );
            ::core::assert!(ref_blocked_tx.send(Cross05Blocked::Ref(blocked)).is_ok());
            let operation_result = ref_repository.create_branch(branch_name, branch_target);
            ::core::assert!(
                ref_completed_tx
                    .send(Cross05Completed::Ref(operation_result.is_ok()))
                    .is_ok()
            );
        });
        let gc_blocked_tx = cross05_blocked_tx.clone();
        let gc_completed_tx = cross05_completed_tx.clone();
        let gc_root = canonical_root.clone();
        let gc_store = object_store.clone();
        let gc_handle = ::std::thread::spawn(move || {
            let maintenance_probe = ::std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(gc_root.join("locks").join("maintenance.lock"))
                .unwrap();
            let blocked = ::core::matches!(
                ::std::fs::File::try_lock(&maintenance_probe),
                Err(::std::fs::TryLockError::WouldBlock)
            );
            ::core::assert!(gc_blocked_tx.send(Cross05Blocked::Gc(blocked)).is_ok());
            let operation_result = crate::gc::acquire_exclusive_gc(&gc_store).and_then(|guard| {
                crate::gc::gc_collect(&gc_store, &gc_snapshot, &gc_verifier, &guard)
            });
            ::core::assert!(
                gc_completed_tx
                    .send(Cross05Completed::Gc(operation_result.is_ok()))
                    .is_ok()
            );
        });
        let cross05_blocked = [
            cross05_blocked_rx.recv().unwrap(),
            cross05_blocked_rx.recv().unwrap(),
            cross05_blocked_rx.recv().unwrap(),
        ];
        let commit_blocked = cross05_blocked
            .iter()
            .any(|event| ::core::matches!(event, Cross05Blocked::Commit(true)));
        let ref_blocked = cross05_blocked
            .iter()
            .any(|event| ::core::matches!(event, Cross05Blocked::Ref(true)));
        let gc_blocked = cross05_blocked
            .iter()
            .any(|event| ::core::matches!(event, Cross05Blocked::Gc(true)));
        let gc_witness_absent_while_blocked = ::core::matches!(
            ::std::fs::symlink_metadata(canonical_root.join("locks").join("gc.lock")),
            Err(error) if error.kind() == ::std::io::ErrorKind::NotFound
        );
        ::core::assert!(exclusive_observed);
        ::core::assert!(commit_blocked);
        ::core::assert!(ref_blocked);
        ::core::assert!(gc_blocked);
        ::core::assert!(gc_witness_absent_while_blocked);
        drop(maintenance);
        let cross05_completed = [
            cross05_completed_rx.recv().unwrap(),
            cross05_completed_rx.recv().unwrap(),
            cross05_completed_rx.recv().unwrap(),
        ];
        let commit_resumed = cross05_completed
            .iter()
            .any(|event| ::core::matches!(event, Cross05Completed::Commit(true)));
        let ref_resumed = cross05_completed
            .iter()
            .any(|event| ::core::matches!(event, Cross05Completed::Ref(true)));
        let gc_resumed = cross05_completed
            .iter()
            .any(|event| ::core::matches!(event, Cross05Completed::Gc(true)));
        let recovery_completed = transaction_recovery_result.is_ok()
            && ref_recovery_result.is_ok()
            && gc_recovery_result.is_ok();
        commit_handle.join().unwrap();
        ref_handle.join().unwrap();
        gc_handle.join().unwrap();
        ::core::assert!(commit_resumed);
        ::core::assert!(ref_resumed);
        ::core::assert!(gc_resumed);
        ::core::assert!(recovery_completed);
    }

    #[test]
    fn ref_no_argument_recovery_holds_exclusive_maintenance() {
        let fixture = cross05_recovery_fixture();
        let canonical_root = ::std::fs::canonicalize(fixture.path()).unwrap();
        let owner_repository = super::BranchRepository::new(canonical_root.clone());
        let (cross05_owner_tx, cross05_owner_rx) =
            ::std::sync::mpsc::sync_channel::<Cross05OwnerSignal>(0);
        let (cross05_release_tx, cross05_release_rx) =
            ::std::sync::mpsc::sync_channel::<Cross05OwnerSignal>(0);
        let owner_handle = ::std::thread::spawn(move || {
            install_ref_recovery_hold(
                owner_repository.root(),
                cross05_owner_tx,
                cross05_release_rx,
            );
            owner_repository.recover_refs()
        });
        let exclusive_observed = ::core::matches!(
            cross05_owner_rx.recv().unwrap(),
            Cross05OwnerSignal::OwnerHeld
        );
        let commit_probe = ::std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(canonical_root.join("locks").join("maintenance.lock"))
            .unwrap();
        let ref_probe = ::std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(canonical_root.join("locks").join("maintenance.lock"))
            .unwrap();
        let gc_probe = ::std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(canonical_root.join("locks").join("maintenance.lock"))
            .unwrap();
        let commit_blocked = ::core::matches!(
            ::std::fs::File::try_lock_shared(&commit_probe),
            Err(::std::fs::TryLockError::WouldBlock)
        );
        let ref_blocked = ::core::matches!(
            ::std::fs::File::try_lock_shared(&ref_probe),
            Err(::std::fs::TryLockError::WouldBlock)
        );
        let gc_blocked = ::core::matches!(
            ::std::fs::File::try_lock(&gc_probe),
            Err(::std::fs::TryLockError::WouldBlock)
        );
        let gc_witness_absent_while_blocked = ::core::matches!(
            ::std::fs::symlink_metadata(canonical_root.join("locks").join("gc.lock")),
            Err(error) if error.kind() == ::std::io::ErrorKind::NotFound
        );
        ::core::assert!(exclusive_observed);
        ::core::assert!(commit_blocked);
        ::core::assert!(ref_blocked);
        ::core::assert!(gc_blocked);
        ::core::assert!(gc_witness_absent_while_blocked);
        ::core::assert!(
            cross05_release_tx
                .send(Cross05OwnerSignal::ReleaseOwner)
                .is_ok()
        );
        let recovery_result = owner_handle.join().unwrap();
        let recovery_completed = recovery_result.is_ok();
        let commit_resumed = ::std::fs::File::try_lock_shared(&commit_probe).is_ok();
        ::std::fs::File::unlock(&commit_probe).unwrap();
        let ref_resumed = ::std::fs::File::try_lock_shared(&ref_probe).is_ok();
        ::std::fs::File::unlock(&ref_probe).unwrap();
        let gc_resumed = ::std::fs::File::try_lock(&gc_probe).is_ok();
        ::std::fs::File::unlock(&gc_probe).unwrap();
        ::core::assert!(commit_resumed);
        ::core::assert!(ref_resumed);
        ::core::assert!(gc_resumed);
        ::core::assert!(recovery_completed);
    }

    fn synthetic_origin(name: &str) -> BranchRecord {
        BranchRecord {
            format_version: RECORD_VERSION,
            branch_name: BranchName::parse(name).unwrap(),
            workspace_id: fixed(1, WorkspaceId::from_bytes),
            origin_transaction_id: fixed(2, TransactionId::from_bytes),
            origin_state_root: fixed(3, StateRoot::from_bytes),
            schema_epoch_id: fixed(4, SchemaEpochId::from_bytes),
            policy_root_id: fixed(5, PolicyRootId::from_bytes),
            dependency_roots: vec![
                fixed(6, StateRoot::from_bytes),
                fixed(7, StateRoot::from_bytes),
            ],
        }
    }

    fn synthetic_ref(origin: &ImportedBranchRecord) -> BranchRefRecord {
        BranchRefRecord {
            format_version: RECORD_VERSION,
            branch_name: origin.record.branch_name.clone(),
            branch_record_digest: origin.digest,
            workspace_id: origin.record.workspace_id,
            head_transaction_id: origin.record.origin_transaction_id,
            head_state_root: origin.record.origin_state_root,
            schema_epoch_id: origin.record.schema_epoch_id,
            policy_root_id: origin.record.policy_root_id,
            dependency_roots: origin.record.dependency_roots.clone(),
        }
    }

    fn rehash(stored: &mut [u8], domain: &[u8]) {
        let preimage_len = stored.len() - DIGEST_LEN;
        let replacement = digest(domain, &stored[..preimage_len]);
        stored[preimage_len..].copy_from_slice(&replacement);
    }

    fn payload_offset(stored: &[u8]) -> usize {
        let mut cursor = ScbValueCursor::new(stored).unwrap();
        cursor.read_fixed_bytes::<8>().unwrap();
        cursor.read_uvar(64).unwrap();
        let payload = cursor.read_sized_payload().unwrap();
        payload.as_ptr() as usize - stored.as_ptr() as usize
    }

    #[test]
    fn error_codes_and_update_statuses_are_closed_and_contiguous() {
        let expected_symbols = [
            "REF_FORMAT_VERSION",
            "REF_NAME_INVALID",
            "REF_NAME_RESERVED",
            "REF_DIGEST_MISMATCH",
            "REF_FIELD_SHAPE",
            "REF_BRANCH_BINDING_MISMATCH",
            "REF_NOT_FOUND",
            "REF_ALREADY_EXISTS",
            "REF_NAME_COLLISION",
            "REF_TARGET_MISMATCH",
            "REF_NAMED_CAS_STALE",
            "BRANCH_RECORD_FORMAT_VERSION",
            "BRANCH_RECORD_DIGEST_MISMATCH",
            "BRANCH_RECORD_FIELD_SHAPE",
            "BRANCH_ORIGIN_MISMATCH",
            "BRANCH_NOT_FAST_FORWARD",
            "BRANCH_ANCESTRY_CYCLE",
            "BRANCH_RESOURCE_LIMIT",
            "RECOVERY_NAMED_REF_INCOMPLETE",
            "REF_IO",
            "REF_INTERNAL_INVARIANT",
        ];
        for (offset, (code, symbol)) in BranchErrorCode::ALL
            .into_iter()
            .zip(expected_symbols)
            .enumerate()
        {
            assert_eq!(code.numeric(), 50_000 + u32::try_from(offset).unwrap());
            assert_eq!(code.symbol(), symbol);
        }
        for (tag, status, symbol) in [
            (1, BranchUpdateStatus::Created, "CREATED"),
            (2, BranchUpdateStatus::Advanced, "ADVANCED"),
            (3, BranchUpdateStatus::Present, "PRESENT"),
        ] {
            assert_eq!(BranchUpdateStatus::from_tag(tag), Some(status));
            assert_eq!(status.tag(), tag);
            assert_eq!(status.symbol(), symbol);
        }
        assert_eq!(BranchUpdateStatus::from_tag(0), None);
        assert_eq!(BranchUpdateStatus::from_tag(4), None);
        assert!(validate_visible_capacity(MAX_BRANCHES - 1).is_ok());
        assert_eq!(
            validate_visible_capacity(MAX_BRANCHES).unwrap_err().code(),
            "BRANCH_RESOURCE_LIMIT"
        );
        assert!(validate_origin_capacity(MAX_BRANCH_ORIGINS - 1, MAX_BRANCH_ORIGINS).is_ok());
        assert_eq!(
            validate_origin_capacity(MAX_BRANCH_ORIGINS, MAX_BRANCH_ORIGINS)
                .unwrap_err()
                .code(),
            "BRANCH_RESOURCE_LIMIT"
        );
    }

    #[test]
    fn branch_name_grammar_is_exact_and_non_normalizing() {
        let component = format!("a{}z", "x".repeat(61));
        let maximum = [component.as_str(); 4].join("/");
        assert_eq!(maximum.len(), MAX_BRANCH_NAME_BYTES);
        for valid in [
            b"a".as_slice(),
            b"a1".as_slice(),
            b"feature/x-1".as_slice(),
            b"release/2.0_alpha".as_slice(),
            maximum.as_bytes(),
        ] {
            let parsed = BranchName::parse(valid).unwrap();
            assert_eq!(parsed.as_bytes(), valid);
        }
        let invalid = [
            Vec::new(),
            b"Main".to_vec(),
            "café".as_bytes().to_vec(),
            b"/a".to_vec(),
            b"a/".to_vec(),
            b"a//b".to_vec(),
            b".".to_vec(),
            b"..".to_vec(),
            b"a\\b".to_vec(),
            b"a:b".to_vec(),
            b"a.lock".to_vec(),
            b"-a".to_vec(),
            b"a_".to_vec(),
            format!("a{}z", "x".repeat(62)).into_bytes(),
            b"a/a/a/a/a/a/a/a/a".to_vec(),
            vec![b'a'; MAX_BRANCH_NAME_BYTES + 1],
        ];
        for name in invalid {
            assert_eq!(
                BranchName::parse(name).unwrap_err().code(),
                "REF_NAME_INVALID"
            );
        }
        for reserved in [b"refs".as_slice(), b"ok/transactions".as_slice()] {
            assert_eq!(
                BranchName::parse(reserved).unwrap_err().code(),
                "REF_NAME_RESERVED"
            );
        }
        for reserved in RESERVED_COMPONENTS {
            assert_eq!(
                BranchName::parse(reserved).unwrap_err().code(),
                "REF_NAME_RESERVED"
            );
            let nested = [b"ok/".as_slice(), *reserved, b"/x".as_slice()].concat();
            assert_eq!(
                BranchName::parse(nested).unwrap_err().code(),
                "REF_NAME_RESERVED"
            );
        }
        for byte in 0_u8..=u8::MAX {
            let candidate = [b'a', byte, b'b'];
            let admitted = byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-' | b'/');
            assert_eq!(
                BranchName::parse(candidate).is_ok(),
                admitted,
                "unexpected byte classification for 0x{byte:02x}"
            );
        }
        assert!(BranchName::parse("a/a/a/a/a/a/a/a").is_ok());
        assert!(BranchName::parse([b'a'; MAX_BRANCH_COMPONENT_BYTES]).is_ok());
        assert_ne!(
            BranchName::parse("feature/a").unwrap().path_key(),
            BranchName::parse("feature/b").unwrap().path_key()
        );
        assert_eq!(
            BranchName::parse("feature/a").unwrap().path_key(),
            BranchName::parse("feature/a").unwrap().path_key()
        );
        assert_eq!(
            hex_digest(&BranchName::parse("feature/a").unwrap().path_key()),
            "c43b584a5dadee3c14d8d4c37751bf66f06324d4b54380ebbe68474d31c8dcdd"
        );
    }

    #[test]
    fn branch_and_ref_codecs_round_trip_and_reject_rehashed_shape_changes() {
        let origin_record = synthetic_origin("feature/a");
        let origin = build_branch_record(&origin_record).unwrap();
        assert_eq!(import_branch_record(&origin.stored_bytes).unwrap(), origin);
        let reference = build_branch_ref(&synthetic_ref(&origin)).unwrap();
        assert_eq!(
            import_branch_ref(&reference.stored_bytes).unwrap(),
            reference
        );

        let mut corrupt_origin = origin.stored_bytes.clone();
        *corrupt_origin.last_mut().unwrap() ^= 1;
        assert_eq!(
            import_branch_record(&corrupt_origin).unwrap_err().code(),
            "BRANCH_RECORD_DIGEST_MISMATCH"
        );
        let mut corrupt_ref = reference.stored_bytes.clone();
        *corrupt_ref.last_mut().unwrap() ^= 1;
        assert_eq!(
            import_branch_ref(&corrupt_ref).unwrap_err().code(),
            "REF_DIGEST_MISMATCH"
        );

        let mut bad_origin_version = origin.stored_bytes.clone();
        let origin_payload = payload_offset(&bad_origin_version);
        bad_origin_version[origin_payload + 3] = 2;
        rehash(&mut bad_origin_version, BRANCH_DIGEST_DOMAIN);
        assert_eq!(
            import_branch_record(&bad_origin_version)
                .unwrap_err()
                .code(),
            "BRANCH_RECORD_FORMAT_VERSION"
        );
        let mut bad_ref_shape = reference.stored_bytes.clone();
        let ref_payload = payload_offset(&bad_ref_shape);
        bad_ref_shape[ref_payload] = 8;
        rehash(&mut bad_ref_shape, REF_DIGEST_DOMAIN);
        assert_eq!(
            import_branch_ref(&bad_ref_shape).unwrap_err().code(),
            "REF_FIELD_SHAPE"
        );

        let mut unsorted = origin_record;
        unsorted.dependency_roots.reverse();
        assert_eq!(
            build_branch_record(&unsorted).unwrap_err().code(),
            "BRANCH_RECORD_FIELD_SHAPE"
        );
    }

    #[test]
    fn dependency_codec_uses_the_scb1_collection_ceiling_not_ancestry_limit() {
        let dependency_roots = (0_u32..65_537)
            .map(|ordinal| {
                let mut bytes = [0_u8; DIGEST_LEN];
                bytes[..4].copy_from_slice(&ordinal.to_be_bytes());
                StateRoot::from_bytes(bytes)
            })
            .collect::<Vec<_>>();
        let mut record = synthetic_origin("codec/large");
        record.dependency_roots = dependency_roots;
        let origin = build_branch_record(&record).unwrap();
        assert_eq!(import_branch_record(&origin.stored_bytes).unwrap(), origin);
        let reference = build_branch_ref(&synthetic_ref(&origin)).unwrap();
        assert_eq!(
            import_branch_ref(&reference.stored_bytes).unwrap(),
            reference
        );

        let impossible_count = encode_uvar(MAX_COLLECTION_ELEMENTS + 1);
        assert_eq!(
            decode_dependencies(&impossible_count, BranchErrorCode::BranchRecordFieldShape,)
                .unwrap_err()
                .code(),
            "BRANCH_RESOURCE_LIMIT"
        );
    }

    #[test]
    fn every_branch_and_ref_semantic_field_perturbation_changes_its_digest() {
        let base_origin_record = synthetic_origin("feature/a");
        let base_origin = build_branch_record(&base_origin_record).unwrap();
        let mut origin_variants = Vec::new();
        let mut variant = base_origin_record.clone();
        variant.branch_name = BranchName::parse("feature/b").unwrap();
        origin_variants.push(variant);
        let mut variant = base_origin_record.clone();
        variant.workspace_id = fixed(11, WorkspaceId::from_bytes);
        origin_variants.push(variant);
        let mut variant = base_origin_record.clone();
        variant.origin_transaction_id = fixed(12, TransactionId::from_bytes);
        origin_variants.push(variant);
        let mut variant = base_origin_record.clone();
        variant.origin_state_root = fixed(13, StateRoot::from_bytes);
        origin_variants.push(variant);
        let mut variant = base_origin_record.clone();
        variant.schema_epoch_id = fixed(14, SchemaEpochId::from_bytes);
        origin_variants.push(variant);
        let mut variant = base_origin_record.clone();
        variant.policy_root_id = fixed(15, PolicyRootId::from_bytes);
        origin_variants.push(variant);
        let mut variant = base_origin_record.clone();
        variant
            .dependency_roots
            .push(fixed(8, StateRoot::from_bytes));
        origin_variants.push(variant);
        for variant in origin_variants {
            let imported = build_branch_record(&variant).unwrap();
            assert_ne!(imported.digest, base_origin.digest);
            assert_eq!(
                import_branch_record(&imported.stored_bytes).unwrap(),
                imported
            );
        }

        let base_ref_record = synthetic_ref(&base_origin);
        let base_ref = build_branch_ref(&base_ref_record).unwrap();
        let mut ref_variants = Vec::new();
        let mut variant = base_ref_record.clone();
        variant.branch_name = BranchName::parse("feature/b").unwrap();
        ref_variants.push(variant);
        let mut variant = base_ref_record.clone();
        variant.branch_record_digest = BranchRecordDigest::from_bytes([16; DIGEST_LEN]);
        ref_variants.push(variant);
        let mut variant = base_ref_record.clone();
        variant.workspace_id = fixed(17, WorkspaceId::from_bytes);
        ref_variants.push(variant);
        let mut variant = base_ref_record.clone();
        variant.head_transaction_id = fixed(18, TransactionId::from_bytes);
        ref_variants.push(variant);
        let mut variant = base_ref_record.clone();
        variant.head_state_root = fixed(19, StateRoot::from_bytes);
        ref_variants.push(variant);
        let mut variant = base_ref_record.clone();
        variant.schema_epoch_id = fixed(20, SchemaEpochId::from_bytes);
        ref_variants.push(variant);
        let mut variant = base_ref_record.clone();
        variant.policy_root_id = fixed(21, PolicyRootId::from_bytes);
        ref_variants.push(variant);
        let mut variant = base_ref_record;
        variant
            .dependency_roots
            .push(fixed(8, StateRoot::from_bytes));
        ref_variants.push(variant);
        for variant in ref_variants {
            let imported = build_branch_ref(&variant).unwrap();
            assert_ne!(imported.digest, base_ref.digest);
            assert_eq!(import_branch_ref(&imported.stored_bytes).unwrap(), imported);
        }
    }

    #[test]
    fn create_resolve_list_and_exact_retry_are_durable() {
        let fixture = Fixture::new("create-resolve");
        assert_eq!(
            fixture
                .branches
                .create_branch("feature/a", fixture.genesis_transaction_id)
                .unwrap(),
            BranchUpdateStatus::Created
        );
        assert_eq!(
            fixture
                .branches
                .create_branch("feature/a", fixture.genesis_transaction_id)
                .unwrap(),
            BranchUpdateStatus::Present
        );
        let resolved = fixture.branches.resolve_branch("feature/a").unwrap();
        assert_eq!(
            resolved.reference.record.head_transaction_id,
            fixture.genesis_transaction_id
        );
        assert_eq!(resolved.origin.record.branch_name.as_str(), "feature/a");
        let listed = fixture.branches.list_branches(1).unwrap();
        assert_eq!(listed, vec![resolved]);
        assert_eq!(
            fixture.branches.list_branches(0).unwrap_err().code(),
            "BRANCH_RESOURCE_LIMIT"
        );
        let path = fixture
            .branches
            .branch_path(&BranchName::parse("feature/a").unwrap());
        assert!(!path.to_string_lossy().contains("feature"));
        assert_eq!(
            path.extension().and_then(|value| value.to_str()),
            Some("scb1")
        );
    }

    #[test]
    fn directory_creation_retry_redurabilizes_layout_and_fanout_before_branch_success() {
        let fixture = Fixture::new("directory-retry-durability");
        let layout_name = BranchName::parse("layout-retry").unwrap();
        let layout_component = fixture.branches.root().join("branches").join("v1");
        assert_eq!(
            fixture
                .branches
                .ensure_layout_with_native_ref_durability_cut(
                    NativeRefDurabilityCut::Rlay01BranchesV1CreateBeforeBranchesSync,
                )
                .unwrap_err()
                .code(),
            "REF_IO"
        );
        assert!(layout_component.is_dir());
        assert!(!fixture.branches.branch_path(&layout_name).exists());
        assert!(!fixture.branches.ref_path(&layout_name).exists());
        fixture.branches.ensure_layout().unwrap();
        assert_eq!(
            fixture
                .branches
                .create_branch("layout-retry", fixture.genesis_transaction_id)
                .unwrap(),
            BranchUpdateStatus::Created
        );

        let fanout_name = BranchName::parse("fanout-retry").unwrap();
        let fanout_hex = hex_digest(&fanout_name.path_key());
        let first_fanout = fixture.branches.branches_dir().join(&fanout_hex[0..2]);
        let second_fanout = first_fanout.join(&fanout_hex[2..4]);
        assert_eq!(
            fixture
                .branches
                .create_branch_with_native_ref_durability_cut(
                    "fanout-retry",
                    fixture.genesis_transaction_id,
                    NativeRefDurabilityCut::Rlay02FirstOriginFanoutCreateBeforeParentSync,
                )
                .unwrap_err()
                .code(),
            "REF_IO"
        );
        assert!(first_fanout.is_dir());
        assert!(!fixture.branches.branch_path(&fanout_name).exists());
        assert!(!fixture.branches.ref_path(&fanout_name).exists());
        assert_eq!(
            fixture
                .branches
                .create_branch_with_native_ref_durability_cut(
                    "fanout-retry",
                    fixture.genesis_transaction_id,
                    NativeRefDurabilityCut::Rlay03SecondOriginFanoutCreateBeforeParentSync,
                )
                .unwrap_err()
                .code(),
            "REF_IO"
        );
        assert!(second_fanout.is_dir());
        assert!(!fixture.branches.branch_path(&fanout_name).exists());
        assert!(!fixture.branches.ref_path(&fanout_name).exists());
        assert_eq!(
            fixture
                .branches
                .create_branch("fanout-retry", fixture.genesis_transaction_id)
                .unwrap(),
            BranchUpdateStatus::Created
        );
        assert_eq!(
            fixture
                .branches
                .resolve_branch("fanout-retry")
                .unwrap()
                .reference
                .record
                .head_transaction_id,
            fixture.genesis_transaction_id
        );
    }

    #[test]
    fn interrupted_install_retries_redurabilize_visible_records() {
        let fixture = Fixture::new("retry-durability");
        fixture.branches.ensure_layout().unwrap();

        let origin_fault_name = BranchName::parse("origin-fault").unwrap();
        let origin_fault_path = fixture
            .branches
            .ensure_branch_path(&origin_fault_name)
            .unwrap();
        assert_eq!(
            fixture
                .branches
                .create_branch_with_native_ref_durability_cut(
                    "origin-fault",
                    fixture.genesis_transaction_id,
                    NativeRefDurabilityCut::Ref03OriginLinkBeforeFirstLeafSync,
                )
                .unwrap_err()
                .code(),
            "REF_IO"
        );
        assert!(origin_fault_path.is_file());
        assert!(!fixture.branches.ref_path(&origin_fault_name).exists());
        assert_eq!(
            fixture
                .branches
                .create_branch("origin-fault", fixture.genesis_transaction_id)
                .unwrap(),
            BranchUpdateStatus::Created
        );

        let ref_fault_name = BranchName::parse("ref-fault").unwrap();
        let ref_fault_path = fixture.branches.ensure_ref_path(&ref_fault_name).unwrap();
        assert_eq!(
            fixture
                .branches
                .create_branch_with_native_ref_durability_cut(
                    "ref-fault",
                    fixture.genesis_transaction_id,
                    NativeRefDurabilityCut::Ref08InitialRefLinkBeforeFirstLeafSync,
                )
                .unwrap_err()
                .code(),
            "REF_IO"
        );
        assert!(ref_fault_path.is_file());
        assert_eq!(
            fixture
                .branches
                .create_branch("ref-fault", fixture.genesis_transaction_id)
                .unwrap(),
            BranchUpdateStatus::Present
        );
    }

    #[test]
    fn interrupted_advance_retry_redurabilizes_the_renamed_ref() {
        let fixture = Fixture::new("advance-retry-durability");
        let child = fixture.commit_child(30);
        fixture
            .branches
            .create_branch("advance-fault", fixture.genesis_transaction_id)
            .unwrap();
        assert_eq!(
            fixture
                .branches
                .advance_branch_with_native_ref_durability_cut(
                    "advance-fault",
                    fixture.genesis_transaction_id,
                    child,
                    NativeRefDurabilityCut::Ref13AdvanceRefRenameBeforeLeafSync,
                )
                .unwrap_err()
                .code(),
            "REF_IO"
        );
        assert_eq!(
            fixture
                .branches
                .advance_branch("advance-fault", fixed(99, TransactionId::from_bytes), child,)
                .unwrap(),
            BranchUpdateStatus::Present
        );
    }

    #[test]
    fn orphan_retry_ignores_exact_owned_ref_stage_during_capacity_scan() {
        let fixture = Fixture::new("owned-stage-capacity");
        fixture.branches.ensure_layout().unwrap();
        let name = BranchName::parse("stage-retry").unwrap();
        let revision = fixture
            .transactions
            .verified_revision(fixture.genesis_transaction_id)
            .unwrap();
        let origin = build_branch_record(&origin_record(&name, &revision)).unwrap();
        let origin_path = fixture.branches.ensure_branch_path(&name).unwrap();
        persist_no_overwrite(
            &origin_path,
            &origin.stored_bytes,
            BRANCH_STAGE_PREFIX,
            BranchErrorCode::BranchOriginMismatch,
            |_| Ok(()),
        )
        .unwrap();
        let ref_path = fixture.branches.ensure_ref_path(&name).unwrap();
        let (stage_path, mut stage) =
            reserve_stage(ref_path.parent().unwrap(), REF_STAGE_PREFIX).unwrap();
        stage.write_all(b"interrupted").unwrap();
        stage.sync_all().unwrap();
        drop(stage);

        assert_eq!(
            fixture
                .branches
                .create_branch("stage-retry", fixture.genesis_transaction_id)
                .unwrap(),
            BranchUpdateStatus::Created
        );
        assert!(stage_path.is_file());
        assert_eq!(
            fixture.branches.recover_refs().unwrap().removed_ref_stages,
            1
        );
    }

    #[test]
    fn origin_capacity_boundary_rejects_before_origin_or_ref_write() {
        let fixture = Fixture::new("origin-capacity");
        fixture
            .branches
            .create_branch("existing", fixture.genesis_transaction_id)
            .unwrap();
        let existing = BranchName::parse("existing").unwrap();
        fs::remove_file(fixture.branches.ref_path(&existing)).unwrap();

        let blocked = BranchName::parse("blocked").unwrap();
        let branch_path = fixture.branches.branch_path(&blocked);
        let ref_path = fixture.branches.ref_path(&blocked);
        let maintenance = fixture.branches.prepare_operation().unwrap();
        let lock = fixture.branches.acquire_refs_lock().unwrap();
        assert_eq!(
            fixture
                .branches
                .create_fresh_with_origin_limit(
                    &maintenance,
                    &branch_path,
                    &ref_path,
                    &blocked,
                    fixture.genesis_transaction_id,
                    1,
                )
                .unwrap_err()
                .code(),
            "BRANCH_RESOURCE_LIMIT"
        );
        assert!(!branch_path.exists());
        assert!(!ref_path.exists());
        drop(lock);
        drop(maintenance);
        assert_eq!(
            fixture
                .branches
                .recover_refs()
                .unwrap()
                .orphan_origins
                .len(),
            1
        );
    }

    #[test]
    fn create_conflicts_and_interrupted_origin_precedence_are_exact() {
        let fixture = Fixture::new("create-conflicts");
        fixture
            .branches
            .create_branch("mainline", fixture.genesis_transaction_id)
            .unwrap();
        let child = fixture.commit_child(30);
        assert_eq!(
            fixture
                .branches
                .create_branch("mainline", child)
                .unwrap_err()
                .code(),
            "BRANCH_ORIGIN_MISMATCH"
        );

        let name = BranchName::parse("mainline").unwrap();
        fs::remove_file(fixture.branches.ref_path(&name)).unwrap();
        assert_eq!(
            fixture
                .branches
                .create_branch("mainline", fixture.genesis_transaction_id)
                .unwrap(),
            BranchUpdateStatus::Created
        );
        fs::remove_file(fixture.branches.branch_path(&name)).unwrap();
        assert_eq!(
            fixture
                .branches
                .resolve_branch("mainline")
                .unwrap_err()
                .code(),
            "RECOVERY_NAMED_REF_INCOMPLETE"
        );
        assert_eq!(
            fixture
                .branches
                .create_branch("mainline", fixed(98, TransactionId::from_bytes))
                .unwrap_err()
                .code(),
            "RECOVERY_NAMED_REF_INCOMPLETE"
        );
    }

    #[test]
    fn create_combined_conflicts_follow_the_frozen_precedence_table() {
        let different_origin = Fixture::new("precedence-different-origin");
        different_origin
            .branches
            .create_branch("mainline", different_origin.genesis_transaction_id)
            .unwrap();
        assert_eq!(
            different_origin
                .branches
                .create_branch("mainline", fixed(90, TransactionId::from_bytes))
                .unwrap_err()
                .code(),
            "BRANCH_ORIGIN_MISMATCH"
        );

        let corrupt_origin = Fixture::new("precedence-corrupt-origin");
        corrupt_origin
            .branches
            .create_branch("mainline", corrupt_origin.genesis_transaction_id)
            .unwrap();
        let name = BranchName::parse("mainline").unwrap();
        let origin_path = corrupt_origin.branches.branch_path(&name);
        let mut bytes = fs::read(&origin_path).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        fs::write(&origin_path, bytes).unwrap();
        assert_eq!(
            corrupt_origin
                .branches
                .create_branch("mainline", fixed(91, TransactionId::from_bytes))
                .unwrap_err()
                .code(),
            "BRANCH_RECORD_DIGEST_MISMATCH"
        );

        for (label, mutate, expected) in [
            ("version", 0_u8, "BRANCH_RECORD_FORMAT_VERSION"),
            ("shape", 1_u8, "BRANCH_RECORD_FIELD_SHAPE"),
        ] {
            let fixture = Fixture::new(&format!("precedence-origin-{label}"));
            fixture
                .branches
                .create_branch("mainline", fixture.genesis_transaction_id)
                .unwrap();
            let path = fixture.branches.branch_path(&name);
            let mut bytes = fs::read(&path).unwrap();
            let payload = payload_offset(&bytes);
            if mutate == 0 {
                bytes[payload + 3] = 2;
            } else {
                bytes[payload] = 7;
            }
            rehash(&mut bytes, BRANCH_DIGEST_DOMAIN);
            fs::write(path, bytes).unwrap();
            assert_eq!(
                fixture
                    .branches
                    .create_branch("mainline", fixed(93, TransactionId::from_bytes))
                    .unwrap_err()
                    .code(),
                expected
            );
        }
    }

    #[test]
    fn create_binding_advanced_and_path_conflicts_have_exact_precedence() {
        let name = BranchName::parse("mainline").unwrap();
        let bad_binding = Fixture::new("precedence-binding");
        bad_binding
            .branches
            .create_branch("mainline", bad_binding.genesis_transaction_id)
            .unwrap();
        let ref_path = bad_binding.branches.ref_path(&name);
        let mut reference = import_branch_ref(&fs::read(&ref_path).unwrap()).unwrap();
        reference.record.branch_record_digest = BranchRecordDigest::from_bytes([77; DIGEST_LEN]);
        fs::write(
            &ref_path,
            build_branch_ref(&reference.record).unwrap().stored_bytes,
        )
        .unwrap();
        assert_eq!(
            bad_binding
                .branches
                .create_branch("mainline", fixed(92, TransactionId::from_bytes))
                .unwrap_err()
                .code(),
            "REF_BRANCH_BINDING_MISMATCH"
        );

        let advanced = Fixture::new("precedence-advanced");
        advanced
            .branches
            .create_branch("mainline", advanced.genesis_transaction_id)
            .unwrap();
        let child = advanced.commit_child(30);
        advanced
            .branches
            .advance_branch("mainline", advanced.genesis_transaction_id, child)
            .unwrap();
        assert_eq!(
            advanced
                .branches
                .create_branch("mainline", advanced.genesis_transaction_id)
                .unwrap_err()
                .code(),
            "REF_ALREADY_EXISTS"
        );

        let wrong_key = Fixture::new("precedence-name-key");
        for branch in ["alpha", "beta"] {
            wrong_key
                .branches
                .create_branch(branch, wrong_key.genesis_transaction_id)
                .unwrap();
        }
        let alpha = BranchName::parse("alpha").unwrap();
        let beta = BranchName::parse("beta").unwrap();
        fs::write(
            wrong_key.branches.branch_path(&beta),
            fs::read(wrong_key.branches.branch_path(&alpha)).unwrap(),
        )
        .unwrap();
        assert_eq!(
            wrong_key
                .branches
                .create_branch("beta", fixed(94, TransactionId::from_bytes))
                .unwrap_err()
                .code(),
            "REF_NAME_COLLISION"
        );
    }

    #[test]
    fn advance_preserves_present_stale_and_non_fast_forward_distinctions() {
        let fixture = Fixture::new("advance");
        fixture
            .branches
            .create_branch("mainline", fixture.genesis_transaction_id)
            .unwrap();
        let child = fixture.commit_child(30);
        assert_eq!(
            fixture
                .branches
                .advance_branch("mainline", fixture.genesis_transaction_id, child)
                .unwrap(),
            BranchUpdateStatus::Advanced
        );
        assert_eq!(
            fixture
                .branches
                .advance_branch("mainline", fixed(88, TransactionId::from_bytes), child,)
                .unwrap(),
            BranchUpdateStatus::Present
        );
        assert_eq!(
            fixture
                .branches
                .advance_branch(
                    "mainline",
                    fixture.genesis_transaction_id,
                    fixed(89, TransactionId::from_bytes),
                )
                .unwrap_err()
                .code(),
            "REF_NAMED_CAS_STALE"
        );
        assert_eq!(
            fixture
                .branches
                .advance_branch("mainline", child, fixture.genesis_transaction_id)
                .unwrap_err()
                .code(),
            "BRANCH_NOT_FAST_FORWARD"
        );

        let child_revision = fixture.transactions.verified_revision(child).unwrap();
        let mut duplicate_parent = child_revision.receipt().transaction.record.clone();
        duplicate_parent
            .parent_transaction_ids
            .push(fixture.genesis_transaction_id);
        duplicate_parent
            .parent_roots
            .push(duplicate_parent.parent_roots[0]);
        assert_eq!(
            build_transaction(&duplicate_parent).unwrap_err().code(),
            "TXN_PARENT_SHAPE"
        );
    }

    #[test]
    fn cross_workspace_advance_and_existing_current_ref_report_origin_mismatch() {
        let primary = Fixture::new_with_workspace("workspace-primary", 1);
        let secondary = Fixture::new_with_workspace("workspace-secondary", 9);
        import_fixture_revisions(&secondary, &primary);
        primary
            .branches
            .create_branch("mainline", primary.genesis_transaction_id)
            .unwrap();
        assert_eq!(
            primary
                .branches
                .advance_branch(
                    "mainline",
                    primary.genesis_transaction_id,
                    secondary.genesis_transaction_id,
                )
                .unwrap_err()
                .code(),
            "BRANCH_ORIGIN_MISMATCH"
        );

        let name = BranchName::parse("mainline").unwrap();
        let origin = primary.branches.resolve_branch("mainline").unwrap().origin;
        let foreign = primary
            .transactions
            .verified_revision(secondary.genesis_transaction_id)
            .unwrap();
        let foreign_ref = build_branch_ref(&ref_record(&name, origin.digest, &foreign)).unwrap();
        fs::write(primary.branches.ref_path(&name), foreign_ref.stored_bytes).unwrap();
        assert_eq!(
            primary
                .branches
                .create_branch("mainline", primary.genesis_transaction_id)
                .unwrap_err()
                .code(),
            "BRANCH_ORIGIN_MISMATCH"
        );
    }

    #[test]
    fn concurrent_create_and_advance_have_one_mutating_winner() {
        let fixture = Fixture::new("concurrent");
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let repository = fixture.branches.clone();
            let barrier = Arc::clone(&barrier);
            let origin = fixture.genesis_transaction_id;
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                repository.create_branch("race", origin)
            }));
        }
        barrier.wait();
        let mut create_statuses = handles
            .into_iter()
            .map(|handle| handle.join().unwrap().unwrap())
            .collect::<Vec<_>>();
        create_statuses.sort_by_key(|status| status.tag());
        assert_eq!(
            create_statuses,
            vec![BranchUpdateStatus::Created, BranchUpdateStatus::Present]
        );

        let child = fixture.commit_child(30);
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let repository = fixture.branches.clone();
            let barrier = Arc::clone(&barrier);
            let origin = fixture.genesis_transaction_id;
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                repository.advance_branch("race", origin, child)
            }));
        }
        barrier.wait();
        let mut advance_statuses = handles
            .into_iter()
            .map(|handle| handle.join().unwrap().unwrap())
            .collect::<Vec<_>>();
        advance_statuses.sort_by_key(|status| status.tag());
        assert_eq!(
            advance_statuses,
            vec![BranchUpdateStatus::Advanced, BranchUpdateStatus::Present]
        );
    }

    #[test]
    fn competing_distinct_create_and_advance_targets_have_one_cas_winner() {
        let create_fixture = Fixture::new("distinct-create-race");
        let create_child = create_fixture.commit_child(30);
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for origin in [create_fixture.genesis_transaction_id, create_child] {
            let repository = create_fixture.branches.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                repository.create_branch("distinct-create", origin)
            }));
        }
        barrier.wait();
        let create_results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            create_results
                .iter()
                .filter(|result| matches!(result, Ok(BranchUpdateStatus::Created)))
                .count(),
            1
        );
        assert_eq!(
            create_results
                .iter()
                .filter(|result| {
                    result
                        .as_ref()
                        .is_err_and(|error| error.code() == "BRANCH_ORIGIN_MISMATCH")
                })
                .count(),
            1
        );

        let primary = Fixture::new("distinct-advance-primary");
        let secondary = Fixture::new("distinct-advance-secondary");
        assert_eq!(
            primary.genesis_transaction_id,
            secondary.genesis_transaction_id
        );
        primary
            .branches
            .create_branch("distinct-advance", primary.genesis_transaction_id)
            .unwrap();
        let left = primary.commit_child(30);
        let right = secondary.commit_child(31);
        import_fixture_revisions(&secondary, &primary);
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for target in [left, right] {
            let repository = primary.branches.clone();
            let barrier = Arc::clone(&barrier);
            let expected = primary.genesis_transaction_id;
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                repository.advance_branch("distinct-advance", expected, target)
            }));
        }
        barrier.wait();
        let advance_results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            advance_results
                .iter()
                .filter(|result| matches!(result, Ok(BranchUpdateStatus::Advanced)))
                .count(),
            1
        );
        assert_eq!(
            advance_results
                .iter()
                .filter(|result| {
                    result
                        .as_ref()
                        .is_err_and(|error| error.code() == "REF_NAMED_CAS_STALE")
                })
                .count(),
            1
        );
    }

    #[test]
    fn exclusive_gc_ownership_serializes_transaction_and_ref_mutation() {
        let fixture = Fixture::new("gc-maintenance-race");
        let head = fixture.transactions.accepted_head().unwrap();
        let candidate = candidate_for(
            head.state_root().record.workspace_id,
            fixture.principal_id,
            head.transaction_id(),
            head.state_root(),
            head.policy_root(),
            30,
        );
        let repository = fixture.transactions.clone();
        let principal_id = fixture.principal_id;
        let parent = head.transaction_id();
        let store = ObjectStore::new(&fixture.temp.path);
        let guard = acquire_exclusive_gc(&store).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let transaction_thread = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let result = repository
                .commit(CommitInput::new(
                    parent,
                    &candidate.stored_bytes,
                    principal_id,
                    &[],
                    NOW,
                    CandidateValidationLimits::full_v1(),
                ))
                .map(|output| output.transaction_id());
            finished_tx.send(result).unwrap();
        });
        started_rx.recv().unwrap();
        assert!(
            finished_rx
                .recv_timeout(Duration::from_millis(100))
                .is_err()
        );
        drop(guard);
        let child = finished_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        transaction_thread.join().unwrap();

        let guard = acquire_exclusive_gc(&store).unwrap();
        let branches = fixture.branches.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let ref_thread = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            finished_tx
                .send(branches.create_branch("gc-serialized", child))
                .unwrap();
        });
        started_rx.recv().unwrap();
        assert!(
            finished_rx
                .recv_timeout(Duration::from_millis(100))
                .is_err()
        );
        drop(guard);
        assert_eq!(
            finished_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap(),
            BranchUpdateStatus::Created
        );
        ref_thread.join().unwrap();
    }

    #[test]
    fn target_mismatch_cross_path_substitution_and_unknown_files_fail_closed() {
        let fixture = Fixture::new("confinement");
        for name in ["alpha", "beta"] {
            fixture
                .branches
                .create_branch(name, fixture.genesis_transaction_id)
                .unwrap();
        }
        let alpha = BranchName::parse("alpha").unwrap();
        let beta = BranchName::parse("beta").unwrap();
        let alpha_origin = fs::read(fixture.branches.branch_path(&alpha)).unwrap();
        fs::write(fixture.branches.branch_path(&beta), alpha_origin).unwrap();
        assert_eq!(
            fixture.branches.resolve_branch("beta").unwrap_err().code(),
            "REF_NAME_COLLISION"
        );

        let reference_path = fixture.branches.ref_path(&alpha);
        let mut reference = import_branch_ref(&fs::read(&reference_path).unwrap()).unwrap();
        reference.record.head_state_root = fixed(99, StateRoot::from_bytes);
        let forged = build_branch_ref(&reference.record).unwrap();
        fs::write(&reference_path, forged.stored_bytes).unwrap();
        assert_eq!(
            fixture.branches.resolve_branch("alpha").unwrap_err().code(),
            "REF_TARGET_MISMATCH"
        );

        fs::write(fixture.branches.refs_dir().join("unknown"), b"foreign").unwrap();
        assert_eq!(
            fixture.branches.list_branches(10).unwrap_err().code(),
            "REF_IO"
        );
    }

    #[test]
    fn branch_resolution_preserves_receipt_root_policy_object_and_manifest_failures() {
        let missing_receipt = Fixture::new("resolve-missing-receipt");
        missing_receipt
            .branches
            .create_branch("mainline", missing_receipt.genesis_transaction_id)
            .unwrap();
        fs::remove_file(transaction_receipt_path(
            &missing_receipt.temp.path,
            missing_receipt.genesis_transaction_id,
        ))
        .unwrap();
        assert_eq!(
            missing_receipt
                .branches
                .resolve_branch("mainline")
                .unwrap_err()
                .code(),
            "RECOVERY_RECEIPT_INCOMPLETE"
        );

        let corrupt_root = Fixture::new("resolve-corrupt-root");
        corrupt_root
            .branches
            .create_branch("mainline", corrupt_root.genesis_transaction_id)
            .unwrap();
        let revision = corrupt_root
            .transactions
            .verified_revision(corrupt_root.genesis_transaction_id)
            .unwrap();
        fs::write(
            transaction_receipt_path(&corrupt_root.temp.path, corrupt_root.genesis_transaction_id),
            corrupt_nested_receipt(
                revision.receipt(),
                &revision.receipt().record.stored_state_root,
            ),
        )
        .unwrap();
        assert_eq!(
            corrupt_root
                .branches
                .resolve_branch("mainline")
                .unwrap_err()
                .code(),
            "SCB_DIGEST_MISMATCH"
        );

        let corrupt_policy = Fixture::new("resolve-corrupt-policy");
        corrupt_policy
            .branches
            .create_branch("mainline", corrupt_policy.genesis_transaction_id)
            .unwrap();
        let revision = corrupt_policy
            .transactions
            .verified_revision(corrupt_policy.genesis_transaction_id)
            .unwrap();
        fs::write(
            transaction_receipt_path(
                &corrupt_policy.temp.path,
                corrupt_policy.genesis_transaction_id,
            ),
            corrupt_nested_receipt(
                revision.receipt(),
                &revision.receipt().record.stored_policy_root,
            ),
        )
        .unwrap();
        assert_eq!(
            corrupt_policy
                .branches
                .resolve_branch("mainline")
                .unwrap_err()
                .code(),
            "SCB_DIGEST_MISMATCH"
        );
    }

    #[test]
    fn branch_resolution_preserves_object_and_manifest_failures() {
        let missing_object = Fixture::new("resolve-missing-object");
        missing_object
            .branches
            .create_branch("mainline", missing_object.genesis_transaction_id)
            .unwrap();
        let object = missing_object
            .transactions
            .verified_revision(missing_object.genesis_transaction_id)
            .unwrap()
            .objects()[0]
            .object_id();
        let store = ObjectStore::new(&missing_object.temp.path);
        fs::remove_file(store.object_path(object)).unwrap();
        assert_eq!(
            missing_object
                .branches
                .resolve_branch("mainline")
                .unwrap_err()
                .code(),
            "STORE_OBJECT_NOT_FOUND"
        );

        let corrupt_object = Fixture::new("resolve-corrupt-object");
        corrupt_object
            .branches
            .create_branch("mainline", corrupt_object.genesis_transaction_id)
            .unwrap();
        let object = corrupt_object
            .transactions
            .verified_revision(corrupt_object.genesis_transaction_id)
            .unwrap()
            .objects()[0]
            .object_id();
        let store = ObjectStore::new(&corrupt_object.temp.path);
        let object_path = store.object_path(object);
        let mut bytes = fs::read(&object_path).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        fs::write(object_path, bytes).unwrap();
        assert_eq!(
            corrupt_object
                .branches
                .resolve_branch("mainline")
                .unwrap_err()
                .code(),
            "SCB_DIGEST_MISMATCH"
        );

        let corrupt_manifest = Fixture::new("resolve-corrupt-manifest");
        corrupt_manifest
            .branches
            .create_branch("mainline", corrupt_manifest.genesis_transaction_id)
            .unwrap();
        let revision = corrupt_manifest
            .transactions
            .verified_revision(corrupt_manifest.genesis_transaction_id)
            .unwrap();
        let mut record = revision.receipt().record.clone();
        record.object_manifest[0].stored_length += 1;
        let forged = build_transaction_receipt(&record).unwrap();
        fs::write(
            transaction_receipt_path(
                &corrupt_manifest.temp.path,
                corrupt_manifest.genesis_transaction_id,
            ),
            forged.stored_bytes,
        )
        .unwrap();
        assert_eq!(
            corrupt_manifest
                .branches
                .resolve_branch("mainline")
                .unwrap_err()
                .code(),
            "TXN_OBJECT_INVENTORY_MISMATCH"
        );
    }

    #[test]
    fn forged_origin_and_every_current_fact_fail_against_durable_evidence() {
        let origin_fixture = Fixture::new("forged-origin-facts");
        origin_fixture
            .branches
            .create_branch("mainline", origin_fixture.genesis_transaction_id)
            .unwrap();
        let name = BranchName::parse("mainline").unwrap();
        let origin_path = origin_fixture.branches.branch_path(&name);
        let ref_path = origin_fixture.branches.ref_path(&name);
        let base_origin = import_branch_record(&fs::read(&origin_path).unwrap()).unwrap();
        let base_ref = import_branch_ref(&fs::read(&ref_path).unwrap()).unwrap();
        let mut variants = Vec::new();
        let mut variant = base_origin.record.clone();
        variant.workspace_id = fixed(61, WorkspaceId::from_bytes);
        variants.push(variant);
        let mut variant = base_origin.record.clone();
        variant.origin_state_root = fixed(62, StateRoot::from_bytes);
        variants.push(variant);
        let mut variant = base_origin.record.clone();
        variant.schema_epoch_id = fixed(63, SchemaEpochId::from_bytes);
        variants.push(variant);
        let mut variant = base_origin.record.clone();
        variant.policy_root_id = fixed(64, PolicyRootId::from_bytes);
        variants.push(variant);
        let mut variant = base_origin.record.clone();
        variant.dependency_roots = vec![fixed(65, StateRoot::from_bytes)];
        variants.push(variant);
        for variant in variants {
            let forged_origin = build_branch_record(&variant).unwrap();
            let mut forged_ref_record = base_ref.record.clone();
            forged_ref_record.branch_record_digest = forged_origin.digest;
            fs::write(&origin_path, forged_origin.stored_bytes).unwrap();
            fs::write(
                &ref_path,
                build_branch_ref(&forged_ref_record).unwrap().stored_bytes,
            )
            .unwrap();
            assert_eq!(
                origin_fixture
                    .branches
                    .resolve_branch("mainline")
                    .unwrap_err()
                    .code(),
                "BRANCH_ORIGIN_MISMATCH"
            );
        }

        let current_fixture = Fixture::new("forged-current-facts");
        current_fixture
            .branches
            .create_branch("mainline", current_fixture.genesis_transaction_id)
            .unwrap();
        let child = current_fixture.commit_child(30);
        let ref_path = current_fixture.branches.ref_path(&name);
        let base_ref = import_branch_ref(&fs::read(&ref_path).unwrap()).unwrap();
        let mut variants = Vec::new();
        let mut variant = base_ref.record.clone();
        variant.workspace_id = fixed(71, WorkspaceId::from_bytes);
        variants.push(variant);
        let mut variant = base_ref.record.clone();
        variant.head_transaction_id = child;
        variants.push(variant);
        let mut variant = base_ref.record.clone();
        variant.head_state_root = fixed(72, StateRoot::from_bytes);
        variants.push(variant);
        let mut variant = base_ref.record.clone();
        variant.schema_epoch_id = fixed(73, SchemaEpochId::from_bytes);
        variants.push(variant);
        let mut variant = base_ref.record.clone();
        variant.policy_root_id = fixed(74, PolicyRootId::from_bytes);
        variants.push(variant);
        let mut variant = base_ref.record;
        variant.dependency_roots = vec![fixed(75, StateRoot::from_bytes)];
        variants.push(variant);
        for variant in variants {
            fs::write(&ref_path, build_branch_ref(&variant).unwrap().stored_bytes).unwrap();
            assert_eq!(
                current_fixture
                    .branches
                    .resolve_branch("mainline")
                    .unwrap_err()
                    .code(),
                "REF_TARGET_MISMATCH"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_roots_locks_and_records_fail_closed() {
        use std::os::unix::fs::symlink;

        let root = TempDir::new("symlink-root");
        let outside = TempDir::new("symlink-outside");
        symlink(&outside.path, root.path.join("refs")).unwrap();
        assert_eq!(
            BranchRepository::new(&root.path)
                .resolve_branch("alpha")
                .unwrap_err()
                .code(),
            "REF_IO"
        );

        let fanout_root = TempDir::new("symlink-fanout");
        let fanout_repository = BranchRepository::new(&fanout_root.path);
        fanout_repository.ensure_layout().unwrap();
        let fanout_name = BranchName::parse("alpha").unwrap();
        let fanout_hex = hex_digest(&fanout_name.path_key());
        symlink(
            &outside.path,
            fanout_root
                .path
                .join("refs")
                .join("v1")
                .join(&fanout_hex[0..2]),
        )
        .unwrap();
        assert_eq!(
            fanout_repository
                .resolve_branch("alpha")
                .unwrap_err()
                .code(),
            "REF_IO"
        );

        let lock_fixture = Fixture::new("symlink-lock");
        let lock_target = outside.path.join("foreign-lock");
        fs::write(&lock_target, b"foreign").unwrap();
        symlink(
            &lock_target,
            lock_fixture.temp.path.join("locks").join("refs.lock"),
        )
        .unwrap();
        assert_eq!(
            lock_fixture
                .branches
                .create_branch("alpha", lock_fixture.genesis_transaction_id)
                .unwrap_err()
                .code(),
            "REF_IO"
        );

        let fixture = Fixture::new("symlink-record");
        fixture
            .branches
            .create_branch("alpha", fixture.genesis_transaction_id)
            .unwrap();
        let name = BranchName::parse("alpha").unwrap();
        let ref_path = fixture.branches.ref_path(&name);
        fs::remove_file(&ref_path).unwrap();
        symlink(fixture.branches.branch_path(&name), &ref_path).unwrap();
        assert_eq!(
            fixture.branches.resolve_branch("alpha").unwrap_err().code(),
            "REF_IO"
        );
    }

    #[test]
    fn ancestry_is_head_first_bounded_convergent_and_cycle_checked() {
        let fixture = Fixture::new("ancestry");
        fixture
            .branches
            .create_branch("lineage", fixture.genesis_transaction_id)
            .unwrap();
        let child = fixture.commit_child(30);
        fixture
            .branches
            .advance_branch("lineage", fixture.genesis_transaction_id, child)
            .unwrap();
        let grandchild = fixture.commit_child(31);
        fixture
            .branches
            .advance_branch("lineage", child, grandchild)
            .unwrap();
        let ancestry = fixture.branches.branch_ancestry("lineage", 3).unwrap();
        assert_eq!(
            ancestry
                .iter()
                .map(|entry| entry.transaction_id)
                .collect::<Vec<_>>(),
            vec![grandchild, child, fixture.genesis_transaction_id]
        );
        assert_eq!(
            fixture
                .branches
                .branch_ancestry("lineage", 2)
                .unwrap_err()
                .code(),
            "BRANCH_RESOURCE_LIMIT"
        );

        let head = fixed(40, TransactionId::from_bytes);
        let left = fixed(41, TransactionId::from_bytes);
        let right = fixed(42, TransactionId::from_bytes);
        let shared = fixed(43, TransactionId::from_bytes);
        let graph = BTreeMap::from([
            (head, vec![left, right]),
            (left, vec![shared]),
            (right, vec![shared]),
            (shared, vec![]),
        ]);
        let convergent = walk_ancestry(head, 4, |transaction_id| {
            Ok(BranchAncestryEntry {
                transaction_id,
                state_root: fixed(50, StateRoot::from_bytes),
                parent_transaction_ids: graph[&transaction_id].clone(),
            })
        })
        .unwrap();
        assert_eq!(
            convergent
                .iter()
                .map(|entry| entry.transaction_id)
                .collect::<Vec<_>>(),
            vec![head, left, shared, right]
        );

        let cycle = BTreeMap::from([(left, vec![right]), (right, vec![left])]);
        assert_eq!(
            walk_ancestry(left, 3, |transaction_id| {
                Ok(BranchAncestryEntry {
                    transaction_id,
                    state_root: fixed(51, StateRoot::from_bytes),
                    parent_transaction_ids: cycle[&transaction_id].clone(),
                })
            })
            .unwrap_err()
            .code(),
            "BRANCH_ANCESTRY_CYCLE"
        );
    }

    #[test]
    fn recovery_removes_only_exact_owned_stages_and_reports_orphans() {
        let fixture = Fixture::new("recovery");
        fixture
            .branches
            .create_branch("orphan", fixture.genesis_transaction_id)
            .unwrap();
        let name = BranchName::parse("orphan").unwrap();
        let branch_path = fixture.branches.branch_path(&name);
        let ref_path = fixture.branches.ref_path(&name);
        fs::remove_file(&ref_path).unwrap();
        let branch_stage = branch_path.parent().unwrap().join(format!(
            "{BRANCH_STAGE_PREFIX}{}-0000000000000001{STAGE_SUFFIX}",
            std::process::id()
        ));
        let ref_stage = ref_path.parent().unwrap().join(format!(
            "{REF_STAGE_PREFIX}{}-0000000000000002{STAGE_SUFFIX}",
            std::process::id()
        ));
        fs::write(&branch_stage, b"partial").unwrap();
        fs::write(&ref_stage, b"partial").unwrap();
        let report = fixture.branches.recover_refs().unwrap();
        assert_eq!(report.removed_branch_stages, 1);
        assert_eq!(report.removed_ref_stages, 1);
        assert_eq!(report.visible_branches, 0);
        assert_eq!(report.orphan_origins.len(), 1);
        assert_eq!(report.orphan_origins[0].branch_name, name);
        assert_eq!(report.verified_ancestry_transactions, 0);
        assert!(!branch_stage.exists());
        assert!(!ref_stage.exists());

        let foreign = branch_path
            .parent()
            .unwrap()
            .join(".sley-branch-stage-foreign.tmp");
        fs::write(&foreign, b"foreign").unwrap();
        let preserved = fixture.branches.recover_refs().unwrap();
        assert_eq!(preserved.removed_branch_stages, 0);
        assert_eq!(preserved.removed_ref_stages, 0);
        assert_eq!(preserved.orphan_origins.len(), 1);
        assert!(foreign.exists());
    }

    #[test]
    fn recovery_retry_syncs_leaf_even_after_prior_delete_sync_failure() {
        let fixture = Fixture::new("recovery-sync-retry");
        fixture
            .branches
            .create_branch("sync-retry", fixture.genesis_transaction_id)
            .unwrap();
        let name = BranchName::parse("sync-retry").unwrap();
        let branch_path = fixture.branches.branch_path(&name);
        let stage_path = branch_path.parent().unwrap().join(format!(
            "{BRANCH_STAGE_PREFIX}{}-0000000000000003{STAGE_SUFFIX}",
            std::process::id()
        ));
        fs::write(&stage_path, b"interrupted").unwrap();
        assert_eq!(
            fixture
                .branches
                .recover_refs_with_native_ref_durability_cut(
                    NativeRefDurabilityCut::Ref15OriginRecoveryStageUnlinkBeforeLeafSync {
                        branch_name: name.clone(),
                    },
                )
                .unwrap_err()
                .code(),
            "REF_IO"
        );
        assert!(!stage_path.exists());
        let retry = fixture.branches.recover_refs().unwrap();
        assert_eq!(retry.removed_branch_stages, 0);
        assert_eq!(retry.removed_ref_stages, 0);
        assert_eq!(retry.visible_branches, 1);

        let ref_stage_path = fixture
            .branches
            .ref_path(&name)
            .parent()
            .unwrap()
            .join(format!(
                "{REF_STAGE_PREFIX}{}-0000000000000004{STAGE_SUFFIX}",
                std::process::id()
            ));
        fs::write(&ref_stage_path, b"interrupted").unwrap();
        assert_eq!(
            fixture
                .branches
                .recover_refs_with_native_ref_durability_cut(
                    NativeRefDurabilityCut::Ref16VisibleRefRecoveryStageUnlinkBeforeLeafSync {
                        branch_name: name.clone(),
                    },
                )
                .unwrap_err()
                .code(),
            "REF_IO"
        );
        assert!(!ref_stage_path.exists());
        let second_retry = fixture.branches.recover_refs().unwrap();
        assert_eq!(second_retry.removed_branch_stages, 0);
        assert_eq!(second_retry.removed_ref_stages, 0);
        assert_eq!(second_retry.visible_branches, 1);
    }

    #[test]
    fn recovery_verifies_shared_and_advanced_branch_ancestries() {
        let fixture = Fixture::new("recovery-ancestry");
        let child = fixture.commit_child(31);
        fixture
            .branches
            .create_branch("mainline", fixture.genesis_transaction_id)
            .unwrap();
        fixture
            .branches
            .advance_branch("mainline", fixture.genesis_transaction_id, child)
            .unwrap();
        fixture
            .branches
            .create_branch("feature", fixture.genesis_transaction_id)
            .unwrap();
        let report = fixture.branches.recover_refs().unwrap();
        assert_eq!(report.removed_branch_stages, 0);
        assert_eq!(report.removed_ref_stages, 0);
        assert_eq!(report.visible_branches, 2);
        assert!(report.orphan_origins.is_empty());
        assert_eq!(report.verified_ancestry_transactions, 2);
        let second = fixture.branches.recover_refs().unwrap();
        assert_eq!(second.verified_ancestry_transactions, 2);
        assert_eq!(second.visible_branches, 2);
    }

    #[test]
    fn native_ref_durability_cut_selection_installs_and_clears() {
        let branch_name = BranchName::parse("cut-clears").unwrap();
        let cuts = [
            NativeRefDurabilityCut::Rlay01BranchesV1CreateBeforeBranchesSync,
            NativeRefDurabilityCut::Rlay02FirstOriginFanoutCreateBeforeParentSync,
            NativeRefDurabilityCut::Rlay03SecondOriginFanoutCreateBeforeParentSync,
            NativeRefDurabilityCut::Ref01DuringOriginStageWrite,
            NativeRefDurabilityCut::Ref02VerifiedOriginStageBeforeFinalLink,
            NativeRefDurabilityCut::Ref03OriginLinkBeforeFirstLeafSync,
            NativeRefDurabilityCut::Ref04FirstOriginLeafSyncBeforeStageUnlink,
            NativeRefDurabilityCut::Ref05OriginStageUnlinkBeforeSecondLeafSync,
            NativeRefDurabilityCut::Ref06DuringInitialRefStageWrite,
            NativeRefDurabilityCut::Ref07VerifiedInitialRefStageBeforeFinalLink,
            NativeRefDurabilityCut::Ref08InitialRefLinkBeforeFirstLeafSync,
            NativeRefDurabilityCut::Ref09FirstInitialRefLeafSyncBeforeStageUnlink,
            NativeRefDurabilityCut::Ref10InitialRefStageUnlinkBeforeSecondLeafSync,
            NativeRefDurabilityCut::Ref11DuringAdvanceRefStageWrite,
            NativeRefDurabilityCut::Ref12VerifiedAdvanceRefStageBeforeRename,
            NativeRefDurabilityCut::Ref13AdvanceRefRenameBeforeLeafSync,
            NativeRefDurabilityCut::Ref14AdvanceRefLeafSyncBeforeResponse,
            NativeRefDurabilityCut::Ref15OriginRecoveryStageUnlinkBeforeLeafSync {
                branch_name: branch_name.clone(),
            },
            NativeRefDurabilityCut::Ref16VisibleRefRecoveryStageUnlinkBeforeLeafSync {
                branch_name,
            },
        ];
        for cut in cuts {
            let selection = NativeRefCutSelection::install(cut);
            drop(selection);
            assert!(!take_selected_native_ref_cut(|_| true));
        }
    }

    #[test]
    fn interrupted_stage_writes_leave_only_removable_stages() {
        let fixture = Fixture::new("stage-write-cuts");
        let name = BranchName::parse("write-fault").unwrap();
        assert_eq!(
            fixture
                .branches
                .create_branch_with_native_ref_durability_cut(
                    "write-fault",
                    fixture.genesis_transaction_id,
                    NativeRefDurabilityCut::Ref01DuringOriginStageWrite,
                )
                .unwrap_err()
                .code(),
            "REF_IO"
        );
        assert!(!fixture.branches.branch_path(&name).exists());
        assert!(!fixture.branches.ref_path(&name).exists());
        assert_eq!(
            fixture
                .branches
                .create_branch_with_native_ref_durability_cut(
                    "write-fault",
                    fixture.genesis_transaction_id,
                    NativeRefDurabilityCut::Ref06DuringInitialRefStageWrite,
                )
                .unwrap_err()
                .code(),
            "REF_IO"
        );
        assert!(fixture.branches.branch_path(&name).exists());
        assert!(!fixture.branches.ref_path(&name).exists());
        let report = fixture.branches.recover_refs().unwrap();
        assert_eq!(report.removed_branch_stages, 1);
        assert_eq!(report.removed_ref_stages, 1);
        assert_eq!(report.visible_branches, 0);
        assert_eq!(report.orphan_origins.len(), 1);
        assert_eq!(report.orphan_origins[0].branch_name, name);
        assert_eq!(
            fixture
                .branches
                .create_branch("write-fault", fixture.genesis_transaction_id)
                .unwrap(),
            BranchUpdateStatus::Created
        );
    }

    #[test]
    fn stage_name_ownership_requires_the_complete_generated_shape() {
        assert!(is_owned_stage_name(
            ".sley-ref-stage-123-0123456789abcdef.tmp",
            REF_STAGE_PREFIX
        ));
        for foreign in [
            ".sley-ref-stage-user.tmp",
            ".sley-ref-stage-123-short.tmp",
            ".sley-ref-stage-123-0123456789ABCDEF.tmp",
            ".sley-ref-stage-0123-0123456789abcdef.tmp",
            ".sley-ref-stage-0-0123456789abcdef.tmp",
            ".sley-ref-stage--0123456789abcdef.tmp",
            ".sley-ref-stage-123-0123456789abcdef.tmp.extra",
        ] {
            assert!(!is_owned_stage_name(foreign, REF_STAGE_PREFIX));
        }
    }

    #[test]
    fn rlay01_branches_v1_create_retry_reaches_sync_hook() {
        let fixture = Fixture::new("rlay01");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1");
        let secondary_path = owner_root.join("refs").join("v1");
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.ensure_layout_with_native_ref_durability_cut(NativeRefDurabilityCut::Rlay01BranchesV1CreateBeforeBranchesSync);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let second_fault_result = branch_repository.ensure_layout_with_native_ref_durability_cut(NativeRefDurabilityCut::Rlay01BranchesV1CreateBeforeBranchesSync);
        let second_fault_error = second_fault_result.expect_err("expected second fault durability error");
        ::core::assert_eq!(second_fault_error.code(), "REF_IO");
        let after_second_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_second_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_second_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let ordinary_retry_result = branch_repository.recover_refs();
        ::core::assert!(ordinary_retry_result.is_ok());
        let after_ordinary_retry_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_ordinary_retry_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_ordinary_retry_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_eq!(after_first_fault_owner_tree_snapshot, after_second_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_second_fault_owner_tree_snapshot, after_ordinary_retry_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_second_fault_primary_path_snapshot);
        ::core::assert_eq!(after_second_fault_primary_path_snapshot, after_ordinary_retry_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_second_fault_secondary_path_snapshot);
        ::core::assert_ne!(after_second_fault_secondary_path_snapshot, after_ordinary_retry_secondary_path_snapshot);
        ::core::assert_eq!(ordinary_retry_result.as_ref().unwrap().removed_branch_stages, 0_u64);
        ::core::assert_eq!(ordinary_retry_result.as_ref().unwrap().visible_branches, 0_u64);
    }

    #[test]
    fn rlay02_first_origin_fanout_retry_reaches_sync_hook() {
        let fixture = Fixture::new("rlay02");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "rlay02-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]);
        let secondary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]);
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Rlay02FirstOriginFanoutCreateBeforeParentSync);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let second_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Rlay02FirstOriginFanoutCreateBeforeParentSync);
        let second_fault_error = second_fault_result.expect_err("expected second fault durability error");
        ::core::assert_eq!(second_fault_error.code(), "REF_IO");
        let after_second_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_second_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_second_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let ordinary_retry_result = branch_repository.create_branch(branch_name, genesis_transaction_id);
        ::core::assert!(ordinary_retry_result.is_ok());
        let after_ordinary_retry_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_ordinary_retry_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_ordinary_retry_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_eq!(after_first_fault_owner_tree_snapshot, after_second_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_second_fault_owner_tree_snapshot, after_ordinary_retry_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_second_fault_primary_path_snapshot);
        ::core::assert_eq!(after_second_fault_primary_path_snapshot, after_ordinary_retry_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_second_fault_secondary_path_snapshot);
        ::core::assert_ne!(after_second_fault_secondary_path_snapshot, after_ordinary_retry_secondary_path_snapshot);
        ::core::assert_eq!(ordinary_retry_result.as_ref().unwrap(), &BranchUpdateStatus::Created);
    }

    #[test]
    fn rlay03_second_origin_fanout_retry_reaches_sync_hook() {
        let fixture = Fixture::new("rlay03");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "rlay03-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]);
        let secondary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Rlay03SecondOriginFanoutCreateBeforeParentSync);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let second_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Rlay03SecondOriginFanoutCreateBeforeParentSync);
        let second_fault_error = second_fault_result.expect_err("expected second fault durability error");
        ::core::assert_eq!(second_fault_error.code(), "REF_IO");
        let after_second_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_second_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_second_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let ordinary_retry_result = branch_repository.create_branch(branch_name, genesis_transaction_id);
        ::core::assert!(ordinary_retry_result.is_ok());
        let after_ordinary_retry_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_ordinary_retry_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_ordinary_retry_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_eq!(after_first_fault_owner_tree_snapshot, after_second_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_second_fault_owner_tree_snapshot, after_ordinary_retry_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_second_fault_primary_path_snapshot);
        ::core::assert_eq!(after_second_fault_primary_path_snapshot, after_ordinary_retry_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_second_fault_secondary_path_snapshot);
        ::core::assert_ne!(after_second_fault_secondary_path_snapshot, after_ordinary_retry_secondary_path_snapshot);
        ::core::assert_eq!(ordinary_retry_result.as_ref().unwrap(), &BranchUpdateStatus::Created);
    }

    #[test]
    fn ref01_during_origin_stage_write_removes_stage() {
        let fixture = Fixture::new("ref01");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref01-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Ref01DuringOriginStageWrite);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_first_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_eq!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_branch_stages, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().orphan_origins.len(), 0_usize);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 0_u64);
    }

    #[test]
    fn ref02_verified_origin_stage_before_link_removes_stage() {
        let fixture = Fixture::new("ref02");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref02-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Ref02VerifiedOriginStageBeforeFinalLink);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_first_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_eq!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_branch_stages, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().orphan_origins.len(), 0_usize);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 0_u64);
    }

    #[test]
    fn ref03_origin_link_before_first_sync_create_retry_redurabilizes() {
        let fixture = Fixture::new("ref03");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref03-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Ref03OriginLinkBeforeFirstLeafSync);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let ordinary_retry_result = branch_repository.create_branch(branch_name, genesis_transaction_id);
        ::core::assert!(ordinary_retry_result.is_ok());
        let after_ordinary_retry_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_ordinary_retry_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_ordinary_retry_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_first_fault_owner_tree_snapshot, after_ordinary_retry_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_ordinary_retry_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_ne!(after_first_fault_secondary_path_snapshot, after_ordinary_retry_secondary_path_snapshot);
        ::core::assert_eq!(ordinary_retry_result.as_ref().unwrap(), &BranchUpdateStatus::Created);
    }

    #[test]
    fn ref04_first_origin_sync_before_unlink_reports_orphan() {
        let fixture = Fixture::new("ref04");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref04-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Ref04FirstOriginLeafSyncBeforeStageUnlink);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_first_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_branch_stages, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().orphan_origins.len(), 1_usize);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 0_u64);
    }

    #[test]
    fn ref05_origin_stage_unlink_before_second_sync_reports_orphan() {
        let fixture = Fixture::new("ref05");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref05-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Ref05OriginStageUnlinkBeforeSecondLeafSync);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_eq!(after_first_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_branch_stages, 0_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().orphan_origins.len(), 1_usize);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 0_u64);
    }

    #[test]
    fn ref06_during_initial_ref_stage_write_reports_orphan() {
        let fixture = Fixture::new("ref06");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref06-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Ref06DuringInitialRefStageWrite);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_first_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_ref_stages, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().orphan_origins.len(), 1_usize);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 0_u64);
    }

    #[test]
    fn ref07_verified_initial_ref_stage_before_link_reports_orphan() {
        let fixture = Fixture::new("ref07");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref07-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Ref07VerifiedInitialRefStageBeforeFinalLink);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_first_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_ref_stages, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().orphan_origins.len(), 1_usize);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 0_u64);
    }

    #[test]
    fn ref08_initial_ref_link_before_first_sync_create_retry_is_present() {
        let fixture = Fixture::new("ref08");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref08-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Ref08InitialRefLinkBeforeFirstLeafSync);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let ordinary_retry_result = branch_repository.create_branch(branch_name, genesis_transaction_id);
        ::core::assert!(ordinary_retry_result.is_ok());
        let after_ordinary_retry_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_ordinary_retry_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_ordinary_retry_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_eq!(after_first_fault_owner_tree_snapshot, after_ordinary_retry_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_ordinary_retry_primary_path_snapshot);
        ::core::assert_ne!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_ordinary_retry_secondary_path_snapshot);
        ::core::assert_eq!(ordinary_retry_result.as_ref().unwrap(), &BranchUpdateStatus::Present);
    }

    #[test]
    fn ref09_first_initial_ref_sync_before_unlink_verifies_branch() {
        let fixture = Fixture::new("ref09");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref09-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Ref09FirstInitialRefLeafSyncBeforeStageUnlink);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_first_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_ne!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_ref_stages, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().verified_ancestry_transactions, 1_u64);
    }

    #[test]
    fn ref10_initial_ref_stage_unlink_before_second_sync_verifies_branch() {
        let fixture = Fixture::new("ref10");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref10-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.create_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, NativeRefDurabilityCut::Ref10InitialRefStageUnlinkBeforeSecondLeafSync);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_eq!(after_first_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_ne!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_ref_stages, 0_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().verified_ancestry_transactions, 1_u64);
    }

    #[test]
    fn ref11_during_advance_ref_stage_write_keeps_old_branch() {
        let fixture = Fixture::new("ref11");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref11-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        branch_repository.create_branch(branch_name, genesis_transaction_id).unwrap();
        let child_transaction_id = fixture.commit_child(41);
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.advance_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, child_transaction_id, NativeRefDurabilityCut::Ref11DuringAdvanceRefStageWrite);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_first_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_eq!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_ref_stages, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().verified_ancestry_transactions, 1_u64);
    }

    #[test]
    fn ref12_verified_advance_ref_stage_before_rename_keeps_old_branch() {
        let fixture = Fixture::new("ref12");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref12-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        branch_repository.create_branch(branch_name, genesis_transaction_id).unwrap();
        let child_transaction_id = fixture.commit_child(42);
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.advance_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, child_transaction_id, NativeRefDurabilityCut::Ref12VerifiedAdvanceRefStageBeforeRename);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_first_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_eq!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_ref_stages, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().verified_ancestry_transactions, 1_u64);
    }

    #[test]
    fn ref13_advance_ref_rename_before_sync_advance_retry_is_present() {
        let fixture = Fixture::new("ref13");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref13-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        branch_repository.create_branch(branch_name, genesis_transaction_id).unwrap();
        let child_transaction_id = fixture.commit_child(43);
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.advance_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, child_transaction_id, NativeRefDurabilityCut::Ref13AdvanceRefRenameBeforeLeafSync);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let ordinary_retry_result = branch_repository.advance_branch(branch_name, genesis_transaction_id, child_transaction_id);
        ::core::assert!(ordinary_retry_result.is_ok());
        let after_ordinary_retry_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_ordinary_retry_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_ordinary_retry_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_eq!(after_first_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_eq!(after_first_recovery_owner_tree_snapshot, after_ordinary_retry_owner_tree_snapshot);
        ::core::assert_eq!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_eq!(after_first_recovery_primary_path_snapshot, after_ordinary_retry_primary_path_snapshot);
        ::core::assert_ne!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(after_first_recovery_secondary_path_snapshot, after_ordinary_retry_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 1_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().verified_ancestry_transactions, 2_u64);
        ::core::assert_eq!(ordinary_retry_result.as_ref().unwrap(), &BranchUpdateStatus::Present);
    }

    #[test]
    fn ref14_advance_ref_sync_before_response_keeps_new_branch() {
        let fixture = Fixture::new("ref14");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref14-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        branch_repository.create_branch(branch_name, genesis_transaction_id).unwrap();
        let child_transaction_id = fixture.commit_child(44);
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.branch.scb1"));
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(::std::format!("{hex}.ref.scb1"));
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.advance_branch_with_native_ref_durability_cut(branch_name, genesis_transaction_id, child_transaction_id, NativeRefDurabilityCut::Ref14AdvanceRefLeafSyncBeforeResponse);
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let second_recovery_result = branch_repository.recover_refs();
        ::core::assert!(second_recovery_result.is_ok());
        let after_second_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_second_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_second_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_eq!(after_first_fault_owner_tree_snapshot, after_second_recovery_owner_tree_snapshot);
        ::core::assert_eq!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_second_recovery_primary_path_snapshot);
        ::core::assert_ne!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_first_fault_secondary_path_snapshot, after_second_recovery_secondary_path_snapshot);
        ::core::assert_eq!(second_recovery_result.as_ref().unwrap().visible_branches, 1_u64);
        ::core::assert_eq!(second_recovery_result.as_ref().unwrap().verified_ancestry_transactions, 2_u64);
    }

    #[test]
    fn ref15_origin_recovery_stage_unlink_retry_syncs_leaf() {
        let fixture = Fixture::new("ref15");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref15-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        branch_repository.create_branch(branch_name, genesis_transaction_id).unwrap();
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(".sley-branch-stage-1-0000000000000001.tmp");
        let secondary_path = owner_root.join("branches").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(".sley-branch-stage-1-0000000000000002.tmp");
        ::std::fs::write(&primary_path, b"partial-a").unwrap();
        ::std::fs::write(&secondary_path, b"partial-b").unwrap();
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.recover_refs_with_native_ref_durability_cut(NativeRefDurabilityCut::Ref15OriginRecoveryStageUnlinkBeforeLeafSync { branch_name: parsed_branch_name.clone() });
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let second_fault_result = branch_repository.recover_refs_with_native_ref_durability_cut(NativeRefDurabilityCut::Ref15OriginRecoveryStageUnlinkBeforeLeafSync { branch_name: parsed_branch_name.clone() });
        let second_fault_error = second_fault_result.expect_err("expected second fault durability error");
        ::core::assert_eq!(second_fault_error.code(), "REF_IO");
        let after_second_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_second_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_second_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_first_fault_owner_tree_snapshot, after_second_fault_owner_tree_snapshot);
        ::core::assert_eq!(after_second_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_second_fault_primary_path_snapshot);
        ::core::assert_eq!(after_second_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_ne!(after_first_fault_secondary_path_snapshot, after_second_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_second_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_branch_stages, 0_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 1_u64);
    }

    #[test]
    fn ref16_visible_ref_recovery_stage_unlink_retry_syncs_leaf() {
        let fixture = Fixture::new("ref16");
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_name = "ref16-branch";
        let parsed_branch_name = BranchName::parse(branch_name).unwrap();
        let hex = hex_digest(&parsed_branch_name.path_key());
        branch_repository.create_branch(branch_name, genesis_transaction_id).unwrap();
        let owner_root = branch_repository.root();
        let primary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(".sley-ref-stage-1-0000000000000001.tmp");
        let secondary_path = owner_root.join("refs").join("v1").join(&hex[0..2]).join(&hex[2..4]).join(".sley-ref-stage-1-0000000000000002.tmp");
        ::std::fs::write(&primary_path, b"partial-a").unwrap();
        ::std::fs::write(&secondary_path, b"partial-b").unwrap();
        let before_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let before_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let before_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_fault_result = branch_repository.recover_refs_with_native_ref_durability_cut(NativeRefDurabilityCut::Ref16VisibleRefRecoveryStageUnlinkBeforeLeafSync { branch_name: parsed_branch_name.clone() });
        let first_fault_error = first_fault_result.expect_err("expected first fault durability error");
        ::core::assert_eq!(first_fault_error.code(), "REF_IO");
        let after_first_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let second_fault_result = branch_repository.recover_refs_with_native_ref_durability_cut(NativeRefDurabilityCut::Ref16VisibleRefRecoveryStageUnlinkBeforeLeafSync { branch_name: parsed_branch_name.clone() });
        let second_fault_error = second_fault_result.expect_err("expected second fault durability error");
        ::core::assert_eq!(second_fault_error.code(), "REF_IO");
        let after_second_fault_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_second_fault_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_second_fault_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        let first_recovery_result = branch_repository.recover_refs();
        ::core::assert!(first_recovery_result.is_ok());
        let after_first_recovery_owner_tree_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let after_first_recovery_primary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&primary_path);
        let after_first_recovery_secondary_path_snapshot = crate::refs::tests::exact_optional_path_snapshot(&secondary_path);
        ::core::assert_ne!(before_fault_owner_tree_snapshot, after_first_fault_owner_tree_snapshot);
        ::core::assert_ne!(after_first_fault_owner_tree_snapshot, after_second_fault_owner_tree_snapshot);
        ::core::assert_eq!(after_second_fault_owner_tree_snapshot, after_first_recovery_owner_tree_snapshot);
        ::core::assert_ne!(before_fault_primary_path_snapshot, after_first_fault_primary_path_snapshot);
        ::core::assert_eq!(after_first_fault_primary_path_snapshot, after_second_fault_primary_path_snapshot);
        ::core::assert_eq!(after_second_fault_primary_path_snapshot, after_first_recovery_primary_path_snapshot);
        ::core::assert_eq!(before_fault_secondary_path_snapshot, after_first_fault_secondary_path_snapshot);
        ::core::assert_ne!(after_first_fault_secondary_path_snapshot, after_second_fault_secondary_path_snapshot);
        ::core::assert_eq!(after_second_fault_secondary_path_snapshot, after_first_recovery_secondary_path_snapshot);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().removed_ref_stages, 0_u64);
        ::core::assert_eq!(first_recovery_result.as_ref().unwrap().visible_branches, 1_u64);
    }

    #[test]
    fn anc02_shared_and_distinct_branch_ancestry_union_is_idempotent() {
        let fixture = Fixture::new("anc02");
        let transaction_repository = ::sley_txn::TransactionRepository::new(fixture.path());
        let branch_repository = super::BranchRepository::new(fixture.path());
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let shared_transaction_id = fixture.commit_child(51);
        let accepted_pointer_path = fixture.path().join("heads").join("accepted");
        let shared_pointer_bytes = ::std::fs::read(&accepted_pointer_path).unwrap();
        let alpha_transaction_id = fixture.commit_child(52);
        ::std::fs::write(&accepted_pointer_path, &shared_pointer_bytes).unwrap();
        let beta_transaction_id = fixture.commit_child(53);
        let alpha_branch_name = MappedBranchName(BranchName::parse("anc02-alpha").unwrap());
        let beta_branch_name = MappedBranchName(BranchName::parse("anc02-beta").unwrap());
        branch_repository.create_branch(&alpha_branch_name, genesis_transaction_id).unwrap();
        branch_repository.advance_branch(&alpha_branch_name, genesis_transaction_id, shared_transaction_id).unwrap();
        branch_repository.advance_branch(&alpha_branch_name, shared_transaction_id, alpha_transaction_id).unwrap();
        branch_repository.create_branch(&beta_branch_name, genesis_transaction_id).unwrap();
        branch_repository.advance_branch(&beta_branch_name, genesis_transaction_id, shared_transaction_id).unwrap();
        branch_repository.advance_branch(&beta_branch_name, shared_transaction_id, beta_transaction_id).unwrap();
        let genesis_revision = transaction_repository.verified_revision(genesis_transaction_id).unwrap();
        let shared_revision = transaction_repository.verified_revision(shared_transaction_id).unwrap();
        let alpha_revision = transaction_repository.verified_revision(alpha_transaction_id).unwrap();
        let beta_revision = transaction_repository.verified_revision(beta_transaction_id).unwrap();
        let alpha_before = branch_repository.resolve_branch(&alpha_branch_name).unwrap();
        let beta_before = branch_repository.resolve_branch(&beta_branch_name).unwrap();
        let alpha_path = branch_repository.checked_ref_path(&alpha_branch_name).unwrap();
        let beta_path = branch_repository.checked_ref_path(&beta_branch_name).unwrap();
        let maintenance = branch_repository.acquire_exclusive_maintenance().unwrap();
        ::core::assert!(maintenance.is_exclusive());
        ::core::assert!(maintenance.covers(transaction_repository.root()));
        ::core::assert!(maintenance.covers(branch_repository.root()));
        let alpha_before_first_snapshot = crate::refs::tests::exact_path_snapshot(&alpha_path);
        let beta_before_first_snapshot = crate::refs::tests::exact_path_snapshot(&beta_path);
        let first_ref_recovery = branch_repository.recover_refs_with_maintenance(&maintenance).unwrap();
        let alpha_after_first_snapshot = crate::refs::tests::exact_path_snapshot(&alpha_path);
        let beta_after_first_snapshot = crate::refs::tests::exact_path_snapshot(&beta_path);
        let alpha_before_second_snapshot = crate::refs::tests::exact_path_snapshot(&alpha_path);
        let beta_before_second_snapshot = crate::refs::tests::exact_path_snapshot(&beta_path);
        let second_ref_recovery = branch_repository.recover_refs_with_maintenance(&maintenance).unwrap();
        let alpha_after_second_snapshot = crate::refs::tests::exact_path_snapshot(&alpha_path);
        let beta_after_second_snapshot = crate::refs::tests::exact_path_snapshot(&beta_path);
        drop(maintenance);
        let alpha_after = branch_repository.resolve_branch(&alpha_branch_name).unwrap();
        let beta_after = branch_repository.resolve_branch(&beta_branch_name).unwrap();
        ::core::assert_eq!(first_ref_recovery.removed_branch_stages, 0);
        ::core::assert_eq!(first_ref_recovery.removed_ref_stages, 0);
        ::core::assert_eq!(first_ref_recovery.visible_branches, 2);
        ::core::assert!(first_ref_recovery.orphan_origins.is_empty());
        ::core::assert_eq!(first_ref_recovery.verified_ancestry_transactions, 4);
        ::core::assert_eq!(second_ref_recovery.removed_branch_stages, 0);
        ::core::assert_eq!(second_ref_recovery.removed_ref_stages, 0);
        ::core::assert_eq!(second_ref_recovery.visible_branches, 2);
        ::core::assert!(second_ref_recovery.orphan_origins.is_empty());
        ::core::assert_eq!(second_ref_recovery.verified_ancestry_transactions, 4);
        ::core::assert_eq!(alpha_before_first_snapshot, alpha_after_first_snapshot);
        ::core::assert_eq!(alpha_after_first_snapshot, alpha_before_second_snapshot);
        ::core::assert_eq!(alpha_before_second_snapshot, alpha_after_second_snapshot);
        ::core::assert_eq!(beta_before_first_snapshot, beta_after_first_snapshot);
        ::core::assert_eq!(beta_after_first_snapshot, beta_before_second_snapshot);
        ::core::assert_eq!(beta_before_second_snapshot, beta_after_second_snapshot);
        ::core::assert!(genesis_revision.receipt().transaction.record.parent_transaction_ids.is_empty());
        ::core::assert_eq!(shared_revision.receipt().transaction.record.parent_transaction_ids.as_slice(), &[genesis_transaction_id]);
        ::core::assert_eq!(alpha_revision.receipt().transaction.record.parent_transaction_ids.as_slice(), &[shared_transaction_id]);
        ::core::assert_eq!(beta_revision.receipt().transaction.record.parent_transaction_ids.as_slice(), &[shared_transaction_id]);
        ::core::assert_ne!(alpha_transaction_id, beta_transaction_id);
        ::core::assert_eq!(alpha_before.reference.record.head_transaction_id, alpha_transaction_id);
        ::core::assert_eq!(beta_before.reference.record.head_transaction_id, beta_transaction_id);
        ::core::assert_eq!(alpha_after.reference.record.head_transaction_id, alpha_transaction_id);
        ::core::assert_eq!(beta_after.reference.record.head_transaction_id, beta_transaction_id);
    }

    #[test]
    fn anc06_ref_nested_store_before_cycle() {
        let fixture = Fixture::new("s20-530-m2-anc-06-ref-nested-store-before-cycle");
        let transaction_repository: &::sley_txn::TransactionRepository = &fixture.transactions;
        let branch_repository: &super::BranchRepository = &fixture.branches;
        let owner_root = branch_repository.root();
        ::core::assert_eq!(owner_root, fixture.path());
        let m2_fixture = prepare_ref_nested_store_cycle_fixture(&fixture);
        let m2_secondary_locator = m2_fixture.m2_secondary_locator.clone();
        let maintenance = branch_repository.acquire_exclusive_maintenance().unwrap();
        let m2_cycle_pristine_ref_path = branch_repository.checked_ref_path(&m2_fixture.selected_branch_name).unwrap();
        let m2_cycle_pristine_ref_bytes = ::std::fs::read(&m2_cycle_pristine_ref_path).unwrap();
        let m2_cycle_pristine_ref = super::import_branch_ref(&m2_cycle_pristine_ref_bytes).unwrap();
        let m2_cycle_left_transaction_id = m2_cycle_pristine_ref.record.head_transaction_id;
        let m2_cycle_left_verified_revision = transaction_repository.verified_revision_with_maintenance(&maintenance, m2_cycle_left_transaction_id).unwrap();
        let m2_branch_head_transaction_id = m2_cycle_left_transaction_id;
        let m2_secondary_cycle_entry_transaction_id = m2_cycle_left_transaction_id;
        let m2_cycle_left_parent_transaction_ids = m2_cycle_left_verified_revision.receipt().transaction.record.parent_transaction_ids.clone();
        ::core::assert_eq!(("m2_cycle_left_has_one_parent", m2_cycle_left_parent_transaction_ids.len()), ("m2_cycle_left_has_one_parent", 1_usize));
        let m2_cycle_right_transaction_id = m2_cycle_left_parent_transaction_ids[0];
        let m2_cycle_right_verified_revision = transaction_repository.verified_revision_with_maintenance(&maintenance, m2_cycle_right_transaction_id).unwrap();
        let m2_cycle_right_parent_transaction_ids = m2_cycle_right_verified_revision.receipt().transaction.record.parent_transaction_ids.clone();
        ::core::assert_eq!(("m2_cycle_right_has_one_parent", m2_cycle_right_parent_transaction_ids.len()), ("m2_cycle_right_has_one_parent", 1_usize));
        let m2_cycle_genesis_transaction_id = m2_cycle_right_parent_transaction_ids[0];
        let m2_cycle_genesis_verified_revision = transaction_repository.verified_revision_with_maintenance(&maintenance, m2_cycle_genesis_transaction_id).unwrap();
        ::core::assert!(m2_cycle_genesis_verified_revision.receipt().transaction.record.parent_transaction_ids.is_empty(), "m2_cycle_genesis_has_no_parents");
        ::core::assert_eq!(("m2_cycle_genesis_kind", m2_cycle_genesis_verified_revision.receipt().transaction.record.transaction_kind), ("m2_cycle_genesis_kind", ::sley_txn::TransactionKind::TrustedGenesis));
        ::core::assert_ne!(("m2_cycle_left_right_distinct_durable", m2_cycle_left_transaction_id), ("m2_cycle_left_right_distinct_durable", m2_cycle_right_transaction_id));
        ::core::assert_ne!(("m2_cycle_left_genesis_distinct_durable", m2_cycle_left_transaction_id), ("m2_cycle_left_genesis_distinct_durable", m2_cycle_genesis_transaction_id));
        ::core::assert_ne!(("m2_cycle_right_genesis_distinct_durable", m2_cycle_right_transaction_id), ("m2_cycle_right_genesis_distinct_durable", m2_cycle_genesis_transaction_id));
        let m2_cycle_right_changed_object_ids = m2_cycle_right_verified_revision.receipt().transaction.record.changed_entity_bindings.iter().filter_map(|binding| binding.postimage).collect::<::std::vec::Vec<_>>();
        ::core::assert_eq!(("m2_cycle_right_has_one_changed_postimage", m2_cycle_right_changed_object_ids.len()), ("m2_cycle_right_has_one_changed_postimage", 1_usize));
        let m2_cycle_right_changed_object_id = m2_cycle_right_changed_object_ids[0];
        ::core::assert!(m2_cycle_right_verified_revision.receipt().record.object_manifest.iter().any(|entry| entry.object_id == m2_cycle_right_changed_object_id), "m2_cycle_right_manifest_contains_changed_postimage");
        let m2_cycle_left_changed_object_ids = m2_cycle_left_verified_revision.receipt().transaction.record.changed_entity_bindings.iter().filter_map(|binding| binding.postimage).collect::<::std::vec::Vec<_>>();
        ::core::assert!(!m2_cycle_left_changed_object_ids.contains(&m2_cycle_right_changed_object_id), "m2_cycle_left_excludes_right_changed_postimage");
        let m2_primary_object_id = m2_cycle_right_changed_object_id;
        let m2_primary_path = ::sley_store::ObjectStore::new(owner_root.to_path_buf()).object_path(m2_primary_object_id);
        let m2_pristine_primary_bytes = ::std::fs::read(&m2_primary_path).unwrap();
        let m2_pristine_primary_object_bytes = m2_pristine_primary_bytes.clone();
        let m2_primary_fault_transaction_id = ::core::option::Option::Some(m2_cycle_right_transaction_id);
        let m2_primary_fault_node: ::core::option::Option<&'static str> = ::core::option::Option::Some("depth_one_right_ancestor_object");
        let m2_cycle_expected_plan_digest = crate::refs::tests::expected_recovery_ancestry_test_plan_digest(owner_root, ::sley_txn::recovery_ancestry_test_hook::RecoveryAncestryTestEpochs::Two, m2_cycle_left_transaction_id, m2_cycle_right_transaction_id);
        ::core::assert_eq!(("m2_cycle_entry_is_left", m2_secondary_cycle_entry_transaction_id), ("m2_cycle_entry_is_left", m2_cycle_left_transaction_id));
        ::core::assert_ne!(("m2_cycle_left_right_distinct", m2_cycle_left_transaction_id), ("m2_cycle_left_right_distinct", m2_cycle_right_transaction_id));
        ::core::assert_eq!(("m2_cycle_primary_origin_contract", m2_primary_fault_transaction_id), ("m2_cycle_primary_origin_contract", ::core::option::Option::Some(m2_cycle_right_transaction_id)));
        ::core::assert_eq!(("m2_cycle_primary_fault_node", m2_primary_fault_node), ("m2_cycle_primary_fault_node", ::core::option::Option::Some("depth_one_right_ancestor_object")));
        let m2_cycle_plan_identity = ::sley_txn::recovery_ancestry_test_hook::install(transaction_repository, &maintenance, ::sley_txn::recovery_ancestry_test_hook::RecoveryAncestryTestEpochs::Two, m2_cycle_left_transaction_id, m2_cycle_right_transaction_id).unwrap();
        ::core::assert_eq!(("m2_cycle_plan_owner_root", m2_cycle_plan_identity.owner_root()), ("m2_cycle_plan_owner_root", owner_root));
        ::core::assert_eq!(("m2_cycle_plan_digest", m2_cycle_plan_identity.digest()), ("m2_cycle_plan_digest", m2_cycle_expected_plan_digest));
        let m2_secondary_cycle_descriptor = [(m2_cycle_left_transaction_id, m2_cycle_right_transaction_id), (m2_cycle_right_transaction_id, m2_cycle_left_transaction_id)];
        let m2_secondary_logical_edges = m2_secondary_cycle_descriptor.to_vec();
        let m2_secondary_only_owner_tree = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_pristine_primary_observation = observe_ref_nested_store_cycle_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_only_observation = observe_ref_nested_store_cycle_fixture_secondary(&fixture, &m2_fixture);
        let m2_primary_activation = activate_ref_nested_store_cycle_fixture_primary(&fixture, &m2_fixture, &m2_primary_path);
        let m2_primary_locator = m2_primary_activation.m2_primary_locator.clone();
        let m2_primary_object_bytes = m2_primary_activation.m2_primary_object_bytes.clone();
        ::core::assert_eq!(("primary_object_identity_from_depth_one_cycle_right", m2_primary_object_id), ("primary_object_identity_from_depth_one_cycle_right", m2_cycle_right_changed_object_id));
        ::core::assert_ne!(("primary_object_bytes_corrupt", m2_primary_object_bytes.as_slice()), ("primary_object_bytes_corrupt", m2_pristine_primary_object_bytes.as_slice()));
        ::core::assert_eq!(("secondary_logical_cycle_l_r_l", m2_secondary_logical_edges.as_slice()), ("secondary_logical_cycle_l_r_l", [(m2_cycle_left_transaction_id, m2_cycle_right_transaction_id), (m2_cycle_right_transaction_id, m2_cycle_left_transaction_id)]));
        ::core::assert_eq!(("secondary_branch_ancestry_cycle", m2_secondary_cycle_entry_transaction_id), ("secondary_branch_ancestry_cycle", m2_branch_head_transaction_id));
        ::core::assert_ne!(("artifact_vs_logical_graph", m2_primary_locator.as_str()), ("artifact_vs_logical_graph", m2_secondary_locator.as_str()));
        let m2_primary_probe_before = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_probe_result =
            probe_ref_nested_store_cycle_object_error(&fixture, &m2_fixture);
        let m2_primary_probe_error = m2_primary_probe_result.expect_err("expected primary multifault probe error");
        let m2_primary_probe_after = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(("primary_probe_scb_digest_mismatch", m2_primary_probe_error.code()), ("primary_probe_scb_digest_mismatch", "SCB_DIGEST_MISMATCH"));
        ::core::assert_eq!(m2_primary_probe_before, m2_primary_probe_after);
        let m2_secondary_probe_before = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_secondary_probe_after = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(m2_secondary_probe_before, m2_secondary_probe_after);
        let m2_receiver_identity_1 = branch_repository.root().to_path_buf();
        let m2_owner_root_1 = owner_root.to_path_buf();
        let m2_guard_identity_1 = maintenance.repository_root().to_path_buf();
        let m2_arguments_1 = m2_fixture.m2_arguments_1.clone();
        let m2_before_1 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_before_1 = observe_ref_nested_store_cycle_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_before_1 = observe_ref_nested_store_cycle_fixture_secondary(&fixture, &m2_fixture);
        let m2_result_1 = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(m2_result_1.is_err());
        let m2_error_1 = m2_result_1.expect_err("expected multifault precedence winner");
        let m2_after_1 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_after_1 = observe_ref_nested_store_cycle_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_after_1 = observe_ref_nested_store_cycle_fixture_secondary(&fixture, &m2_fixture);
        ::core::assert_eq!(("m2_operation_1_code", m2_error_1.code()), ("m2_operation_1_code", "SCB_DIGEST_MISMATCH"));
        ::core::assert!(::core::matches!(&m2_error_1, super::BranchError::Transaction(::sley_txn::CommitError::Store(_))), "m2_operation_1_variant");
        ::core::assert_eq!(("m2_operation_1_source_chain", crate::refs::tests::exact_error_source_chain(&m2_error_1)), ("m2_operation_1_source_chain", ["CommitError", "StoreError"]));
        ::core::assert_eq!(m2_before_1, m2_after_1);
        ::core::assert_eq!(m2_primary_before_1, m2_primary_after_1);
        ::core::assert_eq!(m2_secondary_before_1, m2_secondary_after_1);
        ::std::fs::write(&m2_primary_path, &m2_pristine_primary_bytes).unwrap();
        ::std::fs::File::open(&m2_primary_path).unwrap().sync_all().unwrap();
        super::sync_dir(m2_primary_path.parent().unwrap()).unwrap();
        let m2_primary_after_repair = observe_ref_nested_store_cycle_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_after_repair = observe_ref_nested_store_cycle_fixture_secondary(&fixture, &m2_fixture);
        let m2_after_repair = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(m2_primary_after_repair, m2_pristine_primary_observation);
        ::core::assert_eq!(m2_secondary_after_repair, m2_secondary_only_observation);
        ::core::assert_eq!(m2_after_repair, m2_secondary_only_owner_tree);
        let m2_receiver_identity_2 = branch_repository.root().to_path_buf();
        let m2_owner_root_2 = owner_root.to_path_buf();
        let m2_guard_identity_2 = maintenance.repository_root().to_path_buf();
        let m2_arguments_2 = m2_fixture.m2_arguments_2.clone();
        let m2_before_2 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_before_2 = observe_ref_nested_store_cycle_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_before_2 = observe_ref_nested_store_cycle_fixture_secondary(&fixture, &m2_fixture);
        ::core::assert_eq!(m2_receiver_identity_1, m2_receiver_identity_2);
        ::core::assert_eq!(m2_owner_root_1, m2_owner_root_2);
        ::core::assert_eq!(m2_guard_identity_1, m2_guard_identity_2);
        ::core::assert_eq!(m2_arguments_1, m2_arguments_2);
        let m2_result_2 = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(m2_result_2.is_err());
        let m2_error_2 = m2_result_2.expect_err("expected multifault precedence loser");
        let m2_after_2 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_after_2 = observe_ref_nested_store_cycle_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_after_2 = observe_ref_nested_store_cycle_fixture_secondary(&fixture, &m2_fixture);
        ::core::assert_eq!(("m2_operation_2_code", m2_error_2.code()), ("m2_operation_2_code", "BRANCH_ANCESTRY_CYCLE"));
        ::core::assert!(::core::matches!(&m2_error_2, super::BranchError::Branch(_)), "m2_operation_2_variant");
        ::core::assert_eq!(("m2_operation_2_source_chain", crate::refs::tests::exact_error_source_chain(&m2_error_2)), ("m2_operation_2_source_chain", []));
        ::core::assert_eq!(m2_before_2, m2_after_2);
        ::core::assert_eq!(m2_primary_before_2, m2_primary_after_2);
        ::core::assert_eq!(m2_secondary_before_2, m2_secondary_after_2);
        let m2_cycle_drain = ::sley_txn::recovery_ancestry_test_hook::take(transaction_repository, &maintenance, m2_cycle_plan_identity).unwrap();
        ::core::assert_eq!(("m2_cycle_epoch_budget", m2_cycle_drain.epoch_budget()), ("m2_cycle_epoch_budget", 2_u64));
        ::core::assert_eq!(("m2_cycle_operation_1_claims", m2_cycle_drain.operation_1().claims()), ("m2_cycle_operation_1_claims", 1_u64));
        ::core::assert_eq!(("m2_cycle_operation_1_edge_counts", m2_cycle_drain.operation_1().edge_counts()), ("m2_cycle_operation_1_edge_counts", (1_u64, 0_u64)));
        ::core::assert_eq!(("m2_cycle_operation_2_claims", m2_cycle_drain.operation_2().claims()), ("m2_cycle_operation_2_claims", 1_u64));
        ::core::assert_eq!(("m2_cycle_operation_2_edge_counts", m2_cycle_drain.operation_2().edge_counts()), ("m2_cycle_operation_2_edge_counts", (1_u64, 1_u64)));
        ::core::assert_eq!(("m2_cycle_epochs_exhausted", m2_cycle_drain.remaining_epochs()), ("m2_cycle_epochs_exhausted", 0_u64));
    }

    #[test]
    fn anc06_ref_nested_codec_before_claim_mismatch() {
        let fixture = Fixture::new("s20-530-m2-anc-06-ref-nested-codec-before-claim-mismatch");
        let transaction_repository: &::sley_txn::TransactionRepository = &fixture.transactions;
        let branch_repository: &super::BranchRepository = &fixture.branches;
        let owner_root = branch_repository.root();
        ::core::assert_eq!(owner_root, fixture.path());
        let m2_fixture = prepare_ref_nested_codec_origin_fixture(&fixture);
        let m2_primary_locator = m2_fixture.m2_primary_locator.clone();
        let m2_primary_nested_state_root_bytes = m2_fixture.m2_primary_nested_state_root_bytes.clone();
        let m2_primary_path = m2_fixture.m2_primary_path.clone();
        let m2_pristine_nested_state_root_bytes = m2_fixture.m2_pristine_nested_state_root_bytes.clone();
        let m2_pristine_primary_bytes = m2_fixture.m2_pristine_primary_bytes.clone();
        let m2_pristine_primary_observation = m2_fixture.m2_pristine_primary_observation.clone();
        let m2_ref_head_receipt_path = m2_fixture.m2_ref_head_receipt_path.clone();
        let m2_secondary_head_transaction_id = m2_fixture.m2_secondary_head_transaction_id.clone();
        let m2_secondary_locator = m2_fixture.m2_secondary_locator.clone();
        let m2_secondary_only_observation = m2_fixture.m2_secondary_only_observation.clone();
        let m2_secondary_only_owner_tree = m2_fixture.m2_secondary_only_owner_tree.clone();
        let m2_secondary_origin_transaction_id = m2_fixture.m2_secondary_origin_transaction_id.clone();
        let m2_secondary_reachable_transaction_ids = m2_fixture.m2_secondary_reachable_transaction_ids.clone();
        let maintenance = branch_repository.acquire_exclusive_maintenance().unwrap();
        ::core::assert_eq!(("primary_receipt_path_from_ref_head", m2_primary_path.as_path()), ("primary_receipt_path_from_ref_head", m2_ref_head_receipt_path.as_path()));
        ::core::assert_ne!(("primary_nested_state_root_corruption_present", m2_primary_nested_state_root_bytes.as_slice()), ("primary_nested_state_root_corruption_present", m2_pristine_nested_state_root_bytes.as_slice()));
        ::core::assert_ne!(("secondary_origin_head_distinct", m2_secondary_origin_transaction_id), ("secondary_origin_head_distinct", m2_secondary_head_transaction_id));
        ::core::assert!(!m2_secondary_reachable_transaction_ids.contains(&m2_secondary_origin_transaction_id), "secondary_origin_not_reachable_from_head");
        ::core::assert_ne!(("artifact_vs_origin_relation", m2_primary_locator.as_str()), ("artifact_vs_origin_relation", m2_secondary_locator.as_str()));
        let m2_primary_probe_before = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_probe_result =
            probe_ref_nested_codec_origin_receipt_error(&fixture, &m2_fixture);
        let m2_primary_probe_error = m2_primary_probe_result.expect_err("expected primary multifault probe error");
        let m2_primary_probe_after = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(("primary_probe_scb_digest_mismatch", m2_primary_probe_error.code()), ("primary_probe_scb_digest_mismatch", "SCB_DIGEST_MISMATCH"));
        ::core::assert_eq!(m2_primary_probe_before, m2_primary_probe_after);
        let m2_secondary_probe_before = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_secondary_probe_result = probe_branch_origin_ancestry(&fixture, &m2_fixture);
        let m2_secondary_probe_error = m2_secondary_probe_result.expect_err("expected secondary multifault probe error");
        let m2_secondary_probe_after = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(("secondary_probe_branch_origin_mismatch", m2_secondary_probe_error.code()), ("secondary_probe_branch_origin_mismatch", "BRANCH_ORIGIN_MISMATCH"));
        ::core::assert_eq!(m2_secondary_probe_before, m2_secondary_probe_after);
        let m2_receiver_identity_1 = branch_repository.root().to_path_buf();
        let m2_owner_root_1 = owner_root.to_path_buf();
        let m2_guard_identity_1 = maintenance.repository_root().to_path_buf();
        let m2_arguments_1 = m2_fixture.m2_arguments_1.clone();
        let m2_before_1 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_before_1 = observe_ref_nested_codec_origin_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_before_1 = observe_ref_nested_codec_origin_fixture_secondary(&fixture, &m2_fixture);
        let m2_result_1 = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(m2_result_1.is_err());
        let m2_error_1 = m2_result_1.expect_err("expected multifault precedence winner");
        let m2_after_1 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_after_1 = observe_ref_nested_codec_origin_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_after_1 = observe_ref_nested_codec_origin_fixture_secondary(&fixture, &m2_fixture);
        ::core::assert_eq!(("m2_operation_1_code", m2_error_1.code()), ("m2_operation_1_code", "SCB_DIGEST_MISMATCH"));
        ::core::assert!(::core::matches!(&m2_error_1, super::BranchError::Transaction(::sley_txn::CommitError::Codec(::sley_txn::TransactionCodecError::StateRoot(::sley_state_root::StateRootError::Scb(_))))), "m2_operation_1_variant");
        ::core::assert_eq!(("m2_operation_1_source_chain", crate::refs::tests::exact_error_source_chain(&m2_error_1)), ("m2_operation_1_source_chain", ["CommitError", "TransactionCodecError"]));
        ::core::assert_eq!(m2_before_1, m2_after_1);
        ::core::assert_eq!(m2_primary_before_1, m2_primary_after_1);
        ::core::assert_eq!(m2_secondary_before_1, m2_secondary_after_1);
        ::std::fs::write(&m2_primary_path, &m2_pristine_primary_bytes).unwrap();
        ::std::fs::File::open(&m2_primary_path).unwrap().sync_all().unwrap();
        super::sync_dir(m2_primary_path.parent().unwrap()).unwrap();
        let m2_primary_after_repair = observe_ref_nested_codec_origin_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_after_repair = observe_ref_nested_codec_origin_fixture_secondary(&fixture, &m2_fixture);
        let m2_after_repair = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(m2_primary_after_repair, m2_pristine_primary_observation);
        ::core::assert_eq!(m2_secondary_after_repair, m2_secondary_only_observation);
        ::core::assert_eq!(m2_after_repair, m2_secondary_only_owner_tree);
        let m2_receiver_identity_2 = branch_repository.root().to_path_buf();
        let m2_owner_root_2 = owner_root.to_path_buf();
        let m2_guard_identity_2 = maintenance.repository_root().to_path_buf();
        let m2_arguments_2 = m2_fixture.m2_arguments_2.clone();
        let m2_before_2 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_before_2 = observe_ref_nested_codec_origin_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_before_2 = observe_ref_nested_codec_origin_fixture_secondary(&fixture, &m2_fixture);
        ::core::assert_eq!(m2_receiver_identity_1, m2_receiver_identity_2);
        ::core::assert_eq!(m2_owner_root_1, m2_owner_root_2);
        ::core::assert_eq!(m2_guard_identity_1, m2_guard_identity_2);
        ::core::assert_eq!(m2_arguments_1, m2_arguments_2);
        let m2_result_2 = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(m2_result_2.is_err());
        let m2_error_2 = m2_result_2.expect_err("expected multifault precedence loser");
        let m2_after_2 = crate::refs::tests::exact_tree_snapshot(owner_root);
        let m2_primary_after_2 = observe_ref_nested_codec_origin_fixture_primary(&fixture, &m2_fixture);
        let m2_secondary_after_2 = observe_ref_nested_codec_origin_fixture_secondary(&fixture, &m2_fixture);
        ::core::assert_eq!(("m2_operation_2_code", m2_error_2.code()), ("m2_operation_2_code", "BRANCH_ORIGIN_MISMATCH"));
        ::core::assert!(::core::matches!(&m2_error_2, super::BranchError::Branch(_)), "m2_operation_2_variant");
        ::core::assert_eq!(("m2_operation_2_source_chain", crate::refs::tests::exact_error_source_chain(&m2_error_2)), ("m2_operation_2_source_chain", []));
        ::core::assert_eq!(m2_before_2, m2_after_2);
        ::core::assert_eq!(m2_primary_before_2, m2_primary_after_2);
        ::core::assert_eq!(m2_secondary_before_2, m2_secondary_after_2);
    }

    #[test]
    fn anc06_partial_ref_nested_codec_deep_ancestor_fails_closed() {
        let fixture = Fixture::new("anc06-codec");
        let deep_transaction_id = fixture.commit_child(61);
        let parent_transaction_id = fixture.commit_child(62);
        let head_transaction_id = fixture.commit_child(63);
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_repository = super::BranchRepository::new(fixture.path());
        branch_repository.create_branch("anc06-codec-branch", genesis_transaction_id).unwrap();
        branch_repository.advance_branch("anc06-codec-branch", genesis_transaction_id, deep_transaction_id).unwrap();
        branch_repository.advance_branch("anc06-codec-branch", deep_transaction_id, parent_transaction_id).unwrap();
        branch_repository.advance_branch("anc06-codec-branch", parent_transaction_id, head_transaction_id).unwrap();
        let deep_revision = fixture.transactions.verified_revision(deep_transaction_id).unwrap();
        ::std::fs::write(transaction_receipt_path(fixture.path(), deep_transaction_id), corrupt_nested_receipt(deep_revision.receipt(), &deep_revision.receipt().record.stored_state_root)).unwrap();
        let owner_root = branch_repository.root();
        let maintenance = branch_repository.acquire_exclusive_maintenance().unwrap();
        let owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let result = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(error.code(), "SCB_DIGEST_MISMATCH");
        ::core::assert!(::core::matches!(&error, super::BranchError::Transaction(::sley_txn::CommitError::Codec(::sley_txn::TransactionCodecError::StateRoot(::sley_state_root::StateRootError::Scb(_))))));
        ::core::assert_eq!(crate::refs::tests::exact_error_source_chain(&error), ["CommitError", "TransactionCodecError"]);
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
    }

    #[test]
    fn anc06_partial_ref_nested_store_deep_ancestor_fails_closed() {
        let fixture = Fixture::new("anc06-store");
        let deep_transaction_id = fixture.commit_child(61);
        let parent_transaction_id = fixture.commit_child(62);
        let head_transaction_id = fixture.commit_child(63);
        let genesis_transaction_id = fixture.genesis_transaction_id;
        let branch_repository = super::BranchRepository::new(fixture.path());
        branch_repository.create_branch("anc06-store-branch", genesis_transaction_id).unwrap();
        branch_repository.advance_branch("anc06-store-branch", genesis_transaction_id, deep_transaction_id).unwrap();
        branch_repository.advance_branch("anc06-store-branch", deep_transaction_id, parent_transaction_id).unwrap();
        branch_repository.advance_branch("anc06-store-branch", parent_transaction_id, head_transaction_id).unwrap();
        let deep_revision = fixture.transactions.verified_revision(deep_transaction_id).unwrap();
        let deep_object_id = deep_revision.objects()[0].object_id();
        let deep_object_store = ObjectStore::new(fixture.path());
        let deep_object_path = deep_object_store.object_path(deep_object_id);
        let mut deep_object_bytes = ::std::fs::read(&deep_object_path).unwrap();
        *deep_object_bytes.last_mut().unwrap() ^= 1;
        ::std::fs::write(&deep_object_path, deep_object_bytes).unwrap();
        let owner_root = branch_repository.root();
        let maintenance = branch_repository.acquire_exclusive_maintenance().unwrap();
        let owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let result = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(error.code(), "SCB_DIGEST_MISMATCH");
        ::core::assert!(::core::matches!(&error, super::BranchError::Transaction(::sley_txn::CommitError::Store(_))));
        ::core::assert_eq!(crate::refs::tests::exact_error_source_chain(&error), ["CommitError", "StoreError"]);
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
    }

    #[test]
    fn anc05_deep_missing_branch_ancestor_fails_closed() {
        let fixture = Fixture::new("anc05");
        let (provenance, pointer_path, deep_missing_path) = anc05_provenance(&fixture);
        let branch_repository = super::BranchRepository::new(fixture.path());
        let owner_root = branch_repository.root();
        ::core::assert_eq!(("transaction_ids_distinct", ::std::collections::BTreeSet::from([provenance.genesis_identity, provenance.deep_missing_identity, provenance.direct_parent_identity, provenance.head_identity]).len()), ("transaction_ids_distinct", 4_usize));
        ::core::assert_eq!(("pointer_decodes_head", provenance.pointer_identity), ("pointer_decodes_head", provenance.head_identity));
        ::core::assert_eq!(("head_parent_is_direct_parent", provenance.head_parent_identity), ("head_parent_is_direct_parent", provenance.direct_parent_identity));
        ::core::assert_eq!(("direct_parent_parent_is_deep_missing", provenance.direct_parent_parent_identity), ("direct_parent_parent_is_deep_missing", provenance.deep_missing_identity));
        ::core::assert_eq!(("deep_missing_parent_is_genesis", provenance.deep_missing_parent_identity), ("deep_missing_parent_is_genesis", provenance.genesis_identity));
        ::core::assert_eq!(("deep_missing_is_depth_two", provenance.deep_missing_depth), ("deep_missing_is_depth_two", 2_u64));
        ::core::assert_eq!(("deep_missing_id_derived_from_parent_receipt", provenance.deep_missing_identity), ("deep_missing_id_derived_from_parent_receipt", provenance.derived_deep_missing_identity));
        ::core::assert_eq!(("deep_missing_path_from_derived_id", provenance.deep_missing_receipt_path.as_path()), ("deep_missing_path_from_derived_id", provenance.derived_deep_missing_receipt_path.as_path()));
        let deep_missing_before_snapshot = exact_optional_path_snapshot(&deep_missing_path);
        ::core::assert!(deep_missing_before_snapshot.is_none(), "deep_missing_receipt_absent");
        ::core::assert_eq!(("genesis_receipt_regular", provenance.genesis_receipt_kind), ("genesis_receipt_regular", "regular"));
        ::core::assert_eq!(("direct_parent_receipt_regular", provenance.direct_parent_receipt_kind), ("direct_parent_receipt_regular", "regular"));
        ::core::assert_eq!(("head_receipt_regular", provenance.head_receipt_kind), ("head_receipt_regular", "regular"));
        ::core::assert_eq!(("origin_decodes_genesis", provenance.origin_identity), ("origin_decodes_genesis", provenance.genesis_identity));
        ::core::assert_eq!(("ref_decodes_head", provenance.ref_identity), ("ref_decodes_head", provenance.head_identity));
        ::core::assert_ne!(("origin_head_ids_distinct", provenance.origin_identity), ("origin_head_ids_distinct", provenance.head_identity));
        ::core::assert_eq!(("origin_direct_claim_verified", provenance.origin_claim.clone()), ("origin_direct_claim_verified", provenance.verified_origin_claim.clone()));
        ::core::assert_eq!(("head_direct_claim_verified", provenance.head_claim.clone()), ("head_direct_claim_verified", provenance.verified_head_claim.clone()));
        let pointer_before_snapshot = exact_path_snapshot(&pointer_path);
        let maintenance = branch_repository.acquire_exclusive_maintenance().unwrap();
        ::core::assert!(maintenance.is_exclusive() && maintenance.covers(owner_root), "maintenance_same_root_exclusive");
        let owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let result = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(error.code(), "RECOVERY_RECEIPT_INCOMPLETE");
        ::core::assert!(::core::matches!(&error, super::BranchError::Transaction(::sley_txn::CommitError::Transaction(_))));
        ::core::assert_eq!(crate::refs::tests::exact_error_source_chain(&error), ["CommitError"]);
        let deep_missing_after_snapshot = exact_optional_path_snapshot(&deep_missing_path);
        let pointer_after_snapshot = exact_path_snapshot(&pointer_path);
        ::core::assert_eq!(("pointer_bytes_unchanged", pointer_before_snapshot.2.as_slice()), ("pointer_bytes_unchanged", pointer_after_snapshot.2.as_slice()));
        ::core::assert_eq!(("deep_missing_path_unchanged", deep_missing_before_snapshot), ("deep_missing_path_unchanged", deep_missing_after_snapshot));
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
        ::core::assert_eq!(("owner_tree_unchanged", owner_tree_before_snapshot), ("owner_tree_unchanged", owner_tree_after_snapshot));
    }

    #[test]
    fn guard03_ref_recovery_same_root_shared_fails_closed() {
        let fixture = Fixture::new("guard03");
        let topology = ref_guard_topology(&fixture, ::core::option::Option::None, "guard03-branch", 0xa3);
        let provenance = topology.provenance;
        let transaction_repository = ::sley_txn::TransactionRepository::new(fixture.path());
        let branch_repository = super::BranchRepository::new(fixture.path());
        let owner_root = branch_repository.root();
        let canonical_owner_root = ::std::fs::canonicalize(owner_root).unwrap();
        let loser_probe_error = transaction_repository.verified_revision(provenance.direct_target_identity).unwrap_err();
        let owner_origin_stage_path = topology.owner_origin_stage_path;
        let owner_ref_stage_path = topology.owner_ref_stage_path;
        let authority_pointer_before_snapshot = exact_path_snapshot(&topology.pointer_path);
        let direct_target_receipt_before_snapshot = exact_optional_path_snapshot(&topology.direct_target_receipt_path);
        let owner_origin_stage_before_snapshot = exact_path_snapshot(&owner_origin_stage_path);
        let owner_ref_stage_before_snapshot = exact_path_snapshot(&owner_ref_stage_path);
        let maintenance = branch_repository.acquire_shared_maintenance().unwrap();
        ::core::assert!(::std::fs::symlink_metadata(owner_root).unwrap().is_dir() && !::std::fs::symlink_metadata(owner_root).unwrap().file_type().is_symlink(), "owner_root_real");
        ::core::assert_eq!(("canonical_owner_root", canonical_owner_root.as_path()), ("canonical_owner_root", ::std::fs::canonicalize(owner_root).unwrap().as_path()));
        ::core::assert_eq!(("maintenance_root_equals_canonical_owner", maintenance.repository_root()), ("maintenance_root_equals_canonical_owner", canonical_owner_root.as_path()));
        ::core::assert!(!maintenance.is_exclusive(), "maintenance_is_shared");
        ::core::assert!(maintenance.covers(owner_root), "maintenance_covers_owner");
        ::core::assert_eq!(("direct_target_identity_from_authority", provenance.direct_target_identity), ("direct_target_identity_from_authority", provenance.authority_target_identity));
        ::core::assert_eq!(("direct_target_receipt_path", provenance.direct_target_receipt_path.as_path()), ("direct_target_receipt_path", provenance.expected_direct_target_receipt_path.as_path()));
        ::core::assert!(direct_target_receipt_before_snapshot.is_none(), "direct_target_receipt_absent");
        ::core::assert!(::core::matches!(&loser_probe_error, ::sley_txn::CommitError::Transaction(_)), "loser_probe_variant");
        ::core::assert_eq!(("loser_probe_code", loser_probe_error.code()), ("loser_probe_code", "RECOVERY_RECEIPT_INCOMPLETE"));
        ::core::assert_eq!(("owner_origin_stage_path_from_root", owner_origin_stage_path.as_path()), ("owner_origin_stage_path_from_root", owner_root.join(provenance.owner_origin_stage_relative_path.clone()).as_path()));
        ::core::assert_eq!(("owner_origin_stage_kind_regular", owner_origin_stage_before_snapshot.0), ("owner_origin_stage_kind_regular", "regular"));
        ::core::assert_eq!(("owner_origin_stage_bytes_exact", owner_origin_stage_before_snapshot.2.as_slice()), ("owner_origin_stage_bytes_exact", provenance.owner_origin_stage_expected_bytes.as_slice()));
        ::core::assert_eq!(("owner_ref_stage_path_from_root", owner_ref_stage_path.as_path()), ("owner_ref_stage_path_from_root", owner_root.join(provenance.owner_ref_stage_relative_path.clone()).as_path()));
        ::core::assert_eq!(("owner_ref_stage_kind_regular", owner_ref_stage_before_snapshot.0), ("owner_ref_stage_kind_regular", "regular"));
        ::core::assert_eq!(("owner_ref_stage_bytes_exact", owner_ref_stage_before_snapshot.2.as_slice()), ("owner_ref_stage_bytes_exact", provenance.owner_ref_stage_expected_bytes.as_slice()));
        let owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let result = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(error.code(), "REF_IO");
        ::core::assert!(::core::matches!(&error, super::BranchError::Io(_)));
        ::core::assert_eq!(crate::refs::tests::exact_error_source_chain(&error), ["io::Error(Other)"]);
        let authority_pointer_after_snapshot = exact_path_snapshot(&topology.pointer_path);
        let owner_origin_stage_after_snapshot = exact_path_snapshot(&owner_origin_stage_path);
        let owner_ref_stage_after_snapshot = exact_path_snapshot(&owner_ref_stage_path);
        ::core::assert_eq!(("authority_pointer_unchanged", authority_pointer_before_snapshot), ("authority_pointer_unchanged", authority_pointer_after_snapshot));
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
        ::core::assert_eq!(("owner_tree_unchanged", owner_tree_before_snapshot), ("owner_tree_unchanged", owner_tree_after_snapshot));
        ::core::assert_eq!(("owner_origin_stage_unchanged", owner_origin_stage_before_snapshot), ("owner_origin_stage_unchanged", owner_origin_stage_after_snapshot));
        ::core::assert_eq!(("owner_ref_stage_unchanged", owner_ref_stage_before_snapshot), ("owner_ref_stage_unchanged", owner_ref_stage_after_snapshot));
    }

    #[test]
    fn guard04_ref_recovery_wrong_root_exclusive_fails_closed() {
        let fixture = Fixture::new("guard04");
        let guard_fixture = Fixture::new("guard04-guard");
        let topology = ref_guard_topology(&fixture, ::core::option::Option::Some(&guard_fixture), "guard04-branch", 0xa4);
        let provenance = topology.provenance;
        let transaction_repository = ::sley_txn::TransactionRepository::new(fixture.path());
        let branch_repository = super::BranchRepository::new(fixture.path());
        let guard_repository = super::BranchRepository::new(guard_fixture.path());
        let owner_root = branch_repository.root();
        let guard_root = guard_repository.root();
        let canonical_owner_root = ::std::fs::canonicalize(owner_root).unwrap();
        let canonical_guard_root = ::std::fs::canonicalize(guard_root).unwrap();
        let loser_probe_error = transaction_repository.verified_revision(provenance.direct_target_identity).unwrap_err();
        let owner_origin_stage_path = topology.owner_origin_stage_path;
        let owner_ref_stage_path = topology.owner_ref_stage_path;
        let guard_origin_stage_path = topology.guard_origin_stage_path;
        let guard_ref_stage_path = topology.guard_ref_stage_path;
        let authority_pointer_before_snapshot = exact_path_snapshot(&topology.pointer_path);
        let direct_target_receipt_before_snapshot = exact_optional_path_snapshot(&topology.direct_target_receipt_path);
        let owner_origin_stage_before_snapshot = exact_path_snapshot(&owner_origin_stage_path);
        let owner_ref_stage_before_snapshot = exact_path_snapshot(&owner_ref_stage_path);
        let guard_origin_stage_before_snapshot = exact_path_snapshot(&guard_origin_stage_path);
        let guard_ref_stage_before_snapshot = exact_path_snapshot(&guard_ref_stage_path);
        let maintenance = guard_repository.acquire_exclusive_maintenance().unwrap();
        ::core::assert!(::std::fs::symlink_metadata(owner_root).unwrap().is_dir() && !::std::fs::symlink_metadata(owner_root).unwrap().file_type().is_symlink(), "owner_root_real");
        ::core::assert!(::std::fs::symlink_metadata(guard_root).unwrap().is_dir() && !::std::fs::symlink_metadata(guard_root).unwrap().file_type().is_symlink(), "guard_root_real");
        ::core::assert_eq!(("canonical_owner_root", canonical_owner_root.as_path()), ("canonical_owner_root", ::std::fs::canonicalize(owner_root).unwrap().as_path()));
        ::core::assert_eq!(("canonical_guard_root", canonical_guard_root.as_path()), ("canonical_guard_root", ::std::fs::canonicalize(guard_root).unwrap().as_path()));
        ::core::assert_ne!(("canonical_roots_distinct", canonical_owner_root.as_path()), ("canonical_roots_distinct", canonical_guard_root.as_path()));
        ::core::assert_eq!(("maintenance_root_equals_canonical_guard", maintenance.repository_root()), ("maintenance_root_equals_canonical_guard", canonical_guard_root.as_path()));
        ::core::assert!(maintenance.is_exclusive(), "maintenance_is_exclusive");
        ::core::assert!(maintenance.covers(guard_root), "maintenance_covers_guard");
        ::core::assert!(!maintenance.covers(owner_root), "maintenance_does_not_cover_owner");
        ::core::assert_eq!(("direct_target_identity_from_authority", provenance.direct_target_identity), ("direct_target_identity_from_authority", provenance.authority_target_identity));
        ::core::assert_eq!(("direct_target_receipt_path", provenance.direct_target_receipt_path.as_path()), ("direct_target_receipt_path", provenance.expected_direct_target_receipt_path.as_path()));
        ::core::assert!(direct_target_receipt_before_snapshot.is_none(), "direct_target_receipt_absent");
        ::core::assert!(::core::matches!(&loser_probe_error, ::sley_txn::CommitError::Transaction(_)), "loser_probe_variant");
        ::core::assert_eq!(("loser_probe_code", loser_probe_error.code()), ("loser_probe_code", "RECOVERY_RECEIPT_INCOMPLETE"));
        ::core::assert_eq!(("owner_origin_stage_path_from_root", owner_origin_stage_path.as_path()), ("owner_origin_stage_path_from_root", owner_root.join(provenance.owner_origin_stage_relative_path.clone()).as_path()));
        ::core::assert_eq!(("owner_origin_stage_kind_regular", owner_origin_stage_before_snapshot.0), ("owner_origin_stage_kind_regular", "regular"));
        ::core::assert_eq!(("owner_origin_stage_bytes_exact", owner_origin_stage_before_snapshot.2.as_slice()), ("owner_origin_stage_bytes_exact", provenance.owner_origin_stage_expected_bytes.as_slice()));
        ::core::assert_eq!(("owner_ref_stage_path_from_root", owner_ref_stage_path.as_path()), ("owner_ref_stage_path_from_root", owner_root.join(provenance.owner_ref_stage_relative_path.clone()).as_path()));
        ::core::assert_eq!(("owner_ref_stage_kind_regular", owner_ref_stage_before_snapshot.0), ("owner_ref_stage_kind_regular", "regular"));
        ::core::assert_eq!(("owner_ref_stage_bytes_exact", owner_ref_stage_before_snapshot.2.as_slice()), ("owner_ref_stage_bytes_exact", provenance.owner_ref_stage_expected_bytes.as_slice()));
        ::core::assert_eq!(("guard_origin_stage_path_from_root", guard_origin_stage_path.as_path()), ("guard_origin_stage_path_from_root", guard_root.join(provenance.guard_origin_stage_relative_path.clone()).as_path()));
        ::core::assert_eq!(("guard_origin_stage_kind_regular", guard_origin_stage_before_snapshot.0), ("guard_origin_stage_kind_regular", "regular"));
        ::core::assert_eq!(("guard_origin_stage_bytes_exact", guard_origin_stage_before_snapshot.2.as_slice()), ("guard_origin_stage_bytes_exact", provenance.guard_origin_stage_expected_bytes.as_slice()));
        ::core::assert_eq!(("guard_ref_stage_path_from_root", guard_ref_stage_path.as_path()), ("guard_ref_stage_path_from_root", guard_root.join(provenance.guard_ref_stage_relative_path.clone()).as_path()));
        ::core::assert_eq!(("guard_ref_stage_kind_regular", guard_ref_stage_before_snapshot.0), ("guard_ref_stage_kind_regular", "regular"));
        ::core::assert_eq!(("guard_ref_stage_bytes_exact", guard_ref_stage_before_snapshot.2.as_slice()), ("guard_ref_stage_bytes_exact", provenance.guard_ref_stage_expected_bytes.as_slice()));
        let guard_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(guard_root);
        let owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let result = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let guard_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(guard_root);
        ::core::assert_eq!(error.code(), "REF_IO");
        ::core::assert!(::core::matches!(&error, super::BranchError::Io(_)));
        ::core::assert_eq!(crate::refs::tests::exact_error_source_chain(&error), ["io::Error(Other)"]);
        let authority_pointer_after_snapshot = exact_path_snapshot(&topology.pointer_path);
        let owner_origin_stage_after_snapshot = exact_path_snapshot(&owner_origin_stage_path);
        let owner_ref_stage_after_snapshot = exact_path_snapshot(&owner_ref_stage_path);
        let guard_origin_stage_after_snapshot = exact_path_snapshot(&guard_origin_stage_path);
        let guard_ref_stage_after_snapshot = exact_path_snapshot(&guard_ref_stage_path);
        ::core::assert_eq!(("authority_pointer_unchanged", authority_pointer_before_snapshot), ("authority_pointer_unchanged", authority_pointer_after_snapshot));
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
        ::core::assert_eq!(("owner_tree_unchanged", owner_tree_before_snapshot), ("owner_tree_unchanged", owner_tree_after_snapshot));
        ::core::assert_eq!(("guard_tree_unchanged", guard_tree_before_snapshot), ("guard_tree_unchanged", guard_tree_after_snapshot));
        ::core::assert_eq!(("owner_origin_stage_unchanged", owner_origin_stage_before_snapshot), ("owner_origin_stage_unchanged", owner_origin_stage_after_snapshot));
        ::core::assert_eq!(("owner_ref_stage_unchanged", owner_ref_stage_before_snapshot), ("owner_ref_stage_unchanged", owner_ref_stage_after_snapshot));
        ::core::assert_eq!(("guard_origin_stage_unchanged", guard_origin_stage_before_snapshot), ("guard_origin_stage_unchanged", guard_origin_stage_after_snapshot));
        ::core::assert_eq!(("guard_ref_stage_unchanged", guard_ref_stage_before_snapshot), ("guard_ref_stage_unchanged", guard_ref_stage_after_snapshot));
    }

    #[test]
    fn anc08_branch_ancestry_cycle_fails_closed() {
        let fixture = Fixture::new("anc08");
        let (mut provenance, pointer_path) = anc08_provenance(&fixture);
        let transaction_repository = ::sley_txn::TransactionRepository::new(fixture.path());
        let branch_repository = super::BranchRepository::new(fixture.path());
        let owner_root = branch_repository.root();
        let canonical_owner_root = ::std::fs::canonicalize(owner_root).unwrap();
        let maintenance = branch_repository.acquire_exclusive_maintenance().unwrap();
        let plan_identity = install_recovery_ancestry_l_r_l_test_plan(&transaction_repository, &maintenance, provenance.left_identity, provenance.right_identity);
        provenance.plan_owner_root = plan_identity.owner_root().to_path_buf();
        ::core::assert_eq!(("transaction_ids_distinct", ::std::collections::BTreeSet::from([provenance.genesis_identity, provenance.left_identity, provenance.right_identity]).len()), ("transaction_ids_distinct", 3_usize));
        ::core::assert_eq!(("pointer_decodes_left", provenance.pointer_identity), ("pointer_decodes_left", provenance.left_identity));
        ::core::assert_eq!(("durable_left_parent_is_right", provenance.durable_left_parents.as_slice()), ("durable_left_parent_is_right", [provenance.right_identity].as_slice()));
        ::core::assert_eq!(("durable_right_parent_is_genesis", provenance.durable_right_parents.as_slice()), ("durable_right_parent_is_genesis", [provenance.genesis_identity].as_slice()));
        ::core::assert!(provenance.durable_left_parents.as_slice() == [provenance.right_identity].as_slice() && provenance.durable_right_parents.as_slice() == [provenance.genesis_identity].as_slice(), "durable_graph_acyclic");
        ::core::assert!(maintenance.is_exclusive() && maintenance.covers(owner_root), "maintenance_same_root_exclusive");
        ::core::assert_eq!(("plan_installed_on_owner_repository", provenance.plan_owner_root.as_path()), ("plan_installed_on_owner_repository", canonical_owner_root.as_path()));
        ::core::assert_eq!(("logical_left_parent_is_right", provenance.logical_left_parents.as_slice()), ("logical_left_parent_is_right", [provenance.right_identity].as_slice()));
        ::core::assert_eq!(("logical_right_parent_is_left", provenance.logical_right_parents.as_slice()), ("logical_right_parent_is_left", [provenance.left_identity].as_slice()));
        ::core::assert_eq!(("origin_decodes_genesis", provenance.origin_identity), ("origin_decodes_genesis", provenance.genesis_identity));
        ::core::assert_eq!(("ref_decodes_left", provenance.ref_identity), ("ref_decodes_left", provenance.left_identity));
        ::core::assert_ne!(("origin_left_ids_distinct", provenance.origin_identity), ("origin_left_ids_distinct", provenance.left_identity));
        ::core::assert_eq!(("origin_direct_claim_verified", provenance.origin_claim.clone()), ("origin_direct_claim_verified", provenance.verified_origin_claim.clone()));
        ::core::assert_eq!(("left_direct_claim_verified", provenance.left_claim.clone()), ("left_direct_claim_verified", provenance.verified_left_claim.clone()));
        let pointer_before_snapshot = exact_path_snapshot(&pointer_path);
        let owner_tree_before_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        let result = branch_repository.recover_refs_with_maintenance(&maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::refs::tests::exact_tree_snapshot(owner_root);
        ::core::assert_eq!(error.code(), "BRANCH_ANCESTRY_CYCLE");
        ::core::assert!(::core::matches!(&error, super::BranchError::Branch(_)));
        ::core::assert_eq!(
            crate::refs::tests::exact_error_source_chain(&error),
            [] as [&str; 0]
        );
        let (cycle_observations, plan_consumption_counts) = consumed_l_r_l_cycle_observations(&transaction_repository, &maintenance, plan_identity, &provenance);
        provenance.plan_consumption_counts = plan_consumption_counts;
        let pointer_after_snapshot = exact_path_snapshot(&pointer_path);
        ::core::assert_eq!(("observed_left_durable_and_logical_edges", cycle_observations[0].clone()), ("observed_left_durable_and_logical_edges", (provenance.left_identity, ::std::vec![provenance.right_identity], ::std::vec![provenance.right_identity])));
        ::core::assert_eq!(("observed_right_durable_and_logical_edges", cycle_observations[1].clone()), ("observed_right_durable_and_logical_edges", (provenance.right_identity, ::std::vec![provenance.genesis_identity], ::std::vec![provenance.left_identity])));
        ::core::assert_eq!(("plan_consumed_exactly_once_per_node", provenance.plan_consumption_counts.as_slice()), ("plan_consumed_exactly_once_per_node", [(provenance.left_identity, 1_u64), (provenance.right_identity, 1_u64)].as_slice()));
        ::core::assert_eq!(("pointer_bytes_unchanged", pointer_before_snapshot.2.as_slice()), ("pointer_bytes_unchanged", pointer_after_snapshot.2.as_slice()));
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
        ::core::assert_eq!(("owner_tree_unchanged", owner_tree_before_snapshot), ("owner_tree_unchanged", owner_tree_after_snapshot));
        ::core::assert!(::core::matches!(&error, super::BranchError::Branch(_)), "production_core_binding");
    }
}
