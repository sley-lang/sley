//! SH2 seed-absent host admission (`HOST_ABI_V3`).
//!
//! The pinned native host admits a v3 execution package only when its
//! package identity is in an explicit, caller-supplied pinned set: the
//! operator-approved toolchain images, or images a pinned toolchain built
//! and the bootstrap record binds. This module performs no language
//! judgment and never calls the native gate or the reference lowerer, so a
//! host built from it carries no compiler service on its admission path.
//! It computes digests, compares identities, mints the receipt, and runs
//! the structural approval checks.

use crate::exec_package::{
    ApprovedExecutionPackage, ExecutionPackage, PackageDigests, PackageError, admit_package_v3,
    approve_pinned_package_v3, package_digests_v3,
};

/// Host admission failure (no receipt is minted on any variant).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostAdmissionError {
    /// Package digests could not be computed.
    Digests(PackageError),
    /// The package identity is not in the pinned set.
    NotPinned,
    /// The structural approval cross-check refused.
    ApprovalMismatch(PackageError),
}

impl core::fmt::Display for HostAdmissionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Digests(_) => "HOST_ADMISSION_DIGESTS",
            Self::NotPinned => "HOST_ADMISSION_NOT_PINNED",
            Self::ApprovalMismatch(_) => "HOST_ADMISSION_APPROVAL_MISMATCH",
        })
    }
}

impl std::error::Error for HostAdmissionError {}

/// Admits a v3 package whose identity is pinned.
///
/// # Errors
///
/// [`HostAdmissionError`] on the first failed check.
pub fn admit_pinned_v3_package(
    package: &ExecutionPackage,
    pinned: &[[u8; 32]],
) -> Result<(PackageDigests, ApprovedExecutionPackage), HostAdmissionError> {
    let digests = package_digests_v3(package).map_err(HostAdmissionError::Digests)?;
    if !pinned.contains(&digests.package_digest) {
        return Err(HostAdmissionError::NotPinned);
    }
    let receipt = admit_package_v3(digests.package_digest);
    let approved = approve_pinned_package_v3(package, &digests, receipt)
        .map_err(HostAdmissionError::ApprovalMismatch)?;
    Ok((digests, approved))
}
