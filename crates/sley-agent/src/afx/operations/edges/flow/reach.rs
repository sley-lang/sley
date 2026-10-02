//! Current authored declarations and exact retained reachability flags.

use super::{FnExp, Inventory};
use crate::error::Result;
use serde_json::{Value, json};
use sley_mutate::value::EntityBodyValue;
use sley_ssmc::Reachability;

pub(super) fn check(
    view: &FnExp<'_, '_>,
    report: &mut Inventory,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    let Some(flow) = report.flow.as_ref() else {
        return Ok(rows);
    };
    let mut record = |name: &str, unreachable: bool, at: String| {
        let reachable = flow.reachable[flow.names[name]];
        let valid = reachable != unreachable;
        if !valid {
            report.conflicts.push((at.clone(), format!("block `{name}` reachability declaration disagrees with the current entry: structurally reachable={reachable}, declared unreachable={unreachable}")));
        }
        rows.push(json!({"at":at,"block":name,"reachable":reachable,"declared_unreachable":unreachable,"status":if valid {"checked"} else {"conflict"},"rewritten":false}));
    };
    for block in &view.blocks {
        checkpoint()?;
        // Ordinary AF1 defaults a missing/non-bool flag to required; preserve
        // that interpretation rather than inventing stricter surface syntax.
        let unreachable = block
            .object
            .get("unreachable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        record(
            &block.name,
            unreachable,
            format!("{}/unreachable", block.pointer),
        );
    }
    if !view.kept.is_empty() {
        let Some((_, function)) = view.cx.live_function(&view.fn_name) else {
            return Ok(rows);
        };
        for id in &function.blocks {
            checkpoint()?;
            let name = view.cx.names.leaf(id);
            if view.block_index(&name).is_some() || !flow.names.contains_key(&name) {
                continue;
            }
            let Some(EntityBodyValue::Block(body)) = view.cx.program.body(id) else {
                continue;
            };
            record(
                &name,
                body.reachability == Reachability::ExplicitlyUnreachable,
                format!(
                    "{} (retained block `{}` reachability)",
                    view.fn_pointer,
                    view.cx.names.name(id)
                ),
            );
        }
    }
    checkpoint()?;
    Ok(rows)
}
