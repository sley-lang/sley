//! Extended opcode profile (`docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md`,
//! ADR-0039): the signature judgment and the runtime semantics of the
//! opcode families beyond the restricted Boolean profile. Slice E1 lands
//! the data family (constants, tuples, vectors, comparisons, options,
//! results); every other opcode fails closed until its slice lands.

use sley_check::TypeEnvironment;
use sley_id::EntityId;
use sley_ssmc::{
    BuiltinFailureKind, BuiltinFailureValue, ConstData, ConstValue, ConstantDefinition, Immediate,
    IntegerWidth, Opcode, ResultConst, TypeExpr,
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

fn ordered(value: &TypeExpr) -> bool {
    matches!(
        value,
        TypeExpr::Bool | TypeExpr::SInt(_) | TypeExpr::UInt(_) | TypeExpr::Bytes | TypeExpr::Text
    )
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
    types: &TypeEnvironment,
    constants: &[ConstantDefinition],
    opcode: Opcode,
    immediate: &Immediate,
    operands: &[&TypeExpr],
    declared: &[TypeExpr],
) -> Result<TypeExpr, LowerError> {
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
        Opcode::Equal | Opcode::NotEqual => {
            immediate_none(immediate)?;
            let [left, right] = operands else {
                return fail(LowerErrorCode::SignatureMismatch);
            };
            if left != right || types.require_hashable(left).is_err() || contains_float(left) {
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
    opcode: Opcode,
    immediate: &Immediate,
    operands: &[ConstValue],
    result_type: &TypeExpr,
    constants: &[ConstantDefinition],
) -> Result<ConstValue, ExtendedFault> {
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
