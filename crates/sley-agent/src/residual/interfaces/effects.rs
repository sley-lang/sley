//! Empty effect declarations also constrain complete functions in a source draft.

pub(super) mod edits;
mod patch_targets;
mod restatements;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::{conflict, predicates, signatures};
use crate::{
    afx::{Context, operations},
    error::Result,
    opcodes,
    residual::frontier::Budget,
};

pub(super) fn check(
    cx: &Context<'_>,
    declarations: &Map<String, Value>,
    replaced: Option<&str>,
    budget: &mut Budget,
) -> Result<Value> {
    let mut checked = Vec::new();
    let mut deferred = Vec::new();
    let mut patched = BTreeMap::new();
    let mut defined = BTreeSet::new();
    let edited = edits::groups(declarations, budget)?;
    let ripple = declarations
        .get("ripple")
        .is_some_and(|value| value.as_array().is_none_or(|items| !items.is_empty()));
    if ripple || edited.is_none() {
        return Ok(
            json!({"status":"deferred","reason":"later_body_transformations",
            "source":if ripple {"/ripple"}else{"/edit"},"checked_functions":[],"authority":"none"}),
        );
    }
    let edited = edited.expect("checked edit groups");
    restatements::check(declarations, replaced, budget)?;
    if let Some(patches) = declarations.get("patch") {
        if let Some(patches) = patches.as_array() {
            for (index, patch) in patches.iter().enumerate() {
                budget.checkpoint()?;
                let name = patch
                    .get("fn")
                    .or_else(|| patch.get("name"))
                    .and_then(Value::as_str);
                if let Some(name) = name {
                    *patched.entry(name).or_insert(0_usize) += 1;
                }
                if name.is_none() {
                    deferred.push(format!("/patch/{index}"));
                }
            }
        } else {
            deferred.push("/patch".into());
        }
    }
    for key in ["fns", "functions"] {
        let Some(functions) = declarations.get(key) else {
            continue;
        };
        let Some(functions) = functions.as_array() else {
            deferred.push(format!("/{key}"));
            continue;
        };
        for (index, function) in functions.iter().enumerate() {
            budget.checkpoint()?;
            let at = format!("/{key}/{index}");
            let name = function
                .get("fn")
                .or_else(|| function.get("name"))
                .and_then(Value::as_str);
            if name.is_some() && name == replaced {
                continue;
            }
            if let Some(name) = name {
                defined.insert(name);
            }
            let (Some(name), Some(blocks)) =
                (name, function.get("blocks").and_then(Value::as_array))
            else {
                deferred.push(at);
                continue;
            };
            // A patch can remove/replace an operation in this declaration.
            if patched.contains_key(name) || edited.contains_key(name) {
                deferred.push(at);
                continue;
            }
            let (report, unknown) = body(cx, name, &at, blocks, budget)?;
            deferred.extend(unknown);
            checked.push(report);
        }
    }
    let (patch_reports, unknown) = patches(
        cx,
        declarations,
        replaced,
        &defined,
        &patched,
        &edited,
        budget,
    )?;
    checked.extend(patch_reports);
    deferred.extend(unknown);
    let (edit_reports, unknown) = edits::check(cx, &edited, &defined, &patched, replaced, budget)?;
    checked.extend(edit_reports);
    deferred.extend(unknown);
    Ok(
        json!({"status":if deferred.is_empty(){"checked"}else{"partial"},
        "scope":"authored_definitions_and_accepted_graph_patches_and_edits; conflicting_restatements_refused; ripple_and_unresolved_transforms_deferred",
        "checked_functions":checked,"deferred":deferred,"authority":"none"}),
    )
}

fn body(
    cx: &Context<'_>,
    name: &str,
    at: &str,
    blocks: &[Value],
    budget: &mut Budget,
) -> Result<(Value, Vec<String>)> {
    let mut combined = operations::Inventory::default();
    for (block_index, block) in blocks.iter().enumerate() {
        let inventory = operations::block(
            cx,
            block,
            &format!("{at}/blocks/{block_index}"),
            &mut || budget.checkpoint(),
        )?;
        combined.operations.extend(inventory.operations);
        combined.deferred.extend(inventory.deferred);
    }
    report(cx, name, at, combined, budget)
}

fn report(
    cx: &Context<'_>,
    name: &str,
    at: &str,
    inventory: operations::Inventory,
    budget: &mut Budget,
) -> Result<(Value, Vec<String>)> {
    let mut calls = Vec::new();
    let mut unknown = inventory.deferred;
    for operation in inventory.operations {
        budget.checkpoint()?;
        let Some(row) = opcodes::by_tag(operation.tag) else {
            unknown.push(operation.at);
            continue;
        };
        let source = operation
            .retained
            .map(|id| format!("retained operation `{}`", cx.names.name(&id)));
        let location = source.as_ref().map_or_else(
            || operation.at.clone(),
            |source| format!("{} ({source})", operation.at),
        );
        predicates::admission::check(row, &location)?;
        if operation.tag != 112 {
            continue;
        }
        let Some(callee) = operation.callee else {
            unknown.push(operation.at);
            continue;
        };
        if operation.callee_id.is_some_and(|id| {
            cx.names.resolve(&callee) != Some(id) || cx.reference_deleted(&callee)
        }) {
            return Err(conflict(
                &location,
                &format!(
                    "retained callee `{callee}` is deleted or no longer bound by its original identity"
                ),
            ));
        }
        signatures::read(cx, &callee, &location)?;
        let Some(effects) = cx.reference_function_effects(&callee) else {
            unknown.push(operation.at);
            continue;
        };
        if !effects.is_empty() {
            return Err(conflict(
                &location,
                &format!(
                    "callee `{callee}` declares effects incompatible with the empty effect declaration of `{name}` at {at}"
                ),
            ));
        }
        calls.push(json!({"at":operation.at,"callee":callee,"restated_operation":operation.restated,"callee_binding":if operation.callee_id.is_some(){"retained_entity"}else{"authoring_name"},"retained_operation":operation.retained.map(|id|json!({"id":crate::hex::encode(id.as_bytes()),"name":cx.names.name(&id)})),
            "interface":if cx.declared_signature(&callee){"declared_empty"}else{"accepted_empty"}}));
    }
    let report = json!({"at":at,"function":name,"calls":calls,
    "status":if unknown.is_empty(){"empty_effect_connections_checked"}else{"partial"},
    "deferred":unknown});
    Ok((report, unknown))
}

fn patches(
    cx: &Context<'_>,
    declarations: &Map<String, Value>,
    replaced: Option<&str>,
    defined: &BTreeSet<&str>,
    counts: &BTreeMap<&str, usize>,
    edited: &edits::Groups<'_>,
    budget: &mut Budget,
) -> Result<(Vec<Value>, Vec<String>)> {
    let mut reports = Vec::new();
    let mut deferred = Vec::new();
    for (index, patch) in declarations
        .get("patch")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        budget.checkpoint()?;
        let at = format!("/patch/{index}");
        let Some(name) = patch
            .get("fn")
            .or_else(|| patch.get("name"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if Some(name) == replaced {
            continue;
        }
        if defined.contains(name) || edited.contains_key(name) || counts.get(name) != Some(&1) {
            deferred.push(at);
            continue;
        }
        let dialect = declarations.get("afx") == Some(&json!(1));
        let (targets, target_unknown) =
            patch_targets::check(cx, name, patch, &at, dialect, budget)?;
        let mut inventory =
            operations::patch(cx, name, patch, &at, dialect, &mut || budget.checkpoint())?;
        inventory.deferred.extend(target_unknown);
        let (mut checked, unknown) = report(cx, name, &at, inventory, budget)?;
        checked["source"] = json!("accepted_graph_patch");
        checked["patch_targets"] = targets;
        reports.push(checked);
        deferred.extend(unknown);
    }
    Ok((reports, deferred))
}
