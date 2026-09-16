//! Restricted S20-270 deterministic execution profile.

use core::fmt;
use std::sync::Arc;

use sley_check::{TypeEnvironment, TypeError};
use sley_id::{BytecodeCacheKey, EntityId, ObservationId, SchemaEpochId, StateRoot, ValueHash};
use sley_ssmc::{
    AdapterImport, BuiltinCase, CaseKey, ConstData, ConstValue, ConstantDefinition,
    ContractDefinition, GlobalValueDefinition, ResultConst, TypeExpr,
    fingerprint::{FingerprintError, FingerprintErrorCode, hash_validated_value},
};

use crate::{
    BytecodeSwitchArgument, BytecodeSwitchEdge, BytecodeTargetEdge, BytecodeTerminator,
    CacheProfile, LoweredFunction, LoweringError, LoweringInput, Register,
    SSMC1_DECODER_LIMITS_HASH, SSMC1_FIELD_SCHEMA_HASH,
    host_abi::{ImageError, load_image},
    lower::lower_function,
};

/// Maximum canonical S20-270 observation preimage bytes.
pub const MAX_OBSERVATION_PREIMAGE_BYTES: usize = 67_108_864;
/// Maximum ordered inputs accepted by restricted execution.
pub const MAX_EXECUTION_INPUTS: usize = 262_144;
/// Maximum validated input semantic value units before execution.
pub const MAX_EXECUTION_INPUT_VALUE_UNITS: u64 = 67_108_864;
/// Maximum live per-execution cells.
///
/// Charging cell contents already bounds the table by the request's value-unit
/// budget. This is the ceiling that holds when a caller declares an enormous
/// one, so the table cannot outgrow the profile no matter what a request asks.
pub const MAX_EXECUTION_CELLS: usize = 1_048_576;

/// One restricted-v1 execution request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionRequest {
    /// Function inputs in declaration-parameter order.
    pub inputs: Vec<ConstValue>,
    /// Deterministic execution limits.
    pub limits: ExecutionLimits,
}

/// Deterministic restricted-v1 execution limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionLimits {
    /// Maximum executed Boolean instructions.
    pub max_instructions: u64,
    /// Maximum charged fuel.
    pub max_fuel: u64,
    /// Maximum monotonic semantic value units.
    pub max_value_units: u64,
    /// Maximum returned or trap-payload value units.
    pub max_output_units: u64,
    /// Optional deterministic cancellation fuel point.
    pub cancel_at_fuel: Option<u64>,
}

/// One loaded-image execution request: everything execution needs except
/// the SSMC program inventories, which the image bytes already encode.
///
/// The decoded image carries code, types, registers, and immediates; the
/// caller supplies the execution-relevant inventories (constants, globals,
/// contracts, adapters) plus the epoch, root, and profile they are addressed
/// under. SSMC graphs, parameters, blocks, and operations are never read on
/// this path: there is nothing to lower.
#[derive(Clone, Copy, Debug)]
pub struct LoadedExecutionInput<'a> {
    /// Selected type environment.
    pub types: &'a TypeEnvironment,
    /// Complete Constant inventory (`constant_ref` resolves here).
    pub constants: &'a [ConstantDefinition],
    /// Complete `GlobalValue` inventory (`global_get` resolves here).
    pub globals: &'a [GlobalValueDefinition],
    /// Complete Contract inventory (`contract_assert` resolves here).
    pub contracts: &'a [ContractDefinition],
    /// Complete `AdapterImport` inventory (bridge entries resolve here).
    pub adapters: &'a [AdapterImport],
    /// Exact schema epoch.
    pub schema_epoch: SchemaEpochId,
    /// Exact state root.
    pub state_root: StateRoot,
    /// Requested cache/lowering profile.
    pub profile: CacheProfile,
}

/// The manifest-approved binding for one derived image: every checkable
/// identity `execute_loaded_image` verifies before running anything.
///
/// The manifest itself stays off-crate (the build driver holds it); this
/// struct is the exact shape the driver passes in, so any substitution of
/// bytes, epoch, root, entry, profile, or imports against the approved
/// binding refuses deterministically instead of executing by convention.
/// Limits are deliberately absent: budgets are the caller's per-execution
/// policy, and the observation binds the actual limits used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovedImage {
    /// Manifest-approved image identity (SHA-256 over the exact bytes).
    pub digest: [u8; 32],
    /// Manifest-approved cache identity (epoch, root, entry, profile).
    pub cache_key: BytecodeCacheKey,
    /// Manifest-approved import set: exact admitted import identities.
    pub imports: Vec<EntityId>,
}

/// One loaded-image execution failure: the exact preserved inner failure,
/// never a new code. Structural and identity refusals keep the `IMAGE_*`
/// vocabulary; every later failure keeps the code the lowering path would
/// report for the same condition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoadedExecutionError {
    /// Structural image refusal or manifest-identity mismatch.
    Image(ImageError),
    /// Input, cache-key, or fingerprint failure after a valid load.
    Execution(ExecutionError),
}

impl fmt::Display for LoadedExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Image(error) => error.fmt(formatter),
            Self::Execution(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LoadedExecutionError {}

/// Everything execution reads, however the bytecode arrived: by lowering
/// from canonical program state or by loading derived image bytes.
#[derive(Clone, Copy, Debug)]
struct ExecutionSource<'a> {
    /// Selected type environment.
    types: &'a TypeEnvironment,
    /// Complete Constant inventory.
    constants: &'a [ConstantDefinition],
    /// Complete `GlobalValue` inventory.
    globals: &'a [GlobalValueDefinition],
    /// Complete Contract inventory.
    contracts: &'a [ContractDefinition],
    /// Complete `AdapterImport` inventory.
    adapters: &'a [AdapterImport],
    /// Exact schema epoch.
    schema_epoch: SchemaEpochId,
    /// Exact state root.
    state_root: StateRoot,
    /// Requested cache/lowering profile.
    profile: CacheProfile,
    /// Entry function identity (decoded or lowered alike).
    function: EntityId,
}

impl<'a> ExecutionSource<'a> {
    /// Builds the source from a lowering request.
    fn lowering(input: &LoweringInput<'a>) -> Self {
        Self {
            types: input.types,
            constants: input.constants,
            globals: input.globals,
            contracts: input.contracts,
            adapters: input.adapters,
            schema_epoch: input.schema_epoch,
            state_root: input.state_root,
            profile: input.profile,
            function: input.function.entity_id,
        }
    }

    /// Builds the source from a loaded-image request.
    fn loaded(input: &LoadedExecutionInput<'a>, function: EntityId) -> Self {
        Self {
            types: input.types,
            constants: input.constants,
            globals: input.globals,
            contracts: input.contracts,
            adapters: input.adapters,
            schema_epoch: input.schema_epoch,
            state_root: input.state_root,
            profile: input.profile,
            function,
        }
    }
}

/// Closed S20-270 runtime resource kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    /// Instruction ceiling.
    Instruction,
    /// Fuel ceiling.
    Fuel,
    /// Semantic value-unit ceiling.
    ValueUnits,
    /// Output value-unit ceiling.
    OutputUnits,
    /// Extended profile only: the `call_direct` frame ceiling (contract E6).
    CallDepth,
}

impl ResourceKind {
    /// Returns the exact frozen observation/report tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Instruction => 1,
            Self::Fuel => 2,
            Self::ValueUnits => 3,
            Self::OutputUnits => 4,
            Self::CallDepth => 5,
        }
    }
}

/// Stable S20-270 runtime status failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionStatusCode {
    /// `VM_EXEC_RESOURCE_LIMIT`.
    ResourceLimit,
    /// `VM_EXEC_CANCELLED`.
    Cancelled,
    /// `VM_EXEC_TRAP`.
    Trap,
    /// `VM_EXEC_INTERNAL_INVARIANT`.
    InternalInvariant,
}

impl ExecutionStatusCode {
    /// Returns the stable symbolic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourceLimit => "VM_EXEC_RESOURCE_LIMIT",
            Self::Cancelled => "VM_EXEC_CANCELLED",
            Self::Trap => "VM_EXEC_TRAP",
            Self::InternalInvariant => "VM_EXEC_INTERNAL_INVARIANT",
        }
    }

    /// Returns the stable numeric code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::ResourceLimit => 27_002,
            Self::Cancelled => 27_003,
            Self::Trap => 27_004,
            Self::InternalInvariant => 27_005,
        }
    }
}

impl fmt::Display for ExecutionStatusCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Restricted-v1 execution termination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionTermination {
    /// Returned value.
    Success(ConstValue),
    /// Deterministic resource limit.
    ResourceLimit(ResourceKind),
    /// Deterministic cancellation point.
    Cancelled,
    /// Explicit SSMC trap.
    Trap {
        /// Exact frozen trap tag.
        trap_tag: u32,
        /// Optional persistable payload.
        payload: Option<ConstValue>,
    },
    /// Impossible runtime state after prior successful judgments.
    InternalInvariant,
}

impl ExecutionTermination {
    /// Returns the stable runtime status code, if this is not success.
    #[must_use]
    pub const fn status_code(&self) -> Option<ExecutionStatusCode> {
        match self {
            Self::Success(_) => None,
            Self::ResourceLimit(_) => Some(ExecutionStatusCode::ResourceLimit),
            Self::Cancelled => Some(ExecutionStatusCode::Cancelled),
            Self::Trap { .. } => Some(ExecutionStatusCode::Trap),
            Self::InternalInvariant => Some(ExecutionStatusCode::InternalInvariant),
        }
    }
}

/// Complete in-memory restricted-v1 execution outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionOutcome {
    /// Exact request state root.
    pub state_root: StateRoot,
    /// Exact request schema epoch.
    pub schema_epoch: SchemaEpochId,
    /// Executed Function identity.
    pub function: EntityId,
    /// Root/profile-bound bytecode cache key.
    pub cache_key: BytecodeCacheKey,
    /// Final termination.
    pub termination: ExecutionTermination,
    /// Count of executed Boolean instructions only.
    pub instruction_count: u64,
    /// Charged fuel.
    pub fuel_used: u64,
    /// Peak monotonic semantic value units.
    pub peak_value_units: u64,
    /// Deterministic observation digest.
    pub observation_id: ObservationId,
}

/// Stable S20-270 execution failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionErrorCode {
    /// `VM_EXEC_INPUT_COUNT_MISMATCH`.
    InputCountMismatch,
    /// `VM_EXEC_INPUT_TYPE_MISMATCH`.
    InputTypeMismatch,
    /// `VM_EXEC_INPUT_NOT_CANONICAL`.
    InputNotCanonical,
}

impl ExecutionErrorCode {
    /// Returns the stable symbolic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InputCountMismatch => "VM_EXEC_INPUT_COUNT_MISMATCH",
            Self::InputTypeMismatch => "VM_EXEC_INPUT_TYPE_MISMATCH",
            Self::InputNotCanonical => "VM_EXEC_INPUT_NOT_CANONICAL",
        }
    }

    /// Returns the stable numeric code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::InputCountMismatch => 27_000,
            Self::InputTypeMismatch => 27_001,
            Self::InputNotCanonical => 27_006,
        }
    }
}

impl fmt::Display for ExecutionErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Public pre-execution failure preserving earlier judgments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionError {
    /// Exact S20-210/S20-220/S20-260 failure.
    Lowering(LoweringError),
    /// Exact S20-210 type failure.
    Type(TypeError),
    /// Exact S20-250 value-hash failure.
    Fingerprint(FingerprintError),
    /// Stable S20-270 pre-execution resource failure.
    Status(ExecutionStatusCode),
    /// Exact S20-270 pre-execution failure.
    Exec(ExecutionErrorCode),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowering(value) => value.fmt(formatter),
            Self::Type(value) => value.fmt(formatter),
            Self::Fingerprint(value) => value.fmt(formatter),
            Self::Status(value) => value.fmt(formatter),
            Self::Exec(value) => value.fmt(formatter),
        }
    }
}

impl std::error::Error for ExecutionError {}

impl From<LoweringError> for ExecutionError {
    fn from(value: LoweringError) -> Self {
        Self::Lowering(value)
    }
}

impl From<TypeError> for ExecutionError {
    fn from(value: TypeError) -> Self {
        Self::Type(value)
    }
}

impl From<FingerprintError> for ExecutionError {
    fn from(value: FingerprintError) -> Self {
        Self::Fingerprint(value)
    }
}

#[derive(Clone, Debug)]
struct RuntimeValue {
    root: Arc<ConstValue>,
    payload_depth: usize,
}

impl RuntimeValue {
    fn new(value: ConstValue) -> Self {
        Self {
            root: Arc::new(value),
            payload_depth: 0,
        }
    }

    fn value(&self) -> RuntimeResult<&ConstValue> {
        let mut value = self.root.as_ref();
        for _ in 0..self.payload_depth {
            value = match &value.data {
                ConstData::Variant(variant) => variant.payload.as_deref().ok_or(RuntimeFault)?,
                ConstData::Option(Some(payload))
                | ConstData::Result(ResultConst::Ok(payload) | ResultConst::Err(payload)) => {
                    payload
                }
                _ => return Err(RuntimeFault),
            };
        }
        Ok(value)
    }

    fn payload_view(&self) -> RuntimeResult<Self> {
        let value = self.value()?;
        match &value.data {
            ConstData::Variant(variant) if variant.payload.is_some() => {}
            ConstData::Option(Some(_))
            | ConstData::Result(ResultConst::Ok(_) | ResultConst::Err(_)) => {}
            _ => return Err(RuntimeFault),
        }
        Ok(Self {
            root: Arc::clone(&self.root),
            payload_depth: self.payload_depth.checked_add(1).ok_or(RuntimeFault)?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct RuntimeFault;

type RuntimeResult<T> = Result<T, RuntimeFault>;

pub(crate) struct ValidatedInputs {
    pub(crate) hashes: Vec<ValueHash>,
    value_units: u64,
}

/// Validates exact S20-270 Function inputs and returns their ordered hashes.
///
/// This is the report/conformance evidence boundary for input count, complete
/// constant/type/hashability judgment, exact parameter types, and aggregate
/// input value-unit limits.
///
/// # Errors
///
/// Preserves the exact S20-210/S20-250/S20-270 input failure.
pub fn validated_execution_input_hashes(
    input: LoweringInput<'_>,
    request: &ExecutionRequest,
) -> Result<Vec<ValueHash>, ExecutionError> {
    Ok(validate_inputs(&input, request)?.hashes)
}

/// Returns the S20-270 saturating semantic value units for one constant.
#[must_use]
pub fn execution_value_units(value: &ConstValue) -> u64 {
    value_units_const(value)
}

#[derive(Clone, Debug)]
struct Runtime {
    registers: Vec<Option<RuntimeValue>>,
    block: usize,
    instruction_count: u64,
    fuel_used: u64,
    live_value_units: u64,
    peak_value_units: u64,
    cells: Vec<ConstValue>,
    max_call_depth: usize,
    peak_call_depth: usize,
}

/// Executes one restricted-v1 Function through the integrated lowering authority boundary.
///
/// # Errors
///
/// Returns only preserved lowering/type/fingerprint failures or pre-execution
/// input failures. Runtime terminations are represented as `ExecutionOutcome`.
#[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
pub fn execute_function(
    input: LoweringInput<'_>,
    request: ExecutionRequest,
) -> Result<ExecutionOutcome, ExecutionError> {
    let lowered = lower_function(input)?;
    let source = ExecutionSource::lowering(&input);
    let validated_inputs = validate_inputs(&input, &request)?;
    execute_core(&source, &lowered, &validated_inputs, request)
}

/// Executes supplied derived-image bytes through the validation-before-
/// execution boundary: structural load, manifest-identity verification,
/// approved-binding verification, then the same runner `execute_function`
/// uses.
///
/// The expected binding is the manifest-approved identity for exactly this
/// image: digest, cache key, and import set are each verified against the
/// supplied bytes and request before anything runs. Bytes that are
/// structurally valid but unexpected refuse with
/// [`ImageError::DigestMismatch`]; a verified digest with a mismatched
/// cache identity or import set refuses with
/// [`ImageError::BindingMismatch`]. The loader never decides which image
/// is approved. Inventories arrive from the caller as canonical program
/// state under the supplied epoch and root — the same trust
/// `execute_function` places in its lowering input — and the cache key is
/// re-derived from the caller epoch, root, decoded entry, and profile, so
/// a mismatched epoch, root, or profile refuses instead of executing under
/// another identity. Input validation applies the exact S20-270 checks with
/// types sourced from the decoded registers.
///
/// # Errors
///
/// Returns the structural refusal, the binding mismatch, or the exact
/// preserved failure the lowering path would report for the same condition.
#[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
pub fn execute_loaded_image(
    input: LoadedExecutionInput<'_>,
    expected: &ApprovedImage,
    bytes: &[u8],
    request: ExecutionRequest,
) -> Result<ExecutionOutcome, LoadedExecutionError> {
    let loaded = load_image(bytes).map_err(LoadedExecutionError::Image)?;
    if loaded.digest != expected.digest {
        return Err(LoadedExecutionError::Image(ImageError::DigestMismatch));
    }
    let source = ExecutionSource::loaded(&input, loaded.entry.function);
    let cache_key = crate::derive_cache_key(
        source.schema_epoch,
        source.state_root,
        source.function,
        source.profile,
    )
    .map_err(|error| LoadedExecutionError::Execution(ExecutionError::Lowering(error.into())))?;
    if cache_key != expected.cache_key {
        return Err(LoadedExecutionError::Image(ImageError::BindingMismatch));
    }
    let mut supplied: Vec<EntityId> = input.adapters.iter().map(|row| row.entity_id).collect();
    supplied.sort();
    let mut approved = expected.imports.clone();
    approved.sort();
    if supplied != approved {
        return Err(LoadedExecutionError::Image(ImageError::BindingMismatch));
    }
    let validated_inputs = validate_loaded_inputs(&source, &loaded.entry, &request)
        .map_err(LoadedExecutionError::Execution)?;
    // The load-boundary copy: the runner borrows one lowered model, so the
    // decoded structure travels in the same shape lowering emits. No
    // lowering work ran on this path, so the work counter stays zero.
    let lowered = LoweredFunction {
        bytecode: loaded.entry.clone(),
        bytes: bytes.to_vec(),
        cache_key,
        lowering_work: 0,
        callees: loaded.callees.clone(),
    };
    execute_core(&source, &lowered, &validated_inputs, request)
        .map_err(LoadedExecutionError::Execution)
}

/// One approved-package execution failure: the exact structural refusal,
/// never a new semantic code. Package/binding refusals keep the
/// `PACKAGE_*` vocabulary; structural image refusals keep `IMAGE_*`;
/// every later failure keeps the code the lowering path would report for
/// the same condition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageExecutionError {
    /// Envelope, digest, receipt, or complete-closure binding refusal.
    Package(crate::exec_package::PackageError),
    /// Structural image refusal after a valid package binding.
    Image(ImageError),
    /// Input, cache-key, fingerprint, or resource failure after valid load.
    Execution(ExecutionError),
}

impl fmt::Display for PackageExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Package(error) => error.fmt(formatter),
            Self::Image(error) => error.fmt(formatter),
            Self::Execution(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PackageExecutionError {}

/// Executes one complete execution package through the repaired host
/// boundary (RW-075 AR-01/AR-03).
///
/// Verification order (all bindings, before execution):
/// receipt vs package digest; every section digest vs header; image digest
/// vs bytes; entry/epoch/root/profile/ABI/VM/limits vs package; exact
/// import rows (`==`, full schemas — never IDs alone); re-derived cache
/// key. The execution request limits must equal the admitted limits
/// exactly: out-of-profile budgets cannot ride an approval for another
/// budget, and a broader-profile operation cannot execute inside a
/// bootstrap package merely because the shared runtime supports it.
///
/// Semantic validation (`TypeEnvironment::new`, `check_constant`,
/// `require_hashable`, fingerprint claims, checker/lowerer judgments) is
/// NOT rerun here. The compiler already performed it; the receipt binds
/// that judgment by digest. The host performs only structural hydration
/// (`hydrate_verified_definitions`: duplicate/bound checks), byte/codec
/// framing checks, exact-equality comparisons, and resource accounting.
/// Input values are checked for structural type equality against the
/// decoded register types, canonical codec form, and value-unit bounds —
/// never for language-level well-formedness. Observations are bound to the
/// complete package identity (`SLEYPOBS1` domain plus every section
/// digest), so two distinct packages cannot produce interchangeable
/// authority evidence.
///
/// # Errors
///
/// The exact structural refusal for the first mismatched binding, or the
/// preserved failure the lowering path would report for the same runtime
/// condition.
#[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
pub fn execute_approved_package(
    package: &crate::exec_package::ExecutionPackage,
    expected: &crate::exec_package::ApprovedExecutionPackage,
    request: ExecutionRequest,
) -> Result<ExecutionOutcome, PackageExecutionError> {
    use crate::exec_package::{hydrate_layouts, package_digests, verify_package_binding};
    let digests = package_digests(package).map_err(PackageExecutionError::Package)?;
    verify_package_binding(package, &digests, expected).map_err(PackageExecutionError::Package)?;
    if request.limits != expected.admitted_limits {
        return Err(PackageExecutionError::Package(
            crate::exec_package::PackageError::BindingMismatch,
        ));
    }
    let loaded = load_image(&package.image_bytes).map_err(PackageExecutionError::Image)?;
    if loaded.digest != expected.image_digest {
        return Err(PackageExecutionError::Image(ImageError::DigestMismatch));
    }
    let types = hydrate_layouts(package.type_definitions.clone())
        .map_err(PackageExecutionError::Package)?;
    let source = ExecutionSource {
        types: &types,
        constants: &package.constants,
        globals: &package.globals,
        contracts: &package.contracts,
        adapters: &package.imports,
        schema_epoch: package.schema_epoch,
        state_root: package.state_root,
        profile: package.profile,
        function: loaded.entry.function,
    };
    let cache_key = crate::derive_cache_key(
        source.schema_epoch,
        source.state_root,
        source.function,
        source.profile,
    )
    .map_err(|error| PackageExecutionError::Execution(ExecutionError::Lowering(error.into())))?;
    if cache_key != expected.cache_key {
        return Err(PackageExecutionError::Package(
            crate::exec_package::PackageError::BindingMismatch,
        ));
    }
    let validated_inputs =
        validate_package_inputs_structural(&loaded.entry, &request, source.schema_epoch)
            .map_err(PackageExecutionError::Execution)?;
    let lowered = LoweredFunction {
        bytecode: loaded.entry.clone(),
        bytes: package.image_bytes.clone(),
        cache_key,
        lowering_work: 0,
        callees: loaded.callees.clone(),
    };
    execute_core_package(
        &source,
        &lowered,
        &validated_inputs,
        request,
        expected,
        &digests,
    )
    .map_err(PackageExecutionError::Execution)
}

/// Successor package execution (RW-075 correction, AR-02).
///
/// Identical verification/execution discipline to [`execute_approved_package`],
/// binding the successor package digests (`u32(2)`, v2 profile digest, host
/// ABI version 2) via [`crate::package_digests_v2`] and
/// [`crate::verify_package_binding_v2`]. V1 packages keep their function;
/// v2 packages (including any `RHW1` closure) execute here. Observations
/// remain `SLEYPOBS1`-bound to the complete package identity, so v1 and v2
/// packages never share authority evidence.
///
/// # Errors
///
/// Same vocabulary as [`execute_approved_package`].
pub fn execute_approved_package_v2(
    package: &crate::exec_package::ExecutionPackage,
    expected: &crate::exec_package::ApprovedExecutionPackage,
    request: ExecutionRequest,
) -> Result<ExecutionOutcome, PackageExecutionError> {
    use crate::exec_package::{hydrate_layouts, package_digests_v2, verify_package_binding_v2};
    let digests = package_digests_v2(package).map_err(PackageExecutionError::Package)?;
    verify_package_binding_v2(package, &digests, expected)
        .map_err(PackageExecutionError::Package)?;
    if request.limits != expected.admitted_limits {
        return Err(PackageExecutionError::Package(
            crate::exec_package::PackageError::BindingMismatch,
        ));
    }
    let loaded = load_image(&package.image_bytes).map_err(PackageExecutionError::Image)?;
    if loaded.digest != expected.image_digest {
        return Err(PackageExecutionError::Image(ImageError::DigestMismatch));
    }
    let types = hydrate_layouts(package.type_definitions.clone())
        .map_err(PackageExecutionError::Package)?;
    let source = ExecutionSource {
        types: &types,
        constants: &package.constants,
        globals: &package.globals,
        contracts: &package.contracts,
        adapters: &package.imports,
        schema_epoch: package.schema_epoch,
        state_root: package.state_root,
        profile: package.profile,
        function: loaded.entry.function,
    };
    let cache_key = crate::derive_cache_key(
        source.schema_epoch,
        source.state_root,
        source.function,
        source.profile,
    )
    .map_err(|error| PackageExecutionError::Execution(ExecutionError::Lowering(error.into())))?;
    if cache_key != expected.cache_key {
        return Err(PackageExecutionError::Package(
            crate::exec_package::PackageError::BindingMismatch,
        ));
    }
    let validated_inputs =
        validate_package_inputs_structural(&loaded.entry, &request, source.schema_epoch)
            .map_err(PackageExecutionError::Execution)?;
    let lowered = LoweredFunction {
        bytecode: loaded.entry.clone(),
        bytes: package.image_bytes.clone(),
        cache_key,
        lowering_work: 0,
        callees: loaded.callees.clone(),
    };
    execute_core_package(
        &source,
        &lowered,
        &validated_inputs,
        request,
        expected,
        &digests,
    )
    .map_err(PackageExecutionError::Execution)
}

/// Validates package-path inputs structurally (no semantic judgment).
///
/// Checks, per input: count agreement; structural type equality
/// (`value.value_type == register_type` — never env lookup, trait
/// computation, or inference); canonical codec form (the existing
/// `encode_const_value` framing check, which prevents one semantic map
/// from carrying two identities — a memory-safety/identity property, not
/// a language verdict); capped value-unit accumulation; then the
/// codec-plus-hash (`hash_validated_value`, which itself performs no env
/// judgment). Language-level constant well-formedness and hashability were
/// judged by the compiler and are bound via the receipt; the host trusts
/// that binding and verifies only bytes.
fn validate_package_inputs_structural(
    bytecode: &crate::BytecodeFunction,
    request: &ExecutionRequest,
    schema_epoch: SchemaEpochId,
) -> Result<ValidatedInputs, ExecutionError> {
    if request.inputs.len() != bytecode.parameter_registers.len() {
        return Err(ExecutionError::Exec(ExecutionErrorCode::InputCountMismatch));
    }
    enforce_input_count(request.inputs.len())?;
    let mut hashes = Vec::with_capacity(request.inputs.len());
    let mut value_units = 0_u64;
    for (index, value) in request.inputs.iter().enumerate() {
        let register = usize::try_from(bytecode.parameter_registers[index])
            .ok()
            .and_then(|register| bytecode.register_types.get(register))
            .ok_or(ExecutionError::Exec(ExecutionErrorCode::InputTypeMismatch))?;
        if &value.value_type != register {
            return Err(ExecutionError::Exec(ExecutionErrorCode::InputTypeMismatch));
        }
        value_units = add_input_units(value_units, value_units_const(value))?;
        require_canonical_form(value)?;
        hashes.push(hash_validated_value(schema_epoch, value)?);
    }
    Ok(ValidatedInputs {
        hashes,
        value_units,
    })
}

/// Runs validated package inputs against one bytecode model with a
/// package-bound observation (the structural runner for the repaired path).
#[allow(clippy::too_many_lines)]
fn execute_core_package(
    source: &ExecutionSource<'_>,
    lowered: &LoweredFunction,
    validated_inputs: &ValidatedInputs,
    request: ExecutionRequest,
    expected: &crate::exec_package::ApprovedExecutionPackage,
    digests: &crate::exec_package::PackageDigests,
) -> Result<ExecutionOutcome, ExecutionError> {
    let initial_live_total = initial_value_units(
        &lowered.bytecode.register_types,
        &lowered.bytes,
        validated_inputs.value_units,
    );
    let ExecutionRequest { inputs, limits } = request;
    let mut runtime = Runtime {
        registers: vec![None; lowered.bytecode.register_types.len()],
        block: usize::try_from(lowered.bytecode.entry_block).unwrap_or(usize::MAX),
        instruction_count: 0,
        fuel_used: 0,
        live_value_units: initial_live_total,
        peak_value_units: initial_live_total,
        cells: Vec::new(),
        max_call_depth: MAX_CALL_DEPTH,
        peak_call_depth: 1,
    };
    if runtime.peak_value_units > limits.max_value_units {
        return finish_package(
            source,
            limits,
            lowered.cache_key,
            &validated_inputs.hashes,
            ExecutionTermination::ResourceLimit(ResourceKind::ValueUnits),
            runtime.instruction_count,
            runtime.fuel_used,
            runtime.peak_value_units,
            expected,
            digests,
        );
    }
    for (index, value) in inputs.into_iter().enumerate() {
        let Some(register) = lowered
            .bytecode
            .parameter_registers
            .get(index)
            .and_then(|value| usize::try_from(*value).ok())
        else {
            return observed_invariant_package(
                source,
                limits,
                lowered.cache_key,
                &validated_inputs.hashes,
                &runtime,
                expected,
                digests,
            );
        };
        if write_register(&mut runtime, register, RuntimeValue::new(value)).is_err() {
            return observed_invariant_package(
                source,
                limits,
                lowered.cache_key,
                &validated_inputs.hashes,
                &runtime,
                expected,
                digests,
            );
        }
    }
    let termination = match run(
        &mut runtime,
        limits,
        &lowered.bytecode.blocks,
        &lowered.bytecode.result_type,
        &lowered.bytecode.register_types,
        source,
        Some(lowered),
    ) {
        Ok(termination) => termination,
        Err(RuntimeFault) => ExecutionTermination::InternalInvariant,
    };
    finish_runtime_package(
        source,
        limits,
        lowered.cache_key,
        &validated_inputs.hashes,
        &runtime,
        termination,
        expected,
        digests,
    )
}

fn observed_invariant_package(
    source: &ExecutionSource<'_>,
    limits: ExecutionLimits,
    cache_key: BytecodeCacheKey,
    input_hashes: &[ValueHash],
    runtime: &Runtime,
    expected: &crate::exec_package::ApprovedExecutionPackage,
    digests: &crate::exec_package::PackageDigests,
) -> Result<ExecutionOutcome, ExecutionError> {
    finish_runtime_package(
        source,
        limits,
        cache_key,
        input_hashes,
        runtime,
        ExecutionTermination::InternalInvariant,
        expected,
        digests,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_runtime_package(
    source: &ExecutionSource<'_>,
    limits: ExecutionLimits,
    cache_key: BytecodeCacheKey,
    input_hashes: &[ValueHash],
    runtime: &Runtime,
    termination: ExecutionTermination,
    expected: &crate::exec_package::ApprovedExecutionPackage,
    digests: &crate::exec_package::PackageDigests,
) -> Result<ExecutionOutcome, ExecutionError> {
    finish_package(
        source,
        limits,
        cache_key,
        input_hashes,
        termination,
        runtime.instruction_count,
        runtime.fuel_used,
        runtime.peak_value_units,
        expected,
        digests,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_package(
    source: &ExecutionSource<'_>,
    limits: ExecutionLimits,
    cache_key: BytecodeCacheKey,
    input_hashes: &[ValueHash],
    termination: ExecutionTermination,
    instruction_count: u64,
    fuel_used: u64,
    peak_value_units: u64,
    expected: &crate::exec_package::ApprovedExecutionPackage,
    digests: &crate::exec_package::PackageDigests,
) -> Result<ExecutionOutcome, ExecutionError> {
    let observation_id = ObservationId::derive(observation_preimage_package(
        source,
        limits,
        cache_key,
        input_hashes,
        &termination,
        instruction_count,
        fuel_used,
        peak_value_units,
        expected,
        digests,
    )?);
    Ok(ExecutionOutcome {
        state_root: source.state_root,
        schema_epoch: source.schema_epoch,
        function: source.function,
        cache_key,
        termination,
        instruction_count,
        fuel_used,
        peak_value_units,
        observation_id,
    })
}

/// Package-bound observation preimage (`SLEYPOBS1` domain).
///
/// Binds everything the legacy `SLEYOBS1` preimage binds (epoch, schema
/// hashes, root, function, cache key, input hashes, limits, termination,
/// counts) PLUS the complete approved execution-package identity: package
/// digest, every section digest, the `BOOTSTRAP_PROFILE_1` digest, and the
/// host ABI version. The distinct domain prefix guarantees a package
/// observation can never equal a legacy observation even when all shared
/// fields coincide; the section digests guarantee two distinct packages
/// cannot produce interchangeable evidence.
#[allow(clippy::too_many_arguments)]
fn observation_preimage_package(
    source: &ExecutionSource<'_>,
    limits: ExecutionLimits,
    cache_key: BytecodeCacheKey,
    input_hashes: &[ValueHash],
    termination: &ExecutionTermination,
    instruction_count: u64,
    fuel_used: u64,
    peak_value_units: u64,
    expected: &crate::exec_package::ApprovedExecutionPackage,
    digests: &crate::exec_package::PackageDigests,
) -> Result<Vec<u8>, ExecutionError> {
    let capacity = input_hashes
        .len()
        .checked_mul(32)
        .and_then(|value| value.checked_add(768))
        .filter(|value| *value <= MAX_OBSERVATION_PREIMAGE_BYTES)
        .ok_or_else(|| {
            ExecutionError::Fingerprint(FingerprintError::new(FingerprintErrorCode::ResourceLimit))
        })?;
    let mut preimage = Vec::with_capacity(capacity);
    raw(&mut preimage, b"SLEYPOBS1");
    push_u32(&mut preimage, 1);
    raw(&mut preimage, source.schema_epoch.as_bytes());
    raw(&mut preimage, &SSMC1_FIELD_SCHEMA_HASH);
    raw(&mut preimage, &SSMC1_DECODER_LIMITS_HASH);
    raw(&mut preimage, source.state_root.as_bytes());
    raw(&mut preimage, source.function.as_bytes());
    raw(&mut preimage, cache_key.as_bytes());
    raw(&mut preimage, &digests.package_digest);
    raw(&mut preimage, &digests.image_digest);
    raw(&mut preimage, &digests.constants_digest);
    raw(&mut preimage, &digests.layouts_digest);
    raw(&mut preimage, &digests.imports_digest);
    raw(&mut preimage, &digests.dependency_digest);
    raw(&mut preimage, &expected.profile_digest);
    push_u32(&mut preimage, expected.host_abi_version);
    for part in CacheProfile::RESTRICTED_V1.vm_version {
        push_u32(&mut preimage, part);
    }
    push_u32(&mut preimage, 1);
    push_len(&mut preimage, input_hashes.len());
    for hash in input_hashes {
        raw(&mut preimage, hash.as_bytes());
    }
    push_u64(&mut preimage, limits.max_instructions);
    push_u64(&mut preimage, limits.max_fuel);
    push_u64(&mut preimage, limits.max_value_units);
    push_u64(&mut preimage, limits.max_output_units);
    match limits.cancel_at_fuel {
        None => push_u32(&mut preimage, 1),
        Some(value) => {
            push_u32(&mut preimage, 2);
            push_u64(&mut preimage, value);
        }
    }
    encode_termination_package(&mut preimage, source, termination)?;
    push_u64(&mut preimage, instruction_count);
    push_u64(&mut preimage, fuel_used);
    push_u64(&mut preimage, peak_value_units);
    push_u64(&mut preimage, 0);
    push_u64(&mut preimage, 0);
    push_u64(&mut preimage, 0);
    push_u64(&mut preimage, 0);
    debug_assert!(preimage.len() <= MAX_OBSERVATION_PREIMAGE_BYTES);
    Ok(preimage)
}

/// Package-path termination encoding: identical shapes to the legacy path
/// but WITHOUT `require_hashable` language judgments.
///
/// Success/trap payloads are hashed directly (codec + hash are structural).
/// Hashability was judged by the compiler and is bound via the receipt; the
/// host trusts that binding. Removing the check here is the AR-01 repair:
/// the host no longer performs language-level hashability judgment at
/// observation time.
fn encode_termination_package(
    preimage: &mut Vec<u8>,
    source: &ExecutionSource<'_>,
    termination: &ExecutionTermination,
) -> Result<(), ExecutionError> {
    match termination {
        ExecutionTermination::Success(value) => {
            push_u32(preimage, 1);
            raw(
                preimage,
                hash_validated_value(source.schema_epoch, value)?.as_bytes(),
            );
        }
        ExecutionTermination::ResourceLimit(kind) => {
            push_u32(preimage, 2);
            push_u32(preimage, kind.tag());
        }
        ExecutionTermination::Cancelled => push_u32(preimage, 3),
        ExecutionTermination::Trap { trap_tag, payload } => {
            push_u32(preimage, 4);
            push_u32(preimage, *trap_tag);
            match payload {
                None => push_u32(preimage, 1),
                Some(value) => {
                    push_u32(preimage, 2);
                    raw(
                        preimage,
                        hash_validated_value(source.schema_epoch, value)?.as_bytes(),
                    );
                }
            }
        }
        ExecutionTermination::InternalInvariant => push_u32(preimage, 5),
    }
    Ok(())
}

/// Unhashed result from the shared runner. Native execution performs its output
/// byte admission before deriving any result hash or observation.
pub(crate) struct CoreOutcome {
    pub(crate) termination: ExecutionTermination,
    pub(crate) instruction_count: u64,
    pub(crate) fuel_used: u64,
    pub(crate) peak_value_units: u64,
    pub(crate) peak_call_depth: usize,
}

fn execute_core(
    source: &ExecutionSource<'_>,
    lowered: &LoweredFunction,
    validated_inputs: &ValidatedInputs,
    request: ExecutionRequest,
) -> Result<ExecutionOutcome, ExecutionError> {
    let limits = request.limits;
    let result = run_core(source, lowered, validated_inputs, request, MAX_CALL_DEPTH);
    finish(
        source,
        limits,
        lowered.cache_key,
        &validated_inputs.hashes,
        result.termination,
        result.instruction_count,
        result.fuel_used,
        result.peak_value_units,
    )
}

pub(crate) fn execute_native_core(
    input: &LoweringInput<'_>,
    lowered: &LoweredFunction,
    validated_inputs: &ValidatedInputs,
    request: ExecutionRequest,
    max_call_depth: usize,
) -> CoreOutcome {
    run_core(
        &ExecutionSource::lowering(input),
        lowered,
        validated_inputs,
        request,
        max_call_depth,
    )
}

fn run_core(
    source: &ExecutionSource<'_>,
    lowered: &LoweredFunction,
    validated_inputs: &ValidatedInputs,
    request: ExecutionRequest,
    max_call_depth: usize,
) -> CoreOutcome {
    if max_call_depth == 0 {
        return CoreOutcome {
            termination: ExecutionTermination::ResourceLimit(ResourceKind::CallDepth),
            instruction_count: 0,
            fuel_used: 0,
            peak_value_units: 0,
            peak_call_depth: 0,
        };
    }
    let initial_live_total = initial_value_units(
        &lowered.bytecode.register_types,
        &lowered.bytes,
        validated_inputs.value_units,
    );
    let ExecutionRequest { inputs, limits } = request;
    let mut runtime = Runtime {
        registers: Vec::new(),
        block: usize::try_from(lowered.bytecode.entry_block).unwrap_or(usize::MAX),
        instruction_count: 0,
        fuel_used: 0,
        live_value_units: initial_live_total,
        peak_value_units: initial_live_total,
        cells: Vec::new(),
        max_call_depth,
        peak_call_depth: 1,
    };
    let termination = if runtime.peak_value_units > limits.max_value_units {
        ExecutionTermination::ResourceLimit(ResourceKind::ValueUnits)
    } else {
        runtime.registers = vec![None; lowered.bytecode.register_types.len()];
        match initialize_and_run(&mut runtime, source, lowered, inputs, limits) {
            Ok(termination) => termination,
            Err(RuntimeFault) => ExecutionTermination::InternalInvariant,
        }
    };
    CoreOutcome {
        termination,
        instruction_count: runtime.instruction_count,
        fuel_used: runtime.fuel_used,
        peak_value_units: runtime.peak_value_units,
        peak_call_depth: runtime.peak_call_depth,
    }
}

fn initialize_and_run(
    runtime: &mut Runtime,
    source: &ExecutionSource<'_>,
    lowered: &LoweredFunction,
    inputs: Vec<ConstValue>,
    limits: ExecutionLimits,
) -> RuntimeResult<ExecutionTermination> {
    for (index, value) in inputs.into_iter().enumerate() {
        let register = lowered
            .bytecode
            .parameter_registers
            .get(index)
            .and_then(|value| usize::try_from(*value).ok())
            .ok_or(RuntimeFault)?;
        write_register(runtime, register, RuntimeValue::new(value))?;
    }
    run(
        runtime,
        limits,
        &lowered.bytecode.blocks,
        &lowered.bytecode.result_type,
        &lowered.bytecode.register_types,
        source,
        Some(lowered),
    )
}

#[allow(clippy::too_many_lines)]
fn run(
    runtime: &mut Runtime,
    limits: ExecutionLimits,
    blocks: &[crate::BytecodeBlock],
    result_type: &TypeExpr,
    register_types: &[TypeExpr],
    source: &ExecutionSource<'_>,
    lowered: Option<&LoweredFunction>,
) -> RuntimeResult<ExecutionTermination> {
    let mut stack: Vec<Suspended<'_>> = Vec::new();
    let mut current = Current {
        blocks,
        result_type,
        register_types,
        pc: 0,
    };
    loop {
        let block = current.blocks.get(runtime.block).ok_or(RuntimeFault)?;
        if let Some(instruction) = block.instructions.get(current.pc) {
            current.pc += 1;
            if let Some(termination) =
                charge_action(runtime, &limits, Some(ResourceKind::Instruction))
            {
                return Ok(unwind(runtime, stack, termination));
            }
            if source.profile.is_extended() {
                let call = if instruction.opcode == sley_ssmc::Opcode::CallDirect.tag() {
                    Some(prepare_call(
                        runtime,
                        &limits,
                        instruction,
                        current.register_types,
                        lowered,
                        stack.len().saturating_add(1),
                    )?)
                } else if instruction.opcode == sley_ssmc::Opcode::ContractAssert.tag() {
                    Some(prepare_contract_assert(
                        runtime,
                        &limits,
                        instruction,
                        current.register_types,
                        lowered,
                        stack.len().saturating_add(1),
                        source.contracts,
                    )?)
                } else {
                    None
                };
                if let Some(step) = call {
                    match step {
                        CallStep::Terminated(termination) => {
                            return Ok(unwind(runtime, stack, termination));
                        }
                        CallStep::Enter {
                            child,
                            callee,
                            result_register,
                            shape,
                        } => {
                            stack.push(Suspended {
                                runtime: core::mem::replace(runtime, child),
                                blocks: current.blocks,
                                result_type: current.result_type,
                                register_types: current.register_types,
                                pc: current.pc,
                                result_register,
                                shape,
                            });
                            current = Current {
                                blocks: &callee.blocks,
                                result_type: &callee.result_type,
                                register_types: &callee.register_types,
                                pc: 0,
                            };
                        }
                    }
                    continue;
                }
                if let Some(termination) = execute_extended(
                    runtime,
                    &limits,
                    instruction,
                    current.register_types,
                    source,
                )? {
                    return Ok(unwind(runtime, stack, termination));
                }
                continue;
            }
            if let Some(termination) = execute_restricted(runtime, &limits, instruction)? {
                return Ok(termination);
            }
            continue;
        }

        let Some(termination) = dispatch_terminator(
            &limits,
            runtime,
            &block.terminator,
            current.blocks,
            current.result_type,
        )?
        else {
            current.pc = 0;
            continue;
        };
        let Some(parent) = stack.pop() else {
            return Ok(termination);
        };
        match return_to_caller(runtime, &limits, parent, termination)? {
            Ok(resumed) => current = resumed,
            Err(termination) => return Ok(unwind(runtime, stack, termination)),
        }
    }
}

/// Closes a callee frame: adopts the shared budgets, and on success charges
/// and writes the call result into the caller and resumes it.
fn return_to_caller<'a>(
    runtime: &mut Runtime,
    limits: &ExecutionLimits,
    parent: Suspended<'a>,
    termination: ExecutionTermination,
) -> RuntimeResult<Result<Current<'a>, ExecutionTermination>> {
    let child = core::mem::replace(runtime, parent.runtime);
    adopt_frame(runtime, child);
    let ExecutionTermination::Success(value) = termination else {
        return Ok(Err(termination));
    };
    let value = match parent.shape {
        ReturnShape::Direct => value,
        ReturnShape::ContractAssertion => {
            let ConstData::Bool(held) = value.data else {
                return Err(RuntimeFault);
            };
            contract_assert_value(held)
        }
    };
    if !charge_value(runtime, value_units_const(&value), limits.max_value_units) {
        return Ok(Err(ExecutionTermination::ResourceLimit(
            ResourceKind::ValueUnits,
        )));
    }
    runtime.instruction_count = runtime.instruction_count.saturating_add(1);
    write_register(runtime, parent.result_register, RuntimeValue::new(value))?;
    Ok(Ok(Current {
        blocks: parent.blocks,
        result_type: parent.result_type,
        register_types: parent.register_types,
        pc: parent.pc,
    }))
}

/// Executes one restricted-v1 boolean instruction.
fn execute_restricted(
    runtime: &mut Runtime,
    limits: &ExecutionLimits,
    instruction: &crate::Instruction,
) -> RuntimeResult<Option<ExecutionTermination>> {
    let operands = read_bool_operands(runtime, &instruction.operands)?;
    if instruction.results.len() != 1 {
        return Err(RuntimeFault);
    }
    let result_units = value_units_const(&ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(false),
    });
    if !charge_value(runtime, result_units, limits.max_value_units) {
        return Ok(Some(ExecutionTermination::ResourceLimit(
            ResourceKind::ValueUnits,
        )));
    }
    let value = match (instruction.opcode, operands.as_slice()) {
        (102, [value]) => !value,
        (103, [left, right]) => *left && *right,
        (104, [left, right]) => *left || *right,
        _ => return Err(RuntimeFault),
    };
    let result = ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    };
    runtime.instruction_count = runtime.instruction_count.saturating_add(1);
    let register = usize::try_from(instruction.results[0]).map_err(|_| RuntimeFault)?;
    write_register(runtime, register, RuntimeValue::new(result))?;
    Ok(None)
}

/// Restores the entry frame's runtime (with the shared budgets) after a
/// termination inside a callee frame.
fn unwind(
    runtime: &mut Runtime,
    mut stack: Vec<Suspended<'_>>,
    termination: ExecutionTermination,
) -> ExecutionTermination {
    while let Some(parent) = stack.pop() {
        let child = core::mem::replace(runtime, parent.runtime);
        adopt_frame(runtime, child);
    }
    termination
}

/// Contract E6: at most 256 frames, the entry frame included.
const MAX_CALL_DEPTH: usize = 256;

/// One suspended caller frame of the explicit call stack.
struct Suspended<'a> {
    runtime: Runtime,
    blocks: &'a [crate::BytecodeBlock],
    result_type: &'a TypeExpr,
    register_types: &'a [TypeExpr],
    pc: usize,
    result_register: usize,
    shape: ReturnShape,
}

/// What the caller does with a returned value.
///
/// A `call_direct` frame writes the callee's value unchanged. A slice E7a
/// `contract_assert` frame calls a `Bool` predicate and writes the contract
/// result: `Ok(Unit)` when the predicate held and
/// `Err(BuiltinFailure(ContractViolation, 1))` when it did not.
#[derive(Clone, Copy, Eq, PartialEq)]
enum ReturnShape {
    Direct,
    ContractAssertion,
}

/// The code the current frame executes.
#[derive(Clone, Copy)]
struct Current<'a> {
    blocks: &'a [crate::BytecodeBlock],
    result_type: &'a TypeExpr,
    register_types: &'a [TypeExpr],
    pc: usize,
}

/// What a `call_direct` instruction does next.
enum CallStep<'a> {
    Terminated(ExecutionTermination),
    Enter {
        child: Runtime,
        callee: &'a crate::BytecodeFunction,
        result_register: usize,
        shape: ReturnShape,
    },
}

/// Copies the shared budgets and cells back from a finished callee frame.
fn adopt_frame(runtime: &mut Runtime, child: Runtime) {
    runtime.instruction_count = child.instruction_count;
    runtime.fuel_used = child.fuel_used;
    runtime.live_value_units = child.live_value_units;
    runtime.peak_value_units = child.peak_value_units;
    runtime.cells = child.cells;
    runtime.peak_call_depth = child.peak_call_depth;
}

/// Prepares `call_direct` (contract E6): refuses a frame beyond the ceiling,
/// charges one fuel, and opens a callee register file that shares the
/// budgets and cells with the caller.
fn prepare_call<'a>(
    runtime: &mut Runtime,
    limits: &ExecutionLimits,
    instruction: &crate::Instruction,
    register_types: &[TypeExpr],
    lowered: Option<&'a LoweredFunction>,
    frames: usize,
) -> RuntimeResult<CallStep<'a>> {
    let sley_ssmc::Immediate::Function(reference) = &instruction.immediate else {
        return Err(RuntimeFault);
    };
    prepare_frame(
        runtime,
        limits,
        instruction,
        register_types,
        lowered,
        frames,
        reference.function,
        ReturnShape::Direct,
    )
}

/// Prepares `contract_assert` (slice E7a): resolves the contract's predicate
/// and enters it as an ordinary frame whose return the caller wraps.
fn prepare_contract_assert<'a>(
    runtime: &mut Runtime,
    limits: &ExecutionLimits,
    instruction: &crate::Instruction,
    register_types: &[TypeExpr],
    lowered: Option<&'a LoweredFunction>,
    frames: usize,
    contracts: &[sley_ssmc::ContractDefinition],
) -> RuntimeResult<CallStep<'a>> {
    let sley_ssmc::Immediate::Entity(contract) = &instruction.immediate else {
        return Err(RuntimeFault);
    };
    let definition = contracts
        .iter()
        .find(|candidate| candidate.entity_id == *contract)
        .ok_or(RuntimeFault)?;
    prepare_frame(
        runtime,
        limits,
        instruction,
        register_types,
        lowered,
        frames,
        definition.predicate,
        ReturnShape::ContractAssertion,
    )
}

/// The frame mechanics both call shapes share.
#[allow(clippy::too_many_arguments)]
fn prepare_frame<'a>(
    runtime: &mut Runtime,
    limits: &ExecutionLimits,
    instruction: &crate::Instruction,
    register_types: &[TypeExpr],
    lowered: Option<&'a LoweredFunction>,
    frames: usize,
    function: sley_id::EntityId,
    shape: ReturnShape,
) -> RuntimeResult<CallStep<'a>> {
    // The ceiling counts live frames including the running one: opening a
    // frame that would make 257 live is refused, so 256 live frames (entry
    // included) is the deepest reachable stack (contract E6).
    if frames.saturating_add(1) > runtime.max_call_depth {
        return Ok(CallStep::Terminated(ExecutionTermination::ResourceLimit(
            ResourceKind::CallDepth,
        )));
    }
    if let Some(termination) = charge_action(runtime, limits, None) {
        return Ok(CallStep::Terminated(termination));
    }
    let lowered = lowered.ok_or(RuntimeFault)?;
    let callee = lowered
        .callees
        .iter()
        .find(|callee| callee.function == function)
        .or_else(|| (lowered.bytecode.function == function).then_some(&lowered.bytecode))
        .ok_or(RuntimeFault)?;
    let [result_register] = instruction.results.as_slice() else {
        return Err(RuntimeFault);
    };
    let result_register = usize::try_from(*result_register).map_err(|_| RuntimeFault)?;
    let result_type = register_types.get(result_register).ok_or(RuntimeFault)?;
    let declared_matches = match shape {
        ReturnShape::Direct => callee.result_type == *result_type,
        // The predicate answers `Bool`; the operation's register holds the
        // contract result, so the two types are deliberately different.
        ReturnShape::ContractAssertion => {
            callee.result_type == TypeExpr::Bool && *result_type == contract_assert_type()
        }
    };
    if !declared_matches || instruction.operands.len() != callee.parameter_registers.len() {
        return Err(RuntimeFault);
    }
    let mut child = Runtime {
        registers: vec![None; callee.register_types.len()],
        block: usize::try_from(callee.entry_block).map_err(|_| RuntimeFault)?,
        instruction_count: runtime.instruction_count,
        fuel_used: runtime.fuel_used,
        live_value_units: runtime.live_value_units,
        peak_value_units: runtime.peak_value_units,
        cells: core::mem::take(&mut runtime.cells),
        max_call_depth: runtime.max_call_depth,
        peak_call_depth: runtime.peak_call_depth.max(frames.saturating_add(1)),
    };
    for (operand, parameter) in instruction.operands.iter().zip(&callee.parameter_registers) {
        let value = read_register(runtime, *operand)?.value()?.clone();
        if !charge_value(
            &mut child,
            value_units_const(&value),
            limits.max_value_units,
        ) {
            adopt_frame(runtime, child);
            return Ok(CallStep::Terminated(ExecutionTermination::ResourceLimit(
                ResourceKind::ValueUnits,
            )));
        }
        let parameter = usize::try_from(*parameter).map_err(|_| RuntimeFault)?;
        write_register(&mut child, parameter, RuntimeValue::new(value))?;
    }
    Ok(CallStep::Enter {
        child,
        callee,
        result_register,
        shape,
    })
}

/// The exact slice E7a `contract_assert` result type.
fn contract_assert_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Unit),
        error: Box::new(TypeExpr::BuiltinFailure(
            sley_ssmc::BuiltinFailureKind::ContractViolation,
        )),
    }
}

/// Wraps a predicate's answer in the contract result.
fn contract_assert_value(held: bool) -> ConstValue {
    let error = TypeExpr::BuiltinFailure(sley_ssmc::BuiltinFailureKind::ContractViolation);
    ConstValue {
        value_type: contract_assert_type(),
        data: ConstData::Result(if held {
            sley_ssmc::ResultConst::Ok(Box::new(ConstValue {
                value_type: TypeExpr::Unit,
                data: ConstData::Unit,
            }))
        } else {
            sley_ssmc::ResultConst::Err(Box::new(ConstValue {
                value_type: error,
                data: ConstData::BuiltinFailure(sley_ssmc::BuiltinFailureValue {
                    kind: sley_ssmc::BuiltinFailureKind::ContractViolation,
                    code: 1,
                }),
            }))
        }),
    }
}

/// Executes one instruction under the extended profile: reads every operand,
/// derives the result through the family semantics, charges its value units,
/// and writes the single result register.
fn execute_extended(
    runtime: &mut Runtime,
    limits: &ExecutionLimits,
    instruction: &crate::Instruction,
    register_types: &[TypeExpr],
    source: &ExecutionSource<'_>,
) -> RuntimeResult<Option<ExecutionTermination>> {
    let opcode = sley_ssmc::Opcode::from_tag(instruction.opcode).ok_or(RuntimeFault)?;
    let mut operands = Vec::with_capacity(instruction.operands.len());
    for register in &instruction.operands {
        operands.push(read_register(runtime, *register)?.value()?.clone());
    }
    let [result_register] = instruction.results.as_slice() else {
        return Err(RuntimeFault);
    };
    let register = usize::try_from(*result_register).map_err(|_| RuntimeFault)?;
    let result_type = register_types.get(register).ok_or(RuntimeFault)?;
    // Slice E8: bridge fuel is charged up front, before the arm allocates
    // or converts, so a starved budget terminates without the work being
    // performed (the E6 call-fuel precedent). Capacity refusal still
    // answers Err(Index, 2) under adequate budgets.
    if instruction.opcode == sley_ssmc::Opcode::AdapterInvoke.tag()
        && let Some(elements) = crate::extended::bridge_fuel_surcharge(
            source.adapters,
            &instruction.immediate,
            &operands,
        )
    {
        let fuel = elements.saturating_mul(crate::extended::BRIDGE_ELEMENT_FUEL);
        for _ in 0..fuel {
            if let Some(termination) = charge_action(runtime, limits, None) {
                return Ok(Some(termination));
            }
        }
    }
    let value = {
        let mut context = crate::extended::ExecutionContext {
            types: source.types,
            constants: source.constants,
            globals: source.globals,
            schema_epoch: source.schema_epoch,
            adapters: source.adapters,
            cells: &mut runtime.cells,
        };
        crate::extended::execute_extended_instruction(
            &mut context,
            opcode,
            &instruction.immediate,
            &operands,
            result_type,
        )
        .map_err(|_| RuntimeFault)?
    };
    if &value.value_type != result_type {
        return Err(RuntimeFault);
    }
    // At most MAX_EXECUTION_CELLS cells exist in one execution (contract
    // E5): the check fires as soon as the count reaches the cap, so the
    // table never holds more.
    if runtime.cells.len() >= MAX_EXECUTION_CELLS {
        return Ok(Some(ExecutionTermination::ResourceLimit(
            ResourceKind::ValueUnits,
        )));
    }
    // `cell_new` and `cell_set` clone their value into the cell table, which
    // outlives the instruction. Charging only the result would charge the
    // handle and not the contents, so a loop could hold unbounded host memory
    // with the budget intact; contract E5 counts cell contents as live value
    // units, and this is where they are counted.
    let stored = match opcode {
        sley_ssmc::Opcode::CellNew | sley_ssmc::Opcode::CellSet => {
            operands.last().map_or(0, value_units_const)
        }
        _ => 0,
    };
    if !charge_value(
        runtime,
        value_units_const(&value).saturating_add(stored),
        limits.max_value_units,
    ) {
        return Ok(Some(ExecutionTermination::ResourceLimit(
            ResourceKind::ValueUnits,
        )));
    }
    runtime.instruction_count = runtime.instruction_count.saturating_add(1);
    write_register(runtime, register, RuntimeValue::new(value))?;
    Ok(None)
}

pub(crate) fn validate_inputs(
    input: &LoweringInput<'_>,
    request: &ExecutionRequest,
) -> Result<ValidatedInputs, ExecutionError> {
    if request.inputs.len() != input.function.parameters.len() {
        return Err(ExecutionError::Exec(ExecutionErrorCode::InputCountMismatch));
    }
    enforce_input_count(request.inputs.len())?;
    let mut hashes = Vec::with_capacity(request.inputs.len());
    let mut value_units = 0_u64;
    for (index, value) in request.inputs.iter().enumerate() {
        let parameter_type = input
            .parameters
            .iter()
            .find(|parameter| parameter.entity_id == input.function.parameters[index])
            .map(|parameter| &parameter.value_type)
            .ok_or(ExecutionError::Exec(ExecutionErrorCode::InputTypeMismatch))?;
        value_units = add_input_units(
            value_units,
            check_input_shape(input.types, parameter_type, value)?,
        )?;
        require_canonical_form(value)?;
        hashes.push(hash_validated_value(input.schema_epoch, value)?);
    }
    Ok(ValidatedInputs {
        hashes,
        value_units,
    })
}

/// Validates loaded-image inputs with the exact S20-270 checks, sourcing
/// parameter types from the decoded registers instead of the SSMC
/// inventory: count, per-value shape, capped units, canonical form, hashes.
/// A parameter register the image does not type fails closed before
/// execution; approved images always type every parameter register.
fn validate_loaded_inputs(
    source: &ExecutionSource<'_>,
    bytecode: &crate::BytecodeFunction,
    request: &ExecutionRequest,
) -> Result<ValidatedInputs, ExecutionError> {
    if request.inputs.len() != bytecode.parameter_registers.len() {
        return Err(ExecutionError::Exec(ExecutionErrorCode::InputCountMismatch));
    }
    enforce_input_count(request.inputs.len())?;
    let mut hashes = Vec::with_capacity(request.inputs.len());
    let mut value_units = 0_u64;
    for (index, value) in request.inputs.iter().enumerate() {
        let register = usize::try_from(bytecode.parameter_registers[index])
            .ok()
            .and_then(|register| bytecode.register_types.get(register))
            .ok_or(ExecutionError::Exec(ExecutionErrorCode::InputTypeMismatch))?;
        value_units = add_input_units(
            value_units,
            check_input_shape(source.types, register, value)?,
        )?;
        require_canonical_form(value)?;
        hashes.push(hash_validated_value(source.schema_epoch, value)?);
    }
    Ok(ValidatedInputs {
        hashes,
        value_units,
    })
}

/// Checks one supplied input value against its expected type: complete
/// constant/type/hashability judgment plus exact type equality. Returns the
/// value's semantic units for capped accumulation by the caller.
fn check_input_shape(
    types: &TypeEnvironment,
    expected: &TypeExpr,
    value: &ConstValue,
) -> Result<u64, ExecutionError> {
    types.check_constant(value)?;
    types.require_hashable(&value.value_type)?;
    if &value.value_type != expected {
        return Err(ExecutionError::Exec(ExecutionErrorCode::InputTypeMismatch));
    }
    Ok(value_units_const(value))
}

/// Requires that one supplied value has an exact S20-350 canonical form.
///
/// Ordered-map entry order is the lexicographic order of the keys' canonical
/// bytes. `TYPE_SYSTEM_V1.md` section 5 reserves that ordering to the selected
/// SCB encoder/decoder: S20-210 rejects duplicate and non-orderable keys but
/// never reimplements the byte order and never silently sorts a decoded
/// constant. Extended-profile `equal` and `value_hash` read entry order
/// structurally, so a value reaching this public boundary without crossing the
/// codec could give one semantic map two identities. The VM therefore asks the
/// codec whether the value is canonical and refuses when it is not, rather
/// than sorting it or restating the order itself.
fn require_canonical_form(value: &ConstValue) -> Result<(), ExecutionError> {
    if sley_mutate::encode_const_value(value).is_err() {
        return Err(ExecutionError::Exec(ExecutionErrorCode::InputNotCanonical));
    }
    Ok(())
}

fn enforce_input_count(count: usize) -> Result<(), ExecutionError> {
    if count > MAX_EXECUTION_INPUTS {
        Err(preexecution_resource_error())
    } else {
        Ok(())
    }
}

fn add_input_units(current: u64, amount: u64) -> Result<u64, ExecutionError> {
    current
        .checked_add(amount)
        .filter(|value| *value <= MAX_EXECUTION_INPUT_VALUE_UNITS)
        .ok_or_else(preexecution_resource_error)
}

fn preexecution_resource_error() -> ExecutionError {
    ExecutionError::Status(ExecutionStatusCode::ResourceLimit)
}

fn dispatch_terminator(
    limits: &ExecutionLimits,
    runtime: &mut Runtime,
    terminator: &BytecodeTerminator,
    blocks: &[crate::BytecodeBlock],
    result_type: &TypeExpr,
) -> RuntimeResult<Option<ExecutionTermination>> {
    if let Some(termination) = charge_action(runtime, limits, None) {
        return Ok(Some(termination));
    }
    match terminator {
        BytecodeTerminator::Return(register) => {
            let value = read_register(runtime, *register)?.value()?.clone();
            if &value.value_type != result_type {
                return Ok(Some(ExecutionTermination::InternalInvariant));
            }
            if value_units_const(&value) > limits.max_output_units {
                return Ok(Some(ExecutionTermination::ResourceLimit(
                    ResourceKind::OutputUnits,
                )));
            }
            Ok(Some(ExecutionTermination::Success(value)))
        }
        BytecodeTerminator::Branch(edge) => {
            if let Some(termination) = bind_edge(runtime, blocks, edge, limits)? {
                return Ok(Some(termination));
            }
            Ok(None)
        }
        BytecodeTerminator::CondBranch {
            condition,
            if_true,
            if_false,
        } => {
            let ConstData::Bool(condition) = read_register(runtime, *condition)?.value()?.data
            else {
                return Ok(Some(ExecutionTermination::InternalInvariant));
            };
            let edge = if condition { if_true } else { if_false };
            if let Some(termination) = bind_edge(runtime, blocks, edge, limits)? {
                return Ok(Some(termination));
            }
            Ok(None)
        }
        BytecodeTerminator::VariantSwitch { value, cases } => {
            let selected = read_register(runtime, *value)?.clone();
            let (case_key, payload) = selected_case(&selected)?;
            for case in cases {
                if let Some(termination) = charge_action(runtime, limits, None) {
                    return Ok(Some(termination));
                }
                if case.case_key == case_key {
                    return bind_switch_edge(runtime, blocks, &case.edge, payload.as_ref(), limits);
                }
            }
            Ok(Some(ExecutionTermination::InternalInvariant))
        }
        BytecodeTerminator::Trap { code, payload } => {
            let payload = payload
                .map(|register| {
                    read_register(runtime, register).and_then(|value| value.value().cloned())
                })
                .transpose()?;
            if payload
                .as_ref()
                .is_some_and(|value| value_units_const(value) > limits.max_output_units)
            {
                return Ok(Some(ExecutionTermination::ResourceLimit(
                    ResourceKind::OutputUnits,
                )));
            }
            Ok(Some(ExecutionTermination::Trap {
                trap_tag: *code,
                payload,
            }))
        }
    }
}

fn bind_edge(
    runtime: &mut Runtime,
    blocks: &[crate::BytecodeBlock],
    edge: &BytecodeTargetEdge,
    limits: &ExecutionLimits,
) -> RuntimeResult<Option<ExecutionTermination>> {
    let mut values = Vec::with_capacity(edge.arguments.len());
    for register in &edge.arguments {
        if let Some(termination) = charge_action(runtime, limits, None) {
            return Ok(Some(termination));
        }
        values.push(read_register(runtime, *register)?.clone());
    }
    bind_values(runtime, blocks, edge.target, values)?;
    Ok(None)
}

fn bind_switch_edge(
    runtime: &mut Runtime,
    blocks: &[crate::BytecodeBlock],
    edge: &BytecodeSwitchEdge,
    payload: Option<&RuntimeValue>,
    limits: &ExecutionLimits,
) -> RuntimeResult<Option<ExecutionTermination>> {
    let mut values = Vec::with_capacity(edge.arguments.len());
    for argument in &edge.arguments {
        if let Some(termination) = charge_action(runtime, limits, None) {
            return Ok(Some(termination));
        }
        match argument {
            BytecodeSwitchArgument::Value(register) => {
                values.push(read_register(runtime, *register).cloned()?);
            }
            BytecodeSwitchArgument::CasePayload => {
                if let Some(termination) = charge_action(runtime, limits, None) {
                    return Ok(Some(termination));
                }
                let Some(payload) = payload.cloned() else {
                    return Ok(Some(ExecutionTermination::InternalInvariant));
                };
                values.push(payload);
            }
        }
    }
    bind_values(runtime, blocks, edge.target, values)?;
    Ok(None)
}

fn bind_values(
    runtime: &mut Runtime,
    blocks: &[crate::BytecodeBlock],
    target: u32,
    values: Vec<RuntimeValue>,
) -> RuntimeResult<()> {
    let target = usize::try_from(target).map_err(|_| RuntimeFault)?;
    let block = blocks.get(target).ok_or(RuntimeFault)?;
    if block.parameter_registers.len() != values.len() {
        return Err(RuntimeFault);
    }
    for (register, value) in block.parameter_registers.iter().zip(values) {
        let register = usize::try_from(*register).map_err(|_| RuntimeFault)?;
        write_register(runtime, register, value)?;
    }
    runtime.block = target;
    Ok(())
}

fn selected_case(value: &RuntimeValue) -> RuntimeResult<(CaseKey, Option<RuntimeValue>)> {
    match &value.value()?.data {
        ConstData::Variant(variant) => Ok((
            CaseKey::Member(variant.member_id),
            variant
                .payload
                .as_ref()
                .map(|_| value.payload_view())
                .transpose()?,
        )),
        ConstData::Option(None) => Ok((CaseKey::Builtin(BuiltinCase::None), None)),
        ConstData::Option(Some(_)) => Ok((
            CaseKey::Builtin(BuiltinCase::Some),
            Some(value.payload_view()?),
        )),
        ConstData::Result(ResultConst::Ok(_)) => Ok((
            CaseKey::Builtin(BuiltinCase::Ok),
            Some(value.payload_view()?),
        )),
        ConstData::Result(ResultConst::Err(_)) => Ok((
            CaseKey::Builtin(BuiltinCase::Err),
            Some(value.payload_view()?),
        )),
        _ => Err(RuntimeFault),
    }
}

fn charge_action(
    runtime: &mut Runtime,
    limits: &ExecutionLimits,
    instruction: Option<ResourceKind>,
) -> Option<ExecutionTermination> {
    if limits
        .cancel_at_fuel
        .is_some_and(|cancel_at| cancel_at <= runtime.fuel_used)
    {
        return Some(ExecutionTermination::Cancelled);
    }
    if instruction.is_some() && runtime.instruction_count >= limits.max_instructions {
        return Some(ExecutionTermination::ResourceLimit(
            ResourceKind::Instruction,
        ));
    }
    if runtime.fuel_used >= limits.max_fuel {
        return Some(ExecutionTermination::ResourceLimit(ResourceKind::Fuel));
    }
    runtime.fuel_used = runtime.fuel_used.saturating_add(1);
    None
}

fn charge_value(runtime: &mut Runtime, amount: u64, max_value_units: u64) -> bool {
    let Some(next) = runtime.live_value_units.checked_add(amount) else {
        return false;
    };
    if next > max_value_units {
        return false;
    }
    runtime.live_value_units = next;
    runtime.peak_value_units = runtime.peak_value_units.max(next);
    true
}

fn read_bool_operands(runtime: &Runtime, registers: &[Register]) -> RuntimeResult<Vec<bool>> {
    let mut values = Vec::with_capacity(registers.len());
    for register in registers {
        match &read_register(runtime, *register)?.value()?.data {
            ConstData::Bool(value) => values.push(*value),
            _ => return Err(RuntimeFault),
        }
    }
    Ok(values)
}

fn read_register(runtime: &Runtime, register: Register) -> RuntimeResult<&RuntimeValue> {
    let register = usize::try_from(register).map_err(|_| RuntimeFault)?;
    runtime
        .registers
        .get(register)
        .and_then(Option::as_ref)
        .ok_or(RuntimeFault)
}

fn write_register(
    runtime: &mut Runtime,
    register: usize,
    value: RuntimeValue,
) -> RuntimeResult<()> {
    let slot = runtime.registers.get_mut(register).ok_or(RuntimeFault)?;
    *slot = Some(value);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish(
    source: &ExecutionSource<'_>,
    limits: ExecutionLimits,
    cache_key: BytecodeCacheKey,
    input_hashes: &[ValueHash],
    termination: ExecutionTermination,
    instruction_count: u64,
    fuel_used: u64,
    peak_value_units: u64,
) -> Result<ExecutionOutcome, ExecutionError> {
    let observation_id = ObservationId::derive(observation_preimage(
        source,
        limits,
        cache_key,
        input_hashes,
        &termination,
        instruction_count,
        fuel_used,
        peak_value_units,
    )?);
    Ok(ExecutionOutcome {
        state_root: source.state_root,
        schema_epoch: source.schema_epoch,
        function: source.function,
        cache_key,
        termination,
        instruction_count,
        fuel_used,
        peak_value_units,
        observation_id,
    })
}

/// Rederives one S20-270 observation through the VM semantic authority.
///
/// Callers must supply input hashes produced only after the S20-210/S20-250
/// checks required by `VM_EXEC_RESTRICTED_V1`.
///
/// # Errors
///
/// Preserves exact type/fingerprint failures or returns a bounded resource
/// failure when the canonical observation preimage cannot be encoded.
#[allow(clippy::too_many_arguments)]
pub fn derive_observation_id(
    input: LoweringInput<'_>,
    limits: ExecutionLimits,
    cache_key: BytecodeCacheKey,
    input_hashes: &[ValueHash],
    termination: &ExecutionTermination,
    instruction_count: u64,
    fuel_used: u64,
    peak_value_units: u64,
) -> Result<ObservationId, ExecutionError> {
    let source = ExecutionSource::lowering(&input);
    Ok(ObservationId::derive(observation_preimage(
        &source,
        limits,
        cache_key,
        input_hashes,
        termination,
        instruction_count,
        fuel_used,
        peak_value_units,
    )?))
}

#[allow(clippy::too_many_arguments)]
fn observation_preimage(
    source: &ExecutionSource<'_>,
    limits: ExecutionLimits,
    cache_key: BytecodeCacheKey,
    input_hashes: &[ValueHash],
    termination: &ExecutionTermination,
    instruction_count: u64,
    fuel_used: u64,
    peak_value_units: u64,
) -> Result<Vec<u8>, ExecutionError> {
    let capacity = input_hashes
        .len()
        .checked_mul(32)
        .and_then(|value| value.checked_add(512))
        .filter(|value| *value <= MAX_OBSERVATION_PREIMAGE_BYTES)
        .ok_or_else(|| {
            ExecutionError::Fingerprint(FingerprintError::new(FingerprintErrorCode::ResourceLimit))
        })?;
    let mut preimage = Vec::with_capacity(capacity);
    raw(&mut preimage, b"SLEYOBS1");
    push_u32(&mut preimage, 1);
    raw(&mut preimage, source.schema_epoch.as_bytes());
    raw(&mut preimage, &SSMC1_FIELD_SCHEMA_HASH);
    raw(&mut preimage, &SSMC1_DECODER_LIMITS_HASH);
    raw(&mut preimage, source.state_root.as_bytes());
    raw(&mut preimage, source.function.as_bytes());
    raw(&mut preimage, cache_key.as_bytes());
    // The cache key above already binds the lowering profile, so an
    // observation cannot be ambiguous between the restricted and the extended
    // profile and these two fields stay at their frozen S20-270 values. The
    // S20-290 report envelope needed the opposite treatment: a rejected report
    // carries no cache key, so it binds the profile itself.
    for part in CacheProfile::RESTRICTED_V1.vm_version {
        push_u32(&mut preimage, part);
    }
    push_u32(&mut preimage, 1);
    push_len(&mut preimage, input_hashes.len());
    for hash in input_hashes {
        raw(&mut preimage, hash.as_bytes());
    }
    push_u64(&mut preimage, limits.max_instructions);
    push_u64(&mut preimage, limits.max_fuel);
    push_u64(&mut preimage, limits.max_value_units);
    push_u64(&mut preimage, limits.max_output_units);
    match limits.cancel_at_fuel {
        None => push_u32(&mut preimage, 1),
        Some(value) => {
            push_u32(&mut preimage, 2);
            push_u64(&mut preimage, value);
        }
    }
    encode_termination(&mut preimage, source, termination)?;
    push_u64(&mut preimage, instruction_count);
    push_u64(&mut preimage, fuel_used);
    push_u64(&mut preimage, peak_value_units);
    push_u64(&mut preimage, 0);
    push_u64(&mut preimage, 0);
    push_u64(&mut preimage, 0);
    push_u64(&mut preimage, 0);
    debug_assert!(preimage.len() <= MAX_OBSERVATION_PREIMAGE_BYTES);
    Ok(preimage)
}

fn encode_termination(
    preimage: &mut Vec<u8>,
    source: &ExecutionSource<'_>,
    termination: &ExecutionTermination,
) -> Result<(), ExecutionError> {
    match termination {
        ExecutionTermination::Success(value) => {
            push_u32(preimage, 1);
            source.types.require_hashable(&value.value_type)?;
            raw(
                preimage,
                hash_validated_value(source.schema_epoch, value)?.as_bytes(),
            );
        }
        ExecutionTermination::ResourceLimit(kind) => {
            push_u32(preimage, 2);
            push_u32(preimage, kind.tag());
        }
        ExecutionTermination::Cancelled => push_u32(preimage, 3),
        ExecutionTermination::Trap { trap_tag, payload } => {
            push_u32(preimage, 4);
            push_u32(preimage, *trap_tag);
            match payload {
                None => push_u32(preimage, 1),
                Some(value) => {
                    push_u32(preimage, 2);
                    source.types.require_hashable(&value.value_type)?;
                    raw(
                        preimage,
                        hash_validated_value(source.schema_epoch, value)?.as_bytes(),
                    );
                }
            }
        }
        ExecutionTermination::InternalInvariant => push_u32(preimage, 5),
    }
    Ok(())
}

fn initial_value_units(types: &[TypeExpr], bytes: &[u8], input_units: u64) -> u64 {
    input_units
        .saturating_add(u64::try_from(types.len()).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX))
}

fn value_units_const(value: &ConstValue) -> u64 {
    1_u64
        .saturating_add(value_units_type(&value.value_type))
        .saturating_add(value_units_data(&value.data))
}

fn value_units_type(value: &TypeExpr) -> u64 {
    match value {
        TypeExpr::SInt(_) | TypeExpr::UInt(_) | TypeExpr::BuiltinFailure(_) => 3,
        TypeExpr::Unit
        | TypeExpr::Bool
        | TypeExpr::F32
        | TypeExpr::F64
        | TypeExpr::Bytes
        | TypeExpr::Text => 1,
        TypeExpr::Tuple(items) => 1_u64
            .saturating_add(u64::try_from(items.len()).unwrap_or(u64::MAX))
            .saturating_add(items.iter().map(value_units_type).fold(0, saturating_add)),
        TypeExpr::Named(named) => 33_u64
            .saturating_add(u64::try_from(named.arguments.len()).unwrap_or(u64::MAX))
            .saturating_add(
                named
                    .arguments
                    .iter()
                    .map(value_units_type)
                    .fold(0, saturating_add),
            ),
        TypeExpr::Vector(item) | TypeExpr::Option(item) | TypeExpr::LocalCell(item) => {
            1_u64.saturating_add(value_units_type(item))
        }
        TypeExpr::OrderedMap { key, value }
        | TypeExpr::Result {
            ok: key,
            error: value,
        } => 1_u64
            .saturating_add(value_units_type(key))
            .saturating_add(value_units_type(value)),
        TypeExpr::FunctionRef(function) => 1_u64
            .saturating_add(u64::try_from(function.parameters.len()).unwrap_or(u64::MAX))
            .saturating_add(
                function
                    .parameters
                    .iter()
                    .map(value_units_type)
                    .fold(0, saturating_add),
            )
            .saturating_add(value_units_type(&function.result))
            .saturating_add(u64::try_from(function.effects.len()).unwrap_or(u64::MAX))
            .saturating_add(
                u64::try_from(function.effects.len())
                    .unwrap_or(u64::MAX)
                    .saturating_mul(32),
            ),
        TypeExpr::AdapterHandle(_) | TypeExpr::CapabilityToken(_) => 33,
        TypeExpr::TypeParameter(_) => 5,
    }
}

fn value_units_data(value: &ConstData) -> u64 {
    match value {
        ConstData::Bool(_) => 2,
        ConstData::SInt(_) | ConstData::UInt(_) => 17,
        ConstData::F32Bits(_) | ConstData::BuiltinFailure(_) => 5,
        ConstData::F64Bits(_) => 9,
        ConstData::Bytes(bytes) => {
            1_u64.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX))
        }
        ConstData::Text(text) => {
            1_u64.saturating_add(u64::try_from(text.len()).unwrap_or(u64::MAX))
        }
        ConstData::Sequence(values) => 1_u64
            .saturating_add(u64::try_from(values.len()).unwrap_or(u64::MAX))
            .saturating_add(values.iter().map(value_units_const).fold(0, saturating_add)),
        ConstData::Record(record) => 33_u64
            .saturating_add(u64::try_from(record.fields.len()).unwrap_or(u64::MAX))
            .saturating_add(
                record
                    .fields
                    .iter()
                    .map(|field| 32_u64.saturating_add(value_units_const(&field.value)))
                    .fold(0, saturating_add),
            ),
        ConstData::Variant(variant) => 65_u64.saturating_add(
            variant
                .payload
                .as_deref()
                .map_or(0, |value| 1_u64.saturating_add(value_units_const(value))),
        ),
        ConstData::Map(entries) => 1_u64
            .saturating_add(u64::try_from(entries.len()).unwrap_or(u64::MAX))
            .saturating_add(
                entries
                    .iter()
                    .map(|entry| {
                        value_units_const(&entry.key)
                            .saturating_add(value_units_const(&entry.value))
                    })
                    .fold(0, saturating_add),
            ),
        ConstData::Unit | ConstData::Option(None) => 1,
        ConstData::Option(Some(value)) => 2_u64.saturating_add(value_units_const(value)),
        ConstData::Result(ResultConst::Ok(value) | ResultConst::Err(value)) => {
            2_u64.saturating_add(value_units_const(value))
        }
        ConstData::FunctionRef(reference) => 33_u64
            .saturating_add(u64::try_from(reference.type_arguments.len()).unwrap_or(u64::MAX))
            .saturating_add(
                reference
                    .type_arguments
                    .iter()
                    .map(value_units_type)
                    .fold(0, saturating_add),
            ),
    }
}

const fn saturating_add(left: u64, right: u64) -> u64 {
    left.saturating_add(right)
}

fn raw(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(bytes);
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn push_len(output: &mut Vec<u8>, value: usize) {
    push_u64(output, u64::try_from(value).unwrap_or(u64::MAX));
}

#[cfg(test)]
mod tests {
    use core::fmt::Write as _;
    use std::collections::BTreeSet;

    use super::*;
    use sley_check::TypeEnvironment;
    use sley_id::{SchemaEpochId, StateRoot};
    use sley_ssmc::{
        Block, BranchTerminator, BuiltinCase, CaseKey, CondBranchTerminator, FunctionGraph,
        Immediate, MemberId, NamedType, Opcode, Operation, OperationResultRef, Parameter,
        ParameterRole, Reachability, ReturnTerminator, SwitchArgument, SwitchCase, SwitchEdge,
        TargetEdge, Terminator, TrapCode, TrapTerminator, TypeDefForm, TypeDefinition, ValueRef,
        VariantCase, VariantConst, VariantSwitchTerminator, Visibility,
    };

    fn id(byte: u8) -> EntityId {
        EntityId::from_bytes([byte; 32])
    }

    fn bool_value(value: bool) -> ConstValue {
        ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(value),
        }
    }

    fn hex(bytes: &[u8]) -> String {
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(&mut output, "{byte:02x}").unwrap();
        }
        output
    }

    struct Fixture {
        types: TypeEnvironment,
        function: FunctionGraph,
        parameters: Vec<Parameter>,
        blocks: Vec<Block>,
        operations: Vec<Operation>,
    }

    impl Fixture {
        fn input(&self) -> LoweringInput<'_> {
            LoweringInput {
                types: &self.types,
                function: &self.function,
                parameters: &self.parameters,
                blocks: &self.blocks,
                operations: &self.operations,
                schema_epoch: SchemaEpochId::from_bytes([8; 32]),
                state_root: StateRoot::from_bytes([9; 32]),
                profile: CacheProfile::RESTRICTED_V1,
                constants: &[],
                globals: &[],
                functions: &[],
                contracts: &[],
                adapters: &[],
            }
        }
    }

    fn bool_fixture(opcode: Opcode) -> Fixture {
        let function = id(1);
        let left = id(2);
        let right = id(3);
        let block = id(4);
        let operation = id(5);
        Fixture {
            types: TypeEnvironment::new(Vec::new()).unwrap(),
            function: FunctionGraph {
                entity_id: function,
                type_parameters: Vec::new(),
                parameters: vec![left, right],
                result_type: TypeExpr::Bool,
                effects: Vec::new(),
                entry_block: block,
                blocks: vec![block],
                contracts: Vec::new(),
                visibility: Visibility::Private,
            },
            parameters: vec![
                Parameter {
                    entity_id: left,
                    owner: function,
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: TypeExpr::Bool,
                },
                Parameter {
                    entity_id: right,
                    owner: function,
                    role: ParameterRole::Function,
                    ordinal: 1,
                    value_type: TypeExpr::Bool,
                },
            ],
            blocks: vec![Block {
                entity_id: block,
                function,
                parameters: Vec::new(),
                operations: vec![operation],
                terminator: Terminator::Return(ReturnTerminator {
                    value: ValueRef::OperationResult(OperationResultRef {
                        operation,
                        result_index: 0,
                    }),
                }),
                reachability: Reachability::Required,
            }],
            operations: vec![Operation {
                entity_id: operation,
                block,
                ordinal: 0,
                opcode,
                operands: vec![ValueRef::Parameter(left), ValueRef::Parameter(right)],
                result_types: vec![TypeExpr::Bool],
                immediate: Immediate::None,
            }],
        }
    }

    #[allow(clippy::too_many_lines)]
    fn two_case_switch_fixture(
        types: TypeEnvironment,
        selector_type: TypeExpr,
        first_key: CaseKey,
        second_key: CaseKey,
        first_has_payload: bool,
    ) -> Fixture {
        let function = id(20);
        let selector = id(21);
        let fallback = id(22);
        let entry = id(23);
        let first_block = id(24);
        let first_value = id(25);
        let second_block = id(26);
        let second_value = id(27);
        Fixture {
            types,
            function: FunctionGraph {
                entity_id: function,
                type_parameters: Vec::new(),
                parameters: vec![selector, fallback],
                result_type: TypeExpr::Bool,
                effects: Vec::new(),
                entry_block: entry,
                blocks: vec![entry, first_block, second_block],
                contracts: Vec::new(),
                visibility: Visibility::Private,
            },
            parameters: vec![
                Parameter {
                    entity_id: selector,
                    owner: function,
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: selector_type,
                },
                Parameter {
                    entity_id: fallback,
                    owner: function,
                    role: ParameterRole::Function,
                    ordinal: 1,
                    value_type: TypeExpr::Bool,
                },
                Parameter {
                    entity_id: first_value,
                    owner: first_block,
                    role: ParameterRole::Block,
                    ordinal: 0,
                    value_type: TypeExpr::Bool,
                },
                Parameter {
                    entity_id: second_value,
                    owner: second_block,
                    role: ParameterRole::Block,
                    ordinal: 0,
                    value_type: TypeExpr::Bool,
                },
            ],
            blocks: vec![
                Block {
                    entity_id: entry,
                    function,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::VariantSwitch(VariantSwitchTerminator {
                        value: ValueRef::Parameter(selector),
                        cases: vec![
                            SwitchCase {
                                case_key: first_key,
                                edge: SwitchEdge {
                                    target: first_block,
                                    arguments: vec![if first_has_payload {
                                        SwitchArgument::CasePayload
                                    } else {
                                        SwitchArgument::Value(ValueRef::Parameter(fallback))
                                    }],
                                },
                            },
                            SwitchCase {
                                case_key: second_key,
                                edge: SwitchEdge {
                                    target: second_block,
                                    arguments: vec![SwitchArgument::CasePayload],
                                },
                            },
                        ],
                    }),
                    reachability: Reachability::Required,
                },
                Block {
                    entity_id: first_block,
                    function,
                    parameters: vec![first_value],
                    operations: Vec::new(),
                    terminator: Terminator::Return(ReturnTerminator {
                        value: ValueRef::Parameter(first_value),
                    }),
                    reachability: Reachability::Required,
                },
                Block {
                    entity_id: second_block,
                    function,
                    parameters: vec![second_value],
                    operations: Vec::new(),
                    terminator: Terminator::Return(ReturnTerminator {
                        value: ValueRef::Parameter(second_value),
                    }),
                    reachability: Reachability::Required,
                },
            ],
            operations: Vec::new(),
        }
    }

    fn limits() -> ExecutionLimits {
        ExecutionLimits {
            max_instructions: 100,
            max_fuel: 100,
            max_value_units: 10_000,
            max_output_units: 100,
            cancel_at_fuel: None,
        }
    }

    #[test]
    fn executes_all_boolean_opcodes_and_repeats_deterministically() {
        let cases = [
            (Opcode::BoolNot, vec![true, false], false),
            (Opcode::BoolAnd, vec![true, false], false),
            (Opcode::BoolOr, vec![true, false], true),
        ];
        for (opcode, inputs, expected) in cases {
            let mut fixture = bool_fixture(opcode);
            if opcode == Opcode::BoolNot {
                fixture.operations[0].operands.pop();
            }
            let request = ExecutionRequest {
                inputs: inputs.into_iter().map(bool_value).collect(),
                limits: limits(),
            };
            let first = execute_function(fixture.input(), request.clone()).unwrap();
            assert_eq!(
                first.termination,
                ExecutionTermination::Success(bool_value(expected))
            );
            assert_eq!(first.instruction_count, 1);
            assert_eq!(first.fuel_used, 2);
            for _ in 0..128 {
                assert_eq!(
                    execute_function(fixture.input(), request.clone()).unwrap(),
                    first
                );
            }
        }
    }

    #[test]
    fn input_failures_precede_runtime() {
        let fixture = bool_fixture(Opcode::BoolAnd);
        let error = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![bool_value(true)],
                limits: limits(),
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            ExecutionError::Exec(ExecutionErrorCode::InputCountMismatch)
        );

        let error = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![
                    ConstValue {
                        value_type: TypeExpr::Unit,
                        data: ConstData::Unit,
                    },
                    bool_value(true),
                ],
                limits: limits(),
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            ExecutionError::Exec(ExecutionErrorCode::InputTypeMismatch)
        );
    }

    #[test]
    fn branch_conditional_trap_resource_and_cancel_terminate_deterministically() {
        let mut branch = bool_fixture(Opcode::BoolAnd);
        let result = ValueRef::OperationResult(OperationResultRef {
            operation: id(5),
            result_index: 0,
        });
        let target = id(6);
        let parameter = id(7);
        branch.function.blocks.push(target);
        branch.parameters.push(Parameter {
            entity_id: parameter,
            owner: target,
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        });
        branch.blocks.push(Block {
            entity_id: target,
            function: branch.function.entity_id,
            parameters: vec![parameter],
            operations: Vec::new(),
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::Parameter(parameter),
            }),
            reachability: Reachability::Required,
        });
        branch.blocks[0].terminator = Terminator::Branch(BranchTerminator {
            edge: TargetEdge {
                target,
                arguments: vec![result],
            },
        });
        let request = ExecutionRequest {
            inputs: vec![bool_value(true), bool_value(true)],
            limits: limits(),
        };
        let branch_outcome = execute_function(branch.input(), request).unwrap();
        assert_eq!(
            branch_outcome.termination,
            ExecutionTermination::Success(bool_value(true))
        );
        assert_eq!(branch_outcome.fuel_used, 4);

        let mut cond = bool_fixture(Opcode::BoolAnd);
        cond.blocks[0].terminator = Terminator::CondBranch(CondBranchTerminator {
            condition: result,
            if_true: TargetEdge {
                target: cond.blocks[0].entity_id,
                arguments: Vec::new(),
            },
            if_false: TargetEdge {
                target: cond.blocks[0].entity_id,
                arguments: Vec::new(),
            },
        });
        let mut request = ExecutionRequest {
            inputs: vec![bool_value(true), bool_value(false)],
            limits: limits(),
        };
        request.limits.max_instructions = 1;
        assert_eq!(
            execute_function(cond.input(), request).unwrap().termination,
            ExecutionTermination::ResourceLimit(ResourceKind::Instruction)
        );

        let mut trap = bool_fixture(Opcode::BoolAnd);
        trap.blocks[0].terminator = Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: Some(result),
        });
        let request = ExecutionRequest {
            inputs: vec![bool_value(true), bool_value(true)],
            limits: limits(),
        };
        assert_eq!(
            execute_function(trap.input(), request).unwrap().termination,
            ExecutionTermination::Trap {
                trap_tag: 4,
                payload: Some(bool_value(true)),
            }
        );

        let mut request = ExecutionRequest {
            inputs: vec![bool_value(true), bool_value(true)],
            limits: limits(),
        };
        request.limits.cancel_at_fuel = Some(0);
        assert_eq!(
            execute_function(bool_fixture(Opcode::BoolAnd).input(), request)
                .unwrap()
                .termination,
            ExecutionTermination::Cancelled
        );
    }

    #[test]
    fn option_switch_binds_payload_and_no_payload_edges_with_exact_fuel() {
        let fixture = two_case_switch_fixture(
            TypeEnvironment::new(Vec::new()).unwrap(),
            TypeExpr::Option(Box::new(TypeExpr::Bool)),
            CaseKey::Builtin(BuiltinCase::None),
            CaseKey::Builtin(BuiltinCase::Some),
            false,
        );
        let some = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![
                    ConstValue {
                        value_type: TypeExpr::Option(Box::new(TypeExpr::Bool)),
                        data: ConstData::Option(Some(Box::new(bool_value(true)))),
                    },
                    bool_value(false),
                ],
                limits: limits(),
            },
        )
        .unwrap();
        assert_eq!(
            some.termination,
            ExecutionTermination::Success(bool_value(true))
        );
        assert_eq!(some.fuel_used, 6);

        let none = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![
                    ConstValue {
                        value_type: TypeExpr::Option(Box::new(TypeExpr::Bool)),
                        data: ConstData::Option(None),
                    },
                    bool_value(false),
                ],
                limits: limits(),
            },
        )
        .unwrap();
        assert_eq!(
            none.termination,
            ExecutionTermination::Success(bool_value(false))
        );
        assert_eq!(none.fuel_used, 4);
    }

    #[test]
    fn result_and_named_variant_switch_payloads_are_reference_views() {
        let result_type = TypeExpr::Result {
            ok: Box::new(TypeExpr::Bool),
            error: Box::new(TypeExpr::Bool),
        };
        let result_fixture = two_case_switch_fixture(
            TypeEnvironment::new(Vec::new()).unwrap(),
            result_type.clone(),
            CaseKey::Builtin(BuiltinCase::Ok),
            CaseKey::Builtin(BuiltinCase::Err),
            true,
        );
        let result = execute_function(
            result_fixture.input(),
            ExecutionRequest {
                inputs: vec![
                    ConstValue {
                        value_type: result_type,
                        data: ConstData::Result(ResultConst::Ok(Box::new(bool_value(true)))),
                    },
                    bool_value(false),
                ],
                limits: limits(),
            },
        )
        .unwrap();
        assert_eq!(
            result.termination,
            ExecutionTermination::Success(bool_value(true))
        );

        let definition = id(40);
        let empty_case = MemberId::from_bytes([41; 32]);
        let payload_case = MemberId::from_bytes([42; 32]);
        let named_type = TypeExpr::Named(NamedType {
            definition,
            arguments: Vec::new(),
        });
        let named_fixture = two_case_switch_fixture(
            TypeEnvironment::new(vec![TypeDefinition {
                entity_id: definition,
                type_parameters: Vec::new(),
                form: TypeDefForm::Variant(vec![
                    VariantCase {
                        member_id: empty_case,
                        payload_type: None,
                    },
                    VariantCase {
                        member_id: payload_case,
                        payload_type: Some(TypeExpr::Bool),
                    },
                ]),
                invariants: Vec::new(),
                visibility: Visibility::Private,
            }])
            .unwrap(),
            named_type.clone(),
            CaseKey::Member(empty_case),
            CaseKey::Member(payload_case),
            false,
        );
        let named = execute_function(
            named_fixture.input(),
            ExecutionRequest {
                inputs: vec![
                    ConstValue {
                        value_type: named_type,
                        data: ConstData::Variant(VariantConst {
                            definition,
                            member_id: payload_case,
                            payload: Some(Box::new(bool_value(true))),
                        }),
                    },
                    bool_value(false),
                ],
                limits: limits(),
            },
        )
        .unwrap();
        assert_eq!(
            named.termination,
            ExecutionTermination::Success(bool_value(true))
        );

        let runtime = RuntimeValue::new(ConstValue {
            value_type: TypeExpr::Option(Box::new(TypeExpr::Bool)),
            data: ConstData::Option(Some(Box::new(bool_value(true)))),
        });
        let (_, payload) = selected_case(&runtime).unwrap();
        let payload = payload.unwrap();
        assert!(Arc::ptr_eq(&runtime.root, &payload.root));
        assert_eq!(payload.payload_depth, 1);
    }

    #[test]
    fn observation_changes_with_semantic_inputs_and_limits() {
        let fixture = bool_fixture(Opcode::BoolOr);
        let first = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![bool_value(false), bool_value(false)],
                limits: limits(),
            },
        )
        .unwrap();
        let second = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![bool_value(true), bool_value(false)],
                limits: limits(),
            },
        )
        .unwrap();
        assert_ne!(first.observation_id, second.observation_id);

        let mut changed_limits = limits();
        changed_limits.max_fuel += 1;
        let third = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![bool_value(false), bool_value(false)],
                limits: changed_limits,
            },
        )
        .unwrap();
        assert_ne!(first.observation_id, third.observation_id);
    }

    #[test]
    fn observation_preimage_and_id_are_exact() {
        let fixture = bool_fixture(Opcode::BoolAnd);
        let request = ExecutionRequest {
            inputs: vec![bool_value(true), bool_value(false)],
            limits: limits(),
        };
        let lowered = lower_function(fixture.input()).unwrap();
        let validated_inputs = validate_inputs(&fixture.input(), &request).unwrap();
        let outcome = execute_function(fixture.input(), request.clone()).unwrap();
        let input = fixture.input();
        let source = ExecutionSource::lowering(&input);
        let preimage = observation_preimage(
            &source,
            request.limits,
            lowered.cache_key,
            &validated_inputs.hashes,
            &outcome.termination,
            outcome.instruction_count,
            outcome.fuel_used,
            outcome.peak_value_units,
        )
        .unwrap();
        assert_eq!(preimage.len(), 420);
        assert_eq!(
            hex(&preimage),
            concat!(
                "534c45594f42533100000001",
                "0808080808080808080808080808080808080808080808080808080808080808",
                "1983bc8d6ad9ac3cb5390853f43959cf2c3dc0ae8e0ca18ca8264ca4960133ae",
                "389791b170bc9d8575f7e6f338e4f9e9f2b75f35d7a2e52c7cb106cb2cd6136a",
                "0909090909090909090909090909090909090909090909090909090909090909",
                "0101010101010101010101010101010101010101010101010101010101010101",
                "e4f8abdd00e3124a0979d10240af7b0113cb16ae148e79e236574b3379c368d9",
                "00000001000000000000000000000001",
                "0000000000000002",
                "db8444a32d480f3628e125d71e5521e571d39e13c3b610010b8bc1a7be75104d",
                "24d78841cab551683b11d31fc8985b1c13481fe3f28930e7b0663c4a248d4d58",
                "0000000000000064000000000000006400000000000027100000000000000064",
                "00000001",
                "0000000124d78841cab551683b11d31fc8985b1c13481fe3f28930e7b0663c4a248d4d58",
                "0000000000000001000000000000000200000000000000af",
                "0000000000000000000000000000000000000000000000000000000000000000"
            )
        );
        assert_eq!(outcome.observation_id, ObservationId::derive(preimage));
    }

    #[test]
    fn resource_and_cancellation_precedence_is_exact() {
        let fixture = bool_fixture(Opcode::BoolAnd);

        assert_eq!(
            enforce_input_count(MAX_EXECUTION_INPUTS.saturating_add(1)).unwrap_err(),
            ExecutionError::Status(ExecutionStatusCode::ResourceLimit)
        );
        assert_eq!(
            add_input_units(MAX_EXECUTION_INPUT_VALUE_UNITS, 1).unwrap_err(),
            ExecutionError::Status(ExecutionStatusCode::ResourceLimit)
        );

        let mut cancelled_limits = limits();
        cancelled_limits.cancel_at_fuel = Some(0);
        cancelled_limits.max_fuel = 0;
        cancelled_limits.max_instructions = 0;
        let cancelled = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![bool_value(true), bool_value(true)],
                limits: cancelled_limits,
            },
        )
        .unwrap();
        assert_eq!(cancelled.termination, ExecutionTermination::Cancelled);
        assert_eq!(cancelled.fuel_used, 0);
        assert_eq!(cancelled.instruction_count, 0);

        let mut instruction_limits = limits();
        instruction_limits.max_instructions = 0;
        let instruction = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![bool_value(true), bool_value(true)],
                limits: instruction_limits,
            },
        )
        .unwrap();
        assert_eq!(
            instruction.termination,
            ExecutionTermination::ResourceLimit(ResourceKind::Instruction)
        );
        assert_eq!(instruction.fuel_used, 0);

        let mut fuel_limits = limits();
        fuel_limits.max_fuel = 0;
        let fuel = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![bool_value(true), bool_value(true)],
                limits: fuel_limits,
            },
        )
        .unwrap();
        assert_eq!(
            fuel.termination,
            ExecutionTermination::ResourceLimit(ResourceKind::Fuel)
        );

        let mut value_limits = limits();
        value_limits.max_value_units = 0;
        let value = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![bool_value(true), bool_value(true)],
                limits: value_limits,
            },
        )
        .unwrap();
        assert_eq!(
            value.termination,
            ExecutionTermination::ResourceLimit(ResourceKind::ValueUnits)
        );
        assert_eq!(value.fuel_used, 0);
        assert_eq!(value.instruction_count, 0);
        assert!(value.peak_value_units > 0);

        let mut output_limits = limits();
        output_limits.max_output_units = 0;
        let output = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![bool_value(true), bool_value(true)],
                limits: output_limits,
            },
        )
        .unwrap();
        assert_eq!(
            output.termination,
            ExecutionTermination::ResourceLimit(ResourceKind::OutputUnits)
        );
        assert_eq!(output.instruction_count, 1);
        assert_eq!(output.fuel_used, 2);
    }

    #[test]
    fn lowering_failures_are_preserved_and_runtime_faults_are_internal() {
        let fixture = bool_fixture(Opcode::IntAddChecked);
        let error = execute_function(
            fixture.input(),
            ExecutionRequest {
                inputs: vec![bool_value(true), bool_value(true)],
                limits: limits(),
            },
        )
        .unwrap_err();
        let ExecutionError::Lowering(LoweringError::Lower(error)) = error else {
            panic!("lowering failure");
        };
        assert_eq!(error.code(), crate::LowerErrorCode::OpcodeUnsupported);

        let mut runtime = Runtime {
            registers: vec![None],
            block: 0,
            instruction_count: 0,
            fuel_used: 0,
            live_value_units: 1,
            peak_value_units: 1,
            cells: Vec::new(),
            max_call_depth: MAX_CALL_DEPTH,
            peak_call_depth: 1,
        };
        let blocks = vec![crate::BytecodeBlock {
            slot: 0,
            parameter_registers: Vec::new(),
            instructions: Vec::new(),
            terminator: BytecodeTerminator::Return(0),
            reachability: 1,
        }];
        let fixture = bool_fixture(Opcode::BoolAnd);
        let input = fixture.input();
        let source = ExecutionSource::lowering(&input);
        assert!(
            run(
                &mut runtime,
                limits(),
                &blocks,
                &TypeExpr::Bool,
                &[TypeExpr::Bool],
                &source,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn execution_codes_are_stable() {
        assert_eq!(ExecutionErrorCode::InputCountMismatch.numeric(), 27_000);
        assert_eq!(ExecutionErrorCode::InputTypeMismatch.numeric(), 27_001);
        assert_eq!(ExecutionErrorCode::InputNotCanonical.numeric(), 27_006);
        let errors = [
            ExecutionErrorCode::InputCountMismatch,
            ExecutionErrorCode::InputTypeMismatch,
            ExecutionErrorCode::InputNotCanonical,
        ];
        let statuses = [
            ExecutionStatusCode::ResourceLimit,
            ExecutionStatusCode::Cancelled,
            ExecutionStatusCode::Trap,
            ExecutionStatusCode::InternalInvariant,
        ];
        for (offset, status) in statuses.into_iter().enumerate() {
            assert_eq!(status.numeric(), 27_002 + u32::try_from(offset).unwrap());
        }
        // Pre-execution failures and terminations publish into one numeric
        // space (`VM_EXECUTION_PROFILE_V1.md` section 12), so a code added to
        // either enum must not take a number or symbol the other already
        // holds. Extending one enum alone cannot notice the clash.
        let published: Vec<(u32, &str)> = errors
            .into_iter()
            .map(|code| (code.numeric(), code.as_str()))
            .chain(
                statuses
                    .into_iter()
                    .map(|code| (code.numeric(), code.as_str())),
            )
            .collect();
        let numbers: BTreeSet<u32> = published.iter().map(|(numeric, _)| *numeric).collect();
        assert_eq!(
            numbers.len(),
            published.len(),
            "reused execution code number"
        );
        let symbols: BTreeSet<&str> = published.iter().map(|(_, symbol)| *symbol).collect();
        assert_eq!(
            symbols.len(),
            published.len(),
            "reused execution code symbol"
        );
    }
}
