//! Semantic Comparison v1 (S20-510, contract
//! `docs/spec/SEMANTIC_COMPARISON_V1.md`, ADR-0027).
//!
//! Compares two complete-root requests of one workspace and schema epoch and
//! emits one canonical semantic-delta record with entity, field, body,
//! relation, and root-set plus collateral sections, derived only from the
//! full S20-250 request, its complete-root index, and the restricted
//! `Function` fingerprint. It defines no merge, conflict, or composition.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use sley_id::{
    EntityId, ObjectId, SchemaEpochId, SemanticDeltaId, SemanticFingerprint, StateRoot, WorkspaceId,
};
use sley_query::{
    ImpactEdge, ImpactEntity, ImpactError, ImpactErrorCode, ImpactIndex, ImpactKind,
    ModeledEntityKind, judge_complete_root,
};
use sley_scb1::{ScbErrorCode, encode_list, encode_record, encode_uvar};
use sley_schema::{ContractDescriptor, EpochLimits, SchemaEpochRecordV1, UnicodeVersion};
use sley_ssmc::{
    Block, FunctionGraph, Operation, Parameter, ParameterRole, TypeDefForm,
    fingerprint::{FingerprintError, FunctionFingerprintInput, fingerprint_function},
};

use crate::{
    CompleteRootRequest, PackError, Reader, RecordReader, decode_list, exact_array,
    read_single_uvar,
};

/// Maximum stored delta bytes.
pub const MAX_DELTA_BYTES: usize = 67_108_864;
/// Maximum entity deltas.
pub const MAX_ENTITY_DELTAS: usize = 131_070;
/// Maximum field deltas.
pub const MAX_FIELD_DELTAS: usize = 1_048_560;
/// Maximum body deltas.
pub const MAX_BODY_DELTAS: usize = 65_535;
/// Maximum relation deltas.
pub const MAX_RELATION_DELTAS: usize = 8_000_000;
/// Maximum identities per root set or collateral set.
pub const MAX_IDENTITY_SET: usize = 131_070;
/// Maximum charged comparison work.
pub const MAX_COMPARE_WORK: u64 = 100_000_000;

const MAGIC: &[u8; 8] = b"SLEYSCB1";
const FORMAT_VERSION: u64 = 1;
const CONTRACT_TAG: u32 = 510;
const DIGEST_DOMAIN_TAG: u32 = 20;
const KIND_TAG: u32 = 510;
const ID_LEN: usize = 32;
const FIELD_COUNT: u64 = 14;
const FIELD_SCHEMA_HASH: [u8; 32] = [
    0x5e, 0x58, 0xf9, 0x8e, 0xcf, 0x6d, 0x7a, 0x50, 0x1f, 0xc4, 0x90, 0x11, 0xaa, 0x58, 0x5e, 0x85,
    0xab, 0xef, 0x93, 0x96, 0xff, 0x38, 0x9b, 0x5b, 0x6b, 0xb7, 0xc7, 0x96, 0xf1, 0x18, 0x73, 0x9c,
];
const DECODER_LIMITS_HASH: [u8; 32] = [
    0xd2, 0x5b, 0xaa, 0x2e, 0xb5, 0xfc, 0xb3, 0x94, 0xfb, 0x7f, 0xcf, 0xdc, 0xa0, 0x93, 0x26, 0xeb,
    0x43, 0x73, 0xa1, 0xcc, 0x6e, 0x79, 0x13, 0x95, 0x48, 0xd1, 0xd9, 0xd0, 0xfb, 0x37, 0xf3, 0x70,
];
const ZERO32: [u8; 32] = [0; 32];

/// Stable S20-510 failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompareErrorCode {
    /// `COMPARE_VERSION_UNSUPPORTED`.
    VersionUnsupported,
    /// `COMPARE_DIGEST_MISMATCH`.
    DigestMismatch,
    /// `COMPARE_CANONICAL_ORDER`.
    CanonicalOrder,
    /// `COMPARE_DUPLICATE_ENTRY`.
    DuplicateEntry,
    /// `COMPARE_FORMAT_INVALID`.
    FormatInvalid,
    /// `COMPARE_WORKSPACE_MISMATCH`.
    WorkspaceMismatch,
    /// `COMPARE_EPOCH_MISMATCH`.
    EpochMismatch,
    /// `COMPARE_ROOT_INCOMPLETE`.
    RootIncomplete,
    /// `COMPARE_INVENTORY_INVALID`.
    InventoryInvalid,
    /// `COMPARE_RESOURCE_LIMIT`.
    ResourceLimit,
    /// `COMPARE_INTERNAL_INVARIANT` (reserved).
    InternalInvariant,
}

impl CompareErrorCode {
    /// Every code in numeric order.
    pub const ALL: [Self; 11] = [
        Self::VersionUnsupported,
        Self::DigestMismatch,
        Self::CanonicalOrder,
        Self::DuplicateEntry,
        Self::FormatInvalid,
        Self::WorkspaceMismatch,
        Self::EpochMismatch,
        Self::RootIncomplete,
        Self::InventoryInvalid,
        Self::ResourceLimit,
        Self::InternalInvariant,
    ];

    /// Returns the stable symbolic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VersionUnsupported => "COMPARE_VERSION_UNSUPPORTED",
            Self::DigestMismatch => "COMPARE_DIGEST_MISMATCH",
            Self::CanonicalOrder => "COMPARE_CANONICAL_ORDER",
            Self::DuplicateEntry => "COMPARE_DUPLICATE_ENTRY",
            Self::FormatInvalid => "COMPARE_FORMAT_INVALID",
            Self::WorkspaceMismatch => "COMPARE_WORKSPACE_MISMATCH",
            Self::EpochMismatch => "COMPARE_EPOCH_MISMATCH",
            Self::RootIncomplete => "COMPARE_ROOT_INCOMPLETE",
            Self::InventoryInvalid => "COMPARE_INVENTORY_INVALID",
            Self::ResourceLimit => "COMPARE_RESOURCE_LIMIT",
            Self::InternalInvariant => "COMPARE_INTERNAL_INVARIANT",
        }
    }

    /// Returns the stable numeric code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::VersionUnsupported => 51_000,
            Self::DigestMismatch => 51_001,
            Self::CanonicalOrder => 51_002,
            Self::DuplicateEntry => 51_003,
            Self::FormatInvalid => 51_004,
            Self::WorkspaceMismatch => 51_005,
            Self::EpochMismatch => 51_006,
            Self::RootIncomplete => 51_007,
            Self::InventoryInvalid => 51_008,
            Self::ResourceLimit => 51_009,
            Self::InternalInvariant => 51_010,
        }
    }
}

/// One stable comparison failure with any wrapped lower-layer code preserved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompareError {
    /// A plain S20-510 failure.
    Compare(CompareErrorCode),
    /// `COMPARE_FORMAT_INVALID` wrapping the exact SCB1 or pack code.
    Format(&'static str),
    /// `COMPARE_ROOT_INCOMPLETE` wrapping the exact S20-250 failure.
    Impact(ImpactError),
    /// `COMPARE_INVENTORY_INVALID` wrapping the exact fingerprint failure.
    Fingerprint(FingerprintError),
}

impl CompareError {
    /// Returns the stable S20-510 code.
    #[must_use]
    pub const fn code(&self) -> CompareErrorCode {
        match self {
            Self::Compare(code) => *code,
            Self::Format(_) => CompareErrorCode::FormatInvalid,
            Self::Impact(_) => CompareErrorCode::RootIncomplete,
            Self::Fingerprint(_) => CompareErrorCode::InventoryInvalid,
        }
    }

    /// Returns the exact wrapped source code, if any.
    #[must_use]
    pub fn source_code(&self) -> Option<String> {
        match self {
            Self::Compare(_) => None,
            Self::Format(code) => Some((*code).to_owned()),
            Self::Impact(error) => Some(error.code().as_str().to_owned()),
            Self::Fingerprint(error) => Some(error.code().to_string()),
        }
    }
}

impl fmt::Display for CompareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code().as_str())
    }
}

impl std::error::Error for CompareError {}

type Result<T> = core::result::Result<T, CompareError>;

fn fail<T>(code: CompareErrorCode) -> Result<T> {
    Err(CompareError::Compare(code))
}

impl From<PackError> for CompareError {
    fn from(error: PackError) -> Self {
        Self::Format(error.symbol())
    }
}

/// Frozen change class of one bound identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ChangeClass {
    /// Bound only by the target.
    Added,
    /// Bound only by the base.
    Removed,
    /// Bound by both with the same kind and different canonical bodies.
    Changed,
    /// Bound by both with different kinds.
    Retyped,
    /// Bound by both with the same body and a different object.
    MetadataOnly,
}

impl ChangeClass {
    /// Returns the frozen tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Added => 1,
            Self::Removed => 2,
            Self::Changed => 3,
            Self::Retyped => 4,
            Self::MetadataOnly => 5,
        }
    }

    fn from_tag(tag: u64) -> Result<Self> {
        match tag {
            1 => Ok(Self::Added),
            2 => Ok(Self::Removed),
            3 => Ok(Self::Changed),
            4 => Ok(Self::Retyped),
            5 => Ok(Self::MetadataOnly),
            _ => fail(CompareErrorCode::FormatInvalid),
        }
    }
}

/// Section 1 entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityDelta {
    /// Classified identity.
    pub entity_id: EntityId,
    /// Change class.
    pub change: ChangeClass,
    /// Base kind tag, `0` when added.
    pub base_kind: u32,
    /// Target kind tag, `0` when removed.
    pub target_kind: u32,
    /// Base object, `None` when added.
    pub base_object: Option<ObjectId>,
    /// Target object, `None` when removed.
    pub target_object: Option<ObjectId>,
}

/// Section 2 entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDelta {
    /// Changed entity.
    pub entity_id: EntityId,
    /// SSMC1 kind tag.
    pub kind: u32,
    /// SSMC1 body field tag.
    pub field: u32,
    /// Bit 0 always set; further bits per the contract's field table.
    pub flags: u32,
    /// Identities that entered the field.
    pub added: Vec<EntityId>,
    /// Identities that left the field.
    pub removed: Vec<EntityId>,
}

/// Section 3 entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyDelta {
    /// Function whose restricted fingerprint differs.
    pub entity_id: EntityId,
    /// Base fingerprint.
    pub base_fingerprint: SemanticFingerprint,
    /// Target fingerprint.
    pub target_fingerprint: SemanticFingerprint,
    /// Base owned block count.
    pub base_blocks: u32,
    /// Target owned block count.
    pub target_blocks: u32,
    /// Base owned operation count.
    pub base_operations: u32,
    /// Target owned operation count.
    pub target_operations: u32,
}

/// Direction of a relation delta.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RelationChange {
    /// Present only in the target index.
    Added,
    /// Present only in the base index.
    Removed,
}

impl RelationChange {
    /// Returns the frozen tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Added => 1,
            Self::Removed => 2,
        }
    }
}

/// Section 4 entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationDelta {
    /// Referring entity.
    pub dependent: EntityId,
    /// Referenced entity.
    pub dependency: EntityId,
    /// Frozen edge kind.
    pub kind: ImpactKind,
    /// Direction.
    pub change: RelationChange,
}

/// Complete typed delta between two complete roots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDelta {
    /// Shared workspace.
    pub workspace_id: WorkspaceId,
    /// Shared schema epoch of both roots.
    pub root_schema_epoch: SchemaEpochId,
    /// Base root.
    pub base_root: StateRoot,
    /// Target root.
    pub target_root: StateRoot,
    /// Section 1.
    pub entities: Vec<EntityDelta>,
    /// Section 2.
    pub fields: Vec<FieldDelta>,
    /// Section 3.
    pub bodies: Vec<BodyDelta>,
    /// Section 4.
    pub relations: Vec<RelationDelta>,
    /// Section 5: dependency roots only in the target.
    pub dependency_roots_added: Vec<StateRoot>,
    /// Section 5: dependency roots only in the base.
    pub dependency_roots_removed: Vec<StateRoot>,
    /// Section 5: entry points only in the target.
    pub entry_points_added: Vec<EntityId>,
    /// Section 5: entry points only in the base.
    pub entry_points_removed: Vec<EntityId>,
    /// Section 5: unchanged entities that transitively depend on a change.
    pub collateral: Vec<EntityId>,
}

impl SemanticDelta {
    /// Returns whether the two roots compared equal.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
            && self.fields.is_empty()
            && self.bodies.is_empty()
            && self.relations.is_empty()
            && self.dependency_roots_added.is_empty()
            && self.dependency_roots_removed.is_empty()
            && self.entry_points_added.is_empty()
            && self.entry_points_removed.is_empty()
            && self.collateral.is_empty()
    }
}

/// Canonical delta bytes with their identity and decoded form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredSemanticDelta {
    /// Derived identity.
    pub delta_id: SemanticDeltaId,
    /// Exact stored bytes including the trailer.
    pub stored_bytes: Vec<u8>,
    /// Decoded delta.
    pub delta: SemanticDelta,
}

/// Returns the standalone delta schema epoch record.
#[must_use]
pub fn delta_epoch_record() -> SchemaEpochRecordV1 {
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
            required_fields: (1..=14).collect(),
            optional_fields: Vec::new(),
            variant_tags: Vec::new(),
            decoder_limits_hash: DECODER_LIMITS_HASH,
        }],
        extensions: Vec::new(),
        predecessor: None,
        migration_contracts: Vec::new(),
    }
}

/// Returns the exact delta schema epoch identity.
///
/// # Errors
///
/// Returns `COMPARE_INTERNAL_INVARIANT` if the descriptor record drifts.
pub fn delta_epoch_id() -> Result<SchemaEpochId> {
    delta_epoch_record()
        .schema_epoch_id()
        .map_err(|_| CompareError::Compare(CompareErrorCode::InternalInvariant))
}

struct Work(u64);

impl Work {
    fn charge(&mut self, amount: u64) -> Result<()> {
        self.0 = self
            .0
            .checked_add(amount)
            .ok_or(CompareError::Compare(CompareErrorCode::ResourceLimit))?;
        if self.0 > MAX_COMPARE_WORK {
            return fail(CompareErrorCode::ResourceLimit);
        }
        Ok(())
    }
}

struct Side<'a> {
    entities: BTreeMap<EntityId, ImpactEntity<'a>>,
    objects: BTreeMap<EntityId, ObjectId>,
    parameters: BTreeMap<EntityId, &'a Parameter>,
    index: ImpactIndex,
}

impl<'a> Side<'a> {
    fn build(request: &'a CompleteRootRequest, borrowed: &[ImpactEntity<'a>]) -> Result<Self> {
        let index = judge_complete_root(borrowed, request.facts())
            .map_err(CompareError::Impact)?
            .into_index();
        Ok(Self {
            entities: borrowed
                .iter()
                .map(|entity| (entity.entity_id(), *entity))
                .collect(),
            objects: request.bound_objects().iter().copied().collect(),
            parameters: request
                .entities()
                .parameters
                .iter()
                .map(|parameter| (parameter.entity_id, parameter))
                .collect(),
            index,
        })
    }
}

fn owned_inventory(
    request: &CompleteRootRequest,
    function: &FunctionGraph,
) -> (Vec<Parameter>, Vec<Block>, Vec<Operation>) {
    let blocks: BTreeSet<EntityId> = function.blocks.iter().copied().collect();
    let parameters = request
        .entities()
        .parameters
        .iter()
        .filter(|parameter| match parameter.role {
            ParameterRole::Function => parameter.owner == function.entity_id,
            ParameterRole::Block => blocks.contains(&parameter.owner),
        })
        .cloned()
        .collect();
    let owned_blocks = request
        .entities()
        .blocks
        .iter()
        .filter(|block| block.function == function.entity_id)
        .cloned()
        .collect();
    let operations = request
        .entities()
        .operations
        .iter()
        .filter(|operation| blocks.contains(&operation.block))
        .cloned()
        .collect();
    (parameters, owned_blocks, operations)
}

/// Compares two complete-root requests and returns the canonical delta.
///
/// # Errors
///
/// Returns the first precondition, resource, or encoding failure with any
/// wrapped `IMPACT_*` or `FINGERPRINT_*` code preserved.
#[allow(clippy::too_many_lines)]
pub fn compare_complete_roots(
    base: &CompleteRootRequest,
    target: &CompleteRootRequest,
) -> Result<StoredSemanticDelta> {
    let base_borrowed = base.borrowed();
    let target_borrowed = target.borrowed();
    let base_side = Side::build(base, &base_borrowed)?;
    let target_side = Side::build(target, &target_borrowed)?;
    if base.workspace_id() != target.workspace_id() {
        return fail(CompareErrorCode::WorkspaceMismatch);
    }
    if base.schema_epoch_id() != target.schema_epoch_id() {
        return fail(CompareErrorCode::EpochMismatch);
    }
    let mut work = Work(0);

    // Section 1: classification over the union of bound identities.
    let mut entities = Vec::new();
    let mut fields = Vec::new();
    let mut changed_or_retyped = BTreeSet::new();
    let mut has_entity_delta = BTreeSet::new();
    let union: BTreeSet<EntityId> = base_side
        .entities
        .keys()
        .chain(target_side.entities.keys())
        .copied()
        .collect();
    for id in &union {
        work.charge(1)?;
        let base_entity = base_side.entities.get(id).copied();
        let target_entity = target_side.entities.get(id).copied();
        let delta = match (base_entity, target_entity) {
            (None, Some(entity)) => EntityDelta {
                entity_id: *id,
                change: ChangeClass::Added,
                base_kind: 0,
                target_kind: entity.kind().tag(),
                base_object: None,
                target_object: target_side.objects.get(id).copied(),
            },
            (Some(entity), None) => EntityDelta {
                entity_id: *id,
                change: ChangeClass::Removed,
                base_kind: entity.kind().tag(),
                target_kind: 0,
                base_object: base_side.objects.get(id).copied(),
                target_object: None,
            },
            (Some(before), Some(after)) => {
                let base_object = base_side.objects.get(id).copied();
                let target_object = target_side.objects.get(id).copied();
                if base_object == target_object {
                    continue;
                }
                let change = if before.kind() != after.kind() {
                    ChangeClass::Retyped
                } else if definitions_equal(before, after) {
                    ChangeClass::MetadataOnly
                } else {
                    ChangeClass::Changed
                };
                if change == ChangeClass::Changed {
                    field_deltas(
                        &mut fields,
                        before,
                        after,
                        &base_side.parameters,
                        &target_side.parameters,
                        &mut work,
                    )?;
                }
                EntityDelta {
                    entity_id: *id,
                    change,
                    base_kind: before.kind().tag(),
                    target_kind: after.kind().tag(),
                    base_object,
                    target_object,
                }
            }
            (None, None) => return fail(CompareErrorCode::InternalInvariant),
        };
        if matches!(delta.change, ChangeClass::Changed | ChangeClass::Retyped) {
            changed_or_retyped.insert(*id);
        }
        has_entity_delta.insert(*id);
        entities.push(delta);
    }
    if entities.len() > MAX_ENTITY_DELTAS || fields.len() > MAX_FIELD_DELTAS {
        return fail(CompareErrorCode::ResourceLimit);
    }

    // Section 3: restricted fingerprints over each root's owned inventory.
    let mut bodies = Vec::new();
    let mut body_seeds = BTreeSet::new();
    for (id, entity) in &base_side.entities {
        let (ImpactEntity::Function(before), Some(ImpactEntity::Function(after))) =
            (*entity, target_side.entities.get(id).copied())
        else {
            continue;
        };
        let (base_parameters, base_blocks, base_operations) = owned_inventory(base, before);
        let (target_parameters, target_blocks, target_operations) = owned_inventory(target, after);
        work.charge(
            (base_parameters.len() + base_blocks.len() + base_operations.len()) as u64
                + (target_parameters.len() + target_blocks.len() + target_operations.len()) as u64
                + 2,
        )?;
        let base_fingerprint = fingerprint_function(
            base.schema_epoch_id(),
            FunctionFingerprintInput {
                function: before,
                parameters: &base_parameters,
                blocks: &base_blocks,
                operations: &base_operations,
            },
        )
        .map_err(CompareError::Fingerprint)?;
        let target_fingerprint = fingerprint_function(
            target.schema_epoch_id(),
            FunctionFingerprintInput {
                function: after,
                parameters: &target_parameters,
                blocks: &target_blocks,
                operations: &target_operations,
            },
        )
        .map_err(CompareError::Fingerprint)?;
        if base_fingerprint != target_fingerprint {
            body_seeds.insert(*id);
            bodies.push(BodyDelta {
                entity_id: *id,
                base_fingerprint,
                target_fingerprint,
                base_blocks: u32::try_from(base_blocks.len())
                    .map_err(|_| CompareError::Compare(CompareErrorCode::ResourceLimit))?,
                target_blocks: u32::try_from(target_blocks.len())
                    .map_err(|_| CompareError::Compare(CompareErrorCode::ResourceLimit))?,
                base_operations: u32::try_from(base_operations.len())
                    .map_err(|_| CompareError::Compare(CompareErrorCode::ResourceLimit))?,
                target_operations: u32::try_from(target_operations.len())
                    .map_err(|_| CompareError::Compare(CompareErrorCode::ResourceLimit))?,
            });
        }
    }
    if bodies.len() > MAX_BODY_DELTAS {
        return fail(CompareErrorCode::ResourceLimit);
    }

    // Section 4: symmetric difference of the two direct edge sets.
    let relations = relation_deltas(
        base_side.index.direct_edges(),
        target_side.index.direct_edges(),
        &mut work,
    )?;
    if relations.len() > MAX_RELATION_DELTAS {
        return fail(CompareErrorCode::ResourceLimit);
    }

    // Section 5: root sets and collateral.
    let (dependency_roots_added, dependency_roots_removed) = set_difference(
        base.facts().dependency_roots,
        target.facts().dependency_roots,
    );
    let (entry_points_added, entry_points_removed) =
        set_difference(base.facts().entry_points, target.facts().entry_points);
    let mut seeds_base: BTreeSet<EntityId> = entities
        .iter()
        .filter(|delta| {
            matches!(
                delta.change,
                ChangeClass::Removed | ChangeClass::Changed | ChangeClass::Retyped
            )
        })
        .map(|delta| delta.entity_id)
        .collect();
    let mut seeds_target: BTreeSet<EntityId> = entities
        .iter()
        .filter(|delta| {
            matches!(
                delta.change,
                ChangeClass::Added | ChangeClass::Changed | ChangeClass::Retyped
            )
        })
        .map(|delta| delta.entity_id)
        .collect();
    for seed in &body_seeds {
        seeds_base.insert(*seed);
        seeds_target.insert(*seed);
    }
    let _ = &changed_or_retyped;
    let mut reached = BTreeSet::new();
    for (side, seeds) in [(&base_side, &seeds_base), (&target_side, &seeds_target)] {
        if seeds.is_empty() {
            continue;
        }
        let seed_list: Vec<EntityId> = seeds.iter().copied().collect();
        work.charge(seed_list.len() as u64)?;
        let closure = side.index.transitive_impact(&seed_list).map_err(|error| {
            if error.code() == ImpactErrorCode::ResourceLimit {
                CompareError::Compare(CompareErrorCode::ResourceLimit)
            } else {
                CompareError::Impact(error)
            }
        })?;
        work.charge(closure.len() as u64)?;
        reached.extend(closure);
    }
    let collateral: Vec<EntityId> = reached
        .into_iter()
        .filter(|id| {
            !has_entity_delta.contains(id)
                && base_side.entities.contains_key(id)
                && target_side.entities.contains_key(id)
        })
        .collect();
    if collateral.len() > MAX_IDENTITY_SET
        || dependency_roots_added.len() > MAX_IDENTITY_SET
        || dependency_roots_removed.len() > MAX_IDENTITY_SET
        || entry_points_added.len() > MAX_IDENTITY_SET
        || entry_points_removed.len() > MAX_IDENTITY_SET
    {
        return fail(CompareErrorCode::ResourceLimit);
    }

    encode_semantic_delta(&SemanticDelta {
        workspace_id: base.workspace_id(),
        root_schema_epoch: base.schema_epoch_id(),
        base_root: base.root(),
        target_root: target.root(),
        entities,
        fields,
        bodies,
        relations,
        dependency_roots_added,
        dependency_roots_removed,
        entry_points_added,
        entry_points_removed,
        collateral,
    })
}

fn definitions_equal(before: ImpactEntity<'_>, after: ImpactEntity<'_>) -> bool {
    match (before, after) {
        (ImpactEntity::Workspace(a), ImpactEntity::Workspace(b)) => a == b,
        (ImpactEntity::Package(a), ImpactEntity::Package(b)) => a == b,
        (ImpactEntity::Namespace(a), ImpactEntity::Namespace(b)) => a == b,
        (ImpactEntity::TypeDef(a), ImpactEntity::TypeDef(b)) => a == b,
        (ImpactEntity::Function(a), ImpactEntity::Function(b)) => a == b,
        (ImpactEntity::Parameter(a), ImpactEntity::Parameter(b)) => a == b,
        (ImpactEntity::Block(a), ImpactEntity::Block(b)) => a == b,
        (ImpactEntity::Operation(a), ImpactEntity::Operation(b)) => a == b,
        (ImpactEntity::Constant(a), ImpactEntity::Constant(b)) => a == b,
        (ImpactEntity::GlobalValue(a), ImpactEntity::GlobalValue(b)) => a == b,
        (ImpactEntity::EffectDef(a), ImpactEntity::EffectDef(b)) => a == b,
        (ImpactEntity::CapabilityRequirement(a), ImpactEntity::CapabilityRequirement(b)) => a == b,
        (ImpactEntity::Contract(a), ImpactEntity::Contract(b)) => a == b,
        (ImpactEntity::TestCase(a), ImpactEntity::TestCase(b)) => a == b,
        (ImpactEntity::AdapterImport(a), ImpactEntity::AdapterImport(b)) => a == b,
        (ImpactEntity::EntryPoint(a), ImpactEntity::EntryPoint(b)) => a == b,
        (ImpactEntity::PolicyBinding(a), ImpactEntity::PolicyBinding(b)) => a == b,
        (ImpactEntity::DependencyBinding(a), ImpactEntity::DependencyBinding(b)) => a == b,
        _ => false,
    }
}

fn set_difference<T: Ord + Copy>(base: &[T], target: &[T]) -> (Vec<T>, Vec<T>) {
    let base_set: BTreeSet<T> = base.iter().copied().collect();
    let target_set: BTreeSet<T> = target.iter().copied().collect();
    (
        target_set.difference(&base_set).copied().collect(),
        base_set.difference(&target_set).copied().collect(),
    )
}

fn push_field(
    fields: &mut Vec<FieldDelta>,
    entity_id: EntityId,
    kind: ModeledEntityKind,
    field: u32,
    extra_flags: u32,
    base: &[EntityId],
    target: &[EntityId],
) {
    let (added, removed) = set_difference(base, target);
    fields.push(FieldDelta {
        entity_id,
        kind: kind.tag(),
        field,
        flags: 1 | extra_flags,
        added,
        removed,
    });
}

fn scalar_field<T: PartialEq>(
    fields: &mut Vec<FieldDelta>,
    entity_id: EntityId,
    kind: ModeledEntityKind,
    field: u32,
    base: &T,
    target: &T,
) {
    if base != target {
        push_field(fields, entity_id, kind, field, 0, &[], &[]);
    }
}

fn set_field(
    fields: &mut Vec<FieldDelta>,
    entity_id: EntityId,
    kind: ModeledEntityKind,
    field: u32,
    base: &[EntityId],
    target: &[EntityId],
) {
    if base != target {
        push_field(fields, entity_id, kind, field, 0, base, target);
    }
}

#[allow(clippy::too_many_lines)]
fn field_deltas(
    fields: &mut Vec<FieldDelta>,
    before: ImpactEntity<'_>,
    after: ImpactEntity<'_>,
    base_parameters: &BTreeMap<EntityId, &Parameter>,
    target_parameters: &BTreeMap<EntityId, &Parameter>,
    work: &mut Work,
) -> Result<()> {
    let id = before.entity_id();
    let kind = before.kind();
    work.charge(8)?;
    match (before, after) {
        (ImpactEntity::Workspace(a), ImpactEntity::Workspace(b)) => {
            set_field(fields, id, kind, 1, &a.packages, &b.packages);
            scalar_field(fields, id, kind, 2, &a.root_namespace, &b.root_namespace);
            set_field(
                fields,
                id,
                kind,
                3,
                &a.capability_requirements,
                &b.capability_requirements,
            );
            set_field(fields, id, kind, 4, &a.contracts, &b.contracts);
            set_field(fields, id, kind, 5, &a.tests, &b.tests);
        }
        (ImpactEntity::Package(a), ImpactEntity::Package(b)) => {
            scalar_field(fields, id, kind, 1, &a.workspace, &b.workspace);
            scalar_field(fields, id, kind, 2, &a.root_namespace, &b.root_namespace);
            set_field(fields, id, kind, 3, &a.dependencies, &b.dependencies);
            set_field(fields, id, kind, 4, &a.exports, &b.exports);
        }
        (ImpactEntity::Namespace(a), ImpactEntity::Namespace(b)) => {
            scalar_field(fields, id, kind, 1, &a.parent, &b.parent);
            set_field(fields, id, kind, 2, &a.members, &b.members);
        }
        (ImpactEntity::TypeDef(a), ImpactEntity::TypeDef(b)) => {
            scalar_field(fields, id, kind, 1, &a.type_parameters, &b.type_parameters);
            if a.form != b.form {
                let mut flags = 0;
                let (base_members, target_members): (Vec<[u8; 32]>, Vec<[u8; 32]>) =
                    match (&a.form, &b.form) {
                        (TypeDefForm::Record(x), TypeDefForm::Record(y)) => (
                            x.iter().map(|field| *field.member_id.as_bytes()).collect(),
                            y.iter().map(|field| *field.member_id.as_bytes()).collect(),
                        ),
                        (TypeDefForm::Variant(x), TypeDefForm::Variant(y)) => (
                            x.iter().map(|case| *case.member_id.as_bytes()).collect(),
                            y.iter().map(|case| *case.member_id.as_bytes()).collect(),
                        ),
                        (TypeDefForm::Record(x), TypeDefForm::Variant(y)) => {
                            flags |= 2;
                            (
                                x.iter().map(|field| *field.member_id.as_bytes()).collect(),
                                y.iter().map(|case| *case.member_id.as_bytes()).collect(),
                            )
                        }
                        (TypeDefForm::Variant(x), TypeDefForm::Record(y)) => {
                            flags |= 2;
                            (
                                x.iter().map(|case| *case.member_id.as_bytes()).collect(),
                                y.iter().map(|field| *field.member_id.as_bytes()).collect(),
                            )
                        }
                    };
                let base_set: BTreeSet<[u8; 32]> = base_members.iter().copied().collect();
                let target_set: BTreeSet<[u8; 32]> = target_members.iter().copied().collect();
                let shared: Vec<[u8; 32]> = base_members
                    .iter()
                    .filter(|member| target_set.contains(*member))
                    .copied()
                    .collect();
                let shared_target: Vec<[u8; 32]> = target_members
                    .iter()
                    .filter(|member| base_set.contains(*member))
                    .copied()
                    .collect();
                if shared != shared_target {
                    flags |= 8;
                }
                if flags & 2 == 0 && shared_member_changed(&a.form, &b.form, &base_set, &target_set)
                {
                    flags |= 4;
                }
                let added: Vec<EntityId> = target_set
                    .difference(&base_set)
                    .map(|member| EntityId::from_bytes(*member))
                    .collect();
                let removed: Vec<EntityId> = base_set
                    .difference(&target_set)
                    .map(|member| EntityId::from_bytes(*member))
                    .collect();
                fields.push(FieldDelta {
                    entity_id: id,
                    kind: kind.tag(),
                    field: 2,
                    flags: 1 | flags,
                    added,
                    removed,
                });
            }
            set_field(fields, id, kind, 3, &a.invariants, &b.invariants);
            scalar_field(fields, id, kind, 4, &a.visibility, &b.visibility);
        }
        (ImpactEntity::Function(a), ImpactEntity::Function(b)) => {
            scalar_field(fields, id, kind, 1, &a.type_parameters, &b.type_parameters);
            if a.parameters != b.parameters {
                let base_types: Vec<_> = a
                    .parameters
                    .iter()
                    .map(|parameter| base_parameters.get(parameter).map(|p| &p.value_type))
                    .collect();
                let target_types: Vec<_> = b
                    .parameters
                    .iter()
                    .map(|parameter| target_parameters.get(parameter).map(|p| &p.value_type))
                    .collect();
                let flags = u32::from(base_types != target_types) << 1;
                push_field(fields, id, kind, 2, flags, &a.parameters, &b.parameters);
            }
            scalar_field(fields, id, kind, 3, &a.result_type, &b.result_type);
            set_field(fields, id, kind, 4, &a.effects, &b.effects);
            scalar_field(fields, id, kind, 5, &a.entry_block, &b.entry_block);
            set_field(fields, id, kind, 6, &a.blocks, &b.blocks);
            set_field(fields, id, kind, 7, &a.contracts, &b.contracts);
            scalar_field(fields, id, kind, 8, &a.visibility, &b.visibility);
        }
        (ImpactEntity::Parameter(a), ImpactEntity::Parameter(b)) => {
            scalar_field(fields, id, kind, 1, &a.owner, &b.owner);
            scalar_field(fields, id, kind, 2, &a.role, &b.role);
            scalar_field(fields, id, kind, 3, &a.ordinal, &b.ordinal);
            scalar_field(fields, id, kind, 4, &a.value_type, &b.value_type);
        }
        (ImpactEntity::Block(a), ImpactEntity::Block(b)) => {
            scalar_field(fields, id, kind, 1, &a.function, &b.function);
            set_field(fields, id, kind, 2, &a.parameters, &b.parameters);
            set_field(fields, id, kind, 3, &a.operations, &b.operations);
            scalar_field(fields, id, kind, 4, &a.terminator, &b.terminator);
            scalar_field(fields, id, kind, 5, &a.reachability, &b.reachability);
        }
        (ImpactEntity::Operation(a), ImpactEntity::Operation(b)) => {
            scalar_field(fields, id, kind, 1, &a.block, &b.block);
            scalar_field(fields, id, kind, 2, &a.ordinal, &b.ordinal);
            scalar_field(fields, id, kind, 3, &a.opcode, &b.opcode);
            scalar_field(fields, id, kind, 4, &a.operands, &b.operands);
            scalar_field(fields, id, kind, 5, &a.result_types, &b.result_types);
            scalar_field(fields, id, kind, 6, &a.immediate, &b.immediate);
        }
        (ImpactEntity::Constant(a), ImpactEntity::Constant(b)) => {
            scalar_field(fields, id, kind, 1, &a.value, &b.value);
        }
        (ImpactEntity::GlobalValue(a), ImpactEntity::GlobalValue(b)) => {
            scalar_field(fields, id, kind, 1, &a.value_type, &b.value_type);
            scalar_field(fields, id, kind, 2, &a.initializer, &b.initializer);
            scalar_field(fields, id, kind, 3, &a.visibility, &b.visibility);
        }
        (ImpactEntity::EffectDef(a), ImpactEntity::EffectDef(b)) => {
            scalar_field(fields, id, kind, 1, &a.effect_kind, &b.effect_kind);
            scalar_field(fields, id, kind, 2, &a.scope_type, &b.scope_type);
            scalar_field(fields, id, kind, 3, &a.request_type, &b.request_type);
            scalar_field(fields, id, kind, 4, &a.response_type, &b.response_type);
            scalar_field(fields, id, kind, 5, &a.failure_type, &b.failure_type);
            scalar_field(fields, id, kind, 6, &a.visibility, &b.visibility);
        }
        (ImpactEntity::CapabilityRequirement(a), ImpactEntity::CapabilityRequirement(b)) => {
            scalar_field(fields, id, kind, 1, &a.effect, &b.effect);
            scalar_field(fields, id, kind, 2, &a.allowed_scopes, &b.allowed_scopes);
            set_field(
                fields,
                id,
                kind,
                3,
                &a.constraint_contracts,
                &b.constraint_contracts,
            );
        }
        (ImpactEntity::Contract(a), ImpactEntity::Contract(b)) => {
            scalar_field(fields, id, kind, 1, &a.target, &b.target);
            scalar_field(fields, id, kind, 2, &a.contract_kind, &b.contract_kind);
            scalar_field(fields, id, kind, 3, &a.predicate, &b.predicate);
            scalar_field(fields, id, kind, 4, &a.bindings, &b.bindings);
            scalar_field(fields, id, kind, 5, &a.resource_limits, &b.resource_limits);
        }
        (ImpactEntity::TestCase(a), ImpactEntity::TestCase(b)) => {
            scalar_field(fields, id, kind, 1, &a.target, &b.target);
            scalar_field(fields, id, kind, 2, &a.inputs, &b.inputs);
            scalar_field(
                fields,
                id,
                kind,
                3,
                &a.effect_environment,
                &b.effect_environment,
            );
            scalar_field(fields, id, kind, 4, &a.expected, &b.expected);
            scalar_field(fields, id, kind, 5, &a.observations, &b.observations);
            scalar_field(fields, id, kind, 6, &a.resource_limits, &b.resource_limits);
        }
        (ImpactEntity::AdapterImport(a), ImpactEntity::AdapterImport(b)) => {
            scalar_field(fields, id, kind, 1, &a.adapter_id, &b.adapter_id);
            scalar_field(fields, id, kind, 2, &a.abi_version, &b.abi_version);
            scalar_field(fields, id, kind, 3, &a.request_type, &b.request_type);
            scalar_field(fields, id, kind, 4, &a.response_type, &b.response_type);
            scalar_field(fields, id, kind, 5, &a.failure_type, &b.failure_type);
            set_field(fields, id, kind, 6, &a.effects, &b.effects);
        }
        (ImpactEntity::EntryPoint(a), ImpactEntity::EntryPoint(b)) => {
            scalar_field(fields, id, kind, 1, &a.function, &b.function);
            scalar_field(fields, id, kind, 2, &a.exposure, &b.exposure);
        }
        (ImpactEntity::PolicyBinding(a), ImpactEntity::PolicyBinding(b)) => {
            scalar_field(fields, id, kind, 1, &a.subject, &b.subject);
            set_field(fields, id, kind, 2, &a.requirements, &b.requirements);
        }
        (ImpactEntity::DependencyBinding(a), ImpactEntity::DependencyBinding(b)) => {
            scalar_field(fields, id, kind, 1, &a.dependency_root, &b.dependency_root);
            scalar_field(
                fields,
                id,
                kind,
                2,
                &a.external_package,
                &b.external_package,
            );
            scalar_field(fields, id, kind, 3, &a.local_namespace, &b.local_namespace);
        }
        _ => return fail(CompareErrorCode::InternalInvariant),
    }
    Ok(())
}

fn shared_member_changed(
    base: &TypeDefForm,
    target: &TypeDefForm,
    base_set: &BTreeSet<[u8; 32]>,
    target_set: &BTreeSet<[u8; 32]>,
) -> bool {
    match (base, target) {
        (TypeDefForm::Record(x), TypeDefForm::Record(y)) => x.iter().any(|field| {
            target_set.contains(field.member_id.as_bytes())
                && base_set.contains(field.member_id.as_bytes())
                && y.iter()
                    .find(|other| other.member_id == field.member_id)
                    .is_some_and(|other| {
                        other.value_type != field.value_type || other.visibility != field.visibility
                    })
        }),
        (TypeDefForm::Variant(x), TypeDefForm::Variant(y)) => x.iter().any(|case| {
            target_set.contains(case.member_id.as_bytes())
                && y.iter()
                    .find(|other| other.member_id == case.member_id)
                    .is_some_and(|other| other.payload_type != case.payload_type)
        }),
        _ => false,
    }
}

fn relation_deltas(
    base: &[ImpactEdge],
    target: &[ImpactEdge],
    work: &mut Work,
) -> Result<Vec<RelationDelta>> {
    work.charge((base.len() + target.len()) as u64)?;
    let mut deltas = Vec::new();
    let (mut i, mut j) = (0, 0);
    loop {
        let (take_base, take_target) = match (base.get(i), target.get(j)) {
            (Some(a), Some(b)) => (a <= b, b <= a),
            (Some(_), None) => (true, false),
            (None, Some(_)) => (false, true),
            (None, None) => break,
        };
        if take_base && take_target {
            i += 1;
            j += 1;
        } else if take_base {
            deltas.push(relation(&base[i], RelationChange::Removed));
            i += 1;
        } else {
            deltas.push(relation(&target[j], RelationChange::Added));
            j += 1;
        }
    }
    Ok(deltas)
}

const fn relation(edge: &ImpactEdge, change: RelationChange) -> RelationDelta {
    RelationDelta {
        dependent: edge.dependent,
        dependency: edge.dependency,
        kind: edge.kind,
        change,
    }
}

fn scb(error: &sley_scb1::ScbError) -> CompareError {
    CompareError::Format(error.code().as_str())
}

fn ids_list(ids: &[EntityId]) -> Result<Vec<u8>> {
    let elements: Vec<Vec<u8>> = ids.iter().map(|id| id.as_bytes().to_vec()).collect();
    encode_list(&elements).map_err(|error| scb(&error))
}

fn roots_list(roots: &[StateRoot]) -> Result<Vec<u8>> {
    let elements: Vec<Vec<u8>> = roots.iter().map(|root| root.as_bytes().to_vec()).collect();
    encode_list(&elements).map_err(|error| scb(&error))
}

fn object_bytes(object: Option<ObjectId>) -> Vec<u8> {
    object.map_or_else(|| ZERO32.to_vec(), |object| object.as_bytes().to_vec())
}

/// Encodes a delta into its canonical stored bytes and identity.
///
/// # Errors
///
/// Returns an encoding or resource failure.
#[allow(clippy::too_many_lines)]
pub fn encode_semantic_delta(delta: &SemanticDelta) -> Result<StoredSemanticDelta> {
    require_sorted(&delta.entities, |entry| entry.entity_id)?;
    require_sorted(&delta.fields, |entry| {
        (entry.entity_id, entry.kind, entry.field)
    })?;
    require_sorted(&delta.bodies, |entry| entry.entity_id)?;
    require_sorted(&delta.relations, |entry| {
        (entry.dependent, entry.dependency, entry.kind.tag())
    })?;
    require_sorted(&delta.dependency_roots_added, |root| *root)?;
    require_sorted(&delta.dependency_roots_removed, |root| *root)?;
    require_sorted(&delta.entry_points_added, |id| *id)?;
    require_sorted(&delta.entry_points_removed, |id| *id)?;
    require_sorted(&delta.collateral, |id| *id)?;
    let entities: Vec<Vec<u8>> = delta
        .entities
        .iter()
        .map(|entry| {
            encode_record(&[
                (1, entry.entity_id.as_bytes().to_vec()),
                (2, encode_uvar(u64::from(entry.change.tag()))),
                (3, encode_uvar(u64::from(entry.base_kind))),
                (4, encode_uvar(u64::from(entry.target_kind))),
                (5, object_bytes(entry.base_object)),
                (6, object_bytes(entry.target_object)),
            ])
        })
        .collect::<core::result::Result<_, _>>()
        .map_err(|error| scb(&error))?;
    let mut fields = Vec::with_capacity(delta.fields.len());
    for entry in &delta.fields {
        require_sorted(&entry.added, |id| *id)?;
        require_sorted(&entry.removed, |id| *id)?;
        fields.push(
            encode_record(&[
                (1, entry.entity_id.as_bytes().to_vec()),
                (2, encode_uvar(u64::from(entry.kind))),
                (3, encode_uvar(u64::from(entry.field))),
                (4, encode_uvar(u64::from(entry.flags))),
                (5, ids_list(&entry.added)?),
                (6, ids_list(&entry.removed)?),
            ])
            .map_err(|error| scb(&error))?,
        );
    }
    let bodies: Vec<Vec<u8>> = delta
        .bodies
        .iter()
        .map(|entry| {
            encode_record(&[
                (1, entry.entity_id.as_bytes().to_vec()),
                (2, entry.base_fingerprint.as_bytes().to_vec()),
                (3, entry.target_fingerprint.as_bytes().to_vec()),
                (4, encode_uvar(u64::from(entry.base_blocks))),
                (5, encode_uvar(u64::from(entry.target_blocks))),
                (6, encode_uvar(u64::from(entry.base_operations))),
                (7, encode_uvar(u64::from(entry.target_operations))),
            ])
        })
        .collect::<core::result::Result<_, _>>()
        .map_err(|error| scb(&error))?;
    let relations: Vec<Vec<u8>> = delta
        .relations
        .iter()
        .map(|entry| {
            encode_record(&[
                (1, entry.dependent.as_bytes().to_vec()),
                (2, entry.dependency.as_bytes().to_vec()),
                (3, encode_uvar(u64::from(entry.kind.tag()))),
                (4, encode_uvar(u64::from(entry.change.tag()))),
            ])
        })
        .collect::<core::result::Result<_, _>>()
        .map_err(|error| scb(&error))?;
    let payload = encode_record(&[
        (1, encode_uvar(FORMAT_VERSION)),
        (2, delta.workspace_id.as_bytes().to_vec()),
        (3, delta.root_schema_epoch.as_bytes().to_vec()),
        (4, delta.base_root.as_bytes().to_vec()),
        (5, delta.target_root.as_bytes().to_vec()),
        (6, encode_list(&entities).map_err(|error| scb(&error))?),
        (7, encode_list(&fields).map_err(|error| scb(&error))?),
        (8, encode_list(&bodies).map_err(|error| scb(&error))?),
        (9, encode_list(&relations).map_err(|error| scb(&error))?),
        (10, roots_list(&delta.dependency_roots_added)?),
        (11, roots_list(&delta.dependency_roots_removed)?),
        (12, ids_list(&delta.entry_points_added)?),
        (13, ids_list(&delta.entry_points_removed)?),
        (14, ids_list(&delta.collateral)?),
    ])
    .map_err(|error| scb(&error))?;
    let epoch_id = delta_epoch_id()?;
    let mut preimage = Vec::with_capacity(payload.len() + 96);
    preimage.extend_from_slice(MAGIC);
    preimage.extend_from_slice(&encode_uvar(FORMAT_VERSION));
    preimage.extend_from_slice(&encode_uvar(u64::from(CONTRACT_TAG)));
    preimage.extend_from_slice(epoch_id.as_bytes());
    preimage.extend_from_slice(&encode_uvar(payload.len() as u64));
    preimage.extend_from_slice(&payload);
    if preimage.len() + ID_LEN > MAX_DELTA_BYTES {
        return fail(CompareErrorCode::ResourceLimit);
    }
    let delta_id = SemanticDeltaId::derive(&preimage);
    let mut stored_bytes = preimage;
    stored_bytes.extend_from_slice(delta_id.as_bytes());
    Ok(StoredSemanticDelta {
        delta_id,
        stored_bytes,
        delta: delta.clone(),
    })
}

fn require_sorted<T, K: Ord>(items: &[T], key: impl Fn(&T) -> K) -> Result<()> {
    for pair in items.windows(2) {
        match key(&pair[0]).cmp(&key(&pair[1])) {
            core::cmp::Ordering::Less => {}
            core::cmp::Ordering::Equal => return fail(CompareErrorCode::DuplicateEntry),
            core::cmp::Ordering::Greater => return fail(CompareErrorCode::CanonicalOrder),
        }
    }
    Ok(())
}

fn fixed32(input: &[u8]) -> Result<[u8; 32]> {
    exact_array(input).map_err(CompareError::from)
}

fn small_u32(input: &[u8]) -> Result<u32> {
    let value = read_single_uvar(input).map_err(CompareError::from)?;
    u32::try_from(value).map_err(|_| CompareError::Compare(CompareErrorCode::FormatInvalid))
}

fn decode_ids(input: &[u8], maximum: usize) -> Result<Vec<EntityId>> {
    let elements = decode_list(input, maximum).map_err(CompareError::from)?;
    let ids: Vec<EntityId> = elements
        .into_iter()
        .map(|element| fixed32(element).map(EntityId::from_bytes))
        .collect::<Result<_>>()?;
    require_sorted(&ids, |id| *id)?;
    Ok(ids)
}

fn decode_roots(input: &[u8]) -> Result<Vec<StateRoot>> {
    let elements = decode_list(input, MAX_IDENTITY_SET).map_err(CompareError::from)?;
    let roots: Vec<StateRoot> = elements
        .into_iter()
        .map(|element| fixed32(element).map(StateRoot::from_bytes))
        .collect::<Result<_>>()?;
    require_sorted(&roots, |root| *root)?;
    Ok(roots)
}

fn decode_object(input: &[u8]) -> Result<Option<ObjectId>> {
    let bytes = fixed32(input)?;
    Ok((bytes != ZERO32).then(|| ObjectId::from_bytes(bytes)))
}

fn kind_tag(value: u32, allow_zero: bool) -> Result<u32> {
    if (value == 0 && allow_zero) || (1..=18).contains(&value) {
        Ok(value)
    } else {
        fail(CompareErrorCode::FormatInvalid)
    }
}

/// Decodes and verifies stored delta bytes.
///
/// # Errors
///
/// Returns the exact version, digest, order, duplicate, format, or resource
/// failure; a decoder never sorts or repairs input.
#[allow(clippy::too_many_lines)]
pub fn decode_semantic_delta(input: &[u8]) -> Result<StoredSemanticDelta> {
    if input.len() > MAX_DELTA_BYTES {
        return fail(CompareErrorCode::ResourceLimit);
    }
    let mut reader = Reader::new(input);
    if reader.take_exact(MAGIC.len()).map_err(CompareError::from)? != MAGIC {
        return Err(CompareError::Format(ScbErrorCode::MagicInvalid.as_str()));
    }
    if reader.read_uvar().map_err(CompareError::from)? != FORMAT_VERSION {
        return fail(CompareErrorCode::VersionUnsupported);
    }
    if reader.read_uvar().map_err(CompareError::from)? != u64::from(CONTRACT_TAG) {
        return Err(CompareError::Format(ScbErrorCode::ContractUnknown.as_str()));
    }
    let epoch = SchemaEpochId::from_bytes(reader.take_array().map_err(CompareError::from)?);
    if epoch != delta_epoch_id()? {
        return Err(CompareError::Format(ScbErrorCode::EpochMismatch.as_str()));
    }
    let payload_len = reader
        .read_len(MAX_DELTA_BYTES)
        .map_err(CompareError::from)?;
    let payload = reader.take_exact(payload_len).map_err(CompareError::from)?;
    let trailer = reader.take_array::<ID_LEN>().map_err(CompareError::from)?;
    if !reader.is_finished() {
        return Err(CompareError::Format(ScbErrorCode::TrailingBytes.as_str()));
    }
    let delta_id = SemanticDeltaId::derive(&input[..input.len() - ID_LEN]);
    if trailer != *delta_id.as_bytes() {
        return fail(CompareErrorCode::DigestMismatch);
    }

    let mut record = RecordReader::new(payload).map_err(CompareError::from)?;
    if read_single_uvar(record.required(1).map_err(CompareError::from)?)
        .map_err(CompareError::from)?
        != FORMAT_VERSION
    {
        return fail(CompareErrorCode::VersionUnsupported);
    }
    let workspace_id =
        WorkspaceId::from_bytes(fixed32(record.required(2).map_err(CompareError::from)?)?);
    let root_schema_epoch =
        SchemaEpochId::from_bytes(fixed32(record.required(3).map_err(CompareError::from)?)?);
    let base_root =
        StateRoot::from_bytes(fixed32(record.required(4).map_err(CompareError::from)?)?);
    let target_root =
        StateRoot::from_bytes(fixed32(record.required(5).map_err(CompareError::from)?)?);

    let mut entities = Vec::new();
    for element in decode_list(
        record.required(6).map_err(CompareError::from)?,
        MAX_ENTITY_DELTAS,
    )
    .map_err(CompareError::from)?
    {
        let mut entry = RecordReader::new(element).map_err(CompareError::from)?;
        let entity_id =
            EntityId::from_bytes(fixed32(entry.required(1).map_err(CompareError::from)?)?);
        let change = ChangeClass::from_tag(
            read_single_uvar(entry.required(2).map_err(CompareError::from)?)
                .map_err(CompareError::from)?,
        )?;
        let base_kind = kind_tag(
            small_u32(entry.required(3).map_err(CompareError::from)?)?,
            true,
        )?;
        let target_kind = kind_tag(
            small_u32(entry.required(4).map_err(CompareError::from)?)?,
            true,
        )?;
        let base_object = decode_object(entry.required(5).map_err(CompareError::from)?)?;
        let target_object = decode_object(entry.required(6).map_err(CompareError::from)?)?;
        entry.finish().map_err(CompareError::from)?;
        let shape_ok = match change {
            ChangeClass::Added => base_kind == 0 && base_object.is_none() && target_kind != 0,
            ChangeClass::Removed => target_kind == 0 && target_object.is_none() && base_kind != 0,
            ChangeClass::Changed | ChangeClass::MetadataOnly => {
                base_kind != 0 && base_kind == target_kind && base_object != target_object
            }
            ChangeClass::Retyped => base_kind != 0 && target_kind != 0 && base_kind != target_kind,
        };
        if !shape_ok {
            return fail(CompareErrorCode::FormatInvalid);
        }
        entities.push(EntityDelta {
            entity_id,
            change,
            base_kind,
            target_kind,
            base_object,
            target_object,
        });
    }
    require_sorted(&entities, |entry| entry.entity_id)?;

    let mut fields = Vec::new();
    for element in decode_list(
        record.required(7).map_err(CompareError::from)?,
        MAX_FIELD_DELTAS,
    )
    .map_err(CompareError::from)?
    {
        let mut entry = RecordReader::new(element).map_err(CompareError::from)?;
        let entity_id =
            EntityId::from_bytes(fixed32(entry.required(1).map_err(CompareError::from)?)?);
        let kind = kind_tag(
            small_u32(entry.required(2).map_err(CompareError::from)?)?,
            false,
        )?;
        let field = small_u32(entry.required(3).map_err(CompareError::from)?)?;
        let flags = small_u32(entry.required(4).map_err(CompareError::from)?)?;
        let added = decode_ids(
            entry.required(5).map_err(CompareError::from)?,
            MAX_IDENTITY_SET,
        )?;
        let removed = decode_ids(
            entry.required(6).map_err(CompareError::from)?,
            MAX_IDENTITY_SET,
        )?;
        entry.finish().map_err(CompareError::from)?;
        if field == 0 || field > 8 || flags & 1 == 0 || flags > 15 {
            return fail(CompareErrorCode::FormatInvalid);
        }
        fields.push(FieldDelta {
            entity_id,
            kind,
            field,
            flags,
            added,
            removed,
        });
    }
    require_sorted(&fields, |entry| (entry.entity_id, entry.kind, entry.field))?;

    let mut bodies = Vec::new();
    for element in decode_list(
        record.required(8).map_err(CompareError::from)?,
        MAX_BODY_DELTAS,
    )
    .map_err(CompareError::from)?
    {
        let mut entry = RecordReader::new(element).map_err(CompareError::from)?;
        let entity_id =
            EntityId::from_bytes(fixed32(entry.required(1).map_err(CompareError::from)?)?);
        let base_fingerprint = SemanticFingerprint::from_bytes(fixed32(
            entry.required(2).map_err(CompareError::from)?,
        )?);
        let target_fingerprint = SemanticFingerprint::from_bytes(fixed32(
            entry.required(3).map_err(CompareError::from)?,
        )?);
        let base_blocks = small_u32(entry.required(4).map_err(CompareError::from)?)?;
        let target_blocks = small_u32(entry.required(5).map_err(CompareError::from)?)?;
        let base_operations = small_u32(entry.required(6).map_err(CompareError::from)?)?;
        let target_operations = small_u32(entry.required(7).map_err(CompareError::from)?)?;
        entry.finish().map_err(CompareError::from)?;
        if base_fingerprint == target_fingerprint {
            return fail(CompareErrorCode::FormatInvalid);
        }
        bodies.push(BodyDelta {
            entity_id,
            base_fingerprint,
            target_fingerprint,
            base_blocks,
            target_blocks,
            base_operations,
            target_operations,
        });
    }
    require_sorted(&bodies, |entry| entry.entity_id)?;

    let mut relations = Vec::new();
    for element in decode_list(
        record.required(9).map_err(CompareError::from)?,
        MAX_RELATION_DELTAS,
    )
    .map_err(CompareError::from)?
    {
        let mut entry = RecordReader::new(element).map_err(CompareError::from)?;
        let dependent =
            EntityId::from_bytes(fixed32(entry.required(1).map_err(CompareError::from)?)?);
        let dependency =
            EntityId::from_bytes(fixed32(entry.required(2).map_err(CompareError::from)?)?);
        let kind = impact_kind(small_u32(entry.required(3).map_err(CompareError::from)?)?)?;
        let change = match read_single_uvar(entry.required(4).map_err(CompareError::from)?)
            .map_err(CompareError::from)?
        {
            1 => RelationChange::Added,
            2 => RelationChange::Removed,
            _ => return fail(CompareErrorCode::FormatInvalid),
        };
        entry.finish().map_err(CompareError::from)?;
        relations.push(RelationDelta {
            dependent,
            dependency,
            kind,
            change,
        });
    }
    require_sorted(&relations, |entry| {
        (entry.dependent, entry.dependency, entry.kind.tag())
    })?;

    let dependency_roots_added = decode_roots(record.required(10).map_err(CompareError::from)?)?;
    let dependency_roots_removed = decode_roots(record.required(11).map_err(CompareError::from)?)?;
    let entry_points_added = decode_ids(
        record.required(12).map_err(CompareError::from)?,
        MAX_IDENTITY_SET,
    )?;
    let entry_points_removed = decode_ids(
        record.required(13).map_err(CompareError::from)?,
        MAX_IDENTITY_SET,
    )?;
    let collateral = decode_ids(
        record.required(14).map_err(CompareError::from)?,
        MAX_IDENTITY_SET,
    )?;
    record.finish().map_err(CompareError::from)?;
    let _ = FIELD_COUNT;

    let delta = SemanticDelta {
        workspace_id,
        root_schema_epoch,
        base_root,
        target_root,
        entities,
        fields,
        bodies,
        relations,
        dependency_roots_added,
        dependency_roots_removed,
        entry_points_added,
        entry_points_removed,
        collateral,
    };
    let reencoded = encode_semantic_delta(&delta)?;
    if reencoded.stored_bytes != input {
        return fail(CompareErrorCode::CanonicalOrder);
    }
    Ok(reencoded)
}

fn impact_kind(tag: u32) -> Result<ImpactKind> {
    Ok(match tag {
        1 => ImpactKind::Ownership,
        2 => ImpactKind::TypeReference,
        3 => ImpactKind::ValueReference,
        4 => ImpactKind::ControlFlow,
        5 => ImpactKind::Call,
        6 => ImpactKind::Effect,
        7 => ImpactKind::Capability,
        8 => ImpactKind::Contract,
        9 => ImpactKind::Initializer,
        10 => ImpactKind::TestTarget,
        11 => ImpactKind::Adapter,
        12 => ImpactKind::DefinitionMember,
        _ => return fail(CompareErrorCode::FormatInvalid),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::BTreeMap;

    use sley_id::{EntityId, ObjectId, SchemaEpochId, StateRoot, WorkspaceId};
    use sley_mutate::value::{EntityBodyValue, TypeDefBody};
    use sley_policy::complete_entities::CompleteEntities;
    use sley_ssmc::{
        AdapterImport, Block, CapabilityRequirement, ConstData, ConstValue, ConstantDefinition,
        ContractDefinition, ContractKind, DependencyBindingDefinition, EffectDefinition,
        EffectEnvironment, EffectKind, EntryExposure, EntryPointDefinition, ExpectedOutcome,
        FunctionGraph, FunctionRefValue, GlobalValueDefinition, IntegerWidth, MemberId,
        NamespaceDefinition, PackageDefinition, Parameter, ParameterRole, PolicyBindingDefinition,
        Reachability, RecordField, ResourceLimits, ReturnTerminator, Terminator,
        TestCaseDefinition, TrapCode, TrapTerminator, TypeDefForm, TypeDefinition, TypeExpr,
        ValueRef, Visibility, WorkspaceDefinition,
    };

    use super::*;
    use crate::complete_root::tests::{complete_bodies, genesis, set};

    fn id(byte: u8) -> EntityId {
        EntityId::from_bytes([byte; 32])
    }

    fn root(byte: u8) -> StateRoot {
        StateRoot::from_bytes([byte; 32])
    }

    fn member(byte: u8) -> MemberId {
        MemberId::from_bytes([byte; 32])
    }

    fn field(byte: u8, value_type: TypeExpr) -> RecordField {
        RecordField {
            member_id: member(byte),
            value_type,
            visibility: Visibility::Private,
        }
    }

    /// Owned eighteen-kind complete root with per-entity object versions.
    struct Fixture {
        root_byte: u8,
        workspace_id: WorkspaceId,
        epoch: SchemaEpochId,
        workspace: WorkspaceDefinition,
        workspace_namespace: NamespaceDefinition,
        package: PackageDefinition,
        package_namespace: NamespaceDefinition,
        child_namespace: NamespaceDefinition,
        type_definition: TypeDefinition,
        function: FunctionGraph,
        parameter: Parameter,
        block: Block,
        entry_point: EntryPointDefinition,
        requirement: CapabilityRequirement,
        effect: EffectDefinition,
        contract: ContractDefinition,
        test: TestCaseDefinition,
        adapter: Option<AdapterImport>,
        retyped_adapter: Option<ConstantDefinition>,
        policy_binding: PolicyBindingDefinition,
        dependency_binding: DependencyBindingDefinition,
        constant: ConstantDefinition,
        global: Option<GlobalValueDefinition>,
        extra_constant: Option<ConstantDefinition>,
        versions: BTreeMap<u8, u8>,
        entry_points: Vec<EntityId>,
        dependency_roots: Vec<StateRoot>,
    }

    impl Fixture {
        #[allow(clippy::too_many_lines)]
        fn new(root_byte: u8) -> Self {
            Self {
                root_byte,
                workspace_id: WorkspaceId::from_bytes([1; 32]),
                epoch: SchemaEpochId::from_bytes([7; 32]),
                workspace: WorkspaceDefinition {
                    entity_id: id(1),
                    packages: vec![id(3)],
                    root_namespace: id(2),
                    capability_requirements: vec![id(11)],
                    contracts: vec![id(13)],
                    tests: vec![id(14)],
                },
                workspace_namespace: NamespaceDefinition {
                    entity_id: id(2),
                    parent: None,
                    members: Vec::new(),
                },
                package: PackageDefinition {
                    entity_id: id(3),
                    workspace: id(1),
                    root_namespace: id(4),
                    dependencies: vec![id(17)],
                    exports: vec![id(6), id(7)],
                },
                package_namespace: NamespaceDefinition {
                    entity_id: id(4),
                    parent: None,
                    members: vec![
                        id(5),
                        id(6),
                        id(10),
                        id(11),
                        id(12),
                        id(13),
                        id(14),
                        id(15),
                        id(16),
                    ],
                },
                child_namespace: NamespaceDefinition {
                    entity_id: id(5),
                    parent: Some(id(4)),
                    members: vec![id(7), id(18), id(19)],
                },
                type_definition: TypeDefinition {
                    entity_id: id(6),
                    type_parameters: Vec::new(),
                    form: TypeDefForm::Record(vec![
                        field(41, TypeExpr::Bool),
                        field(42, TypeExpr::Unit),
                    ]),
                    invariants: vec![id(13)],
                    visibility: Visibility::Private,
                },
                function: FunctionGraph {
                    entity_id: id(7),
                    type_parameters: Vec::new(),
                    parameters: vec![id(8)],
                    result_type: TypeExpr::Bool,
                    effects: Vec::new(),
                    entry_block: id(9),
                    blocks: vec![id(9)],
                    contracts: Vec::new(),
                    visibility: Visibility::Private,
                },
                parameter: Parameter {
                    entity_id: id(8),
                    owner: id(7),
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: TypeExpr::Bool,
                },
                block: Block {
                    entity_id: id(9),
                    function: id(7),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return(ReturnTerminator {
                        value: ValueRef::Parameter(id(8)),
                    }),
                    reachability: Reachability::Required,
                },
                entry_point: EntryPointDefinition {
                    entity_id: id(10),
                    function: id(7),
                    exposure: EntryExposure::Protocol,
                },
                requirement: CapabilityRequirement {
                    entity_id: id(11),
                    effect: id(12),
                    allowed_scopes: Vec::new(),
                    constraint_contracts: vec![id(13)],
                },
                effect: EffectDefinition {
                    entity_id: id(12),
                    effect_kind: EffectKind::StdoutWrite,
                    scope_type: TypeExpr::Unit,
                    request_type: TypeExpr::Unit,
                    response_type: TypeExpr::Unit,
                    failure_type: TypeExpr::Unit,
                    visibility: Visibility::Private,
                },
                contract: ContractDefinition {
                    entity_id: id(13),
                    target: id(6),
                    contract_kind: ContractKind::Invariant,
                    predicate: id(7),
                    bindings: Vec::new(),
                    resource_limits: None,
                },
                test: TestCaseDefinition {
                    entity_id: id(14),
                    target: id(7),
                    inputs: Vec::new(),
                    effect_environment: EffectEnvironment::Replay(Vec::new()),
                    expected: ExpectedOutcome::FailureCode(1),
                    observations: Vec::new(),
                    resource_limits: ResourceLimits {
                        fuel: 1,
                        memory_bytes: 1,
                        output_bytes: 1,
                        effect_count: 1,
                        call_depth: 1,
                        wall_timeout_millis: 1,
                    },
                },
                adapter: Some(AdapterImport {
                    entity_id: id(15),
                    adapter_id: [0; 32],
                    abi_version: 1,
                    request_type: TypeExpr::Unit,
                    response_type: TypeExpr::Unit,
                    failure_type: TypeExpr::Unit,
                    effects: vec![id(12)],
                }),
                retyped_adapter: None,
                policy_binding: PolicyBindingDefinition {
                    entity_id: id(16),
                    subject: id(7),
                    requirements: vec![id(11)],
                },
                dependency_binding: DependencyBindingDefinition {
                    entity_id: id(17),
                    dependency_root: root(9),
                    external_package: id(99),
                    local_namespace: id(4),
                },
                constant: ConstantDefinition {
                    entity_id: id(18),
                    value: ConstValue {
                        value_type: TypeExpr::Bool,
                        data: ConstData::Bool(true),
                    },
                },
                global: Some(GlobalValueDefinition {
                    entity_id: id(19),
                    value_type: TypeExpr::Bool,
                    initializer: id(18),
                    visibility: Visibility::Private,
                }),
                extra_constant: None,
                versions: BTreeMap::new(),
                entry_points: vec![id(10)],
                dependency_roots: vec![root(9)],
            }
        }

        /// Marks an entity's object as a new version (a different `ObjectId`).
        fn touch(&mut self, byte: u8) {
            *self.versions.entry(byte).or_insert(0) += 1;
        }

        fn object(&self, byte: u8) -> ObjectId {
            let mut bytes = [byte; 32];
            bytes[1] = self.versions.get(&byte).copied().unwrap_or(0);
            ObjectId::from_bytes(bytes)
        }

        fn request(&self) -> CompleteRootRequest {
            let mut entities = CompleteEntities {
                workspaces: vec![self.workspace.clone()],
                packages: vec![self.package.clone()],
                namespaces: vec![
                    self.workspace_namespace.clone(),
                    self.package_namespace.clone(),
                    self.child_namespace.clone(),
                ],
                type_definitions: vec![self.type_definition.clone()],
                functions: vec![self.function.clone()],
                parameters: vec![self.parameter.clone()],
                blocks: vec![self.block.clone()],
                operations: Vec::new(),
                constants: vec![self.constant.clone()],
                globals: self.global.clone().into_iter().collect(),
                effects: vec![self.effect.clone()],
                requirements: vec![self.requirement.clone()],
                contracts: vec![self.contract.clone()],
                tests: vec![self.test.clone()],
                adapters: self.adapter.clone().into_iter().collect(),
                entry_points: vec![self.entry_point.clone()],
                policy_bindings: vec![self.policy_binding.clone()],
                dependency_bindings: vec![self.dependency_binding.clone()],
                reference_edges: Vec::new(),
            };
            if let Some(constant) = &self.retyped_adapter {
                entities.constants.push(constant.clone());
            }
            if let Some(constant) = &self.extra_constant {
                entities.constants.push(constant.clone());
            }
            entities
                .constants
                .sort_by_key(|constant| constant.entity_id);
            let mut bound: Vec<(EntityId, ObjectId)> = crate::borrow_entities(&entities)
                .iter()
                .map(|entity| {
                    let byte = entity.entity_id().as_bytes()[0];
                    (entity.entity_id(), self.object(byte))
                })
                .collect();
            bound.sort_by_key(|(id, _)| *id);
            CompleteRootRequest::from_parts(
                entities,
                root(self.root_byte),
                self.workspace_id,
                self.epoch,
                bound,
                self.entry_points.clone(),
                self.dependency_roots.clone(),
            )
        }
    }

    fn compare(base: &Fixture, target: &Fixture) -> StoredSemanticDelta {
        compare_complete_roots(&base.request(), &target.request()).unwrap()
    }

    fn class_of(delta: &SemanticDelta, byte: u8) -> Option<ChangeClass> {
        delta
            .entities
            .iter()
            .find(|entry| entry.entity_id == id(byte))
            .map(|entry| entry.change)
    }

    fn field_of(delta: &SemanticDelta, byte: u8, field: u32) -> Option<&FieldDelta> {
        delta
            .fields
            .iter()
            .find(|entry| entry.entity_id == id(byte) && entry.field == field)
    }

    fn has_relation(
        delta: &SemanticDelta,
        dependent: u8,
        dependency: u8,
        kind: ImpactKind,
        change: RelationChange,
    ) -> bool {
        delta.relations.iter().any(|entry| {
            entry.dependent == id(dependent)
                && entry.dependency == id(dependency)
                && entry.kind == kind
                && entry.change == change
        })
    }

    #[test]
    fn identical_roots_yield_an_empty_valid_delta_with_a_stable_identity() {
        let base = Fixture::new(50);
        let first = compare(&base, &base);
        assert!(first.delta.is_empty());
        assert_eq!(first.delta.base_root, first.delta.target_root);
        assert_eq!(decode_semantic_delta(&first.stored_bytes).unwrap(), first);
        for _ in 0..128 {
            assert_eq!(compare(&base, &base), first);
        }
        assert_eq!(
            first.delta_id,
            SemanticDeltaId::derive(&first.stored_bytes[..first.stored_bytes.len() - ID_LEN])
        );
    }

    #[test]
    fn every_change_class_is_classified_exactly_once() {
        let base = Fixture::new(50);
        let mut target = Fixture::new(51);
        // Added: a new constant that joins the child namespace.
        target.extra_constant = Some(ConstantDefinition {
            entity_id: id(30),
            value: ConstValue {
                value_type: TypeExpr::Unit,
                data: ConstData::Unit,
            },
        });
        target.child_namespace.members = vec![id(7), id(18), id(19), id(30)];
        target.touch(5);
        // Removed: the global leaves.
        target.global = None;
        target
            .child_namespace
            .members
            .retain(|member| *member != id(19));
        // Changed: the type definition's visibility.
        target.type_definition.visibility = Visibility::Exported;
        target.touch(6);
        // Retyped: the adapter becomes a constant with the same identity.
        target.adapter = None;
        target.retyped_adapter = Some(ConstantDefinition {
            entity_id: id(15),
            value: ConstValue {
                value_type: TypeExpr::Unit,
                data: ConstData::Unit,
            },
        });
        target.touch(15);
        // MetadataOnly: the effect's object changes, its body does not.
        target.touch(12);

        let stored = compare(&base, &target);
        let delta = &stored.delta;
        assert_eq!(class_of(delta, 30), Some(ChangeClass::Added));
        assert_eq!(class_of(delta, 19), Some(ChangeClass::Removed));
        assert_eq!(class_of(delta, 6), Some(ChangeClass::Changed));
        assert_eq!(class_of(delta, 15), Some(ChangeClass::Retyped));
        assert_eq!(class_of(delta, 12), Some(ChangeClass::MetadataOnly));
        assert_eq!(class_of(delta, 5), Some(ChangeClass::Changed));
        assert_eq!(class_of(delta, 7), None);
        assert_eq!(delta.entities.len(), 6);
        let visibility = field_of(delta, 6, 4).unwrap();
        assert_eq!(visibility.flags, 1);
        assert!(visibility.added.is_empty() && visibility.removed.is_empty());
        let members = field_of(delta, 5, 2).unwrap();
        assert_eq!(members.added, vec![id(30)]);
        assert_eq!(members.removed, vec![id(19)]);
        // MetadataOnly and Retyped entities carry no field deltas.
        assert!(
            delta
                .fields
                .iter()
                .all(|entry| entry.entity_id != id(12) && entry.entity_id != id(15))
        );
        // Retyped is only a section 1 fact: the adapter's effect relation went away.
        assert!(has_relation(
            delta,
            15,
            12,
            ImpactKind::Effect,
            RelationChange::Removed
        ));
        assert!(has_relation(
            delta,
            19,
            18,
            ImpactKind::Initializer,
            RelationChange::Removed
        ));
        assert!(has_relation(
            delta,
            5,
            30,
            ImpactKind::Ownership,
            RelationChange::Added
        ));
        assert_eq!(decode_semantic_delta(&stored.stored_bytes).unwrap(), stored);
    }

    #[test]
    fn type_delta_reports_members_added_removed_changed_and_reordered() {
        let base = Fixture::new(50);
        let mut target = Fixture::new(51);
        target.type_definition.form = TypeDefForm::Record(vec![
            field(42, TypeExpr::UInt(IntegerWidth::from_bits(8))),
            field(43, TypeExpr::Bool),
        ]);
        target.touch(6);
        let delta = compare(&base, &target).delta;
        let form = field_of(&delta, 6, 2).unwrap();
        assert_eq!(form.flags, 1 | 4, "member 42 changed type");
        assert_eq!(form.added, vec![EntityId::from_bytes([43; 32])]);
        assert_eq!(form.removed, vec![EntityId::from_bytes([41; 32])]);

        let mut reordered = Fixture::new(52);
        reordered.type_definition.form =
            TypeDefForm::Record(vec![field(42, TypeExpr::Unit), field(41, TypeExpr::Bool)]);
        reordered.touch(6);
        let delta = compare(&base, &reordered).delta;
        let form = field_of(&delta, 6, 2).unwrap();
        assert_eq!(form.flags, 1 | 8, "pure reorder");
        assert!(form.added.is_empty() && form.removed.is_empty());

        let mut variant = Fixture::new(53);
        variant.type_definition.form = TypeDefForm::Variant(Vec::new());
        variant.touch(6);
        let delta = compare(&base, &variant).delta;
        assert_eq!(field_of(&delta, 6, 2).unwrap().flags, 1 | 2);
    }

    #[test]
    fn signature_change_yields_field_and_body_deltas_together() {
        let base = Fixture::new(50);
        let mut target = Fixture::new(51);
        target.function.result_type = TypeExpr::Unit;
        target.touch(7);
        let delta = compare(&base, &target).delta;
        assert_eq!(class_of(&delta, 7), Some(ChangeClass::Changed));
        assert!(field_of(&delta, 7, 3).is_some());
        assert_eq!(delta.bodies.len(), 1);
        assert_eq!(delta.bodies[0].entity_id, id(7));
        assert_ne!(
            delta.bodies[0].base_fingerprint,
            delta.bodies[0].target_fingerprint
        );
        assert_eq!(
            (delta.bodies[0].base_blocks, delta.bodies[0].target_blocks),
            (1, 1)
        );
    }

    #[test]
    fn owned_entity_change_produces_a_body_delta_for_an_unchanged_function() {
        let base = Fixture::new(50);
        let mut target = Fixture::new(51);
        target.block.terminator = Terminator::Trap(TrapTerminator {
            code: TrapCode::Unreachable,
            payload: None,
        });
        target.touch(9);
        let delta = compare(&base, &target).delta;
        assert_eq!(
            class_of(&delta, 7),
            None,
            "the function's own body is unchanged"
        );
        assert_eq!(class_of(&delta, 9), Some(ChangeClass::Changed));
        assert!(field_of(&delta, 9, 4).is_some());
        assert_eq!(delta.bodies.len(), 1);
        assert_eq!(delta.bodies[0].entity_id, id(7));
        // The function is a collateral seed through its body delta, so its
        // dependents surface as collateral.
        for byte in [10, 13, 14, 16] {
            assert!(
                delta.collateral.contains(&id(byte)),
                "missing collateral {byte}"
            );
        }
        assert!(!delta.collateral.contains(&id(9)));
    }

    #[test]
    fn relation_deltas_cover_call_effect_capability_contract_and_test_kinds() {
        let base = Fixture::new(50);
        let mut target = Fixture::new(51);
        target.constant.value = ConstValue {
            value_type: TypeExpr::FunctionRef(sley_ssmc::FunctionType {
                parameters: vec![TypeExpr::Bool],
                result: Box::new(TypeExpr::Bool),
                effects: Vec::new(),
            }),
            data: ConstData::FunctionRef(FunctionRefValue {
                function: id(7),
                type_arguments: Vec::new(),
            }),
        };
        target.touch(18);
        if let Some(adapter) = &mut target.adapter {
            adapter.effects = Vec::new();
        }
        target.touch(15);
        target.policy_binding.requirements = Vec::new();
        target.touch(16);
        target.contract.target = id(11);
        target.touch(13);
        target.workspace.tests = Vec::new();
        target.touch(1);
        let delta = compare(&base, &target).delta;
        assert!(has_relation(
            &delta,
            18,
            7,
            ImpactKind::Call,
            RelationChange::Added
        ));
        assert!(has_relation(
            &delta,
            15,
            12,
            ImpactKind::Effect,
            RelationChange::Removed
        ));
        assert!(has_relation(
            &delta,
            16,
            11,
            ImpactKind::Capability,
            RelationChange::Removed
        ));
        assert!(has_relation(
            &delta,
            13,
            6,
            ImpactKind::Contract,
            RelationChange::Removed
        ));
        assert!(has_relation(
            &delta,
            13,
            11,
            ImpactKind::Contract,
            RelationChange::Added
        ));
        assert!(has_relation(
            &delta,
            1,
            14,
            ImpactKind::TestTarget,
            RelationChange::Removed
        ));
        assert_eq!(field_of(&delta, 1, 5).unwrap().removed, vec![id(14)]);
    }

    #[test]
    fn entry_point_and_dependency_root_sets_are_reported() {
        let base = Fixture::new(50);
        let mut target = Fixture::new(51);
        target.entry_point.exposure = EntryExposure::Local;
        target.touch(10);
        target.dependency_binding.dependency_root = root(8);
        target.dependency_roots = vec![root(8)];
        target.touch(17);
        let delta = compare(&base, &target).delta;
        assert!(field_of(&delta, 10, 2).is_some());
        assert!(field_of(&delta, 17, 1).is_some());
        assert_eq!(delta.dependency_roots_added, vec![root(8)]);
        assert_eq!(delta.dependency_roots_removed, vec![root(9)]);
        assert!(delta.entry_points_added.is_empty() && delta.entry_points_removed.is_empty());

        let mut dropped = Fixture::new(52);
        dropped.entry_points = Vec::new();
        dropped
            .package_namespace
            .members
            .retain(|member| *member != id(10));
        dropped.touch(4);
        // Removing the entry point entity itself needs a request without it.
        let mut entities_request = dropped.request();
        let _ = &mut entities_request;
        // Build the target without the entry point by comparing against a
        // fixture whose entry point is absent from the projected entities.
        let base_request = base.request();
        let mut target_entities = dropped.request().entities().clone();
        target_entities.entry_points.clear();
        let bound: Vec<(EntityId, ObjectId)> = crate::borrow_entities(&target_entities)
            .iter()
            .map(|entity| {
                (
                    entity.entity_id(),
                    dropped.object(entity.entity_id().as_bytes()[0]),
                )
            })
            .collect();
        let target_request = CompleteRootRequest::from_parts(
            target_entities,
            root(52),
            dropped.workspace_id,
            dropped.epoch,
            bound,
            Vec::new(),
            dropped.dependency_roots.clone(),
        );
        let delta = compare_complete_roots(&base_request, &target_request)
            .unwrap()
            .delta;
        assert_eq!(class_of(&delta, 10), Some(ChangeClass::Removed));
        assert_eq!(delta.entry_points_removed, vec![id(10)]);
        assert!(has_relation(
            &delta,
            10,
            7,
            ImpactKind::Ownership,
            RelationChange::Removed
        ));
    }

    #[test]
    fn collateral_names_the_dependents_a_naive_entity_comparison_misses() {
        let base = Fixture::new(50);
        let mut target = Fixture::new(51);
        target.type_definition.visibility = Visibility::Workspace;
        target.touch(6);
        let delta = compare(&base, &target).delta;
        assert_eq!(delta.entities.len(), 1);
        for byte in [1, 3, 4, 13] {
            assert!(
                delta.collateral.contains(&id(byte)),
                "missing collateral {byte}"
            );
        }
        assert!(!delta.collateral.contains(&id(6)));
        assert!(!delta.collateral.contains(&id(7)));
    }

    #[test]
    fn swapped_direction_mirrors_the_delta() {
        let base = Fixture::new(50);
        let mut target = Fixture::new(51);
        target.extra_constant = Some(ConstantDefinition {
            entity_id: id(30),
            value: ConstValue {
                value_type: TypeExpr::Unit,
                data: ConstData::Unit,
            },
        });
        target.child_namespace.members = vec![id(7), id(18), id(19), id(30)];
        target.touch(5);
        target.dependency_binding.dependency_root = root(8);
        target.dependency_roots = vec![root(8)];
        target.touch(17);
        let forward = compare(&base, &target).delta;
        let backward = compare(&target, &base).delta;
        assert_eq!(class_of(&forward, 30), Some(ChangeClass::Added));
        assert_eq!(class_of(&backward, 30), Some(ChangeClass::Removed));
        assert_eq!(
            forward.dependency_roots_added,
            backward.dependency_roots_removed
        );
        assert_eq!(
            forward.dependency_roots_removed,
            backward.dependency_roots_added
        );
        assert_eq!(forward.collateral, backward.collateral);
        assert_eq!(
            forward
                .relations
                .iter()
                .filter(|entry| entry.change == RelationChange::Added)
                .count(),
            backward
                .relations
                .iter()
                .filter(|entry| entry.change == RelationChange::Removed)
                .count()
        );
        assert_ne!(forward, backward);
    }

    #[test]
    fn preconditions_fail_closed_with_wrapped_codes() {
        let base = Fixture::new(50);
        let mut other_workspace = Fixture::new(51);
        other_workspace.workspace_id = WorkspaceId::from_bytes([2; 32]);
        assert_eq!(
            compare_complete_roots(&base.request(), &other_workspace.request())
                .unwrap_err()
                .code(),
            CompareErrorCode::WorkspaceMismatch
        );
        let mut other_epoch = Fixture::new(52);
        other_epoch.epoch = SchemaEpochId::from_bytes([8; 32]);
        assert_eq!(
            compare_complete_roots(&base.request(), &other_epoch.request())
                .unwrap_err()
                .code(),
            CompareErrorCode::EpochMismatch
        );
        let incomplete = Fixture::new(53);
        let mut entities = incomplete.request().entities().clone();
        entities.workspaces.clear();
        let bound: Vec<(EntityId, ObjectId)> = crate::borrow_entities(&entities)
            .iter()
            .map(|entity| {
                (
                    entity.entity_id(),
                    incomplete.object(entity.entity_id().as_bytes()[0]),
                )
            })
            .collect();
        let request = CompleteRootRequest::from_parts(
            entities,
            root(53),
            incomplete.workspace_id,
            incomplete.epoch,
            bound,
            incomplete.entry_points.clone(),
            incomplete.dependency_roots.clone(),
        );
        let error = compare_complete_roots(&base.request(), &request).unwrap_err();
        assert_eq!(error.code(), CompareErrorCode::RootIncomplete);
        assert_eq!(
            error.source_code().as_deref(),
            Some("IMPACT_UNRESOLVED_ENTITY")
        );

        let mut inventory = Fixture::new(54);
        inventory.function.parameters = Vec::new();
        inventory.touch(7);
        let error = compare_complete_roots(&base.request(), &inventory.request()).unwrap_err();
        assert_eq!(error.code(), CompareErrorCode::InventoryInvalid);
        assert!(error.source_code().unwrap().starts_with("FINGERPRINT_"));
    }

    #[test]
    fn decoder_rejection_matrix_reaches_every_frozen_code() {
        let base = Fixture::new(50);
        let mut target = Fixture::new(51);
        target.type_definition.visibility = Visibility::Exported;
        target.touch(6);
        let stored = compare(&base, &target);
        let bytes = &stored.stored_bytes;

        let mut version = bytes.clone();
        version[8] = 2;
        assert_eq!(
            decode_semantic_delta(&version).unwrap_err().code(),
            CompareErrorCode::VersionUnsupported
        );
        let mut digest = bytes.clone();
        let last = digest.len() - 1;
        digest[last] ^= 1;
        assert_eq!(
            decode_semantic_delta(&digest).unwrap_err().code(),
            CompareErrorCode::DigestMismatch
        );
        let mut contract = bytes.clone();
        contract[9] = 0x7f;
        let error = decode_semantic_delta(&contract).unwrap_err();
        assert_eq!(error.code(), CompareErrorCode::FormatInvalid);
        assert_eq!(error.source_code().as_deref(), Some("SCB_CONTRACT_UNKNOWN"));
        let mut trailing = bytes.clone();
        trailing.push(0);
        let error = decode_semantic_delta(&trailing).unwrap_err();
        assert_eq!(error.code(), CompareErrorCode::FormatInvalid);
        assert_eq!(error.source_code().as_deref(), Some("SCB_TRAILING_BYTES"));
        assert_eq!(
            decode_semantic_delta(&vec![0; MAX_DELTA_BYTES + 1])
                .unwrap_err()
                .code(),
            CompareErrorCode::ResourceLimit
        );

        let mut reversed = stored.delta.clone();
        reversed.collateral.reverse();
        assert_eq!(
            encode_semantic_delta(&reversed).unwrap_err().code(),
            CompareErrorCode::CanonicalOrder
        );
        let mut duplicated = stored.delta.clone();
        duplicated.entities.push(duplicated.entities[0]);
        assert_eq!(
            encode_semantic_delta(&duplicated).unwrap_err().code(),
            CompareErrorCode::DuplicateEntry
        );
        let mut malformed = stored.delta.clone();
        malformed.entities[0].base_kind = 0;
        let encoded = encode_semantic_delta(&malformed).unwrap();
        assert_eq!(
            decode_semantic_delta(&encoded.stored_bytes)
                .unwrap_err()
                .code(),
            CompareErrorCode::FormatInvalid
        );
        for (offset, code) in CompareErrorCode::ALL.iter().enumerate() {
            assert_eq!(code.numeric(), 51_000 + u32::try_from(offset).unwrap());
            assert!(code.as_str().starts_with("COMPARE_"));
        }
    }

    #[test]
    fn repository_backed_roots_compare_through_the_extraction_adapter() {
        let (_base_temp, base_transactions, base_genesis) =
            genesis("compare-base", complete_bodies(), &[root(9)]);
        let mut changed = complete_bodies();
        for (byte, body) in &mut changed {
            if *byte == 6 {
                *body = EntityBodyValue::TypeDef(TypeDefBody {
                    type_parameters: Vec::new(),
                    form: TypeDefForm::Record(Vec::new()),
                    invariants: set(&[]),
                    visibility: Visibility::Exported,
                });
            }
        }
        let (_target_temp, target_transactions, target_genesis) =
            genesis("compare-target", changed, &[root(9)]);
        let base = CompleteRootRequest::extract(
            &base_transactions.verified_revision(base_genesis).unwrap(),
        )
        .unwrap();
        let target = CompleteRootRequest::extract(
            &target_transactions
                .verified_revision(target_genesis)
                .unwrap(),
        )
        .unwrap();
        let stored = compare_complete_roots(&base, &target).unwrap();
        assert_eq!(class_of(&stored.delta, 6), Some(ChangeClass::Changed));
        assert_eq!(stored.delta.fields.len(), 1);
        assert!(stored.delta.collateral.contains(&id(3)));
        assert_eq!(stored.delta.base_root, base.root());
        assert_eq!(decode_semantic_delta(&stored.stored_bytes).unwrap(), stored);
    }
    // ---- corpus emitter (scripts/generate_semantic_comparison_fixtures.py) ----

    pub(crate) fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        bytes
            .iter()
            .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
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

    fn json_str(value: &str) -> String {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }

    #[allow(clippy::too_many_lines)]
    fn entity_json(entity: ImpactEntity<'_>, object: ObjectId) -> String {
        let head = format!(
            "\"id\":\"{}\",\"kind\":{},\"object\":\"{}\"",
            hex(entity.entity_id().as_bytes()),
            entity.kind().tag(),
            hex(object.as_bytes())
        );
        let body = match entity {
            ImpactEntity::Workspace(v) => format!(
                "\"packages\":{},\"root_namespace\":\"{}\",\"capability_requirements\":{},\"contracts\":{},\"tests\":{}",
                json_ids(&v.packages),
                hex(v.root_namespace.as_bytes()),
                json_ids(&v.capability_requirements),
                json_ids(&v.contracts),
                json_ids(&v.tests)
            ),
            ImpactEntity::Package(v) => format!(
                "\"workspace\":\"{}\",\"root_namespace\":\"{}\",\"dependencies\":{},\"exports\":{}",
                hex(v.workspace.as_bytes()),
                hex(v.root_namespace.as_bytes()),
                json_ids(&v.dependencies),
                json_ids(&v.exports)
            ),
            ImpactEntity::Namespace(v) => format!(
                "\"parent\":{},\"members\":{}",
                v.parent.map_or_else(
                    || "null".to_owned(),
                    |parent| format!("\"{}\"", hex(parent.as_bytes()))
                ),
                json_ids(&v.members)
            ),
            ImpactEntity::TypeDef(v) => {
                let form = match &v.form {
                    TypeDefForm::Record(fields) => format!(
                        "{{\"kind\":\"record\",\"members\":[{}]}}",
                        fields
                            .iter()
                            .map(|f| format!(
                                "[\"{}\",{},{}]",
                                hex(f.member_id.as_bytes()),
                                json_str(&format!("{:?}", f.value_type)),
                                f.visibility.tag()
                            ))
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                    TypeDefForm::Variant(cases) => format!(
                        "{{\"kind\":\"variant\",\"members\":[{}]}}",
                        cases
                            .iter()
                            .map(|c| format!(
                                "[\"{}\",{}]",
                                hex(c.member_id.as_bytes()),
                                c.payload_type.as_ref().map_or_else(
                                    || "null".to_owned(),
                                    |t| json_str(&format!("{t:?}"))
                                )
                            ))
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                };
                format!(
                    "\"type_parameters\":{},\"form\":{},\"invariants\":{},\"visibility\":{}",
                    v.type_parameters.len(),
                    form,
                    json_ids(&v.invariants),
                    v.visibility.tag()
                )
            }
            ImpactEntity::Function(v) => format!(
                "\"type_parameters\":{},\"parameters\":{},\"result_type\":{},\"effects\":{},\"entry_block\":\"{}\",\"blocks\":{},\"contracts\":{},\"visibility\":{}",
                v.type_parameters.len(),
                json_ids(&v.parameters),
                json_str(&format!("{:?}", v.result_type)),
                json_ids(&v.effects),
                hex(v.entry_block.as_bytes()),
                json_ids(&v.blocks),
                json_ids(&v.contracts),
                v.visibility.tag()
            ),
            ImpactEntity::Parameter(v) => format!(
                "\"owner\":\"{}\",\"role\":\"{}\",\"ordinal\":{},\"value_type\":{}",
                hex(v.owner.as_bytes()),
                match v.role {
                    ParameterRole::Function => "function",
                    ParameterRole::Block => "block",
                },
                v.ordinal,
                json_str(&format!("{:?}", v.value_type))
            ),
            ImpactEntity::Block(v) => {
                let (terminator_kind, return_parameter) = match &v.terminator {
                    Terminator::Return(ReturnTerminator {
                        value: ValueRef::Parameter(parameter),
                    }) => ("return", format!("\"{}\"", hex(parameter.as_bytes()))),
                    Terminator::Trap(_) => ("trap", "null".to_owned()),
                    _ => panic!("corpus blocks return a parameter or trap"),
                };
                format!(
                    "\"function\":\"{}\",\"parameters\":{},\"operations\":{},\"terminator_kind\":\"{}\",\"return_parameter\":{},\"reachability\":{}",
                    hex(v.function.as_bytes()),
                    json_ids(&v.parameters),
                    json_ids(&v.operations),
                    terminator_kind,
                    return_parameter,
                    v.reachability.tag()
                )
            }
            ImpactEntity::Operation(_) => panic!("corpus carries no operations"),
            ImpactEntity::Constant(v) => {
                let function_ref = match &v.value.data {
                    ConstData::FunctionRef(value) => {
                        format!("\"{}\"", hex(value.function.as_bytes()))
                    }
                    _ => "null".to_owned(),
                };
                format!(
                    "\"value\":{},\"function_ref\":{}",
                    json_str(&format!("{:?}", v.value)),
                    function_ref
                )
            }
            ImpactEntity::GlobalValue(v) => format!(
                "\"value_type\":{},\"initializer\":\"{}\",\"visibility\":{}",
                json_str(&format!("{:?}", v.value_type)),
                hex(v.initializer.as_bytes()),
                v.visibility.tag()
            ),
            ImpactEntity::EffectDef(v) => format!(
                "\"effect_kind\":{},\"types\":[{},{},{},{}],\"visibility\":{}",
                v.effect_kind.tag(),
                json_str(&format!("{:?}", v.scope_type)),
                json_str(&format!("{:?}", v.request_type)),
                json_str(&format!("{:?}", v.response_type)),
                json_str(&format!("{:?}", v.failure_type)),
                v.visibility.tag()
            ),
            ImpactEntity::CapabilityRequirement(v) => format!(
                "\"effect\":\"{}\",\"allowed_scopes\":{},\"constraint_contracts\":{}",
                hex(v.effect.as_bytes()),
                json_str(&format!("{:?}", v.allowed_scopes)),
                json_ids(&v.constraint_contracts)
            ),
            ImpactEntity::Contract(v) => format!(
                "\"target\":\"{}\",\"contract_kind\":{},\"predicate\":\"{}\",\"bindings\":{},\"resource_limits\":{}",
                hex(v.target.as_bytes()),
                v.contract_kind.tag(),
                hex(v.predicate.as_bytes()),
                json_str(&format!("{:?}", v.bindings)),
                json_str(&format!("{:?}", v.resource_limits))
            ),
            ImpactEntity::TestCase(v) => format!(
                "\"target\":\"{}\",\"inputs\":{},\"environment\":{},\"expected\":{},\"observations\":{},\"limits\":{}",
                hex(v.target.as_bytes()),
                json_str(&format!("{:?}", v.inputs)),
                json_str(&format!("{:?}", v.effect_environment)),
                json_str(&format!("{:?}", v.expected)),
                json_str(&format!("{:?}", v.observations)),
                json_str(&format!("{:?}", v.resource_limits))
            ),
            ImpactEntity::AdapterImport(v) => format!(
                "\"adapter_id\":\"{}\",\"abi_version\":{},\"types\":[{},{},{}],\"effects\":{}",
                hex(&v.adapter_id),
                v.abi_version,
                json_str(&format!("{:?}", v.request_type)),
                json_str(&format!("{:?}", v.response_type)),
                json_str(&format!("{:?}", v.failure_type)),
                json_ids(&v.effects)
            ),
            ImpactEntity::EntryPoint(v) => format!(
                "\"function\":\"{}\",\"exposure\":{}",
                hex(v.function.as_bytes()),
                v.exposure.tag()
            ),
            ImpactEntity::PolicyBinding(v) => format!(
                "\"subject\":\"{}\",\"requirements\":{}",
                hex(v.subject.as_bytes()),
                json_ids(&v.requirements)
            ),
            ImpactEntity::DependencyBinding(v) => format!(
                "\"dependency_root\":\"{}\",\"external_package\":\"{}\",\"local_namespace\":\"{}\"",
                hex(v.dependency_root.as_bytes()),
                hex(v.external_package.as_bytes()),
                hex(v.local_namespace.as_bytes())
            ),
        };
        format!("{{{head},{body}}}")
    }

    pub(crate) fn root_json(request: &CompleteRootRequest) -> String {
        let objects: BTreeMap<EntityId, ObjectId> =
            request.bound_objects().iter().copied().collect();
        let entities: Vec<String> = request
            .borrowed()
            .into_iter()
            .map(|entity| entity_json(entity, objects[&entity.entity_id()]))
            .collect();
        let roots: Vec<String> = request
            .facts()
            .dependency_roots
            .iter()
            .map(|root| format!("\"{}\"", hex(root.as_bytes())))
            .collect();
        format!(
            "{{\"root\":\"{}\",\"workspace_id\":\"{}\",\"schema_epoch_id\":\"{}\",\"entities\":[{}],\"entry_points\":{},\"dependency_roots\":[{}]}}",
            hex(request.root().as_bytes()),
            hex(request.workspace_id().as_bytes()),
            hex(request.schema_epoch_id().as_bytes()),
            entities.join(","),
            json_ids(request.facts().entry_points),
            roots.join(",")
        )
    }

    fn delta_json(delta: &SemanticDelta) -> String {
        let entities: Vec<String> = delta
            .entities
            .iter()
            .map(|e| {
                format!(
                    "[\"{}\",{},{},{},{},{}]",
                    hex(e.entity_id.as_bytes()),
                    e.change.tag(),
                    e.base_kind,
                    e.target_kind,
                    e.base_object.map_or_else(
                        || "null".to_owned(),
                        |o| format!("\"{}\"", hex(o.as_bytes()))
                    ),
                    e.target_object.map_or_else(
                        || "null".to_owned(),
                        |o| format!("\"{}\"", hex(o.as_bytes()))
                    )
                )
            })
            .collect();
        let fields: Vec<String> = delta
            .fields
            .iter()
            .map(|f| {
                format!(
                    "[\"{}\",{},{},{},{},{}]",
                    hex(f.entity_id.as_bytes()),
                    f.kind,
                    f.field,
                    f.flags,
                    json_ids(&f.added),
                    json_ids(&f.removed)
                )
            })
            .collect();
        let bodies: Vec<String> = delta
            .bodies
            .iter()
            .map(|b| {
                format!(
                    "[\"{}\",\"{}\",\"{}\",{},{},{},{}]",
                    hex(b.entity_id.as_bytes()),
                    hex(b.base_fingerprint.as_bytes()),
                    hex(b.target_fingerprint.as_bytes()),
                    b.base_blocks,
                    b.target_blocks,
                    b.base_operations,
                    b.target_operations
                )
            })
            .collect();
        let relations: Vec<String> = delta
            .relations
            .iter()
            .map(|r| {
                format!(
                    "[\"{}\",\"{}\",{},{}]",
                    hex(r.dependent.as_bytes()),
                    hex(r.dependency.as_bytes()),
                    r.kind.tag(),
                    r.change.tag()
                )
            })
            .collect();
        let roots = |roots: &[StateRoot]| -> String {
            format!(
                "[{}]",
                roots
                    .iter()
                    .map(|r| format!("\"{}\"", hex(r.as_bytes())))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        format!(
            "{{\"workspace_id\":\"{}\",\"root_schema_epoch\":\"{}\",\"base_root\":\"{}\",\"target_root\":\"{}\",\"entities\":[{}],\"fields\":[{}],\"bodies\":[{}],\"relations\":[{}],\"dependency_roots_added\":{},\"dependency_roots_removed\":{},\"entry_points_added\":{},\"entry_points_removed\":{},\"collateral\":{}}}",
            hex(delta.workspace_id.as_bytes()),
            hex(delta.root_schema_epoch.as_bytes()),
            hex(delta.base_root.as_bytes()),
            hex(delta.target_root.as_bytes()),
            entities.join(","),
            fields.join(","),
            bodies.join(","),
            relations.join(","),
            roots(&delta.dependency_roots_added),
            roots(&delta.dependency_roots_removed),
            json_ids(&delta.entry_points_added),
            json_ids(&delta.entry_points_removed),
            json_ids(&delta.collateral)
        )
    }

    type Mutation = Box<dyn Fn(&mut Fixture)>;

    /// Emits the frozen comparison corpus for
    /// `scripts/generate_semantic_comparison_fixtures.py`.
    #[test]
    #[ignore = "fixture refresh emitter; run through the generator script"]
    #[allow(clippy::too_many_lines)]
    fn emit_semantic_comparison_corpus_for_fixture_refresh() {
        let cases: Vec<(&str, Mutation)> = vec![
            ("identical", Box::new(|_| {})),
            (
                "change-classes",
                Box::new(|t| {
                    t.extra_constant = Some(ConstantDefinition {
                        entity_id: id(30),
                        value: ConstValue {
                            value_type: TypeExpr::Unit,
                            data: ConstData::Unit,
                        },
                    });
                    t.child_namespace.members = vec![id(7), id(18), id(30)];
                    t.touch(5);
                    t.global = None;
                    t.type_definition.visibility = Visibility::Exported;
                    t.touch(6);
                    t.adapter = None;
                    t.retyped_adapter = Some(ConstantDefinition {
                        entity_id: id(15),
                        value: ConstValue {
                            value_type: TypeExpr::Unit,
                            data: ConstData::Unit,
                        },
                    });
                    t.touch(15);
                    t.touch(12);
                }),
            ),
            (
                "type-members",
                Box::new(|t| {
                    t.type_definition.form = TypeDefForm::Record(vec![
                        field(42, TypeExpr::UInt(IntegerWidth::from_bits(8))),
                        field(43, TypeExpr::Bool),
                    ]);
                    t.touch(6);
                }),
            ),
            (
                "type-reorder",
                Box::new(|t| {
                    t.type_definition.form = TypeDefForm::Record(vec![
                        field(42, TypeExpr::Unit),
                        field(41, TypeExpr::Bool),
                    ]);
                    t.touch(6);
                }),
            ),
            (
                "signature",
                Box::new(|t| {
                    t.function.result_type = TypeExpr::Unit;
                    t.touch(7);
                }),
            ),
            (
                "body-only",
                Box::new(|t| {
                    t.block.terminator = Terminator::Trap(TrapTerminator {
                        code: TrapCode::Unreachable,
                        payload: None,
                    });
                    t.touch(9);
                }),
            ),
            (
                "relations",
                Box::new(|t| {
                    t.constant.value = ConstValue {
                        value_type: TypeExpr::FunctionRef(sley_ssmc::FunctionType {
                            parameters: vec![TypeExpr::Bool],
                            result: Box::new(TypeExpr::Bool),
                            effects: Vec::new(),
                        }),
                        data: ConstData::FunctionRef(FunctionRefValue {
                            function: id(7),
                            type_arguments: Vec::new(),
                        }),
                    };
                    t.touch(18);
                    if let Some(adapter) = &mut t.adapter {
                        adapter.effects = Vec::new();
                    }
                    t.touch(15);
                    t.policy_binding.requirements = Vec::new();
                    t.touch(16);
                    t.contract.target = id(11);
                    t.touch(13);
                    t.workspace.tests = Vec::new();
                    t.touch(1);
                }),
            ),
            (
                "root-sets",
                Box::new(|t| {
                    t.entry_point.exposure = EntryExposure::Local;
                    t.touch(10);
                    t.dependency_binding.dependency_root = root(8);
                    t.dependency_roots = vec![root(8)];
                    t.touch(17);
                }),
            ),
            (
                "collateral",
                Box::new(|t| {
                    t.type_definition.visibility = Visibility::Workspace;
                    t.touch(6);
                }),
            ),
        ];
        let base = Fixture::new(50);
        let mut rejections: Vec<(&str, &str, Vec<u8>)> = Vec::new();
        for (name, mutate) in cases {
            let mut target = Fixture::new(51);
            mutate(&mut target);
            let stored = compare(&base, &target);
            assert_eq!(decode_semantic_delta(&stored.stored_bytes).unwrap(), stored);
            println!(
                "COMPARE_VECTOR|{name}|{}|{}|{}|{}|{}",
                root_json(&base.request()),
                root_json(&target.request()),
                delta_json(&stored.delta),
                hex(&stored.stored_bytes),
                hex(stored.delta_id.as_bytes())
            );
            if name == "collateral" {
                let mut version = stored.stored_bytes.clone();
                version[8] = 2;
                rejections.push(("version", "COMPARE_VERSION_UNSUPPORTED", version));
                let mut digest = stored.stored_bytes.clone();
                let last = digest.len() - 1;
                digest[last] ^= 1;
                rejections.push(("flip-trailer", "COMPARE_DIGEST_MISMATCH", digest));
                let mut contract = stored.stored_bytes.clone();
                contract[9] = 0x7f;
                rejections.push(("contract-tag", "COMPARE_FORMAT_INVALID", contract));
                let mut trailing = stored.stored_bytes.clone();
                trailing.push(0);
                rejections.push(("trailing-byte", "COMPARE_FORMAT_INVALID", trailing));
                let mut malformed = stored.delta.clone();
                malformed.entities[0].base_kind = 0;
                let encoded = encode_semantic_delta(&malformed).unwrap();
                rejections.push((
                    "added-shape",
                    "COMPARE_FORMAT_INVALID",
                    encoded.stored_bytes,
                ));
            }
        }
        for (name, code, input) in rejections {
            assert_eq!(
                decode_semantic_delta(&input).unwrap_err().code().as_str(),
                code
            );
            println!("COMPARE_REJECT|{name}|{code}|{}", hex(&input));
        }
    }
}
