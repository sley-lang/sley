use core::fmt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::{
    Arc, Mutex,
    mpsc::{Receiver, SyncSender},
};

use sley_id::{ObjectId, StateRoot};
use sley_scb1::ScbError;
use sley_state_root::{
    AcceptedStateRoot, conformance_registry as state_root_registry, import_state_root,
};
use sley_store::{CanonicalVerifier, ObjectStore, StoreErrorCode};
use sley_txn::{
    RepositoryMaintenanceGuard, acquire_exclusive_repository_maintenance,
    initialize_repository_maintenance,
};

const GC_LOCK_BYTES: &[u8; 8] = b"SLEYGC01";
const GC_LOCK_DIR: &str = "locks";
const GC_LOCK_FILE: &str = "gc.lock";
const ID_LEN: usize = 32;
const OBJECT_SUFFIX: &str = ".scb1";

#[cfg(test)]
enum GcDurabilityCut {
    Gcw01WitnessCreateBeforeWrite,
    Gcw02DuringWitnessWrite,
    Gcw03WitnessWriteBeforeFileSync,
    Gcw04WitnessFileSyncBeforeLockDirectorySync,
    Gcw05WitnessRemoveBeforeLockDirectorySync,
    Gcw06CollectionOwnsMaintenanceBeforeWitnessAccess { gate: Arc<GcMaintenanceRaceGate> },
    Gc01BeforeSecondCandidateDelete,
    Gc02SecondCandidateUnlinkedBeforeLeafSync,
}

#[cfg(test)]
struct GcMaintenanceRaceGate {
    owner_tx: SyncSender<()>,
    release_rx: Mutex<Receiver<()>>,
}

#[cfg(test)]
impl GcMaintenanceRaceGate {
    fn hold_before_witness_access(&self) {
        ::core::assert!(self.owner_tx.send(()).is_ok());
        ::core::assert!(self.release_rx.lock().unwrap().recv().is_ok());
    }
}

/// Maximum retention anchors in one snapshot.
pub const MAX_GC_ANCHORS: usize = 65_536;
/// Maximum total anchor targets.
pub const MAX_GC_TARGETS: usize = 262_144;
/// Maximum roots in the caller-owned catalog.
pub const MAX_GC_ROOTS: usize = 65_536;
/// Maximum traversed root-dependency edges.
pub const MAX_GC_DEPENDENCY_EDGES: usize = 1_000_000;
/// Maximum traversed object-reference edges.
pub const MAX_GC_OBJECT_REFERENCE_EDGES: usize = 1_000_000;
/// Maximum inventory objects.
pub const MAX_GC_INVENTORY_OBJECTS: usize = 262_144;
/// Maximum ID entries across successful report lists.
pub const MAX_GC_REPORT_ENTRIES: usize = 786_432;
/// Approximate GC-owned allocation budget.
pub const MAX_GC_ALLOCATION: usize = 134_217_728;

/// Stable garbage-collection failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GcErrorCode {
    /// `GC_RESOURCE_LIMIT`.
    ResourceLimit,
    /// `GC_ANCHOR_MALFORMED`.
    AnchorMalformed,
    /// `GC_ANCHOR_UNRESOLVED`.
    AnchorUnresolved,
    /// `GC_ROOT_MISSING`.
    RootMissing,
    /// `GC_ROOT_INVALID`.
    RootInvalid,
    /// `GC_DEPENDENCY_MISSING`.
    DependencyMissing,
    /// `GC_OBJECT_REFERENCE_MALFORMED`.
    ObjectReferenceMalformed,
    /// `GC_OBJECT_MISSING`.
    ObjectMissing,
    /// `GC_INVENTORY_INVALID`.
    InventoryInvalid,
    /// Reserved `GC_DRY_RUN_REQUIRED`.
    DryRunRequired,
    /// `GC_EXCLUSIVE_LOCK_REQUIRED`.
    ExclusiveLockRequired,
    /// `GC_DELETE_IO`.
    DeleteIo,
    /// `GC_REACHABILITY_VIOLATION`.
    ReachabilityViolation,
    /// `GC_INTERNAL_INVARIANT`.
    InternalInvariant,
}

impl GcErrorCode {
    /// Returns the exact stable symbol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourceLimit => "GC_RESOURCE_LIMIT",
            Self::AnchorMalformed => "GC_ANCHOR_MALFORMED",
            Self::AnchorUnresolved => "GC_ANCHOR_UNRESOLVED",
            Self::RootMissing => "GC_ROOT_MISSING",
            Self::RootInvalid => "GC_ROOT_INVALID",
            Self::DependencyMissing => "GC_DEPENDENCY_MISSING",
            Self::ObjectReferenceMalformed => "GC_OBJECT_REFERENCE_MALFORMED",
            Self::ObjectMissing => "GC_OBJECT_MISSING",
            Self::InventoryInvalid => "GC_INVENTORY_INVALID",
            Self::DryRunRequired => "GC_DRY_RUN_REQUIRED",
            Self::ExclusiveLockRequired => "GC_EXCLUSIVE_LOCK_REQUIRED",
            Self::DeleteIo => "GC_DELETE_IO",
            Self::ReachabilityViolation => "GC_REACHABILITY_VIOLATION",
            Self::InternalInvariant => "GC_INTERNAL_INVARIANT",
        }
    }
}

/// Garbage-collection error with an optional partial deletion report.
#[derive(Debug)]
pub struct GcError {
    symbol: &'static str,
    partial_report: Option<Box<GcReport>>,
    source: Option<io::Error>,
}

impl GcError {
    const fn gc(code: GcErrorCode) -> Self {
        Self {
            symbol: code.as_str(),
            partial_report: None,
            source: None,
        }
    }

    const fn upstream(symbol: &'static str) -> Self {
        Self {
            symbol,
            partial_report: None,
            source: None,
        }
    }

    fn io(code: GcErrorCode, error: io::Error) -> Self {
        Self {
            symbol: code.as_str(),
            partial_report: None,
            source: Some(error),
        }
    }

    fn partial(error: io::Error, report: GcReport) -> Self {
        Self {
            symbol: GcErrorCode::DeleteIo.as_str(),
            partial_report: Some(Box::new(report)),
            source: Some(error),
        }
    }

    /// Returns the exact stable failure symbol.
    #[must_use]
    pub const fn symbol(&self) -> &'static str {
        self.symbol
    }

    /// Returns the partial collect report when deletion or sync failed.
    #[must_use]
    pub fn partial_report(&self) -> Option<&GcReport> {
        self.partial_report.as_deref()
    }
}

impl fmt::Display for GcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol)
    }
}

impl std::error::Error for GcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

type Result<T> = core::result::Result<T, GcError>;

/// Closed retention-anchor kind.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RetentionKind {
    /// Caller-resolved retained ref.
    Ref = 1,
    /// Caller-resolved retained tag.
    Tag = 2,
    /// Caller-declared active lease.
    Lease = 3,
    /// Caller-resolved retained transaction.
    Transaction = 4,
    /// Caller-resolved retained pack manifest.
    PackManifest = 5,
    /// Caller-declared protected root.
    ProtectedRoot = 6,
    /// Caller-declared active session pin.
    SessionPin = 7,
}

/// One exact retention target.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RetentionTarget {
    /// Exact semantic root.
    StateRoot(StateRoot),
    /// Exact immutable object.
    Object(ObjectId),
}

/// One caller-owned explicit retention anchor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionAnchor {
    kind: RetentionKind,
    anchor_id: [u8; ID_LEN],
    targets: Vec<RetentionTarget>,
}

impl RetentionAnchor {
    /// Creates an anchor. Snapshot construction validates target presence and uniqueness.
    #[must_use]
    pub fn new(
        kind: RetentionKind,
        anchor_id: [u8; ID_LEN],
        targets: Vec<RetentionTarget>,
    ) -> Self {
        Self {
            kind,
            anchor_id,
            targets,
        }
    }

    /// Returns the closed anchor kind.
    #[must_use]
    pub const fn kind(&self) -> RetentionKind {
        self.kind
    }

    /// Returns the opaque caller-owned anchor ID.
    #[must_use]
    pub const fn anchor_id(&self) -> &[u8; ID_LEN] {
        &self.anchor_id
    }

    /// Returns canonicalized targets after snapshot construction.
    #[must_use]
    pub fn targets(&self) -> &[RetentionTarget] {
        &self.targets
    }
}

/// Canonical key emitted for an examined anchor.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RetentionAnchorKey {
    /// Closed kind.
    pub kind: RetentionKind,
    /// Opaque caller-owned ID.
    pub anchor_id: [u8; ID_LEN],
}

/// Validated caller-owned retention snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionSnapshot {
    anchors: Vec<RetentionAnchor>,
    roots: Vec<AcceptedStateRoot>,
}

impl RetentionSnapshot {
    /// Canonicalizes unordered anchors/targets/roots and rejects duplicates.
    ///
    /// # Errors
    ///
    /// Returns a stable GC error for empty/duplicate anchors, duplicate targets,
    /// duplicate roots, or closed count limits.
    pub fn new(
        mut anchors: Vec<RetentionAnchor>,
        mut roots: Vec<AcceptedStateRoot>,
    ) -> Result<Self> {
        if anchors.len() > MAX_GC_ANCHORS || roots.len() > MAX_GC_ROOTS {
            return Err(GcError::gc(GcErrorCode::ResourceLimit));
        }
        let mut target_count = 0_usize;
        for anchor in &mut anchors {
            if anchor.targets.is_empty() {
                return Err(GcError::gc(GcErrorCode::AnchorMalformed));
            }
            target_count = target_count
                .checked_add(anchor.targets.len())
                .ok_or_else(|| GcError::gc(GcErrorCode::ResourceLimit))?;
            if target_count > MAX_GC_TARGETS {
                return Err(GcError::gc(GcErrorCode::ResourceLimit));
            }
            anchor.targets.sort_unstable();
            if anchor.targets.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(GcError::gc(GcErrorCode::AnchorMalformed));
            }
        }
        anchors.sort_by_key(|anchor| (anchor.kind, anchor.anchor_id));
        if anchors
            .windows(2)
            .any(|pair| (pair[0].kind, pair[0].anchor_id) == (pair[1].kind, pair[1].anchor_id))
        {
            return Err(GcError::gc(GcErrorCode::AnchorMalformed));
        }
        roots.sort_by_key(|root| root.root);
        if roots.windows(2).any(|pair| pair[0].root == pair[1].root) {
            return Err(GcError::gc(GcErrorCode::RootInvalid));
        }
        Ok(Self { anchors, roots })
    }

    /// Returns canonical anchors.
    #[must_use]
    pub fn anchors(&self) -> &[RetentionAnchor] {
        &self.anchors
    }

    /// Returns canonical root catalog entries.
    #[must_use]
    pub fn roots(&self) -> &[AcceptedStateRoot] {
        &self.roots
    }
}

/// Canonical verifier plus schema-selected object-reference extraction.
pub trait GcObjectVerifier: CanonicalVerifier {
    /// Returns every `ObjectId` referenced by this already bounded standalone record.
    ///
    /// # Errors
    ///
    /// Returns an exact SCB error when the selected reference shape is malformed.
    fn references(&self, record: &[u8]) -> core::result::Result<Vec<ObjectId>, ScbError>;
}

/// Successful GC decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GcDecision {
    /// Mark/report only; no mutation occurred.
    DryRun,
    /// Every planned unreachable object was deleted and synced.
    Collected,
    /// Host deletion or sync failed after zero or more completed deletions.
    PartialDeleteFailure,
}

/// Deterministic machine-readable GC report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GcReport {
    /// Canonical examined anchor keys.
    pub examined_anchors: Vec<RetentionAnchorKey>,
    /// Retained roots including dependency closure.
    pub retained_roots: Vec<StateRoot>,
    /// Complete reachable object closure.
    pub reachable_objects: Vec<ObjectId>,
    /// Complete verified store inventory.
    pub inventory_objects: Vec<ObjectId>,
    /// Verified inventory objects absent from reachability.
    pub deletion_candidates: Vec<ObjectId>,
    /// Aggregate inventory bytes.
    pub inventory_bytes: u64,
    /// Aggregate candidate bytes.
    pub candidate_bytes: u64,
    /// Run decision.
    pub decision: GcDecision,
    /// Successfully deleted and synced objects.
    pub deleted_objects: Vec<ObjectId>,
    /// Object whose delete or directory sync failed.
    pub failed_object: Option<ObjectId>,
}

#[derive(Clone, Debug)]
struct InventoryEntry {
    object_id: ObjectId,
    path: PathBuf,
    bytes: u64,
}

struct Reachability {
    examined_anchors: Vec<RetentionAnchorKey>,
    retained_roots: BTreeSet<StateRoot>,
    reachable_objects: BTreeSet<ObjectId>,
}

/// Result of recovering one exact durable GC witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GcWitnessRecoveryStatus {
    /// Removed a complete exact `SLEYGC01` witness and synced its directory.
    RemovedExact,
    /// Removed an empty or strict-prefix witness and synced its directory.
    RemovedIncomplete,
    /// Observed no witness and synced its directory.
    Absent,
}

/// Exclusive local GC guard for one exact real store root.
#[derive(Debug)]
pub struct ExclusiveGcGuard {
    store_root: PathBuf,
    lock_path: PathBuf,
    maintenance: RepositoryMaintenanceGuard,
    active: bool,
}

impl ExclusiveGcGuard {
    /// Returns the exact canonical store root bound to this guard.
    #[must_use]
    pub fn store_root(&self) -> &Path {
        &self.store_root
    }

    /// Explicitly releases the guard and syncs the lock directory.
    ///
    /// # Errors
    ///
    /// Returns `GC_EXCLUSIVE_LOCK_REQUIRED` when removal or sync fails.
    pub fn release(mut self) -> Result<()> {
        self.release_inner()?;
        self.active = false;
        Ok(())
    }

    fn release_inner(&self) -> Result<()> {
        fs::remove_file(&self.lock_path)
            .and_then(|()| sync_dir(self.lock_path.parent().expect("lock has parent")))
            .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))
    }
}

impl Drop for ExclusiveGcGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_file(&self.lock_path);
            if let Some(parent) = self.lock_path.parent() {
                let _ = sync_dir(parent);
            }
        }
    }
}

/// Fails closed with the S20-390 code `TXN_INCOMPLETE_CLONE` while the store
/// root carries an S20-540 exchange stage marker.
fn require_not_incomplete_clone(root: &Path) -> Result<()> {
    if ::sley_txn::incomplete_clone_marker_present(root)
        .map_err(|error| GcError::upstream(error.code()))?
    {
        return Err(GcError::upstream("TXN_INCOMPLETE_CLONE"));
    }
    Ok(())
}

/// Atomically acquires the local exclusive GC guard.
///
/// The returned guard owns both the durable GC witness and the exclusive
/// repository-maintenance lock. Cooperating transaction and ref operations
/// cannot run until it is released.
///
/// # Errors
///
/// Returns `GC_EXCLUSIVE_LOCK_REQUIRED` for an invalid root, existing guard, or
/// lock creation/sync failure.
pub fn acquire_exclusive_gc(store: &ObjectStore) -> Result<ExclusiveGcGuard> {
    let root_metadata = fs::symlink_metadata(store.root())
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    if !root_metadata.file_type().is_dir() {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    let store_root = fs::canonicalize(store.root())
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    initialize_repository_maintenance(&store_root)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    let maintenance = acquire_exclusive_repository_maintenance(&store_root)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    acquire_exclusive_gc_with_maintenance(store, store_root, maintenance)
}

fn acquire_exclusive_gc_with_maintenance(
    store: &ObjectStore,
    store_root: PathBuf,
    maintenance: RepositoryMaintenanceGuard,
) -> Result<ExclusiveGcGuard> {
    if !maintenance.is_exclusive()
        || !maintenance.covers(&store_root)
        || fs::canonicalize(store.root())
            .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?
            != store_root
    {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    require_not_incomplete_clone(&store_root)?;
    let lock_dir = store_root.join(GC_LOCK_DIR);
    create_real_dir(&store_root, &lock_dir)?;
    let lock_path = lock_dir.join(GC_LOCK_FILE);
    let mut lock = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    if let Err(error) = lock
        .write_all(GC_LOCK_BYTES)
        .and_then(|()| lock.sync_all())
        .and_then(|()| sync_dir(&lock_dir))
    {
        let _ = fs::remove_file(&lock_path);
        let _ = sync_dir(&lock_dir);
        return Err(GcError::io(GcErrorCode::ExclusiveLockRequired, error));
    }
    Ok(ExclusiveGcGuard {
        store_root,
        lock_path,
        maintenance,
        active: true,
    })
}

#[cfg(test)]
fn acquire_exclusive_gc_with_gc_durability_cut(
    store: &ObjectStore,
    cut: &GcDurabilityCut,
) -> Result<ExclusiveGcGuard> {
    let root_metadata = fs::symlink_metadata(store.root())
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    if !root_metadata.file_type().is_dir() {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    let store_root = fs::canonicalize(store.root())
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    initialize_repository_maintenance(&store_root)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    let maintenance = acquire_exclusive_repository_maintenance(&store_root)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;

    if let GcDurabilityCut::Gcw06CollectionOwnsMaintenanceBeforeWitnessAccess { gate } = &cut {
        gate.hold_before_witness_access();
        return acquire_exclusive_gc_with_maintenance(store, store_root, maintenance);
    }

    if !maintenance.is_exclusive()
        || !maintenance.covers(&store_root)
        || fs::canonicalize(store.root())
            .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?
            != store_root
    {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    let lock_dir = store_root.join(GC_LOCK_DIR);
    create_real_dir(&store_root, &lock_dir)?;
    let lock_path = lock_dir.join(GC_LOCK_FILE);
    let mut lock = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    let injected = || {
        GcError::io(
            GcErrorCode::ExclusiveLockRequired,
            io::Error::other("injected GC-witness durability cut"),
        )
    };
    match cut {
        GcDurabilityCut::Gcw01WitnessCreateBeforeWrite => Err(injected()),
        GcDurabilityCut::Gcw02DuringWitnessWrite => {
            let prefix_len = GC_LOCK_BYTES.len() / 2;
            ::core::assert!(prefix_len > 0);
            ::core::assert!(prefix_len < GC_LOCK_BYTES.len());
            lock.write_all(&GC_LOCK_BYTES[..prefix_len])
                .and_then(|()| lock.sync_all())
                .and_then(|()| sync_dir(&lock_dir))
                .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
            Err(injected())
        }
        GcDurabilityCut::Gcw03WitnessWriteBeforeFileSync => {
            lock.write_all(GC_LOCK_BYTES)
                .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
            Err(injected())
        }
        GcDurabilityCut::Gcw04WitnessFileSyncBeforeLockDirectorySync => {
            lock.write_all(GC_LOCK_BYTES)
                .and_then(|()| lock.sync_all())
                .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
            Err(injected())
        }
        GcDurabilityCut::Gcw05WitnessRemoveBeforeLockDirectorySync
        | GcDurabilityCut::Gcw06CollectionOwnsMaintenanceBeforeWitnessAccess { .. }
        | GcDurabilityCut::Gc01BeforeSecondCandidateDelete
        | GcDurabilityCut::Gc02SecondCandidateUnlinkedBeforeLeafSync => {
            unreachable!("durability cut does not belong to GC acquisition")
        }
    }
}

/// Recovers an exact interrupted GC witness under caller-owned exclusive
/// repository maintenance.
///
/// # Errors
///
/// Returns `GC_EXCLUSIVE_LOCK_REQUIRED` for a shared or wrong-root guard, an
/// invalid lock boundary, arbitrary witness bytes, or any witness I/O failure.
pub fn recover_gc_witness(
    store: &ObjectStore,
    maintenance: &RepositoryMaintenanceGuard,
) -> Result<GcWitnessRecoveryStatus> {
    let store_root = validate_gc_recovery_maintenance(store, maintenance)?;
    require_not_incomplete_clone(&store_root)?;
    let lock_dir = store_root.join(GC_LOCK_DIR);
    ensure_real_dir(&lock_dir, GcErrorCode::ExclusiveLockRequired)?;
    let lock_path = lock_dir.join(GC_LOCK_FILE);
    let metadata = match fs::symlink_metadata(&lock_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            sync_dir(&lock_dir)
                .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
            return Ok(GcWitnessRecoveryStatus::Absent);
        }
        Err(error) => {
            return Err(GcError::io(GcErrorCode::ExclusiveLockRequired, error));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    let length = usize::try_from(metadata.len())
        .map_err(|_| GcError::gc(GcErrorCode::ExclusiveLockRequired))?;
    if length > GC_LOCK_BYTES.len() {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    let mut bytes = [0_u8; GC_LOCK_BYTES.len()];
    let mut witness = File::open(&lock_path)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    witness
        .read_exact(&mut bytes[..length])
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    let mut extra = [0_u8; 1];
    if witness
        .read(&mut extra)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?
        != 0
        || bytes[..length] != GC_LOCK_BYTES[..length]
    {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    let status = if length == GC_LOCK_BYTES.len() {
        GcWitnessRecoveryStatus::RemovedExact
    } else {
        GcWitnessRecoveryStatus::RemovedIncomplete
    };
    fs::remove_file(&lock_path)
        .and_then(|()| sync_dir(&lock_dir))
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    Ok(status)
}

#[cfg(test)]
fn recover_gc_witness_with_gc_durability_cut(
    store: &ObjectStore,
    maintenance: &RepositoryMaintenanceGuard,
    cut: &GcDurabilityCut,
) -> Result<GcWitnessRecoveryStatus> {
    match cut {
        GcDurabilityCut::Gcw05WitnessRemoveBeforeLockDirectorySync => {}
        GcDurabilityCut::Gcw01WitnessCreateBeforeWrite
        | GcDurabilityCut::Gcw02DuringWitnessWrite
        | GcDurabilityCut::Gcw03WitnessWriteBeforeFileSync
        | GcDurabilityCut::Gcw04WitnessFileSyncBeforeLockDirectorySync
        | GcDurabilityCut::Gcw06CollectionOwnsMaintenanceBeforeWitnessAccess { .. }
        | GcDurabilityCut::Gc01BeforeSecondCandidateDelete
        | GcDurabilityCut::Gc02SecondCandidateUnlinkedBeforeLeafSync => {
            unreachable!("durability cut does not belong to GC-witness recovery")
        }
    }
    let store_root = validate_gc_recovery_maintenance(store, maintenance)?;
    let lock_dir = store_root.join(GC_LOCK_DIR);
    ensure_real_dir(&lock_dir, GcErrorCode::ExclusiveLockRequired)?;
    let lock_path = lock_dir.join(GC_LOCK_FILE);
    let metadata = fs::symlink_metadata(&lock_path)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    if !metadata.file_type().is_file() || metadata.len() != GC_LOCK_BYTES.len() as u64 {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    let mut bytes = [0_u8; GC_LOCK_BYTES.len()];
    let mut witness = File::open(&lock_path)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    witness
        .read_exact(&mut bytes)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    let mut extra = [0_u8; 1];
    if witness
        .read(&mut extra)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?
        != 0
        || bytes != *GC_LOCK_BYTES
    {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    fs::remove_file(&lock_path)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    Err(GcError::io(
        GcErrorCode::ExclusiveLockRequired,
        io::Error::other("injected GC-witness directory-sync durability cut"),
    ))
}

fn validate_gc_recovery_maintenance(
    store: &ObjectStore,
    maintenance: &RepositoryMaintenanceGuard,
) -> Result<PathBuf> {
    if !maintenance.is_exclusive() || !maintenance.covers(store.root()) {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    let store_root = fs::canonicalize(store.root())
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    if store_root != maintenance.repository_root() {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    Ok(store_root)
}

/// Computes a complete deterministic dry-run report without mutation.
///
/// # Errors
///
/// Fails closed on any malformed/unresolved anchor, root, dependency,
/// reference, inventory entry, object, or resource bound.
pub fn gc_dry_run<V: GcObjectVerifier>(
    store: &ObjectStore,
    snapshot: &RetentionSnapshot,
    verifier: &V,
) -> Result<GcReport> {
    plan_gc(store, snapshot, verifier)
}

/// Replans under the exact exclusive guard and deletes only verified candidates.
///
/// # Errors
///
/// Returns planning errors before deletion. A delete/sync host failure returns
/// `GC_DELETE_IO` with `partial_report()` populated.
pub fn gc_collect<V: GcObjectVerifier>(
    store: &ObjectStore,
    snapshot: &RetentionSnapshot,
    verifier: &V,
    guard: &ExclusiveGcGuard,
) -> Result<GcReport> {
    collect_inner(store, snapshot, verifier, guard)
}

fn plan_gc<V: GcObjectVerifier>(
    store: &ObjectStore,
    snapshot: &RetentionSnapshot,
    verifier: &V,
) -> Result<GcReport> {
    let inventory = inventory(store)?;
    let inventory_ids = inventory
        .iter()
        .map(|entry| entry.object_id)
        .collect::<BTreeSet<_>>();
    let inventory_bytes = inventory.iter().try_fold(0_u64, |total, entry| {
        total
            .checked_add(entry.bytes)
            .ok_or_else(|| GcError::gc(GcErrorCode::ResourceLimit))
    })?;
    let roots = validate_roots(snapshot)?;
    let reachability = resolve_reachability(store, snapshot, verifier, &roots, &inventory_ids)?;
    for entry in &inventory {
        if !reachability.reachable_objects.contains(&entry.object_id) {
            store
                .read(entry.object_id, verifier)
                .map_err(|error| map_store_error(&error))?;
        }
    }
    let deletion_candidates = inventory_ids
        .difference(&reachability.reachable_objects)
        .copied()
        .collect::<Vec<_>>();
    let candidate_set = deletion_candidates.iter().copied().collect::<BTreeSet<_>>();
    let candidate_bytes = inventory.iter().try_fold(0_u64, |total, entry| {
        if candidate_set.contains(&entry.object_id) {
            total
                .checked_add(entry.bytes)
                .ok_or_else(|| GcError::gc(GcErrorCode::ResourceLimit))
        } else {
            Ok(total)
        }
    })?;
    let report_entries = reachability
        .examined_anchors
        .len()
        .checked_add(reachability.retained_roots.len())
        .and_then(|count| count.checked_add(reachability.reachable_objects.len()))
        .and_then(|count| count.checked_add(inventory.len()))
        .and_then(|count| count.checked_add(deletion_candidates.len()))
        .ok_or_else(|| GcError::gc(GcErrorCode::ResourceLimit))?;
    if report_entries > MAX_GC_REPORT_ENTRIES {
        return Err(GcError::gc(GcErrorCode::ResourceLimit));
    }
    check_allocation(report_entries, &inventory)?;
    Ok(GcReport {
        examined_anchors: reachability.examined_anchors,
        retained_roots: reachability.retained_roots.into_iter().collect(),
        reachable_objects: reachability.reachable_objects.into_iter().collect(),
        inventory_objects: inventory_ids.into_iter().collect(),
        deletion_candidates,
        inventory_bytes,
        candidate_bytes,
        decision: GcDecision::DryRun,
        deleted_objects: Vec::new(),
        failed_object: None,
    })
}

fn resolve_reachability<V: GcObjectVerifier>(
    store: &ObjectStore,
    snapshot: &RetentionSnapshot,
    verifier: &V,
    roots: &BTreeMap<StateRoot, AcceptedStateRoot>,
    inventory_ids: &BTreeSet<ObjectId>,
) -> Result<Reachability> {
    let (examined_anchors, root_queue, mut object_queue) =
        seed_anchor_targets(snapshot, roots, inventory_ids)?;
    let retained_roots = traverse_roots(root_queue, &mut object_queue, roots)?;
    let reachable_objects = traverse_objects(store, verifier, inventory_ids, object_queue)?;
    Ok(Reachability {
        examined_anchors,
        retained_roots,
        reachable_objects,
    })
}

fn seed_anchor_targets(
    snapshot: &RetentionSnapshot,
    roots: &BTreeMap<StateRoot, AcceptedStateRoot>,
    inventory_ids: &BTreeSet<ObjectId>,
) -> Result<(
    Vec<RetentionAnchorKey>,
    VecDeque<StateRoot>,
    VecDeque<ObjectId>,
)> {
    let mut examined = Vec::with_capacity(snapshot.anchors.len());
    let mut root_queue = VecDeque::new();
    let mut object_queue = VecDeque::new();
    for anchor in &snapshot.anchors {
        examined.push(RetentionAnchorKey {
            kind: anchor.kind,
            anchor_id: anchor.anchor_id,
        });
        for target in &anchor.targets {
            match target {
                RetentionTarget::StateRoot(root) if roots.contains_key(root) => {
                    root_queue.push_back(*root);
                }
                RetentionTarget::StateRoot(_) => {
                    return Err(GcError::gc(GcErrorCode::RootMissing));
                }
                RetentionTarget::Object(object) if inventory_ids.contains(object) => {
                    object_queue.push_back(*object);
                }
                RetentionTarget::Object(_) => {
                    return Err(GcError::gc(GcErrorCode::AnchorUnresolved));
                }
            }
        }
    }
    Ok((examined, root_queue, object_queue))
}

fn traverse_roots(
    mut queue: VecDeque<StateRoot>,
    object_queue: &mut VecDeque<ObjectId>,
    roots: &BTreeMap<StateRoot, AcceptedStateRoot>,
) -> Result<BTreeSet<StateRoot>> {
    let mut retained = BTreeSet::new();
    let mut edge_count = 0_usize;
    while let Some(root_id) = queue.pop_front() {
        if !retained.insert(root_id) {
            continue;
        }
        let root = roots
            .get(&root_id)
            .ok_or_else(|| GcError::gc(GcErrorCode::DependencyMissing))?;
        for dependency in &root.record.dependency_roots {
            edge_count = edge_count
                .checked_add(1)
                .ok_or_else(|| GcError::gc(GcErrorCode::ResourceLimit))?;
            if edge_count > MAX_GC_DEPENDENCY_EDGES {
                return Err(GcError::gc(GcErrorCode::ResourceLimit));
            }
            if !roots.contains_key(dependency) {
                return Err(GcError::gc(GcErrorCode::DependencyMissing));
            }
            queue.push_back(*dependency);
        }
        object_queue.push_back(root.record.contract_root);
        object_queue.push_back(root.record.test_root);
        object_queue.extend(
            root.record
                .entity_bindings
                .iter()
                .map(|(_, object_id)| *object_id),
        );
    }
    Ok(retained)
}

fn traverse_objects<V: GcObjectVerifier>(
    store: &ObjectStore,
    verifier: &V,
    inventory_ids: &BTreeSet<ObjectId>,
    mut queue: VecDeque<ObjectId>,
) -> Result<BTreeSet<ObjectId>> {
    let mut reachable = BTreeSet::new();
    let mut edge_count = 0_usize;
    while let Some(object_id) = queue.pop_front() {
        if !reachable.insert(object_id) {
            continue;
        }
        if !inventory_ids.contains(&object_id) {
            return Err(GcError::gc(GcErrorCode::ObjectMissing));
        }
        let record = store
            .read(object_id, verifier)
            .map_err(|error| map_store_error(&error))?;
        let mut references = verifier
            .references(&record)
            .map_err(|_| GcError::gc(GcErrorCode::ObjectReferenceMalformed))?;
        references.sort_unstable();
        references.dedup();
        edge_count = edge_count
            .checked_add(references.len())
            .ok_or_else(|| GcError::gc(GcErrorCode::ResourceLimit))?;
        if edge_count > MAX_GC_OBJECT_REFERENCE_EDGES {
            return Err(GcError::gc(GcErrorCode::ResourceLimit));
        }
        for reference in references {
            if !inventory_ids.contains(&reference) {
                return Err(GcError::gc(GcErrorCode::ObjectMissing));
            }
            queue.push_back(reference);
        }
    }
    Ok(reachable)
}

fn validate_roots(snapshot: &RetentionSnapshot) -> Result<BTreeMap<StateRoot, AcceptedStateRoot>> {
    let registry =
        state_root_registry().map_err(|error| GcError::upstream(error.code().as_str()))?;
    let mut roots = BTreeMap::new();
    for candidate in &snapshot.roots {
        let imported = import_state_root(&registry, &candidate.stored_bytes)
            .map_err(|error| GcError::upstream(error.code_str()))?;
        if imported.root != candidate.root || imported.record != candidate.record {
            return Err(GcError::gc(GcErrorCode::RootInvalid));
        }
        if roots.insert(imported.root, imported).is_some() {
            return Err(GcError::gc(GcErrorCode::RootInvalid));
        }
    }
    let mut edge_count = 0_usize;
    for root in roots.values() {
        for dependency in &root.record.dependency_roots {
            edge_count = edge_count
                .checked_add(1)
                .ok_or_else(|| GcError::gc(GcErrorCode::ResourceLimit))?;
            if edge_count > MAX_GC_DEPENDENCY_EDGES {
                return Err(GcError::gc(GcErrorCode::ResourceLimit));
            }
            if !roots.contains_key(dependency) {
                return Err(GcError::gc(GcErrorCode::DependencyMissing));
            }
        }
    }
    Ok(roots)
}

fn inventory(store: &ObjectStore) -> Result<Vec<InventoryEntry>> {
    ensure_real_dir(store.root(), GcErrorCode::InventoryInvalid)?;
    let object_root = store.root().join("objects");
    if !real_dir_or_absent(&object_root)? {
        return Ok(Vec::new());
    }
    let scb1_root = object_root.join("scb1");
    if !real_dir_or_absent(&scb1_root)? {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for first in sorted_entries(&scb1_root)? {
        require_hex_dir(&first, 2)?;
        let first_file_name = first.file_name();
        let first_name = os_text(&first_file_name)?;
        for second in sorted_entries(&first.path())? {
            require_hex_dir(&second, 2)?;
            let second_file_name = second.file_name();
            let second_name = os_text(&second_file_name)?;
            for object in sorted_entries(&second.path())? {
                let file_type = object
                    .file_type()
                    .map_err(|error| GcError::io(GcErrorCode::InventoryInvalid, error))?;
                if !file_type.is_file() {
                    return Err(GcError::gc(GcErrorCode::InventoryInvalid));
                }
                let object_file_name = object.file_name();
                let name = os_text(&object_file_name)?;
                let hex = name
                    .strip_suffix(OBJECT_SUFFIX)
                    .ok_or_else(|| GcError::gc(GcErrorCode::InventoryInvalid))?;
                let object_id = parse_object_id(hex)?;
                if first_name != &hex[0..2] || second_name != &hex[2..4] {
                    return Err(GcError::gc(GcErrorCode::InventoryInvalid));
                }
                if object.path() != store.object_path(object_id) {
                    return Err(GcError::gc(GcErrorCode::InventoryInvalid));
                }
                let metadata = fs::symlink_metadata(object.path())
                    .map_err(|error| GcError::io(GcErrorCode::InventoryInvalid, error))?;
                if !metadata.file_type().is_file() {
                    return Err(GcError::gc(GcErrorCode::InventoryInvalid));
                }
                out.push(InventoryEntry {
                    object_id,
                    path: object.path(),
                    bytes: metadata.len(),
                });
                if out.len() > MAX_GC_INVENTORY_OBJECTS {
                    return Err(GcError::gc(GcErrorCode::ResourceLimit));
                }
            }
        }
    }
    out.sort_by_key(|entry| entry.object_id);
    if out
        .windows(2)
        .any(|pair| pair[0].object_id == pair[1].object_id)
    {
        return Err(GcError::gc(GcErrorCode::InventoryInvalid));
    }
    Ok(out)
}

fn collect_inner<V: GcObjectVerifier>(
    store: &ObjectStore,
    snapshot: &RetentionSnapshot,
    verifier: &V,
    guard: &ExclusiveGcGuard,
) -> Result<GcReport> {
    require_guard(store, guard)?;
    let report = plan_gc(store, snapshot, verifier)?;
    sync_inventory_leaf_directories(store)?;
    delete_planned_candidates(store, verifier, report)
}

/// Deletes the plan's candidates in order. The reachability guard runs
/// before every read and unlink: a candidate the plan also lists as
/// reachable is `GC_REACHABILITY_VIOLATION`, and nothing is deleted for
/// that run. `plan_gc` never produces such a plan, so the guard is the
/// invariant's last line, exercised by the injected-plan test.
fn delete_planned_candidates<V: GcObjectVerifier>(
    store: &ObjectStore,
    verifier: &V,
    mut report: GcReport,
) -> Result<GcReport> {
    let reachable = report
        .reachable_objects
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for object_id in report.deletion_candidates.clone() {
        if reachable.contains(&object_id) {
            return Err(GcError::gc(GcErrorCode::ReachabilityViolation));
        }
        store
            .read(object_id, verifier)
            .map_err(|error| map_store_error(&error))?;
        let path = store.object_path(object_id);
        let metadata = fs::symlink_metadata(&path).map_err(map_delete_io)?;
        if !metadata.file_type().is_file() {
            return Err(GcError::gc(GcErrorCode::InventoryInvalid));
        }
        if let Err(error) = fs::remove_file(&path) {
            report.decision = GcDecision::PartialDeleteFailure;
            report.failed_object = Some(object_id);
            return Err(GcError::partial(error, report));
        }
        if let Err(error) = sync_dir(path.parent().expect("object path has parent")) {
            report.decision = GcDecision::PartialDeleteFailure;
            report.failed_object = Some(object_id);
            return Err(GcError::partial(error, report));
        }
        report.deleted_objects.push(object_id);
    }
    report.decision = GcDecision::Collected;
    Ok(report)
}

/// T40 fault seeding: a plan whose candidate list names a reachable object.
/// The planner cannot produce one, so the only way to reach the guard is to
/// hand the deleter a corrupted plan.
#[cfg(test)]
fn gc_collect_with_injected_reachable_candidate<V: GcObjectVerifier>(
    store: &ObjectStore,
    snapshot: &RetentionSnapshot,
    verifier: &V,
    guard: &ExclusiveGcGuard,
) -> Result<GcReport> {
    require_guard(store, guard)?;
    let mut report = plan_gc(store, snapshot, verifier)?;
    sync_inventory_leaf_directories(store)?;
    let reachable = *report
        .reachable_objects
        .first()
        .ok_or_else(|| GcError::gc(GcErrorCode::InternalInvariant))?;
    report.deletion_candidates.insert(0, reachable);
    delete_planned_candidates(store, verifier, report)
}

#[cfg(test)]
fn gc_collect_with_injected_pre_cleanup_sync_failure<V: GcObjectVerifier>(
    store: &ObjectStore,
    snapshot: &RetentionSnapshot,
    verifier: &V,
    guard: &ExclusiveGcGuard,
) -> Result<GcReport> {
    require_guard(store, guard)?;
    let _report = plan_gc(store, snapshot, verifier)?;
    Err(GcError::io(
        GcErrorCode::DeleteIo,
        io::Error::other("injected pre-cleanup inventory-leaf sync failure"),
    ))
}

#[cfg(test)]
fn gc_collect_with_gc_durability_cut<V: GcObjectVerifier>(
    store: &ObjectStore,
    snapshot: &RetentionSnapshot,
    verifier: &V,
    guard: &ExclusiveGcGuard,
    cut: &GcDurabilityCut,
) -> Result<GcReport> {
    let cut_before_delete = match cut {
        GcDurabilityCut::Gc01BeforeSecondCandidateDelete => true,
        GcDurabilityCut::Gc02SecondCandidateUnlinkedBeforeLeafSync => false,
        GcDurabilityCut::Gcw01WitnessCreateBeforeWrite
        | GcDurabilityCut::Gcw02DuringWitnessWrite
        | GcDurabilityCut::Gcw03WitnessWriteBeforeFileSync
        | GcDurabilityCut::Gcw04WitnessFileSyncBeforeLockDirectorySync
        | GcDurabilityCut::Gcw05WitnessRemoveBeforeLockDirectorySync
        | GcDurabilityCut::Gcw06CollectionOwnsMaintenanceBeforeWitnessAccess { .. } => {
            unreachable!("durability cut does not belong to GC collection")
        }
    };
    require_guard(store, guard)?;
    let mut report = plan_gc(store, snapshot, verifier)?;
    sync_inventory_leaf_directories(store)?;
    if report.deletion_candidates.len() != 3 {
        return Err(GcError::gc(GcErrorCode::InternalInvariant));
    }
    let reachable = report
        .reachable_objects
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for (candidate_index, object_id) in report.deletion_candidates.clone().into_iter().enumerate() {
        if reachable.contains(&object_id) {
            return Err(GcError::gc(GcErrorCode::ReachabilityViolation));
        }
        store
            .read(object_id, verifier)
            .map_err(|error| map_store_error(&error))?;
        let path = store.object_path(object_id);
        let metadata = fs::symlink_metadata(&path).map_err(map_delete_io)?;
        if !metadata.file_type().is_file() {
            return Err(GcError::gc(GcErrorCode::InventoryInvalid));
        }
        if candidate_index == 1 && cut_before_delete {
            report.decision = GcDecision::PartialDeleteFailure;
            report.failed_object = Some(object_id);
            return Err(GcError::partial(
                io::Error::other("injected second-candidate delete failure"),
                report,
            ));
        }
        if let Err(error) = fs::remove_file(&path) {
            report.decision = GcDecision::PartialDeleteFailure;
            report.failed_object = Some(object_id);
            return Err(GcError::partial(error, report));
        }
        if candidate_index == 1 && !cut_before_delete {
            report.decision = GcDecision::PartialDeleteFailure;
            report.failed_object = Some(object_id);
            return Err(GcError::partial(
                io::Error::other("injected second-candidate directory sync failure"),
                report,
            ));
        }
        if let Err(error) = sync_dir(path.parent().expect("object path has parent")) {
            report.decision = GcDecision::PartialDeleteFailure;
            report.failed_object = Some(object_id);
            return Err(GcError::partial(error, report));
        }
        report.deleted_objects.push(object_id);
    }
    report.decision = GcDecision::Collected;
    Ok(report)
}

fn sync_inventory_leaf_directories(store: &ObjectStore) -> Result<()> {
    ensure_real_dir(store.root(), GcErrorCode::InventoryInvalid)?;
    let object_root = store.root().join("objects");
    if !real_dir_or_absent(&object_root)? {
        return Ok(());
    }
    let scb1_root = object_root.join("scb1");
    if !real_dir_or_absent(&scb1_root)? {
        return Ok(());
    }
    for first in sorted_entries(&scb1_root)? {
        require_hex_dir(&first, 2)?;
        for second in sorted_entries(&first.path())? {
            require_hex_dir(&second, 2)?;
            sync_dir(&second.path()).map_err(map_delete_io)?;
        }
    }
    Ok(())
}

fn require_guard(store: &ObjectStore, guard: &ExclusiveGcGuard) -> Result<()> {
    if !guard.active {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    let store_root = fs::canonicalize(store.root())
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    if store_root != guard.store_root {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    if !guard.maintenance.is_exclusive() || !guard.maintenance.covers(store.root()) {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    let lock_metadata = fs::symlink_metadata(&guard.lock_path)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    if !lock_metadata.file_type().is_file() {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    let lock_bytes = fs::read(&guard.lock_path)
        .map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))?;
    if lock_bytes != GC_LOCK_BYTES {
        return Err(GcError::gc(GcErrorCode::ExclusiveLockRequired));
    }
    Ok(())
}

fn check_allocation(report_entries: usize, inventory: &[InventoryEntry]) -> Result<()> {
    let id_budget = report_entries
        .checked_mul(128)
        .ok_or_else(|| GcError::gc(GcErrorCode::ResourceLimit))?;
    let path_budget = inventory.iter().try_fold(0_usize, |total, entry| {
        total
            .checked_add(entry.path.as_os_str().len())
            .ok_or_else(|| GcError::gc(GcErrorCode::ResourceLimit))
    })?;
    let estimated = id_budget
        .checked_add(path_budget)
        .ok_or_else(|| GcError::gc(GcErrorCode::ResourceLimit))?;
    if estimated > MAX_GC_ALLOCATION {
        return Err(GcError::gc(GcErrorCode::ResourceLimit));
    }
    Ok(())
}

fn map_store_error(error: &sley_store::StoreError) -> GcError {
    if error.code() == StoreErrorCode::StoreObjectNotFound {
        GcError::gc(GcErrorCode::ObjectMissing)
    } else {
        GcError::upstream(error.symbol())
    }
}

fn map_delete_io(error: io::Error) -> GcError {
    GcError::io(GcErrorCode::DeleteIo, error)
}

fn create_real_dir(parent: &Path, path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => {
            sync_dir(parent).map_err(|error| GcError::io(GcErrorCode::ExclusiveLockRequired, error))
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            ensure_real_dir(path, GcErrorCode::ExclusiveLockRequired)
        }
        Err(error) => Err(GcError::io(GcErrorCode::ExclusiveLockRequired, error)),
    }
}

fn ensure_real_dir(path: &Path, code: GcErrorCode) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| GcError::io(code, error))?;
    if metadata.file_type().is_dir() {
        Ok(())
    } else {
        Err(GcError::gc(code))
    }
}

fn real_dir_or_absent(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(true),
        Ok(_) => Err(GcError::gc(GcErrorCode::InventoryInvalid)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(GcError::io(GcErrorCode::InventoryInvalid, error)),
    }
}

fn sorted_entries(path: &Path) -> Result<Vec<fs::DirEntry>> {
    let reader =
        fs::read_dir(path).map_err(|error| GcError::io(GcErrorCode::InventoryInvalid, error))?;
    let mut entries = Vec::new();
    for entry in reader {
        entries.push(entry.map_err(|error| GcError::io(GcErrorCode::InventoryInvalid, error))?);
        if entries.len() > MAX_GC_INVENTORY_OBJECTS {
            return Err(GcError::gc(GcErrorCode::ResourceLimit));
        }
    }
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

fn require_hex_dir(entry: &fs::DirEntry, length: usize) -> Result<()> {
    let file_type = entry
        .file_type()
        .map_err(|error| GcError::io(GcErrorCode::InventoryInvalid, error))?;
    let file_name = entry.file_name();
    let name = os_text(&file_name)?;
    if !file_type.is_dir() || name.len() != length || !name.bytes().all(is_lower_hex) {
        return Err(GcError::gc(GcErrorCode::InventoryInvalid));
    }
    Ok(())
}

fn os_text(name: &std::ffi::OsStr) -> Result<&str> {
    name.to_str()
        .ok_or_else(|| GcError::gc(GcErrorCode::InventoryInvalid))
}

fn parse_object_id(hex: &str) -> Result<ObjectId> {
    if hex.len() != ID_LEN * 2 || !hex.bytes().all(is_lower_hex) {
        return Err(GcError::gc(GcErrorCode::InventoryInvalid));
    }
    let mut bytes = [0_u8; ID_LEN];
    for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        bytes[index] = (high << 4) | low;
    }
    Ok(ObjectId::from_bytes(bytes))
}

const fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (byte >= b'a' && byte <= b'f')
}

fn hex_nibble(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(GcError::gc(GcErrorCode::InventoryInvalid)),
    }
}

fn sync_dir(path: &Path) -> io::Result<()> {
    File::open(path).and_then(|directory| directory.sync_all())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use sley_id::{EntityId, PolicyRootId, WorkspaceId};
    use sley_scb1::{
        FixtureContract, MAX_STANDALONE_BYTES, ScbErrorCode, decode_standalone_fixture,
        encode_bool, encode_record, encode_standalone_fixture,
    };
    use sley_state_root::{StateRootBuilder, conformance_registry};

    use super::*;

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(label: &str) -> Self {
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("sley2-gc-{label}-{}-{counter}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
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

    fn exact_error_source_chain(
        error: &(dyn ::std::error::Error + 'static),
    ) -> ::std::vec::Vec<::std::string::String> {
        let mut chain = ::std::vec::Vec::new();
        let mut source = ::std::error::Error::source(error);
        while let ::core::option::Option::Some(current) = source {
            let label = if let ::core::option::Option::Some(io_error) =
                current.downcast_ref::<::std::io::Error>()
            {
                ::std::format!("io::Error({:?})", io_error.kind())
            } else {
                ::std::format!("unknown({})", ::std::any::type_name_of_val(current),)
            };
            chain.push(label);
            source = ::std::error::Error::source(current);
        }
        chain
    }

    fn initialize_exact_gc_witness(store: &ObjectStore, bytes: &[u8]) -> PathBuf {
        initialize_repository_maintenance(store.root()).unwrap();
        let lock_dir = store.root().join(GC_LOCK_DIR);
        let lock_path = lock_dir.join(GC_LOCK_FILE);
        let mut witness = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .unwrap();
        witness.write_all(bytes).unwrap();
        witness.sync_all().unwrap();
        sync_dir(&lock_dir).unwrap();
        lock_path
    }

    #[derive(Default)]
    struct FixtureVerifier {
        references: BTreeMap<ObjectId, Vec<ObjectId>>,
        reject_references: BTreeSet<ObjectId>,
    }

    impl CanonicalVerifier for FixtureVerifier {
        fn verify(&self, record: &[u8]) -> core::result::Result<ObjectId, ScbError> {
            decode_standalone_fixture(record, FixtureContract::EmptyObject)
                .or_else(|_| decode_standalone_fixture(record, FixtureContract::RequiredBool))
                .map(|fixture| fixture.object_id)
        }
    }

    impl GcObjectVerifier for FixtureVerifier {
        fn references(&self, record: &[u8]) -> core::result::Result<Vec<ObjectId>, ScbError> {
            let object_id = self.verify(record)?;
            if self.reject_references.contains(&object_id) {
                return Err(ScbError::new(ScbErrorCode::FieldUnknown));
            }
            Ok(self.references.get(&object_id).cloned().unwrap_or_default())
        }
    }

    fn empty_object() -> (Vec<u8>, ObjectId) {
        encode_standalone_fixture(FixtureContract::EmptyObject, &encode_record(&[]).unwrap())
            .unwrap()
    }

    fn bool_object(value: bool) -> (Vec<u8>, ObjectId) {
        let payload = encode_record(&[(1, encode_bool(value))]).unwrap();
        encode_standalone_fixture(FixtureContract::RequiredBool, &payload).unwrap()
    }

    fn root(
        contract_root: ObjectId,
        test_root: ObjectId,
        bound: ObjectId,
        dependency: Option<StateRoot>,
    ) -> AcceptedStateRoot {
        let registry = conformance_registry().unwrap();
        let mut builder = StateRootBuilder::new(
            WorkspaceId::from_bytes([7; 32]),
            contract_root,
            test_root,
            PolicyRootId::from_bytes([9; 32]),
        )
        .entity_binding(EntityId::from_bytes([8; 32]), bound)
        .entry_point(EntityId::from_bytes([8; 32]));
        if let Some(dependency) = dependency {
            builder = builder.dependency_root(dependency);
        }
        builder.build(&registry).unwrap()
    }

    fn anchor(kind: RetentionKind, byte: u8, targets: Vec<RetentionTarget>) -> RetentionAnchor {
        RetentionAnchor::new(kind, [byte; 32], targets)
    }

    struct Fixture {
        _temp: TempRoot,
        store: ObjectStore,
        retained: AcceptedStateRoot,
        unreachable_id: ObjectId,
        child_id: ObjectId,
        verifier: FixtureVerifier,
    }

    fn fixture() -> Fixture {
        let temp = TempRoot::new("fixture");
        let store = ObjectStore::new(&temp.0);
        let (contract, contract_id) = empty_object();
        let (child, child_id) = bool_object(true);
        let (unreachable, unreachable_id) = bool_object(false);
        let verifier = FixtureVerifier {
            references: BTreeMap::from([(contract_id, vec![child_id])]),
            reject_references: BTreeSet::new(),
        };
        for (bytes, id) in [
            (&contract, contract_id),
            (&child, child_id),
            (&unreachable, unreachable_id),
        ] {
            store.put(id, bytes, &verifier).unwrap();
        }
        let retained = root(contract_id, contract_id, contract_id, None);
        Fixture {
            _temp: temp,
            store,
            retained,
            unreachable_id,
            child_id,
            verifier,
        }
    }

    #[test]
    fn gcw01_empty_witness_recovers_incomplete() {
        let fixture = fixture();
        let error = acquire_exclusive_gc_with_gc_durability_cut(
            &fixture.store,
            &GcDurabilityCut::Gcw01WitnessCreateBeforeWrite,
        )
        .unwrap_err();
        ::core::assert_eq!(error.symbol(), "GC_EXCLUSIVE_LOCK_REQUIRED");
        ::core::assert_eq!(
            exact_error_source_chain(&error),
            ::std::vec!["io::Error(Other)"]
        );
        let witness = fixture.store.root().join(GC_LOCK_DIR).join(GC_LOCK_FILE);
        let immediate_state_verified = fs::read(&witness).unwrap().is_empty();
        ::core::assert!(immediate_state_verified);

        let maintenance = acquire_exclusive_repository_maintenance(fixture.store.root()).unwrap();
        let status = recover_gc_witness(&fixture.store, &maintenance).unwrap();
        let recovery_result_verified =
            status == GcWitnessRecoveryStatus::RemovedIncomplete && !witness.exists();
        ::core::assert!(recovery_result_verified);
    }

    #[test]
    fn gcw02_half_prefix_recovers_incomplete() {
        let fixture = fixture();
        let error = acquire_exclusive_gc_with_gc_durability_cut(
            &fixture.store,
            &GcDurabilityCut::Gcw02DuringWitnessWrite,
        )
        .unwrap_err();
        ::core::assert_eq!(error.symbol(), "GC_EXCLUSIVE_LOCK_REQUIRED");
        ::core::assert_eq!(
            exact_error_source_chain(&error),
            ::std::vec!["io::Error(Other)"]
        );
        let witness = fixture.store.root().join(GC_LOCK_DIR).join(GC_LOCK_FILE);
        let immediate_state_verified =
            fs::read(&witness).unwrap() == b"SLEY" && fs::metadata(&witness).unwrap().len() == 4;
        ::core::assert!(immediate_state_verified);

        let maintenance = acquire_exclusive_repository_maintenance(fixture.store.root()).unwrap();
        let status = recover_gc_witness(&fixture.store, &maintenance).unwrap();
        let recovery_result_verified =
            status == GcWitnessRecoveryStatus::RemovedIncomplete && !witness.exists();
        ::core::assert!(recovery_result_verified);
    }

    #[test]
    fn gcw03_exact_unsynced_witness_recovers_exact() {
        let fixture = fixture();
        let error = acquire_exclusive_gc_with_gc_durability_cut(
            &fixture.store,
            &GcDurabilityCut::Gcw03WitnessWriteBeforeFileSync,
        )
        .unwrap_err();
        ::core::assert_eq!(error.symbol(), "GC_EXCLUSIVE_LOCK_REQUIRED");
        ::core::assert_eq!(
            exact_error_source_chain(&error),
            ::std::vec!["io::Error(Other)"]
        );
        let witness = fixture.store.root().join(GC_LOCK_DIR).join(GC_LOCK_FILE);
        let immediate_state_verified = fs::read(&witness).unwrap() == GC_LOCK_BYTES;
        ::core::assert!(immediate_state_verified);

        let maintenance = acquire_exclusive_repository_maintenance(fixture.store.root()).unwrap();
        let status = recover_gc_witness(&fixture.store, &maintenance).unwrap();
        let recovery_result_verified =
            status == GcWitnessRecoveryStatus::RemovedExact && !witness.exists();
        ::core::assert!(recovery_result_verified);
    }

    #[test]
    fn gcw04_file_synced_witness_recovers_exact() {
        let fixture = fixture();
        let error = acquire_exclusive_gc_with_gc_durability_cut(
            &fixture.store,
            &GcDurabilityCut::Gcw04WitnessFileSyncBeforeLockDirectorySync,
        )
        .unwrap_err();
        ::core::assert_eq!(error.symbol(), "GC_EXCLUSIVE_LOCK_REQUIRED");
        ::core::assert_eq!(
            exact_error_source_chain(&error),
            ::std::vec!["io::Error(Other)"]
        );
        let witness = fixture.store.root().join(GC_LOCK_DIR).join(GC_LOCK_FILE);
        let immediate_state_verified = fs::read(&witness).unwrap() == GC_LOCK_BYTES;
        ::core::assert!(immediate_state_verified);

        let maintenance = acquire_exclusive_repository_maintenance(fixture.store.root()).unwrap();
        let status = recover_gc_witness(&fixture.store, &maintenance).unwrap();
        let recovery_result_verified =
            status == GcWitnessRecoveryStatus::RemovedExact && !witness.exists();
        ::core::assert!(recovery_result_verified);
    }

    #[test]
    fn gcw05_removed_witness_retry_reports_absent() {
        let fixture = fixture();
        let witness = initialize_exact_gc_witness(&fixture.store, GC_LOCK_BYTES);
        let maintenance = acquire_exclusive_repository_maintenance(fixture.store.root()).unwrap();
        let error = recover_gc_witness_with_gc_durability_cut(
            &fixture.store,
            &maintenance,
            &GcDurabilityCut::Gcw05WitnessRemoveBeforeLockDirectorySync,
        )
        .unwrap_err();
        ::core::assert_eq!(error.symbol(), "GC_EXCLUSIVE_LOCK_REQUIRED");
        ::core::assert_eq!(
            exact_error_source_chain(&error),
            ::std::vec!["io::Error(Other)"]
        );
        let immediate_state_verified = !witness.exists();
        ::core::assert!(immediate_state_verified);
        drop(maintenance);

        let retry_maintenance =
            acquire_exclusive_repository_maintenance(fixture.store.root()).unwrap();
        let retry = recover_gc_witness(&fixture.store, &retry_maintenance).unwrap();
        let recovery_result_verified =
            retry == GcWitnessRecoveryStatus::Absent && !witness.exists();
        ::core::assert!(recovery_result_verified);
    }

    #[test]
    fn gcw06_recovery_waits_for_live_collection() {
        let fixture = fixture();
        let (owner_tx, owner_rx) = ::std::sync::mpsc::sync_channel(0);
        let (release_tx, release_rx) = ::std::sync::mpsc::sync_channel(0);
        let gate = Arc::new(GcMaintenanceRaceGate {
            owner_tx,
            release_rx: Mutex::new(release_rx),
        });
        let cut_gate = Arc::clone(&gate);
        let owner_store = fixture.store.clone();
        let owner = ::std::thread::spawn(move || {
            acquire_exclusive_gc_with_gc_durability_cut(
                &owner_store,
                &GcDurabilityCut::Gcw06CollectionOwnsMaintenanceBeforeWitnessAccess {
                    gate: cut_gate,
                },
            )
        });
        owner_rx.recv().unwrap();

        let maintenance_probe = OpenOptions::new()
            .read(true)
            .write(true)
            .open(fixture.store.root().join("locks/maintenance.lock"))
            .unwrap();
        ::core::assert!(::core::matches!(
            File::try_lock(&maintenance_probe),
            Err(::std::fs::TryLockError::WouldBlock)
        ));
        let witness = fixture.store.root().join(GC_LOCK_DIR).join(GC_LOCK_FILE);
        let immediate_state_verified = !witness.exists();
        ::core::assert!(immediate_state_verified);

        let (waiter_started_tx, waiter_started_rx) = ::std::sync::mpsc::sync_channel(0);
        let (waiter_result_tx, waiter_result_rx) = ::std::sync::mpsc::sync_channel(0);
        let waiter_store = fixture.store.clone();
        let waiter = ::std::thread::spawn(move || {
            waiter_started_tx.send(()).unwrap();
            let waiter_maintenance =
                acquire_exclusive_repository_maintenance(waiter_store.root()).unwrap();
            let waiter_status = recover_gc_witness(&waiter_store, &waiter_maintenance);
            waiter_result_tx.send(waiter_status).unwrap();
        });
        waiter_started_rx.recv().unwrap();

        release_tx.send(()).unwrap();
        let guard = owner.join().unwrap().unwrap();
        ::core::assert!(guard.maintenance.is_exclusive());
        ::core::assert!(guard.maintenance.covers(fixture.store.root()));
        ::core::assert_eq!(fs::read(&witness).unwrap(), GC_LOCK_BYTES);
        guard.release().unwrap();

        let waiter_result = waiter_result_rx.recv().unwrap().unwrap();
        let recovery_result_verified =
            waiter_result == GcWitnessRecoveryStatus::Absent && !witness.exists();
        ::core::assert!(recovery_result_verified);
        waiter.join().unwrap();
    }

    #[test]
    fn guard05_same_root_shared_preserves_witness() {
        let fixture = fixture();
        let store = fixture.store.clone();
        initialize_exact_gc_witness(&store, GC_LOCK_BYTES);
        let maintenance = ::sley_txn::acquire_shared_repository_maintenance(store.root()).unwrap();
        ::core::assert!(!maintenance.is_exclusive());
        ::core::assert!(maintenance.covers(store.root()));
        let owner_root = store.root();
        let owner_tree_before_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_path = owner_root.join("locks").join("gc.lock");
        let gc_witness_before_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_before_kind = gc_witness_before_snapshot.0;
        ::core::assert_eq!(gc_witness_before_kind, "regular");
        ::core::assert_eq!(gc_witness_before_snapshot.2, b"SLEYGC01".to_vec());
        let result = super::recover_gc_witness(&store, &maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_after_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_after_kind = gc_witness_after_snapshot.0;
        ::core::assert_eq!(error.symbol(), "GC_EXCLUSIVE_LOCK_REQUIRED");
        ::core::assert!(::core::matches!(
            &error,
            super::GcError {
                symbol: "GC_EXCLUSIVE_LOCK_REQUIRED",
                partial_report: ::core::option::Option::None,
                source: ::core::option::Option::None
            }
        ));
        ::core::assert_eq!(
            crate::gc::tests::exact_error_source_chain(&error),
            Vec::<String>::new()
        );
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
        ::core::assert_eq!(gc_witness_before_snapshot, gc_witness_after_snapshot);
        ::core::assert_eq!(gc_witness_before_kind, gc_witness_after_kind);
    }

    #[test]
    fn guard06_wrong_root_exclusive_preserves_both_roots() {
        let fixture = fixture();
        let store = fixture.store.clone();
        initialize_exact_gc_witness(&store, GC_LOCK_BYTES);
        let other = TempRoot::new("wrong-maintenance-root");
        initialize_repository_maintenance(&other.0).unwrap();
        let maintenance = acquire_exclusive_repository_maintenance(&other.0).unwrap();
        ::core::assert!(maintenance.is_exclusive());
        ::core::assert!(!maintenance.covers(store.root()));
        let guard_root_tree_before_snapshot = crate::gc::tests::exact_tree_snapshot(&other.0);
        let owner_root = store.root();
        let owner_tree_before_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_path = owner_root.join("locks").join("gc.lock");
        let gc_witness_before_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_before_kind = gc_witness_before_snapshot.0;
        ::core::assert_eq!(gc_witness_before_kind, "regular");
        ::core::assert_eq!(gc_witness_before_snapshot.2, b"SLEYGC01".to_vec());
        let result = super::recover_gc_witness(&store, &maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_after_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_after_kind = gc_witness_after_snapshot.0;
        ::core::assert_eq!(error.symbol(), "GC_EXCLUSIVE_LOCK_REQUIRED");
        ::core::assert!(::core::matches!(
            &error,
            super::GcError {
                symbol: "GC_EXCLUSIVE_LOCK_REQUIRED",
                partial_report: ::core::option::Option::None,
                source: ::core::option::Option::None
            }
        ));
        ::core::assert_eq!(
            crate::gc::tests::exact_error_source_chain(&error),
            Vec::<String>::new()
        );
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
        ::core::assert_eq!(gc_witness_before_snapshot, gc_witness_after_snapshot);
        ::core::assert_eq!(gc_witness_before_kind, gc_witness_after_kind);
        ::core::assert_eq!(
            crate::gc::tests::exact_tree_snapshot(&other.0),
            guard_root_tree_before_snapshot
        );
    }

    #[test]
    fn cor08_wrong_magic_preserves_witness() {
        let fixture = fixture();
        let store = fixture.store.clone();
        initialize_exact_gc_witness(&store, b"NOTGC001");
        let maintenance = acquire_exclusive_repository_maintenance(store.root()).unwrap();
        let owner_root = store.root();
        let owner_tree_before_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_path = owner_root.join("locks").join("gc.lock");
        let gc_witness_before_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_before_kind = gc_witness_before_snapshot.0;
        ::core::assert_eq!(gc_witness_before_kind, "regular");
        ::core::assert_eq!(gc_witness_before_snapshot.2, b"NOTGC001".to_vec());
        let result = super::recover_gc_witness(&store, &maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_after_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_after_kind = gc_witness_after_snapshot.0;
        ::core::assert_eq!(error.symbol(), "GC_EXCLUSIVE_LOCK_REQUIRED");
        ::core::assert!(::core::matches!(
            &error,
            super::GcError {
                symbol: "GC_EXCLUSIVE_LOCK_REQUIRED",
                partial_report: ::core::option::Option::None,
                source: ::core::option::Option::None
            }
        ));
        ::core::assert_eq!(
            crate::gc::tests::exact_error_source_chain(&error),
            Vec::<String>::new()
        );
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
        ::core::assert_eq!(gc_witness_before_snapshot, gc_witness_after_snapshot);
        ::core::assert_eq!(gc_witness_before_kind, gc_witness_after_kind);
    }

    #[test]
    fn cor08_oversize_witness_preserves_witness() {
        let fixture = fixture();
        let store = fixture.store.clone();
        initialize_exact_gc_witness(&store, b"SLEYGC01X");
        let maintenance = acquire_exclusive_repository_maintenance(store.root()).unwrap();
        let owner_root = store.root();
        let owner_tree_before_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_path = owner_root.join("locks").join("gc.lock");
        let gc_witness_before_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_before_kind = gc_witness_before_snapshot.0;
        ::core::assert_eq!(gc_witness_before_kind, "regular");
        ::core::assert_eq!(gc_witness_before_snapshot.2, b"SLEYGC01X".to_vec());
        let result = super::recover_gc_witness(&store, &maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_after_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_after_kind = gc_witness_after_snapshot.0;
        ::core::assert_eq!(error.symbol(), "GC_EXCLUSIVE_LOCK_REQUIRED");
        ::core::assert!(::core::matches!(
            &error,
            super::GcError {
                symbol: "GC_EXCLUSIVE_LOCK_REQUIRED",
                partial_report: ::core::option::Option::None,
                source: ::core::option::Option::None
            }
        ));
        ::core::assert_eq!(
            crate::gc::tests::exact_error_source_chain(&error),
            Vec::<String>::new()
        );
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
        ::core::assert_eq!(gc_witness_before_snapshot, gc_witness_after_snapshot);
        ::core::assert_eq!(gc_witness_before_kind, gc_witness_after_kind);
    }

    #[test]
    fn cor08_relative_symlink_preserves_witness() {
        let fixture = fixture();
        let store = fixture.store.clone();
        initialize_repository_maintenance(store.root()).unwrap();
        let maintenance = acquire_exclusive_repository_maintenance(store.root()).unwrap();
        let owner_root = store.root();
        let gc_witness_path = owner_root.join("locks").join("gc.lock");
        fs::write(
            owner_root.join("locks").join("gc-witness-target"),
            GC_LOCK_BYTES,
        )
        .unwrap();
        ::std::os::unix::fs::symlink("gc-witness-target", &gc_witness_path).unwrap();
        sync_dir(&owner_root.join("locks")).unwrap();
        let owner_tree_before_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_before_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_before_kind = gc_witness_before_snapshot.0;
        ::core::assert_eq!(gc_witness_before_kind, "symlink");
        ::core::assert_eq!(
            gc_witness_before_snapshot.3,
            ::core::option::Option::Some(::std::path::PathBuf::from("gc-witness-target"))
        );
        let result = super::recover_gc_witness(&store, &maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_after_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_after_kind = gc_witness_after_snapshot.0;
        ::core::assert_eq!(error.symbol(), "GC_EXCLUSIVE_LOCK_REQUIRED");
        ::core::assert!(::core::matches!(
            &error,
            super::GcError {
                symbol: "GC_EXCLUSIVE_LOCK_REQUIRED",
                partial_report: ::core::option::Option::None,
                source: ::core::option::Option::None
            }
        ));
        ::core::assert_eq!(
            crate::gc::tests::exact_error_source_chain(&error),
            Vec::<String>::new()
        );
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
        ::core::assert_eq!(gc_witness_before_snapshot, gc_witness_after_snapshot);
        ::core::assert_eq!(gc_witness_before_kind, gc_witness_after_kind);
    }

    #[test]
    fn cor08_unix_socket_preserves_witness() {
        let fixture = fixture();
        let store = fixture.store.clone();
        initialize_repository_maintenance(store.root()).unwrap();
        let maintenance = acquire_exclusive_repository_maintenance(store.root()).unwrap();
        let owner_root = store.root();
        let gc_witness_path = owner_root.join("locks").join("gc.lock");
        let listener = ::std::os::unix::net::UnixListener::bind(&gc_witness_path).unwrap();
        sync_dir(&owner_root.join("locks")).unwrap();
        let owner_tree_before_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_before_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_before_kind = gc_witness_before_snapshot.0;
        ::core::assert_eq!(gc_witness_before_kind, "non_regular");
        ::core::assert_eq!(gc_witness_before_snapshot.1 & 0o170_000, 0o140_000);
        let result = super::recover_gc_witness(&store, &maintenance);
        ::core::assert!(result.is_err());
        let error = result.expect_err("expected recovery error");
        let owner_tree_after_snapshot = crate::gc::tests::exact_tree_snapshot(owner_root);
        let gc_witness_after_snapshot = crate::gc::tests::exact_path_snapshot(&gc_witness_path);
        let gc_witness_after_kind = gc_witness_after_snapshot.0;
        ::core::assert_eq!(error.symbol(), "GC_EXCLUSIVE_LOCK_REQUIRED");
        ::core::assert!(::core::matches!(
            &error,
            super::GcError {
                symbol: "GC_EXCLUSIVE_LOCK_REQUIRED",
                partial_report: ::core::option::Option::None,
                source: ::core::option::Option::None
            }
        ));
        ::core::assert_eq!(
            crate::gc::tests::exact_error_source_chain(&error),
            Vec::<String>::new()
        );
        ::core::assert_eq!(owner_tree_before_snapshot, owner_tree_after_snapshot);
        ::core::assert_eq!(gc_witness_before_snapshot, gc_witness_after_snapshot);
        ::core::assert_eq!(gc_witness_before_kind, gc_witness_after_kind);
        drop(listener);
    }

    #[test]
    fn dry_run_marks_root_and_transitive_object_references() {
        let fixture = fixture();
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::Ref,
                1,
                vec![RetentionTarget::StateRoot(fixture.retained.root)],
            )],
            vec![fixture.retained.clone()],
        )
        .unwrap();
        let report = gc_dry_run(&fixture.store, &snapshot, &fixture.verifier).unwrap();
        assert_eq!(report.decision, GcDecision::DryRun);
        assert!(report.reachable_objects.contains(&fixture.child_id));
        assert_eq!(report.deletion_candidates, vec![fixture.unreachable_id]);
        assert!(fixture.store.object_path(fixture.unreachable_id).is_file());
    }

    #[test]
    fn every_anchor_kind_retains_the_same_root_closure() {
        let fixture = fixture();
        for (index, kind) in [
            RetentionKind::Ref,
            RetentionKind::Tag,
            RetentionKind::Lease,
            RetentionKind::Transaction,
            RetentionKind::PackManifest,
            RetentionKind::ProtectedRoot,
            RetentionKind::SessionPin,
        ]
        .into_iter()
        .enumerate()
        {
            let snapshot = RetentionSnapshot::new(
                vec![anchor(
                    kind,
                    u8::try_from(index).unwrap(),
                    vec![RetentionTarget::StateRoot(fixture.retained.root)],
                )],
                vec![fixture.retained.clone()],
            )
            .unwrap();
            let report = gc_dry_run(&fixture.store, &snapshot, &fixture.verifier).unwrap();
            assert!(report.reachable_objects.contains(&fixture.child_id));
            assert_eq!(report.deletion_candidates, vec![fixture.unreachable_id]);
        }
    }

    #[test]
    fn dependency_roots_are_retained_transitively() {
        let fixture = fixture();
        let dependency = fixture.retained.clone();
        let parent = root(
            dependency.record.contract_root,
            dependency.record.test_root,
            dependency.record.contract_root,
            Some(dependency.root),
        );
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::Tag,
                2,
                vec![RetentionTarget::StateRoot(parent.root)],
            )],
            vec![parent.clone(), dependency.clone()],
        )
        .unwrap();
        let report = gc_dry_run(&fixture.store, &snapshot, &fixture.verifier).unwrap();
        let mut expected = vec![dependency.root, parent.root];
        expected.sort_unstable();
        assert_eq!(report.retained_roots, expected);
    }

    #[test]
    fn missing_dependency_fails_closed() {
        let fixture = fixture();
        let parent = root(
            fixture.retained.record.contract_root,
            fixture.retained.record.test_root,
            fixture.retained.record.contract_root,
            Some(fixture.retained.root),
        );
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::Ref,
                3,
                vec![RetentionTarget::StateRoot(parent.root)],
            )],
            vec![parent],
        )
        .unwrap();
        assert_eq!(
            gc_dry_run(&fixture.store, &snapshot, &fixture.verifier)
                .unwrap_err()
                .symbol(),
            "GC_DEPENDENCY_MISSING"
        );
    }

    #[test]
    fn unresolved_direct_object_anchor_fails_closed() {
        let fixture = fixture();
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::ProtectedRoot,
                4,
                vec![RetentionTarget::Object(ObjectId::from_bytes([0xfe; 32]))],
            )],
            Vec::new(),
        )
        .unwrap();
        assert_eq!(
            gc_dry_run(&fixture.store, &snapshot, &fixture.verifier)
                .unwrap_err()
                .symbol(),
            "GC_ANCHOR_UNRESOLVED"
        );
    }

    #[test]
    fn empty_retention_snapshot_marks_all_verified_objects_as_candidates() {
        let fixture = fixture();
        let snapshot = RetentionSnapshot::new(Vec::new(), Vec::new()).unwrap();
        let report = gc_dry_run(&fixture.store, &snapshot, &fixture.verifier).unwrap();
        assert!(report.examined_anchors.is_empty());
        assert!(report.retained_roots.is_empty());
        assert!(report.reachable_objects.is_empty());
        assert_eq!(report.deletion_candidates, report.inventory_objects);
        assert_eq!(report.deletion_candidates.len(), 3);
    }

    #[test]
    fn missing_transitive_object_fails_closed() {
        let fixture = fixture();
        fs::remove_file(fixture.store.object_path(fixture.child_id)).unwrap();
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::Ref,
                14,
                vec![RetentionTarget::StateRoot(fixture.retained.root)],
            )],
            vec![fixture.retained.clone()],
        )
        .unwrap();
        assert_eq!(
            gc_dry_run(&fixture.store, &snapshot, &fixture.verifier)
                .unwrap_err()
                .symbol(),
            "GC_OBJECT_MISSING"
        );
    }

    #[test]
    fn corrupt_unreachable_object_is_not_silently_deleted() {
        let fixture = fixture();
        let path = fixture.store.object_path(fixture.unreachable_id);
        let mut bytes = fs::read(&path).unwrap();
        bytes[10] ^= 1;
        fs::write(&path, bytes).unwrap();
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::Lease,
                15,
                vec![RetentionTarget::StateRoot(fixture.retained.root)],
            )],
            vec![fixture.retained.clone()],
        )
        .unwrap();
        assert_eq!(
            gc_dry_run(&fixture.store, &snapshot, &fixture.verifier)
                .unwrap_err()
                .symbol(),
            "SCB_DIGEST_MISMATCH"
        );
        assert!(path.is_file());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_inventory_object_fails_closed() {
        use std::os::unix::fs::symlink;

        let fixture = fixture();
        let path = fixture.store.object_path(fixture.unreachable_id);
        fs::remove_file(&path).unwrap();
        symlink(
            fixture
                .store
                .object_path(fixture.retained.record.contract_root),
            &path,
        )
        .unwrap();
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::Ref,
                16,
                vec![RetentionTarget::StateRoot(fixture.retained.root)],
            )],
            vec![fixture.retained.clone()],
        )
        .unwrap();
        assert_eq!(
            gc_dry_run(&fixture.store, &snapshot, &fixture.verifier)
                .unwrap_err()
                .symbol(),
            "GC_INVENTORY_INVALID"
        );
    }

    #[test]
    fn malformed_reference_shape_fails_before_deletion() {
        let mut fixture = fixture();
        fixture
            .verifier
            .reject_references
            .insert(fixture.retained.record.contract_root);
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::Lease,
                5,
                vec![RetentionTarget::StateRoot(fixture.retained.root)],
            )],
            vec![fixture.retained.clone()],
        )
        .unwrap();
        assert_eq!(
            gc_dry_run(&fixture.store, &snapshot, &fixture.verifier)
                .unwrap_err()
                .symbol(),
            "GC_OBJECT_REFERENCE_MALFORMED"
        );
        assert!(fixture.store.object_path(fixture.unreachable_id).is_file());
    }

    #[test]
    fn malformed_inventory_path_fails_closed() {
        let fixture = fixture();
        fs::write(
            fixture.store.root().join("objects/scb1/not-a-fanout"),
            b"foreign",
        )
        .unwrap();
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::Ref,
                6,
                vec![RetentionTarget::StateRoot(fixture.retained.root)],
            )],
            vec![fixture.retained.clone()],
        )
        .unwrap();
        assert_eq!(
            gc_dry_run(&fixture.store, &snapshot, &fixture.verifier)
                .unwrap_err()
                .symbol(),
            "GC_INVENTORY_INVALID"
        );
    }

    #[test]
    fn collect_deletes_only_unreachable_and_is_idempotent() {
        let fixture = fixture();
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::SessionPin,
                7,
                vec![RetentionTarget::StateRoot(fixture.retained.root)],
            )],
            vec![fixture.retained.clone()],
        )
        .unwrap();
        let guard = acquire_exclusive_gc(&fixture.store).unwrap();
        let first = gc_collect(&fixture.store, &snapshot, &fixture.verifier, &guard).unwrap();
        assert_eq!(first.decision, GcDecision::Collected);
        assert_eq!(first.deleted_objects, vec![fixture.unreachable_id]);
        assert!(!fixture.store.object_path(fixture.unreachable_id).exists());
        for object in &first.reachable_objects {
            assert!(fixture.store.object_path(*object).is_file());
        }
        let second = gc_collect(&fixture.store, &snapshot, &fixture.verifier, &guard).unwrap();
        assert!(second.deletion_candidates.is_empty());
        assert!(second.deleted_objects.is_empty());
    }

    #[test]
    fn concurrent_guard_blocks_and_wrong_store_guard_fails_closed() {
        let fixture = fixture();
        let other_temp = TempRoot::new("other");
        let other = ObjectStore::new(&other_temp.0);
        let guard = acquire_exclusive_gc(&fixture.store).unwrap();
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::Ref,
                8,
                vec![RetentionTarget::StateRoot(fixture.retained.root)],
            )],
            vec![fixture.retained.clone()],
        )
        .unwrap();
        assert_eq!(
            gc_collect(&other, &snapshot, &fixture.verifier, &guard)
                .unwrap_err()
                .symbol(),
            "GC_EXCLUSIVE_LOCK_REQUIRED"
        );

        let concurrent_store = fixture.store.clone();
        let (started_tx, started_rx) = ::std::sync::mpsc::channel();
        let (finished_tx, finished_rx) = ::std::sync::mpsc::channel();
        let contender = ::std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            finished_tx
                .send(acquire_exclusive_gc(&concurrent_store))
                .unwrap();
        });
        started_rx.recv().unwrap();
        assert!(
            finished_rx
                .recv_timeout(::std::time::Duration::from_millis(100))
                .is_err()
        );
        drop(guard);
        let concurrent_guard = finished_rx
            .recv_timeout(::std::time::Duration::from_secs(2))
            .unwrap()
            .unwrap();
        drop(concurrent_guard);
        contender.join().unwrap();
    }

    #[test]
    fn injected_delete_failure_retry_is_idempotent() {
        let fixture = fixture();
        let snapshot = RetentionSnapshot::new(Vec::new(), Vec::new()).unwrap();
        let dry_run = gc_dry_run(&fixture.store, &snapshot, &fixture.verifier).unwrap();
        let deletion_candidates = dry_run.deletion_candidates.clone();
        let candidate_count = deletion_candidates.len();
        ::core::assert_eq!(candidate_count, 3);
        let candidate_order_verified = deletion_candidates.windows(2).all(|pair| pair[0] < pair[1]);
        ::core::assert!(candidate_order_verified);
        let durable_prefix = deletion_candidates[0];
        let failed_candidate = deletion_candidates[1];
        let untouched_suffix = deletion_candidates[2];
        let guard = acquire_exclusive_gc(&fixture.store).unwrap();
        let expected_result = gc_collect_with_gc_durability_cut(
            &fixture.store,
            &snapshot,
            &fixture.verifier,
            &guard,
            &GcDurabilityCut::Gc01BeforeSecondCandidateDelete,
        )
        .unwrap_err();
        ::core::assert_eq!(expected_result.symbol(), "GC_DELETE_IO");
        let report = expected_result.partial_report().unwrap();
        ::core::assert_eq!(report.decision, GcDecision::PartialDeleteFailure);
        let failed_candidate_index = report
            .deletion_candidates
            .iter()
            .position(|candidate| report.failed_object == Some(*candidate))
            .unwrap();
        ::core::assert_eq!(failed_candidate_index, 1);
        let durable_prefix_count = report.deleted_objects.len();
        ::core::assert_eq!(durable_prefix_count, 1);
        let original_deletion_candidates_exact = report.deletion_candidates == deletion_candidates;
        ::core::assert!(original_deletion_candidates_exact);
        let reachable_objects_unchanged = report.reachable_objects == dry_run.reachable_objects;
        ::core::assert!(reachable_objects_unchanged);
        let deleted_objects_exact_prefix = report.deleted_objects
            == deletion_candidates[..durable_prefix_count]
            && !fixture.store.object_path(durable_prefix).exists();
        ::core::assert!(deleted_objects_exact_prefix);
        let failed_object_exact_current = report.failed_object == Some(failed_candidate);
        ::core::assert!(failed_object_exact_current);
        let failed_current_excluded = !report.deleted_objects.contains(&failed_candidate);
        ::core::assert!(failed_current_excluded);
        let suffix_untouched = fixture.store.object_path(untouched_suffix).is_file();
        ::core::assert!(suffix_untouched);
        let failed_current_present = fixture.store.object_path(failed_candidate).is_file();
        ::core::assert!(failed_current_present);

        let retry = gc_collect(&fixture.store, &snapshot, &fixture.verifier, &guard).unwrap();
        let retry_deleted_exact_remaining =
            retry.deleted_objects == ::std::vec![failed_candidate, untouched_suffix];
        ::core::assert!(retry_deleted_exact_remaining);
        let retry_complete = retry.failed_object.is_none()
            && retry.deleted_objects == retry.deletion_candidates
            && !fixture.store.object_path(failed_candidate).exists()
            && !fixture.store.object_path(untouched_suffix).exists();
        ::core::assert!(retry_complete);

        let third = gc_collect(&fixture.store, &snapshot, &fixture.verifier, &guard).unwrap();
        let third_run_empty =
            third.deletion_candidates.is_empty() && third.deleted_objects.is_empty();
        ::core::assert!(third_run_empty);
    }

    #[test]
    fn injected_sync_failure_retry_redurabilizes_absent_object() {
        let fixture = fixture();
        let snapshot = RetentionSnapshot::new(Vec::new(), Vec::new()).unwrap();
        let dry_run = gc_dry_run(&fixture.store, &snapshot, &fixture.verifier).unwrap();
        let deletion_candidates = dry_run.deletion_candidates.clone();
        let candidate_count = deletion_candidates.len();
        ::core::assert_eq!(candidate_count, 3);
        let candidate_order_verified = deletion_candidates.windows(2).all(|pair| pair[0] < pair[1]);
        ::core::assert!(candidate_order_verified);
        let durable_prefix = deletion_candidates[0];
        let failed_candidate = deletion_candidates[1];
        let untouched_suffix = deletion_candidates[2];
        let failed_leaf = fixture
            .store
            .object_path(failed_candidate)
            .parent()
            .unwrap()
            .to_path_buf();
        let guard = acquire_exclusive_gc(&fixture.store).unwrap();
        let expected_result = gc_collect_with_gc_durability_cut(
            &fixture.store,
            &snapshot,
            &fixture.verifier,
            &guard,
            &GcDurabilityCut::Gc02SecondCandidateUnlinkedBeforeLeafSync,
        )
        .unwrap_err();
        ::core::assert_eq!(expected_result.symbol(), "GC_DELETE_IO");
        let report = expected_result.partial_report().unwrap();
        ::core::assert_eq!(report.decision, GcDecision::PartialDeleteFailure);
        let failed_candidate_index = report
            .deletion_candidates
            .iter()
            .position(|candidate| report.failed_object == Some(*candidate))
            .unwrap();
        ::core::assert_eq!(failed_candidate_index, 1);
        let durable_prefix_count = report.deleted_objects.len();
        ::core::assert_eq!(durable_prefix_count, 1);
        let original_deletion_candidates_exact = report.deletion_candidates == deletion_candidates;
        ::core::assert!(original_deletion_candidates_exact);
        let reachable_objects_unchanged = report.reachable_objects == dry_run.reachable_objects;
        ::core::assert!(reachable_objects_unchanged);
        let deleted_objects_exact_prefix = report.deleted_objects
            == deletion_candidates[..durable_prefix_count]
            && !fixture.store.object_path(durable_prefix).exists();
        ::core::assert!(deleted_objects_exact_prefix);
        let failed_object_exact_current = report.failed_object == Some(failed_candidate);
        ::core::assert!(failed_object_exact_current);
        let failed_current_excluded = !report.deleted_objects.contains(&failed_candidate);
        ::core::assert!(failed_current_excluded);
        let suffix_untouched = fixture.store.object_path(untouched_suffix).is_file();
        ::core::assert!(suffix_untouched);
        let failed_current_present = fixture.store.object_path(failed_candidate).is_file();
        ::core::assert!(!failed_current_present);
        ::core::assert!(failed_leaf.is_dir());

        let corrupt_leaf = fixture
            .store
            .object_path(untouched_suffix)
            .parent()
            .unwrap()
            .to_path_buf();
        ::core::assert_ne!(corrupt_leaf, failed_leaf);
        let corrupt_inventory = corrupt_leaf.join("corrupt-inventory");
        fs::write(&corrupt_inventory, b"corrupt").unwrap();
        sync_dir(&corrupt_leaf).unwrap();
        let corrupt_tree_before = exact_tree_snapshot(fixture.store.root());
        let preflight = gc_collect_with_injected_pre_cleanup_sync_failure(
            &fixture.store,
            &snapshot,
            &fixture.verifier,
            &guard,
        )
        .unwrap_err();
        let retry_resynced_failed_leaf_before_replan = preflight.symbol() == "GC_INVENTORY_INVALID"
            && exact_error_source_chain(&preflight).is_empty()
            && exact_tree_snapshot(fixture.store.root()) == corrupt_tree_before
            && failed_leaf.is_dir();
        ::core::assert!(retry_resynced_failed_leaf_before_replan);
        fs::remove_file(&corrupt_inventory).unwrap();
        sync_dir(&corrupt_leaf).unwrap();

        let retry = gc_collect(&fixture.store, &snapshot, &fixture.verifier, &guard).unwrap();
        let retry_deleted_exact_suffix = retry.deletion_candidates == ::std::vec![untouched_suffix]
            && retry.deleted_objects == ::std::vec![untouched_suffix];
        ::core::assert!(retry_deleted_exact_suffix);
        let retry_did_not_claim_failed_current = !retry.deleted_objects.contains(&failed_candidate);
        ::core::assert!(retry_did_not_claim_failed_current);
        let retry_complete = retry.failed_object.is_none()
            && retry.deleted_objects == retry.deletion_candidates
            && !fixture.store.object_path(untouched_suffix).exists();
        ::core::assert!(retry_complete);

        let third = gc_collect(&fixture.store, &snapshot, &fixture.verifier, &guard).unwrap();
        let third_run_empty =
            third.deletion_candidates.is_empty() && third.deleted_objects.is_empty();
        ::core::assert!(third_run_empty);
    }

    #[test]
    fn unordered_snapshot_produces_identical_report() {
        let fixture = fixture();
        let a = anchor(
            RetentionKind::Tag,
            11,
            vec![RetentionTarget::StateRoot(fixture.retained.root)],
        );
        let b = anchor(
            RetentionKind::ProtectedRoot,
            12,
            vec![RetentionTarget::Object(fixture.child_id)],
        );
        let one =
            RetentionSnapshot::new(vec![a.clone(), b.clone()], vec![fixture.retained.clone()])
                .unwrap();
        let two = RetentionSnapshot::new(vec![b, a], vec![fixture.retained.clone()]).unwrap();
        assert_eq!(
            gc_dry_run(&fixture.store, &one, &fixture.verifier).unwrap(),
            gc_dry_run(&fixture.store, &two, &fixture.verifier).unwrap()
        );
    }

    #[test]
    fn duplicate_and_empty_anchors_fail_closed() {
        let target = RetentionTarget::Object(ObjectId::from_bytes([1; 32]));
        assert_eq!(
            RetentionSnapshot::new(
                vec![anchor(RetentionKind::Ref, 1, vec![target, target])],
                Vec::new(),
            )
            .unwrap_err()
            .symbol(),
            "GC_ANCHOR_MALFORMED"
        );
        assert_eq!(
            RetentionSnapshot::new(vec![anchor(RetentionKind::Ref, 1, Vec::new())], Vec::new(),)
                .unwrap_err()
                .symbol(),
            "GC_ANCHOR_MALFORMED"
        );
    }

    #[test]
    fn t40_seed_reachable_candidate_assertion_is_effective() {
        let fixture = fixture();
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::Lease,
                13,
                vec![RetentionTarget::StateRoot(fixture.retained.root)],
            )],
            vec![fixture.retained.clone()],
        )
        .unwrap();
        let report = gc_dry_run(&fixture.store, &snapshot, &fixture.verifier).unwrap();
        assert!(
            report
                .reachable_objects
                .iter()
                .all(|object| !report.deletion_candidates.contains(object))
        );
        assert!(report.reachable_objects.contains(&fixture.child_id));
    }

    #[test]
    fn t40_reachable_candidate_in_a_corrupted_plan_trips_the_guard_before_any_delete() {
        let fixture = fixture();
        let snapshot = RetentionSnapshot::new(
            vec![anchor(
                RetentionKind::Lease,
                13,
                vec![RetentionTarget::StateRoot(fixture.retained.root)],
            )],
            vec![fixture.retained.clone()],
        )
        .unwrap();
        let guard = acquire_exclusive_gc(&fixture.store).unwrap();
        let error = gc_collect_with_injected_reachable_candidate(
            &fixture.store,
            &snapshot,
            &fixture.verifier,
            &guard,
        )
        .unwrap_err();
        assert_eq!(error.symbol(), "GC_REACHABILITY_VIOLATION");
        assert!(error.partial_report().is_none());
        // The guard fired before the first unlink: every object, reachable
        // or not, is still on disk.
        assert!(fixture.store.object_path(fixture.child_id).exists());
        assert!(fixture.store.object_path(fixture.unreachable_id).exists());
        // An honest plan over the untouched store still collects exactly the
        // unreachable object, so the failed run left no partial state.
        let collected = gc_collect(&fixture.store, &snapshot, &fixture.verifier, &guard).unwrap();
        assert_eq!(collected.deleted_objects, vec![fixture.unreachable_id]);
        assert!(fixture.store.object_path(fixture.child_id).exists());
    }

    #[test]
    fn stable_gc_error_symbols_are_frozen() {
        assert_eq!(GcErrorCode::DryRunRequired.as_str(), "GC_DRY_RUN_REQUIRED");
        assert_eq!(
            GcErrorCode::ReachabilityViolation.as_str(),
            "GC_REACHABILITY_VIOLATION"
        );
        assert_eq!(MAX_STANDALONE_BYTES, 67_108_864);
    }
}
