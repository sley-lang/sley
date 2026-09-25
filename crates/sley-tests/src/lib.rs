#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod approval;
pub mod bundle;
pub mod codec;
pub mod compare;
pub mod context;
pub mod measurement;
pub mod plan;
pub mod policy;
pub mod profile;
pub mod report;
pub mod statement;
pub mod supervisor;
pub mod trust;

pub use approval::{
    APPROVAL_MAGIC, ApprovalDecision, AttestationBinding, DECISION_ACCEPTED, DECISION_REJECTED,
    DETAIL_ID_MISMATCH, DETAIL_NONE, DETAIL_PHASE, DETAIL_RESOURCE, IdMismatchDetail, NativeDetail,
    NativeFailureRecord, NativeTestApprovalParts, NativeTestApprovalV1, RESOURCE_COUNT,
    RESOURCE_DEPTH, RESOURCE_EFFECTS, RESOURCE_EVIDENCE, RESOURCE_FUEL, RESOURCE_MEMORY,
    RESOURCE_OUTPUT, RESOURCE_WALL, ResourceDetail,
};
pub use bundle::{
    BUNDLE_MAGIC, MAX_BUNDLE_EMBEDDED_BYTES, MAX_EXECUTION_ITEM_BYTES, NativeEvidenceBundleParts,
    NativeEvidenceBundleV1, TestEmbedded,
};
pub use compare::{ExpectedEvidence, RestrictedComparison, compare_expected_evidence};
pub use context::{
    HISTORICAL_CONTEXT_MAGIC, HistoricalAdmissionContextParts, HistoricalAdmissionContextV1,
    MAX_CONTEXT_PROJECTION_BYTES,
};
pub use measurement::{
    MEASUREMENT_MAGIC, MEASUREMENT_SIGNATURE_CONTEXT, MeasuredTestAttestationParts,
    MeasuredTestAttestationV1, MemoryEvents, TERMINATION_COMPLETE, TERMINATION_CRASH,
    TERMINATION_ENFORCER_ERROR, TERMINATION_KILLED, TERMINATION_PRELAUNCH_REFUSED,
    TERMINATION_TIMEOUT, measurement_signature_preimage, unsigned_record_prefix,
};
/// Re-exported native execution profile identity constructor.
///
/// Receiver trust configuration names the execution profile scope without
/// the transaction owner depending on the VM owner.
pub use sley_vm::native_execution::profile_id as native_execution_profile_id;
/// Re-exported native execution ceiling types owned by the VM crate.
///
/// The transaction owner names these ceiling types in commit inputs without
/// depending on the VM owner; all execution semantics stay in `sley-vm`.
pub use sley_vm::native_execution::{
    NativeDeclaredLimits, NativeImplementationLimits, NativeObservedTermination,
};

pub use plan::{ChangedTest, NativeTestPlanParts, NativeTestPlanV1, SelectedEntry};
pub use policy::{
    GrantCeilings, NATIVE_WALL_CAP_MILLIS, NativeAggregateLimits, NativeResourcePolicyParts,
    NativeResourcePolicyV1, ValidationLimits,
};
pub use profile::{
    ACCEPTANCE_SIGNATURE_PROFILE_V1, ADMISSION_CLEANUP_MILLIS, ADMISSION_PROFILE_MAGIC,
    CANCEL_BETWEEN_REQUESTS, LOCK_WAIT_MILLIS, MEASUREMENT_PROFILE_V1, NativeAdmissionProfileParts,
    NativeAdmissionProfileV1, PREPROMOTION_WATCHDOG_MILLIS, SELECTION_RULE_NATIVE_V1,
};
pub use report::{
    COMPARISON_EXECUTION_REJECTED, COMPARISON_MATCH, COMPARISON_MISMATCH, EXECUTION_REPORT_MAGIC,
    EXPECTED_FAILURE_CODE, EXPECTED_VALUE, MAX_EXECUTION_REPORT_STORED, MAX_SYMBOL_BYTES,
    MAX_TEST_REPORT_ENTRIES, NativeExecutionEvidence, NativeExecutionReportParts,
    NativeExecutionReportV1, NativeExpected, NativeTestEntry, NativeTestReportV1, REJECT_PHASE_CFG,
    REJECT_PHASE_EFFECT, REJECT_PHASE_EXECUTION, REJECT_PHASE_FINGERPRINT, REJECT_PHASE_LOWERING,
    REJECT_PHASE_NATIVE_PROFILE, REJECT_PHASE_TYPE, RejectedEvidence, TEST_REPORT_MAGIC,
    TestComparison, compare_native_expected, rejected_from_error,
};
pub use statement::{
    ADMISSION_SIGNATURE_BYTES, ADMISSION_SIGNATURE_CONTEXT, CommitAdmissionStatementParts,
    CommitAdmissionStatementV1, STATEMENT_MAGIC, admission_signature_preimage,
    unsigned_statement_prefix,
};
pub use supervisor::{
    Caller, MAX_CALLERS, MAX_PROPERTIES, MAX_PROPERTY_NAME_BYTES, MAX_PROPERTY_VALUE_BYTES,
    Property, SUPERVISOR_CLEANUP_MILLIS, SUPERVISOR_CONFIG_MAGIC, SUPERVISOR_LAUNCH_PROFILE,
    SupervisorConfigParts, SupervisorConfigV1,
};
pub use trust::{
    HistoricalTrustPolicyParts, HistoricalTrustPolicyV1, MAX_TRUST_ENTRIES, MAX_TRUST_SET,
    ROLE_ACCEPTANCE, ROLE_MEASUREMENT, TRUST_MAGIC, TrustEntry,
};
