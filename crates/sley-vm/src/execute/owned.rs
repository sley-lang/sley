//! Ownership-aware execution of the hot extended opcodes.
//!
//! These arms produce exactly the values the borrowed reference arms in
//! `extended.rs` produce, and charge exactly the value units a full walk of
//! the result would compute; they differ only in host mechanics. An operand
//! whose register is dead after the instruction is moved out of the
//! register file, so persistent updates (`vector_set`, `map_insert`,
//! `map_remove`) mutate in place when nothing else shares the value, and
//! constructors and projections avoid deep copies. Result units are derived
//! arithmetically from the operands' cached units instead of re-walking
//! large values. Any arm that cannot prove its arithmetic falls back to the
//! full walk.

use std::collections::HashMap;

use sley_id::EntityId;
use sley_ssmc::{
    BuiltinFailureKind, BuiltinFailureValue, ConstData, ConstValue, FieldConst, Immediate,
    MapEntryConst, Opcode, RecordConst, ResultConst, TypeExpr, VariantConst,
};

use super::{
    ExecutionSource, Runtime, RuntimeFault, RuntimeResult, RuntimeValue, read_register,
    take_register, value_units_const, value_units_type,
};

/// Constant inventory index: each constant shared once per execution with
/// its cached value units, so `constant_ref` hands out a shared root
/// instead of a deep copy.
pub(super) type ConstantIndex = HashMap<EntityId, RuntimeValue>;

/// Builds the constant index for one execution.
pub(super) fn constant_index(source: &ExecutionSource<'_>) -> ConstantIndex {
    let mut index = HashMap::with_capacity(source.constants.len());
    for constant in source.constants {
        // The first definition wins, as the reference linear search does.
        index
            .entry(constant.entity_id)
            .or_insert_with(|| RuntimeValue::new(constant.value.clone()));
    }
    index
}

/// Whether a register's value is the only owner of its root: moving out of
/// a shared root would copy the whole root, so projections share instead.
fn unique(runtime: &Runtime, register: u32) -> bool {
    read_register(runtime, register)
        .is_ok_and(|value| std::sync::Arc::strong_count(&value.root) == 1)
}

fn operand(runtime: &mut Runtime, register: u32, dead: bool) -> RuntimeResult<RuntimeValue> {
    if dead {
        take_register(runtime, register)
    } else {
        Ok(read_register(runtime, register)?.clone())
    }
}

fn key_bytes(value: &ConstValue) -> RuntimeResult<Vec<u8>> {
    sley_mutate::encode_const_value(value).map_err(|_| RuntimeFault)
}

/// Binary search over canonically ordered map entries.
///
/// Keys order by their complete canonical encodings. Two `Bytes` keys of
/// one map share a type, and when they also share a length their encodings
/// share every header byte, so their order is the order of their raw
/// bytes: that common case (entity identities) compares without encoding.
fn search(
    entries: &[MapEntryConst],
    query_value: &ConstValue,
    query: &[u8],
) -> RuntimeResult<Result<usize, usize>> {
    let (mut low, mut high) = (0_usize, entries.len());
    while low < high {
        let middle = low + (high - low) / 2;
        let stored = &entries[middle].key;
        let order = match (&stored.data, &query_value.data) {
            (ConstData::Bytes(left), ConstData::Bytes(right))
                if left.len() == right.len() && stored.value_type == query_value.value_type =>
            {
                left.as_slice().cmp(right.as_slice())
            }
            _ => key_bytes(stored)?.as_slice().cmp(query),
        };
        match order {
            core::cmp::Ordering::Less => low = middle + 1,
            core::cmp::Ordering::Greater => high = middle,
            core::cmp::Ordering::Equal => return Ok(Ok(middle)),
        }
    }
    Ok(Err(low))
}

/// Whether a projected part is worth sharing instead of copying: parts
/// that own heap data. Scalars copy faster than a view tracks them.
fn shares_well(value: &ConstValue) -> bool {
    matches!(
        value.data,
        ConstData::Sequence(_)
            | ConstData::Map(_)
            | ConstData::Record(_)
            | ConstData::Bytes(_)
            | ConstData::Text(_)
            | ConstData::Variant(_)
            | ConstData::Option(Some(_))
            | ConstData::Result(_)
    )
}

fn add(left: u64, right: u64) -> Option<u64> {
    left.checked_add(right)
}

/// Units of a value with `data` typed `value_type`, from the data units.
fn whole(value_type: &TypeExpr, data_units: u64) -> Option<u64> {
    add(1, value_units_type(value_type))?.checked_add(data_units)
}

/// Executes one hot extended opcode, or returns `None` to defer to the
/// borrowed reference path.
#[allow(clippy::too_many_lines)]
pub(super) fn execute(
    runtime: &mut Runtime,
    constants: &ConstantIndex,
    source: &ExecutionSource<'_>,
    opcode: Opcode,
    instruction: &crate::Instruction,
    mask: u64,
    result_type: &TypeExpr,
) -> RuntimeResult<Option<RuntimeValue>> {
    let dead = |k: usize| k < 64 && mask & (1_u64 << k) != 0;
    let operands = instruction.operands.as_slice();
    let typed = |data: ConstData| ConstValue {
        value_type: result_type.clone(),
        data,
    };
    let produced = match (opcode, operands) {
        (Opcode::ConstantRef, []) => {
            let Immediate::Entity(id) = &instruction.immediate else {
                return Err(RuntimeFault);
            };
            return Ok(Some(constants.get(id).ok_or(RuntimeFault)?.clone()));
        }
        (Opcode::TupleNew | Opcode::VectorNew, registers) => {
            let mut items = Vec::with_capacity(registers.len());
            let mut parts = Vec::with_capacity(registers.len());
            let mut units = u64::try_from(registers.len())
                .ok()
                .and_then(|n| add(1, n))
                .ok_or(RuntimeFault)?;
            for (k, register) in registers.iter().enumerate() {
                let value = operand(runtime, *register, dead(k))?;
                units = add(units, value.units).ok_or(RuntimeFault)?;
                parts.push(value.units);
                items.push(value.into_value()?);
            }
            let units = whole(result_type, units).ok_or(RuntimeFault)?;
            let value = typed(ConstData::Sequence(items));
            if opcode == Opcode::TupleNew {
                return Ok(Some(RuntimeValue::with_parts(value, units, parts)));
            }
            Some((value, units))
        }
        (Opcode::RecordNew, registers) => {
            let Immediate::Entity(definition) = &instruction.immediate else {
                return Err(RuntimeFault);
            };
            let fields = crate::extended::record_fields(source.types, *definition)
                .map_err(|_| RuntimeFault)?;
            if fields.len() != registers.len() {
                return Err(RuntimeFault);
            }
            let mut units = u64::try_from(registers.len())
                .ok()
                .and_then(|n| add(33, n))
                .ok_or(RuntimeFault)?;
            let mut values = Vec::with_capacity(registers.len());
            let mut parts = Vec::with_capacity(registers.len());
            for (k, (field, register)) in fields.iter().zip(registers).enumerate() {
                let value = operand(runtime, *register, dead(k))?;
                units =
                    add(units, add(32, value.units).ok_or(RuntimeFault)?).ok_or(RuntimeFault)?;
                parts.push(value.units);
                values.push(FieldConst {
                    member_id: field.member_id,
                    value: value.into_value()?,
                });
            }
            let units = whole(result_type, units).ok_or(RuntimeFault)?;
            return Ok(Some(RuntimeValue::with_parts(
                typed(ConstData::Record(RecordConst {
                    definition: *definition,
                    fields: values,
                })),
                units,
                parts,
            )));
        }
        (Opcode::VariantNew, registers) if registers.len() <= 1 => {
            let Immediate::Variant(variant) = &instruction.immediate else {
                return Err(RuntimeFault);
            };
            let (payload, payload_units) = match registers {
                [register] => {
                    let value = operand(runtime, *register, dead(0))?;
                    let units = add(1, value.units).ok_or(RuntimeFault)?;
                    (Some(Box::new(value.into_value()?)), units)
                }
                _ => (None, 0),
            };
            let units = whole(result_type, add(65, payload_units).ok_or(RuntimeFault)?)
                .ok_or(RuntimeFault)?;
            Some((
                typed(ConstData::Variant(VariantConst {
                    definition: variant.definition,
                    member_id: variant.member_id,
                    payload,
                })),
                units,
            ))
        }
        (Opcode::OptionSome | Opcode::ResultOk | Opcode::ResultErr, [register]) => {
            let value = operand(runtime, *register, dead(0))?;
            let units =
                whole(result_type, add(2, value.units).ok_or(RuntimeFault)?).ok_or(RuntimeFault)?;
            let inner = value.parts.clone();
            let payload = Box::new(value.into_value()?);
            let data = match opcode {
                Opcode::OptionSome => ConstData::Option(Some(payload)),
                Opcode::ResultOk => ConstData::Result(ResultConst::Ok(payload)),
                _ => ConstData::Result(ResultConst::Err(payload)),
            };
            return Ok(Some(RuntimeValue::with_inner(typed(data), units, inner)));
        }
        (Opcode::TupleGet, [register]) => {
            let Immediate::Index(index) = &instruction.immediate else {
                return Err(RuntimeFault);
            };
            let index = usize::try_from(*index).map_err(|_| RuntimeFault)?;
            let element = if dead(0) && unique(runtime, *register) {
                let ConstData::Sequence(mut items) =
                    take_register(runtime, *register)?.into_value()?.data
                else {
                    return Err(RuntimeFault);
                };
                if index >= items.len() {
                    return Err(RuntimeFault);
                }
                items.swap_remove(index)
            } else {
                let source = read_register(runtime, *register)?;
                let ConstData::Sequence(items) = &source.value()?.data else {
                    return Err(RuntimeFault);
                };
                let element = items.get(index).ok_or(RuntimeFault)?;
                if shares_well(element) {
                    let units = source
                        .part_units(index)
                        .unwrap_or_else(|| value_units_const(element));
                    let index = u32::try_from(index).map_err(|_| RuntimeFault)?;
                    return Ok(Some(source.item_view(index, units)?));
                }
                element.clone()
            };
            let units = value_units_const(&element);
            Some((element, units))
        }
        (Opcode::RecordGet, [register]) => {
            let Immediate::Field(member) = &instruction.immediate else {
                return Err(RuntimeFault);
            };
            let element = if dead(0) && unique(runtime, *register) {
                let ConstData::Record(record) =
                    take_register(runtime, *register)?.into_value()?.data
                else {
                    return Err(RuntimeFault);
                };
                record
                    .fields
                    .into_iter()
                    .find(|field| field.member_id == *member)
                    .map(|field| field.value)
                    .ok_or(RuntimeFault)?
            } else {
                let source = read_register(runtime, *register)?;
                let ConstData::Record(record) = &source.value()?.data else {
                    return Err(RuntimeFault);
                };
                let position = record
                    .fields
                    .iter()
                    .position(|field| field.member_id == *member)
                    .ok_or(RuntimeFault)?;
                let element = &record.fields[position].value;
                if shares_well(element) {
                    let units = source
                        .part_units(position)
                        .unwrap_or_else(|| value_units_const(element));
                    let position = u32::try_from(position).map_err(|_| RuntimeFault)?;
                    return Ok(Some(source.item_view(position, units)?));
                }
                element.clone()
            };
            let units = value_units_const(&element);
            Some((element, units))
        }
        (Opcode::VectorGet, [vector, index]) => {
            let ConstData::UInt(index) = read_register(runtime, *index)?.value()?.data else {
                return Err(RuntimeFault);
            };
            let position = usize::try_from(index).ok();
            let element = if dead(0) && unique(runtime, *vector) {
                let ConstData::Sequence(mut items) =
                    take_register(runtime, *vector)?.into_value()?.data
                else {
                    return Err(RuntimeFault);
                };
                position
                    .filter(|position| *position < items.len())
                    .map(|position| items.swap_remove(position))
            } else {
                let ConstData::Sequence(items) = &read_register(runtime, *vector)?.value()?.data
                else {
                    return Err(RuntimeFault);
                };
                position.and_then(|position| items.get(position)).cloned()
            };
            let value = typed(ConstData::Option(element.map(Box::new)));
            let units = value_units_const(&value);
            Some((value, units))
        }
        (Opcode::VectorSet, [vector, index, replacement]) => {
            let ConstData::UInt(index) = read_register(runtime, *index)?.value()?.data else {
                return Err(RuntimeFault);
            };
            let TypeExpr::Result { ok, error } = result_type else {
                return Err(RuntimeFault);
            };
            let vector = operand(runtime, *vector, dead(0))?;
            let vector_units = vector.units;
            let mut vector = vector.into_value()?;
            let ConstData::Sequence(items) = &mut vector.data else {
                return Err(RuntimeFault);
            };
            if let Some(position) = usize::try_from(index)
                .ok()
                .filter(|position| *position < items.len())
            {
                let replacement = operand(runtime, *replacement, dead(2))?;
                let old_units = value_units_const(&items[position]);
                let new_units = replacement.units;
                items[position] = replacement.into_value()?;
                let inner = vector_units
                    .checked_sub(value_units_type(&vector.value_type))
                    .and_then(|units| units.checked_add(value_units_type(ok)))
                    .and_then(|units| units.checked_sub(old_units))
                    .and_then(|units| units.checked_add(new_units));
                vector.value_type = ok.as_ref().clone();
                let value = typed(ConstData::Result(ResultConst::Ok(Box::new(vector))));
                let units = inner
                    .and_then(|inner| whole(result_type, add(2, inner)?))
                    .unwrap_or_else(|| value_units_const(&value));
                Some((value, units))
            } else {
                let value = typed(ConstData::Result(ResultConst::Err(Box::new(ConstValue {
                    value_type: error.as_ref().clone(),
                    data: ConstData::BuiltinFailure(BuiltinFailureValue {
                        kind: BuiltinFailureKind::Index,
                        code: 1,
                    }),
                }))));
                let units = value_units_const(&value);
                Some((value, units))
            }
        }
        (Opcode::MapGet | Opcode::MapContains, [map, key]) => {
            let key_value = read_register(runtime, *key)?.value()?;
            let query = key_bytes(key_value)?;
            let map = read_register(runtime, *map)?.value()?;
            let ConstData::Map(entries) = &map.data else {
                return Err(RuntimeFault);
            };
            let found = search(entries, key_value, &query)?.ok();
            let value = if opcode == Opcode::MapGet {
                typed(ConstData::Option(
                    found.map(|position| Box::new(entries[position].value.clone())),
                ))
            } else {
                ConstValue {
                    value_type: TypeExpr::Bool,
                    data: ConstData::Bool(found.is_some()),
                }
            };
            let units = value_units_const(&value);
            Some((value, units))
        }
        (Opcode::MapInsert, [map, key, value]) => {
            let key = operand(runtime, *key, dead(1))?;
            let value = operand(runtime, *value, dead(2))?;
            let map = operand(runtime, *map, dead(0))?;
            let (map_units, key_units, value_units) = (map.units, key.units, value.units);
            let mut map = map.into_value()?;
            let key = key.into_value()?;
            let query = key_bytes(&key)?;
            let ConstData::Map(entries) = &mut map.data else {
                return Err(RuntimeFault);
            };
            let entry_key = key.clone();
            let entry = MapEntryConst {
                key,
                value: value.into_value()?,
            };
            let delta = match search(entries, &entry_key, &query)? {
                Ok(position) => {
                    let old = value_units_const(&entries[position].key)
                        .checked_add(value_units_const(&entries[position].value));
                    entries[position] = entry;
                    old.map(|old| (old, add(key_units, value_units)))
                }
                Err(position) => {
                    entries.insert(position, entry);
                    Some((0, add(1, key_units).and_then(|u| add(u, value_units))))
                }
            };
            let units = delta.and_then(|(removed, added)| {
                map_units
                    .checked_sub(value_units_type(&map.value_type))?
                    .checked_add(value_units_type(result_type))?
                    .checked_sub(removed)?
                    .checked_add(added?)
            });
            map.value_type = result_type.clone();
            let units = units.unwrap_or_else(|| value_units_const(&map));
            Some((map, units))
        }
        (Opcode::MapRemove, [map, key]) => {
            let key_value = read_register(runtime, *key)?.value()?.clone();
            let query = key_bytes(&key_value)?;
            let map = operand(runtime, *map, dead(0))?;
            let map_units = map.units;
            let mut map = map.into_value()?;
            let ConstData::Map(entries) = &mut map.data else {
                return Err(RuntimeFault);
            };
            let removed = match search(entries, &key_value, &query)? {
                Ok(position) => {
                    let entry = entries.remove(position);
                    add(1, value_units_const(&entry.key))
                        .and_then(|u| add(u, value_units_const(&entry.value)))
                }
                Err(_) => Some(0),
            };
            let units = removed.and_then(|removed| {
                map_units
                    .checked_sub(value_units_type(&map.value_type))?
                    .checked_add(value_units_type(result_type))?
                    .checked_sub(removed)
            });
            map.value_type = result_type.clone();
            let units = units.unwrap_or_else(|| value_units_const(&map));
            Some((map, units))
        }
        _ => None,
    };
    Ok(produced.map(|(value, units)| RuntimeValue::with_units(value, units)))
}
