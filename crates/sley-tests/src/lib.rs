#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod approval;
pub mod codec;
pub mod measurement;
pub mod plan;
pub mod policy;
pub mod report;

pub use approval::{
    APPROVAL_MAGIC, ApprovalDecision, AttestationBinding, DECISION_ACCEPTED, DECISION_REJECTED,
    DETAIL_ID_MISMATCH, DETAIL_NONE, DETAIL_PHASE, DETAIL_RESOURCE, IdMismatchDetail, NativeDetail,
    NativeFailureRecord, NativeTestApprovalParts, NativeTestApprovalV1, RESOURCE_COUNT,
    RESOURCE_DEPTH, RESOURCE_EFFECTS, RESOURCE_EVIDENCE, RESOURCE_FUEL, RESOURCE_MEMORY,
    RESOURCE_OUTPUT, RESOURCE_WALL, ResourceDetail,
};
pub use measurement::{
    MEASUREMENT_MAGIC, MEASUREMENT_SIGNATURE_CONTEXT, MeasuredTestAttestationParts,
    MeasuredTestAttestationV1, MemoryEvents, TERMINATION_COMPLETE, TERMINATION_CRASH,
    TERMINATION_ENFORCER_ERROR, TERMINATION_KILLED, TERMINATION_PRELAUNCH_REFUSED,
    TERMINATION_TIMEOUT, measurement_signature_preimage, unsigned_record_prefix,
};

pub use plan::{ChangedTest, NativeTestPlanParts, NativeTestPlanV1, SelectedEntry};
pub use policy::{
    GrantCeilings, NATIVE_WALL_CAP_MILLIS, NativeAggregateLimits, NativeResourcePolicyParts,
    NativeResourcePolicyV1, ValidationLimits,
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
