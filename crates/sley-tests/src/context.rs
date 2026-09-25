//! Historical admission context (`SLEYNCT1`) from `NATIVE_TEST_ADMISSION_V1.md` §4.
//!
//! The context preserves the exact nonsecret projection bytes already hashed
//! by the policy owner's validator: the eleven-field static context
//! projection and the capability-summary projection whose digest recomputes
//! from the preserved bytes. It binds the exact native resource-policy
//! record and the authorization profile (the [`NativeAdmissionProfileV1`]
//! descriptor). Projection bytes come from an owner accessor, never a
//! duplicate encoder here; MAC keys, private signing keys, and capability
//! secrets are never exported. Import-time digest and binding checks are
//! later owners' work; this crate only fixes the canonical shape.

use sley_id::{HistoricalAdmissionContextId, NativeAdmissionProfileId};
use sley_scb1::{ScbError, ScbErrorCode, encode_record, encode_uvar};

use crate::codec::{
    RECORD_VERSION, decode_envelope, decode_fields, expect_tags, preimage_bytes, read_id,
};
use crate::policy::NativeResourcePolicyV1;

/// `SLEYNCT1` envelope magic for [`HistoricalAdmissionContextV1`].
pub const HISTORICAL_CONTEXT_MAGIC: [u8; 8] = *b"SLEYNCT1";
/// Maximum preserved projection bytes per field (16 MiB SCB1 bound).
pub const MAX_CONTEXT_PROJECTION_BYTES: usize = 16_777_216;

/// Caller-supplied context facts; constructing these proves no history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalAdmissionContextParts {
    /// Exact static context projection bytes from the policy owner.
    pub static_context_projection: Vec<u8>,
    /// Exact nonsecret capability-summary projection bytes.
    pub capability_summary_projection: Vec<u8>,
    /// Exact native resource-policy projection bound to the plan.
    pub resource_policy: NativeResourcePolicyV1,
    /// Authorization profile: the exact admission descriptor identity.
    pub authorization_profile: NativeAdmissionProfileId,
}

/// Immutable canonical historical context; callers cannot forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalAdmissionContextV1 {
    parts: HistoricalAdmissionContextParts,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: HistoricalAdmissionContextId,
}

impl HistoricalAdmissionContextV1 {
    /// Builds a validated context from caller-supplied facts.
    ///
    /// # Errors
    /// Returns `SCB_LENGTH_OVERFLOW` when a projection exceeds 16 MiB.
    pub fn build(parts: HistoricalAdmissionContextParts) -> Result<Self, ScbError> {
        validate_parts(&parts)?;
        let record = record_parts(&parts)?;
        let preimage = preimage_bytes(HISTORICAL_CONTEXT_MAGIC, &record)?;
        let id = HistoricalAdmissionContextId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        Ok(Self {
            parts,
            record,
            stored,
            id,
        })
    }

    /// Parses and strictly validates stored context bytes.
    ///
    /// # Errors
    /// Returns the stable refusal for shape, bound, policy, or digest
    /// violations.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        let (record, trailer) = decode_envelope(stored, HISTORICAL_CONTEXT_MAGIC)?;
        let fields = decode_fields(&record)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5])?;
        let parts = HistoricalAdmissionContextParts {
            static_context_projection: fields[1].1.clone(),
            capability_summary_projection: fields[2].1.clone(),
            resource_policy: NativeResourcePolicyV1::parse(&envelope_policy(&fields[3].1)?)?,
            authorization_profile: NativeAdmissionProfileId::from_bytes(read_id(&fields[4].1)?),
        };
        validate_parts(&parts)?;
        let preimage = preimage_bytes(HISTORICAL_CONTEXT_MAGIC, &record)?;
        let id = HistoricalAdmissionContextId::derive(&preimage);
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

    /// Domain-separated context identity.
    #[must_use]
    pub const fn id(&self) -> HistoricalAdmissionContextId {
        self.id
    }

    /// Bound resource-policy projection.
    #[must_use]
    pub const fn resource_policy(&self) -> &NativeResourcePolicyV1 {
        &self.parts.resource_policy
    }

    /// Authorization profile identity.
    #[must_use]
    pub const fn authorization_profile(&self) -> NativeAdmissionProfileId {
        self.parts.authorization_profile
    }
}

fn validate_parts(parts: &HistoricalAdmissionContextParts) -> Result<(), ScbError> {
    if parts.static_context_projection.len() > MAX_CONTEXT_PROJECTION_BYTES
        || parts.capability_summary_projection.len() > MAX_CONTEXT_PROJECTION_BYTES
    {
        return Err(ScbError::new(ScbErrorCode::LengthOverflow));
    }
    Ok(())
}

fn record_parts(parts: &HistoricalAdmissionContextParts) -> Result<Vec<u8>, ScbError> {
    encode_record(&[
        (1, encode_uvar(RECORD_VERSION)),
        (2, parts.static_context_projection.clone()),
        (3, parts.capability_summary_projection.clone()),
        (4, parts.resource_policy.record_bytes()?),
        (5, parts.authorization_profile.as_bytes().to_vec()),
    ])
}

/// Re-envelopes an embedded policy record so its digest verifies, mirroring
/// the plan owner's projection binding.
fn envelope_policy(record: &[u8]) -> Result<Vec<u8>, ScbError> {
    use crate::codec::POLICY_MAGIC;
    use sley_id::NativeResourcePolicyId;

    let preimage = preimage_bytes(POLICY_MAGIC, record)?;
    let id = NativeResourcePolicyId::derive(&preimage);
    let mut stored = preimage;
    stored.extend_from_slice(id.as_bytes());
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::sample_parts as policy_parts;

    fn decode_hex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex decodes"))
            .collect()
    }

    fn golden(field: &str) -> Vec<u8> {
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/native-final-golden.json"
        ));
        let marker = format!("\"{field}\": \"");
        let start = text.find(&marker).expect("golden field present") + marker.len();
        let end = text[start..].find('"').expect("golden field ends") + start;
        decode_hex(&text[start..end])
    }

    fn golden_parts() -> HistoricalAdmissionContextParts {
        HistoricalAdmissionContextParts {
            static_context_projection: b"static-context-projection-fixture".to_vec(),
            capability_summary_projection: b"capability-summary-projection-fixture".to_vec(),
            resource_policy: NativeResourcePolicyV1::build(policy_parts()).expect("policy builds"),
            authorization_profile: NativeAdmissionProfileId::from_bytes(
                golden("profile_id").try_into().expect("32 bytes"),
            ),
        }
    }

    #[test]
    fn golden_context_parses_with_exact_id() {
        let stored = golden("context_stored");
        let parsed = HistoricalAdmissionContextV1::parse(&stored).expect("golden parses");
        assert_eq!(parsed.id().as_bytes().to_vec(), golden("context_id"));
        assert_eq!(parsed.record_bytes(), golden("context_record").as_slice());
        let rebuilt = HistoricalAdmissionContextV1::build(golden_parts()).expect("parts build");
        assert_eq!(rebuilt.stored_bytes(), stored.as_slice());
    }

    #[test]
    fn context_refusals_keep_stable_codes() {
        let mut oversized = golden_parts();
        oversized.static_context_projection = vec![0x00; MAX_CONTEXT_PROJECTION_BYTES + 1];
        assert_eq!(
            HistoricalAdmissionContextV1::build(oversized)
                .expect_err("oversized")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        let stored = golden("context_stored");
        assert_eq!(
            HistoricalAdmissionContextV1::parse(&stored[..stored.len() - 1])
                .expect_err("truncated")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        let mut tampered = stored.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert_eq!(
            HistoricalAdmissionContextV1::parse(&tampered)
                .expect_err("digest")
                .code(),
            ScbErrorCode::DigestMismatch
        );
    }
}
