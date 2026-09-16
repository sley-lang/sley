//! Native test plan (`SLEYTPL1`) from `NATIVE_TEST_ADMISSION_V1.md` section 3.
//!
//! The plan binds the selection mode, workspace, epoch, parent and proposed
//! roots, policy root, candidate and static-result identities, protected
//! required tests, the raw-ID-sorted selected entries with exact declared
//! limits, the complete changed-test inventory, the immutable execution
//! profile, implementation ceilings, the static selected set, and the exact
//! resource-policy record. Parsing validates shape, order, and mode rules; it
//! never grants selection authority, which stays with the policy owner (N4).

use sley_id::{
    CandidateId, CandidateResultId, EntityId, NativeResourcePolicyId, NativeTestPlanId, ObjectId,
    PolicyRootId, SchemaEpochId, StateRoot, TransactionId, WorkspaceId,
};
use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_list, encode_record, encode_uvar};
use sley_vm::native_execution::{NativeDeclaredLimits, NativeImplementationLimits, profile_id};

use crate::codec::{
    MAX_CHANGED_ENTRIES, MAX_SELECTED_ENTRIES, PLAN_MAGIC, POLICY_MAGIC, RECORD_VERSION,
    check_sorted_unique, decode_envelope, decode_fields, encode_id_list, encode_option_id,
    expect_tags, preimage_bytes, read_id, read_id_list, read_option_id, read_uvar_value,
};
use crate::policy::{NativeResourcePolicyV1, parse_implementation_limits};

/// Explicit-root diagnostic selection: one bound root, no candidate.
pub const SELECTION_MODE_EXPLICIT_ROOT: u32 = 1;
/// Candidate-affected selection over a proposed root.
pub const SELECTION_MODE_CANDIDATE_AFFECTED: u32 = 2;

/// One selected test with exact proposed-state bindings and declared limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedEntry {
    /// Selected test entity.
    pub test_entity: EntityId,
    /// Exact proposed test object; ID reuse cannot substitute expectations.
    pub test_object: ObjectId,
    /// Target function entity.
    pub target_function: EntityId,
    /// Exact proposed target object.
    pub target_object: ObjectId,
    /// Literal `TestCase` limits, without policy clamping.
    pub declared_limits: NativeDeclaredLimits,
}

impl SelectedEntry {
    fn record(self) -> Result<Vec<u8>, ScbError> {
        encode_record(&[
            (1, self.test_entity.as_bytes().to_vec()),
            (2, self.test_object.as_bytes().to_vec()),
            (3, self.target_function.as_bytes().to_vec()),
            (4, self.target_object.as_bytes().to_vec()),
            (5, declared_record(self.declared_limits)),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5])?;
        Ok(Self {
            test_entity: EntityId::from_bytes(read_id(&fields[0].1)?),
            test_object: ObjectId::from_bytes(read_id(&fields[1].1)?),
            target_function: EntityId::from_bytes(read_id(&fields[2].1)?),
            target_object: ObjectId::from_bytes(read_id(&fields[3].1)?),
            declared_limits: parse_declared_limits(&fields[4].1)?,
        })
    }
}

/// One changed-test inventory entry with before/after object identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChangedTest {
    /// Changed test entity.
    pub test_entity: EntityId,
    /// Previous object, absent when created.
    pub before: Option<ObjectId>,
    /// New object, absent when deleted.
    pub after: Option<ObjectId>,
}

impl ChangedTest {
    fn record(self) -> Result<Vec<u8>, ScbError> {
        encode_record(&[
            (1, self.test_entity.as_bytes().to_vec()),
            (2, encode_option_id(self.before.map(|id| *id.as_bytes()))?),
            (3, encode_option_id(self.after.map(|id| *id.as_bytes()))?),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2, 3])?;
        let parsed = Self {
            test_entity: EntityId::from_bytes(read_id(&fields[0].1)?),
            before: read_option_id(&fields[1].1)?.map(ObjectId::from_bytes),
            after: read_option_id(&fields[2].1)?.map(ObjectId::from_bytes),
        };
        check_changed(&parsed)?;
        Ok(parsed)
    }
}

/// Caller-supplied plan facts; constructing these grants no authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeTestPlanParts {
    /// One for explicit-root diagnostics, two for candidate-affected plans.
    pub selection_mode: u32,
    /// Workspace the roots belong to.
    pub workspace: WorkspaceId,
    /// Semantic epoch of the tested state.
    pub semantic_epoch: SchemaEpochId,
    /// Exact accepted parent transaction.
    pub parent_transaction: TransactionId,
    /// Exact accepted parent root.
    pub parent_root: StateRoot,
    /// Proposed root under test.
    pub proposed_root: StateRoot,
    /// Protected policy root the resource policy was derived from.
    pub policy_root: PolicyRootId,
    /// Candidate under test; required in candidate mode, absent in explicit.
    pub candidate_id: Option<CandidateId>,
    /// Fresh static result; required in candidate mode, absent in explicit.
    pub static_result_id: Option<CandidateResultId>,
    /// Protected required test entities, raw-ID sorted unique.
    pub protected_required_ids: Vec<EntityId>,
    /// Selected entries, strictly test-ID sorted, at most 256.
    pub selected: Vec<SelectedEntry>,
    /// Complete changed-test inventory, strictly test-ID sorted.
    pub changed: Vec<ChangedTest>,
    /// Exact local implementation ceilings, within native hard maxima.
    pub implementation_limits: NativeImplementationLimits,
    /// Static selected entities, raw-ID sorted unique.
    pub static_selected_ids: Vec<EntityId>,
    /// Exact effective resource policy, embedded as a direct record.
    pub resource_policy: NativeResourcePolicyV1,
}

/// Immutable canonical test plan; external callers cannot forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeTestPlanV1 {
    parts: NativeTestPlanParts,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: NativeTestPlanId,
}

impl NativeTestPlanV1 {
    /// Builds a validated plan from caller-supplied facts.
    ///
    /// The execution profile is always the immutable native-v1 profile; it is
    /// derived, never accepted from the caller.
    ///
    /// # Errors
    /// Returns `SCB_FIELD_ORDER`/`SCB_FIELD_DUPLICATE` for unsorted or
    /// repeated identities, `SCB_RESOURCE_LIMIT` for oversized lists or
    /// ceilings, `SCB_FIELD_MISSING` for identities a mode requires, and
    /// `SCB_CONTRACT_UNKNOWN` for identities or roots a mode forbids.
    pub fn build(parts: NativeTestPlanParts) -> Result<Self, ScbError> {
        validate_parts(&parts)?;
        let record = record_parts(&parts)?;
        let preimage = preimage_bytes(PLAN_MAGIC, &record)?;
        let id = NativeTestPlanId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        Ok(Self {
            parts,
            record,
            stored,
            id,
        })
    }

    /// Strictly parses and validates one stored plan envelope.
    ///
    /// Parsing checks shape, order, mode rules, and the embedded policy. It
    /// does not re-derive selection, authenticate policy, or admit anything.
    ///
    /// # Errors
    /// Returns the first stable SCB1 failure encountered while decoding the
    /// envelope, verifying its digest, or validating fields and mode rules.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        let (record, trailer) = decode_envelope(stored, PLAN_MAGIC)?;
        let preimage = preimage_bytes(PLAN_MAGIC, &record)?;
        if NativeTestPlanId::derive(&preimage).into_bytes() != trailer {
            return Err(ScbError::new(ScbErrorCode::DigestMismatch));
        }
        let fields = decode_fields(&record)?;
        expect_tags(
            &fields,
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17],
        )?;
        if read_uvar_value(&fields[0].1, 32)? != RECORD_VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        if read_id(&fields[13].1)? != *profile_id().as_bytes() {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        Self::build(NativeTestPlanParts {
            selection_mode: u32::try_from(read_uvar_value(&fields[1].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?,
            workspace: WorkspaceId::from_bytes(read_id(&fields[2].1)?),
            semantic_epoch: SchemaEpochId::from_bytes(read_id(&fields[3].1)?),
            parent_transaction: TransactionId::from_bytes(read_id(&fields[4].1)?),
            parent_root: StateRoot::from_bytes(read_id(&fields[5].1)?),
            proposed_root: StateRoot::from_bytes(read_id(&fields[6].1)?),
            policy_root: PolicyRootId::from_bytes(read_id(&fields[7].1)?),
            candidate_id: read_option_id(&fields[8].1)?.map(CandidateId::from_bytes),
            static_result_id: read_option_id(&fields[9].1)?.map(CandidateResultId::from_bytes),
            protected_required_ids: read_entity_list(&fields[10].1)?,
            selected: parse_selected(&fields[11].1)?,
            changed: parse_changed(&fields[12].1)?,
            implementation_limits: parse_implementation_limits(&fields[14].1)?,
            static_selected_ids: read_entity_list(&fields[15].1)?,
            resource_policy: NativeResourcePolicyV1::parse(&envelope_policy(&fields[16].1)?)?,
        })
    }

    /// Canonical SCB1 record without envelope or trailer.
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }

    /// Complete canonical envelope and digest trailer.
    #[must_use]
    pub fn stored_bytes(&self) -> &[u8] {
        &self.stored
    }

    /// Test plan identity over the exact envelope preimage.
    #[must_use]
    pub const fn plan_id(&self) -> NativeTestPlanId {
        self.id
    }

    /// Bound selection mode.
    #[must_use]
    pub const fn selection_mode(&self) -> u32 {
        self.parts.selection_mode
    }

    /// Bound workspace.
    #[must_use]
    pub const fn workspace(&self) -> WorkspaceId {
        self.parts.workspace
    }

    /// Bound semantic epoch.
    #[must_use]
    pub const fn semantic_epoch(&self) -> SchemaEpochId {
        self.parts.semantic_epoch
    }

    /// Bound parent transaction.
    #[must_use]
    pub const fn parent_transaction(&self) -> TransactionId {
        self.parts.parent_transaction
    }

    /// Bound parent root.
    #[must_use]
    pub const fn parent_root(&self) -> StateRoot {
        self.parts.parent_root
    }

    /// Bound proposed root.
    #[must_use]
    pub const fn proposed_root(&self) -> StateRoot {
        self.parts.proposed_root
    }

    /// Bound policy root.
    #[must_use]
    pub const fn policy_root(&self) -> PolicyRootId {
        self.parts.policy_root
    }

    /// Bound candidate identity, if any.
    #[must_use]
    pub const fn candidate_id(&self) -> Option<CandidateId> {
        self.parts.candidate_id
    }

    /// Bound static-result identity, if any.
    #[must_use]
    pub const fn static_result_id(&self) -> Option<CandidateResultId> {
        self.parts.static_result_id
    }

    /// Bound protected required test entities.
    #[must_use]
    pub fn protected_required_ids(&self) -> &[EntityId] {
        &self.parts.protected_required_ids
    }

    /// Bound selected entries in test-ID order.
    #[must_use]
    pub fn selected(&self) -> &[SelectedEntry] {
        &self.parts.selected
    }

    /// Bound changed-test inventory in test-ID order.
    #[must_use]
    pub fn changed(&self) -> &[ChangedTest] {
        &self.parts.changed
    }

    /// Immutable native-v1 execution profile bound into this plan.
    #[must_use]
    pub fn execution_profile(&self) -> sley_id::NativeExecutionProfileId {
        profile_id()
    }

    /// Bound implementation ceilings.
    #[must_use]
    pub const fn implementation_limits(&self) -> NativeImplementationLimits {
        self.parts.implementation_limits
    }

    /// Bound static selected entities.
    #[must_use]
    pub fn static_selected_ids(&self) -> &[EntityId] {
        &self.parts.static_selected_ids
    }

    /// Bound exact resource policy.
    #[must_use]
    pub const fn resource_policy(&self) -> &NativeResourcePolicyV1 {
        &self.parts.resource_policy
    }
}

fn validate_parts(parts: &NativeTestPlanParts) -> Result<(), ScbError> {
    entity_order(&parts.protected_required_ids)?;
    selected_order(&parts.selected)?;
    changed_order(&parts.changed)?;
    entity_order(&parts.static_selected_ids)?;
    if u64::try_from(parts.selected.len()).map_err(|_| resource_error())? > MAX_SELECTED_ENTRIES {
        return Err(resource_error());
    }
    if u64::try_from(parts.changed.len()).map_err(|_| resource_error())? > MAX_CHANGED_ENTRIES {
        return Err(resource_error());
    }
    if !parts.implementation_limits.within_hard_maxima() {
        return Err(resource_error());
    }
    match parts.selection_mode {
        SELECTION_MODE_EXPLICIT_ROOT => {
            if parts.parent_root != parts.proposed_root
                || parts.candidate_id.is_some()
                || parts.static_result_id.is_some()
                || !parts.static_selected_ids.is_empty()
                || !parts.changed.is_empty()
            {
                return Err(ScbError::new(ScbErrorCode::ContractUnknown));
            }
        }
        SELECTION_MODE_CANDIDATE_AFFECTED => {
            if parts.candidate_id.is_none() || parts.static_result_id.is_none() {
                return Err(ScbError::new(ScbErrorCode::FieldMissing));
            }
        }
        _ => return Err(ScbError::new(ScbErrorCode::ContractUnknown)),
    }
    Ok(())
}

fn resource_error() -> ScbError {
    ScbError::new(ScbErrorCode::ResourceLimit)
}

fn entity_keys(ids: &[EntityId]) -> Vec<[u8; 32]> {
    ids.iter().map(|id| *id.as_bytes()).collect()
}

fn entity_order(ids: &[EntityId]) -> Result<(), ScbError> {
    if u64::try_from(ids.len()).map_err(|_| resource_error())? > MAX_CHANGED_ENTRIES {
        return Err(resource_error());
    }
    check_sorted_unique(&entity_keys(ids))
}

fn selected_order(entries: &[SelectedEntry]) -> Result<(), ScbError> {
    let keys: Vec<[u8; 32]> = entries
        .iter()
        .map(|entry| *entry.test_entity.as_bytes())
        .collect();
    check_sorted_unique(&keys)
}

fn changed_order(entries: &[ChangedTest]) -> Result<(), ScbError> {
    let keys: Vec<[u8; 32]> = entries
        .iter()
        .map(|entry| *entry.test_entity.as_bytes())
        .collect();
    check_sorted_unique(&keys)?;
    for entry in entries {
        check_changed(entry)?;
    }
    Ok(())
}

/// At least one of before/after exists, and equal pairs refuse.
fn check_changed(entry: &ChangedTest) -> Result<(), ScbError> {
    if entry.before.is_none() && entry.after.is_none() {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    if entry.before == entry.after {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    Ok(())
}

fn record_parts(parts: &NativeTestPlanParts) -> Result<Vec<u8>, ScbError> {
    let selected = parts
        .selected
        .iter()
        .map(|entry| entry.record())
        .collect::<Result<Vec<_>, _>>()?;
    let changed = parts
        .changed
        .iter()
        .map(|entry| entry.record())
        .collect::<Result<Vec<_>, _>>()?;
    encode_record(&[
        (1, encode_uvar(RECORD_VERSION)),
        (2, encode_uvar(u64::from(parts.selection_mode))),
        (3, parts.workspace.as_bytes().to_vec()),
        (4, parts.semantic_epoch.as_bytes().to_vec()),
        (5, parts.parent_transaction.as_bytes().to_vec()),
        (6, parts.parent_root.as_bytes().to_vec()),
        (7, parts.proposed_root.as_bytes().to_vec()),
        (8, parts.policy_root.as_bytes().to_vec()),
        (
            9,
            encode_option_id(parts.candidate_id.map(|id| *id.as_bytes()))?,
        ),
        (
            10,
            encode_option_id(parts.static_result_id.map(|id| *id.as_bytes()))?,
        ),
        (
            11,
            encode_id_list(&entity_keys(&parts.protected_required_ids))?,
        ),
        (12, encode_list(&selected)?),
        (13, encode_list(&changed)?),
        (14, profile_id().as_bytes().to_vec()),
        (
            15,
            encode_record(&[
                (1, encode_uvar(parts.implementation_limits.max_instructions)),
                (2, encode_uvar(parts.implementation_limits.max_value_units)),
                (3, encode_uvar(parts.implementation_limits.max_output_units)),
                (4, encode_uvar(parts.implementation_limits.max_call_depth)),
                (5, encode_uvar(parts.implementation_limits.max_report_bytes)),
            ])?,
        ),
        (
            16,
            encode_id_list(&entity_keys(&parts.static_selected_ids))?,
        ),
        (17, parts.resource_policy.record_bytes()?),
    ])
}

/// Declared-limit layout, field-for-field identical to the VM owner's private
/// record (`sley-vm` `records.rs`): fuel, memory, output, effects, depth,
/// wall. The cross-crate pin test there fails loudly on any reorder.
fn declared_record(limits: NativeDeclaredLimits) -> Vec<u8> {
    encode_record(&[
        (1, encode_uvar(limits.fuel)),
        (2, encode_uvar(limits.memory_bytes)),
        (3, encode_uvar(limits.output_bytes)),
        (4, encode_uvar(limits.effect_count)),
        (5, encode_uvar(limits.call_depth)),
        (6, encode_uvar(limits.wall_timeout_millis)),
    ])
    .expect("six small fields always encode")
}

fn parse_declared_limits(value: &[u8]) -> Result<NativeDeclaredLimits, ScbError> {
    let fields = decode_fields(value)?;
    expect_tags(&fields, &[1, 2, 3, 4, 5, 6])?;
    Ok(NativeDeclaredLimits {
        fuel: read_uvar_value(&fields[0].1, 64)?,
        memory_bytes: read_uvar_value(&fields[1].1, 64)?,
        output_bytes: read_uvar_value(&fields[2].1, 64)?,
        effect_count: read_uvar_value(&fields[3].1, 64)?,
        call_depth: read_uvar_value(&fields[4].1, 64)?,
        wall_timeout_millis: read_uvar_value(&fields[5].1, 64)?,
    })
}

fn read_entity_list(value: &[u8]) -> Result<Vec<EntityId>, ScbError> {
    read_id_list(value, MAX_CHANGED_ENTRIES)?
        .into_iter()
        .map(|raw| Ok(EntityId::from_bytes(raw)))
        .collect()
}

fn parse_selected(value: &[u8]) -> Result<Vec<SelectedEntry>, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_list_count()?;
    if count > MAX_SELECTED_ENTRIES {
        return Err(resource_error());
    }
    let mut entries = Vec::new();
    for _ in 0..count {
        entries.push(SelectedEntry::parse(cursor.read_bytes()?)?);
    }
    cursor.check_finished()?;
    Ok(entries)
}

fn parse_changed(value: &[u8]) -> Result<Vec<ChangedTest>, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_list_count()?;
    if count > MAX_CHANGED_ENTRIES {
        return Err(resource_error());
    }
    let mut entries = Vec::new();
    for _ in 0..count {
        entries.push(ChangedTest::parse(cursor.read_bytes()?)?);
    }
    cursor.check_finished()?;
    Ok(entries)
}

/// The embedded policy travels as a direct record; the verifier re-envelopes
/// it under `SLEYNRP1` before checking its bound digest in `parse`.
fn envelope_policy(record: &[u8]) -> Result<Vec<u8>, ScbError> {
    let preimage = preimage_bytes(POLICY_MAGIC, record)?;
    let id = NativeResourcePolicyId::derive(&preimage);
    let mut stored = preimage;
    stored.extend_from_slice(id.as_bytes());
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{decode_fields as raw_fields, preimage_bytes as raw_pre};
    use crate::policy::sample_parts as policy_parts;
    use sley_scb1::{encode_list as raw_list, encode_record as raw_record};

    const EMPTY_RECORD: &str = "11010101020101032020202020202020202020202020202020202020202020202020202020202020200420212121212121212121212121212121212121212121212121212121212121212105202222222222222222222222222222222222222222222222222222222222222222062023232323232323232323232323232323232323232323232323232323232323230720232323232323232323232323232323232323232323232323232323232323232308201010101010101010101010101010101010101010101010101010101010101010090200000a0200000b01000c01000d01000e20392927d6c948b9a03a27b87f26ebfe08c65ddf4e831a18e1d82f02dc4abb0bf40f1c05010480ade204020480808020030480808020040280020503808010100100119c020a010101022010101010101010101010101010101010101010101010101010101010101010100320111111111111111111111111111111111111111111111111111111111111111104201212121212121212121212121212121212121212121212121212121212121212051b060103c0843d0205808080800403038080040401000501640601000625090102e807020140030380800404038080040503a08d060601100702800808014009028827071c05010480ade204020480808020030480808020040280020503808010082708010110020480c2d72f030580808080200403808040050100060280080702904e0804808080040903b0ea010a201313131313131313131313131313131313131313131313131313131313131313";
    const EMPTY_ID: &str = "790e68d7efb4379d544810c74fa74d0b66081a060ac1cedfb4fb5c7ccb48d55d";
    const SINGLE_ID: &str = "6ec27f996ac8bb6189a256a7dc0aa600848e9585b78482b4ec9158c8063e5228";

    fn hex_of(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use core::fmt::Write as _;
            write!(out, "{byte:02x}").expect("hex formatting never fails");
        }
        out
    }

    fn policy() -> NativeResourcePolicyV1 {
        NativeResourcePolicyV1::build(policy_parts()).expect("policy builds")
    }

    fn empty_parts() -> NativeTestPlanParts {
        NativeTestPlanParts {
            selection_mode: SELECTION_MODE_EXPLICIT_ROOT,
            workspace: WorkspaceId::from_bytes([0x20; 32]),
            semantic_epoch: SchemaEpochId::from_bytes([0x21; 32]),
            parent_transaction: TransactionId::from_bytes([0x22; 32]),
            parent_root: StateRoot::from_bytes([0x23; 32]),
            proposed_root: StateRoot::from_bytes([0x23; 32]),
            policy_root: PolicyRootId::from_bytes([0x10; 32]),
            candidate_id: None,
            static_result_id: None,
            protected_required_ids: Vec::new(),
            selected: Vec::new(),
            changed: Vec::new(),
            implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
            static_selected_ids: Vec::new(),
            resource_policy: policy(),
        }
    }

    fn single_parts() -> NativeTestPlanParts {
        NativeTestPlanParts {
            selection_mode: SELECTION_MODE_CANDIDATE_AFFECTED,
            workspace: WorkspaceId::from_bytes([0x20; 32]),
            semantic_epoch: SchemaEpochId::from_bytes([0x21; 32]),
            parent_transaction: TransactionId::from_bytes([0x22; 32]),
            parent_root: StateRoot::from_bytes([0x23; 32]),
            proposed_root: StateRoot::from_bytes([0x24; 32]),
            policy_root: PolicyRootId::from_bytes([0x10; 32]),
            candidate_id: Some(CandidateId::from_bytes([0x30; 32])),
            static_result_id: Some(CandidateResultId::from_bytes([0x31; 32])),
            protected_required_ids: vec![EntityId::from_bytes([0x40; 32])],
            selected: vec![SelectedEntry {
                test_entity: EntityId::from_bytes([0x40; 32]),
                test_object: ObjectId::from_bytes([0x41; 32]),
                target_function: EntityId::from_bytes([0x42; 32]),
                target_object: ObjectId::from_bytes([0x43; 32]),
                declared_limits: NativeDeclaredLimits {
                    fuel: 100,
                    memory_bytes: 4_096,
                    output_bytes: 64,
                    effect_count: 0,
                    call_depth: 8,
                    wall_timeout_millis: 1_000,
                },
            }],
            changed: vec![ChangedTest {
                test_entity: EntityId::from_bytes([0x40; 32]),
                before: None,
                after: Some(ObjectId::from_bytes([0x41; 32])),
            }],
            implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
            static_selected_ids: vec![EntityId::from_bytes([0x40; 32])],
            resource_policy: policy(),
        }
    }

    fn entry_at(first: u8, last: u8) -> SelectedEntry {
        let mut raw = [0x00; 32];
        raw[0] = first;
        raw[31] = last;
        SelectedEntry {
            test_entity: EntityId::from_bytes(raw),
            test_object: ObjectId::from_bytes([0x41; 32]),
            target_function: EntityId::from_bytes([0x42; 32]),
            target_object: ObjectId::from_bytes([0x43; 32]),
            declared_limits: NativeDeclaredLimits {
                fuel: 100,
                memory_bytes: 4_096,
                output_bytes: 64,
                effect_count: 0,
                call_depth: 8,
                wall_timeout_millis: 1_000,
            },
        }
    }

    fn re_envelope(record: &[u8]) -> Vec<u8> {
        let preimage = raw_pre(PLAN_MAGIC, record).expect("preimage fits");
        let id = NativeTestPlanId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        stored
    }

    #[test]
    fn empty_plan_matches_independent_python_golden() {
        let plan = NativeTestPlanV1::build(empty_parts()).expect("empty builds");
        assert_eq!(hex_of(plan.record_bytes()), EMPTY_RECORD);
        assert_eq!(hex_of(plan.plan_id().as_bytes()), EMPTY_ID);
        let parsed = NativeTestPlanV1::parse(plan.stored_bytes()).expect("roundtrip");
        assert_eq!(parsed, plan);
        assert_eq!(parsed.selection_mode(), SELECTION_MODE_EXPLICIT_ROOT);
        assert_eq!(parsed.parent_root(), parsed.proposed_root());
        assert!(parsed.selected().is_empty());
        assert_eq!(
            hex_of(parsed.resource_policy().policy_id().as_bytes()),
            "fc18cbb305ec5e94f332a6d0d19003611e486e2f5e8997176be246a7a3177ecd"
        );
    }

    #[test]
    fn single_plan_matches_independent_python_golden() {
        let plan = NativeTestPlanV1::build(single_parts()).expect("single builds");
        let parsed = NativeTestPlanV1::parse(plan.stored_bytes()).expect("roundtrip");
        assert_eq!(parsed, plan);
        assert_eq!(hex_of(plan.plan_id().as_bytes()), SINGLE_ID);
        assert_eq!(parsed.selected().len(), 1);
        assert_eq!(parsed.selected()[0].declared_limits.fuel, 100);
        assert_eq!(parsed.changed()[0].before, None);
        assert_eq!(parsed.candidate_id(), single_parts().candidate_id);
    }

    #[test]
    fn build_refuses_order_count_and_mode_violations() {
        let mut unsorted = single_parts();
        unsorted.selected.push(entry_at(0x39, 0x00));
        assert_eq!(
            NativeTestPlanV1::build(unsorted).expect_err("order").code(),
            ScbErrorCode::FieldOrder
        );
        let mut duplicate = single_parts();
        duplicate.selected.push(duplicate.selected[0]);
        assert_eq!(
            NativeTestPlanV1::build(duplicate)
                .expect_err("duplicate")
                .code(),
            ScbErrorCode::FieldDuplicate
        );
        let mut oversize = single_parts();
        oversize.selected = (0..257_u16)
            .map(|n| entry_at((n >> 8) as u8, (n & 0xFF) as u8))
            .collect();
        assert_eq!(
            NativeTestPlanV1::build(oversize).expect_err("count").code(),
            ScbErrorCode::ResourceLimit
        );
        let mut explicit_candidate = empty_parts();
        explicit_candidate.candidate_id = Some(CandidateId::from_bytes([0x30; 32]));
        assert_eq!(
            NativeTestPlanV1::build(explicit_candidate)
                .expect_err("mode")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut explicit_roots = empty_parts();
        explicit_roots.proposed_root = StateRoot::from_bytes([0x24; 32]);
        assert_eq!(
            NativeTestPlanV1::build(explicit_roots)
                .expect_err("roots")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut candidate_bare = single_parts();
        candidate_bare.static_result_id = None;
        assert_eq!(
            NativeTestPlanV1::build(candidate_bare)
                .expect_err("missing")
                .code(),
            ScbErrorCode::FieldMissing
        );
        let mut bad_mode = single_parts();
        bad_mode.selection_mode = 3;
        assert_eq!(
            NativeTestPlanV1::build(bad_mode)
                .expect_err("mode tag")
                .code(),
            ScbErrorCode::ContractUnknown
        );
    }

    #[test]
    fn selection_count_ceiling_is_exact() {
        let ceiling: Vec<SelectedEntry> = (0..256_u16)
            .map(|n| entry_at((n >> 8) as u8, (n & 0xFF) as u8))
            .collect();
        NativeTestPlanV1::build(NativeTestPlanParts {
            selected: ceiling,
            ..single_parts()
        })
        .expect("256 passes");
    }

    #[test]
    fn changed_inventory_rejects_empty_and_equal_pairs() {
        let mut neither = single_parts();
        neither.changed = vec![ChangedTest {
            test_entity: EntityId::from_bytes([0x40; 32]),
            before: None,
            after: None,
        }];
        assert_eq!(
            NativeTestPlanV1::build(neither)
                .expect_err("neither")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut equal = single_parts();
        equal.changed = vec![ChangedTest {
            test_entity: EntityId::from_bytes([0x40; 32]),
            before: Some(ObjectId::from_bytes([0x41; 32])),
            after: Some(ObjectId::from_bytes([0x41; 32])),
        }];
        assert_eq!(
            NativeTestPlanV1::build(equal).expect_err("equal").code(),
            ScbErrorCode::ContractUnknown
        );
    }

    #[test]
    fn parse_rejects_tampered_profile_and_swapped_order() {
        let plan = NativeTestPlanV1::build(single_parts()).expect("single builds");
        let fields = raw_fields(plan.record_bytes()).expect("golden fields");
        let mut tampered = fields;
        for (tag, value) in &mut tampered {
            if *tag == 14 {
                *value = vec![0x00; 32];
            }
        }
        assert_eq!(
            NativeTestPlanV1::parse(&re_envelope(
                &raw_record(&tampered).expect("tampered encodes")
            ))
            .expect_err("profile")
            .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut two = single_parts();
        two.selected.push(SelectedEntry {
            test_entity: EntityId::from_bytes([0x44; 32]),
            test_object: ObjectId::from_bytes([0x45; 32]),
            target_function: EntityId::from_bytes([0x46; 32]),
            target_object: ObjectId::from_bytes([0x47; 32]),
            declared_limits: two.selected[0].declared_limits,
        });
        let built = NativeTestPlanV1::build(two).expect("two builds");
        let fields = raw_fields(built.record_bytes()).expect("two fields");
        let mut swapped = fields;
        for (tag, value) in &mut swapped {
            if *tag == 12 {
                let mut cursor = ScbValueCursor::new(value).expect("list cursor");
                let count = cursor.read_list_count().expect("count");
                assert_eq!(count, 2);
                let first = cursor.read_bytes().expect("first").to_vec();
                let second = cursor.read_bytes().expect("second").to_vec();
                cursor.check_finished().expect("finished");
                *value = raw_list(&[second, first]).expect("swapped encodes");
            }
        }
        assert_eq!(
            NativeTestPlanV1::parse(&re_envelope(
                &raw_record(&swapped).expect("swapped encodes")
            ))
            .expect_err("order")
            .code(),
            ScbErrorCode::FieldOrder
        );
    }

    #[test]
    fn parse_rejects_digest_mismatch() {
        let plan = NativeTestPlanV1::build(empty_parts()).expect("empty builds");
        let mut stored = plan.stored_bytes().to_vec();
        let last = stored.len() - 1;
        stored[last] ^= 0x01;
        assert_eq!(
            NativeTestPlanV1::parse(&stored).expect_err("digest").code(),
            ScbErrorCode::DigestMismatch
        );
    }
}
