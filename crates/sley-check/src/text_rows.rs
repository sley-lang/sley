//! The slice-E9 native text rows (`docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md`
//! section E9): one table, read by the S20-230 declaration and invocation
//! judgment here and by the VM's lowering judgment, fuel, and execution, so
//! the row shapes cannot drift between the checker and the VM.
//!
//! A text row is an `AdapterImport` whose adapter identity is
//! `SLY1/BRIDGE/` plus a four-byte entry code, zero padded to 32 bytes; its
//! entity identity is free (a candidate creates it under its own nonce).

use sley_ssmc::{BuiltinFailureKind, IntegerWidth, TypeExpr};

/// One native text entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextKind {
    /// `TLN1` text-length: `(Unit, Text) -> UInt(64)`, the scalar count.
    Length,
    /// `TSL1` text-slice: `(Text, (UInt(64) start, UInt(64) length)) ->
    /// Text`, scalars `start..start + length`.
    Slice,
    /// `TCT1` text-concat: `(Unit, Vector<Text>) -> Text`.
    Concat,
    /// `TCS1` text-scalars: `(Unit, Text) -> Vector<Text>`.
    Scalars,
    /// `TOR1` text-ord: `(Unit, Text) -> UInt(32)`.
    Ord,
    /// `TCH1` text-chr: `(Unit, UInt(32)) -> Text`.
    Chr,
    /// `TFI1` text-from-int: `(Unit, SInt(64)) -> Text`.
    FromInt,
}

/// Every text entry with its four-byte code, in table order.
pub const TEXT_ENTRIES: [(TextKind, [u8; 4]); 7] = [
    (TextKind::Length, *b"TLN1"),
    (TextKind::Slice, *b"TSL1"),
    (TextKind::Concat, *b"TCT1"),
    (TextKind::Scalars, *b"TCS1"),
    (TextKind::Ord, *b"TOR1"),
    (TextKind::Chr, *b"TCH1"),
    (TextKind::FromInt, *b"TFI1"),
];

/// The frozen adapter identity of an entry code: `SLY1/BRIDGE/`, the code,
/// zero padding (the E8 identity form).
#[must_use]
pub const fn text_identity(code: [u8; 4]) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    let prefix = *b"SLY1/BRIDGE/";
    let mut i = 0;
    while i < 12 {
        bytes[i] = prefix[i];
        i += 1;
    }
    let mut j = 0;
    while j < 4 {
        bytes[12 + j] = code[j];
        j += 1;
    }
    bytes
}

/// The frozen adapter identity of one entry.
#[must_use]
pub fn text_adapter_id(kind: TextKind) -> [u8; 32] {
    let code = TEXT_ENTRIES
        .iter()
        .find(|(entry, _)| *entry == kind)
        .map_or([0; 4], |(_, code)| *code);
    text_identity(code)
}

/// The text entry an adapter identity names, if any.
#[must_use]
pub fn text_kind(adapter_id: &[u8; 32]) -> Option<TextKind> {
    TEXT_ENTRIES
        .iter()
        .find(|(_, code)| text_identity(*code) == *adapter_id)
        .map(|(kind, _)| *kind)
}

fn u64_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(64))
}

fn u32_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(32))
}

/// The failure type every text row pins: `BuiltinFailure(Index)`.
#[must_use]
pub fn text_failure_type() -> TypeExpr {
    TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)
}

/// The frozen `(scope, request, response)` types of one entry.
#[must_use]
pub fn text_row_types(kind: TextKind) -> (TypeExpr, TypeExpr, TypeExpr) {
    match kind {
        TextKind::Length => (TypeExpr::Unit, TypeExpr::Text, u64_type()),
        TextKind::Slice => (
            TypeExpr::Text,
            TypeExpr::Tuple(vec![u64_type(), u64_type()]),
            TypeExpr::Text,
        ),
        TextKind::Concat => (
            TypeExpr::Unit,
            TypeExpr::Vector(Box::new(TypeExpr::Text)),
            TypeExpr::Text,
        ),
        TextKind::Scalars => (
            TypeExpr::Unit,
            TypeExpr::Text,
            TypeExpr::Vector(Box::new(TypeExpr::Text)),
        ),
        TextKind::Ord => (TypeExpr::Unit, TypeExpr::Text, u32_type()),
        TextKind::Chr => (TypeExpr::Unit, u32_type(), TypeExpr::Text),
        TextKind::FromInt => (
            TypeExpr::Unit,
            TypeExpr::SInt(IntegerWidth::from_bits(64)),
            TypeExpr::Text,
        ),
    }
}
