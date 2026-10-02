//! Group live operation edits without guessing malformed or ambiguous targets.

use crate::afx::Context;
use crate::{error::Result, residual::frontier::Budget};
use serde_json::{Map, Value};
use sley_mutate::value::EntityBodyValue;
use std::collections::BTreeMap;

pub(in crate::residual::interfaces) type Groups<'a> = BTreeMap<&'a str, Vec<(usize, &'a Value)>>;

pub(in crate::residual::interfaces) fn groups<'a>(
    declarations: &'a Map<String, Value>,
    budget: &mut Budget,
) -> Result<Option<Groups<'a>>> {
    let mut groups = Groups::new();
    let Some(value) = declarations.get("edit") else {
        return Ok(Some(groups));
    };
    let Some(edits) = value.as_array() else {
        return Ok(None);
    };
    for (index, edit) in edits.iter().enumerate() {
        budget.checkpoint()?;
        let Some(name) = edit
            .get("fn")
            .or_else(|| edit.get("function"))
            .or_else(|| edit.get("name"))
            .and_then(Value::as_str)
        else {
            return Ok(None);
        };
        groups.entry(name).or_default().push((index, edit));
    }
    Ok(Some(groups))
}

pub(super) fn check(
    cx: &crate::afx::Context<'_>,
    edited: &Groups<'_>,
    defined: &std::collections::BTreeSet<&str>,
    patched: &BTreeMap<&str, usize>,
    replaced: Option<&str>,
    budget: &mut Budget,
) -> Result<(Vec<Value>, Vec<String>)> {
    let mut checked = Vec::new();
    let mut deferred = Vec::new();
    for (name, group) in edited {
        budget.checkpoint()?;
        let at = format!("/edit/{}", group[0].0);
        if defined.contains(name) || patched.contains_key(name) || Some(*name) == replaced {
            deferred.push(at);
            continue;
        }
        let targets = targets(cx, name, group, budget)?;
        let inventory =
            crate::afx::operations::edits(cx, name, group, &mut || budget.checkpoint())?;
        let (mut checked_edit, unknown) = super::report(cx, name, &at, inventory, budget)?;
        checked_edit["source"] = serde_json::json!("accepted_graph_edit");
        checked_edit["edit_targets"] = targets;
        checked.push(checked_edit);
        deferred.extend(unknown);
    }
    Ok((checked, deferred))
}

/// Resolve every target before a deferred replacement can stop body inventory.
/// The ordinary builder applies these edits to the accepted graph, not a
/// speculative graph produced by another transformation of the same owner.
fn targets(
    cx: &Context<'_>,
    name: &str,
    edits: &[(usize, &Value)],
    budget: &mut Budget,
) -> Result<Value> {
    let at = format!("/edit/{}", edits[0].0);
    let (owner, function) = cx
        .live_function(name)
        .filter(|_| !cx.reference_deleted(name))
        .ok_or_else(|| {
            super::conflict(
                &at,
                &format!("edit owner `{name}` is not an available accepted function"),
            )
        })?;
    let mut seen = BTreeMap::new();
    let mut result = Vec::new();
    for (index, edit) in edits {
        budget.checkpoint()?;
        let at = format!("/edit/{index}/replace_op");
        let (block_name, operation_name) = edit
            .get("replace_op")
            .and_then(Value::as_str)
            .and_then(|path| path.split_once('.'))
            .ok_or_else(|| {
                super::conflict(&at, "edit target must be an explicit block.operation path")
            })?;
        let mut block = None;
        for id in &function.blocks {
            budget.checkpoint()?;
            if cx.names.leaf(id) == block_name {
                block = Some(*id);
                break;
            }
        }
        let block = block.ok_or_else(|| {
            super::conflict(
                &at,
                &format!("no accepted block `{block_name}` in edit owner `{name}`"),
            )
        })?;
        let Some(EntityBodyValue::Block(body)) = cx.program.body(&block) else {
            return Err(super::conflict(
                &at,
                "edit target has no accepted block body",
            ));
        };
        let mut operation = None;
        for id in &body.operations {
            budget.checkpoint()?;
            if cx.names.leaf(id) == operation_name {
                operation = Some(*id);
                break;
            }
        }
        let operation = operation.ok_or_else(|| super::conflict(&at, &format!(
            "no accepted operation `{operation_name}` in `{name}.{block_name}`; split AF1-X blocks require their actual graph location or an explicit block patch"
        )))?;
        if let Some(previous) = seen.insert(operation, at.clone()) {
            return Err(super::conflict(
                &at,
                &format!(
                    "operation `{name}.{block_name}.{operation_name}` is edited twice; conflicts with {previous}"
                ),
            ));
        }
        if !edit
            .get("with")
            .is_some_and(|with| with.is_array() || with.is_object())
        {
            return Err(super::conflict(
                &format!("/edit/{index}/with"),
                "edit replacement must be an explicit operation array or object",
            ));
        }
        result.push(serde_json::json!({"at":at,"function":name,
            "owner_id":crate::hex::encode(owner.as_bytes()),
            "block_id":crate::hex::encode(block.as_bytes()),
            "operation_id":crate::hex::encode(operation.as_bytes()),
            "operation":cx.names.name(&operation),"binding":"accepted_graph_target",
            "authority":"none"}));
    }
    Ok(Value::Array(result))
}
