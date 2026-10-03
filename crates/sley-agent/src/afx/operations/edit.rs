//! Ordinary AF1 edits replace named live operations and restate edited blocks.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::{Context, Inventory, Parser, Result, Stmt, patch};
use crate::afx::{EntityBodyValue, is_extended_op};

pub(crate) fn edits(
    cx: &Context<'_>,
    name: &str,
    edits: &[(usize, &Value)],
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Inventory> {
    checkpoint()?;
    let at = format!("/edit/{}", edits[0].0);
    let unknown = || Inventory {
        operations: Vec::new(),
        deferred: vec![at.clone()],
    };
    let Some((_, function)) = cx.live_function(name) else {
        return Ok(unknown());
    };
    if cx.reference_deleted(name) {
        return Ok(unknown());
    }
    let mut replacements = BTreeMap::new();
    let mut blocks = BTreeSet::new();
    for (index, edit) in edits {
        checkpoint()?;
        let Some((block, leaf)) = edit
            .get("replace_op")
            .and_then(Value::as_str)
            .and_then(|path| path.split_once('.'))
        else {
            return Ok(unknown());
        };
        let Some(block) = function.blocks.iter().find(|id| cx.names.leaf(id) == block) else {
            return Ok(unknown());
        };
        let Some(EntityBodyValue::Block(body)) = cx.program.body(block) else {
            return Ok(unknown());
        };
        let Some(id) = body.operations.iter().find(|id| cx.names.leaf(id) == leaf) else {
            return Ok(unknown());
        };
        let Some(with) = edit.get("with") else {
            return Ok(unknown());
        };
        if is_extended_op(with) || replacements.contains_key(id) {
            return Ok(unknown());
        }
        let replacement = match with {
            Value::Array(items) => Value::Array(
                std::iter::once(json!(leaf))
                    .chain(items.iter().cloned())
                    .collect(),
            ),
            Value::Object(object) => {
                let mut object = object.clone();
                object.insert("name".into(), json!(leaf));
                Value::Object(object)
            }
            _ => return Ok(unknown()),
        };
        let mut obligations = Vec::new();
        let parsed = Parser {
            obligations: &mut obligations,
        }
        .stmt(&replacement, &format!("/edit/{index}/with"));
        let Stmt::Op(node) = parsed else {
            return Ok(unknown());
        };
        if !obligations.is_empty() {
            return Ok(unknown());
        }
        let mut inventory = Inventory::default();
        inventory.node(&node, checkpoint)?;
        if !inventory.deferred.is_empty() {
            return Ok(unknown());
        }
        replacements.insert(*id, inventory);
        blocks.insert(*block);
    }
    let mut inventory = patch(cx, name, &json!({}), &at, false, checkpoint)?;
    inventory.operations.retain(|operation| {
        operation
            .retained
            .is_none_or(|id| !replacements.contains_key(&id))
    });
    for operation in &mut inventory.operations {
        checkpoint()?;
        if operation.retained.is_some_and(|id|matches!(cx.program.body(&id),Some(EntityBodyValue::Operation(body)) if blocks.contains(&body.block))) {
            // frame::restate_block renders these as authored operation specs.
            // Their callee name is resolved again in the proposed frame.
            operation.callee_id = None;
            operation.restated = true;
        }
    }
    for (_, replacement) in replacements {
        inventory.operations.extend(replacement.operations);
    }
    Ok(inventory)
}
