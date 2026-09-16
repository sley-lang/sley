//! Canonical native execution profile and VM-owned observation records.

use sley_id::{
    BytecodeCacheKey, EntityId, NativeExecutionProfileId, NativeObservationId, SchemaEpochId,
    StateRoot, ValueHash,
};
use sley_scb1::{
    ScbError, ScbErrorCode, ScbValueCursor, encode_list, encode_record, encode_union, encode_uvar,
};

/// Exact literal `TestCase` resource limits, without policy clamping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeDeclaredLimits {
    /// Charged VM fuel ceiling.
    pub fuel: u64,
    /// Supervisor-enforced memory ceiling.
    pub memory_bytes: u64,
    /// Canonical returned value or trap payload bytes.
    pub output_bytes: u64,
    /// Declared effect ceiling; pure native execution observes zero.
    pub effect_count: u64,
    /// Entry counts as depth one.
    pub call_depth: u64,
    /// Supervisor-enforced elapsed-time ceiling.
    pub wall_timeout_millis: u64,
}

impl From<sley_ssmc::ResourceLimits> for NativeDeclaredLimits {
    fn from(value: sley_ssmc::ResourceLimits) -> Self {
        Self {
            fuel: value.fuel,
            memory_bytes: value.memory_bytes,
            output_bytes: value.output_bytes,
            effect_count: value.effect_count,
            call_depth: value.call_depth,
            wall_timeout_millis: value.wall_timeout_millis,
        }
    }
}

impl NativeDeclaredLimits {
    fn record(self) -> Result<Vec<u8>, ScbError> {
        integer_record(&[
            self.fuel,
            self.memory_bytes,
            self.output_bytes,
            self.effect_count,
            self.call_depth,
            self.wall_timeout_millis,
        ])
    }
}

/// Implementation safety ceilings, separately bound from `TestCase` limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeImplementationLimits {
    /// Maximum dispatched instructions.
    pub max_instructions: u64,
    /// Maximum live semantic value units.
    pub max_value_units: u64,
    /// Maximum returned semantic value units.
    pub max_output_units: u64,
    /// Maximum active call frames.
    pub max_call_depth: u64,
    /// Maximum stored observation bytes, including envelope and digest.
    pub max_report_bytes: u64,
}

impl NativeImplementationLimits {
    /// Immutable native-v1 maxima; local values may only tighten these.
    pub const HARD_MAXIMA: Self = Self {
        max_instructions: 10_000_000,
        max_value_units: 67_108_864,
        max_output_units: 67_108_864,
        max_call_depth: 256,
        max_report_bytes: 262_144,
    };

    /// Whether every field respects its native-v1 maximum. Zero is literal.
    #[must_use]
    pub const fn within_hard_maxima(self) -> bool {
        self.max_instructions <= Self::HARD_MAXIMA.max_instructions
            && self.max_value_units <= Self::HARD_MAXIMA.max_value_units
            && self.max_output_units <= Self::HARD_MAXIMA.max_output_units
            && self.max_call_depth <= Self::HARD_MAXIMA.max_call_depth
            && self.max_report_bytes <= Self::HARD_MAXIMA.max_report_bytes
    }

    fn record(self) -> Result<Vec<u8>, ScbError> {
        integer_record(&[
            self.max_instructions,
            self.max_value_units,
            self.max_output_units,
            self.max_call_depth,
            self.max_report_bytes,
        ])
    }
}

/// Closed deterministic native resource refusal tags.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeResourceKind {
    /// Instruction safety ceiling.
    Instructions,
    /// Declared fuel ceiling.
    Fuel,
    /// Live semantic value-unit ceiling.
    ValueUnits,
    /// Returned semantic value-unit ceiling.
    OutputUnits,
    /// Active call depth ceiling.
    CallDepth,
    /// Declared canonical output-byte ceiling.
    OutputBytes,
    /// Canonical codec safety ceiling.
    OutputEncoding,
}

impl NativeResourceKind {
    /// Frozen native-v1 resource union payload.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Instructions => 1,
            Self::Fuel => 2,
            Self::ValueUnits => 3,
            Self::OutputUnits => 4,
            Self::CallDepth => 5,
            Self::OutputBytes => 6,
            Self::OutputEncoding => 7,
        }
    }

    /// Inverse of [`tag`](Self::tag); unknown tags refuse at parse.
    #[must_use]
    pub const fn from_tag(tag: u32) -> Option<Self> {
        match tag {
            1 => Some(Self::Instructions),
            2 => Some(Self::Fuel),
            3 => Some(Self::ValueUnits),
            4 => Some(Self::OutputUnits),
            5 => Some(Self::CallDepth),
            6 => Some(Self::OutputBytes),
            7 => Some(Self::OutputEncoding),
            _ => None,
        }
    }
}

/// Hash-only projection of a VM-validated termination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeObservedTermination {
    /// Returned value hash.
    Success(ValueHash),
    /// Deterministic resource refusal, never an expected trap.
    ResourceLimit(NativeResourceKind),
    /// Frozen trap tag1..4 and optional validated payload hash.
    Trap {
        /// Frozen trap code 1 through 4.
        trap_tag: u32,
        /// Optional validated payload hash.
        payload: Option<ValueHash>,
    },
    /// Internal execution invariant failed.
    InternalInvariant,
}

impl NativeObservedTermination {
    fn encoded(&self) -> Result<Vec<u8>, ScbError> {
        match self {
            Self::Success(value) => encode_union(1, value.as_bytes()),
            Self::ResourceLimit(resource) => {
                encode_union(2, &encode_uvar(u64::from(resource.tag())))
            }
            Self::Trap { trap_tag, payload } => {
                if !(1..=4).contains(trap_tag) {
                    return Err(ScbError::new(ScbErrorCode::UnionInvalid));
                }
                let payload = match payload {
                    Some(hash) => encode_union(1, hash.as_bytes())?,
                    None => encode_union(0, &[])?,
                };
                encode_union(
                    3,
                    &encode_record(&[(1, encode_uvar(u64::from(*trap_tag))), (2, payload)])?,
                )
            }
            Self::InternalInvariant => encode_union(4, &[]),
        }
    }
}

/// Only the native VM parent may supply observed execution facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ObservationFacts {
    pub schema_epoch: SchemaEpochId,
    pub field_schema_hash: [u8; 32],
    pub decoder_limits_hash: [u8; 32],
    pub state_root: StateRoot,
    pub function: EntityId,
    pub cache_key: BytecodeCacheKey,
    pub input_hashes: Vec<ValueHash>,
    pub declared_limits: NativeDeclaredLimits,
    pub implementation_limits: NativeImplementationLimits,
    pub termination: NativeObservedTermination,
    pub instruction_count: u64,
    pub fuel_used: u64,
    pub peak_value_units: u64,
    pub output_bytes_counted: u64,
    pub peak_call_depth: u64,
}

/// Immutable canonical runtime observation; external callers cannot construct it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeExecutionObservationV1 {
    facts: ObservationFacts,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: NativeObservationId,
}

impl NativeExecutionObservationV1 {
    pub(super) fn build(facts: ObservationFacts) -> Result<Self, ScbError> {
        let count = u64::try_from(facts.input_hashes.len()).map_err(|_| resource_error())?;
        let mut fields = fields_without_inputs(&facts)?;
        let (record_len, stored_len) = encoded_lengths(count, &fields)?;
        if stored_len > facts.implementation_limits.max_report_bytes {
            return Err(resource_error());
        }
        let inputs = facts
            .input_hashes
            .iter()
            .map(|hash| hash.as_bytes().to_vec())
            .collect::<Vec<_>>();
        fields.insert(8, (9, encode_list(&inputs)?));
        let record = encode_record(&fields)?;
        debug_assert_eq!(record.len() as u64, record_len);
        let mut stored = preimage(*b"SLEYNOB1", &record);
        let id = NativeObservationId::derive(&stored);
        stored.extend_from_slice(id.as_bytes());
        debug_assert_eq!(stored.len() as u64, stored_len);
        Ok(Self {
            facts,
            record,
            stored,
            id,
        })
    }

    /// Canonical SCB1 record without envelope or trailer.
    #[must_use]
    pub fn record(&self) -> &[u8] {
        &self.record
    }
    /// Complete canonical envelope and digest trailer.
    #[must_use]
    pub fn stored_bytes(&self) -> &[u8] {
        &self.stored
    }
    /// Native observation identity, distinct from old VM observations.
    #[must_use]
    pub const fn observation_id(&self) -> NativeObservationId {
        self.id
    }
    /// Bound schema epoch.
    #[must_use]
    pub const fn schema_epoch(&self) -> SchemaEpochId {
        self.facts.schema_epoch
    }
    /// Bound field schema digest.
    #[must_use]
    pub const fn field_schema_hash(&self) -> &[u8; 32] {
        &self.facts.field_schema_hash
    }
    /// Bound decoder limits digest.
    #[must_use]
    pub const fn decoder_limits_hash(&self) -> &[u8; 32] {
        &self.facts.decoder_limits_hash
    }
    /// Exact proposed root.
    #[must_use]
    pub const fn state_root(&self) -> StateRoot {
        self.facts.state_root
    }
    /// Target entity identity.
    #[must_use]
    pub const fn function(&self) -> EntityId {
        self.facts.function
    }
    /// Unchanged extended lowering cache key.
    #[must_use]
    pub const fn cache_key(&self) -> BytecodeCacheKey {
        self.facts.cache_key
    }
    /// Immutable native execution rules bound into this observation.
    #[must_use]
    pub fn native_execution_profile(&self) -> NativeExecutionProfileId {
        profile_id()
    }
    /// Ordered validated inputs, never sorted or deduplicated.
    #[must_use]
    pub fn input_hashes(&self) -> &[ValueHash] {
        &self.facts.input_hashes
    }
    /// Literal `TestCase` limits.
    #[must_use]
    pub const fn declared_limits(&self) -> NativeDeclaredLimits {
        self.facts.declared_limits
    }
    /// Exact local implementation ceilings.
    #[must_use]
    pub const fn implementation_limits(&self) -> NativeImplementationLimits {
        self.facts.implementation_limits
    }
    /// Validated hash-only termination projection.
    #[must_use]
    pub const fn termination(&self) -> &NativeObservedTermination {
        &self.facts.termination
    }
    /// Dispatched instruction count.
    #[must_use]
    pub const fn instruction_count(&self) -> u64 {
        self.facts.instruction_count
    }
    /// Charged fuel consumed.
    #[must_use]
    pub const fn fuel_used(&self) -> u64 {
        self.facts.fuel_used
    }
    /// Peak live semantic units.
    #[must_use]
    pub const fn peak_value_units(&self) -> u64 {
        self.facts.peak_value_units
    }
    /// Exact sized bytes or mandatory declared-cap-plus-one overflow marker.
    #[must_use]
    pub const fn output_bytes_counted(&self) -> u64 {
        self.facts.output_bytes_counted
    }
    /// Peak active call depth.
    #[must_use]
    pub const fn peak_call_depth(&self) -> u64 {
        self.facts.peak_call_depth
    }
    /// Native-v1 pure execution performs no effects.
    #[must_use]
    pub const fn effect_count(&self) -> u64 {
        0
    }

    /// Strictly parses one stored observation envelope.
    ///
    /// Checks the report size bound, magic, version, digest, exact 18-field
    /// shape, the immutable profile binding, hard-maxima limits, termination
    /// ranges, and the literal zero effect count, in that order. Parsing
    /// proves bytes for test comparison; it never constructs a runtime
    /// outcome, which only VM execution builds.
    ///
    /// # Errors
    /// Returns the first stable SCB1 failure encountered while decoding the
    /// envelope, verifying its digest, or validating fields in tag order.
    pub fn parse_stored(stored: &[u8]) -> Result<ParsedNativeObservation, ScbError> {
        if u64::try_from(stored.len()).map_err(|_| resource_error())?
            > NativeImplementationLimits::HARD_MAXIMA.max_report_bytes
        {
            return Err(resource_error());
        }
        let mut cursor = ScbValueCursor::new(stored)?;
        if cursor.read_exact_bytes(8)? != *b"SLEYNOB1" {
            return Err(ScbError::new(ScbErrorCode::MagicInvalid));
        }
        if cursor.read_uvar(64)? != 1 {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        let record = cursor.read_bytes()?.to_vec();
        let trailer = cursor.read_exact_bytes(32)?;
        cursor.check_finished()?;
        let id = NativeObservationId::derive(preimage(*b"SLEYNOB1", &record));
        if trailer != id.as_bytes() {
            return Err(ScbError::new(ScbErrorCode::DigestMismatch));
        }
        let fields = decode_record_fields(&record)?;
        expect_record_tags(
            &fields,
            &[
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
            ],
        )?;
        if read_field_uvar(&fields[0].1, 32)? != 1 {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        let _schema_epoch = SchemaEpochId::from_bytes(read_field_id(&fields[1].1)?);
        let _field_schema_hash = read_field_id(&fields[2].1)?;
        let _decoder_limits_hash = read_field_id(&fields[3].1)?;
        let _state_root = StateRoot::from_bytes(read_field_id(&fields[4].1)?);
        let _function = EntityId::from_bytes(read_field_id(&fields[5].1)?);
        let _cache_key = BytecodeCacheKey::from_bytes(read_field_id(&fields[6].1)?);
        if read_field_id(&fields[7].1)? != *profile_id().as_bytes() {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        let _input_hashes = read_hash_list(&fields[8].1)?;
        let _declared = parse_declared_record(&fields[9].1)?;
        let implementation = parse_implementation_record(&fields[10].1)?;
        if !implementation.within_hard_maxima() {
            return Err(resource_error());
        }
        let termination = parse_termination(&fields[11].1)?;
        let _instruction_count = read_field_uvar(&fields[12].1, 64)?;
        let _fuel_used = read_field_uvar(&fields[13].1, 64)?;
        let _peak_value_units = read_field_uvar(&fields[14].1, 64)?;
        let _output_bytes_counted = read_field_uvar(&fields[15].1, 64)?;
        let _peak_call_depth = read_field_uvar(&fields[16].1, 64)?;
        if read_field_uvar(&fields[17].1, 64)? != 0 {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        Ok(ParsedNativeObservation { termination, id })
    }
}

/// Strictly parsed native observation; proves bytes, never runtime execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedNativeObservation {
    termination: NativeObservedTermination,
    id: NativeObservationId,
}

impl ParsedNativeObservation {
    /// Validated hash-only termination projection for test comparison.
    #[must_use]
    pub const fn termination(&self) -> &NativeObservedTermination {
        &self.termination
    }

    /// Observation identity verified against the envelope trailer.
    #[must_use]
    pub const fn observation_id(&self) -> NativeObservationId {
        self.id
    }
}

/// Record field loop shared with the test owner shape checks. The VM cannot
/// depend on `sley-tests`, so this mirrors that owner's strict loop instead
/// of importing it; any divergence fails the cross-crate pin test.
fn decode_record_fields(record: &[u8]) -> Result<Vec<(u32, Vec<u8>)>, ScbError> {
    let mut cursor = ScbValueCursor::new(record)?;
    let count = cursor.read_record_field_count()?;
    let mut fields = Vec::new();
    let mut previous: Option<u32> = None;
    for _ in 0..count {
        let tag = cursor.read_uvar(32)?;
        let tag = u32::try_from(tag).map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
        if let Some(previous) = previous {
            if tag == previous {
                return Err(ScbError::new(ScbErrorCode::FieldDuplicate));
            }
            if tag < previous {
                return Err(ScbError::new(ScbErrorCode::FieldOrder));
            }
        }
        let value = cursor.read_sized_payload()?.to_vec();
        previous = Some(tag);
        fields.push((tag, value));
    }
    cursor.check_finished()?;
    Ok(fields)
}

fn expect_record_tags(fields: &[(u32, Vec<u8>)], expected: &[u32]) -> Result<(), ScbError> {
    let mut index = 0;
    for (tag, _) in fields {
        match expected.get(index) {
            Some(want) if want == tag => index += 1,
            Some(want) if want < tag => {
                return Err(ScbError::new(ScbErrorCode::FieldMissing));
            }
            _ => return Err(ScbError::new(ScbErrorCode::FieldUnknown)),
        }
    }
    if index == expected.len() {
        Ok(())
    } else {
        Err(ScbError::new(ScbErrorCode::FieldMissing))
    }
}

fn read_field_id(value: &[u8]) -> Result<[u8; 32], ScbError> {
    <[u8; 32]>::try_from(value).map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))
}

fn read_field_uvar(value: &[u8], width: u8) -> Result<u64, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let parsed = cursor.read_uvar(width)?;
    cursor.check_finished()?;
    Ok(parsed)
}

fn read_hash_list(value: &[u8]) -> Result<Vec<ValueHash>, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_list_count()?;
    if count > 65_535 {
        return Err(resource_error());
    }
    let mut hashes = Vec::new();
    for _ in 0..count {
        hashes.push(ValueHash::from_bytes(read_field_id(cursor.read_bytes()?)?));
    }
    cursor.check_finished()?;
    Ok(hashes)
}

fn parse_declared_record(value: &[u8]) -> Result<NativeDeclaredLimits, ScbError> {
    let fields = decode_record_fields(value)?;
    expect_record_tags(&fields, &[1, 2, 3, 4, 5, 6])?;
    Ok(NativeDeclaredLimits {
        fuel: read_field_uvar(&fields[0].1, 64)?,
        memory_bytes: read_field_uvar(&fields[1].1, 64)?,
        output_bytes: read_field_uvar(&fields[2].1, 64)?,
        effect_count: read_field_uvar(&fields[3].1, 64)?,
        call_depth: read_field_uvar(&fields[4].1, 64)?,
        wall_timeout_millis: read_field_uvar(&fields[5].1, 64)?,
    })
}

fn parse_implementation_record(value: &[u8]) -> Result<NativeImplementationLimits, ScbError> {
    let fields = decode_record_fields(value)?;
    expect_record_tags(&fields, &[1, 2, 3, 4, 5])?;
    Ok(NativeImplementationLimits {
        max_instructions: read_field_uvar(&fields[0].1, 64)?,
        max_value_units: read_field_uvar(&fields[1].1, 64)?,
        max_output_units: read_field_uvar(&fields[2].1, 64)?,
        max_call_depth: read_field_uvar(&fields[3].1, 64)?,
        max_report_bytes: read_field_uvar(&fields[4].1, 64)?,
    })
}

fn parse_termination(value: &[u8]) -> Result<NativeObservedTermination, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let (tag, payload) = cursor.read_union()?;
    cursor.check_finished()?;
    match tag {
        1 => Ok(NativeObservedTermination::Success(ValueHash::from_bytes(
            read_field_id(payload)?,
        ))),
        2 => {
            let raw = read_union_uvar(payload, 32)?;
            let tag =
                u32::try_from(raw).map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
            let kind = NativeResourceKind::from_tag(tag)
                .ok_or_else(|| ScbError::new(ScbErrorCode::UnionInvalid))?;
            Ok(NativeObservedTermination::ResourceLimit(kind))
        }
        3 => {
            let fields = decode_record_fields(payload)?;
            expect_record_tags(&fields, &[1, 2])?;
            let trap_tag = u32::try_from(read_field_uvar(&fields[0].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
            if !(1..=4).contains(&trap_tag) {
                return Err(ScbError::new(ScbErrorCode::UnionInvalid));
            }
            let mut option = ScbValueCursor::new(&fields[1].1)?;
            let (present, bytes) = option.read_union()?;
            option.check_finished()?;
            let payload = match present {
                0 => {
                    if bytes.is_empty() {
                        None
                    } else {
                        return Err(ScbError::new(ScbErrorCode::UnionInvalid));
                    }
                }
                1 => Some(ValueHash::from_bytes(read_field_id(bytes)?)),
                _ => return Err(ScbError::new(ScbErrorCode::UnionInvalid)),
            };
            Ok(NativeObservedTermination::Trap { trap_tag, payload })
        }
        4 => {
            if payload.is_empty() {
                Ok(NativeObservedTermination::InternalInvariant)
            } else {
                Err(ScbError::new(ScbErrorCode::UnionInvalid))
            }
        }
        _ => Err(ScbError::new(ScbErrorCode::UnionInvalid)),
    }
}

fn read_union_uvar(payload: &[u8], width: u8) -> Result<u64, ScbError> {
    let mut cursor = ScbValueCursor::new(payload)?;
    let parsed = cursor.read_uvar(width)?;
    cursor.check_finished()?;
    Ok(parsed)
}

/// Conservative stored-observation reservation before any VM execution.
///
/// This does not include the outer execution report; the test owner reserves
/// that wrapper separately. The five dynamic counters use `u64::MAX` and the
/// termination uses the longest allowed Trap with a present payload hash.
///
/// # Errors
/// Returns `SCB_RESOURCE_LIMIT` for invalid hard limits or an excessive input count.
pub fn observation_capacity_required(
    input_count: usize,
    declared_limits: NativeDeclaredLimits,
    implementation_limits: NativeImplementationLimits,
) -> Result<u64, ScbError> {
    let facts = ObservationFacts {
        schema_epoch: SchemaEpochId::from_bytes([0; 32]),
        field_schema_hash: [0; 32],
        decoder_limits_hash: [0; 32],
        state_root: StateRoot::from_bytes([0; 32]),
        function: EntityId::from_bytes([0; 32]),
        cache_key: BytecodeCacheKey::from_bytes([0; 32]),
        input_hashes: Vec::new(),
        declared_limits,
        implementation_limits,
        termination: NativeObservedTermination::Trap {
            trap_tag: 4,
            payload: Some(ValueHash::from_bytes([0; 32])),
        },
        instruction_count: u64::MAX,
        fuel_used: u64::MAX,
        peak_value_units: u64::MAX,
        output_bytes_counted: u64::MAX,
        peak_call_depth: u64::MAX,
    };
    let fields = fields_without_inputs(&facts)?;
    let count = u64::try_from(input_count).map_err(|_| resource_error())?;
    encoded_lengths(count, &fields).map(|(_, stored)| stored)
}

fn fields_without_inputs(facts: &ObservationFacts) -> Result<Vec<(u32, Vec<u8>)>, ScbError> {
    if !facts.implementation_limits.within_hard_maxima() {
        return Err(resource_error());
    }
    Ok(vec![
        (1, encode_uvar(1)),
        (2, facts.schema_epoch.as_bytes().to_vec()),
        (3, facts.field_schema_hash.to_vec()),
        (4, facts.decoder_limits_hash.to_vec()),
        (5, facts.state_root.as_bytes().to_vec()),
        (6, facts.function.as_bytes().to_vec()),
        (7, facts.cache_key.as_bytes().to_vec()),
        (8, profile_id().as_bytes().to_vec()),
        (10, facts.declared_limits.record()?),
        (11, facts.implementation_limits.record()?),
        (12, facts.termination.encoded()?),
        (13, encode_uvar(facts.instruction_count)),
        (14, encode_uvar(facts.fuel_used)),
        (15, encode_uvar(facts.peak_value_units)),
        (16, encode_uvar(facts.output_bytes_counted)),
        (17, encode_uvar(facts.peak_call_depth)),
        (18, encode_uvar(0)),
    ])
}

// Lengths are exact for the same fields that encode_record consumes; only the
// fixed-width input-hash list is sized without allocation. Hard count bounds
// make all arithmetic below bounded well below u64::MAX.
fn encoded_lengths(count: u64, fields: &[(u32, Vec<u8>)]) -> Result<(u64, u64), ScbError> {
    if count > 65_535 {
        return Err(resource_error());
    }
    let inputs_len = encode_uvar(count).len() as u64 + count * 33;
    let mut record_len = 1 + 1 + encode_uvar(inputs_len).len() as u64 + inputs_len;
    for (tag, value) in fields {
        record_len += encode_uvar(u64::from(*tag)).len() as u64
            + encode_uvar(value.len() as u64).len() as u64
            + value.len() as u64;
    }
    let stored_len = 8 + 1 + encode_uvar(record_len).len() as u64 + record_len + 32;
    Ok((record_len, stored_len))
}

fn integer_record(values: &[u64]) -> Result<Vec<u8>, ScbError> {
    let mut fields = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let tag = u32::try_from(index + 1).map_err(|_| resource_error())?;
        fields.push((tag, encode_uvar(*value)));
    }
    encode_record(&fields)
}

fn resource_error() -> ScbError {
    ScbError::new(ScbErrorCode::ResourceLimit)
}

fn preimage(magic: [u8; 8], record: &[u8]) -> Vec<u8> {
    let mut out = magic.to_vec();
    out.extend_from_slice(&encode_uvar(1));
    out.extend_from_slice(&encode_uvar(record.len() as u64));
    out.extend_from_slice(record);
    out
}

// Immutable SCB1 record from NATIVE_TEST_EXECUTION_V1 section2. A test below
// independently assembles the ten fields and pins the Python-derived identity.
// Bytes are the independent golden `profile_record` from
// `native-records-golden.json` (profile preimage `SLEYNXP1`, id
// `392927d6...abb0bf`); never use an empty placeholder as evidence.
const PROFILE_RECORD: &[u8] = &[
    0x0a, 0x01, 0x01, 0x01, 0x02, 0x01, 0x02, 0x03, 0x01, 0x01, 0x04, 0x01, 0x00, 0x05, 0x01, 0x00,
    0x06, 0x01, 0x01, 0x07, 0x01, 0x01, 0x08, 0x01, 0x01, 0x09, 0x1c, 0x05, 0x01, 0x04, 0x80, 0xad,
    0xe2, 0x04, 0x02, 0x04, 0x80, 0x80, 0x80, 0x20, 0x03, 0x04, 0x80, 0x80, 0x80, 0x20, 0x04, 0x02,
    0x80, 0x02, 0x05, 0x03, 0x80, 0x80, 0x10, 0x0a, 0x01, 0x00,
];

/// Canonical immutable native-v1 execution profile record.
#[must_use]
pub const fn profile_record() -> &'static [u8] {
    PROFILE_RECORD
}

/// Identity derived from the exact immutable native-v1 execution profile.
#[must_use]
pub fn profile_id() -> NativeExecutionProfileId {
    NativeExecutionProfileId::derive(preimage(*b"SLEYNXP1", PROFILE_RECORD))
}

/// Complete immutable native-v1 profile envelope and digest trailer.
#[must_use]
pub fn profile_stored_bytes() -> Vec<u8> {
    let mut stored = preimage(*b"SLEYNXP1", PROFILE_RECORD);
    stored.extend_from_slice(profile_id().as_bytes());
    stored
}

#[cfg(test)]
mod tests {
    use super::*;
    use sley_scb1::encode_bool;

    const GOLDEN_PROFILE_RECORD: &str = "0a010101020102030101040100050100060101070101080101091c05010480ade2040204808080200304808080200402800205038080100a0100";
    const GOLDEN_PROFILE_PREIMAGE: &str = "534c45594e585031013a0a010101020102030101040100050100060101070101080101091c05010480ade2040204808080200304808080200402800205038080100a0100";
    const GOLDEN_PROFILE_ID: &str =
        "392927d6c948b9a03a27b87f26ebfe08c65ddf4e831a18e1d82f02dc4abb0bf4";
    const GOLDEN_OBSERVATION_RECORD: &str = "120101010220010101010101010101010101010101010101010101010101010101010101010103200202020202020202020202020202020202020202020202020202020202020202042003030303030303030303030303030303030303030303030303030303030303030520040404040404040404040404040404040404040404040404040404040404040406200505050505050505050505050505050505050505050505050505050505050505072006060606060606060606060606060606060606060606060606060606060606060820392927d6c948b9a03a27b87f26ebfe08c65ddf4e831a18e1d82f02dc4abb0bf40943022007070707070707070707070707070707070707070707070707070707070707072008080808080808080808080808080808080808080808080808080808080808080a130601010a02011403011e0401000501280601320b1c05010480ade2040204808080200304808080200402800205038080100c22012009090909090909090909090909090909090909090909090909090909090909090d010b0e010c0f010d10010e11010f120100";
    const GOLDEN_OBSERVATION_STORED: &str = "534c45594e4f423101a003120101010220010101010101010101010101010101010101010101010101010101010101010103200202020202020202020202020202020202020202020202020202020202020202042003030303030303030303030303030303030303030303030303030303030303030520040404040404040404040404040404040404040404040404040404040404040406200505050505050505050505050505050505050505050505050505050505050505072006060606060606060606060606060606060606060606060606060606060606060820392927d6c948b9a03a27b87f26ebfe08c65ddf4e831a18e1d82f02dc4abb0bf40943022007070707070707070707070707070707070707070707070707070707070707072008080808080808080808080808080808080808080808080808080808080808080a130601010a02011403011e0401000501280601320b1c05010480ade2040204808080200304808080200402800205038080100c22012009090909090909090909090909090909090909090909090909090909090909090d010b0e010c0f010d10010e11010f120100ffa9f168c870cf82d059559f82d0dd7210875faba6e8828e2fa17a7ede6317e8";
    const GOLDEN_OBSERVATION_ID: &str =
        "ffa9f168c870cf82d059559f82d0dd7210875faba6e8828e2fa17a7ede6317e8";

    fn decode_hex(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        let bytes = hex.as_bytes();
        let mut out = Vec::with_capacity(hex.len() / 2);
        for chunk in bytes.chunks_exact(2) {
            let text = core::str::from_utf8(chunk).expect("hex is ascii");
            out.push(u8::from_str_radix(text, 16).expect("hex digits"));
        }
        out
    }

    fn golden_facts() -> ObservationFacts {
        ObservationFacts {
            schema_epoch: SchemaEpochId::from_bytes([1; 32]),
            field_schema_hash: [2; 32],
            decoder_limits_hash: [3; 32],
            state_root: StateRoot::from_bytes([4; 32]),
            function: EntityId::from_bytes([5; 32]),
            cache_key: BytecodeCacheKey::from_bytes([6; 32]),
            input_hashes: vec![
                ValueHash::from_bytes([7; 32]),
                ValueHash::from_bytes([8; 32]),
            ],
            declared_limits: NativeDeclaredLimits {
                fuel: 10,
                memory_bytes: 20,
                output_bytes: 30,
                effect_count: 0,
                call_depth: 40,
                wall_timeout_millis: 50,
            },
            implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
            termination: NativeObservedTermination::Success(ValueHash::from_bytes([9; 32])),
            instruction_count: 11,
            fuel_used: 12,
            peak_value_units: 13,
            output_bytes_counted: 14,
            peak_call_depth: 15,
        }
    }

    #[test]
    fn profile_record_matches_independent_golden() {
        let limits = encode_record(&[
            (1, encode_uvar(10_000_000)),
            (2, encode_uvar(67_108_864)),
            (3, encode_uvar(67_108_864)),
            (4, encode_uvar(256)),
            (5, encode_uvar(262_144)),
        ])
        .expect("golden limits encode");
        let assembled = encode_record(&[
            (1, encode_uvar(1)),
            (2, encode_uvar(2)),
            (3, encode_uvar(1)),
            (4, encode_uvar(0)),
            (5, encode_uvar(0)),
            (6, encode_uvar(1)),
            (7, encode_uvar(1)),
            (8, encode_bool(true)),
            (9, limits),
            (10, encode_uvar(0)),
        ])
        .expect("golden profile encode");
        assert_eq!(assembled, PROFILE_RECORD);
        assert_eq!(assembled, decode_hex(GOLDEN_PROFILE_RECORD));
        assert_eq!(
            profile_record(),
            decode_hex(GOLDEN_PROFILE_RECORD).as_slice()
        );
        assert_eq!(
            profile_id().as_bytes(),
            decode_hex(GOLDEN_PROFILE_ID).as_slice()
        );
        let mut expected_preimage = decode_hex(GOLDEN_PROFILE_PREIMAGE);
        assert_eq!(preimage(*b"SLEYNXP1", PROFILE_RECORD), expected_preimage);
        expected_preimage.extend_from_slice(&decode_hex(GOLDEN_PROFILE_ID));
        assert_eq!(profile_stored_bytes(), expected_preimage);
    }

    #[test]
    fn observation_golden_matches_independent_python() {
        let observation =
            NativeExecutionObservationV1::build(golden_facts()).expect("golden builds");
        assert_eq!(
            observation.record(),
            decode_hex(GOLDEN_OBSERVATION_RECORD).as_slice()
        );
        assert_eq!(
            observation.stored_bytes(),
            decode_hex(GOLDEN_OBSERVATION_STORED).as_slice()
        );
        assert_eq!(
            observation.observation_id().as_bytes(),
            decode_hex(GOLDEN_OBSERVATION_ID).as_slice()
        );
        assert_eq!(observation.effect_count(), 0);
        assert_eq!(observation.native_execution_profile(), profile_id());
        let repeated = NativeExecutionObservationV1::build(golden_facts()).expect("repeat builds");
        assert_eq!(observation, repeated);
    }

    #[test]
    fn observation_preserves_input_order_and_count() {
        let mut swapped = golden_facts();
        swapped.input_hashes.reverse();
        let first = NativeExecutionObservationV1::build(golden_facts()).expect("first builds");
        let second = NativeExecutionObservationV1::build(swapped).expect("swapped builds");
        assert_ne!(first.observation_id(), second.observation_id());
        assert_eq!(
            first.input_hashes(),
            &[
                ValueHash::from_bytes([7; 32]),
                ValueHash::from_bytes([8; 32])
            ]
        );
        assert_eq!(
            second.input_hashes(),
            &[
                ValueHash::from_bytes([8; 32]),
                ValueHash::from_bytes([7; 32])
            ]
        );
        assert!(
            observation_capacity_required(
                65_536,
                first.declared_limits(),
                first.implementation_limits()
            )
            .is_err()
        );
    }

    // Independent report golden from `native-report-golden.py`: one input
    // hash, Success(value 0x65), counters 11..15. The report owner embeds
    // these exact stored bytes; parsing here proves the termination view.
    const REPORT_GOLDEN_STORED: &str = "534c45594e4f4231018103120101010220545454545454545454545454545454545454545454545454545454545454545403206161616161616161616161616161616161616161616161616161616161616161042062626262626262626262626262626262626262626262626262626262626262620520555555555555555555555555555555555555555555555555555555555555555506205151515151515151515151515151515151515151515151515151515151515151072063636363636363636363636363636363636363636363636363636363636363630820392927d6c948b9a03a27b87f26ebfe08c65ddf4e831a18e1d82f02dc4abb0bf40922012064646464646464646464646464646464646464646464646464646464646464640a1506010164020280200301400401000501080602e8070b1c05010480ade2040204808080200304808080200402800205038080100c22012065656565656565656565656565656565656565656565656565656565656565650d010b0e010c0f010d10010e11010f12010057510a52bb567b20f6c36e56e4fecfb44071a593df7b1395b6a98bf2f8c2fe44";
    const REPORT_GOLDEN_ID: &str =
        "57510a52bb567b20f6c36e56e4fecfb44071a593df7b1395b6a98bf2f8c2fe44";

    fn re_envelope_observation(record: &[u8]) -> Vec<u8> {
        let image = preimage(*b"SLEYNOB1", record);
        let id = NativeObservationId::derive(&image);
        let mut stored = image;
        stored.extend_from_slice(id.as_bytes());
        stored
    }

    #[test]
    fn parse_stored_accepts_independent_report_golden() {
        let stored = decode_hex(REPORT_GOLDEN_STORED);
        let parsed = NativeExecutionObservationV1::parse_stored(&stored).expect("golden parses");
        assert_eq!(
            parsed.termination(),
            &NativeObservedTermination::Success(ValueHash::from_bytes([0x65; 32]))
        );
        assert_eq!(
            parsed.observation_id().as_bytes(),
            decode_hex(REPORT_GOLDEN_ID).as_slice()
        );
    }

    #[test]
    fn parse_stored_refuses_tampered_profile_effect_and_termination() {
        let stored = decode_hex(REPORT_GOLDEN_STORED);
        let record_start = 8 + 1 + 2;
        let record_end = stored.len() - 32;
        let fields = decode_record_fields(&stored[record_start..record_end]).expect("fields");
        assert_eq!(fields.len(), 18);
        let tamper = |tag: u32, value: Vec<u8>| {
            let mut edited = fields.clone();
            for (field_tag, field_value) in &mut edited {
                if *field_tag == tag {
                    *field_value = value.clone();
                }
            }
            NativeExecutionObservationV1::parse_stored(&re_envelope_observation(
                &encode_record(&edited).expect("edited encodes"),
            ))
        };
        assert_eq!(
            tamper(8, vec![0x00; 32]).expect_err("profile").code(),
            ScbErrorCode::ContractUnknown
        );
        assert_eq!(
            tamper(18, encode_uvar(1)).expect_err("effects").code(),
            ScbErrorCode::ContractUnknown
        );
        assert_eq!(
            tamper(
                12,
                encode_union(2, &encode_uvar(8)).expect("resource encodes")
            )
            .expect_err("resource tag")
            .code(),
            ScbErrorCode::UnionInvalid
        );
        assert_eq!(
            tamper(
                12,
                encode_union(
                    3,
                    &encode_record(&[
                        (1, encode_uvar(5)),
                        (2, encode_union(0, &[]).expect("none"))
                    ])
                    .expect("trap encodes"),
                )
                .expect("union encodes"),
            )
            .expect_err("trap tag")
            .code(),
            ScbErrorCode::UnionInvalid
        );
        let mut digest = stored.clone();
        let last = digest.len() - 1;
        digest[last] ^= 0x01;
        assert_eq!(
            NativeExecutionObservationV1::parse_stored(&digest)
                .expect_err("digest")
                .code(),
            ScbErrorCode::DigestMismatch
        );
        assert_eq!(
            NativeExecutionObservationV1::parse_stored(&stored[..stored.len() - 1])
                .expect_err("truncated")
                .code(),
            ScbErrorCode::LengthOverflow
        );
    }

    #[test]
    fn observation_capacity_is_conservative_and_zero_refuses() {
        let facts = golden_facts();
        let required = observation_capacity_required(
            facts.input_hashes.len(),
            facts.declared_limits,
            facts.implementation_limits,
        )
        .expect("capacity computes");
        let built = NativeExecutionObservationV1::build(facts).expect("golden builds");
        assert!(required >= built.stored_bytes().len() as u64);
        let zero = NativeImplementationLimits {
            max_report_bytes: 0,
            ..NativeImplementationLimits::HARD_MAXIMA
        };
        assert!(observation_capacity_required(2, golden_facts().declared_limits, zero).is_ok());
        let mut refused = golden_facts();
        refused.implementation_limits = zero;
        assert!(NativeExecutionObservationV1::build(refused).is_err());
        let empty = ObservationFacts {
            input_hashes: Vec::new(),
            ..golden_facts()
        };
        let empty_built = NativeExecutionObservationV1::build(empty).expect("empty inputs build");
        assert!(empty_built.input_hashes().is_empty());
    }

    #[test]
    fn termination_rejects_invalid_trap_tags() {
        for tag in [0, 5, u32::MAX] {
            let mut facts = golden_facts();
            facts.termination = NativeObservedTermination::Trap {
                trap_tag: tag,
                payload: None,
            };
            assert!(
                NativeExecutionObservationV1::build(facts).is_err(),
                "tag {tag}"
            );
        }
        let mut facts = golden_facts();
        facts.termination = NativeObservedTermination::Trap {
            trap_tag: 4,
            payload: Some(ValueHash::from_bytes([0; 32])),
        };
        assert!(NativeExecutionObservationV1::build(facts).is_ok());
    }

    #[test]
    fn limit_record_layouts_are_frozen_for_test_owner() {
        // The test-evidence owner re-encodes these exact layouts from public
        // fields; any reorder here must fail loudly in both crates.
        fn hex_of(bytes: &[u8]) -> String {
            let mut out = String::with_capacity(bytes.len() * 2);
            for byte in bytes {
                use core::fmt::Write as _;
                write!(out, "{byte:02x}").expect("hex formatting never fails");
            }
            out
        }
        assert_eq!(
            hex_of(
                &NativeImplementationLimits::HARD_MAXIMA
                    .record()
                    .expect("hard maxima encode")
            ),
            "05010480ade204020480808020030480808020040280020503808010"
        );
        assert_eq!(
            hex_of(
                &NativeDeclaredLimits {
                    fuel: 100,
                    memory_bytes: 4_096,
                    output_bytes: 64,
                    effect_count: 0,
                    call_depth: 8,
                    wall_timeout_millis: 1_000,
                }
                .record()
                .expect("declared encode")
            ),
            "06010164020280200301400401000501080602e807"
        );
    }

    #[test]
    fn resource_kind_tags_and_hard_maxima_are_frozen() {
        assert_eq!(
            [
                NativeResourceKind::Instructions.tag(),
                NativeResourceKind::Fuel.tag(),
                NativeResourceKind::ValueUnits.tag(),
                NativeResourceKind::OutputUnits.tag(),
                NativeResourceKind::CallDepth.tag(),
                NativeResourceKind::OutputBytes.tag(),
                NativeResourceKind::OutputEncoding.tag(),
            ],
            [1, 2, 3, 4, 5, 6, 7]
        );
        assert!(NativeImplementationLimits::HARD_MAXIMA.within_hard_maxima());
        let tight = NativeImplementationLimits {
            max_instructions: 0,
            ..NativeImplementationLimits::HARD_MAXIMA
        };
        assert!(tight.within_hard_maxima());
        let loose = NativeImplementationLimits {
            max_call_depth: NativeImplementationLimits::HARD_MAXIMA.max_call_depth + 1,
            ..NativeImplementationLimits::HARD_MAXIMA
        };
        assert!(!loose.within_hard_maxima());
    }
}
