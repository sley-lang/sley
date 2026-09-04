//! Extended opcode profile (`docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md`,
//! ADR-0039): the signature judgment and the runtime semantics of the
//! opcode families beyond the restricted Boolean profile. Slice E1 lands
//! the data family (constants, tuples, vectors, comparisons, options,
//! results); every other opcode fails closed until its slice lands.

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId};
use sley_ssmc::{
    BuiltinFailureKind, BuiltinFailureValue, ConstData, ConstValue, ConstantDefinition,
    ContractDefinition, ContractKind, FieldConst, FunctionGraph, FunctionType,
    GlobalValueDefinition, Immediate, IntegerWidth, MapEntryConst, NamedType, Opcode, Parameter,
    RecordConst, RecordField, ResultConst, TypeDefForm, TypeExpr, VariantCase, VariantConst,
    fingerprint::hash_validated_value,
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
            if signed {
                let shifted = signed_value.checked_mul(1_i128 << amount);
                Ok(match shifted {
                    Some(result) if fits(true, bits, result, 0) => Checked::Value(result, 0),
                    _ => Checked::Failure(ARITHMETIC_OVERFLOW),
                })
            } else {
                let shifted = unsigned_value.checked_mul(1_u128 << amount);
                Ok(match shifted {
                    Some(result) if fits(false, bits, 0, result) => Checked::Value(0, result),
                    _ => Checked::Failure(ARITHMETIC_OVERFLOW),
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
            for pair in values.chunks(2) {
                let [key, value] = pair else {
                    return Err(ExtendedFault);
                };
                if entries.iter().any(|entry| entry.key == *key) {
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
            }
            typed(ConstData::Result(ResultConst::Ok(Box::new(map_value(
                ok, entries,
            )?))))
        }
        (Opcode::MapGet, [map, key]) => {
            let found = map_entries(map)?
                .iter()
                .find(|entry| entry.key == *key)
                .map(|entry| Box::new(entry.value.clone()));
            typed(ConstData::Option(found))
        }
        (Opcode::MapContains, [map, key]) => {
            bool_value(map_entries(map)?.iter().any(|entry| entry.key == *key))
        }
        (Opcode::MapInsert, [map, key, value]) => {
            let mut entries: Vec<MapEntryConst> = map_entries(map)?
                .iter()
                .filter(|entry| entry.key != *key)
                .cloned()
                .collect();
            entries.push(MapEntryConst {
                key: key.clone(),
                value: value.clone(),
            });
            map_value(result_type, entries)?
        }
        (Opcode::MapRemove, [map, key]) => {
            let entries: Vec<MapEntryConst> = map_entries(map)?
                .iter()
                .filter(|entry| entry.key != *key)
                .cloned()
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
