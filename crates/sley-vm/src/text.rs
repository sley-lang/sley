//! Slice E9 native text entries (`docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md`
//! section E9, revision 18): seven pure, profile-pinned `Text` operations
//! carried over the already-frozen `adapter_invoke` opcode (161), in the
//! frozen invocation shape of slice E8 (two operands `scope` and `request`,
//! an `Entity` immediate naming a carried `AdapterImport`).
//!
//! `Text` is an exact Unicode scalar sequence (SSMC1 section 3, never
//! normalized), so every count, index, and length here is in scalars, not
//! bytes, and every result is a valid scalar sequence by construction.
//!
//! Identity: a text row is resolved by its frozen *adapter identity*
//! (`SLY1/BRIDGE/` plus the four-byte entry code, zero padded), not by its
//! entity identity. Candidate validation derives every created entity id
//! from the workspace nonce, so a program can carry a text row it created
//! itself; what pins the row to the native implementation is the adapter
//! identity, ABI version 1, the exact request/response types, the `Index`
//! failure type, and the empty effect list, all checked here at judgment,
//! fuel, and execution alike. The E8 bridge entries keep their entity-id
//! resolution unchanged; this module never resolves one of them, and the
//! bootstrap gates never resolve a text entry.

use sley_ssmc::{
    AdapterImport, BuiltinFailureKind, BuiltinFailureValue, ConstData, ConstValue, Immediate,
    ResultConst, TypeExpr,
};

use crate::extended::{BRIDGE_CAPACITY_CODE, BRIDGE_MAX_ITEMS, ExtendedFault, large_bridge_fuel};

/// The SSMC1 epoch-1 `Text` payload ceiling (`TYPE_SYSTEM_V1.md`): no text
/// entry constructs a longer text; past it the result is the capacity
/// value `Err(Index, 2)`.
pub const TEXT_MAX_BYTES: usize = 16_777_216;

/// The index-out-of-range code under `BuiltinFailure(Index)`, as in
/// `vector_set` and `BSL1`.
const OUT_OF_RANGE_CODE: u16 = 1;

pub use sley_check::text_rows::{TEXT_ENTRIES, TextKind, text_adapter_id, text_row_types};

fn index_error() -> TypeExpr {
    sley_check::text_rows::text_failure_type()
}

/// The frozen `AdapterImport` row of one text entry under `entity_id`: any
/// entity identity, the frozen adapter identity, ABI version 1, the exact
/// request/response types, `Index` failure, and no effects.
#[must_use]
pub fn text_import(kind: TextKind, entity_id: sley_id::EntityId) -> AdapterImport {
    let (_, request, response) = text_row_types(kind);
    AdapterImport {
        entity_id,
        adapter_id: text_adapter_id(kind),
        abi_version: 1,
        request_type: request,
        response_type: response,
        failure_type: index_error(),
        effects: Vec::new(),
    }
}

/// Resolves an `adapter_invoke` immediate to a text entry: the immediate
/// must name a carried import whose adapter identity is a text entry's and
/// whose every other frozen field equals that entry's row. Anything else is
/// not a text entry.
#[must_use]
pub fn resolve_text_entry<'a>(
    adapters: &'a [AdapterImport],
    immediate: &Immediate,
) -> Option<(TextKind, &'a AdapterImport)> {
    let Immediate::Entity(id) = immediate else {
        return None;
    };
    let carried = adapters.iter().find(|import| import.entity_id == *id)?;
    let kind = sley_check::text_rows::text_kind(&carried.adapter_id)?;
    let (_, request, response) = text_row_types(kind);
    (carried.abi_version == 1
        && carried.failure_type == index_error()
        && carried.effects.is_empty()
        && carried.request_type == request
        && carried.response_type == response)
        .then_some((kind, carried))
}

/// The judged result type of a text use: the operand types must equal the
/// entry's scope and request exactly. `None` is a signature mismatch.
#[must_use]
pub fn judge_text_use(kind: TextKind, scope: &TypeExpr, request: &TypeExpr) -> Option<TypeExpr> {
    let (want_scope, want_request, response) = text_row_types(kind);
    (*scope == want_scope && *request == want_request).then(|| TypeExpr::Result {
        ok: Box::new(response),
        error: Box::new(index_error()),
    })
}

fn text_bytes(value: &ConstValue) -> Option<usize> {
    match &value.data {
        ConstData::Text(text) => Some(text.len()),
        _ => None,
    }
}

/// Up-front fuel of one text use, charged before the entry does any work
/// (the E8 precedent), proportional to the bytes the entry can touch:
/// `1 + ceil(bytes/1024)` over the text it reads (`TLN1`, `TSL1`), the
/// vector length plus that schedule over the joined bytes (`TCT1`, the
/// `BCT1` precedent), one per byte read for `TCS1` (it builds one value per
/// scalar, the `B2V1` precedent), and 1 for the bounded `TOR1`, `TCH1`,
/// and `TFI1`. `None` when the operands do not have the entry's shapes.
#[must_use]
pub fn text_fuel(kind: TextKind, operands: &[&ConstValue]) -> Option<u64> {
    let [scope, request] = operands else {
        return None;
    };
    match kind {
        TextKind::Length => text_bytes(request).map(large_bridge_fuel),
        TextKind::Slice => text_bytes(scope).map(large_bridge_fuel),
        TextKind::Concat => match &request.data {
            ConstData::Sequence(items) => {
                let mut total = 0_usize;
                for item in items {
                    total = total.saturating_add(text_bytes(item)?);
                }
                u64::try_from(items.len())
                    .ok()?
                    .checked_add(large_bridge_fuel(total))
            }
            _ => None,
        },
        TextKind::Scalars => {
            text_bytes(request).and_then(|len| u64::try_from(len).ok()?.checked_add(1))
        }
        TextKind::Ord | TextKind::Chr | TextKind::FromInt => Some(1),
    }
}

/// Runs one text entry. Total: every refusal is a typed `Err(Index, code)`
/// value (1 out of range, 2 capacity); operand shapes and the carried row
/// are rechecked, and a mismatch is an internal fault, never a mistyped
/// value.
#[allow(clippy::too_many_lines)] // one arm per entry
pub(crate) fn text_execute(
    kind: TextKind,
    carried: &AdapterImport,
    operands: &[&ConstValue],
    result_type: &TypeExpr,
) -> Result<ConstValue, ExtendedFault> {
    let [scope, request] = operands else {
        return Err(ExtendedFault);
    };
    let (want_scope, want_request, response) = text_row_types(kind);
    let TypeExpr::Result { ok, error } = result_type else {
        return Err(ExtendedFault);
    };
    if **ok != response
        || **error != index_error()
        || carried.response_type != response
        || carried.request_type != want_request
        || scope.value_type != want_scope
        || request.value_type != want_request
    {
        return Err(ExtendedFault);
    }
    let done = |data: ConstData| ConstValue {
        value_type: result_type.clone(),
        data: ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
            value_type: response.clone(),
            data,
        }))),
    };
    let refuse = |code: u16| ConstValue {
        value_type: result_type.clone(),
        data: ConstData::Result(ResultConst::Err(Box::new(ConstValue {
            value_type: index_error(),
            data: ConstData::BuiltinFailure(BuiltinFailureValue {
                kind: BuiltinFailureKind::Index,
                code,
            }),
        }))),
    };
    let unit = |value: &ConstValue| match value.data {
        ConstData::Unit => Ok(()),
        _ => Err(ExtendedFault),
    };
    let one_scalar = |c: char| ConstValue {
        value_type: TypeExpr::Text,
        data: ConstData::Text(c.to_string()),
    };
    match kind {
        TextKind::Length => {
            unit(scope)?;
            let ConstData::Text(text) = &request.data else {
                return Err(ExtendedFault);
            };
            let count = u128::try_from(text.chars().count()).map_err(|_| ExtendedFault)?;
            Ok(done(ConstData::UInt(count)))
        }
        TextKind::Slice => {
            let ConstData::Text(text) = &scope.data else {
                return Err(ExtendedFault);
            };
            let ConstData::Sequence(range) = &request.data else {
                return Err(ExtendedFault);
            };
            let [start, length] = range.as_slice() else {
                return Err(ExtendedFault);
            };
            let (ConstData::UInt(start), ConstData::UInt(length)) = (&start.data, &length.data)
            else {
                return Err(ExtendedFault);
            };
            let (Ok(start), Ok(length)) = (usize::try_from(*start), usize::try_from(*length))
            else {
                return Ok(refuse(OUT_OF_RANGE_CODE));
            };
            if start.checked_add(length).is_none() {
                return Ok(refuse(OUT_OF_RANGE_CODE));
            }
            // Byte offsets of scalar `start` and scalar `end`; one pass
            // stops at `end`, and `end == count` is the text's end.
            let mut offsets = text
                .char_indices()
                .map(|(offset, _)| offset)
                .chain(std::iter::once(text.len()));
            let Some(from) = offsets.nth(start) else {
                return Ok(refuse(OUT_OF_RANGE_CODE));
            };
            let to = if length == 0 {
                from
            } else {
                match offsets.nth(length - 1) {
                    Some(to) => to,
                    None => return Ok(refuse(OUT_OF_RANGE_CODE)),
                }
            };
            Ok(done(ConstData::Text(text[from..to].to_owned())))
        }
        TextKind::Concat => {
            unit(scope)?;
            let ConstData::Sequence(items) = &request.data else {
                return Err(ExtendedFault);
            };
            let mut total = 0_usize;
            for item in items {
                if item.value_type != TypeExpr::Text {
                    return Err(ExtendedFault);
                }
                let ConstData::Text(text) = &item.data else {
                    return Err(ExtendedFault);
                };
                total = total.saturating_add(text.len());
            }
            if total > TEXT_MAX_BYTES {
                return Ok(refuse(BRIDGE_CAPACITY_CODE));
            }
            let mut joined = String::with_capacity(total);
            for item in items {
                if let ConstData::Text(text) = &item.data {
                    joined.push_str(text);
                }
            }
            Ok(done(ConstData::Text(joined)))
        }
        TextKind::Scalars => {
            unit(scope)?;
            let ConstData::Text(text) = &request.data else {
                return Err(ExtendedFault);
            };
            if text.chars().count() > BRIDGE_MAX_ITEMS {
                return Ok(refuse(BRIDGE_CAPACITY_CODE));
            }
            Ok(done(ConstData::Sequence(
                text.chars().map(one_scalar).collect(),
            )))
        }
        TextKind::Ord => {
            unit(scope)?;
            let ConstData::Text(text) = &request.data else {
                return Err(ExtendedFault);
            };
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Ok(done(ConstData::UInt(u128::from(u32::from(c))))),
                _ => Ok(refuse(OUT_OF_RANGE_CODE)),
            }
        }
        TextKind::Chr => {
            unit(scope)?;
            let ConstData::UInt(value) = request.data else {
                return Err(ExtendedFault);
            };
            match u32::try_from(value).ok().and_then(char::from_u32) {
                Some(c) => Ok(done(ConstData::Text(c.to_string()))),
                None => Ok(refuse(OUT_OF_RANGE_CODE)),
            }
        }
        TextKind::FromInt => {
            unit(scope)?;
            let ConstData::SInt(value) = request.data else {
                return Err(ExtendedFault);
            };
            Ok(done(ConstData::Text(value.to_string())))
        }
    }
}
