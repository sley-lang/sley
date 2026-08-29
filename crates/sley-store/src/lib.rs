#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sley_id::ObjectId;
use sley_scb1::{MAX_STANDALONE_BYTES, ScbError, ScbErrorCode};

const DIGEST_TRAILER_LEN: usize = 32;
const STAGE_PREFIX: &str = ".sley-store-stage-";
const STAGE_SUFFIX: &str = ".tmp";
const STAGE_TOKEN_HEX_LEN: usize = 80;
const FINAL_SUFFIX: &str = ".scb1";
const FINAL_OBJECT_ID_HEX_LEN: usize = 64;
const OBJECT_RECOVERY_MAX_FANOUT_DIRECTORIES: u64 = 65_792;
const OBJECT_RECOVERY_MAX_LEAF_ENTRIES: u64 = 524_288;
const OBJECT_RECOVERY_MAX_FINAL_OBJECTS: u64 = 262_144;
const OBJECT_RECOVERY_MAX_REMOVABLE_STAGES: u64 = 262_144;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectRecoveryLimits {
    fanout_directories: u64,
    leaf_entries: u64,
    final_objects: u64,
    removable_stages: u64,
}

const fn object_recovery_limits() -> ObjectRecoveryLimits {
    ObjectRecoveryLimits {
        fanout_directories: OBJECT_RECOVERY_MAX_FANOUT_DIRECTORIES,
        leaf_entries: OBJECT_RECOVERY_MAX_LEAF_ENTRIES,
        final_objects: OBJECT_RECOVERY_MAX_FINAL_OBJECTS,
        removable_stages: OBJECT_RECOVERY_MAX_REMOVABLE_STAGES,
    }
}

#[derive(Default)]
struct ObjectRecoveryUsage {
    fanout_directories: u64,
    leaf_entries: u64,
    final_objects: u64,
    removable_stages: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObjectRecoveryLeafKind {
    Final,
    OwnedStage,
    Unknown,
}

/// Result alias for object-store operations.
pub type Result<T> = std::result::Result<T, StoreError>;

/// Caller-supplied canonical SCB1 verifier selected by schema epoch.
pub trait CanonicalVerifier {
    /// Verifies a complete standalone SCB1 object record and returns its object ID.
    ///
    /// # Errors
    ///
    /// Returns the exact stable `ScbError` produced by the canonical decoder.
    fn verify(&self, record: &[u8]) -> std::result::Result<ObjectId, ScbError>;
}

impl<F> CanonicalVerifier for F
where
    F: Fn(&[u8]) -> std::result::Result<ObjectId, ScbError>,
{
    fn verify(&self, record: &[u8]) -> std::result::Result<ObjectId, ScbError> {
        self(record)
    }
}

/// Stable object-store failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreErrorCode {
    /// Local host I/O failed.
    StoreIo,
    /// The requested object path does not exist.
    StoreObjectNotFound,
    /// The record is larger than the SCB1 epoch limit.
    ScbResourceLimit,
    /// The record is too short for, or disagrees with, its digest trailer.
    ScbDigestMismatch,
    /// Canonical SCB1 verification failed with this exact code.
    Scb(ScbErrorCode),
    /// A valid object was found where the declared or path-derived ID differs.
    StoreObjectSubstitution,
}

impl StoreErrorCode {
    /// Returns the stable symbolic code.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::StoreIo => "STORE_IO",
            Self::StoreObjectNotFound => "STORE_OBJECT_NOT_FOUND",
            Self::ScbResourceLimit => "SCB_RESOURCE_LIMIT",
            Self::ScbDigestMismatch => "SCB_DIGEST_MISMATCH",
            Self::Scb(code) => code.as_str(),
            Self::StoreObjectSubstitution => "STORE_OBJECT_SUBSTITUTION",
        }
    }
}

/// Object-store error with deterministic code precedence.
#[derive(Debug)]
pub struct StoreError {
    code: StoreErrorCode,
    source: Option<io::Error>,
}

impl StoreError {
    /// Creates an error from a stable store code.
    #[must_use]
    pub const fn new(code: StoreErrorCode) -> Self {
        Self { code, source: None }
    }

    /// Creates an I/O error preserving the host source.
    #[must_use]
    pub fn io(error: io::Error) -> Self {
        let code = if error.kind() == io::ErrorKind::NotFound {
            StoreErrorCode::StoreObjectNotFound
        } else {
            StoreErrorCode::StoreIo
        };
        Self {
            code,
            source: Some(error),
        }
    }

    /// Returns the stable failure code.
    #[must_use]
    pub const fn code(&self) -> StoreErrorCode {
        self.code
    }

    /// Returns the stable symbolic failure code.
    #[must_use]
    pub const fn symbol(&self) -> &'static str {
        self.code.symbol()
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

/// Result of writing an immutable object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PutStatus {
    /// The object was newly promoted.
    Promoted,
    /// A valid same-ID object was already present.
    Present,
}

/// Recovery event emitted for a removed staged object remnant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryEvent {
    /// Stable recovery event code.
    pub code: &'static str,
    /// Store-root-relative staging path that was inspected and removed.
    pub relative_path: PathBuf,
}

/// Immutable SCB1 object store rooted out of band.
#[derive(Clone, Debug)]
pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    /// Creates a store handle rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the configured store root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the final path for an object ID.
    #[must_use]
    pub fn object_path(&self, object_id: ObjectId) -> PathBuf {
        self.root.join(relative_object_path(object_id))
    }

    /// Returns the verified on-disk length without reading object content.
    ///
    /// # Errors
    ///
    /// Returns the exact path, file-kind, I/O, or standalone-size error.
    pub fn bounded_object_len(&self, object_id: ObjectId) -> Result<u64> {
        let path = self.verified_object_path(object_id)?;
        let metadata = fs::symlink_metadata(&path).map_err(StoreError::io)?;
        if !metadata.file_type().is_file() {
            return Err(StoreError::new(StoreErrorCode::StoreIo));
        }
        if metadata.len() > MAX_STANDALONE_BYTES as u64 {
            return Err(StoreError::new(StoreErrorCode::ScbResourceLimit));
        }
        Ok(metadata.len())
    }

    /// Reads and verifies an object by path-derived ID.
    ///
    /// # Errors
    ///
    /// Returns deterministic store or exact SCB1 verifier codes.
    pub fn read<V: CanonicalVerifier>(&self, object_id: ObjectId, verifier: &V) -> Result<Vec<u8>> {
        let path = self.verified_object_path(object_id)?;
        let record = bounded_read(&path)?;
        verify_record(&record, object_id, verifier)?;
        Ok(record)
    }

    /// Stages, verifies, and atomically promotes an immutable object.
    ///
    /// # Errors
    ///
    /// Returns deterministic store or exact SCB1 verifier codes. No failure
    /// promotes bytes or repairs an existing final object.
    pub fn put<V: CanonicalVerifier>(
        &self,
        declared_id: ObjectId,
        record: &[u8],
        verifier: &V,
    ) -> Result<PutStatus> {
        self.put_inner(declared_id, record, verifier)
    }

    fn put_inner<V: CanonicalVerifier>(
        &self,
        declared_id: ObjectId,
        record: &[u8],
        verifier: &V,
    ) -> Result<PutStatus> {
        verify_record(record, declared_id, verifier)?;

        let final_dir = self.ensure_object_dir(declared_id)?;
        let final_path = self.object_path(declared_id);

        if final_path.exists() {
            return Self::handle_existing(&final_path, declared_id, verifier);
        }

        #[cfg(test)]
        fail_selected_store_cut(|cut| {
            matches!(cut, StoreDurabilityCut::Obj01BeforeObjectStageWrite)
        })?;
        let (stage_path, mut stage) = reserve_stage_file(&final_dir, declared_id)?;
        #[cfg(test)]
        if take_selected_store_cut(|cut| {
            matches!(cut, StoreDurabilityCut::Obj02DuringObjectStageWrite)
        }) {
            let prefix_len = record.len() / 2;
            stage
                .write_all(&record[..prefix_len])
                .map_err(StoreError::io)?;
            stage.flush().map_err(StoreError::io)?;
            stage.sync_all().map_err(StoreError::io)?;
            return Err(StoreError::new(StoreErrorCode::StoreIo));
        }
        stage.write_all(record).map_err(StoreError::io)?;
        stage.flush().map_err(StoreError::io)?;
        stage.sync_all().map_err(StoreError::io)?;
        drop(stage);

        let staged_record = bounded_read(&stage_path)?;
        verify_record(&staged_record, declared_id, verifier)?;
        #[cfg(test)]
        fail_selected_store_cut(|cut| {
            matches!(
                cut,
                StoreDurabilityCut::Obj03VerifiedObjectStageBeforePromotion
            )
        })?;

        match fs::hard_link(&stage_path, &final_path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let status = Self::handle_existing(&final_path, declared_id, verifier)?;
                remove_file_if_exists(&stage_path)?;
                sync_dir(&final_dir)?;
                return Ok(status);
            }
            Err(error) => return Err(StoreError::io(error)),
        }
        #[cfg(test)]
        fail_selected_store_cut(|cut| {
            matches!(
                cut,
                StoreDurabilityCut::Obj04FinalObjectLinkBeforeFirstLeafSync
            )
        })?;
        sync_dir(&final_dir)?;
        #[cfg(test)]
        fail_selected_store_cut(|cut| {
            matches!(
                cut,
                StoreDurabilityCut::Obj05FirstLeafSyncBeforeObjectStageUnlink
            )
        })?;
        remove_file_if_exists(&stage_path)?;
        #[cfg(test)]
        fail_selected_store_cut(|cut| {
            matches!(
                cut,
                StoreDurabilityCut::Obj06ObjectStageUnlinkBeforeSecondLeafSync
            )
        })?;
        sync_dir(&final_dir)?;

        let final_record = bounded_read(&final_path)?;
        verify_record(&final_record, declared_id, verifier)?;
        Ok(PutStatus::Promoted)
    }

    fn ensure_object_dir(&self, object_id: ObjectId) -> Result<PathBuf> {
        ensure_existing_dir(&self.root)?;
        let hex = object_id_hex(object_id);
        let mut current = self.root.clone();
        for (component_index, component) in ["objects", "scb1", &hex[0..2], &hex[2..4]]
            .into_iter()
            .enumerate()
        {
            let next = current.join(component);
            create_dir_component(&current, &next, component_index)?;
            current = next;
        }
        Ok(current)
    }

    fn verified_object_path(&self, object_id: ObjectId) -> Result<PathBuf> {
        ensure_existing_dir(&self.root)?;
        let hex = object_id_hex(object_id);
        let mut current = self.root.clone();
        for component in ["objects", "scb1", &hex[0..2], &hex[2..4]] {
            current.push(component);
            ensure_existing_dir(&current)?;
        }
        Ok(current.join(format!("{hex}.scb1")))
    }

    fn handle_existing<V: CanonicalVerifier>(
        final_path: &Path,
        declared_id: ObjectId,
        verifier: &V,
    ) -> Result<PutStatus> {
        let existing = bounded_read(final_path)?;
        verify_record(&existing, declared_id, verifier)?;
        File::open(final_path)
            .and_then(|file| file.sync_all())
            .map_err(StoreError::io)?;
        let final_dir = final_path
            .parent()
            .ok_or_else(|| StoreError::new(StoreErrorCode::StoreIo))?;
        sync_dir(final_dir)?;
        Ok(PutStatus::Present)
    }

    /// Removes store-owned staging remnants and preserves final object paths.
    ///
    /// The caller must hold exclusive startup/recovery ownership of this store
    /// root. S20-150 intentionally does not define a cross-process lock.
    ///
    /// # Errors
    ///
    /// Returns `STORE_IO` if enumeration or removal fails.
    pub fn recover_staged(&self) -> Result<Vec<RecoveryEvent>> {
        self.recover_staged_with_limits(object_recovery_limits())
    }

    fn recover_staged_with_limits(
        &self,
        limits: ObjectRecoveryLimits,
    ) -> Result<Vec<RecoveryEvent>> {
        ensure_existing_dir(&self.root)?;
        let objects = self.root.join("objects");
        if !existing_dir_or_absent(&objects)? {
            return Ok(Vec::new());
        }
        let objects_dir = objects.join("scb1");
        if !existing_dir_or_absent(&objects_dir)? {
            return Ok(Vec::new());
        }

        let mut usage = ObjectRecoveryUsage::default();
        let mut pending_directories = vec![(objects_dir, 0_usize)];
        let mut leaf_directories = Vec::new();
        let mut final_objects = Vec::new();
        let mut removal_plan = Vec::new();

        while let Some((directory, depth)) = pending_directories.pop() {
            ensure_existing_dir(&directory)?;
            if depth == 2 {
                leaf_directories.push(directory.clone());
            }
            for entry in fs::read_dir(&directory).map_err(StoreError::io)? {
                let entry = entry.map_err(StoreError::io)?;
                if depth < 2 {
                    let classified_fanout_path = classify_object_recovery_fanout(&entry, depth)?;
                    let fanout_path = classified_fanout_path;
                    let one_fanout_directory = 1_u64;
                    let next_object_fanout_directories = usage
                        .fanout_directories
                        .checked_add(one_fanout_directory)
                        .ok_or_else(|| StoreError::new(StoreErrorCode::StoreIo))?;
                    ensure_object_recovery_limit(
                        next_object_fanout_directories,
                        limits.fanout_directories,
                    )?;
                    usage.fanout_directories = next_object_fanout_directories;
                    pending_directories.push(fanout_path);
                    continue;
                }

                let next_leaf_path = entry.path();
                let leaf_path = next_leaf_path;
                let one_leaf_entry = 1_u64;
                let next_object_leaf_entries = usage
                    .leaf_entries
                    .checked_add(one_leaf_entry)
                    .ok_or_else(|| StoreError::new(StoreErrorCode::StoreIo))?;
                ensure_object_recovery_limit(next_object_leaf_entries, limits.leaf_entries)?;
                usage.leaf_entries = next_object_leaf_entries;
                classify_object_recovery_leaf(&leaf_path)?;

                match object_recovery_leaf_kind(&leaf_path)? {
                    ObjectRecoveryLeafKind::Final => {
                        let classified_final_object = leaf_path;
                        let final_object = classified_final_object;
                        let one_final_object = 1_u64;
                        let next_final_objects = usage
                            .final_objects
                            .checked_add(one_final_object)
                            .ok_or_else(|| StoreError::new(StoreErrorCode::StoreIo))?;
                        ensure_object_recovery_limit(next_final_objects, limits.final_objects)?;
                        usage.final_objects = next_final_objects;
                        final_objects.push(final_object);
                    }
                    ObjectRecoveryLeafKind::OwnedStage => {
                        let classified_owned_stage_path = leaf_path;
                        let stage_path = classified_owned_stage_path;
                        let one_removable_stage = 1_u64;
                        let next_object_stages = usage
                            .removable_stages
                            .checked_add(one_removable_stage)
                            .ok_or_else(|| StoreError::new(StoreErrorCode::StoreIo))?;
                        ensure_object_recovery_limit(next_object_stages, limits.removable_stages)?;
                        usage.removable_stages = next_object_stages;
                        removal_plan.push(stage_path);
                    }
                    ObjectRecoveryLeafKind::Unknown => {}
                }
            }
        }

        final_objects.sort();
        leaf_directories.sort();
        removal_plan.sort();
        let mut events = Vec::new();
        for stage_path in removal_plan {
            let metadata = match fs::symlink_metadata(&stage_path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(StoreError::io(error)),
            };
            if !metadata.file_type().is_file() {
                return Err(StoreError::new(StoreErrorCode::StoreIo));
            }
            let relative_path = stage_path
                .strip_prefix(&self.root)
                .map_err(|_| StoreError::new(StoreErrorCode::StoreIo))?
                .to_path_buf();
            fs::remove_file(&stage_path).map_err(StoreError::io)?;
            #[cfg(test)]
            fail_selected_object_recovery_cut(&stage_path)?;
            events.push(RecoveryEvent {
                code: "RECOVERY_STAGED_OBJECT",
                relative_path,
            });
        }
        for directory in leaf_directories {
            ensure_existing_dir(&directory)?;
            sync_dir(&directory)?;
        }
        events.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(events)
    }

    #[cfg(test)]
    fn put_with_store_durability_cut<V: CanonicalVerifier>(
        &self,
        declared_id: ObjectId,
        record: &[u8],
        verifier: &V,
        cut: StoreDurabilityCut,
    ) -> Result<PutStatus> {
        let _selection = StoreCutSelection::install(cut);
        self.put_inner(declared_id, record, verifier)
    }

    #[cfg(test)]
    fn recover_staged_with_store_durability_cut(
        &self,
        cut: StoreDurabilityCut,
    ) -> Result<Vec<RecoveryEvent>> {
        let _selection = StoreCutSelection::install(cut);
        self.recover_staged()
    }
}

/// Returns the store-root-relative final path for an object ID.
#[must_use]
pub fn relative_object_path(object_id: ObjectId) -> PathBuf {
    let hex = object_id_hex(object_id);
    Path::new("objects")
        .join("scb1")
        .join(&hex[0..2])
        .join(&hex[2..4])
        .join(format!("{hex}.scb1"))
}

/// Returns lowercase hexadecimal raw `ObjectId` bytes.
#[must_use]
pub fn object_id_hex(object_id: ObjectId) -> String {
    let mut out = String::with_capacity(64);
    for byte in object_id.as_bytes() {
        use fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn verify_record<V: CanonicalVerifier>(
    record: &[u8],
    declared_id: ObjectId,
    verifier: &V,
) -> Result<ObjectId> {
    let derived = verify_digest_trailer(record)?;
    if derived != declared_id {
        return Err(StoreError::new(StoreErrorCode::StoreObjectSubstitution));
    }
    let verified_id = verifier
        .verify(record)
        .map_err(|error| map_scb_error(error.code()))?;
    if verified_id != declared_id {
        return Err(StoreError::new(StoreErrorCode::StoreObjectSubstitution));
    }
    Ok(derived)
}

fn verify_digest_trailer(record: &[u8]) -> Result<ObjectId> {
    if record.len() > MAX_STANDALONE_BYTES {
        return Err(StoreError::new(StoreErrorCode::ScbResourceLimit));
    }
    if record.len() < DIGEST_TRAILER_LEN {
        return Err(StoreError::new(StoreErrorCode::ScbDigestMismatch));
    }
    let preimage_len = record.len() - DIGEST_TRAILER_LEN;
    let derived = ObjectId::derive(&record[..preimage_len]);
    if record[preimage_len..] != *derived.as_bytes() {
        return Err(StoreError::new(StoreErrorCode::ScbDigestMismatch));
    }
    Ok(derived)
}

fn map_scb_error(code: ScbErrorCode) -> StoreError {
    match code {
        ScbErrorCode::ResourceLimit => StoreError::new(StoreErrorCode::ScbResourceLimit),
        ScbErrorCode::DigestMismatch => StoreError::new(StoreErrorCode::ScbDigestMismatch),
        other => StoreError::new(StoreErrorCode::Scb(other)),
    }
}

fn bounded_read(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(StoreError::io)?;
    if !metadata.file_type().is_file() {
        return Err(StoreError::new(StoreErrorCode::StoreIo));
    }
    if metadata.len() > MAX_STANDALONE_BYTES as u64 {
        return Err(StoreError::new(StoreErrorCode::ScbResourceLimit));
    }
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| StoreError::new(StoreErrorCode::ScbResourceLimit))?;
    let mut file = File::open(path).map_err(StoreError::io)?;
    let mut record = Vec::with_capacity(capacity);
    let limit = u64::try_from(MAX_STANDALONE_BYTES)
        .map_err(|_| StoreError::new(StoreErrorCode::ScbResourceLimit))?
        + 1;
    let mut reader = Read::by_ref(&mut file).take(limit);
    reader.read_to_end(&mut record).map_err(StoreError::io)?;
    if record.len() > MAX_STANDALONE_BYTES {
        return Err(StoreError::new(StoreErrorCode::ScbResourceLimit));
    }
    Ok(record)
}

fn stage_path(final_dir: &Path, object_id: ObjectId, counter: u32) -> PathBuf {
    let pid = std::process::id();
    final_dir.join(format!(
        "{STAGE_PREFIX}{}{pid:08x}{counter:08x}{STAGE_SUFFIX}",
        object_id_hex(object_id)
    ))
}

fn reserve_stage_file(final_dir: &Path, object_id: ObjectId) -> Result<(PathBuf, File)> {
    for counter in 0_u32..=u32::MAX {
        let path = stage_path(final_dir, object_id, counter);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(StoreError::io(error)),
        }
    }
    Err(StoreError::new(StoreErrorCode::StoreIo))
}

fn create_dir_component(parent: &Path, path: &Path, component_index: usize) -> Result<()> {
    let _ = component_index;
    match fs::create_dir(path) {
        Ok(()) => {
            #[cfg(test)]
            fail_selected_store_layout_cut(component_index)?;
            sync_dir(parent)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            ensure_existing_dir(path)?;
            sync_dir(parent)
        }
        Err(error) => Err(StoreError::io(error)),
    }
}

fn ensure_existing_dir(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(StoreError::io)?;
    if metadata.file_type().is_dir() {
        Ok(())
    } else {
        Err(StoreError::new(StoreErrorCode::StoreIo))
    }
}

fn existing_dir_or_absent(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(true),
        Ok(_) => Err(StoreError::new(StoreErrorCode::StoreIo)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(StoreError::io(error)),
    }
}

fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|dir| dir.sync_all())
        .map_err(StoreError::io)
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(StoreError::io(error)),
    }
}

fn ensure_object_recovery_limit(value: u64, limit: u64) -> Result<()> {
    if value > limit {
        return Err(StoreError::new(StoreErrorCode::StoreIo));
    }
    Ok(())
}

fn classify_object_recovery_fanout(entry: &fs::DirEntry, depth: usize) -> Result<(PathBuf, usize)> {
    if depth >= 2
        || !entry.file_type().map_err(StoreError::io)?.is_dir()
        || !is_hex_dir_name(&entry.file_name())
    {
        return Err(StoreError::new(StoreErrorCode::StoreIo));
    }
    Ok((entry.path(), depth + 1))
}

fn classify_object_recovery_leaf(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(StoreError::io)?;
    if !metadata.file_type().is_file() {
        return Err(StoreError::new(StoreErrorCode::StoreIo));
    }
    Ok(())
}

fn object_recovery_leaf_kind(path: &Path) -> Result<ObjectRecoveryLeafKind> {
    let name = path
        .file_name()
        .ok_or_else(|| StoreError::new(StoreErrorCode::StoreIo))?;
    let directory = path
        .parent()
        .ok_or_else(|| StoreError::new(StoreErrorCode::StoreIo))?;
    if is_final_object_name(name) {
        if !is_final_object_name_for_dir(name, directory) {
            return Err(StoreError::new(StoreErrorCode::StoreIo));
        }
        return Ok(ObjectRecoveryLeafKind::Final);
    }
    if is_stage_name_for_dir(name, directory) {
        return Ok(ObjectRecoveryLeafKind::OwnedStage);
    }
    Ok(ObjectRecoveryLeafKind::Unknown)
}

fn is_final_object_name(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(object_id) = name.strip_suffix(FINAL_SUFFIX) else {
        return false;
    };
    object_id.len() == FINAL_OBJECT_ID_HEX_LEN && object_id.bytes().all(is_lower_hex)
}

fn is_final_object_name_for_dir(name: &std::ffi::OsStr, dir: &Path) -> bool {
    if !is_final_object_name(name) {
        return false;
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    let object_id = &name[..name.len() - FINAL_SUFFIX.len()];
    let Some(second) = dir.file_name().and_then(std::ffi::OsStr::to_str) else {
        return false;
    };
    let Some(first) = dir
        .parent()
        .and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
    else {
        return false;
    };
    first == &object_id[0..2] && second == &object_id[2..4]
}

fn is_stage_name(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(token) = name
        .strip_prefix(STAGE_PREFIX)
        .and_then(|name| name.strip_suffix(STAGE_SUFFIX))
    else {
        return false;
    };
    if token.len() != STAGE_TOKEN_HEX_LEN || !token.bytes().all(is_lower_hex) {
        return false;
    }
    u32::from_str_radix(&token[64..72], 16).is_ok_and(|pid| pid > 0)
}

fn is_stage_name_for_dir(name: &std::ffi::OsStr, dir: &Path) -> bool {
    if !is_stage_name(name) {
        return false;
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    let token = &name[STAGE_PREFIX.len()..name.len() - STAGE_SUFFIX.len()];
    let Some(second) = dir.file_name().and_then(std::ffi::OsStr::to_str) else {
        return false;
    };
    let Some(first) = dir
        .parent()
        .and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
    else {
        return false;
    };
    first == &token[0..2] && second == &token[2..4]
}

fn is_hex_dir_name(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    name.len() == 2 && name.bytes().all(is_lower_hex)
}

const fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (byte >= b'a' && byte <= b'f')
}

#[cfg(test)]
enum StoreDurabilityCut {
    Obj01BeforeObjectStageWrite,
    Obj02DuringObjectStageWrite,
    Obj03VerifiedObjectStageBeforePromotion,
    Obj04FinalObjectLinkBeforeFirstLeafSync,
    Obj05FirstLeafSyncBeforeObjectStageUnlink,
    Obj06ObjectStageUnlinkBeforeSecondLeafSync,
    Obj07RecoveryObjectStageUnlinkBeforeLeafSync { object_id: ObjectId },
    Olay01Scb1DirectoryCreateBeforeObjectsSync,
    Olay02FirstObjectFanoutCreateBeforeParentSync,
    Olay03SecondObjectFanoutCreateBeforeParentSync,
}

#[cfg(test)]
std::thread_local! {
    static SELECTED_STORE_CUT: std::cell::RefCell<Option<StoreDurabilityCut>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
struct StoreCutSelection;

#[cfg(test)]
impl StoreCutSelection {
    fn install(cut: StoreDurabilityCut) -> Self {
        SELECTED_STORE_CUT.with(|selected| {
            let previous = selected.replace(Some(cut));
            assert!(
                previous.is_none(),
                "store durability selection is not nested"
            );
        });
        Self
    }
}

#[cfg(test)]
impl Drop for StoreCutSelection {
    fn drop(&mut self) {
        SELECTED_STORE_CUT.with(|selected| {
            selected.replace(None);
        });
    }
}

#[cfg(test)]
fn take_selected_store_cut(predicate: impl FnOnce(&StoreDurabilityCut) -> bool) -> bool {
    SELECTED_STORE_CUT.with(|selected| {
        let take = selected.borrow().as_ref().is_some_and(predicate);
        if take {
            selected.borrow_mut().take();
        }
        take
    })
}

#[cfg(test)]
fn fail_selected_store_cut(predicate: impl FnOnce(&StoreDurabilityCut) -> bool) -> Result<()> {
    if take_selected_store_cut(predicate) {
        return Err(StoreError::new(StoreErrorCode::StoreIo));
    }
    Ok(())
}

#[cfg(test)]
fn fail_selected_store_layout_cut(component_index: usize) -> Result<()> {
    fail_selected_store_cut(|cut| {
        matches!(
            (component_index, cut),
            (
                1,
                StoreDurabilityCut::Olay01Scb1DirectoryCreateBeforeObjectsSync
            ) | (
                2,
                StoreDurabilityCut::Olay02FirstObjectFanoutCreateBeforeParentSync
            ) | (
                3,
                StoreDurabilityCut::Olay03SecondObjectFanoutCreateBeforeParentSync
            )
        )
    })
}

#[cfg(test)]
fn fail_selected_object_recovery_cut(stage_path: &Path) -> Result<()> {
    let Some(name) = stage_path.file_name().and_then(std::ffi::OsStr::to_str) else {
        return Ok(());
    };
    let Some(token) = name
        .strip_prefix(STAGE_PREFIX)
        .and_then(|name| name.strip_suffix(STAGE_SUFFIX))
    else {
        return Ok(());
    };
    fail_selected_store_cut(|cut| {
        let StoreDurabilityCut::Obj07RecoveryObjectStageUnlinkBeforeLeafSync { object_id } = cut
        else {
            return false;
        };
        token[..64] == object_id_hex(*object_id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sley_scb1::{
        FixtureContract, decode_standalone_fixture, encode_bool, encode_record,
        encode_standalone_fixture,
    };

    struct TempDir {
        path: ::std::path::PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let mut path = ::std::env::temp_dir();
            path.push(::std::format!(
                "sley-store-{name}-{}-{}",
                ::std::process::id(),
                unique_counter()
            ));
            ::std::fs::create_dir(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &::std::path::Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = ::std::fs::remove_dir_all(&self.path);
        }
    }

    fn unique_counter() -> u64 {
        static NEXT: ::std::sync::atomic::AtomicU64 = ::std::sync::atomic::AtomicU64::new(0);
        NEXT.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
    }

    fn bool_record(value: bool) -> (Vec<u8>, ObjectId) {
        let payload = encode_record(&[(1, encode_bool(value))]).expect("payload encodes");
        encode_standalone_fixture(FixtureContract::RequiredBool, &payload).expect("fixture encodes")
    }

    fn empty_record() -> (Vec<u8>, ObjectId) {
        let payload = encode_record(&[]).expect("payload encodes");
        encode_standalone_fixture(FixtureContract::EmptyObject, &payload).expect("fixture encodes")
    }

    fn verifier(record: &[u8]) -> std::result::Result<ObjectId, ScbError> {
        decode_standalone_fixture(record, FixtureContract::RequiredBool)
            .map(|fixture| fixture.object_id)
    }

    fn any_fixture_verifier(record: &[u8]) -> std::result::Result<ObjectId, ScbError> {
        decode_standalone_fixture(record, FixtureContract::RequiredBool)
            .or_else(|_| decode_standalone_fixture(record, FixtureContract::EmptyObject))
            .map(|fixture| fixture.object_id)
    }

    #[test]
    fn write_read_round_trip_and_exact_path_derivation() {
        let temp = TempDir::new("round-trip");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);

        assert_eq!(
            store.put(object_id, &record, &verifier).unwrap(),
            PutStatus::Promoted
        );
        assert_eq!(store.read(object_id, &verifier).unwrap(), record);

        let hex = object_id_hex(object_id);
        assert_eq!(
            relative_object_path(object_id),
            Path::new("objects")
                .join("scb1")
                .join(&hex[0..2])
                .join(&hex[2..4])
                .join(format!("{hex}.scb1"))
        );
        assert!(store.object_path(object_id).is_file());
    }

    #[test]
    fn same_object_idempotence_does_not_replace_existing_bytes() {
        let temp = TempDir::new("idempotent");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);

        assert_eq!(
            store.put(object_id, &record, &verifier).unwrap(),
            PutStatus::Promoted
        );
        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt as _;
            store.object_path(object_id).metadata().unwrap().ino()
        };
        assert_eq!(
            store.put(object_id, &record, &verifier).unwrap(),
            PutStatus::Present
        );
        assert_eq!(fs::read(store.object_path(object_id)).unwrap(), record);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            assert_eq!(
                store.object_path(object_id).metadata().unwrap().ino(),
                inode
            );
        }
    }

    #[test]
    fn payload_and_trailer_corruption_return_digest_mismatch() {
        let temp = TempDir::new("corrupt");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);

        let mut payload_corrupt = record.clone();
        payload_corrupt[10] ^= 1;
        assert_eq!(
            store
                .put(object_id, &payload_corrupt, &verifier)
                .unwrap_err()
                .code(),
            StoreErrorCode::ScbDigestMismatch
        );

        let mut trailer_corrupt = record;
        let last = trailer_corrupt.len() - 1;
        trailer_corrupt[last] ^= 1;
        assert_eq!(
            store
                .put(object_id, &trailer_corrupt, &verifier)
                .unwrap_err()
                .code(),
            StoreErrorCode::ScbDigestMismatch
        );
    }

    #[test]
    fn different_valid_object_at_target_path_is_substitution() {
        let temp = TempDir::new("substitution");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);
        let (different, _different_id) = bool_record(false);

        let path = store.object_path(object_id);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &different).unwrap();

        assert_eq!(
            store.put(object_id, &record, &verifier).unwrap_err().code(),
            StoreErrorCode::StoreObjectSubstitution
        );
        assert_eq!(fs::read(path).unwrap(), different);
    }

    #[test]
    fn declared_id_substitution_is_detected_before_staging() {
        let temp = TempDir::new("declared-substitution");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);
        let (_other, other_id) = bool_record(false);

        assert_eq!(
            store.put(other_id, &record, &verifier).unwrap_err().code(),
            StoreErrorCode::StoreObjectSubstitution
        );
        assert!(!store.object_path(object_id).exists());
        assert!(!temp.path().join("objects").exists());
    }

    #[test]
    fn exclusive_staging_collision_retries_without_overwrite() {
        let temp = TempDir::new("stage-exclusive");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);
        let dir = store.ensure_object_dir(object_id).unwrap();
        let collision = stage_path(&dir, object_id, 0);
        fs::write(&collision, b"preexisting-stage").unwrap();

        assert_eq!(
            store
                .put(object_id, &record, &verifier)
                .expect("collision retries"),
            PutStatus::Promoted
        );
        assert_eq!(fs::read(&collision).unwrap(), b"preexisting-stage");
        assert_eq!(store.recover_staged().unwrap().len(), 1);
        assert_eq!(store.read(object_id, &verifier).unwrap(), record);
    }

    #[test]
    fn concurrent_same_object_writers_all_resolve_without_store_io() {
        use std::sync::{Arc, Barrier};

        const WRITERS: usize = 8;
        let temp = TempDir::new("concurrent-writers");
        let store = Arc::new(ObjectStore::new(temp.path()));
        let (record, object_id) = bool_record(true);
        let record = Arc::new(record);
        let barrier = Arc::new(Barrier::new(WRITERS));
        let mut threads = Vec::new();
        for _ in 0..WRITERS {
            let store = Arc::clone(&store);
            let record = Arc::clone(&record);
            let barrier = Arc::clone(&barrier);
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                store.put(object_id, record.as_slice(), &verifier)
            }));
        }

        let statuses: Vec<PutStatus> = threads
            .into_iter()
            .map(|thread| {
                thread
                    .join()
                    .expect("writer does not panic")
                    .expect("put succeeds")
            })
            .collect();
        assert_eq!(
            statuses
                .iter()
                .filter(|status| **status == PutStatus::Promoted)
                .count(),
            1
        );
        assert_eq!(
            statuses
                .iter()
                .filter(|status| **status == PutStatus::Present)
                .count(),
            WRITERS - 1
        );
        assert_eq!(store.read(object_id, &verifier).unwrap(), *record);
        assert!(store.recover_staged().unwrap().is_empty());
    }

    #[test]
    fn atomic_no_overwrite_promotion_handles_racing_existing_object() {
        let temp = TempDir::new("promotion-race");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);
        let final_path = store.object_path(object_id);
        let calls = std::cell::Cell::new(0_u8);
        let verifier = |bytes: &[u8]| {
            calls.set(calls.get() + 1);
            if calls.get() == 2 && !final_path.exists() {
                fs::write(&final_path, &record).expect("race final write");
            }
            decode_standalone_fixture(bytes, FixtureContract::RequiredBool)
                .map(|fixture| fixture.object_id)
        };

        assert_eq!(
            store.put(object_id, &record, &verifier).unwrap(),
            PutStatus::Present
        );
        assert_eq!(fs::read(store.object_path(object_id)).unwrap(), record);
        assert!(
            fs::read_dir(store.object_path(object_id).parent().unwrap())
                .unwrap()
                .all(|entry| !is_stage_name(&entry.unwrap().file_name()))
        );
    }

    #[test]
    fn interruption_before_promotion_leaves_only_stage_and_no_final_object() {
        let temp = TempDir::new("before-promote");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);

        assert_eq!(
            store
                .put_with_store_durability_cut(
                    object_id,
                    &record,
                    &verifier,
                    StoreDurabilityCut::Obj03VerifiedObjectStageBeforePromotion,
                )
                .unwrap_err()
                .code(),
            StoreErrorCode::StoreIo
        );
        assert!(!store.object_path(object_id).exists());
        let events = store.recover_staged().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code, "RECOVERY_STAGED_OBJECT");
        assert!(!store.object_path(object_id).exists());
    }

    #[test]
    fn interruption_after_promotion_before_cleanup_preserves_final_and_reports_stage() {
        let temp = TempDir::new("after-promote");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);

        assert_eq!(
            store
                .put_with_store_durability_cut(
                    object_id,
                    &record,
                    &verifier,
                    StoreDurabilityCut::Obj05FirstLeafSyncBeforeObjectStageUnlink,
                )
                .unwrap_err()
                .code(),
            StoreErrorCode::StoreIo
        );
        assert_eq!(fs::read(store.object_path(object_id)).unwrap(), record);
        let events = store.recover_staged().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(fs::read(store.object_path(object_id)).unwrap(), record);
    }

    #[test]
    fn recovery_removes_only_store_owned_stage_files() {
        let temp = TempDir::new("recovery");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);
        assert_eq!(
            store.put(object_id, &record, &verifier).unwrap(),
            PutStatus::Promoted
        );

        let dir = store.object_path(object_id).parent().unwrap().to_path_buf();
        let owned_stage = stage_path(&dir, object_id, 0);
        fs::write(&owned_stage, b"stage").unwrap();
        let lookalike = dir.join(format!("{STAGE_PREFIX}abc{STAGE_SUFFIX}"));
        fs::write(&lookalike, b"keep").unwrap();
        fs::write(dir.join("not-stage.tmp"), b"keep").unwrap();

        let events = store.recover_staged().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code, "RECOVERY_STAGED_OBJECT");
        assert!(!owned_stage.exists());
        assert!(lookalike.exists());
        assert_eq!(fs::read(store.object_path(object_id)).unwrap(), record);
        assert!(dir.join("not-stage.tmp").exists());
    }

    #[test]
    fn recovery_events_are_sorted_by_relative_path() {
        let temp = TempDir::new("recovery-order");
        let store = ObjectStore::new(temp.path());
        let (first_record, first_id) = bool_record(true);
        let (second_record, second_id) = bool_record(false);
        for (record, object_id) in [(first_record, first_id), (second_record, second_id)] {
            let dir = store.object_path(object_id).parent().unwrap().to_path_buf();
            fs::create_dir_all(&dir).unwrap();
            fs::write(stage_path(&dir, object_id, 0), record).unwrap();
        }

        let events = store.recover_staged().unwrap();
        assert_eq!(events.len(), 2);
        assert!(events[0].relative_path < events[1].relative_path);
    }

    #[cfg(unix)]
    #[test]
    fn bounded_read_rejects_symlink_object_paths() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new("symlink");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);
        let outside = temp.path().join("outside.scb1");
        fs::write(&outside, record).unwrap();
        let path = store.object_path(object_id);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(&outside, &path).unwrap();

        assert_eq!(
            store.read(object_id, &verifier).unwrap_err().code(),
            StoreErrorCode::StoreIo
        );
    }

    #[cfg(unix)]
    #[test]
    fn store_root_and_fanout_symlinks_fail_closed() {
        use std::os::unix::fs::symlink;

        fn assert_no_escape(store: &ObjectStore, outside: &Path) {
            let (record, object_id) = bool_record(true);
            assert_eq!(
                store.put(object_id, &record, &verifier).unwrap_err().code(),
                StoreErrorCode::StoreIo
            );
            assert_eq!(
                store.read(object_id, &verifier).unwrap_err().code(),
                StoreErrorCode::StoreIo
            );
            assert_eq!(
                store.recover_staged().unwrap_err().code(),
                StoreErrorCode::StoreIo
            );
            assert!(!store.object_path(object_id).exists());
            assert!(fs::read_dir(outside).unwrap().next().is_none());
        }

        let root_case = TempDir::new("symlink-root");
        let root_target = root_case.path().join("outside-root");
        let linked_root = root_case.path().join("store-root");
        fs::create_dir(&root_target).unwrap();
        symlink(&root_target, &linked_root).unwrap();
        assert_no_escape(&ObjectStore::new(&linked_root), &root_target);

        let objects_case = TempDir::new("symlink-objects");
        let objects_target = objects_case.path().join("outside-objects");
        fs::create_dir(&objects_target).unwrap();
        symlink(&objects_target, objects_case.path().join("objects")).unwrap();
        assert_no_escape(&ObjectStore::new(objects_case.path()), &objects_target);

        let scb1_case = TempDir::new("symlink-scb1");
        let scb1_target = scb1_case.path().join("outside-scb1");
        fs::create_dir(&scb1_target).unwrap();
        fs::create_dir(scb1_case.path().join("objects")).unwrap();
        symlink(&scb1_target, scb1_case.path().join("objects/scb1")).unwrap();
        assert_no_escape(&ObjectStore::new(scb1_case.path()), &scb1_target);
    }

    #[test]
    fn bounded_read_rejects_over_limit_by_metadata() {
        let temp = TempDir::new("limit");
        let path = temp.path().join("oversized.scb1");
        let file = File::create(&path).unwrap();
        file.set_len(MAX_STANDALONE_BYTES as u64 + 1).unwrap();

        assert_eq!(
            bounded_read(&path).unwrap_err().code(),
            StoreErrorCode::ScbResourceLimit
        );
    }

    #[test]
    fn exact_maximum_size_is_allowed_until_digest_verification() {
        let temp = TempDir::new("exact-max");
        let path = temp.path().join("max.scb1");
        let file = File::create(&path).unwrap();
        file.set_len(MAX_STANDALONE_BYTES as u64).unwrap();

        let record = bounded_read(&path).unwrap();
        assert_eq!(record.len(), MAX_STANDALONE_BYTES);
        assert_eq!(
            verify_digest_trailer(&record).unwrap_err().code(),
            StoreErrorCode::ScbDigestMismatch
        );
    }

    #[test]
    fn randomized_invalid_records_never_promote() {
        let temp = TempDir::new("random-invalid");
        let store = ObjectStore::new(temp.path());
        let (_record, object_id) = bool_record(true);
        let mut seed = 0x5eed_5eed_u64;
        for len in 0..128 {
            let mut invalid = Vec::with_capacity(len);
            for _ in 0..len {
                seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                invalid.push(seed.to_le_bytes()[4]);
            }
            let _ = store.put(object_id, &invalid, &verifier);
            assert!(!store.object_path(object_id).exists());
        }
    }

    #[test]
    fn t03_seed_hash_mismatch_assertion_is_effective() {
        let (_record, object_id) = bool_record(true);
        let mut too_short = object_id.as_bytes().to_vec();
        too_short.pop();
        assert_eq!(
            verify_record(&too_short, object_id, &verifier)
                .unwrap_err()
                .symbol(),
            "SCB_DIGEST_MISMATCH"
        );
    }

    #[test]
    fn t04_seed_wrong_preimage_assertion_is_effective() {
        let (record, _object_id) = bool_record(true);
        let (_different, different_id) = bool_record(false);
        assert_eq!(
            verify_record(&record, different_id, &verifier)
                .unwrap_err()
                .symbol(),
            "STORE_OBJECT_SUBSTITUTION"
        );
    }

    #[test]
    fn t37_seed_recovery_event_assertion_is_effective() {
        let temp = TempDir::new("t37");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = bool_record(true);
        let dir = store.object_path(object_id).parent().unwrap().to_path_buf();
        fs::create_dir_all(&dir).unwrap();
        fs::write(stage_path(&dir, object_id, 0), &record).unwrap();

        let events = store.recover_staged().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code, "RECOVERY_STAGED_OBJECT");
    }

    #[test]
    fn canonical_verifier_boundary_preserves_exact_scb_code() {
        let temp = TempDir::new("scb-code");
        let store = ObjectStore::new(temp.path());
        let (record, object_id) = empty_record();

        assert_eq!(
            store
                .put(object_id, &record, &any_fixture_verifier)
                .expect("empty fixture accepted by broad verifier"),
            PutStatus::Promoted
        );
        assert_eq!(
            store.read(object_id, &verifier).unwrap_err().code(),
            StoreErrorCode::Scb(ScbErrorCode::ContractUnknown)
        );
    }

    #[test]
    fn read_missing_object_uses_not_found_code() {
        let temp = TempDir::new("missing");
        let store = ObjectStore::new(temp.path());
        let (_record, object_id) = bool_record(true);

        assert_eq!(
            store.read(object_id, &verifier).unwrap_err().code(),
            StoreErrorCode::StoreObjectNotFound
        );
    }
}
