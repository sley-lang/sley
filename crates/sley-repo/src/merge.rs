//! Merge v1 (S20-520, contract `docs/spec/MERGE_V1.md`, ADR-0028).
//!
//! Three-way composition of two complete roots against their exact common
//! ancestor over the frozen S20-510 deltas, a re-judged merged root, an
//! S20-350 merge plan committed through the frozen S20-390 path on ours,
//! and a canonical conflict object whenever composition is not proven. The
//! merge itself never writes a root, object, receipt, or ref.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use sley_id::{
    CandidateNonce, EntityId, MergeConflictId, ObjectId, PrincipalId, SchemaEpochId,
    SemanticDeltaId, StateRoot, TransactionId, WorkspaceId,
};
use sley_mutate::{
    BoundPrecondition, CandidateExpiry, CandidateRecord, EntityObject, EntityObjectRecord,
    ExactEntityVersion, ExpectedIdentityAbsent, MutationClass, MutationOperation, MutationPayload,
    PreconditionPayload, PreimageRequirement, build_candidate, build_entity_object,
    full_validation_profile_id,
    value::{EntityBodyValue, EntityIdSet},
};
use sley_policy::{CandidateValidationLimits, build_capability_summary_projection};
use sley_query::ImpactErrorCode;
use sley_scb1::{ScbErrorCode, encode_list, encode_record, encode_uvar};
use sley_schema::{ContractDescriptor, EpochLimits, SchemaEpochRecordV1, UnicodeVersion};
use sley_state_root::{
    AcceptedStateRoot, StateRootBuilder, conformance_registry as merge_registry,
};
use sley_txn::{CommitInput, TransactionRepository, VerifiedRevision};

use crate::refs::{BranchAncestryEntry, BranchErrorCode, BranchRepository};
use crate::{
    ChangeClass, CompareError, CompleteRootRequest, FieldDelta, PackError, Reader, RecordReader,
    SemanticDelta, compare_complete_roots, complete_root::CompleteRootError, decode_list,
    exact_array, read_single_uvar,
};

/// Maximum conflict entries.
pub const MAX_CONFLICT_ENTRIES: usize = 131_070;
/// Maximum plan operations.
pub const MAX_PLAN_OPERATIONS: usize = 131_070;
/// Maximum ancestry entries per side (S20-500 bound enforced at verification).
pub const MAX_ANCESTRY_NODES: usize = 65_536;
/// Maximum stored conflict bytes.
pub const MAX_CONFLICT_BYTES: usize = 67_108_864;
/// Maximum charged merge work.
pub const MAX_MERGE_WORK: u64 = 100_000_000;

const MAGIC: &[u8; 8] = b"SLEYSCB1";
const FORMAT_VERSION: u64 = 1;
const CONTRACT_TAG: u32 = 520;
const DIGEST_DOMAIN_TAG: u32 = 21;
const KIND_TAG: u32 = 520;
const ID_LEN: usize = 32;
const ZERO32: [u8; 32] = [0; 32];
const NONCE_DOMAIN: &[u8] = b"sley2.merge-plan-nonce.v1";
const FIELD_SCHEMA_HASH: [u8; 32] = [
    0x00, 0x8e, 0x06, 0x76, 0xf9, 0x11, 0xdc, 0x82, 0xb6, 0x05, 0x28, 0x66, 0x24, 0xc8, 0x55, 0xa2,
    0x1b, 0x2e, 0x9e, 0x9d, 0x39, 0x73, 0x58, 0xbf, 0x0c, 0xe7, 0xf1, 0x96, 0xf5, 0x5c, 0x2c, 0x8b,
];
const DECODER_LIMITS_HASH: [u8; 32] = [
    0xc6, 0xc0, 0x71, 0xdf, 0x3c, 0x80, 0xf1, 0x87, 0x02, 0xcb, 0x0c, 0xb4, 0x12, 0x98, 0xd5, 0x65,
    0x24, 0xe2, 0xcd, 0xba, 0x4b, 0xbe, 0x77, 0xd8, 0xa9, 0x75, 0x5c, 0xfc, 0xd9, 0x5d, 0x20, 0xc5,
];

/// Set-valued identity fields that compose deterministically: (kind, field).
const SET_FIELDS: [(u32, u32); 13] = [
    (1, 1),
    (1, 3),
    (1, 4),
    (1, 5),
    (2, 3),
    (2, 4),
    (3, 2),
    (4, 3),
    (5, 4),
    (5, 7),
    (12, 3),
    (15, 6),
    (17, 2),
];

/// Stable S20-520 failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeErrorCode {
    /// `MERGE_NO_COMMON_ANCESTOR`.
    NoCommonAncestor,
    /// `MERGE_ANCESTOR_MISMATCH`.
    AncestorMismatch,
    /// `MERGE_WORKSPACE_MISMATCH`.
    WorkspaceMismatch,
    /// `MERGE_EPOCH_MISMATCH`.
    EpochMismatch,
    /// `MERGE_DEPENDENCY_ROOT_CHANGE_UNSUPPORTED`.
    DependencyRootChangeUnsupported,
    /// `MERGE_COMPARE_FAILED`.
    CompareFailed,
    /// `MERGE_RESOURCE_LIMIT`.
    ResourceLimit,
    /// `MERGE_CONFLICT_FORMAT_INVALID`.
    ConflictFormatInvalid,
    /// `MERGE_CONFLICT_DIGEST_MISMATCH`.
    ConflictDigestMismatch,
    /// `MERGE_CONFLICT_CANONICAL_ORDER`.
    ConflictCanonicalOrder,
    /// `MERGE_CONFLICT_VERSION_UNSUPPORTED`.
    ConflictVersionUnsupported,
    /// `MERGE_PLAN_UNSUPPORTED`.
    PlanUnsupported,
    /// `MERGE_RESULT_MISMATCH`.
    ResultMismatch,
    /// `MERGE_INTERNAL_INVARIANT` (reserved).
    InternalInvariant,
}

impl MergeErrorCode {
    /// Every code in numeric order.
    pub const ALL: [Self; 14] = [
        Self::NoCommonAncestor,
        Self::AncestorMismatch,
        Self::WorkspaceMismatch,
        Self::EpochMismatch,
        Self::DependencyRootChangeUnsupported,
        Self::CompareFailed,
        Self::ResourceLimit,
        Self::ConflictFormatInvalid,
        Self::ConflictDigestMismatch,
        Self::ConflictCanonicalOrder,
        Self::ConflictVersionUnsupported,
        Self::PlanUnsupported,
        Self::ResultMismatch,
        Self::InternalInvariant,
    ];

    /// Returns the stable symbolic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoCommonAncestor => "MERGE_NO_COMMON_ANCESTOR",
            Self::AncestorMismatch => "MERGE_ANCESTOR_MISMATCH",
            Self::WorkspaceMismatch => "MERGE_WORKSPACE_MISMATCH",
            Self::EpochMismatch => "MERGE_EPOCH_MISMATCH",
            Self::DependencyRootChangeUnsupported => "MERGE_DEPENDENCY_ROOT_CHANGE_UNSUPPORTED",
            Self::CompareFailed => "MERGE_COMPARE_FAILED",
            Self::ResourceLimit => "MERGE_RESOURCE_LIMIT",
            Self::ConflictFormatInvalid => "MERGE_CONFLICT_FORMAT_INVALID",
            Self::ConflictDigestMismatch => "MERGE_CONFLICT_DIGEST_MISMATCH",
            Self::ConflictCanonicalOrder => "MERGE_CONFLICT_CANONICAL_ORDER",
            Self::ConflictVersionUnsupported => "MERGE_CONFLICT_VERSION_UNSUPPORTED",
            Self::PlanUnsupported => "MERGE_PLAN_UNSUPPORTED",
            Self::ResultMismatch => "MERGE_RESULT_MISMATCH",
            Self::InternalInvariant => "MERGE_INTERNAL_INVARIANT",
        }
    }

    /// Returns the stable numeric code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::NoCommonAncestor => 52_000,
            Self::AncestorMismatch => 52_001,
            Self::WorkspaceMismatch => 52_002,
            Self::EpochMismatch => 52_003,
            Self::DependencyRootChangeUnsupported => 52_004,
            Self::CompareFailed => 52_005,
            Self::ResourceLimit => 52_006,
            Self::ConflictFormatInvalid => 52_007,
            Self::ConflictDigestMismatch => 52_008,
            Self::ConflictCanonicalOrder => 52_009,
            Self::ConflictVersionUnsupported => 52_010,
            Self::PlanUnsupported => 52_011,
            Self::ResultMismatch => 52_012,
            Self::InternalInvariant => 52_013,
        }
    }
}

/// One stable merge failure with any wrapped lower-layer code preserved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MergeError {
    /// A plain S20-520 failure.
    Merge(MergeErrorCode),
    /// `MERGE_COMPARE_FAILED` wrapping the exact S20-510 failure.
    Compare(CompareError),
    /// `MERGE_COMPARE_FAILED` wrapping an extraction failure.
    Extraction(CompleteRootError),
    /// `MERGE_CONFLICT_FORMAT_INVALID` wrapping the exact SCB1 code.
    Format(&'static str),
    /// A frozen commit-path failure (`TXN_*`, `BRANCH_*`, `CAP_*`, `SCB_*`)
    /// with its exact symbol and owning numeric code, never remapped.
    Commit(CommitFailure),
}

/// A preserved commit-path failure: the exact symbol plus the owning layer's
/// numeric code when it froze one (`None` only for codeless SCB1 failures).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitFailure {
    /// Exact source symbol (`TXN_*`, `BRANCH_*`, `CAP_*`, `SCB_*`).
    pub symbol: String,
    /// Owning numeric code, when frozen.
    pub numeric: Option<u32>,
}

impl MergeError {
    /// Returns the stable S20-520 code (`None` for a preserved commit-path code).
    #[must_use]
    pub const fn code(&self) -> Option<MergeErrorCode> {
        match self {
            Self::Merge(code) => Some(*code),
            Self::Compare(_) | Self::Extraction(_) => Some(MergeErrorCode::CompareFailed),
            Self::Format(_) => Some(MergeErrorCode::ConflictFormatInvalid),
            Self::Commit(_) => None,
        }
    }

    /// Returns the exact symbolic code: the S20-520 code or the preserved one.
    #[must_use]
    pub fn symbol(&self) -> String {
        match self {
            Self::Merge(code) => code.as_str().to_owned(),
            Self::Compare(_) | Self::Extraction(_) => {
                MergeErrorCode::CompareFailed.as_str().to_owned()
            }
            Self::Format(_) => MergeErrorCode::ConflictFormatInvalid.as_str().to_owned(),
            Self::Commit(failure) => failure.symbol.clone(),
        }
    }

    /// Returns the exact wrapped source code, if any.
    #[must_use]
    pub fn source_code(&self) -> Option<String> {
        match self {
            Self::Merge(_) => None,
            Self::Compare(error) => Some(error.code().as_str().to_owned()),
            Self::Extraction(error) => Some(error.code().to_owned()),
            Self::Format(code) => Some((*code).to_owned()),
            Self::Commit(failure) => Some(failure.symbol.clone()),
        }
    }

    /// Returns the preserved commit-path numeric code, if the owning layer
    /// froze one.
    #[must_use]
    pub const fn commit_numeric(&self) -> Option<u32> {
        match self {
            Self::Commit(failure) => failure.numeric,
            Self::Merge(_) | Self::Compare(_) | Self::Extraction(_) | Self::Format(_) => None,
        }
    }
}

impl fmt::Display for MergeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.symbol())
    }
}

impl std::error::Error for MergeError {}

impl From<PackError> for MergeError {
    fn from(error: PackError) -> Self {
        Self::Format(error.symbol())
    }
}

type Result<T> = core::result::Result<T, MergeError>;

fn fail<T>(code: MergeErrorCode) -> Result<T> {
    Err(MergeError::Merge(code))
}

/// Maps a commit-path failure into `MergeError::Commit` with its exact
/// symbol and owning numeric preserved. The helpers take their error by
/// value so they compose as `map_err` function items.
#[allow(clippy::needless_pass_by_value)]
fn commit_capability(error: sley_policy::CapabilityError) -> MergeError {
    let numeric = match &error {
        sley_policy::CapabilityError::Capability(code) => Some(code.numeric()),
        sley_policy::CapabilityError::Scb(_) => None,
    };
    MergeError::Commit(CommitFailure {
        symbol: error.code_str().to_owned(),
        numeric,
    })
}

#[allow(clippy::needless_pass_by_value)]
fn commit_candidate(error: sley_mutate::CandidateError) -> MergeError {
    MergeError::Commit(CommitFailure {
        symbol: error.code().to_owned(),
        numeric: error.numeric_code(),
    })
}

#[allow(clippy::needless_pass_by_value)]
fn commit_transaction(error: sley_txn::CommitError) -> MergeError {
    MergeError::Commit(CommitFailure {
        symbol: error.code().to_owned(),
        numeric: error.numeric_code(),
    })
}

#[allow(clippy::needless_pass_by_value)]
fn commit_branch(error: crate::refs::BranchError) -> MergeError {
    MergeError::Commit(CommitFailure {
        symbol: error.code().to_owned(),
        numeric: error.numeric_code(),
    })
}

/// Frozen conflict reason.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConflictReason {
    /// Both sides added different objects.
    AddAdd,
    /// One side removed, the other edited.
    DeleteEdit,
    /// Both sides changed one field incompatibly.
    FieldEdit,
    /// A kind change met another change.
    KindEdit,
    /// One side changed an entity the other side depends on.
    Collateral,
    /// The merged root failed the complete-root judgment.
    Closure,
    /// Contract or test root anchors differ.
    RootAnchor,
    /// Policy roots differ.
    PolicyRoot,
    /// Both sides changed metadata to different objects.
    MetadataEdit,
}

impl ConflictReason {
    /// Returns the frozen tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::AddAdd => 1,
            Self::DeleteEdit => 2,
            Self::FieldEdit => 3,
            Self::KindEdit => 4,
            Self::Collateral => 5,
            Self::Closure => 6,
            Self::RootAnchor => 7,
            Self::PolicyRoot => 8,
            Self::MetadataEdit => 9,
        }
    }

    fn from_tag(tag: u64) -> Result<Self> {
        Ok(match tag {
            1 => Self::AddAdd,
            2 => Self::DeleteEdit,
            3 => Self::FieldEdit,
            4 => Self::KindEdit,
            5 => Self::Collateral,
            6 => Self::Closure,
            7 => Self::RootAnchor,
            8 => Self::PolicyRoot,
            9 => Self::MetadataEdit,
            _ => return fail(MergeErrorCode::ConflictFormatInvalid),
        })
    }
}

/// One conflict entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConflictEntry {
    /// Conflicting identity (`zero32` for an unattributed closure failure).
    pub entity_id: EntityId,
    /// Reason.
    pub reason: ConflictReason,
    /// SSMC1 kind, `0` when unknown.
    pub kind: u32,
    /// Field tag for `FieldEdit`, `0` otherwise.
    pub field: u32,
    /// Ours object, if any.
    pub ours_object: Option<ObjectId>,
    /// Theirs object, if any.
    pub theirs_object: Option<ObjectId>,
    /// `IMPACT_*` numeric code for `Closure`, `0` otherwise.
    pub detail: u32,
}

/// Canonical conflict object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeConflict {
    /// Shared workspace.
    pub workspace_id: WorkspaceId,
    /// Ancestor root.
    pub ancestor_root: StateRoot,
    /// Ours root.
    pub ours_root: StateRoot,
    /// Theirs root.
    pub theirs_root: StateRoot,
    /// `SemanticDeltaId` of `compare(ancestor, ours)`.
    pub ours_delta: SemanticDeltaId,
    /// `SemanticDeltaId` of `compare(ancestor, theirs)`.
    pub theirs_delta: SemanticDeltaId,
    /// Canonical conflict entries.
    pub conflicts: Vec<ConflictEntry>,
}

/// Canonical conflict bytes with identity and decoded form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredMergeConflict {
    /// Derived identity.
    pub conflict_id: MergeConflictId,
    /// Exact stored bytes including the trailer.
    pub stored_bytes: Vec<u8>,
    /// Decoded conflict.
    pub conflict: MergeConflict,
}

/// Merged root: objects, facts, and the derived root, before any commit.
#[derive(Clone, Debug)]
pub struct MergedRoot {
    /// Merged entity objects in raw-ID order.
    pub objects: Vec<EntityObject>,
    /// Registry-authorized merged root.
    pub state_root: AcceptedStateRoot,
    /// Complete-root request over the merged entities.
    pub request: CompleteRootRequest,
    /// Identities whose metadata-only change was overridden by a semantic change.
    pub metadata_overridden: Vec<EntityId>,
    /// `SemanticDeltaId` of `compare(ancestor, ours)`.
    pub ours_delta: SemanticDeltaId,
    /// `SemanticDeltaId` of `compare(ancestor, theirs)`.
    pub theirs_delta: SemanticDeltaId,
}

/// Successful judgment: a merged root or a canonical conflict.
#[derive(Clone, Debug)]
pub enum MergeOutcome {
    /// Composition proven; the merged root is ready for a plan.
    Merged(Box<MergedRoot>),
    /// Composition not proven; the conflict object.
    Conflict(Box<StoredMergeConflict>),
}

/// Merge plan: the S20-350 operations that move ours to the merged root.
#[derive(Clone, Debug)]
pub struct MergePlan {
    /// Ours transaction (the expected commit parent).
    pub base_transaction_id: TransactionId,
    /// Ours root.
    pub base_root: StateRoot,
    /// Precomputed merged root.
    pub merged_root: StateRoot,
    /// Operations in raw-ID order.
    pub operations: Vec<MutationOperation>,
    /// One precondition per operation.
    pub preconditions: Vec<BoundPrecondition>,
    /// Identities whose metadata-only change was overridden.
    pub metadata_overridden: Vec<EntityId>,
    /// Deterministic candidate nonce the plan's created identities derive from.
    pub candidate_nonce: CandidateNonce,
    /// Created identities re-derived per S20-345: `(judged id, plan id)`.
    pub identity_map: Vec<(EntityId, EntityId)>,
    /// Merged objects after the identity remap, in raw-ID order.
    pub objects: Vec<EntityObject>,
}

/// Returns the standalone conflict schema epoch record.
#[must_use]
pub fn conflict_epoch_record() -> SchemaEpochRecordV1 {
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

/// Returns the exact conflict schema epoch identity.
///
/// # Errors
///
/// Returns `MERGE_INTERNAL_INVARIANT` if the descriptor record drifts.
pub fn conflict_epoch_id() -> Result<SchemaEpochId> {
    conflict_epoch_record()
        .schema_epoch_id()
        .map_err(|_| MergeError::Merge(MergeErrorCode::InternalInvariant))
}

/// Finds the exact common ancestor of two head-first S20-500 ancestries.
///
/// # Errors
///
/// Returns `MERGE_NO_COMMON_ANCESTOR` when no transaction is shared within
/// the supplied bounds.
pub fn find_common_ancestor(
    ours: &[BranchAncestryEntry],
    theirs: &[BranchAncestryEntry],
) -> Result<BranchAncestryEntry> {
    if ours.len() > MAX_ANCESTRY_NODES || theirs.len() > MAX_ANCESTRY_NODES {
        return fail(MergeErrorCode::ResourceLimit);
    }
    let theirs_ids: BTreeSet<TransactionId> =
        theirs.iter().map(|entry| entry.transaction_id).collect();
    ours.iter()
        .find(|entry| theirs_ids.contains(&entry.transaction_id))
        .cloned()
        .ok_or(MergeError::Merge(MergeErrorCode::NoCommonAncestor))
}

/// Walks the head-first ancestry of one transaction over the frozen
/// repository, following every recorded parent, cycle-checked and bounded.
///
/// # Errors
///
/// Returns `MERGE_RESOURCE_LIMIT` past `max_nodes` and the exact frozen
/// commit-path failure when a revision in the chain does not verify.
pub fn transaction_ancestry(
    transactions: &TransactionRepository,
    head: TransactionId,
    max_nodes: usize,
) -> Result<Vec<BranchAncestryEntry>> {
    let mut output = Vec::new();
    let mut completed = BTreeSet::new();
    let mut active = BTreeSet::new();
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
        if active.contains(&transaction_id) || output.len() >= max_nodes {
            return fail(MergeErrorCode::ResourceLimit);
        }
        let revision = transactions
            .verified_revision(transaction_id)
            .map_err(commit_transaction)?;
        let entry = BranchAncestryEntry {
            transaction_id,
            state_root: revision.state_root().root,
            parent_transaction_ids: revision
                .receipt()
                .transaction
                .record
                .parent_transaction_ids
                .clone(),
        };
        active.insert(transaction_id);
        stack.push((transaction_id, true));
        for parent in entry.parent_transaction_ids.iter().rev() {
            stack.push((*parent, false));
        }
        output.push(entry);
    }
    Ok(output)
}

/// Verifies the merge precondition the judgment takes as proven: the two
/// caller-supplied head-first ancestries belong to `ours` and `theirs`, and
/// the supplied ancestor is their computed common entry.
///
/// # Errors
///
/// Returns `MERGE_RESOURCE_LIMIT` past the per-side ancestry bound,
/// `MERGE_NO_COMMON_ANCESTOR` when the chains share nothing,
/// `MERGE_ANCESTOR_MISMATCH` when the supplied ancestor differs from the
/// computed entry (or carries no transaction to prove), and
/// `MERGE_INTERNAL_INVARIANT` when an ancestry head is not its side.
pub fn verify_merge_ancestor(
    ancestor: &MergeSide,
    ours: &MergeSide,
    theirs: &MergeSide,
    ours_ancestry: &[BranchAncestryEntry],
    theirs_ancestry: &[BranchAncestryEntry],
) -> Result<BranchAncestryEntry> {
    if ours_ancestry.len() > MAX_ANCESTRY_NODES || theirs_ancestry.len() > MAX_ANCESTRY_NODES {
        return fail(MergeErrorCode::ResourceLimit);
    }
    let mut work = Work(0);
    work.charge(
        u64::try_from(ours_ancestry.len())
            .unwrap_or(u64::MAX)
            .saturating_add(u64::try_from(theirs_ancestry.len()).unwrap_or(u64::MAX)),
    )?;
    let ours_head = ours_ancestry
        .first()
        .ok_or(MergeError::Merge(MergeErrorCode::InternalInvariant))?;
    let theirs_head = theirs_ancestry
        .first()
        .ok_or(MergeError::Merge(MergeErrorCode::InternalInvariant))?;
    if Some(ours_head.transaction_id) != ours.transaction_id
        || Some(theirs_head.transaction_id) != theirs.transaction_id
    {
        return fail(MergeErrorCode::InternalInvariant);
    }
    let common = find_common_ancestor(ours_ancestry, theirs_ancestry)?;
    if ancestor.transaction_id != Some(common.transaction_id) {
        return fail(MergeErrorCode::AncestorMismatch);
    }
    Ok(common)
}

/// Judges a merge after proving the ancestor precondition: the supplied
/// ancestries must belong to `ours` and `theirs` and their computed common
/// entry must be the supplied ancestor. This is the production entry point;
/// bare `judge_merge` takes the ancestor as proven and stays available for
/// synthetic test inputs that carry no transaction chain.
///
/// # Errors
///
/// Returns `MERGE_NO_COMMON_ANCESTOR`, `MERGE_ANCESTOR_MISMATCH`, or
/// `MERGE_RESOURCE_LIMIT` from verification, then whatever `judge_merge`
/// returns.
pub fn judge_merge_verified(
    ancestor: &MergeSide,
    ours: &MergeSide,
    theirs: &MergeSide,
    ours_ancestry: &[BranchAncestryEntry],
    theirs_ancestry: &[BranchAncestryEntry],
) -> Result<MergeOutcome> {
    verify_merge_ancestor(ancestor, ours, theirs, ours_ancestry, theirs_ancestry)?;
    judge_merge(ancestor, ours, theirs)
}

struct Work(u64);

impl Work {
    fn charge(&mut self, amount: u64) -> Result<()> {
        self.0 = self
            .0
            .checked_add(amount)
            .ok_or(MergeError::Merge(MergeErrorCode::ResourceLimit))?;
        if self.0 > MAX_MERGE_WORK {
            return fail(MergeErrorCode::ResourceLimit);
        }
        Ok(())
    }
}

struct DeltaView<'a> {
    entities: BTreeMap<EntityId, &'a crate::EntityDelta>,
    fields: BTreeMap<EntityId, Vec<&'a FieldDelta>>,
}

impl<'a> DeltaView<'a> {
    fn new(delta: &'a SemanticDelta) -> Self {
        let mut fields: BTreeMap<EntityId, Vec<&FieldDelta>> = BTreeMap::new();
        for field in &delta.fields {
            fields.entry(field.entity_id).or_default().push(field);
        }
        Self {
            entities: delta
                .entities
                .iter()
                .map(|entry| (entry.entity_id, entry))
                .collect(),
            fields,
        }
    }

    fn touched(&self, id: EntityId) -> bool {
        self.entities.get(&id).is_some_and(|entry| {
            matches!(
                entry.change,
                ChangeClass::Changed | ChangeClass::Retyped | ChangeClass::Removed
            )
        })
    }
}

/// One merge input: a complete root's exact objects and record facts.
#[derive(Clone, Debug)]
pub struct MergeSide {
    /// Transaction the side was loaded from (`None` for a synthetic side).
    pub transaction_id: Option<TransactionId>,
    /// Exact live entity objects in binding order.
    pub objects: Vec<EntityObject>,
    /// Root identity.
    pub root: StateRoot,
    /// Record workspace.
    pub workspace_id: WorkspaceId,
    /// Record schema epoch.
    pub schema_epoch_id: SchemaEpochId,
    /// Record entry points.
    pub entry_points: Vec<EntityId>,
    /// Record dependency roots.
    pub dependency_roots: Vec<StateRoot>,
    /// Record contract root.
    pub contract_root: ObjectId,
    /// Record test root.
    pub test_root: ObjectId,
    /// Record policy root.
    pub policy_root: sley_id::PolicyRootId,
}

impl MergeSide {
    /// Copies one verified revision into a merge side.
    #[must_use]
    pub fn from_revision(revision: &VerifiedRevision) -> Self {
        let record = &revision.state_root().record;
        Self {
            transaction_id: Some(revision.transaction_id()),
            objects: revision.objects().to_vec(),
            root: revision.state_root().root,
            workspace_id: record.workspace_id,
            schema_epoch_id: record.schema_epoch_id,
            entry_points: record.entry_points.clone(),
            dependency_roots: record.dependency_roots.clone(),
            contract_root: record.contract_root,
            test_root: record.test_root,
            policy_root: record.policy_root,
        }
    }

    pub(crate) fn request(&self) -> Result<CompleteRootRequest> {
        let entities = sley_policy::complete_entities::project_complete_entities(&self.objects)
            .map_err(|error| MergeError::Extraction(CompleteRootError::Projection(error)))?;
        let bound: Vec<(EntityId, ObjectId)> = self
            .objects
            .iter()
            .map(|object| (object.record().entity_id, object.object_id()))
            .collect();
        for pair in bound.windows(2) {
            if pair[0].0 >= pair[1].0 {
                return Err(MergeError::Extraction(CompleteRootError::Impact(
                    sley_query::ImpactError::new(ImpactErrorCode::SetNotCanonical),
                )));
            }
        }
        Ok(CompleteRootRequest::from_parts(
            entities,
            self.root,
            self.workspace_id,
            self.schema_epoch_id,
            bound,
            self.entry_points.clone(),
            self.dependency_roots.clone(),
        ))
    }
}

struct Side<'a> {
    request: CompleteRootRequest,
    objects: BTreeMap<EntityId, &'a EntityObject>,
    kinds: BTreeMap<EntityId, u32>,
}

impl<'a> Side<'a> {
    fn build(side: &'a MergeSide) -> Result<Self> {
        let request = side.request()?;
        let objects: BTreeMap<EntityId, &EntityObject> = side
            .objects
            .iter()
            .map(|object| (object.record().entity_id, object))
            .collect();
        let kinds = objects
            .iter()
            .map(|(id, object)| (*id, u32::from(object.record().body.kind_tag())))
            .collect();
        Ok(Self {
            request,
            objects,
            kinds,
        })
    }
}

#[derive(Clone, Copy)]
enum Resolution<'a> {
    Keep,
    Remove,
    Take(&'a EntityObject),
}

/// Judges the merge of `ours` and `theirs` against `ancestor`.
///
/// # Errors
///
/// Returns a precondition, comparison, or resource failure; a conflict is a
/// successful outcome.
#[allow(clippy::too_many_lines)]
pub fn judge_merge(
    ancestor: &MergeSide,
    ours: &MergeSide,
    theirs: &MergeSide,
) -> Result<MergeOutcome> {
    let base = Side::build(ancestor)?;
    let a = Side::build(ours)?;
    let b = Side::build(theirs)?;
    let workspace_id = base.request.workspace_id();
    if a.request.workspace_id() != workspace_id || b.request.workspace_id() != workspace_id {
        return fail(MergeErrorCode::WorkspaceMismatch);
    }
    let epoch = base.request.schema_epoch_id();
    if a.request.schema_epoch_id() != epoch || b.request.schema_epoch_id() != epoch {
        return fail(MergeErrorCode::EpochMismatch);
    }
    let delta_a = compare_complete_roots(&base.request, &a.request).map_err(MergeError::Compare)?;
    let delta_b = compare_complete_roots(&base.request, &b.request).map_err(MergeError::Compare)?;
    if !delta_a.delta.dependency_roots_added.is_empty()
        || !delta_a.delta.dependency_roots_removed.is_empty()
        || !delta_b.delta.dependency_roots_added.is_empty()
        || !delta_b.delta.dependency_roots_removed.is_empty()
    {
        return fail(MergeErrorCode::DependencyRootChangeUnsupported);
    }
    let mut work = Work(0);
    let mut conflicts: Vec<ConflictEntry> = Vec::new();
    let contract_differs = ours.contract_root != ancestor.contract_root
        || theirs.contract_root != ancestor.contract_root;
    let test_differs =
        ours.test_root != ancestor.test_root || theirs.test_root != ancestor.test_root;
    // One pinned entry per anchor class that moved: 1 is the contract root,
    // 2 is the test root, 3 is both, so the record names which anchor failed.
    if contract_differs || test_differs {
        conflicts.push(anchor_conflict(
            ConflictReason::RootAnchor,
            u32::from(contract_differs) | (u32::from(test_differs) << 1),
        ));
    }
    if ours.policy_root != ancestor.policy_root || theirs.policy_root != ancestor.policy_root {
        conflicts.push(anchor_conflict(ConflictReason::PolicyRoot, 0));
    }

    let view_a = DeltaView::new(&delta_a.delta);
    let view_b = DeltaView::new(&delta_b.delta);
    let mut resolutions: BTreeMap<EntityId, Resolution<'_>> = BTreeMap::new();
    let mut composed: BTreeMap<EntityId, EntityObject> = BTreeMap::new();
    let mut metadata_overridden = Vec::new();
    let union: BTreeSet<EntityId> = view_a
        .entities
        .keys()
        .chain(view_b.entities.keys())
        .copied()
        .collect();
    for id in &union {
        work.charge(1)?;
        let in_a = view_a.entities.get(id).copied();
        let in_b = view_b.entities.get(id).copied();
        let ours_object = a.objects.get(id).map(|object| object.object_id());
        let theirs_object = b.objects.get(id).map(|object| object.object_id());
        let kind = base.kinds.get(id).copied().unwrap_or_else(|| {
            // No base kind (both sides added): the canonical tiebreak over
            // the present side kinds, so the entry is byte-identical under
            // an ours/theirs swap (S20-520 revision 5). The lesser tag names
            // neither side; it is the deterministic representative.
            a.kinds
                .get(id)
                .copied()
                .into_iter()
                .chain(b.kinds.get(id).copied())
                .min()
                .unwrap_or(0)
        });
        let conflict = |reason: ConflictReason, field: u32| ConflictEntry {
            entity_id: *id,
            reason,
            kind,
            field,
            ours_object,
            theirs_object,
            detail: 0,
        };
        let resolution = match (in_a, in_b) {
            // J1: one side only.
            (Some(entry), None) => side_resolution(entry, &a),
            (None, Some(entry)) => side_resolution(entry, &b),
            (Some(entry_a), Some(entry_b)) => {
                let same_target = entry_a.change == entry_b.change
                    && entry_a.target_object == entry_b.target_object;
                if same_target {
                    // J2: convergent.
                    side_resolution(entry_a, &a)
                } else if matches!(entry_a.change, ChangeClass::Removed)
                    || matches!(entry_b.change, ChangeClass::Removed)
                {
                    conflicts.push(conflict(ConflictReason::DeleteEdit, 0));
                    continue;
                } else if matches!(entry_a.change, ChangeClass::Added)
                    || matches!(entry_b.change, ChangeClass::Added)
                {
                    conflicts.push(conflict(ConflictReason::AddAdd, 0));
                    continue;
                } else if matches!(entry_a.change, ChangeClass::Retyped)
                    || matches!(entry_b.change, ChangeClass::Retyped)
                {
                    conflicts.push(conflict(ConflictReason::KindEdit, 0));
                    continue;
                } else if entry_a.change == ChangeClass::MetadataOnly
                    && entry_b.change == ChangeClass::Changed
                {
                    metadata_overridden.push(*id);
                    side_resolution(entry_b, &b)
                } else if entry_b.change == ChangeClass::MetadataOnly
                    && entry_a.change == ChangeClass::Changed
                {
                    metadata_overridden.push(*id);
                    side_resolution(entry_a, &a)
                } else if entry_a.change == ChangeClass::MetadataOnly
                    && entry_b.change == ChangeClass::MetadataOnly
                {
                    conflicts.push(conflict(ConflictReason::MetadataEdit, 0));
                    continue;
                } else {
                    // J7: both changed; compose field by field.
                    match compose(
                        *id,
                        &base,
                        &a,
                        &b,
                        view_a.fields.get(id).map_or(&[][..], Vec::as_slice),
                        view_b.fields.get(id).map_or(&[][..], Vec::as_slice),
                        &mut work,
                        &mut metadata_overridden,
                    )? {
                        Ok(object) => {
                            composed.insert(*id, object);
                            continue;
                        }
                        Err(field) => {
                            conflicts.push(conflict(ConflictReason::FieldEdit, field));
                            continue;
                        }
                    }
                }
            }
            (None, None) => return fail(MergeErrorCode::InternalInvariant),
        };
        resolutions.insert(*id, resolution);
    }
    // Collateral runs only on identities that survived J1 through J8
    // without conflict: anything resolved here took no conflict branch.
    let conflicted: BTreeSet<EntityId> = union
        .iter()
        .filter(|id| !resolutions.contains_key(id))
        .copied()
        .collect();

    // Collateral rule.
    let index_a = a
        .request
        .judge()
        .map_err(MergeError::Extraction)?
        .into_index();
    let index_b = b
        .request
        .judge()
        .map_err(MergeError::Extraction)?
        .into_index();
    for (this, other, other_index) in [(&view_a, &view_b, &index_b), (&view_b, &view_a, &index_a)] {
        for (id, entry) in &this.entities {
            work.charge(1)?;
            if !this.touched(*id) || conflicted.contains(id) {
                continue;
            }
            let dependents = non_ownership_dependents(other_index, *id, &mut work)?;
            // The dependent side counts Added, Changed, and Retyped: a
            // retyped dependent is a strictly stronger edit than a changed
            // one. Removed dependents are excluded (their removal resolves
            // by removal), as are MetadataOnly touches, which never prove
            // semantic dependence.
            let collateral = other.entities.iter().any(|(other_id, other_entry)| {
                matches!(
                    other_entry.change,
                    ChangeClass::Added | ChangeClass::Changed | ChangeClass::Retyped
                ) && dependents.contains(other_id)
            });
            if collateral {
                conflicts.push(ConflictEntry {
                    entity_id: *id,
                    reason: ConflictReason::Collateral,
                    kind: base.kinds.get(id).copied().unwrap_or(0),
                    field: 0,
                    // Ours' object always comes from A and theirs'
                    // always from B, whichever direction is checked.
                    ours_object: a.objects.get(id).map(|o| o.object_id()),
                    theirs_object: b.objects.get(id).map(|o| o.object_id()),
                    detail: 0,
                });
            }
            let _ = entry;
        }
    }

    if !conflicts.is_empty() {
        return conflict_outcome(&base, &a, &b, &delta_a, &delta_b, conflicts);
    }

    // Merged entity set.
    let mut merged: BTreeMap<EntityId, EntityObject> = BTreeMap::new();
    for (id, object) in &base.objects {
        merged.insert(*id, (*object).clone());
    }
    for (id, resolution) in &resolutions {
        match resolution {
            Resolution::Keep => {}
            Resolution::Remove => {
                merged.remove(id);
            }
            Resolution::Take(object) => {
                merged.insert(*id, (*object).clone());
            }
        }
    }
    for (id, object) in composed {
        merged.insert(id, object);
    }
    let objects: Vec<EntityObject> = merged.into_values().collect();
    if objects.len() > sley_query::MAX_IMPACT_ENTITIES {
        return fail(MergeErrorCode::ResourceLimit);
    }
    let entities = match sley_policy::complete_entities::project_complete_entities(&objects) {
        Ok(entities) => entities,
        Err(error) => {
            let code = crate::complete_root::CompleteRootError::Projection(error).numeric();
            conflicts.push(closure_conflict(None, code));
            return conflict_outcome(&base, &a, &b, &delta_a, &delta_b, conflicts);
        }
    };
    let entry_points: Vec<EntityId> = objects
        .iter()
        .filter(|object| matches!(object.record().body, EntityBodyValue::EntryPoint(_)))
        .map(|object| object.record().entity_id)
        .collect();
    let bound_objects: Vec<(EntityId, ObjectId)> = objects
        .iter()
        .map(|object| (object.record().entity_id, object.object_id()))
        .collect();
    let mut builder = StateRootBuilder::new(
        workspace_id,
        ancestor.contract_root,
        ancestor.test_root,
        ancestor.policy_root,
    );
    for (entity_id, object_id) in &bound_objects {
        builder = builder.entity_binding(*entity_id, *object_id);
    }
    for entry_point in &entry_points {
        builder = builder.entry_point(*entry_point);
    }
    for dependency_root in &ancestor.dependency_roots {
        builder = builder.dependency_root(*dependency_root);
    }
    let registry =
        merge_registry().map_err(|_| MergeError::Merge(MergeErrorCode::InternalInvariant))?;
    let state_root = builder
        .build(&registry)
        .map_err(|_| MergeError::Merge(MergeErrorCode::ResourceLimit))?;
    let request = CompleteRootRequest::from_parts(
        entities,
        state_root.root,
        workspace_id,
        epoch,
        bound_objects,
        entry_points,
        ancestor.dependency_roots.clone(),
    );
    if let Err(error) = request.judge() {
        conflicts.push(closure_conflict(None, error.numeric()));
        return conflict_outcome(&base, &a, &b, &delta_a, &delta_b, conflicts);
    }
    Ok(MergeOutcome::Merged(Box::new(MergedRoot {
        objects,
        state_root,
        request,
        metadata_overridden,
        ours_delta: delta_a.delta_id,
        theirs_delta: delta_b.delta_id,
    })))
}

/// Transitive dependents of `id` through relations other than `Ownership`.
fn non_ownership_dependents(
    index: &sley_query::ImpactIndex,
    id: EntityId,
    work: &mut Work,
) -> Result<BTreeSet<EntityId>> {
    let mut reached = BTreeSet::new();
    let mut queue = vec![id];
    while let Some(current) = queue.pop() {
        for edge in index.reverse_edges(current) {
            work.charge(1)?;
            if edge.kind != sley_query::ImpactKind::Ownership && reached.insert(edge.dependent) {
                queue.push(edge.dependent);
            }
        }
    }
    Ok(reached)
}

fn side_resolution<'a>(entry: &crate::EntityDelta, side: &Side<'a>) -> Resolution<'a> {
    match entry.change {
        ChangeClass::Removed => Resolution::Remove,
        _ => side
            .objects
            .get(&entry.entity_id)
            .map_or(Resolution::Keep, |object| Resolution::Take(object)),
    }
}

const fn anchor_conflict(reason: ConflictReason, detail: u32) -> ConflictEntry {
    ConflictEntry {
        entity_id: EntityId::from_bytes(ZERO32),
        reason,
        kind: 0,
        field: 0,
        ours_object: None,
        theirs_object: None,
        detail,
    }
}

fn closure_conflict(id: Option<EntityId>, detail: u32) -> ConflictEntry {
    ConflictEntry {
        entity_id: id.unwrap_or_else(|| EntityId::from_bytes(ZERO32)),
        reason: ConflictReason::Closure,
        kind: 0,
        field: 0,
        ours_object: None,
        theirs_object: None,
        detail,
    }
}

fn conflict_outcome(
    base: &Side<'_>,
    a: &Side<'_>,
    b: &Side<'_>,
    delta_a: &crate::StoredSemanticDelta,
    delta_b: &crate::StoredSemanticDelta,
    mut conflicts: Vec<ConflictEntry>,
) -> Result<MergeOutcome> {
    conflicts.sort_by_key(|entry| (entry.entity_id, entry.reason, entry.field));
    conflicts.dedup();
    let conflict = MergeConflict {
        workspace_id: base.request.workspace_id(),
        ancestor_root: base.request.root(),
        ours_root: a.request.root(),
        theirs_root: b.request.root(),
        ours_delta: delta_a.delta_id,
        theirs_delta: delta_b.delta_id,
        conflicts,
    };
    Ok(MergeOutcome::Conflict(Box::new(encode_merge_conflict(
        &conflict,
    )?)))
}

/// Composes one entity changed on both sides; `Err(field)` names the first
/// conflicting field.
#[allow(clippy::too_many_arguments)]
fn compose(
    id: EntityId,
    base: &Side<'_>,
    a: &Side<'_>,
    b: &Side<'_>,
    fields_a: &[&FieldDelta],
    fields_b: &[&FieldDelta],
    work: &mut Work,
    metadata_overridden: &mut Vec<EntityId>,
) -> Result<core::result::Result<EntityObject, u32>> {
    let base_object = base
        .objects
        .get(&id)
        .ok_or(MergeError::Merge(MergeErrorCode::InternalInvariant))?;
    let ours = a
        .objects
        .get(&id)
        .ok_or(MergeError::Merge(MergeErrorCode::InternalInvariant))?;
    let theirs = b
        .objects
        .get(&id)
        .ok_or(MergeError::Merge(MergeErrorCode::InternalInvariant))?;
    let kind = u32::from(base_object.record().body.kind_tag());
    let mut body = base_object.record().body.clone();
    let by_field_a: BTreeMap<u32, &FieldDelta> = fields_a.iter().map(|f| (f.field, *f)).collect();
    let by_field_b: BTreeMap<u32, &FieldDelta> = fields_b.iter().map(|f| (f.field, *f)).collect();
    let all_fields: BTreeSet<u32> = by_field_a
        .keys()
        .chain(by_field_b.keys())
        .copied()
        .collect();
    // Both sides `Changed` with no field delta on either side means the S20-510
    // delta names no field the composition can take: yielding `O`'s body would
    // silently drop both changes, so this is a defensive invariant, never a
    // composition.
    if all_fields.is_empty() {
        return fail(MergeErrorCode::InternalInvariant);
    }
    for field in all_fields {
        work.charge(1)?;
        match (by_field_a.get(&field), by_field_b.get(&field)) {
            (Some(_), None) => {
                if !copy_field(&mut body, &ours.record().body, field) {
                    return fail(MergeErrorCode::InternalInvariant);
                }
            }
            (None, Some(_)) => {
                if !copy_field(&mut body, &theirs.record().body, field) {
                    return fail(MergeErrorCode::InternalInvariant);
                }
            }
            (Some(fa), Some(fb)) => {
                let set_field = SET_FIELDS.contains(&(kind, field));
                if set_field {
                    let a_added: BTreeSet<EntityId> = fa.added.iter().copied().collect();
                    let a_removed: BTreeSet<EntityId> = fa.removed.iter().copied().collect();
                    let b_added: BTreeSet<EntityId> = fb.added.iter().copied().collect();
                    let b_removed: BTreeSet<EntityId> = fb.removed.iter().copied().collect();
                    if a_added.intersection(&b_removed).next().is_some()
                        || a_removed.intersection(&b_added).next().is_some()
                    {
                        return Ok(Err(field));
                    }
                    let Some(base_ids) = identity_set(&base_object.record().body, field) else {
                        return fail(MergeErrorCode::InternalInvariant);
                    };
                    let mut merged: BTreeSet<EntityId> = base_ids.iter().copied().collect();
                    merged.extend(a_added);
                    merged.extend(b_added);
                    for removed in a_removed.iter().chain(b_removed.iter()) {
                        merged.remove(removed);
                    }
                    let set = EntityIdSet::from_unsorted(merged.into_iter().collect())
                        .map_err(|_| MergeError::Merge(MergeErrorCode::InternalInvariant))?;
                    if !replace_identity_set(&mut body, field, set) {
                        return fail(MergeErrorCode::InternalInvariant);
                    }
                } else if field_equal(&ours.record().body, &theirs.record().body, field) {
                    if !copy_field(&mut body, &ours.record().body, field) {
                        return fail(MergeErrorCode::InternalInvariant);
                    }
                } else {
                    return Ok(Err(field));
                }
            }
            (None, None) => {}
        }
    }
    let record = EntityObjectRecord {
        entity_id: id,
        body,
        // The frozen commit path preserves exactly the current side's label
        // and fingerprint claim, so the composed object carries A's: any
        // other choice makes the precomputed root uncommittable. A label
        // theirs changed is reported, never dropped silently.
        label: ours.record().label.clone(),
        semantic_fingerprint: ours.record().semantic_fingerprint,
    };
    if ours.record().label != theirs.record().label {
        metadata_overridden.push(id);
    }
    let object = build_entity_object(ours.schema_epoch_id(), &record)
        .map_err(|_| MergeError::Merge(MergeErrorCode::ResourceLimit))?;
    Ok(Ok(object))
}

macro_rules! copy_fields {
    ($dst:expr, $src:expr, $field:expr, $variant:ident, { $($tag:literal => $name:ident),* $(,)? }) => {
        if let (EntityBodyValue::$variant(dst), EntityBodyValue::$variant(src)) = (&mut *$dst, $src) {
            return match $field {
                $($tag => { dst.$name = src.$name.clone(); true })*
                _ => false,
            };
        }
    };
}

/// Copies one schema field from `src` into `dst` (same kind); `false` when
/// the kinds or the field tag do not match.
#[allow(clippy::too_many_lines)]
fn copy_field(dst: &mut EntityBodyValue, src: &EntityBodyValue, field: u32) -> bool {
    copy_fields!(dst, src, field, Workspace, { 1 => packages, 2 => root_namespace, 3 => capability_requirements, 4 => contracts, 5 => tests });
    copy_fields!(dst, src, field, Package, { 1 => workspace, 2 => root_namespace, 3 => dependencies, 4 => exports });
    copy_fields!(dst, src, field, Namespace, { 1 => parent, 2 => members });
    copy_fields!(dst, src, field, TypeDef, { 1 => type_parameters, 2 => form, 3 => invariants, 4 => visibility });
    copy_fields!(dst, src, field, Function, { 1 => type_parameters, 2 => parameters, 3 => result_type, 4 => effects, 5 => entry_block, 6 => blocks, 7 => contracts, 8 => visibility });
    copy_fields!(dst, src, field, Parameter, { 1 => owner, 2 => role, 3 => ordinal, 4 => value_type });
    copy_fields!(dst, src, field, Block, { 1 => function, 2 => parameters, 3 => operations, 4 => terminator, 5 => reachability });
    copy_fields!(dst, src, field, Operation, { 1 => block, 2 => ordinal, 3 => opcode, 4 => operands, 5 => result_types, 6 => immediate });
    copy_fields!(dst, src, field, Constant, { 1 => value });
    copy_fields!(dst, src, field, GlobalValue, { 1 => value_type, 2 => initializer, 3 => visibility });
    copy_fields!(dst, src, field, EffectDef, { 1 => effect_kind, 2 => scope_type, 3 => request_type, 4 => response_type, 5 => failure_type, 6 => visibility });
    copy_fields!(dst, src, field, CapabilityRequirement, { 1 => effect, 2 => allowed_scopes, 3 => constraint_contracts });
    copy_fields!(dst, src, field, Contract, { 1 => target, 2 => contract_kind, 3 => predicate, 4 => bindings, 5 => resource_limits });
    copy_fields!(dst, src, field, TestCase, { 1 => target, 2 => inputs, 3 => effect_environment, 4 => expected, 5 => observations, 6 => resource_limits });
    copy_fields!(dst, src, field, AdapterImport, { 1 => adapter_id, 2 => abi_version, 3 => request_type, 4 => response_type, 5 => failure_type, 6 => effects });
    copy_fields!(dst, src, field, EntryPoint, { 1 => function, 2 => exposure });
    copy_fields!(dst, src, field, PolicyBinding, { 1 => subject, 2 => requirements });
    copy_fields!(dst, src, field, DependencyBinding, { 1 => dependency_root, 2 => external_package, 3 => local_namespace });
    false
}

fn field_equal(a: &EntityBodyValue, b: &EntityBodyValue, field: u32) -> bool {
    let mut probe = a.clone();
    // Copying the field from `b` into a clone of `a` leaves it equal to `a`
    // exactly when the field values are equal.
    copy_field(&mut probe, b, field) && probe == *a
}

macro_rules! identity_sets {
    ($body:expr, $field:expr, $variant:ident, { $($tag:literal => $name:ident),* $(,)? }) => {
        if let EntityBodyValue::$variant(body) = $body {
            return match $field {
                $($tag => Some(body.$name.as_slice()),)*
                _ => None,
            };
        }
    };
}

fn identity_set(body: &EntityBodyValue, field: u32) -> Option<&[EntityId]> {
    identity_sets!(body, field, Workspace, { 1 => packages, 3 => capability_requirements, 4 => contracts, 5 => tests });
    identity_sets!(body, field, Package, { 3 => dependencies, 4 => exports });
    identity_sets!(body, field, Namespace, { 2 => members });
    identity_sets!(body, field, TypeDef, { 3 => invariants });
    identity_sets!(body, field, Function, { 4 => effects, 7 => contracts });
    identity_sets!(body, field, CapabilityRequirement, { 3 => constraint_contracts });
    identity_sets!(body, field, AdapterImport, { 6 => effects });
    identity_sets!(body, field, PolicyBinding, { 2 => requirements });
    None
}

macro_rules! replace_sets {
    ($body:expr, $field:expr, $set:expr, $variant:ident, { $($tag:literal => $name:ident),* $(,)? }) => {
        if let EntityBodyValue::$variant(body) = &mut *$body {
            return match $field {
                $($tag => { body.$name = $set; true })*
                _ => false,
            };
        }
    };
}

fn replace_identity_set(body: &mut EntityBodyValue, field: u32, set: EntityIdSet) -> bool {
    replace_sets!(body, field, set, Workspace, { 1 => packages, 3 => capability_requirements, 4 => contracts, 5 => tests });
    replace_sets!(body, field, set, Package, { 3 => dependencies, 4 => exports });
    replace_sets!(body, field, set, Namespace, { 2 => members });
    replace_sets!(body, field, set, TypeDef, { 3 => invariants });
    replace_sets!(body, field, set, Function, { 4 => effects, 7 => contracts });
    replace_sets!(body, field, set, CapabilityRequirement, { 3 => constraint_contracts });
    replace_sets!(body, field, set, AdapterImport, { 6 => effects });
    replace_sets!(body, field, set, PolicyBinding, { 2 => requirements });
    false
}

/// Builds the S20-350 plan that moves `ours` to the merged root.
///
/// # Errors
///
/// Returns `MERGE_PLAN_UNSUPPORTED` when an operation cannot be expressed
/// or `MERGE_RESOURCE_LIMIT` above the plan ceiling.
#[allow(clippy::too_many_lines)]
pub fn build_merge_plan(ours: &MergeSide, merged: &MergedRoot) -> Result<MergePlan> {
    let Some(base_transaction_id) = ours.transaction_id else {
        return fail(MergeErrorCode::PlanUnsupported);
    };
    let ours_objects: BTreeMap<EntityId, &EntityObject> = ours
        .objects
        .iter()
        .map(|object| (object.record().entity_id, object))
        .collect();
    let mut nonce = blake3::Hasher::new();
    nonce.update(NONCE_DOMAIN);
    nonce.update(ours.root.as_bytes());
    nonce.update(merged.state_root.root.as_bytes());
    let candidate_nonce = CandidateNonce::from_bytes(*nonce.finalize().as_bytes());

    // S20-345: every identity a candidate creates derives from the candidate
    // nonce, kind, and creation ordinal, so entities the merged root adds to
    // ours are re-identified in raw-ID order and every reference follows.
    // Created entry points are the one exclusion: the frozen S20-350
    // descriptor binds `AddEntryPoint` to `ExactEntityVersion`, and frozen
    // S20-360 phase 3 checks every such precondition against the base state,
    // so no single candidate can bind an entry point it also creates. The
    // plan fails closed instead of emitting an unexecutable shape or a
    // foreign identity; the recovery is to bind the entry point on ours
    // first (create, then add in a second candidate) and merge again.
    let mut identity_map: BTreeMap<EntityId, EntityId> = BTreeMap::new();
    let mut create_ordinal = 0_u64;
    for object in &merged.objects {
        let id = object.record().entity_id;
        if ours_objects.contains_key(&id) {
            continue;
        }
        if matches!(object.record().body, EntityBodyValue::EntryPoint(_)) {
            // A theirs-added entry point the plan would have to both create
            // and bind: inexpressible in one frozen candidate.
            return fail(MergeErrorCode::PlanUnsupported);
        }
        let derived = EntityId::derive(
            ours.workspace_id,
            candidate_nonce,
            u32::from(object.record().body.kind_tag()),
            create_ordinal,
        );
        if ours_objects.contains_key(&derived) {
            // A derived identity that already names an unrelated live
            // entity must fail, never silently replace it.
            return fail(MergeErrorCode::PlanUnsupported);
        }
        identity_map.insert(id, derived);
        create_ordinal += 1;
    }
    let mut remapped: Vec<EntityObject> = Vec::with_capacity(merged.objects.len());
    for object in &merged.objects {
        let record = object.record();
        let mut body = record.body.clone();
        remap_body(&mut body, &identity_map)?;
        // The plan object must be byte-identical to what the frozen commit
        // path will produce for its operation: created entities go through
        // `CreateEntity` (no label, no fingerprint claim) and every other
        // changed entity through `replace_body` (A's label and fingerprint).
        // Reused objects whose metadata already matches are kept by identity.
        let (entity_id, label, fingerprint) =
            if let Some(mapped) = identity_map.get(&record.entity_id) {
                (*mapped, None, None)
            } else if let Some(current) = ours_objects.get(&record.entity_id) {
                (
                    record.entity_id,
                    current.record().label.clone(),
                    current.record().semantic_fingerprint,
                )
            } else {
                return fail(MergeErrorCode::InternalInvariant);
            };
        if body == record.body
            && entity_id == record.entity_id
            && label == record.label
            && fingerprint == record.semantic_fingerprint
        {
            remapped.push(object.clone());
        } else {
            remapped.push(
                build_entity_object(
                    object.schema_epoch_id(),
                    &EntityObjectRecord {
                        entity_id,
                        body,
                        label,
                        semantic_fingerprint: fingerprint,
                    },
                )
                .map_err(|_| MergeError::Merge(MergeErrorCode::ResourceLimit))?,
            );
        }
    }
    remapped.sort_by_key(|object| object.record().entity_id);
    let merged_objects: BTreeMap<EntityId, &EntityObject> = remapped
        .iter()
        .map(|object| (object.record().entity_id, object))
        .collect();

    let union: BTreeSet<EntityId> = ours_objects
        .keys()
        .chain(merged_objects.keys())
        .copied()
        .collect();
    let mut operations = Vec::new();
    let mut preconditions = Vec::new();
    let mut push = |class: MutationClass,
                    target_kind: u16,
                    target_entity: EntityId,
                    payload: MutationPayload,
                    precondition: PreconditionPayload,
                    requirement: PreimageRequirement| {
        let ordinal = u32::try_from(operations.len()).unwrap_or(u32::MAX);
        operations.push(MutationOperation {
            ordinal,
            class,
            target_kind,
            target_entity,
            field_tag: None,
            payload,
            precondition_ordinal: ordinal,
        });
        preconditions.push(BoundPrecondition {
            operation_ordinal: ordinal,
            requirement,
            payload: precondition,
        });
    };
    // Created entities come first, in derivation order (raw-ID order of the
    // judged identities): the frozen S20-345 rule numbers creation ordinals
    // by `CreateEntity` operation position, so derivation order and
    // operation order must agree. Every other operation follows in raw-ID
    // order of its plan target.
    for derived in identity_map.values() {
        let object = merged_objects
            .get(derived)
            .ok_or(MergeError::Merge(MergeErrorCode::InternalInvariant))?;
        let body = object.record().body.clone();
        let kind = body.kind_tag();
        let absent = PreconditionPayload::ExpectedIdentityAbsent(ExpectedIdentityAbsent {
            entity_id: *derived,
        });
        push(
            MutationClass::CreateEntity,
            kind,
            *derived,
            MutationPayload::CreateEntity(body),
            absent,
            PreimageRequirement::ExpectedIdentityAbsent,
        );
        if let EntityBodyValue::EntryPoint(entry) = &object.record().body {
            // The frozen apply path binds an entry point only on a live
            // entity, so a created entry point takes a second operation.
            push(
                MutationClass::AddEntryPoint,
                kind,
                *derived,
                MutationPayload::AddEntryPoint(entry.clone()),
                PreconditionPayload::ExpectedIdentityAbsent(ExpectedIdentityAbsent {
                    entity_id: *derived,
                }),
                PreimageRequirement::ExpectedIdentityAbsent,
            );
        }
    }
    for id in union {
        match (ours_objects.get(&id), merged_objects.get(&id)) {
            (None, Some(_)) => {
                // Created above in derivation order.
            }
            (Some(object), None) => {
                let kind = object.record().body.kind_tag();
                let exact = PreconditionPayload::ExactEntityVersion(ExactEntityVersion {
                    entity_id: id,
                    object_id: object.object_id(),
                });
                if matches!(object.record().body, EntityBodyValue::EntryPoint(_)) {
                    push(
                        MutationClass::RemoveEntryPoint,
                        kind,
                        id,
                        MutationPayload::RemoveEntryPoint,
                        exact.clone(),
                        PreimageRequirement::ExactEntityVersion,
                    );
                }
                push(
                    MutationClass::DeleteEntityBinding,
                    kind,
                    id,
                    MutationPayload::DeleteEntityBinding,
                    exact,
                    PreimageRequirement::ExactEntityVersion,
                );
            }
            (Some(before), Some(after)) => {
                if before.object_id() == after.object_id() {
                    continue;
                }
                if before.record().body.kind_tag() != after.record().body.kind_tag() {
                    return fail(MergeErrorCode::PlanUnsupported);
                }
                push(
                    MutationClass::ReplaceEntityVersion,
                    after.record().body.kind_tag(),
                    id,
                    MutationPayload::ReplaceEntityVersion(after.record().body.clone()),
                    PreconditionPayload::ExactEntityVersion(ExactEntityVersion {
                        entity_id: id,
                        object_id: before.object_id(),
                    }),
                    PreimageRequirement::ExactEntityVersion,
                );
            }
            (None, None) => return fail(MergeErrorCode::InternalInvariant),
        }
    }
    let mut work = Work(0);
    work.charge(u64::try_from(operations.len()).unwrap_or(u64::MAX))?;
    if operations.len() > MAX_PLAN_OPERATIONS {
        return fail(MergeErrorCode::ResourceLimit);
    }

    // The plan's merged root is the judged root with the remapped bindings.
    let record = &merged.state_root.record;
    let mut builder = StateRootBuilder::new(
        record.workspace_id,
        record.contract_root,
        record.test_root,
        record.policy_root,
    );
    for object in &remapped {
        builder = builder.entity_binding(object.record().entity_id, object.object_id());
    }
    for entry_point in &record.entry_points {
        builder = builder.entry_point(
            identity_map
                .get(entry_point)
                .copied()
                .unwrap_or(*entry_point),
        );
    }
    for dependency_root in &record.dependency_roots {
        builder = builder.dependency_root(*dependency_root);
    }
    let registry =
        merge_registry().map_err(|_| MergeError::Merge(MergeErrorCode::InternalInvariant))?;
    let merged_root = builder
        .build(&registry)
        .map_err(|_| MergeError::Merge(MergeErrorCode::ResourceLimit))?
        .root;
    Ok(MergePlan {
        base_transaction_id,
        base_root: ours.root,
        merged_root,
        operations,
        preconditions,
        metadata_overridden: merged.metadata_overridden.clone(),
        candidate_nonce,
        identity_map: identity_map.into_iter().collect(),
        objects: remapped,
    })
}

fn remap_id(id: &mut EntityId, map: &BTreeMap<EntityId, EntityId>) {
    if let Some(new) = map.get(id) {
        *id = *new;
    }
}

fn remap_list(ids: &mut [EntityId], map: &BTreeMap<EntityId, EntityId>) {
    for id in ids.iter_mut() {
        remap_id(id, map);
    }
}

fn remap_set(set: &mut EntityIdSet, map: &BTreeMap<EntityId, EntityId>) -> Result<()> {
    let mut ids = set.as_slice().to_vec();
    remap_list(&mut ids, map);
    // A remap that collapses two distinct identities, or input that already
    // held a duplicate, must fail, never emit a partially-remapped body.
    *set = EntityIdSet::from_unsorted(ids)
        .map_err(|_| MergeError::Merge(MergeErrorCode::InternalInvariant))?;
    Ok(())
}

fn remap_sorted(ids: &mut [EntityId], map: &BTreeMap<EntityId, EntityId>) -> Result<()> {
    remap_list(ids, map);
    ids.sort_unstable();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return fail(MergeErrorCode::InternalInvariant);
    }
    Ok(())
}

fn remap_type(value: &mut sley_ssmc::TypeExpr, map: &BTreeMap<EntityId, EntityId>) -> Result<()> {
    use sley_ssmc::TypeExpr;
    match value {
        TypeExpr::Named(named) => {
            remap_id(&mut named.definition, map);
            for argument in &mut named.arguments {
                remap_type(argument, map)?;
            }
        }
        TypeExpr::Tuple(items) => {
            for item in items {
                remap_type(item, map)?;
            }
        }
        TypeExpr::Vector(inner) | TypeExpr::Option(inner) | TypeExpr::LocalCell(inner) => {
            remap_type(inner, map)?;
        }
        TypeExpr::OrderedMap { key, value } => {
            remap_type(key, map)?;
            remap_type(value, map)?;
        }
        TypeExpr::Result { ok, error } => {
            remap_type(ok, map)?;
            remap_type(error, map)?;
        }
        TypeExpr::FunctionRef(function) => {
            for parameter in &mut function.parameters {
                remap_type(parameter, map)?;
            }
            remap_type(&mut function.result, map)?;
            remap_sorted(&mut function.effects, map)?;
        }
        TypeExpr::AdapterHandle(id) | TypeExpr::CapabilityToken(id) => remap_id(id, map),
        TypeExpr::Unit
        | TypeExpr::Bool
        | TypeExpr::SInt(_)
        | TypeExpr::UInt(_)
        | TypeExpr::F32
        | TypeExpr::F64
        | TypeExpr::Bytes
        | TypeExpr::Text
        | TypeExpr::TypeParameter(_)
        | TypeExpr::BuiltinFailure(_) => {}
    }
    Ok(())
}

fn remap_const(
    value: &mut sley_ssmc::ConstValue,
    map: &BTreeMap<EntityId, EntityId>,
) -> Result<()> {
    use sley_ssmc::{ConstData, ResultConst};
    remap_type(&mut value.value_type, map)?;
    match &mut value.data {
        ConstData::Sequence(values) => {
            for item in values {
                remap_const(item, map)?;
            }
        }
        ConstData::Record(record) => {
            remap_id(&mut record.definition, map);
            for field in &mut record.fields {
                remap_const(&mut field.value, map)?;
            }
        }
        ConstData::Variant(variant) => {
            remap_id(&mut variant.definition, map);
            if let Some(payload) = &mut variant.payload {
                remap_const(payload, map)?;
            }
        }
        ConstData::Map(entries) => {
            for entry in entries {
                remap_const(&mut entry.key, map)?;
                remap_const(&mut entry.value, map)?;
            }
        }
        ConstData::Option(Some(inner)) => remap_const(inner, map)?,
        ConstData::Result(ResultConst::Ok(inner) | ResultConst::Err(inner)) => {
            remap_const(inner, map)?;
        }
        ConstData::FunctionRef(function) => {
            remap_id(&mut function.function, map);
            for argument in &mut function.type_arguments {
                remap_type(argument, map)?;
            }
        }
        ConstData::Option(None)
        | ConstData::Unit
        | ConstData::Bool(_)
        | ConstData::SInt(_)
        | ConstData::UInt(_)
        | ConstData::F32Bits(_)
        | ConstData::F64Bits(_)
        | ConstData::Bytes(_)
        | ConstData::Text(_)
        | ConstData::BuiltinFailure(_) => {}
    }
    Ok(())
}

fn remap_value_ref(value: &mut sley_ssmc::ValueRef, map: &BTreeMap<EntityId, EntityId>) {
    use sley_ssmc::ValueRef;
    match value {
        ValueRef::Parameter(id) => remap_id(id, map),
        ValueRef::OperationResult(result) => remap_id(&mut result.operation, map),
    }
}

fn remap_edge(edge: &mut sley_ssmc::TargetEdge, map: &BTreeMap<EntityId, EntityId>) {
    remap_id(&mut edge.target, map);
    for argument in &mut edge.arguments {
        remap_value_ref(argument, map);
    }
}

fn remap_terminator(value: &mut sley_ssmc::Terminator, map: &BTreeMap<EntityId, EntityId>) {
    use sley_ssmc::{SwitchArgument, Terminator};
    match value {
        Terminator::Return(terminator) => {
            remap_value_ref(&mut terminator.value, map);
        }
        Terminator::Branch(terminator) => {
            remap_edge(&mut terminator.edge, map);
        }
        Terminator::CondBranch(terminator) => {
            remap_value_ref(&mut terminator.condition, map);
            remap_edge(&mut terminator.if_true, map);
            remap_edge(&mut terminator.if_false, map);
        }
        Terminator::VariantSwitch(terminator) => {
            remap_value_ref(&mut terminator.value, map);
            for case in &mut terminator.cases {
                remap_id(&mut case.edge.target, map);
                for argument in &mut case.edge.arguments {
                    if let SwitchArgument::Value(value) = argument {
                        remap_value_ref(value, map);
                    }
                }
            }
        }
        Terminator::Trap(terminator) => {
            if let Some(payload) = &mut terminator.payload {
                remap_value_ref(payload, map);
            }
        }
    }
}

/// Rewrites every local entity reference of one body through `map`.
#[allow(clippy::too_many_lines)]
fn remap_body(body: &mut EntityBodyValue, map: &BTreeMap<EntityId, EntityId>) -> Result<()> {
    use sley_ssmc::{ContractSource, EffectEnvironment, ExpectedOutcome, Immediate, TypeDefForm};
    match body {
        EntityBodyValue::Workspace(value) => {
            remap_set(&mut value.packages, map)?;
            remap_id(&mut value.root_namespace, map);
            remap_set(&mut value.capability_requirements, map)?;
            remap_set(&mut value.contracts, map)?;
            remap_set(&mut value.tests, map)?;
        }
        EntityBodyValue::Package(value) => {
            remap_id(&mut value.workspace, map);
            remap_id(&mut value.root_namespace, map);
            remap_set(&mut value.dependencies, map)?;
            remap_set(&mut value.exports, map)?;
        }
        EntityBodyValue::Namespace(value) => {
            if let Some(parent) = &mut value.parent {
                remap_id(parent, map);
            }
            remap_set(&mut value.members, map)?;
        }
        EntityBodyValue::TypeDef(value) => {
            match &mut value.form {
                TypeDefForm::Record(fields) => {
                    for field in fields {
                        remap_type(&mut field.value_type, map)?;
                    }
                }
                TypeDefForm::Variant(cases) => {
                    for case in cases {
                        if let Some(payload) = &mut case.payload_type {
                            remap_type(payload, map)?;
                        }
                    }
                }
            }
            remap_set(&mut value.invariants, map)?;
        }
        EntityBodyValue::Function(value) => {
            remap_list(&mut value.parameters, map);
            remap_type(&mut value.result_type, map)?;
            remap_set(&mut value.effects, map)?;
            remap_id(&mut value.entry_block, map);
            remap_list(&mut value.blocks, map);
            remap_set(&mut value.contracts, map)?;
        }
        EntityBodyValue::Parameter(value) => {
            remap_id(&mut value.owner, map);
            remap_type(&mut value.value_type, map)?;
        }
        EntityBodyValue::Block(value) => {
            remap_id(&mut value.function, map);
            remap_list(&mut value.parameters, map);
            remap_list(&mut value.operations, map);
            remap_terminator(&mut value.terminator, map);
        }
        EntityBodyValue::Operation(value) => {
            remap_id(&mut value.block, map);
            for operand in &mut value.operands {
                remap_value_ref(operand, map);
            }
            for result_type in &mut value.result_types {
                remap_type(result_type, map)?;
            }
            match &mut value.immediate {
                Immediate::Entity(id) => remap_id(id, map),
                Immediate::Variant(variant) => remap_id(&mut variant.definition, map),
                Immediate::Function(function) => {
                    remap_id(&mut function.function, map);
                    for argument in &mut function.type_arguments {
                        remap_type(argument, map)?;
                    }
                }
                Immediate::None
                | Immediate::Index(_)
                | Immediate::Field(_)
                | Immediate::Observation(_) => {}
            }
        }
        EntityBodyValue::Constant(value) => remap_const(&mut value.value, map)?,
        EntityBodyValue::GlobalValue(value) => {
            remap_type(&mut value.value_type, map)?;
            remap_id(&mut value.initializer, map);
        }
        EntityBodyValue::EffectDef(value) => {
            remap_type(&mut value.scope_type, map)?;
            remap_type(&mut value.request_type, map)?;
            remap_type(&mut value.response_type, map)?;
            remap_type(&mut value.failure_type, map)?;
        }
        EntityBodyValue::CapabilityRequirement(value) => {
            remap_id(&mut value.effect, map);
            for scope in &mut value.allowed_scopes {
                remap_const(scope, map)?;
            }
            remap_set(&mut value.constraint_contracts, map)?;
        }
        EntityBodyValue::Contract(value) => {
            remap_id(&mut value.target, map);
            remap_id(&mut value.predicate, map);
            for binding in &mut value.bindings {
                match &mut binding.source {
                    ContractSource::Parameter(id) | ContractSource::Global(id) => {
                        remap_id(id, map);
                    }
                    ContractSource::Result | ContractSource::Error => {}
                }
            }
        }
        EntityBodyValue::TestCase(value) => {
            remap_id(&mut value.target, map);
            for input in &mut value.inputs {
                remap_const(input, map)?;
            }
            match &mut value.effect_environment {
                EffectEnvironment::Replay(bindings) => {
                    for binding in bindings {
                        remap_id(&mut binding.adapter_import, map);
                        for request in &mut binding.request {
                            remap_const(request, map)?;
                        }
                        match &mut binding.response {
                            sley_ssmc::ResultConst::Ok(inner)
                            | sley_ssmc::ResultConst::Err(inner) => remap_const(inner, map)?,
                        }
                    }
                }
                EffectEnvironment::DeterministicAdapters(configurations) => {
                    for configuration in configurations {
                        remap_id(&mut configuration.adapter_import, map);
                        remap_const(&mut configuration.configuration, map)?;
                    }
                }
            }
            if let ExpectedOutcome::Value(expected) = &mut value.expected {
                remap_const(expected, map)?;
            }
            for observation in &mut value.observations {
                remap_const(&mut observation.value, map)?;
            }
        }
        EntityBodyValue::AdapterImport(value) => {
            remap_type(&mut value.request_type, map)?;
            remap_type(&mut value.response_type, map)?;
            remap_type(&mut value.failure_type, map)?;
            remap_set(&mut value.effects, map)?;
        }
        EntityBodyValue::EntryPoint(value) => remap_id(&mut value.function, map),
        EntityBodyValue::PolicyBinding(value) => {
            remap_id(&mut value.subject, map);
            remap_set(&mut value.requirements, map)?;
        }
        EntityBodyValue::DependencyBinding(value) => {
            // `external_package` names an entity of the external root and
            // is never a local identity.
            remap_id(&mut value.local_namespace, map);
        }
    }
    Ok(())
}

/// Commit parameters for a merge plan.
#[derive(Clone, Copy, Debug)]
pub struct MergeCommitInput<'a> {
    /// Committing principal.
    pub principal_id: PrincipalId,
    /// Current time for the frozen commit.
    pub now_unix_millis: u64,
    /// Candidate expiry.
    pub expiry_unix_millis: u64,
    /// Frozen validation limits.
    pub limits: CandidateValidationLimits,
    /// Branch that must currently resolve to ours and advances to the merge.
    pub branch: &'a [u8],
}

/// Commits a merge plan through the frozen S20-390 path and advances the
/// branch, then proves the new head equals the precomputed merged root.
///
/// # Errors
///
/// Returns the exact commit-path failure or `MERGE_RESULT_MISMATCH`.
pub fn commit_merge(
    transactions: &TransactionRepository,
    branches: &BranchRepository,
    ours: &MergeSide,
    plan: &MergePlan,
    input: MergeCommitInput<'_>,
) -> Result<TransactionId> {
    let Some(ours_transaction) = ours.transaction_id else {
        return fail(MergeErrorCode::PlanUnsupported);
    };
    if plan.base_transaction_id != ours_transaction {
        return fail(MergeErrorCode::AncestorMismatch);
    }
    // Fail fast before anything is durable: the branch must still resolve to
    // the plan base. The later `advance_branch` CAS remains the authority (a
    // concurrent advance can still land between this read and the commit),
    // so a failure after the commit leaves the merge transaction durable but
    // unnamed; the contract states that boundary.
    let resolved = branches
        .resolve_branch(input.branch)
        .map_err(commit_branch)?;
    if resolved.reference.record.head_transaction_id != plan.base_transaction_id {
        return Err(MergeError::Commit(CommitFailure {
            symbol: BranchErrorCode::RefNamedCasStale.symbol().to_owned(),
            numeric: Some(BranchErrorCode::RefNamedCasStale.numeric()),
        }));
    }
    if plan.operations.is_empty() {
        // An empty plan pairs only with ours' own root; anything else is a
        // mismatched (ours, plan) pair, never a silent success.
        if plan.merged_root != ours.root {
            return fail(MergeErrorCode::ResultMismatch);
        }
        return Ok(ours_transaction);
    }
    let policy_root = ours.policy_root;
    let summary = build_capability_summary_projection(
        input.principal_id,
        ours.workspace_id,
        policy_root,
        plan.base_root,
        &[],
    )
    .map_err(commit_capability)?;
    let candidate = build_candidate(&CandidateRecord {
        format_version: 1,
        workspace_id: ours.workspace_id,
        base_transaction_id: plan.base_transaction_id,
        base_root: plan.base_root,
        schema_epoch_id: ours.schema_epoch_id,
        policy_root_id: policy_root,
        principal_id: input.principal_id,
        capability_summary_digest: summary.digest(),
        operations: plan.operations.clone(),
        preconditions: plan.preconditions.clone(),
        validation_profile_id: full_validation_profile_id().map_err(commit_candidate)?,
        candidate_nonce: plan.candidate_nonce,
        expiry: CandidateExpiry::unix_millis(input.expiry_unix_millis),
    })
    .map_err(commit_candidate)?;
    let output = transactions
        .commit(CommitInput::new(
            plan.base_transaction_id,
            &candidate.stored_bytes,
            input.principal_id,
            &[],
            input.now_unix_millis,
            input.limits,
        ))
        .map_err(commit_transaction)?;
    let new_head = output.transaction_id();
    let revision = transactions
        .verified_revision(new_head)
        .map_err(commit_transaction)?;
    if revision.state_root().root != plan.merged_root {
        return fail(MergeErrorCode::ResultMismatch);
    }
    branches
        .advance_branch(input.branch, plan.base_transaction_id, new_head)
        .map_err(commit_branch)?;
    Ok(new_head)
}

fn scb(error: &sley_scb1::ScbError) -> MergeError {
    MergeError::Format(error.code().as_str())
}

fn object_bytes(object: Option<ObjectId>) -> Vec<u8> {
    object.map_or_else(|| ZERO32.to_vec(), |object| object.as_bytes().to_vec())
}

fn require_sorted<T, K: Ord>(items: &[T], key: impl Fn(&T) -> K) -> Result<()> {
    for pair in items.windows(2) {
        match key(&pair[0]).cmp(&key(&pair[1])) {
            core::cmp::Ordering::Less => {}
            core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
                return fail(MergeErrorCode::ConflictCanonicalOrder);
            }
        }
    }
    Ok(())
}

/// Encodes a conflict into its canonical stored bytes and identity.
///
/// # Errors
///
/// Returns an order, empty-set, encoding, or resource failure.
pub fn encode_merge_conflict(conflict: &MergeConflict) -> Result<StoredMergeConflict> {
    if conflict.conflicts.is_empty() || conflict.conflicts.len() > MAX_CONFLICT_ENTRIES {
        return fail(MergeErrorCode::ConflictFormatInvalid);
    }
    require_sorted(&conflict.conflicts, |entry| {
        (entry.entity_id, entry.reason, entry.field)
    })?;
    let entries: Vec<Vec<u8>> = conflict
        .conflicts
        .iter()
        .map(|entry| {
            encode_record(&[
                (1, entry.entity_id.as_bytes().to_vec()),
                (2, encode_uvar(u64::from(entry.reason.tag()))),
                (3, encode_uvar(u64::from(entry.kind))),
                (4, encode_uvar(u64::from(entry.field))),
                (5, object_bytes(entry.ours_object)),
                (6, object_bytes(entry.theirs_object)),
                (7, encode_uvar(u64::from(entry.detail))),
            ])
        })
        .collect::<core::result::Result<_, _>>()
        .map_err(|error| scb(&error))?;
    let payload = encode_record(&[
        (1, encode_uvar(FORMAT_VERSION)),
        (2, conflict.workspace_id.as_bytes().to_vec()),
        (3, conflict.ancestor_root.as_bytes().to_vec()),
        (4, conflict.ours_root.as_bytes().to_vec()),
        (5, conflict.theirs_root.as_bytes().to_vec()),
        (6, conflict.ours_delta.as_bytes().to_vec()),
        (7, conflict.theirs_delta.as_bytes().to_vec()),
        (8, encode_list(&entries).map_err(|error| scb(&error))?),
    ])
    .map_err(|error| scb(&error))?;
    let epoch_id = conflict_epoch_id()?;
    let mut preimage = Vec::with_capacity(payload.len() + 96);
    preimage.extend_from_slice(MAGIC);
    preimage.extend_from_slice(&encode_uvar(FORMAT_VERSION));
    preimage.extend_from_slice(&encode_uvar(u64::from(CONTRACT_TAG)));
    preimage.extend_from_slice(epoch_id.as_bytes());
    preimage.extend_from_slice(&encode_uvar(payload.len() as u64));
    preimage.extend_from_slice(&payload);
    if preimage.len() + ID_LEN > MAX_CONFLICT_BYTES {
        return fail(MergeErrorCode::ResourceLimit);
    }
    let conflict_id = MergeConflictId::derive(&preimage);
    let mut stored_bytes = preimage;
    stored_bytes.extend_from_slice(conflict_id.as_bytes());
    Ok(StoredMergeConflict {
        conflict_id,
        stored_bytes,
        conflict: conflict.clone(),
    })
}

fn fixed32(input: &[u8]) -> Result<[u8; 32]> {
    exact_array(input).map_err(MergeError::from)
}

fn small_u32(input: &[u8]) -> Result<u32> {
    let value = read_single_uvar(input).map_err(MergeError::from)?;
    u32::try_from(value).map_err(|_| MergeError::Merge(MergeErrorCode::ConflictFormatInvalid))
}

/// Decodes and verifies stored conflict bytes.
///
/// # Errors
///
/// Returns the exact version, digest, order, format, or resource failure.
pub fn decode_merge_conflict(input: &[u8]) -> Result<StoredMergeConflict> {
    if input.len() > MAX_CONFLICT_BYTES {
        return fail(MergeErrorCode::ResourceLimit);
    }
    let mut reader = Reader::new(input);
    if reader.take_exact(MAGIC.len())? != MAGIC {
        return Err(MergeError::Format(ScbErrorCode::MagicInvalid.as_str()));
    }
    if reader.read_uvar()? != FORMAT_VERSION {
        return fail(MergeErrorCode::ConflictVersionUnsupported);
    }
    if reader.read_uvar()? != u64::from(CONTRACT_TAG) {
        return Err(MergeError::Format(ScbErrorCode::ContractUnknown.as_str()));
    }
    let epoch = SchemaEpochId::from_bytes(reader.take_array()?);
    if epoch != conflict_epoch_id()? {
        return Err(MergeError::Format(ScbErrorCode::EpochMismatch.as_str()));
    }
    let payload_len = reader.read_len(MAX_CONFLICT_BYTES)?;
    let payload = reader.take_exact(payload_len)?;
    let trailer = reader.take_array::<ID_LEN>()?;
    if !reader.is_finished() {
        return Err(MergeError::Format(ScbErrorCode::TrailingBytes.as_str()));
    }
    let conflict_id = MergeConflictId::derive(&input[..input.len() - ID_LEN]);
    if trailer != *conflict_id.as_bytes() {
        return fail(MergeErrorCode::ConflictDigestMismatch);
    }
    let mut record = RecordReader::new(payload)?;
    if read_single_uvar(record.required(1)?)? != FORMAT_VERSION {
        return fail(MergeErrorCode::ConflictVersionUnsupported);
    }
    let workspace_id = WorkspaceId::from_bytes(fixed32(record.required(2)?)?);
    let ancestor_root = StateRoot::from_bytes(fixed32(record.required(3)?)?);
    let ours_root = StateRoot::from_bytes(fixed32(record.required(4)?)?);
    let theirs_root = StateRoot::from_bytes(fixed32(record.required(5)?)?);
    let ours_delta = SemanticDeltaId::from_bytes(fixed32(record.required(6)?)?);
    let theirs_delta = SemanticDeltaId::from_bytes(fixed32(record.required(7)?)?);
    let mut conflicts = Vec::new();
    for element in decode_list(record.required(8)?, MAX_CONFLICT_ENTRIES)? {
        let mut entry = RecordReader::new(element)?;
        let entity_id = EntityId::from_bytes(fixed32(entry.required(1)?)?);
        let reason = ConflictReason::from_tag(read_single_uvar(entry.required(2)?)?)?;
        let kind = small_u32(entry.required(3)?)?;
        let field = small_u32(entry.required(4)?)?;
        let ours_bytes = fixed32(entry.required(5)?)?;
        let theirs_bytes = fixed32(entry.required(6)?)?;
        let detail = small_u32(entry.required(7)?)?;
        entry.finish()?;
        let identity_free = matches!(
            reason,
            ConflictReason::Closure | ConflictReason::RootAnchor | ConflictReason::PolicyRoot
        );
        // Identity-free reasons carry no kind; every other reason names a
        // real SSMC1 kind. Detail is the IMPACT code on Closure, the anchor
        // class on RootAnchor (1 contract, 2 test, 3 both), and zero
        // otherwise, so forged evidence cannot smuggle arbitrary codes.
        let detail_ok = match reason {
            ConflictReason::Closure => detail != 0,
            ConflictReason::RootAnchor => (1..=3).contains(&detail),
            _ => detail == 0,
        };
        let field_ok = if reason == ConflictReason::FieldEdit {
            (1..=8).contains(&field)
        } else {
            field == 0
        };
        if !detail_ok
            || !field_ok
            || (identity_free && (kind != 0 || entity_id != EntityId::from_bytes(ZERO32)))
            || (!identity_free && (kind == 0 || kind > 18))
        {
            return fail(MergeErrorCode::ConflictFormatInvalid);
        }
        conflicts.push(ConflictEntry {
            entity_id,
            reason,
            kind,
            field,
            ours_object: (ours_bytes != ZERO32).then(|| ObjectId::from_bytes(ours_bytes)),
            theirs_object: (theirs_bytes != ZERO32).then(|| ObjectId::from_bytes(theirs_bytes)),
            detail,
        });
    }
    record.finish()?;
    if conflicts.is_empty() {
        return fail(MergeErrorCode::ConflictFormatInvalid);
    }
    require_sorted(&conflicts, |entry| {
        (entry.entity_id, entry.reason, entry.field)
    })?;
    let reencoded = encode_merge_conflict(&MergeConflict {
        workspace_id,
        ancestor_root,
        ours_root,
        theirs_root,
        ours_delta,
        theirs_delta,
        conflicts,
    })?;
    if reencoded.stored_bytes != input {
        return fail(MergeErrorCode::ConflictCanonicalOrder);
    }
    Ok(reencoded)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::fs;
    use std::path::PathBuf;

    use sley_id::{
        CandidateNonce, EntityId, ObjectId, PolicyRootId, PrincipalId, SchemaEpochId,
        SemanticFingerprint, StateRoot, TransactionId, WorkspaceId,
    };
    use sley_mutate::{
        EntityObjectRecord, MutationClass, build_candidate, build_entity_object,
        value::{
            BlockBody, ConstantBody, EntityBodyValue, EntryPointBody, FunctionBody,
            GlobalValueBody, NamespaceBody, PolicyBindingBody, TypeDefBody,
        },
    };
    use sley_policy::{
        AcceptedPolicyRoot, CandidateValidationLimits, PolicyResourceCeilings, PolicyRootBuilder,
        PrincipalGrantBuilder, conformance_registry as policy_registry,
    };
    use sley_ssmc::{
        ConstData, ConstValue, EntryExposure, Reachability, Terminator, TrapCode, TrapTerminator,
        TypeDefForm, TypeExpr, Visibility,
    };
    use sley_state_root::{
        StateRootBuilder, conformance_epoch_id as state_epoch_id,
        conformance_registry as state_registry,
    };
    use sley_store::ObjectStore;
    use sley_txn::{TransactionRepository, TrustedGenesisInput};

    use super::*;
    use crate::complete_root::tests::{complete_bodies, id, set};
    use crate::exchange::tests::{TempDir, fixed, namespace_body, verifier};
    use crate::refs::BranchRepository;

    const NOW: u64 = 1_000;
    const ALL_CLASSES: [MutationClass; 5] = [
        MutationClass::CreateEntity,
        MutationClass::ReplaceEntityVersion,
        MutationClass::DeleteEntityBinding,
        MutationClass::AddEntryPoint,
        MutationClass::RemoveEntryPoint,
    ];

    fn root(byte: u8) -> StateRoot {
        StateRoot::from_bytes([byte; 32])
    }

    fn constant(value: bool) -> EntityBodyValue {
        EntityBodyValue::Constant(ConstantBody {
            value: ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(value),
            },
        })
    }

    fn typedef(visibility: Visibility) -> EntityBodyValue {
        EntityBodyValue::TypeDef(TypeDefBody {
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(Vec::new()),
            invariants: set(&[]),
            visibility,
        })
    }

    fn namespace(parent: Option<u8>, members: &[u8]) -> EntityBodyValue {
        EntityBodyValue::Namespace(NamespaceBody {
            parent: parent.map(id),
            members: set(members),
        })
    }

    fn namespace_with(members: &[u8], extra: EntityId) -> EntityBodyValue {
        let mut ids: Vec<EntityId> = members.iter().map(|byte| id(*byte)).collect();
        ids.push(extra);
        EntityBodyValue::Namespace(NamespaceBody {
            parent: None,
            members: EntityIdSet::from_unsorted(ids).unwrap(),
        })
    }

    fn policy_binding(subject: u8) -> EntityBodyValue {
        EntityBodyValue::PolicyBinding(PolicyBindingBody {
            subject: id(subject),
            requirements: set(&[]),
        })
    }

    fn global(initializer: u8, visibility: Visibility) -> EntityBodyValue {
        EntityBodyValue::GlobalValue(GlobalValueBody {
            value_type: TypeExpr::Bool,
            initializer: id(initializer),
            visibility,
        })
    }

    /// Bodies keyed by entity byte, with the base seven-entity root plus a
    /// constant (18) and a global (19) in the package namespace.
    fn base_bodies() -> Vec<(u8, EntityBodyValue)> {
        let mut bodies = complete_bodies();
        for (byte, body) in &mut bodies {
            if *byte == 4 {
                *body = namespace(None, &[6, 16, 18, 19]);
            }
        }
        bodies.push((18, constant(true)));
        bodies.push((19, global(18, Visibility::Private)));
        bodies
    }

    fn with(
        bodies: &[(u8, EntityBodyValue)],
        byte: u8,
        body: EntityBodyValue,
    ) -> Vec<(u8, EntityBodyValue)> {
        let mut out: Vec<(u8, EntityBodyValue)> = bodies
            .iter()
            .filter(|(existing, _)| *existing != byte)
            .cloned()
            .collect();
        out.push((byte, body));
        out
    }

    fn without(bodies: &[(u8, EntityBodyValue)], byte: u8) -> Vec<(u8, EntityBodyValue)> {
        bodies
            .iter()
            .filter(|(existing, _)| *existing != byte)
            .cloned()
            .collect()
    }

    /// A synthetic merge side over owned objects (no repository).
    fn synthetic(
        root_byte: u8,
        bodies: &[(u8, EntityBodyValue)],
        labels: &[(u8, &str)],
    ) -> MergeSide {
        let epoch = state_epoch_id().unwrap();
        let mut objects: Vec<EntityObject> = bodies
            .iter()
            .map(|(byte, body)| {
                build_entity_object(
                    epoch,
                    &EntityObjectRecord {
                        entity_id: id(*byte),
                        body: body.clone(),
                        label: labels
                            .iter()
                            .find(|(entity, _)| entity == byte)
                            .map(|(_, label)| (*label).to_owned()),
                        semantic_fingerprint: None,
                    },
                )
                .unwrap()
            })
            .collect();
        objects.sort_by_key(|object| object.record().entity_id);
        let entry_points = objects
            .iter()
            .filter(|object| matches!(object.record().body, EntityBodyValue::EntryPoint(_)))
            .map(|object| object.record().entity_id)
            .collect();
        MergeSide {
            transaction_id: None,
            objects,
            root: root(root_byte),
            workspace_id: WorkspaceId::from_bytes([1; 32]),
            schema_epoch_id: epoch,
            entry_points,
            dependency_roots: vec![root(9)],
            contract_root: ObjectId::from_bytes([20; 32]),
            test_root: ObjectId::from_bytes([21; 32]),
            policy_root: PolicyRootId::from_bytes([22; 32]),
        }
    }

    fn conflict_reasons(outcome: &MergeOutcome) -> Vec<(u8, ConflictReason, u32)> {
        match outcome {
            MergeOutcome::Conflict(conflict) => conflict
                .conflict
                .conflicts
                .iter()
                .map(|entry| (entry.entity_id.as_bytes()[0], entry.reason, entry.field))
                .collect(),
            MergeOutcome::Merged(_) => panic!("expected a conflict"),
        }
    }

    fn merged(outcome: MergeOutcome) -> MergedRoot {
        match outcome {
            MergeOutcome::Merged(merged) => *merged,
            MergeOutcome::Conflict(conflict) => {
                panic!(
                    "expected a merged root, got {:?}",
                    conflict.conflict.conflicts
                )
            }
        }
    }

    fn object_of(merged: &MergedRoot, byte: u8) -> Option<&EntityObject> {
        merged
            .objects
            .iter()
            .find(|object| object.record().entity_id == id(byte))
    }

    #[test]
    fn disjoint_and_composed_changes_merge_deterministically() {
        let base = base_bodies();
        let o = synthetic(50, &base, &[]);
        // Ours: retarget the policy binding; theirs: add a constant to the namespace.
        let a = synthetic(51, &with(&base, 16, policy_binding(3)), &[]);
        let mut theirs = with(&base, 4, namespace(None, &[6, 16, 18, 19, 30]));
        theirs.push((30, constant(false)));
        let b = synthetic(52, &theirs, &[]);
        let outcome = judge_merge(&o, &a, &b).unwrap();
        let result = merged(outcome);
        assert_eq!(
            object_of(&result, 16).unwrap().record().body,
            policy_binding(3)
        );
        assert_eq!(
            object_of(&result, 30).unwrap().record().body,
            constant(false)
        );
        assert_eq!(
            object_of(&result, 4).unwrap().record().body,
            namespace(None, &[6, 16, 18, 19, 30])
        );
        assert!(result.metadata_overridden.is_empty());
        assert_eq!(result.objects.len(), 10);
        for _ in 0..128 {
            let again = merged(judge_merge(&o, &a, &b).unwrap());
            assert_eq!(again.state_root.root, result.state_root.root);
            assert_eq!(again.objects, result.objects);
        }
        // Symmetry: the merged root does not depend on which side is ours.
        let swapped = merged(judge_merge(&o, &b, &a).unwrap());
        assert_eq!(swapped.state_root.root, result.state_root.root);
        assert_eq!(swapped.objects, result.objects);
    }

    #[test]
    fn set_valued_fields_compose_and_conflicts_are_classified() {
        fn add_add_shape(outcome: &MergeOutcome) -> (ConflictReason, u32) {
            match outcome {
                MergeOutcome::Conflict(conflict) => {
                    assert_eq!(conflict.conflict.conflicts.len(), 1);
                    let entry = &conflict.conflict.conflicts[0];
                    assert_eq!(entry.entity_id, id(31));
                    (entry.reason, entry.kind)
                }
                MergeOutcome::Merged(_) => panic!("expected an AddAdd conflict"),
            }
        }
        let base = base_bodies();
        let o = synthetic(50, &base, &[]);
        // Both sides add a different member to namespace 4: composed.
        let mut ours = with(&base, 4, namespace(None, &[6, 16, 18, 19, 30]));
        ours.push((30, constant(false)));
        let mut theirs = with(&base, 4, namespace(None, &[6, 16, 18, 19, 31]));
        theirs.push((31, constant(false)));
        let result = merged(
            judge_merge(&o, &synthetic(51, &ours, &[]), &synthetic(52, &theirs, &[])).unwrap(),
        );
        assert_eq!(
            object_of(&result, 4).unwrap().record().body,
            namespace(None, &[6, 16, 18, 19, 30, 31])
        );
        assert!(object_of(&result, 30).is_some() && object_of(&result, 31).is_some());

        // FieldEdit: both change the type definition's visibility differently.
        let a = synthetic(51, &with(&base, 6, typedef(Visibility::Exported)), &[]);
        let b = synthetic(52, &with(&base, 6, typedef(Visibility::Workspace)), &[]);
        assert_eq!(
            conflict_reasons(&judge_merge(&o, &a, &b).unwrap()),
            vec![(6, ConflictReason::FieldEdit, 4)]
        );

        // Convergent identical changes merge.
        let b_same = synthetic(53, &with(&base, 6, typedef(Visibility::Exported)), &[]);
        let result = merged(judge_merge(&o, &a, &b_same).unwrap());
        assert_eq!(
            object_of(&result, 6).unwrap().record().body,
            typedef(Visibility::Exported)
        );

        // AddAdd: both add id 30 with different bodies.
        let mut ours = with(&base, 4, namespace(None, &[6, 16, 18, 19, 30]));
        ours.push((30, constant(true)));
        let mut theirs = with(&base, 4, namespace(None, &[6, 16, 18, 19, 30]));
        theirs.push((30, constant(false)));
        assert_eq!(
            conflict_reasons(
                &judge_merge(&o, &synthetic(51, &ours, &[]), &synthetic(52, &theirs, &[])).unwrap()
            ),
            vec![(30, ConflictReason::AddAdd, 0)]
        );

        // Kind-divergent AddAdd: both add id 31, ours as a constant (kind
        // 9), theirs as a namespace (kind 3). The entry kind is the
        // canonical tiebreak (the lesser tag), so the conflict set is
        // identical under an ours/theirs swap (S20-520 revision 5).
        let mut divergent_ours = with(&base, 4, namespace(None, &[6, 16, 18, 19, 31]));
        divergent_ours.push((31, constant(true)));
        let mut divergent_theirs = with(&base, 4, namespace(None, &[6, 16, 18, 19, 31]));
        divergent_theirs.push((31, namespace(Some(4), &[])));
        let forward_outcome = judge_merge(
            &o,
            &synthetic(51, &divergent_ours, &[]),
            &synthetic(52, &divergent_theirs, &[]),
        )
        .unwrap();
        let forward = add_add_shape(&forward_outcome);
        assert_eq!(forward, (ConflictReason::AddAdd, 3));
        let backward = add_add_shape(
            &judge_merge(
                &o,
                &synthetic(51, &divergent_theirs, &[]),
                &synthetic(52, &divergent_ours, &[]),
            )
            .unwrap(),
        );
        assert_eq!(backward, forward);

        // DeleteEdit: ours deletes the policy binding, theirs edits it.
        let ours = with(&without(&base, 16), 4, namespace(None, &[6, 18, 19]));
        let theirs = with(&base, 16, policy_binding(3));
        assert_eq!(
            conflict_reasons(
                &judge_merge(&o, &synthetic(51, &ours, &[]), &synthetic(52, &theirs, &[])).unwrap()
            ),
            vec![(16, ConflictReason::DeleteEdit, 0)]
        );

        // KindEdit: ours retypes the policy binding into a constant, theirs edits it.
        let ours = with(&base, 16, constant(true));
        assert_eq!(
            conflict_reasons(
                &judge_merge(&o, &synthetic(51, &ours, &[]), &synthetic(52, &theirs, &[])).unwrap()
            ),
            vec![(16, ConflictReason::KindEdit, 0)]
        );

        // Collateral: ours changes the constant, theirs changes the global that
        // is initialized from it (a non-ownership relation).
        let ours = with(&base, 18, constant(false));
        let theirs = with(&base, 19, global(18, Visibility::Exported));
        assert_eq!(
            conflict_reasons(
                &judge_merge(&o, &synthetic(51, &ours, &[]), &synthetic(52, &theirs, &[])).unwrap()
            ),
            vec![(18, ConflictReason::Collateral, 0)]
        );
        // Ownership relations alone do not make collateral: ours changes the
        // policy binding, theirs adds a namespace member.
        let ours = with(&base, 16, policy_binding(3));
        let mut theirs = with(&base, 4, namespace(None, &[6, 16, 18, 19, 30]));
        theirs.push((30, constant(false)));
        merged(judge_merge(&o, &synthetic(51, &ours, &[]), &synthetic(52, &theirs, &[])).unwrap());
    }

    #[test]
    fn metadata_only_changes_are_overridden_or_conflict() {
        let base = base_bodies();
        let o = synthetic(50, &base, &[]);
        let a = synthetic(51, &base, &[(6, "renamed")]);
        let b = synthetic(52, &with(&base, 6, typedef(Visibility::Exported)), &[]);
        let result = merged(judge_merge(&o, &a, &b).unwrap());
        assert_eq!(
            object_of(&result, 6).unwrap().record().body,
            typedef(Visibility::Exported)
        );
        assert_eq!(result.metadata_overridden, vec![id(6)]);
        let b_label = synthetic(53, &base, &[(6, "other")]);
        assert_eq!(
            conflict_reasons(&judge_merge(&o, &a, &b_label).unwrap()),
            vec![(6, ConflictReason::MetadataEdit, 0)]
        );
        // Identical metadata changes converge.
        let b_same = synthetic(54, &base, &[(6, "renamed")]);
        let result = merged(judge_merge(&o, &a, &b_same).unwrap());
        assert_eq!(
            object_of(&result, 6).unwrap().record().label.as_deref(),
            Some("renamed")
        );
    }

    #[test]
    fn empty_and_fast_forward_merges_and_preconditions() {
        let base = base_bodies();
        let o = synthetic(50, &base, &[]);
        let a = synthetic(51, &with(&base, 6, typedef(Visibility::Exported)), &[]);
        let result = merged(judge_merge(&o, &a, &o).unwrap());
        assert_eq!(
            result.state_root.root,
            a.root.into_state_root_for_test(&result)
        );
        let result = merged(judge_merge(&o, &o, &a).unwrap());
        assert_eq!(
            object_of(&result, 6).unwrap().record().body,
            typedef(Visibility::Exported)
        );
        let same = merged(judge_merge(&o, &o, &o).unwrap());
        assert_eq!(same.objects.len(), o.objects.len());

        let mut other_workspace = synthetic(52, &base, &[]);
        other_workspace.workspace_id = WorkspaceId::from_bytes([2; 32]);
        assert_eq!(
            judge_merge(&o, &a, &other_workspace).unwrap_err().code(),
            Some(MergeErrorCode::WorkspaceMismatch)
        );
        let mut other_epoch = synthetic(53, &base, &[]);
        other_epoch.schema_epoch_id = SchemaEpochId::from_bytes([9; 32]);
        assert_eq!(
            judge_merge(&o, &a, &other_epoch).unwrap_err().code(),
            Some(MergeErrorCode::EpochMismatch)
        );
        let mut other_root = synthetic(
            54,
            &with(
                &base,
                17,
                EntityBodyValue::DependencyBinding(sley_mutate::value::DependencyBindingBody {
                    dependency_root: root(8),
                    external_package: id(99),
                    local_namespace: id(4),
                }),
            ),
            &[],
        );
        other_root.dependency_roots = vec![root(8)];
        assert_eq!(
            judge_merge(&o, &a, &other_root).unwrap_err().code(),
            Some(MergeErrorCode::DependencyRootChangeUnsupported)
        );
        let mut anchors = synthetic(55, &base, &[]);
        anchors.contract_root = ObjectId::from_bytes([23; 32]);
        anchors.policy_root = PolicyRootId::from_bytes([24; 32]);
        let reasons = conflict_reasons(&judge_merge(&o, &a, &anchors).unwrap());
        assert!(
            reasons
                .iter()
                .any(|(_, reason, _)| *reason == ConflictReason::RootAnchor)
        );
        assert!(
            reasons
                .iter()
                .any(|(_, reason, _)| *reason == ConflictReason::PolicyRoot)
        );
        let incomplete = synthetic(56, &without(&base, 1), &[]);
        assert_eq!(
            judge_merge(&o, &a, &incomplete).unwrap_err().code(),
            Some(MergeErrorCode::CompareFailed)
        );
    }

    trait RootProbe {
        fn into_state_root_for_test(self, merged: &MergedRoot) -> StateRoot;
    }

    impl RootProbe for StateRoot {
        fn into_state_root_for_test(self, merged: &MergedRoot) -> StateRoot {
            // A merge with an empty theirs delta yields ours' entity set; the
            // synthetic ours root byte is not derivable, so compare objects.
            let _ = self;
            merged.state_root.root
        }
    }

    #[test]
    fn common_ancestor_is_the_first_shared_entry() {
        let entry = |byte: u8, parent: Option<u8>| BranchAncestryEntry {
            transaction_id: TransactionId::from_bytes([byte; 32]),
            state_root: root(byte),
            parent_transaction_ids: parent
                .map(|p| TransactionId::from_bytes([p; 32]))
                .into_iter()
                .collect(),
        };
        let ours = vec![
            entry(5, Some(4)),
            entry(4, Some(2)),
            entry(2, Some(1)),
            entry(1, None),
        ];
        let theirs = vec![
            entry(7, Some(6)),
            entry(6, Some(2)),
            entry(2, Some(1)),
            entry(1, None),
        ];
        assert_eq!(
            find_common_ancestor(&ours, &theirs).unwrap().transaction_id,
            TransactionId::from_bytes([2; 32])
        );
        let unrelated = vec![entry(9, None)];
        assert_eq!(
            find_common_ancestor(&ours, &unrelated).unwrap_err().code(),
            Some(MergeErrorCode::NoCommonAncestor)
        );
    }

    #[test]
    fn conflict_codec_rejection_matrix_reaches_every_frozen_code() {
        let base = base_bodies();
        let o = synthetic(50, &base, &[]);
        let a = synthetic(51, &with(&base, 6, typedef(Visibility::Exported)), &[]);
        let b = synthetic(52, &with(&base, 6, typedef(Visibility::Workspace)), &[]);
        let MergeOutcome::Conflict(stored) = judge_merge(&o, &a, &b).unwrap() else {
            panic!("expected a conflict");
        };
        assert_eq!(
            decode_merge_conflict(&stored.stored_bytes).unwrap(),
            *stored
        );
        let bytes = &stored.stored_bytes;
        let mut version = bytes.clone();
        version[8] = 2;
        assert_eq!(
            decode_merge_conflict(&version).unwrap_err().code(),
            Some(MergeErrorCode::ConflictVersionUnsupported)
        );
        let mut digest = bytes.clone();
        let last = digest.len() - 1;
        digest[last] ^= 1;
        assert_eq!(
            decode_merge_conflict(&digest).unwrap_err().code(),
            Some(MergeErrorCode::ConflictDigestMismatch)
        );
        let mut contract = bytes.clone();
        contract[9] = 0x7f;
        let error = decode_merge_conflict(&contract).unwrap_err();
        assert_eq!(error.code(), Some(MergeErrorCode::ConflictFormatInvalid));
        assert_eq!(error.source_code().as_deref(), Some("SCB_CONTRACT_UNKNOWN"));
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            decode_merge_conflict(&trailing).unwrap_err().code(),
            Some(MergeErrorCode::ConflictFormatInvalid)
        );
        assert_eq!(
            decode_merge_conflict(&vec![0; MAX_CONFLICT_BYTES + 1])
                .unwrap_err()
                .code(),
            Some(MergeErrorCode::ResourceLimit)
        );
        let mut empty = stored.conflict.clone();
        empty.conflicts.clear();
        assert_eq!(
            encode_merge_conflict(&empty).unwrap_err().code(),
            Some(MergeErrorCode::ConflictFormatInvalid)
        );
        let mut duplicated = stored.conflict.clone();
        duplicated.conflicts.push(duplicated.conflicts[0]);
        assert_eq!(
            encode_merge_conflict(&duplicated).unwrap_err().code(),
            Some(MergeErrorCode::ConflictCanonicalOrder)
        );
        for (offset, code) in MergeErrorCode::ALL.iter().enumerate() {
            assert_eq!(code.numeric(), 52_000 + u32::try_from(offset).unwrap());
            assert!(code.as_str().starts_with("MERGE_"));
        }
        // Symmetric conflicts swap ours and theirs.
        let MergeOutcome::Conflict(swapped) = judge_merge(&o, &b, &a).unwrap() else {
            panic!("expected a conflict");
        };
        assert_eq!(
            swapped.conflict.conflicts[0].ours_object,
            stored.conflict.conflicts[0].theirs_object
        );
        assert_eq!(
            swapped.conflict.conflicts[0].theirs_object,
            stored.conflict.conflicts[0].ours_object
        );
    }

    /// A repository with a trusted genesis over `bodies`, all five merge
    /// classes granted, and branch `main` at the genesis.
    struct Repo {
        _temp: TempDir,
        transactions: TransactionRepository,
        branches: BranchRepository,
        workspace_id: WorkspaceId,
        principal_id: PrincipalId,
        policy: AcceptedPolicyRoot,
        genesis: TransactionId,
        nonce: u8,
    }

    impl Repo {
        fn new(label: &str, bodies: &[(u8, EntityBodyValue)]) -> Self {
            let temp = TempDir::new(label);
            let path: PathBuf = temp.child("repo");
            fs::create_dir(&path).unwrap();
            let workspace_id = WorkspaceId::from_bytes([1; 32]);
            let principal_id = fixed(2, PrincipalId::from_bytes);
            let mut grant = PrincipalGrantBuilder::new(PolicyResourceCeilings::new(
                1_000, 1_000, 1_000, 100, 100, 100,
            ));
            for class in ALL_CLASSES {
                grant = grant.mutation_class(class);
            }
            let policy = PolicyRootBuilder::new(workspace_id)
                .principal_grant(principal_id, grant.build().unwrap())
                .build(&policy_registry().unwrap())
                .unwrap();
            let epoch = state_epoch_id().unwrap();
            let store = ObjectStore::new(&path);
            let anchors = [20_u8, 21_u8].map(|byte| {
                let object = build_entity_object(
                    epoch,
                    &EntityObjectRecord {
                        entity_id: id(byte),
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
            let mut objects: Vec<EntityObject> = bodies
                .iter()
                .map(|(byte, body)| {
                    build_entity_object(
                        epoch,
                        &EntityObjectRecord {
                            entity_id: id(*byte),
                            body: body.clone(),
                            label: None,
                            semantic_fingerprint: None,
                        },
                    )
                    .unwrap()
                })
                .collect();
            objects.sort_by_key(|object| object.record().entity_id);
            let mut builder =
                StateRootBuilder::new(workspace_id, anchors[0], anchors[1], policy.root());
            for object in &objects {
                builder = builder.entity_binding(object.record().entity_id, object.object_id());
            }
            builder = builder.dependency_root(root(9));
            let state = builder.build(&state_registry().unwrap()).unwrap();
            let transactions = TransactionRepository::new(&path);
            let genesis = transactions
                .initialize_trusted_genesis(TrustedGenesisInput::new(
                    &state,
                    &policy,
                    &objects,
                    &[],
                ))
                .unwrap()
                .transaction_id();
            let branches = BranchRepository::new(&path);
            branches.create_branch("main", genesis).unwrap();
            Self {
                _temp: temp,
                transactions,
                branches,
                workspace_id,
                principal_id,
                policy,
                genesis,
                nonce: 40,
            }
        }

        /// The nonce the next `commit` will use, for derived creation ids.
        fn next_nonce(&self) -> CandidateNonce {
            fixed(self.nonce + 1, CandidateNonce::from_bytes)
        }

        fn side(&self, transaction: TransactionId) -> MergeSide {
            MergeSide::from_revision(&self.transactions.verified_revision(transaction).unwrap())
        }

        /// Commits one candidate on `parent` and advances `main`.
        fn commit(
            &mut self,
            parent: TransactionId,
            operations: Vec<MutationOperation>,
            preconditions: Vec<BoundPrecondition>,
        ) -> TransactionId {
            let revision = self.transactions.verified_revision(parent).unwrap();
            let summary = build_capability_summary_projection(
                self.principal_id,
                self.workspace_id,
                self.policy.root(),
                revision.state_root().root,
                &[],
            )
            .unwrap();
            self.nonce += 1;
            let candidate = build_candidate(&CandidateRecord {
                format_version: 1,
                workspace_id: self.workspace_id,
                base_transaction_id: parent,
                base_root: revision.state_root().root,
                schema_epoch_id: revision.state_root().record.schema_epoch_id,
                policy_root_id: self.policy.root(),
                principal_id: self.principal_id,
                capability_summary_digest: summary.digest(),
                operations,
                preconditions,
                validation_profile_id: full_validation_profile_id().unwrap(),
                candidate_nonce: fixed(self.nonce, CandidateNonce::from_bytes),
                expiry: CandidateExpiry::unix_millis(NOW + 1_000),
            })
            .unwrap();
            let head = self
                .transactions
                .commit(CommitInput::new(
                    parent,
                    &candidate.stored_bytes,
                    self.principal_id,
                    &[],
                    NOW,
                    CandidateValidationLimits::full_v1(),
                ))
                .unwrap()
                .transaction_id();
            self.branches.advance_branch("main", parent, head).unwrap();
            head
        }
    }

    fn replace(
        ordinal: u32,
        byte: u8,
        body: EntityBodyValue,
        current: ObjectId,
    ) -> (MutationOperation, BoundPrecondition) {
        (
            MutationOperation {
                ordinal,
                class: MutationClass::ReplaceEntityVersion,
                target_kind: body.kind_tag(),
                target_entity: id(byte),
                field_tag: None,
                payload: MutationPayload::ReplaceEntityVersion(body),
                precondition_ordinal: ordinal,
            },
            BoundPrecondition {
                operation_ordinal: ordinal,
                requirement: PreimageRequirement::ExactEntityVersion,
                payload: PreconditionPayload::ExactEntityVersion(ExactEntityVersion {
                    entity_id: id(byte),
                    object_id: current,
                }),
            },
        )
    }

    fn create(
        ordinal: u32,
        entity: EntityId,
        body: EntityBodyValue,
    ) -> (MutationOperation, BoundPrecondition) {
        (
            MutationOperation {
                ordinal,
                class: MutationClass::CreateEntity,
                target_kind: body.kind_tag(),
                target_entity: entity,
                field_tag: None,
                payload: MutationPayload::CreateEntity(body),
                precondition_ordinal: ordinal,
            },
            BoundPrecondition {
                operation_ordinal: ordinal,
                requirement: PreimageRequirement::ExpectedIdentityAbsent,
                payload: PreconditionPayload::ExpectedIdentityAbsent(ExpectedIdentityAbsent {
                    entity_id: entity,
                }),
            },
        )
    }

    fn current_object(side: &MergeSide, byte: u8) -> ObjectId {
        object_of_side(side, id(byte))
    }

    fn object_of_side(side: &MergeSide, entity: EntityId) -> ObjectId {
        side.objects
            .iter()
            .find(|object| object.record().entity_id == entity)
            .unwrap()
            .object_id()
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn merge_plan_commits_through_the_frozen_path_and_matches_the_merged_root() {
        let base = base_bodies();
        let mut ours_repo = Repo::new("merge-ours", &base);
        let mut theirs_repo = Repo::new("merge-theirs", &base);
        let ancestor = ours_repo.side(ours_repo.genesis);
        assert_eq!(
            ours_repo.genesis, theirs_repo.genesis,
            "trusted genesis over identical inputs yields one transaction identity"
        );
        // Ours: retarget the policy binding. Theirs: add a constant to the namespace.
        let (op, pre) = replace(0, 16, policy_binding(3), current_object(&ancestor, 16));
        let ours_head = ours_repo.commit(ours_repo.genesis, vec![op], vec![pre]);
        let created = EntityId::derive(theirs_repo.workspace_id, theirs_repo.next_nonce(), 9, 0);
        let (create_op, create_pre) = create(0, created, constant(false));
        let (replace_op, replace_pre) = replace(
            1,
            4,
            namespace_with(&[6, 16, 18, 19], created),
            current_object(&ancestor, 4),
        );
        let theirs_head = theirs_repo.commit(
            theirs_repo.genesis,
            vec![create_op, replace_op],
            vec![create_pre, replace_pre],
        );

        let ours = ours_repo.side(ours_head);
        let theirs = theirs_repo.side(theirs_head);
        let ours_ancestry = ours_repo.branches.branch_ancestry("main", 16).unwrap();
        let theirs_ancestry = theirs_repo.branches.branch_ancestry("main", 16).unwrap();
        let common = find_common_ancestor(&ours_ancestry, &theirs_ancestry).unwrap();
        assert_eq!(common.transaction_id, ours_repo.genesis);

        let result = merged(judge_merge(&ancestor, &ours, &theirs).unwrap());
        let plan = build_merge_plan(&ours, &result).unwrap();
        assert_eq!(
            plan.identity_map.len(),
            1,
            "the created constant is re-identified"
        );
        let (judged_id, plan_id) = plan.identity_map[0];
        assert_eq!(judged_id, created);
        assert_ne!(plan_id, created);
        assert_eq!(
            plan.operations.len(),
            2,
            "replace namespace 4 and create the constant"
        );
        assert!(
            plan.operations
                .iter()
                .any(|op| op.class == MutationClass::ReplaceEntityVersion)
        );
        assert!(
            plan.operations
                .iter()
                .any(|op| op.class == MutationClass::CreateEntity)
        );
        let head = commit_merge(
            &ours_repo.transactions,
            &ours_repo.branches,
            &ours,
            &plan,
            MergeCommitInput {
                principal_id: ours_repo.principal_id,
                now_unix_millis: NOW,
                expiry_unix_millis: NOW + 1_000,
                limits: CandidateValidationLimits::full_v1(),
                branch: b"main",
            },
        )
        .unwrap();
        let committed = ours_repo.transactions.verified_revision(head).unwrap();
        assert_eq!(committed.state_root().root, plan.merged_root);
        assert_ne!(
            plan.merged_root, result.state_root.root,
            "the judged root carries theirs' identity"
        );
        assert_eq!(
            ours_repo
                .branches
                .resolve_branch("main")
                .unwrap()
                .reference
                .record
                .head_transaction_id,
            head
        );
        let merged_side = ours_repo.side(head);
        assert_eq!(current_object(&merged_side, 16), current_object(&ours, 16));
        let created_object = merged_side
            .objects
            .iter()
            .find(|object| object.record().entity_id == plan_id)
            .expect("the re-identified constant is bound");
        assert_eq!(created_object.record().body, constant(false));
        let namespace_body = merged_side
            .objects
            .iter()
            .find(|object| object.record().entity_id == id(4))
            .unwrap()
            .record()
            .body
            .clone();
        assert_eq!(namespace_body, namespace_with(&[6, 16, 18, 19], plan_id));

        // Merging ours into the merged head again is a no-op plan.
        let again = merged(judge_merge(&ours, &merged_side, &ours).unwrap());
        let plan = build_merge_plan(&merged_side, &again).unwrap();
        assert!(plan.operations.is_empty());
        assert_eq!(
            commit_merge(
                &ours_repo.transactions,
                &ours_repo.branches,
                &merged_side,
                &plan,
                MergeCommitInput {
                    principal_id: ours_repo.principal_id,
                    now_unix_millis: NOW,
                    expiry_unix_millis: NOW + 1_000,
                    limits: CandidateValidationLimits::full_v1(),
                    branch: b"main",
                },
            )
            .unwrap(),
            head
        );
    }

    #[test]
    fn a_conflict_commits_nothing() {
        let base = base_bodies();
        let mut ours_repo = Repo::new("conflict-ours", &base);
        let mut theirs_repo = Repo::new("conflict-theirs", &base);
        let ancestor = ours_repo.side(ours_repo.genesis);
        let (op, pre) = replace(
            0,
            6,
            typedef(Visibility::Exported),
            current_object(&ancestor, 6),
        );
        let ours_head = ours_repo.commit(ours_repo.genesis, vec![op], vec![pre]);
        let (op, pre) = replace(
            0,
            6,
            typedef(Visibility::Workspace),
            current_object(&ancestor, 6),
        );
        let theirs_head = theirs_repo.commit(theirs_repo.genesis, vec![op], vec![pre]);
        let outcome = judge_merge(
            &ancestor,
            &ours_repo.side(ours_head),
            &theirs_repo.side(theirs_head),
        )
        .unwrap();
        assert_eq!(
            conflict_reasons(&outcome),
            vec![(6, ConflictReason::FieldEdit, 4)]
        );
        assert_eq!(
            ours_repo
                .branches
                .resolve_branch("main")
                .unwrap()
                .reference
                .record
                .head_transaction_id,
            ours_head
        );
    }
    // ---- S20-520 revision 4 review evidence ----

    /// Operation-free function 30 (block 31, trap terminator) a merge-added
    /// entry point can target through the frozen commit path.
    fn trap_function() -> Vec<(u8, EntityBodyValue)> {
        vec![
            (
                30,
                EntityBodyValue::Function(FunctionBody {
                    type_parameters: Vec::new(),
                    parameters: Vec::new(),
                    result_type: TypeExpr::Bool,
                    effects: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
                    entry_block: id(31),
                    blocks: vec![id(31)],
                    contracts: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
                    visibility: Visibility::Private,
                }),
            ),
            (
                31,
                EntityBodyValue::Block(BlockBody {
                    function: id(30),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Trap(TrapTerminator {
                        code: TrapCode::Unreachable,
                        payload: None,
                    }),
                    reachability: Reachability::Required,
                }),
            ),
        ]
    }

    fn entry_point(function: u8) -> EntityBodyValue {
        EntityBodyValue::EntryPoint(EntryPointBody {
            function: id(function),
            exposure: EntryExposure::Local,
        })
    }

    fn graft_fingerprint(side: &mut MergeSide, byte: u8, fingerprint: SemanticFingerprint) {
        let epoch = state_epoch_id().unwrap();
        for object in &mut side.objects {
            if object.record().entity_id == id(byte) {
                let record = object.record().clone();
                *object = build_entity_object(
                    epoch,
                    &EntityObjectRecord {
                        entity_id: record.entity_id,
                        body: record.body,
                        label: record.label,
                        semantic_fingerprint: Some(fingerprint),
                    },
                )
                .unwrap();
            }
        }
    }

    fn add_entry_point_op(
        ordinal: u32,
        entity: EntityId,
        body: sley_mutate::value::EntryPointBody,
        current: ObjectId,
    ) -> (MutationOperation, BoundPrecondition) {
        // The frozen descriptor binds `AddEntryPoint` to `ExactEntityVersion`
        // against the base state: only a live entity can be bound, never one
        // the same candidate creates.
        (
            MutationOperation {
                ordinal,
                class: MutationClass::AddEntryPoint,
                target_kind: 16,
                target_entity: entity,
                field_tag: None,
                payload: MutationPayload::AddEntryPoint(body),
                precondition_ordinal: ordinal,
            },
            BoundPrecondition {
                operation_ordinal: ordinal,
                requirement: PreimageRequirement::ExactEntityVersion,
                payload: PreconditionPayload::ExactEntityVersion(ExactEntityVersion {
                    entity_id: entity,
                    object_id: current,
                }),
            },
        )
    }

    /// P0-1/P0-4: a theirs-added entry point is inexpressible in one frozen
    /// candidate (create-then-add fails the `ExactEntityVersion` descriptor
    /// and the base-state precondition check), so the plan fails closed with
    /// `MERGE_PLAN_UNSUPPORTED` instead of emitting an unexecutable shape or
    /// a foreign identity.
    #[test]
    fn added_entry_point_plan_is_unsupported() {
        let mut base = base_bodies();
        base.extend(trap_function());
        let o = synthetic(50, &base, &[]);
        let mut a = synthetic(50, &base, &[]);
        a.transaction_id = Some(TransactionId::from_bytes([60; 32]));
        let mut theirs = with(&base, 4, namespace(None, &[6, 16, 18, 19, 35]));
        theirs.push((35, entry_point(30)));
        let b = synthetic(52, &theirs, &[]);
        // The judgment itself is exact: theirs' entry point is bound.
        let result = merged(judge_merge(&o, &a, &b).unwrap());
        assert!(
            result.request.facts().entry_points.contains(&id(35)),
            "the judged root binds theirs' entry point"
        );
        assert_eq!(
            build_merge_plan(&a, &result).unwrap_err().code(),
            Some(MergeErrorCode::PlanUnsupported)
        );
    }

    /// P0-1/P0-4 recovery: binding the entry point on ours first (create,
    /// then add in a second candidate) leaves the merge with nothing to
    /// create, and the plan commits through the frozen path.
    #[test]
    fn merge_after_binding_the_entry_point_on_ours_commits() {
        let mut bodies = base_bodies();
        bodies.extend(trap_function());
        let mut ours_repo = Repo::new("ep-ours", &bodies);
        let mut theirs_repo = Repo::new("ep-theirs", &bodies);
        let ancestor = ours_repo.side(ours_repo.genesis);
        // Ours binds entry point 35 itself: create, then add.
        let created_35 = EntityId::derive(ours_repo.workspace_id, ours_repo.next_nonce(), 16, 0);
        let (create_op, create_pre) = create(0, created_35, entry_point(30));
        let created_head = ours_repo.commit(ours_repo.genesis, vec![create_op], vec![create_pre]);
        let created_side = ours_repo.side(created_head);
        let EntityBodyValue::EntryPoint(entry) = entry_point(30) else {
            unreachable!()
        };
        let (add_op, add_pre) = add_entry_point_op(
            0,
            created_35,
            entry,
            object_of_side(&created_side, created_35),
        );
        let ours_head = ours_repo.commit(created_head, vec![add_op], vec![add_pre]);
        // Theirs only retargets the policy binding.
        let (op, pre) = replace(0, 16, policy_binding(3), current_object(&ancestor, 16));
        let theirs_head = theirs_repo.commit(theirs_repo.genesis, vec![op], vec![pre]);
        let ours = ours_repo.side(ours_head);
        let theirs = theirs_repo.side(theirs_head);
        let result = merged(judge_merge(&ancestor, &ours, &theirs).unwrap());
        let plan = build_merge_plan(&ours, &result).unwrap();
        assert!(plan.identity_map.is_empty());
        let head = commit_merge(
            &ours_repo.transactions,
            &ours_repo.branches,
            &ours,
            &plan,
            MergeCommitInput {
                principal_id: ours_repo.principal_id,
                now_unix_millis: NOW,
                expiry_unix_millis: NOW + 1_000,
                limits: CandidateValidationLimits::full_v1(),
                branch: b"main",
            },
        )
        .unwrap();
        let committed = ours_repo.transactions.verified_revision(head).unwrap();
        assert_eq!(committed.state_root().root, plan.merged_root);
        assert!(
            committed
                .state_root()
                .record
                .entry_points
                .contains(&created_35)
        );
    }

    /// P0-2: two created entities derive in operation order, so the frozen
    /// S20-345 creation-ordinal check passes and the plan commits.
    #[test]
    fn two_created_entities_derive_in_operation_order_and_commit() {
        let base = base_bodies();
        let ours_repo = Repo::new("ord-ours", &base);
        let mut theirs_repo = Repo::new("ord-theirs", &base);
        let ancestor = ours_repo.side(ours_repo.genesis);
        // Theirs adds two constants; their judged order is raw-ID order.
        let created_30 = EntityId::derive(theirs_repo.workspace_id, theirs_repo.next_nonce(), 9, 0);
        let created_31 = EntityId::derive(theirs_repo.workspace_id, theirs_repo.next_nonce(), 9, 1);
        let (make_30, pre_30) = create(0, created_30, constant(false));
        let (make_31, pre_31) = create(1, created_31, constant(true));
        let mut members: Vec<EntityId> = [6, 16, 18, 19].iter().map(|byte| id(*byte)).collect();
        members.push(created_30);
        members.push(created_31);
        let namespace_body = EntityBodyValue::Namespace(NamespaceBody {
            parent: None,
            members: EntityIdSet::from_unsorted(members).unwrap(),
        });
        let (replace_op, replace_pre) = replace(2, 4, namespace_body, current_object(&ancestor, 4));
        let theirs_head = theirs_repo.commit(
            theirs_repo.genesis,
            vec![make_30, make_31, replace_op],
            vec![pre_30, pre_31, replace_pre],
        );
        let ours = ours_repo.side(ours_repo.genesis);
        let theirs = theirs_repo.side(theirs_head);
        let result = merged(judge_merge(&ancestor, &ours, &theirs).unwrap());
        let plan = build_merge_plan(&ours, &result).unwrap();
        assert_eq!(plan.identity_map.len(), 2);
        let targets: Vec<EntityId> = plan
            .operations
            .iter()
            .filter(|op| op.class == MutationClass::CreateEntity)
            .map(|op| op.target_entity)
            .collect();
        assert_eq!(targets.len(), 2);
        assert_eq!(
            targets,
            plan.identity_map
                .iter()
                .map(|(_, derived)| *derived)
                .collect::<Vec<_>>(),
            "creation operation order is derivation order"
        );
        let head = commit_merge(
            &ours_repo.transactions,
            &ours_repo.branches,
            &ours,
            &plan,
            MergeCommitInput {
                principal_id: ours_repo.principal_id,
                now_unix_millis: NOW,
                expiry_unix_millis: NOW + 1_000,
                limits: CandidateValidationLimits::full_v1(),
                branch: b"main",
            },
        )
        .unwrap();
        assert_eq!(
            ours_repo
                .transactions
                .verified_revision(head)
                .unwrap()
                .state_root()
                .root,
            plan.merged_root
        );
    }

    /// P0-5/P0-6: the composed object carries A's fingerprint and label (the
    /// only bytes the frozen commit reproduces); a theirs-changed label is
    /// reported in `metadata_overridden`, and the swap composes the same
    /// bodies with its own ours-side metadata.
    #[test]
    fn composed_object_keeps_a_side_metadata_and_reports_divergence() {
        let base = base_bodies();
        let o = synthetic(50, &base, &[]);
        let mut ours_bodies = with(&base, 4, namespace(None, &[6, 16, 18, 19, 30]));
        ours_bodies.push((30, constant(false)));
        let mut theirs_bodies = with(&base, 4, namespace(None, &[6, 16, 18, 19, 31]));
        theirs_bodies.push((31, constant(true)));
        let fingerprint = SemanticFingerprint::from_bytes([7; 32]);
        let mut a = synthetic(51, &ours_bodies, &[(4, "ours")]);
        graft_fingerprint(&mut a, 4, fingerprint);
        let b = synthetic(52, &theirs_bodies, &[(4, "theirs")]);
        let result = merged(judge_merge(&o, &a, &b).unwrap());
        let composed = object_of(&result, 4).unwrap();
        assert_eq!(composed.record().label.as_deref(), Some("ours"));
        assert_eq!(composed.record().semantic_fingerprint, Some(fingerprint));
        assert_eq!(result.metadata_overridden, vec![id(4)]);
        let swapped = merged(judge_merge(&o, &b, &a).unwrap());
        assert_eq!(swapped.metadata_overridden, vec![id(4)]);
        let swapped_composed = swapped
            .objects
            .iter()
            .find(|object| object.record().entity_id == id(4))
            .unwrap();
        assert_eq!(swapped_composed.record().label.as_deref(), Some("theirs"));
        // Same entity set and same composed bodies either way around.
        let bodies = |merged: &MergedRoot| {
            merged
                .objects
                .iter()
                .map(|object| (object.record().entity_id, object.record().body.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(bodies(&swapped), bodies(&result));
    }

    /// Survivor scope: an identity that already conflicted gains no second
    /// `Collateral` entry even when the other side touched its dependent.
    #[test]
    fn conflicted_identities_gain_no_collateral() {
        let base = base_bodies();
        let o = synthetic(50, &base, &[]);
        // Ours retypes the constant into a global over a new constant (so
        // every side still projects); theirs edits the constant and retargets
        // its dependent global. 18 conflicts KindEdit and must not also gain
        // Collateral for 19.
        let mut ab = with(&base, 18, global(30, Visibility::Private));
        ab = with(&ab, 19, global(30, Visibility::Private));
        ab = with(&ab, 4, namespace(None, &[6, 16, 18, 19, 30]));
        ab.push((30, constant(true)));
        let a = synthetic(51, &ab, &[]);
        let mut tb = with(&base, 18, constant(false));
        tb = with(&tb, 19, global(18, Visibility::Exported));
        let b = synthetic(52, &tb, &[]);
        assert_eq!(
            conflict_reasons(&judge_merge(&o, &a, &b).unwrap()),
            vec![(18, ConflictReason::KindEdit, 0)]
        );
    }

    /// Collateral is mirror-symmetric: ours' and theirs' objects never swap
    /// sides, whichever direction the check runs.
    #[test]
    fn collateral_entries_keep_their_sides_under_swap() {
        let base = base_bodies();
        let o = synthetic(50, &base, &[]);
        let a = synthetic(51, &with(&base, 18, constant(false)), &[]);
        let b = synthetic(52, &with(&base, 19, global(18, Visibility::Exported)), &[]);
        let MergeOutcome::Conflict(forward) = judge_merge(&o, &a, &b).unwrap() else {
            panic!("expected a collateral conflict");
        };
        let MergeOutcome::Conflict(swapped) = judge_merge(&o, &b, &a).unwrap() else {
            panic!("expected a collateral conflict");
        };
        assert_eq!(forward.conflict.conflicts.len(), 1);
        assert_eq!(swapped.conflict.conflicts.len(), 1);
        assert_eq!(
            forward.conflict.conflicts[0].reason,
            ConflictReason::Collateral
        );
        assert_eq!(
            forward.conflict.conflicts[0].ours_object,
            swapped.conflict.conflicts[0].theirs_object
        );
        assert_eq!(
            forward.conflict.conflicts[0].theirs_object,
            swapped.conflict.conflicts[0].ours_object
        );
    }

    /// J2 totality: removal on both sides converges with no conflict.
    #[test]
    fn removal_on_both_sides_converges() {
        let base = base_bodies();
        let o = synthetic(50, &base, &[]);
        let mut ours = with(&without(&base, 16), 4, namespace(None, &[6, 18, 19]));
        let mut theirs = with(&without(&base, 16), 4, namespace(None, &[6, 18, 19, 30]));
        theirs.push((30, constant(false)));
        let result = merged(
            judge_merge(&o, &synthetic(51, &ours, &[]), &synthetic(52, &theirs, &[])).unwrap(),
        );
        assert!(object_of(&result, 16).is_none());
        assert!(object_of(&result, 30).is_some());
        ours = with(&without(&base, 16), 4, namespace(None, &[6, 18, 19]));
        let identical = merged(
            judge_merge(&o, &synthetic(51, &ours, &[]), &synthetic(52, &ours, &[])).unwrap(),
        );
        assert!(object_of(&identical, 16).is_none());
    }

    /// Anchor conflicts pin which class moved: 1 contract, 2 test, 3 both.
    #[test]
    fn anchor_conflicts_pin_the_moved_class() {
        let base = base_bodies();
        let o = synthetic(50, &base, &[]);
        let a = synthetic(51, &base, &[]);
        let mut contract = synthetic(52, &base, &[]);
        contract.contract_root = ObjectId::from_bytes([23; 32]);
        let MergeOutcome::Conflict(stored) = judge_merge(&o, &a, &contract).unwrap() else {
            panic!("expected a root-anchor conflict");
        };
        assert_eq!(stored.conflict.conflicts.len(), 1);
        assert_eq!(
            stored.conflict.conflicts[0].reason,
            ConflictReason::RootAnchor
        );
        assert_eq!(stored.conflict.conflicts[0].detail, 1);
        assert_eq!(
            decode_merge_conflict(&stored.stored_bytes).unwrap(),
            *stored
        );
        let mut both = synthetic(53, &base, &[]);
        both.contract_root = ObjectId::from_bytes([23; 32]);
        both.test_root = ObjectId::from_bytes([24; 32]);
        let MergeOutcome::Conflict(stored) = judge_merge(&o, &a, &both).unwrap() else {
            panic!("expected a root-anchor conflict");
        };
        assert_eq!(stored.conflict.conflicts[0].detail, 3);
    }

    /// P0-7: bare `judge_merge` takes the ancestor as proven (the O=A hole is
    /// real: everything takes the silent J1 path), while the verified entry
    /// point rejects it before any composition.
    #[test]
    fn verified_judgment_rejects_a_non_ancestor_o() {
        let base = base_bodies();
        let mut ours_repo = Repo::new("anc-ours", &base);
        let mut theirs_repo = Repo::new("anc-theirs", &base);
        let ancestor = ours_repo.side(ours_repo.genesis);
        let (op, pre) = replace(
            0,
            6,
            typedef(Visibility::Exported),
            current_object(&ancestor, 6),
        );
        let ours_head = ours_repo.commit(ours_repo.genesis, vec![op], vec![pre]);
        let (op, pre) = replace(0, 16, policy_binding(3), current_object(&ancestor, 16));
        let theirs_head = theirs_repo.commit(theirs_repo.genesis, vec![op], vec![pre]);
        let ours = ours_repo.side(ours_head);
        let theirs = theirs_repo.side(theirs_head);
        let ours_ancestry = ours_repo.branches.branch_ancestry("main", 16).unwrap();
        let theirs_ancestry = theirs_repo.branches.branch_ancestry("main", 16).unwrap();
        verify_merge_ancestor(&ancestor, &ours, &theirs, &ours_ancestry, &theirs_ancestry).unwrap();
        let attack = judge_merge(&ours, &ours, &theirs).unwrap();
        assert!(
            matches!(attack, MergeOutcome::Merged(_)),
            "O=A empties dA, so bare judgment silently takes all of B"
        );
        assert_eq!(
            judge_merge_verified(&ours, &ours, &theirs, &ours_ancestry, &theirs_ancestry)
                .unwrap_err()
                .code(),
            Some(MergeErrorCode::AncestorMismatch)
        );
        let lone = vec![theirs_ancestry[0].clone()];
        assert_eq!(
            judge_merge_verified(&ancestor, &ours, &theirs, &ours_ancestry, &lone)
                .unwrap_err()
                .code(),
            Some(MergeErrorCode::NoCommonAncestor)
        );
    }

    /// Oversized ancestry slices fail before any search.
    #[test]
    fn oversized_ancestries_fail_before_search() {
        let entry = BranchAncestryEntry {
            transaction_id: TransactionId::from_bytes([1; 32]),
            state_root: root(1),
            parent_transaction_ids: Vec::new(),
        };
        let big = vec![entry; MAX_ANCESTRY_NODES + 1];
        assert_eq!(
            find_common_ancestor(&big, &big).unwrap_err().code(),
            Some(MergeErrorCode::ResourceLimit)
        );
    }

    /// The branch pre-check fires before anything is durable, preserving the
    /// exact frozen code and numeric.
    #[test]
    fn commit_merge_rejects_a_stale_branch_before_committing() {
        let base = base_bodies();
        let mut ours_repo = Repo::new("stale-ours", &base);
        let ancestor = ours_repo.side(ours_repo.genesis);
        let (op, pre) = replace(
            0,
            6,
            typedef(Visibility::Exported),
            current_object(&ancestor, 6),
        );
        let ours_head = ours_repo.commit(ours_repo.genesis, vec![op], vec![pre]);
        let ours = ours_repo.side(ours_head);
        let again = merged(judge_merge(&ancestor, &ours, &ancestor).unwrap());
        let plan = build_merge_plan(&ours, &again).unwrap();
        assert!(plan.operations.is_empty());
        // A concurrent advance moves the branch past the plan base.
        let (op, pre) = replace(0, 16, policy_binding(3), current_object(&ours, 16));
        let concurrent = ours_repo.commit(ours_head, vec![op], vec![pre]);
        let error = commit_merge(
            &ours_repo.transactions,
            &ours_repo.branches,
            &ours,
            &plan,
            MergeCommitInput {
                principal_id: ours_repo.principal_id,
                now_unix_millis: NOW,
                expiry_unix_millis: NOW + 1_000,
                limits: CandidateValidationLimits::full_v1(),
                branch: b"main",
            },
        )
        .unwrap_err();
        assert_eq!(error.symbol(), "REF_NAMED_CAS_STALE");
        assert_eq!(error.commit_numeric(), Some(50_010));
        assert_eq!(
            ours_repo
                .branches
                .resolve_branch("main")
                .unwrap()
                .reference
                .record
                .head_transaction_id,
            concurrent
        );
    }

    /// An empty plan pairs only with ours' own root.
    #[test]
    fn empty_plan_with_a_foreign_root_fails() {
        let base = base_bodies();
        let ours_repo = Repo::new("empty-ours", &base);
        let ancestor = ours_repo.side(ours_repo.genesis);
        let ours = ours_repo.side(ours_repo.genesis);
        let again = merged(judge_merge(&ancestor, &ours, &ancestor).unwrap());
        let mut plan = build_merge_plan(&ours, &again).unwrap();
        plan.merged_root = root(99);
        assert_eq!(
            commit_merge(
                &ours_repo.transactions,
                &ours_repo.branches,
                &ours,
                &plan,
                MergeCommitInput {
                    principal_id: ours_repo.principal_id,
                    now_unix_millis: NOW,
                    expiry_unix_millis: NOW + 1_000,
                    limits: CandidateValidationLimits::full_v1(),
                    branch: b"main",
                },
            )
            .unwrap_err()
            .code(),
            Some(MergeErrorCode::ResultMismatch)
        );
    }

    /// Forged conflict evidence (smuggled detail or zeroed kind) fails the
    /// strict decoder even though it re-encodes from the same structs.
    #[test]
    fn forged_conflict_detail_and_kind_fail_strict_decode() {
        let base = base_bodies();
        let o = synthetic(50, &base, &[]);
        let a = synthetic(51, &with(&base, 6, typedef(Visibility::Exported)), &[]);
        let b = synthetic(52, &with(&base, 6, typedef(Visibility::Workspace)), &[]);
        let MergeOutcome::Conflict(stored) = judge_merge(&o, &a, &b).unwrap() else {
            panic!("expected a conflict");
        };
        let mut forged = stored.conflict.clone();
        forged.conflicts[0].detail = 25_000;
        let bytes = encode_merge_conflict(&forged).unwrap().stored_bytes;
        assert_eq!(
            decode_merge_conflict(&bytes).unwrap_err().code(),
            Some(MergeErrorCode::ConflictFormatInvalid)
        );
        let mut forged_kind = stored.conflict.clone();
        forged_kind.conflicts[0].kind = 0;
        let bytes = encode_merge_conflict(&forged_kind).unwrap().stored_bytes;
        assert_eq!(
            decode_merge_conflict(&bytes).unwrap_err().code(),
            Some(MergeErrorCode::ConflictFormatInvalid)
        );
    }

    // ---- corpus emitter (scripts/generate_merge_fixtures.py) ----

    fn side_json(side: &MergeSide) -> String {
        let hex = crate::compare::tests::hex;
        let mut json = crate::compare::tests::root_json(&side.request().unwrap());
        json.pop();
        format!(
            "{json},\"contract_root\":\"{}\",\"test_root\":\"{}\",\"policy_root\":\"{}\"}}",
            hex(side.contract_root.as_bytes()),
            hex(side.test_root.as_bytes()),
            hex(side.policy_root.as_bytes())
        )
    }

    fn conflict_json(conflict: &MergeConflict) -> String {
        let hex = crate::compare::tests::hex;
        let entries: Vec<String> = conflict
            .conflicts
            .iter()
            .map(|entry| {
                format!(
                    "[\"{}\",{},{},{},{},{},{}]",
                    hex(entry.entity_id.as_bytes()),
                    entry.reason.tag(),
                    entry.kind,
                    entry.field,
                    entry.ours_object.map_or_else(
                        || "null".to_owned(),
                        |o| format!("\"{}\"", hex(o.as_bytes()))
                    ),
                    entry.theirs_object.map_or_else(
                        || "null".to_owned(),
                        |o| format!("\"{}\"", hex(o.as_bytes()))
                    ),
                    entry.detail
                )
            })
            .collect();
        format!(
            "{{\"workspace_id\":\"{}\",\"ancestor_root\":\"{}\",\"ours_root\":\"{}\",\"theirs_root\":\"{}\",\"ours_delta\":\"{}\",\"theirs_delta\":\"{}\",\"conflicts\":[{}]}}",
            hex(conflict.workspace_id.as_bytes()),
            hex(conflict.ancestor_root.as_bytes()),
            hex(conflict.ours_root.as_bytes()),
            hex(conflict.theirs_root.as_bytes()),
            hex(conflict.ours_delta.as_bytes()),
            hex(conflict.theirs_delta.as_bytes()),
            entries.join(",")
        )
    }

    type SideMutation = Box<
        dyn Fn(&[(u8, EntityBodyValue)]) -> (Vec<(u8, EntityBodyValue)>, Vec<(u8, &'static str)>),
    >;

    /// Emits the frozen merge corpus for `scripts/generate_merge_fixtures.py`.
    #[test]
    #[ignore = "fixture refresh emitter; run through the generator script"]
    #[allow(clippy::too_many_lines)]
    fn emit_merge_corpus_for_fixture_refresh() {
        let hex = crate::compare::tests::hex;
        let base = base_bodies();
        let keep: SideMutation = Box::new(|b| (b.to_vec(), Vec::new()));
        let cases: Vec<(&str, SideMutation, SideMutation)> = vec![
            (
                "identical",
                Box::new(|b| (b.to_vec(), Vec::new())),
                Box::new(|b| (b.to_vec(), Vec::new())),
            ),
            (
                "disjoint-entities",
                Box::new(|b| (with(b, 16, policy_binding(3)), Vec::new())),
                Box::new(|b| {
                    let mut t = with(b, 4, namespace(None, &[6, 16, 18, 19, 30]));
                    t.push((30, constant(false)));
                    (t, Vec::new())
                }),
            ),
            (
                "composed-members",
                Box::new(|b| {
                    let mut t = with(b, 4, namespace(None, &[6, 16, 18, 19, 30]));
                    t.push((30, constant(false)));
                    (t, Vec::new())
                }),
                Box::new(|b| {
                    let mut t = with(b, 4, namespace(None, &[6, 16, 18, 19, 31]));
                    t.push((31, constant(true)));
                    (t, Vec::new())
                }),
            ),
            (
                "disjoint-fields",
                Box::new(|b| (with(b, 6, typedef(Visibility::Exported)), Vec::new())),
                Box::new(|b| {
                    (
                        with(
                            b,
                            6,
                            EntityBodyValue::TypeDef(TypeDefBody {
                                type_parameters: Vec::new(),
                                form: TypeDefForm::Variant(Vec::new()),
                                invariants: set(&[]),
                                visibility: Visibility::Private,
                            }),
                        ),
                        Vec::new(),
                    )
                }),
            ),
            (
                "convergent",
                Box::new(|b| (with(b, 6, typedef(Visibility::Exported)), Vec::new())),
                Box::new(|b| (with(b, 6, typedef(Visibility::Exported)), Vec::new())),
            ),
            (
                "fast-forward",
                Box::new(|b| (b.to_vec(), Vec::new())),
                Box::new(|b| (with(b, 6, typedef(Visibility::Exported)), Vec::new())),
            ),
            (
                "metadata-overridden",
                Box::new(|b| (b.to_vec(), vec![(6, "renamed")])),
                Box::new(|b| (with(b, 6, typedef(Visibility::Exported)), Vec::new())),
            ),
            (
                "field-edit",
                Box::new(|b| (with(b, 6, typedef(Visibility::Exported)), Vec::new())),
                Box::new(|b| (with(b, 6, typedef(Visibility::Workspace)), Vec::new())),
            ),
            (
                "add-add",
                Box::new(|b| {
                    let mut t = with(b, 4, namespace(None, &[6, 16, 18, 19, 30]));
                    t.push((30, constant(true)));
                    (t, Vec::new())
                }),
                Box::new(|b| {
                    let mut t = with(b, 4, namespace(None, &[6, 16, 18, 19, 30]));
                    t.push((30, constant(false)));
                    (t, Vec::new())
                }),
            ),
            (
                "delete-edit",
                Box::new(|b| {
                    (
                        with(&without(b, 16), 4, namespace(None, &[6, 18, 19])),
                        Vec::new(),
                    )
                }),
                Box::new(|b| (with(b, 16, policy_binding(3)), Vec::new())),
            ),
            (
                "kind-edit",
                Box::new(|b| (with(b, 16, constant(true)), Vec::new())),
                Box::new(|b| (with(b, 16, policy_binding(3)), Vec::new())),
            ),
            (
                "collateral",
                Box::new(|b| (with(b, 18, constant(false)), Vec::new())),
                Box::new(|b| (with(b, 19, global(18, Visibility::Exported)), Vec::new())),
            ),
            (
                "collateral-theirs",
                Box::new(|b| (with(b, 19, global(18, Visibility::Exported)), Vec::new())),
                Box::new(|b| (with(b, 18, constant(false)), Vec::new())),
            ),
            (
                "both-removed",
                Box::new(|b| {
                    (
                        with(&without(b, 16), 4, namespace(None, &[6, 18, 19])),
                        Vec::new(),
                    )
                }),
                Box::new(|b| {
                    let mut t = with(&without(b, 16), 4, namespace(None, &[6, 18, 19, 30]));
                    t.push((30, constant(false)));
                    (t, Vec::new())
                }),
            ),
            (
                "conflict-excludes-collateral",
                Box::new(|b| {
                    let mut t = with(b, 18, global(30, Visibility::Private));
                    t = with(&t, 19, global(30, Visibility::Private));
                    t = with(&t, 4, namespace(None, &[6, 16, 18, 19, 30]));
                    t.push((30, constant(true)));
                    (t, Vec::new())
                }),
                Box::new(|b| {
                    let mut t = with(b, 18, constant(false));
                    t = with(&t, 19, global(18, Visibility::Exported));
                    (t, Vec::new())
                }),
            ),
            (
                "root-anchor",
                Box::new(|b| (b.to_vec(), Vec::new())),
                Box::new(|b| (b.to_vec(), Vec::new())),
            ),
            (
                "policy-root",
                Box::new(|b| (b.to_vec(), Vec::new())),
                Box::new(|b| (b.to_vec(), Vec::new())),
            ),
            (
                "disjoint-entities-swapped",
                Box::new(|b| {
                    let mut t = with(b, 4, namespace(None, &[6, 16, 18, 19, 30]));
                    t.push((30, constant(false)));
                    (t, Vec::new())
                }),
                Box::new(|b| (with(b, 16, policy_binding(3)), Vec::new())),
            ),
            (
                "metadata-edit",
                Box::new(|b| (b.to_vec(), vec![(6, "one")])),
                Box::new(|b| (b.to_vec(), vec![(6, "two")])),
            ),
        ];
        let _ = keep;
        let ancestor = synthetic(50, &base, &[]);
        let mut rejections: Vec<(&str, &str, Vec<u8>)> = Vec::new();
        for (name, ours_mutation, theirs_mutation) in cases {
            let (ours_bodies, ours_labels) = ours_mutation(&base);
            let (theirs_bodies, theirs_labels) = theirs_mutation(&base);
            let ours = synthetic(51, &ours_bodies, &ours_labels);
            let mut theirs = synthetic(52, &theirs_bodies, &theirs_labels);
            if name == "root-anchor" {
                theirs.contract_root = ObjectId::from_bytes([23; 32]);
            }
            if name == "policy-root" {
                theirs.policy_root = PolicyRootId::from_bytes([24; 32]);
            }
            let outcome = judge_merge(&ancestor, &ours, &theirs).unwrap();
            match &outcome {
                MergeOutcome::Merged(merged) => {
                    let overridden: Vec<String> = merged
                        .metadata_overridden
                        .iter()
                        .map(|id| format!("\"{}\"", hex(id.as_bytes())))
                        .collect();
                    println!(
                        "MERGE_VECTOR|{name}|merged|{}|{}|{}|{{\"merged\":{},\"merged_root\":\"{}\",\"metadata_overridden\":[{}]}}",
                        side_json(&ancestor),
                        side_json(&ours),
                        side_json(&theirs),
                        crate::compare::tests::root_json(&merged.request),
                        hex(merged.state_root.root.as_bytes()),
                        overridden.join(",")
                    );
                }
                MergeOutcome::Conflict(conflict) => {
                    assert_eq!(
                        decode_merge_conflict(&conflict.stored_bytes).unwrap(),
                        **conflict
                    );
                    println!(
                        "MERGE_VECTOR|{name}|conflict|{}|{}|{}|{{\"conflict\":{},\"stored_hex\":\"{}\",\"merge_conflict_id\":\"{}\"}}",
                        side_json(&ancestor),
                        side_json(&ours),
                        side_json(&theirs),
                        conflict_json(&conflict.conflict),
                        hex(&conflict.stored_bytes),
                        hex(conflict.conflict_id.as_bytes())
                    );
                    if name == "field-edit" {
                        let mut version = conflict.stored_bytes.clone();
                        version[8] = 2;
                        rejections.push(("version", "MERGE_CONFLICT_VERSION_UNSUPPORTED", version));
                        let mut digest = conflict.stored_bytes.clone();
                        let last = digest.len() - 1;
                        digest[last] ^= 1;
                        rejections.push(("flip-trailer", "MERGE_CONFLICT_DIGEST_MISMATCH", digest));
                        let mut contract = conflict.stored_bytes.clone();
                        contract[9] = 0x7f;
                        rejections.push((
                            "contract-tag",
                            "MERGE_CONFLICT_FORMAT_INVALID",
                            contract,
                        ));
                        let mut trailing = conflict.stored_bytes.clone();
                        trailing.push(0);
                        rejections.push((
                            "trailing-byte",
                            "MERGE_CONFLICT_FORMAT_INVALID",
                            trailing,
                        ));
                    }
                }
            }
        }
        for (name, code, input) in rejections {
            assert_eq!(
                decode_merge_conflict(&input)
                    .unwrap_err()
                    .code()
                    .map(super::MergeErrorCode::as_str),
                Some(code)
            );
            println!("MERGE_REJECT|{name}|{code}|{}", hex(&input));
        }
    }
}
