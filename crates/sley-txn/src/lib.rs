#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod codec;
mod maintenance;
mod native_codec;
mod native_commit;
mod repository;

#[cfg(any(test, feature = "s20-530-test-hooks"))]
#[doc(hidden)]
pub mod recovery_ancestry_test_hook;
#[cfg(any(test, feature = "s20-530-test-hooks"))]
#[doc(hidden)]
pub mod recovery_path_read_test_hook;

/// Returns the exact feature name that exposes the S20-530 test hooks.
#[cfg(any(test, feature = "s20-530-test-hooks"))]
#[doc(hidden)]
#[must_use]
pub const fn s20_530_test_hook_feature_name() -> &'static str {
    "s20-530-test-hooks"
}

pub use codec::{
    ChangedBinding, CommitMetadata, ImportedTransaction, ImportedTransactionReceipt,
    ObjectManifestEntry, TransactionCodecError, TransactionErrorCode, TransactionKind,
    TransactionReceiptRecord, TransactionRecord, build_transaction, build_transaction_receipt,
    import_transaction, import_transaction_receipt,
};
pub use maintenance::{
    RepositoryMaintenanceGuard, acquire_exclusive_repository_maintenance,
    acquire_exclusive_repository_maintenance_nonblocking, acquire_shared_repository_maintenance,
    acquire_shared_repository_maintenance_nonblocking, initialize_repository_maintenance,
};
pub use native_codec::{
    ImportedNativeTransaction, ImportedNativeTransactionReceipt, ImportedReceipt,
    MAX_NATIVE_EVIDENCE_BYTES, MAX_NATIVE_SELECTED_TESTS, NATIVE_FORMAT_VERSION,
    NATIVE_RECEIPT_FIELD_COUNT, NATIVE_RECEIPT_MAGIC, NATIVE_TRANSACTION_FIELD_COUNT,
    NATIVE_TRANSACTION_MAGIC, NativeEvidenceBudget, NativeEvidencePin, NativeEvidenceSummary,
    NativeTransactionReceiptRecord, NativeTransactionRecord, build_native_transaction,
    build_native_transaction_receipt, check_native_evidence_bounds, import_native_transaction,
    import_native_transaction_receipt, import_receipt_any, native_receipt_trust_policy_ids,
};
pub use native_commit::{
    ATTEMPT_SUFFIX, ATTEMPTS_DIR, AttemptRecord, AttemptState, AttemptStatus, ExecutedNativeTest,
    JOURNAL_MAGIC, JOURNAL_VERSION, MAX_COMMIT_WALL_MILLIS, MAX_JOURNAL_BYTES,
    NativeAcceptanceSigner, NativeAttemptId, NativeCommitError, NativeCommitInput,
    NativeCommitOutcome, NativeCommitOutput, NativeRejection, NativeTestExecutor,
    NativeVerifiedRevision, attempt_path, commit_needs_executor, native_receipt_committed_root,
};
pub use repository::{
    AcceptedHead, CommitError, CommitInput, CommitOutput, RecoveryAncestryError,
    RecoveryAncestryHeadReport, RecoveryAncestryReport, RecoveryAncestryRequest, RecoveryReport,
    RecoveryRevisionClaim, RecoveryWorkUsage, TransactionRepository, TrustedGenesisInput,
    VerifiedRevision, incomplete_clone_marker_present, verify_any_receipt_against_objects,
    verify_native_receipt_against_objects, verify_receipt_against_objects,
};
