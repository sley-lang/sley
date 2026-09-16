//! Non-committing native diagnostic execution (N7c): the 601 `tests.selected`
//! and 602 `tests.affected` selection reads.
//!
//! The owner derives the selection, runs it through the configured test
//! executor, and assembles the diagnostic report the 605 pager later serves.
//! No approval, bundle, transaction, receipt, journal record, or head change
//! results: diagnostic evidence verifies exactly like commit evidence except
//! for the checks that need commit-time authority (measurement-trust roles
//! against receiver manifests, and the attestation principal against the
//! authenticated committer), which the commit path always re-applies from
//! its own re-derivation. The executor itself is the trusted component on
//! this path, configured by the server operator, never by the caller.

use std::collections::BTreeMap;

use sley_id::{EntityId, SchemaEpochId, WorkspaceId};
use sley_mutate::value::TestCaseBody;
use sley_policy::native_expected_outcome;
use sley_tests::{
    MeasuredTestAttestationV1, NativeDeclaredLimits, NativeExecutionEvidence, NativeTestEntry,
    NativeTestPlanV1, NativeTestReportV1, TestComparison, compare_native_expected,
};

use super::native_commit::{ExecutedNativeTest, check_execution_coverage};
use crate::codec::TransactionErrorCode;

/// Diagnostic comparison status, tags 1..=4 per
/// `NATIVE_TEST_ADMISSION_V1.md` appendix C.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeDiagnosticStatus {
    /// Every executed comparison matched.
    ComparisonComplete,
    /// At least one comparison mismatched.
    Mismatch,
    /// Every entry was execution-rejected before any observation existed.
    ExecutionRejected,
    /// At least one attestation measured over its declared limits.
    MeasuredResourceRefusal,
}

impl NativeDiagnosticStatus {
    /// Frozen diagnostic wire tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::ComparisonComplete => 1,
            Self::Mismatch => 2,
            Self::ExecutionRejected => 3,
            Self::MeasuredResourceRefusal => 4,
        }
    }

    /// Frozen operator-facing symbol.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::ComparisonComplete => "NATIVE_DIAGNOSTIC_COMPARISON_COMPLETE",
            Self::Mismatch => "NATIVE_DIAGNOSTIC_MISMATCH",
            Self::ExecutionRejected => "NATIVE_DIAGNOSTIC_EXECUTION_REJECTED",
            Self::MeasuredResourceRefusal => "NATIVE_DIAGNOSTIC_MEASURED_RESOURCE_REFUSAL",
        }
    }
}

impl core::fmt::Display for NativeDiagnosticStatus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.symbol())
    }
}

/// Assembled diagnostic evidence: the stored report plus its status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeDiagnosticAssembly {
    /// Deterministic report covering exactly the plan selection.
    pub report: NativeTestReportV1,
    /// Diagnostic status derived from comparisons and measurements.
    pub status: NativeDiagnosticStatus,
}

/// Assembles the diagnostic report from executor-returned evidence pairs.
///
/// Every execution report and attestation re-parses; plan, object, linkage,
/// and configuration bindings verify exactly like the commit path; the
/// attestation workspace must equal the session workspace and declared
/// limits must equal the plan entry's, or the executor is broken.
/// Expectations derive independently from the owner-loaded canonical
/// `TestCase` bytes and comparisons recompute through the shared kernel.
/// Measurement-trust roles and the attestation principal are not checked
/// here (no receiver manifests and no authenticated principal exist on the
/// diagnostic path); the commit path re-checks both from its own
/// re-derivation before anything is accepted.
///
/// Status follows the decided table: resource refusal first, then any
/// mismatch, then the all-rejected run, else comparison-complete. A
/// missing or transport-refusing executor never reaches assembly: the
/// caller refuses with `NATIVE_EXECUTOR_UNAVAILABLE` having written
/// nothing, since a response without report and token cannot exist.
///
/// # Errors
///
/// Returns `TXN_RECEIPT_BINDING_MISMATCH` for count, order, identity, plan
/// binding, workspace, declared-limits, report linkage, configuration, or
/// test-case divergence, and `TXN_INTERNAL_INVARIANT` when a validated test
/// case fails expectation fingerprinting or the covered report fails to
/// build.
pub fn assemble_diagnostic_report(
    plan: &NativeTestPlanV1,
    schema_epoch: SchemaEpochId,
    workspace: WorkspaceId,
    test_cases: &BTreeMap<EntityId, &TestCaseBody>,
    executions: &[ExecutedNativeTest],
) -> Result<NativeDiagnosticAssembly, TransactionErrorCode> {
    use sley_tests::NativeExecutionReportV1;
    let binding = TransactionErrorCode::ReceiptBindingMismatch;
    check_execution_coverage(plan, executions)?;
    let mut entries = Vec::with_capacity(plan.selected().len());
    let mut comparisons = Vec::with_capacity(plan.selected().len());
    let mut resource_refusal = false;
    for (plan_entry, execution) in plan.selected().iter().zip(executions) {
        let execution_report =
            NativeExecutionReportV1::parse(&execution.execution_stored).map_err(|_| binding)?;
        let attestation =
            MeasuredTestAttestationV1::parse(&execution.attestation_stored).map_err(|_| binding)?;
        if attestation.workspace() != workspace
            || attestation.declared_limits() != plan_entry.declared_limits
        {
            return Err(binding);
        }
        let body = test_cases.get(&plan_entry.test_entity).ok_or(binding)?;
        let expected = native_expected_outcome(schema_epoch, &body.expected)
            .map_err(|_| TransactionErrorCode::InternalInvariant)?;
        let comparison = match execution_report.evidence() {
            NativeExecutionEvidence::Observed { termination, .. } => {
                compare_native_expected(expected, termination)
            }
            NativeExecutionEvidence::Rejected(_) => TestComparison::ExecutionRejected,
        };
        if measured_over_declared(&attestation, &plan_entry.declared_limits) {
            resource_refusal = true;
        }
        comparisons.push(comparison);
        entries.push(NativeTestEntry {
            test_entity: plan_entry.test_entity,
            test_object: plan_entry.test_object,
            execution_report_id: execution_report.report_id(),
            expected,
            comparison,
        });
    }
    // Coverage already proved exact plan/order/count agreement, so a build
    // failure here is an internal invariant breach, never executor evidence.
    let report = NativeTestReportV1::build(plan, entries)
        .map_err(|_| TransactionErrorCode::InternalInvariant)?;
    let status = diagnostic_status(&comparisons, resource_refusal);
    Ok(NativeDiagnosticAssembly { report, status })
}

/// Decides the diagnostic status from its comparisons.
///
/// Resource refusal precedes everything: any over-declared measurement
/// refuses even when every comparison matches. Otherwise a single
/// mismatch decides mismatch; an all-rejected run with no observation at
/// all reports execution-rejected; the empty and all-match runs complete.
/// Test-only callers prove this table without execution envelopes.
fn diagnostic_status(
    comparisons: &[TestComparison],
    resource_refusal: bool,
) -> NativeDiagnosticStatus {
    if resource_refusal {
        return NativeDiagnosticStatus::MeasuredResourceRefusal;
    }
    if comparisons.contains(&TestComparison::Mismatch) {
        return NativeDiagnosticStatus::Mismatch;
    }
    if comparisons.contains(&TestComparison::ExecutionRejected) {
        return NativeDiagnosticStatus::ExecutionRejected;
    }
    NativeDiagnosticStatus::ComparisonComplete
}

/// Returns whether one attestation measured over its declared limits.
///
/// Breach events, a memory peak or installed cap above declared, or elapsed
/// nanoseconds past the declared wall (saturating, so an absurd wall never
/// wraps into admission) each refuse independently of the comparison.
fn measured_over_declared(
    attestation: &MeasuredTestAttestationV1,
    declared: &NativeDeclaredLimits,
) -> bool {
    let events = attestation.memory_events();
    if events.oom != 0 || events.oom_kill != 0 {
        return true;
    }
    if attestation.measured_memory_peak() > declared.memory_bytes
        || attestation.installed_memory_cap() > declared.memory_bytes
    {
        return true;
    }
    attestation.elapsed_ns() > declared.wall_timeout_millis.saturating_mul(1_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_tags_and_symbols_are_frozen() {
        assert_eq!(NativeDiagnosticStatus::ComparisonComplete.tag(), 1);
        assert_eq!(NativeDiagnosticStatus::Mismatch.tag(), 2);
        assert_eq!(NativeDiagnosticStatus::ExecutionRejected.tag(), 3);
        assert_eq!(NativeDiagnosticStatus::MeasuredResourceRefusal.tag(), 4);
        assert_eq!(
            NativeDiagnosticStatus::ComparisonComplete.symbol(),
            "NATIVE_DIAGNOSTIC_COMPARISON_COMPLETE"
        );
        assert_eq!(
            NativeDiagnosticStatus::Mismatch.symbol(),
            "NATIVE_DIAGNOSTIC_MISMATCH"
        );
        assert_eq!(
            NativeDiagnosticStatus::ExecutionRejected.symbol(),
            "NATIVE_DIAGNOSTIC_EXECUTION_REJECTED"
        );
        assert_eq!(
            NativeDiagnosticStatus::MeasuredResourceRefusal.symbol(),
            "NATIVE_DIAGNOSTIC_MEASURED_RESOURCE_REFUSAL"
        );
    }

    #[test]
    fn status_decision_table_prefers_resources_then_mismatch() {
        use TestComparison::{ExecutionRejected, Match, Mismatch};
        // Empty and all-match runs complete.
        assert_eq!(
            diagnostic_status(&[], false),
            NativeDiagnosticStatus::ComparisonComplete
        );
        assert_eq!(
            diagnostic_status(&[Match, Match], false),
            NativeDiagnosticStatus::ComparisonComplete
        );
        // One mismatch decides mismatch even beside rejections.
        assert_eq!(
            diagnostic_status(&[Mismatch, ExecutionRejected, Match], false),
            NativeDiagnosticStatus::Mismatch
        );
        // All-rejected with no observation reports execution-rejected.
        assert_eq!(
            diagnostic_status(&[ExecutionRejected, ExecutionRejected], false),
            NativeDiagnosticStatus::ExecutionRejected
        );
        // Resource refusal precedes every comparison outcome.
        for comparisons in [vec![], vec![Match], vec![Mismatch], vec![ExecutionRejected]] {
            assert_eq!(
                diagnostic_status(&comparisons, true),
                NativeDiagnosticStatus::MeasuredResourceRefusal
            );
        }
    }
}
