//! Repository-owned complete-root index cache (S20-300 full, contract
//! `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` section 5).
//!
//! The one place an index snapshot is reused without a rebuild. A cached
//! record is accepted only under the four cheap rules of the contract and
//! serves read-only derived query surfaces, plus (profile revision 4 and
//! later) the read-only identity probe whose one consumer is the SMP1
//! `workspace.open` snapshot identity; validation, comparison, merge,
//! commit, exchange, GC, and recovery never read it. Files are derived and
//! disposable.

use core::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sley_id::{IndexSnapshotId, StateRoot};
use sley_query::{
    CacheDiscardReason, IndexSnapshot, IndexSnapshotBuildError, IndexSnapshotError,
    IndexSnapshotErrorCode, MAX_SNAPSHOT_RECORD_BYTES, SnapshotContext,
    build_complete_root_snapshot, decode_complete_root_snapshot, discard_reason,
};
use sley_txn::{RepositoryMaintenanceGuard, VerifiedRevision};

use crate::CompleteRootRequest;
use crate::complete_root::CompleteRootError;

const INDEX_DIRECTORY: &str = "index";
const INDEX_VERSION_DIRECTORY: &str = "v1";
const INDEX_SUFFIX: &str = ".idx.scb1";

/// Cache failure with the exact wrapped code preserved.
#[derive(Debug)]
pub enum IndexCacheError {
    /// The fresh build failed (`INDEX_SNAPSHOT_*`, wrapping `IMPACT_*`).
    Snapshot(IndexSnapshotBuildError),
    /// Extraction from the verified revision failed.
    Extraction(CompleteRootError),
    /// Cache read or write failed and no fresh snapshot could be returned.
    Io(std::io::Error),
}

impl IndexCacheError {
    /// Returns the stable symbolic code.
    #[must_use]
    pub fn code(&self) -> String {
        match self {
            Self::Snapshot(IndexSnapshotBuildError::Impact(_)) => {
                IndexSnapshotErrorCode::RootIncomplete.as_str().to_owned()
            }
            Self::Snapshot(IndexSnapshotBuildError::Snapshot(error)) => {
                error.code().as_str().to_owned()
            }
            Self::Extraction(error) => error.code().to_owned(),
            Self::Io(_) => IndexSnapshotErrorCode::RootIo.as_str().to_owned(),
        }
    }
}

impl fmt::Display for IndexCacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.code())
    }
}

impl std::error::Error for IndexCacheError {}

impl From<std::io::Error> for IndexCacheError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Outcome of a cache lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheOutcome {
    /// The cached record was accepted without decoding objects.
    Hit,
    /// The record was absent or discarded; the fresh build was written.
    Rebuilt(CacheDiscardReason),
}

/// Returns the cache file path for one root.
#[must_use]
pub fn index_cache_path(repository: &Path, root: StateRoot) -> PathBuf {
    let hex: String = root
        .as_bytes()
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            use core::fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
            out
        });
    repository
        .join(INDEX_DIRECTORY)
        .join(INDEX_VERSION_DIRECTORY)
        .join(format!("{hex}{INDEX_SUFFIX}"))
}

fn context_for(revision: &VerifiedRevision) -> SnapshotContext {
    SnapshotContext {
        schema_epoch: revision.state_root().record.schema_epoch_id,
        claimed_root_context: Some(revision.state_root().root),
    }
}

/// Accepts a cached record under the contract's four rules, or reports why
/// it is discarded. Reads no object and extracts no edge.
fn accept_cached(
    revision: &VerifiedRevision,
    record: &[u8],
) -> Result<IndexSnapshot, IndexSnapshotError> {
    let snapshot = decode_complete_root_snapshot(context_for(revision), record)?;
    let bindings = &revision.state_root().record.entity_bindings;
    let aligned = snapshot.inventory().len() == bindings.len()
        && snapshot
            .inventory()
            .iter()
            .zip(bindings)
            .all(|(entry, (bound, _))| entry.entity == *bound);
    if !aligned {
        return Err(IndexSnapshotError::new(
            IndexSnapshotErrorCode::RootMismatch,
        ));
    }
    Ok(snapshot)
}

/// Builds a fresh snapshot from a verified revision without touching the
/// cache. Exported evidence (capsules) builds from this path, never from
/// a bare cache hit.
pub(crate) fn fresh_snapshot(
    revision: &VerifiedRevision,
) -> Result<IndexSnapshot, IndexCacheError> {
    let request = CompleteRootRequest::extract(revision).map_err(IndexCacheError::Extraction)?;
    let borrowed = request.borrowed();
    build_complete_root_snapshot(
        revision.state_root().record.schema_epoch_id,
        revision.state_root().root,
        &borrowed,
        request.facts(),
    )
    .map_err(IndexCacheError::Snapshot)
}

fn discard_reason_of(error: &IndexSnapshotError) -> CacheDiscardReason {
    // The decoder never emits the judgment and I/O codes, so those arms
    // are defensive: kept for exhaustiveness over the code enum, mapping
    // to the format reason rather than panicking if the decoder ever
    // grows a code.
    discard_reason(error.code())
}

static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

fn write_record(path: &Path, record: &[u8]) -> Result<(), IndexCacheError> {
    let directory = path
        .parent()
        .ok_or_else(|| std::io::Error::other("index cache path has no parent"))?;
    fs::create_dir_all(directory)?;
    // Unique temporary names with exclusive creation: a fixed `.tmp` name
    // lets a planted symlink redirect the write through to another file,
    // and lets concurrent writers tear each other. The process id plus a
    // process counter makes the name unpredictable within the cache
    // directory, and `create_new` refuses anything already there
    // (including a planted symlink) instead of following it.
    let temporary = path.with_file_name(format!(
        "{}.tmp.{}.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| std::io::Error::other("index cache file name is not text"))?,
        std::process::id(),
        TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(record)?;
        file.sync_all()?;
    }
    fs::rename(&temporary, path)?;
    File::open(directory)?.sync_all()?;
    Ok(())
}

/// Returns the complete-root snapshot for a verified revision: the cached
/// record when the four acceptance rules hold, otherwise a fresh build that
/// is written back by temp-and-rename.
///
/// The caller MUST hold shared repository maintenance over `repository`
/// (the contract's "under shared repository maintenance"): the guard pins
/// the lock boundary against a concurrent import purge or exclusive owner
/// while the cache is read and written. Cache I/O trouble is fail-open —
/// a fresh build is returned whenever one can be built — while a non-file
/// at the cache path (tampering, never trouble) fails closed.
///
/// # Errors
///
/// Returns the fresh build's failure, an extraction failure, a guard
/// mismatch, or `INDEX_SNAPSHOT_IO` when neither a cached nor a fresh
/// record can be returned.
pub fn complete_root_snapshot(
    repository: &Path,
    revision: &VerifiedRevision,
    guard: &RepositoryMaintenanceGuard,
) -> Result<(IndexSnapshot, CacheOutcome), IndexCacheError> {
    if !guard.covers(repository) {
        return Err(IndexCacheError::Io(std::io::Error::other(
            "index cache guard covers a different repository",
        )));
    }
    let path = index_cache_path(repository, revision.state_root().root);
    let reason = match read_record(&path) {
        Some(record) => match accept_cached(revision, &record) {
            Ok(snapshot) => return Ok((snapshot, CacheOutcome::Hit)),
            Err(error) => discard_reason_of(&error),
        },
        None => CacheDiscardReason::Missing,
    };
    let fresh = fresh_snapshot(revision)?;
    if let Ok(metadata) = fs::symlink_metadata(&path)
        && !metadata.is_file()
    {
        // Never follow or overwrite a non-file at the cache path.
        return Err(IndexCacheError::Io(std::io::Error::other(
            "index cache path is not a regular file",
        )));
    }
    // Best-effort write-back: the cache is derived and disposable, so a
    // write failure still returns the fresh build instead of failing the
    // snapshot (contract section 5 fail-open).
    let _ = write_record(&path, fresh.record());
    Ok((fresh, CacheOutcome::Rebuilt(reason)))
}

/// Probes the cache for the verified revision's already-materialized
/// complete-root snapshot and returns its identity, without ever building
/// one (S20-300, owner Merlin; REQ-10 accepted-head disclosure).
///
/// Only the cheap path runs: at most one cache file is read (bounded by
/// `MAX_SNAPSHOT_RECORD_BYTES`) and bounded-decoded under the same four
/// acceptance rules as [`complete_root_snapshot`] (`accept_cached` reads no
/// object and extracts no edge; the decode still verifies the record's
/// digest and edge inversion). An absent, unreadable, non-file, or
/// discarded record yields `None`; there is no `fresh_snapshot`
/// fallthrough, no write-back, and no delete. Builds stay on the query
/// paths ([`complete_root_snapshot`] under `query.root`/`query.continue`).
///
/// The caller MUST hold shared repository maintenance over
/// `repository`, as for [`complete_root_snapshot`].
///
/// # Errors
///
/// Returns `INDEX_SNAPSHOT_IO` only for a guard that does not cover
/// `repository` (compared canonically, as `RepositoryMaintenanceGuard::covers`
/// does); every cache condition is an absence, not an error.
pub fn cached_complete_root_snapshot_id(
    repository: &Path,
    revision: &VerifiedRevision,
    guard: &RepositoryMaintenanceGuard,
) -> Result<Option<IndexSnapshotId>, IndexCacheError> {
    if !guard.covers(repository) {
        return Err(IndexCacheError::Io(std::io::Error::other(
            "index cache guard covers a different repository",
        )));
    }
    let path = index_cache_path(repository, revision.state_root().root);
    Ok(read_record(&path)
        .and_then(|record| accept_cached(revision, &record).ok())
        .map(|snapshot| snapshot.snapshot_id()))
}

/// Reads the cache file when it is a regular file, absent for anything
/// else. There is no check-then-open window: the path is opened once with
/// `O_NOFOLLOW` (a symlink at the cache path fails the open) and
/// `O_NONBLOCK` (a FIFO or device cannot make the open wait), and the
/// open handle is then required to be a regular file, so a planted
/// symlink, FIFO, or directory is absence. The byte cap bounds what one
/// read can materialize.
fn read_record(path: &Path) -> Option<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut record = Vec::new();
    file.take(MAX_SNAPSHOT_RECORD_BYTES + 1)
        .read_to_end(&mut record)
        .ok()?;
    Some(record)
}

/// Outcome of a cache audit: absent and differing are distinct, because a
/// missing cache is benign while a differing one signals tampering or a
/// derivation drift worth investigating.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheVerify {
    /// The cached record rebuilds byte for byte.
    Match,
    /// No cache file exists yet.
    Missing,
    /// A cache file exists but differs from the rebuild.
    Mismatch,
}

/// Rebuilds the snapshot from the revision and compares it byte for byte
/// with the cached record, distinguishing a missing cache from a
/// differing one for audits and Tier 2 evidence.
///
/// # Errors
///
/// Returns the fresh build's failure or an I/O failure.
pub fn verify_cached_snapshot(
    repository: &Path,
    revision: &VerifiedRevision,
    guard: &RepositoryMaintenanceGuard,
) -> Result<CacheVerify, IndexCacheError> {
    if !guard.covers(repository) {
        return Err(IndexCacheError::Io(std::io::Error::other(
            "index cache guard covers a different repository",
        )));
    }
    let fresh = fresh_snapshot(revision)?;
    let path = index_cache_path(repository, revision.state_root().root);
    match read_record(&path) {
        Some(record) => Ok(if record == fresh.record() {
            CacheVerify::Match
        } else {
            CacheVerify::Mismatch
        }),
        None => Ok(if fs::symlink_metadata(&path).is_ok() {
            // A present-but-unreadable non-file is tampering, not
            // absence; the audit reports mismatch, never a clean miss.
            CacheVerify::Mismatch
        } else {
            CacheVerify::Missing
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use sley_id::StateRoot;
    use sley_query::IndexCompleteness;

    use super::*;
    use crate::complete_root::tests::{complete_bodies, genesis, id};

    fn root(byte: u8) -> StateRoot {
        StateRoot::from_bytes([byte; 32])
    }

    /// Shared maintenance over a test repository, as production callers
    /// must hold it before touching the cache.
    fn hold(repository: &Path) -> RepositoryMaintenanceGuard {
        fs::create_dir_all(repository).unwrap();
        sley_txn::initialize_repository_maintenance(repository).unwrap();
        sley_txn::acquire_shared_repository_maintenance(repository).unwrap()
    }

    #[test]
    fn miss_then_hit_without_object_access_and_verification() {
        let (temp, transactions, genesis_id) =
            genesis("index-cache", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let (first, outcome) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        assert_eq!(outcome, CacheOutcome::Rebuilt(CacheDiscardReason::Missing));
        assert_eq!(first.completeness(), IndexCompleteness::CompleteRoot);
        assert_eq!(
            first.context().claimed_root_context,
            Some(revision.state_root().root)
        );
        assert_eq!(first.inventory().len(), 7);
        assert!(index_cache_path(&repository, revision.state_root().root).is_file());
        assert_eq!(
            verify_cached_snapshot(&repository, &revision, &guard).unwrap(),
            CacheVerify::Match
        );
        // Remove the object store: a hit must not touch it.
        fs::remove_dir_all(repository.join("objects")).unwrap();
        let (second, outcome) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        assert_eq!(outcome, CacheOutcome::Hit);
        assert_eq!(second, first);
    }

    #[test]
    fn forged_inventory_corrupt_bytes_and_wrong_arm_are_discarded_and_rewritten() {
        let (temp, transactions, genesis_id) =
            genesis("index-forged", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let (fresh, _) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        let path = index_cache_path(&repository, revision.state_root().root);

        // A digest-valid record over a different inventory: build it from a
        // request with one entity dropped, then re-sign it.
        let request = CompleteRootRequest::extract(&revision).unwrap();
        let mut entities = request.entities().clone();
        entities.policy_bindings.clear();
        let bound: Vec<_> = crate::borrow_entities(&entities)
            .iter()
            .map(|entity| (entity.entity_id(), sley_id::ObjectId::from_bytes([1; 32])))
            .collect();
        let forged_request = CompleteRootRequest::from_parts(
            entities,
            revision.state_root().root,
            request.workspace_id(),
            request.schema_epoch_id(),
            bound,
            Vec::new(),
            request.facts().dependency_roots.to_vec(),
        );
        let mut forged_entities = forged_request.entities().clone();
        forged_entities.namespaces.iter_mut().for_each(|namespace| {
            namespace.members.retain(|member| *member != id(16));
        });
        let forged_bound: Vec<_> = crate::borrow_entities(&forged_entities)
            .iter()
            .map(|entity| (entity.entity_id(), sley_id::ObjectId::from_bytes([1; 32])))
            .collect();
        let forged_request = CompleteRootRequest::from_parts(
            forged_entities,
            revision.state_root().root,
            request.workspace_id(),
            request.schema_epoch_id(),
            forged_bound,
            Vec::new(),
            request.facts().dependency_roots.to_vec(),
        );
        let forged = build_complete_root_snapshot(
            request.schema_epoch_id(),
            revision.state_root().root,
            &forged_request.borrowed(),
            forged_request.facts(),
        )
        .unwrap();
        fs::write(&path, forged.record()).unwrap();
        let (rebuilt, outcome) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        assert_eq!(
            outcome,
            CacheOutcome::Rebuilt(CacheDiscardReason::RootMismatch)
        );
        assert_eq!(rebuilt, fresh);
        assert_eq!(fs::read(&path).unwrap(), fresh.record());

        let mut corrupt = fresh.record().to_vec();
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        fs::write(&path, &corrupt).unwrap();
        let (_, outcome) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        assert_eq!(
            outcome,
            CacheOutcome::Rebuilt(CacheDiscardReason::DigestMismatch)
        );
        assert_eq!(fs::read(&path).unwrap(), fresh.record());

        let restricted = sley_query::build_index_snapshot(
            SnapshotContext {
                schema_epoch: request.schema_epoch_id(),
                claimed_root_context: Some(revision.state_root().root),
            },
            &[],
        )
        .unwrap();
        fs::write(&path, restricted.record()).unwrap();
        let (_, outcome) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        assert_eq!(
            outcome,
            CacheOutcome::Rebuilt(CacheDiscardReason::CompletenessUnsupported)
        );
        assert_eq!(
            verify_cached_snapshot(&repository, &revision, &guard).unwrap(),
            CacheVerify::Match
        );

        fs::write(&path, b"garbage").unwrap();
        let (_, outcome) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        assert_eq!(
            outcome,
            CacheOutcome::Rebuilt(CacheDiscardReason::FormatInvalid)
        );
    }

    #[test]
    fn a_second_root_caches_beside_the_first_and_paths_are_root_named() {
        let (temp, transactions, genesis_id) = genesis("index-two", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        complete_root_snapshot(&repository, &revision, &guard).unwrap();
        let other = index_cache_path(&repository, root(7));
        assert!(!other.exists());
        assert!(other.to_string_lossy().ends_with(".idx.scb1"));
        assert!(index_cache_path(&repository, revision.state_root().root).is_file());
        assert_eq!(
            fs::read_dir(repository.join("index").join("v1"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn verify_distinguishes_missing_match_and_mismatch() {
        let (temp, transactions, genesis_id) =
            genesis("index-verify", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        assert_eq!(
            verify_cached_snapshot(&repository, &revision, &guard).unwrap(),
            CacheVerify::Missing
        );
        let (fresh, _) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        assert_eq!(
            verify_cached_snapshot(&repository, &revision, &guard).unwrap(),
            CacheVerify::Match
        );
        let path = index_cache_path(&repository, revision.state_root().root);
        let mut bytes = fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&path, &bytes).unwrap();
        assert_eq!(
            verify_cached_snapshot(&repository, &revision, &guard).unwrap(),
            CacheVerify::Mismatch
        );
        // The audit still rebuilds the true record underneath.
        let (rebuilt, outcome) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        assert_eq!(
            outcome,
            CacheOutcome::Rebuilt(CacheDiscardReason::DigestMismatch)
        );
        assert_eq!(rebuilt, fresh);
    }

    #[test]
    fn guard_for_another_repository_is_refused() {
        let (temp, transactions, genesis_id) =
            genesis("index-guard", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let other = temp.child("elsewhere");
        fs::create_dir_all(&other).unwrap();
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let error = complete_root_snapshot(&other, &revision, &guard).unwrap_err();
        assert_eq!(
            error.code().as_str(),
            IndexSnapshotErrorCode::RootIo.as_str()
        );
        let error = verify_cached_snapshot(&other, &revision, &guard).unwrap_err();
        assert_eq!(
            error.code().as_str(),
            IndexSnapshotErrorCode::RootIo.as_str()
        );
        let error = cached_complete_root_snapshot_id(&other, &revision, &guard).unwrap_err();
        assert_eq!(
            error.code().as_str(),
            IndexSnapshotErrorCode::RootIo.as_str()
        );
    }

    #[test]
    fn probe_reports_only_a_materialized_snapshot_and_never_builds() {
        let (temp, transactions, genesis_id) =
            genesis("index-probe", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let path = index_cache_path(&repository, revision.state_root().root);
        // Cold: absent, and the probe writes nothing back.
        assert_eq!(
            cached_complete_root_snapshot_id(&repository, &revision, &guard).unwrap(),
            None
        );
        assert!(!path.exists(), "the probe must never build or write");
        // Materialized by the query path: the probe names that snapshot.
        let (built, _) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        assert_eq!(
            cached_complete_root_snapshot_id(&repository, &revision, &guard).unwrap(),
            Some(built.snapshot_id())
        );
        // A hit reads no object: removing the store does not change it.
        fs::remove_dir_all(repository.join("objects")).unwrap();
        assert_eq!(
            cached_complete_root_snapshot_id(&repository, &revision, &guard).unwrap(),
            Some(built.snapshot_id())
        );
        // A record the four rules discard is an absence, not a rebuild.
        let mut corrupt = built.record().to_vec();
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        fs::write(&path, &corrupt).unwrap();
        assert_eq!(
            cached_complete_root_snapshot_id(&repository, &revision, &guard).unwrap(),
            None
        );
        assert_eq!(fs::read(&path).unwrap(), corrupt, "no write-back");
    }

    #[test]
    #[cfg(unix)]
    fn symlink_at_the_cache_path_fails_closed() {
        use std::os::unix::fs::symlink;
        let (temp, transactions, genesis_id) =
            genesis("index-symlink", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        complete_root_snapshot(&repository, &revision, &guard).unwrap();
        let path = index_cache_path(&repository, revision.state_root().root);
        let target = temp.child("decoy");
        fs::write(&target, b"decoy").unwrap();
        fs::remove_file(&path).unwrap();
        symlink(&target, &path).unwrap();
        // Tampering is never fail-open: no fresh build is served and
        // nothing is overwritten through the link.
        assert!(complete_root_snapshot(&repository, &revision, &guard).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"decoy");
        assert_eq!(
            verify_cached_snapshot(&repository, &revision, &guard).unwrap(),
            CacheVerify::Mismatch
        );
    }

    #[test]
    #[cfg(unix)]
    fn a_symlink_to_a_valid_record_is_absence_for_the_probe() {
        // O_NOFOLLOW: even a link to a byte-identical valid record is not
        // followed; the probe answers absence, the query path fails closed.
        use std::os::unix::fs::symlink;
        let (temp, transactions, genesis_id) =
            genesis("index-probe-link", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let (built, _) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        let path = index_cache_path(&repository, revision.state_root().root);
        let copy = temp.child("valid-copy");
        fs::write(&copy, built.record()).unwrap();
        fs::remove_file(&path).unwrap();
        symlink(&copy, &path).unwrap();
        assert_eq!(
            cached_complete_root_snapshot_id(&repository, &revision, &guard).unwrap(),
            None
        );
        assert!(complete_root_snapshot(&repository, &revision, &guard).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn a_fifo_at_the_cache_path_answers_at_once() {
        // O_NONBLOCK: a FIFO with no writer cannot make the open wait; the
        // probe reports absence and the query path fails closed, both
        // promptly (a blocking open would hang this test).
        let (temp, transactions, genesis_id) =
            genesis("index-probe-fifo", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let path = index_cache_path(&repository, revision.state_root().root);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let made = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap();
        assert!(made.success());
        let started = std::time::Instant::now();
        assert_eq!(
            cached_complete_root_snapshot_id(&repository, &revision, &guard).unwrap(),
            None
        );
        assert!(complete_root_snapshot(&repository, &revision, &guard).is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(30));
    }

    #[test]
    #[cfg(unix)]
    fn a_non_canonical_repository_spelling_is_covered_by_its_guard() {
        // The guard holds the canonical root. A caller that names the same
        // repository through a symlinked parent directory or a `..`
        // component is covered (guard.covers, the S20-390 rule), so the
        // probe and the query path work instead of refusing; a symlink as
        // the repository's own final component stays refused by that rule.
        use std::os::unix::fs::symlink;
        let (temp, transactions, genesis_id) =
            genesis("index-probe-alias", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let parent_alias = temp.child("parent-alias");
        symlink(repository.parent().unwrap(), &parent_alias).unwrap();
        let through_link = parent_alias.join("repo");
        let dotted = repository.join("objects").join("..");
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let (built, _) = complete_root_snapshot(&through_link, &revision, &guard).unwrap();
        for spelling in [&through_link, &dotted] {
            assert_eq!(
                cached_complete_root_snapshot_id(spelling, &revision, &guard).unwrap(),
                Some(built.snapshot_id())
            );
            assert_eq!(
                verify_cached_snapshot(spelling, &revision, &guard).unwrap(),
                CacheVerify::Match
            );
        }
        let final_alias = temp.child("repo-alias");
        symlink(&repository, &final_alias).unwrap();
        assert!(cached_complete_root_snapshot_id(&final_alias, &revision, &guard).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn unwritable_cache_still_returns_a_fresh_build() {
        use std::os::unix::fs::PermissionsExt;
        let (temp, transactions, genesis_id) =
            genesis("index-readonly", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let (fresh, _) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        let directory = repository.join("index").join("v1");
        let path = index_cache_path(&repository, revision.state_root().root);
        fs::remove_file(&path).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o555)).unwrap();
        // Fail-open: the fresh build is returned even though the
        // write-back cannot land, and no error escapes.
        let (rebuilt, outcome) = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        assert_eq!(outcome, CacheOutcome::Rebuilt(CacheDiscardReason::Missing));
        assert_eq!(rebuilt, fresh);
        assert!(!path.exists());
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
