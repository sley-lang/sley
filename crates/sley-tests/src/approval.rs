//! Native test approval (`SLEYNAP1`) from `NATIVE_TEST_ADMISSION_V1.md` section 3.
//!
//! The approval binds the candidate, its fresh static result, the validation
//! context, parent and proposed roots, policy root, plan, deterministic test
//! report, ordered attestation bindings, measurement trust policy, resource
//! policy, historical context, and the accept/reject decision. Parsing yields
//! a strictly validated record; only verification against a fresh policy-owner
//! plan and configured measured authority (N5) constructs approval authority.
//! This crate never verifies signatures or grants commit power.

use sley_id::{
    CandidateId, CandidateResultId, ContextCapsuleId, EntityId, MeasuredTestAttestationId,
    NativeResourcePolicyId, NativeTestApprovalId, NativeTestPlanId, PolicyRootId, StateRoot,
    TestReportId, TransactionId,
};
use sley_scb1::{
    ScbError, ScbErrorCode, ScbValueCursor, encode_record, encode_text, encode_union, encode_uvar,
};

use crate::codec::{
    MAX_SELECTED_ENTRIES, RECORD_VERSION, check_sorted_unique, decode_envelope, decode_fields,
    expect_tags, preimage_bytes, read_id, read_option_id, read_uvar_value,
};
use crate::report::MAX_SYMBOL_BYTES;

/// `SLEYNAP1` envelope magic for [`NativeTestApprovalV1`].
pub const APPROVAL_MAGIC: [u8; 8] = *b"SLEYNAP1";
/// Accepted decision union tag: empty payload, no failure record.
pub const DECISION_ACCEPTED: u32 = 1;
/// Rejected decision union tag: payload is one [`NativeFailureRecord`].
pub const DECISION_REJECTED: u32 = 2;
/// Absent failure-detail union tag.
pub const DETAIL_NONE: u32 = 0;
/// Resource-detail union tag: `{test, resource, actual, limit}`.
pub const DETAIL_RESOURCE: u32 = 1;
/// Identity-mismatch detail union tag: `{expected, actual}`.
pub const DETAIL_ID_MISMATCH: u32 = 2;
/// Phase detail union tag: `{phase}`.
pub const DETAIL_PHASE: u32 = 3;
/// Declared fuel field identifier in resource details.
pub const RESOURCE_FUEL: u32 = 1;
/// Declared memory field identifier in resource details.
pub const RESOURCE_MEMORY: u32 = 2;
/// Declared output field identifier in resource details.
pub const RESOURCE_OUTPUT: u32 = 3;
/// Declared effects field identifier in resource details.
pub const RESOURCE_EFFECTS: u32 = 4;
/// Declared depth field identifier in resource details.
pub const RESOURCE_DEPTH: u32 = 5;
/// Declared wall field identifier in resource details.
pub const RESOURCE_WALL: u32 = 6;
/// Selected-count identifier in resource details.
pub const RESOURCE_COUNT: u32 = 7;
/// Evidence-bytes identifier in resource details.
pub const RESOURCE_EVIDENCE: u32 = 8;

/// Resource-limit detail with the exact offending measurement, if any.
///
/// The `test` identity is present when the detail concerns one selected test;
/// aggregate details leave it absent. `resource` is one of the `RESOURCE_*`
/// identifiers above, which name literal policy fields and are a different
/// enum from the VM termination resource tags.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceDetail {
    /// Selected test the detail concerns, if any.
    pub test: Option<EntityId>,
    /// Literal policy field identifier, 1..=8.
    pub resource: u32,
    /// Measured or declared value that exceeded the ceiling.
    pub actual: u64,
    /// Ceiling that was exceeded.
    pub limit: u64,
}

/// Identity-substitution detail: what the binding required versus found.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdMismatchDetail {
    /// Required identity bytes.
    pub expected: [u8; 32],
    /// Identity bytes actually referenced.
    pub actual: [u8; 32],
}

/// Optional native failure detail, union tags 0..=3.
///
/// Tag 0 carries no payload. The record shapes are fixed by this codec:
/// resource details are tags `[1, 2, 3, 4]`, identity mismatches `[1, 2]`,
/// and phase details `[1]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeDetail {
    /// No further detail beyond numeric code and symbol.
    None,
    /// Field-level limit breach with measured values.
    Resource(ResourceDetail),
    /// Expected-versus-actual identity bytes.
    IdMismatch(IdMismatchDetail),
    /// Rejection phase, 1..=7 as in report evidence.
    Phase(u32),
}

/// Native failure record: exact numeric code, ASCII symbol, optional detail.
///
/// Symbols are ASCII, 1..=96 bytes, matching the rejected-evidence bound so
/// approval and report diagnostics share one alphabet. Numeric codes are the
/// reserved native assignments; earlier owner codes are never remapped here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFailureRecord {
    /// Exact native numeric failure code.
    pub numeric_code: u32,
    /// Exact native symbol.
    pub symbol: String,
    /// Optional structured detail.
    pub detail: NativeDetail,
}

impl NativeFailureRecord {
    /// Builds a validated failure record from caller-supplied facts.
    ///
    /// # Errors
    /// Returns `SCB_CONTRACT_UNKNOWN` for an empty, oversized, or non-ASCII
    /// symbol, a resource identifier outside 1..=8, or a phase outside 1..=7.
    pub fn from_parts(
        numeric_code: u32,
        symbol: &str,
        detail: NativeDetail,
    ) -> Result<Self, ScbError> {
        if symbol.is_empty() || symbol.len() > MAX_SYMBOL_BYTES || !symbol.is_ascii() {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        match &detail {
            NativeDetail::None | NativeDetail::IdMismatch(_) => {}
            NativeDetail::Resource(inner) => {
                if !(RESOURCE_FUEL..=RESOURCE_EVIDENCE).contains(&inner.resource) {
                    return Err(ScbError::new(ScbErrorCode::ContractUnknown));
                }
            }
            NativeDetail::Phase(phase) => {
                if !(1..=7).contains(phase) {
                    return Err(ScbError::new(ScbErrorCode::ContractUnknown));
                }
            }
        }
        Ok(Self {
            numeric_code,
            symbol: symbol.to_owned(),
            detail,
        })
    }

    /// Exact native numeric failure code.
    #[must_use]
    pub const fn numeric_code(&self) -> u32 {
        self.numeric_code
    }

    /// Exact native symbol.
    #[must_use]
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    fn record(&self) -> Result<Vec<u8>, ScbError> {
        encode_record(&[
            (1, encode_uvar(u64::from(self.numeric_code))),
            (2, encode_text(&self.symbol)?),
            (3, encode_detail(&self.detail)?),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2, 3])?;
        let numeric_code = u32::try_from(read_uvar_value(&fields[0].1, 32)?)
            .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
        let symbol = {
            let mut cursor = ScbValueCursor::new(&fields[1].1)?;
            let text = cursor.read_text()?;
            cursor.check_finished()?;
            text.to_owned()
        };
        let detail = parse_detail(&fields[2].1)?;
        Self::from_parts(numeric_code, &symbol, detail)
    }
}

fn encode_detail(detail: &NativeDetail) -> Result<Vec<u8>, ScbError> {
    match detail {
        NativeDetail::None => encode_union(DETAIL_NONE, &[]),
        NativeDetail::Resource(inner) => {
            let record = encode_record(&[
                (
                    1,
                    crate::codec::encode_option_id(inner.test.map(|id| *id.as_bytes()))?,
                ),
                (2, encode_uvar(u64::from(inner.resource))),
                (3, encode_uvar(inner.actual)),
                (4, encode_uvar(inner.limit)),
            ])?;
            encode_union(DETAIL_RESOURCE, &record)
        }
        NativeDetail::IdMismatch(inner) => {
            let record =
                encode_record(&[(1, inner.expected.to_vec()), (2, inner.actual.to_vec())])?;
            encode_union(DETAIL_ID_MISMATCH, &record)
        }
        NativeDetail::Phase(phase) => {
            let record = encode_record(&[(1, encode_uvar(u64::from(*phase)))])?;
            encode_union(DETAIL_PHASE, &record)
        }
    }
}

fn parse_detail(value: &[u8]) -> Result<NativeDetail, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let (tag, payload) = cursor.read_union()?;
    cursor.check_finished()?;
    match tag {
        DETAIL_NONE => {
            if payload.is_empty() {
                Ok(NativeDetail::None)
            } else {
                Err(ScbError::new(ScbErrorCode::UnionInvalid))
            }
        }
        DETAIL_RESOURCE => {
            let fields = decode_fields(payload)?;
            expect_tags(&fields, &[1, 2, 3, 4])?;
            let resource = u32::try_from(read_uvar_value(&fields[1].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
            Ok(NativeDetail::Resource(ResourceDetail {
                test: read_option_id(&fields[0].1)?.map(EntityId::from_bytes),
                resource,
                actual: read_uvar_value(&fields[2].1, 64)?,
                limit: read_uvar_value(&fields[3].1, 64)?,
            }))
        }
        DETAIL_ID_MISMATCH => {
            let fields = decode_fields(payload)?;
            expect_tags(&fields, &[1, 2])?;
            Ok(NativeDetail::IdMismatch(IdMismatchDetail {
                expected: read_id(&fields[0].1)?,
                actual: read_id(&fields[1].1)?,
            }))
        }
        DETAIL_PHASE => {
            let fields = decode_fields(payload)?;
            expect_tags(&fields, &[1])?;
            let phase = u32::try_from(read_uvar_value(&fields[0].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
            Ok(NativeDetail::Phase(phase))
        }
        _ => Err(ScbError::new(ScbErrorCode::UnionInvalid)),
    }
}

/// Approval decision: accepted with no payload, or rejected with a record.
///
/// Accepted requires the N5 verifier to have checked exact one-to-one
/// report/measurement/plan selection, every comparison match, authorized
/// measurement signatures, and all limits. Parsing an accepted decision
/// proves none of that; it only proves the bytes are canonical.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApprovalDecision {
    /// All checks passed; carries no payload.
    Accepted,
    /// Rejected with the exact native failure record.
    Rejected(NativeFailureRecord),
}

impl ApprovalDecision {
    fn encode(&self) -> Result<Vec<u8>, ScbError> {
        match self {
            Self::Accepted => encode_union(DECISION_ACCEPTED, &[]),
            Self::Rejected(record) => encode_union(DECISION_REJECTED, &record.record()?),
        }
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let mut cursor = ScbValueCursor::new(value)?;
        let (tag, payload) = cursor.read_union()?;
        cursor.check_finished()?;
        match tag {
            DECISION_ACCEPTED => {
                if payload.is_empty() {
                    Ok(Self::Accepted)
                } else {
                    Err(ScbError::new(ScbErrorCode::UnionInvalid))
                }
            }
            DECISION_REJECTED => Ok(Self::Rejected(NativeFailureRecord::parse(payload)?)),
            _ => Err(ScbError::new(ScbErrorCode::UnionInvalid)),
        }
    }
}

/// One attestation binding: selected test to its measured attestation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttestationBinding {
    /// Selected test entity.
    pub test_entity: EntityId,
    /// Measured attestation covering that test.
    pub attestation_id: MeasuredTestAttestationId,
}

impl AttestationBinding {
    fn record(self) -> Result<Vec<u8>, ScbError> {
        encode_record(&[
            (1, self.test_entity.as_bytes().to_vec()),
            (2, self.attestation_id.as_bytes().to_vec()),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2])?;
        Ok(Self {
            test_entity: EntityId::from_bytes(read_id(&fields[0].1)?),
            attestation_id: MeasuredTestAttestationId::from_bytes(read_id(&fields[1].1)?),
        })
    }
}

/// Caller-supplied approval facts; constructing these grants no authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeTestApprovalParts {
    /// Candidate under test.
    pub candidate: CandidateId,
    /// Fresh static result the plan was derived from.
    pub static_result: CandidateResultId,
    /// Exact validation context the static result was produced under.
    pub validation_context: ContextCapsuleId,
    /// Exact accepted parent transaction.
    pub parent_transaction: TransactionId,
    /// Exact accepted parent root.
    pub parent_root: StateRoot,
    /// Proposed root under test.
    pub proposed_root: StateRoot,
    /// Protected policy root.
    pub policy_root: PolicyRootId,
    /// Approved test plan.
    pub plan_id: NativeTestPlanId,
    /// Deterministic test report the decision covers.
    pub test_report_id: TestReportId,
    /// Attestation bindings, strictly test-ID sorted, at most 256.
    pub attestations: Vec<AttestationBinding>,
    /// Receiver-provisioned measurement trust policy the signatures use.
    ///
    /// The `SLEYNTR1` domain is not yet promoted; N1d registers it and
    /// tightens this to its typed identity.
    pub measurement_trust_policy: [u8; 32],
    /// Exact effective resource policy the plan was checked against.
    pub resource_policy_id: NativeResourcePolicyId,
    /// Historical admission context binding the original authorization.
    ///
    /// The `SLEYNCT1` domain is not yet promoted; N1d registers it and
    /// tightens this to its typed identity.
    pub historical_context_id: [u8; 32],
    /// Accept or reject with the exact failure record.
    pub decision: ApprovalDecision,
}

/// Immutable canonical test approval; external callers cannot forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeTestApprovalV1 {
    parts: NativeTestApprovalParts,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: NativeTestApprovalId,
}

impl NativeTestApprovalV1 {
    /// Builds a validated approval from caller-supplied facts.
    ///
    /// # Errors
    /// Returns `SCB_FIELD_ORDER`/`SCB_FIELD_DUPLICATE` for unsorted or
    /// repeated binding identities, or `SCB_RESOURCE_LIMIT` beyond 256
    /// bindings.
    pub fn build(parts: NativeTestApprovalParts) -> Result<Self, ScbError> {
        validate_parts(&parts)?;
        let record = record_parts(&parts)?;
        let preimage = preimage_bytes(APPROVAL_MAGIC, &record)?;
        let id = NativeTestApprovalId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        Ok(Self {
            parts,
            record,
            stored,
            id,
        })
    }

    /// Strictly parses and validates one stored approval envelope.
    ///
    /// Parsing checks shape, order, bindings, and digest. It does not verify
    /// signatures, re-derive selection, or admit anything.
    ///
    /// # Errors
    /// Returns the first stable SCB1 failure encountered while decoding the
    /// envelope, verifying its digest, or validating fields.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        let (record, trailer) = decode_envelope(stored, APPROVAL_MAGIC)?;
        let preimage = preimage_bytes(APPROVAL_MAGIC, &record)?;
        if NativeTestApprovalId::derive(&preimage).into_bytes() != trailer {
            return Err(ScbError::new(ScbErrorCode::DigestMismatch));
        }
        let fields = decode_fields(&record)?;
        expect_tags(
            &fields,
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        )?;
        if read_uvar_value(&fields[0].1, 32)? != RECORD_VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        Self::build(NativeTestApprovalParts {
            candidate: CandidateId::from_bytes(read_id(&fields[1].1)?),
            static_result: CandidateResultId::from_bytes(read_id(&fields[2].1)?),
            validation_context: ContextCapsuleId::from_bytes(read_id(&fields[3].1)?),
            parent_transaction: TransactionId::from_bytes(read_id(&fields[4].1)?),
            parent_root: StateRoot::from_bytes(read_id(&fields[5].1)?),
            proposed_root: StateRoot::from_bytes(read_id(&fields[6].1)?),
            policy_root: PolicyRootId::from_bytes(read_id(&fields[7].1)?),
            plan_id: NativeTestPlanId::from_bytes(read_id(&fields[8].1)?),
            test_report_id: TestReportId::from_bytes(read_id(&fields[9].1)?),
            attestations: parse_bindings(&fields[10].1)?,
            measurement_trust_policy: read_id(&fields[11].1)?,
            resource_policy_id: NativeResourcePolicyId::from_bytes(read_id(&fields[12].1)?),
            historical_context_id: read_id(&fields[13].1)?,
            decision: ApprovalDecision::parse(&fields[14].1)?,
        })
    }

    /// Canonical SCB1 record without envelope or trailer.
    #[must_use]
    pub fn record_bytes(&self) -> &[u8] {
        &self.record
    }

    /// Complete stored envelope with digest trailer.
    #[must_use]
    pub fn stored_bytes(&self) -> &[u8] {
        &self.stored
    }

    /// Domain-separated approval identity.
    #[must_use]
    pub const fn id(&self) -> NativeTestApprovalId {
        self.id
    }

    /// Plan the decision covers; the bundle requires it to match.
    #[must_use]
    pub const fn plan_id(&self) -> NativeTestPlanId {
        self.parts.plan_id
    }

    /// Deterministic test report the decision covers.
    #[must_use]
    pub const fn test_report_id(&self) -> TestReportId {
        self.parts.test_report_id
    }

    /// Attestation bindings in strict test-ID order.
    #[must_use]
    pub fn attestations(&self) -> &[AttestationBinding] {
        &self.parts.attestations
    }
}

fn validate_parts(parts: &NativeTestApprovalParts) -> Result<(), ScbError> {
    if u64::try_from(parts.attestations.len()).map_err(|_| resource_error())? > MAX_SELECTED_ENTRIES
    {
        return Err(resource_error());
    }
    let keys: Vec<[u8; 32]> = parts
        .attestations
        .iter()
        .map(|binding| *binding.test_entity.as_bytes())
        .collect();
    check_sorted_unique(&keys)
}

fn resource_error() -> ScbError {
    ScbError::new(ScbErrorCode::ResourceLimit)
}

fn record_parts(parts: &NativeTestApprovalParts) -> Result<Vec<u8>, ScbError> {
    encode_record(&[
        (1, encode_uvar(RECORD_VERSION)),
        (2, parts.candidate.as_bytes().to_vec()),
        (3, parts.static_result.as_bytes().to_vec()),
        (4, parts.validation_context.as_bytes().to_vec()),
        (5, parts.parent_transaction.as_bytes().to_vec()),
        (6, parts.parent_root.as_bytes().to_vec()),
        (7, parts.proposed_root.as_bytes().to_vec()),
        (8, parts.policy_root.as_bytes().to_vec()),
        (9, parts.plan_id.as_bytes().to_vec()),
        (10, parts.test_report_id.as_bytes().to_vec()),
        (11, encode_bindings(&parts.attestations)?),
        (12, parts.measurement_trust_policy.to_vec()),
        (13, parts.resource_policy_id.as_bytes().to_vec()),
        (14, parts.historical_context_id.to_vec()),
        (15, parts.decision.encode()?),
    ])
}

fn encode_bindings(bindings: &[AttestationBinding]) -> Result<Vec<u8>, ScbError> {
    let mut items = Vec::new();
    for binding in bindings {
        items.push(binding.record()?);
    }
    sley_scb1::encode_list(&items)
}

fn parse_bindings(value: &[u8]) -> Result<Vec<AttestationBinding>, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_list_count()?;
    if count > MAX_SELECTED_ENTRIES {
        return Err(resource_error());
    }
    let mut bindings = Vec::new();
    for _ in 0..count {
        bindings.push(AttestationBinding::parse(cursor.read_bytes()?)?);
    }
    cursor.check_finished()?;
    let keys: Vec<[u8; 32]> = bindings
        .iter()
        .map(|binding| *binding.test_entity.as_bytes())
        .collect();
    check_sorted_unique(&keys)?;
    Ok(bindings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sley_scb1::encode_list;

    fn decode_hex(hex: &str) -> Vec<u8> {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex decodes"))
            .collect();
        bytes
    }

    fn golden(field: &str) -> Vec<u8> {
        let text = include_str!(
            "/home/gfarch/Work/checkpoints/sley2-finish-20260915/native-approval-golden.json"
        );
        let marker = format!("\"{field}\": \"");
        let start = text.find(&marker).expect("golden field present") + marker.len();
        let end = text[start..].find('"').expect("golden field ends") + start;
        decode_hex(&text[start..end])
    }

    #[test]
    fn golden_vectors_parse_with_exact_ids() {
        for (record_field, stored_field, id_field) in [
            (
                "approval_accepted_record",
                "approval_accepted_stored",
                "approval_accepted_id",
            ),
            (
                "approval_rejected_record",
                "approval_rejected_stored",
                "approval_rejected_id",
            ),
        ] {
            let parsed = NativeTestApprovalV1::parse(&golden(stored_field)).expect("golden parses");
            assert_eq!(parsed.record_bytes(), golden(record_field));
            assert_eq!(parsed.stored_bytes(), golden(stored_field));
            assert_eq!(parsed.id().as_bytes().to_vec(), golden(id_field));
        }
        let accepted =
            NativeTestApprovalV1::parse(&golden("approval_accepted_stored")).expect("parses");
        assert_eq!(accepted.parts.attestations.len(), 1);
        assert!(matches!(
            accepted.parts.decision,
            ApprovalDecision::Accepted
        ));
        let rejected =
            NativeTestApprovalV1::parse(&golden("approval_rejected_stored")).expect("parses");
        assert!(rejected.parts.attestations.is_empty());
        match &rejected.parts.decision {
            ApprovalDecision::Rejected(record) => {
                assert_eq!(record.numeric_code(), 29211);
                assert_eq!(record.symbol(), "NATIVE_TEST_EXECUTION_REJECTED");
                assert_eq!(record.detail, NativeDetail::Phase(5));
            }
            ApprovalDecision::Accepted => panic!("rejected golden must reject"),
        }
    }

    #[test]
    fn decision_and_detail_roundtrips_cover_every_variant() {
        for decision in [
            ApprovalDecision::Accepted,
            ApprovalDecision::Rejected(
                NativeFailureRecord::from_parts(
                    29200,
                    "NATIVE_TEST_PROFILE_UNSUPPORTED",
                    NativeDetail::None,
                )
                .expect("record builds"),
            ),
            ApprovalDecision::Rejected(
                NativeFailureRecord::from_parts(
                    29204,
                    "NATIVE_TEST_DECLARED_LIMIT_EXCEEDS_POLICY",
                    NativeDetail::Resource(ResourceDetail {
                        test: Some(EntityId::from_bytes([0x11; 32])),
                        resource: RESOURCE_MEMORY,
                        actual: 8192,
                        limit: 4096,
                    }),
                )
                .expect("record builds"),
            ),
            ApprovalDecision::Rejected(
                NativeFailureRecord::from_parts(
                    29217,
                    "NATIVE_TEST_ADMISSION_BINDING_MISMATCH",
                    NativeDetail::IdMismatch(IdMismatchDetail {
                        expected: [0x21; 32],
                        actual: [0x22; 32],
                    }),
                )
                .expect("record builds"),
            ),
            ApprovalDecision::Rejected(
                NativeFailureRecord::from_parts(
                    29211,
                    "NATIVE_TEST_EXECUTION_REJECTED",
                    NativeDetail::Phase(7),
                )
                .expect("record builds"),
            ),
        ] {
            let encoded = decision.encode().expect("decision encodes");
            let parsed = ApprovalDecision::parse(&encoded).expect("decision parses");
            assert_eq!(parsed, decision);
        }
        let aggregate = NativeDetail::Resource(ResourceDetail {
            test: None,
            resource: RESOURCE_EVIDENCE,
            actual: 1,
            limit: 0,
        });
        let encoded = encode_detail(&aggregate).expect("detail encodes");
        assert_eq!(parse_detail(&encoded).expect("detail parses"), aggregate);
    }

    #[test]
    fn approval_refusals_keep_stable_codes() {
        let stored = golden("approval_accepted_stored");
        assert_eq!(
            NativeTestApprovalV1::parse(&stored[..stored.len() - 1])
                .expect_err("truncated")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        let mut tampered = stored.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert_eq!(
            NativeTestApprovalV1::parse(&tampered)
                .expect_err("digest")
                .code(),
            ScbErrorCode::DigestMismatch
        );
        let record = golden("approval_accepted_record");
        let fields = decode_fields(&record).expect("fields decode");
        let mut reordered = fields.clone();
        reordered.swap(0, 1);
        // `encode_record` canonicalizes order, so frame the swapped tags
        // manually to present genuinely misordered bytes to the decoder.
        let mut bad = sley_scb1::encode_uvar(reordered.len() as u64);
        for (tag, value) in &reordered {
            bad.extend_from_slice(&sley_scb1::encode_uvar(u64::from(*tag)));
            bad.extend_from_slice(&sley_scb1::encode_bytes(value).expect("small field encodes"));
        }
        let preimage = preimage_bytes(APPROVAL_MAGIC, &bad).expect("preimage encodes");
        let id = NativeTestApprovalId::derive(&preimage);
        let mut bad_stored = preimage;
        bad_stored.extend_from_slice(id.as_bytes());
        assert_eq!(
            NativeTestApprovalV1::parse(&bad_stored)
                .expect_err("order")
                .code(),
            ScbErrorCode::FieldOrder
        );
        assert_eq!(
            NativeFailureRecord::from_parts(1, "", NativeDetail::None)
                .expect_err("empty symbol")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        assert_eq!(
            NativeFailureRecord::from_parts(
                1,
                "NATIVE_TEST_PROFILE_UNSUPPORTED",
                NativeDetail::Phase(8)
            )
            .expect_err("bad phase")
            .code(),
            ScbErrorCode::ContractUnknown
        );
        assert_eq!(
            NativeFailureRecord::from_parts(
                1,
                "NATIVE_TEST_PROFILE_UNSUPPORTED",
                NativeDetail::Resource(ResourceDetail {
                    test: None,
                    resource: 9,
                    actual: 0,
                    limit: 0,
                }),
            )
            .expect_err("bad resource")
            .code(),
            ScbErrorCode::ContractUnknown
        );
        assert_eq!(
            ApprovalDecision::parse(&encode_union(3, &[]).expect("union encodes"))
                .expect_err("bad decision tag")
                .code(),
            ScbErrorCode::UnionInvalid
        );
        assert_eq!(
            ApprovalDecision::parse(&encode_union(1, &[0x01]).expect("union encodes"))
                .expect_err("nonempty accept")
                .code(),
            ScbErrorCode::UnionInvalid
        );
    }

    #[test]
    fn binding_order_and_duplicates_refuse() {
        let bindings = vec![
            AttestationBinding {
                test_entity: EntityId::from_bytes([0x02; 32]),
                attestation_id: MeasuredTestAttestationId::from_bytes([0x0a; 32]),
            },
            AttestationBinding {
                test_entity: EntityId::from_bytes([0x01; 32]),
                attestation_id: MeasuredTestAttestationId::from_bytes([0x0b; 32]),
            },
        ];
        let encoded = encode_bindings(&bindings).expect("bindings encode");
        assert_eq!(
            parse_bindings(&encoded).expect_err("order").code(),
            ScbErrorCode::FieldOrder
        );
        let dup = vec![
            AttestationBinding {
                test_entity: EntityId::from_bytes([0x01; 32]),
                attestation_id: MeasuredTestAttestationId::from_bytes([0x0a; 32]),
            },
            AttestationBinding {
                test_entity: EntityId::from_bytes([0x01; 32]),
                attestation_id: MeasuredTestAttestationId::from_bytes([0x0b; 32]),
            },
        ];
        let encoded = encode_bindings(&dup).expect("bindings encode");
        assert_eq!(
            parse_bindings(&encoded).expect_err("duplicate").code(),
            ScbErrorCode::FieldDuplicate
        );
        let over = encode_list(&vec![bindings[0].record().expect("record encodes"); 257])
            .expect("list encodes");
        assert_eq!(
            parse_bindings(&over).expect_err("over count").code(),
            ScbErrorCode::ResourceLimit
        );
    }
}
