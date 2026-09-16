//! Root-backed query classes over one verified root (S20-310 full, contract
//! `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md`).
//!
//! The engine is pure: it answers the nineteen classes from an arm-2
//! complete-root snapshot, the complete-root request bodies, the record's
//! bindings and facts, and the stored field-4 fingerprints, after binding
//! them together exactly. Results are computed completely and then paged by
//! canonical key under explicit continuation.

// Variant, field, and getter names are the contract's own names; the
// contract text is their documentation.
#![allow(missing_docs)]

use core::fmt;
use std::collections::{BTreeMap, VecDeque};

use sley_id::{
    EntityId, IndexSnapshotId, ObjectId, PolicyRootId, RootQueryId, SchemaEpochId,
    SemanticFingerprint, StateRoot, WorkspaceId,
};
use sley_ssmc::EntryExposure;
use sley_state_root::{StateRootRecord, recompute_root};

use crate::query::{
    LIMITS_PROFILE, MAX_QUERY_RESPONSE_BYTES, MAX_QUERY_WORK, OPTION_NONE, OPTION_SOME, QueryError,
    QueryErrorCode, QueryLimits, append_request, append_response, charge_work,
    encode_limits_request, encode_limits_response, push_request_u32, push_request_u64,
    push_response_u32, push_response_u64, reverse_closure, strictly_increasing, to_u64,
    validate_limits,
};
use crate::{
    CompleteRootFacts, ImpactEdge, ImpactEntity, ImpactKind, IndexCompleteness,
    IndexInventoryEntry, IndexSnapshot, ModeledEntityKind,
};

const ROOT_QUERY_MAGIC: &[u8; 8] = b"SLEYRQQ1";
const ROOT_RESPONSE_MAGIC: &[u8; 8] = b"SLEYRQR1";
const FORMAT_VERSION: u32 = 1;
const PROFILE_VERSION: u32 = 1;
const COMPLETENESS_COMPLETE_ROOT: u32 = 2;
const FLAG_FALSE: u32 = 1;
const FLAG_TRUE: u32 = 2;
const CURSOR_ENTITY: u32 = 1;
const CURSOR_EDGE: u32 = 2;
const CURSOR_ROOT: u32 = 3;
const ENTITY_KINDS: usize = 18;
const MAX_FILTER_KINDS: usize = 12;
const MAX_SEEDS: usize = 65_535;

/// Number of root-backed query classes.
pub const ROOT_QUERY_CLASSES: u32 = 19;

/// Stable failure codes of the root-backed profile (31000 through 31010).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootQueryErrorCode {
    ProfileUnsupported,
    RequestNotCanonical,
    Unsupported,
    SnapshotMismatch,
    UnresolvedEntity,
    ResourceLimit,
    RequiredFactOmitted,
    InternalInvariant,
    RootMismatch,
    ContinuationInvalid,
    ClassNotApplicable,
}

impl RootQueryErrorCode {
    /// Every code in numeric order.
    pub const ALL: [Self; 11] = [
        Self::ProfileUnsupported,
        Self::RequestNotCanonical,
        Self::Unsupported,
        Self::SnapshotMismatch,
        Self::UnresolvedEntity,
        Self::ResourceLimit,
        Self::RequiredFactOmitted,
        Self::InternalInvariant,
        Self::RootMismatch,
        Self::ContinuationInvalid,
        Self::ClassNotApplicable,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProfileUnsupported => "QUERY_PROFILE_UNSUPPORTED",
            Self::RequestNotCanonical => "QUERY_REQUEST_NOT_CANONICAL",
            Self::Unsupported => "QUERY_UNSUPPORTED",
            Self::SnapshotMismatch => "QUERY_SNAPSHOT_MISMATCH",
            Self::UnresolvedEntity => "QUERY_UNRESOLVED_ENTITY",
            Self::ResourceLimit => "QUERY_RESOURCE_LIMIT",
            Self::RequiredFactOmitted => "QUERY_REQUIRED_FACT_OMITTED",
            Self::InternalInvariant => "QUERY_INTERNAL_INVARIANT",
            Self::RootMismatch => "QUERY_ROOT_MISMATCH",
            Self::ContinuationInvalid => "QUERY_CONTINUATION_INVALID",
            Self::ClassNotApplicable => "QUERY_CLASS_NOT_APPLICABLE",
        }
    }

    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::ProfileUnsupported => 31_000,
            Self::RequestNotCanonical => 31_001,
            Self::Unsupported => 31_002,
            Self::SnapshotMismatch => 31_003,
            Self::UnresolvedEntity => 31_004,
            Self::ResourceLimit => 31_005,
            Self::RequiredFactOmitted => 31_006,
            Self::InternalInvariant => 31_007,
            Self::RootMismatch => 31_008,
            Self::ContinuationInvalid => 31_009,
            Self::ClassNotApplicable => 31_010,
        }
    }

    const fn from_restricted(code: QueryErrorCode) -> Self {
        match code {
            QueryErrorCode::ProfileUnsupported => Self::ProfileUnsupported,
            QueryErrorCode::RequestNotCanonical => Self::RequestNotCanonical,
            QueryErrorCode::Unsupported => Self::Unsupported,
            QueryErrorCode::SnapshotMismatch => Self::SnapshotMismatch,
            QueryErrorCode::UnresolvedEntity => Self::UnresolvedEntity,
            QueryErrorCode::ResourceLimit => Self::ResourceLimit,
            QueryErrorCode::RequiredFactOmitted => Self::RequiredFactOmitted,
            QueryErrorCode::InternalInvariant => Self::InternalInvariant,
        }
    }
}

/// Root-backed query failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootQueryError(RootQueryErrorCode);

impl RootQueryError {
    #[must_use]
    pub const fn new(code: RootQueryErrorCode) -> Self {
        Self(code)
    }

    #[must_use]
    pub const fn code(&self) -> RootQueryErrorCode {
        self.0
    }
}

impl fmt::Display for RootQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for RootQueryError {}

impl From<QueryError> for RootQueryError {
    fn from(value: QueryError) -> Self {
        Self(RootQueryErrorCode::from_restricted(value.code()))
    }
}

fn fail<T>(code: RootQueryErrorCode) -> Result<T, RootQueryError> {
    Err(RootQueryError(code))
}

/// Typed continuation cursor: the class's canonical key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cursor {
    Entity(EntityId),
    Edge(ImpactEdge),
    Root(StateRoot),
}

impl Cursor {
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Entity(_) => CURSOR_ENTITY,
            Self::Edge(_) => CURSOR_EDGE,
            Self::Root(_) => CURSOR_ROOT,
        }
    }

    /// Encoded `option(cursor)` bytes: option tag, cursor tag, payload.
    const fn encoded_bytes(self) -> u64 {
        match self {
            Self::Entity(_) | Self::Root(_) => 4 + 4 + 32,
            Self::Edge(_) => 4 + 4 + 68,
        }
    }
}

/// Everything a root-backed query may inspect, bound to one root.
#[derive(Clone, Copy)]
pub struct RootQueryInput<'a> {
    pub snapshot: &'a IndexSnapshot,
    pub entities: &'a [ImpactEntity<'a>],
    pub facts: CompleteRootFacts<'a>,
    pub bindings: &'a [(EntityId, ObjectId)],
    pub fingerprints: &'a [(EntityId, SemanticFingerprint)],
    pub root: StateRoot,
    pub workspace_id: WorkspaceId,
    pub schema_epoch: SchemaEpochId,
    pub contract_root: ObjectId,
    pub test_root: ObjectId,
    pub policy_root: PolicyRootId,
    pub interpretation_flags: &'a [u32],
}

impl RootQueryInput<'_> {
    /// Checks the section 1 binding rules.
    ///
    /// Section 1 carve-out: a snapshot whose completeness is not
    /// `CompleteRoot(2)` is not a binding failure. Both entry points
    /// (`build_root_query_request` and `execute_root_query`) route arm
    /// disagreement through the arm gate as `QUERY_PROFILE_UNSUPPORTED`
    /// (precedence item 2) before this check runs, so the completeness
    /// comparison below is a defensive residue that the gated entry points
    /// never reach with an arm-1 snapshot; it is not the
    /// `QUERY_ROOT_MISMATCH` arm rule.
    ///
    /// # Errors
    ///
    /// Returns `QUERY_ROOT_MISMATCH` when the snapshot context, bodies,
    /// bindings, facts, or fingerprints disagree, and when the nine
    /// `STATE_ROOT_V1` fields the input carries do not recompute to the
    /// claimed root. The recompute binds every caller-supplied
    /// answer-bearing fact (the bound `ObjectId` values, the entry points,
    /// the dependency roots, the three roots, and the interpretation flags)
    /// instead of checking shape agreement alone.
    pub fn verify(&self) -> Result<(), RootQueryError> {
        if self.snapshot.completeness() != IndexCompleteness::CompleteRoot
            || self.snapshot.context().schema_epoch != self.schema_epoch
            || self.snapshot.context().claimed_root_context != Some(self.root)
        {
            return fail(RootQueryErrorCode::RootMismatch);
        }
        let inventory = self.snapshot.inventory();
        if inventory.len() != self.bindings.len()
            || inventory.len() != self.facts.bound_entities.len()
            || inventory.len() != self.entities.len()
        {
            return fail(RootQueryErrorCode::RootMismatch);
        }
        let mut previous: Option<EntityId> = None;
        for (index, entry) in inventory.iter().enumerate() {
            let entity = &self.entities[index];
            if self.bindings[index].0 != entry.entity
                || self.facts.bound_entities[index] != entry.entity
                || entity.entity_id() != entry.entity
                || entity.kind() != entry.kind
                || previous.is_some_and(|last| last >= entry.entity)
            {
                return fail(RootQueryErrorCode::RootMismatch);
            }
            previous = Some(entry.entity);
        }
        let mut last_fingerprint: Option<EntityId> = None;
        for (entity, _) in self.fingerprints {
            if last_fingerprint.is_some_and(|last| last >= *entity) {
                return fail(RootQueryErrorCode::RootMismatch);
            }
            last_fingerprint = Some(*entity);
            match self.kind_of(*entity) {
                Some(ModeledEntityKind::TypeDef | ModeledEntityKind::Function) => {}
                _ => return fail(RootQueryErrorCode::RootMismatch),
            }
        }
        // Binding rule: the root-committed fact sets arrive in canonical
        // (strictly increasing) order. The recompute below binds order only
        // implicitly, so an unsorted-but-self-consistent pair would otherwise
        // pass while classes 10/11 emit verbatim out-of-order pages that the
        // cursor predicate can silently truncate (a lost fact with
        // truncated=false).
        if !strictly_increasing(self.facts.entry_points)
            || !strictly_increasing(self.facts.dependency_roots)
        {
            return fail(RootQueryErrorCode::RootMismatch);
        }
        // The digest commits every answer-bearing fact, not just the
        // shapes above: the fields are encoded exactly as given, so any
        // reordered, substituted, or extended fact recomputes to another
        // root. Any encoding failure is a mismatch, never a bypass.
        let record = StateRootRecord {
            workspace_id: self.workspace_id,
            schema_epoch_id: self.schema_epoch,
            entity_bindings: self.bindings.to_vec(),
            entry_points: self.facts.entry_points.to_vec(),
            dependency_roots: self.facts.dependency_roots.to_vec(),
            contract_root: self.contract_root,
            test_root: self.test_root,
            policy_root: self.policy_root,
            interpretation_flags: self.interpretation_flags.to_vec(),
        };
        match recompute_root(&record) {
            Ok(root) if root == self.root => Ok(()),
            _ => fail(RootQueryErrorCode::RootMismatch),
        }
    }

    fn index_of(&self, entity: EntityId) -> Option<usize> {
        self.snapshot
            .inventory()
            .binary_search_by_key(&entity, |entry| entry.entity)
            .ok()
    }

    fn kind_of(&self, entity: EntityId) -> Option<ModeledEntityKind> {
        self.index_of(entity)
            .map(|index| self.snapshot.inventory()[index].kind)
    }

    fn body(&self, entity: EntityId) -> Option<&ImpactEntity<'_>> {
        self.index_of(entity).map(|index| &self.entities[index])
    }

    fn fingerprint(&self, entity: EntityId) -> Option<SemanticFingerprint> {
        self.fingerprints
            .binary_search_by_key(&entity, |(id, _)| *id)
            .ok()
            .map(|index| self.fingerprints[index].1)
    }
}

/// The nineteen root-backed query classes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RootQuery {
    GetRootSummary,
    GetEntity {
        entity: EntityId,
    },
    GetSemanticFingerprint {
        entity: EntityId,
    },
    ListEntitiesByKind {
        kind: ModeledEntityKind,
    },
    ListWorkspacePackages,
    ListPackageExports {
        package: EntityId,
    },
    ListPackageDependencies {
        package: EntityId,
    },
    ListNamespaceMembers {
        namespace: EntityId,
    },
    ListOwningNamespaces {
        entity: EntityId,
    },
    ListEntryPoints,
    ListDependencyRoots,
    ListDirectDependencies {
        entity: EntityId,
        kinds: Vec<ImpactKind>,
    },
    ListDirectDependents {
        entity: EntityId,
        kinds: Vec<ImpactKind>,
    },
    ReverseImpactClosure {
        seeds: Vec<EntityId>,
    },
    ForwardDependencyClosure {
        seeds: Vec<EntityId>,
    },
    ListContractsFor {
        target: EntityId,
    },
    ListTestsFor {
        target: EntityId,
    },
    ListDeclaredEffects {
        entity: EntityId,
    },
    ListCapabilityRequirementsFor {
        subject: EntityId,
    },
}

impl RootQuery {
    /// Returns the exact class tag.
    #[must_use]
    pub const fn tag(&self) -> u32 {
        match self {
            Self::GetRootSummary => 1,
            Self::GetEntity { .. } => 2,
            Self::GetSemanticFingerprint { .. } => 3,
            Self::ListEntitiesByKind { .. } => 4,
            Self::ListWorkspacePackages => 5,
            Self::ListPackageExports { .. } => 6,
            Self::ListPackageDependencies { .. } => 7,
            Self::ListNamespaceMembers { .. } => 8,
            Self::ListOwningNamespaces { .. } => 9,
            Self::ListEntryPoints => 10,
            Self::ListDependencyRoots => 11,
            Self::ListDirectDependencies { .. } => 12,
            Self::ListDirectDependents { .. } => 13,
            Self::ReverseImpactClosure { .. } => 14,
            Self::ForwardDependencyClosure { .. } => 15,
            Self::ListContractsFor { .. } => 16,
            Self::ListTestsFor { .. } => 17,
            Self::ListDeclaredEffects { .. } => 18,
            Self::ListCapabilityRequirementsFor { .. } => 19,
        }
    }

    /// Returns the cursor tag of a paged class, or `None` for single-key classes.
    #[must_use]
    pub const fn key_tag(&self) -> Option<u32> {
        match self {
            Self::GetRootSummary
            | Self::GetEntity { .. }
            | Self::GetSemanticFingerprint { .. }
            | Self::ListOwningNamespaces { .. } => None,
            Self::ListDependencyRoots => Some(CURSOR_ROOT),
            Self::ListDirectDependencies { .. } | Self::ListDirectDependents { .. } => {
                Some(CURSOR_EDGE)
            }
            _ => Some(CURSOR_ENTITY),
        }
    }

    pub(crate) fn named_entities(&self) -> Vec<EntityId> {
        match self {
            Self::GetRootSummary
            | Self::ListEntitiesByKind { .. }
            | Self::ListWorkspacePackages
            | Self::ListEntryPoints
            | Self::ListDependencyRoots => Vec::new(),
            Self::GetEntity { entity }
            | Self::GetSemanticFingerprint { entity }
            | Self::ListOwningNamespaces { entity }
            | Self::ListDeclaredEffects { entity }
            | Self::ListDirectDependencies { entity, .. }
            | Self::ListDirectDependents { entity, .. } => vec![*entity],
            Self::ListPackageExports { package } | Self::ListPackageDependencies { package } => {
                vec![*package]
            }
            Self::ListNamespaceMembers { namespace } => vec![*namespace],
            Self::ReverseImpactClosure { seeds } | Self::ForwardDependencyClosure { seeds } => {
                seeds.clone()
            }
            Self::ListContractsFor { target } | Self::ListTestsFor { target } => vec![*target],
            Self::ListCapabilityRequirementsFor { subject } => vec![*subject],
        }
    }

    /// The question's subject for single-entity attribution: the first
    /// named entity, if the question names any. The capsule dictionary
    /// attributes single-entity payloads to this subject (S20-320 full
    /// revision 4); positional `named_entities().first()` call sites must
    /// use this accessor so the convention has one declared home.
    pub(crate) fn subject_entity(&self) -> Option<EntityId> {
        self.named_entities().into_iter().next()
    }

    fn validate_shape(&self) -> Result<(), RootQueryError> {
        match self {
            Self::ListDirectDependencies { kinds, .. }
            | Self::ListDirectDependents { kinds, .. } => {
                if kinds.is_empty() || kinds.len() > MAX_FILTER_KINDS || !strictly_increasing(kinds)
                {
                    return fail(RootQueryErrorCode::RequestNotCanonical);
                }
            }
            Self::ReverseImpactClosure { seeds } | Self::ForwardDependencyClosure { seeds } => {
                if seeds.is_empty() || seeds.len() > MAX_SEEDS || !strictly_increasing(seeds) {
                    return fail(RootQueryErrorCode::RequestNotCanonical);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Canonical root-backed request bound to one snapshot and root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootQueryRequest {
    query_id: RootQueryId,
    snapshot_id: IndexSnapshotId,
    schema_epoch: SchemaEpochId,
    root: StateRoot,
    workspace_id: WorkspaceId,
    limits: QueryLimits,
    allow_continuation: bool,
    after: Option<Cursor>,
    query: RootQuery,
    preimage: Vec<u8>,
}

impl RootQueryRequest {
    #[must_use]
    pub const fn query_id(&self) -> RootQueryId {
        self.query_id
    }

    #[must_use]
    pub const fn snapshot_id(&self) -> IndexSnapshotId {
        self.snapshot_id
    }

    #[must_use]
    pub const fn root(&self) -> StateRoot {
        self.root
    }

    #[must_use]
    pub const fn limits(&self) -> QueryLimits {
        self.limits
    }

    #[must_use]
    pub const fn allow_continuation(&self) -> bool {
        self.allow_continuation
    }

    #[must_use]
    pub const fn after(&self) -> Option<Cursor> {
        self.after
    }

    #[must_use]
    pub const fn query(&self) -> &RootQuery {
        &self.query
    }

    #[must_use]
    pub fn preimage(&self) -> &[u8] {
        &self.preimage
    }
}

/// Root summary result (class 1).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootSummary {
    pub workspace_id: WorkspaceId,
    pub schema_epoch: SchemaEpochId,
    pub root: StateRoot,
    pub contract_root: ObjectId,
    pub test_root: ObjectId,
    pub policy_root: PolicyRootId,
    pub kind_counts: [u64; ENTITY_KINDS],
    pub entry_points: u64,
    pub dependency_roots: u64,
    pub direct_edges: u64,
}

/// Dependency binding row (class 7).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DependencyRow {
    pub binding: EntityId,
    pub dependency_root: StateRoot,
    pub external_package: EntityId,
    pub local_namespace: EntityId,
}

/// Entry point row (class 10).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntryRow {
    pub entry_point: EntityId,
    pub function: EntityId,
    pub exposure: u32,
}

/// Typed result payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RootQueryResult {
    RootSummary(Box<RootSummary>),
    Entity {
        kind: ModeledEntityKind,
        object_id: ObjectId,
        fingerprint: Option<SemanticFingerprint>,
    },
    Fingerprint(Option<SemanticFingerprint>),
    Entities(Vec<EntityId>),
    DependencyRows(Vec<DependencyRow>),
    InventoryEntries(Vec<IndexInventoryEntry>),
    EntryRows(Vec<EntryRow>),
    Roots(Vec<StateRoot>),
    Edges(Vec<ImpactEdge>),
}

/// Exact response with its canonical `SLEYRQR1` record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootQueryResponse {
    query_id: RootQueryId,
    snapshot_id: IndexSnapshotId,
    schema_epoch: SchemaEpochId,
    root: StateRoot,
    workspace_id: WorkspaceId,
    applied_limits: QueryLimits,
    allow_continuation: bool,
    after: Option<Cursor>,
    class_tag: u32,
    total_count: u64,
    returned: u64,
    truncated: bool,
    next_after: Option<Cursor>,
    reached_depth: u32,
    charged_work: u64,
    response_bytes: u64,
    result: RootQueryResult,
    record: Vec<u8>,
}

impl RootQueryResponse {
    #[must_use]
    pub const fn query_id(&self) -> RootQueryId {
        self.query_id
    }

    #[must_use]
    pub const fn snapshot_id(&self) -> IndexSnapshotId {
        self.snapshot_id
    }

    #[must_use]
    pub const fn root(&self) -> StateRoot {
        self.root
    }

    #[must_use]
    pub const fn schema_epoch(&self) -> SchemaEpochId {
        self.schema_epoch
    }

    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    #[must_use]
    pub const fn class_tag(&self) -> u32 {
        self.class_tag
    }

    #[must_use]
    pub const fn total_count(&self) -> u64 {
        self.total_count
    }

    #[must_use]
    pub const fn returned(&self) -> u64 {
        self.returned
    }

    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    #[must_use]
    pub const fn next_after(&self) -> Option<Cursor> {
        self.next_after
    }

    #[must_use]
    pub const fn reached_depth(&self) -> u32 {
        self.reached_depth
    }

    #[must_use]
    pub const fn charged_work(&self) -> u64 {
        self.charged_work
    }

    #[must_use]
    pub const fn response_bytes(&self) -> u64 {
        self.response_bytes
    }

    #[must_use]
    pub const fn result(&self) -> &RootQueryResult {
        &self.result
    }

    #[must_use]
    pub fn record(&self) -> &[u8] {
        &self.record
    }
}

/// Builds a canonical root-backed request.
///
/// # Errors
///
/// Fails in contract precedence: limits, arm, shape, cursor type, then the
/// input binding.
pub fn build_root_query_request(
    input: &RootQueryInput<'_>,
    query: RootQuery,
    limits: QueryLimits,
    allow_continuation: bool,
    after: Option<Cursor>,
) -> Result<RootQueryRequest, RootQueryError> {
    validate_limits(limits)?;
    if input.snapshot.completeness() != IndexCompleteness::CompleteRoot {
        return fail(RootQueryErrorCode::ProfileUnsupported);
    }
    query.validate_shape()?;
    validate_cursor(&query, after)?;
    input.verify()?;
    let preimage = encode_preimage(
        input.snapshot.snapshot_id(),
        input.schema_epoch,
        input.root,
        input.workspace_id,
        limits,
        allow_continuation,
        after,
        &query,
    )?;
    let query_id = RootQueryId::derive(&preimage);
    Ok(RootQueryRequest {
        query_id,
        snapshot_id: input.snapshot.snapshot_id(),
        schema_epoch: input.schema_epoch,
        root: input.root,
        workspace_id: input.workspace_id,
        limits,
        allow_continuation,
        after,
        query,
        preimage,
    })
}

fn validate_cursor(query: &RootQuery, after: Option<Cursor>) -> Result<(), RootQueryError> {
    match (query.key_tag(), after) {
        (_, None) => Ok(()),
        (Some(expected), Some(cursor)) if cursor.tag() == expected => Ok(()),
        _ => fail(RootQueryErrorCode::ContinuationInvalid),
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_preimage(
    snapshot_id: IndexSnapshotId,
    schema_epoch: SchemaEpochId,
    root: StateRoot,
    workspace_id: WorkspaceId,
    limits: QueryLimits,
    allow_continuation: bool,
    after: Option<Cursor>,
    query: &RootQuery,
) -> Result<Vec<u8>, RootQueryError> {
    let mut out = Vec::new();
    append_request(&mut out, ROOT_QUERY_MAGIC)?;
    push_request_u32(&mut out, FORMAT_VERSION)?;
    push_request_u32(&mut out, PROFILE_VERSION)?;
    append_request(&mut out, snapshot_id.as_bytes())?;
    append_request(&mut out, schema_epoch.as_bytes())?;
    append_request(&mut out, root.as_bytes())?;
    append_request(&mut out, workspace_id.as_bytes())?;
    push_request_u32(&mut out, COMPLETENESS_COMPLETE_ROOT)?;
    push_request_u32(&mut out, LIMITS_PROFILE)?;
    encode_limits_request(&mut out, limits)?;
    push_request_u32(&mut out, flag(allow_continuation))?;
    encode_cursor_request(&mut out, after)?;
    push_request_u32(&mut out, query.tag())?;
    encode_class_body(&mut out, query)?;
    Ok(out)
}

/// Encodes the question section shared by the request preimage and the
/// S20-320 context capsule: class tag, class body, limits, continuation
/// flag, and cursor.
pub(crate) fn encode_question(
    out: &mut Vec<u8>,
    request: &RootQueryRequest,
) -> Result<(), QueryError> {
    encode_question_parts(
        out,
        request.limits,
        request.allow_continuation,
        request.after,
        &request.query,
    )
}

fn encode_question_parts(
    out: &mut Vec<u8>,
    limits: QueryLimits,
    allow_continuation: bool,
    after: Option<Cursor>,
    query: &RootQuery,
) -> Result<(), QueryError> {
    push_request_u32(out, query.tag())?;
    encode_class_body(out, query)?;
    encode_limits_request(out, limits)?;
    push_request_u32(out, flag(allow_continuation))?;
    encode_cursor_request(out, after)
}

fn encode_class_body(out: &mut Vec<u8>, query: &RootQuery) -> Result<(), QueryError> {
    match query {
        RootQuery::GetRootSummary
        | RootQuery::ListWorkspacePackages
        | RootQuery::ListEntryPoints
        | RootQuery::ListDependencyRoots => {}
        RootQuery::GetEntity { entity }
        | RootQuery::GetSemanticFingerprint { entity }
        | RootQuery::ListOwningNamespaces { entity }
        | RootQuery::ListDeclaredEffects { entity } => {
            append_request(out, entity.as_bytes())?;
        }
        RootQuery::ListPackageExports { package }
        | RootQuery::ListPackageDependencies { package } => {
            append_request(out, package.as_bytes())?;
        }
        RootQuery::ListNamespaceMembers { namespace } => {
            append_request(out, namespace.as_bytes())?;
        }
        RootQuery::ListContractsFor { target } | RootQuery::ListTestsFor { target } => {
            append_request(out, target.as_bytes())?;
        }
        RootQuery::ListCapabilityRequirementsFor { subject } => {
            append_request(out, subject.as_bytes())?;
        }
        RootQuery::ListEntitiesByKind { kind } => {
            push_request_u32(out, kind.tag())?;
        }
        RootQuery::ListDirectDependencies { entity, kinds }
        | RootQuery::ListDirectDependents { entity, kinds } => {
            append_request(out, entity.as_bytes())?;
            push_request_u64(out, to_u64(kinds.len())?)?;
            for kind in kinds {
                push_request_u32(out, kind.tag())?;
            }
        }
        RootQuery::ReverseImpactClosure { seeds }
        | RootQuery::ForwardDependencyClosure { seeds } => {
            push_request_u64(out, to_u64(seeds.len())?)?;
            for seed in seeds {
                append_request(out, seed.as_bytes())?;
            }
        }
    }
    Ok(())
}

const fn flag(value: bool) -> u32 {
    if value { FLAG_TRUE } else { FLAG_FALSE }
}

fn encode_cursor_request(out: &mut Vec<u8>, cursor: Option<Cursor>) -> Result<(), QueryError> {
    match cursor {
        None => push_request_u32(out, OPTION_NONE),
        Some(cursor) => {
            push_request_u32(out, OPTION_SOME)?;
            push_request_u32(out, cursor.tag())?;
            match cursor {
                Cursor::Entity(entity) => append_request(out, entity.as_bytes()),
                Cursor::Root(root) => append_request(out, root.as_bytes()),
                Cursor::Edge(edge) => {
                    append_request(out, edge.dependent.as_bytes())?;
                    append_request(out, edge.dependency.as_bytes())?;
                    push_request_u32(out, edge.kind.tag())
                }
            }
        }
    }
}

fn encode_cursor_response(
    out: &mut Vec<u8>,
    cursor: Option<Cursor>,
    limit: usize,
) -> Result<(), QueryError> {
    match cursor {
        None => push_response_u32(out, OPTION_NONE, limit),
        Some(cursor) => {
            push_response_u32(out, OPTION_SOME, limit)?;
            push_response_u32(out, cursor.tag(), limit)?;
            match cursor {
                Cursor::Entity(entity) => append_response(out, entity.as_bytes(), limit),
                Cursor::Root(root) => append_response(out, root.as_bytes(), limit),
                Cursor::Edge(edge) => {
                    append_response(out, edge.dependent.as_bytes(), limit)?;
                    append_response(out, edge.dependency.as_bytes(), limit)?;
                    push_response_u32(out, edge.kind.tag(), limit)
                }
            }
        }
    }
}

fn cursor_bytes(cursor: Option<Cursor>) -> u64 {
    cursor.map_or(4, Cursor::encoded_bytes)
}

/// Executes a canonical request against its exact bound input.
///
/// # Errors
///
/// Fails without a partial response in contract precedence.
pub fn execute_root_query(
    input: &RootQueryInput<'_>,
    request: &RootQueryRequest,
) -> Result<RootQueryResponse, RootQueryError> {
    validate_limits(request.limits)?;
    if input.snapshot.completeness() != IndexCompleteness::CompleteRoot {
        return fail(RootQueryErrorCode::ProfileUnsupported);
    }
    request.query.validate_shape()?;
    validate_cursor(&request.query, request.after)?;
    let expected = encode_preimage(
        request.snapshot_id,
        request.schema_epoch,
        request.root,
        request.workspace_id,
        request.limits,
        request.allow_continuation,
        request.after,
        &request.query,
    )?;
    if expected != request.preimage || RootQueryId::derive(&expected) != request.query_id {
        return fail(RootQueryErrorCode::RequestNotCanonical);
    }
    input.verify()?;
    if request.snapshot_id != input.snapshot.snapshot_id()
        || request.schema_epoch != input.schema_epoch
        || request.root != input.root
        || request.workspace_id != input.workspace_id
    {
        return fail(RootQueryErrorCode::SnapshotMismatch);
    }
    for entity in request.query.named_entities() {
        if input.index_of(entity).is_none() {
            return fail(RootQueryErrorCode::UnresolvedEntity);
        }
    }
    // The class-kind applicability table (contract section 2) runs here,
    // after input binding and presence checks and before any class runs,
    // so an absent entity is always `UnresolvedEntity` (precedence 7) and
    // a present entity of an unadmitted kind is always
    // `ClassNotApplicable` (precedence 8). Presence was just established,
    // so a missing kind here is an impossible trusted-construction state.
    for entity in request.query.named_entities() {
        let kind = input
            .kind_of(entity)
            .ok_or_else(|| RootQueryError::new(RootQueryErrorCode::InternalInvariant))?;
        if !class_applies_to(request.query.tag(), kind) {
            return fail(RootQueryErrorCode::ClassNotApplicable);
        }
    }

    let mut work = 0_u64;
    let max_work = request.limits.max_work;
    let (complete, reached_depth) = compute(input, &request.query, &mut work, max_work)?;
    if reached_depth > request.limits.max_depth {
        return fail(RootQueryErrorCode::RequiredFactOmitted);
    }
    let paged = page(complete, request)?;
    build_response(request, paged, reached_depth, work)
}

struct Complete {
    result: RootQueryResult,
}

struct Paged {
    result: RootQueryResult,
    total_count: u64,
    returned: u64,
    truncated: bool,
    next_after: Option<Cursor>,
    payload_bytes: u64,
}

#[allow(clippy::too_many_lines)]
fn compute(
    input: &RootQueryInput<'_>,
    query: &RootQuery,
    work: &mut u64,
    max_work: u64,
) -> Result<(Complete, u32), RootQueryError> {
    let mut depth = 0_u32;
    let result = match query {
        RootQuery::GetRootSummary => {
            let mut kind_counts = [0_u64; ENTITY_KINDS];
            for entry in input.snapshot.inventory() {
                charge_work(work, 1, max_work)?;
                let slot = usize::try_from(entry.kind.tag())
                    .ok()
                    .and_then(|tag| tag.checked_sub(1))
                    .filter(|slot| *slot < ENTITY_KINDS)
                    .ok_or_else(|| RootQueryError::new(RootQueryErrorCode::InternalInvariant))?;
                kind_counts[slot] = kind_counts[slot].saturating_add(1);
            }
            RootQueryResult::RootSummary(Box::new(RootSummary {
                workspace_id: input.workspace_id,
                schema_epoch: input.schema_epoch,
                root: input.root,
                contract_root: input.contract_root,
                test_root: input.test_root,
                policy_root: input.policy_root,
                kind_counts,
                entry_points: to_u64(input.facts.entry_points.len())?,
                dependency_roots: to_u64(input.facts.dependency_roots.len())?,
                direct_edges: to_u64(input.snapshot.direct_edges().len())?,
            }))
        }
        RootQuery::GetEntity { entity } => {
            charge_work(work, 3, max_work)?;
            let index = input
                .index_of(*entity)
                .ok_or_else(|| RootQueryError::new(RootQueryErrorCode::InternalInvariant))?;
            RootQueryResult::Entity {
                kind: input.snapshot.inventory()[index].kind,
                object_id: input.bindings[index].1,
                fingerprint: input.fingerprint(*entity),
            }
        }
        RootQuery::GetSemanticFingerprint { entity } => {
            charge_work(work, 2, max_work)?;
            RootQueryResult::Fingerprint(input.fingerprint(*entity))
        }
        RootQuery::ListEntitiesByKind { kind } => {
            let mut entities = Vec::new();
            for entry in input.snapshot.inventory() {
                charge_work(work, 1, max_work)?;
                if entry.kind == *kind {
                    entities.push(entry.entity);
                }
            }
            RootQueryResult::Entities(entities)
        }
        RootQuery::ListWorkspacePackages => {
            let mut packages = Vec::new();
            for entity in input.entities {
                charge_work(work, 1, max_work)?;
                if let ImpactEntity::Workspace(workspace) = entity {
                    packages.extend(workspace.packages.iter().copied());
                }
            }
            RootQueryResult::Entities(canonical(packages))
        }
        RootQuery::ListPackageExports { package } => {
            charge_work(work, 1, max_work)?;
            match input.body(*package) {
                Some(ImpactEntity::Package(body)) => {
                    charge_work(work, to_u64(body.exports.len())?, max_work)?;
                    RootQueryResult::Entities(canonical(body.exports.clone()))
                }
                _ => return fail(RootQueryErrorCode::ClassNotApplicable),
            }
        }
        RootQuery::ListPackageDependencies { package } => {
            charge_work(work, 1, max_work)?;
            let Some(ImpactEntity::Package(body)) = input.body(*package) else {
                return fail(RootQueryErrorCode::ClassNotApplicable);
            };
            let mut rows = Vec::new();
            for binding in canonical(body.dependencies.clone()) {
                charge_work(work, 1, max_work)?;
                let Some(ImpactEntity::DependencyBinding(dependency)) = input.body(binding) else {
                    return fail(RootQueryErrorCode::InternalInvariant);
                };
                rows.push(DependencyRow {
                    binding,
                    dependency_root: dependency.dependency_root,
                    external_package: dependency.external_package,
                    local_namespace: dependency.local_namespace,
                });
            }
            RootQueryResult::DependencyRows(rows)
        }
        RootQuery::ListNamespaceMembers { namespace } => {
            charge_work(work, 1, max_work)?;
            let Some(ImpactEntity::Namespace(body)) = input.body(*namespace) else {
                return fail(RootQueryErrorCode::ClassNotApplicable);
            };
            let mut entries = Vec::new();
            for member in canonical(body.members.clone()) {
                charge_work(work, 1, max_work)?;
                let kind = input
                    .kind_of(member)
                    .ok_or_else(|| RootQueryError::new(RootQueryErrorCode::InternalInvariant))?;
                entries.push(IndexInventoryEntry {
                    entity: member,
                    kind,
                });
            }
            RootQueryResult::InventoryEntries(entries)
        }
        RootQuery::ListOwningNamespaces { entity } => {
            let mut owner = None;
            let start = if let Some(ImpactEntity::Namespace(body)) = input.body(*entity) {
                body.parent
            } else {
                for candidate in input.entities {
                    charge_work(work, 1, max_work)?;
                    if let ImpactEntity::Namespace(body) = candidate
                        && body.members.binary_search(entity).is_ok()
                    {
                        owner = Some(body.entity_id);
                        break;
                    }
                }
                owner
            };
            let mut chain = Vec::new();
            let mut cursor = start;
            while let Some(current) = cursor {
                charge_work(work, 1, max_work)?;
                if chain.len() >= input.entities.len() {
                    return fail(RootQueryErrorCode::InternalInvariant);
                }
                chain.push(current);
                cursor = match input.body(current) {
                    Some(ImpactEntity::Namespace(body)) => body.parent,
                    _ => return fail(RootQueryErrorCode::InternalInvariant),
                };
            }
            RootQueryResult::Entities(chain)
        }
        RootQuery::ListEntryPoints => {
            let mut rows = Vec::new();
            for entry_point in input.facts.entry_points {
                charge_work(work, 1, max_work)?;
                let Some(ImpactEntity::EntryPoint(body)) = input.body(*entry_point) else {
                    return fail(RootQueryErrorCode::InternalInvariant);
                };
                rows.push(EntryRow {
                    entry_point: *entry_point,
                    function: body.function,
                    exposure: exposure_tag(body.exposure),
                });
            }
            RootQueryResult::EntryRows(rows)
        }
        RootQuery::ListDependencyRoots => {
            charge_work(work, to_u64(input.facts.dependency_roots.len())?, max_work)?;
            RootQueryResult::Roots(input.facts.dependency_roots.to_vec())
        }
        RootQuery::ListDirectDependencies { entity, kinds }
        | RootQuery::ListDirectDependents { entity, kinds } => {
            let dependencies = matches!(query, RootQuery::ListDirectDependencies { .. });
            let mut edges = Vec::new();
            for edge in input.snapshot.direct_edges() {
                charge_work(work, 1, max_work)?;
                let matches = if dependencies {
                    edge.dependent == *entity
                } else {
                    edge.dependency == *entity
                };
                if matches && kinds.binary_search(&edge.kind).is_ok() {
                    edges.push(*edge);
                }
            }
            RootQueryResult::Edges(edges)
        }
        RootQuery::ReverseImpactClosure { seeds } => {
            let (entities, reached) = reverse_closure(
                input.snapshot,
                seeds,
                work,
                QueryLimits {
                    max_work,
                    ..QueryLimits::profile_maximum()
                },
            )?;
            depth = reached;
            RootQueryResult::Entities(entities)
        }
        RootQuery::ForwardDependencyClosure { seeds } => {
            let (entities, reached) = forward_closure(input.snapshot, seeds, work, max_work)?;
            depth = reached;
            RootQueryResult::Entities(entities)
        }
        RootQuery::ListContractsFor { target } => {
            let mut contracts = Vec::new();
            for entity in input.entities {
                charge_work(work, 1, max_work)?;
                if let ImpactEntity::Contract(body) = entity
                    && body.target == *target
                {
                    contracts.push(body.entity_id);
                }
            }
            RootQueryResult::Entities(contracts)
        }
        RootQuery::ListTestsFor { target } => {
            let mut tests = Vec::new();
            for entity in input.entities {
                charge_work(work, 1, max_work)?;
                if let ImpactEntity::TestCase(body) = entity
                    && body.target == *target
                {
                    tests.push(body.entity_id);
                }
            }
            RootQueryResult::Entities(tests)
        }
        RootQuery::ListDeclaredEffects { entity } => {
            charge_work(work, 1, max_work)?;
            let effects = match input.body(*entity) {
                Some(ImpactEntity::Function(body)) => body.effects.clone(),
                Some(ImpactEntity::AdapterImport(body)) => body.effects.clone(),
                Some(ImpactEntity::CapabilityRequirement(body)) => vec![body.effect],
                _ => return fail(RootQueryErrorCode::ClassNotApplicable),
            };
            charge_work(work, to_u64(effects.len())?, max_work)?;
            RootQueryResult::Entities(canonical(effects))
        }
        RootQuery::ListCapabilityRequirementsFor { subject } => {
            let mut requirements = Vec::new();
            for entity in input.entities {
                charge_work(work, 1, max_work)?;
                match entity {
                    ImpactEntity::PolicyBinding(body) if body.subject == *subject => {
                        requirements.extend(body.requirements.iter().copied());
                    }
                    ImpactEntity::Workspace(body) if body.entity_id == *subject => {
                        requirements.extend(body.capability_requirements.iter().copied());
                    }
                    _ => {}
                }
            }
            RootQueryResult::Entities(canonical(requirements))
        }
    };
    Ok((Complete { result }, depth))
}

fn canonical(mut ids: Vec<EntityId>) -> Vec<EntityId> {
    ids.sort_unstable();
    ids.dedup();
    ids
}

const fn exposure_tag(exposure: EntryExposure) -> u32 {
    match exposure {
        EntryExposure::Local => 1,
        EntryExposure::Protocol => 2,
    }
}

fn forward_closure(
    snapshot: &IndexSnapshot,
    seeds: &[EntityId],
    work: &mut u64,
    max_work: u64,
) -> Result<(Vec<EntityId>, u32), RootQueryError> {
    let edges = snapshot.direct_edges();
    let mut depths = BTreeMap::<EntityId, u32>::new();
    let mut queue = VecDeque::new();
    for seed in seeds {
        depths.insert(*seed, 0);
        queue.push_back((*seed, 0_u32));
    }
    let mut reached = 0_u32;
    while let Some((dependent, depth)) = queue.pop_front() {
        charge_work(work, 1, max_work)?;
        let start = edges.partition_point(|edge| edge.dependent < dependent);
        for edge in edges[start..]
            .iter()
            .take_while(|edge| edge.dependent == dependent)
        {
            charge_work(work, 1, max_work)?;
            if depths.contains_key(&edge.dependency) {
                continue;
            }
            let next = depth
                .checked_add(1)
                .ok_or_else(|| RootQueryError::new(RootQueryErrorCode::ResourceLimit))?;
            depths.insert(edge.dependency, next);
            reached = reached.max(next);
            queue.push_back((edge.dependency, next));
        }
    }
    Ok((depths.into_keys().collect(), reached))
}

fn edge_key(edge: &ImpactEdge) -> (EntityId, EntityId, u32) {
    (edge.dependent, edge.dependency, edge.kind.tag())
}

#[allow(clippy::too_many_lines)]
fn page(complete: Complete, request: &RootQueryRequest) -> Result<Paged, RootQueryError> {
    let limits = request.limits;
    let after = request.after;
    let allow = request.allow_continuation;
    let single = |result: RootQueryResult, payload_bytes: u64| Paged {
        result,
        total_count: 1,
        returned: 1,
        truncated: false,
        next_after: None,
        payload_bytes,
    };
    Ok(match complete.result {
        RootQueryResult::RootSummary(summary) => single(
            RootQueryResult::RootSummary(summary),
            32 * 6 + 8 * 18 + 8 * 3,
        ),
        RootQueryResult::Entity {
            kind,
            object_id,
            fingerprint,
        } => {
            let bytes = 4 + 32 + option_bytes(fingerprint.is_some());
            single(
                RootQueryResult::Entity {
                    kind,
                    object_id,
                    fingerprint,
                },
                bytes,
            )
        }
        RootQueryResult::Fingerprint(fingerprint) => {
            let bytes = option_bytes(fingerprint.is_some());
            single(RootQueryResult::Fingerprint(fingerprint), bytes)
        }
        RootQueryResult::Entities(entities) => {
            if request.query.key_tag().is_none() {
                // Single-key chain: never paged.
                let count = to_u64(entities.len())?;
                if count > limits.max_returned_entities {
                    return fail(RootQueryErrorCode::RequiredFactOmitted);
                }
                return Ok(Paged {
                    payload_bytes: list_bytes(count, 32)?,
                    result: RootQueryResult::Entities(entities),
                    total_count: count,
                    returned: count,
                    truncated: false,
                    next_after: None,
                });
            }
            let after_key = match after {
                Some(Cursor::Entity(entity)) => Some(entity),
                _ => None,
            };
            let (items, total, truncated) = page_items(
                entities,
                |entity| after_key.is_none_or(|key| *entity > key),
                limits.max_returned_entities,
                allow,
            )?;
            let next = if truncated {
                items.last().copied().map(Cursor::Entity)
            } else {
                None
            };
            let returned = to_u64(items.len())?;
            Paged {
                payload_bytes: list_bytes(returned, 32)?,
                result: RootQueryResult::Entities(items),
                total_count: total,
                returned,
                truncated,
                next_after: next,
            }
        }
        RootQueryResult::DependencyRows(rows) => {
            let after_key = match after {
                Some(Cursor::Entity(entity)) => Some(entity),
                _ => None,
            };
            let (items, total, truncated) = page_items(
                rows,
                |row| after_key.is_none_or(|key| row.binding > key),
                limits.max_returned_entities,
                allow,
            )?;
            let next = if truncated {
                items.last().map(|row| Cursor::Entity(row.binding))
            } else {
                None
            };
            let returned = to_u64(items.len())?;
            Paged {
                payload_bytes: list_bytes(returned, 128)?,
                result: RootQueryResult::DependencyRows(items),
                total_count: total,
                returned,
                truncated,
                next_after: next,
            }
        }
        RootQueryResult::InventoryEntries(entries) => {
            let after_key = match after {
                Some(Cursor::Entity(entity)) => Some(entity),
                _ => None,
            };
            let (items, total, truncated) = page_items(
                entries,
                |entry| after_key.is_none_or(|key| entry.entity > key),
                limits.max_returned_entities,
                allow,
            )?;
            let next = if truncated {
                items.last().map(|entry| Cursor::Entity(entry.entity))
            } else {
                None
            };
            let returned = to_u64(items.len())?;
            Paged {
                payload_bytes: list_bytes(returned, 36)?,
                result: RootQueryResult::InventoryEntries(items),
                total_count: total,
                returned,
                truncated,
                next_after: next,
            }
        }
        RootQueryResult::EntryRows(rows) => {
            let after_key = match after {
                Some(Cursor::Entity(entity)) => Some(entity),
                _ => None,
            };
            let (items, total, truncated) = page_items(
                rows,
                |row| after_key.is_none_or(|key| row.entry_point > key),
                limits.max_returned_entities,
                allow,
            )?;
            let next = if truncated {
                items.last().map(|row| Cursor::Entity(row.entry_point))
            } else {
                None
            };
            let returned = to_u64(items.len())?;
            Paged {
                payload_bytes: list_bytes(returned, 68)?,
                result: RootQueryResult::EntryRows(items),
                total_count: total,
                returned,
                truncated,
                next_after: next,
            }
        }
        RootQueryResult::Roots(roots) => {
            let after_key = match after {
                Some(Cursor::Root(root)) => Some(root),
                _ => None,
            };
            let (items, total, truncated) = page_items(
                roots,
                |root| after_key.is_none_or(|key| *root > key),
                limits.max_returned_entities,
                allow,
            )?;
            let next = if truncated {
                items.last().copied().map(Cursor::Root)
            } else {
                None
            };
            let returned = to_u64(items.len())?;
            Paged {
                payload_bytes: list_bytes(returned, 32)?,
                result: RootQueryResult::Roots(items),
                total_count: total,
                returned,
                truncated,
                next_after: next,
            }
        }
        RootQueryResult::Edges(edges) => {
            let after_key = match after {
                Some(Cursor::Edge(edge)) => Some(edge_key(&edge)),
                _ => None,
            };
            let (items, total, truncated) = page_items(
                edges,
                |edge| after_key.is_none_or(|key| edge_key(edge) > key),
                limits.max_returned_edges,
                allow,
            )?;
            let next = if truncated {
                items.last().copied().map(Cursor::Edge)
            } else {
                None
            };
            let returned = to_u64(items.len())?;
            Paged {
                payload_bytes: list_bytes(returned, 68)?,
                result: RootQueryResult::Edges(items),
                total_count: total,
                returned,
                truncated,
                next_after: next,
            }
        }
    })
}

fn page_items<T>(
    items: Vec<T>,
    after: impl Fn(&T) -> bool,
    limit: u64,
    allow_continuation: bool,
) -> Result<(Vec<T>, u64, bool), RootQueryError> {
    let total = to_u64(items.len())?;
    let remaining: Vec<T> = items.into_iter().filter(|item| after(item)).collect();
    let remaining_count = to_u64(remaining.len())?;
    let truncated = remaining_count > limit;
    if truncated && !allow_continuation {
        return fail(RootQueryErrorCode::RequiredFactOmitted);
    }
    let take = usize::try_from(limit.min(remaining_count))
        .map_err(|_| RootQueryError::new(RootQueryErrorCode::ResourceLimit))?;
    let mut page = remaining;
    page.truncate(take);
    Ok((page, total, truncated))
}

const fn option_bytes(present: bool) -> u64 {
    if present { 4 + 32 } else { 4 }
}

fn list_bytes(count: u64, item_bytes: u64) -> Result<u64, RootQueryError> {
    count
        .checked_mul(item_bytes)
        .and_then(|bytes| bytes.checked_add(8))
        .ok_or_else(|| RootQueryError::new(RootQueryErrorCode::ResourceLimit))
}

fn build_response(
    request: &RootQueryRequest,
    paged: Paged,
    reached_depth: u32,
    traversal_work: u64,
) -> Result<RootQueryResponse, RootQueryError> {
    // Fixed header: magic 8, versions 8, five identities 160, arm 4,
    // profile 4, limits 36, flag 4, cursor, class 4, counts 16, truncated 4,
    // cursor, depth 4, work 8, bytes 8, result tag 4.
    let header = 8_u64
        + 8
        + 160
        + 4
        + 4
        + 36
        + 4
        + cursor_bytes(request.after)
        + 4
        + 16
        + 4
        + cursor_bytes(paged.next_after)
        + 4
        + 8
        + 8
        + 4;
    let response_bytes = header
        .checked_add(paged.payload_bytes)
        .ok_or_else(|| RootQueryError::new(RootQueryErrorCode::ResourceLimit))?;
    if response_bytes > MAX_QUERY_RESPONSE_BYTES {
        return fail(RootQueryErrorCode::ResourceLimit);
    }
    let charged_work = traversal_work
        .checked_add(response_bytes)
        .ok_or_else(|| RootQueryError::new(RootQueryErrorCode::ResourceLimit))?;
    if charged_work > MAX_QUERY_WORK || charged_work > request.limits.max_work {
        return fail(RootQueryErrorCode::ResourceLimit);
    }
    if response_bytes > request.limits.max_response_bytes {
        return fail(RootQueryErrorCode::RequiredFactOmitted);
    }
    let record = encode_response(request, &paged, reached_depth, charged_work, response_bytes)?;
    Ok(RootQueryResponse {
        query_id: request.query_id,
        snapshot_id: request.snapshot_id,
        schema_epoch: request.schema_epoch,
        root: request.root,
        workspace_id: request.workspace_id,
        applied_limits: request.limits,
        allow_continuation: request.allow_continuation,
        after: request.after,
        class_tag: request.query.tag(),
        total_count: paged.total_count,
        returned: paged.returned,
        truncated: paged.truncated,
        next_after: paged.next_after,
        reached_depth,
        charged_work,
        response_bytes,
        result: paged.result,
        record,
    })
}

#[allow(clippy::too_many_lines)]
fn encode_response(
    request: &RootQueryRequest,
    paged: &Paged,
    reached_depth: u32,
    charged_work: u64,
    response_bytes: u64,
) -> Result<Vec<u8>, RootQueryError> {
    let capacity = usize::try_from(response_bytes)
        .map_err(|_| RootQueryError::new(RootQueryErrorCode::ResourceLimit))?;
    let mut out = Vec::with_capacity(capacity);
    append_response(&mut out, ROOT_RESPONSE_MAGIC, capacity)?;
    push_response_u32(&mut out, FORMAT_VERSION, capacity)?;
    push_response_u32(&mut out, PROFILE_VERSION, capacity)?;
    append_response(&mut out, request.query_id.as_bytes(), capacity)?;
    append_response(&mut out, request.snapshot_id.as_bytes(), capacity)?;
    append_response(&mut out, request.schema_epoch.as_bytes(), capacity)?;
    append_response(&mut out, request.root.as_bytes(), capacity)?;
    append_response(&mut out, request.workspace_id.as_bytes(), capacity)?;
    push_response_u32(&mut out, COMPLETENESS_COMPLETE_ROOT, capacity)?;
    push_response_u32(&mut out, LIMITS_PROFILE, capacity)?;
    encode_limits_response(&mut out, request.limits, capacity)?;
    push_response_u32(&mut out, flag(request.allow_continuation), capacity)?;
    encode_cursor_response(&mut out, request.after, capacity)?;
    push_response_u32(&mut out, request.query.tag(), capacity)?;
    push_response_u64(&mut out, paged.total_count, capacity)?;
    push_response_u64(&mut out, paged.returned, capacity)?;
    push_response_u32(&mut out, flag(paged.truncated), capacity)?;
    encode_cursor_response(&mut out, paged.next_after, capacity)?;
    push_response_u32(&mut out, reached_depth, capacity)?;
    push_response_u64(&mut out, charged_work, capacity)?;
    push_response_u64(&mut out, response_bytes, capacity)?;
    push_response_u32(&mut out, request.query.tag(), capacity)?;
    match &paged.result {
        RootQueryResult::RootSummary(summary) => {
            append_response(&mut out, summary.workspace_id.as_bytes(), capacity)?;
            append_response(&mut out, summary.schema_epoch.as_bytes(), capacity)?;
            append_response(&mut out, summary.root.as_bytes(), capacity)?;
            append_response(&mut out, summary.contract_root.as_bytes(), capacity)?;
            append_response(&mut out, summary.test_root.as_bytes(), capacity)?;
            append_response(&mut out, summary.policy_root.as_bytes(), capacity)?;
            for count in summary.kind_counts {
                push_response_u64(&mut out, count, capacity)?;
            }
            push_response_u64(&mut out, summary.entry_points, capacity)?;
            push_response_u64(&mut out, summary.dependency_roots, capacity)?;
            push_response_u64(&mut out, summary.direct_edges, capacity)?;
        }
        RootQueryResult::Entity {
            kind,
            object_id,
            fingerprint,
        } => {
            push_response_u32(&mut out, kind.tag(), capacity)?;
            append_response(&mut out, object_id.as_bytes(), capacity)?;
            encode_fingerprint_option(&mut out, *fingerprint, capacity)?;
        }
        RootQueryResult::Fingerprint(fingerprint) => {
            encode_fingerprint_option(&mut out, *fingerprint, capacity)?;
        }
        RootQueryResult::Entities(entities) => {
            push_response_u64(&mut out, to_u64(entities.len())?, capacity)?;
            for entity in entities {
                append_response(&mut out, entity.as_bytes(), capacity)?;
            }
        }
        RootQueryResult::DependencyRows(rows) => {
            push_response_u64(&mut out, to_u64(rows.len())?, capacity)?;
            for row in rows {
                append_response(&mut out, row.binding.as_bytes(), capacity)?;
                append_response(&mut out, row.dependency_root.as_bytes(), capacity)?;
                append_response(&mut out, row.external_package.as_bytes(), capacity)?;
                append_response(&mut out, row.local_namespace.as_bytes(), capacity)?;
            }
        }
        RootQueryResult::InventoryEntries(entries) => {
            push_response_u64(&mut out, to_u64(entries.len())?, capacity)?;
            for entry in entries {
                append_response(&mut out, entry.entity.as_bytes(), capacity)?;
                push_response_u32(&mut out, entry.kind.tag(), capacity)?;
            }
        }
        RootQueryResult::EntryRows(rows) => {
            push_response_u64(&mut out, to_u64(rows.len())?, capacity)?;
            for row in rows {
                append_response(&mut out, row.entry_point.as_bytes(), capacity)?;
                append_response(&mut out, row.function.as_bytes(), capacity)?;
                push_response_u32(&mut out, row.exposure, capacity)?;
            }
        }
        RootQueryResult::Roots(roots) => {
            push_response_u64(&mut out, to_u64(roots.len())?, capacity)?;
            for root in roots {
                append_response(&mut out, root.as_bytes(), capacity)?;
            }
        }
        RootQueryResult::Edges(edges) => {
            push_response_u64(&mut out, to_u64(edges.len())?, capacity)?;
            for edge in edges {
                append_response(&mut out, edge.dependent.as_bytes(), capacity)?;
                append_response(&mut out, edge.dependency.as_bytes(), capacity)?;
                push_response_u32(&mut out, edge.kind.tag(), capacity)?;
            }
        }
    }
    if out.len() != capacity {
        return fail(RootQueryErrorCode::InternalInvariant);
    }
    Ok(out)
}

fn encode_fingerprint_option(
    out: &mut Vec<u8>,
    fingerprint: Option<SemanticFingerprint>,
    limit: usize,
) -> Result<(), QueryError> {
    match fingerprint {
        None => push_response_u32(out, OPTION_NONE, limit),
        Some(fingerprint) => {
            push_response_u32(out, OPTION_SOME, limit)?;
            append_response(out, fingerprint.as_bytes(), limit)
        }
    }
}

/// Returns whether the class admits the subject kind, executing the
/// normative class-kind applicability table in section 2 of the
/// root-backed query profile. Classes without a subject (1, 4, 5, 10,
/// 11) accept every kind here because they name no entity; every other
/// class bearing an entity, seed, target, or subject accepts every kind
/// except 6, 7, 8, and 18, which are restricted to the kinds their
/// bodies are defined over.
#[must_use]
pub fn class_applies_to(class_tag: u32, kind: ModeledEntityKind) -> bool {
    match class_tag {
        6 | 7 => kind == ModeledEntityKind::Package,
        8 => kind == ModeledEntityKind::Namespace,
        18 => matches!(
            kind,
            ModeledEntityKind::Function
                | ModeledEntityKind::AdapterImport
                | ModeledEntityKind::CapabilityRequirement
        ),
        1 | 2 | 3 | 4 | 5 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 19 => true,
        _ => false,
    }
}

#[cfg(test)]
#[allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::needless_pass_by_value
)]
pub(crate) mod tests {
    use super::*;
    use crate::SnapshotContext;
    use crate::complete_root::tests::Fixture;
    use crate::{build_complete_root_snapshot, build_index_snapshot};

    /// The honestly committed root of the shared fixture: `verify()`
    /// recomputes the `StateRoot` digest from the nine `STATE_ROOT_V1`
    /// fields, so the test root is that digest over the exact fields
    /// `Owned` carries, not a bare constant. Any caller-declared fact
    /// that drifts from these fields stops verifying.
    pub(crate) fn root() -> StateRoot {
        let fixture = Fixture::new();
        let bindings: Vec<(EntityId, ObjectId)> = fixture
            .bound_entities
            .iter()
            .map(|entity| {
                (
                    *entity,
                    ObjectId::from_bytes([entity.as_bytes()[0] | 0x80; 32]),
                )
            })
            .collect();
        let record = StateRootRecord {
            workspace_id: workspace(),
            schema_epoch_id: epoch(),
            entity_bindings: bindings,
            entry_points: fixture.entry_points.clone(),
            dependency_roots: fixture.dependency_roots.clone(),
            contract_root: ObjectId::from_bytes([0xC0; 32]),
            test_root: ObjectId::from_bytes([0xD0; 32]),
            policy_root: PolicyRootId::from_bytes([0xE0; 32]),
            interpretation_flags: Vec::new(),
        };
        recompute_root(&record).expect("fixture fields recompute to the test root")
    }

    pub(crate) fn epoch() -> SchemaEpochId {
        SchemaEpochId::from_bytes([0x11; 32])
    }

    pub(crate) fn workspace() -> WorkspaceId {
        WorkspaceId::from_bytes([0x22; 32])
    }

    pub(crate) fn id(byte: u8) -> EntityId {
        EntityId::from_bytes([byte; 32])
    }

    /// Owned parts of a root-backed input over the shared fixture.
    pub(crate) struct Owned {
        pub(crate) fixture: Fixture,
        pub(crate) snapshot: IndexSnapshot,
        pub(crate) bindings: Vec<(EntityId, ObjectId)>,
        pub(crate) fingerprints: Vec<(EntityId, SemanticFingerprint)>,
        pub(crate) interpretation_flags: Vec<u32>,
    }

    impl Owned {
        pub(crate) fn new() -> Self {
            let fixture = Fixture::new();
            let entities = fixture.entities();
            let snapshot =
                build_complete_root_snapshot(epoch(), root(), &entities, fixture.facts()).unwrap();
            let bindings = fixture
                .bound_entities
                .iter()
                .map(|entity| {
                    (
                        *entity,
                        ObjectId::from_bytes([entity.as_bytes()[0] | 0x80; 32]),
                    )
                })
                .collect();
            let fingerprints = vec![
                (id(0x06), SemanticFingerprint::from_bytes([0x66; 32])),
                (id(0x07), SemanticFingerprint::from_bytes([0x77; 32])),
            ];
            Self {
                fixture,
                snapshot,
                bindings,
                fingerprints,
                interpretation_flags: Vec::new(),
            }
        }
    }

    pub(crate) struct Borrowed<'a> {
        pub(crate) entities: Vec<ImpactEntity<'a>>,
        pub(crate) owned: &'a Owned,
    }

    impl<'a> Borrowed<'a> {
        pub(crate) fn new(owned: &'a Owned) -> Self {
            Self {
                entities: owned.fixture.entities(),
                owned,
            }
        }

        pub(crate) fn input(&'a self) -> RootQueryInput<'a> {
            RootQueryInput {
                snapshot: &self.owned.snapshot,
                entities: &self.entities,
                facts: self.owned.fixture.facts(),
                bindings: &self.owned.bindings,
                fingerprints: &self.owned.fingerprints,
                root: root(),
                workspace_id: workspace(),
                schema_epoch: epoch(),
                contract_root: ObjectId::from_bytes([0xC0; 32]),
                test_root: ObjectId::from_bytes([0xD0; 32]),
                policy_root: PolicyRootId::from_bytes([0xE0; 32]),
                interpretation_flags: &self.owned.interpretation_flags,
            }
        }
    }

    pub(crate) fn all_classes() -> Vec<RootQuery> {
        vec![
            RootQuery::GetRootSummary,
            RootQuery::GetEntity { entity: id(0x07) },
            RootQuery::GetSemanticFingerprint { entity: id(0x06) },
            RootQuery::ListEntitiesByKind {
                kind: ModeledEntityKind::Namespace,
            },
            RootQuery::ListWorkspacePackages,
            RootQuery::ListPackageExports { package: id(0x03) },
            RootQuery::ListPackageDependencies { package: id(0x03) },
            RootQuery::ListNamespaceMembers {
                namespace: id(0x04),
            },
            RootQuery::ListOwningNamespaces { entity: id(0x07) },
            RootQuery::ListEntryPoints,
            RootQuery::ListDependencyRoots,
            RootQuery::ListDirectDependencies {
                entity: id(0x07),
                kinds: vec![ImpactKind::Ownership, ImpactKind::ControlFlow],
            },
            RootQuery::ListDirectDependents {
                entity: id(0x07),
                kinds: vec![
                    ImpactKind::Ownership,
                    ImpactKind::Contract,
                    ImpactKind::TestTarget,
                ],
            },
            RootQuery::ReverseImpactClosure {
                seeds: vec![id(0x07)],
            },
            RootQuery::ForwardDependencyClosure {
                seeds: vec![id(0x0a)],
            },
            RootQuery::ListContractsFor { target: id(0x06) },
            RootQuery::ListTestsFor { target: id(0x07) },
            RootQuery::ListDeclaredEffects { entity: id(0x0f) },
            RootQuery::ListCapabilityRequirementsFor { subject: id(0x07) },
        ]
    }

    /// The frozen fixture has only one item in these classes, so synthetic
    /// complete results exercise truncation while requests use real validation.
    #[test]
    fn single_item_classes_walk_two_items_at_limit_one() {
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let one = QueryLimits {
            max_returned_entities: 1,
            ..QueryLimits::profile_maximum()
        };
        let roots = [
            StateRoot::from_bytes([0x10; 32]),
            StateRoot::from_bytes([0xFE; 32]),
        ];
        let rows = [
            EntryRow {
                entry_point: id(0x30),
                function: id(0x07),
                exposure: 1,
            },
            EntryRow {
                entry_point: id(0x31),
                function: id(0x07),
                exposure: 1,
            },
        ];
        let first_request =
            build_root_query_request(&input, RootQuery::ListDependencyRoots, one, true, None)
                .unwrap();
        let first = page(
            Complete {
                result: RootQueryResult::Roots(roots.to_vec()),
            },
            &first_request,
        )
        .unwrap();
        assert_eq!(
            (first.total_count, first.returned, first.truncated),
            (2, 1, true)
        );
        assert_eq!(first.next_after, Some(Cursor::Root(roots[0])));
        let second_request = build_root_query_request(
            &input,
            RootQuery::ListDependencyRoots,
            one,
            true,
            first.next_after,
        )
        .unwrap();
        let second = page(
            Complete {
                result: RootQueryResult::Roots(roots.to_vec()),
            },
            &second_request,
        )
        .unwrap();
        assert_eq!(
            (second.total_count, second.returned, second.truncated),
            (2, 1, false)
        );
        assert_eq!(second.next_after, None);
        match (&first.result, &second.result) {
            (RootQueryResult::Roots(a), RootQueryResult::Roots(b)) => {
                assert_eq!((a.as_slice(), b.as_slice()), (&roots[..1], &roots[1..]));
            }
            other => panic!("expected root pages, got {other:?}"),
        }

        let first_request =
            build_root_query_request(&input, RootQuery::ListEntryPoints, one, true, None).unwrap();
        let first = page(
            Complete {
                result: RootQueryResult::EntryRows(rows.to_vec()),
            },
            &first_request,
        )
        .unwrap();
        assert_eq!(
            (first.total_count, first.returned, first.truncated),
            (2, 1, true)
        );
        assert_eq!(first.next_after, Some(Cursor::Entity(rows[0].entry_point)));
        let second_request = build_root_query_request(
            &input,
            RootQuery::ListEntryPoints,
            one,
            true,
            first.next_after,
        )
        .unwrap();
        let second = page(
            Complete {
                result: RootQueryResult::EntryRows(rows.to_vec()),
            },
            &second_request,
        )
        .unwrap();
        assert_eq!(
            (second.total_count, second.returned, second.truncated),
            (2, 1, false)
        );
        assert_eq!(second.next_after, None);
        match (&first.result, &second.result) {
            (RootQueryResult::EntryRows(a), RootQueryResult::EntryRows(b)) => {
                assert_eq!(a.len() + b.len(), 2);
                assert_eq!(a[0].entry_point, rows[0].entry_point);
                assert_eq!(b[0].entry_point, rows[1].entry_point);
            }
            other => panic!("expected entry pages, got {other:?}"),
        }

        // Class 7 `ListPackageDependencies`: `DependencyRows` keyed by
        // `row.binding` with an entity cursor. `id(0x03)` is the fixture
        // package the arm gate admits.
        let dependency_rows = [
            DependencyRow {
                binding: id(0x40),
                dependency_root: StateRoot::from_bytes([0x09; 32]),
                external_package: id(0x03),
                local_namespace: id(0x04),
            },
            DependencyRow {
                binding: id(0x41),
                dependency_root: StateRoot::from_bytes([0x09; 32]),
                external_package: id(0x03),
                local_namespace: id(0x04),
            },
        ];
        let first_request = build_root_query_request(
            &input,
            RootQuery::ListPackageDependencies { package: id(0x03) },
            one,
            true,
            None,
        )
        .unwrap();
        let first = page(
            Complete {
                result: RootQueryResult::DependencyRows(dependency_rows.to_vec()),
            },
            &first_request,
        )
        .unwrap();
        assert_eq!(
            (first.total_count, first.returned, first.truncated),
            (2, 1, true)
        );
        assert_eq!(
            first.next_after,
            Some(Cursor::Entity(dependency_rows[0].binding))
        );
        let second_request = build_root_query_request(
            &input,
            RootQuery::ListPackageDependencies { package: id(0x03) },
            one,
            true,
            first.next_after,
        )
        .unwrap();
        let second = page(
            Complete {
                result: RootQueryResult::DependencyRows(dependency_rows.to_vec()),
            },
            &second_request,
        )
        .unwrap();
        assert_eq!(
            (second.total_count, second.returned, second.truncated),
            (2, 1, false)
        );
        assert_eq!(second.next_after, None);
        match (&first.result, &second.result) {
            (RootQueryResult::DependencyRows(a), RootQueryResult::DependencyRows(b)) => {
                assert_eq!(
                    (a.as_slice(), b.as_slice()),
                    (&dependency_rows[..1], &dependency_rows[1..])
                );
            }
            other => panic!("expected dependency-row pages, got {other:?}"),
        }

        // Class 8 `ListNamespaceMembers`: `InventoryEntries` keyed by
        // `entry.entity` with an entity cursor. `id(0x04)` is the fixture
        // namespace the arm gate admits.
        let entries = [
            IndexInventoryEntry {
                entity: id(0x50),
                kind: ModeledEntityKind::Function,
            },
            IndexInventoryEntry {
                entity: id(0x51),
                kind: ModeledEntityKind::TypeDef,
            },
        ];
        let first_request = build_root_query_request(
            &input,
            RootQuery::ListNamespaceMembers {
                namespace: id(0x04),
            },
            one,
            true,
            None,
        )
        .unwrap();
        let first = page(
            Complete {
                result: RootQueryResult::InventoryEntries(entries.to_vec()),
            },
            &first_request,
        )
        .unwrap();
        assert_eq!(
            (first.total_count, first.returned, first.truncated),
            (2, 1, true)
        );
        assert_eq!(first.next_after, Some(Cursor::Entity(entries[0].entity)));
        let second_request = build_root_query_request(
            &input,
            RootQuery::ListNamespaceMembers {
                namespace: id(0x04),
            },
            one,
            true,
            first.next_after,
        )
        .unwrap();
        let second = page(
            Complete {
                result: RootQueryResult::InventoryEntries(entries.to_vec()),
            },
            &second_request,
        )
        .unwrap();
        assert_eq!(
            (second.total_count, second.returned, second.truncated),
            (2, 1, false)
        );
        assert_eq!(second.next_after, None);
        match (&first.result, &second.result) {
            (RootQueryResult::InventoryEntries(a), RootQueryResult::InventoryEntries(b)) => {
                assert_eq!((a.as_slice(), b.as_slice()), (&entries[..1], &entries[1..]));
            }
            other => panic!("expected inventory-entry pages, got {other:?}"),
        }
    }

    fn run(input: &RootQueryInput<'_>, query: RootQuery) -> RootQueryResponse {
        let request =
            build_root_query_request(input, query, QueryLimits::profile_maximum(), false, None)
                .unwrap();
        execute_root_query(input, &request).unwrap()
    }

    fn run_err(
        input: &RootQueryInput<'_>,
        query: RootQuery,
        limits: QueryLimits,
        allow: bool,
        after: Option<Cursor>,
    ) -> RootQueryErrorCode {
        match build_root_query_request(input, query, limits, allow, after) {
            Ok(request) => execute_root_query(input, &request).unwrap_err().code(),
            Err(error) => error.code(),
        }
    }

    #[test]
    fn every_class_answers_exact_facts_over_the_fixture() {
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let classes = all_classes();
        assert_eq!(classes.len(), ROOT_QUERY_CLASSES as usize);
        let responses: Vec<RootQueryResponse> = classes
            .iter()
            .cloned()
            .map(|query| run(&input, query))
            .collect();
        for (index, response) in responses.iter().enumerate() {
            assert_eq!(response.class_tag(), index as u32 + 1);
            assert!(!response.truncated());
            assert_eq!(response.record().len(), response.response_bytes() as usize);
            assert_eq!(&response.record()[..8], b"SLEYRQR1");
        }
        let RootQueryResult::RootSummary(summary) = responses[0].result() else {
            panic!("class 1");
        };
        assert_eq!(summary.kind_counts.iter().sum::<u64>(), 19);
        assert_eq!(summary.kind_counts[2], 3);
        assert_eq!(summary.entry_points, 1);
        assert_eq!(summary.dependency_roots, 1);
        assert_eq!(
            summary.direct_edges,
            owned.snapshot.direct_edges().len() as u64
        );
        assert_eq!(
            responses[1].result(),
            &RootQueryResult::Entity {
                kind: ModeledEntityKind::Function,
                object_id: ObjectId::from_bytes([0x87; 32]),
                fingerprint: Some(SemanticFingerprint::from_bytes([0x77; 32])),
            }
        );
        assert_eq!(
            responses[2].result(),
            &RootQueryResult::Fingerprint(Some(SemanticFingerprint::from_bytes([0x66; 32])))
        );
        assert_eq!(
            responses[3].result(),
            &RootQueryResult::Entities(vec![id(0x02), id(0x04), id(0x05)])
        );
        assert_eq!(
            responses[4].result(),
            &RootQueryResult::Entities(vec![id(0x03)])
        );
        let RootQueryResult::DependencyRows(rows) = responses[6].result() else {
            panic!("class 7");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].binding, id(0x11));
        assert_eq!(rows[0].dependency_root, StateRoot::from_bytes([0x09; 32]));
        let RootQueryResult::InventoryEntries(entries) = responses[7].result() else {
            panic!("class 8");
        };
        assert!(entries.iter().any(|entry| entry.entity == id(0x05)));
        let RootQueryResult::Entities(chain) = responses[8].result() else {
            panic!("class 9");
        };
        assert!(!chain.is_empty(), "the function is owned by a namespace");
        assert_eq!(
            *chain.last().unwrap(),
            id(0x04),
            "package root namespace ends the chain"
        );
        assert_eq!(
            responses[9].result(),
            &RootQueryResult::EntryRows(vec![EntryRow {
                entry_point: id(0x0a),
                function: id(0x07),
                exposure: 2,
            }])
        );
        assert_eq!(
            responses[10].result(),
            &RootQueryResult::Roots(vec![StateRoot::from_bytes([0x09; 32])])
        );
        let RootQueryResult::Edges(dependencies) = responses[11].result() else {
            panic!("class 12");
        };
        assert!(dependencies.iter().all(|edge| edge.dependent == id(0x07)));
        assert!(dependencies.iter().any(|edge| edge.dependency == id(0x09)));
        let RootQueryResult::Edges(dependents) = responses[12].result() else {
            panic!("class 13");
        };
        assert!(dependents.iter().all(|edge| edge.dependency == id(0x07)));
        assert!(dependents.iter().any(|edge| edge.dependent == id(0x0d)));
        let RootQueryResult::Entities(reverse) = responses[13].result() else {
            panic!("class 14");
        };
        assert!(reverse.contains(&id(0x07)) && reverse.contains(&id(0x0a)));
        let RootQueryResult::Entities(forward) = responses[14].result() else {
            panic!("class 15");
        };
        assert_eq!(
            forward[0],
            id(0x07),
            "the entry point depends on its function"
        );
        assert!(forward.contains(&id(0x0a)) && forward.contains(&id(0x09)));
        assert_eq!(
            responses[15].result(),
            &RootQueryResult::Entities(vec![id(0x0d)])
        );
        assert_eq!(
            responses[16].result(),
            &RootQueryResult::Entities(vec![id(0x0e)])
        );
        assert_eq!(
            responses[17].result(),
            &RootQueryResult::Entities(vec![id(0x0c)])
        );
        assert_eq!(
            responses[18].result(),
            &RootQueryResult::Entities(vec![id(0x0b)])
        );
        let requirements = run(
            &input,
            RootQuery::ListCapabilityRequirementsFor { subject: id(0x01) },
        );
        assert_eq!(
            requirements.result(),
            &RootQueryResult::Entities(vec![id(0x0b)])
        );
        for _ in 0..128 {
            let again: Vec<RootQueryResponse> = classes
                .iter()
                .cloned()
                .map(|query| run(&input, query))
                .collect();
            assert_eq!(again, responses);
        }
    }

    #[test]
    fn continuation_pages_union_to_the_complete_result_with_exact_counts() {
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let limits = QueryLimits {
            max_returned_entities: 2,
            max_returned_edges: 3,
            ..QueryLimits::profile_maximum()
        };
        let complete = run(
            &input,
            RootQuery::ListEntitiesByKind {
                kind: ModeledEntityKind::Namespace,
            },
        );
        let RootQueryResult::Entities(expected) = complete.result().clone() else {
            panic!();
        };
        assert_eq!(
            run_err(
                &input,
                RootQuery::ListEntitiesByKind {
                    kind: ModeledEntityKind::Namespace
                },
                limits,
                false,
                None
            ),
            RootQueryErrorCode::RequiredFactOmitted
        );
        let mut after = None;
        let mut union = Vec::new();
        let mut pages = 0;
        loop {
            let request = build_root_query_request(
                &input,
                RootQuery::ListEntitiesByKind {
                    kind: ModeledEntityKind::Namespace,
                },
                limits,
                true,
                after,
            )
            .unwrap();
            let response = execute_root_query(&input, &request).unwrap();
            assert_eq!(response.total_count(), expected.len() as u64);
            let RootQueryResult::Entities(page) = response.result() else {
                panic!();
            };
            union.extend(page.iter().copied());
            pages += 1;
            if !response.truncated() {
                assert!(response.next_after().is_none());
                break;
            }
            after = response.next_after();
            assert!(after.is_some());
        }
        assert_eq!(pages, 2);
        assert_eq!(union, expected);

        // Edge paging with an edge cursor.
        let all_edges = run(
            &input,
            RootQuery::ListDirectDependencies {
                entity: id(0x01),
                kinds: vec![
                    ImpactKind::Ownership,
                    ImpactKind::Capability,
                    ImpactKind::Contract,
                    ImpactKind::TestTarget,
                ],
            },
        );
        let RootQueryResult::Edges(expected_edges) = all_edges.result().clone() else {
            panic!();
        };
        assert!(expected_edges.len() > 3);
        let first = execute_root_query(
            &input,
            &build_root_query_request(
                &input,
                RootQuery::ListDirectDependencies {
                    entity: id(0x01),
                    kinds: vec![
                        ImpactKind::Ownership,
                        ImpactKind::Capability,
                        ImpactKind::Contract,
                        ImpactKind::TestTarget,
                    ],
                },
                limits,
                true,
                None,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(first.truncated());
        assert_eq!(first.returned(), 3);
        let Some(Cursor::Edge(cursor)) = first.next_after() else {
            panic!("edge cursor");
        };
        assert_eq!(cursor, expected_edges[2]);
        let second = execute_root_query(
            &input,
            &build_root_query_request(
                &input,
                RootQuery::ListDirectDependencies {
                    entity: id(0x01),
                    kinds: vec![
                        ImpactKind::Ownership,
                        ImpactKind::Capability,
                        ImpactKind::Contract,
                        ImpactKind::TestTarget,
                    ],
                },
                limits,
                true,
                first.next_after(),
            )
            .unwrap(),
        )
        .unwrap();
        let RootQueryResult::Edges(rest) = second.result() else {
            panic!();
        };
        assert_eq!(rest, &expected_edges[3..]);
        assert_ne!(first.query_id(), second.query_id());

        // Wrong cursor type and cursors on single-key classes are invalid.
        assert_eq!(
            run_err(
                &input,
                RootQuery::ListEntitiesByKind {
                    kind: ModeledEntityKind::Namespace
                },
                limits,
                true,
                Some(Cursor::Root(root()))
            ),
            RootQueryErrorCode::ContinuationInvalid
        );
        assert_eq!(
            run_err(
                &input,
                RootQuery::GetEntity { entity: id(0x07) },
                limits,
                true,
                Some(Cursor::Entity(id(0x01)))
            ),
            RootQueryErrorCode::ContinuationInvalid
        );
        // Depth cuts are never paged.
        assert_eq!(
            run_err(
                &input,
                RootQuery::ReverseImpactClosure {
                    seeds: vec![id(0x09)]
                },
                QueryLimits {
                    max_depth: 0,
                    ..QueryLimits::profile_maximum()
                },
                true,
                None
            ),
            RootQueryErrorCode::RequiredFactOmitted
        );
        // A single-key chain that does not fit is omitted, never paged.
        assert_eq!(
            run_err(
                &input,
                RootQuery::ListOwningNamespaces { entity: id(0x07) },
                QueryLimits {
                    max_returned_entities: 1,
                    ..QueryLimits::profile_maximum()
                },
                true,
                None
            ),
            RootQueryErrorCode::RequiredFactOmitted
        );
    }

    #[test]
    fn applicability_resolution_shape_and_binding_failures_are_exact() {
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let limits = QueryLimits::profile_maximum();
        for (query, expected) in [
            (
                RootQuery::ListPackageExports { package: id(0x04) },
                RootQueryErrorCode::ClassNotApplicable,
            ),
            (
                RootQuery::ListNamespaceMembers {
                    namespace: id(0x03),
                },
                RootQueryErrorCode::ClassNotApplicable,
            ),
            (
                RootQuery::ListDeclaredEffects { entity: id(0x02) },
                RootQueryErrorCode::ClassNotApplicable,
            ),
            (
                RootQuery::GetEntity { entity: id(0x99) },
                RootQueryErrorCode::UnresolvedEntity,
            ),
            (
                RootQuery::ReverseImpactClosure {
                    seeds: vec![id(0x07), id(0x99)],
                },
                RootQueryErrorCode::UnresolvedEntity,
            ),
            (
                RootQuery::ListDirectDependencies {
                    entity: id(0x07),
                    kinds: vec![],
                },
                RootQueryErrorCode::RequestNotCanonical,
            ),
            (
                RootQuery::ForwardDependencyClosure {
                    seeds: vec![id(0x07), id(0x07)],
                },
                RootQueryErrorCode::RequestNotCanonical,
            ),
        ] {
            assert_eq!(
                run_err(&input, query.clone(), limits, false, None),
                expected,
                "{query:?}"
            );
        }
        assert_eq!(
            run_err(
                &input,
                RootQuery::GetRootSummary,
                QueryLimits {
                    max_work: 0,
                    ..limits
                },
                false,
                None
            ),
            RootQueryErrorCode::ResourceLimit
        );
        for kind in [
            ModeledEntityKind::Package,
            ModeledEntityKind::Namespace,
            ModeledEntityKind::Function,
        ] {
            assert!(class_applies_to(4, kind));
        }
        assert!(!class_applies_to(6, ModeledEntityKind::Namespace));
        assert!(!class_applies_to(18, ModeledEntityKind::Namespace));

        // Binding failures.
        let other_root = build_complete_root_snapshot(
            epoch(),
            StateRoot::from_bytes([0x34; 32]),
            &borrowed.entities,
            owned.fixture.facts(),
        )
        .unwrap();
        let mut wrong_root = input;
        wrong_root.snapshot = &other_root;
        assert_eq!(
            run_err(&wrong_root, RootQuery::GetRootSummary, limits, false, None),
            RootQueryErrorCode::RootMismatch
        );
        let mut short_bindings = input;
        short_bindings.bindings = &owned.bindings[1..];
        assert_eq!(
            run_err(
                &short_bindings,
                RootQuery::GetRootSummary,
                limits,
                false,
                None
            ),
            RootQueryErrorCode::RootMismatch
        );
        let bad_fingerprints = vec![(id(0x02), SemanticFingerprint::from_bytes([1; 32]))];
        let mut wrong_fingerprint = input;
        wrong_fingerprint.fingerprints = &bad_fingerprints;
        assert_eq!(
            run_err(
                &wrong_fingerprint,
                RootQuery::GetRootSummary,
                limits,
                false,
                None
            ),
            RootQueryErrorCode::RootMismatch
        );
        let mut swapped = borrowed.entities.clone();
        swapped.swap(0, 1);
        let mut unsorted = input;
        unsorted.entities = &swapped;
        assert_eq!(
            run_err(&unsorted, RootQuery::GetRootSummary, limits, false, None),
            RootQueryErrorCode::RootMismatch
        );
        let restricted = build_index_snapshot(
            SnapshotContext {
                schema_epoch: epoch(),
                claimed_root_context: Some(root()),
            },
            &[],
        )
        .unwrap();
        let mut arm_one = input;
        arm_one.snapshot = &restricted;
        assert_eq!(
            run_err(&arm_one, RootQuery::GetRootSummary, limits, false, None),
            RootQueryErrorCode::ProfileUnsupported
        );
        // A request bound to another input is a snapshot mismatch. The
        // foreign snapshot carries the same claimed root over different
        // edges (the workspace drops its capability requirement, which
        // removes exactly the workspace-to-requirement edge), so it
        // verifies on its own input while its snapshot id differs. A
        // forged root constant can no longer play this role: it fails
        // binding outright.
        let mut tweaked = Fixture::new();
        tweaked.workspace.capability_requirements = Vec::new();
        let tweaked_entities = tweaked.entities();
        let tweaked_snapshot =
            build_complete_root_snapshot(epoch(), root(), &tweaked_entities, tweaked.facts())
                .unwrap();
        assert_ne!(tweaked_snapshot.snapshot_id(), input.snapshot.snapshot_id());
        let tweaked_input = RootQueryInput {
            snapshot: &tweaked_snapshot,
            entities: &tweaked_entities,
            ..input
        };
        let foreign = build_root_query_request(
            &tweaked_input,
            RootQuery::GetRootSummary,
            limits,
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            execute_root_query(&input, &foreign).unwrap_err().code(),
            RootQueryErrorCode::SnapshotMismatch
        );
        let mut drifted =
            build_root_query_request(&input, RootQuery::GetRootSummary, limits, false, None)
                .unwrap();
        drifted.allow_continuation = true;
        assert_eq!(
            execute_root_query(&input, &drifted).unwrap_err().code(),
            RootQueryErrorCode::RequestNotCanonical
        );
        assert_eq!(RootQueryErrorCode::ALL.len(), 11);
        assert_eq!(RootQueryErrorCode::ALL[10].numeric(), 31_010);
        for pair in RootQueryErrorCode::ALL.windows(2) {
            assert!(pair[0].numeric() < pair[1].numeric());
        }
    }

    #[test]
    fn caller_declared_facts_are_bound_to_the_committed_root() {
        // Nabu P0: seven root-committed answer-bearing facts reach the
        // engine caller-declared (the bound ObjectIds class 2 answers,
        // facts.entry_points class 10 answers, facts.dependency_roots
        // class 11 answers, and the contract/test/policy roots class 1
        // answers, plus the interpretation flags), and none of them is in
        // the RootQueryId preimage. verify() must recompute the StateRoot
        // from the nine STATE_ROOT_V1 fields, so any single tampered fact
        // is QUERY_ROOT_MISMATCH rather than a committed answer.
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        input.verify().unwrap();
        // The bound ObjectId answers class 2: same key, another value.
        let mut tampered_bindings = owned.bindings.clone();
        tampered_bindings[0].1 = ObjectId::from_bytes([0xEE; 32]);
        let mut tampered = input;
        tampered.bindings = &tampered_bindings;
        assert_eq!(
            tampered.verify().unwrap_err().code(),
            RootQueryErrorCode::RootMismatch
        );
        // facts.entry_points answers class 10.
        let mut tampered_entry_points = owned.fixture.entry_points.clone();
        tampered_entry_points[0] = id(0x06);
        let mut tampered = input;
        tampered.facts.entry_points = &tampered_entry_points;
        assert_eq!(
            tampered.verify().unwrap_err().code(),
            RootQueryErrorCode::RootMismatch
        );
        // facts.dependency_roots answers class 11.
        let tampered_roots = vec![StateRoot::from_bytes([0x98; 32])];
        let mut tampered = input;
        tampered.facts.dependency_roots = &tampered_roots;
        assert_eq!(
            tampered.verify().unwrap_err().code(),
            RootQueryErrorCode::RootMismatch
        );
        // The three roots answer class 1.
        let mut tampered = input;
        tampered.contract_root = ObjectId::from_bytes([0xC1; 32]);
        assert_eq!(
            tampered.verify().unwrap_err().code(),
            RootQueryErrorCode::RootMismatch
        );
        let mut tampered = input;
        tampered.test_root = ObjectId::from_bytes([0xD1; 32]);
        assert_eq!(
            tampered.verify().unwrap_err().code(),
            RootQueryErrorCode::RootMismatch
        );
        let mut tampered = input;
        tampered.policy_root = PolicyRootId::from_bytes([0xE1; 32]);
        assert_eq!(
            tampered.verify().unwrap_err().code(),
            RootQueryErrorCode::RootMismatch
        );
        // The interpretation flags ride the same commitment.
        let tampered_flags = vec![1_u32];
        let mut tampered = input;
        tampered.interpretation_flags = &tampered_flags;
        assert_eq!(
            tampered.verify().unwrap_err().code(),
            RootQueryErrorCode::RootMismatch
        );
    }

    #[test]
    fn unsorted_root_committed_fact_sets_are_root_mismatch() {
        // Vulcan P1 (facts ordering): the recompute binds order only
        // implicitly, so verify() explicitly requires the root-committed
        // fact sets in canonical (strictly increasing) order. An
        // unsorted-but-self-consistent pair must stop here, before
        // classes 10/11 can emit verbatim out-of-order pages that the
        // cursor predicate silently truncates.
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        input.verify().unwrap();
        // entry_points out of order: the single committed fact followed by
        // a smaller one is a descending pair, never canonical order.
        let mut unsorted_entry_points = owned.fixture.entry_points.clone();
        unsorted_entry_points.push(id(0x09));
        assert!(!strictly_increasing(&unsorted_entry_points));
        let mut unsorted = input;
        unsorted.facts.entry_points = &unsorted_entry_points;
        assert_eq!(
            unsorted.verify().unwrap_err().code(),
            RootQueryErrorCode::RootMismatch
        );
        // dependency_roots out of order: a duplicated committed root is
        // never strictly increasing.
        let mut unsorted_roots = owned.fixture.dependency_roots.clone();
        unsorted_roots.push(unsorted_roots[0]);
        assert!(!strictly_increasing(&unsorted_roots));
        let mut unsorted = input;
        unsorted.facts.dependency_roots = &unsorted_roots;
        assert_eq!(
            unsorted.verify().unwrap_err().code(),
            RootQueryErrorCode::RootMismatch
        );
    }

    #[test]
    fn class_kind_applicability_table_is_executed() {
        // The unit matrix pins the section 2 table: only classes 6, 7, 8,
        // and 18 restrict kinds, and unknown tags admit nothing.
        for tag in 0..=20_u32 {
            for kind_tag in 1..=18_u32 {
                let kind = ModeledEntityKind::from_ssmc_tag(kind_tag).unwrap();
                let expected = match tag {
                    6 | 7 => kind == ModeledEntityKind::Package,
                    8 => kind == ModeledEntityKind::Namespace,
                    18 => matches!(
                        kind,
                        ModeledEntityKind::Function
                            | ModeledEntityKind::AdapterImport
                            | ModeledEntityKind::CapabilityRequirement
                    ),
                    1 | 2 | 3 | 4 | 5 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 19 => true,
                    _ => false,
                };
                assert_eq!(
                    class_applies_to(tag, kind),
                    expected,
                    "tag {tag} kind {kind:?}"
                );
            }
        }
        // End to end over the fixture, which covers all eighteen kinds:
        // every-kind classes succeed for every entity, restricted classes
        // fail `ClassNotApplicable` exactly off their table row, and an
        // absent entity is still `UnresolvedEntity` (precedence 7 before 8).
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let limits = QueryLimits::profile_maximum();
        let entities: Vec<EntityId> = borrowed
            .entities
            .iter()
            .map(|entry| entry.entity_id())
            .collect();
        assert_eq!(entities.len(), owned.snapshot.inventory().len());
        for entity in &entities {
            let kind = input.kind_of(*entity).unwrap();
            for query in [
                RootQuery::GetEntity { entity: *entity },
                RootQuery::GetSemanticFingerprint { entity: *entity },
                RootQuery::ListOwningNamespaces { entity: *entity },
                RootQuery::ListDirectDependencies {
                    entity: *entity,
                    kinds: vec![ImpactKind::Ownership],
                },
                RootQuery::ListDirectDependents {
                    entity: *entity,
                    kinds: vec![ImpactKind::Ownership],
                },
                RootQuery::ReverseImpactClosure {
                    seeds: vec![*entity],
                },
                RootQuery::ForwardDependencyClosure {
                    seeds: vec![*entity],
                },
                RootQuery::ListContractsFor { target: *entity },
                RootQuery::ListTestsFor { target: *entity },
                RootQuery::ListCapabilityRequirementsFor { subject: *entity },
            ] {
                assert!(
                    execute_root_query(
                        &input,
                        &build_root_query_request(&input, query.clone(), limits, false, None)
                            .unwrap()
                    )
                    .is_ok(),
                    "{query:?} applies to every kind, including {kind:?}"
                );
            }
            let package_only = [
                RootQuery::ListPackageExports { package: *entity },
                RootQuery::ListPackageDependencies { package: *entity },
            ];
            for query in package_only {
                if kind == ModeledEntityKind::Package {
                    assert_eq!(run(&input, query.clone()).class_tag(), query.tag());
                } else {
                    assert_eq!(
                        run_err(&input, query.clone(), limits, false, None),
                        RootQueryErrorCode::ClassNotApplicable,
                        "{query:?} on {kind:?}"
                    );
                }
            }
            if kind == ModeledEntityKind::Namespace {
                assert_eq!(
                    run(
                        &input,
                        RootQuery::ListNamespaceMembers { namespace: *entity }
                    )
                    .class_tag(),
                    8
                );
            } else {
                assert_eq!(
                    run_err(
                        &input,
                        RootQuery::ListNamespaceMembers { namespace: *entity },
                        limits,
                        false,
                        None
                    ),
                    RootQueryErrorCode::ClassNotApplicable,
                    "class 8 on {kind:?}"
                );
            }
            let effects_admitted = matches!(
                kind,
                ModeledEntityKind::Function
                    | ModeledEntityKind::AdapterImport
                    | ModeledEntityKind::CapabilityRequirement
            );
            if effects_admitted {
                assert_eq!(
                    run(&input, RootQuery::ListDeclaredEffects { entity: *entity }).class_tag(),
                    18
                );
            } else {
                assert_eq!(
                    run_err(
                        &input,
                        RootQuery::ListDeclaredEffects { entity: *entity },
                        limits,
                        false,
                        None
                    ),
                    RootQueryErrorCode::ClassNotApplicable,
                    "class 18 on {kind:?}"
                );
            }
        }
        assert_eq!(
            run_err(
                &input,
                RootQuery::ListPackageExports { package: id(0x99) },
                limits,
                false,
                None
            ),
            RootQueryErrorCode::UnresolvedEntity
        );
        assert_eq!(
            run_err(
                &input,
                RootQuery::ListDeclaredEffects { entity: id(0x99) },
                limits,
                false,
                None
            ),
            RootQueryErrorCode::UnresolvedEntity
        );
    }

    #[test]
    fn charged_work_follows_the_section_4_schedule() {
        // Pins the exact section 4 schedule over the fixture (19 inventory
        // entries, 41 snapshot edges): traversal work per class plus the
        // exact response record bytes. Any charge or layout drift fails
        // here before it can enter a bound SLEYRQR1 record.
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let expected_traversal = [
            19_u64, 3, 2, 19, 19, 3, 2, 10, 7, 1, 1, 41, 41, 47, 11, 19, 19, 2, 19,
        ];
        let expected_charged = [
            659_u64, 355, 318, 403, 339, 355, 418, 622, 359, 357, 321, 533, 873, 783, 427, 339,
            339, 322, 339,
        ];
        assert_eq!(all_classes().len(), expected_traversal.len());
        for (index, query) in all_classes().into_iter().enumerate() {
            let response = run(&input, query);
            assert_eq!(
                response.charged_work() - response.response_bytes(),
                expected_traversal[index],
                "class {} traversal",
                index + 1
            );
            assert_eq!(
                response.charged_work(),
                expected_charged[index],
                "class {} charged",
                index + 1
            );
        }
        // Nabu's remaining schedule example: class 9 skips the entity
        // scan for a namespace subject and charges only the chain links
        // above it. The package root namespace has no parent, so its
        // traversal is zero; the child namespace follows one link.
        for (subject, expected) in [(id(0x04), 0_u64), (id(0x05), 1_u64)] {
            let response = run(&input, RootQuery::ListOwningNamespaces { entity: subject });
            assert_eq!(
                response.charged_work() - response.response_bytes(),
                expected,
                "class 9 traversal for namespace subject {subject:?}"
            );
        }
    }
    fn hex(bytes: &[u8]) -> String {
        use core::fmt::Write as _;
        bytes.iter().fold(String::new(), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
    }

    fn json_ids(ids: &[EntityId]) -> String {
        let items: Vec<String> = ids
            .iter()
            .map(|id| format!("\"{}\"", hex(id.as_bytes())))
            .collect();
        format!("[{}]", items.join(","))
    }

    fn describe(query: &RootQuery) -> String {
        let tag = query.tag();
        match query {
            RootQuery::GetRootSummary
            | RootQuery::ListWorkspacePackages
            | RootQuery::ListEntryPoints
            | RootQuery::ListDependencyRoots => format!("{{\"class\":{tag}}}"),
            RootQuery::GetEntity { entity }
            | RootQuery::GetSemanticFingerprint { entity }
            | RootQuery::ListOwningNamespaces { entity }
            | RootQuery::ListDeclaredEffects { entity } => {
                format!(
                    "{{\"class\":{tag},\"entity\":\"{}\"}}",
                    hex(entity.as_bytes())
                )
            }
            RootQuery::ListPackageExports { package }
            | RootQuery::ListPackageDependencies { package } => {
                format!(
                    "{{\"class\":{tag},\"entity\":\"{}\"}}",
                    hex(package.as_bytes())
                )
            }
            RootQuery::ListNamespaceMembers { namespace } => {
                format!(
                    "{{\"class\":{tag},\"entity\":\"{}\"}}",
                    hex(namespace.as_bytes())
                )
            }
            RootQuery::ListContractsFor { target } | RootQuery::ListTestsFor { target } => {
                format!(
                    "{{\"class\":{tag},\"entity\":\"{}\"}}",
                    hex(target.as_bytes())
                )
            }
            RootQuery::ListCapabilityRequirementsFor { subject } => {
                format!(
                    "{{\"class\":{tag},\"entity\":\"{}\"}}",
                    hex(subject.as_bytes())
                )
            }
            RootQuery::ListEntitiesByKind { kind } => {
                format!("{{\"class\":{tag},\"kind\":{}}}", kind.tag())
            }
            RootQuery::ListDirectDependencies { entity, kinds }
            | RootQuery::ListDirectDependents { entity, kinds } => {
                let tags: Vec<String> = kinds.iter().map(|kind| kind.tag().to_string()).collect();
                format!(
                    "{{\"class\":{tag},\"entity\":\"{}\",\"kinds\":[{}]}}",
                    hex(entity.as_bytes()),
                    tags.join(",")
                )
            }
            RootQuery::ReverseImpactClosure { seeds }
            | RootQuery::ForwardDependencyClosure { seeds } => {
                format!("{{\"class\":{tag},\"seeds\":{}}}", json_ids(seeds))
            }
        }
    }

    fn describe_cursor(cursor: Option<Cursor>) -> String {
        match cursor {
            None => "null".to_string(),
            Some(Cursor::Entity(entity)) => {
                format!("{{\"tag\":1,\"entity\":\"{}\"}}", hex(entity.as_bytes()))
            }
            Some(Cursor::Root(root)) => {
                format!("{{\"tag\":3,\"root\":\"{}\"}}", hex(root.as_bytes()))
            }
            Some(Cursor::Edge(edge)) => format!(
                "{{\"tag\":2,\"dependent\":\"{}\",\"dependency\":\"{}\",\"kind\":{}}}",
                hex(edge.dependent.as_bytes()),
                hex(edge.dependency.as_bytes()),
                edge.kind.tag()
            ),
        }
    }

    fn describe_limits(limits: QueryLimits) -> String {
        format!(
            "{{\"max_returned_entities\":{},\"max_returned_edges\":{},\"max_depth\":{},\"max_response_bytes\":{},\"max_work\":{}}}",
            limits.max_returned_entities,
            limits.max_returned_edges,
            limits.max_depth,
            limits.max_response_bytes,
            limits.max_work
        )
    }

    fn emit(
        input: &RootQueryInput<'_>,
        label: &str,
        query: RootQuery,
        limits: QueryLimits,
        allow: bool,
        after: Option<Cursor>,
    ) -> RootQueryResponse {
        let request = build_root_query_request(input, query.clone(), limits, allow, after).unwrap();
        let response = execute_root_query(input, &request).unwrap();
        println!(
            "ROOT_QUERY_VECTOR|{label}|{}|{}|{}|{}|{}|{}|{}",
            describe(&query),
            describe_limits(limits),
            allow,
            describe_cursor(after),
            hex(request.query_id().as_bytes()),
            hex(response.record()),
            describe_cursor(response.next_after())
        );
        response
    }

    /// Emits the frozen root-backed query vectors for
    /// `scripts/generate_root_backed_query_fixtures.py`.
    #[test]
    #[ignore = "fixture refresh emitter; run through the generator script"]
    fn emit_root_query_vectors_for_fixture_refresh() {
        let owned = Owned::new();
        let borrowed = Borrowed::new(&owned);
        let input = borrowed.input();
        let bindings: Vec<String> = owned
            .bindings
            .iter()
            .map(|(entity, object)| {
                format!(
                    "[\"{}\",\"{}\"]",
                    hex(entity.as_bytes()),
                    hex(object.as_bytes())
                )
            })
            .collect();
        let fingerprints: Vec<String> = owned
            .fingerprints
            .iter()
            .map(|(entity, fingerprint)| {
                format!(
                    "[\"{}\",\"{}\"]",
                    hex(entity.as_bytes()),
                    hex(fingerprint.as_bytes())
                )
            })
            .collect();
        let flags: Vec<String> = owned
            .interpretation_flags
            .iter()
            .map(ToString::to_string)
            .collect();
        println!(
            "ROOT_QUERY_CONTEXT|{}|{}|{}|{}|{}|{}|{}|[{}]|[{}]|[{}]",
            hex(owned.snapshot.snapshot_id().as_bytes()),
            hex(root().as_bytes()),
            hex(epoch().as_bytes()),
            hex(workspace().as_bytes()),
            hex(input.contract_root.as_bytes()),
            hex(input.test_root.as_bytes()),
            hex(input.policy_root.as_bytes()),
            bindings.join(","),
            fingerprints.join(","),
            flags.join(",")
        );
        let full = QueryLimits::profile_maximum();
        for (index, query) in all_classes().into_iter().enumerate() {
            emit(
                &input,
                &format!("class-{:02}", index + 1),
                query,
                full,
                false,
                None,
            );
        }
        let paged = QueryLimits {
            max_returned_entities: 2,
            max_returned_edges: 3,
            ..full
        };
        let namespaces = RootQuery::ListEntitiesByKind {
            kind: ModeledEntityKind::Namespace,
        };
        let first = emit(
            &input,
            "page-namespaces-1",
            namespaces.clone(),
            paged,
            true,
            None,
        );
        emit(
            &input,
            "page-namespaces-2",
            namespaces,
            paged,
            true,
            first.next_after(),
        );
        let edges = RootQuery::ListDirectDependencies {
            entity: id(0x01),
            kinds: vec![
                ImpactKind::Ownership,
                ImpactKind::Capability,
                ImpactKind::Contract,
                ImpactKind::TestTarget,
            ],
        };
        let first = emit(&input, "page-edges-1", edges.clone(), paged, true, None);
        emit(
            &input,
            "page-edges-2",
            edges,
            paged,
            true,
            first.next_after(),
        );
        // P1-6: every paged class walks. The frozen fixture carries one
        // dependency root and one entry point, so each walk is a full page
        // plus the after-cursor page whose union is the complete result;
        // the second pages pin the tag-3 Root cursor and the entity cursor.
        let only_root = borrowed.input().facts.dependency_roots[0];
        emit(
            &input,
            "page-roots-1",
            RootQuery::ListDependencyRoots,
            paged,
            true,
            None,
        );
        emit(
            &input,
            "page-roots-2",
            RootQuery::ListDependencyRoots,
            paged,
            true,
            Some(Cursor::Root(only_root)),
        );
        let only_entry = borrowed.input().facts.entry_points[0];
        emit(
            &input,
            "page-entry-points-1",
            RootQuery::ListEntryPoints,
            paged,
            true,
            None,
        );
        emit(
            &input,
            "page-entry-points-2",
            RootQuery::ListEntryPoints,
            paged,
            true,
            Some(Cursor::Entity(only_entry)),
        );
        let rejections: Vec<(&str, RootQuery, QueryLimits, bool, Option<Cursor>)> = vec![
            (
                "truncated-without-continuation",
                RootQuery::ListEntitiesByKind {
                    kind: ModeledEntityKind::Namespace,
                },
                paged,
                false,
                None,
            ),
            (
                "cursor-wrong-type",
                RootQuery::ListEntitiesByKind {
                    kind: ModeledEntityKind::Namespace,
                },
                paged,
                true,
                Some(Cursor::Root(root())),
            ),
            (
                "cursor-on-single-key",
                RootQuery::GetEntity { entity: id(0x07) },
                paged,
                true,
                Some(Cursor::Entity(id(0x01))),
            ),
            (
                "class-not-applicable",
                RootQuery::ListPackageExports { package: id(0x04) },
                full,
                false,
                None,
            ),
            (
                "unresolved-entity",
                RootQuery::GetEntity { entity: id(0x99) },
                full,
                false,
                None,
            ),
            (
                "filter-not-canonical",
                RootQuery::ListDirectDependents {
                    entity: id(0x07),
                    kinds: vec![ImpactKind::Contract, ImpactKind::Ownership],
                },
                full,
                false,
                None,
            ),
            (
                "depth-cut",
                RootQuery::ReverseImpactClosure {
                    seeds: vec![id(0x09)],
                },
                QueryLimits {
                    max_depth: 0,
                    ..full
                },
                true,
                None,
            ),
            (
                "work-exhausted",
                RootQuery::GetRootSummary,
                QueryLimits {
                    max_work: 0,
                    ..full
                },
                false,
                None,
            ),
        ];
        for (label, query, limits, allow, after) in rejections {
            let code = match build_root_query_request(&input, query.clone(), limits, allow, after) {
                Ok(request) => execute_root_query(&input, &request).unwrap_err().code(),
                Err(error) => error.code(),
            };
            println!(
                "ROOT_QUERY_REJECT|{label}|{}|{}|{}|{}|{}|{}",
                describe(&query),
                describe_limits(limits),
                allow,
                describe_cursor(after),
                code.as_str(),
                code.numeric()
            );
        }
        // P2-7: the binding-failure matrix pins the headline binding
        // failure, not just the shape rejections above. A substituted root
        // no longer recomputes, so verification fails QUERY_ROOT_MISMATCH
        // (31008) rather than entering an answer.
        let mut tampered = borrowed.input();
        tampered.root = StateRoot::from_bytes([0x09; 32]);
        // Binding is verified at build as well as at execute: either
        // stage must refuse the substituted fact with the same code.
        let tampered_code =
            match build_root_query_request(&tampered, RootQuery::GetRootSummary, full, false, None)
            {
                Ok(tampered_request) => execute_root_query(&tampered, &tampered_request)
                    .unwrap_err()
                    .code(),
                Err(error) => error.code(),
            };
        assert_eq!(tampered_code.numeric(), 31_008);
        println!(
            "ROOT_QUERY_REJECT|binding-substituted-fact|{}|{}|false|{}|{}|{}|{}",
            describe(&RootQuery::GetRootSummary),
            describe_limits(full),
            describe_cursor(None),
            tampered_code.as_str(),
            tampered_code.numeric(),
            describe_input_context(&tampered)
        );
        // P2-7 and section 9: the restricted arm-1 snapshot is not a
        // binding failure; the arm selects the profile first, so the
        // engine answers QUERY_PROFILE_UNSUPPORTED (31000).
        let restricted: Vec<ImpactEntity<'_>> = borrowed
            .entities
            .iter()
            .filter(|entity| entity.kind().restricted_kind())
            .copied()
            .collect();
        let arm1 = build_index_snapshot(
            SnapshotContext {
                schema_epoch: epoch(),
                claimed_root_context: Some(root()),
            },
            &restricted,
        )
        .expect("fixture restricted-kind subset builds the arm-1 snapshot");
        let mut arm1_input = borrowed.input();
        arm1_input.snapshot = &arm1;
        // Like the binding case, the arm is refused at build as well as
        // at execute: either stage must answer QUERY_PROFILE_UNSUPPORTED.
        let arm1_code = match build_root_query_request(
            &arm1_input,
            RootQuery::GetRootSummary,
            full,
            false,
            None,
        ) {
            Ok(arm1_request) => execute_root_query(&arm1_input, &arm1_request)
                .unwrap_err()
                .code(),
            Err(error) => error.code(),
        };
        assert_eq!(arm1_code.numeric(), 31_000);
        println!(
            "ROOT_QUERY_REJECT|arm-1-snapshot-profile|{}|{}|false|{}|{}|{}|{}",
            describe(&RootQuery::GetRootSummary),
            describe_limits(full),
            describe_cursor(None),
            arm1_code.as_str(),
            arm1_code.numeric(),
            describe_input_context(&arm1_input)
        );
    }

    fn describe_input_context(input: &RootQueryInput<'_>) -> String {
        format!(
            "{{\"root_hex\":\"{}\",\"snapshot_id\":\"{}\",\"snapshot_record_hex\":\"{}\"}}",
            hex(input.root.as_bytes()),
            hex(input.snapshot.snapshot_id().as_bytes()),
            hex(input.snapshot.record())
        )
    }
}
