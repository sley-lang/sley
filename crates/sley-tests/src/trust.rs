//! Historical trust policy (`SLEYNTR1`) from `NATIVE_TEST_ADMISSION_V1.md` §4.
//!
//! The policy is a receiver-provisioned immutable manifest assigning Ed25519
//! key IDs to measurement or acceptance roles with workspace scopes, allowed
//! profile identities, and half-open historical validity intervals. For the
//! measurement role the profile set holds supervisor-configuration IDs; for
//! the acceptance role it holds admission-profile IDs. Entries are strictly
//! ordered by key ID then role; sets are nonempty and bounded. Parsing yields
//! a strictly validated manifest; it never installs trust — the receiver
//! establishes exact manifest IDs out of band, and signatures are verified
//! by later owners against the recorded historical time, never by this crate.

use sley_id::HistoricalTrustPolicyId;
use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_list, encode_record, encode_uvar};

use crate::codec::{
    RECORD_VERSION, check_sorted_unique, decode_envelope, decode_fields, encode_id_list,
    expect_tags, preimage_bytes, read_id, read_id_list, read_uvar_value,
};

/// `SLEYNTR1` envelope magic for [`HistoricalTrustPolicyV1`].
pub const TRUST_MAGIC: [u8; 8] = *b"SLEYNTR1";
/// Measurement-signer role tag.
pub const ROLE_MEASUREMENT: u32 = 1;
/// Acceptance-signer role tag.
pub const ROLE_ACCEPTANCE: u32 = 2;
/// Maximum trust entries in one manifest.
pub const MAX_TRUST_ENTRIES: u64 = 256;
/// Maximum workspace/profile identities in one entry set.
pub const MAX_TRUST_SET: u64 = 256;

/// One role grant: raw Ed25519 public key with scopes and validity.
///
/// Workspace and profile members stay raw 32-byte identities because the
/// profile set's meaning is role-dependent (supervisor-configuration IDs
/// for measurement, admission-profile IDs for acceptance); the owning
/// verifier interprets them, never this codec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustEntry {
    /// Exact Ed25519 public key this grant covers.
    pub key_id: [u8; 32],
    /// Signer role: [`ROLE_MEASUREMENT`] or [`ROLE_ACCEPTANCE`].
    pub role: u32,
    /// Workspace scope, nonempty, sorted unique, at most 256.
    pub workspaces: Vec<[u8; 32]>,
    /// Allowed profile identities, nonempty, sorted unique, at most 256.
    pub profiles: Vec<[u8; 32]>,
    /// Validity interval start, inclusive Unix millis.
    pub valid_from_unix_millis: u64,
    /// Validity interval end, exclusive Unix millis, after the start.
    pub valid_until_unix_millis: u64,
}

impl TrustEntry {
    fn record(&self) -> Result<Vec<u8>, ScbError> {
        validate_entry_shape(self)?;
        encode_record(&[
            (1, self.key_id.to_vec()),
            (2, encode_uvar(u64::from(self.role))),
            (3, encode_id_list(&self.workspaces)?),
            (4, encode_id_list(&self.profiles)?),
            (5, encode_uvar(self.valid_from_unix_millis)),
            (6, encode_uvar(self.valid_until_unix_millis)),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5, 6])?;
        let entry = Self {
            key_id: read_id(&fields[0].1)?,
            role: u32::try_from(read_uvar_value(&fields[1].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?,
            workspaces: read_id_list(&fields[2].1, MAX_TRUST_SET)?,
            profiles: read_id_list(&fields[3].1, MAX_TRUST_SET)?,
            valid_from_unix_millis: read_uvar_value(&fields[4].1, 64)?,
            valid_until_unix_millis: read_uvar_value(&fields[5].1, 64)?,
        };
        validate_entry_shape(&entry)?;
        Ok(entry)
    }
}

fn validate_entry_shape(entry: &TrustEntry) -> Result<(), ScbError> {
    if entry.role != ROLE_MEASUREMENT && entry.role != ROLE_ACCEPTANCE {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    if entry.workspaces.is_empty() || entry.profiles.is_empty() {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    check_sorted_unique(&entry.workspaces)?;
    check_sorted_unique(&entry.profiles)?;
    if entry.valid_from_unix_millis >= entry.valid_until_unix_millis {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    Ok(())
}

/// Caller-supplied manifest facts; constructing these grants no authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalTrustPolicyParts {
    /// Policy nonce distinguishing deliberate manifest versions.
    pub policy_nonce: [u8; 32],
    /// Role grants, strictly key-ID then role ordered, at most 256.
    pub entries: Vec<TrustEntry>,
}

/// Immutable canonical trust manifest; callers cannot forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalTrustPolicyV1 {
    parts: HistoricalTrustPolicyParts,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: HistoricalTrustPolicyId,
}

impl HistoricalTrustPolicyV1 {
    /// Builds a validated manifest from caller-supplied facts.
    ///
    /// # Errors
    /// Returns the stable refusal for role, set, interval, order, or
    /// count violations.
    pub fn build(parts: HistoricalTrustPolicyParts) -> Result<Self, ScbError> {
        validate_parts(&parts)?;
        let record = record_parts(&parts)?;
        let preimage = preimage_bytes(TRUST_MAGIC, &record)?;
        let id = HistoricalTrustPolicyId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        Ok(Self {
            parts,
            record,
            stored,
            id,
        })
    }

    /// Parses and strictly validates stored manifest bytes.
    ///
    /// # Errors
    /// Returns the stable refusal for shape, order, role, set, interval,
    /// count, or digest violations.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        let (record, trailer) = decode_envelope(stored, TRUST_MAGIC)?;
        let fields = decode_fields(&record)?;
        expect_tags(&fields, &[1, 2, 3])?;
        let mut cursor = ScbValueCursor::new(&fields[2].1)?;
        let count = cursor.read_list_count()?;
        if count > MAX_TRUST_ENTRIES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let mut entries = Vec::new();
        for _ in 0..count {
            entries.push(TrustEntry::parse(cursor.read_bytes()?)?);
        }
        cursor.check_finished()?;
        let parts = HistoricalTrustPolicyParts {
            policy_nonce: read_id(&fields[1].1)?,
            entries,
        };
        validate_parts(&parts)?;
        let preimage = preimage_bytes(TRUST_MAGIC, &record)?;
        let id = HistoricalTrustPolicyId::derive(&preimage);
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

    /// Domain-separated manifest identity.
    #[must_use]
    pub const fn id(&self) -> HistoricalTrustPolicyId {
        self.id
    }

    /// Role grants in key-ID then role order.
    #[must_use]
    pub fn entries(&self) -> &[TrustEntry] {
        &self.parts.entries
    }

    /// Whether `key` holds `role` over `workspace`/`profile` at `now_millis`.
    ///
    /// Pure membership predicate over parsed bytes; trust installation and
    /// revocation remain the receiver's explicit decisions, not this check.
    #[must_use]
    pub fn grants(
        &self,
        key: &[u8; 32],
        role: u32,
        workspace: &[u8; 32],
        profile: &[u8; 32],
        now_millis: u64,
    ) -> bool {
        self.parts.entries.iter().any(|entry| {
            &entry.key_id == key
                && entry.role == role
                && entry.workspaces.contains(workspace)
                && entry.profiles.contains(profile)
                && entry.valid_from_unix_millis <= now_millis
                && now_millis < entry.valid_until_unix_millis
        })
    }
}

fn validate_parts(parts: &HistoricalTrustPolicyParts) -> Result<(), ScbError> {
    let count = u64::try_from(parts.entries.len())
        .map_err(|_| ScbError::new(ScbErrorCode::ResourceLimit))?;
    if count > MAX_TRUST_ENTRIES {
        return Err(ScbError::new(ScbErrorCode::ResourceLimit));
    }
    let mut previous: Option<([u8; 32], u32)> = None;
    for entry in &parts.entries {
        validate_entry_shape(entry)?;
        let key = (entry.key_id, entry.role);
        if let Some(previous) = previous {
            if key == previous {
                return Err(ScbError::new(ScbErrorCode::FieldDuplicate));
            }
            if key < previous {
                return Err(ScbError::new(ScbErrorCode::FieldOrder));
            }
        }
        previous = Some(key);
    }
    Ok(())
}

fn record_parts(parts: &HistoricalTrustPolicyParts) -> Result<Vec<u8>, ScbError> {
    let mut encoded = Vec::new();
    for entry in &parts.entries {
        encoded.push(entry.record()?);
    }
    encode_record(&[
        (1, encode_uvar(RECORD_VERSION)),
        (2, parts.policy_nonce.to_vec()),
        (3, encode_list(&encoded)?),
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

    fn golden_parts() -> HistoricalTrustPolicyParts {
        HistoricalTrustPolicyParts {
            policy_nonce: [0x84; 32],
            entries: vec![
                TrustEntry {
                    key_id: [0x85; 32],
                    role: ROLE_MEASUREMENT,
                    workspaces: vec![[0xB0; 32]],
                    profiles: vec![golden("config_id").try_into().expect("32 bytes")],
                    valid_from_unix_millis: 1_757_960_000_000,
                    valid_until_unix_millis: 1_758_046_400_000,
                },
                TrustEntry {
                    key_id: [0x86; 32],
                    role: ROLE_ACCEPTANCE,
                    workspaces: vec![[0xB0; 32]],
                    profiles: vec![golden("profile_id").try_into().expect("32 bytes")],
                    valid_from_unix_millis: 1_757_960_000_000,
                    valid_until_unix_millis: 1_758_046_400_000,
                },
            ],
        }
    }

    #[test]
    fn golden_manifest_parses_with_exact_id() {
        let stored = golden("trust_stored");
        let parsed = HistoricalTrustPolicyV1::parse(&stored).expect("golden parses");
        assert_eq!(parsed.id().as_bytes().to_vec(), golden("trust_id"));
        assert_eq!(parsed.record_bytes(), golden("trust_record").as_slice());
        assert_eq!(parsed.entries().len(), 2);
        let rebuilt = HistoricalTrustPolicyV1::build(golden_parts()).expect("parts build");
        assert_eq!(rebuilt.stored_bytes(), stored.as_slice());
        let config_id: [u8; 32] = golden("config_id").try_into().expect("32 bytes");
        let workspace = [0xB0; 32];
        assert!(parsed.grants(
            &[0x85; 32],
            ROLE_MEASUREMENT,
            &workspace,
            &config_id,
            1_757_960_000_000
        ));
        assert!(!parsed.grants(
            &[0x85; 32],
            ROLE_ACCEPTANCE,
            &workspace,
            &config_id,
            1_757_960_000_000
        ));
        assert!(!parsed.grants(
            &[0x85; 32],
            ROLE_MEASUREMENT,
            &workspace,
            &config_id,
            1_758_046_400_000
        ));
    }

    #[test]
    fn manifest_refusals_keep_stable_codes() {
        let mut bad_role = golden_parts();
        bad_role.entries[0].role = 3;
        assert_eq!(
            HistoricalTrustPolicyV1::build(bad_role)
                .expect_err("role")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut empty_set = golden_parts();
        empty_set.entries[0].workspaces.clear();
        assert_eq!(
            HistoricalTrustPolicyV1::build(empty_set)
                .expect_err("empty set")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut bad_interval = golden_parts();
        bad_interval.entries[0].valid_until_unix_millis = 1_757_960_000_000;
        assert_eq!(
            HistoricalTrustPolicyV1::build(bad_interval)
                .expect_err("interval")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut swapped = golden_parts();
        swapped.entries.swap(0, 1);
        assert_eq!(
            HistoricalTrustPolicyV1::build(swapped)
                .expect_err("order")
                .code(),
            ScbErrorCode::FieldOrder
        );
        let mut dup = golden_parts();
        dup.entries[1] = dup.entries[0].clone();
        assert_eq!(
            HistoricalTrustPolicyV1::build(dup).expect_err("dup").code(),
            ScbErrorCode::FieldDuplicate
        );
        let stored = golden("trust_stored");
        assert_eq!(
            HistoricalTrustPolicyV1::parse(&stored[..stored.len() - 1])
                .expect_err("truncated")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        let mut tampered = stored.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert_eq!(
            HistoricalTrustPolicyV1::parse(&tampered)
                .expect_err("digest")
                .code(),
            ScbErrorCode::DigestMismatch
        );
    }
}
