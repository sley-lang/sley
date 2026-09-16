//! Strict shared envelope and record helpers for native test evidence.
//!
//! Every record uses one envelope shape: eight magic bytes, record version one,
//! a sized canonical record, and a 32-byte domain-separated digest trailer.
//! Decoders enforce the total bound first, then magic, version, length, digest,
//! strict record shape, field ranges and counts, nested fields in tag order,
//! and finally cross-bindings. Failures keep their stable SCB1 codes.

use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_list, encode_union, encode_uvar};

/// `SLEYTPL1` envelope magic for [`crate::plan::NativeTestPlanV1`].
pub const PLAN_MAGIC: [u8; 8] = *b"SLEYTPL1";
/// `SLEYNRP1` envelope magic for [`crate::policy::NativeResourcePolicyV1`].
pub const POLICY_MAGIC: [u8; 8] = *b"SLEYNRP1";
/// Exact record version accepted by every native test evidence envelope.
pub const RECORD_VERSION: u64 = 1;
/// Native-v1 implementation ceiling for one stored evidence envelope.
///
/// Well under the 48 MiB aggregate evidence bound so a maximal plan still
/// leaves room for receipt fields; local policy may only tighten it.
pub const MAX_STORED_BYTES: usize = 4_194_304;
/// Maximum selected entries in one native test plan.
pub const MAX_SELECTED_ENTRIES: u64 = 256;
/// Maximum changed-test inventory entries in one native test plan.
pub const MAX_CHANGED_ENTRIES: u64 = 65_535;

fn resource_error() -> ScbError {
    ScbError::new(ScbErrorCode::ResourceLimit)
}

/// Canonical preimage bytes: magic, version one, and the sized record.
///
/// The caller derives the domain-separated identity over these bytes and
/// appends the digest as the envelope trailer.
///
/// # Errors
/// Returns `SCB_RESOURCE_LIMIT` when the record exceeds the stored bound.
pub fn preimage_bytes(magic: [u8; 8], record: &[u8]) -> Result<Vec<u8>, ScbError> {
    if record.len() > MAX_STORED_BYTES {
        return Err(resource_error());
    }
    let len = u64::try_from(record.len()).map_err(|_| resource_error())?;
    let mut out = Vec::with_capacity(8 + 1 + 9 + record.len());
    out.extend_from_slice(&magic);
    out.extend_from_slice(&encode_uvar(RECORD_VERSION));
    out.extend_from_slice(&encode_uvar(len));
    out.extend_from_slice(record);
    Ok(out)
}

/// Strictly decodes one envelope, returning the record and digest trailer.
///
/// # Errors
/// Returns `SCB_RESOURCE_LIMIT` for an oversized envelope, `SCB_MAGIC_INVALID`
/// for a wrong magic, `SCB_VERSION_UNSUPPORTED` for a wrong version,
/// `SCB_LENGTH_OVERFLOW` for truncation, or `SCB_TRAILING_BYTES` for surplus.
pub fn decode_envelope(stored: &[u8], magic: [u8; 8]) -> Result<(Vec<u8>, [u8; 32]), ScbError> {
    if stored.len() > MAX_STORED_BYTES {
        return Err(resource_error());
    }
    let mut cursor = ScbValueCursor::new(stored)?;
    let actual = cursor.read_exact_bytes(8)?;
    if actual != magic {
        return Err(ScbError::new(ScbErrorCode::MagicInvalid));
    }
    if cursor.read_uvar(64)? != RECORD_VERSION {
        return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
    }
    let record = cursor.read_bytes()?.to_vec();
    let trailer = cursor.read_exact_bytes(32)?;
    cursor.check_finished()?;
    let trailer = <[u8; 32]>::try_from(trailer).map_err(|_| resource_error())?;
    Ok((record, trailer))
}

/// Strictly decodes record fields in ascending tag order without duplicates.
///
/// # Errors
/// Returns `SCB_FIELD_DUPLICATE` for a repeated tag, `SCB_FIELD_ORDER` for a
/// descending tag, or the stable length/trailing codes for malformed input.
pub fn decode_fields(record: &[u8]) -> Result<Vec<(u32, Vec<u8>)>, ScbError> {
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

/// Requires the exact expected tag set in tag order.
///
/// # Errors
/// Returns `SCB_FIELD_MISSING` for a skipped required tag or a short record,
/// `SCB_FIELD_UNKNOWN` for an extra tag.
pub fn expect_tags(fields: &[(u32, Vec<u8>)], expected: &[u32]) -> Result<(), ScbError> {
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

/// Reads an exact 32-byte identity from a field value.
///
/// # Errors
/// Returns `SCB_LENGTH_OVERFLOW` when the value is not exactly 32 bytes.
pub fn read_id(value: &[u8]) -> Result<[u8; 32], ScbError> {
    <[u8; 32]>::try_from(value).map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))
}

/// Reads one canonical unsigned integer field with an explicit bit width.
///
/// # Errors
/// Returns the stable varint, integer, or trailing codes for malformed input.
pub fn read_uvar_value(value: &[u8], width: u8) -> Result<u64, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let parsed = cursor.read_uvar(width)?;
    cursor.check_finished()?;
    Ok(parsed)
}

/// Reads one optional identity union: tag zero empty, tag one 32 bytes.
///
/// # Errors
/// Returns `SCB_UNION_INVALID` for any other tag or a nonempty absent payload,
/// `SCB_LENGTH_OVERFLOW` for a present payload that is not 32 bytes.
pub fn read_option_id(value: &[u8]) -> Result<Option<[u8; 32]>, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let (tag, payload) = cursor.read_union()?;
    cursor.check_finished()?;
    match tag {
        0 => {
            if payload.is_empty() {
                Ok(None)
            } else {
                Err(ScbError::new(ScbErrorCode::UnionInvalid))
            }
        }
        1 => Ok(Some(read_id(payload)?)),
        _ => Err(ScbError::new(ScbErrorCode::UnionInvalid)),
    }
}

/// Reads a bounded list of exact 32-byte identities.
///
/// # Errors
/// Returns `SCB_RESOURCE_LIMIT` beyond `max` elements, `SCB_LENGTH_OVERFLOW`
/// for a non-32-byte element, or the stable trailing code for surplus bytes.
pub fn read_id_list(value: &[u8], max: u64) -> Result<Vec<[u8; 32]>, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_list_count()?;
    if count > max {
        return Err(resource_error());
    }
    let mut ids = Vec::new();
    for _ in 0..count {
        ids.push(read_id(cursor.read_bytes()?)?);
    }
    cursor.check_finished()?;
    Ok(ids)
}

/// Requires raw-ID ascending order without duplicates.
///
/// # Errors
/// Returns `SCB_FIELD_DUPLICATE` for a repeated identity, `SCB_FIELD_ORDER`
/// for a descending identity.
pub fn check_sorted_unique(ids: &[[u8; 32]]) -> Result<(), ScbError> {
    for pair in ids.windows(2) {
        if pair[1] == pair[0] {
            return Err(ScbError::new(ScbErrorCode::FieldDuplicate));
        }
        if pair[1] < pair[0] {
            return Err(ScbError::new(ScbErrorCode::FieldOrder));
        }
    }
    Ok(())
}

/// Encodes an identity list from already sorted unique identities.
///
/// # Errors
/// Returns `SCB_RESOURCE_LIMIT` beyond epoch collection limits.
pub fn encode_id_list(ids: &[[u8; 32]]) -> Result<Vec<u8>, ScbError> {
    encode_list(&ids.iter().map(|id| id.to_vec()).collect::<Vec<_>>())
}

/// Encodes an optional identity union: tag zero empty, tag one 32 bytes.
///
/// # Errors
/// Returns `SCB_RESOURCE_LIMIT` beyond epoch value limits.
pub fn encode_option_id(id: Option<[u8; 32]>) -> Result<Vec<u8>, ScbError> {
    match id {
        None => encode_union(0, &[]),
        Some(id) => encode_union(1, &id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sley_scb1::encode_record;

    #[test]
    fn envelope_roundtrip_preserves_record_and_trailer() {
        let record = vec![0x01, 0x02, 0x03];
        let preimage = preimage_bytes(*b"SLEYTPL1", &record).expect("envelope encodes");
        assert_eq!(&preimage[..8], b"SLEYTPL1");
        let mut stored = preimage.clone();
        stored.extend_from_slice(&[0x77; 32]);
        let (parsed, trailer) = decode_envelope(&stored, *b"SLEYTPL1").expect("envelope parses");
        assert_eq!(parsed, record);
        assert_eq!(trailer, [0x77; 32]);
    }

    #[test]
    fn envelope_refusals_keep_stable_codes() {
        let record = vec![0x01];
        let preimage = preimage_bytes(*b"SLEYTPL1", &record).expect("envelope encodes");
        let mut stored = preimage.clone();
        stored.extend_from_slice(&[0x01; 32]);
        assert_eq!(
            decode_envelope(&stored, *b"SLEYNRP1")
                .expect_err("wrong magic")
                .code(),
            ScbErrorCode::MagicInvalid
        );
        let mut bad_version = b"SLEYTPL1".to_vec();
        bad_version.extend_from_slice(&encode_uvar(2));
        bad_version.extend_from_slice(&preimage[9..]);
        bad_version.extend_from_slice(&[0x01; 32]);
        assert_eq!(
            decode_envelope(&bad_version, *b"SLEYTPL1")
                .expect_err("wrong version")
                .code(),
            ScbErrorCode::VersionUnsupported
        );
        assert_eq!(
            decode_envelope(&stored[..stored.len() - 1], *b"SLEYTPL1")
                .expect_err("truncated")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        let mut trailing = stored.clone();
        trailing.push(0x00);
        assert_eq!(
            decode_envelope(&trailing, *b"SLEYTPL1")
                .expect_err("trailing")
                .code(),
            ScbErrorCode::TrailingBytes
        );
        assert_eq!(
            preimage_bytes(*b"SLEYTPL1", &vec![0x00; MAX_STORED_BYTES + 1])
                .expect_err("oversized")
                .code(),
            ScbErrorCode::ResourceLimit
        );
    }

    #[test]
    fn field_shape_checks_are_exact() {
        let record = encode_record(&[(1, vec![0x01]), (3, vec![0x03])]).expect("record encodes");
        let fields = decode_fields(&record).expect("fields decode");
        assert_eq!(
            expect_tags(&fields, &[1, 2, 3]).expect_err("gap").code(),
            ScbErrorCode::FieldMissing
        );
        assert_eq!(
            expect_tags(&fields, &[1, 3, 4]).expect_err("short").code(),
            ScbErrorCode::FieldMissing
        );
        assert_eq!(
            expect_tags(&fields, &[1]).expect_err("extra").code(),
            ScbErrorCode::FieldUnknown
        );
        expect_tags(&fields, &[1, 3]).expect("exact set passes");
        let dup = encode_record(&[(1, vec![0x01]), (1, vec![0x02])]).expect_err("dup refused");
        assert_eq!(dup.code(), ScbErrorCode::FieldDuplicate);
    }

    #[test]
    fn option_and_list_codecs_reject_noncanonical_shapes() {
        assert_eq!(
            read_option_id(&encode_option_id(None).expect("none encodes")).expect("none"),
            None
        );
        assert_eq!(
            read_option_id(&encode_option_id(Some([0x09; 32])).expect("some encodes"))
                .expect("some"),
            Some([0x09; 32])
        );
        assert_eq!(
            read_option_id(&encode_union(2, &[]).expect("tag encodes"))
                .expect_err("tag")
                .code(),
            ScbErrorCode::UnionInvalid
        );
        assert_eq!(
            read_option_id(&encode_union(0, &[0x01]).expect("tag encodes"))
                .expect_err("payload")
                .code(),
            ScbErrorCode::UnionInvalid
        );
        assert_eq!(
            read_id(&[0x01; 31]).expect_err("short id").code(),
            ScbErrorCode::LengthOverflow
        );
        let ids = [[0x01; 32], [0x02; 32]];
        let encoded = encode_id_list(&ids).expect("list encodes");
        assert_eq!(read_id_list(&encoded, 2).expect("list parses"), ids);
        assert_eq!(
            read_id_list(&encoded, 1).expect_err("over max").code(),
            ScbErrorCode::ResourceLimit
        );
        assert!(check_sorted_unique(&ids).is_ok());
        assert_eq!(
            check_sorted_unique(&[[0x02; 32], [0x01; 32]])
                .expect_err("order")
                .code(),
            ScbErrorCode::FieldOrder
        );
        assert_eq!(
            check_sorted_unique(&[[0x01; 32], [0x01; 32]])
                .expect_err("dup")
                .code(),
            ScbErrorCode::FieldDuplicate
        );
    }
}
