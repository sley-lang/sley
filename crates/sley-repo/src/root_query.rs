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
    Cursor, QueryLimits, RootQuery, RootQueryError, RootQueryInput, RootQueryResponse,
    build_root_query_request, execute_root_query,
};
use sley_txn::VerifiedRevision;

use crate::complete_root::{CompleteRootError, CompleteRootRequest};
use crate::index_cache::{CacheOutcome, IndexCacheError, complete_root_snapshot};

/// Failure of the repository query surface with every wrapped code preserved.
#[derive(Debug)]
pub enum RepositoryQueryError {
    /// The S20-250 full extraction failed.
    Extraction(CompleteRootError),
    /// The S20-300 cache could not return a snapshot.
    Cache(IndexCacheError),
    /// The engine rejected the query.
    Query(RootQueryError),
}

impl RepositoryQueryError {
    /// Returns the wrapped stable code.
    #[must_use]
    pub fn code(&self) -> String {
        match self {
            Self::Extraction(error) => error.code().to_string(),
            Self::Cache(error) => error.code(),
            Self::Query(error) => error.code().as_str().to_string(),
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

impl From<RootQueryError> for RepositoryQueryError {
    fn from(value: RootQueryError) -> Self {
        Self::Query(value)
    }
}

/// One answered query and the cache outcome that supplied its edges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryQueryOutcome {
    /// The exact response.
    pub response: RootQueryResponse,
    /// Whether the snapshot came from the cache or a rebuild.
    pub cache: CacheOutcome,
}

/// Answers one root-backed query over a verified revision.
///
/// # Errors
///
/// Preserves extraction, cache, and query failure namespaces; returns no
/// partial response.
pub fn run_root_query(
    repository: &Path,
    revision: &VerifiedRevision,
    query: RootQuery,
    limits: QueryLimits,
    allow_continuation: bool,
    after: Option<Cursor>,
) -> Result<RepositoryQueryOutcome, RepositoryQueryError> {
    let extracted = CompleteRootRequest::extract(revision)?;
    let (snapshot, cache) = complete_root_snapshot(repository, revision)?;
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
    };
    let request = build_root_query_request(&input, query, limits, allow_continuation, after)?;
    let response = execute_root_query(&input, &request)?;
    Ok(RepositoryQueryOutcome { response, cache })
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

    #[test]
    fn cache_hit_and_rebuild_answer_byte_identical_records_without_object_access() {
        let (temp, transactions, genesis_id) = genesis(
            "root-query",
            complete_bodies(),
            &[StateRoot::from_bytes([9; 32])],
        );
        let repository = temp.child("repo");
        let revision = transactions.verified_revision(genesis_id).unwrap();
        let limits = QueryLimits::profile_maximum();
        let first = run_root_query(
            &repository,
            &revision,
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
        let cached = complete_root_snapshot(&repository, &revision).unwrap();
        assert_eq!(cached.1, CacheOutcome::Hit);
        assert_eq!(cached.0.direct_edges().len(), summary.direct_edges as usize);
    }
}
