//! Commit admission statement (`SLEYNSA1`) from `NATIVE_TEST_ADMISSION_V1.md` §5.
//!
//! The statement is the acceptance signer's attestation that the configured
//! transaction authority checked original issuer-secret capability
//! authentication, expiry at the preserved historical time, protected
//! policy, fresh static validation, selected test execution, and native
//! approval before authorizing the transaction. It binds the admission
//! profile, workspace, principal, parent, candidate, static result,
//! historical context, static-context digest, resource policy, approval,
//! committed root, evidence bundle, transaction, historical validation
//! time, and acceptance trust policy, plus the 64-byte signature. The
//! signature signs ASCII `sley2.native-test-admission-signature.v1`
//! followed by `P(SLEYNSA1, fields 1..18)` in strict Ed25519 pure mode;
//! the statement lives in the receipt only, after the transaction ID is
//! computed, so no hash cycle occurs. This crate fixes the canonical shape
//! and the shared preimage construction; curve verification and trust stay
//! with the N5 transaction owner. Required even for empty test sets.

use sley_id::{
    CandidateId, CandidateResultId, CommitAdmissionStatementId, HistoricalAdmissionContextId,
    HistoricalTrustPolicyId, NativeAdmissionProfileId, NativeEvidenceBundleId,
    NativeResourcePolicyId, NativeTestApprovalId, PrincipalId, StateRoot, TransactionId,
    WorkspaceId,
};
use sley_scb1::{ScbError, ScbErrorCode, encode_record, encode_uvar};

use crate::codec::{
    RECORD_VERSION, decode_envelope, decode_fields, expect_tags, preimage_bytes, read_id,
    read_uvar_value,
};

/// `SLEYNSA1` envelope magic for [`CommitAdmissionStatementV1`].
pub const STATEMENT_MAGIC: [u8; 8] = *b"SLEYNSA1";
/// Ed25519 signing context prefixing the unsigned statement envelope.
///
/// A signing context, not a BLAKE3 identifier domain: registry tooling must
/// classify it separately rather than invent a phantom identifier domain.
pub const ADMISSION_SIGNATURE_CONTEXT: &[u8] = b"sley2.native-test-admission-signature.v1";
/// Exact Ed25519 signature bytes in the statement's final field.
pub const ADMISSION_SIGNATURE_BYTES: usize = 64;

/// Caller-supplied statement facts; constructing these signs nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommitAdmissionStatementParts {
    /// Admission descriptor the acceptance ran under.
    pub admission_profile: NativeAdmissionProfileId,
    /// Raw acceptance public key claiming this statement.
    pub key_id: [u8; 32],
    /// Workspace the commit was authorized in.
    pub workspace: WorkspaceId,
    /// Principal the commit was authorized for.
    pub principal: PrincipalId,
    /// Accepted parent transaction the candidate built on.
    pub parent_transaction: TransactionId,
    /// Accepted parent root the candidate built on.
    pub parent_root: StateRoot,
    /// Candidate admitted by this statement.
    pub candidate: CandidateId,
    /// Fresh static result the approval bound.
    pub static_result: CandidateResultId,
    /// Historical context the acceptance preserved.
    pub historical_context_id: HistoricalAdmissionContextId,
    /// Validator-owned static-context digest; opaque to this codec.
    ///
    /// Recomputed by the transaction owner from the preserved projection
    /// bytes, never minted by parsing.
    pub static_context_digest: [u8; 32],
    /// Resource-policy projection the plan bound.
    pub resource_policy_id: NativeResourcePolicyId,
    /// Native approval authorizing the test evidence.
    pub native_approval_id: NativeTestApprovalId,
    /// Root committed by the accepted transaction.
    pub committed_root: StateRoot,
    /// Evidence bundle embedded in the receipt.
    pub bundle_id: NativeEvidenceBundleId,
    /// Transaction core computed before the statement was signed.
    pub transaction_id: TransactionId,
    /// Historical validation time the acceptance evaluated.
    pub historical_validation_time: u64,
    /// Receiver trust manifest covering the acceptance key.
    pub acceptance_trust_policy_id: HistoricalTrustPolicyId,
    /// Opaque Ed25519 signature over [`admission_signature_preimage`].
    ///
    /// Shape-checked to 64 bytes here; curve validity is N5's, and this
    /// crate never verifies or trusts it.
    pub signature: [u8; 64],
}

/// Immutable canonical admission statement; callers cannot forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitAdmissionStatementV1 {
    parts: CommitAdmissionStatementParts,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: CommitAdmissionStatementId,
}

impl CommitAdmissionStatementV1 {
    /// Builds a validated statement from caller-supplied facts.
    ///
    /// # Errors
    /// Returns the stable refusal when the record exceeds the stored bound,
    /// which cannot happen for validated facts.
    pub fn build(parts: CommitAdmissionStatementParts) -> Result<Self, ScbError> {
        let record = record_parts(&parts)?;
        let preimage = preimage_bytes(STATEMENT_MAGIC, &record)?;
        let id = CommitAdmissionStatementId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        Ok(Self {
            parts,
            record,
            stored,
            id,
        })
    }

    /// Parses and strictly validates stored statement bytes.
    ///
    /// # Errors
    /// Returns the stable refusal for shape, range, or digest violations.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        let (record, trailer) = decode_envelope(stored, STATEMENT_MAGIC)?;
        let fields = decode_fields(&record)?;
        expect_tags(
            &fields,
            &[
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
            ],
        )?;
        let signature: [u8; 64] = fields[18]
            .1
            .clone()
            .try_into()
            .map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))?;
        let parts = CommitAdmissionStatementParts {
            admission_profile: NativeAdmissionProfileId::from_bytes(read_id(&fields[1].1)?),
            key_id: read_id(&fields[2].1)?,
            workspace: WorkspaceId::from_bytes(read_id(&fields[3].1)?),
            principal: PrincipalId::from_bytes(read_id(&fields[4].1)?),
            parent_transaction: TransactionId::from_bytes(read_id(&fields[5].1)?),
            parent_root: StateRoot::from_bytes(read_id(&fields[6].1)?),
            candidate: CandidateId::from_bytes(read_id(&fields[7].1)?),
            static_result: CandidateResultId::from_bytes(read_id(&fields[8].1)?),
            historical_context_id: HistoricalAdmissionContextId::from_bytes(read_id(&fields[9].1)?),
            static_context_digest: read_id(&fields[10].1)?,
            resource_policy_id: NativeResourcePolicyId::from_bytes(read_id(&fields[11].1)?),
            native_approval_id: NativeTestApprovalId::from_bytes(read_id(&fields[12].1)?),
            committed_root: StateRoot::from_bytes(read_id(&fields[13].1)?),
            bundle_id: NativeEvidenceBundleId::from_bytes(read_id(&fields[14].1)?),
            transaction_id: TransactionId::from_bytes(read_id(&fields[15].1)?),
            historical_validation_time: read_uvar_value(&fields[16].1, 64)?,
            acceptance_trust_policy_id: HistoricalTrustPolicyId::from_bytes(read_id(
                &fields[17].1,
            )?),
            signature,
        };
        let preimage = preimage_bytes(STATEMENT_MAGIC, &record)?;
        let id = CommitAdmissionStatementId::derive(&preimage);
        if id.as_bytes() != &trailer {
            return Err(ScbError::new(ScbErrorCode::DigestMismatch));
        }
        Ok(Self {
            parts,
            record,
            stored: stored.to_vec(),
            id,
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

    /// Domain-separated statement identity.
    #[must_use]
    pub const fn id(&self) -> CommitAdmissionStatementId {
        self.id
    }

    /// Approval this statement accepts the evidence of.
    #[must_use]
    pub const fn native_approval_id(&self) -> NativeTestApprovalId {
        self.parts.native_approval_id
    }

    /// Bundle this statement accepts the bytes of.
    #[must_use]
    pub const fn bundle_id(&self) -> NativeEvidenceBundleId {
        self.parts.bundle_id
    }
}

/// Canonical unsigned statement record: fields 1..18 without the signature.
///
/// Signers encode the statement facts, strip the signature field through
/// this helper, frame the [`admission_signature_preimage`], and sign that.
/// Verifiers (N5) repeat the construction over parsed facts, so neither side
/// can sign or check a different byte layout.
///
/// # Errors
/// Returns the stable SCB1 failure if re-encoding the facts fails, which
/// cannot happen for a validated statement.
pub fn unsigned_statement_prefix(
    parts: &CommitAdmissionStatementParts,
) -> Result<Vec<u8>, ScbError> {
    encode_record(&unsigned_fields(parts))
}

/// Canonical signature preimage for one unsigned statement record.
///
/// Signs exact ASCII `sley2.native-test-admission-signature.v1` followed by
/// `P(SLEYNSA1, fields 1..18)`: the caller supplies the canonical unsigned
/// record, and this helper frames the shared construction so signers and N5
/// verifiers cannot diverge.
///
/// # Errors
/// Returns `SCB_RESOURCE_LIMIT` when the unsigned record exceeds the stored
/// bound, which cannot happen for a parsed statement.
pub fn admission_signature_preimage(unsigned_record: &[u8]) -> Result<Vec<u8>, ScbError> {
    let envelope = preimage_bytes(STATEMENT_MAGIC, unsigned_record)?;
    let mut out = Vec::with_capacity(ADMISSION_SIGNATURE_CONTEXT.len() + envelope.len());
    out.extend_from_slice(ADMISSION_SIGNATURE_CONTEXT);
    out.extend_from_slice(&envelope);
    Ok(out)
}

fn record_parts(parts: &CommitAdmissionStatementParts) -> Result<Vec<u8>, ScbError> {
    let mut fields = unsigned_fields(parts);
    fields.push((19, parts.signature.to_vec()));
    encode_record(&fields)
}

/// Fields 1..18 without the signature, shared by the record builder and the
/// unsigned-prefix helper so the two constructions cannot diverge.
fn unsigned_fields(parts: &CommitAdmissionStatementParts) -> Vec<(u32, Vec<u8>)> {
    vec![
        (1, encode_uvar(RECORD_VERSION)),
        (2, parts.admission_profile.as_bytes().to_vec()),
        (3, parts.key_id.to_vec()),
        (4, parts.workspace.as_bytes().to_vec()),
        (5, parts.principal.as_bytes().to_vec()),
        (6, parts.parent_transaction.as_bytes().to_vec()),
        (7, parts.parent_root.as_bytes().to_vec()),
        (8, parts.candidate.as_bytes().to_vec()),
        (9, parts.static_result.as_bytes().to_vec()),
        (10, parts.historical_context_id.as_bytes().to_vec()),
        (11, parts.static_context_digest.to_vec()),
        (12, parts.resource_policy_id.as_bytes().to_vec()),
        (13, parts.native_approval_id.as_bytes().to_vec()),
        (14, parts.committed_root.as_bytes().to_vec()),
        (15, parts.bundle_id.as_bytes().to_vec()),
        (16, parts.transaction_id.as_bytes().to_vec()),
        (17, encode_uvar(parts.historical_validation_time)),
        (18, parts.acceptance_trust_policy_id.as_bytes().to_vec()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_hex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex decodes"))
            .collect()
    }

    fn golden(field: &str) -> Vec<u8> {
        let text = include_str!(
            "/home/gfarch/Work/checkpoints/sley2-finish-20260915/native-final-golden.json"
        );
        let marker = format!("\"{field}\": \"");
        let start = text.find(&marker).expect("golden field present") + marker.len();
        let end = text[start..].find('"').expect("golden field ends") + start;
        decode_hex(&text[start..end])
    }

    fn plan_golden(field: &str) -> Vec<u8> {
        let text = include_str!(
            "/home/gfarch/Work/checkpoints/sley2-finish-20260915/native-plan-golden.json"
        );
        let marker = format!("\"{field}\": \"");
        let start = text.find(&marker).expect("golden field present") + marker.len();
        let end = text[start..].find('"').expect("golden field ends") + start;
        decode_hex(&text[start..end])
    }

    fn id32(field: &str) -> [u8; 32] {
        golden(field).try_into().expect("32 bytes")
    }

    fn golden_parts() -> CommitAdmissionStatementParts {
        CommitAdmissionStatementParts {
            admission_profile: NativeAdmissionProfileId::from_bytes(id32("profile_id")),
            key_id: [0x91; 32],
            workspace: WorkspaceId::from_bytes([0xB0; 32]),
            principal: PrincipalId::from_bytes([0xB1; 32]),
            parent_transaction: TransactionId::from_bytes([0x22; 32]),
            parent_root: StateRoot::from_bytes([0xA2; 32]),
            candidate: CandidateId::from_bytes([0xA4; 32]),
            static_result: CandidateResultId::from_bytes([0xA5; 32]),
            historical_context_id: HistoricalAdmissionContextId::from_bytes(id32("context_id")),
            static_context_digest: [0x98; 32],
            resource_policy_id: NativeResourcePolicyId::from_bytes(
                plan_golden("policy_id").try_into().expect("32 bytes"),
            ),
            native_approval_id: NativeTestApprovalId::from_bytes(id32("approval_id")),
            committed_root: StateRoot::from_bytes([0xB3; 32]),
            bundle_id: NativeEvidenceBundleId::from_bytes(id32("bundle_id")),
            transaction_id: TransactionId::from_bytes([0xB4; 32]),
            historical_validation_time: 1_757_960_000_000,
            acceptance_trust_policy_id: HistoricalTrustPolicyId::from_bytes(id32("trust_id")),
            signature: [0x42; 64],
        }
    }

    #[test]
    fn golden_statement_parses_with_exact_id_and_preimage() {
        let stored = golden("statement_stored");
        let parsed = CommitAdmissionStatementV1::parse(&stored).expect("golden parses");
        assert_eq!(parsed.id().as_bytes().to_vec(), golden("statement_id"));
        assert_eq!(parsed.record_bytes(), golden("statement_record").as_slice());
        let rebuilt = CommitAdmissionStatementV1::build(golden_parts()).expect("parts build");
        assert_eq!(rebuilt.stored_bytes(), stored.as_slice());
        let unsigned = unsigned_statement_prefix(&golden_parts()).expect("prefix encodes");
        assert_eq!(unsigned, golden("statement_unsigned_record").as_slice());
        let preimage = admission_signature_preimage(&unsigned).expect("preimage frames");
        assert_eq!(preimage, golden("statement_signing_preimage").as_slice());
    }

    #[test]
    fn statement_refusals_keep_stable_codes() {
        let stored = golden("statement_stored");
        assert_eq!(
            CommitAdmissionStatementV1::parse(&stored[..stored.len() - 1])
                .expect_err("truncated")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        let mut tampered = stored.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert_eq!(
            CommitAdmissionStatementV1::parse(&tampered)
                .expect_err("digest")
                .code(),
            ScbErrorCode::DigestMismatch
        );
    }
}
