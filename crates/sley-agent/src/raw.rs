//! The raw operation path: mutation operations in the 2.0.0 trial-tool JSON
//! form (`{"class", "kind", "target", "payload"}` with union values as
//! `{"variant", "value"}`), compiled to the same record AF1 produces.
//!
//! Two conveniences remove the old two-pass propose/compose: a create may
//! carry `"key"`, and any identity position may say `"@key"` for the
//! identity that create derives; identities may also be local names.
//! Types accept AF1 shorthand strings as well as union objects.

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use sley_id::{CandidateNonce, EntityId};
use sley_mutate::MutationPayload;
use sley_mutate::value::{
    BlockBody, ConstantBody, EntityBodyValue, EntityIdSet, FunctionBody, NamespaceBody,
    OperationBody, ParameterBody, TestCaseBody, TypeDefBody,
};
use sley_ssmc::{
    BranchTerminator, BuiltinCase, BuiltinFailureKind, BuiltinFailureValue, CaseKey,
    CondBranchTerminator, ConstData, ConstValue, EffectEnvironment, ExpectedOutcome, FieldConst,
    FunctionRefValue, FunctionType, Immediate, IntegerWidth, MapEntryConst, MemberId, NamedType,
    OperationResultRef, ParameterRole, Reachability, RecordConst, RecordField, ResourceLimits,
    ResultConst, ReturnTerminator, SwitchArgument, SwitchCase, SwitchEdge, TargetEdge, Terminator,
    TrapCode, TrapTerminator, TypeDefForm, TypeExpr, TypeParameterDef, ValueRef, VariantCase,
    VariantConst, VariantImmediate, VariantSwitchTerminator, Visibility,
};

use crate::candidate::PlannedOp;
use crate::error::{Result, frame};
use crate::hex;
use crate::names::{NameMap, Names};
use crate::types::TypeNames;
use crate::workspace::Program;

/// Live entities the list deletes by name or id: a create may take the name
/// of one of these, and of no other live top-level entity.
fn deleted_targets(names: &Names, list: &[Value]) -> Vec<EntityId> {
    list.iter()
        .filter(|op| op.get("class").and_then(Value::as_str) == Some("DeleteEntityBinding"))
        .filter_map(|op| op.get("target").and_then(Value::as_str))
        .filter_map(|target| names.resolve(target))
        .collect()
}

/// A top-level key names the new entity, so it cannot be the name of a live
/// top-level entity the list keeps (AF1's rule: that would silently rename
/// the live one).
fn check_key(names: &Names, key: &str, deleted: &[EntityId], pointer: &str) -> Result<()> {
    if !key.contains('.')
        && let Some(live) = names.resolve(key)
        && names.scope(&live) == crate::names::Scope::Top
        && !deleted.contains(&live)
    {
        return Err(frame(
            &format!("{pointer}/key"),
            format!(
                "`{key}` already names a live entity; use another key, or delete `{key}` in the same list"
            ),
        ));
    }
    Ok(())
}

/// Compiles a raw operation list.
///
/// # Errors
///
/// `AGENT_FRAME_INVALID` with a JSON-pointer locator.
///
/// # Panics
///
/// Never: the second pass re-reads fields the first pass checked.
pub fn compile(
    program: &Program,
    names: &Names,
    ops: &Value,
    nonce: CandidateNonce,
) -> Result<(Vec<PlannedOp>, NameMap)> {
    let list = ops
        .as_array()
        .ok_or_else(|| frame("", "raw operations are a JSON array"))?;
    let mut keys = BTreeMap::new();
    let mut new_names = NameMap::default();
    let mut ordinal = 0_u64;
    let mut targets = Vec::with_capacity(list.len());
    let deleted = deleted_targets(names, list);
    // Pass 1: derive create identities in create order.
    for (index, op) in list.iter().enumerate() {
        let pointer = format!("/{index}");
        let object = op
            .as_object()
            .ok_or_else(|| frame(&pointer, "an operation is an object"))?;
        let class = object
            .get("class")
            .and_then(Value::as_str)
            .ok_or_else(|| frame(&pointer, "missing \"class\""))?;
        let kind = object
            .get("kind")
            .and_then(Value::as_u64)
            .and_then(|kind| u16::try_from(kind).ok())
            .ok_or_else(|| frame(&pointer, "missing \"kind\""))?;
        if class == "CreateEntity" {
            let id = EntityId::derive(program.workspace(), nonce, u32::from(kind), ordinal);
            ordinal += 1;
            if let Some(key) = object.get("key").and_then(Value::as_str) {
                if keys.insert(key.to_owned(), id).is_some() {
                    return Err(frame(
                        &format!("{pointer}/key"),
                        format!("key `{key}` is used twice"),
                    ));
                }
                check_key(names, key, &deleted, &pointer)?;
                // `fn.block.op` keys name the entity by their last segment.
                let leaf = key.rsplit('.').next().unwrap_or(key);
                if crate::names::is_identifier(leaf) {
                    new_names.insert(*id.as_bytes(), leaf);
                }
            }
            targets.push(Some(id));
        } else {
            targets.push(None);
        }
    }
    let reader = Reader { names, keys: &keys };
    let mut planned = Vec::with_capacity(list.len());
    for (index, op) in list.iter().enumerate() {
        let pointer = format!("/{index}");
        let object = op.as_object().expect("checked");
        let class = object["class"].as_str().expect("checked");
        let kind = u16::try_from(object["kind"].as_u64().expect("checked")).expect("checked");
        let (target, payload) = match class {
            "CreateEntity" => (
                targets[index].expect("derived"),
                MutationPayload::CreateEntity(reader.body(
                    kind,
                    object.get("payload"),
                    &format!("{pointer}/payload"),
                )?),
            ),
            "ReplaceEntityVersion" => (
                reader.id(
                    object.get("target").unwrap_or(&Value::Null),
                    &format!("{pointer}/target"),
                )?,
                MutationPayload::ReplaceEntityVersion(reader.body(
                    kind,
                    object.get("payload"),
                    &format!("{pointer}/payload"),
                )?),
            ),
            "DeleteEntityBinding" => (
                reader.id(
                    object.get("target").unwrap_or(&Value::Null),
                    &format!("{pointer}/target"),
                )?,
                MutationPayload::DeleteEntityBinding,
            ),
            other => {
                return Err(frame(
                    &format!("{pointer}/class"),
                    format!(
                        "`{other}`: the raw path takes CreateEntity, ReplaceEntityVersion, and DeleteEntityBinding"
                    ),
                ));
            }
        };
        if class != "CreateEntity" && !program.contains(&target) {
            return Err(frame(&format!("{pointer}/target"), "not a live entity"));
        }
        planned.push(PlannedOp {
            kind,
            target,
            payload,
            field_tag: None,
        });
    }
    Ok((planned, new_names))
}

struct Reader<'a> {
    names: &'a Names,
    keys: &'a BTreeMap<String, EntityId>,
}

fn field<'v>(object: &'v Map<String, Value>, key: &str, pointer: &str) -> Result<&'v Value> {
    object
        .get(key)
        .ok_or_else(|| frame(pointer, format!("missing \"{key}\"")))
}

fn record<'v>(value: &'v Value, pointer: &str) -> Result<&'v Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| frame(pointer, "expected an object"))
}

fn list<'v>(value: &'v Value, pointer: &str) -> Result<&'v Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| frame(pointer, "expected an array"))
}

fn number(value: &Value, pointer: &str) -> Result<u64> {
    match value {
        Value::Number(n) => n.as_u64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
    .ok_or_else(|| frame(pointer, "expected a non-negative integer"))
}

fn u32_of(value: &Value, pointer: &str) -> Result<u32> {
    u32::try_from(number(value, pointer)?).map_err(|_| frame(pointer, "does not fit u32"))
}

/// A union value: `{"variant": name, "value"?: payload}`.
fn union<'v>(value: &'v Value, pointer: &str) -> Result<(&'v str, Option<&'v Value>)> {
    let object = record(value, pointer)?;
    let variant = object
        .get("variant")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            frame(
                pointer,
                "a union is {\"variant\": name, \"value\": payload}",
            )
        })?;
    Ok((variant, object.get("value")))
}

fn payload<'v>(value: Option<&'v Value>, pointer: &str) -> Result<&'v Value> {
    value.ok_or_else(|| frame(pointer, "missing \"value\""))
}

impl TypeNames for Reader<'_> {
    fn type_definition(&self, name: &str) -> Option<EntityId> {
        self.keys
            .get(name)
            .copied()
            .or_else(|| self.names.resolve(name))
    }
}

impl Reader<'_> {
    fn id(&self, value: &Value, pointer: &str) -> Result<EntityId> {
        let text = value
            .as_str()
            .ok_or_else(|| frame(pointer, "an identity is 64 hex, \"@key\", or a name"))?;
        if let Some(key) = text.strip_prefix('@') {
            return self
                .keys
                .get(key)
                .copied()
                .ok_or_else(|| frame(pointer, format!("no create with key `{key}`")));
        }
        if let Some(bytes) = hex::decode32(text) {
            return Ok(EntityId::from_bytes(bytes));
        }
        self.names
            .resolve(text)
            .ok_or_else(|| frame(pointer, format!("no entity named `{text}`")))
    }

    fn ids(&self, value: &Value, pointer: &str) -> Result<Vec<EntityId>> {
        list(value, pointer)?
            .iter()
            .enumerate()
            .map(|(index, item)| self.id(item, &format!("{pointer}/{index}")))
            .collect()
    }

    fn set(&self, value: &Value, pointer: &str) -> Result<EntityIdSet> {
        let mut ids = self.ids(value, pointer)?;
        ids.sort_unstable();
        EntityIdSet::from_unsorted(ids)
            .map_err(|_| frame(pointer, "a set must not repeat an identity"))
    }

    fn member(&self, value: &Value, pointer: &str) -> Result<MemberId> {
        let text = value
            .as_str()
            .ok_or_else(|| frame(pointer, "a member is 64 hex or Type.Member"))?;
        if let Some(bytes) = hex::decode32(text) {
            return Ok(MemberId::from_bytes(bytes));
        }
        self.names
            .resolve_member(text)
            .map(|(_, member)| member)
            .ok_or_else(|| frame(pointer, format!("no member `{text}`")))
    }

    fn visibility(value: Option<&Value>, pointer: &str) -> Result<Visibility> {
        Ok(match value.and_then(Value::as_str).unwrap_or("Exported") {
            "Private" => Visibility::Private,
            "Package" => Visibility::Package,
            "Workspace" => Visibility::Workspace,
            "Exported" => Visibility::Exported,
            other => return Err(frame(pointer, format!("unknown visibility `{other}`"))),
        })
    }

    #[allow(clippy::too_many_lines)]
    fn ty(&self, value: &Value, pointer: &str) -> Result<TypeExpr> {
        if value.is_string() {
            return crate::types::read(value, self, pointer);
        }
        let (variant, inner) = union(value, pointer)?;
        let at = format!("{pointer}/value");
        let width = |inner: Option<&Value>| -> Result<IntegerWidth> {
            let bits = u16::try_from(number(payload(inner, pointer)?, &at)?)
                .map_err(|_| frame(&at, "bad width"))?;
            Ok(IntegerWidth::from_bits(bits))
        };
        Ok(match variant {
            "Unit" => TypeExpr::Unit,
            "Bool" => TypeExpr::Bool,
            "SInt" => TypeExpr::SInt(width(inner)?),
            "UInt" => TypeExpr::UInt(width(inner)?),
            "F32" => TypeExpr::F32,
            "F64" => TypeExpr::F64,
            "Bytes" => TypeExpr::Bytes,
            "Text" => TypeExpr::Text,
            "Tuple" => TypeExpr::Tuple(self.types(payload(inner, pointer)?, &at)?),
            "Named" => {
                let object = record(payload(inner, pointer)?, &at)?;
                TypeExpr::Named(NamedType {
                    definition: self.id(
                        field(object, "definition", &at)?,
                        &format!("{at}/definition"),
                    )?,
                    arguments: match object.get("arguments") {
                        Some(arguments) => self.types(arguments, &format!("{at}/arguments"))?,
                        None => Vec::new(),
                    },
                })
            }
            "Vector" => TypeExpr::Vector(Box::new(self.ty(payload(inner, pointer)?, &at)?)),
            "Option" => TypeExpr::Option(Box::new(self.ty(payload(inner, pointer)?, &at)?)),
            "LocalCell" => TypeExpr::LocalCell(Box::new(self.ty(payload(inner, pointer)?, &at)?)),
            "OrderedMap" => {
                let object = record(payload(inner, pointer)?, &at)?;
                TypeExpr::OrderedMap {
                    key: Box::new(self.ty(field(object, "key", &at)?, &format!("{at}/key"))?),
                    value: Box::new(self.ty(field(object, "value", &at)?, &format!("{at}/value"))?),
                }
            }
            "Result" => {
                let object = record(payload(inner, pointer)?, &at)?;
                TypeExpr::Result {
                    ok: Box::new(self.ty(field(object, "ok", &at)?, &format!("{at}/ok"))?),
                    error: Box::new(self.ty(field(object, "error", &at)?, &format!("{at}/error"))?),
                }
            }
            "FunctionRef" => {
                let object = record(payload(inner, pointer)?, &at)?;
                TypeExpr::FunctionRef(FunctionType {
                    parameters: self.types(
                        field(object, "parameters", &at)?,
                        &format!("{at}/parameters"),
                    )?,
                    result: Box::new(
                        self.ty(field(object, "result", &at)?, &format!("{at}/result"))?,
                    ),
                    effects: match object.get("effects") {
                        Some(effects) => self.set(effects, &format!("{at}/effects"))?.into_vec(),
                        None => Vec::new(),
                    },
                })
            }
            "AdapterHandle" => TypeExpr::AdapterHandle(self.id(payload(inner, pointer)?, &at)?),
            "CapabilityToken" => TypeExpr::CapabilityToken(self.id(payload(inner, pointer)?, &at)?),
            "TypeParameter" => TypeExpr::TypeParameter(u32_of(payload(inner, pointer)?, &at)?),
            "BuiltinFailure" => {
                TypeExpr::BuiltinFailure(failure_kind(payload(inner, pointer)?, &at)?)
            }
            other => return Err(frame(pointer, format!("unknown type variant `{other}`"))),
        })
    }

    fn types(&self, value: &Value, pointer: &str) -> Result<Vec<TypeExpr>> {
        list(value, pointer)?
            .iter()
            .enumerate()
            .map(|(index, item)| self.ty(item, &format!("{pointer}/{index}")))
            .collect()
    }

    fn const_value(&self, value: &Value, pointer: &str) -> Result<ConstValue> {
        let object = record(value, pointer)?;
        Ok(ConstValue {
            value_type: self.ty(
                field(object, "value_type", pointer)?,
                &format!("{pointer}/value_type"),
            )?,
            data: self.const_data(field(object, "data", pointer)?, &format!("{pointer}/data"))?,
        })
    }

    fn const_values(&self, value: &Value, pointer: &str) -> Result<Vec<ConstValue>> {
        list(value, pointer)?
            .iter()
            .enumerate()
            .map(|(index, item)| self.const_value(item, &format!("{pointer}/{index}")))
            .collect()
    }

    #[allow(clippy::too_many_lines)]
    fn const_data(&self, value: &Value, pointer: &str) -> Result<ConstData> {
        let (variant, inner) = union(value, pointer)?;
        let at = format!("{pointer}/value");
        let integer = |inner: Option<&Value>| -> Result<i128> {
            match payload(inner, pointer)? {
                Value::Number(n) => n
                    .as_i64()
                    .map(i128::from)
                    .or_else(|| n.as_u64().map(i128::from)),
                Value::String(text) => text.parse().ok(),
                _ => None,
            }
            .ok_or_else(|| frame(&at, "expected an integer"))
        };
        Ok(match variant {
            "Unit" => ConstData::Unit,
            "Bool" => ConstData::Bool(
                payload(inner, pointer)?
                    .as_bool()
                    .ok_or_else(|| frame(&at, "expected a boolean"))?,
            ),
            "SInt" => ConstData::SInt(integer(inner)?),
            "UInt" => ConstData::UInt(
                u128::try_from(integer(inner)?).map_err(|_| frame(&at, "negative"))?,
            ),
            "F32Bits" => {
                let bytes = hex::decode(payload(inner, pointer)?.as_str().unwrap_or(""))
                    .ok_or_else(|| frame(&at, "hex bits"))?;
                ConstData::F32Bits(u32::from_be_bytes(
                    bytes.try_into().map_err(|_| frame(&at, "4 bytes"))?,
                ))
            }
            "F64Bits" => {
                let bytes = hex::decode(payload(inner, pointer)?.as_str().unwrap_or(""))
                    .ok_or_else(|| frame(&at, "hex bits"))?;
                ConstData::F64Bits(u64::from_be_bytes(
                    bytes.try_into().map_err(|_| frame(&at, "8 bytes"))?,
                ))
            }
            "Bytes" => ConstData::Bytes(
                hex::decode(payload(inner, pointer)?.as_str().unwrap_or(""))
                    .ok_or_else(|| frame(&at, "expected hex"))?,
            ),
            "Text" => ConstData::Text(
                payload(inner, pointer)?
                    .as_str()
                    .ok_or_else(|| frame(&at, "expected text"))?
                    .to_owned(),
            ),
            "Sequence" => ConstData::Sequence(self.const_values(payload(inner, pointer)?, &at)?),
            "Record" => {
                let object = record(payload(inner, pointer)?, &at)?;
                ConstData::Record(RecordConst {
                    definition: self.id(
                        field(object, "definition", &at)?,
                        &format!("{at}/definition"),
                    )?,
                    fields: list(field(object, "fields", &at)?, &format!("{at}/fields"))?
                        .iter()
                        .enumerate()
                        .map(|(index, item)| {
                            let item_at = format!("{at}/fields/{index}");
                            let item = record(item, &item_at)?;
                            Ok(FieldConst {
                                member_id: self
                                    .member(field(item, "member_id", &item_at)?, &item_at)?,
                                value: self
                                    .const_value(field(item, "value", &item_at)?, &item_at)?,
                            })
                        })
                        .collect::<Result<_>>()?,
                })
            }
            "Variant" => {
                let object = record(payload(inner, pointer)?, &at)?;
                let payload_value = match object.get("payload") {
                    None => None,
                    Some(option) => match union(option, &format!("{at}/payload"))? {
                        ("None", _) => None,
                        ("Some", Some(inner)) => Some(Box::new(
                            self.const_value(inner, &format!("{at}/payload/value"))?,
                        )),
                        _ => return Err(frame(&format!("{at}/payload"), "expected None or Some")),
                    },
                };
                ConstData::Variant(VariantConst {
                    definition: self.id(
                        field(object, "definition", &at)?,
                        &format!("{at}/definition"),
                    )?,
                    member_id: self
                        .member(field(object, "member_id", &at)?, &format!("{at}/member_id"))?,
                    payload: payload_value,
                })
            }
            "Map" => ConstData::Map(
                list(payload(inner, pointer)?, &at)?
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let item_at = format!("{at}/{index}");
                        let item = record(item, &item_at)?;
                        Ok(MapEntryConst {
                            key: self.const_value(field(item, "key", &item_at)?, &item_at)?,
                            value: self.const_value(field(item, "value", &item_at)?, &item_at)?,
                        })
                    })
                    .collect::<Result<_>>()?,
            ),
            "Option" => match union(payload(inner, pointer)?, &at)? {
                ("None", _) => ConstData::Option(None),
                ("Some", Some(inner)) => ConstData::Option(Some(Box::new(
                    self.const_value(inner, &format!("{at}/value"))?,
                ))),
                _ => return Err(frame(&at, "expected None or Some")),
            },
            "Result" => match union(payload(inner, pointer)?, &at)? {
                ("Ok", Some(inner)) => ConstData::Result(ResultConst::Ok(Box::new(
                    self.const_value(inner, &format!("{at}/value"))?,
                ))),
                ("Err", Some(inner)) => ConstData::Result(ResultConst::Err(Box::new(
                    self.const_value(inner, &format!("{at}/value"))?,
                ))),
                _ => return Err(frame(&at, "expected Ok or Err")),
            },
            "FunctionRef" => {
                ConstData::FunctionRef(self.function_ref(payload(inner, pointer)?, &at)?)
            }
            "BuiltinFailure" => {
                let object = record(payload(inner, pointer)?, &at)?;
                ConstData::BuiltinFailure(BuiltinFailureValue {
                    kind: failure_kind(field(object, "kind", &at)?, &format!("{at}/kind"))?,
                    code: u16::try_from(number(
                        field(object, "code", &at)?,
                        &format!("{at}/code"),
                    )?)
                    .map_err(|_| frame(&at, "code"))?,
                })
            }
            other => return Err(frame(pointer, format!("unknown data variant `{other}`"))),
        })
    }

    fn function_ref(&self, value: &Value, pointer: &str) -> Result<FunctionRefValue> {
        let object = record(value, pointer)?;
        Ok(FunctionRefValue {
            function: self.id(
                field(object, "function", pointer)?,
                &format!("{pointer}/function"),
            )?,
            type_arguments: match object.get("type_arguments") {
                Some(arguments) => self.types(arguments, &format!("{pointer}/type_arguments"))?,
                None => Vec::new(),
            },
        })
    }

    fn value_ref(&self, value: &Value, pointer: &str) -> Result<ValueRef> {
        let (variant, inner) = union(value, pointer)?;
        let at = format!("{pointer}/value");
        Ok(match variant {
            "Parameter" => ValueRef::Parameter(self.id(payload(inner, pointer)?, &at)?),
            "OperationResult" => {
                let object = record(payload(inner, pointer)?, &at)?;
                ValueRef::OperationResult(OperationResultRef {
                    operation: self
                        .id(field(object, "operation", &at)?, &format!("{at}/operation"))?,
                    result_index: match object.get("result_index") {
                        Some(index) => u32_of(index, &format!("{at}/result_index"))?,
                        None => 0,
                    },
                })
            }
            other => return Err(frame(pointer, format!("unknown value variant `{other}`"))),
        })
    }

    fn value_refs(&self, value: &Value, pointer: &str) -> Result<Vec<ValueRef>> {
        list(value, pointer)?
            .iter()
            .enumerate()
            .map(|(index, item)| self.value_ref(item, &format!("{pointer}/{index}")))
            .collect()
    }

    fn edge(&self, value: &Value, pointer: &str) -> Result<TargetEdge> {
        let object = record(value, pointer)?;
        Ok(TargetEdge {
            target: self.id(
                field(object, "target", pointer)?,
                &format!("{pointer}/target"),
            )?,
            arguments: match object.get("arguments") {
                Some(arguments) => self.value_refs(arguments, &format!("{pointer}/arguments"))?,
                None => Vec::new(),
            },
        })
    }

    #[allow(clippy::too_many_lines)]
    fn terminator(&self, value: &Value, pointer: &str) -> Result<Terminator> {
        let (variant, inner) = union(value, pointer)?;
        let at = format!("{pointer}/value");
        let object = || record(payload(inner, pointer)?, &at);
        Ok(match variant {
            "Return" => Terminator::Return(ReturnTerminator {
                value: self.value_ref(field(object()?, "value", &at)?, &format!("{at}/value"))?,
            }),
            "Branch" => Terminator::Branch(BranchTerminator {
                edge: self.edge(field(object()?, "edge", &at)?, &format!("{at}/edge"))?,
            }),
            "CondBranch" => {
                let object = object()?;
                Terminator::CondBranch(CondBranchTerminator {
                    condition: self
                        .value_ref(field(object, "condition", &at)?, &format!("{at}/condition"))?,
                    if_true: self.edge(field(object, "if_true", &at)?, &format!("{at}/if_true"))?,
                    if_false: self
                        .edge(field(object, "if_false", &at)?, &format!("{at}/if_false"))?,
                })
            }
            "VariantSwitch" => {
                let object = object()?;
                let mut cases = Vec::new();
                for (index, case) in list(field(object, "cases", &at)?, &format!("{at}/cases"))?
                    .iter()
                    .enumerate()
                {
                    let case_at = format!("{at}/cases/{index}");
                    let case = record(case, &case_at)?;
                    let key = match union(field(case, "case_key", &case_at)?, &case_at)? {
                        ("Builtin", Some(Value::String(name))) => {
                            CaseKey::Builtin(match name.as_str() {
                                "None" => BuiltinCase::None,
                                "Some" => BuiltinCase::Some,
                                "Ok" => BuiltinCase::Ok,
                                "Err" => BuiltinCase::Err,
                                other => {
                                    return Err(frame(
                                        &case_at,
                                        format!("unknown builtin case `{other}`"),
                                    ));
                                }
                            })
                        }
                        ("Member", Some(member)) => CaseKey::Member(self.member(member, &case_at)?),
                        _ => return Err(frame(&case_at, "a case key is Builtin or Member")),
                    };
                    let edge = record(field(case, "edge", &case_at)?, &case_at)?;
                    let arguments = match edge.get("arguments") {
                        Some(arguments) => list(arguments, &case_at)?
                            .iter()
                            .map(|argument| match union(argument, &case_at)? {
                                ("CasePayload", _) => Ok(SwitchArgument::CasePayload),
                                ("Value", Some(value)) => {
                                    Ok(SwitchArgument::Value(self.value_ref(value, &case_at)?))
                                }
                                _ => Err(frame(&case_at, "an argument is Value or CasePayload")),
                            })
                            .collect::<Result<_>>()?,
                        None => Vec::new(),
                    };
                    cases.push(SwitchCase {
                        case_key: key,
                        edge: SwitchEdge {
                            target: self.id(
                                field(edge, "target", &case_at)?,
                                &format!("{case_at}/edge/target"),
                            )?,
                            arguments,
                        },
                    });
                }
                Terminator::VariantSwitch(VariantSwitchTerminator {
                    value: self.value_ref(field(object, "value", &at)?, &format!("{at}/value"))?,
                    cases,
                })
            }
            "Trap" => {
                let object = object()?;
                Terminator::Trap(TrapTerminator {
                    code: match object
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or("Unreachable")
                    {
                        "Unreachable" => TrapCode::Unreachable,
                        "ResourceExhausted" => TrapCode::ResourceExhausted,
                        "AdapterContractViolation" => TrapCode::AdapterContractViolation,
                        "InternalInvariant" => TrapCode::InternalInvariant,
                        other => return Err(frame(&at, format!("unknown trap code `{other}`"))),
                    },
                    payload: match object.get("payload") {
                        None => None,
                        Some(option) => match union(option, &at)? {
                            ("None", _) => None,
                            ("Some", Some(value)) => Some(self.value_ref(value, &at)?),
                            _ => return Err(frame(&at, "expected None or Some")),
                        },
                    },
                })
            }
            other => return Err(frame(pointer, format!("unknown terminator `{other}`"))),
        })
    }

    fn immediate(&self, value: &Value, pointer: &str) -> Result<Immediate> {
        let (variant, inner) = union(value, pointer)?;
        let at = format!("{pointer}/value");
        Ok(match variant {
            "None" => Immediate::None,
            "Entity" => Immediate::Entity(self.id(payload(inner, pointer)?, &at)?),
            "Index" => Immediate::Index(u32_of(payload(inner, pointer)?, &at)?),
            "Field" => Immediate::Field(self.member(payload(inner, pointer)?, &at)?),
            "Variant" => {
                let object = record(payload(inner, pointer)?, &at)?;
                Immediate::Variant(VariantImmediate {
                    definition: self.id(
                        field(object, "definition", &at)?,
                        &format!("{at}/definition"),
                    )?,
                    member_id: self
                        .member(field(object, "member_id", &at)?, &format!("{at}/member_id"))?,
                })
            }
            "Observation" => Immediate::Observation(
                hex::decode32(payload(inner, pointer)?.as_str().unwrap_or(""))
                    .ok_or_else(|| frame(&at, "64 hex"))?,
            ),
            "Function" => Immediate::Function(self.function_ref(payload(inner, pointer)?, &at)?),
            other => return Err(frame(pointer, format!("unknown immediate `{other}`"))),
        })
    }

    #[allow(clippy::too_many_lines)]
    fn body(&self, kind: u16, value: Option<&Value>, pointer: &str) -> Result<EntityBodyValue> {
        let value = value.ok_or_else(|| frame(pointer, "missing payload"))?;
        let object = record(value, pointer)?;
        let get = |key: &str| field(object, key, pointer);
        let at = |key: &str| format!("{pointer}/{key}");
        Ok(match kind {
            3 => EntityBodyValue::Namespace(NamespaceBody {
                parent: match union(get("parent")?, &at("parent"))? {
                    ("None", _) => None,
                    ("Some", Some(parent)) => Some(self.id(parent, &at("parent"))?),
                    _ => return Err(frame(&at("parent"), "expected None or Some")),
                },
                members: self.set(get("members")?, &at("members"))?,
            }),
            4 => {
                let form = match union(get("form")?, &at("form"))? {
                    ("Variant", Some(cases)) => TypeDefForm::Variant(
                        list(cases, &at("form"))?
                            .iter()
                            .enumerate()
                            .map(|(index, case)| {
                                let case_at = format!("{pointer}/form/value/{index}");
                                let case = record(case, &case_at)?;
                                Ok(VariantCase {
                                    member_id: self
                                        .member(field(case, "member_id", &case_at)?, &case_at)?,
                                    payload_type: match case.get("payload_type") {
                                        None => None,
                                        Some(option) => match union(option, &case_at)? {
                                            ("None", _) => None,
                                            ("Some", Some(ty)) => Some(self.ty(ty, &case_at)?),
                                            _ => {
                                                return Err(frame(
                                                    &case_at,
                                                    "expected None or Some",
                                                ));
                                            }
                                        },
                                    },
                                })
                            })
                            .collect::<Result<_>>()?,
                    ),
                    ("Record", Some(fields)) => TypeDefForm::Record(
                        list(fields, &at("form"))?
                            .iter()
                            .enumerate()
                            .map(|(index, entry)| {
                                let entry_at = format!("{pointer}/form/value/{index}");
                                let entry = record(entry, &entry_at)?;
                                Ok(RecordField {
                                    member_id: self
                                        .member(field(entry, "member_id", &entry_at)?, &entry_at)?,
                                    value_type: self
                                        .ty(field(entry, "value_type", &entry_at)?, &entry_at)?,
                                    visibility: Self::visibility(
                                        entry.get("visibility"),
                                        &entry_at,
                                    )?,
                                })
                            })
                            .collect::<Result<_>>()?,
                    ),
                    _ => return Err(frame(&at("form"), "a form is Variant or Record")),
                };
                EntityBodyValue::TypeDef(TypeDefBody {
                    type_parameters: type_parameters(
                        object.get("type_parameters"),
                        &at("type_parameters"),
                    )?,
                    form,
                    invariants: match object.get("invariants") {
                        Some(set) => self.set(set, &at("invariants"))?,
                        None => EntityIdSet::from_unsorted(Vec::new()).expect("empty"),
                    },
                    visibility: Self::visibility(object.get("visibility"), &at("visibility"))?,
                })
            }
            5 => EntityBodyValue::Function(FunctionBody {
                type_parameters: type_parameters(
                    object.get("type_parameters"),
                    &at("type_parameters"),
                )?,
                parameters: self.ids(get("parameters")?, &at("parameters"))?,
                result_type: self.ty(get("result_type")?, &at("result_type"))?,
                effects: match object.get("effects") {
                    Some(set) => self.set(set, &at("effects"))?,
                    None => EntityIdSet::from_unsorted(Vec::new()).expect("empty"),
                },
                entry_block: self.id(get("entry_block")?, &at("entry_block"))?,
                blocks: self.ids(get("blocks")?, &at("blocks"))?,
                contracts: match object.get("contracts") {
                    Some(set) => self.set(set, &at("contracts"))?,
                    None => EntityIdSet::from_unsorted(Vec::new()).expect("empty"),
                },
                visibility: Self::visibility(object.get("visibility"), &at("visibility"))?,
            }),
            6 => EntityBodyValue::Parameter(ParameterBody {
                owner: self.id(get("owner")?, &at("owner"))?,
                role: match get("role")?.as_str() {
                    Some("Function") => ParameterRole::Function,
                    Some("Block") => ParameterRole::Block,
                    _ => return Err(frame(&at("role"), "Function or Block")),
                },
                ordinal: u32_of(get("ordinal")?, &at("ordinal"))?,
                value_type: self.ty(get("value_type")?, &at("value_type"))?,
            }),
            7 => EntityBodyValue::Block(BlockBody {
                function: self.id(get("function")?, &at("function"))?,
                parameters: self.ids(get("parameters")?, &at("parameters"))?,
                operations: self.ids(get("operations")?, &at("operations"))?,
                terminator: self.terminator(get("terminator")?, &at("terminator"))?,
                reachability: match object
                    .get("reachability")
                    .and_then(Value::as_str)
                    .unwrap_or("Required")
                {
                    "Required" => Reachability::Required,
                    "ExplicitlyUnreachable" => Reachability::ExplicitlyUnreachable,
                    _ => {
                        return Err(frame(
                            &at("reachability"),
                            "Required or ExplicitlyUnreachable",
                        ));
                    }
                },
            }),
            8 => EntityBodyValue::Operation(OperationBody {
                block: self.id(get("block")?, &at("block"))?,
                ordinal: u32_of(get("ordinal")?, &at("ordinal"))?,
                opcode: u32_of(get("opcode")?, &at("opcode"))?,
                operands: self.value_refs(get("operands")?, &at("operands"))?,
                result_types: self.types(get("result_types")?, &at("result_types"))?,
                immediate: match object.get("immediate") {
                    Some(immediate) => self.immediate(immediate, &at("immediate"))?,
                    None => Immediate::None,
                },
            }),
            9 => EntityBodyValue::Constant(ConstantBody {
                value: self.const_value(get("value")?, &at("value"))?,
            }),
            14 => EntityBodyValue::TestCase(TestCaseBody {
                target: self.id(get("target")?, &at("target"))?,
                inputs: self.const_values(get("inputs")?, &at("inputs"))?,
                effect_environment: EffectEnvironment::Replay(Vec::new()),
                expected: match union(get("expected")?, &at("expected"))? {
                    ("Value", Some(value)) => {
                        ExpectedOutcome::Value(self.const_value(value, &at("expected"))?)
                    }
                    ("FailureCode", Some(code)) => {
                        ExpectedOutcome::FailureCode(u32_of(code, &at("expected"))?)
                    }
                    _ => return Err(frame(&at("expected"), "Value or FailureCode")),
                },
                observations: Vec::new(),
                resource_limits: resource_limits(get("resource_limits")?, &at("resource_limits"))?,
            }),
            other => {
                return Err(frame(
                    pointer,
                    format!("kind {other} is not supported on the raw path (use kinds 3-9 or 14)"),
                ));
            }
        })
    }
}

fn type_parameters(value: Option<&Value>, pointer: &str) -> Result<Vec<TypeParameterDef>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    list(value, pointer)?
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let at = format!("{pointer}/{index}");
            let ordinal = match item {
                Value::Object(object) => u32_of(field(object, "ordinal", &at)?, &at)?,
                other => u32_of(other, &at)?,
            };
            Ok(TypeParameterDef { ordinal })
        })
        .collect()
}

fn resource_limits(value: &Value, pointer: &str) -> Result<ResourceLimits> {
    let object = record(value, pointer)?;
    let get = |key: &str| {
        field(object, key, pointer).and_then(|value| number(value, &format!("{pointer}/{key}")))
    };
    Ok(ResourceLimits {
        fuel: get("fuel")?,
        memory_bytes: get("memory_bytes")?,
        output_bytes: get("output_bytes")?,
        effect_count: get("effect_count")?,
        call_depth: get("call_depth")?,
        wall_timeout_millis: get("wall_timeout_millis")?,
    })
}

fn failure_kind(value: &Value, pointer: &str) -> Result<BuiltinFailureKind> {
    Ok(match value {
        Value::String(name) => match name.as_str() {
            "ArithmeticError" => BuiltinFailureKind::Arithmetic,
            "IndexError" => BuiltinFailureKind::Index,
            "DuplicateKeyError" => BuiltinFailureKind::DuplicateKey,
            "ContractViolation" => BuiltinFailureKind::ContractViolation,
            "CapabilityFailure" => BuiltinFailureKind::Capability,
            other => return Err(frame(pointer, format!("unknown failure kind `{other}`"))),
        },
        other => match number(other, pointer)? {
            1 => BuiltinFailureKind::Arithmetic,
            2 => BuiltinFailureKind::Index,
            3 => BuiltinFailureKind::DuplicateKey,
            4 => BuiltinFailureKind::ContractViolation,
            5 => BuiltinFailureKind::Capability,
            _ => return Err(frame(pointer, "failure kinds are 1..5")),
        },
    })
}
