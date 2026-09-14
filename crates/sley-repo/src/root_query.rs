//! Repository surface for root-backed queries (S20-310 full, contract
//! `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` section 7).
//!
//! This is the only producer of a `RootQueryInput` from persistent state:
//! the S20-250 full adapter extracts the bodies, bindings, and root facts
//! from an S20-390 verified revision, the S20-300 cache supplies the arm-2
//! snapshot (a hit supplies edges only), and the pure `sley-query` engine
//! binds and answers. It grants no root, commit, policy, mutation, session,
//! or protocol authority.

use core::fmt;
use std::path::Path;

use sley_id::{EntityId, SemanticFingerprint};
use sley_query::{
    CacheDiscardReason, ContextCapsule, ContextCapsuleError, Cursor, QueryLimits, RootQuery,
    RootQueryError, RootQueryInput, RootQueryRequest, RootQueryResponse, build_context_capsule,
    build_root_query_request, execute_root_query,
};
use sley_txn::{RepositoryMaintenanceGuard, VerifiedRevision};

use crate::complete_root::{CompleteRootError, CompleteRootRequest};
use crate::index_cache::{
    CacheOutcome, IndexCacheError, complete_root_snapshot, fresh_snapshot,
};

/// Failure of the repository query surface with every wrapped code preserved.
#[derive(Debug)]
pub enum RepositoryQueryError {
    /// The S20-250 full extraction failed.
    Extraction(CompleteRootError),
    /// The S20-300 cache could not return a snapshot.
    Cache(IndexCacheError),
    /// The engine rejected the query.
    Query(RootQueryError),
    /// The S20-320 capsule could not be built.
    Capsule(ContextCapsuleError),
}

impl RepositoryQueryError {
    /// Returns the wrapped stable code.
    #[must_use]
    pub fn code(&self) -> String {
        match self {
            Self::Extraction(error) => error.code().to_string(),
            Self::Cache(error) => error.code(),
            Self::Query(error) => error.code().as_str().to_string(),
            Self::Capsule(error) => error.code().as_str().to_string(),
        }
    }
}

impl fmt::Display for RepositoryQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.code())
    }
}

impl std::error::Error for RepositoryQueryError {}

impl From<CompleteRootError> for RepositoryQueryError {
    fn from(value: CompleteRootError) -> Self {
        Self::Extraction(value)
    }
}

impl From<IndexCacheError> for RepositoryQueryError {
    fn from(value: IndexCacheError) -> Self {
        Self::Cache(value)
    }
}

impl From<ContextCapsuleError> for RepositoryQueryError {
    fn from(value: ContextCapsuleError) -> Self {
        Self::Capsule(value)
    }
}

impl From<RootQueryError> for RepositoryQueryError {
    fn from(value: RootQueryError) -> Self {
        Self::Query(value)
    }
}

/// One answered query and the cache outcome that supplied its edges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryQueryOutcome {
    /// The canonical request the engine answered.
    pub request: RootQueryRequest,
    /// The exact response.
    pub response: RootQueryResponse,
    /// Whether the snapshot came from the cache or a rebuild.
    pub cache: CacheOutcome,
}

/// Answers one root-backed query over a verified revision.
///
/// The snapshot comes from the repository cache (a hit supplies edges
/// only) for transient reads; exported evidence must use
/// [`run_root_query_fresh`] instead, never a bare hit.
///
/// # Errors
///
/// Preserves extraction, cache, and query failure namespaces; returns no
/// partial response.
pub fn run_root_query(
    repository: &Path,
    revision: &VerifiedRevision,
    guard: &RepositoryMaintenanceGuard,
    query: RootQuery,
    limits: QueryLimits,
    allow_continuation: bool,
    after: Option<Cursor>,
) -> Result<RepositoryQueryOutcome, RepositoryQueryError> {
    let extracted = CompleteRootRequest::extract(revision)?;
    let (snapshot, cache) = complete_root_snapshot(repository, revision, guard)?;
    let entities = extracted.borrowed();
    let fingerprints = stored_fingerprints(revision);
    let record = &revision.state_root().record;
    let input = RootQueryInput {
        snapshot: &snapshot,
        entities: &entities,
        facts: extracted.facts(),
        bindings: extracted.bound_objects(),
        fingerprints: &fingerprints,
        root: extracted.root(),
        workspace_id: extracted.workspace_id(),
        schema_epoch: extracted.schema_epoch_id(),
        contract_root: record.contract_root,
        test_root: record.test_root,
        policy_root: record.policy_root,
        interpretation_flags: &record.interpretation_flags,
    };
    let request = build_root_query_request(&input, query, limits, allow_continuation, after)?;
    let response = execute_root_query(&input, &request)?;
    Ok(RepositoryQueryOutcome {
        request,
        response,
        cache,
    })
}

/// Answers one root-backed query over a verified revision from a freshly
/// built snapshot, never touching the cache.
///
/// Exported evidence must come from this path: a cache hit is accepted
/// without re-deriving edges, so only a fresh build (or an admission
/// against one) may underwrite anything that leaves the process.
///
/// # Errors
///
/// Preserves extraction, cache, and query failure namespaces; returns no
/// partial response.
pub fn run_root_query_fresh(
    revision: &VerifiedRevision,
    query: RootQuery,
    limits: QueryLimits,
    allow_continuation: bool,
    after: Option<Cursor>,
) -> Result<RepositoryQueryOutcome, RepositoryQueryError> {
    let extracted = CompleteRootRequest::extract(revision)?;
    let snapshot = fresh_snapshot(revision).map_err(RepositoryQueryError::Cache)?;
    let entities = extracted.borrowed();
    let fingerprints = stored_fingerprints(revision);
    let record = &revision.state_root().record;
    let input = RootQueryInput {
        snapshot: &snapshot,
        entities: &entities,
        facts: extracted.facts(),
        bindings: extracted.bound_objects(),
        fingerprints: &fingerprints,
        root: extracted.root(),
        workspace_id: extracted.workspace_id(),
        schema_epoch: extracted.schema_epoch_id(),
        contract_root: record.contract_root,
        test_root: record.test_root,
        policy_root: record.policy_root,
        interpretation_flags: &record.interpretation_flags,
    };
    let request = build_root_query_request(&input, query, limits, allow_continuation, after)?;
    let response = execute_root_query(&input, &request)?;
    // The fresh path never consults the cache: `Missing` marks "no cache
    // consulted", not a lookup result.
    Ok(RepositoryQueryOutcome {
        request,
        response,
        cache: CacheOutcome::Rebuilt(CacheDiscardReason::Missing),
    })
}

/// Answers one root-backed query and wraps it in the master context capsule
/// (S20-320 full, contract section 8), built from a fresh snapshot: an
/// exported capsule never rests on a cache hit.
///
/// # Errors
///
/// Preserves extraction, cache, query, and capsule failure namespaces.
pub fn run_context_capsule(
    revision: &VerifiedRevision,
    query: RootQuery,
    limits: QueryLimits,
    allow_continuation: bool,
    after: Option<Cursor>,
) -> Result<ContextCapsule, RepositoryQueryError> {
    let outcome = run_root_query_fresh(revision, query, limits, allow_continuation, after)?;
    build_context_capsule(&outcome.request, &outcome.response).map_err(RepositoryQueryError::Capsule)
}

/// Field-4 fingerprints carried by the verified objects, in binding order.
#[must_use]
pub fn stored_fingerprints(revision: &VerifiedRevision) -> Vec<(EntityId, SemanticFingerprint)> {
    revision
        .objects()
        .iter()
        .filter_map(|object| {
            object
                .record()
                .semantic_fingerprint
                .map(|fingerprint| (object.record().entity_id, fingerprint))
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use std::fs;

    use sley_id::StateRoot;
    use sley_query::{ModeledEntityKind, RootQueryResult};

    use super::*;
    use crate::complete_root::tests::{complete_bodies, genesis};

    /// Shared maintenance over a test repository, as production callers
    /// must hold it before touching the cache.
    fn hold(repository: &std::path::Path) -> RepositoryMaintenanceGuard {
        std::fs::create_dir_all(repository).unwrap();
        sley_txn::initialize_repository_maintenance(repository).unwrap();
        sley_txn::acquire_shared_repository_maintenance(repository).unwrap()
    }

    #[test]
    fn cache_hit_and_rebuild_answer_byte_identical_records_without_object_access() {
        let (temp, transactions, genesis_id) = genesis(
            "root-query",
            complete_bodies(),
            &[StateRoot::from_bytes([9; 32])],
        );
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let limits = QueryLimits::profile_maximum();
        let first = run_root_query(
            &repository,
            &revision,
            &guard,
            RootQuery::GetRootSummary,
            limits,
            false,
            None,
        )
        .unwrap();
        assert!(matches!(first.cache, CacheOutcome::Rebuilt(_)));
        let second = run_root_query(
            &repository,
            &revision,
            &guard,
            RootQuery::GetRootSummary,
            limits,
            false,
            None,
        )
        .unwrap();
        assert_eq!(second.cache, CacheOutcome::Hit);
        assert_eq!(second.response, first.response);
        let RootQueryResult::RootSummary(summary) = first.response.result() else {
            panic!("summary");
        };
        assert_eq!(summary.root, revision.state_root().root);
        assert_eq!(
            summary.kind_counts.iter().sum::<u64>(),
            revision.objects().len() as u64
        );

        let by_kind = run_root_query(
            &repository,
            &revision,
            &guard,
            RootQuery::ListEntitiesByKind {
                kind: ModeledEntityKind::TypeDef,
            },
            limits,
            false,
            None,
        )
        .unwrap();
        let RootQueryResult::Entities(functions) = by_kind.response.result() else {
            panic!("functions");
        };
        assert!(!functions.is_empty());
        let entity = run_root_query(
            &repository,
            &revision,
            &guard,
            RootQuery::GetEntity {
                entity: functions[0],
            },
            limits,
            false,
            None,
        )
        .unwrap();
        let RootQueryResult::Entity {
            kind, object_id, ..
        } = entity.response.result()
        else {
            panic!("entity");
        };
        assert_eq!(*kind, ModeledEntityKind::TypeDef);
        assert!(
            revision
                .objects()
                .iter()
                .any(|object| object.object_id() == *object_id)
        );
        // The edges of a cached hit are served without the object store.
        fs::remove_dir_all(repository.join("objects")).unwrap();
        let cached = complete_root_snapshot(&repository, &revision, &guard).unwrap();
        assert_eq!(cached.1, CacheOutcome::Hit);
        assert_eq!(cached.0.direct_edges().len(), summary.direct_edges as usize);
    }

    #[test]
    fn capsules_build_fresh_and_never_rest_on_a_cache_hit() {
        use crate::index_cache::complete_root_snapshot;
        let (temp, transactions, genesis_id) = genesis(
            "context-capsule",
            complete_bodies(),
            &[StateRoot::from_bytes([9; 32])],
        );
        let repository = temp.child("repo");
        let guard = hold(&repository);
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let limits = QueryLimits::profile_maximum();
        // With no cache at all, the capsule builds from a fresh snapshot.
        let first = run_context_capsule(
            &revision,
            RootQuery::ListEntitiesByKind {
                kind: ModeledEntityKind::TypeDef,
            },
            limits,
            false,
            None,
        )
        .unwrap();
        // With a populated cache — and even with a corrupt one — the
        // exported capsule is identical: it never reads the cache.
        complete_root_snapshot(&repository, &revision, &guard).unwrap();
        let second = run_context_capsule(
            &revision,
            RootQuery::ListEntitiesByKind {
                kind: ModeledEntityKind::TypeDef,
            },
            limits,
            false,
            None,
        )
        .unwrap();
        assert_eq!(second, first);
        assert_eq!(first.root(), revision.state_root().root);
        assert_eq!(
            first.workspace_id(),
            revision.state_root().record.workspace_id
        );
        assert!(!first.entities().is_empty());
    }
}
