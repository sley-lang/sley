#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod codec;
mod maintenance;
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
    initialize_repository_maintenance,
};
pub use repository::{
    AcceptedHead, CommitError, CommitInput, CommitOutput, RecoveryAncestryError,
    RecoveryAncestryHeadReport, RecoveryAncestryReport, RecoveryAncestryRequest, RecoveryReport,
    RecoveryRevisionClaim, RecoveryWorkUsage, TransactionRepository, TrustedGenesisInput,
    VerifiedRevision, incomplete_clone_marker_present, verify_receipt_against_objects,
};
