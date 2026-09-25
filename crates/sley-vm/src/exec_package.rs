//! The complete execution-package closure for loaded execution (RW-075
//! repair of AR-01 and AR-03).
//!
//! The final loaded execution path must NOT require Rust to semantically
//! construct the program-definition environment the Sley compiler should
//! have produced. This module makes the compiler/native distinction
//! explicit through a package/envelope around the existing `SLEYBC02`
//! image (the image format itself is unchanged):
//!
//! COMPILER-OWNED INFORMATION (correctness derives from Sley program/schema
//! semantics; must eventually be produced by the Sley checker/lowerer):
//!
//! * named type definitions and their record-field / variant-case layouts;
//! * typed constants and their well-formedness;
//! * map-key orderability / hashability / persistability traits;
//! * definition reference integrity, definition cycles, type well-formedness;
//! * import-row semantic admissibility (beyond byte-exact binding);
//! * entry-point designation, dependency closure membership.
//!
//! NATIVE RUNTIME INFORMATION (mechanical structures needed only to execute
//! an already-approved package safely):
//!
//! * byte/framing decode of the envelope and the inner image;
//! * SHA-256 digest verification over exact bytes and section digests;
//! * bounds checking (counts, lengths, ceilings) before allocation;
//! * allocation of runtime representations;
//! * exact-equality binding checks (digest == expected, row == row,
//!   type == type — never semantic inference);
//! * structural layout hydration for memory safety (duplicate-identity and
//!   count bounds only; see `hydrate_verified_definitions`).
//!
//! The [`ExecutionPackage`] binds, as required by the runtime:
//!
//! * executable image bytes/digest (`SLEYBC02`, SHA-256);
//! * constant table/value closure (typed constants, digest-bound);
//! * runtime type/layout descriptors (named definitions, digest-bound);
//! * import rows and exact row schemas (full rows, digest-bound — not IDs);
//! * global/contract inventories (bound via the dependency digest);
//! * entry point;
//! * `BOOTSTRAP_PROFILE_1` identity (digest);
//! * schema epoch; VM semantic version; host ABI identity/version;
//! * resource/limit profile (admitted limits; the request must match);
//! * admission/validation evidence identity (receipt digest);
//! * complete dependency/inventory digest.
//!
//! Native hydration ([`hydrate_package`]) MAY decode, bounds-check,
//! allocate, verify digests, and reject malformed data. It MUST NOT perform
//! definition-shape semantic judgment, resolve Sley references
//! semantically, discover definition cycles, determine map-key/hashability
//! rules, reconstruct record schemas from high-level definitions,
//! typecheck, or repair/infer missing layout data. Where runtime memory
//! safety requires local structural checks (duplicate identities, count
//! ceilings, operand-count vs field-count agreement inside
//! `execute_extended_instruction`), those are specified as structural and
//! are distinct from language semantic judgment: they prevent out-of-bounds
//! memory access on already-admitted data, they never decide whether a
//! program is well-typed.
//!
//! Machine record: `conformance/exec-package/v1/exec-package.json`
//! (authoritative for identity/version/bounds); contract doc:
//! `docs/spec/EXEC_PACKAGE_V1.md`.

use sley_check::TypeEnvironment;
use sley_id::{BytecodeCacheKey, EntityId, SchemaEpochId, SemanticFingerprint, StateRoot};
use sley_ssmc::{
    AdapterImport, BuiltinFailureKind, ConstantDefinition, ContractBinding, ContractDefinition,
    ContractKind, ContractSource, FunctionType, GlobalValueDefinition, IntegerWidth, MemberId,
    NamedType, RecordField, ResourceLimits, TypeDefForm, TypeDefinition, TypeExpr,
    TypeParameterDef, VariantCase, Visibility,
};

use crate::bootstrap::BootstrapProfileReport;
use crate::bootstrap::BootstrapProfileVersion;
use crate::host_abi::{IMAGE_MAX_BYTES, image_digest};

// Re-exported for the runner (`execute.rs`); the unit pins below use it too.
pub(crate) use crate::CacheProfile;

/// Complete execution-package identity.
pub const EXEC_PACKAGE_IDENTITY: &str = "EXEC_PACKAGE_V1";
/// Complete execution-package contract.
pub const EXEC_PACKAGE_CONTRACT: &str = "sley2-exec-package-1";
/// Complete execution-package version.
pub const EXEC_PACKAGE_VERSION: u32 = 1;
/// Complete execution-package envelope magic (`SLEYPKG1`).
pub const EXEC_PACKAGE_MAGIC: &[u8; 8] = b"SLEYPKG1";
/// Total package envelope ceiling (matches the image ceiling; sections are
/// sub-budgets inside it, not additions to it).
pub const EXEC_PACKAGE_MAX_BYTES: usize = 67_108_864;
/// Constants-section ceiling (structure + values; provisional early-R3
/// budget, NOT an increase of any frozen limit).
pub const EXEC_PACKAGE_MAX_CONSTANTS_BYTES: usize = 8_388_608;
/// Layouts-section ceiling (provisional, as above).
pub const EXEC_PACKAGE_MAX_LAYOUTS_BYTES: usize = 8_388_608;
/// Imports-section ceiling (exact rows; provisional, as above).
pub const EXEC_PACKAGE_MAX_IMPORTS_BYTES: usize = 1_048_576;
/// Dependency-section ceiling (entry/epoch/root/profile/limits plus
/// globals/contracts by reference; provisional, as above).
pub const EXEC_PACKAGE_MAX_DEPENDENCY_BYTES: usize = 8_388_608;
/// Maximum globals carried in one package (allocation bound only).
pub const EXEC_PACKAGE_MAX_GLOBALS: usize = 1_000_000;
/// Maximum contracts carried in one package (allocation bound only).
pub const EXEC_PACKAGE_MAX_CONTRACTS: usize = 1_000_000;
/// Maximum definitions carried in one package (mirrors the type-system
/// bound so hydration can allocate safely).
pub const EXEC_PACKAGE_MAX_DEFINITIONS: usize = 1_000_000;
/// Maximum constants carried in one package (allocation bound only).
pub const EXEC_PACKAGE_MAX_CONSTANTS: usize = 1_000_000;
/// Maximum imports carried in one package (allocation bound only).
pub const EXEC_PACKAGE_MAX_IMPORTS: usize = 1_000_000;

/// Frozen `BOOTSTRAP_PROFILE_1` digest bound by every v1 package (contract
/// `sley2-bootstrap-profile-1`): the raw-byte SHA-256 of
/// `conformance/bootstrap-profile/v1/profile.json`, as the frozen contract
/// states. Before 2026-09-08 this literal was a shifted hex transcription of
/// that digest (implementation erratum E1 in `EXEC_PACKAGE_V2.md`); no v1
/// package, receipt, or observation identity computed from the old literal
/// was ever persisted, and `check_exec_package_markers.py` now binds all
/// 32 bytes to the record.
pub const BOOTSTRAP_PROFILE_1_DIGEST: [u8; 32] = [
    0x4f, 0x26, 0x91, 0x50, 0x4b, 0x5c, 0x75, 0x6e, 0xae, 0x1f, 0x5e, 0xf0, 0x1e, 0x6e, 0x99, 0x8c,
    0xc4, 0xcd, 0x62, 0x8d, 0x4b, 0x52, 0x4b, 0x03, 0x8b, 0x10, 0xd5, 0x83, 0xbf, 0xef, 0xd6, 0x30,
];

/// Successor `BOOTSTRAP_PROFILE_2` digest (RW-075 correction, AR-02).
///
/// `BOOTSTRAP_PROFILE_1` (`4f269150...efd630`) is retained as history.
/// `BOOTSTRAP_PROFILE_2` (`fb2d8cc8...847459`) is the current R2 candidate:
/// strict superset adding exactly `RHW1` (`Unit, Bytes` ->
/// `Result<Bytes32, Index>`); no opcode/type/effect/capability broadening.
pub const BOOTSTRAP_PROFILE_2_DIGEST: [u8; 32] = [
    0xfb, 0x2d, 0x8c, 0xc8, 0x7e, 0xe7, 0xde, 0x68, 0xcd, 0xe8, 0x19, 0x7a, 0x77, 0x00, 0x3a, 0x41,
    0x7a, 0x00, 0x62, 0xac, 0xb6, 0xed, 0x08, 0x7d, 0x85, 0xf8, 0x99, 0xda, 0x1a, 0x84, 0x74, 0x59,
];

/// Successor execution-package identity (RW-075 correction).
pub const EXEC_PACKAGE_V2_IDENTITY: &str = "EXEC_PACKAGE_V2";
/// Successor execution-package contract.
pub const EXEC_PACKAGE_V2_CONTRACT: &str = "sley2-exec-package-2";
/// Successor execution-package version (envelope `u32` 2; v1 envelope 1
/// preserved as history).
pub const EXEC_PACKAGE_V2_VERSION: u32 = 2;
/// Fixed v2 serialized-envelope header size. The header is exactly the
/// existing package-digest preimage; five `u64` section lengths follow it.
pub const EXEC_PACKAGE_V2_ENVELOPE_HEADER_BYTES: usize = 316;
/// Fixed length-framing overhead for the five package sections.
pub const EXEC_PACKAGE_V2_ENVELOPE_LENGTH_BYTES: usize = 40;

/// Execution-package structural failure vocabulary.
///
/// Semantic failures (type errors, lowering errors, fingerprint errors) are
/// never produced on this path: the compiler already judged them and the
/// receipt binds that judgment. These codes cover framing, bounds, digest,
/// and binding mismatches only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageError {
    /// Envelope magic is not `SLEYPKG1`.
    UnknownMagic,
    /// Envelope version is not [`EXEC_PACKAGE_VERSION`].
    UnsupportedVersion,
    /// Bytes end before the structure does.
    Truncated,
    /// Envelope or a section exceeds its ceiling.
    Oversized,
    /// Trailing bytes follow the last section.
    TrailingData,
    /// A tag, shape, or count the frozen layout cannot carry.
    Malformed,
    /// A section digest does not match the header binding.
    SectionDigestMismatch,
    /// The package digest does not match the admission receipt.
    ReceiptMismatch,
    /// A complete-closure binding does not match (image, constants,
    /// layouts, imports, entry, epoch, root, profile, ABI, VM, limits).
    BindingMismatch,
    /// Structural hydration refused (duplicate identity or count bound).
    HydrationRefused,
}

impl PackageError {
    /// Returns the stable symbolic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownMagic => "PACKAGE_UNKNOWN_MAGIC",
            Self::UnsupportedVersion => "PACKAGE_UNSUPPORTED_VERSION",
            Self::Truncated => "PACKAGE_TRUNCATED",
            Self::Oversized => "PACKAGE_OVERSIZED",
            Self::TrailingData => "PACKAGE_TRAILING_DATA",
            Self::Malformed => "PACKAGE_MALFORMED",
            Self::SectionDigestMismatch => "PACKAGE_SECTION_DIGEST_MISMATCH",
            Self::ReceiptMismatch => "PACKAGE_RECEIPT_MISMATCH",
            Self::BindingMismatch => "PACKAGE_BINDING_MISMATCH",
            Self::HydrationRefused => "PACKAGE_HYDRATION_REFUSED",
        }
    }
}

impl core::fmt::Display for PackageError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::error::Error for PackageError {}

/// SHA-256 over exact bytes (host mechanic; the compiler does not construct
/// this preimage as language semantics).
#[must_use]
pub fn section_digest(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// One complete execution package (owned Rust shape).
///
/// Built by the Sley lowerer/image-builder (RW-110) and the build driver
/// (RW-120); in RW-075 tests it is built by explicit typed seed data plus
/// the deterministic seed assembler path. The host never synthesizes one
/// from high-level program definitions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionPackage {
    /// Exact `SLEYBC02` image bytes (entry + callee table).
    pub image_bytes: Vec<u8>,
    /// Complete constant inventory (`constant_ref` resolves here).
    pub constants: Vec<ConstantDefinition>,
    /// Complete named type definitions (record/variant layouts).
    pub type_definitions: Vec<TypeDefinition>,
    /// Complete adapter-import inventory (exact rows, not IDs).
    pub imports: Vec<AdapterImport>,
    /// Complete global-value inventory.
    pub globals: Vec<GlobalValueDefinition>,
    /// Complete contract inventory.
    pub contracts: Vec<ContractDefinition>,
    /// Designated entry function.
    pub entry: EntityId,
    /// Exact schema epoch.
    pub schema_epoch: SchemaEpochId,
    /// Exact state root.
    pub state_root: StateRoot,
    /// Requested cache/lowering profile (must be `EXTENDED_V1`).
    pub profile: CacheProfile,
    /// Admitted execution limits (the request must match exactly; limits
    /// bind the observation post hoc through this package).
    pub admitted_limits: crate::execute::ExecutionLimits,
    /// Gate-judged operation count for the closure (from the admission
    /// authority's report; digested so a report for another closure —
    /// even with the same entry and imports — cannot approve this
    /// package unless its quantitative claims also match).
    pub gate_operation_count: u32,
    /// Gate-admitted bridge-use count (as above).
    pub gate_bridge_uses: u32,
    /// Canonical semantic fingerprints of the judged closure, entry first
    /// (from the admission authority's sealed report; digested so the
    /// approval is bound to the exact judged closure bytes, not only to
    /// entry/import/count summaries — equal-count replays refuse).
    pub gate_closure_fingerprints: Vec<SemanticFingerprint>,
}

/// Section and package digests for one [`ExecutionPackage`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageDigests {
    /// SHA-256 over the exact image bytes.
    pub image_digest: [u8; 32],
    /// SHA-256 over the canonical constants section bytes.
    pub constants_digest: [u8; 32],
    /// SHA-256 over the canonical layouts section bytes.
    pub layouts_digest: [u8; 32],
    /// SHA-256 over the canonical exact-row imports section bytes.
    pub imports_digest: [u8; 32],
    /// SHA-256 over the dependency/inventory section bytes
    /// (globals + contracts + entry + epoch + root + profile + limits).
    pub dependency_digest: [u8; 32],
    /// The package identity: SHA-256 over the header preimage (magic,
    /// version, profile digest, ABI version, VM version, the five section
    /// digests, entry, epoch, root), never over serialized envelope bytes.
    pub package_digest: [u8; 32],
}

/// Strictly decoded v2 envelope framing and raw canonical section bytes.
///
/// This is deliberately a structural handoff. Hydrating the section bytes
/// into compiler-owned semantic inventories is a separate operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedPackageEnvelopeV2 {
    /// Exact `SLEYBC02` image section bytes.
    pub image_bytes: Vec<u8>,
    /// Exact canonical constants-section bytes.
    pub constants_bytes: Vec<u8>,
    /// Exact canonical layouts-section bytes.
    pub layouts_bytes: Vec<u8>,
    /// Exact canonical imports-section bytes.
    pub imports_bytes: Vec<u8>,
    /// Exact canonical dependency-section bytes.
    pub dependency_bytes: Vec<u8>,
    /// Entry identity repeated in the package header.
    pub entry: EntityId,
    /// Schema epoch repeated in the package header.
    pub schema_epoch: SchemaEpochId,
    /// State root repeated in the package header.
    pub state_root: StateRoot,
    /// Header and section digests, including the digest of the exact header.
    pub digests: PackageDigests,
}

/// Structurally decoded dependency/inventory section.
///
/// This carries the complete byte-level rows needed to reconstruct an
/// [`ExecutionPackage`]. It does not assert that references resolve, that
/// contracts are valid, or that the closure is semantically complete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDependencySection {
    /// Designated entry function.
    pub entry: EntityId,
    /// Exact schema epoch.
    pub schema_epoch: SchemaEpochId,
    /// Exact state root.
    pub state_root: StateRoot,
    /// Encoded cache/lowering profile.
    pub profile: CacheProfile,
    /// Encoded admitted execution limits.
    pub admitted_limits: crate::execute::ExecutionLimits,
    /// Gate-judged operation count.
    pub gate_operation_count: u32,
    /// Gate-admitted bridge-use count.
    pub gate_bridge_uses: u32,
    /// Gate-judged closure fingerprints in encoded order.
    pub gate_closure_fingerprints: Vec<SemanticFingerprint>,
    /// Complete global-value inventory.
    pub globals: Vec<GlobalValueDefinition>,
    /// Complete contract inventory.
    pub contracts: Vec<ContractDefinition>,
}

/// A v2 envelope after strict framing and section hydration.
///
/// Admission is deliberately absent: this result reconstructs exact package
/// bytes and their digests, but never mints or substitutes an admission
/// receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HydratedPackageEnvelopeV2 {
    /// Reconstructed execution package.
    pub package: ExecutionPackage,
    /// Authenticated section and package digests from the envelope header.
    pub digests: PackageDigests,
}

/// The admission receipt for one exact package (separately accepted
/// authority; never the Rust semantic checker rerun at execution time).
///
/// Sealed two ways (like `BootstrapProfileReport`): `#[non_exhaustive]`
/// prevents downstream struct-literal construction, and private fields
/// prevent downstream mutation or forgery of a genuine receipt. Reads go
/// through the `package_digest`, `profile_digest`, and `host_abi_version`
/// accessors. V1 receipts come from [`admit_package`] (historical staged
/// authority); v2 receipts come exclusively from the staged authority
/// (`crate::admission_authority::admit_v2_package`, via the crate-private
/// [`admit_package_v2`] constructor).
///
/// Produced by the staged admission authority (gate report + package digest
/// binding) and verified byte-for-byte before anything runs. The host never
/// decides which package is approved: it compares digests.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionReceipt {
    /// The exact package digest this receipt approves.
    package_digest: [u8; 32],
    /// The bootstrap profile digest the admission was judged under
    /// (v1 digest for v1 receipts, successor digest for v2 receipts).
    profile_digest: [u8; 32],
    /// The host ABI version the admission was judged under
    /// (1 for v1 receipts, 2 for v2 receipts).
    host_abi_version: u32,
}

impl AdmissionReceipt {
    /// The exact package digest this receipt approves.
    #[must_use]
    pub const fn package_digest(&self) -> &[u8; 32] {
        &self.package_digest
    }

    /// The bootstrap profile digest the admission was judged under.
    #[must_use]
    pub const fn profile_digest(&self) -> &[u8; 32] {
        &self.profile_digest
    }

    /// The host ABI version the admission was judged under.
    #[must_use]
    pub const fn host_abi_version(&self) -> u32 {
        self.host_abi_version
    }
}

/// The complete approved execution-package binding (AR-03 successor to the
/// incomplete `ApprovedImage`, which bound only digest + cache key +
/// import IDs).
///
/// Every field is verified against the supplied package and receipt before
/// execution. Limits are included: the execution request must equal
/// `admitted_limits` exactly, so out-of-profile budgets cannot ride an
/// approval for another budget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovedExecutionPackage {
    /// Expected package identity (SHA-256 over the header preimage).
    pub package_digest: [u8; 32],
    /// Expected image identity (SHA-256 over the exact image bytes).
    pub image_digest: [u8; 32],
    /// Expected complete runtime data/constants digest.
    pub constants_digest: [u8; 32],
    /// Expected complete runtime-layout digest.
    pub layouts_digest: [u8; 32],
    /// Expected exact import-row manifest digest (full rows, not IDs).
    pub imports_digest: [u8; 32],
    /// Expected complete dependency/inventory digest.
    pub dependency_digest: [u8; 32],
    /// Expected manifest-approved cache identity (epoch, root, entry,
    /// profile — re-derived, never trusted from the package).
    pub cache_key: BytecodeCacheKey,
    /// Expected exact import rows (full schemas; compared with `==`).
    pub imports: Vec<AdapterImport>,
    /// Expected entry function.
    pub entry: EntityId,
    /// Expected schema epoch.
    pub schema_epoch: SchemaEpochId,
    /// Expected state root.
    pub state_root: StateRoot,
    /// Expected cache/lowering profile.
    pub profile: CacheProfile,
    /// Expected `BOOTSTRAP_PROFILE_1` digest.
    pub profile_digest: [u8; 32],
    /// Expected VM semantic version.
    pub vm_version: [u32; 3],
    /// Expected host ABI version.
    pub host_abi_version: u32,
    /// Expected admitted limits (request must match exactly).
    pub admitted_limits: crate::execute::ExecutionLimits,
    /// The admission receipt approving exactly this package.
    pub receipt: AdmissionReceipt,
}

// ---------------------------------------------------------------------------
// Structural type/layout codec (memory-safety framing only).
//
// Tags mirror `host_abi::Cursor::type_expr` exactly so the same structural
// shapes travel in the package and the image. This encoder/decoder carries
// widths, counts, and versions without judging any of them: opcode
// admissibility, type agreement, reference integrity, trait rules, and
// resource ceilings stay with the compiler (bound via receipt), never with
// this codec.
// ---------------------------------------------------------------------------

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn encode_visibility(output: &mut Vec<u8>, value: Visibility) {
    push_u32(output, value.tag());
}

/// Structural type encoder with depth and byte-budget guards.
///
/// `depth` mirrors the loader's `MAX_TYPE_DEPTH` bound so attacker-shaped
/// nesting cannot overflow the encoder stack; `ceiling` is the owning
/// section's byte bound, enforced at every nesting level (not just after
/// the complete value) so one deeply nested type cannot greatly exceed it
/// before refusal. Both are memory-safety framing, never semantic
/// judgment: well-formedness stays with the compiler.
///
/// # Errors
///
/// Returns `Malformed` past the nesting bound, or `Oversized` past the
/// byte ceiling.
#[allow(clippy::too_many_lines)]
fn encode_type_expr(
    output: &mut Vec<u8>,
    value: &TypeExpr,
    depth: usize,
    ceiling: usize,
) -> Result<(), PackageError> {
    if depth > sley_ssmc::MAX_TYPE_DEPTH {
        return Err(PackageError::Malformed);
    }
    if output.len() > ceiling {
        return Err(PackageError::Oversized);
    }
    let next = depth.saturating_add(1);
    match value {
        TypeExpr::Unit => push_u32(output, 1),
        TypeExpr::Bool => push_u32(output, 2),
        TypeExpr::SInt(width) => {
            push_u32(output, 3);
            output.extend_from_slice(&width.bits().to_be_bytes());
        }
        TypeExpr::UInt(width) => {
            push_u32(output, 4);
            output.extend_from_slice(&width.bits().to_be_bytes());
        }
        TypeExpr::F32 => push_u32(output, 5),
        TypeExpr::F64 => push_u32(output, 6),
        TypeExpr::Bytes => push_u32(output, 7),
        TypeExpr::Text => push_u32(output, 8),
        TypeExpr::Tuple(items) => {
            push_u32(output, 9);
            push_u64(output, items.len() as u64);
            for item in items {
                encode_type_expr(output, item, next, ceiling)?;
            }
        }
        TypeExpr::Named(named) => {
            push_u32(output, 10);
            push_u32(output, 1);
            output.extend_from_slice(named.definition.as_bytes());
            push_u64(output, named.arguments.len() as u64);
            for argument in &named.arguments {
                encode_type_expr(output, argument, next, ceiling)?;
            }
        }
        TypeExpr::Vector(inner) => {
            push_u32(output, 11);
            encode_type_expr(output, inner, next, ceiling)?;
        }
        TypeExpr::OrderedMap { key, value } => {
            push_u32(output, 12);
            encode_type_expr(output, key, next, ceiling)?;
            encode_type_expr(output, value, next, ceiling)?;
        }
        TypeExpr::Option(inner) => {
            push_u32(output, 13);
            encode_type_expr(output, inner, next, ceiling)?;
        }
        TypeExpr::Result { ok, error } => {
            push_u32(output, 14);
            encode_type_expr(output, ok, next, ceiling)?;
            encode_type_expr(output, error, next, ceiling)?;
        }
        TypeExpr::FunctionRef(function) => {
            push_u32(output, 15);
            push_u64(output, function.parameters.len() as u64);
            for parameter in &function.parameters {
                encode_type_expr(output, parameter, next, ceiling)?;
            }
            encode_type_expr(output, &function.result, next, ceiling)?;
            push_u64(output, function.effects.len() as u64);
            for effect in &function.effects {
                push_u32(output, 1);
                output.extend_from_slice(effect.as_bytes());
            }
        }
        TypeExpr::AdapterHandle(id) => {
            push_u32(output, 16);
            push_u32(output, 1);
            output.extend_from_slice(id.as_bytes());
        }
        TypeExpr::CapabilityToken(id) => {
            push_u32(output, 17);
            push_u32(output, 1);
            output.extend_from_slice(id.as_bytes());
        }
        TypeExpr::LocalCell(inner) => {
            push_u32(output, 18);
            encode_type_expr(output, inner, next, ceiling)?;
        }
        TypeExpr::TypeParameter(ordinal) => {
            push_u32(output, 19);
            push_u32(output, *ordinal);
        }
        TypeExpr::BuiltinFailure(kind) => {
            push_u32(output, 20);
            let code: u16 = match kind {
                BuiltinFailureKind::Arithmetic => 1,
                BuiltinFailureKind::Index => 2,
                BuiltinFailureKind::DuplicateKey => 3,
                BuiltinFailureKind::ContractViolation => 4,
                BuiltinFailureKind::Capability => 5,
            };
            output.extend_from_slice(&code.to_be_bytes());
        }
    }
    if output.len() > ceiling {
        return Err(PackageError::Oversized);
    }
    Ok(())
}

fn decoded_count(
    cursor: &mut PackageEnvelopeCursor<'_>,
    maximum: usize,
    minimum_item_bytes: usize,
) -> Result<usize, PackageError> {
    let count = usize::try_from(cursor.u64()?).map_err(|_| PackageError::Oversized)?;
    if count > maximum {
        return Err(PackageError::Oversized);
    }
    if minimum_item_bytes != 0 && count > cursor.remaining() / minimum_item_bytes {
        return Err(PackageError::Truncated);
    }
    Ok(count)
}

fn decoded_visibility(tag: u32) -> Result<Visibility, PackageError> {
    match tag {
        1 => Ok(Visibility::Private),
        2 => Ok(Visibility::Package),
        3 => Ok(Visibility::Workspace),
        4 => Ok(Visibility::Exported),
        _ => Err(PackageError::Malformed),
    }
}

fn decoded_builtin_failure(tag: u16) -> Result<BuiltinFailureKind, PackageError> {
    match tag {
        1 => Ok(BuiltinFailureKind::Arithmetic),
        2 => Ok(BuiltinFailureKind::Index),
        3 => Ok(BuiltinFailureKind::DuplicateKey),
        4 => Ok(BuiltinFailureKind::ContractViolation),
        5 => Ok(BuiltinFailureKind::Capability),
        _ => Err(PackageError::Malformed),
    }
}

#[allow(clippy::too_many_lines)]
fn decode_type_expr(
    cursor: &mut PackageEnvelopeCursor<'_>,
    depth: usize,
) -> Result<TypeExpr, PackageError> {
    if depth > sley_ssmc::MAX_TYPE_DEPTH {
        return Err(PackageError::Malformed);
    }
    let next = depth.saturating_add(1);
    match cursor.u32()? {
        1 => Ok(TypeExpr::Unit),
        2 => Ok(TypeExpr::Bool),
        3 => Ok(TypeExpr::SInt(IntegerWidth::from_bits(cursor.u16()?))),
        4 => Ok(TypeExpr::UInt(IntegerWidth::from_bits(cursor.u16()?))),
        5 => Ok(TypeExpr::F32),
        6 => Ok(TypeExpr::F64),
        7 => Ok(TypeExpr::Bytes),
        8 => Ok(TypeExpr::Text),
        9 => {
            let count = decoded_count(cursor, sley_ssmc::MAX_TUPLE_ITEMS, 4)?;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                items.push(decode_type_expr(cursor, next)?);
            }
            Ok(TypeExpr::Tuple(items))
        }
        10 => {
            if cursor.u32()? != 1 {
                return Err(PackageError::Malformed);
            }
            let definition = EntityId::from_bytes(cursor.fixed_32()?);
            let count = decoded_count(cursor, sley_ssmc::MAX_TYPE_ARGUMENTS, 4)?;
            let mut arguments = Vec::with_capacity(count);
            for _ in 0..count {
                arguments.push(decode_type_expr(cursor, next)?);
            }
            Ok(TypeExpr::Named(NamedType {
                definition,
                arguments,
            }))
        }
        11 => Ok(TypeExpr::Vector(Box::new(decode_type_expr(cursor, next)?))),
        12 => Ok(TypeExpr::OrderedMap {
            key: Box::new(decode_type_expr(cursor, next)?),
            value: Box::new(decode_type_expr(cursor, next)?),
        }),
        13 => Ok(TypeExpr::Option(Box::new(decode_type_expr(cursor, next)?))),
        14 => Ok(TypeExpr::Result {
            ok: Box::new(decode_type_expr(cursor, next)?),
            error: Box::new(decode_type_expr(cursor, next)?),
        }),
        15 => {
            let count = decoded_count(cursor, sley_ssmc::MAX_MEMBERS, 4)?;
            let mut parameters = Vec::with_capacity(count);
            for _ in 0..count {
                parameters.push(decode_type_expr(cursor, next)?);
            }
            let result = Box::new(decode_type_expr(cursor, next)?);
            let effect_count = decoded_count(cursor, EXEC_PACKAGE_MAX_DEFINITIONS, 36)?;
            let mut effects = Vec::with_capacity(effect_count);
            for _ in 0..effect_count {
                if cursor.u32()? != 1 {
                    return Err(PackageError::Malformed);
                }
                effects.push(EntityId::from_bytes(cursor.fixed_32()?));
            }
            Ok(TypeExpr::FunctionRef(FunctionType {
                parameters,
                result,
                effects,
            }))
        }
        16 => {
            if cursor.u32()? != 1 {
                return Err(PackageError::Malformed);
            }
            Ok(TypeExpr::AdapterHandle(EntityId::from_bytes(
                cursor.fixed_32()?,
            )))
        }
        17 => {
            if cursor.u32()? != 1 {
                return Err(PackageError::Malformed);
            }
            Ok(TypeExpr::CapabilityToken(EntityId::from_bytes(
                cursor.fixed_32()?,
            )))
        }
        18 => Ok(TypeExpr::LocalCell(Box::new(decode_type_expr(
            cursor, next,
        )?))),
        19 => Ok(TypeExpr::TypeParameter(cursor.u32()?)),
        20 => Ok(TypeExpr::BuiltinFailure(decoded_builtin_failure(
            cursor.u16()?,
        )?)),
        _ => Err(PackageError::Malformed),
    }
}

fn decode_type_definition(input: &[u8]) -> Result<TypeDefinition, PackageError> {
    let mut cursor = PackageEnvelopeCursor::new(input);
    let entity_id = EntityId::from_bytes(cursor.fixed_32()?);
    let parameter_count = decoded_count(&mut cursor, sley_ssmc::MAX_TYPE_ARGUMENTS, 4)?;
    let mut type_parameters = Vec::with_capacity(parameter_count);
    for _ in 0..parameter_count {
        type_parameters.push(TypeParameterDef {
            ordinal: cursor.u32()?,
        });
    }
    let form = match cursor.u32()? {
        1 => {
            let field_count = decoded_count(&mut cursor, sley_ssmc::MAX_MEMBERS, 40)?;
            let mut fields = Vec::with_capacity(field_count);
            for _ in 0..field_count {
                fields.push(RecordField {
                    member_id: MemberId::from_bytes(cursor.fixed_32()?),
                    value_type: decode_type_expr(&mut cursor, 1)?,
                    visibility: decoded_visibility(cursor.u32()?)?,
                });
            }
            TypeDefForm::Record(fields)
        }
        2 => {
            let case_count = decoded_count(&mut cursor, sley_ssmc::MAX_MEMBERS, 36)?;
            let mut cases = Vec::with_capacity(case_count);
            for _ in 0..case_count {
                let member_id = MemberId::from_bytes(cursor.fixed_32()?);
                let payload_type = match cursor.u32()? {
                    1 => None,
                    2 => Some(decode_type_expr(&mut cursor, 1)?),
                    _ => return Err(PackageError::Malformed),
                };
                cases.push(VariantCase {
                    member_id,
                    payload_type,
                });
            }
            TypeDefForm::Variant(cases)
        }
        _ => return Err(PackageError::Malformed),
    };
    let invariant_count = decoded_count(&mut cursor, EXEC_PACKAGE_MAX_CONTRACTS, 32)?;
    let mut invariants = Vec::with_capacity(invariant_count);
    for _ in 0..invariant_count {
        invariants.push(EntityId::from_bytes(cursor.fixed_32()?));
    }
    let visibility = decoded_visibility(cursor.u32()?)?;
    if cursor.remaining() != 0 {
        return Err(PackageError::Malformed);
    }
    Ok(TypeDefinition {
        entity_id,
        type_parameters,
        form,
        invariants,
        visibility,
    })
}

/// Structural definition encoder under the owning section ceiling.
///
/// # Errors
///
/// Returns `Malformed` past the nesting bound, or `Oversized` past the
/// byte ceiling.
fn encode_type_definition(
    output: &mut Vec<u8>,
    definition: &TypeDefinition,
    ceiling: usize,
) -> Result<(), PackageError> {
    if output.len() > ceiling {
        return Err(PackageError::Oversized);
    }
    let next = 1_usize;
    output.extend_from_slice(definition.entity_id.as_bytes());
    push_u64(output, definition.type_parameters.len() as u64);
    for parameter in &definition.type_parameters {
        push_u32(output, parameter.ordinal);
    }
    match &definition.form {
        TypeDefForm::Record(fields) => {
            push_u32(output, 1);
            push_u64(output, fields.len() as u64);
            for field in fields {
                output.extend_from_slice(field.member_id.as_bytes());
                encode_type_expr(output, &field.value_type, next, ceiling)?;
                encode_visibility(output, field.visibility);
            }
        }
        TypeDefForm::Variant(cases) => {
            push_u32(output, 2);
            push_u64(output, cases.len() as u64);
            for case in cases {
                output.extend_from_slice(case.member_id.as_bytes());
                match &case.payload_type {
                    None => push_u32(output, 1),
                    Some(payload) => {
                        push_u32(output, 2);
                        encode_type_expr(output, payload, next, ceiling)?;
                    }
                }
            }
        }
    }
    push_u64(output, definition.invariants.len() as u64);
    for invariant in &definition.invariants {
        output.extend_from_slice(invariant.as_bytes());
    }
    encode_visibility(output, definition.visibility);
    if output.len() > ceiling {
        return Err(PackageError::Oversized);
    }
    Ok(())
}

/// Structural import-row encoder under the owning section ceiling.
///
/// # Errors
///
/// Returns `Malformed` past the nesting bound, or `Oversized` past the
/// byte ceiling.
fn encode_adapter_import(
    output: &mut Vec<u8>,
    row: &AdapterImport,
    ceiling: usize,
) -> Result<(), PackageError> {
    if output.len() > ceiling {
        return Err(PackageError::Oversized);
    }
    let next = 1_usize;
    output.extend_from_slice(row.entity_id.as_bytes());
    output.extend_from_slice(&row.adapter_id);
    push_u32(output, row.abi_version);
    encode_type_expr(output, &row.request_type, next, ceiling)?;
    encode_type_expr(output, &row.response_type, next, ceiling)?;
    encode_type_expr(output, &row.failure_type, next, ceiling)?;
    push_u64(output, row.effects.len() as u64);
    for effect in &row.effects {
        output.extend_from_slice(effect.as_bytes());
    }
    if output.len() > ceiling {
        return Err(PackageError::Oversized);
    }
    Ok(())
}

fn decode_adapter_import(input: &[u8]) -> Result<AdapterImport, PackageError> {
    let mut cursor = PackageEnvelopeCursor::new(input);
    let entity_id = EntityId::from_bytes(cursor.fixed_32()?);
    let adapter_id = cursor.fixed_32()?;
    let abi_version = cursor.u32()?;
    let request_type = decode_type_expr(&mut cursor, 1)?;
    let response_type = decode_type_expr(&mut cursor, 1)?;
    let failure_type = decode_type_expr(&mut cursor, 1)?;
    let effect_count = decoded_count(&mut cursor, EXEC_PACKAGE_MAX_DEFINITIONS, 32)?;
    let mut effects = Vec::with_capacity(effect_count);
    for _ in 0..effect_count {
        effects.push(EntityId::from_bytes(cursor.fixed_32()?));
    }
    if cursor.remaining() != 0 {
        return Err(PackageError::Malformed);
    }
    Ok(AdapterImport {
        entity_id,
        adapter_id,
        abi_version,
        request_type,
        response_type,
        failure_type,
        effects,
    })
}

/// Canonical constants-section bytes (count plus length-prefixed rows of
/// entity identity beside the encoded constant value).
///
/// The existing canonical codec provides structural framing plus
/// canonical-form enforcement here; using it binds the exact runtime
/// values without any language-level constant judgment by the host.
///
/// # Errors
///
/// Returns `Oversized` when the section exceeds its ceiling, or `Malformed`
/// when a value has no canonical encoding.
pub fn encode_constants_section(constants: &[ConstantDefinition]) -> Result<Vec<u8>, PackageError> {
    if constants.len() > EXEC_PACKAGE_MAX_CONSTANTS {
        return Err(PackageError::Oversized);
    }
    let mut output = Vec::new();
    push_u64(&mut output, constants.len() as u64);
    for constant in constants {
        output.extend_from_slice(constant.entity_id.as_bytes());
        let encoded = sley_mutate::encode_const_value(&constant.value)
            .map_err(|_| PackageError::Malformed)?;
        if output.len().saturating_add(encoded.len()) > EXEC_PACKAGE_MAX_CONSTANTS_BYTES {
            return Err(PackageError::Oversized);
        }
        push_u64(&mut output, encoded.len() as u64);
        output.extend_from_slice(&encoded);
    }
    if output.len() > EXEC_PACKAGE_MAX_CONSTANTS_BYTES {
        return Err(PackageError::Oversized);
    }
    Ok(output)
}

/// Strictly decodes the canonical constants section.
///
/// This is structural hydration only: it checks framing, the count and byte
/// ceilings, duplicate identities, and canonical constant-value encoding.
/// It performs no type or persistability judgment.
///
/// # Errors
///
/// Returns `Oversized` for section/count ceilings, `Truncated` for incomplete
/// rows, `Malformed` for non-canonical values or bytes after the declared
/// inventory, and `HydrationRefused` for duplicate identities.
pub fn decode_constants_section(input: &[u8]) -> Result<Vec<ConstantDefinition>, PackageError> {
    const MINIMUM_ROW_BYTES: usize = 32 + 8;

    if input.len() > EXEC_PACKAGE_MAX_CONSTANTS_BYTES {
        return Err(PackageError::Oversized);
    }
    let mut cursor = PackageEnvelopeCursor::new(input);
    let count = usize::try_from(cursor.u64()?).map_err(|_| PackageError::Oversized)?;
    if count > EXEC_PACKAGE_MAX_CONSTANTS {
        return Err(PackageError::Oversized);
    }
    if count > cursor.remaining() / MINIMUM_ROW_BYTES {
        return Err(PackageError::Truncated);
    }
    let mut constants = Vec::with_capacity(count);
    let mut identities = std::collections::BTreeSet::new();
    for _ in 0..count {
        let entity_id = EntityId::from_bytes(cursor.fixed_32()?);
        if !identities.insert(entity_id) {
            return Err(PackageError::HydrationRefused);
        }
        let value_len = usize::try_from(cursor.u64()?).map_err(|_| PackageError::Oversized)?;
        if value_len > EXEC_PACKAGE_MAX_CONSTANTS_BYTES {
            return Err(PackageError::Oversized);
        }
        let value = sley_mutate::decode_const_value(cursor.take(value_len)?)
            .map_err(|_| PackageError::Malformed)?;
        constants.push(ConstantDefinition { entity_id, value });
    }
    if cursor.remaining() != 0 {
        return Err(PackageError::Malformed);
    }
    Ok(constants)
}

/// Canonical layouts-section bytes: count + each encoded definition.
///
/// # Errors
///
/// Returns `Oversized` when the section exceeds its ceiling.
pub fn encode_layouts_section(definitions: &[TypeDefinition]) -> Result<Vec<u8>, PackageError> {
    if definitions.len() > EXEC_PACKAGE_MAX_DEFINITIONS {
        return Err(PackageError::Oversized);
    }
    let mut output = Vec::new();
    push_u64(&mut output, definitions.len() as u64);
    for definition in definitions {
        let mut encoded = Vec::new();
        encode_type_definition(&mut encoded, definition, EXEC_PACKAGE_MAX_LAYOUTS_BYTES)?;
        if output.len().saturating_add(encoded.len()) > EXEC_PACKAGE_MAX_LAYOUTS_BYTES {
            return Err(PackageError::Oversized);
        }
        push_u64(&mut output, encoded.len() as u64);
        output.append(&mut encoded);
    }
    if output.len() > EXEC_PACKAGE_MAX_LAYOUTS_BYTES {
        return Err(PackageError::Oversized);
    }
    Ok(output)
}

/// Strictly decodes the canonical layouts section without semantic judgment.
///
/// # Errors
///
/// Returns structural `PackageError` values for bounds, truncation, invalid
/// tags/shapes, bytes after a row or section, and duplicate definition IDs.
pub fn decode_layouts_section(input: &[u8]) -> Result<Vec<TypeDefinition>, PackageError> {
    // u64 row length + the smallest definition row: entity, empty type
    // parameters, record tag, empty field count, empty invariant count,
    // visibility.
    const MINIMUM_FRAMED_DEFINITION_BYTES: usize = 8 + 32 + 8 + 4 + 8 + 8 + 4;

    if input.len() > EXEC_PACKAGE_MAX_LAYOUTS_BYTES {
        return Err(PackageError::Oversized);
    }
    let mut cursor = PackageEnvelopeCursor::new(input);
    let count = decoded_count(
        &mut cursor,
        EXEC_PACKAGE_MAX_DEFINITIONS,
        MINIMUM_FRAMED_DEFINITION_BYTES,
    )?;
    let mut definitions = Vec::with_capacity(count);
    let mut identities = std::collections::BTreeSet::new();
    for _ in 0..count {
        let row_len = usize::try_from(cursor.u64()?).map_err(|_| PackageError::Oversized)?;
        if row_len > EXEC_PACKAGE_MAX_LAYOUTS_BYTES {
            return Err(PackageError::Oversized);
        }
        let definition = decode_type_definition(cursor.take(row_len)?)?;
        if !identities.insert(definition.entity_id) {
            return Err(PackageError::HydrationRefused);
        }
        definitions.push(definition);
    }
    if cursor.remaining() != 0 {
        return Err(PackageError::Malformed);
    }
    Ok(definitions)
}

/// Canonical exact-row imports-section bytes: count + each encoded row.
///
/// This binds exact row schemas (request/response/failure/effects), not
/// just import IDs — the AR-03 repair of the `ApprovedImage` gap.
///
/// # Errors
///
/// Returns `Oversized` when the section exceeds its ceiling.
pub fn encode_imports_section(imports: &[AdapterImport]) -> Result<Vec<u8>, PackageError> {
    if imports.len() > EXEC_PACKAGE_MAX_IMPORTS {
        return Err(PackageError::Oversized);
    }
    let mut output = Vec::new();
    push_u64(&mut output, imports.len() as u64);
    for row in imports {
        let mut encoded = Vec::new();
        encode_adapter_import(&mut encoded, row, EXEC_PACKAGE_MAX_IMPORTS_BYTES)?;
        if output.len().saturating_add(encoded.len()) > EXEC_PACKAGE_MAX_IMPORTS_BYTES {
            return Err(PackageError::Oversized);
        }
        push_u64(&mut output, encoded.len() as u64);
        output.append(&mut encoded);
    }
    if output.len() > EXEC_PACKAGE_MAX_IMPORTS_BYTES {
        return Err(PackageError::Oversized);
    }
    Ok(output)
}

/// Strictly decodes the canonical exact-row imports section.
///
/// # Errors
///
/// Returns structural `PackageError` values for bounds, truncation, invalid
/// tags/shapes, bytes after a row or section, and duplicate entity or adapter
/// identities. Registry admissibility remains outside this decoder.
pub fn decode_imports_section(input: &[u8]) -> Result<Vec<AdapterImport>, PackageError> {
    // u64 row length + entity, adapter, ABI, three minimum type tags, and an
    // empty effects count.
    const MINIMUM_FRAMED_IMPORT_BYTES: usize = 8 + 32 + 32 + 4 + (3 * 4) + 8;

    if input.len() > EXEC_PACKAGE_MAX_IMPORTS_BYTES {
        return Err(PackageError::Oversized);
    }
    let mut cursor = PackageEnvelopeCursor::new(input);
    let count = decoded_count(
        &mut cursor,
        EXEC_PACKAGE_MAX_IMPORTS,
        MINIMUM_FRAMED_IMPORT_BYTES,
    )?;
    let mut imports = Vec::with_capacity(count);
    let mut entity_ids = std::collections::BTreeSet::new();
    let mut adapter_ids = std::collections::BTreeSet::new();
    for _ in 0..count {
        let row_len = usize::try_from(cursor.u64()?).map_err(|_| PackageError::Oversized)?;
        if row_len > EXEC_PACKAGE_MAX_IMPORTS_BYTES {
            return Err(PackageError::Oversized);
        }
        let row = decode_adapter_import(cursor.take(row_len)?)?;
        if !entity_ids.insert(row.entity_id) || !adapter_ids.insert(row.adapter_id) {
            return Err(PackageError::HydrationRefused);
        }
        imports.push(row);
    }
    if cursor.remaining() != 0 {
        return Err(PackageError::Malformed);
    }
    Ok(imports)
}

/// Dependency/inventory section bytes: entry + epoch + root + profile +
/// VM/lowerer versions + admitted limits + global/contract counts and IDs.
///
/// Globals and contracts carry their complete structural rows. Entity
/// references inside those rows remain unresolved: their semantic validity is
/// compiler-owned and is bound by admission rather than judged by this codec.
/// This section binds the complete dependency/inventory digest.
///
/// # Errors
///
/// Returns `Oversized` when the inventory exceeds its count bounds or the
/// section exceeds its byte ceiling.
#[allow(clippy::too_many_lines)]
pub fn encode_dependency_section(package: &ExecutionPackage) -> Result<Vec<u8>, PackageError> {
    if package.globals.len() > EXEC_PACKAGE_MAX_GLOBALS
        || package.contracts.len() > EXEC_PACKAGE_MAX_CONTRACTS
    {
        return Err(PackageError::Oversized);
    }
    let mut output = Vec::new();
    output.extend_from_slice(package.entry.as_bytes());
    output.extend_from_slice(package.schema_epoch.as_bytes());
    output.extend_from_slice(package.state_root.as_bytes());
    for part in package.profile.vm_version {
        push_u32(&mut output, part);
    }
    push_u32(&mut output, package.profile.lowering_profile);
    for part in package.profile.lowerer_version {
        push_u32(&mut output, part);
    }
    push_u64(&mut output, package.profile.entry_type_arguments);
    push_u64(&mut output, package.profile.adapter_abi_entries);
    push_u64(&mut output, package.profile.execution_abi_flags);
    push_u32(&mut output, package.gate_operation_count);
    push_u32(&mut output, package.gate_bridge_uses);
    push_u64(&mut output, package.gate_closure_fingerprints.len() as u64);
    for fingerprint in &package.gate_closure_fingerprints {
        output.extend_from_slice(fingerprint.as_bytes());
        // Incremental ceiling: each fingerprint row is checked as encoded.
        if output.len() > EXEC_PACKAGE_MAX_DEPENDENCY_BYTES {
            return Err(PackageError::Oversized);
        }
    }
    push_u64(&mut output, package.admitted_limits.max_instructions);
    push_u64(&mut output, package.admitted_limits.max_fuel);
    push_u64(&mut output, package.admitted_limits.max_value_units);
    push_u64(&mut output, package.admitted_limits.max_output_units);
    match package.admitted_limits.cancel_at_fuel {
        None => push_u32(&mut output, 1),
        Some(value) => {
            push_u32(&mut output, 2);
            push_u64(&mut output, value);
        }
    }
    push_u64(&mut output, package.globals.len() as u64);
    for global in &package.globals {
        output.extend_from_slice(global.entity_id.as_bytes());
        encode_type_expr(
            &mut output,
            &global.value_type,
            1,
            EXEC_PACKAGE_MAX_DEPENDENCY_BYTES,
        )?;
        output.extend_from_slice(global.initializer.as_bytes());
        encode_visibility(&mut output, global.visibility);
        // Incremental ceiling: refuse as soon as growth exceeds the bound
        // instead of encoding the whole inventory first. Allocation can
        // never greatly exceed the ceiling before refusal.
        if output.len() > EXEC_PACKAGE_MAX_DEPENDENCY_BYTES {
            return Err(PackageError::Oversized);
        }
    }
    push_u64(&mut output, package.contracts.len() as u64);
    for contract in &package.contracts {
        output.extend_from_slice(contract.entity_id.as_bytes());
        output.extend_from_slice(contract.target.as_bytes());
        push_u32(&mut output, contract.contract_kind.tag());
        output.extend_from_slice(contract.predicate.as_bytes());
        push_u64(&mut output, contract.bindings.len() as u64);
        for binding in &contract.bindings {
            push_u32(&mut output, binding.predicate_parameter);
            push_u32(&mut output, binding.source.tag());
            match binding.source {
                sley_ssmc::ContractSource::Parameter(id)
                | sley_ssmc::ContractSource::Global(id) => {
                    output.extend_from_slice(id.as_bytes());
                }
                sley_ssmc::ContractSource::Result | sley_ssmc::ContractSource::Error => {}
            }
            if output.len() > EXEC_PACKAGE_MAX_DEPENDENCY_BYTES {
                return Err(PackageError::Oversized);
            }
        }
        match &contract.resource_limits {
            None => push_u32(&mut output, 1),
            Some(limits) => {
                push_u32(&mut output, 2);
                push_u64(&mut output, limits.fuel);
                push_u64(&mut output, limits.memory_bytes);
                push_u64(&mut output, limits.output_bytes);
                push_u64(&mut output, limits.effect_count);
                push_u64(&mut output, limits.call_depth);
                push_u64(&mut output, limits.wall_timeout_millis);
            }
        }
        if output.len() > EXEC_PACKAGE_MAX_DEPENDENCY_BYTES {
            return Err(PackageError::Oversized);
        }
    }
    if output.len() > EXEC_PACKAGE_MAX_DEPENDENCY_BYTES {
        return Err(PackageError::Oversized);
    }
    Ok(output)
}

fn decoded_contract_kind(tag: u32) -> Result<ContractKind, PackageError> {
    match tag {
        1 => Ok(ContractKind::Precondition),
        2 => Ok(ContractKind::Postcondition),
        3 => Ok(ContractKind::Invariant),
        4 => Ok(ContractKind::EffectBound),
        5 => Ok(ContractKind::CapabilityBound),
        6 => Ok(ContractKind::ResultPredicate),
        7 => Ok(ContractKind::ResourceCeiling),
        _ => Err(PackageError::Malformed),
    }
}

fn decode_contract_source(
    cursor: &mut PackageEnvelopeCursor<'_>,
) -> Result<ContractSource, PackageError> {
    match cursor.u32()? {
        1 => Ok(ContractSource::Parameter(EntityId::from_bytes(
            cursor.fixed_32()?,
        ))),
        2 => Ok(ContractSource::Result),
        3 => Ok(ContractSource::Error),
        4 => Ok(ContractSource::Global(EntityId::from_bytes(
            cursor.fixed_32()?,
        ))),
        _ => Err(PackageError::Malformed),
    }
}

/// Strictly decodes the dependency/inventory section.
///
/// The decoder bounds every allocation, rejects duplicate global/contract
/// identities, and carries types and references exactly. It does not resolve
/// references, judge contracts, or validate the claimed gate evidence.
///
/// # Errors
///
/// Returns structural `PackageError` values for byte/count ceilings,
/// truncation, invalid tags, trailing bytes, and duplicate identities.
#[allow(clippy::too_many_lines)]
pub fn decode_dependency_section(input: &[u8]) -> Result<DecodedDependencySection, PackageError> {
    const MINIMUM_GLOBAL_BYTES: usize = 32 + 4 + 32 + 4;
    const MINIMUM_CONTRACT_BYTES: usize = 32 + 32 + 4 + 32 + 8 + 4;

    if input.len() > EXEC_PACKAGE_MAX_DEPENDENCY_BYTES {
        return Err(PackageError::Oversized);
    }
    let mut cursor = PackageEnvelopeCursor::new(input);
    let entry = EntityId::from_bytes(cursor.fixed_32()?);
    let schema_epoch = SchemaEpochId::from_bytes(cursor.fixed_32()?);
    let state_root = StateRoot::from_bytes(cursor.fixed_32()?);
    let profile = CacheProfile {
        vm_version: [cursor.u32()?, cursor.u32()?, cursor.u32()?],
        lowering_profile: cursor.u32()?,
        lowerer_version: [cursor.u32()?, cursor.u32()?, cursor.u32()?],
        entry_type_arguments: cursor.u64()?,
        adapter_abi_entries: cursor.u64()?,
        execution_abi_flags: cursor.u64()?,
    };
    let gate_operation_count = cursor.u32()?;
    let gate_bridge_uses = cursor.u32()?;
    let fingerprint_count = decoded_count(&mut cursor, EXEC_PACKAGE_MAX_DEPENDENCY_BYTES / 32, 32)?;
    let mut gate_closure_fingerprints = Vec::with_capacity(fingerprint_count);
    for _ in 0..fingerprint_count {
        gate_closure_fingerprints.push(SemanticFingerprint::from_bytes(cursor.fixed_32()?));
    }
    let admitted_limits = crate::execute::ExecutionLimits {
        max_instructions: cursor.u64()?,
        max_fuel: cursor.u64()?,
        max_value_units: cursor.u64()?,
        max_output_units: cursor.u64()?,
        cancel_at_fuel: match cursor.u32()? {
            1 => None,
            2 => Some(cursor.u64()?),
            _ => return Err(PackageError::Malformed),
        },
    };

    let global_count = decoded_count(&mut cursor, EXEC_PACKAGE_MAX_GLOBALS, MINIMUM_GLOBAL_BYTES)?;
    let mut globals = Vec::with_capacity(global_count);
    let mut global_ids = std::collections::BTreeSet::new();
    for _ in 0..global_count {
        let entity_id = EntityId::from_bytes(cursor.fixed_32()?);
        if !global_ids.insert(entity_id) {
            return Err(PackageError::HydrationRefused);
        }
        globals.push(GlobalValueDefinition {
            entity_id,
            value_type: decode_type_expr(&mut cursor, 1)?,
            initializer: EntityId::from_bytes(cursor.fixed_32()?),
            visibility: decoded_visibility(cursor.u32()?)?,
        });
    }

    let contract_count = decoded_count(
        &mut cursor,
        EXEC_PACKAGE_MAX_CONTRACTS,
        MINIMUM_CONTRACT_BYTES,
    )?;
    let mut contracts = Vec::with_capacity(contract_count);
    let mut contract_ids = std::collections::BTreeSet::new();
    for _ in 0..contract_count {
        let entity_id = EntityId::from_bytes(cursor.fixed_32()?);
        if !contract_ids.insert(entity_id) {
            return Err(PackageError::HydrationRefused);
        }
        let target = EntityId::from_bytes(cursor.fixed_32()?);
        let contract_kind = decoded_contract_kind(cursor.u32()?)?;
        let predicate = EntityId::from_bytes(cursor.fixed_32()?);
        let binding_count = decoded_count(&mut cursor, EXEC_PACKAGE_MAX_DEPENDENCY_BYTES / 8, 8)?;
        let mut bindings = Vec::with_capacity(binding_count);
        for _ in 0..binding_count {
            bindings.push(ContractBinding {
                predicate_parameter: cursor.u32()?,
                source: decode_contract_source(&mut cursor)?,
            });
        }
        let resource_limits = match cursor.u32()? {
            1 => None,
            2 => Some(ResourceLimits {
                fuel: cursor.u64()?,
                memory_bytes: cursor.u64()?,
                output_bytes: cursor.u64()?,
                effect_count: cursor.u64()?,
                call_depth: cursor.u64()?,
                wall_timeout_millis: cursor.u64()?,
            }),
            _ => return Err(PackageError::Malformed),
        };
        contracts.push(ContractDefinition {
            entity_id,
            target,
            contract_kind,
            predicate,
            bindings,
            resource_limits,
        });
    }
    if cursor.remaining() != 0 {
        return Err(PackageError::Malformed);
    }
    Ok(DecodedDependencySection {
        entry,
        schema_epoch,
        state_root,
        profile,
        admitted_limits,
        gate_operation_count,
        gate_bridge_uses,
        gate_closure_fingerprints,
        globals,
        contracts,
    })
}

/// Computes every section digest plus the package identity for one package.
///
/// The package digest is SHA-256 over:
/// `SLEYPKG1 || u32(1) || profile_digest || host_abi_version || vm_version ||
/// image_digest || constants_digest || layouts_digest || imports_digest ||
/// dependency_digest || entry || epoch || root`.
///
/// Preserved for v1 history; successor packages use [`package_digests_v2`].
///
/// # Errors
///
/// Returns `Oversized` when the image, a section, or the complete envelope
/// exceeds its ceiling, or `Malformed` when a constant has no canonical
/// encoding.
pub fn package_digests(package: &ExecutionPackage) -> Result<PackageDigests, PackageError> {
    if package.image_bytes.len() > IMAGE_MAX_BYTES {
        return Err(PackageError::Oversized);
    }
    let image_digest = image_digest(&package.image_bytes);
    let constants_bytes = encode_constants_section(&package.constants)?;
    let constants_digest = section_digest(&constants_bytes);
    let layouts_bytes = encode_layouts_section(&package.type_definitions)?;
    let layouts_digest = section_digest(&layouts_bytes);
    let imports_bytes = encode_imports_section(&package.imports)?;
    let imports_digest = section_digest(&imports_bytes);
    let dependency_bytes = encode_dependency_section(package)?;
    let dependency_digest = section_digest(&dependency_bytes);
    let envelope_len = package
        .image_bytes
        .len()
        .saturating_add(constants_bytes.len())
        .saturating_add(layouts_bytes.len())
        .saturating_add(imports_bytes.len())
        .saturating_add(dependency_bytes.len());
    if envelope_len > EXEC_PACKAGE_MAX_BYTES {
        return Err(PackageError::Oversized);
    }
    let mut preimage = Vec::new();
    preimage.extend_from_slice(EXEC_PACKAGE_MAGIC);
    push_u32(&mut preimage, EXEC_PACKAGE_VERSION);
    preimage.extend_from_slice(&BOOTSTRAP_PROFILE_1_DIGEST);
    push_u32(&mut preimage, crate::host_abi::HOST_ABI_VERSION);
    for part in package.profile.vm_version {
        push_u32(&mut preimage, part);
    }
    preimage.extend_from_slice(&image_digest);
    preimage.extend_from_slice(&constants_digest);
    preimage.extend_from_slice(&layouts_digest);
    preimage.extend_from_slice(&imports_digest);
    preimage.extend_from_slice(&dependency_digest);
    preimage.extend_from_slice(package.entry.as_bytes());
    preimage.extend_from_slice(package.schema_epoch.as_bytes());
    preimage.extend_from_slice(package.state_root.as_bytes());
    let package_digest = section_digest(&preimage);
    Ok(PackageDigests {
        image_digest,
        constants_digest,
        layouts_digest,
        imports_digest,
        dependency_digest,
        package_digest,
    })
}

fn package_header_v2(package: &ExecutionPackage, digests: &PackageDigests) -> Vec<u8> {
    let mut header = Vec::with_capacity(EXEC_PACKAGE_V2_ENVELOPE_HEADER_BYTES);
    header.extend_from_slice(EXEC_PACKAGE_MAGIC);
    push_u32(&mut header, EXEC_PACKAGE_V2_VERSION);
    header.extend_from_slice(&BOOTSTRAP_PROFILE_2_DIGEST);
    push_u32(&mut header, crate::host_abi::HOST_ABI_V2_VERSION);
    for part in package.profile.vm_version {
        push_u32(&mut header, part);
    }
    header.extend_from_slice(&digests.image_digest);
    header.extend_from_slice(&digests.constants_digest);
    header.extend_from_slice(&digests.layouts_digest);
    header.extend_from_slice(&digests.imports_digest);
    header.extend_from_slice(&digests.dependency_digest);
    header.extend_from_slice(package.entry.as_bytes());
    header.extend_from_slice(package.schema_epoch.as_bytes());
    header.extend_from_slice(package.state_root.as_bytes());
    debug_assert_eq!(header.len(), EXEC_PACKAGE_V2_ENVELOPE_HEADER_BYTES);
    header
}

/// Successor package digests (RW-075 correction, AR-02).
///
/// Same envelope layout/bounds as v1; preimage uses `u32(2)`,
/// `BOOTSTRAP_PROFILE_2_DIGEST`, and `HOST_ABI_V2_VERSION`. V1 packages
/// keep their digests; v2 packages (including any `RHW1` import) bind the
/// successor profile/ABI. No other broadening.
///
/// # Errors
///
/// Same ceilings as v1.
pub fn package_digests_v2(package: &ExecutionPackage) -> Result<PackageDigests, PackageError> {
    if package.image_bytes.len() > IMAGE_MAX_BYTES {
        return Err(PackageError::Oversized);
    }
    let image_digest = image_digest(&package.image_bytes);
    let constants_bytes = encode_constants_section(&package.constants)?;
    let constants_digest = section_digest(&constants_bytes);
    let layouts_bytes = encode_layouts_section(&package.type_definitions)?;
    let layouts_digest = section_digest(&layouts_bytes);
    let imports_bytes = encode_imports_section(&package.imports)?;
    let imports_digest = section_digest(&imports_bytes);
    let dependency_bytes = encode_dependency_section(package)?;
    let dependency_digest = section_digest(&dependency_bytes);
    let envelope_len = package
        .image_bytes
        .len()
        .saturating_add(constants_bytes.len())
        .saturating_add(layouts_bytes.len())
        .saturating_add(imports_bytes.len())
        .saturating_add(dependency_bytes.len());
    if envelope_len > EXEC_PACKAGE_MAX_BYTES {
        return Err(PackageError::Oversized);
    }
    let mut digests = PackageDigests {
        image_digest,
        constants_digest,
        layouts_digest,
        imports_digest,
        dependency_digest,
        package_digest: [0; 32],
    };
    digests.package_digest = section_digest(&package_header_v2(package, &digests));
    Ok(digests)
}

/// Encodes the canonical v2 package byte envelope.
///
/// The fixed header is byte-for-byte the existing package-digest preimage.
/// It is followed by five `u64` big-endian length-prefixed sections in this
/// order: image, constants, layouts, imports, dependency. Section payload
/// bytes retain their existing canonical encodings and limits.
///
/// # Errors
///
/// Returns the same structural and size failures as the section encoders and
/// [`package_digests_v2`].
pub fn encode_package_envelope_v2(package: &ExecutionPackage) -> Result<Vec<u8>, PackageError> {
    let constants = encode_constants_section(&package.constants)?;
    let layouts = encode_layouts_section(&package.type_definitions)?;
    let imports = encode_imports_section(&package.imports)?;
    let dependency = encode_dependency_section(package)?;
    let sections: [&[u8]; 5] = [
        &package.image_bytes,
        &constants,
        &layouts,
        &imports,
        &dependency,
    ];
    let payload_len = sections.iter().try_fold(0_usize, |total, section| {
        total
            .checked_add(section.len())
            .ok_or(PackageError::Oversized)
    })?;
    if payload_len > EXEC_PACKAGE_MAX_BYTES {
        return Err(PackageError::Oversized);
    }
    let digests = package_digests_v2(package)?;
    let capacity = EXEC_PACKAGE_V2_ENVELOPE_HEADER_BYTES
        .checked_add(EXEC_PACKAGE_V2_ENVELOPE_LENGTH_BYTES)
        .and_then(|overhead| overhead.checked_add(payload_len))
        .ok_or(PackageError::Oversized)?;
    let mut output = package_header_v2(package, &digests);
    output.reserve(EXEC_PACKAGE_V2_ENVELOPE_LENGTH_BYTES + payload_len);
    for section in sections {
        push_u64(
            &mut output,
            u64::try_from(section.len()).map_err(|_| PackageError::Oversized)?,
        );
        output.extend_from_slice(section);
    }
    debug_assert_eq!(output.len(), capacity);
    Ok(output)
}

struct PackageEnvelopeCursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> PackageEnvelopeCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], PackageError> {
        let end = self
            .position
            .checked_add(len)
            .ok_or(PackageError::Oversized)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(PackageError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, PackageError> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| PackageError::Truncated)?;
        Ok(u32::from_be_bytes(bytes))
    }

    fn u16(&mut self) -> Result<u16, PackageError> {
        let bytes: [u8; 2] = self
            .take(2)?
            .try_into()
            .map_err(|_| PackageError::Truncated)?;
        Ok(u16::from_be_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, PackageError> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .map_err(|_| PackageError::Truncated)?;
        Ok(u64::from_be_bytes(bytes))
    }

    fn fixed_32(&mut self) -> Result<[u8; 32], PackageError> {
        self.take(32)?
            .try_into()
            .map_err(|_| PackageError::Truncated)
    }

    fn section(&mut self, ceiling: usize) -> Result<&'a [u8], PackageError> {
        let len = usize::try_from(self.u64()?).map_err(|_| PackageError::Oversized)?;
        if len > ceiling {
            return Err(PackageError::Oversized);
        }
        self.take(len)
    }
}

/// Strictly decodes and authenticates the canonical v2 package envelope.
///
/// This validates only framing, fixed profile/ABI/VM bindings, section
/// ceilings, exact section digests, and absence of trailing bytes. It returns
/// raw canonical section bytes; it performs no type, reference, closure, or
/// compiler judgment and does not mint admission evidence.
///
/// # Errors
///
/// Returns the reserved `PACKAGE_*` framing failures with this precedence:
/// total ceiling, truncation, magic, version, fixed binding, section ceiling,
/// trailing data, then section digest.
#[allow(clippy::too_many_lines)]
pub fn decode_package_envelope_v2(bytes: &[u8]) -> Result<DecodedPackageEnvelopeV2, PackageError> {
    let serialized_ceiling = EXEC_PACKAGE_MAX_BYTES
        .checked_add(EXEC_PACKAGE_V2_ENVELOPE_HEADER_BYTES)
        .and_then(|value| value.checked_add(EXEC_PACKAGE_V2_ENVELOPE_LENGTH_BYTES))
        .ok_or(PackageError::Oversized)?;
    if bytes.len() > serialized_ceiling {
        return Err(PackageError::Oversized);
    }
    let mut cursor = PackageEnvelopeCursor::new(bytes);
    if cursor.take(EXEC_PACKAGE_MAGIC.len())? != EXEC_PACKAGE_MAGIC {
        return Err(PackageError::UnknownMagic);
    }
    if cursor.u32()? != EXEC_PACKAGE_V2_VERSION {
        return Err(PackageError::UnsupportedVersion);
    }
    if cursor.fixed_32()? != BOOTSTRAP_PROFILE_2_DIGEST
        || cursor.u32()? != crate::host_abi::HOST_ABI_V2_VERSION
    {
        return Err(PackageError::BindingMismatch);
    }
    let vm_version = [cursor.u32()?, cursor.u32()?, cursor.u32()?];
    if vm_version != CacheProfile::EXTENDED_V1.vm_version {
        return Err(PackageError::BindingMismatch);
    }
    let expected_image_digest = cursor.fixed_32()?;
    let expected_constants_digest = cursor.fixed_32()?;
    let expected_layouts_digest = cursor.fixed_32()?;
    let expected_imports_digest = cursor.fixed_32()?;
    let expected_dependency_digest = cursor.fixed_32()?;
    let entry = EntityId::from_bytes(cursor.fixed_32()?);
    let schema_epoch = SchemaEpochId::from_bytes(cursor.fixed_32()?);
    let state_root = StateRoot::from_bytes(cursor.fixed_32()?);
    debug_assert_eq!(cursor.position, EXEC_PACKAGE_V2_ENVELOPE_HEADER_BYTES);

    let image = cursor.section(IMAGE_MAX_BYTES)?;
    let constants = cursor.section(EXEC_PACKAGE_MAX_CONSTANTS_BYTES)?;
    let layouts = cursor.section(EXEC_PACKAGE_MAX_LAYOUTS_BYTES)?;
    let imports = cursor.section(EXEC_PACKAGE_MAX_IMPORTS_BYTES)?;
    let dependency = cursor.section(EXEC_PACKAGE_MAX_DEPENDENCY_BYTES)?;
    let payload_len = [image, constants, layouts, imports, dependency]
        .iter()
        .try_fold(0_usize, |total, section| {
            total
                .checked_add(section.len())
                .ok_or(PackageError::Oversized)
        })?;
    if payload_len > EXEC_PACKAGE_MAX_BYTES {
        return Err(PackageError::Oversized);
    }
    if cursor.position != bytes.len() {
        return Err(PackageError::TrailingData);
    }

    let image_digest = image_digest(image);
    let constants_digest = section_digest(constants);
    let layouts_digest = section_digest(layouts);
    let imports_digest = section_digest(imports);
    let dependency_digest = section_digest(dependency);
    if image_digest != expected_image_digest
        || constants_digest != expected_constants_digest
        || layouts_digest != expected_layouts_digest
        || imports_digest != expected_imports_digest
        || dependency_digest != expected_dependency_digest
    {
        return Err(PackageError::SectionDigestMismatch);
    }
    let package_digest = section_digest(&bytes[..EXEC_PACKAGE_V2_ENVELOPE_HEADER_BYTES]);
    Ok(DecodedPackageEnvelopeV2 {
        image_bytes: image.to_vec(),
        constants_bytes: constants.to_vec(),
        layouts_bytes: layouts.to_vec(),
        imports_bytes: imports.to_vec(),
        dependency_bytes: dependency.to_vec(),
        entry,
        schema_epoch,
        state_root,
        digests: PackageDigests {
            image_digest,
            constants_digest,
            layouts_digest,
            imports_digest,
            dependency_digest,
            package_digest,
        },
    })
}

/// Strictly decodes a v2 envelope and reconstructs its execution package.
///
/// This performs byte-level authentication, bounded structural section
/// decoding, repeated-header binding checks, and canonical re-encoding. It
/// does not validate Sley semantics or create admission evidence.
///
/// # Errors
///
/// Returns the first structural package failure. A mismatch between repeated
/// header/dependency bindings or reconstructed digests is `BindingMismatch`;
/// a section accepted by a decoder but not reproduced byte-for-byte by its
/// canonical encoder is `Malformed`.
pub fn hydrate_package_envelope_v2(
    bytes: &[u8],
) -> Result<HydratedPackageEnvelopeV2, PackageError> {
    let decoded = decode_package_envelope_v2(bytes)?;
    let constants = decode_constants_section(&decoded.constants_bytes)?;
    let type_definitions = decode_layouts_section(&decoded.layouts_bytes)?;
    let imports = decode_imports_section(&decoded.imports_bytes)?;
    let dependency = decode_dependency_section(&decoded.dependency_bytes)?;
    if dependency.entry != decoded.entry
        || dependency.schema_epoch != decoded.schema_epoch
        || dependency.state_root != decoded.state_root
        || dependency.profile != CacheProfile::EXTENDED_V1
    {
        return Err(PackageError::BindingMismatch);
    }
    let package = ExecutionPackage {
        image_bytes: decoded.image_bytes.clone(),
        constants,
        type_definitions,
        imports,
        globals: dependency.globals,
        contracts: dependency.contracts,
        entry: dependency.entry,
        schema_epoch: dependency.schema_epoch,
        state_root: dependency.state_root,
        profile: dependency.profile,
        admitted_limits: dependency.admitted_limits,
        gate_operation_count: dependency.gate_operation_count,
        gate_bridge_uses: dependency.gate_bridge_uses,
        gate_closure_fingerprints: dependency.gate_closure_fingerprints,
    };
    if encode_constants_section(&package.constants)? != decoded.constants_bytes
        || encode_layouts_section(&package.type_definitions)? != decoded.layouts_bytes
        || encode_imports_section(&package.imports)? != decoded.imports_bytes
        || encode_dependency_section(&package)? != decoded.dependency_bytes
    {
        return Err(PackageError::Malformed);
    }
    let digests = package_digests_v2(&package)?;
    if digests != decoded.digests {
        return Err(PackageError::BindingMismatch);
    }
    Ok(HydratedPackageEnvelopeV2 { package, digests })
}

/// Builds the admission receipt for one exact package digest.
///
/// Authority discipline: this function MUST only be called by the staged
/// admission authority after it has judged the closure's full SSMC graphs
/// with `judge_bootstrap_profile` and holds the resulting
/// [`BootstrapProfileReport`]. The receipt alone never approves anything:
/// [`approve_package`] additionally requires that report and verifies its
/// entry/import consistency against the package, so a receipt minted
/// without (or against) a gate judgment cannot produce an approval. The
/// host never mints a receipt at execution time; it only compares one.
///
/// Preserved for v1 history; successor packages use [`admit_package_v2`].
#[must_use]
pub fn admit_package(package_digest: [u8; 32]) -> AdmissionReceipt {
    AdmissionReceipt {
        package_digest,
        profile_digest: BOOTSTRAP_PROFILE_1_DIGEST,
        host_abi_version: crate::host_abi::HOST_ABI_VERSION,
    }
}

/// Successor admission receipt (RW-075 correction).
///
/// Same authority discipline as v1, binding the successor profile digest
/// and host ABI version 2. V1 receipts keep their digests; v2 receipts
/// (including any `RHW1` closure) bind v2. Exclusive minter: the staged
/// authority (`crate::admission_authority::admit_v2_package`); this
/// constructor is `pub(crate)` so reviewed integration paths cannot mint
/// outside it (direct calls exist only in crate unit tests as negatives).
#[must_use]
pub(crate) fn admit_package_v2(package_digest: [u8; 32]) -> AdmissionReceipt {
    AdmissionReceipt {
        package_digest,
        profile_digest: BOOTSTRAP_PROFILE_2_DIGEST,
        host_abi_version: crate::host_abi::HOST_ABI_V2_VERSION,
    }
}

/// Builds the complete [`ApprovedExecutionPackage`] binding for one package
/// plus the admission authority's gate report for its closure.
///
/// Besides every digest/receipt check, this verifies the gate evidence is
/// *for this package*: the report's entry-first function must equal the
/// package entry, and the report's admitted import set must equal the
/// package import-identity set. A receipt minted without a gate judgment —
/// or a gate report for another closure (including any out-of-bootstrap
/// program the gate refuses, for which no report exists) — cannot produce
/// an approval. The host never runs the gate here; it checks the
/// authority's evidence for consistency with the package.
///
/// # Errors
///
/// Returns `ReceiptMismatch` when the receipt names another package digest,
/// or `BindingMismatch` for any other binding failure (profile, ABI,
/// cache, gate-report consistency).
pub fn approve_package(
    package: &ExecutionPackage,
    digests: &PackageDigests,
    receipt: AdmissionReceipt,
    gate: &BootstrapProfileReport,
) -> Result<ApprovedExecutionPackage, PackageError> {
    if receipt.package_digest != digests.package_digest {
        return Err(PackageError::ReceiptMismatch);
    }
    if gate.functions().first() != Some(&package.entry) {
        return Err(PackageError::BindingMismatch);
    }
    let mut gate_imports = gate.imports().to_vec();
    gate_imports.sort();
    let mut package_imports: Vec<EntityId> =
        package.imports.iter().map(|row| row.entity_id).collect();
    package_imports.sort();
    if gate_imports != package_imports {
        return Err(PackageError::BindingMismatch);
    }
    // The report's quantitative claims are digested into the package, so a
    // report for another closure — even one sharing the entry and import
    // set — cannot approve this package unless its operation and bridge
    // counts also match. The fingerprints bind the exact judged closure
    // bytes: equal-count replays with different operations refuse below.
    if gate.operation_count() != package.gate_operation_count
        || gate.bridge_uses() != package.gate_bridge_uses
    {
        return Err(PackageError::BindingMismatch);
    }
    if gate.closure_fingerprints() != package.gate_closure_fingerprints.as_slice() {
        return Err(PackageError::BindingMismatch);
    }
    // Executable-content binding: the sealed report commits to the exact
    // image bytes presented at admission; approval re-derives the digest
    // from the package image and refuses on mismatch. This binds gate
    // evidence to the complete lowered structure (operands, immediates,
    // CFG, and termination included — anything rewired changes bytes and
    // therefore the digest), with no semantic recomputation: pure
    // byte-hash equality. Graphs-to-image correspondence (builder
    // faithfulness) is the admission authority's reference re-lowering
    // comparison (RW-080 contract), not host work.
    if gate.admitted_image_digest() != &crate::host_abi::image_digest(&package.image_bytes) {
        return Err(PackageError::BindingMismatch);
    }
    if receipt.profile_digest != BOOTSTRAP_PROFILE_1_DIGEST {
        return Err(PackageError::BindingMismatch);
    }
    // Version isolation: a v1 approval requires a v1-judged report. A
    // v2-judged report (the only kind that can name the successor
    // raw-hash row) can never back a v1 package, so v1 authority stays
    // isolated from successor imports even though the report type is
    // shared.
    if gate.profile_version() != BootstrapProfileVersion::V1 {
        return Err(PackageError::BindingMismatch);
    }
    if receipt.host_abi_version != crate::host_abi::HOST_ABI_VERSION {
        return Err(PackageError::BindingMismatch);
    }
    if package.profile != CacheProfile::EXTENDED_V1 {
        return Err(PackageError::BindingMismatch);
    }
    let cache_key = crate::derive_cache_key(
        package.schema_epoch,
        package.state_root,
        package.entry,
        package.profile,
    )
    .map_err(|_| PackageError::BindingMismatch)?;
    Ok(ApprovedExecutionPackage {
        package_digest: digests.package_digest,
        image_digest: digests.image_digest,
        constants_digest: digests.constants_digest,
        layouts_digest: digests.layouts_digest,
        imports_digest: digests.imports_digest,
        dependency_digest: digests.dependency_digest,
        cache_key,
        imports: package.imports.clone(),
        entry: package.entry,
        schema_epoch: package.schema_epoch,
        state_root: package.state_root,
        profile: package.profile,
        profile_digest: BOOTSTRAP_PROFILE_1_DIGEST,
        vm_version: package.profile.vm_version,
        host_abi_version: crate::host_abi::HOST_ABI_VERSION,
        admitted_limits: package.admitted_limits,
        receipt,
    })
}

/// Successor package approval (RW-075 correction).
///
/// Identical binding discipline to v1, binding the successor profile
/// digest and host ABI version 2. The gate-report consistency checks
/// (entry-first, import-set equality, operation/bridge counts, closure
/// fingerprints, admitted image digest) are unchanged: a v2 report for a
/// `RHW1` closure approves only its exact package.
///
/// # Errors
///
/// Same vocabulary as v1.
pub fn approve_package_v2(
    package: &ExecutionPackage,
    digests: &PackageDigests,
    receipt: AdmissionReceipt,
    gate: &BootstrapProfileReport,
) -> Result<ApprovedExecutionPackage, PackageError> {
    if receipt.package_digest != digests.package_digest {
        return Err(PackageError::ReceiptMismatch);
    }
    if gate.functions().first() != Some(&package.entry) {
        return Err(PackageError::BindingMismatch);
    }
    let mut gate_imports = gate.imports().to_vec();
    gate_imports.sort();
    let mut package_imports: Vec<EntityId> =
        package.imports.iter().map(|row| row.entity_id).collect();
    package_imports.sort();
    if gate_imports != package_imports {
        return Err(PackageError::BindingMismatch);
    }
    if gate.operation_count() != package.gate_operation_count
        || gate.bridge_uses() != package.gate_bridge_uses
    {
        return Err(PackageError::BindingMismatch);
    }
    if gate.closure_fingerprints() != package.gate_closure_fingerprints.as_slice() {
        return Err(PackageError::BindingMismatch);
    }
    if gate.admitted_image_digest() != &crate::host_abi::image_digest(&package.image_bytes) {
        return Err(PackageError::BindingMismatch);
    }
    if receipt.profile_digest != BOOTSTRAP_PROFILE_2_DIGEST {
        return Err(PackageError::BindingMismatch);
    }
    // Mirror pin: a v2 approval requires a v2-judged report, so a
    // v1-judged report can never back a successor package.
    if gate.profile_version() != BootstrapProfileVersion::V2 {
        return Err(PackageError::BindingMismatch);
    }
    if receipt.host_abi_version != crate::host_abi::HOST_ABI_V2_VERSION {
        return Err(PackageError::BindingMismatch);
    }
    if package.profile != CacheProfile::EXTENDED_V1 {
        return Err(PackageError::BindingMismatch);
    }
    let cache_key = crate::derive_cache_key(
        package.schema_epoch,
        package.state_root,
        package.entry,
        package.profile,
    )
    .map_err(|_| PackageError::BindingMismatch)?;
    Ok(ApprovedExecutionPackage {
        package_digest: digests.package_digest,
        image_digest: digests.image_digest,
        constants_digest: digests.constants_digest,
        layouts_digest: digests.layouts_digest,
        imports_digest: digests.imports_digest,
        dependency_digest: digests.dependency_digest,
        cache_key,
        imports: package.imports.clone(),
        entry: package.entry,
        schema_epoch: package.schema_epoch,
        state_root: package.state_root,
        profile: package.profile,
        profile_digest: BOOTSTRAP_PROFILE_2_DIGEST,
        vm_version: package.profile.vm_version,
        host_abi_version: crate::host_abi::HOST_ABI_V2_VERSION,
        admitted_limits: package.admitted_limits,
        receipt,
    })
}

/// Hydrates the runtime type/layout closure structurally (no semantic
/// judgment).
///
/// Verifies definition-count bounds and duplicate identities only (the two
/// structural memory-safety checks), then holds the inventory for direct
/// structural lookups during execution (field-count agreement, member-ID
/// presence). Never validates shapes, resolves references semantically,
/// discovers cycles, determines map-key/hashability rules, reconstructs
/// schemas, typechecks, or infers/repairs layouts — those are
/// compiler-owned and bound via the receipt.
///
/// # Errors
///
/// `HydrationRefused` when the inventory exceeds
/// [`EXEC_PACKAGE_MAX_DEFINITIONS`] or repeats an identity.
pub fn hydrate_layouts(definitions: Vec<TypeDefinition>) -> Result<TypeEnvironment, PackageError> {
    if definitions.len() > EXEC_PACKAGE_MAX_DEFINITIONS {
        return Err(PackageError::HydrationRefused);
    }
    TypeEnvironment::hydrate_verified_definitions(definitions)
        .map_err(|_| PackageError::HydrationRefused)
}

/// Verifies a supplied package against its approved binding (all bindings,
/// before execution).
///
/// Checks, in order: receipt vs package digest; image digest vs bytes;
/// constants/layouts/imports/dependency digests vs recomputed sections;
/// entry/epoch/root/profile/ABI/VM/limits vs package; exact import rows
/// (`==`, full schemas — not IDs); cache key re-derivation. Any mismatch
/// refuses with the structural vocabulary; nothing is repaired or inferred.
///
/// # Errors
///
/// The exact structural refusal for the first mismatched binding.
pub fn verify_package_binding(
    package: &ExecutionPackage,
    digests: &PackageDigests,
    expected: &ApprovedExecutionPackage,
) -> Result<(), PackageError> {
    if digests.package_digest != expected.package_digest
        || digests.package_digest != expected.receipt.package_digest
    {
        return Err(PackageError::ReceiptMismatch);
    }
    if digests.image_digest != expected.image_digest {
        return Err(PackageError::BindingMismatch);
    }
    if image_digest(&package.image_bytes) != expected.image_digest {
        return Err(PackageError::BindingMismatch);
    }
    if digests.constants_digest != expected.constants_digest
        || digests.layouts_digest != expected.layouts_digest
        || digests.imports_digest != expected.imports_digest
        || digests.dependency_digest != expected.dependency_digest
    {
        return Err(PackageError::BindingMismatch);
    }
    let recomputed = package_digests(package)?;
    if recomputed != *digests {
        return Err(PackageError::BindingMismatch);
    }
    if package.entry != expected.entry
        || package.schema_epoch != expected.schema_epoch
        || package.state_root != expected.state_root
        || package.profile != expected.profile
        || package.admitted_limits != expected.admitted_limits
    {
        return Err(PackageError::BindingMismatch);
    }
    if expected.profile_digest != BOOTSTRAP_PROFILE_1_DIGEST
        || expected.host_abi_version != crate::host_abi::HOST_ABI_VERSION
        || expected.vm_version != package.profile.vm_version
    {
        return Err(PackageError::BindingMismatch);
    }
    if package.imports != expected.imports {
        return Err(PackageError::BindingMismatch);
    }
    let cache_key = crate::derive_cache_key(
        package.schema_epoch,
        package.state_root,
        package.entry,
        package.profile,
    )
    .map_err(|_| PackageError::BindingMismatch)?;
    if cache_key != expected.cache_key {
        return Err(PackageError::BindingMismatch);
    }
    Ok(())
}

/// Successor package verification (RW-075 correction).
///
/// Same checks as v1, recomputing with [`package_digests_v2`] and binding
/// the successor profile digest / host ABI version 2. V1 bindings keep
/// their function; v2 bindings (including `RHW1` rows) verify here.
///
/// # Errors
///
/// Same vocabulary as v1.
pub fn verify_package_binding_v2(
    package: &ExecutionPackage,
    digests: &PackageDigests,
    expected: &ApprovedExecutionPackage,
) -> Result<(), PackageError> {
    if digests.package_digest != expected.package_digest
        || digests.package_digest != expected.receipt.package_digest
    {
        return Err(PackageError::ReceiptMismatch);
    }
    if digests.image_digest != expected.image_digest {
        return Err(PackageError::BindingMismatch);
    }
    if image_digest(&package.image_bytes) != expected.image_digest {
        return Err(PackageError::BindingMismatch);
    }
    if digests.constants_digest != expected.constants_digest
        || digests.layouts_digest != expected.layouts_digest
        || digests.imports_digest != expected.imports_digest
        || digests.dependency_digest != expected.dependency_digest
    {
        return Err(PackageError::BindingMismatch);
    }
    let recomputed = package_digests_v2(package)?;
    if recomputed != *digests {
        return Err(PackageError::BindingMismatch);
    }
    if package.entry != expected.entry
        || package.schema_epoch != expected.schema_epoch
        || package.state_root != expected.state_root
        || package.profile != expected.profile
        || package.admitted_limits != expected.admitted_limits
    {
        return Err(PackageError::BindingMismatch);
    }
    if expected.profile_digest != BOOTSTRAP_PROFILE_2_DIGEST
        || expected.host_abi_version != crate::host_abi::HOST_ABI_V2_VERSION
        || expected.vm_version != package.profile.vm_version
    {
        return Err(PackageError::BindingMismatch);
    }
    if package.imports != expected.imports {
        return Err(PackageError::BindingMismatch);
    }
    let cache_key = crate::derive_cache_key(
        package.schema_epoch,
        package.state_root,
        package.entry,
        package.profile,
    )
    .map_err(|_| PackageError::BindingMismatch)?;
    if cache_key != expected.cache_key {
        return Err(PackageError::BindingMismatch);
    }
    Ok(())
}

/// Structural import-row manifest digest (exact rows).
///
/// Used by negative tests to prove that a correct image with a wrong import
/// row (same IDs, different schemas — or vice versa) refuses: the digest
/// covers full rows, so any row substitution changes it.
///
/// # Errors
///
/// Returns `Oversized` when the section exceeds its ceiling.
pub fn imports_manifest_digest(imports: &[AdapterImport]) -> Result<[u8; 32], PackageError> {
    Ok(section_digest(&encode_imports_section(imports)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sley_ssmc::{BuiltinFailureKind, ConstData, ConstValue, IntegerWidth};
    use sley_ssmc::{FunctionType, MemberId, NamedType};
    use std::collections::BTreeMap;

    // The strict v2 envelope decoder constructs SectionDigestMismatch for a
    // header-bound section mismatch. This smaller pin keeps the stable symbol
    // explicit beside the broader behavioral refusal test below.
    #[test]
    fn section_digest_mismatch_variant_is_stable() {
        assert_eq!(
            PackageError::SectionDigestMismatch.as_str(),
            "PACKAGE_SECTION_DIGEST_MISMATCH"
        );
    }

    fn test_digest() -> [u8; 32] {
        [0x08; 32]
    }

    fn envelope_test_package() -> ExecutionPackage {
        ExecutionPackage {
            image_bytes: b"SLEYBC02\0".to_vec(),
            constants: Vec::new(),
            type_definitions: Vec::new(),
            imports: Vec::new(),
            globals: Vec::new(),
            contracts: Vec::new(),
            entry: EntityId::from_bytes([1; 32]),
            schema_epoch: SchemaEpochId::from_bytes([8; 32]),
            state_root: StateRoot::from_bytes([9; 32]),
            profile: CacheProfile::EXTENDED_V1,
            admitted_limits: crate::ExecutionLimits {
                max_instructions: 1_000,
                max_fuel: 2_000,
                max_value_units: 3_000,
                max_output_units: 4_000,
                cancel_at_fuel: None,
            },
            gate_operation_count: 0,
            gate_bridge_uses: 0,
            gate_closure_fingerprints: Vec::new(),
        }
    }

    fn populated_envelope_test_package() -> ExecutionPackage {
        let mut package = envelope_test_package();
        let constant = ConstantDefinition {
            entity_id: EntityId::from_bytes([0xa1; 32]),
            value: ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(true),
            },
        };
        package.constants.push(constant.clone());
        package.type_definitions.push(TypeDefinition {
            entity_id: EntityId::from_bytes([0xb1; 32]),
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(vec![RecordField {
                member_id: MemberId::from_bytes([0xb2; 32]),
                value_type: TypeExpr::Bool,
                visibility: Visibility::Private,
            }]),
            invariants: Vec::new(),
            visibility: Visibility::Package,
        });
        package.imports.push(AdapterImport {
            entity_id: EntityId::from_bytes([0xc1; 32]),
            adapter_id: [0xc2; 32],
            abi_version: 2,
            request_type: TypeExpr::Bytes,
            response_type: TypeExpr::Bytes,
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
            effects: vec![EntityId::from_bytes([0xc3; 32])],
        });
        let global = GlobalValueDefinition {
            entity_id: EntityId::from_bytes([0xd1; 32]),
            value_type: TypeExpr::Option(Box::new(TypeExpr::Bool)),
            initializer: constant.entity_id,
            visibility: Visibility::Workspace,
        };
        package.globals.push(global.clone());
        package.contracts.push(ContractDefinition {
            entity_id: EntityId::from_bytes([0xe1; 32]),
            target: package.entry,
            contract_kind: ContractKind::Postcondition,
            predicate: EntityId::from_bytes([0xe2; 32]),
            bindings: vec![
                ContractBinding {
                    predicate_parameter: 0,
                    source: ContractSource::Result,
                },
                ContractBinding {
                    predicate_parameter: 1,
                    source: ContractSource::Global(global.entity_id),
                },
            ],
            resource_limits: Some(ResourceLimits {
                fuel: 10,
                memory_bytes: 20,
                output_bytes: 30,
                effect_count: 40,
                call_depth: 50,
                wall_timeout_millis: 60,
            }),
        });
        package.admitted_limits.cancel_at_fuel = Some(1_500);
        package.gate_operation_count = 7;
        package.gate_bridge_uses = 1;
        package.gate_closure_fingerprints = vec![
            SemanticFingerprint::from_bytes([0xf1; 32]),
            SemanticFingerprint::from_bytes([0xf2; 32]),
        ];
        package
    }

    #[test]
    fn v2_envelope_round_trips_exact_raw_sections_and_header_digest() {
        use core::fmt::Write as _;

        let package = envelope_test_package();
        let bytes = encode_package_envelope_v2(&package).expect("envelope encodes");
        let decoded = decode_package_envelope_v2(&bytes).expect("envelope decodes");
        let expected = package_digests_v2(&package).expect("package digests");
        assert_eq!(&bytes[..8], EXEC_PACKAGE_MAGIC);
        assert_eq!(decoded.image_bytes, package.image_bytes);
        assert_eq!(
            decoded.constants_bytes,
            encode_constants_section(&package.constants).unwrap()
        );
        assert_eq!(
            decoded.layouts_bytes,
            encode_layouts_section(&package.type_definitions).unwrap()
        );
        assert_eq!(
            decoded.imports_bytes,
            encode_imports_section(&package.imports).unwrap()
        );
        assert_eq!(
            decoded.dependency_bytes,
            encode_dependency_section(&package).unwrap()
        );
        assert_eq!(decoded.entry, package.entry);
        assert_eq!(decoded.schema_epoch, package.schema_epoch);
        assert_eq!(decoded.state_root, package.state_root);
        assert_eq!(decoded.digests, expected);
        let mut encoded_hex = String::with_capacity(bytes.len() * 2);
        for byte in &bytes {
            write!(&mut encoded_hex, "{byte:02x}").expect("writing to String cannot fail");
        }
        assert_eq!(
            encoded_hex,
            include_str!("../../../conformance/exec-package-envelope/v2/accepted.hex").trim(),
            "Rust emitter reproduces the independent candidate vector"
        );
        assert_eq!(
            encode_package_envelope_v2(&package).unwrap(),
            bytes,
            "envelope encoding is deterministic"
        );
    }

    #[test]
    fn v2_envelope_refusal_vocabulary_is_live() {
        let package = envelope_test_package();
        let bytes = encode_package_envelope_v2(&package).expect("envelope encodes");

        let mut wrong_magic = bytes.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            decode_package_envelope_v2(&wrong_magic),
            Err(PackageError::UnknownMagic)
        );

        let mut wrong_version = bytes.clone();
        wrong_version[8..12].copy_from_slice(&3_u32.to_be_bytes());
        assert_eq!(
            decode_package_envelope_v2(&wrong_version),
            Err(PackageError::UnsupportedVersion)
        );

        let mut wrong_profile = bytes.clone();
        wrong_profile[12] ^= 1;
        assert_eq!(
            decode_package_envelope_v2(&wrong_profile),
            Err(PackageError::BindingMismatch)
        );

        let mut wrong_digest = bytes.clone();
        wrong_digest[60] ^= 1;
        assert_eq!(
            decode_package_envelope_v2(&wrong_digest),
            Err(PackageError::SectionDigestMismatch)
        );

        let mut truncated = bytes.clone();
        truncated.pop();
        assert_eq!(
            decode_package_envelope_v2(&truncated),
            Err(PackageError::Truncated)
        );

        let mut trailing = bytes;
        trailing.push(0);
        assert_eq!(
            decode_package_envelope_v2(&trailing),
            Err(PackageError::TrailingData)
        );
    }

    #[test]
    fn constants_section_strictly_round_trips_and_refuses_bad_structure() {
        let constant = ConstantDefinition {
            entity_id: EntityId::from_bytes([0x44; 32]),
            value: ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(true),
            },
        };
        let encoded = encode_constants_section(std::slice::from_ref(&constant)).unwrap();
        assert_eq!(
            decode_constants_section(&encoded).unwrap(),
            vec![constant.clone()]
        );

        let mut truncated = encoded.clone();
        truncated.pop();
        assert_eq!(
            decode_constants_section(&truncated),
            Err(PackageError::Truncated)
        );

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            decode_constants_section(&trailing),
            Err(PackageError::Malformed)
        );

        let duplicate = encode_constants_section(&[constant.clone(), constant]).unwrap();
        assert_eq!(
            decode_constants_section(&duplicate),
            Err(PackageError::HydrationRefused)
        );

        let oversized_count = u64::try_from(EXEC_PACKAGE_MAX_CONSTANTS)
            .unwrap()
            .checked_add(1)
            .unwrap()
            .to_be_bytes();
        assert_eq!(
            decode_constants_section(&oversized_count),
            Err(PackageError::Oversized)
        );
    }

    #[test]
    fn layouts_section_round_trips_nested_forms_and_refuses_bad_rows() {
        let record = TypeDefinition {
            entity_id: EntityId::from_bytes([0x50; 32]),
            type_parameters: vec![TypeParameterDef { ordinal: 0 }],
            form: TypeDefForm::Record(vec![RecordField {
                member_id: MemberId::from_bytes([0x51; 32]),
                value_type: TypeExpr::Vector(Box::new(TypeExpr::TypeParameter(0))),
                visibility: Visibility::Package,
            }]),
            invariants: vec![EntityId::from_bytes([0x52; 32])],
            visibility: Visibility::Exported,
        };
        let variant = TypeDefinition {
            entity_id: EntityId::from_bytes([0x60; 32]),
            type_parameters: Vec::new(),
            form: TypeDefForm::Variant(vec![
                VariantCase {
                    member_id: MemberId::from_bytes([0x61; 32]),
                    payload_type: None,
                },
                VariantCase {
                    member_id: MemberId::from_bytes([0x62; 32]),
                    payload_type: Some(TypeExpr::Named(NamedType {
                        definition: record.entity_id,
                        arguments: vec![TypeExpr::Bool],
                    })),
                },
            ]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        };
        let definitions = vec![record.clone(), variant];
        let encoded = encode_layouts_section(&definitions).unwrap();
        assert_eq!(decode_layouts_section(&encoded).unwrap(), definitions);

        let duplicate = encode_layouts_section(&[record.clone(), record]).unwrap();
        assert_eq!(
            decode_layouts_section(&duplicate),
            Err(PackageError::HydrationRefused)
        );

        let mut bad_form = encode_layouts_section(std::slice::from_ref(&definitions[0])).unwrap();
        bad_form[60..64].copy_from_slice(&99_u32.to_be_bytes());
        assert_eq!(
            decode_layouts_section(&bad_form),
            Err(PackageError::Malformed)
        );
    }

    #[test]
    fn imports_section_round_trips_exact_rows_and_refuses_bad_rows() {
        let row = AdapterImport {
            entity_id: EntityId::from_bytes([0x70; 32]),
            adapter_id: [0x71; 32],
            abi_version: 3,
            request_type: TypeExpr::Tuple(vec![TypeExpr::Bool, TypeExpr::Bytes]),
            response_type: TypeExpr::Result {
                ok: Box::new(TypeExpr::Bytes),
                error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
            },
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
            effects: vec![EntityId::from_bytes([0x72; 32])],
        };
        let encoded = encode_imports_section(std::slice::from_ref(&row)).unwrap();
        assert_eq!(decode_imports_section(&encoded).unwrap(), vec![row.clone()]);

        let duplicate = encode_imports_section(&[row.clone(), row]).unwrap();
        assert_eq!(
            decode_imports_section(&duplicate),
            Err(PackageError::HydrationRefused)
        );

        let mut bad_request_type = encoded;
        bad_request_type[84..88].copy_from_slice(&99_u32.to_be_bytes());
        assert_eq!(
            decode_imports_section(&bad_request_type),
            Err(PackageError::Malformed)
        );
    }

    #[test]
    fn dependency_section_round_trips_complete_rows_and_refuses_duplicates() {
        let package = populated_envelope_test_package();
        let encoded = encode_dependency_section(&package).unwrap();
        let decoded = decode_dependency_section(&encoded).unwrap();
        assert_eq!(decoded.entry, package.entry);
        assert_eq!(decoded.schema_epoch, package.schema_epoch);
        assert_eq!(decoded.state_root, package.state_root);
        assert_eq!(decoded.profile, package.profile);
        assert_eq!(decoded.admitted_limits, package.admitted_limits);
        assert_eq!(decoded.gate_operation_count, package.gate_operation_count);
        assert_eq!(decoded.gate_bridge_uses, package.gate_bridge_uses);
        assert_eq!(
            decoded.gate_closure_fingerprints,
            package.gate_closure_fingerprints
        );
        assert_eq!(decoded.globals, package.globals);
        assert_eq!(decoded.contracts, package.contracts);

        let mut duplicate_global = package.clone();
        duplicate_global.globals.push(package.globals[0].clone());
        assert_eq!(
            decode_dependency_section(&encode_dependency_section(&duplicate_global).unwrap()),
            Err(PackageError::HydrationRefused)
        );

        let mut duplicate_contract = package.clone();
        duplicate_contract
            .contracts
            .push(package.contracts[0].clone());
        assert_eq!(
            decode_dependency_section(&encode_dependency_section(&duplicate_contract).unwrap()),
            Err(PackageError::HydrationRefused)
        );
    }

    #[test]
    fn v2_envelope_hydrates_complete_package_and_refuses_split_bindings() {
        const DEPENDENCY_DIGEST_OFFSET: usize = 8 + 4 + 32 + 4 + 12 + (4 * 32);

        let package = populated_envelope_test_package();
        let bytes = encode_package_envelope_v2(&package).unwrap();
        let hydrated = hydrate_package_envelope_v2(&bytes).unwrap();
        assert_eq!(hydrated.package, package);
        assert_eq!(hydrated.digests, package_digests_v2(&package).unwrap());

        let dependency_len = encode_dependency_section(&package).unwrap().len();
        let dependency_start = bytes.len() - dependency_len;
        let mut split_binding = bytes;
        split_binding[dependency_start] ^= 1;
        let replacement_digest = section_digest(&split_binding[dependency_start..]);
        split_binding[DEPENDENCY_DIGEST_OFFSET..DEPENDENCY_DIGEST_OFFSET + 32]
            .copy_from_slice(&replacement_digest);
        assert!(decode_package_envelope_v2(&split_binding).is_ok());
        assert_eq!(
            hydrate_package_envelope_v2(&split_binding),
            Err(PackageError::BindingMismatch)
        );
    }

    #[test]
    fn section_digests_are_deterministic_and_tamper_sensitive() {
        let first = section_digest(b"package section v1");
        assert_eq!(first, section_digest(b"package section v1"));
        assert_ne!(first, section_digest(b"package section v2"));
    }

    #[test]
    fn empty_package_sections_have_stable_digests() {
        let constants = encode_constants_section(&[]).expect("empty constants encode");
        let layouts = encode_layouts_section(&[]).expect("empty layouts encode");
        let imports = encode_imports_section(&[]).expect("empty imports encode");
        assert_eq!(constants, encode_constants_section(&[]).unwrap());
        assert_eq!(layouts, encode_layouts_section(&[]).unwrap());
        assert_eq!(imports, encode_imports_section(&[]).unwrap());
        let _ = test_digest();
    }

    #[test]
    fn imports_digest_covers_exact_rows_not_just_ids() {
        use crate::host_abi::{BRIDGE_ABI_VERSION, bridge_identity};
        let identity = EntityId::from_bytes(bridge_identity(*b"B2V1"));
        let row = AdapterImport {
            entity_id: identity,
            adapter_id: *identity.as_bytes(),
            abi_version: BRIDGE_ABI_VERSION,
            request_type: TypeExpr::Bytes,
            response_type: TypeExpr::Vector(Box::new(TypeExpr::UInt(IntegerWidth::from_bits(8)))),
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
            effects: Vec::new(),
        };
        let mut wrong_schema = row.clone();
        wrong_schema.response_type = TypeExpr::Bytes;
        assert_ne!(
            imports_manifest_digest(std::slice::from_ref(&row)).unwrap(),
            imports_manifest_digest(std::slice::from_ref(&wrong_schema)).unwrap(),
            "same ID with a different schema must change the manifest digest"
        );
    }

    #[test]
    fn hydration_refuses_duplicates_structurally() {
        use sley_ssmc::{TypeDefinition, Visibility};
        let definition = TypeDefinition {
            entity_id: EntityId::from_bytes([0x41; 32]),
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(Vec::new()),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        };
        hydrate_layouts(vec![definition.clone()]).expect("single definition hydrates");
        assert!(
            matches!(
                hydrate_layouts(vec![definition.clone(), definition]),
                Err(PackageError::HydrationRefused)
            ),
            "duplicate identities refuse without any semantic judgment"
        );
    }

    #[test]
    fn type_expression_codec_is_structural_only() {
        let mut first = Vec::new();
        encode_type_expr(
            &mut first,
            &TypeExpr::Vector(Box::new(TypeExpr::UInt(IntegerWidth::from_bits(8)))),
            1,
            EXEC_PACKAGE_MAX_LAYOUTS_BYTES,
        )
        .expect("structural encode");
        let mut second = Vec::new();
        encode_type_expr(
            &mut second,
            &TypeExpr::Bytes,
            1,
            EXEC_PACKAGE_MAX_LAYOUTS_BYTES,
        )
        .expect("structural encode");
        assert_ne!(first, second);
        assert_eq!(first, {
            let mut again = Vec::new();
            encode_type_expr(
                &mut again,
                &TypeExpr::Vector(Box::new(TypeExpr::UInt(IntegerWidth::from_bits(8)))),
                1,
                EXEC_PACKAGE_MAX_LAYOUTS_BYTES,
            )
            .expect("structural encode");
            again
        });
        let _ = ConstValue {
            value_type: TypeExpr::Unit,
            data: ConstData::Unit,
        };
    }

    #[test]
    fn nesting_past_the_depth_bound_refuses() {
        let mut nested = TypeExpr::Bool;
        for _ in 0..=sley_ssmc::MAX_TYPE_DEPTH {
            nested = TypeExpr::Vector(Box::new(nested));
        }
        let mut output = Vec::new();
        assert_eq!(
            encode_type_expr(&mut output, &nested, 1, EXEC_PACKAGE_MAX_LAYOUTS_BYTES),
            Err(PackageError::Malformed),
            "attacker-shaped nesting refuses instead of overflowing the encoder"
        );
    }

    #[test]
    fn named_and_function_types_encode() {
        let named = TypeExpr::Named(NamedType {
            definition: EntityId::from_bytes([0x42; 32]),
            arguments: Vec::new(),
        });
        let mut output = Vec::new();
        encode_type_expr(&mut output, &named, 1, EXEC_PACKAGE_MAX_LAYOUTS_BYTES)
            .expect("structural encode");
        assert!(!output.is_empty());
        let function = TypeExpr::FunctionRef(FunctionType {
            parameters: vec![TypeExpr::Bool],
            result: Box::new(TypeExpr::Unit),
            effects: Vec::new(),
        });
        let mut encoded = Vec::new();
        encode_type_expr(&mut encoded, &function, 1, EXEC_PACKAGE_MAX_LAYOUTS_BYTES)
            .expect("structural encode");
        assert!(!encoded.is_empty());
        let member = MemberId::from_bytes([0x43; 32]);
        let _ = (member, BTreeMap::<EntityId, TypeDefinition>::new());
    }
}
