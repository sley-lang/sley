//! The closed opcode table: SSMC1 tag, AV1/AF1 mnemonic, SSMC1 name, and
//! the immediate the opcode carries. AF1 accepts the mnemonic, the SSMC1
//! name, or the numeric tag; AV1 prints the mnemonic.

/// The immediate an opcode takes (SSMC1 schema `op` rows).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmediateKind {
    /// No immediate.
    None,
    /// An entity (constant, record type, contract, effect, adapter, global).
    Entity,
    /// A tuple index.
    Index,
    /// A record field member.
    Field,
    /// A variant case (definition and member).
    Variant,
    /// An observation identity.
    Observation,
    /// A function reference (callee).
    Function,
}

/// One opcode row.
#[derive(Clone, Copy, Debug)]
pub struct OpcodeRow {
    /// Frozen SSMC1 tag.
    pub tag: u32,
    /// Short AV1/AF1 mnemonic.
    pub mnemonic: &'static str,
    /// SSMC1 schema name.
    pub name: &'static str,
    /// Immediate kind.
    pub immediate: ImmediateKind,
}

const fn row(
    tag: u32,
    mnemonic: &'static str,
    name: &'static str,
    immediate: ImmediateKind,
) -> OpcodeRow {
    OpcodeRow {
        tag,
        mnemonic,
        name,
        immediate,
    }
}

/// Every epoch-1 opcode.
pub const OPCODES: &[OpcodeRow] = &[
    row(1, "const", "constant_ref", ImmediateKind::Entity),
    row(16, "tuple", "tuple_new", ImmediateKind::None),
    row(17, "tuple_get", "tuple_get", ImmediateKind::Index),
    row(18, "record", "record_new", ImmediateKind::Entity),
    row(19, "field", "record_get", ImmediateKind::Field),
    row(20, "variant", "variant_new", ImmediateKind::Variant),
    row(21, "variant_get", "variant_get", ImmediateKind::Variant),
    row(32, "vec", "vector_new", ImmediateKind::None),
    row(33, "vec_len", "vector_len", ImmediateKind::None),
    row(34, "vec_get", "vector_get", ImmediateKind::None),
    row(35, "vec_set", "vector_set", ImmediateKind::None),
    row(36, "map", "map_new", ImmediateKind::None),
    row(37, "map_get", "map_get", ImmediateKind::None),
    row(38, "map_has", "map_contains", ImmediateKind::None),
    row(39, "map_insert", "map_insert", ImmediateKind::None),
    row(40, "map_remove", "map_remove", ImmediateKind::None),
    row(64, "add", "int_add_checked", ImmediateKind::None),
    row(65, "sub", "int_sub_checked", ImmediateKind::None),
    row(66, "mul", "int_mul_checked", ImmediateKind::None),
    row(67, "div", "int_div_checked", ImmediateKind::None),
    row(68, "rem", "int_rem_checked", ImmediateKind::None),
    row(69, "neg", "int_neg_checked", ImmediateKind::None),
    row(70, "shl", "int_shl_checked", ImmediateKind::None),
    row(71, "shr", "int_shr_checked", ImmediateKind::None),
    row(80, "fadd", "float_add", ImmediateKind::None),
    row(81, "fsub", "float_sub", ImmediateKind::None),
    row(82, "fmul", "float_mul", ImmediateKind::None),
    row(83, "fdiv", "float_div", ImmediateKind::None),
    row(84, "fneg", "float_neg", ImmediateKind::None),
    row(85, "fma", "float_fma", ImmediateKind::None),
    row(96, "eq", "equal", ImmediateKind::None),
    row(97, "ne", "not_equal", ImmediateKind::None),
    row(98, "lt", "less_than", ImmediateKind::None),
    row(99, "le", "less_equal", ImmediateKind::None),
    row(100, "gt", "greater_than", ImmediateKind::None),
    row(101, "ge", "greater_equal", ImmediateKind::None),
    row(102, "not", "bool_not", ImmediateKind::None),
    row(103, "and", "bool_and", ImmediateKind::None),
    row(104, "or", "bool_or", ImmediateKind::None),
    row(112, "call", "call_direct", ImmediateKind::Function),
    row(128, "some", "option_some", ImmediateKind::None),
    row(129, "none", "option_none", ImmediateKind::None),
    row(130, "ok", "result_ok", ImmediateKind::None),
    row(131, "err", "result_err", ImmediateKind::None),
    row(144, "assert", "contract_assert", ImmediateKind::Entity),
    row(145, "observe", "test_observe", ImmediateKind::Observation),
    row(160, "effect", "effect_request", ImmediateKind::Entity),
    row(161, "adapter", "adapter_invoke", ImmediateKind::Entity),
    row(162, "narrow", "capability_narrow", ImmediateKind::Entity),
    row(176, "cell", "cell_new", ImmediateKind::None),
    row(177, "cell_get", "cell_get", ImmediateKind::None),
    row(178, "cell_set", "cell_set", ImmediateKind::None),
    row(192, "hash", "value_hash", ImmediateKind::None),
    row(193, "global", "global_get", ImmediateKind::Entity),
    row(194, "fnref", "function_ref", ImmediateKind::Function),
];

/// Looks an opcode up by tag.
#[must_use]
pub fn by_tag(tag: u32) -> Option<&'static OpcodeRow> {
    OPCODES.iter().find(|row| row.tag == tag)
}

/// Looks an opcode up by mnemonic, SSMC1 name, or decimal tag.
#[must_use]
pub fn by_word(word: &str) -> Option<&'static OpcodeRow> {
    if let Ok(tag) = word.parse::<u32>() {
        return by_tag(tag);
    }
    OPCODES
        .iter()
        .find(|row| row.mnemonic == word || row.name == word)
}
