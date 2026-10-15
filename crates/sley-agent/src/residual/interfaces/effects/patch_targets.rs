//! Validate source patch targets without predicting generated AF1-X blocks.

use serde_json::{Value, json};

use crate::{afx::Context, error::Result, names::is_identifier, residual::frontier::Budget};

pub(super) fn check(
    cx: &Context<'_>,
    name: &str,
    patch: &Value,
    at: &str,
    dialect: bool,
    budget: &mut Budget,
) -> Result<(Value, Vec<String>)> {
    budget.checkpoint()?;
    let (owner, function) = cx
        .live_function(name)
        .filter(|_| !cx.reference_deleted(name))
        .ok_or_else(|| {
            super::conflict(
                at,
                &format!("patch owner `{name}` is not an available accepted function"),
            )
        })?;
    let mut targets = Vec::new();
    let mut deferred = Vec::new();
    let Some(blocks) = patch.get("blocks") else {
        return Ok((json!(targets), deferred));
    };
    let blocks = blocks.as_object().ok_or_else(|| {
        super::conflict(
            &format!("{at}/blocks"),
            "patch blocks must be an object keyed by block name",
        )
    })?;
    for (name, body) in blocks {
        budget.checkpoint()?;
        let key = name.replace('~', "~0").replace('/', "~1");
        let at = format!("{at}/blocks/{key}");
        if !is_identifier(name) {
            return Err(super::conflict(&at, "invalid patch block name"));
        }
        let mut block = None;
        for id in &function.blocks {
            budget.checkpoint()?;
            if cx.names.leaf(id) == *name {
                block = Some(*id);
                break;
            }
        }
        // Expansion emits generated pieces after copying null/raw entries.
        // Thus a generated block can overwrite such an entry, including a
        // missing deletion target. All generated block names contain `__`.
        // Only the actual expansion can resolve those collisions.
        let unresolved = dialect
            && name.contains("__")
            && ((!body.is_null() && !body.is_object()) || (body.is_null() && block.is_none()));
        let action = if unresolved {
            deferred.push(at.clone());
            "pending_expansion"
        } else if body.is_null() {
            if block.is_none() {
                return Err(super::conflict(&at, "no accepted block to delete"));
            }
            "delete"
        } else {
            if !body.is_object() {
                return Err(super::conflict(
                    &at,
                    "a patch block must be an object or null",
                ));
            }
            if block.is_some() { "restate" } else { "create" }
        };
        targets.push(
            json!({"at":at,"owner_id":crate::hex::encode(owner.as_bytes()),
            "block_id":block.map(|id|crate::hex::encode(id.as_bytes())),
            "block":name,"action":action,"binding":"source_graph_target","authority":"none"}),
        );
    }
    Ok((Value::Array(targets), deferred))
}
