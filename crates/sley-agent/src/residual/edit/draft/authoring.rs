//! Retain editable source syntax, including tables, while changing only the
//! requested literal immediates in its deterministic plain AF1 expansion.

use serde_json::{Value, json};
use sley_mutate::value::EntityBodyValue;

use crate::afx::{self, SourceMap};
use crate::error::Result;
use crate::names::Names;
use crate::residual::frontier::Budget;
use crate::workspace::Program;

use super::super::preserve;

mod size;

/// Authoring artifacts for a future revision. These do not publish a revision
/// or certify that its candidate bytes correspond to this frame.
pub struct Retained {
    /// Feature counts from the repaired expansion.
    pub stats: serde_json::Map<String, Value>,
    /// Fresh Ripple inventory when the original source used Ripple.
    pub ripple: Option<Value>,
    /// Editable source syntax with mapped literal values repaired and, when
    /// completed by the bound loader, explicit inherited constant declarations.
    pub frame: Value,
    /// Deterministically expanded repaired frame.
    pub expanded: Value,
    /// Fresh source map for the repaired syntax.
    pub source_map: Value,
    /// Exact authored locations changed by the repair.
    pub changes: Value,
    /// Source-object origins for locally appended constant declarations.
    pub inherited_constants: Vec<Value>,
}

/// Repair authored literal sites using a freshly computed source map. Check
/// that re-expansion differs only at the explicitly selected immediates.
/// Existing tests, table rows, namespaces and other author decisions survive.
///
/// # Errors
/// Refuses ambiguous sites, unmappable generated constructs, expansion
/// obligations, unrelated expansion changes, and exhausted budgets.
pub fn retain(
    program: &Program,
    names: &Names,
    source: &Value,
    delta: &Value,
    budget: &mut Budget,
) -> Result<Retained> {
    budget.checkpoint()?;
    let original = expand(program, names, source)?;
    let (mut expected, map) = (original.frame, original.map);
    let mut frame = source.clone();
    let mut changes = Vec::new();
    for edit in delta["edit"].as_array().into_iter().flatten() {
        budget.checkpoint()?;
        let typed = &edit["with"][1];
        if edit["with"][0] != "const" || typed.get("value").is_none() {
            return Err(preserve("authoring repair requires a typed constant edit"));
        }
        let sites = sites(&expected, edit, budget)?;
        budget.charge(sites.len() + 1)?;
        let authored = match sites.as_slice() {
            [] => {
                // The operation belongs to the accepted graph. Keep an
                // ordinary edit; do not copy the delta's namespace:null.
                let target = format!(
                    "{}.{}",
                    edit["fn"].as_str().unwrap_or(""),
                    edit["replace_op"].as_str().unwrap_or("")
                );
                let id = names.resolve(&target).filter(|id| names.name(id) == target);
                if !id.is_some_and(|id| {
                    matches!(program.body(&id),
                    Some(EntityBodyValue::Operation(op)) if op.opcode == 1)
                }) {
                    return Err(preserve(
                        "literal is absent from both source syntax and accepted graph",
                    ));
                }
                append_edit(&mut expected, edit)?;
                append_edit(&mut frame, edit)?
            }
            [pointer] => {
                let authored = authored_location(source, &map, pointer)?;
                let value = frame.pointer_mut(&authored).ok_or_else(|| {
                    preserve("expanded literal has no retained authored location")
                })?;
                // Preserve an authored type annotation or contextual literal;
                // replacing a named constant reference uses a private inline
                // value instead of changing the shared constant declaration.
                if let Some(old) = value.get_mut("value") {
                    *old = typed["value"].clone();
                } else if value.is_number() {
                    *value = typed["value"].clone();
                } else if value.is_string() {
                    *value = typed.clone();
                } else {
                    return Err(preserve(
                        "generated literal is not an editable authored value",
                    ));
                }
                let value = expected.pointer_mut(pointer).ok_or_else(|| {
                    preserve("expanded literal location disappeared during repair")
                })?;
                if let Some(old) = value.get_mut("value") {
                    *old = typed["value"].clone();
                } else if value.is_number() {
                    *value = typed["value"].clone();
                } else {
                    *value = typed.clone();
                }
                authored
            }
            _ => return Err(preserve("literal has multiple authoring definitions")),
        };
        changes.push(json!({"fn":edit["fn"],"replace_op":edit["replace_op"],
            "authored":authored,"value":typed["value"]}));
    }
    budget.checkpoint()?;
    let expansion = expand(program, names, &frame)?;
    budget.checkpoint()?;
    if expansion.frame != expected {
        return Err(preserve(
            "authoring repair changed unrelated expanded constructs",
        ));
    }
    Ok(Retained {
        stats: expansion.stats.to_json(),
        ripple: expansion.ripple,
        frame,
        expanded: expansion.frame,
        source_map: expansion.map.to_json(),
        changes: Value::Array(changes),
        inherited_constants: Vec::new(),
    })
}

pub(crate) fn inherit_constants(
    retained: &mut Retained,
    program: &Program,
    names: &Names,
    declarations: Vec<(Value, Value)>,
    budget: &mut Budget,
) -> Result<()> {
    let mut frame = retained.frame.clone();
    let mut expected = retained.expanded.clone();
    let mut origins = Vec::new();
    for (declaration, mut origin) in declarations {
        budget.charge(1)?;
        let index = append_constant(&mut frame, declaration.clone())?;
        append_constant(&mut expected, declaration)?;
        origin["authored"] = json!(format!("/consts/{index}"));
        origins.push(origin);
    }
    if !size::fits(&frame, crate::residual::binding::MAX_BOUND_ARTIFACT_BYTES) {
        return Err(super::super::limit(
            "retained frame exceeds receipt byte, value-count or nesting bounds",
        ));
    }
    budget.checkpoint()?;
    let expansion = expand(program, names, &frame)?;
    budget.checkpoint()?;
    if expansion.frame != expected {
        return Err(preserve(
            "inherited constants changed unrelated expanded constructs",
        ));
    }
    retained.frame = frame;
    retained.expanded = expansion.frame;
    retained.source_map = expansion.map.to_json();
    retained.stats = expansion.stats.to_json();
    retained.ripple = expansion.ripple;
    retained.inherited_constants.extend(origins);
    Ok(())
}

fn append_constant(frame: &mut Value, declaration: Value) -> Result<usize> {
    let constants = frame
        .as_object_mut()
        .ok_or_else(|| preserve("retained frame is not an object"))?
        .entry("consts")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| preserve("retained constants are not an array"))?;
    let index = constants.len();
    constants.push(declaration);
    Ok(index)
}

fn expand(program: &Program, names: &Names, frame: &Value) -> Result<afx::Expansion> {
    if frame.get("afx").is_none() {
        return Ok(afx::Expansion {
            frame: frame.clone(),
            map: SourceMap::default(),
            stats: afx::AfxStats::default(),
            obligations: Vec::new(),
            ripple: None,
        });
    }
    let expansion = afx::expand(program, names, frame)?;
    if !expansion.obligations.is_empty() {
        return Err(afx::refusal(&expansion.obligations));
    }
    Ok(expansion)
}

fn append_edit(frame: &mut Value, edit: &Value) -> Result<String> {
    let object = frame
        .as_object_mut()
        .ok_or_else(|| preserve("source frame is not an object"))?;
    let entries = object
        .entry("edit")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| preserve("source edits are not an array"))?;
    let pointer = format!("/edit/{}/with/1", entries.len());
    entries.push(edit.clone());
    Ok(pointer)
}

pub(super) fn sites(frame: &Value, edit: &Value, budget: &mut Budget) -> Result<Vec<String>> {
    let mut sites = Vec::new();
    let Some((block, operation)) = edit["replace_op"].as_str().and_then(|s| s.split_once('.'))
    else {
        return Err(preserve("literal edit has no block.operation selector"));
    };
    for key in ["fns", "functions", "patch"] {
        for (i, function) in frame[key].as_array().into_iter().flatten().enumerate() {
            budget.charge(1)?;
            if function.get("fn").or_else(|| function.get("name")) != edit.get("fn") {
                continue;
            }
            if key == "patch" {
                collect_ops(
                    &function["blocks"][block],
                    operation,
                    &format!("/{key}/{i}/blocks/{}", escape(block)),
                    &mut sites,
                    budget,
                )?;
            } else {
                for (j, candidate) in function["blocks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    budget.charge(1)?;
                    if candidate["name"] == block {
                        collect_ops(
                            candidate,
                            operation,
                            &format!("/{key}/{i}/blocks/{j}"),
                            &mut sites,
                            budget,
                        )?;
                    }
                }
            }
        }
    }
    for (i, existing) in frame["edit"].as_array().into_iter().flatten().enumerate() {
        budget.charge(1)?;
        if existing["fn"] == edit["fn"] && existing["replace_op"] == edit["replace_op"] {
            sites.push(format!(
                "/edit/{i}/with{}",
                immediate(&existing["with"], false)?
            ));
        }
    }
    Ok(sites)
}

fn collect_ops(
    block: &Value,
    name: &str,
    pointer: &str,
    sites: &mut Vec<String>,
    budget: &mut Budget,
) -> Result<()> {
    for (i, op) in block["ops"].as_array().into_iter().flatten().enumerate() {
        budget.charge(1)?;
        if op[0] == name || op["name"] == name {
            sites.push(format!("{pointer}/ops/{i}{}", immediate(op, true)?));
        }
    }
    Ok(())
}

fn immediate(op: &Value, named: bool) -> Result<&'static str> {
    let word = if op.is_array() {
        &op[usize::from(named)]
    } else {
        op.get("op")
            .or_else(|| op.get("opcode"))
            .unwrap_or(&Value::Null)
    };
    if word
        .as_str()
        .and_then(crate::opcodes::by_word)
        .is_none_or(|row| row.tag != 1)
    {
        return Err(preserve("source syntax target is not a constant load"));
    }
    Ok(if op.is_array() {
        if named { "/2" } else { "/1" }
    } else if op.get("args").is_some() {
        "/args/0"
    } else {
        "/operands/0"
    })
}

fn authored_location(source: &Value, map: &SourceMap, pointer: &str) -> Result<String> {
    // AF1-X may normalize an object operation into array syntax. Map the
    // immediate through the operation's origin rather than copying its suffix.
    if let Some(entry) = map.entries.iter().find(|entry| {
        entry.role == afx::Role::Op && pointer.starts_with(&format!("{}/", entry.expanded))
    }) {
        let op = source
            .pointer(&entry.authored)
            .ok_or_else(|| preserve("missing authored operation"))?;
        return Ok(format!("{}{}", entry.authored, immediate(op, true)?));
    }
    Ok(map.authored(pointer).unwrap_or_else(|| pointer.to_owned()))
}

fn escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
