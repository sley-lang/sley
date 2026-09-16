//! Protected native test plan (N4) from `NATIVE_TEST_ADMISSION_V1.md`.
//!
//! This owner derives the stronger native selection from one fully valid
//! static result plus the exact validator-owned candidate, proposed state,
//! and trusted base/policy context. It rechecks every final union member
//! against the six principal grant ceilings, the effective validation
//! limits, the configured native hard caps, the final count, checked
//! aggregate sums, and the evidence cap, then binds the exact effective
//! resource policy and plan. The full-v1 static result bytes are never
//! modified; a non-`Valid` output refuses before any selection derives.

use std::collections::{BTreeMap, BTreeSet};

use sley_id::{CapabilitySummaryDigest, EntityId, PrincipalId, TransactionId};
use sley_mutate::{EntityObject, ProposedEntityState, full_validation_profile_id};
use sley_state_root::AcceptedStateRoot;
use sley_tests::{
    ACCEPTANCE_SIGNATURE_PROFILE_V1, ADMISSION_CLEANUP_MILLIS, CANCEL_BETWEEN_REQUESTS,
    ChangedTest, GrantCeilings, LOCK_WAIT_MILLIS, MAX_EXECUTION_REPORT_STORED,
    MEASUREMENT_PROFILE_V1, NATIVE_WALL_CAP_MILLIS, NativeAdmissionProfileParts,
    NativeAdmissionProfileV1, NativeAggregateLimits, NativeExpected, NativeResourcePolicyParts,
    NativeResourcePolicyV1, NativeTestPlanParts, NativeTestPlanV1, PREPROMOTION_WATCHDOG_MILLIS,
    SELECTION_RULE_NATIVE_V1, SelectedEntry, ValidationLimits,
    plan::{SELECTION_MODE_CANDIDATE_AFFECTED, SELECTION_MODE_EXPLICIT_ROOT},
};
use sley_vm::native_execution::{
    NativeDeclaredLimits, NativeImplementationLimits, observation_capacity_required, profile_id,
};

use super::candidate_program::{CandidateProgram, CandidateProgramError};
use super::candidate_validation::CandidateValidationOutput;
use crate::{AcceptedPolicyRoot, CandidateValidationLimits};

/// Canonical six-field resource order shared by per-test and aggregate checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePlanResource {
    /// Charged VM fuel.
    Fuel,
    /// Supervisor-enforced memory bytes.
    Memory,
    /// Canonical output bytes.
    Output,
    /// Declared effect operations.
    Effects,
    /// Active call depth.
    Depth,
    /// Supervisor wall timeout in milliseconds.
    Wall,
}

impl NativePlanResource {
    /// Frozen native-v1 resource tag in canonical field order.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Fuel => 1,
            Self::Memory => 2,
            Self::Output => 3,
            Self::Effects => 4,
            Self::Depth => 5,
            Self::Wall => 6,
        }
    }

    /// Frozen native-v1 resource symbol with the profile-local prefix.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Fuel => "NATIVE_TEST_FUEL",
            Self::Memory => "NATIVE_TEST_MEMORY_BYTES",
            Self::Output => "NATIVE_TEST_OUTPUT_BYTES",
            Self::Effects => "NATIVE_TEST_EFFECT_COUNT",
            Self::Depth => "NATIVE_TEST_CALL_DEPTH",
            Self::Wall => "NATIVE_TEST_WALL_TIMEOUT_MILLIS",
        }
    }
}

/// Profile-local native plan refusal union. This is a new native error union;
/// no tags are added to the frozen full-v1 candidate-decision/error codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePlanErrorV1 {
    /// Static result is not `Valid`, selection does not resolve, or a
    /// protected required test is missing/deleted.
    SelectionInvalid,
    /// Final selected count exceeds `min(effective, 256)`.
    SelectedCountExceeded,
    /// One selected test declares over its policy ceiling.
    DeclaredLimitExceedsPolicy {
        /// Offending selected test in raw-ID order position.
        test: EntityId,
        /// Ceiling the declaration exceeded.
        resource: NativePlanResource,
    },
    /// A checked aggregate sum overflowed `u64`.
    AggregateOverflow {
        /// Summed resource that overflowed.
        resource: NativePlanResource,
    },
    /// A checked aggregate sum exceeds its aggregate ceiling.
    AggregateLimitExceeded {
        /// Summed resource that exceeded its ceiling.
        resource: NativePlanResource,
    },
    /// Reserved evidence exceeds the per-report or aggregate evidence cap.
    EvidenceLimitExceeded,
}

impl NativePlanErrorV1 {
    /// Frozen native-v1 refusal tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::SelectionInvalid => 1,
            Self::SelectedCountExceeded => 2,
            Self::DeclaredLimitExceedsPolicy { .. } => 3,
            Self::AggregateOverflow { .. } => 4,
            Self::AggregateLimitExceeded { .. } => 5,
            Self::EvidenceLimitExceeded => 6,
        }
    }

    /// Frozen native-v1 refusal symbol with the profile-local prefix.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::SelectionInvalid => "NATIVE_TEST_SELECTION_INVALID",
            Self::SelectedCountExceeded => "NATIVE_TEST_SELECTED_COUNT_EXCEEDED",
            Self::DeclaredLimitExceedsPolicy { .. } => "NATIVE_TEST_DECLARED_LIMIT_EXCEEDS_POLICY",
            Self::AggregateOverflow { .. } => "NATIVE_TEST_AGGREGATE_OVERFLOW",
            Self::AggregateLimitExceeded { .. } => "NATIVE_TEST_AGGREGATE_LIMIT_EXCEEDED",
            Self::EvidenceLimitExceeded => "NATIVE_TEST_EVIDENCE_LIMIT_EXCEEDED",
        }
    }
}

impl core::fmt::Display for NativePlanErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SelectionInvalid | Self::SelectedCountExceeded | Self::EvidenceLimitExceeded => {
                formatter.write_str(self.symbol())
            }
            Self::DeclaredLimitExceedsPolicy { test, resource } => write!(
                formatter,
                "{} test={} resource={}",
                self.symbol(),
                hex_id(test.as_bytes()),
                resource.symbol()
            ),
            Self::AggregateOverflow { resource } | Self::AggregateLimitExceeded { resource } => {
                write!(
                    formatter,
                    "{} resource={}",
                    self.symbol(),
                    resource.symbol()
                )
            }
        }
    }
}

impl std::error::Error for NativePlanErrorV1 {}

fn hex_id(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Trusted owner inputs for native plan derivation. Every value must come
/// from the same accepted parent context that produced the static result;
///
/// constructing these inputs grants no selection authority by itself.
pub struct NativePlanInputs<'a> {
    /// Exact accepted parent transaction the candidate was validated against.
    pub base_transaction_id: TransactionId,
    /// Exact accepted parent state.
    pub base_state: &'a AcceptedStateRoot,
    /// Exact trusted base inventory matching the parent state bindings.
    pub base_objects: &'a [EntityObject],
    /// Exact accepted protected policy root.
    pub policy: &'a AcceptedPolicyRoot,
    /// Canonical capability-summary digest of the granting context.
    pub capability_summary: CapabilitySummaryDigest,
    /// Requested local validation ceilings; the effective minimum applies.
    pub limits: CandidateValidationLimits,
    /// Configured local native implementation ceilings.
    pub implementation_limits: NativeImplementationLimits,
    /// Configured local aggregate ceilings within the native hard maxima.
    pub aggregate: NativeAggregateLimits,
}

/// Returns the exact fixed native admission descriptor every plan binds.
///
/// The descriptor is constant: full-v1 static validation profile, native
/// execution profile, the three accepted rule tags, hard-maximum aggregates,
/// and the fixed lock-wait, watchdog, cleanup, and cancel bounds. The
/// transaction owner rebuilds this descriptor to check the caller's claimed
/// profile identity without accepting caller-chosen authority facts.
///
/// # Errors
///
/// Returns `SelectionInvalid` if the fixed facts ever fail validation, which
/// cannot happen for these constants.
pub fn fixed_native_admission_profile() -> Result<NativeAdmissionProfileV1, NativePlanErrorV1> {
    NativeAdmissionProfileV1::build(NativeAdmissionProfileParts {
        static_validation_profile: full_validation_profile_id()
            .map_err(|_| NativePlanErrorV1::SelectionInvalid)?,
        execution_profile: profile_id(),
        selection_rule: SELECTION_RULE_NATIVE_V1,
        measurement_profile: MEASUREMENT_PROFILE_V1,
        acceptance_signature_profile: ACCEPTANCE_SIGNATURE_PROFILE_V1,
        aggregate_limits: NativeAggregateLimits::HARD_MAXIMA,
        lock_wait_millis: LOCK_WAIT_MILLIS,
        prepromotion_watchdog_millis: PREPROMOTION_WATCHDOG_MILLIS,
        cleanup_millis: ADMISSION_CLEANUP_MILLIS,
        cancel_profile: CANCEL_BETWEEN_REQUESTS,
    })
    .map_err(|_| NativePlanErrorV1::SelectionInvalid)
}

/// Derives the hash-only native expectation for one canonical `TestCase`.
///
/// The transaction owner derives every report expectation itself from the
/// canonical proposed `TestCase` bytes; the test executor never supplies
/// one. An expected value hashes through the shared validated-value
/// fingerprint, and an expected trap code passes through with its frozen
/// 1..=4 range enforced by the report codec at entry build.
///
/// # Errors
///
/// Returns `SelectionInvalid` when the expected value does not fingerprint,
/// which cannot happen for a validated `TestCase`.
pub fn native_expected_outcome(
    schema_epoch: sley_id::SchemaEpochId,
    expected: &sley_ssmc::ExpectedOutcome,
) -> Result<NativeExpected, NativePlanErrorV1> {
    match expected {
        sley_ssmc::ExpectedOutcome::Value(value) => {
            sley_ssmc::fingerprint::hash_validated_value(schema_epoch, value)
                .map(NativeExpected::Value)
                .map_err(|_| NativePlanErrorV1::SelectionInvalid)
        }
        sley_ssmc::ExpectedOutcome::FailureCode(code) => Ok(NativeExpected::FailureCode(*code)),
    }
}

/// Derives the protected native test plan from one fully valid static result.
///
/// This is read-only over the validation output: result bytes are never
/// modified and a non-`Valid` output refuses with `SelectionInvalid` before
/// any selection derives, preserving the existing static failure.
///
/// # Errors
///
/// Returns the first refusal in fixed precedence: static validity, protected
/// selection resolution/order, final count, per-test declared ceilings in
/// raw-ID order across fuel/memory/output/effects/depth/wall, checked
/// aggregate sums in the same field order, then the evidence cap.
pub fn native_test_plan(
    output: &CandidateValidationOutput,
    inputs: &NativePlanInputs<'_>,
) -> Result<NativeTestPlanV1, NativePlanErrorV1> {
    if !output.is_valid() {
        return Err(NativePlanErrorV1::SelectionInvalid);
    }
    let validated = output
        .validated_plan()
        .ok_or(NativePlanErrorV1::SelectionInvalid)?;
    if !inputs.implementation_limits.within_hard_maxima() || !inputs.aggregate.within_hard_maxima()
    {
        return Err(NativePlanErrorV1::SelectionInvalid);
    }
    let effective = inputs.limits.effective();
    let candidate = validated.candidate();
    let proposed = validated.proposed_state();

    let base_program =
        CandidateProgram::project(inputs.base_objects).map_err(|_| selection_invalid())?;
    let program =
        CandidateProgram::project(proposed.entities()).map_err(|_| selection_invalid())?;
    let affected = affected_functions(&base_program, &program, candidate)?;

    let live_tests = live_test_map(&program);
    let base_tests = live_test_map(&base_program);
    let static_selected = static_selected(output)?;
    for selected in &static_selected {
        if !live_tests.contains_key(selected) {
            return Err(selection_invalid());
        }
    }
    let required = required_tests(inputs)?;
    let changed = changed_inventory(proposed, inputs.base_objects, &live_tests, &base_tests)?;

    let mut selection = BTreeSet::new();
    selection.extend(static_selected.iter().copied());
    selection.extend(required.iter().copied());
    for entity in live_tests.keys() {
        let created_or_replaced = changed
            .iter()
            .any(|entry: &ChangedTest| entry.test_entity == *entity && entry.after.is_some());
        if created_or_replaced || affected.contains(entity) {
            selection.insert(*entity);
        }
    }
    let selection: Vec<EntityId> = selection.into_iter().collect();

    let entries = resolve_entries(proposed, &live_tests, &selection)?;
    let max_selected = u64::from(effective.max_selected_tests).min(256);
    if selection.len() as u64 > max_selected {
        return Err(NativePlanErrorV1::SelectedCountExceeded);
    }
    let principal = candidate.record.principal_id;
    check_declared_limits(&entries, inputs, effective, principal)?;
    check_aggregates(&entries, &inputs.aggregate)?;
    check_evidence(
        &entries,
        &live_tests,
        inputs.implementation_limits,
        &inputs.aggregate,
    )?;

    let resource_policy = resource_policy(inputs, effective, candidate)?;
    let parts = NativeTestPlanParts {
        selection_mode: SELECTION_MODE_CANDIDATE_AFFECTED,
        workspace: inputs.base_state.record.workspace_id,
        semantic_epoch: candidate.record.schema_epoch_id,
        parent_transaction: inputs.base_transaction_id,
        parent_root: inputs.base_state.root,
        proposed_root: validated.candidate_root().root,
        policy_root: inputs.policy.root(),
        candidate_id: Some(candidate.candidate_id),
        static_result_id: Some(output.result().candidate_result_id),
        protected_required_ids: required,
        selected: entries,
        changed,
        implementation_limits: inputs.implementation_limits,
        static_selected_ids: static_selected,
        resource_policy,
    };
    NativeTestPlanV1::build(parts).map_err(|_| selection_invalid())
}

const fn selection_invalid() -> NativePlanErrorV1 {
    NativePlanErrorV1::SelectionInvalid
}

/// Trusted owner inputs for explicit-root diagnostic plan derivation
/// (`NATIVE_TEST_ADMISSION_V1.md` appendix C, 601 `tests.selected`).
///
/// Unlike [`NativePlanInputs`], there is no candidate, no static result,
/// and no authenticated principal: the plan tests one accepted state as-is.
/// The diagnostic resource policy marks this with a zero principal and
/// hard-maxima-mapped grant ceilings; the commit path always re-derives
/// under the authenticated principal and never accepts a diagnostic plan.
pub struct NativeExplicitRootInputs<'a> {
    /// Accepted head transaction the session root was bound from.
    pub head_transaction_id: TransactionId,
    /// Exact accepted state under test (equals the session root).
    pub state: &'a AcceptedStateRoot,
    /// Exact live entity objects of that state.
    pub objects: &'a [EntityObject],
    /// Exact accepted protected policy root.
    pub policy: &'a AcceptedPolicyRoot,
    /// Caller-named tests, strictly raw-ID sorted unique.
    pub caller_selected: &'a [EntityId],
    /// Requested local validation ceilings; the effective minimum applies.
    pub limits: CandidateValidationLimits,
    /// Configured local native implementation ceilings.
    pub implementation_limits: NativeImplementationLimits,
    /// Configured local aggregate ceilings within the native hard maxima.
    pub aggregate: NativeAggregateLimits,
}

/// Derives the diagnostic explicit-root plan over one accepted state.
///
/// The final selection is the caller set union the protected required tests
/// resolvable in that state; every named test must resolve live. Aside from
/// the principal-grant ceilings (unchecked: sessions carry no authenticated
/// principal, enforced instead at commit), every candidate-path check
/// applies: final count, per-test depth/wall ceilings, checked aggregates,
/// and the evidence cap, in the same fixed precedence.
///
/// # Errors
///
/// Returns `SelectionInvalid` for an unsorted caller set, an unresolvable
/// test, or a deleted protected required test; `SelectedCountExceeded` for
/// an over-count final set; the declared/aggregate/evidence errors per the
/// candidate path.
pub fn native_test_plan_explicit_root(
    inputs: &NativeExplicitRootInputs<'_>,
) -> Result<NativeTestPlanV1, NativePlanErrorV1> {
    if !inputs.implementation_limits.within_hard_maxima() || !inputs.aggregate.within_hard_maxima()
    {
        return Err(NativePlanErrorV1::SelectionInvalid);
    }
    if inputs
        .caller_selected
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(NativePlanErrorV1::SelectionInvalid);
    }
    let effective = inputs.limits.effective();
    let program = CandidateProgram::project(inputs.objects).map_err(|_| selection_invalid())?;
    let live_tests = live_test_map(&program);
    let objects = inputs
        .objects
        .iter()
        .map(|object| (object.record().entity_id, object.object_id()))
        .collect::<BTreeMap<_, _>>();
    if objects.len() != inputs.objects.len() {
        return Err(selection_invalid());
    }
    let required = required_from_policy(inputs.policy)?;
    for test in inputs.caller_selected.iter().chain(required.iter()) {
        if !live_tests.contains_key(test) {
            return Err(selection_invalid());
        }
    }
    let selection: Vec<EntityId> = inputs
        .caller_selected
        .iter()
        .copied()
        .chain(required.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut entries = Vec::with_capacity(selection.len());
    for entity in &selection {
        let test = live_tests.get(entity).ok_or(selection_invalid())?;
        let test_object = objects.get(entity).copied().ok_or(selection_invalid())?;
        let target_object = objects
            .get(&test.target)
            .copied()
            .ok_or(selection_invalid())?;
        entries.push(selected_entry(test, test_object, target_object));
    }
    let max_selected = u64::from(effective.max_selected_tests).min(256);
    if selection.len() as u64 > max_selected {
        return Err(NativePlanErrorV1::SelectedCountExceeded);
    }
    check_diagnostic_declared_limits(&entries, effective, inputs.implementation_limits)?;
    check_aggregates(&entries, &inputs.aggregate)?;
    check_evidence(
        &entries,
        &live_tests,
        inputs.implementation_limits,
        &inputs.aggregate,
    )?;
    let resource_policy = diagnostic_resource_policy(inputs, effective)?;
    let parts = NativeTestPlanParts {
        selection_mode: SELECTION_MODE_EXPLICIT_ROOT,
        workspace: inputs.state.record.workspace_id,
        semantic_epoch: inputs.state.record.schema_epoch_id,
        parent_transaction: inputs.head_transaction_id,
        parent_root: inputs.state.root,
        proposed_root: inputs.state.root,
        policy_root: inputs.policy.root(),
        candidate_id: None,
        static_result_id: None,
        protected_required_ids: required,
        selected: entries,
        changed: Vec::new(),
        implementation_limits: inputs.implementation_limits,
        static_selected_ids: Vec::new(),
        resource_policy,
    };
    NativeTestPlanV1::build(parts).map_err(|_| selection_invalid())
}

/// Builds the diagnostic resource policy: zero principal, empty capability
/// marker, and hard-maxima-mapped grant ceilings.
///
/// Every value is an explicit diagnostic marker, never a granted ceiling:
/// the commit path re-derives the policy under the authenticated principal.
/// Mutation/adapter scope is unbounded here because diagnostics grant no
/// capability scope at all; tests still run under their declared limits
/// through the configured executor.
///
/// # Errors
///
/// Returns `SelectionInvalid` if the fixed admission facts ever fail
/// validation, which cannot happen for these constants.
fn diagnostic_resource_policy(
    inputs: &NativeExplicitRootInputs<'_>,
    effective: CandidateValidationLimits,
) -> Result<NativeResourcePolicyV1, NativePlanErrorV1> {
    let profile = fixed_native_admission_profile()?;
    let hard = NativeAggregateLimits::HARD_MAXIMA;
    let parts = NativeResourcePolicyParts {
        policy_root: inputs.policy.root(),
        principal: PrincipalId::from_bytes([0; 32]),
        capability_summary: CapabilitySummaryDigest::from_bytes([0; 32]),
        grant: GrantCeilings {
            max_fuel: hard.max_fuel,
            max_memory_bytes: hard.max_memory_sum,
            max_output_bytes: hard.max_output_sum,
            max_effect_count: hard.max_effect_sum,
            max_mutation_count: u64::MAX,
            max_adapter_calls: u64::MAX,
        },
        validation: ValidationLimits {
            max_operations: effective.max_operations,
            max_preconditions: effective.max_preconditions,
            max_candidate_bytes: effective.max_candidate_bytes,
            max_decoded_value_bytes: effective.max_decoded_value_bytes,
            max_graph_work: effective.max_graph_work,
            max_selected_tests: effective.max_selected_tests,
            max_entities: effective.max_entities,
            max_test_call_depth: effective.max_test_call_depth,
            max_test_wall_timeout_millis: effective.max_test_wall_timeout_millis,
        },
        implementation: inputs.implementation_limits,
        aggregate: inputs.aggregate,
        admission_profile: *profile.id().as_bytes(),
    };
    NativeResourcePolicyV1::build(parts).map_err(|_| selection_invalid())
}

/// Checks per-test depth and wall ceilings without a principal grant.
///
/// Sessions carry no authenticated principal, so the fuel/memory/output
/// /effects grant comparisons of [`check_declared_limits`] do not apply:
/// diagnostics enforce the principal-independent depth/wall caps and the
/// commit path enforces the grant under the authenticated principal.
///
/// # Errors
///
/// Returns `DeclaredLimitExceedsPolicy` for the first over-ceiling test in
/// raw-ID order.
fn check_diagnostic_declared_limits(
    entries: &[SelectedEntry],
    effective: CandidateValidationLimits,
    implementation_limits: NativeImplementationLimits,
) -> Result<(), NativePlanErrorV1> {
    let depth_cap = effective
        .max_test_call_depth
        .min(implementation_limits.max_call_depth);
    let wall_cap = effective
        .max_test_wall_timeout_millis
        .min(NATIVE_WALL_CAP_MILLIS);
    for entry in entries {
        let declared = entry.declared_limits;
        for (declared_value, ceiling, resource) in [
            (declared.call_depth, depth_cap, NativePlanResource::Depth),
            (
                declared.wall_timeout_millis,
                wall_cap,
                NativePlanResource::Wall,
            ),
        ] {
            if declared_value > ceiling {
                return Err(NativePlanErrorV1::DeclaredLimitExceedsPolicy {
                    test: entry.test_entity,
                    resource,
                });
            }
        }
    }
    Ok(())
}

fn sorted_union(left: &[EntityId], right: &[EntityId]) -> Vec<EntityId> {
    left.iter()
        .chain(right)
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn affected_functions(
    base_program: &CandidateProgram,
    program: &CandidateProgram,
    candidate: &sley_mutate::ImportedCandidate,
) -> Result<Vec<EntityId>, NativePlanErrorV1> {
    let mut seeds = candidate
        .record
        .operations
        .iter()
        .map(|operation| operation.target_entity)
        .collect::<Vec<_>>();
    seeds.sort_unstable();
    seeds.dedup();
    let map_error = |_: CandidateProgramError| selection_invalid();
    let base_closure = base_program.affected_closure(&seeds).map_err(map_error)?;
    let proposed_closure = program.affected_closure(&seeds).map_err(map_error)?;
    let closure = sorted_union(&base_closure, &proposed_closure);
    let base_functions = base_program.affected_functions(&closure);
    let proposed_functions = program.affected_functions(&closure);
    Ok(sorted_union(&base_functions, &proposed_functions))
}

fn live_test_map(program: &CandidateProgram) -> BTreeMap<EntityId, &sley_ssmc::TestCaseDefinition> {
    program
        .tests
        .iter()
        .map(|test| (test.entity_id, test))
        .collect()
}

fn static_selected(output: &CandidateValidationOutput) -> Result<Vec<EntityId>, NativePlanErrorV1> {
    let selected = output.result().record.selected_tests.clone();
    if selected.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(selection_invalid());
    }
    Ok(selected)
}

fn required_tests(inputs: &NativePlanInputs<'_>) -> Result<Vec<EntityId>, NativePlanErrorV1> {
    required_from_policy(inputs.policy)
}

/// Reads the protected required tests from one policy root.
///
/// The list must already be raw-ID sorted unique; diagnostics share this
/// check with the candidate path so a corrupt policy refuses identically.
fn required_from_policy(policy: &AcceptedPolicyRoot) -> Result<Vec<EntityId>, NativePlanErrorV1> {
    let required = policy.record().required_tests.clone();
    if required.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(selection_invalid());
    }
    Ok(required)
}

/// Builds the complete created/replaced/deleted `TestCase` inventory.
///
/// Created tests have no base object; replaced tests have a differing
/// proposed object; deleted tests are absent from the live proposed map.
/// A deleted protected required test fails; deleted nonrequired tests are
/// recorded but never executed.
fn changed_inventory(
    proposed: &ProposedEntityState,
    base_objects: &[EntityObject],
    live_tests: &BTreeMap<EntityId, &sley_ssmc::TestCaseDefinition>,
    base_tests: &BTreeMap<EntityId, &sley_ssmc::TestCaseDefinition>,
) -> Result<Vec<ChangedTest>, NativePlanErrorV1> {
    let base_map = base_objects
        .iter()
        .map(|object| (object.record().entity_id, object))
        .collect::<BTreeMap<_, _>>();
    let mut changed = BTreeMap::new();
    for entity in proposed.affected_entities() {
        if !live_tests.contains_key(entity) {
            continue;
        }
        let after = proposed
            .entity(*entity)
            .map(EntityObject::object_id)
            .ok_or(selection_invalid())?;
        match base_map.get(entity) {
            None => {
                changed.insert(
                    *entity,
                    ChangedTest {
                        test_entity: *entity,
                        before: None,
                        after: Some(after),
                    },
                );
            }
            Some(base) if base.object_id() != after => {
                changed.insert(
                    *entity,
                    ChangedTest {
                        test_entity: *entity,
                        before: Some(base.object_id()),
                        after: Some(after),
                    },
                );
            }
            Some(_) => {}
        }
    }
    for entity in proposed.deleted_entities() {
        if !base_tests.contains_key(entity) {
            continue;
        }
        let before = base_map
            .get(entity)
            .map(|object| object.object_id())
            .ok_or(selection_invalid())?;
        changed.insert(
            *entity,
            ChangedTest {
                test_entity: *entity,
                before: Some(before),
                after: None,
            },
        );
    }
    Ok(changed.into_values().collect())
}

fn resolve_entries(
    proposed: &ProposedEntityState,
    live_tests: &BTreeMap<EntityId, &sley_ssmc::TestCaseDefinition>,
    selection: &[EntityId],
) -> Result<Vec<SelectedEntry>, NativePlanErrorV1> {
    let mut entries = Vec::with_capacity(selection.len());
    for entity in selection {
        let test = live_tests.get(entity).ok_or(selection_invalid())?;
        let test_object = proposed
            .entity(*entity)
            .map(EntityObject::object_id)
            .ok_or(selection_invalid())?;
        let target_object = proposed
            .entity(test.target)
            .map(EntityObject::object_id)
            .ok_or(selection_invalid())?;
        entries.push(selected_entry(test, test_object, target_object));
    }
    Ok(entries)
}

/// Binds one selected test to its exact test and target objects.
///
/// Candidate and explicit-root resolution share this constructor so the
/// entry shape cannot diverge between the commit and diagnostic paths.
fn selected_entry(
    test: &sley_ssmc::TestCaseDefinition,
    test_object: sley_id::ObjectId,
    target_object: sley_id::ObjectId,
) -> SelectedEntry {
    SelectedEntry {
        test_entity: test.entity_id,
        test_object,
        target_function: test.target,
        target_object,
        declared_limits: NativeDeclaredLimits::from(test.resource_limits),
    }
}

fn check_declared_limits(
    entries: &[SelectedEntry],
    inputs: &NativePlanInputs<'_>,
    effective: CandidateValidationLimits,
    principal: sley_id::PrincipalId,
) -> Result<(), NativePlanErrorV1> {
    let grant = inputs
        .policy
        .principal_grant(principal)
        .map_err(|_| selection_invalid())?;
    let ceilings = grant.resource_ceilings();
    let depth_cap = effective
        .max_test_call_depth
        .min(inputs.implementation_limits.max_call_depth);
    let wall_cap = effective
        .max_test_wall_timeout_millis
        .min(NATIVE_WALL_CAP_MILLIS);
    for entry in entries {
        let declared = entry.declared_limits;
        let pairs = [
            (declared.fuel, ceilings.max_fuel, NativePlanResource::Fuel),
            (
                declared.memory_bytes,
                ceilings.max_memory_bytes,
                NativePlanResource::Memory,
            ),
            (
                declared.output_bytes,
                ceilings.max_output_bytes,
                NativePlanResource::Output,
            ),
            (
                declared.effect_count,
                ceilings.max_effect_count,
                NativePlanResource::Effects,
            ),
            (declared.call_depth, depth_cap, NativePlanResource::Depth),
            (
                declared.wall_timeout_millis,
                wall_cap,
                NativePlanResource::Wall,
            ),
        ];
        for (declared_value, ceiling, resource) in pairs {
            if declared_value > ceiling {
                return Err(NativePlanErrorV1::DeclaredLimitExceedsPolicy {
                    test: entry.test_entity,
                    resource,
                });
            }
        }
    }
    Ok(())
}

fn entry_values(entries: &[SelectedEntry], resource: NativePlanResource) -> Vec<u64> {
    entries
        .iter()
        .map(|entry| {
            let declared = entry.declared_limits;
            match resource {
                NativePlanResource::Fuel => declared.fuel,
                NativePlanResource::Memory => declared.memory_bytes,
                NativePlanResource::Output => declared.output_bytes,
                NativePlanResource::Effects => declared.effect_count,
                NativePlanResource::Depth => declared.call_depth,
                NativePlanResource::Wall => declared.wall_timeout_millis,
            }
        })
        .collect()
}

fn check_aggregates(
    entries: &[SelectedEntry],
    aggregate: &NativeAggregateLimits,
) -> Result<(), NativePlanErrorV1> {
    let ceilings = [
        (aggregate.max_fuel, NativePlanResource::Fuel),
        (aggregate.max_memory_sum, NativePlanResource::Memory),
        (aggregate.max_output_sum, NativePlanResource::Output),
        (aggregate.max_effect_sum, NativePlanResource::Effects),
        (aggregate.max_depth_sum, NativePlanResource::Depth),
        (aggregate.max_wall_millis_sum, NativePlanResource::Wall),
    ];
    for (ceiling, resource) in ceilings {
        let sum = checked_sum(&entry_values(entries, resource), resource)?;
        if sum > ceiling {
            return Err(NativePlanErrorV1::AggregateLimitExceeded { resource });
        }
    }
    Ok(())
}

/// Checked `u64` sum over one resource column. The policy owner caps every
/// grant ceiling at `MAX_POLICY_RESOURCE_CEILING` and the final count at
/// 256, so accepted sums stay far below `u64::MAX`; the check still refuses
/// instead of wrapping if those bounds ever widen.
fn checked_sum(values: &[u64], resource: NativePlanResource) -> Result<u64, NativePlanErrorV1> {
    let mut sum = 0_u64;
    for value in values {
        sum = sum
            .checked_add(*value)
            .ok_or(NativePlanErrorV1::AggregateOverflow { resource })?;
    }
    Ok(sum)
}

fn check_evidence(
    entries: &[SelectedEntry],
    live_tests: &BTreeMap<EntityId, &sley_ssmc::TestCaseDefinition>,
    implementation_limits: NativeImplementationLimits,
    aggregate: &NativeAggregateLimits,
) -> Result<(), NativePlanErrorV1> {
    let mut total = 0_u64;
    for entry in entries {
        // Input counts come from the validator-owned proposed program, never
        // from caller claims.
        let test_inputs = live_tests
            .get(&entry.test_entity)
            .map(|test| test.inputs.len())
            .ok_or(selection_invalid())?;
        let required = observation_capacity_required(
            test_inputs,
            entry.declared_limits,
            implementation_limits,
        )
        .map_err(|_| NativePlanErrorV1::EvidenceLimitExceeded)?;
        if required > MAX_EXECUTION_REPORT_STORED as u64 {
            return Err(NativePlanErrorV1::EvidenceLimitExceeded);
        }
        total = total
            .checked_add(required)
            .ok_or(NativePlanErrorV1::EvidenceLimitExceeded)?;
    }
    if total > aggregate.max_evidence_bytes {
        return Err(NativePlanErrorV1::EvidenceLimitExceeded);
    }
    Ok(())
}

fn resource_policy(
    inputs: &NativePlanInputs<'_>,
    effective: CandidateValidationLimits,
    candidate: &sley_mutate::ImportedCandidate,
) -> Result<NativeResourcePolicyV1, NativePlanErrorV1> {
    let grant = inputs
        .policy
        .principal_grant(candidate.record.principal_id)
        .map_err(|_| selection_invalid())?;
    let ceilings = grant.resource_ceilings();
    let profile = fixed_native_admission_profile()?;
    let parts = NativeResourcePolicyParts {
        policy_root: inputs.policy.root(),
        principal: candidate.record.principal_id,
        capability_summary: inputs.capability_summary,
        grant: GrantCeilings {
            max_fuel: ceilings.max_fuel,
            max_memory_bytes: ceilings.max_memory_bytes,
            max_output_bytes: ceilings.max_output_bytes,
            max_effect_count: ceilings.max_effect_count,
            max_mutation_count: ceilings.max_mutation_count,
            max_adapter_calls: ceilings.max_adapter_calls,
        },
        validation: ValidationLimits {
            max_operations: effective.max_operations,
            max_preconditions: effective.max_preconditions,
            max_candidate_bytes: effective.max_candidate_bytes,
            max_decoded_value_bytes: effective.max_decoded_value_bytes,
            max_graph_work: effective.max_graph_work,
            max_selected_tests: effective.max_selected_tests,
            max_entities: effective.max_entities,
            max_test_call_depth: effective.max_test_call_depth,
            max_test_wall_timeout_millis: effective.max_test_wall_timeout_millis,
        },
        implementation: inputs.implementation_limits,
        aggregate: inputs.aggregate,
        admission_profile: *profile.id().as_bytes(),
    };
    NativeResourcePolicyV1::build(parts).map_err(|_| selection_invalid())
}

#[cfg(test)]
mod tests {
    use sley_id::{CandidateNonce, CapabilitySummaryDigest, EntityId, ObjectId};
    use sley_mutate::value::{
        BlockBody, EntityBodyValue, EntityIdSet, FunctionBody, ParameterBody, TestCaseBody,
    };
    use sley_ssmc::{
        ConstData, ConstValue, EffectEnvironment, ExpectedOutcome, ParameterRole, Reachability,
        ResourceLimits, ReturnTerminator, Terminator, TypeExpr, ValueRef, Visibility,
    };
    use sley_state_root::{StateRootBuilder, conformance_registry as state_registry};
    use sley_tests::NativeAggregateLimits;
    use sley_vm::native_execution::NativeImplementationLimits;

    use super::super::candidate_validation::tests::{Fixture, fixed};
    use super::*;
    use crate::validate_candidate_bytes;

    fn unit() -> ConstValue {
        ConstValue {
            value_type: TypeExpr::Unit,
            data: ConstData::Unit,
        }
    }

    fn ceiling_limits() -> ResourceLimits {
        ResourceLimits {
            fuel: 1_000,
            memory_bytes: 1,
            output_bytes: 1,
            effect_count: 1,
            call_depth: 1,
            wall_timeout_millis: 1,
        }
    }

    fn function_test_bodies(
        fixture: &Fixture,
        nonce_byte: u8,
        limits: ResourceLimits,
    ) -> Vec<(u16, EntityBodyValue)> {
        let function = fixture.created_id(nonce_byte, 5, 0);
        let parameter = fixture.created_id(nonce_byte, 6, 1);
        let block = fixture.created_id(nonce_byte, 7, 2);
        vec![
            (
                5,
                EntityBodyValue::Function(FunctionBody {
                    type_parameters: vec![],
                    parameters: vec![parameter],
                    result_type: TypeExpr::Unit,
                    effects: EntityIdSet::from_unsorted(vec![]).unwrap(),
                    entry_block: block,
                    blocks: vec![block],
                    contracts: EntityIdSet::from_unsorted(vec![]).unwrap(),
                    visibility: Visibility::Private,
                }),
            ),
            (
                6,
                EntityBodyValue::Parameter(ParameterBody {
                    owner: function,
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: TypeExpr::Unit,
                }),
            ),
            (
                7,
                EntityBodyValue::Block(BlockBody {
                    function,
                    parameters: vec![],
                    operations: vec![],
                    terminator: Terminator::Return(ReturnTerminator {
                        value: ValueRef::Parameter(parameter),
                    }),
                    reachability: Reachability::Required,
                }),
            ),
            (
                14,
                EntityBodyValue::TestCase(TestCaseBody {
                    target: function,
                    inputs: vec![unit()],
                    effect_environment: EffectEnvironment::Replay(vec![]),
                    expected: ExpectedOutcome::Value(unit()),
                    observations: vec![],
                    resource_limits: limits,
                }),
            ),
        ]
    }

    fn inputs(fixture: &Fixture, summary: CapabilitySummaryDigest) -> NativePlanInputs<'_> {
        NativePlanInputs {
            base_transaction_id: fixture.transaction_id,
            base_state: &fixture.base_state,
            base_objects: &fixture.base_objects,
            policy: &fixture.policy,
            capability_summary: summary,
            limits: crate::CandidateValidationLimits::full_v1(),
            implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
            aggregate: NativeAggregateLimits::HARD_MAXIMA,
        }
    }

    fn validated_plan_fixture(
        required: &[EntityId],
        nonce_byte: u8,
        limits: ResourceLimits,
    ) -> (
        Fixture,
        CapabilitySummaryDigest,
        crate::CandidateValidationOutput,
    ) {
        let fixture = Fixture::with_policy_options_and_tests(true, false, None, required);
        let candidate = fixture.create_candidate(
            nonce_byte,
            function_test_bodies(&fixture, nonce_byte, limits),
        );
        let context = fixture.context();
        let summary = context.capability_summary_digest();
        let output =
            validate_candidate_bytes(&context, &candidate.stored_bytes).expect("output builds");
        (fixture, summary, output)
    }

    /// Builds explicit-root diagnostic inputs over the validated fixture's
    /// proposed state, rebuilt as an accepted state the same way the
    /// deletion tests rebuild theirs.
    fn explicit_inputs<'a>(
        fixture: &'a Fixture,
        objects: &'a [sley_mutate::EntityObject],
        state: &'a sley_state_root::AcceptedStateRoot,
        caller_selected: &'a [EntityId],
    ) -> NativeExplicitRootInputs<'a> {
        NativeExplicitRootInputs {
            head_transaction_id: fixture.transaction_id,
            state,
            objects,
            policy: &fixture.policy,
            caller_selected,
            limits: crate::CandidateValidationLimits::full_v1(),
            implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
            aggregate: NativeAggregateLimits::HARD_MAXIMA,
        }
    }

    fn accepted_over(
        fixture: &Fixture,
        objects: &[sley_mutate::EntityObject],
    ) -> sley_state_root::AcceptedStateRoot {
        let mut builder = StateRootBuilder::new(
            fixture.workspace_id,
            fixed(20, ObjectId::from_bytes),
            fixed(21, ObjectId::from_bytes),
            fixture.policy.root(),
        );
        for object in objects {
            builder = builder.entity_binding(object.record().entity_id, object.object_id());
        }
        builder.build(&state_registry().unwrap()).unwrap()
    }

    #[test]
    fn explicit_root_derives_caller_selection_with_exact_objects() {
        let test = fixed(73, CandidateNonce::from_bytes);
        let test = EntityId::derive(fixed(1, sley_id::WorkspaceId::from_bytes), test, 14, 3);
        let (fixture, _, output) = validated_plan_fixture(&[], 73, ceiling_limits());
        assert!(output.is_valid());
        let proposed = output
            .validated_plan()
            .expect("valid output carries a plan")
            .proposed_state();
        // Rebuild the proposed entities as the accepted session-root state.
        let objects: Vec<sley_mutate::EntityObject> = proposed.entities().to_vec();
        let state = accepted_over(&fixture, &objects);
        let selected = [test];
        let plan =
            native_test_plan_explicit_root(&explicit_inputs(&fixture, &objects, &state, &selected))
                .expect("plan derives");
        assert_eq!(plan.selection_mode(), SELECTION_MODE_EXPLICIT_ROOT);
        assert_eq!(plan.selected().len(), 1);
        let entry = plan.selected()[0];
        assert_eq!(entry.test_entity, test);
        let function = fixture.created_id(73, 5, 0);
        assert_eq!(entry.target_function, function);
        assert_eq!(plan.candidate_id(), None);
        assert_eq!(plan.static_result_id(), None);
        assert!(plan.changed().is_empty());
        assert!(plan.static_selected_ids().is_empty());
        assert_eq!(plan.parent_root(), plan.proposed_root());
        assert_eq!(plan.proposed_root(), state.root);
        // The diagnostic policy marks the missing principal explicitly and
        // maps the hard maxima; it can never authorize a commit.
        assert_eq!(
            plan.resource_policy().principal(),
            sley_id::PrincipalId::from_bytes([0; 32])
        );
        assert_eq!(
            plan.resource_policy().grant().max_fuel,
            NativeAggregateLimits::HARD_MAXIMA.max_fuel
        );
        let repeat =
            native_test_plan_explicit_root(&explicit_inputs(&fixture, &objects, &state, &selected))
                .expect("plan re-derives");
        assert_eq!(plan.plan_id(), repeat.plan_id());
        assert_eq!(plan.stored_bytes(), repeat.stored_bytes());
    }

    #[test]
    fn explicit_root_adds_required_tests_to_an_empty_caller_set() {
        let test = fixed(73, CandidateNonce::from_bytes);
        let test = EntityId::derive(fixed(1, sley_id::WorkspaceId::from_bytes), test, 14, 3);
        let (fixture, _, output) = validated_plan_fixture(&[test], 73, ceiling_limits());
        assert!(output.is_valid());
        let proposed = output
            .validated_plan()
            .expect("valid output carries a plan")
            .proposed_state();
        let objects: Vec<sley_mutate::EntityObject> = proposed.entities().to_vec();
        let state = accepted_over(&fixture, &objects);
        let selected: [EntityId; 0] = [];
        let plan =
            native_test_plan_explicit_root(&explicit_inputs(&fixture, &objects, &state, &selected))
                .expect("plan derives");
        assert_eq!(plan.selected().len(), 1);
        assert_eq!(plan.selected()[0].test_entity, test);
        assert_eq!(plan.protected_required_ids(), &[test]);
    }

    #[test]
    fn explicit_root_refuses_unknown_and_unsorted_caller_tests() {
        let (fixture, _, output) = validated_plan_fixture(&[], 73, ceiling_limits());
        assert!(output.is_valid());
        let proposed = output
            .validated_plan()
            .expect("valid output carries a plan")
            .proposed_state();
        let objects: Vec<sley_mutate::EntityObject> = proposed.entities().to_vec();
        let state = accepted_over(&fixture, &objects);
        let unknown = [EntityId::from_bytes([0x77; 32])];
        assert_eq!(
            native_test_plan_explicit_root(&explicit_inputs(&fixture, &objects, &state, &unknown))
                .expect_err("refuses"),
            NativePlanErrorV1::SelectionInvalid
        );
        // An unsorted caller set refuses even when every member resolves.
        let test = fixture.created_id(73, 14, 3);
        let function = fixture.created_id(73, 5, 0);
        let unsorted = [test.max(function), test.min(function)];
        assert!(unsorted[0] > unsorted[1]);
        assert_eq!(
            native_test_plan_explicit_root(&explicit_inputs(&fixture, &objects, &state, &unsorted))
                .expect_err("refuses"),
            NativePlanErrorV1::SelectionInvalid
        );
    }

    #[test]
    fn explicit_root_skips_grant_ceilings_but_checks_wall() {
        // Fuel over the fixture grant (1,000) but under the hard maxima:
        // static validation would refuse this candidate, so the objects
        // are built directly into the accepted state and the diagnostic
        // admits what the candidate path cannot.
        use sley_mutate::{EntityObjectRecord, build_entity_object};
        use sley_state_root::conformance_epoch_id as state_epoch_id;
        let fixture = Fixture::valid();
        let epoch = state_epoch_id().unwrap();
        let over_grant = ResourceLimits {
            fuel: 1_000_000,
            ..ceiling_limits()
        };
        let bodies = function_test_bodies(&fixture, 73, over_grant);
        let ids = [
            fixture.created_id(73, 5, 0),
            fixture.created_id(73, 6, 1),
            fixture.created_id(73, 7, 2),
            fixture.created_id(73, 14, 3),
        ];
        let mut objects = fixture.base_objects.clone();
        for ((_, body), entity_id) in bodies.into_iter().zip(ids) {
            objects.push(
                build_entity_object(
                    epoch,
                    &EntityObjectRecord {
                        entity_id,
                        body,
                        label: None,
                        semantic_fingerprint: None,
                    },
                )
                .unwrap(),
            );
        }
        objects.sort_by_key(|object| object.record().entity_id);
        let state = accepted_over(&fixture, &objects);
        let selected = [fixture.created_id(73, 14, 3)];
        let plan =
            native_test_plan_explicit_root(&explicit_inputs(&fixture, &objects, &state, &selected))
                .expect("diagnostic admits over-grant fuel");
        assert_eq!(plan.selected().len(), 1);
        // Wall over the native cap still refuses with test and resource.
        let over_wall = ResourceLimits {
            wall_timeout_millis: NATIVE_WALL_CAP_MILLIS + 1,
            ..ceiling_limits()
        };
        let (wfixture, _, woutput) = validated_plan_fixture(&[], 73, over_wall);
        assert!(woutput.is_valid());
        let wproposed = woutput
            .validated_plan()
            .expect("valid output carries a plan")
            .proposed_state();
        let wobjects: Vec<sley_mutate::EntityObject> = wproposed.entities().to_vec();
        let wstate = accepted_over(&wfixture, &wobjects);
        let wselected = [wfixture.created_id(73, 14, 3)];
        assert_eq!(
            native_test_plan_explicit_root(&explicit_inputs(
                &wfixture, &wobjects, &wstate, &wselected
            ))
            .expect_err("refuses"),
            NativePlanErrorV1::DeclaredLimitExceedsPolicy {
                test: wselected[0],
                resource: NativePlanResource::Wall,
            }
        );
    }

    #[test]
    fn explicit_root_count_exceeded_refuses() {
        let (fixture, _, output) = validated_plan_fixture(&[], 73, ceiling_limits());
        assert!(output.is_valid());
        let proposed = output
            .validated_plan()
            .expect("valid output carries a plan")
            .proposed_state();
        let objects: Vec<sley_mutate::EntityObject> = proposed.entities().to_vec();
        let state = accepted_over(&fixture, &objects);
        let selected = [fixture.created_id(73, 14, 3)];
        let mut tight = explicit_inputs(&fixture, &objects, &state, &selected);
        tight.limits.max_selected_tests = 0;
        assert_eq!(
            native_test_plan_explicit_root(&tight).expect_err("refuses"),
            NativePlanErrorV1::SelectedCountExceeded
        );
    }

    #[test]
    fn error_tags_and_symbols_are_frozen() {
        assert_eq!(NativePlanErrorV1::SelectionInvalid.tag(), 1);
        assert_eq!(NativePlanErrorV1::SelectedCountExceeded.tag(), 2);
        assert_eq!(
            NativePlanErrorV1::DeclaredLimitExceedsPolicy {
                test: EntityId::from_bytes([9; 32]),
                resource: NativePlanResource::Fuel,
            }
            .tag(),
            3
        );
        assert_eq!(
            NativePlanErrorV1::AggregateOverflow {
                resource: NativePlanResource::Wall,
            }
            .tag(),
            4
        );
        assert_eq!(
            NativePlanErrorV1::AggregateLimitExceeded {
                resource: NativePlanResource::Memory,
            }
            .tag(),
            5
        );
        assert_eq!(NativePlanErrorV1::EvidenceLimitExceeded.tag(), 6);
        assert_eq!(
            NativePlanErrorV1::SelectionInvalid.symbol(),
            "NATIVE_TEST_SELECTION_INVALID"
        );
        assert_eq!(
            NativePlanErrorV1::SelectedCountExceeded.symbol(),
            "NATIVE_TEST_SELECTED_COUNT_EXCEEDED"
        );
        assert_eq!(
            NativePlanErrorV1::EvidenceLimitExceeded.symbol(),
            "NATIVE_TEST_EVIDENCE_LIMIT_EXCEEDED"
        );
        assert_eq!(
            [
                NativePlanResource::Fuel.tag(),
                NativePlanResource::Memory.tag(),
                NativePlanResource::Output.tag(),
                NativePlanResource::Effects.tag(),
                NativePlanResource::Depth.tag(),
                NativePlanResource::Wall.tag(),
            ],
            [1, 2, 3, 4, 5, 6]
        );
        assert_eq!(NativePlanResource::Fuel.symbol(), "NATIVE_TEST_FUEL");
        assert_eq!(
            NativePlanResource::Wall.symbol(),
            "NATIVE_TEST_WALL_TIMEOUT_MILLIS"
        );
    }

    #[test]
    fn created_test_is_selected_with_exact_objects_and_limits() {
        let test = fixed(73, CandidateNonce::from_bytes);
        let test = EntityId::derive(fixed(1, sley_id::WorkspaceId::from_bytes), test, 14, 3);
        let (fixture, summary, output) = validated_plan_fixture(&[test], 73, ceiling_limits());
        assert!(output.is_valid());
        let before = output.result().stored_bytes.clone();

        let plan = native_test_plan(&output, &inputs(&fixture, summary)).expect("plan derives");
        // The read-only API never modifies the static result bytes.
        assert_eq!(output.result().stored_bytes, before);

        let function = fixture.created_id(73, 5, 0);
        let proposed = output
            .validated_plan()
            .expect("valid output carries a plan")
            .proposed_state();
        let test_object = proposed.entity(test).expect("test live").object_id();
        let target_object = proposed.entity(function).expect("target live").object_id();
        assert_eq!(plan.selected().len(), 1);
        let entry = plan.selected()[0];
        assert_eq!(entry.test_entity, test);
        assert_eq!(entry.test_object, test_object);
        assert_eq!(entry.target_function, function);
        assert_eq!(entry.target_object, target_object);
        assert_eq!(
            entry.declared_limits,
            sley_vm::native_execution::NativeDeclaredLimits::from(ceiling_limits())
        );
        assert_eq!(plan.static_selected_ids(), &[test]);
        assert_eq!(plan.changed().len(), 1);
        assert_eq!(plan.changed()[0].test_entity, test);
        assert_eq!(plan.changed()[0].before, None);
        assert_eq!(plan.changed()[0].after, Some(test_object));
        // Boundary equality with the grant ceiling passes without clamping.
        assert_eq!(plan.resource_policy().grant().max_fuel, 1_000);
        assert_eq!(
            plan.resource_policy().aggregate(),
            NativeAggregateLimits::HARD_MAXIMA
        );

        let repeat =
            native_test_plan(&output, &inputs(&fixture, summary)).expect("plan re-derives");
        assert_eq!(plan.plan_id(), repeat.plan_id());
        assert_eq!(plan.stored_bytes(), repeat.stored_bytes());
    }

    #[test]
    fn empty_selection_plan_admits_test_free_candidate() {
        let fixture = Fixture::valid();
        let context = fixture.context();
        let summary = context.capability_summary_digest();
        let output = validate_candidate_bytes(&context, &fixture.candidate.stored_bytes)
            .expect("output builds");
        assert!(output.is_valid());
        let plan = native_test_plan(&output, &inputs(&fixture, summary)).expect("plan derives");
        assert!(plan.selected().is_empty());
        assert!(plan.changed().is_empty());
        assert!(plan.static_selected_ids().is_empty());
    }

    #[test]
    fn static_failure_is_preserved_before_native_selection() {
        let over = ResourceLimits {
            fuel: 1_000_000,
            ..ceiling_limits()
        };
        let (fixture, summary, output) = validated_plan_fixture(&[], 73, over);
        assert!(!output.is_valid());
        assert_eq!(
            native_test_plan(&output, &inputs(&fixture, summary)).expect_err("refuses"),
            NativePlanErrorV1::SelectionInvalid
        );
    }

    #[test]
    fn wall_over_native_cap_refuses_with_test_and_resource() {
        let wall = ResourceLimits {
            wall_timeout_millis: NATIVE_WALL_CAP_MILLIS + 1,
            ..ceiling_limits()
        };
        let (fixture, summary, output) = validated_plan_fixture(&[], 73, wall);
        assert!(output.is_valid());
        let test = fixture.created_id(73, 14, 3);
        assert_eq!(
            native_test_plan(&output, &inputs(&fixture, summary)).expect_err("refuses"),
            NativePlanErrorV1::DeclaredLimitExceedsPolicy {
                test,
                resource: NativePlanResource::Wall,
            }
        );
    }

    #[test]
    fn depth_over_configured_implementation_cap_refuses() {
        let (fixture, summary, output) = validated_plan_fixture(&[], 73, ceiling_limits());
        assert!(output.is_valid());
        let mut tight = inputs(&fixture, summary);
        tight.implementation_limits = NativeImplementationLimits {
            max_call_depth: 0,
            ..NativeImplementationLimits::HARD_MAXIMA
        };
        let test = fixture.created_id(73, 14, 3);
        assert_eq!(
            native_test_plan(&output, &tight).expect_err("refuses"),
            NativePlanErrorV1::DeclaredLimitExceedsPolicy {
                test,
                resource: NativePlanResource::Depth,
            }
        );
    }

    #[test]
    fn tightened_aggregate_fuel_ceiling_refuses() {
        let (fixture, summary, output) = validated_plan_fixture(&[], 73, ceiling_limits());
        assert!(output.is_valid());
        let mut tight = inputs(&fixture, summary);
        tight.aggregate = NativeAggregateLimits {
            max_fuel: 999,
            ..NativeAggregateLimits::HARD_MAXIMA
        };
        assert_eq!(
            native_test_plan(&output, &tight).expect_err("refuses"),
            NativePlanErrorV1::AggregateLimitExceeded {
                resource: NativePlanResource::Fuel,
            }
        );
    }

    #[test]
    fn tightened_evidence_cap_refuses() {
        let (fixture, summary, output) = validated_plan_fixture(&[], 73, ceiling_limits());
        assert!(output.is_valid());
        let mut tight = inputs(&fixture, summary);
        tight.aggregate = NativeAggregateLimits {
            max_evidence_bytes: 0,
            ..NativeAggregateLimits::HARD_MAXIMA
        };
        assert_eq!(
            native_test_plan(&output, &tight).expect_err("refuses"),
            NativePlanErrorV1::EvidenceLimitExceeded
        );
    }

    /// Checked-sum proof: saturating inputs refuse instead of wrapping.
    /// Grant ceilings are capped by the policy owner and the final count at
    /// 256, so this arm is unreachable through accepted roots today; the
    /// arithmetic still refuses rather than assuming those bounds forever.
    #[test]
    fn checked_sums_refuse_overflow_instead_of_wrapping() {
        assert_eq!(
            checked_sum(&[u64::MAX, 1], NativePlanResource::Fuel).expect_err("overflows"),
            NativePlanErrorV1::AggregateOverflow {
                resource: NativePlanResource::Fuel,
            }
        );
        assert_eq!(
            checked_sum(&[u64::MAX, u64::MAX], NativePlanResource::Wall).expect_err("overflows"),
            NativePlanErrorV1::AggregateOverflow {
                resource: NativePlanResource::Wall,
            }
        );
        assert_eq!(
            checked_sum(&[1, 2, 3], NativePlanResource::Memory).expect("sums"),
            6
        );
        assert_eq!(
            checked_sum(&[], NativePlanResource::Effects).expect("empty sums"),
            0
        );
    }
}

#[cfg(test)]
mod deletion_tests {
    use sley_id::{CandidateNonce, ObjectId, PrincipalId, TransactionId, WorkspaceId};
    use sley_mutate::{
        BoundPrecondition, CandidateExpiry, CandidateRecord, EntityObject, EntityObjectRecord,
        ExactEntityVersion, ImportedCandidate, MutationClass, MutationOperation, MutationPayload,
        PreconditionPayload, PreimageRequirement, build_candidate, build_entity_object,
        full_validation_profile_id,
        value::{
            BlockBody, EntityBodyValue, EntityIdSet, FunctionBody, NamespaceBody, ParameterBody,
            TestCaseBody,
        },
    };
    use sley_ssmc::{
        ConstData, ConstValue, EffectEnvironment, ExpectedOutcome, ParameterRole, Reachability,
        ResourceLimits, ReturnTerminator, Terminator, TypeExpr, ValueRef, Visibility,
    };
    use sley_state_root::{
        AcceptedStateRoot, StateRootBuilder, conformance_epoch_id as state_epoch_id,
        conformance_registry as state_registry,
    };
    use sley_tests::NativeAggregateLimits;
    use sley_vm::native_execution::NativeImplementationLimits;

    use super::super::candidate_validation::tests::fixed;
    use super::*;
    use crate::{
        AcceptedPolicyRoot, CandidateValidationContext, CandidateValidationLimits,
        PolicyResourceCeilings, PolicyRootBuilder, PrincipalGrantBuilder,
        build_capability_summary_projection, conformance_registry as policy_registry,
        validate_candidate_bytes,
    };

    const NOW: u64 = 1_000;

    fn unit() -> ConstValue {
        ConstValue {
            value_type: TypeExpr::Unit,
            data: ConstData::Unit,
        }
    }

    struct DeleteSetup {
        transaction_id: TransactionId,
        base_objects: Vec<EntityObject>,
        base_state: AcceptedStateRoot,
        policy: AcceptedPolicyRoot,
        candidate: ImportedCandidate,
        test: EntityId,
        test_object: ObjectId,
    }

    #[allow(
        clippy::too_many_lines,
        reason = "delete fixture threads base/policy/candidate together; splitting hides the shared bindings"
    )]
    fn delete_setup(required: bool) -> DeleteSetup {
        let workspace_id = fixed(1, WorkspaceId::from_bytes);
        let principal_id = fixed(2, PrincipalId::from_bytes);
        let transaction_id = fixed(3, TransactionId::from_bytes);
        let namespace = fixed(10, EntityId::from_bytes);
        let function = fixed(40, EntityId::from_bytes);
        let parameter = fixed(41, EntityId::from_bytes);
        let block = fixed(42, EntityId::from_bytes);
        let test = fixed(43, EntityId::from_bytes);
        let grant = PrincipalGrantBuilder::new(PolicyResourceCeilings::new(
            1_000, 1_000, 1_000, 100, 100, 100,
        ))
        .mutation_class(MutationClass::DeleteEntityBinding)
        .build()
        .unwrap();
        let mut policy = PolicyRootBuilder::new(workspace_id).principal_grant(principal_id, grant);
        if required {
            policy = policy.required_test(test);
        }
        let policy = policy.build(&policy_registry().unwrap()).unwrap();

        let schema_epoch_id = state_epoch_id().unwrap();
        let object = |entity_id: EntityId, body: EntityBodyValue| {
            build_entity_object(
                schema_epoch_id,
                &EntityObjectRecord {
                    entity_id,
                    body,
                    label: None,
                    semantic_fingerprint: None,
                },
            )
            .unwrap()
        };
        let base_objects = vec![
            object(
                namespace,
                EntityBodyValue::Namespace(NamespaceBody {
                    parent: None,
                    members: EntityIdSet::from_unsorted(vec![]).unwrap(),
                }),
            ),
            object(
                function,
                EntityBodyValue::Function(FunctionBody {
                    type_parameters: vec![],
                    parameters: vec![parameter],
                    result_type: TypeExpr::Unit,
                    effects: EntityIdSet::from_unsorted(vec![]).unwrap(),
                    entry_block: block,
                    blocks: vec![block],
                    contracts: EntityIdSet::from_unsorted(vec![]).unwrap(),
                    visibility: Visibility::Private,
                }),
            ),
            object(
                parameter,
                EntityBodyValue::Parameter(ParameterBody {
                    owner: function,
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: TypeExpr::Unit,
                }),
            ),
            object(
                block,
                EntityBodyValue::Block(BlockBody {
                    function,
                    parameters: vec![],
                    operations: vec![],
                    terminator: Terminator::Return(ReturnTerminator {
                        value: ValueRef::Parameter(parameter),
                    }),
                    reachability: Reachability::Required,
                }),
            ),
            object(
                test,
                EntityBodyValue::TestCase(TestCaseBody {
                    target: function,
                    inputs: vec![unit()],
                    effect_environment: EffectEnvironment::Replay(vec![]),
                    expected: ExpectedOutcome::Value(unit()),
                    observations: vec![],
                    resource_limits: ResourceLimits {
                        fuel: 10,
                        memory_bytes: 64,
                        output_bytes: 64,
                        effect_count: 0,
                        call_depth: 2,
                        wall_timeout_millis: 100,
                    },
                }),
            ),
        ];
        let test_object = base_objects[4].object_id();
        let mut state_builder = StateRootBuilder::new(
            workspace_id,
            fixed(20, ObjectId::from_bytes),
            fixed(21, ObjectId::from_bytes),
            policy.root(),
        );
        for object in &base_objects {
            state_builder =
                state_builder.entity_binding(object.record().entity_id, object.object_id());
        }
        let base_state = state_builder.build(&state_registry().unwrap()).unwrap();
        let summary = build_capability_summary_projection(
            principal_id,
            workspace_id,
            policy.root(),
            base_state.root,
            &[],
        )
        .unwrap();
        let candidate = build_candidate(&CandidateRecord {
            format_version: 1,
            workspace_id,
            base_transaction_id: transaction_id,
            base_root: base_state.root,
            schema_epoch_id,
            policy_root_id: policy.root(),
            principal_id,
            capability_summary_digest: summary.digest(),
            operations: vec![MutationOperation {
                ordinal: 0,
                class: MutationClass::DeleteEntityBinding,
                target_kind: 14,
                target_entity: test,
                field_tag: None,
                payload: MutationPayload::DeleteEntityBinding,
                precondition_ordinal: 0,
            }],
            preconditions: vec![BoundPrecondition {
                operation_ordinal: 0,
                requirement: PreimageRequirement::ExactEntityVersion,
                payload: PreconditionPayload::ExactEntityVersion(ExactEntityVersion {
                    entity_id: test,
                    object_id: test_object,
                }),
            }],
            validation_profile_id: full_validation_profile_id().unwrap(),
            candidate_nonce: fixed(31, CandidateNonce::from_bytes),
            expiry: CandidateExpiry::unix_millis(NOW + 1_000),
        })
        .unwrap();
        DeleteSetup {
            transaction_id,
            base_objects,
            base_state,
            policy,
            candidate,
            test,
            test_object,
        }
    }

    fn delete_inputs(setup: &DeleteSetup) -> (NativePlanInputs<'_>, CapabilitySummaryDigest) {
        let context = CandidateValidationContext::new(
            setup.transaction_id,
            &setup.base_state,
            &setup.base_objects,
            &[],
            &setup.policy,
            fixed(2, PrincipalId::from_bytes),
            &[],
            NOW,
            CandidateValidationLimits::full_v1(),
        )
        .unwrap();
        let summary = context.capability_summary_digest();
        let inputs = NativePlanInputs {
            base_transaction_id: setup.transaction_id,
            base_state: &setup.base_state,
            base_objects: &setup.base_objects,
            policy: &setup.policy,
            capability_summary: summary,
            limits: CandidateValidationLimits::full_v1(),
            implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
            aggregate: NativeAggregateLimits::HARD_MAXIMA,
        };
        (inputs, summary)
    }

    #[test]
    fn deleted_nonrequired_test_is_recorded_but_not_executed() {
        let setup = delete_setup(false);
        let context = CandidateValidationContext::new(
            setup.transaction_id,
            &setup.base_state,
            &setup.base_objects,
            &[],
            &setup.policy,
            fixed(2, PrincipalId::from_bytes),
            &[],
            NOW,
            CandidateValidationLimits::full_v1(),
        )
        .unwrap();
        let output = validate_candidate_bytes(&context, &setup.candidate.stored_bytes)
            .expect("output builds");
        assert!(output.is_valid());
        let (inputs, _) = delete_inputs(&setup);
        let plan = native_test_plan(&output, &inputs).expect("plan derives");
        assert!(plan.selected().is_empty());
        assert_eq!(plan.changed().len(), 1);
        assert_eq!(plan.changed()[0].test_entity, setup.test);
        assert_eq!(plan.changed()[0].before, Some(setup.test_object));
        assert_eq!(plan.changed()[0].after, None);
    }

    #[test]
    fn deleting_a_protected_required_test_preserves_the_static_refusal() {
        let setup = delete_setup(true);
        let context = CandidateValidationContext::new(
            setup.transaction_id,
            &setup.base_state,
            &setup.base_objects,
            &[],
            &setup.policy,
            fixed(2, PrincipalId::from_bytes),
            &[],
            NOW,
            CandidateValidationLimits::full_v1(),
        )
        .unwrap();
        let output = validate_candidate_bytes(&context, &setup.candidate.stored_bytes)
            .expect("output builds");
        assert!(!output.is_valid());
        let (inputs, _) = delete_inputs(&setup);
        assert_eq!(
            native_test_plan(&output, &inputs).expect_err("refuses"),
            NativePlanErrorV1::SelectionInvalid
        );
    }
}
