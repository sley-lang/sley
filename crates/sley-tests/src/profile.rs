//! Native admission profile (`SLEYNAD1`) from `NATIVE_TEST_ADMISSION_V1.md` §4.
//!
//! The profile is the exact admission descriptor the historical context
//! binds as its authorization profile: static validation profile, native
//! execution profile, fixed selection/measurement/signature rule tags, the
//! aggregate hard maxima, lock-wait, pre-promotion watchdog and cleanup
//! bounds, and the between-requests cancel profile. Every constant is exact;
//! configured local values tighten enforcement elsewhere but never alter
//! this descriptor. Parsing refuses any deviation under this profile.

use sley_id::{NativeAdmissionProfileId, NativeExecutionProfileId, ValidationProfileId};
use sley_scb1::{ScbError, ScbErrorCode, encode_record, encode_uvar};

use crate::codec::{
    RECORD_VERSION, decode_envelope, decode_fields, expect_tags, preimage_bytes, read_id,
    read_uvar_value,
};
use crate::policy::NativeAggregateLimits;

/// `SLEYNAD1` envelope magic for [`NativeAdmissionProfileV1`].
pub const ADMISSION_PROFILE_MAGIC: [u8; 8] = *b"SLEYNAD1";
/// Only accepted selection rule: the native stronger-set union.
pub const SELECTION_RULE_NATIVE_V1: u32 = 1;
/// Only accepted measurement profile: qualified supervised measurement.
pub const MEASUREMENT_PROFILE_V1: u32 = 1;
/// Only accepted acceptance-signature profile: distinct acceptance key.
pub const ACCEPTANCE_SIGNATURE_PROFILE_V1: u32 = 1;
/// Writer-lock acquisition bound in milliseconds.
pub const LOCK_WAIT_MILLIS: u64 = 2000;
/// Pre-promotion watchdog bound in milliseconds.
pub const PREPROMOTION_WATCHDOG_MILLIS: u64 = 35_000;
/// Worker cleanup bound in milliseconds.
pub const ADMISSION_CLEANUP_MILLIS: u64 = 2000;
/// Only accepted cancel profile: between-requests cancellation.
pub const CANCEL_BETWEEN_REQUESTS: u32 = 1;

/// Caller-supplied descriptor facts; constructing these admits nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeAdmissionProfileParts {
    /// Full-v1 static validation profile bound inside the descriptor.
    pub static_validation_profile: ValidationProfileId,
    /// Native execution profile the descriptor authorizes.
    pub execution_profile: NativeExecutionProfileId,
    /// Selection rule; must be [`SELECTION_RULE_NATIVE_V1`].
    pub selection_rule: u32,
    /// Measurement profile; must be [`MEASUREMENT_PROFILE_V1`].
    pub measurement_profile: u32,
    /// Acceptance-signature profile; must be [`ACCEPTANCE_SIGNATURE_PROFILE_V1`].
    pub acceptance_signature_profile: u32,
    /// Aggregate ceilings; must equal the native hard maxima exactly.
    pub aggregate_limits: NativeAggregateLimits,
    /// Writer-lock wait bound; must be [`LOCK_WAIT_MILLIS`].
    pub lock_wait_millis: u64,
    /// Pre-promotion watchdog bound; must be [`PREPROMOTION_WATCHDOG_MILLIS`].
    pub prepromotion_watchdog_millis: u64,
    /// Cleanup bound; must be [`ADMISSION_CLEANUP_MILLIS`].
    pub cleanup_millis: u64,
    /// Cancel profile; must be [`CANCEL_BETWEEN_REQUESTS`].
    pub cancel_profile: u32,
}

/// Immutable canonical admission descriptor; callers cannot forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeAdmissionProfileV1 {
    parts: NativeAdmissionProfileParts,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: NativeAdmissionProfileId,
}

impl NativeAdmissionProfileV1 {
    /// Builds a validated descriptor from caller-supplied facts.
    ///
    /// # Errors
    /// Returns `SCB_CONTRACT_UNKNOWN` for any constant or maxima deviation.
    pub fn build(parts: NativeAdmissionProfileParts) -> Result<Self, ScbError> {
        validate_parts(&parts)?;
        let record = record_parts(&parts)?;
        let preimage = preimage_bytes(ADMISSION_PROFILE_MAGIC, &record)?;
        let id = NativeAdmissionProfileId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        Ok(Self {
            parts,
            record,
            stored,
            id,
        })
    }

    /// Parses and strictly validates stored descriptor bytes.
    ///
    /// # Errors
    /// Returns the stable refusal for shape, constant, maxima, or digest
    /// violations.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        let (record, trailer) = decode_envelope(stored, ADMISSION_PROFILE_MAGIC)?;
        let fields = decode_fields(&record)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])?;
        let u32_field = |index: usize| {
            u32::try_from(read_uvar_value(&fields[index].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))
        };
        let parts = NativeAdmissionProfileParts {
            static_validation_profile: ValidationProfileId::from_bytes(read_id(&fields[1].1)?),
            execution_profile: NativeExecutionProfileId::from_bytes(read_id(&fields[2].1)?),
            selection_rule: u32_field(3)?,
            measurement_profile: u32_field(4)?,
            acceptance_signature_profile: u32_field(5)?,
            aggregate_limits: NativeAggregateLimits::parse(&fields[6].1)?,
            lock_wait_millis: read_uvar_value(&fields[7].1, 64)?,
            prepromotion_watchdog_millis: read_uvar_value(&fields[8].1, 64)?,
            cleanup_millis: read_uvar_value(&fields[9].1, 64)?,
            cancel_profile: u32_field(10)?,
        };
        validate_parts(&parts)?;
        let preimage = preimage_bytes(ADMISSION_PROFILE_MAGIC, &record)?;
        let id = NativeAdmissionProfileId::derive(&preimage);
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

    /// Domain-separated descriptor identity.
    #[must_use]
    pub const fn id(&self) -> NativeAdmissionProfileId {
        self.id
    }

    /// Validated descriptor facts; constructing these admits nothing.
    #[must_use]
    pub const fn parts(&self) -> &NativeAdmissionProfileParts {
        &self.parts
    }
}

fn validate_parts(parts: &NativeAdmissionProfileParts) -> Result<(), ScbError> {
    if parts.selection_rule != SELECTION_RULE_NATIVE_V1
        || parts.measurement_profile != MEASUREMENT_PROFILE_V1
        || parts.acceptance_signature_profile != ACCEPTANCE_SIGNATURE_PROFILE_V1
        || parts.aggregate_limits != NativeAggregateLimits::HARD_MAXIMA
        || parts.lock_wait_millis != LOCK_WAIT_MILLIS
        || parts.prepromotion_watchdog_millis != PREPROMOTION_WATCHDOG_MILLIS
        || parts.cleanup_millis != ADMISSION_CLEANUP_MILLIS
        || parts.cancel_profile != CANCEL_BETWEEN_REQUESTS
    {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    Ok(())
}

fn record_parts(parts: &NativeAdmissionProfileParts) -> Result<Vec<u8>, ScbError> {
    encode_record(&[
        (1, encode_uvar(RECORD_VERSION)),
        (2, parts.static_validation_profile.as_bytes().to_vec()),
        (3, parts.execution_profile.as_bytes().to_vec()),
        (4, encode_uvar(u64::from(parts.selection_rule))),
        (5, encode_uvar(u64::from(parts.measurement_profile))),
        (
            6,
            encode_uvar(u64::from(parts.acceptance_signature_profile)),
        ),
        (7, parts.aggregate_limits.record()?),
        (8, encode_uvar(parts.lock_wait_millis)),
        (9, encode_uvar(parts.prepromotion_watchdog_millis)),
        (10, encode_uvar(parts.cleanup_millis)),
        (11, encode_uvar(u64::from(parts.cancel_profile))),
    ])
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
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/golden/native-final-golden.json"
        ));
        let marker = format!("\"{field}\": \"");
        let start = text.find(&marker).expect("golden field present") + marker.len();
        let end = text[start..].find('"').expect("golden field ends") + start;
        decode_hex(&text[start..end])
    }

    fn golden_parts() -> NativeAdmissionProfileParts {
        NativeAdmissionProfileParts {
            static_validation_profile: ValidationProfileId::from_bytes([0x87; 32]),
            execution_profile: NativeExecutionProfileId::from_bytes([0x88; 32]),
            selection_rule: SELECTION_RULE_NATIVE_V1,
            measurement_profile: MEASUREMENT_PROFILE_V1,
            acceptance_signature_profile: ACCEPTANCE_SIGNATURE_PROFILE_V1,
            aggregate_limits: NativeAggregateLimits::HARD_MAXIMA,
            lock_wait_millis: LOCK_WAIT_MILLIS,
            prepromotion_watchdog_millis: PREPROMOTION_WATCHDOG_MILLIS,
            cleanup_millis: ADMISSION_CLEANUP_MILLIS,
            cancel_profile: CANCEL_BETWEEN_REQUESTS,
        }
    }

    #[test]
    fn golden_descriptor_parses_with_exact_id() {
        let stored = golden("profile_stored");
        let parsed = NativeAdmissionProfileV1::parse(&stored).expect("golden parses");
        assert_eq!(parsed.id().as_bytes().to_vec(), golden("profile_id"));
        assert_eq!(parsed.record_bytes(), golden("profile_record").as_slice());
        let rebuilt = NativeAdmissionProfileV1::build(golden_parts()).expect("parts build");
        assert_eq!(rebuilt.stored_bytes(), stored.as_slice());
    }

    #[test]
    fn descriptor_refusals_keep_stable_codes() {
        let mut bad_rule = golden_parts();
        bad_rule.selection_rule = 2;
        assert_eq!(
            NativeAdmissionProfileV1::build(bad_rule)
                .expect_err("rule")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut bad_maxima = golden_parts();
        let mut tightened = NativeAggregateLimits::HARD_MAXIMA;
        tightened.max_selected_tests = 128;
        bad_maxima.aggregate_limits = tightened;
        assert_eq!(
            NativeAdmissionProfileV1::build(bad_maxima)
                .expect_err("maxima")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut bad_watchdog = golden_parts();
        bad_watchdog.prepromotion_watchdog_millis = 30_000;
        assert_eq!(
            NativeAdmissionProfileV1::build(bad_watchdog)
                .expect_err("watchdog")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let stored = golden("profile_stored");
        assert_eq!(
            NativeAdmissionProfileV1::parse(&stored[..stored.len() - 1])
                .expect_err("truncated")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        let mut tampered = stored.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert_eq!(
            NativeAdmissionProfileV1::parse(&tampered)
                .expect_err("digest")
                .code(),
            ScbErrorCode::DigestMismatch
        );
    }
}
