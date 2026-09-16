//! Pure native test execution. Host memory and elapsed time are measured by
//! the separate runner; these deterministic observations confer no admission.

use core::fmt;
use sley_check::effects::EffectValidationError;
use sley_id::NativeExecutionProfileId;
use sley_mutate::{ConstValueByteMeasure, measure_const_value_bounded};
use sley_scb1::ScbErrorCode;
use sley_ssmc::{
    CapabilityRequirement, ConstValue, EffectDefinition, fingerprint::hash_validated_value,
};

use crate::{
    CacheProfile, ExecutionError, ExecutionLimits, ExecutionRequest, ExecutionStatusCode,
    ExecutionTermination, LoweringInput, ResourceKind,
    execute::{execute_native_core, validate_inputs},
    lower_function,
};

mod purity;
mod records;

use records::ObservationFacts;
pub use records::{
    NativeDeclaredLimits, NativeExecutionObservationV1, NativeImplementationLimits,
    NativeObservedTermination, NativeResourceKind, ParsedNativeObservation, profile_id,
    profile_record, profile_stored_bytes,
};

/// Complete pure-owner inventories for one native execution.
pub struct NativeExecutionInput<'a> {
    /// Existing integrated lowering input, including the target in functions.
    pub lowering: LoweringInput<'a>,
    /// Complete effect definitions.
    pub effects: &'a [EffectDefinition],
    /// Complete capability requirements.
    pub requirements: &'a [CapabilityRequirement],
}

/// Literal caller data; constructing this request is not admission evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeExecutionRequestV1 {
    /// Ordered canonical arguments.
    pub inputs: Vec<ConstValue>,
    /// Exact `TestCase` limits, without conversions or clamping.
    pub declared_limits: NativeDeclaredLimits,
    /// Separately bound implementation ceilings.
    pub implementation_limits: NativeImplementationLimits,
    /// Immutable native execution rules.
    pub profile_id: NativeExecutionProfileId,
}

/// Stable native profile rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeExecutionProfileError {
    /// Unsupported native profile or effectful target/contract predicate.
    Unsupported,
    /// Inconsistent supplied ownership inventories.
    InvalidContext,
}

impl NativeExecutionProfileError {
    /// Frozen numeric owner code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::Unsupported => 29_200,
            Self::InvalidContext => 29_208,
        }
    }
    /// Frozen owner symbol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "NATIVE_TEST_PROFILE_UNSUPPORTED",
            Self::InvalidContext => "NATIVE_TEST_CONTEXT_MISMATCH",
        }
    }
}

/// A failure before an observed native execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeExecutionError {
    /// Earlier owner error retained exactly.
    Preserved(ExecutionError),
    /// Earlier static effect-program error retained exactly.
    Effect(EffectValidationError),
    /// Native profile or context refusal.
    Profile(NativeExecutionProfileError),
}

impl fmt::Display for NativeExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preserved(e) => e.fmt(f),
            Self::Effect(e) => e.fmt(f),
            Self::Profile(e) => f.write_str(e.as_str()),
        }
    }
}
impl std::error::Error for NativeExecutionError {}
impl From<ExecutionError> for NativeExecutionError {
    fn from(value: ExecutionError) -> Self {
        Self::Preserved(value)
    }
}

/// Value-bearing native runtime result, prior to test comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeExecutionTermination {
    /// Validated value returned by the shared VM.
    Success(ConstValue),
    /// A deterministic limit stopped execution.
    ResourceLimit(NativeResourceKind),
    /// Explicit program trap, never a host/resource failure.
    Trap {
        /// Frozen trap code 1 through 4.
        trap_tag: u32,
        /// Optional validated payload.
        payload: Option<ConstValue>,
    },
    /// A runtime invariant failed.
    InternalInvariant,
}

/// Built only by VM execution. Parsed reports cannot construct this evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeExecutionOutcome {
    termination: NativeExecutionTermination,
    observation: NativeExecutionObservationV1,
}
impl NativeExecutionOutcome {
    /// Returns the actual value-bearing termination.
    #[must_use]
    pub const fn termination(&self) -> &NativeExecutionTermination {
        &self.termination
    }
    /// Returns the deterministic execution facts.
    #[must_use]
    pub const fn observation(&self) -> &NativeExecutionObservationV1 {
        &self.observation
    }
    /// Consumes the outcome without allowing construction of runtime evidence.
    #[must_use]
    pub fn into_parts(self) -> (NativeExecutionTermination, NativeExecutionObservationV1) {
        (self.termination, self.observation)
    }
}

/// Executes under native deterministic limits through the existing VM.
///
/// # Errors
/// Preserves integrated lowering/input/effect failures. This API does not
/// verify `TestCase` replay/expected values or host memory/time enforcement.
// N0 freezes this request by value; do not change to a reference to silence lint.
#[allow(clippy::needless_pass_by_value)]
pub fn execute_native_function(
    input: NativeExecutionInput<'_>,
    request: NativeExecutionRequestV1,
) -> Result<NativeExecutionOutcome, NativeExecutionError> {
    admit_profile(&input, &request)?;
    let lowering = input.lowering;
    let lowered = lower_function(lowering).map_err(ExecutionError::from)?;
    let limits = execution_limits(&request);
    let old_request = ExecutionRequest {
        inputs: request.inputs,
        limits,
    };
    let validated = validate_inputs(&lowering, &old_request)?;
    purity::validate(&input)?;
    let required = records::observation_capacity_required(
        validated.hashes.len(),
        request.declared_limits,
        request.implementation_limits,
    )
    .map_err(|_| report_resource_error())?;
    if required > request.implementation_limits.max_report_bytes {
        return Err(report_resource_error());
    }
    let depth = usize::try_from(
        request
            .declared_limits
            .call_depth
            .min(request.implementation_limits.max_call_depth),
    )
    .map_err(|_| profile_error(NativeExecutionProfileError::Unsupported))?;
    let result = execute_native_core(&lowering, &lowered, &validated, old_request, depth);
    let (termination, observed, output_bytes_counted) = admit_output(
        &lowering,
        result.termination,
        request.declared_limits.output_bytes,
    )?;
    let observation = NativeExecutionObservationV1::build(ObservationFacts {
        schema_epoch: lowering.schema_epoch,
        field_schema_hash: crate::SSMC1_FIELD_SCHEMA_HASH,
        decoder_limits_hash: crate::SSMC1_DECODER_LIMITS_HASH,
        state_root: lowering.state_root,
        function: lowering.function.entity_id,
        cache_key: lowered.cache_key,
        input_hashes: validated.hashes,
        declared_limits: request.declared_limits,
        implementation_limits: request.implementation_limits,
        termination: observed,
        instruction_count: result.instruction_count,
        fuel_used: result.fuel_used,
        peak_value_units: result.peak_value_units,
        output_bytes_counted,
        peak_call_depth: u64::try_from(result.peak_call_depth).unwrap_or(u64::MAX),
    })
    .map_err(|_| {
        NativeExecutionError::Preserved(ExecutionError::Status(
            ExecutionStatusCode::InternalInvariant,
        ))
    })?;
    Ok(NativeExecutionOutcome {
        termination,
        observation,
    })
}

fn profile_error(error: NativeExecutionProfileError) -> NativeExecutionError {
    NativeExecutionError::Profile(error)
}
fn report_resource_error() -> NativeExecutionError {
    NativeExecutionError::Preserved(ExecutionError::Status(ExecutionStatusCode::ResourceLimit))
}
fn admit_profile(
    input: &NativeExecutionInput<'_>,
    request: &NativeExecutionRequestV1,
) -> Result<(), NativeExecutionError> {
    if request.profile_id != profile_id()
        || input.lowering.profile != CacheProfile::EXTENDED_V1
        || !request.implementation_limits.within_hard_maxima()
        || request.inputs.len() > 65_535
    {
        return Err(profile_error(NativeExecutionProfileError::Unsupported));
    }
    Ok(())
}
fn execution_limits(request: &NativeExecutionRequestV1) -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: request.implementation_limits.max_instructions,
        max_fuel: request.declared_limits.fuel,
        max_value_units: request.implementation_limits.max_value_units,
        max_output_units: request.implementation_limits.max_output_units,
        cancel_at_fuel: None,
    }
}

type OutputAdmission = (NativeExecutionTermination, NativeObservedTermination, u64);

// Post-run output validation mirrors the existing VM's `encode_termination`: a
// type/hashability/fingerprint failure stays a preserved owner `Err`, not an
// observed `InternalInvariant`, so N1 keeps the exact leaf code/symbol. Only
// codec/hard-limit outcomes become observed `OutputEncoding`/counter 0 and only
// fully valid oversized encodings become `OutputBytes`/cap+1, per N0 section 4.
fn admit_output(
    input: &LoweringInput<'_>,
    termination: ExecutionTermination,
    cap: u64,
) -> Result<OutputAdmission, NativeExecutionError> {
    let output = match &termination {
        ExecutionTermination::Success(value) => Some(value),
        ExecutionTermination::Trap { payload, .. } => payload.as_ref(),
        _ => None,
    };
    let mut count = 0;
    let mut value_hash = None;
    if let Some(value) = output {
        input
            .types
            .check_constant(value)
            .map_err(ExecutionError::from)?;
        input
            .types
            .require_hashable(&value.value_type)
            .map_err(ExecutionError::from)?;
        match measure_const_value_bounded(value, cap) {
            Ok(ConstValueByteMeasure::Exact(bytes)) => count = bytes,
            Ok(ConstValueByteMeasure::OverLimit) => {
                let Some(actual) = cap.checked_add(1) else {
                    return Ok(invariant_output());
                };
                return Ok(resource_output(NativeResourceKind::OutputBytes, actual));
            }
            Err(error) if error.code() == ScbErrorCode::ResourceLimit => {
                return Ok(resource_output(NativeResourceKind::OutputEncoding, 0));
            }
            Err(_) => return Ok(invariant_output()),
        }
        value_hash =
            Some(hash_validated_value(input.schema_epoch, value).map_err(ExecutionError::from)?);
    }
    let (termination, observed) = match termination {
        ExecutionTermination::Success(value) => {
            let Some(hash) = value_hash else {
                return Ok(invariant_output());
            };
            (
                NativeExecutionTermination::Success(value),
                NativeObservedTermination::Success(hash),
            )
        }
        ExecutionTermination::Trap { trap_tag, payload } if (1..=4).contains(&trap_tag) => (
            NativeExecutionTermination::Trap { trap_tag, payload },
            NativeObservedTermination::Trap {
                trap_tag,
                payload: value_hash,
            },
        ),
        ExecutionTermination::ResourceLimit(kind) => {
            let kind = match kind {
                ResourceKind::Instruction => NativeResourceKind::Instructions,
                ResourceKind::Fuel => NativeResourceKind::Fuel,
                ResourceKind::ValueUnits => NativeResourceKind::ValueUnits,
                ResourceKind::OutputUnits => NativeResourceKind::OutputUnits,
                ResourceKind::CallDepth => NativeResourceKind::CallDepth,
            };
            return Ok(resource_output(kind, 0));
        }
        _ => return Ok(invariant_output()),
    };
    Ok((termination, observed, count))
}
fn resource_output(kind: NativeResourceKind, count: u64) -> OutputAdmission {
    (
        NativeExecutionTermination::ResourceLimit(kind),
        NativeObservedTermination::ResourceLimit(kind),
        count,
    )
}
fn invariant_output() -> OutputAdmission {
    (
        NativeExecutionTermination::InternalInvariant,
        NativeObservedTermination::InternalInvariant,
        0,
    )
}
