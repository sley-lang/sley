//! Numeric failure codes the toolchain program returns.
//!
//! Codes that already have frozen numeric values keep them (`SSMC_*` 20000+,
//! `TYPE_*` 21000+, `GRAPH_*`/`CFG_*` 22000+, `VM_LOWER_*` 26000+). The
//! `SCB_*` vocabulary is symbolic only, so the toolchain assigns 10000 plus
//! the `ScbErrorCode` declaration ordinal. Toolchain-driver failures use
//! 29000+.

/// `SCB_MAGIC_INVALID`.
pub const SCB_MAGIC_INVALID: u32 = 10_000;
/// `SCB_VERSION_UNSUPPORTED`.
pub const SCB_VERSION_UNSUPPORTED: u32 = 10_001;
/// `SCB_CONTRACT_UNKNOWN`.
pub const SCB_CONTRACT_UNKNOWN: u32 = 10_002;
/// `SCB_EPOCH_MISMATCH`.
pub const SCB_EPOCH_MISMATCH: u32 = 10_003;
/// `SCB_DIGEST_MISMATCH`.
pub const SCB_DIGEST_MISMATCH: u32 = 10_004;
/// `SCB_TRAILING_BYTES`.
pub const SCB_TRAILING_BYTES: u32 = 10_005;
/// `SCB_VARINT_NON_MINIMAL`.
pub const SCB_VARINT_NON_MINIMAL: u32 = 10_006;
/// `SCB_INTEGER_OVERFLOW`.
pub const SCB_INTEGER_OVERFLOW: u32 = 10_007;
/// `SCB_LENGTH_OVERFLOW`.
pub const SCB_LENGTH_OVERFLOW: u32 = 10_008;
/// `SCB_BOOL_INVALID`.
pub const SCB_BOOL_INVALID: u32 = 10_009;
/// `SCB_UTF8_INVALID`.
pub const SCB_UTF8_INVALID: u32 = 10_010;
/// `SCB_LABEL_NOT_NFC`.
pub const SCB_LABEL_NOT_NFC: u32 = 10_011;
/// `SCB_FLOAT_NON_CANONICAL`.
pub const SCB_FLOAT_NON_CANONICAL: u32 = 10_012;
/// `SCB_FIELD_MISSING`.
pub const SCB_FIELD_MISSING: u32 = 10_013;
/// `SCB_FIELD_UNKNOWN`.
pub const SCB_FIELD_UNKNOWN: u32 = 10_014;
/// `SCB_FIELD_DUPLICATE`.
pub const SCB_FIELD_DUPLICATE: u32 = 10_015;
/// `SCB_FIELD_ORDER`.
pub const SCB_FIELD_ORDER: u32 = 10_016;
/// `SCB_UNION_INVALID`.
pub const SCB_UNION_INVALID: u32 = 10_017;
/// `SCB_MAP_ORDER`.
pub const SCB_MAP_ORDER: u32 = 10_018;
/// `SCB_MAP_DUPLICATE`.
pub const SCB_MAP_DUPLICATE: u32 = 10_019;
/// `SCB_EXTENSION_UNKNOWN`.
pub const SCB_EXTENSION_UNKNOWN: u32 = 10_020;
/// `SCB_RESOURCE_LIMIT`.
pub const SCB_RESOURCE_LIMIT: u32 = 10_021;

/// Maps a native `ScbErrorCode` to its toolchain number.
#[must_use]
pub fn scb_code(code: sley_scb1::ScbErrorCode) -> u32 {
    use sley_scb1::ScbErrorCode as C;
    match code {
        C::MagicInvalid => SCB_MAGIC_INVALID,
        C::VersionUnsupported => SCB_VERSION_UNSUPPORTED,
        C::ContractUnknown => SCB_CONTRACT_UNKNOWN,
        C::EpochMismatch => SCB_EPOCH_MISMATCH,
        C::DigestMismatch => SCB_DIGEST_MISMATCH,
        C::TrailingBytes => SCB_TRAILING_BYTES,
        C::VarintNonMinimal => SCB_VARINT_NON_MINIMAL,
        C::IntegerOverflow => SCB_INTEGER_OVERFLOW,
        C::LengthOverflow => SCB_LENGTH_OVERFLOW,
        C::BoolInvalid => SCB_BOOL_INVALID,
        C::Utf8Invalid => SCB_UTF8_INVALID,
        C::LabelNotNfc => SCB_LABEL_NOT_NFC,
        C::FloatNonCanonical => SCB_FLOAT_NON_CANONICAL,
        C::FieldMissing => SCB_FIELD_MISSING,
        C::FieldUnknown => SCB_FIELD_UNKNOWN,
        C::FieldDuplicate => SCB_FIELD_DUPLICATE,
        C::FieldOrder => SCB_FIELD_ORDER,
        C::UnionInvalid => SCB_UNION_INVALID,
        C::MapOrder => SCB_MAP_ORDER,
        C::MapDuplicate => SCB_MAP_DUPLICATE,
        C::ExtensionUnknown => SCB_EXTENSION_UNKNOWN,
        _ => SCB_RESOURCE_LIMIT,
    }
}

/// `SSMC_ENTITY_KIND_UNKNOWN`.
pub const SSMC_ENTITY_KIND_UNKNOWN: u32 = 20_000;
/// `SSMC_BODY_KIND_MISMATCH`.
pub const SSMC_BODY_KIND_MISMATCH: u32 = 20_001;
/// `SSMC_RESERVED_FIELD_PRESENT`.
pub const SSMC_RESERVED_FIELD_PRESENT: u32 = 20_015;

/// Toolchain: an entity kind outside the toolchain's program domain.
pub const TOOL_KIND_OUT_OF_DOMAIN: u32 = 29_000;
/// Toolchain: an object label outside the ASCII label subset.
pub const TOOL_LABEL_OUT_OF_DOMAIN: u32 = 29_001;
/// Toolchain: the object set and the root bindings disagree.
pub const TOOL_ROOT_BINDING_MISMATCH: u32 = 29_002;
/// Toolchain: malformed build request.
pub const TOOL_REQUEST_MALFORMED: u32 = 29_003;
