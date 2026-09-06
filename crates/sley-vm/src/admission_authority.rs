//! Staged v2 admission authority (RW-075 correction, R2).
//!
//! The host execution path verifies byte-hash equality but cannot prove a
//! package image was lowered from the judged graphs. This staged authority
//! is the exclusive v2 minter in reviewed paths: it takes one canonical
//! closure bundle, derives the gate input and the reference-lowering input
//! from that same bundle internally (so two callers cannot supply graph A
//! for the gate and graph B for the lowering), judges, re-lowers,
//! compares bytes exactly, verifies the package carries the gate's claims,
//! and mints a v2 receipt only on exact match. A mismatch aborts with no
//! receipt, so no approval or execution can follow. This is the exact
//! procedure the Sley build driver replicates per the RW-080 contract
//! §1.4; no toolchain graph, no C1, and no RW-080 construction live here.
//!
//! Exclusivity is enforced in code: the raw v2 constructor
//! (`exec_package::admit_package_v2`) is `pub(crate)`, and its public
//! re-export was removed, so reviewed integration paths cannot mint v2
//! receipts except through [`admit_v2_package`]. Authority discipline, not
//! cryptography, secures staging (as in v1, where every native
//! gate/checker/oracle is secured by campaign-declaration plus review).
//! R2 authority evidence consists of receipts returned by
//! [`admit_v2_package`]; direct constructor calls exist only in crate
//! unit tests as explicitly marked negatives, never as R2 evidence.
//!
//! This module is the one production location permitted to call the gate
//! plus the reference lowerer on the admission path. The package,
//! execution, raw-hash, host-ABI, and bridge modules remain free of those
//! calls (pinned by `scripts/check_exec_package_markers.py` and
//! `scripts/check_host_abi_markers.py`).

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, ConstantDefinition, ContractDefinition, FunctionGraph,
    GlobalValueDefinition, Operation, Parameter,
};

use crate::CacheProfile;
use crate::bootstrap::{BootstrapProfileInput, BootstrapProfileReport, judge_bootstrap_profile};
use crate::exec_package::{
    AdmissionReceipt, ExecutionPackage, PackageDigests, PackageError, admit_package_v2,
    approve_package_v2, package_digests_v2,
};
use crate::lower::{LoweringInput, lower_function};

/// One canonical closure bundle feeding both authority legs.
///
/// The authority derives the gate input and the reference-lowering input
/// from this single bundle internally, so a caller cannot judge graph A
/// while re-lowering graph B. The presented image is always the candidate
/// package's own bytes (no separate presented parameter to diverge).
pub struct V2Closure<'a> {
    /// Selected type environment.
    pub types: &'a TypeEnvironment,
    /// Exact schema epoch.
    pub schema_epoch: SchemaEpochId,
    /// Exact state root.
    pub state_root: StateRoot,
    /// Entry function id (resolved in `functions`).
    pub entry: EntityId,
    /// Complete function inventory.
    pub functions: &'a [FunctionGraph],
    /// Complete parameter inventory.
    pub parameters: &'a [Parameter],
    /// Complete block inventory.
    pub blocks: &'a [Block],
    /// Complete operation inventory.
    pub operations: &'a [Operation],
    /// Complete import inventory.
    pub adapters: &'a [AdapterImport],
    /// Complete constant inventory.
    pub constants: &'a [ConstantDefinition],
    /// Complete global inventory (lowering only).
    pub globals: &'a [GlobalValueDefinition],
    /// Complete contract inventory (lowering only).
    pub contracts: &'a [ContractDefinition],
}

/// Staged authority failure (no receipt minted on any variant).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorityError {
    /// The entry resolves to no supplied function.
    UnknownEntry,
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
            Self::UnknownEntry => "AUTHORITY_UNKNOWN_ENTRY",
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

/// Judge one closure, reference re-lower it, compare exactly, and mint.
///
/// Both legs derive from `closure` internally; `package` supplies only the
/// candidate bytes plus the claimed gate counts/fingerprints (verified,
/// never trusted).
///
/// # Errors
///
/// [`AuthorityError`] on the first failed leg; no receipt is minted on any
/// failure.
pub fn admit_v2_package(
    closure: &V2Closure<'_>,
    package: &ExecutionPackage,
) -> Result<(PackageDigests, AdmissionReceipt, BootstrapProfileReport), AuthorityError> {
    let entry = closure
        .functions
        .iter()
        .find(|graph| graph.entity_id == closure.entry)
        .ok_or(AuthorityError::UnknownEntry)?;
    let report = judge_bootstrap_profile(&BootstrapProfileInput {
        types: closure.types,
        schema_epoch: closure.schema_epoch,
        entry,
        presented_image_bytes: &package.image_bytes,
        functions: closure.functions,
        parameters: closure.parameters,
        blocks: closure.blocks,
        operations: closure.operations,
        adapters: closure.adapters,
        constants: closure.constants,
    })
    .map_err(|_| AuthorityError::GateRefused)?;
    let reference = lower_function(LoweringInput {
        types: closure.types,
        function: entry,
        parameters: closure.parameters,
        blocks: closure.blocks,
        operations: closure.operations,
        schema_epoch: closure.schema_epoch,
        state_root: closure.state_root,
        profile: CacheProfile::EXTENDED_V1,
        constants: closure.constants,
        globals: closure.globals,
        functions: closure.functions,
        contracts: closure.contracts,
        adapters: closure.adapters,
    })
    .map_err(|_| AuthorityError::ReferenceMismatch)?;
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
            AuthorityError::UnknownEntry.to_string(),
            "AUTHORITY_UNKNOWN_ENTRY"
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
