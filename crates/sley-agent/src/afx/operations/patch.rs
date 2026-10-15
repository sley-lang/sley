//! Inventory surviving patch operations using AF1-X's existing live graph rules.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::{Context, Inventory, Operation, Result, named_block};
use crate::afx::{EntityBodyValue, Immediate, LiveGraph};

pub(crate) fn patch(
    cx: &Context<'_>,
    name: &str,
    value: &Value,
    at: &str,
    dialect: bool,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Inventory> {
    checkpoint()?;
    let mut inventory = Inventory::default();
    let empty = Map::new();
    let patched = match value.get("blocks") {
        None => &empty,
        Some(Value::Object(blocks)) => blocks,
        Some(_) => {
            inventory.deferred.push(format!("{at}/blocks"));
            return Ok(inventory);
        }
    };
    let Some((_, function)) = cx.live_function(name) else {
        inventory.deferred.push(at.into());
        return Ok(inventory);
    };
    let live = LiveGraph::new(cx, &function.blocks);
    checkpoint()?;
    if live.blocks.len() != function.blocks.len() {
        inventory.deferred.push(at.into());
    }
    let mut removed = BTreeSet::new();
    if dialect {
        for key in patched.keys() {
            checkpoint()?;
            removed.extend(live.pieces_of(key));
            checkpoint()?;
        }
    }
    for (leaf, _, body) in &live.blocks {
        checkpoint()?;
        if patched.contains_key(leaf) || removed.contains(leaf) {
            continue;
        }
        // The shared recognizer accepts only generated none/err/variant-return
        // bodies. These have no call/effect operations whether kept or removed.
        if dialect && live.is_generated_exit(leaf, body) {
            continue;
        }
        for id in &body.operations {
            checkpoint()?;
            let Some(EntityBodyValue::Operation(operation)) = cx.program.body(id) else {
                inventory
                    .deferred
                    .push(format!("{at} (missing {})", cx.names.name(id)));
                continue;
            };
            let callee_id = match &operation.immediate {
                Immediate::Function(reference) if operation.opcode == 112 => {
                    Some(reference.function)
                }
                _ => None,
            };
            inventory.operations.push(Operation {
                tag: operation.opcode,
                callee: callee_id.map(|id| cx.names.name(&id)),
                callee_id,
                retained: Some(*id),
                restated: false,
                at: at.into(),
            });
        }
    }
    for (name, block) in patched {
        checkpoint()?;
        if block.is_null() {
            continue;
        }
        let key = name.replace('~', "~0").replace('/', "~1");
        let authored = named_block(cx, block, name, &format!("{at}/blocks/{key}"), checkpoint)?;
        inventory.operations.extend(authored.operations);
        inventory.deferred.extend(authored.deferred);
    }
    Ok(inventory)
}
