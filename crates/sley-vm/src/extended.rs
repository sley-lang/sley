//! Extended opcode profile (`docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md`,
//! ADR-0039): the signature judgment and the runtime semantics of the
//! opcode families beyond the restricted Boolean profile. Slice E1 lands
//! the data family (constants, tuples, vectors, comparisons, options,
//! results); every other opcode fails closed until its slice lands.

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId};
use sley_ssmc::{
    AdapterImport, BuiltinFailureKind, BuiltinFailureValue, ConstData, ConstValue,
    ConstantDefinition, ContractDefinition, ContractKind, FieldConst, FunctionGraph, FunctionType,
    GlobalValueDefinition, Immediate, IntegerWidth, MapEntryConst, NamedType, Opcode, Operation,
    Parameter, RecordConst, RecordField, ResultConst, TypeDefForm, TypeExpr, VariantCase,
    VariantConst, fingerprint::hash_validated_value,
};

use crate::{LowerError, LowerErrorCode};

/// Largest tuple the extended profile constructs.
pub const MAX_TUPLE_ARITY: usize = 64;

fn fail<T>(code: LowerErrorCode) -> Result<T, LowerError> {
    Err(LowerError::new(code))
}

fn u64_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(64))
}

/// Whether a type contains a float anywhere; E1 equality excludes floats
/// until E3 defines IEEE comparison.
fn contains_float(value: &TypeExpr) -> bool {
    match value {
        TypeExpr::F32 | TypeExpr::F64 => true,
        TypeExpr::Tuple(items) => items.iter().any(contains_float),
        TypeExpr::Named(named) => named.arguments.iter().any(contains_float),
        TypeExpr::Vector(inner) | TypeExpr::Option(inner) | TypeExpr::LocalCell(inner) => {
            contains_float(inner)
        }
        TypeExpr::OrderedMap { key, value } => contains_float(key) || contains_float(value),
        TypeExpr::Result { ok, error } => contains_float(ok) || contains_float(error),
        TypeExpr::FunctionRef(function) => {
            function.parameters.iter().any(contains_float) || contains_float(&function.result)
        }
        _ => false,
    }
}

/// The signedness and width of an epoch-1 integer type.
fn integer_width(value: &TypeExpr) -> Option<(bool, u16)> {
    match value {
        TypeExpr::SInt(width) if width.is_epoch_1() => Some((true, width.bits())),
        TypeExpr::UInt(width) if width.is_epoch_1() => Some((false, width.bits())),
        _ => None,
    }
}

fn u32_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(32))
}

fn arithmetic_result(value: &TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(value.clone()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    }
}

/// Everything the extended judgment reads from the lowering inventory.
pub struct LoweringContext<'a> {
    /// Selected type environment.
    pub types: &'a TypeEnvironment,
    /// Complete Constant inventory.
    pub constants: &'a [ConstantDefinition],
    /// Complete `GlobalValue` inventory.
    pub globals: &'a [GlobalValueDefinition],
    /// Complete Function inventory (referenced functions).
    pub functions: &'a [FunctionGraph],
    /// Complete Parameter inventory (referenced function signatures).
    pub parameters: &'a [Parameter],
    /// Complete Contract inventory (slice E7a `contract_assert`).
    pub contracts: &'a [ContractDefinition],
    /// Complete `AdapterImport` inventory (slice E8 bridge entries).
    pub adapters: &'a [AdapterImport],
    /// The function whose operations are being judged (slice E7a).
    pub function: EntityId,
}

/// Everything the extended semantics read or mutate during one execution.
pub struct ExecutionContext<'a> {
    /// Selected type environment.
    pub types: &'a TypeEnvironment,
    /// Complete Constant inventory.
    pub constants: &'a [ConstantDefinition],
    /// Complete `GlobalValue` inventory.
    pub globals: &'a [GlobalValueDefinition],
    /// Exact schema epoch (`value_hash`).
    pub schema_epoch: SchemaEpochId,
    /// Complete `AdapterImport` inventory (slice E8 bridge entries resolve
    /// their `Entity` immediate here, as in judgment).
    pub adapters: &'a [AdapterImport],
    /// Per-execution cell contents; a cell handle is the index into this list.
    pub cells: &'a mut Vec<ConstValue>,
}

/// Whether a type contains a `LocalCell` anywhere.
#[must_use]
pub fn contains_cell(value: &TypeExpr) -> bool {
    match value {
        TypeExpr::LocalCell(_) => true,
        TypeExpr::Tuple(items) => items.iter().any(contains_cell),
        TypeExpr::Named(named) => named.arguments.iter().any(contains_cell),
        TypeExpr::Vector(inner) | TypeExpr::Option(inner) => contains_cell(inner),
        TypeExpr::OrderedMap { key, value } => contains_cell(key) || contains_cell(value),
        TypeExpr::Result { ok, error } => contains_cell(ok) || contains_cell(error),
        TypeExpr::FunctionRef(function) => {
            function.parameters.iter().any(contains_cell) || contains_cell(&function.result)
        }
        _ => false,
    }
}

/// A Function result may not carry a cell out of its execution (contract E5).
///
/// # Errors
///
/// `VM_LOWER_SIGNATURE_MISMATCH` when the result type contains a `LocalCell`.
pub fn check_result_type(result_type: &TypeExpr) -> Result<(), LowerError> {
    if contains_cell(result_type) {
        fail(LowerErrorCode::SignatureMismatch)
    } else {
        Ok(())
    }
}

/// Slice E8 host bridge (contract section E8): the frozen import-entry
/// capacity all bridge byte strings and octet vectors share.
pub(crate) const BRIDGE_MAX_ITEMS: usize = 1_048_576;

/// Slice E8: the bridge-capacity failure code under `BuiltinFailure(Index)`,
/// distinct from the index-out-of-range code 1 the `VectorSet` precedent
/// pins. The RW-030 admission record's "typed Limit failure" is realized as
/// this code: the `BuiltinFailureKind` set is epoch-closed while codes are
/// per-kind values, and capacity refusal belongs to the collection-bounds
/// family the `Index` kind already owns.
pub(crate) const BRIDGE_CAPACITY_CODE: u16 = 2;

/// Slice E8: fuel charged per converted or pushed element through
/// `charge_action`, on top of the existing per-instruction charge.
pub(crate) const BRIDGE_ELEMENT_FUEL: u64 = 1;

/// One landed slice-E8 bridge entry over `adapter_invoke` (contract E8).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BridgeEntry {
    /// `host-bytes-to-u8vector`.
    BytesToVector,
    /// `host-u8vector-to-bytes`.
    VectorToBytes,
    /// `vector-push`.
    VectorPush,
}

/// The frozen `Entity` identity of one bridge entry: twelve ASCII bytes
/// `SLY1/BRIDGE/`, the four-byte entry code, zero padding to 32 bytes.
///
/// These are REWEAVE host-ABI import identities (the RW-070 freeze records
/// them), not reference-adapter identities: bridge entries are pure value
/// functions over caller-owned values, so no `AdapterCall` effect, no
/// fixture state, and no reference-registry kind applies to them. What they
/// share with every frozen `adapter_invoke` use is the invocation shape —
/// two operands (`scope`, `request`) and an `Entity` immediate naming an
/// import with declared request/response/failure types — which the judgment
/// below enforces exactly.
pub(crate) fn bridge_entry_id(code: [u8; 4]) -> EntityId {
    let mut bytes = [0_u8; 32];
    bytes[..12].copy_from_slice(b"SLY1/BRIDGE/");
    bytes[12..16].copy_from_slice(&code);
    EntityId::from_bytes(bytes)
}

/// The frozen external adapter identity bytes of one bridge entry.
fn bridge_adapter_id(code: [u8; 4]) -> [u8; 32] {
    *bridge_entry_id(code).as_bytes()
}

/// Builds the frozen `AdapterImport` value for one bridge entry: a genuine
/// epoch-1 import row (identity, adapter identity, ABI version 1, exact
/// request/response types, `Index` failure type, empty effect list).
/// Entries are pure value functions, so the effect list is correctly empty:
/// there is no host-state authority for an `AdapterCall` effect to confine
/// (owner amendment A1, S20-230 §1.5: registered pure deterministic host
/// primitives; no `AdapterCall` is manufactured).
///
/// Test-only constructor: production inventories arrive in S20-230
/// requests; only fixtures build the frozen rows directly.
#[cfg(test)]
pub(crate) fn bridge_import(
    entry: BridgeEntry,
    request: TypeExpr,
    response: TypeExpr,
) -> AdapterImport {
    let code = match entry {
        BridgeEntry::BytesToVector => *b"B2V1",
        BridgeEntry::VectorToBytes => *b"V2B1",
        BridgeEntry::VectorPush => *b"PSH1",
    };
    AdapterImport {
        entity_id: bridge_entry_id(code),
        adapter_id: bridge_adapter_id(code),
        abi_version: 1,
        request_type: request,
        response_type: response,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    }
}

/// The three frozen bridge imports for test and fixture inventories: the
/// two conversions with their exact rows, and the push row with its
/// representative `UInt(8)` instantiation (judgment derives push types
/// per use from the operands; the row pins identity and purity).
#[cfg(test)]
pub(crate) fn bridge_test_imports() -> [AdapterImport; 3] {
    [
        bridge_import(
            BridgeEntry::BytesToVector,
            TypeExpr::Bytes,
            TypeExpr::Vector(Box::new(u8_type())),
        ),
        bridge_import(
            BridgeEntry::VectorToBytes,
            TypeExpr::Vector(Box::new(u8_type())),
            TypeExpr::Bytes,
        ),
        bridge_import(
            BridgeEntry::VectorPush,
            u8_type(),
            TypeExpr::Vector(Box::new(u8_type())),
        ),
    ]
}

/// One resolved bridge call: which entry the carried import names after
/// every frozen field pins it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BridgeKind {
    BytesToVector,
    VectorToBytes,
    VectorPush,
}

/// Resolves an `adapter_invoke` immediate against the supplied import
/// inventory to its landed bridge entry, if it names one. Resolution is
/// genuine: the immediate must name a carried `AdapterImport`, and every
/// frozen field of that row must equal the frozen bridge values — identity,
/// adapter identity, ABI version 1, `Index` failure type, and empty effect
/// list. For the conversions the carried request/response types must equal
/// the frozen rows. For push the carried row must satisfy the relationship
/// pin itself (response exactly `Vector` of the request type); per-use
/// operand binding against the row happens at judgment, so a conforming
/// row serves only its own element type. Anything else is not a landed
/// import and stays `VM_LOWER_OPCODE_UNSUPPORTED`.
fn resolve_bridge_entry<'a>(
    adapters: &'a [AdapterImport],
    id: &EntityId,
) -> Option<(BridgeKind, &'a AdapterImport)> {
    let carried = adapters.iter().find(|import| import.entity_id == *id)?;
    let entry = if carried.entity_id == bridge_entry_id(*b"B2V1") {
        BridgeEntry::BytesToVector
    } else if carried.entity_id == bridge_entry_id(*b"V2B1") {
        BridgeEntry::VectorToBytes
    } else if carried.entity_id == bridge_entry_id(*b"PSH1") {
        BridgeEntry::VectorPush
    } else {
        return None;
    };
    let code = match entry {
        BridgeEntry::BytesToVector => *b"B2V1",
        BridgeEntry::VectorToBytes => *b"V2B1",
        BridgeEntry::VectorPush => *b"PSH1",
    };
    if carried.adapter_id != bridge_adapter_id(code)
        || carried.abi_version != 1
        || carried.failure_type != TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)
        || !carried.effects.is_empty()
    {
        return None;
    }
    match entry {
        BridgeEntry::VectorPush => {
            if carried.response_type == TypeExpr::Vector(Box::new(carried.request_type.clone())) {
                Some((BridgeKind::VectorPush, carried))
            } else {
                None
            }
        }
        conversion => {
            let (request, response) = match conversion {
                BridgeEntry::BytesToVector => (TypeExpr::Bytes, u8vec_type()),
                BridgeEntry::VectorToBytes => (u8vec_type(), TypeExpr::Bytes),
                BridgeEntry::VectorPush => return None,
            };
            if carried.request_type == request && carried.response_type == response {
                Some((
                    if conversion == BridgeEntry::BytesToVector {
                        BridgeKind::BytesToVector
                    } else {
                        BridgeKind::VectorToBytes
                    },
                    carried,
                ))
            } else {
                None
            }
        }
    }
}

fn u8_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(8))
}

fn u8vec_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(u8_type()))
}

fn bridge_index_error() -> TypeExpr {
    TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)
}

/// Per-element fuel a bridge entry charges: the request length for the two
/// conversions, one for a push. `None` outside the landed entries.
#[must_use]
pub fn bridge_fuel_surcharge(
    adapters: &[AdapterImport],
    immediate: &Immediate,
    operands: &[ConstValue],
) -> Option<u64> {
    let Immediate::Entity(entry) = immediate else {
        return None;
    };
    match resolve_bridge_entry(adapters, entry)?.0 {
        BridgeKind::BytesToVector => match operands {
            [_, request] => match &request.data {
                ConstData::Bytes(bytes) => u64::try_from(bytes.len()).ok(),
                _ => None,
            },
            _ => None,
        },
        BridgeKind::VectorToBytes => match operands {
            [_, request] => match &request.data {
                ConstData::Sequence(items) => u64::try_from(items.len()).ok(),
                _ => None,
            },
            _ => None,
        },
        BridgeKind::VectorPush => match operands {
            [_, _] => Some(1),
            _ => None,
        },
    }
}

/// Runs one landed bridge entry. Total by construction: representation
/// conversion only, capacity refusal as an `Index` code-2 value, element
/// shapes rechecked (a mismatch is an internal fault, never a mistyped
/// value), and no tag parsing, schema dispatch, canonical-form judgment,
/// image assembly, or verdict anywhere (contract section E8).
///
/// Value-type binding (Ariadne Phase-2 HIGH-1 repair): the carried row is
/// revalidated here, not just at judgment, so the public execution helper
/// cannot serve a registered identity with off-row operand or result
/// types. Operand value types must equal the row (scope `Unit` for the
/// conversions, the row response for push; request always the row
/// request), and the result must equal `Result<row response, row
/// failure>`. Anything else is an internal fault.
fn bridge_execute(
    entry: BridgeKind,
    carried: &AdapterImport,
    operands: &[ConstValue],
    result_type: &TypeExpr,
) -> Result<ConstValue, ExtendedFault> {
    let TypeExpr::Result { ok, error } = result_type else {
        return Err(ExtendedFault);
    };
    if **ok != carried.response_type || **error != carried.failure_type {
        return Err(ExtendedFault);
    }
    let ok_value = |data: ConstData| ConstValue {
        value_type: ok.as_ref().clone(),
        data,
    };
    let done = |data: ConstData| ConstValue {
        value_type: result_type.clone(),
        data: ConstData::Result(ResultConst::Ok(Box::new(ok_value(data)))),
    };
    let capacity = || ConstValue {
        value_type: result_type.clone(),
        data: ConstData::Result(ResultConst::Err(Box::new(ConstValue {
            value_type: error.as_ref().clone(),
            data: ConstData::BuiltinFailure(BuiltinFailureValue {
                kind: BuiltinFailureKind::Index,
                code: BRIDGE_CAPACITY_CODE,
            }),
        }))),
    };
    match entry {
        BridgeKind::BytesToVector => {
            let [scope, request] = operands else {
                return Err(ExtendedFault);
            };
            if scope.value_type != TypeExpr::Unit || request.value_type != carried.request_type {
                return Err(ExtendedFault);
            }
            let ConstData::Unit = scope.data else {
                return Err(ExtendedFault);
            };
            let ConstData::Bytes(bytes) = &request.data else {
                return Err(ExtendedFault);
            };
            if bytes.len() > BRIDGE_MAX_ITEMS {
                return Ok(capacity());
            }
            Ok(done(ConstData::Sequence(
                bytes
                    .iter()
                    .map(|byte| ConstValue {
                        value_type: u8_type(),
                        data: ConstData::UInt(u128::from(*byte)),
                    })
                    .collect(),
            )))
        }
        BridgeKind::VectorToBytes => {
            let [scope, request] = operands else {
                return Err(ExtendedFault);
            };
            if scope.value_type != TypeExpr::Unit || request.value_type != carried.request_type {
                return Err(ExtendedFault);
            }
            let ConstData::Unit = scope.data else {
                return Err(ExtendedFault);
            };
            let ConstData::Sequence(items) = &request.data else {
                return Err(ExtendedFault);
            };
            if items.len() > BRIDGE_MAX_ITEMS {
                return Ok(capacity());
            }
            let mut bytes = Vec::with_capacity(items.len());
            for item in items {
                let ConstData::UInt(byte) = item.data else {
                    return Err(ExtendedFault);
                };
                bytes.push(u8::try_from(byte).map_err(|_| ExtendedFault)?);
            }
            Ok(done(ConstData::Bytes(bytes)))
        }
        BridgeKind::VectorPush => {
            let [scope, request] = operands else {
                return Err(ExtendedFault);
            };
            if scope.value_type != carried.response_type
                || request.value_type != carried.request_type
            {
                return Err(ExtendedFault);
            }
            let ConstData::Sequence(items) = &scope.data else {
                return Err(ExtendedFault);
            };
            if items.len() >= BRIDGE_MAX_ITEMS {
                return Ok(capacity());
            }
            let mut grown = items.clone();
            grown.push(request.clone());
            Ok(done(ConstData::Sequence(grown)))
        }
    }
}

/// The exact `contract_assert` result type of slice E7a.
fn contract_assert_result() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Unit),
        error: Box::new(TypeExpr::BuiltinFailure(
            BuiltinFailureKind::ContractViolation,
        )),
    }
}

fn function_signature(
    context: &LoweringContext<'_>,
    function: EntityId,
) -> Result<FunctionType, LowerError> {
    let graph = context
        .functions
        .iter()
        .find(|graph| graph.entity_id == function)
        .filter(|graph| graph.type_parameters.is_empty())
        .ok_or_else(|| LowerError::new(LowerErrorCode::ImmediateMismatch))?;
    let mut parameters = Vec::with_capacity(graph.parameters.len());
    for parameter in &graph.parameters {
        let found = context
            .parameters
            .iter()
            .find(|candidate| candidate.entity_id == *parameter)
            .ok_or_else(|| LowerError::new(LowerErrorCode::ImmediateMismatch))?;
        parameters.push(found.value_type.clone());
    }
    Ok(FunctionType {
        parameters,
        result: Box::new(graph.result_type.clone()),
        effects: graph.effects.clone(),
    })
}

fn global_initializer<'a>(
    globals: &'a [GlobalValueDefinition],
    constants: &'a [ConstantDefinition],
    global: EntityId,
) -> Option<(&'a GlobalValueDefinition, &'a ConstantDefinition)> {
    let definition = globals
        .iter()
        .find(|candidate| candidate.entity_id == global)?;
    let constant = constants
        .iter()
        .find(|candidate| candidate.entity_id == definition.initializer)?;
    (constant.value.value_type == definition.value_type).then_some((definition, constant))
}

/// Requires that every constant one Function's operations name has an exact
/// S20-350 canonical form (contract E4).
///
/// `constant_ref` and `global_get` hand an artifact value straight to a
/// register, and extended `equal` and `value_hash` read ordered-map entry
/// order structurally. That order is established by the selected SCB
/// encoder/decoder and deliberately not by S20-210, which never reimplements
/// the byte order and never silently sorts a decoded constant
/// (`TYPE_SYSTEM_V1.md` section 5). This asks the codec the same question
/// instead of sorting the value or restating the order.
///
/// Only referenced constants are examined: one an operation cannot name can
/// never reach a register. An immediate that resolves to nothing is the
/// judgment's failure, not this one.
///
/// # Errors
///
/// `VM_LOWER_IMMEDIATE_MISMATCH` when a named constant has no canonical form.
pub(crate) fn require_canonical_referenced_constants(
    operations: &[Operation],
    constants: &[ConstantDefinition],
    globals: &[GlobalValueDefinition],
) -> Result<(), LowerError> {
    for operation in operations {
        let named = match (operation.opcode, &operation.immediate) {
            (Opcode::ConstantRef, Immediate::Entity(id)) => constants
                .iter()
                .find(|candidate| candidate.entity_id == *id)
                .map(|constant| &constant.value),
            (Opcode::GlobalGet, Immediate::Entity(id)) => {
                global_initializer(globals, constants, *id).map(|(_, constant)| &constant.value)
            }
            _ => None,
        };
        if let Some(value) = named
            && sley_mutate::encode_const_value(value).is_err()
        {
            return fail(LowerErrorCode::ImmediateMismatch);
        }
    }
    Ok(())
}

fn named(definition: EntityId) -> TypeExpr {
    TypeExpr::Named(NamedType {
        definition,
        arguments: Vec::new(),
    })
}

fn named_definition(value: &TypeExpr) -> Option<EntityId> {
    match value {
        TypeExpr::Named(named) if named.arguments.is_empty() => Some(named.definition),
        _ => None,
    }
}

/// The fields of a non-generic record definition, or an immediate failure.
fn record_fields(
    types: &TypeEnvironment,
    definition: EntityId,
) -> Result<&[RecordField], LowerError> {
    match types.definition(definition) {
        Ok(found) if found.type_parameters.is_empty() => match &found.form {
            TypeDefForm::Record(fields) => Ok(fields),
            TypeDefForm::Variant(_) => fail(LowerErrorCode::ImmediateMismatch),
        },
        _ => fail(LowerErrorCode::ImmediateMismatch),
    }
}

/// The cases of a non-generic variant definition, or an immediate failure.
fn variant_cases(
    types: &TypeEnvironment,
    definition: EntityId,
) -> Result<&[VariantCase], LowerError> {
    match types.definition(definition) {
        Ok(found) if found.type_parameters.is_empty() => match &found.form {
            TypeDefForm::Variant(cases) => Ok(cases),
            TypeDefForm::Record(_) => fail(LowerErrorCode::ImmediateMismatch),
        },
        _ => fail(LowerErrorCode::ImmediateMismatch),
    }
}

fn map_result(key: &TypeExpr, value: &TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::OrderedMap {
            key: Box::new(key.clone()),
            value: Box::new(value.clone()),
        }),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    }
}

/// Map keys need the S20-210 total order and, like equality, no float.
fn map_key_admissible(types: &TypeEnvironment, key: &TypeExpr) -> bool {
    types.traits(key).is_ok_and(|traits| traits.total_order) && !contains_float(key)
}

fn ordered(value: &TypeExpr) -> bool {
    matches!(
        value,
        TypeExpr::Bool
            | TypeExpr::SInt(_)
            | TypeExpr::UInt(_)
            | TypeExpr::Bytes
            | TypeExpr::Text
            | TypeExpr::F32
            | TypeExpr::F64
    )
}

fn is_float(value: &TypeExpr) -> bool {
    matches!(value, TypeExpr::F32 | TypeExpr::F64)
}

/// Derives the exact result type of one extended-profile operation from its
/// opcode, immediate, and operand types, or fails with the frozen lowering
/// code (contract section 3).
///
/// # Errors
///
/// `VM_LOWER_OPCODE_UNSUPPORTED` for an opcode outside the landed slices,
/// `VM_LOWER_IMMEDIATE_MISMATCH` for a wrong or unresolved immediate, and
/// `VM_LOWER_SIGNATURE_MISMATCH` for operand or declared-result mismatches.
#[allow(clippy::too_many_lines)] // one arm per opcode of the contract table
pub fn judge_extended_operation(
    context: &LoweringContext<'_>,
    opcode: Opcode,
    immediate: &Immediate,
    operands: &[&TypeExpr],
    declared: &[TypeExpr],
) -> Result<TypeExpr, LowerError> {
    let types = context.types;
    let constants = context.constants;
    if !matches!(opcode, Opcode::CellGet | Opcode::CellSet)
        && operands.iter().any(|operand| contains_cell(operand))
    {
        return fail(LowerErrorCode::SignatureMismatch);
    }
    let immediate_none = |value: &Immediate| -> Result<(), LowerError> {
        if *value == Immediate::None {
            Ok(())
        } else {
            fail(LowerErrorCode::ImmediateMismatch)
        }
    };
    let single_declared = || -> Result<&TypeExpr, LowerError> {
        match declared {
            [value] => Ok(value),
            _ => fail(LowerErrorCode::SignatureMismatch),
        }
    };
    let derived = match opcode {
        Opcode::ConstantRef => {
            let Immediate::Entity(id) = immediate else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            if !operands.is_empty() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            constants
                .iter()
                .find(|constant| constant.entity_id == *id)
                .map(|constant| constant.value.value_type.clone())
                .ok_or_else(|| LowerError::new(LowerErrorCode::ImmediateMismatch))?
        }
        Opcode::TupleNew => {
            immediate_none(immediate)?;
            if operands.len() > MAX_TUPLE_ARITY {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            TypeExpr::Tuple(operands.iter().map(|value| (*value).clone()).collect())
        }
        Opcode::TupleGet => {
            let Immediate::Index(index) = immediate else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            let [TypeExpr::Tuple(items)] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            usize::try_from(*index)
                .ok()
                .and_then(|index| items.get(index))
                .cloned()
                .ok_or_else(|| LowerError::new(LowerErrorCode::ImmediateMismatch))?
        }
        Opcode::VectorNew => {
            immediate_none(immediate)?;
            match operands.split_first() {
                None => {
                    let declared = single_declared()?;
                    if !matches!(declared, TypeExpr::Vector(_)) {
                        return fail(LowerErrorCode::SignatureMismatch);
                    }
                    declared.clone()
                }
                Some((first, rest)) => {
                    if rest.iter().any(|value| value != first) {
                        return fail(LowerErrorCode::SignatureMismatch);
                    }
                    TypeExpr::Vector(Box::new((*first).clone()))
                }
            }
        }
        Opcode::VectorLen => {
            immediate_none(immediate)?;
            let [TypeExpr::Vector(_)] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            u64_type()
        }
        Opcode::VectorGet => {
            immediate_none(immediate)?;
            let [TypeExpr::Vector(element), index] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if **index != u64_type() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            TypeExpr::Option(element.clone())
        }
        Opcode::VectorSet => {
            immediate_none(immediate)?;
            let [TypeExpr::Vector(element), index, replacement] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if **index != u64_type() || *replacement != element.as_ref() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Vector(element.clone())),
                error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
            }
        }
        Opcode::IntAddChecked
        | Opcode::IntSubChecked
        | Opcode::IntMulChecked
        | Opcode::IntDivChecked
        | Opcode::IntRemChecked => {
            immediate_none(immediate)?;
            let [left, right] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if left != right || integer_width(left).is_none() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            arithmetic_result(left)
        }
        Opcode::IntNegChecked => {
            immediate_none(immediate)?;
            let [value] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if !matches!(integer_width(value), Some((true, _))) {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            arithmetic_result(value)
        }
        Opcode::IntShlChecked | Opcode::IntShrChecked => {
            immediate_none(immediate)?;
            let [value, amount] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if integer_width(value).is_none() || **amount != u32_type() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            arithmetic_result(value)
        }
        Opcode::FloatAdd | Opcode::FloatSub | Opcode::FloatMul | Opcode::FloatDiv => {
            immediate_none(immediate)?;
            let [left, right] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if left != right || !is_float(left) {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            (*left).clone()
        }
        Opcode::FloatNeg => {
            immediate_none(immediate)?;
            let [value] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if !is_float(value) {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            (*value).clone()
        }
        Opcode::FloatFma => {
            immediate_none(immediate)?;
            let [a, b, c] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if a != b || b != c || !is_float(a) {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            (*a).clone()
        }
        Opcode::RecordNew => {
            let Immediate::Entity(definition) = immediate else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            let fields = record_fields(types, *definition)?;
            if operands.len() != fields.len()
                || operands
                    .iter()
                    .zip(fields)
                    .any(|(operand, field)| **operand != field.value_type)
            {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            named(*definition)
        }
        Opcode::RecordGet => {
            let Immediate::Field(member) = immediate else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            let [value] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            let Some(definition) = named_definition(value) else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            record_fields(types, definition)?
                .iter()
                .find(|field| field.member_id == *member)
                .map(|field| field.value_type.clone())
                .ok_or_else(|| LowerError::new(LowerErrorCode::ImmediateMismatch))?
        }
        Opcode::VariantNew => {
            let Immediate::Variant(variant) = immediate else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            let case = variant_cases(types, variant.definition)?
                .iter()
                .find(|case| case.member_id == variant.member_id)
                .ok_or_else(|| LowerError::new(LowerErrorCode::ImmediateMismatch))?;
            match (&case.payload_type, operands) {
                (Some(payload), [operand]) if *operand == payload => {}
                (None, []) => {}
                _ => return fail(LowerErrorCode::SignatureMismatch),
            }
            named(variant.definition)
        }
        Opcode::VariantGet => {
            let Immediate::Variant(variant) = immediate else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            let [value] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if named_definition(value) != Some(variant.definition) {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            let case = variant_cases(types, variant.definition)?
                .iter()
                .find(|case| case.member_id == variant.member_id)
                .ok_or_else(|| LowerError::new(LowerErrorCode::ImmediateMismatch))?;
            let Some(payload) = &case.payload_type else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            TypeExpr::Option(Box::new(payload.clone()))
        }
        Opcode::MapNew => {
            immediate_none(immediate)?;
            if !operands.len().is_multiple_of(2) {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            if operands.is_empty() {
                let declared = single_declared()?;
                let TypeExpr::Result { ok, error } = declared else {
                    return fail(LowerErrorCode::SignatureMismatch);
                };
                let (
                    TypeExpr::OrderedMap { key, .. },
                    TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey),
                ) = (ok.as_ref(), error.as_ref())
                else {
                    return fail(LowerErrorCode::SignatureMismatch);
                };
                if !map_key_admissible(types, key) {
                    return fail(LowerErrorCode::SignatureMismatch);
                }
                declared.clone()
            } else {
                let key = operands[0];
                let value = operands[1];
                let aligned = operands
                    .iter()
                    .enumerate()
                    .all(|(index, operand)| *operand == if index % 2 == 0 { key } else { value });
                if !aligned || !map_key_admissible(types, key) {
                    return fail(LowerErrorCode::SignatureMismatch);
                }
                map_result(key, value)
            }
        }
        Opcode::MapGet | Opcode::MapContains | Opcode::MapRemove => {
            immediate_none(immediate)?;
            let [TypeExpr::OrderedMap { key, value }, probe] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if *probe != key.as_ref() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            match opcode {
                Opcode::MapGet => TypeExpr::Option(value.clone()),
                Opcode::MapContains => TypeExpr::Bool,
                _ => (*operands[0]).clone(),
            }
        }
        Opcode::MapInsert => {
            immediate_none(immediate)?;
            let [TypeExpr::OrderedMap { key, value }, probe, replacement] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if *probe != key.as_ref() || *replacement != value.as_ref() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            (*operands[0]).clone()
        }
        Opcode::CellNew => {
            immediate_none(immediate)?;
            let [value] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if !types.traits(value).is_ok_and(|traits| traits.persistable) {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            TypeExpr::LocalCell(Box::new((*value).clone()))
        }
        Opcode::CellGet => {
            immediate_none(immediate)?;
            let [TypeExpr::LocalCell(inner)] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            inner.as_ref().clone()
        }
        Opcode::CellSet => {
            immediate_none(immediate)?;
            let [TypeExpr::LocalCell(inner), value] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if *value != inner.as_ref() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            TypeExpr::Unit
        }
        Opcode::ValueHash => {
            immediate_none(immediate)?;
            let [value] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if types.require_hashable(value).is_err() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            TypeExpr::Bytes
        }
        Opcode::GlobalGet => {
            let Immediate::Entity(global) = immediate else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            if !operands.is_empty() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            let Some((definition, _)) = global_initializer(context.globals, constants, *global)
            else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            definition.value_type.clone()
        }
        Opcode::FunctionRef => {
            let Immediate::Function(reference) = immediate else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            if !reference.type_arguments.is_empty() {
                return fail(LowerErrorCode::ImmediateMismatch);
            }
            if !operands.is_empty() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            TypeExpr::FunctionRef(function_signature(context, reference.function)?)
        }
        Opcode::ContractAssert => {
            // Slice E7a. `CONTRACT_TEST_PROFILE_V1.md` section 2 accepts this
            // operation statically under epoch 1 and assigns predicate
            // execution to S20-270, so the profile re-derives every rule
            // rather than trusting the checker that already passed.
            let Immediate::Entity(contract) = immediate else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            let definition = context
                .contracts
                .iter()
                .find(|candidate| candidate.entity_id == *contract)
                .ok_or_else(|| LowerError::new(LowerErrorCode::ImmediateMismatch))?;
            if !matches!(
                definition.contract_kind,
                ContractKind::Precondition
                    | ContractKind::Postcondition
                    | ContractKind::ResultPredicate
            ) || definition.resource_limits.is_some()
            {
                return fail(LowerErrorCode::ImmediateMismatch);
            }
            if definition.target != context.function || definition.predicate == definition.target {
                return fail(LowerErrorCode::ImmediateMismatch);
            }
            let predicate = context
                .functions
                .iter()
                .find(|graph| graph.entity_id == definition.predicate)
                .ok_or_else(|| LowerError::new(LowerErrorCode::ImmediateMismatch))?;
            if !predicate.effects.is_empty() || !predicate.contracts.is_empty() {
                return fail(LowerErrorCode::ImmediateMismatch);
            }
            let signature = function_signature(context, definition.predicate)?;
            if *signature.result != TypeExpr::Bool
                || definition.bindings.len() != signature.parameters.len()
                || operands.len() != signature.parameters.len()
                || operands
                    .iter()
                    .zip(&signature.parameters)
                    .any(|(operand, parameter)| *operand != parameter)
            {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            contract_assert_result()
        }
        Opcode::CallDirect => {
            let Immediate::Function(reference) = immediate else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            if !reference.type_arguments.is_empty() {
                return fail(LowerErrorCode::ImmediateMismatch);
            }
            let signature = function_signature(context, reference.function)?;
            if operands.len() != signature.parameters.len()
                || operands
                    .iter()
                    .zip(&signature.parameters)
                    .any(|(operand, parameter)| *operand != parameter)
            {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            *signature.result
        }
        Opcode::Equal | Opcode::NotEqual => {
            immediate_none(immediate)?;
            let [left, right] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if left != right
                || types.require_hashable(left).is_err()
                || (contains_float(left) && !is_float(left))
            {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            TypeExpr::Bool
        }
        Opcode::LessThan | Opcode::LessEqual | Opcode::GreaterThan | Opcode::GreaterEqual => {
            immediate_none(immediate)?;
            let [left, right] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if left != right || !ordered(left) {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            TypeExpr::Bool
        }
        Opcode::BoolNot => {
            immediate_none(immediate)?;
            let [TypeExpr::Bool] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            TypeExpr::Bool
        }
        Opcode::BoolAnd | Opcode::BoolOr => {
            immediate_none(immediate)?;
            let [TypeExpr::Bool, TypeExpr::Bool] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            TypeExpr::Bool
        }
        Opcode::OptionSome => {
            immediate_none(immediate)?;
            let [payload] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            TypeExpr::Option(Box::new((*payload).clone()))
        }
        Opcode::OptionNone => {
            immediate_none(immediate)?;
            if !operands.is_empty() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            let declared = single_declared()?;
            if !matches!(declared, TypeExpr::Option(_)) {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            declared.clone()
        }
        Opcode::ResultOk | Opcode::ResultErr => {
            immediate_none(immediate)?;
            let [payload] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            let declared = single_declared()?;
            let TypeExpr::Result { ok, error } = declared else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            let expected = if opcode == Opcode::ResultOk {
                ok
            } else {
                error
            };
            if *payload != expected.as_ref() {
                return fail(LowerErrorCode::SignatureMismatch);
            }
            declared.clone()
        }
        Opcode::AdapterInvoke => {
            // Slice E8. Only the three frozen bridge entries land, resolved
            // as genuine `AdapterImport` values from the supplied import
            // inventory: the immediate must name a carried import, and every
            // frozen field of that row must equal the frozen bridge values.
            // Every other adapter stays VM_LOWER_OPCODE_UNSUPPORTED however
            // well formed its operands are (contract section E8).
            let Immediate::Entity(entry) = immediate else {
                return fail(LowerErrorCode::ImmediateMismatch);
            };
            let Some((entry, carried)) = resolve_bridge_entry(context.adapters, entry) else {
                return fail(LowerErrorCode::OpcodeUnsupported);
            };
            let [scope, request] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            let response_type = match entry {
                BridgeKind::VectorPush => {
                    // Push-row binding (Ariadne HIGH-2 repair, matching
                    // S20-230 §1.5): per-use operand types must equal the
                    // carried row — scope is the row response, request is
                    // the row request — and the row already satisfies
                    // response == Vector<request> via resolve. Generic over
                    // `E` by monomorphization only, byte-unaware by
                    // construction.
                    if **scope != carried.response_type || **request != carried.request_type {
                        return fail(LowerErrorCode::SignatureMismatch);
                    }
                    carried.response_type.clone()
                }
                BridgeKind::BytesToVector => {
                    if **scope != TypeExpr::Unit || **request != TypeExpr::Bytes {
                        return fail(LowerErrorCode::SignatureMismatch);
                    }
                    TypeExpr::Vector(Box::new(u8_type()))
                }
                BridgeKind::VectorToBytes => {
                    if **scope != TypeExpr::Unit || **request != u8vec_type() {
                        return fail(LowerErrorCode::SignatureMismatch);
                    }
                    TypeExpr::Bytes
                }
            };
            TypeExpr::Result {
                ok: Box::new(response_type),
                error: Box::new(bridge_index_error()),
            }
        }
        _ => return fail(LowerErrorCode::OpcodeUnsupported),
    };
    if declared != std::slice::from_ref(&derived) {
        return fail(LowerErrorCode::SignatureMismatch);
    }
    Ok(derived)
}

/// Runtime fault: state impossible after successful lowering.
#[derive(Clone, Copy, Debug)]
pub struct ExtendedFault;

/// The canonical quiet NaN bit patterns (contract E3).
const CANONICAL_NAN_F32: u32 = 0x7fc0_0000;
const CANONICAL_NAN_F64: u64 = 0x7ff8_0000_0000_0000;

/// S20-210 canonical floats admit one quiet NaN and no negative zero, so a
/// NaN result becomes the canonical NaN and a negative-zero result becomes
/// positive zero; every other bit pattern is stored exactly.
fn canonical_f32(value: f32) -> u32 {
    if value.is_nan() {
        CANONICAL_NAN_F32
    } else if value == 0.0 {
        0
    } else {
        value.to_bits()
    }
}

fn canonical_f64(value: f64) -> u64 {
    if value.is_nan() {
        CANONICAL_NAN_F64
    } else if value == 0.0 {
        0
    } else {
        value.to_bits()
    }
}

/// One float family operation over operands of one width, IEEE-754
/// round-to-nearest-ties-to-even with canonicalized NaN results.
fn float_operation(
    opcode: Opcode,
    operands: &[ConstValue],
    result_type: &TypeExpr,
) -> Result<ConstValue, ExtendedFault> {
    let data = match result_type {
        TypeExpr::F32 => {
            let values: Vec<f32> = operands
                .iter()
                .map(|value| match value.data {
                    ConstData::F32Bits(bits) => Ok(f32::from_bits(bits)),
                    _ => Err(ExtendedFault),
                })
                .collect::<Result<_, _>>()?;
            let result = match (opcode, values.as_slice()) {
                (Opcode::FloatAdd, [a, b]) => a + b,
                (Opcode::FloatSub, [a, b]) => a - b,
                (Opcode::FloatMul, [a, b]) => a * b,
                (Opcode::FloatDiv, [a, b]) => a / b,
                (Opcode::FloatNeg, [a]) => -a,
                (Opcode::FloatFma, [a, b, c]) => a.mul_add(*b, *c),
                _ => return Err(ExtendedFault),
            };
            ConstData::F32Bits(canonical_f32(result))
        }
        TypeExpr::F64 => {
            let values: Vec<f64> = operands
                .iter()
                .map(|value| match value.data {
                    ConstData::F64Bits(bits) => Ok(f64::from_bits(bits)),
                    _ => Err(ExtendedFault),
                })
                .collect::<Result<_, _>>()?;
            let result = match (opcode, values.as_slice()) {
                (Opcode::FloatAdd, [a, b]) => a + b,
                (Opcode::FloatSub, [a, b]) => a - b,
                (Opcode::FloatMul, [a, b]) => a * b,
                (Opcode::FloatDiv, [a, b]) => a / b,
                (Opcode::FloatNeg, [a]) => -a,
                (Opcode::FloatFma, [a, b, c]) => a.mul_add(*b, *c),
                _ => return Err(ExtendedFault),
            };
            ConstData::F64Bits(canonical_f64(result))
        }
        _ => return Err(ExtendedFault),
    };
    Ok(ConstValue {
        value_type: result_type.clone(),
        data,
    })
}

/// IEEE comparison of two floats of one width: `None` when unordered.
fn float_partial_order(
    left: &ConstData,
    right: &ConstData,
) -> Result<Option<core::cmp::Ordering>, ExtendedFault> {
    match (left, right) {
        (ConstData::F32Bits(a), ConstData::F32Bits(b)) => {
            Ok(f32::from_bits(*a).partial_cmp(&f32::from_bits(*b)))
        }
        (ConstData::F64Bits(a), ConstData::F64Bits(b)) => {
            Ok(f64::from_bits(*a).partial_cmp(&f64::from_bits(*b)))
        }
        _ => Err(ExtendedFault),
    }
}

fn is_float_data(value: &ConstData) -> bool {
    matches!(value, ConstData::F32Bits(_) | ConstData::F64Bits(_))
}

fn compare_data(left: &ConstData, right: &ConstData) -> Result<core::cmp::Ordering, ExtendedFault> {
    Ok(match (left, right) {
        (ConstData::Bool(a), ConstData::Bool(b)) => a.cmp(b),
        (ConstData::SInt(a), ConstData::SInt(b)) => a.cmp(b),
        (ConstData::UInt(a), ConstData::UInt(b)) => a.cmp(b),
        (ConstData::Bytes(a), ConstData::Bytes(b)) => a.as_slice().cmp(b.as_slice()),
        (ConstData::Text(a), ConstData::Text(b)) => a.as_bytes().cmp(b.as_bytes()),
        _ => return Err(ExtendedFault),
    })
}

/// Arithmetic failure codes (S20-210 closed set).
const ARITHMETIC_OVERFLOW: u16 = 1;
const ARITHMETIC_DIVIDE_BY_ZERO: u16 = 2;
const ARITHMETIC_INVALID_SHIFT: u16 = 3;

/// One checked-integer outcome: the exact value or the arithmetic code.
#[derive(Clone, Copy)]
enum Checked {
    Value(i128, u128),
    Failure(u16),
}

fn signed_bounds(bits: u16) -> (i128, i128) {
    if bits >= 128 {
        (i128::MIN, i128::MAX)
    } else {
        let half = 1_i128 << (bits - 1);
        (-half, half - 1)
    }
}

fn unsigned_max(bits: u16) -> u128 {
    if bits >= 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    }
}

fn fits(signed: bool, bits: u16, signed_value: i128, unsigned_value: u128) -> bool {
    if signed {
        let (low, high) = signed_bounds(bits);
        (low..=high).contains(&signed_value)
    } else {
        unsigned_value <= unsigned_max(bits)
    }
}

#[allow(clippy::too_many_lines)] // one arm per checked operation of the contract table
fn checked_integer(
    opcode: Opcode,
    signed: bool,
    bits: u16,
    operands: &[ConstValue],
) -> Result<Checked, ExtendedFault> {
    let read = |value: &ConstValue| -> Result<(i128, u128), ExtendedFault> {
        match value.data {
            ConstData::SInt(value) if signed => Ok((value, 0)),
            ConstData::UInt(value) if !signed => Ok((0, value)),
            _ => Err(ExtendedFault),
        }
    };
    let overflow = Ok(Checked::Failure(ARITHMETIC_OVERFLOW));
    let ranged = |signed_value: Option<i128>, unsigned_value: Option<u128>| -> Checked {
        match (signed, signed_value, unsigned_value) {
            (true, Some(value), _) if fits(true, bits, value, 0) => Checked::Value(value, 0),
            (false, _, Some(value)) if fits(false, bits, 0, value) => Checked::Value(0, value),
            _ => Checked::Failure(ARITHMETIC_OVERFLOW),
        }
    };
    match opcode {
        Opcode::IntAddChecked | Opcode::IntSubChecked | Opcode::IntMulChecked => {
            let [left, right] = operands else {
                return Err(ExtendedFault);
            };
            let (ls, lu) = read(left)?;
            let (rs, ru) = read(right)?;
            let (signed_value, unsigned_value) = match opcode {
                Opcode::IntAddChecked => (ls.checked_add(rs), lu.checked_add(ru)),
                Opcode::IntSubChecked => (ls.checked_sub(rs), lu.checked_sub(ru)),
                _ => (ls.checked_mul(rs), lu.checked_mul(ru)),
            };
            Ok(ranged(signed_value, unsigned_value))
        }
        Opcode::IntDivChecked | Opcode::IntRemChecked => {
            let [left, right] = operands else {
                return Err(ExtendedFault);
            };
            let (ls, lu) = read(left)?;
            let (rs, ru) = read(right)?;
            if (signed && rs == 0) || (!signed && ru == 0) {
                return Ok(Checked::Failure(ARITHMETIC_DIVIDE_BY_ZERO));
            }
            if signed && rs == -1 && ls == signed_bounds(bits).0 {
                return overflow;
            }
            if signed {
                let value = if opcode == Opcode::IntDivChecked {
                    ls / rs
                } else {
                    ls % rs
                };
                Ok(ranged(Some(value), None))
            } else {
                let value = if opcode == Opcode::IntDivChecked {
                    lu / ru
                } else {
                    lu % ru
                };
                Ok(ranged(None, Some(value)))
            }
        }
        Opcode::IntNegChecked => {
            let [value] = operands else {
                return Err(ExtendedFault);
            };
            let (signed_value, _) = read(value)?;
            if signed_value == signed_bounds(bits).0 {
                return overflow;
            }
            Ok(ranged(Some(-signed_value), None))
        }
        Opcode::IntShlChecked | Opcode::IntShrChecked => {
            let [value, amount] = operands else {
                return Err(ExtendedFault);
            };
            let (signed_value, unsigned_value) = read(value)?;
            let ConstData::UInt(amount) = amount.data else {
                return Err(ExtendedFault);
            };
            let Some(amount) = u32::try_from(amount)
                .ok()
                .filter(|amount| *amount < u32::from(bits))
            else {
                return Ok(Checked::Failure(ARITHMETIC_INVALID_SHIFT));
            };
            if opcode == Opcode::IntShrChecked {
                return Ok(if signed {
                    Checked::Value(signed_value >> amount, 0)
                } else {
                    Checked::Value(0, unsigned_value >> amount)
                });
            }
            // Left shift: bits shifted out must be the sign fill (signed) or
            // zero (unsigned); the result stays within the width.
            //
            // The test compares the operand against the width bound shifted
            // down, never a multiplier shifted up. `1 << amount` is itself
            // out of range at the top of the widest width (`1_i128 << 127` is
            // `i128::MIN`), which inverted both directions there: `1 shl 127`
            // answered `Value(MIN)` where the rule requires overflow, and
            // `-1 shl 127` answered overflow where the rule requires
            // `Value(MIN)`. Ariadne's S20-260 contract review, P0-2.
            if signed {
                let (low, high) = signed_bounds(bits);
                let representable = match signed_value.cmp(&0) {
                    core::cmp::Ordering::Equal => true,
                    core::cmp::Ordering::Greater => signed_value <= (high >> amount),
                    core::cmp::Ordering::Less => signed_value >= (low >> amount),
                };
                Ok(if representable {
                    Checked::Value(signed_value << amount, 0)
                } else {
                    Checked::Failure(ARITHMETIC_OVERFLOW)
                })
            } else {
                Ok(if unsigned_value <= (unsigned_max(bits) >> amount) {
                    Checked::Value(0, unsigned_value << amount)
                } else {
                    Checked::Failure(ARITHMETIC_OVERFLOW)
                })
            }
        }
        _ => Err(ExtendedFault),
    }
}

fn arithmetic_value(
    outcome: Checked,
    signed: bool,
    result_type: &TypeExpr,
) -> Result<ConstValue, ExtendedFault> {
    let TypeExpr::Result { ok, error } = result_type else {
        return Err(ExtendedFault);
    };
    let data = match outcome {
        Checked::Value(signed_value, unsigned_value) => {
            ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
                value_type: ok.as_ref().clone(),
                data: if signed {
                    ConstData::SInt(signed_value)
                } else {
                    ConstData::UInt(unsigned_value)
                },
            })))
        }
        Checked::Failure(code) => ConstData::Result(ResultConst::Err(Box::new(ConstValue {
            value_type: error.as_ref().clone(),
            data: ConstData::BuiltinFailure(BuiltinFailureValue {
                kind: BuiltinFailureKind::Arithmetic,
                code,
            }),
        }))),
    };
    Ok(ConstValue {
        value_type: result_type.clone(),
        data,
    })
}

fn key_bytes(value: &ConstValue) -> Result<Vec<u8>, ExtendedFault> {
    sley_mutate::encode_const_value(value).map_err(|_| ExtendedFault)
}

/// Sorts map entries by their keys' S20-350 canonical bytes (contract E4).
fn sorted_map(entries: Vec<MapEntryConst>) -> Result<Vec<MapEntryConst>, ExtendedFault> {
    let mut keyed = entries
        .into_iter()
        .map(|entry| key_bytes(&entry.key).map(|bytes| (bytes, entry)))
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(keyed.into_iter().map(|(_, entry)| entry).collect())
}

/// The canonical key bytes alongside each entry. Key identity is the
/// encoding, not structural equality: dedup and every probe compare these
/// bytes, so the map that `map_new` accepts is exactly the map the codec
/// would accept, and two keys that encode alike are one key (contract E4).
fn keyed_entries(
    entries: &[MapEntryConst],
) -> Result<Vec<(Vec<u8>, &MapEntryConst)>, ExtendedFault> {
    entries
        .iter()
        .map(|entry| key_bytes(&entry.key).map(|bytes| (bytes, entry)))
        .collect()
}

fn map_entries(value: &ConstValue) -> Result<&[MapEntryConst], ExtendedFault> {
    match &value.data {
        ConstData::Map(entries) => Ok(entries),
        _ => Err(ExtendedFault),
    }
}

fn map_value(
    result_type: &TypeExpr,
    entries: Vec<MapEntryConst>,
) -> Result<ConstValue, ExtendedFault> {
    Ok(ConstValue {
        value_type: result_type.clone(),
        data: ConstData::Map(sorted_map(entries)?),
    })
}

fn bool_value(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

/// Executes one extended-profile instruction over already-read operand
/// values and returns the result value typed as the result register
/// (contract section 3, slice E1).
///
/// # Errors
///
/// `ExtendedFault` for any operand form the successful judgment excludes.
#[allow(clippy::too_many_lines)] // one arm per opcode of the contract table
pub fn execute_extended_instruction(
    context: &mut ExecutionContext<'_>,
    opcode: Opcode,
    immediate: &Immediate,
    operands: &[ConstValue],
    result_type: &TypeExpr,
) -> Result<ConstValue, ExtendedFault> {
    let environment = context.types;
    let constants = context.constants;
    let typed = |data: ConstData| ConstValue {
        value_type: result_type.clone(),
        data,
    };
    Ok(match (opcode, operands) {
        (Opcode::ConstantRef, []) => {
            let Immediate::Entity(id) = immediate else {
                return Err(ExtendedFault);
            };
            constants
                .iter()
                .find(|constant| constant.entity_id == *id)
                .map(|constant| constant.value.clone())
                .ok_or(ExtendedFault)?
        }
        (Opcode::RecordNew, values) => {
            let Immediate::Entity(definition) = immediate else {
                return Err(ExtendedFault);
            };
            let fields = record_fields(environment, *definition).map_err(|_| ExtendedFault)?;
            if fields.len() != values.len() {
                return Err(ExtendedFault);
            }
            typed(ConstData::Record(RecordConst {
                definition: *definition,
                fields: fields
                    .iter()
                    .zip(values)
                    .map(|(field, value)| FieldConst {
                        member_id: field.member_id,
                        value: value.clone(),
                    })
                    .collect(),
            }))
        }
        (Opcode::RecordGet, [record]) => {
            let (Immediate::Field(member), ConstData::Record(record)) = (immediate, &record.data)
            else {
                return Err(ExtendedFault);
            };
            record
                .fields
                .iter()
                .find(|field| field.member_id == *member)
                .map(|field| field.value.clone())
                .ok_or(ExtendedFault)?
        }
        (Opcode::VariantNew, values) => {
            let Immediate::Variant(variant) = immediate else {
                return Err(ExtendedFault);
            };
            let payload = match values {
                [] => None,
                [value] => Some(Box::new(value.clone())),
                _ => return Err(ExtendedFault),
            };
            typed(ConstData::Variant(VariantConst {
                definition: variant.definition,
                member_id: variant.member_id,
                payload,
            }))
        }
        (Opcode::VariantGet, [value]) => {
            let (Immediate::Variant(variant), ConstData::Variant(found)) = (immediate, &value.data)
            else {
                return Err(ExtendedFault);
            };
            let payload = if found.member_id == variant.member_id {
                found.payload.clone()
            } else {
                None
            };
            typed(ConstData::Option(payload))
        }
        (Opcode::MapNew, values) => {
            let TypeExpr::Result { ok, error } = result_type else {
                return Err(ExtendedFault);
            };
            let mut entries: Vec<MapEntryConst> = Vec::with_capacity(values.len() / 2);
            let mut seen: Vec<Vec<u8>> = Vec::with_capacity(values.len() / 2);
            for pair in values.chunks(2) {
                let [key, value] = pair else {
                    return Err(ExtendedFault);
                };
                let bytes = key_bytes(key)?;
                if seen.contains(&bytes) {
                    return Ok(typed(ConstData::Result(ResultConst::Err(Box::new(
                        ConstValue {
                            value_type: error.as_ref().clone(),
                            data: ConstData::BuiltinFailure(BuiltinFailureValue {
                                kind: BuiltinFailureKind::DuplicateKey,
                                code: 1,
                            }),
                        },
                    )))));
                }
                entries.push(MapEntryConst {
                    key: key.clone(),
                    value: value.clone(),
                });
                seen.push(bytes);
            }
            typed(ConstData::Result(ResultConst::Ok(Box::new(map_value(
                ok, entries,
            )?))))
        }
        (Opcode::MapGet, [map, key]) => {
            let query = key_bytes(key)?;
            let found = keyed_entries(map_entries(map)?)?
                .into_iter()
                .find(|(bytes, _)| *bytes == query)
                .map(|(_, entry)| Box::new(entry.value.clone()));
            typed(ConstData::Option(found))
        }
        (Opcode::MapContains, [map, key]) => {
            let query = key_bytes(key)?;
            bool_value(
                keyed_entries(map_entries(map)?)?
                    .iter()
                    .any(|(bytes, _)| *bytes == query),
            )
        }
        (Opcode::MapInsert, [map, key, value]) => {
            let query = key_bytes(key)?;
            let keyed = keyed_entries(map_entries(map)?)?;
            let mut entries: Vec<MapEntryConst> = keyed
                .into_iter()
                .filter(|(bytes, _)| *bytes != query)
                .map(|(_, entry)| entry.clone())
                .collect();
            entries.push(MapEntryConst {
                key: key.clone(),
                value: value.clone(),
            });
            map_value(result_type, entries)?
        }
        (Opcode::MapRemove, [map, key]) => {
            let query = key_bytes(key)?;
            let entries: Vec<MapEntryConst> = keyed_entries(map_entries(map)?)?
                .into_iter()
                .filter(|(bytes, _)| *bytes != query)
                .map(|(_, entry)| entry.clone())
                .collect();
            map_value(result_type, entries)?
        }
        (Opcode::CellNew, [value]) => {
            let index = u128::try_from(context.cells.len()).map_err(|_| ExtendedFault)?;
            context.cells.push(value.clone());
            typed(ConstData::UInt(index))
        }
        (Opcode::CellGet, [cell]) => {
            let ConstData::UInt(index) = cell.data else {
                return Err(ExtendedFault);
            };
            let index = usize::try_from(index).map_err(|_| ExtendedFault)?;
            context.cells.get(index).cloned().ok_or(ExtendedFault)?
        }
        (Opcode::CellSet, [cell, value]) => {
            let ConstData::UInt(index) = cell.data else {
                return Err(ExtendedFault);
            };
            let index = usize::try_from(index).map_err(|_| ExtendedFault)?;
            let slot = context.cells.get_mut(index).ok_or(ExtendedFault)?;
            *slot = value.clone();
            typed(ConstData::Unit)
        }
        (Opcode::ValueHash, [value]) => {
            let hash =
                hash_validated_value(context.schema_epoch, value).map_err(|_| ExtendedFault)?;
            typed(ConstData::Bytes(hash.as_bytes().to_vec()))
        }
        (Opcode::GlobalGet, []) => {
            let Immediate::Entity(global) = immediate else {
                return Err(ExtendedFault);
            };
            let (_, constant) =
                global_initializer(context.globals, constants, *global).ok_or(ExtendedFault)?;
            constant.value.clone()
        }
        (Opcode::FunctionRef, []) => {
            let Immediate::Function(reference) = immediate else {
                return Err(ExtendedFault);
            };
            typed(ConstData::FunctionRef(reference.clone()))
        }
        (Opcode::TupleNew | Opcode::VectorNew, items) => typed(ConstData::Sequence(items.to_vec())),
        (Opcode::TupleGet, [tuple]) => {
            let (Immediate::Index(index), ConstData::Sequence(items)) = (immediate, &tuple.data)
            else {
                return Err(ExtendedFault);
            };
            usize::try_from(*index)
                .ok()
                .and_then(|index| items.get(index))
                .cloned()
                .ok_or(ExtendedFault)?
        }
        (Opcode::VectorLen, [vector]) => {
            let ConstData::Sequence(items) = &vector.data else {
                return Err(ExtendedFault);
            };
            typed(ConstData::UInt(
                u128::try_from(items.len()).map_err(|_| ExtendedFault)?,
            ))
        }
        (Opcode::VectorGet, [vector, index]) => {
            let (ConstData::Sequence(items), ConstData::UInt(index)) = (&vector.data, &index.data)
            else {
                return Err(ExtendedFault);
            };
            let element = usize::try_from(*index)
                .ok()
                .and_then(|index| items.get(index));
            typed(ConstData::Option(
                element.map(|value| Box::new(value.clone())),
            ))
        }
        (Opcode::VectorSet, [vector, index, replacement]) => {
            let (ConstData::Sequence(items), ConstData::UInt(index)) = (&vector.data, &index.data)
            else {
                return Err(ExtendedFault);
            };
            let TypeExpr::Result { ok, error } = result_type else {
                return Err(ExtendedFault);
            };
            match usize::try_from(*index)
                .ok()
                .filter(|index| *index < items.len())
            {
                Some(position) => {
                    let mut updated = items.clone();
                    updated[position] = replacement.clone();
                    typed(ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
                        value_type: ok.as_ref().clone(),
                        data: ConstData::Sequence(updated),
                    }))))
                }
                None => typed(ConstData::Result(ResultConst::Err(Box::new(ConstValue {
                    value_type: error.as_ref().clone(),
                    data: ConstData::BuiltinFailure(BuiltinFailureValue {
                        kind: BuiltinFailureKind::Index,
                        code: 1,
                    }),
                })))),
            }
        }
        (
            Opcode::IntAddChecked
            | Opcode::IntSubChecked
            | Opcode::IntMulChecked
            | Opcode::IntDivChecked
            | Opcode::IntRemChecked
            | Opcode::IntNegChecked
            | Opcode::IntShlChecked
            | Opcode::IntShrChecked,
            values,
        ) => {
            let (signed, bits) = integer_width(&values.first().ok_or(ExtendedFault)?.value_type)
                .ok_or(ExtendedFault)?;
            arithmetic_value(
                checked_integer(opcode, signed, bits, values)?,
                signed,
                result_type,
            )?
        }
        (
            Opcode::FloatAdd
            | Opcode::FloatSub
            | Opcode::FloatMul
            | Opcode::FloatDiv
            | Opcode::FloatNeg
            | Opcode::FloatFma,
            values,
        ) => float_operation(opcode, values, result_type)?,
        (
            Opcode::Equal
            | Opcode::NotEqual
            | Opcode::LessThan
            | Opcode::LessEqual
            | Opcode::GreaterThan
            | Opcode::GreaterEqual,
            [left, right],
        ) if is_float_data(&left.data) => {
            let order = float_partial_order(&left.data, &right.data)?;
            bool_value(match opcode {
                Opcode::Equal => order == Some(core::cmp::Ordering::Equal),
                Opcode::NotEqual => order != Some(core::cmp::Ordering::Equal),
                Opcode::LessThan => order.is_some_and(core::cmp::Ordering::is_lt),
                Opcode::LessEqual => order.is_some_and(core::cmp::Ordering::is_le),
                Opcode::GreaterThan => order.is_some_and(core::cmp::Ordering::is_gt),
                _ => order.is_some_and(core::cmp::Ordering::is_ge),
            })
        }
        (Opcode::Equal, [left, right]) => bool_value(left == right),
        (Opcode::NotEqual, [left, right]) => bool_value(left != right),
        (Opcode::LessThan, [left, right]) => {
            bool_value(compare_data(&left.data, &right.data)?.is_lt())
        }
        (Opcode::LessEqual, [left, right]) => {
            bool_value(compare_data(&left.data, &right.data)?.is_le())
        }
        (Opcode::GreaterThan, [left, right]) => {
            bool_value(compare_data(&left.data, &right.data)?.is_gt())
        }
        (Opcode::GreaterEqual, [left, right]) => {
            bool_value(compare_data(&left.data, &right.data)?.is_ge())
        }
        (Opcode::BoolNot, [value]) => {
            let ConstData::Bool(value) = value.data else {
                return Err(ExtendedFault);
            };
            bool_value(!value)
        }
        (Opcode::BoolAnd | Opcode::BoolOr, [left, right]) => {
            let (ConstData::Bool(left), ConstData::Bool(right)) = (&left.data, &right.data) else {
                return Err(ExtendedFault);
            };
            bool_value(if opcode == Opcode::BoolAnd {
                *left && *right
            } else {
                *left || *right
            })
        }
        (Opcode::OptionSome, [payload]) => {
            typed(ConstData::Option(Some(Box::new(payload.clone()))))
        }
        (Opcode::OptionNone, []) => typed(ConstData::Option(None)),
        (Opcode::ResultOk, [payload]) => typed(ConstData::Result(ResultConst::Ok(Box::new(
            payload.clone(),
        )))),
        (Opcode::ResultErr, [payload]) => typed(ConstData::Result(ResultConst::Err(Box::new(
            payload.clone(),
        )))),
        (Opcode::AdapterInvoke, operands) => {
            // Slice E8. Judgment admits only the three frozen bridge
            // entries resolved from the supplied inventory, so anything
            // else here is an internal fault. Execution re-resolves from
            // the same inventory rather than trusting the judgment — the
            // public helper cannot bypass registration: without the exact
            // frozen row (identity, ABI, schemas, relationship, purity)
            // this faults.
            let Immediate::Entity(entry) = immediate else {
                return Err(ExtendedFault);
            };
            let Some((entry, carried)) = resolve_bridge_entry(context.adapters, entry) else {
                return Err(ExtendedFault);
            };
            bridge_execute(entry, carried, operands, result_type)?
        }
        _ => return Err(ExtendedFault),
    })
}

/// The entity an immediate names, when it names one (constants and globals).
#[must_use]
pub fn immediate_entity(immediate: &Immediate) -> Option<EntityId> {
    match immediate {
        Immediate::Entity(id) => Some(*id),
        _ => None,
    }
}
