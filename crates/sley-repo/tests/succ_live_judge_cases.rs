//! Live-trial strict-case driver for the `sley_2_0` arm.
//!
//! The Python trial oracle (`bench/fixtures/sley2_live_judge.py`) cannot
//! judge computed values over SMP1: execution reports carry observation
//! hashes by product design, never values. This frozen driver executes
//! strict cases in-process (the exact S3 conformance pattern) and reports
//! values, codes, and fuel as JSON the Python judge compares.
//!
//! Environment in (all required unless noted):
//! - `SUCC_JUDGE_REPO`: scratch repository path (already seeded copy).
//! - `SUCC_JUDGE_MANIFEST`: `task_manifest.json` path (principal source).
//! - `SUCC_JUDGE_CANDIDATE`: candidate record hex, or empty to skip commit.
//! - `SUCC_JUDGE_FUNCTION`: target function entity hex.
//! - `SUCC_JUDGE_CASES`: JSON array of `{"inputs": [...]}`. Each input
//!   item is decoded against the target function's DECLARED parameter
//!   type (positional), using the submitted program's accepted type
//!   definitions for named types. Legacy items carrying an explicit
//!   `"type"` tag (`SInt`/`Bool`/`UInt`/`Unit`) keep working and are
//!   checked for consistency with the declared type. New items omit the
//!   tag and are driven purely by the declared type:
//!   - `SInt`/`UInt`: `{"value": N}` (JSON integer, never bool; range
//!     checked against the declared width).
//!   - `Bool`: `{"value": b}`.
//!   - `Unit`: `{}`.
//!   - `Vector(element)`: `{"values": [...]}` (empty allowed), each
//!     element decoded against the declared element type.
//!   - `Named(record definition)`: `{"fields": {memberhex: item}}` with
//!     EXACTLY the definition's member set (no missing, no extra),
//!     each field decoded against its declared field type, fields
//!     stored in definition order. The definition must exist in the
//!     submitted accepted state; variant definitions are explicitly
//!     unsupported as driver inputs.
//!   - `Result{ok, error}`: `{"ok": item}` or `{"err": item}` (exactly
//!     one), decoded against the corresponding arm type.
//!
//!   Nesting beyond 8 levels is rejected. Every decoded value passes
//!   production `check_constant` before execution. Missing or malformed
//!   required fields are driver errors, never silent zeros.
//! - `SUCC_JUDGE_BASELINE`: `1` to also run cases pre-commit.
//!
//! Stdout: one `LIVE_JUDGE_RESULT {...}` line. Exit 0 with a verdict
//! inside the JSON; any driver failure prints a harness-error verdict
//! (`{"ok": false, "harness_error": ...}`), never a panic without a
//! line. Per-case input/render failures report
//! `{"ok": false, "code": "DRIVER_INPUT_INVALID: ..."}` or
//! `"DRIVER_RENDER_INVALID: ..."`; the Python judge maps any `DRIVER_`
//! code to a harness failure, never candidate blame.

use sley_check::TypeEnvironment;
use sley_id::EntityId;
use sley_repo::RepositoryObjectVerifier;
use sley_ssmc::{
    ConstData, ConstValue, FieldConst, IntegerWidth, MemberId, NamedType, RecordConst, ResultConst,
    TypeDefinition, TypeExpr, Visibility,
};
use sley_state_root::conformance_epoch_id as state_epoch_id;
use sley_store::ObjectStore;
use sley_txn::TransactionRepository;
use sley_vm::{
    CacheProfile, ExecutionLimits, ExecutionRequest, ExecutionTermination, LoweringInput,
    execute_function,
};
use std::collections::BTreeMap;

/// Maximum nested input depth (records/vectors/results). This bounds the
/// structural decoder; deeper inputs are rejected, never truncated.
const MAX_CONST_DEPTH: u32 = 8;

fn unhex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("hex length".to_string());
    }
    (0..text.len() / 2)
        .map(|i| {
            u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|_| "hex digits".to_string())
        })
        .collect()
}

fn hex32(text: &str) -> Result<[u8; 32], String> {
    let bytes = unhex(text).map_err(|detail| format!("hex32 {detail}"))?;
    bytes.try_into().map_err(|_| "hex32 length".to_string())
}

fn hex_of(id: &EntityId) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let bytes = id.as_bytes();
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    out
}

fn member_hex(id: &MemberId) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in id.as_bytes() {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    out
}

/// Named-type resolution against the submitted program's accepted state.
struct ConstResolver<'a> {
    types: &'a TypeEnvironment,
    definitions: BTreeMap<EntityId, &'a TypeDefinition>,
}

impl<'a> ConstResolver<'a> {
    fn definition(&self, id: &EntityId) -> Result<&'a TypeDefinition, String> {
        self.definitions
            .get(id)
            .copied()
            .ok_or_else(|| format!("unknown type definition {}", hex_of(id)))
    }
}

fn sint_range(width: u16, value: i128) -> Result<i128, String> {
    let (low, high) = match width {
        8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
        16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
        32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
        64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
        128 => (i128::MIN, i128::MAX),
        other => return Err(format!("unsupported SInt width {other}")),
    };
    if (low..=high).contains(&value) {
        Ok(value)
    } else {
        Err(format!("SInt{width} out of range"))
    }
}

fn uint_range(width: u16, value: u128) -> Result<u128, String> {
    let high = match width {
        8 => u128::from(u8::MAX),
        16 => u128::from(u16::MAX),
        32 => u128::from(u32::MAX),
        64 => u128::from(u64::MAX),
        128 => u128::MAX,
        other => return Err(format!("unsupported UInt width {other}")),
    };
    if value <= high {
        Ok(value)
    } else {
        Err(format!("UInt{width} out of range"))
    }
}

fn json_int(value: &serde_json::Value) -> Result<i128, String> {
    match value {
        serde_json::Value::Number(number) => number
            .as_i64()
            .map(i128::from)
            .ok_or_else(|| "integer out of range".to_string()),
        _ => Err("integer required".to_string()),
    }
}

fn json_uint(value: &serde_json::Value) -> Result<u128, String> {
    match value {
        serde_json::Value::Number(number) => number
            .as_u64()
            .map(u128::from)
            .ok_or_else(|| "unsigned integer out of range".to_string()),
        _ => Err("unsigned integer required".to_string()),
    }
}

/// Decode one input item against its declared parameter type. Legacy
/// `"type"`-tagged items are accepted for leaf scalars and checked for
/// consistency with the declared type; every other item is driven purely
/// by the declared type. Failures are errors, never defaulted values.
fn const_of(
    resolver: &ConstResolver<'_>,
    expected: &TypeExpr,
    item: &serde_json::Value,
    depth: u32,
) -> Result<ConstValue, String> {
    if depth > MAX_CONST_DEPTH {
        return Err("input nesting exceeds bound".to_string());
    }
    if let Some(tag) = item.get("type").and_then(serde_json::Value::as_str) {
        return legacy_const_of(resolver, expected, tag, item, depth);
    }
    let data = match expected {
        TypeExpr::SInt(width) => {
            let raw = item
                .get("value")
                .map(json_int)
                .transpose()?
                .ok_or_else(|| "SInt value required".to_string())
                .map_err(|detail| format!("SInt input invalid: {detail}"))?;
            ConstData::SInt(sint_range(width.bits(), raw)?)
        }
        TypeExpr::UInt(width) => {
            let raw = item
                .get("value")
                .map(json_uint)
                .transpose()?
                .ok_or_else(|| "UInt value required".to_string())
                .map_err(|detail| format!("UInt input invalid: {detail}"))?;
            ConstData::UInt(uint_range(width.bits(), raw)?)
        }
        TypeExpr::Bool => {
            let flag = item
                .get("value")
                .and_then(serde_json::Value::as_bool)
                .ok_or_else(|| "Bool value required".to_string())?;
            ConstData::Bool(flag)
        }
        TypeExpr::Unit => ConstData::Unit,
        TypeExpr::Vector(element) => {
            let values = item
                .get("values")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| "Vector values required".to_string())?;
            let mut elements = Vec::with_capacity(values.len());
            for entry in values {
                elements.push(const_of(resolver, element, entry, depth + 1)?);
            }
            ConstData::Sequence(elements)
        }
        TypeExpr::Named(named) => record_of(resolver, &named.definition, item, depth)?,
        TypeExpr::Result { ok, error } => {
            let has_ok = item.get("ok").is_some();
            let has_err = item.get("err").is_some();
            match (has_ok, has_err) {
                (true, false) => {
                    let inner = const_of(
                        resolver,
                        ok,
                        item.get("ok").unwrap_or(&serde_json::Value::Null),
                        depth + 1,
                    )?;
                    ConstData::Result(ResultConst::Ok(Box::new(inner)))
                }
                (false, true) => {
                    let inner = const_of(
                        resolver,
                        error,
                        item.get("err").unwrap_or(&serde_json::Value::Null),
                        depth + 1,
                    )?;
                    ConstData::Result(ResultConst::Err(Box::new(inner)))
                }
                _ => return Err("Result input needs exactly one of ok/err".to_string()),
            }
        }
        other => {
            return Err(format!(
                "unsupported declared input type {}",
                type_summary(other)
            ));
        }
    };
    let value = ConstValue {
        value_type: expected.clone(),
        data,
    };
    resolver
        .types
        .check_constant(&value)
        .map_err(|error| format!("production constant check failed: {error:?}"))?;
    Ok(value)
}

/// Decode a record item against a submitted record definition: the member
/// set must match exactly, fields decode against declared field types in
/// definition order, and the definition identity is preserved.
fn record_of(
    resolver: &ConstResolver<'_>,
    definition: &EntityId,
    item: &serde_json::Value,
    depth: u32,
) -> Result<ConstData, String> {
    let typedef = resolver.definition(definition)?;
    let fields = match &typedef.form {
        sley_ssmc::TypeDefForm::Record(fields) => fields,
        sley_ssmc::TypeDefForm::Variant(_) => {
            return Err("variant definition unsupported as driver input".to_string());
        }
    };
    let supplied = item
        .get("fields")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "Record fields required".to_string())?;
    if supplied.len() != fields.len() {
        return Err(format!(
            "Record field count {} != definition {}",
            supplied.len(),
            fields.len()
        ));
    }
    let mut consts = Vec::with_capacity(fields.len());
    for field in fields {
        let key = member_hex(&field.member_id);
        let entry = supplied
            .get(&key)
            .ok_or_else(|| format!("Record member missing {key}"))?;
        consts.push(FieldConst {
            member_id: field.member_id,
            value: const_of(resolver, &field.value_type, entry, depth + 1)?,
        });
    }
    Ok(ConstData::Record(RecordConst {
        definition: *definition,
        fields: consts,
    }))
}

/// Legacy `"type"`-tagged items (existing judges): strict decoding with
/// consistency against the declared parameter type. Missing or malformed
/// required fields are errors, never silent defaults.
fn legacy_const_of(
    resolver: &ConstResolver<'_>,
    expected: &TypeExpr,
    tag: &str,
    item: &serde_json::Value,
    depth: u32,
) -> Result<ConstValue, String> {
    match tag {
        "SInt" => {
            let TypeExpr::SInt(width) = expected else {
                return Err("SInt item for non-SInt parameter".to_string());
            };
            let bits = item
                .get("bits")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| "SInt bits required".to_string())?;
            if bits != u64::from(width.bits()) {
                return Err(format!("SInt bits {bits} != declared {}", width.bits()));
            }
            let raw = item
                .get("value")
                .map(json_int)
                .transpose()?
                .ok_or_else(|| "SInt value required".to_string())
                .map_err(|detail| format!("SInt input invalid: {detail}"))?;
            let value = ConstValue {
                value_type: expected.clone(),
                data: ConstData::SInt(sint_range(width.bits(), raw)?),
            };
            resolver
                .types
                .check_constant(&value)
                .map_err(|error| format!("production constant check failed: {error:?}"))?;
            Ok(value)
        }
        "UInt" => {
            let TypeExpr::UInt(width) = expected else {
                return Err("UInt item for non-UInt parameter".to_string());
            };
            let bits = item
                .get("bits")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| "UInt bits required".to_string())?;
            if bits != u64::from(width.bits()) {
                return Err(format!("UInt bits {bits} != declared {}", width.bits()));
            }
            let raw = item
                .get("value")
                .map(json_uint)
                .transpose()?
                .ok_or_else(|| "UInt value required".to_string())
                .map_err(|detail| format!("UInt input invalid: {detail}"))?;
            let value = ConstValue {
                value_type: expected.clone(),
                data: ConstData::UInt(uint_range(width.bits(), raw)?),
            };
            resolver
                .types
                .check_constant(&value)
                .map_err(|error| format!("production constant check failed: {error:?}"))?;
            Ok(value)
        }
        "Bool" => {
            if *expected != TypeExpr::Bool {
                return Err("Bool item for non-Bool parameter".to_string());
            }
            let flag = item
                .get("value")
                .and_then(serde_json::Value::as_bool)
                .ok_or_else(|| "Bool value required".to_string())?;
            Ok(ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(flag),
            })
        }
        "Unit" => {
            if *expected != TypeExpr::Unit {
                return Err("Unit item for non-Unit parameter".to_string());
            }
            Ok(ConstValue {
                value_type: TypeExpr::Unit,
                data: ConstData::Unit,
            })
        }
        other => {
            let _ = depth;
            Err(format!("unsupported const kind {other}"))
        }
    }
}

fn type_summary(kind: &TypeExpr) -> String {
    match kind {
        TypeExpr::Unit => "Unit".to_string(),
        TypeExpr::Bool => "Bool".to_string(),
        TypeExpr::SInt(width) => format!("SInt{}", width.bits()),
        TypeExpr::UInt(width) => format!("UInt{}", width.bits()),
        TypeExpr::F32 => "F32".to_string(),
        TypeExpr::F64 => "F64".to_string(),
        TypeExpr::Bytes => "Bytes".to_string(),
        TypeExpr::Text => "Text".to_string(),
        TypeExpr::Tuple(items) => {
            let inner: Vec<String> = items.iter().map(type_summary).collect();
            format!("Tuple({})", inner.join(","))
        }
        TypeExpr::Named(named) => format!("Named({})", hex_of(&named.definition)),
        TypeExpr::Vector(element) => format!("Vector({})", type_summary(element)),
        TypeExpr::OrderedMap { key, value } => {
            format!("OrderedMap({},{})", type_summary(key), type_summary(value))
        }
        TypeExpr::Option(inner) => format!("Option({})", type_summary(inner)),
        TypeExpr::Result { ok, error } => {
            format!("Result({},{})", type_summary(ok), type_summary(error))
        }
        TypeExpr::FunctionRef(_) => "FunctionRef".to_string(),
        TypeExpr::AdapterHandle(id) => format!("AdapterHandle({})", hex_of(id)),
        TypeExpr::CapabilityToken(id) => format!("CapabilityToken({})", hex_of(id)),
        TypeExpr::LocalCell(inner) => format!("LocalCell({})", type_summary(inner)),
        TypeExpr::TypeParameter(index) => format!("TypeParameter({index})"),
        TypeExpr::BuiltinFailure(kind) => format!("BuiltinFailure({kind:?})"),
    }
}

struct ProgramSlice {
    function: sley_ssmc::FunctionGraph,
    types: sley_check::TypeEnvironment,
    parameters: Vec<TypeExpr>,
    definitions: BTreeMap<EntityId, TypeDefinition>,
}

fn function_inputs(
    complete: &sley_policy::complete_entities::CompleteEntities,
    func: EntityId,
) -> Result<ProgramSlice, String> {
    // Only the entry graph is target-selected. Parameters, blocks,
    // and operations travel as the complete inventories production
    // lowering requires (see the LoweringInput construction): the
    // driver never slices the root.
    let function = complete
        .functions
        .iter()
        .find(|f| f.entity_id == func)
        .ok_or_else(|| "function bound".to_string())?
        .clone();
    let mut parameters = Vec::with_capacity(function.parameters.len());
    for id in &function.parameters {
        let parameter = complete
            .parameters
            .iter()
            .find(|p| p.entity_id == *id)
            .ok_or_else(|| "parameter bound".to_string())?;
        parameters.push(parameter.value_type.clone());
    }
    let types = TypeEnvironment::new(complete.type_definitions.clone())
        .map_err(|error| format!("type environment invalid: {error:?}"))?;
    let definitions = complete
        .type_definitions
        .iter()
        .map(|definition| (definition.entity_id, definition.clone()))
        .collect();
    Ok(ProgramSlice {
        function,
        types,
        parameters,
        definitions,
    })
}

fn epoch() -> Result<sley_id::SchemaEpochId, String> {
    sley_state_root::conformance_epoch_id().map_err(|error| format!("epoch: {error:?}"))
}

fn outcome_json(
    resolver: &ConstResolver<'_>,
    outcome: &Result<sley_vm::ExecutionOutcome, sley_vm::ExecutionError>,
) -> serde_json::Value {
    match outcome {
        Ok(ok) => {
            let value = match &ok.termination {
                ExecutionTermination::Success(value) => {
                    match const_summary(&resolver.definitions, &value.data) {
                        Ok(summary) => summary,
                        Err(detail) => {
                            return serde_json::json!({"ok": false,
                                "code": format!("DRIVER_RENDER_INVALID: {detail}")});
                        }
                    }
                }
                other => serde_json::json!({"non_success": format!("{other:?}")}),
            };
            serde_json::json!({"ok": true, "value": value,
                "instructions": ok.instruction_count, "fuel": ok.fuel_used,
                "peak_value_units": ok.peak_value_units})
        }
        Err(error) => serde_json::json!({"ok": false, "code": error.to_string()}),
    }
}

/// Structured rendering of every closed constant shape the engine can
/// produce, in definition order for records. No `Debug` fallback: values
/// the judge must verify render as data the judge can compare.
fn const_summary(
    definitions: &BTreeMap<EntityId, &TypeDefinition>,
    data: &ConstData,
) -> Result<serde_json::Value, String> {
    match data {
        ConstData::Unit => Ok(serde_json::json!({"Unit": true})),
        ConstData::Bool(flag) => Ok(serde_json::json!({"Bool": flag})),
        ConstData::SInt(number) => Ok(serde_json::json!({"SInt": number.to_string()})),
        ConstData::UInt(number) => Ok(serde_json::json!({"UInt": number.to_string()})),
        ConstData::F32Bits(bits) => Ok(serde_json::json!({"F32Bits": bits.to_string()})),
        ConstData::F64Bits(bits) => Ok(serde_json::json!({"F64Bits": bits.to_string()})),
        ConstData::Bytes(bytes) => Ok(serde_json::json!({"Bytes": hex_bytes(bytes)})),
        ConstData::Text(text) => Ok(serde_json::json!({"Text": text})),
        ConstData::Sequence(elements) => {
            let mut out = Vec::with_capacity(elements.len());
            for element in elements {
                out.push(const_summary(definitions, &element.data)?);
            }
            Ok(serde_json::json!({"Sequence": out}))
        }
        ConstData::Record(record) => {
            let typedef = definitions.get(&record.definition).ok_or_else(|| {
                format!(
                    "render references unknown definition {}",
                    hex_of(&record.definition)
                )
            })?;
            let order = match &typedef.form {
                sley_ssmc::TypeDefForm::Record(fields) => fields
                    .iter()
                    .map(|field| field.member_id)
                    .collect::<Vec<_>>(),
                sley_ssmc::TypeDefForm::Variant(_) => {
                    return Err("render of variant-typed record".to_string());
                }
            };
            if record.fields.len() != order.len() {
                return Err("render record arity mismatch".to_string());
            }
            let mut fields = serde_json::Map::with_capacity(order.len());
            for member in order {
                let field = record
                    .fields
                    .iter()
                    .find(|entry| entry.member_id == member)
                    .ok_or_else(|| "render record member missing".to_string())?;
                fields.insert(
                    member_hex(&member),
                    const_summary(definitions, &field.value.data)?,
                );
            }
            Ok(serde_json::json!({"Record": {
                "definition": hex_of(&record.definition),
                "fields": fields,
            }}))
        }
        ConstData::Variant(variant) => {
            let payload = match &variant.payload {
                Some(inner) => const_summary(definitions, &inner.data)?,
                None => serde_json::Value::Null,
            };
            Ok(serde_json::json!({"Variant": {
                "definition": hex_of(&variant.definition),
                "member": member_hex(&variant.member_id),
                "payload": payload,
            }}))
        }
        ConstData::Map(entries) => {
            let mut out = Vec::with_capacity(entries.len());
            for entry in entries {
                out.push(serde_json::json!({
                    "key": const_summary(definitions, &entry.key.data)?,
                    "value": const_summary(definitions, &entry.value.data)?,
                }));
            }
            Ok(serde_json::json!({"Map": out}))
        }
        ConstData::Option(inner) => match inner {
            Some(value) => Ok(serde_json::json!({
                "Option": const_summary(definitions, &value.data)?,
            })),
            None => Ok(serde_json::json!({"Option": null})),
        },
        ConstData::Result(result) => match result {
            ResultConst::Ok(inner) => Ok(serde_json::json!({
                "Result": {"Ok": const_summary(definitions, &inner.data)?},
            })),
            ResultConst::Err(inner) => Ok(serde_json::json!({
                "Result": {"Err": const_summary(definitions, &inner.data)?},
            })),
        },
        ConstData::FunctionRef(reference) => Ok(serde_json::json!({"FunctionRef": {
            "function": hex_of(&reference.function),
        }})),
        ConstData::BuiltinFailure(failure) => Ok(serde_json::json!({
            "BuiltinFailure": {"kind": format!("{:?}", failure.kind), "code": failure.code},
        })),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    out
}

fn run_cases(
    complete: &sley_policy::complete_entities::CompleteEntities,
    program: &ProgramSlice,
    state_root: sley_id::StateRoot,
    cases: &[serde_json::Value],
) -> Result<Vec<serde_json::Value>, String> {
    let definition_refs: BTreeMap<EntityId, &TypeDefinition> = program
        .definitions
        .iter()
        .map(|(id, definition)| (*id, definition))
        .collect();
    let resolver = ConstResolver {
        types: &program.types,
        definitions: definition_refs,
    };
    let mut out = Vec::new();
    for case in cases {
        let items = case["inputs"]
            .as_array()
            .ok_or_else(|| "case inputs shape".to_string())?;
        if items.len() != program.parameters.len() {
            out.push(serde_json::json!({"ok": false, "code": format!(
                "DRIVER_INPUT_INVALID: arity {} != parameters {}",
                items.len(),
                program.parameters.len(),
            )}));
            continue;
        }
        let mut inputs = Vec::with_capacity(items.len());
        let mut failed = None;
        for (item, expected) in items.iter().zip(program.parameters.iter()) {
            match const_of(&resolver, expected, item, 0) {
                Ok(value) => inputs.push(value),
                Err(detail) => {
                    failed = Some(detail);
                    break;
                }
            }
        }
        if let Some(detail) = failed {
            out.push(
                serde_json::json!({"ok": false, "code": format!("DRIVER_INPUT_INVALID: {detail}")}),
            );
            continue;
        }
        let request = ExecutionRequest {
            inputs,
            limits: ExecutionLimits {
                max_instructions: 1_000,
                max_fuel: 10_000,
                max_value_units: 100_000,
                max_output_units: 10_000,
                cancel_at_fuel: None,
            },
        };
        let input = LoweringInput {
            types: &program.types,
            function: &program.function,
            // Complete root inventories: production lowering narrows
            // per function itself (owned_inventory) and judges
            // call_direct callees against the root context (contract
            // E6). A root sliced to the target starves callee
            // signature resolution (VM_LOWER_IMMEDIATE_MISMATCH) and
            // callee narrowing. Only the entry graph itself is
            // target-selected; everything else stays complete.
            parameters: &complete.parameters,
            blocks: &complete.blocks,
            operations: &complete.operations,
            schema_epoch: epoch()?,
            state_root,
            profile: CacheProfile::EXTENDED_V1,
            constants: &complete.constants,
            globals: &complete.globals,
            functions: &complete.functions,
            contracts: &[],
            adapters: &[],
        };
        out.push(outcome_json(&resolver, &execute_function(input, request)));
    }
    Ok(out)
}

fn drive() -> Result<serde_json::Value, String> {
    // Env: SUCC_JUDGE_REPO (seeded scratch repo), SUCC_JUDGE_FUNCTION
    // (target entity hex), SUCC_JUDGE_CASES (JSON array of
    // {"inputs": [...]} decoded against declared parameter types).
    // Never commits, never writes: the Python judge owns state changes.
    let repo_path = std::env::var("SUCC_JUDGE_REPO").map_err(|_| "SUCC_JUDGE_REPO".to_string())?;
    let function_hex =
        std::env::var("SUCC_JUDGE_FUNCTION").map_err(|_| "SUCC_JUDGE_FUNCTION".to_string())?;
    let function = EntityId::from_bytes(hex32(&function_hex)?);
    let cases_text =
        std::env::var("SUCC_JUDGE_CASES").map_err(|_| "SUCC_JUDGE_CASES".to_string())?;
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&cases_text).map_err(|_| "SUCC_JUDGE_CASES JSON".to_string())?;
    let root = std::path::PathBuf::from(&repo_path);
    let repo = TransactionRepository::new(&root);
    let epoch_id = state_epoch_id().map_err(|error| format!("epoch: {error:?}"))?;
    let store = ObjectStore::new(&root);
    let verifier = RepositoryObjectVerifier::new(epoch_id);
    let head = repo
        .accepted_head()
        .map_err(|error| format!("head: {error:?}"))?;
    let mut objects = Vec::new();
    for meta in head.objects() {
        let bytes = store
            .read(meta.object_id(), &verifier)
            .map_err(|error| format!("object read: {error:?}"))?;
        objects.push(
            sley_mutate::import_entity_object(epoch_id, &bytes)
                .map_err(|error| format!("object import: {error:?}"))?,
        );
    }
    objects.sort_by_key(|o: &sley_mutate::EntityObject| o.record().entity_id);
    let complete = sley_policy::complete_entities::project_complete_entities(&objects)
        .map_err(|error| format!("complete projection: {error:?}"))?;
    let program = function_inputs(&complete, function)?;
    let results = run_cases(&complete, &program, head.state_root().root, &cases)?;
    Ok(serde_json::json!({"ok": true, "cases": results}))
}

#[test]
fn live_case_driver() {
    // Stdout: one LIVE_JUDGE_RESULT {"ok": true, "cases": [...]} line
    // with per-case values, codes, and fuel. Driver failures print a
    // harness-error verdict instead of panicking without a line.
    let report = match drive() {
        Ok(report) => report,
        Err(detail) => serde_json::json!({"ok": false, "harness_error": detail}),
    };
    println!(
        "LIVE_JUDGE_RESULT {}",
        serde_json::to_string(&report).unwrap_or_else(|_| "{\"ok\":false}".to_string())
    );
}

#[cfg(test)]
mod typed_driver_tests {
    use super::*;

    const MONEY_DEF: [u8; 32] = [0xA1; 32];
    const LINE_DEF: [u8; 32] = [0xA2; 32];
    const CENTS_MEMBER: [u8; 32] = [0xC1; 32];
    const QTY_MEMBER: [u8; 32] = [0xC2; 32];
    const UNIT_MEMBER: [u8; 32] = [0xC3; 32];

    fn si64() -> TypeExpr {
        TypeExpr::SInt(IntegerWidth::from_bits(64))
    }

    fn record_field(member: [u8; 32]) -> sley_ssmc::RecordField {
        sley_ssmc::RecordField {
            member_id: MemberId::from_bytes(member),
            value_type: si64(),
            visibility: Visibility::Private,
        }
    }

    fn fixture_definitions() -> Vec<TypeDefinition> {
        vec![
            TypeDefinition {
                entity_id: EntityId::from_bytes(MONEY_DEF),
                type_parameters: vec![],
                form: sley_ssmc::TypeDefForm::Record(vec![record_field(CENTS_MEMBER)]),
                invariants: vec![],
                visibility: Visibility::Private,
            },
            TypeDefinition {
                entity_id: EntityId::from_bytes(LINE_DEF),
                type_parameters: vec![],
                form: sley_ssmc::TypeDefForm::Record(vec![
                    record_field(QTY_MEMBER),
                    record_field(UNIT_MEMBER),
                ]),
                invariants: vec![],
                visibility: Visibility::Private,
            },
        ]
    }

    fn fixture_resolver<'a>(
        types: &'a TypeEnvironment,
        definitions: &'a [TypeDefinition],
    ) -> ConstResolver<'a> {
        ConstResolver {
            types,
            definitions: definitions
                .iter()
                .map(|definition| (definition.entity_id, definition))
                .collect(),
        }
    }

    fn named(definition: [u8; 32]) -> TypeExpr {
        TypeExpr::Named(NamedType {
            definition: EntityId::from_bytes(definition),
            arguments: vec![],
        })
    }

    fn invoice_input_type() -> TypeExpr {
        TypeExpr::Vector(Box::new(named(LINE_DEF)))
    }

    fn money_result_type() -> TypeExpr {
        TypeExpr::Result {
            ok: Box::new(named(MONEY_DEF)),
            error: Box::new(TypeExpr::BuiltinFailure(
                sley_ssmc::BuiltinFailureKind::Arithmetic,
            )),
        }
    }

    fn hex_of(member: &MemberId) -> String {
        super::member_hex(member)
    }

    fn line_fields(quantity: i64, unit: i64) -> serde_json::Map<String, serde_json::Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(
            hex_of(&MemberId::from_bytes(QTY_MEMBER)),
            serde_json::json!({"value": quantity}),
        );
        fields.insert(
            hex_of(&MemberId::from_bytes(UNIT_MEMBER)),
            serde_json::json!({"value": unit}),
        );
        fields
    }

    fn record_item(fields: &serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
        serde_json::json!({"values": [{"fields": fields}]})
    }

    #[test]
    fn nested_record_vector_round_trip() {
        let definitions = fixture_definitions();
        let types = TypeEnvironment::new(definitions.clone()).unwrap();
        let resolver = fixture_resolver(&types, &definitions);
        let item = record_item(&line_fields(2, 1250));
        let value = const_of(&resolver, &invoice_input_type(), &item, 0).unwrap();
        let refs: BTreeMap<EntityId, &TypeDefinition> = definitions
            .iter()
            .map(|definition| (definition.entity_id, definition))
            .collect();
        let summary = const_summary(&refs, &value.data).unwrap();
        let qty_key = hex_of(&MemberId::from_bytes(QTY_MEMBER));
        let unit_key = hex_of(&MemberId::from_bytes(UNIT_MEMBER));
        let line_hex = super::hex_of(&EntityId::from_bytes(LINE_DEF));
        let mut expect_fields = serde_json::Map::new();
        expect_fields.insert(qty_key, serde_json::json!({"SInt": "2"}));
        expect_fields.insert(unit_key, serde_json::json!({"SInt": "1250"}));
        let expect = serde_json::json!({"Sequence": [{
            "Record": {"definition": line_hex, "fields": expect_fields},
        }]});
        assert_eq!(summary, expect);
    }

    #[test]
    fn empty_vector_decodes() {
        let definitions = fixture_definitions();
        let types = TypeEnvironment::new(definitions.clone()).unwrap();
        let resolver = fixture_resolver(&types, &definitions);
        let item = serde_json::json!({"values": []});
        let value = const_of(&resolver, &invoice_input_type(), &item, 0).unwrap();
        assert!(matches!(value.data, ConstData::Sequence(ref items) if items.is_empty()));
    }

    #[test]
    fn missing_member_is_an_error() {
        let definitions = fixture_definitions();
        let types = TypeEnvironment::new(definitions.clone()).unwrap();
        let resolver = fixture_resolver(&types, &definitions);
        let mut fields = serde_json::Map::new();
        fields.insert(
            hex_of(&MemberId::from_bytes(QTY_MEMBER)),
            serde_json::json!({"value": 2}),
        );
        let item = serde_json::json!({"values": [{"fields": fields}]});
        let error = const_of(&resolver, &invoice_input_type(), &item, 0).unwrap_err();
        assert!(error.contains("count"), "unexpected: {error}");
    }

    #[test]
    fn extra_member_is_an_error() {
        let definitions = fixture_definitions();
        let types = TypeEnvironment::new(definitions.clone()).unwrap();
        let resolver = fixture_resolver(&types, &definitions);
        let mut fields = serde_json::Map::new();
        fields.insert(
            hex_of(&MemberId::from_bytes(QTY_MEMBER)),
            serde_json::json!({"value": 2}),
        );
        fields.insert(
            hex_of(&MemberId::from_bytes(UNIT_MEMBER)),
            serde_json::json!({"value": 1250}),
        );
        fields.insert("ff".repeat(32), serde_json::json!({"value": 1}));
        let item = serde_json::json!({"values": [{"fields": fields}]});
        let error = const_of(&resolver, &invoice_input_type(), &item, 0).unwrap_err();
        assert!(error.contains("count"), "unexpected: {error}");
    }

    #[test]
    fn unknown_definition_is_an_error() {
        let definitions = fixture_definitions();
        let types = TypeEnvironment::new(definitions.clone()).unwrap();
        let resolver = fixture_resolver(&types, &definitions);
        let item = serde_json::json!({"fields": {}});
        let expected = named([0xFF; 32]);
        let error = const_of(&resolver, &expected, &item, 0).unwrap_err();
        assert!(
            error.contains("unknown type definition"),
            "unexpected: {error}"
        );
    }

    #[test]
    fn legacy_bits_mismatch_is_an_error() {
        let definitions = fixture_definitions();
        let types = TypeEnvironment::new(definitions.clone()).unwrap();
        let resolver = fixture_resolver(&types, &definitions);
        let item = serde_json::json!({"type": "SInt", "bits": 32, "value": 7});
        let error = const_of(&resolver, &si64(), &item, 0).unwrap_err();
        assert!(error.contains("bits"), "unexpected: {error}");
    }

    #[test]
    fn no_silent_defaults() {
        let definitions = fixture_definitions();
        let types = TypeEnvironment::new(definitions.clone()).unwrap();
        let resolver = fixture_resolver(&types, &definitions);
        for item in [
            serde_json::json!({}),
            serde_json::json!({"value": null}),
            serde_json::json!({"value": true}),
            serde_json::json!({"type": "SInt"}),
            serde_json::json!({"type": "Bool"}),
            serde_json::json!({"values": "nope"}),
        ] {
            let expected = if item.get("type").and_then(|kind| kind.as_str()) == Some("Bool") {
                TypeExpr::Bool
            } else if item.get("values").is_some() {
                invoice_input_type()
            } else {
                si64()
            };
            assert!(
                const_of(&resolver, &expected, &item, 0).is_err(),
                "silent default for {item}"
            );
        }
    }

    #[test]
    fn nested_result_renders_structured() {
        let definitions = fixture_definitions();
        let types = TypeEnvironment::new(definitions.clone()).unwrap();
        let resolver = fixture_resolver(&types, &definitions);
        let owned: BTreeMap<EntityId, &TypeDefinition> = definitions
            .iter()
            .map(|definition| (definition.entity_id, definition))
            .collect();
        let money = ConstValue {
            value_type: named(MONEY_DEF),
            data: ConstData::Record(RecordConst {
                definition: EntityId::from_bytes(MONEY_DEF),
                fields: vec![FieldConst {
                    member_id: MemberId::from_bytes(CENTS_MEMBER),
                    value: ConstValue {
                        value_type: si64(),
                        data: ConstData::SInt(2681),
                    },
                }],
            }),
        };
        let nested = ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
            value_type: TypeExpr::Vector(Box::new(named(MONEY_DEF))),
            data: ConstData::Sequence(vec![money]),
        })));
        let summary = const_summary(&owned, &nested).unwrap();
        let money_hex = super::hex_of(&EntityId::from_bytes(MONEY_DEF));
        let cents_key = hex_of(&MemberId::from_bytes(CENTS_MEMBER));
        let mut expect_fields = serde_json::Map::new();
        expect_fields.insert(cents_key.clone(), serde_json::json!({"SInt": "2681"}));
        let expect = serde_json::json!({"Result": {"Ok": {"Sequence": [{
            "Record": {"definition": money_hex.clone(), "fields": expect_fields},
        }]}}});
        assert_eq!(summary, expect);
        let mut ok_fields = serde_json::Map::new();
        ok_fields.insert(cents_key.clone(), serde_json::json!({"value": 2681}));
        let ok_item = serde_json::json!({"ok": {"fields": ok_fields}});
        let decoded = const_of(&resolver, &money_result_type(), &ok_item, 0).unwrap();
        let mut expect_record_fields = serde_json::Map::new();
        expect_record_fields.insert(cents_key, serde_json::json!({"SInt": "2681"}));
        assert_eq!(
            const_summary(&owned, &decoded.data).unwrap(),
            serde_json::json!({"Result": {"Ok": {
                "Record": {"definition": money_hex, "fields": expect_record_fields},
            }}})
        );
    }

    #[test]
    fn depth_bound_rejects_deep_nesting() {
        let definitions = fixture_definitions();
        let types = TypeEnvironment::new(definitions.clone()).unwrap();
        let resolver = fixture_resolver(&types, &definitions);
        // Nine nested vectors exceed the depth bound of eight.
        let mut item = serde_json::json!({"values": []});
        for _ in 0..9 {
            item = serde_json::json!({"values": [item]});
        }
        let expected = nested_vector(si64(), 10);
        let error = const_of(&resolver, &expected, &item, 0).unwrap_err();
        assert!(error.contains("nesting"), "unexpected: {error}");
    }

    fn nested_vector(inner: TypeExpr, depth: u32) -> TypeExpr {
        if depth == 0 {
            inner
        } else {
            TypeExpr::Vector(Box::new(nested_vector(inner, depth - 1)))
        }
    }
}
