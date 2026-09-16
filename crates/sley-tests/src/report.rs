//! Native execution/test reports (`SLEYNEX1`/`SLEYNTS1`) from
//! `NATIVE_TEST_EXECUTION_V1.md` section 5.
//!
//! The execution report binds one plan entry to either an embedded Stored
//! native observation or a rejected pre-execution failure. The test report
//! binds one entry per selected plan test with an exact expected projection
//! and comparison, plus checked totals. Parsing validates shape, order,
//! ranges, digests, and totals; it never proves the VM ran. Exact comparison
//! delegates to the shared [`sley_conformance`] kernel one-way, so the old
//! restricted reports and the native reports cannot diverge on the match
//! rule. The rejected-evidence mapping covers every
//! `NativeExecutionError` leaf from the N0 table using each owner's exact
//! numeric and symbol accessors.

use sley_check::{cfg::CfgValidationError, effects::EffectValidationError};
use sley_conformance::{ExpectedEvidence, RestrictedComparison, compare_expected_evidence};
use sley_id::{
    EntityId, ExecutionReportId, NativeObservationId, NativeTestPlanId, ObjectId, SchemaEpochId,
    StateRoot, TestReportId, ValueHash,
};
use sley_scb1::{
    ScbError, ScbErrorCode, ScbValueCursor, encode_list, encode_record, encode_text, encode_union,
    encode_uvar,
};
use sley_vm::{
    ExecutionError, LoweringError,
    native_execution::{
        NativeExecutionError, NativeExecutionObservationV1, NativeObservedTermination,
    },
};

use crate::codec::{
    RECORD_VERSION, check_sorted_unique, decode_envelope, decode_fields, expect_tags,
    preimage_bytes, read_id, read_uvar_value,
};
use crate::plan::NativeTestPlanV1;

/// `SLEYNEX1` envelope magic for [`NativeExecutionReportV1`].
pub const EXECUTION_REPORT_MAGIC: [u8; 8] = *b"SLEYNEX1";
/// `SLEYNTS1` envelope magic for [`NativeTestReportV1`].
pub const TEST_REPORT_MAGIC: [u8; 8] = *b"SLEYNTS1";
/// Native execution report stored ceiling, including envelope and trailer.
pub const MAX_EXECUTION_REPORT_STORED: usize = 262_144;
/// Maximum entries in one native test report.
pub const MAX_TEST_REPORT_ENTRIES: u64 = 256;
/// Maximum symbol bytes in one rejected-evidence record.
pub const MAX_SYMBOL_BYTES: usize = 96;

/// Rejected type-phase evidence.
pub const REJECT_PHASE_TYPE: u32 = 1;
/// Rejected CFG-phase evidence.
pub const REJECT_PHASE_CFG: u32 = 2;
/// Rejected lowering-phase evidence.
pub const REJECT_PHASE_LOWERING: u32 = 3;
/// Rejected fingerprint-phase evidence.
pub const REJECT_PHASE_FINGERPRINT: u32 = 4;
/// Rejected execution-phase evidence.
pub const REJECT_PHASE_EXECUTION: u32 = 5;
/// Rejected native-profile evidence.
pub const REJECT_PHASE_NATIVE_PROFILE: u32 = 6;
/// Rejected effect-phase evidence.
pub const REJECT_PHASE_EFFECT: u32 = 7;

/// Expected-outcome union tag for an exact value hash.
pub const EXPECTED_VALUE: u32 = 1;
/// Expected-outcome union tag for a frozen trap code.
pub const EXPECTED_FAILURE_CODE: u32 = 2;

/// Comparison tag for a matching expectation.
pub const COMPARISON_MATCH: u32 = 1;
/// Comparison tag for a differing expectation.
pub const COMPARISON_MISMATCH: u32 = 2;
/// Comparison tag for pre-observation rejection.
pub const COMPARISON_EXECUTION_REJECTED: u32 = 3;

/// Pre-execution failure with the exact owner phase, code, and symbol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedEvidence {
    phase: u32,
    numeric_code: u32,
    symbol: String,
}

impl RejectedEvidence {
    /// Builds validated rejected evidence from owner failure facts.
    ///
    /// # Errors
    /// Returns `SCB_CONTRACT_UNKNOWN` for a phase outside 1..=7 or a symbol
    /// that is empty, longer than 96 bytes, or not ASCII.
    pub fn from_parts(phase: u32, numeric_code: u32, symbol: &str) -> Result<Self, ScbError> {
        if !(REJECT_PHASE_TYPE..=REJECT_PHASE_EFFECT).contains(&phase) {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        if symbol.is_empty() || symbol.len() > MAX_SYMBOL_BYTES || !symbol.is_ascii() {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        Ok(Self {
            phase,
            numeric_code,
            symbol: symbol.to_owned(),
        })
    }

    /// Rejected owner phase, 1..=7 per the N0 mapping table.
    #[must_use]
    pub const fn phase(&self) -> u32 {
        self.phase
    }

    /// Exact leaf owner numeric code.
    #[must_use]
    pub const fn numeric_code(&self) -> u32 {
        self.numeric_code
    }

    /// Exact leaf owner symbol.
    #[must_use]
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    fn record(&self) -> Result<Vec<u8>, ScbError> {
        encode_record(&[
            (1, encode_uvar(u64::from(self.phase))),
            (2, encode_uvar(u64::from(self.numeric_code))),
            (3, encode_text(&self.symbol)?),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2, 3])?;
        let phase = u32::try_from(read_uvar_value(&fields[0].1, 32)?)
            .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
        let numeric_code = u32::try_from(read_uvar_value(&fields[1].1, 32)?)
            .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
        let symbol = read_symbol(&fields[2].1)?;
        Self::from_parts(phase, numeric_code, &symbol)
    }
}

/// Maps one native execution failure to its rejected-evidence record.
///
/// Every `NativeExecutionError` leaf from the N0 table maps to its fixed
/// phase with the leaf owner's exact numeric code and symbol; wrapper origin
/// never changes the phase or code.
///
/// # Errors
/// Returns `SCB_CONTRACT_UNKNOWN` when an owner symbol violates the
/// rejected-evidence shape, which cannot happen for the frozen symbols.
pub fn rejected_from_error(error: &NativeExecutionError) -> Result<RejectedEvidence, ScbError> {
    // Type-phase rows share one body: direct, lowering-nested, and both
    // effect-nested routes to a `TypeError`. CFG-phase rows likewise share
    // the `CfgError` body. Merged OR arms keep the N0 table coverage while
    // satisfying the identical-body lint; each leaf is still exercised by
    // `rejected_mapping_covers_every_n0_table_row`.
    let (phase, numeric_code, symbol) = match error {
        NativeExecutionError::Preserved(
            ExecutionError::Type(error)
            | ExecutionError::Lowering(LoweringError::Cfg(CfgValidationError::Type(error))),
        )
        | NativeExecutionError::Effect(
            EffectValidationError::Type(error)
            | EffectValidationError::Cfg(CfgValidationError::Type(error)),
        ) => (
            REJECT_PHASE_TYPE,
            error.code().numeric(),
            error.code().as_str(),
        ),
        NativeExecutionError::Preserved(ExecutionError::Lowering(LoweringError::Cfg(
            CfgValidationError::Cfg(error),
        )))
        | NativeExecutionError::Effect(EffectValidationError::Cfg(CfgValidationError::Cfg(
            error,
        ))) => (
            REJECT_PHASE_CFG,
            error.code().numeric(),
            error.code().as_str(),
        ),
        NativeExecutionError::Preserved(ExecutionError::Lowering(LoweringError::Lower(error))) => (
            REJECT_PHASE_LOWERING,
            error.code().numeric(),
            error.code().as_str(),
        ),
        NativeExecutionError::Preserved(ExecutionError::Fingerprint(error)) => (
            REJECT_PHASE_FINGERPRINT,
            error.code().numeric(),
            error.code().as_str(),
        ),
        NativeExecutionError::Preserved(ExecutionError::Status(error)) => {
            (REJECT_PHASE_EXECUTION, error.numeric(), error.as_str())
        }
        NativeExecutionError::Preserved(ExecutionError::Exec(error)) => {
            (REJECT_PHASE_EXECUTION, error.numeric(), error.as_str())
        }
        NativeExecutionError::Effect(EffectValidationError::Effect(error)) => (
            REJECT_PHASE_EFFECT,
            error.code().numeric(),
            error.code().as_str(),
        ),
        NativeExecutionError::Profile(error) => {
            (REJECT_PHASE_NATIVE_PROFILE, error.numeric(), error.as_str())
        }
    };
    RejectedEvidence::from_parts(phase, numeric_code, symbol)
}

/// Hash-only expected `TestCase` outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeExpected {
    /// Exact expected value hash.
    Value(ValueHash),
    /// Exact frozen trap code 1..=4.
    FailureCode(u32),
}

impl NativeExpected {
    /// Expected-outcome union tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Value(_) => EXPECTED_VALUE,
            Self::FailureCode(_) => EXPECTED_FAILURE_CODE,
        }
    }

    fn record(self) -> Result<Vec<u8>, ScbError> {
        match self {
            Self::Value(hash) => encode_union(EXPECTED_VALUE, hash.as_bytes()),
            Self::FailureCode(code) => {
                if !(1..=4).contains(&code) {
                    return Err(ScbError::new(ScbErrorCode::ContractUnknown));
                }
                encode_union(EXPECTED_FAILURE_CODE, &encode_uvar(u64::from(code)))
            }
        }
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let mut cursor = ScbValueCursor::new(value)?;
        let (tag, payload) = cursor.read_union()?;
        cursor.check_finished()?;
        match tag {
            EXPECTED_VALUE => Ok(Self::Value(ValueHash::from_bytes(read_id(payload)?))),
            EXPECTED_FAILURE_CODE => {
                let code = u32::try_from(read_union_uvar(payload, 32)?)
                    .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
                if !(1..=4).contains(&code) {
                    return Err(ScbError::new(ScbErrorCode::ContractUnknown));
                }
                Ok(Self::FailureCode(code))
            }
            _ => Err(ScbError::new(ScbErrorCode::UnionInvalid)),
        }
    }
}

/// Native expectation comparison. A deterministic report alone is not a
/// measured pass; admission also requires protected plan and measurements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestComparison {
    /// Expected and observed projections match.
    Match,
    /// Expected and observed projections differ.
    Mismatch,
    /// Execution was rejected before an observation existed.
    ExecutionRejected,
}

impl TestComparison {
    /// Comparison wire tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Match => COMPARISON_MATCH,
            Self::Mismatch => COMPARISON_MISMATCH,
            Self::ExecutionRejected => COMPARISON_EXECUTION_REJECTED,
        }
    }

    /// Inverse of [`tag`](Self::tag); unknown tags refuse at parse.
    #[must_use]
    pub const fn from_tag(tag: u32) -> Option<Self> {
        match tag {
            COMPARISON_MATCH => Some(Self::Match),
            COMPARISON_MISMATCH => Some(Self::Mismatch),
            COMPARISON_EXECUTION_REJECTED => Some(Self::ExecutionRejected),
            _ => None,
        }
    }
}

/// Compares one expected projection against an observed termination.
///
/// Exact validated value-hash equality or exact trap-tag equality matches.
/// Resource refusal, wrong result, internal failure, or any non-trap shape
/// against a failure expectation never matches. A rejected execution report
/// maps to `ExecutionRejected` by the caller holding the evidence, not here.
///
/// The rule delegates to the shared conformance kernel, so native and
/// restricted reports cannot diverge on what counts as a match.
#[must_use]
pub fn compare_native_expected(
    expected: NativeExpected,
    termination: &NativeObservedTermination,
) -> TestComparison {
    let (success, trap_tag) = match termination {
        NativeObservedTermination::Success(hash) => (Some(*hash), None),
        NativeObservedTermination::Trap { trap_tag, .. } => (None, Some(*trap_tag)),
        NativeObservedTermination::ResourceLimit(_)
        | NativeObservedTermination::InternalInvariant => (None, None),
    };
    let projected = match expected {
        NativeExpected::Value(hash) => ExpectedEvidence::Value(hash),
        NativeExpected::FailureCode(code) => ExpectedEvidence::FailureCode(code),
    };
    match compare_expected_evidence(projected, success, trap_tag, true) {
        RestrictedComparison::Match => TestComparison::Match,
        RestrictedComparison::Mismatch => TestComparison::Mismatch,
        // Unreachable with `observed = true`, kept total over the kernel type.
        RestrictedComparison::ExecutionRejected => TestComparison::ExecutionRejected,
    }
}

/// Observed or rejected evidence bound into one execution report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeExecutionEvidence {
    /// Embedded Stored observation with its parsed termination projection.
    Observed {
        /// Complete Stored observation envelope and trailer.
        stored: Vec<u8>,
        /// Termination parsed from the embedded bytes.
        termination: NativeObservedTermination,
        /// Observation identity verified against the embedded trailer.
        observation_id: NativeObservationId,
    },
    /// Pre-execution failure; diagnostic, never proof of execution.
    Rejected(RejectedEvidence),
}

impl NativeExecutionEvidence {
    /// Validates embedded observation bytes into observed evidence.
    ///
    /// # Errors
    /// Returns the first stable SCB1 failure from strict observation parsing.
    pub fn observed(stored: Vec<u8>) -> Result<Self, ScbError> {
        let parsed = NativeExecutionObservationV1::parse_stored(&stored)?;
        Ok(Self::Observed {
            stored,
            termination: parsed.termination().clone(),
            observation_id: parsed.observation_id(),
        })
    }

    fn record(&self) -> Result<Vec<u8>, ScbError> {
        match self {
            Self::Observed { stored, .. } => encode_union(1, stored),
            Self::Rejected(rejected) => encode_union(2, &rejected.record()?),
        }
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let mut cursor = ScbValueCursor::new(value)?;
        let (tag, payload) = cursor.read_union()?;
        cursor.check_finished()?;
        match tag {
            1 => Self::observed(payload.to_vec()),
            2 => Ok(Self::Rejected(RejectedEvidence::parse(payload)?)),
            _ => Err(ScbError::new(ScbErrorCode::UnionInvalid)),
        }
    }
}

/// Caller-supplied execution report facts; constructing these proves nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeExecutionReportParts {
    /// Test plan this execution was selected by.
    pub plan_id: NativeTestPlanId,
    /// Executed test entity.
    pub test_entity: EntityId,
    /// Exact executed test object.
    pub test_object: ObjectId,
    /// Exact executed target object.
    pub target_object: ObjectId,
    /// Observed or rejected evidence.
    pub evidence: NativeExecutionEvidence,
}

/// Immutable canonical execution report; external callers cannot forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeExecutionReportV1 {
    parts: NativeExecutionReportParts,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: ExecutionReportId,
}

impl NativeExecutionReportV1 {
    /// Builds a validated execution report from caller-supplied facts.
    ///
    /// Observed evidence re-parses its embedded observation; the report
    /// stored bytes must fit the 262,144-byte ceiling.
    ///
    /// # Errors
    /// Returns `SCB_RESOURCE_LIMIT` for an oversized report, or the first
    /// stable SCB1 failure from embedded observation validation.
    pub fn build(parts: NativeExecutionReportParts) -> Result<Self, ScbError> {
        if let NativeExecutionEvidence::Observed { stored, .. } = &parts.evidence {
            // Re-parse so a mutated projection cannot smuggle unchecked bytes.
            let reparsed = NativeExecutionEvidence::observed(stored.clone())?;
            if reparsed != parts.evidence {
                return Err(ScbError::new(ScbErrorCode::DigestMismatch));
            }
        }
        let record = encode_record(&[
            (1, encode_uvar(RECORD_VERSION)),
            (2, parts.plan_id.as_bytes().to_vec()),
            (3, parts.test_entity.as_bytes().to_vec()),
            (4, parts.test_object.as_bytes().to_vec()),
            (5, parts.target_object.as_bytes().to_vec()),
            (6, parts.evidence.record()?),
        ])?;
        let preimage = preimage_bytes(EXECUTION_REPORT_MAGIC, &record)?;
        let id = ExecutionReportId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        if stored.len() > MAX_EXECUTION_REPORT_STORED {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        Ok(Self {
            parts,
            record,
            stored,
            id,
        })
    }

    /// Strictly parses and validates one stored execution report envelope.
    ///
    /// The report ceiling applies before any other check; parsing validates
    /// the embedded observation or rejected record but admits nothing.
    ///
    /// # Errors
    /// Returns the first stable SCB1 failure encountered while decoding the
    /// envelope, verifying its digest, or validating fields in tag order.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        if stored.len() > MAX_EXECUTION_REPORT_STORED {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let (record, trailer) = decode_envelope(stored, EXECUTION_REPORT_MAGIC)?;
        let preimage = preimage_bytes(EXECUTION_REPORT_MAGIC, &record)?;
        if ExecutionReportId::derive(&preimage).into_bytes() != trailer {
            return Err(ScbError::new(ScbErrorCode::DigestMismatch));
        }
        let fields = decode_fields(&record)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5, 6])?;
        if read_uvar_value(&fields[0].1, 32)? != RECORD_VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        Self::build(NativeExecutionReportParts {
            plan_id: NativeTestPlanId::from_bytes(read_id(&fields[1].1)?),
            test_entity: EntityId::from_bytes(read_id(&fields[2].1)?),
            test_object: ObjectId::from_bytes(read_id(&fields[3].1)?),
            target_object: ObjectId::from_bytes(read_id(&fields[4].1)?),
            evidence: NativeExecutionEvidence::parse(&fields[5].1)?,
        })
    }

    /// Canonical SCB1 record without envelope or trailer.
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }

    /// Complete canonical envelope and digest trailer.
    #[must_use]
    pub fn stored_bytes(&self) -> &[u8] {
        &self.stored
    }

    /// Execution report identity over the exact envelope preimage.
    #[must_use]
    pub const fn report_id(&self) -> ExecutionReportId {
        self.id
    }

    /// Bound plan identity.
    #[must_use]
    pub const fn plan_id(&self) -> NativeTestPlanId {
        self.parts.plan_id
    }

    /// Bound test entity.
    #[must_use]
    pub const fn test_entity(&self) -> EntityId {
        self.parts.test_entity
    }

    /// Bound test object.
    #[must_use]
    pub const fn test_object(&self) -> ObjectId {
        self.parts.test_object
    }

    /// Bound target object.
    #[must_use]
    pub const fn target_object(&self) -> ObjectId {
        self.parts.target_object
    }

    /// Bound observed or rejected evidence.
    #[must_use]
    pub const fn evidence(&self) -> &NativeExecutionEvidence {
        &self.parts.evidence
    }
}

/// One test-report entry with its expected projection and comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeTestEntry {
    /// Tested entity.
    pub test_entity: EntityId,
    /// Exact tested object.
    pub test_object: ObjectId,
    /// Execution report proving the observed or rejected run.
    pub execution_report_id: ExecutionReportId,
    /// Hash-only expected projection.
    pub expected: NativeExpected,
    /// Expectation comparison.
    pub comparison: TestComparison,
}

impl NativeTestEntry {
    fn record(self) -> Result<Vec<u8>, ScbError> {
        encode_record(&[
            (1, self.test_entity.as_bytes().to_vec()),
            (2, self.test_object.as_bytes().to_vec()),
            (3, self.execution_report_id.as_bytes().to_vec()),
            (4, self.expected.record()?),
            (5, encode_uvar(u64::from(self.comparison.tag()))),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5])?;
        let comparison = TestComparison::from_tag(
            u32::try_from(read_uvar_value(&fields[4].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?,
        )
        .ok_or_else(|| ScbError::new(ScbErrorCode::ContractUnknown))?;
        Ok(Self {
            test_entity: EntityId::from_bytes(read_id(&fields[0].1)?),
            test_object: ObjectId::from_bytes(read_id(&fields[1].1)?),
            execution_report_id: ExecutionReportId::from_bytes(read_id(&fields[2].1)?),
            expected: NativeExpected::parse(&fields[3].1)?,
            comparison,
        })
    }
}

/// Immutable canonical test report; external callers cannot forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeTestReportV1 {
    plan_id: NativeTestPlanId,
    schema_epoch: SchemaEpochId,
    proposed_root: StateRoot,
    entries: Vec<NativeTestEntry>,
    match_count: u64,
    mismatch_count: u64,
    rejected_count: u64,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: TestReportId,
}

impl NativeTestReportV1 {
    /// Builds a validated test report covering exactly one plan's selection.
    ///
    /// Plan identity, epoch, and proposed root derive from the plan itself;
    /// entries must cover every selected test with its exact object, in
    /// test-ID order, at most 256. Totals are computed with checked sums. An
    /// empty selection yields the explicit empty report with zero counts.
    ///
    /// # Errors
    /// Returns `SCB_FIELD_ORDER`/`SCB_FIELD_DUPLICATE` for unsorted or
    /// repeated entries, `SCB_RESOURCE_LIMIT` beyond 256 entries or on total
    /// overflow, and `SCB_CONTRACT_UNKNOWN` when entries do not cover the
    /// plan selection exactly.
    pub fn build(plan: &NativeTestPlanV1, entries: Vec<NativeTestEntry>) -> Result<Self, ScbError> {
        entry_order(&entries)?;
        if u64::try_from(entries.len()).map_err(|_| resource_error())? > MAX_TEST_REPORT_ENTRIES {
            return Err(resource_error());
        }
        let (match_count, mismatch_count, rejected_count) = count_entries(&entries)?;
        let encoded = entries
            .iter()
            .map(|entry| entry.record())
            .collect::<Result<Vec<_>, _>>()?;
        let record = encode_record(&[
            (1, encode_uvar(RECORD_VERSION)),
            (2, plan.plan_id().as_bytes().to_vec()),
            (3, plan.semantic_epoch().as_bytes().to_vec()),
            (4, plan.proposed_root().as_bytes().to_vec()),
            (5, encode_list(&encoded)?),
            (6, encode_uvar(match_count)),
            (7, encode_uvar(mismatch_count)),
            (8, encode_uvar(rejected_count)),
        ])?;
        let preimage = preimage_bytes(TEST_REPORT_MAGIC, &record)?;
        let id = TestReportId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        let report = Self {
            plan_id: plan.plan_id(),
            schema_epoch: plan.semantic_epoch(),
            proposed_root: plan.proposed_root(),
            entries,
            match_count,
            mismatch_count,
            rejected_count,
            record,
            stored,
            id,
        };
        report.verify_plan_coverage(plan)?;
        Ok(report)
    }

    /// Strictly parses and validates one stored test report envelope.
    ///
    /// Parsing checks shape, entry order, ranges, and exact totals. Plan
    /// coverage needs the plan itself; use [`verify_plan_coverage`](Self::verify_plan_coverage).
    ///
    /// # Errors
    /// Returns the first stable SCB1 failure encountered while decoding the
    /// envelope, verifying its digest, or validating fields and totals.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        let (record, trailer) = decode_envelope(stored, TEST_REPORT_MAGIC)?;
        let preimage = preimage_bytes(TEST_REPORT_MAGIC, &record)?;
        if TestReportId::derive(&preimage).into_bytes() != trailer {
            return Err(ScbError::new(ScbErrorCode::DigestMismatch));
        }
        let fields = decode_fields(&record)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5, 6, 7, 8])?;
        if read_uvar_value(&fields[0].1, 32)? != RECORD_VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        let entries = parse_entries(&fields[4].1)?;
        let (match_count, mismatch_count, rejected_count) = count_entries(&entries)?;
        if match_count != read_uvar_value(&fields[5].1, 64)?
            || mismatch_count != read_uvar_value(&fields[6].1, 64)?
            || rejected_count != read_uvar_value(&fields[7].1, 64)?
        {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        entry_order(&entries)?;
        Ok(Self {
            plan_id: NativeTestPlanId::from_bytes(read_id(&fields[1].1)?),
            schema_epoch: SchemaEpochId::from_bytes(read_id(&fields[2].1)?),
            proposed_root: StateRoot::from_bytes(read_id(&fields[3].1)?),
            entries,
            match_count,
            mismatch_count,
            rejected_count,
            record,
            stored: stored.to_vec(),
            id: TestReportId::from_bytes(trailer),
        })
    }

    /// Requires entries to cover exactly the plan's selected tests.
    ///
    /// Same length, same test entities in order, same exact test objects.
    /// Entry execution IDs, expectations, and comparisons are admission
    /// evidence checked by later owners, not here.
    ///
    /// # Errors
    /// Returns `SCB_CONTRACT_UNKNOWN` for any coverage divergence.
    pub fn verify_plan_coverage(&self, plan: &NativeTestPlanV1) -> Result<(), ScbError> {
        if self.plan_id != plan.plan_id()
            || self.schema_epoch != plan.semantic_epoch()
            || self.proposed_root != plan.proposed_root()
        {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        if self.entries.len() != plan.selected().len() {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        for (entry, selected) in self.entries.iter().zip(plan.selected()) {
            if entry.test_entity != selected.test_entity
                || entry.test_object != selected.test_object
            {
                return Err(ScbError::new(ScbErrorCode::ContractUnknown));
            }
        }
        Ok(())
    }

    /// Canonical SCB1 record without envelope or trailer.
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }

    /// Complete canonical envelope and digest trailer.
    #[must_use]
    pub fn stored_bytes(&self) -> &[u8] {
        &self.stored
    }

    /// Test report identity over the exact envelope preimage.
    #[must_use]
    pub const fn report_id(&self) -> TestReportId {
        self.id
    }

    /// Bound plan identity.
    #[must_use]
    pub const fn plan_id(&self) -> NativeTestPlanId {
        self.plan_id
    }

    /// Bound schema epoch.
    #[must_use]
    pub const fn schema_epoch(&self) -> SchemaEpochId {
        self.schema_epoch
    }

    /// Bound proposed root.
    #[must_use]
    pub const fn proposed_root(&self) -> StateRoot {
        self.proposed_root
    }

    /// Bound entries in test-ID order.
    #[must_use]
    pub fn entries(&self) -> &[NativeTestEntry] {
        &self.entries
    }

    /// Checked match total.
    #[must_use]
    pub const fn match_count(&self) -> u64 {
        self.match_count
    }

    /// Checked mismatch total.
    #[must_use]
    pub const fn mismatch_count(&self) -> u64 {
        self.mismatch_count
    }

    /// Checked rejection total.
    #[must_use]
    pub const fn rejected_count(&self) -> u64 {
        self.rejected_count
    }
}

fn resource_error() -> ScbError {
    ScbError::new(ScbErrorCode::ResourceLimit)
}

fn read_symbol(value: &[u8]) -> Result<String, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let text = cursor.read_text()?;
    cursor.check_finished()?;
    Ok(text.to_owned())
}

fn read_union_uvar(payload: &[u8], width: u8) -> Result<u64, ScbError> {
    let mut cursor = ScbValueCursor::new(payload)?;
    let parsed = cursor.read_uvar(width)?;
    cursor.check_finished()?;
    Ok(parsed)
}

fn entry_order(entries: &[NativeTestEntry]) -> Result<(), ScbError> {
    let keys: Vec<[u8; 32]> = entries
        .iter()
        .map(|entry| *entry.test_entity.as_bytes())
        .collect();
    check_sorted_unique(&keys)
}

fn count_entries(entries: &[NativeTestEntry]) -> Result<(u64, u64, u64), ScbError> {
    let mut matched = 0_u64;
    let mut mismatched = 0_u64;
    let mut rejected = 0_u64;
    for entry in entries {
        let counter = match entry.comparison {
            TestComparison::Match => &mut matched,
            TestComparison::Mismatch => &mut mismatched,
            TestComparison::ExecutionRejected => &mut rejected,
        };
        *counter = counter
            .checked_add(1)
            .ok_or_else(|| ScbError::new(ScbErrorCode::ResourceLimit))?;
    }
    Ok((matched, mismatched, rejected))
}

fn parse_entries(value: &[u8]) -> Result<Vec<NativeTestEntry>, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_list_count()?;
    if count > MAX_TEST_REPORT_ENTRIES {
        return Err(resource_error());
    }
    if count > u64::from(u32::MAX) {
        return Err(resource_error());
    }
    let mut entries = Vec::new();
    for _ in 0..count {
        entries.push(NativeTestEntry::parse(cursor.read_bytes()?)?);
    }
    cursor.check_finished()?;
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{decode_fields as raw_fields, preimage_bytes as raw_pre};
    use crate::plan::{ChangedTest, NativeTestPlanParts, SelectedEntry};
    use crate::policy::sample_parts as policy_parts;
    use sley_check::{
        TypeError, TypeErrorCode,
        cfg::{CfgError, CfgErrorCode, CfgValidationError},
        effects::{EffectError, EffectErrorCode, EffectValidationError},
    };
    use sley_id::{CandidateId, CandidateResultId, PolicyRootId, TransactionId, WorkspaceId};
    use sley_scb1::{encode_list as raw_list, encode_record as raw_record};
    use sley_ssmc::fingerprint::{FingerprintError, FingerprintErrorCode};
    use sley_vm::native_execution::NativeExecutionProfileError;
    use sley_vm::{ExecutionErrorCode, ExecutionStatusCode, LowerError, LowerErrorCode};

    use crate::policy::NativeResourcePolicyV1;

    const GOLDEN_OBSERVATION_STORED: &str = "534c45594e4f4231018103120101010220545454545454545454545454545454545454545454545454545454545454545403206161616161616161616161616161616161616161616161616161616161616161042062626262626262626262626262626262626262626262626262626262626262620520555555555555555555555555555555555555555555555555555555555555555506205151515151515151515151515151515151515151515151515151515151515151072063636363636363636363636363636363636363636363636363636363636363630820392927d6c948b9a03a27b87f26ebfe08c65ddf4e831a18e1d82f02dc4abb0bf40922012064646464646464646464646464646464646464646464646464646464646464640a1506010164020280200301400401000501080602e8070b1c05010480ade2040204808080200304808080200402800205038080100c22012065656565656565656565656565656565656565656565656565656565656565650d010b0e010c0f010d10010e11010f12010057510a52bb567b20f6c36e56e4fecfb44071a593df7b1395b6a98bf2f8c2fe44";
    const GOLDEN_OBSERVATION_ID: &str =
        "57510a52bb567b20f6c36e56e4fecfb44071a593df7b1395b6a98bf2f8c2fe44";
    const GOLDEN_EXEC_REJECTED_RECORD: &str = "060101010220505050505050505050505050505050505050505050505050505050505050505003205151515151515151515151515151515151515151515151515151515151515151042052525252525252525252525252525252525252525252525252525252525252520520535353535353535353535353535353535353535353535353535353535353535306240222030101050203fad201031716564d5f455845435f5245534f555243455f4c494d4954";
    const GOLDEN_EXEC_REJECTED_ID: &str =
        "34b7d958c10b3212291009ff74271aaa27b384d8b7ad1eb5af395954ef30c1be";
    const GOLDEN_EXEC_OBSERVED_RECORD: &str = "060101010220505050505050505050505050505050505050505050505050505050505050505003205151515151515151515151515151515151515151515151515151515151515151042052525252525252525252525252525252525252525252525252525252525252520520535353535353535353535353535353535353535353535353535353535353535306af0301ac03534c45594e4f4231018103120101010220545454545454545454545454545454545454545454545454545454545454545403206161616161616161616161616161616161616161616161616161616161616161042062626262626262626262626262626262626262626262626262626262626262620520555555555555555555555555555555555555555555555555555555555555555506205151515151515151515151515151515151515151515151515151515151515151072063636363636363636363636363636363636363636363636363636363636363630820392927d6c948b9a03a27b87f26ebfe08c65ddf4e831a18e1d82f02dc4abb0bf40922012064646464646464646464646464646464646464646464646464646464646464640a1506010164020280200301400401000501080602e8070b1c05010480ade2040204808080200304808080200402800205038080100c22012065656565656565656565656565656565656565656565656565656565656565650d010b0e010c0f010d10010e11010f12010057510a52bb567b20f6c36e56e4fecfb44071a593df7b1395b6a98bf2f8c2fe44";
    const GOLDEN_EXEC_OBSERVED_ID: &str =
        "bf828bad0b15d16b85430d7d631212be33252189753c6fa2e98210372c4b87dd";
    const GOLDEN_TEST_ONE_RECORD: &str = "08010101022050505050505050505050505050505050505050505050505050505050505050500320545454545454545454545454545454545454545454545454545454545454545404205555555555555555555555555555555555555555555555555555555555555555059101018e010501205151515151515151515151515151515151515151515151515151515151515151022052525252525252525252525252525252525252525252525252525252525252520320bf828bad0b15d16b85430d7d631212be33252189753c6fa2e98210372c4b87dd042201206565656565656565656565656565656565656565656565656565656565656565050101060101070100080100";
    const GOLDEN_TEST_ONE_ID: &str =
        "0a02a6a7cababb3e1c5bdef651a447c8b747e230dae432ffe7d1d85d7918490f";
    const GOLDEN_TEST_EMPTY_RECORD: &str = "08010101022050505050505050505050505050505050505050505050505050505050505050500320545454545454545454545454545454545454545454545454545454545454545404205555555555555555555555555555555555555555555555555555555555555555050100060100070100080100";
    const GOLDEN_TEST_EMPTY_ID: &str =
        "f2a9807a6a69112640648c3ba51cd13b61b58a38322858c0606bc15b123fe8cd";

    fn decode_hex(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        let mut out = Vec::with_capacity(hex.len() / 2);
        for chunk in hex.as_bytes().chunks_exact(2) {
            let text = core::str::from_utf8(chunk).expect("hex is ascii");
            out.push(u8::from_str_radix(text, 16).expect("hex digits"));
        }
        out
    }

    fn hex_of(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use core::fmt::Write as _;
            write!(out, "{byte:02x}").expect("hex formatting never fails");
        }
        out
    }

    fn rejected_parts() -> NativeExecutionReportParts {
        NativeExecutionReportParts {
            plan_id: NativeTestPlanId::from_bytes([0x50; 32]),
            test_entity: EntityId::from_bytes([0x51; 32]),
            test_object: ObjectId::from_bytes([0x52; 32]),
            target_object: ObjectId::from_bytes([0x53; 32]),
            evidence: NativeExecutionEvidence::Rejected(
                RejectedEvidence::from_parts(5, 27_002, "VM_EXEC_RESOURCE_LIMIT")
                    .expect("golden rejected builds"),
            ),
        }
    }

    fn observed_parts() -> NativeExecutionReportParts {
        NativeExecutionReportParts {
            plan_id: NativeTestPlanId::from_bytes([0x50; 32]),
            test_entity: EntityId::from_bytes([0x51; 32]),
            test_object: ObjectId::from_bytes([0x52; 32]),
            target_object: ObjectId::from_bytes([0x53; 32]),
            evidence: NativeExecutionEvidence::observed(decode_hex(GOLDEN_OBSERVATION_STORED))
                .expect("golden observation parses"),
        }
    }

    fn re_envelope_exec(record: &[u8]) -> Vec<u8> {
        let preimage = raw_pre(EXECUTION_REPORT_MAGIC, record).expect("preimage fits");
        let id = ExecutionReportId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        stored
    }

    fn re_envelope_test(record: &[u8]) -> Vec<u8> {
        let preimage = raw_pre(TEST_REPORT_MAGIC, record).expect("preimage fits");
        let id = TestReportId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        stored
    }

    #[test]
    fn rejected_report_matches_independent_python_golden() {
        let report = NativeExecutionReportV1::build(rejected_parts()).expect("rejected builds");
        assert_eq!(hex_of(report.record_bytes()), GOLDEN_EXEC_REJECTED_RECORD);
        assert_eq!(
            hex_of(report.report_id().as_bytes()),
            GOLDEN_EXEC_REJECTED_ID
        );
        let parsed =
            NativeExecutionReportV1::parse(report.stored_bytes()).expect("roundtrip parses");
        assert_eq!(parsed, report);
        assert_eq!(parsed.plan_id(), NativeTestPlanId::from_bytes([0x50; 32]));
        match parsed.evidence() {
            NativeExecutionEvidence::Rejected(rejected) => {
                assert_eq!(rejected.phase(), 5);
                assert_eq!(rejected.numeric_code(), 27_002);
                assert_eq!(rejected.symbol(), "VM_EXEC_RESOURCE_LIMIT");
            }
            NativeExecutionEvidence::Observed { .. } => panic!("golden is rejected"),
        }
    }

    #[test]
    fn observed_report_matches_independent_python_golden() {
        let report = NativeExecutionReportV1::build(observed_parts()).expect("observed builds");
        assert_eq!(hex_of(report.record_bytes()), GOLDEN_EXEC_OBSERVED_RECORD);
        assert_eq!(
            hex_of(report.report_id().as_bytes()),
            GOLDEN_EXEC_OBSERVED_ID
        );
        let parsed =
            NativeExecutionReportV1::parse(report.stored_bytes()).expect("roundtrip parses");
        assert_eq!(parsed, report);
        match parsed.evidence() {
            NativeExecutionEvidence::Observed {
                termination,
                observation_id,
                ..
            } => {
                assert_eq!(
                    termination,
                    &NativeObservedTermination::Success(ValueHash::from_bytes([0x65; 32]))
                );
                assert_eq!(hex_of(observation_id.as_bytes()), GOLDEN_OBSERVATION_ID);
            }
            NativeExecutionEvidence::Rejected(_) => panic!("golden is observed"),
        }
    }

    #[test]
    fn test_reports_match_independent_python_goldens() {
        let one = NativeTestReportV1::parse(&re_envelope_test(&decode_hex(GOLDEN_TEST_ONE_RECORD)))
            .expect("re-enveloped golden parses");
        assert_eq!(hex_of(one.record_bytes()), GOLDEN_TEST_ONE_RECORD);
        assert_eq!(hex_of(one.report_id().as_bytes()), GOLDEN_TEST_ONE_ID);
        assert_eq!(one.entries().len(), 1);
        let entry = one.entries()[0];
        assert_eq!(entry.test_entity, EntityId::from_bytes([0x51; 32]));
        assert_eq!(entry.test_object, ObjectId::from_bytes([0x52; 32]));
        assert_eq!(
            hex_of(entry.execution_report_id.as_bytes()),
            GOLDEN_EXEC_OBSERVED_ID
        );
        assert_eq!(
            entry.expected,
            NativeExpected::Value(ValueHash::from_bytes([0x65; 32]))
        );
        assert_eq!(entry.comparison, TestComparison::Match);
        assert_eq!(
            (
                one.match_count(),
                one.mismatch_count(),
                one.rejected_count()
            ),
            (1, 0, 0)
        );
        let empty =
            NativeTestReportV1::parse(&re_envelope_test(&decode_hex(GOLDEN_TEST_EMPTY_RECORD)))
                .expect("empty golden parses");
        assert_eq!(hex_of(empty.record_bytes()), GOLDEN_TEST_EMPTY_RECORD);
        assert_eq!(hex_of(empty.report_id().as_bytes()), GOLDEN_TEST_EMPTY_ID);
        assert!(empty.entries().is_empty());
        assert_eq!(
            (
                empty.match_count(),
                empty.mismatch_count(),
                empty.rejected_count()
            ),
            (0, 0, 0)
        );
    }

    fn mapped(error: &NativeExecutionError) -> (u32, u32, String) {
        let rejected = rejected_from_error(error).expect("owner error maps");
        (
            rejected.phase(),
            rejected.numeric_code(),
            rejected.symbol().to_owned(),
        )
    }

    #[test]
    fn rejected_mapping_covers_every_n0_table_row() {
        // Preserved rows: type, lowering(type), lowering(cfg), lowering(lower),
        // fingerprint, status, exec.
        assert_eq!(
            mapped(&NativeExecutionError::Preserved(ExecutionError::Type(
                TypeError::new(TypeErrorCode::DepthLimit)
            ))),
            (1, 21_000, "TYPE_DEPTH_LIMIT".to_owned())
        );
        assert_eq!(
            mapped(&NativeExecutionError::Preserved(ExecutionError::Lowering(
                LoweringError::Cfg(CfgValidationError::Type(TypeError::new(
                    TypeErrorCode::DepthLimit
                )))
            ))),
            (1, 21_000, "TYPE_DEPTH_LIMIT".to_owned())
        );
        assert_eq!(
            mapped(&NativeExecutionError::Preserved(ExecutionError::Lowering(
                LoweringError::Cfg(CfgValidationError::Cfg(CfgError::new(
                    CfgErrorCode::EntryInvalid
                )))
            ))),
            (2, 22_005, "CFG_ENTRY_INVALID".to_owned())
        );
        assert_eq!(
            mapped(&NativeExecutionError::Preserved(ExecutionError::Lowering(
                LoweringError::Lower(LowerError::new(LowerErrorCode::OpcodeUnsupported))
            ))),
            (3, 26_001, "VM_LOWER_OPCODE_UNSUPPORTED".to_owned())
        );
        assert_eq!(
            mapped(&NativeExecutionError::Preserved(
                ExecutionError::Fingerprint(FingerprintError::new(FingerprintErrorCode::Mismatch))
            )),
            (4, 25_004, "FINGERPRINT_MISMATCH".to_owned())
        );
        assert_eq!(
            mapped(&NativeExecutionError::Preserved(ExecutionError::Status(
                ExecutionStatusCode::ResourceLimit
            ))),
            (5, 27_002, "VM_EXEC_RESOURCE_LIMIT".to_owned())
        );
        assert_eq!(
            mapped(&NativeExecutionError::Preserved(ExecutionError::Exec(
                ExecutionErrorCode::InputCountMismatch
            ))),
            (5, 27_000, "VM_EXEC_INPUT_COUNT_MISMATCH".to_owned())
        );
        // Effect rows: both nested type/cfg routes plus the true S20-230 row.
        assert_eq!(
            mapped(&NativeExecutionError::Effect(EffectValidationError::Type(
                TypeError::new(TypeErrorCode::DepthLimit)
            ))),
            (1, 21_000, "TYPE_DEPTH_LIMIT".to_owned())
        );
        assert_eq!(
            mapped(&NativeExecutionError::Effect(EffectValidationError::Cfg(
                CfgValidationError::Type(TypeError::new(TypeErrorCode::DepthLimit))
            ))),
            (1, 21_000, "TYPE_DEPTH_LIMIT".to_owned())
        );
        assert_eq!(
            mapped(&NativeExecutionError::Effect(EffectValidationError::Cfg(
                CfgValidationError::Cfg(CfgError::new(CfgErrorCode::EntryInvalid))
            ))),
            (2, 22_005, "CFG_ENTRY_INVALID".to_owned())
        );
        assert_eq!(
            mapped(&NativeExecutionError::Effect(
                EffectValidationError::Effect(EffectError::new(EffectErrorCode::ClosureMismatch))
            )),
            (7, 23_003, "EFFECT_CLOSURE_MISMATCH".to_owned())
        );
        // Native profile rows share phase six with distinct owner codes.
        assert_eq!(
            mapped(&NativeExecutionError::Profile(
                NativeExecutionProfileError::Unsupported
            )),
            (6, 29_200, "NATIVE_TEST_PROFILE_UNSUPPORTED".to_owned())
        );
        assert_eq!(
            mapped(&NativeExecutionError::Profile(
                NativeExecutionProfileError::InvalidContext
            )),
            (6, 29_208, "NATIVE_TEST_CONTEXT_MISMATCH".to_owned())
        );
        // The mapped golden symbol re-encodes through the strict record.
        let rejected = rejected_from_error(&NativeExecutionError::Preserved(
            ExecutionError::Status(ExecutionStatusCode::ResourceLimit),
        ))
        .expect("golden symbol maps");
        assert_eq!(rejected.symbol(), "VM_EXEC_RESOURCE_LIMIT");
    }

    #[test]
    fn comparison_matrix_matches_shared_kernel_rule() {
        let value = ValueHash::from_bytes([0x65; 32]);
        let other = ValueHash::from_bytes([0x66; 32]);
        let success = NativeObservedTermination::Success(value);
        assert_eq!(
            compare_native_expected(NativeExpected::Value(value), &success),
            TestComparison::Match
        );
        assert_eq!(
            compare_native_expected(NativeExpected::Value(other), &success),
            TestComparison::Mismatch
        );
        let trap = NativeObservedTermination::Trap {
            trap_tag: 2,
            payload: Some(other),
        };
        assert_eq!(
            compare_native_expected(NativeExpected::FailureCode(2), &trap),
            TestComparison::Match
        );
        assert_eq!(
            compare_native_expected(NativeExpected::FailureCode(3), &trap),
            TestComparison::Mismatch
        );
        // Payload hashes never substitute for the expected projection.
        assert_eq!(
            compare_native_expected(NativeExpected::Value(other), &trap),
            TestComparison::Mismatch
        );
        assert_eq!(
            compare_native_expected(NativeExpected::FailureCode(2), &success),
            TestComparison::Mismatch
        );
        for termination in [
            NativeObservedTermination::ResourceLimit(
                sley_vm::native_execution::NativeResourceKind::Fuel,
            ),
            NativeObservedTermination::InternalInvariant,
        ] {
            assert_eq!(
                compare_native_expected(NativeExpected::Value(value), &termination),
                TestComparison::Mismatch
            );
            assert_eq!(
                compare_native_expected(NativeExpected::FailureCode(2), &termination),
                TestComparison::Mismatch
            );
        }
        // Absent trap payload still matches its explicit tag.
        let bare = NativeObservedTermination::Trap {
            trap_tag: 4,
            payload: None,
        };
        assert_eq!(
            compare_native_expected(NativeExpected::FailureCode(4), &bare),
            TestComparison::Match
        );
    }

    #[test]
    fn rejected_parts_refuse_bad_phases_and_symbols() {
        assert_eq!(
            RejectedEvidence::from_parts(0, 27_002, "VM_EXEC_RESOURCE_LIMIT")
                .expect_err("phase zero")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        assert_eq!(
            RejectedEvidence::from_parts(8, 27_002, "VM_EXEC_RESOURCE_LIMIT")
                .expect_err("phase eight")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        assert_eq!(
            RejectedEvidence::from_parts(5, 27_002, "")
                .expect_err("empty")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        assert_eq!(
            RejectedEvidence::from_parts(5, 27_002, &"A".repeat(97))
                .expect_err("long")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        assert_eq!(
            RejectedEvidence::from_parts(5, 27_002, "töp")
                .expect_err("non-ascii")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        RejectedEvidence::from_parts(5, 27_002, &"A".repeat(96)).expect("96 passes");
    }

    fn candidate_plan() -> crate::plan::NativeTestPlanV1 {
        let entry = SelectedEntry {
            test_entity: EntityId::from_bytes([0x51; 32]),
            test_object: ObjectId::from_bytes([0x52; 32]),
            target_function: EntityId::from_bytes([0x53; 32]),
            target_object: ObjectId::from_bytes([0x54; 32]),
            declared_limits: sley_vm::native_execution::NativeDeclaredLimits {
                fuel: 100,
                memory_bytes: 4_096,
                output_bytes: 64,
                effect_count: 0,
                call_depth: 8,
                wall_timeout_millis: 1_000,
            },
        };
        crate::plan::NativeTestPlanV1::build(NativeTestPlanParts {
            selection_mode: crate::plan::SELECTION_MODE_CANDIDATE_AFFECTED,
            workspace: WorkspaceId::from_bytes([0x20; 32]),
            semantic_epoch: SchemaEpochId::from_bytes([0x21; 32]),
            parent_transaction: TransactionId::from_bytes([0x22; 32]),
            parent_root: StateRoot::from_bytes([0x23; 32]),
            proposed_root: StateRoot::from_bytes([0x24; 32]),
            policy_root: PolicyRootId::from_bytes([0x10; 32]),
            candidate_id: Some(CandidateId::from_bytes([0x30; 32])),
            static_result_id: Some(CandidateResultId::from_bytes([0x31; 32])),
            protected_required_ids: Vec::new(),
            selected: vec![entry],
            changed: vec![ChangedTest {
                test_entity: EntityId::from_bytes([0x51; 32]),
                before: None,
                after: Some(ObjectId::from_bytes([0x52; 32])),
            }],
            implementation_limits:
                sley_vm::native_execution::NativeImplementationLimits::HARD_MAXIMA,
            static_selected_ids: Vec::new(),
            resource_policy: NativeResourcePolicyV1::build(policy_parts()).expect("policy builds"),
        })
        .expect("candidate plan builds")
    }

    fn covering_entry(comparison: TestComparison) -> NativeTestEntry {
        NativeTestEntry {
            test_entity: EntityId::from_bytes([0x51; 32]),
            test_object: ObjectId::from_bytes([0x52; 32]),
            execution_report_id: ExecutionReportId::from_bytes([0x60; 32]),
            expected: NativeExpected::Value(ValueHash::from_bytes([0x65; 32])),
            comparison,
        }
    }

    #[test]
    fn plan_coverage_binds_exact_selection() {
        let plan = candidate_plan();
        let report = NativeTestReportV1::build(&plan, vec![covering_entry(TestComparison::Match)])
            .expect("covering report builds");
        assert_eq!(
            (
                report.match_count(),
                report.mismatch_count(),
                report.rejected_count()
            ),
            (1, 0, 0)
        );
        assert_eq!(report.plan_id(), plan.plan_id());
        let parsed = NativeTestReportV1::parse(report.stored_bytes()).expect("roundtrip parses");
        assert_eq!(parsed, report);
        parsed.verify_plan_coverage(&plan).expect("coverage holds");
        let mut wrong_object = covering_entry(TestComparison::Match);
        wrong_object.test_object = ObjectId::from_bytes([0x59; 32]);
        assert_eq!(
            NativeTestReportV1::build(&plan, vec![wrong_object])
                .expect_err("object")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        assert_eq!(
            NativeTestReportV1::build(&plan, Vec::new())
                .expect_err("missing")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let other = candidate_plan();
        assert_eq!(other.plan_id(), plan.plan_id());
    }

    #[test]
    fn report_order_counts_and_ranges_are_exact() {
        let plan = candidate_plan();
        let mut swapped = vec![
            covering_entry(TestComparison::Match),
            covering_entry(TestComparison::Mismatch),
        ];
        swapped[1].test_entity = EntityId::from_bytes([0x50; 32]);
        assert_eq!(
            NativeTestReportV1::build(&plan, swapped)
                .expect_err("order first")
                .code(),
            ScbErrorCode::FieldOrder
        );
        let mut duplicate = vec![
            covering_entry(TestComparison::Match),
            covering_entry(TestComparison::Match),
        ];
        duplicate[1].execution_report_id = ExecutionReportId::from_bytes([0x61; 32]);
        assert_eq!(
            NativeTestReportV1::build(&plan, duplicate)
                .expect_err("duplicate")
                .code(),
            ScbErrorCode::FieldDuplicate
        );
        // Totals mismatch refuses at parse even with a valid digest.
        let report = NativeTestReportV1::build(&plan, vec![covering_entry(TestComparison::Match)])
            .expect("report builds");
        let fields = raw_fields(report.record_bytes()).expect("fields decode");
        let mut edited = fields;
        for (tag, value) in &mut edited {
            if *tag == 6 {
                *value = sley_scb1::encode_uvar(0);
            }
        }
        assert_eq!(
            NativeTestReportV1::parse(&re_envelope_test(
                &raw_record(&edited).expect("edited encodes")
            ))
            .expect_err("totals")
            .code(),
            ScbErrorCode::ContractUnknown
        );
        // Failure codes outside 1..=4 refuse at entry parse.
        let mut bad_code = covering_entry(TestComparison::Match);
        bad_code.expected = NativeExpected::FailureCode(5);
        assert_eq!(
            NativeTestReportV1::build(&plan, vec![bad_code])
                .expect_err("code range")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        // Unknown comparison tags refuse at parse.
        let fixed = raw_record(&[
            (1, sley_scb1::encode_uvar(1)),
            (2, plan.plan_id().as_bytes().to_vec()),
            (3, plan.semantic_epoch().as_bytes().to_vec()),
            (4, plan.proposed_root().as_bytes().to_vec()),
            (
                5,
                raw_list(&[raw_record(&[
                    (1, EntityId::from_bytes([0x51; 32]).as_bytes().to_vec()),
                    (2, ObjectId::from_bytes([0x52; 32]).as_bytes().to_vec()),
                    (
                        3,
                        ExecutionReportId::from_bytes([0x60; 32])
                            .as_bytes()
                            .to_vec(),
                    ),
                    (
                        4,
                        NativeExpected::Value(ValueHash::from_bytes([0x65; 32]))
                            .record()
                            .expect("expected encodes"),
                    ),
                    (5, sley_scb1::encode_uvar(4)),
                ])
                .expect("entry encodes")])
                .expect("list encodes"),
            ),
            (6, sley_scb1::encode_uvar(0)),
            (7, sley_scb1::encode_uvar(0)),
            (8, sley_scb1::encode_uvar(1)),
        ])
        .expect("record encodes");
        assert_eq!(
            NativeTestReportV1::parse(&re_envelope_test(&fixed))
                .expect_err("comparison tag")
                .code(),
            ScbErrorCode::ContractUnknown
        );
    }

    #[test]
    fn parse_rejects_digest_embedded_tamper_and_size() {
        let report = NativeExecutionReportV1::build(observed_parts()).expect("observed builds");
        let mut digest = report.stored_bytes().to_vec();
        let last = digest.len() - 1;
        digest[last] ^= 0x01;
        assert_eq!(
            NativeExecutionReportV1::parse(&digest)
                .expect_err("digest")
                .code(),
            ScbErrorCode::DigestMismatch
        );
        // Tamper inside the embedded observation, then re-envelope so only
        // the embedded digest fails.
        let fields = raw_fields(report.record_bytes()).expect("fields decode");
        let mut edited = fields;
        for (tag, value) in &mut edited {
            if *tag == 6 {
                let mut cursor = ScbValueCursor::new(value).expect("evidence cursor");
                let (union_tag, payload) = cursor.read_union().expect("union reads");
                assert_eq!(union_tag, 1);
                let mut embedded = payload.to_vec();
                let flip = embedded.len() / 2;
                embedded[flip] ^= 0x01;
                *value = sley_scb1::encode_union(1, &embedded).expect("union encodes");
            }
        }
        assert_eq!(
            NativeExecutionReportV1::parse(&re_envelope_exec(
                &raw_record(&edited).expect("edited encodes")
            ))
            .expect_err("embedded digest")
            .code(),
            ScbErrorCode::DigestMismatch
        );
        // The report ceiling applies before any other parse check.
        let mut oversized = report.stored_bytes().to_vec();
        oversized.resize(MAX_EXECUTION_REPORT_STORED + 1, 0x00);
        assert_eq!(
            NativeExecutionReportV1::parse(&oversized)
                .expect_err("size")
                .code(),
            ScbErrorCode::ResourceLimit
        );
    }

    #[test]
    fn build_refuses_oversized_and_profile_substituted_evidence() {
        // 7,930 input hashes fit the observation ceiling but overflow the
        // report wrapper: obs 262,089 bytes, report 262,281 bytes.
        use crate::codec::decode_envelope as obs_envelope;
        let stored = decode_hex(GOLDEN_OBSERVATION_STORED);
        let (record, _) = obs_envelope(&stored, *b"SLEYNOB1").expect("golden envelope decodes");
        let fields = raw_fields(&record).expect("obs fields decode");
        let mut edited = fields;
        let hashes: Vec<Vec<u8>> = (0..7_930_u32)
            .map(|n| {
                let mut hash = [0x64; 32];
                hash[0..4].copy_from_slice(&n.to_le_bytes());
                hash.to_vec()
            })
            .collect();
        for (tag, value) in &mut edited {
            if *tag == 9 {
                *value = raw_list(&hashes).expect("hashes encode");
            }
        }
        let obs_record = raw_record(&edited).expect("obs encodes");
        let obs_preimage = raw_pre(*b"SLEYNOB1", &obs_record).expect("preimage fits");
        let obs_id = NativeObservationId::derive(&obs_preimage);
        let mut obs_stored = obs_preimage;
        obs_stored.extend_from_slice(obs_id.as_bytes());
        let big = NativeExecutionReportParts {
            evidence: NativeExecutionEvidence::observed(obs_stored).expect("big obs parses"),
            ..observed_parts()
        };
        assert_eq!(
            NativeExecutionReportV1::build(big)
                .expect_err("report ceiling")
                .code(),
            ScbErrorCode::ResourceLimit
        );
        // A substituted profile re-enveloped with a valid digest still refuses.
        let mut swapped = raw_fields(&record).expect("obs fields decode");
        for (tag, value) in &mut swapped {
            if *tag == 8 {
                *value = vec![0x00; 32];
            }
        }
        let swapped_record = raw_record(&swapped).expect("swapped encodes");
        let swapped_preimage = raw_pre(*b"SLEYNOB1", &swapped_record).expect("preimage fits");
        let swapped_id = NativeObservationId::derive(&swapped_preimage);
        let mut swapped_stored = swapped_preimage;
        swapped_stored.extend_from_slice(swapped_id.as_bytes());
        assert_eq!(
            NativeExecutionEvidence::observed(swapped_stored)
                .expect_err("profile")
                .code(),
            ScbErrorCode::ContractUnknown
        );
    }
}
