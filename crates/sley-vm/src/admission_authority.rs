//! Staged v2 admission authority (RW-075 correction, R2).
//!
//! The host execution path verifies byte-hash equality but cannot prove a
//! package image was lowered from the judged graphs. This staged authority
//! performs that comparison in code: judge the closure, reference re-lower
//! with the native lowerer, compare bytes exactly, verify the package
//! carries the gate's claims, and mint a v2 receipt only on exact match.
//! A mismatch aborts with no receipt, so no approval or execution can
//! follow. This is the exact procedure the Sley build driver replicates
//! per the RW-080 contract §1.4; no toolchain graph, no C1, and no RW-080
//! construction live here.
//!
//! Authority discipline, not cryptography, secures staging: production
//! `admit_package_v2` stays a pure-data constructor (as in v1, where every
//! native gate/checker/oracle is secured by campaign-declaration plus
//! review). R2 authority evidence consists of receipts returned by
//! [`admit_v2_package`] (identified by the comparison the authority
//! performed); direct constructor calls outside it are test negatives or
//! non-evidence staging, never R2 authority evidence.
//!
//! This module is the one production location permitted to call the gate
//! and the reference lowerer on the admission path. The package,
//! execution, raw-hash, host-ABI, and bridge modules remain free of those
//! calls (pinned by `scripts/check_exec_package_markers.py` and
//! `scripts/check_host_abi_markers.py`).

use crate::bootstrap::{BootstrapProfileInput, BootstrapProfileReport, judge_bootstrap_profile};
use crate::exec_package::{
    AdmissionReceipt, ExecutionPackage, PackageDigests, PackageError, admit_package_v2,
    approve_package_v2, package_digests_v2,
};
use crate::lower::{LoweringInput, lower_function};

/// Staged authority failure (no receipt minted on any variant).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorityError {
    /// The presented image bytes do not equal the candidate package bytes.
    PresentedMismatch,
    /// The gate refused the closure (no report exists for refused closures).
    GateRefused,
    /// Reference re-lowering failed or its bytes differ from the package.
    ReferenceMismatch,
    /// The package does not carry the gate's quantitative claims.
    ClaimsMismatch,
    /// Package digests could not be computed (bounds/encoding).
    Digests(PackageError),
    /// Approval cross-check failed (never mint unapprovable receipts).
    ApprovalMismatch,
}

impl core::fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let code = match self {
            Self::PresentedMismatch => "AUTHORITY_PRESENTED_MISMATCH",
            Self::GateRefused => "AUTHORITY_GATE_REFUSED",
            Self::ReferenceMismatch => "AUTHORITY_REFERENCE_MISMATCH",
            Self::ClaimsMismatch => "AUTHORITY_CLAIMS_MISMATCH",
            Self::Digests(_) => "AUTHORITY_DIGESTS",
            Self::ApprovalMismatch => "AUTHORITY_APPROVAL_MISMATCH",
        };
        formatter.write_str(code)
    }
}

impl std::error::Error for AuthorityError {}

/// Judge, reference re-lower, compare exactly, and mint a v2 receipt.
///
/// # Errors
///
/// [`AuthorityError`] on the first failed leg; no receipt is minted on any
/// failure.
pub fn admit_v2_package(
    gate: &BootstrapProfileInput<'_>,
    lowering: &LoweringInput<'_>,
    package: &ExecutionPackage,
) -> Result<(PackageDigests, AdmissionReceipt, BootstrapProfileReport), AuthorityError> {
    if gate.presented_image_bytes != package.image_bytes.as_slice() {
        return Err(AuthorityError::PresentedMismatch);
    }
    let report = judge_bootstrap_profile(gate).map_err(|_| AuthorityError::GateRefused)?;
    let reference = lower_function(*lowering).map_err(|_| AuthorityError::ReferenceMismatch)?;
    if reference.bytes != package.image_bytes {
        return Err(AuthorityError::ReferenceMismatch);
    }
    if report.operation_count() != package.gate_operation_count
        || report.bridge_uses() != package.gate_bridge_uses
        || report.closure_fingerprints() != package.gate_closure_fingerprints.as_slice()
    {
        return Err(AuthorityError::ClaimsMismatch);
    }
    let digests = package_digests_v2(package).map_err(AuthorityError::Digests)?;
    let receipt = admit_package_v2(digests.package_digest);
    approve_package_v2(package, &digests, receipt, &report)
        .map_err(|_| AuthorityError::ApprovalMismatch)?;
    Ok((digests, receipt, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_error_codes_are_stable() {
        assert_eq!(
            AuthorityError::PresentedMismatch.to_string(),
            "AUTHORITY_PRESENTED_MISMATCH"
        );
        assert_eq!(
            AuthorityError::GateRefused.to_string(),
            "AUTHORITY_GATE_REFUSED"
        );
        assert_eq!(
            AuthorityError::ReferenceMismatch.to_string(),
            "AUTHORITY_REFERENCE_MISMATCH"
        );
        assert_eq!(
            AuthorityError::ClaimsMismatch.to_string(),
            "AUTHORITY_CLAIMS_MISMATCH"
        );
        assert_eq!(
            AuthorityError::ApprovalMismatch.to_string(),
            "AUTHORITY_APPROVAL_MISMATCH"
        );
    }
}
