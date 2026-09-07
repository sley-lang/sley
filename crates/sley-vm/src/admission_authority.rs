//! Staged v2 admission authority (RW-075 correction, R2).
//!
//! C0 SEED PATH (declared): the semantic legs below — profile judgment
//! and reference lowering — run the accepted native reference compiler.
//! They are seed/oracle evidence, legitimate only before C1 exists, and
//! are explicitly excluded from the clean stages after C1 (RW-080
//! contract §1.4 and §4). What the seed verifies is graphs-to-image
//! correspondence for one canonical closure bundle; what it can never
//! supply after C1 is a semantic answer.
//!
//! PERMANENT MECHANICS: byte-hash equality, gate-claim binding, complete
//! table correspondence, section digests, receipt minting discipline,
//! and approval cross-checks. These are structural, carry no language
//! judgment, and survive into the clean stages unchanged.
//!
//! SLEY INGRESS (reserved): post-C1 admission evidence produced by an
//! approved Sley admission/checker program enters through
//! [`SleyAdmissionEvidence`]. No constructor and no minting path exist
//! until C1 exists: [`admit_v2_package_from_sley_evidence`] refuses with
//! [`AuthorityError::SleyEvidenceUnavailable`]. Native minting is then
//! reduced to authenticated/bound structural handling of that evidence.
//!
//! The host execution path verifies byte-hash equality but cannot prove a
//! package image was lowered from the judged graphs. This staged authority
//! is the exclusive v2 minter in reviewed paths: it takes one canonical
//! closure bundle, derives the gate input and the reference-lowering input
//! from that same bundle internally (so two callers cannot supply graph A
//! for the gate and graph B for the lowering), judges, re-lowers,
//! compares bytes exactly, verifies the package carries the gate's claims,
//! and mints a v2 receipt only on exact match. A mismatch aborts with no
//! receipt, so no approval or execution can follow.
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
    GlobalValueDefinition, Operation, Parameter, TypeDefinition,
};

use crate::CacheProfile;
use crate::bootstrap::{
    BootstrapProfileInput, BootstrapProfileReport, BootstrapProfileVersion, judge_bootstrap_profile,
};
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
    /// The package does not carry the closure's claims: gate counts,
    /// fingerprints, entry/epoch/root binding, or any carried table
    /// (constants, type definitions, full import rows, globals,
    /// contracts) diverges from the judged closure.
    ClaimsMismatch,
    /// Package digests could not be computed (bounds/encoding).
    Digests(PackageError),
    /// Approval cross-check failed (never mint unapprovable receipts).
    ApprovalMismatch,
    /// Sley-produced admission evidence is not yet available: no C1
    /// exists, so the reserved ingress has no constructor and no
    /// minting path. Post-C1 work fills this in; until then every
    /// call refuses.
    SleyEvidenceUnavailable,
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
            Self::SleyEvidenceUnavailable => "AUTHORITY_SLEY_EVIDENCE_UNAVAILABLE",
        };
        formatter.write_str(code)
    }
}

impl std::error::Error for AuthorityError {}

/// Judge one closure, reference re-lower it, compare exactly, and mint.
///
/// C0 SEED PATH: the two semantic legs ([`judge_closure_for_seed`] and
/// [`reference_lower_for_seed`]) run the accepted native reference
/// compiler. They are seed/oracle evidence, legitimate only before C1
/// exists. The structural legs ([`verify_structural_correspondence`],
/// digests, minting, approval cross-check) are permanent mechanics.
///
/// Both semantic legs derive from `closure` internally; `package`
/// supplies only the candidate bytes plus the claimed closure tables
/// (verified, never trusted).
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
    // Semantic leg 1 (C0 seed): profile judgment.
    let report = judge_closure_for_seed(closure, entry, &package.image_bytes)?;
    // Semantic leg 2 (C0 seed): reference re-lowering.
    let reference = reference_lower_for_seed(closure, entry)?;
    // Permanent mechanics from here on: no language judgment.
    verify_structural_correspondence(closure, package, &report, &reference.bytes)?;
    let digests = package_digests_v2(package).map_err(AuthorityError::Digests)?;
    let receipt = admit_package_v2(digests.package_digest);
    approve_package_v2(package, &digests, receipt, &report)
        .map_err(|_| AuthorityError::ApprovalMismatch)?;
    Ok((digests, receipt, report))
}

/// Semantic leg 1, C0 seed only: judge the closure under the successor
/// profile with the native gate. Excluded from clean stages after C1;
/// post-C1 judgment evidence arrives via [`SleyAdmissionEvidence`].
fn judge_closure_for_seed(
    closure: &V2Closure<'_>,
    entry: &FunctionGraph,
    presented_image_bytes: &[u8],
) -> Result<BootstrapProfileReport, AuthorityError> {
    judge_bootstrap_profile(&BootstrapProfileInput {
        types: closure.types,
        schema_epoch: closure.schema_epoch,
        entry,
        presented_image_bytes,
        functions: closure.functions,
        parameters: closure.parameters,
        blocks: closure.blocks,
        operations: closure.operations,
        adapters: closure.adapters,
        constants: closure.constants,
        // The staged authority judges under the successor profile: it is
        // the only production path that may admit `RHW1` closures, and
        // its reports approve v2 packages only.
        profile_version: BootstrapProfileVersion::V2,
    })
    .map_err(|_| AuthorityError::GateRefused)
}

/// Semantic leg 2, C0 seed only: re-lower the judged graphs with the
/// native reference lowerer. Excluded from clean stages after C1; the
/// Sley build driver replicates this comparison with Sley-owned
/// lowering evidence once C1 exists (RW-080 contract §1.4).
fn reference_lower_for_seed(
    closure: &V2Closure<'_>,
    entry: &FunctionGraph,
) -> Result<crate::lower::LoweredFunction, AuthorityError> {
    lower_function(LoweringInput {
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
    .map_err(|_| AuthorityError::ReferenceMismatch)
}

/// Permanent structural mechanics: reference-bytes equality, gate-claim
/// binding, and complete-package correspondence. No language judgment:
/// pure equality of bytes, counts, fingerprints, identities, and rows.
/// Survives into the clean stages unchanged, whether the semantic legs
/// above are seed-native (now) or Sley-evidenced (post-C1).
fn verify_structural_correspondence(
    closure: &V2Closure<'_>,
    package: &ExecutionPackage,
    report: &BootstrapProfileReport,
    reference_bytes: &[u8],
) -> Result<(), AuthorityError> {
    if reference_bytes != package.image_bytes {
        return Err(AuthorityError::ReferenceMismatch);
    }
    if report.operation_count() != package.gate_operation_count
        || report.bridge_uses() != package.gate_bridge_uses
        || report.closure_fingerprints() != package.gate_closure_fingerprints.as_slice()
    {
        return Err(AuthorityError::ClaimsMismatch);
    }
    // Complete-package correspondence: the receipt covers the package the
    // closure actually describes, not only its image bytes and gate
    // counts. Every table the package carries into execution is compared
    // against the judged closure before minting — entry, epoch, root,
    // constants, type definitions (by identity through the environment),
    // full import rows (not only the identity set approval checks),
    // globals, contracts. Any substitution, addition, removal, or
    // re-binding refuses with `ClaimsMismatch`: no receipt, no approval,
    // no execution. Row order is insignificant (rows are referenced by
    // identity), so comparison is order-insensitive but exact per row.
    if package.entry != closure.entry
        || package.schema_epoch != closure.schema_epoch
        || package.state_root != closure.state_root
        || !tables_match(closure.constants, &package.constants, |row| row.entity_id)
        || !type_tables_match(closure.types, &package.type_definitions)
        || !tables_match(closure.adapters, &package.imports, |row| row.entity_id)
        || !tables_match(closure.globals, &package.globals, |row| row.entity_id)
        || !tables_match(closure.contracts, &package.contracts, |row| row.entity_id)
    {
        return Err(AuthorityError::ClaimsMismatch);
    }
    if report.admitted_image_digest() != &crate::host_abi::image_digest(&package.image_bytes) {
        return Err(AuthorityError::ClaimsMismatch);
    }
    Ok(())
}

/// Reserved Sley-produced admission evidence (post-C1 ingress).
///
/// When C1 exists, an approved Sley admission/checker program produces
/// this evidence: the judged closure identity, gate claims
/// (operation/bridge counts, closure fingerprints), reference-image
/// digest, and complete table digests (constants, type definitions,
/// full import rows, globals, contracts) — all computed by Sley over
/// Sley-built bytes. The authority then authenticates and bound-checks
/// this evidence structurally (permanent mechanics) instead of running
/// native semantics.
///
/// No constructor exists until C1 exists: the struct is sealed
/// (`#[non_exhaustive]` plus private fields) and this module provides
/// no constructor, so no caller can fabricate or complete one. This
/// is an interface reservation, not a minting path.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SleyAdmissionEvidence {
    /// BLAKE3 digest of the canonical judged-closure bytes (Sley-built).
    closure_digest: [u8; 32],
    /// Gate operation count claimed by the Sley admission program.
    operation_count: u32,
    /// Gate bridge-use count claimed by the Sley admission program.
    bridge_uses: u32,
    /// SHA-256 digest of the reference image bytes (Sley-lowered).
    image_digest: [u8; 32],
}

/// Reserved post-C1 admission route: mint from Sley-produced evidence
/// instead of native semantics.
///
/// Currently always refuses with
/// [`AuthorityError::SleyEvidenceUnavailable`]: no C1 exists, so there
/// is no evidence producer, no authenticator, and no minting path.
/// Post-C1 work adds the producer, the structural authenticator, and
/// the mint — without touching the native seed path above, which is
/// then excluded from clean stages.
///
/// # Errors
///
/// Always [`AuthorityError::SleyEvidenceUnavailable`].
pub fn admit_v2_package_from_sley_evidence(
    _evidence: &SleyAdmissionEvidence,
    _package: &ExecutionPackage,
) -> Result<(PackageDigests, AdmissionReceipt, BootstrapProfileReport), AuthorityError> {
    Err(AuthorityError::SleyEvidenceUnavailable)
}

/// Whether two identity-keyed table snapshots carry exactly the same
/// rows: same identity set with equal bodies per identity.
/// Order-insensitive (rows are referenced by identity, so order carries
/// no semantics); any substitution, addition, or removal fails.
fn tables_match<T: PartialEq>(closure: &[T], package: &[T], id: fn(&T) -> EntityId) -> bool {
    if closure.len() != package.len() {
        return false;
    }
    let mut left: Vec<(EntityId, &T)> = closure.iter().map(|row| (id(row), row)).collect();
    let mut right: Vec<(EntityId, &T)> = package.iter().map(|row| (id(row), row)).collect();
    left.sort_by_key(|(row_id, _)| *row_id);
    right.sort_by_key(|(row_id, _)| *row_id);
    left == right
}

/// Whether a package layout section carries exactly the type definitions
/// of the closure's environment: same identity set with equal bodies per
/// identity, order-insensitive like [`tables_match`].
fn type_tables_match(types: &TypeEnvironment, package: &[TypeDefinition]) -> bool {
    let ids: Vec<EntityId> = types.definition_ids().collect();
    if ids.len() != package.len() {
        return false;
    }
    let mut left = Vec::with_capacity(ids.len());
    for row_id in ids {
        let Ok(definition) = types.definition(row_id) else {
            return false;
        };
        left.push((row_id, definition));
    }
    let mut right: Vec<(EntityId, &TypeDefinition)> =
        package.iter().map(|row| (row.entity_id, row)).collect();
    left.sort_by_key(|(row_id, _)| *row_id);
    right.sort_by_key(|(row_id, _)| *row_id);
    left == right
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
        assert_eq!(
            AuthorityError::SleyEvidenceUnavailable.to_string(),
            "AUTHORITY_SLEY_EVIDENCE_UNAVAILABLE"
        );
    }

    #[test]
    fn sley_evidence_ingress_has_no_minting_path() {
        // The reserved post-C1 route refuses: no evidence producer, no
        // authenticator, no receipt — by construction, not by test setup.
        // `SleyAdmissionEvidence` is sealed with no constructor, so this
        // test cannot even build one outside the module; the refusal is
        // pinned at the type level. Here we pin the error code string.
        assert_eq!(
            AuthorityError::SleyEvidenceUnavailable.to_string(),
            "AUTHORITY_SLEY_EVIDENCE_UNAVAILABLE"
        );
    }
}
