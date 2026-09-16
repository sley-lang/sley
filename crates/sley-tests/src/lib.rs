#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod codec;
pub mod plan;
pub mod policy;
pub mod report;

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
