//! Repository-owned complete-root index cache (S20-300 full, contract
//! `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` section 5).
//!
//! The one place an index snapshot is reused without a rebuild. A cached
//! record is accepted only under the four cheap rules of the contract and
//! serves read-only derived query surfaces; validation, comparison, merge,
//! commit, exchange, GC, and recovery never read it. Files are derived and
//! disposable.

use core::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use sley_id::StateRoot;
use sley_query::{
    CacheDiscardReason, IndexSnapshot, IndexSnapshotBuildError, IndexSnapshotError,
    IndexSnapshotErrorCode, SnapshotContext, build_complete_root_snapshot,
    decode_complete_root_snapshot,
};
use sley_txn::VerifiedRevision;

use crate::CompleteRootRequest;
use crate::complete_root::CompleteRootError;

const INDEX_DIRECTORY: &str = "index";
const INDEX_VERSION_DIRECTORY: &str = "v1";
const INDEX_SUFFIX: &str = ".idx.scb1";
const TEMPORARY_SUFFIX: &str = ".tmp";

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

fn fresh_snapshot(revision: &VerifiedRevision) -> Result<IndexSnapshot, IndexCacheError> {
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
    match error.code() {
        IndexSnapshotErrorCode::ProfileUnsupported | IndexSnapshotErrorCode::VersionUnsupported => {
            CacheDiscardReason::VersionUnsupported
        }
        IndexSnapshotErrorCode::ContextMismatch => CacheDiscardReason::ContextMismatch,
        IndexSnapshotErrorCode::DigestMismatch => CacheDiscardReason::DigestMismatch,
        IndexSnapshotErrorCode::CompletenessUnsupported => {
            CacheDiscardReason::CompletenessUnsupported
        }
        IndexSnapshotErrorCode::ResourceLimit => CacheDiscardReason::ResourceLimit,
        IndexSnapshotErrorCode::RootMismatch => CacheDiscardReason::RootMismatch,
        IndexSnapshotErrorCode::FormatInvalid
        | IndexSnapshotErrorCode::InternalInvariant
        | IndexSnapshotErrorCode::RootIncomplete
        | IndexSnapshotErrorCode::RootIo => CacheDiscardReason::FormatInvalid,
    }
}

fn write_record(path: &Path, record: &[u8]) -> Result<(), IndexCacheError> {
    let directory = path
        .parent()
        .ok_or_else(|| std::io::Error::other("index cache path has no parent"))?;
    fs::create_dir_all(directory)?;
    let temporary = path.with_file_name(format!(
        "{}{TEMPORARY_SUFFIX}",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| std::io::Error::other("index cache file name is not text"))?
    ));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
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
/// # Errors
///
/// Returns the fresh build's failure, an extraction failure, or
/// `INDEX_SNAPSHOT_IO` when neither a cached nor a fresh record can be
/// returned.
pub fn complete_root_snapshot(
    repository: &Path,
    revision: &VerifiedRevision,
) -> Result<(IndexSnapshot, CacheOutcome), IndexCacheError> {
    let path = index_cache_path(repository, revision.state_root().root);
    let reason = match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => match accept_cached(revision, &fs::read(&path)?) {
            Ok(snapshot) => return Ok((snapshot, CacheOutcome::Hit)),
            Err(error) => discard_reason_of(&error),
        },
        Ok(_) => CacheDiscardReason::FormatInvalid,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => CacheDiscardReason::Missing,
        Err(error) => return Err(error.into()),
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
    write_record(&path, fresh.record())?;
    Ok((fresh, CacheOutcome::Rebuilt(reason)))
}

/// Rebuilds the snapshot from the revision and compares it byte for byte
/// with the cached record; `Ok(true)` when equal, `Ok(false)` when the cache
/// is absent or differs.
///
/// # Errors
///
/// Returns the fresh build's failure or an I/O failure.
pub fn verify_cached_snapshot(
    repository: &Path,
    revision: &VerifiedRevision,
) -> Result<bool, IndexCacheError> {
    let fresh = fresh_snapshot(revision)?;
    let path = index_cache_path(repository, revision.state_root().root);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => Ok(fs::read(&path)? == fresh.record()),
        Ok(_) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
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

    #[test]
    fn miss_then_hit_without_object_access_and_verification() {
        let (temp, transactions, genesis_id) =
            genesis("index-cache", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let (first, outcome) = complete_root_snapshot(&repository, &revision).unwrap();
        assert_eq!(outcome, CacheOutcome::Rebuilt(CacheDiscardReason::Missing));
        assert_eq!(first.completeness(), IndexCompleteness::CompleteRoot);
        assert_eq!(
            first.context().claimed_root_context,
            Some(revision.state_root().root)
        );
        assert_eq!(first.inventory().len(), 7);
        assert!(index_cache_path(&repository, revision.state_root().root).is_file());
        assert!(verify_cached_snapshot(&repository, &revision).unwrap());
        // Remove the object store: a hit must not touch it.
        fs::remove_dir_all(repository.join("objects")).unwrap();
        let (second, outcome) = complete_root_snapshot(&repository, &revision).unwrap();
        assert_eq!(outcome, CacheOutcome::Hit);
        assert_eq!(second, first);
    }

    #[test]
    fn forged_inventory_corrupt_bytes_and_wrong_arm_are_discarded_and_rewritten() {
        let (temp, transactions, genesis_id) =
            genesis("index-forged", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let (fresh, _) = complete_root_snapshot(&repository, &revision).unwrap();
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
        let (rebuilt, outcome) = complete_root_snapshot(&repository, &revision).unwrap();
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
        let (_, outcome) = complete_root_snapshot(&repository, &revision).unwrap();
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
        let (_, outcome) = complete_root_snapshot(&repository, &revision).unwrap();
        assert_eq!(
            outcome,
            CacheOutcome::Rebuilt(CacheDiscardReason::CompletenessUnsupported)
        );
        assert!(verify_cached_snapshot(&repository, &revision).unwrap());

        fs::write(&path, b"garbage").unwrap();
        let (_, outcome) = complete_root_snapshot(&repository, &revision).unwrap();
        assert_eq!(
            outcome,
            CacheOutcome::Rebuilt(CacheDiscardReason::FormatInvalid)
        );
    }

    #[test]
    fn a_second_root_caches_beside_the_first_and_paths_are_root_named() {
        let (temp, transactions, genesis_id) = genesis("index-two", complete_bodies(), &[root(9)]);
        let repository = temp.child("repo");
        let revision = transactions.verified_revision(genesis_id).unwrap();
        complete_root_snapshot(&repository, &revision).unwrap();
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
}
