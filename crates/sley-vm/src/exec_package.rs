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
    AdapterImport, BuiltinFailureKind, ConstantDefinition, ContractDefinition,
    GlobalValueDefinition, TypeDefForm, TypeDefinition, TypeExpr, Visibility,
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
/// `sley2-bootstrap-profile-1`; preserved byte-identical as history).
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

/// Dependency/inventory section bytes: entry + epoch + root + profile +
/// VM/lowerer versions + admitted limits + global/contract counts and IDs.
///
/// Globals and contracts travel by reference (initializer/contract IDs bound
/// here; their bodies are canonical repository state, not execution
/// immediates). This section binds the complete dependency/inventory digest.
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
    let mut preimage = Vec::new();
    preimage.extend_from_slice(EXEC_PACKAGE_MAGIC);
    push_u32(&mut preimage, EXEC_PACKAGE_V2_VERSION);
    preimage.extend_from_slice(&BOOTSTRAP_PROFILE_2_DIGEST);
    push_u32(&mut preimage, crate::host_abi::HOST_ABI_V2_VERSION);
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

    fn test_digest() -> [u8; 32] {
        [0x08; 32]
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
