//! Supervisor configuration (`SLEYNHC1`) from `NATIVE_TEST_EXECUTION_V1.md` §7.
//!
//! The configuration binds the worker and supervisor binary digests (raw
//! SHA-256 measurements, not `sley2.*` content IDs), the exact normalized
//! unit-property projection, allowed caller identities, page size, cleanup
//! bound, and launch profile. Parsing enforces the required property set
//! exactly: the fourteen fixed properties with exact values, the two
//! resource-instantiated properties present with decimal values, and no
//! unknown, missing, or extra properties. Anything else refuses under this
//! profile rather than relying on manager defaults. This crate never installs
//! units or verifies that a host honored the configuration; that proof is
//! N3's with the attestation binding the configuration identity.

use sley_id::{PrincipalId, SupervisorConfigId, WorkspaceId};
use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_list, encode_record, encode_uvar};

use crate::codec::{
    RECORD_VERSION, decode_envelope, decode_fields, expect_tags, preimage_bytes, read_id,
    read_uvar_value,
};

/// `SLEYNHC1` envelope magic for [`SupervisorConfigV1`].
pub const SUPERVISOR_CONFIG_MAGIC: [u8; 8] = *b"SLEYNHC1";
/// Fixed cleanup bound in milliseconds: part of the launch profile.
pub const SUPERVISOR_CLEANUP_MILLIS: u64 = 2000;
/// Only accepted launch profile: fixed binary/input/output mapping.
pub const SUPERVISOR_LAUNCH_PROFILE: u32 = 1;
/// Maximum unit properties in one configuration.
pub const MAX_PROPERTIES: u64 = 64;
/// Maximum allowed callers in one configuration.
pub const MAX_CALLERS: u64 = 256;
/// Maximum property-name bytes; names are ASCII.
pub const MAX_PROPERTY_NAME_BYTES: usize = 128;
/// Maximum property-value bytes.
pub const MAX_PROPERTY_VALUE_BYTES: usize = 4096;

/// Fixed unit properties with exact required values, in name order.
const FIXED_PROPERTIES: [(&str, &str); 14] = [
    ("CapabilityBoundingSet", "empty"),
    ("DynamicUser", "yes"),
    ("KillMode", "control-group"),
    ("MemoryAccounting", "yes"),
    ("MemorySwapMax", "0"),
    ("NoNewPrivileges", "yes"),
    ("PrivateNetwork", "yes"),
    ("PrivateTmp", "yes"),
    ("ProtectControlGroups", "yes"),
    ("ProtectHome", "yes"),
    ("ProtectSystem", "strict"),
    ("SendSIGKILL", "yes"),
    ("TasksMax", "1"),
    ("TimeoutStopUSec", "2000000"),
];

/// Resource-instantiated properties: present with decimal values.
///
/// The runner verifies the actual installed numbers against the declared
/// ceilings and binds them through attestation limits; this codec requires
/// presence and decimal shape, not the numeric relationship.
const DECIMAL_PROPERTIES: [&str; 2] = ["MemoryMax", "RuntimeMaxUSec"];

/// One normalized unit property: unique sorted name with bounded value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Property {
    /// Property name; ASCII, at most 128 bytes, unique and sorted.
    pub name: String,
    /// Property value; at most 4096 bytes.
    pub value: String,
}

impl Property {
    fn record(&self) -> Result<Vec<u8>, ScbError> {
        if self.name.len() > MAX_PROPERTY_NAME_BYTES || self.value.len() > MAX_PROPERTY_VALUE_BYTES
        {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        // Raw UTF-8 bytes: `encode_record` applies the single size framing,
        // mirroring how `decode_fields` plus `from_utf8` parses them back.
        encode_record(&[
            (1, self.name.as_bytes().to_vec()),
            (2, self.value.as_bytes().to_vec()),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2])?;
        // `decode_fields` already consumed the Text size framing; the field
        // values are the raw UTF-8 bytes, not a nested length-delimited item.
        let name = core::str::from_utf8(&fields[0].1)
            .map_err(|_| ScbError::new(ScbErrorCode::Utf8Invalid))?;
        let text = core::str::from_utf8(&fields[1].1)
            .map_err(|_| ScbError::new(ScbErrorCode::Utf8Invalid))?;
        let parsed = Self {
            name: name.to_owned(),
            value: text.to_owned(),
        };
        if parsed.name.len() > MAX_PROPERTY_NAME_BYTES
            || parsed.value.len() > MAX_PROPERTY_VALUE_BYTES
        {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        if !parsed.name.is_ascii() {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        Ok(parsed)
    }
}

/// One allowed caller: sorted by UID, then workspace, then principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Caller {
    /// Allowed caller UID; peer credentials must match.
    pub uid: u32,
    /// Workspace the caller may run for.
    pub workspace: WorkspaceId,
    /// Principal the caller may run as.
    pub principal: PrincipalId,
}

impl Caller {
    fn record(self) -> Result<Vec<u8>, ScbError> {
        encode_record(&[
            (1, encode_uvar(u64::from(self.uid))),
            (2, self.workspace.as_bytes().to_vec()),
            (3, self.principal.as_bytes().to_vec()),
        ])
    }

    fn parse(value: &[u8]) -> Result<Self, ScbError> {
        let fields = decode_fields(value)?;
        expect_tags(&fields, &[1, 2, 3])?;
        Ok(Self {
            uid: u32::try_from(read_uvar_value(&fields[0].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?,
            workspace: WorkspaceId::from_bytes(read_id(&fields[1].1)?),
            principal: PrincipalId::from_bytes(read_id(&fields[2].1)?),
        })
    }

    fn sort_key(self) -> (u32, [u8; 32], [u8; 32]) {
        (
            self.uid,
            *self.workspace.as_bytes(),
            *self.principal.as_bytes(),
        )
    }
}

/// Caller-supplied configuration facts; constructing these installs nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupervisorConfigParts {
    /// Raw SHA-256 of the pinned worker binary; not a content ID.
    pub worker_digest: [u8; 32],
    /// Raw SHA-256 of the supervisor binary; not a content ID.
    pub supervisor_digest: [u8; 32],
    /// Normalized unit properties: the exact required projection.
    pub properties: Vec<Property>,
    /// Allowed callers, sorted by UID then workspace then principal.
    pub callers: Vec<Caller>,
    /// Host page size used for the page-floor computation.
    pub page_size: u64,
    /// Cleanup bound in milliseconds; must be 2000.
    pub cleanup_millis: u64,
    /// Launch profile; must be 1.
    pub launch_profile: u32,
}

/// Immutable canonical supervisor configuration; callers cannot forge one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupervisorConfigV1 {
    parts: SupervisorConfigParts,
    record: Vec<u8>,
    stored: Vec<u8>,
    id: SupervisorConfigId,
}

impl SupervisorConfigV1 {
    /// Builds a validated configuration from caller-supplied facts.
    ///
    /// # Errors
    /// Returns the stable refusal when the property projection, caller
    /// order, counts, or profile constants deviate from execution §7.
    pub fn build(parts: SupervisorConfigParts) -> Result<Self, ScbError> {
        validate_parts(&parts)?;
        let record = record_parts(&parts)?;
        let preimage = preimage_bytes(SUPERVISOR_CONFIG_MAGIC, &record)?;
        let id = SupervisorConfigId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(id.as_bytes());
        Ok(Self {
            parts,
            record,
            stored,
            id,
        })
    }

    /// Parses and strictly validates stored configuration bytes.
    ///
    /// # Errors
    /// Returns the stable refusal for shape, projection, order, constant,
    /// or digest violations.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        let (record, trailer) = decode_envelope(stored, SUPERVISOR_CONFIG_MAGIC)?;
        let fields = decode_fields(&record)?;
        expect_tags(&fields, &[1, 2, 3, 4, 5, 6, 7, 8])?;
        let mut properties_cursor = ScbValueCursor::new(&fields[3].1)?;
        let property_count = properties_cursor.read_list_count()?;
        if property_count > MAX_PROPERTIES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let mut properties = Vec::new();
        for _ in 0..property_count {
            properties.push(Property::parse(properties_cursor.read_bytes()?)?);
        }
        properties_cursor.check_finished()?;
        let mut callers_cursor = ScbValueCursor::new(&fields[4].1)?;
        let caller_count = callers_cursor.read_list_count()?;
        if caller_count > MAX_CALLERS {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let mut callers = Vec::new();
        for _ in 0..caller_count {
            callers.push(Caller::parse(callers_cursor.read_bytes()?)?);
        }
        callers_cursor.check_finished()?;
        let parts = SupervisorConfigParts {
            worker_digest: read_id(&fields[1].1)?,
            supervisor_digest: read_id(&fields[2].1)?,
            properties,
            callers,
            page_size: read_uvar_value(&fields[5].1, 64)?,
            cleanup_millis: read_uvar_value(&fields[6].1, 64)?,
            launch_profile: u32::try_from(read_uvar_value(&fields[7].1, 32)?)
                .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?,
        };
        validate_parts(&parts)?;
        let preimage = preimage_bytes(SUPERVISOR_CONFIG_MAGIC, &record)?;
        let id = SupervisorConfigId::derive(&preimage);
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

    /// Domain-separated configuration identity.
    #[must_use]
    pub const fn id(&self) -> SupervisorConfigId {
        self.id
    }

    /// Normalized unit properties in name order.
    #[must_use]
    pub fn properties(&self) -> &[Property] {
        &self.parts.properties
    }

    /// Allowed callers in UID order.
    #[must_use]
    pub fn callers(&self) -> &[Caller] {
        &self.parts.callers
    }
}

fn validate_parts(parts: &SupervisorConfigParts) -> Result<(), ScbError> {
    if parts.cleanup_millis != SUPERVISOR_CLEANUP_MILLIS
        || parts.launch_profile != SUPERVISOR_LAUNCH_PROFILE
    {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    let names: Vec<&[u8]> = parts
        .properties
        .iter()
        .map(|property| property.name.as_bytes())
        .collect();
    check_names_sorted_unique(&names)?;
    require_projection(&parts.properties)?;
    let mut previous: Option<(u32, [u8; 32], [u8; 32])> = None;
    for caller in &parts.callers {
        let key = caller.sort_key();
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

fn check_names_sorted_unique(names: &[&[u8]]) -> Result<(), ScbError> {
    for pair in names.windows(2) {
        if pair[1] == pair[0] {
            return Err(ScbError::new(ScbErrorCode::FieldDuplicate));
        }
        if pair[1] < pair[0] {
            return Err(ScbError::new(ScbErrorCode::FieldOrder));
        }
    }
    Ok(())
}

/// Requires exactly the §7 projection: fourteen fixed properties with exact
/// values plus the two resource-instantiated decimal properties, nothing
/// else. The record is name-sorted, so fixed and decimal entries interleave
/// (`MemoryMax` sorts between `MemoryAccounting` and `MemorySwapMax`).
fn require_projection(properties: &[Property]) -> Result<(), ScbError> {
    if properties.len() != FIXED_PROPERTIES.len() + DECIMAL_PROPERTIES.len() {
        return Err(ScbError::new(ScbErrorCode::ContractUnknown));
    }
    for property in properties {
        if let Some((_, value)) = FIXED_PROPERTIES
            .iter()
            .find(|(name, _)| *name == property.name)
        {
            if property.value != *value {
                return Err(ScbError::new(ScbErrorCode::ContractUnknown));
            }
        } else if DECIMAL_PROPERTIES.contains(&property.name.as_str()) {
            if property.value.is_empty()
                || !property.value.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(ScbError::new(ScbErrorCode::ContractUnknown));
            }
        } else {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
    }
    Ok(())
}

fn record_parts(parts: &SupervisorConfigParts) -> Result<Vec<u8>, ScbError> {
    let mut encoded_properties = Vec::new();
    for property in &parts.properties {
        encoded_properties.push(property.record()?);
    }
    let mut encoded_callers = Vec::new();
    for caller in &parts.callers {
        encoded_callers.push(caller.record()?);
    }
    encode_record(&[
        (1, encode_uvar(RECORD_VERSION)),
        (2, parts.worker_digest.to_vec()),
        (3, parts.supervisor_digest.to_vec()),
        (4, encode_list(&encoded_properties)?),
        (5, encode_list(&encoded_callers)?),
        (6, encode_uvar(parts.page_size)),
        (7, encode_uvar(parts.cleanup_millis)),
        (8, encode_uvar(u64::from(parts.launch_profile))),
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
        let text = include_str!(
            "/home/gfarch/Work/checkpoints/sley2-finish-20260915/native-final-golden.json"
        );
        let marker = format!("\"{field}\": \"");
        let start = text.find(&marker).expect("golden field present") + marker.len();
        let end = text[start..].find('"').expect("golden field ends") + start;
        decode_hex(&text[start..end])
    }

    fn golden_parts() -> SupervisorConfigParts {
        // The record is name-sorted, so fixed and decimal entries interleave.
        let mut properties: Vec<Property> = FIXED_PROPERTIES
            .iter()
            .map(|(name, value)| Property {
                name: (*name).to_owned(),
                value: (*value).to_owned(),
            })
            .chain(DECIMAL_PROPERTIES.iter().map(|name| Property {
                name: (*name).to_owned(),
                value: if *name == "MemoryMax" {
                    "4096".to_owned()
                } else {
                    "3000000".to_owned()
                },
            }))
            .collect();
        properties.sort_by(|a, b| a.name.cmp(&b.name));
        SupervisorConfigParts {
            worker_digest: [0x80; 32],
            supervisor_digest: [0x81; 32],
            properties,
            callers: vec![Caller {
                uid: 1000,
                workspace: WorkspaceId::from_bytes([0xB0; 32]),
                principal: PrincipalId::from_bytes([0xB1; 32]),
            }],
            page_size: 4096,
            cleanup_millis: SUPERVISOR_CLEANUP_MILLIS,
            launch_profile: SUPERVISOR_LAUNCH_PROFILE,
        }
    }

    fn property_index(parts: &SupervisorConfigParts, name: &str) -> usize {
        parts
            .properties
            .iter()
            .position(|property| property.name == name)
            .expect("property present")
    }

    #[test]
    fn golden_config_parses_with_exact_id() {
        let stored = golden("config_stored");
        let parsed = SupervisorConfigV1::parse(&stored).expect("golden parses");
        assert_eq!(parsed.id().as_bytes().to_vec(), golden("config_id"));
        assert_eq!(parsed.record_bytes(), golden("config_record").as_slice());
        assert_eq!(parsed.properties().len(), 16);
        assert_eq!(parsed.callers().len(), 1);
        let rebuilt = SupervisorConfigV1::build(golden_parts()).expect("parts build");
        assert_eq!(rebuilt.stored_bytes(), stored.as_slice());
    }

    #[test]
    fn projection_refusals_keep_stable_codes() {
        let mut missing = golden_parts();
        missing.properties.pop();
        assert_eq!(
            SupervisorConfigV1::build(missing)
                .expect_err("missing property")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut extra = golden_parts();
        // Insert in sorted position so the refusal proves the set check,
        // not the order check.
        extra.properties.insert(
            4,
            Property {
                name: "MemoryFoo".to_owned(),
                value: "yes".to_owned(),
            },
        );
        assert_eq!(
            SupervisorConfigV1::build(extra)
                .expect_err("extra property")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut wrong_value = golden_parts();
        wrong_value.properties[0].value = "no".to_owned();
        assert_eq!(
            SupervisorConfigV1::build(wrong_value)
                .expect_err("wrong value")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut non_decimal = golden_parts();
        let memory_max = property_index(&non_decimal, "MemoryMax");
        non_decimal.properties[memory_max].value = "4KiB".to_owned();
        assert_eq!(
            SupervisorConfigV1::build(non_decimal)
                .expect_err("non-decimal MemoryMax")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut swapped = golden_parts();
        swapped.properties.swap(0, 1);
        assert_eq!(
            SupervisorConfigV1::build(swapped)
                .expect_err("unsorted names")
                .code(),
            ScbErrorCode::FieldOrder
        );
        let mut dup = golden_parts();
        dup.properties[1] = dup.properties[0].clone();
        assert_eq!(
            SupervisorConfigV1::build(dup)
                .expect_err("duplicate")
                .code(),
            ScbErrorCode::FieldDuplicate
        );
        let mut bad_cleanup = golden_parts();
        bad_cleanup.cleanup_millis = 1000;
        assert_eq!(
            SupervisorConfigV1::build(bad_cleanup)
                .expect_err("cleanup")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut bad_profile = golden_parts();
        bad_profile.launch_profile = 2;
        assert_eq!(
            SupervisorConfigV1::build(bad_profile)
                .expect_err("profile")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let stored = golden("config_stored");
        assert_eq!(
            SupervisorConfigV1::parse(&stored[..stored.len() - 1])
                .expect_err("truncated")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        let mut tampered = stored.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert_eq!(
            SupervisorConfigV1::parse(&tampered)
                .expect_err("digest")
                .code(),
            ScbErrorCode::DigestMismatch
        );
    }
}
