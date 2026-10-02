//! Source function/block ownership before fragment generation. This is not a CFG proof.

use super::{conflict, effects};
use crate::{
    afx::{
        Context,
        operations::{edges as source_edges, topology},
    },
    error::Result,
    names::is_identifier,
    residual::frontier::Budget,
};
use serde_json::{Map, Value, json};
use sley_mutate::value::EntityBodyValue;
use sley_ssmc::Terminator;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn check(
    cx: &Context<'_>,
    source: &Map<String, Value>,
    replaced: Option<&str>,
    budget: &mut Budget,
) -> Result<Value> {
    cx.check_declared_constants(&mut || budget.checkpoint())?;
    let edited = effects::edits::groups(source, budget)?;
    if source
        .get("ripple")
        .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
        || edited.is_none()
    {
        return Ok(
            json!({"status":"deferred","reason":"later_body_transformations","checked_functions":[],"authority":"none"}),
        );
    }
    let edited = edited.expect("checked edit groups");
    let mut patched = BTreeSet::new();
    for patch in source
        .get("patch")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        budget.checkpoint()?;
        if let Some(name) = function_name(patch) {
            patched.insert(name);
        }
    }
    let dialect = source.get("afx") == Some(&json!(1));
    let mut reports = Vec::new();
    let mut deferred = Vec::new();
    let mut defined = BTreeSet::new();
    for key in ["fns", "functions"] {
        for (index, function) in source
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            budget.checkpoint()?;
            let at = format!("/{key}/{index}");
            let Some(name) = function
                .get("fn")
                .or_else(|| function.get("name"))
                .and_then(Value::as_str)
            else {
                deferred.push(at);
                continue;
            };
            defined.insert(name);
            if Some(name) == replaced {
                continue;
            }
            if patched.contains(name) || edited.contains_key(name) {
                deferred.push(at);
                continue;
            }
            let (report, unknown) = definition(cx, function, name, &at, dialect, budget)?;
            deferred.extend(unknown);
            reports.extend(report);
        }
    }
    for (index, patch) in source
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
            deferred.push(at);
            continue;
        };
        if Some(name) == replaced {
            continue;
        }
        if defined.contains(name) || edited.contains_key(name) {
            deferred.push(at);
            continue;
        }
        let (report, unknown) = patch_body(cx, name, patch, &at, dialect, budget)?;
        deferred.extend(unknown);
        reports.push(report);
    }
    let excluded = defined
        .union(&patched)
        .copied()
        .chain(edited.keys().copied())
        .collect();
    for (report, unknown) in retained_functions(cx, replaced, &excluded, budget)? {
        append_deferred(&mut deferred, unknown, budget)?;
        reports.push(report);
    }
    Ok(summary(&reports, &deferred))
}

/// Keep established fragment-binding diagnostics ahead of new retained-operation
/// conflicts. Both phases still finish before any fragment expansion.
pub(super) fn retained_operations(report: &Value, budget: &mut Budget) -> Result<()> {
    for function in report["checked_functions"].as_array().into_iter().flatten() {
        budget.checkpoint()?;
        for binding in function["retained_operation_conflicts"]
            .as_array()
            .into_iter()
            .flatten()
        {
            budget.checkpoint()?;
            if let (Some(at), Some(detail)) = (binding[0].as_str(), binding[1].as_str()) {
                return Err(conflict(at, detail));
            }
        }
    }
    Ok(())
}

fn summary(reports: &[Value], deferred: &[String]) -> Value {
    json!({"status":if deferred.is_empty(){"explicit_targets_checked"}else{"partial"},
        "scope":"authored_named_starts_and_symbolic_continuations_with_retained_canonical_edges; generated_allocation_shared_exit_identity_and_full_body_validity_deferred",
        "edge_argument_scope":"authored_definitions_and_patches_explicit_arity_and_known_types; retained_edges_keep_value_identity; implicit_afx_fill_literal_admission_generated_identity_availability_and_generated_cfg_deferred",
        "terminator_input_scope":"authored_and_retained_known_condition_selector_types_and_complete_case_sets; unknown_types_literal_admission_availability_and_generated_cfg_deferred",
        "function_exit_scope":"authored_and_retained_known_return_types_and_closed_persistable_trap_payloads; nongeneric_afx_ok_fail_routes_checked; generic_failure_construction_unknown_types_literal_admission_and_availability_deferred",
        "value_availability_scope":"authored_local_order_qualified_ownership_symbolic_dominance_nearest_definition_rebinding_index_grammar_cardinality_and_resolved_read_site_types_checked; retained_exact_id_operand_availability_checked; unknown_names_incomplete_graphs_shared_exit_identity_and_generated_flags_deferred",
        "operation_signature_scope":"authored_operations_use_ordinary_literal_contexts_and_emitted_use_hints; complete_immediate_free_tuple_index_direct_callee_record_variant_field_constant_and_global_signatures_reuse_canonical_vm_judgment; known_literal_and_declared_constant_data_use_ordinary_reader_and_canonical_checker; retained_canonical_operations_with_current_operands_results_and_supported_bound_immediates_checked_without_rebinding; other_immediates_unknown_authored_types_and_unknown_literal_contexts_deferred",
        "checked_functions":reports,"deferred":deferred,"authority":"none"})
}

fn retained_functions(
    cx: &Context<'_>,
    replaced: Option<&str>,
    excluded: &BTreeSet<&str>,
    budget: &mut Budget,
) -> Result<Vec<(Value, Vec<String>)>> {
    let mut reports = Vec::new();
    for object in cx.program.objects() {
        budget.checkpoint()?;
        let record = object.record();
        if !matches!(record.body, EntityBodyValue::Function(_)) {
            continue;
        }
        let name = cx.names.name(&record.entity_id);
        if Some(name.as_str()) == replaced
            || excluded.contains(name.as_str())
            || cx.reference_deleted(&name)
        {
            continue;
        }
        // Accepted blocks are canonical, including generated pieces. They have
        // no authored AFX sugar to reconstruct, so keep their exact graph.
        let at = format!("accepted function `{name}`");
        let patch = json!({"fn":name,"blocks":{}});
        let (mut report, unknown) = patch_body(cx, &name, &patch, &at, false, budget)?;
        report["source"] = json!("accepted_graph_retained");
        reports.push((report, unknown));
    }
    Ok(reports)
}

fn append_deferred(
    into: &mut Vec<String>,
    additional: Vec<String>,
    budget: &mut Budget,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for pointer in into.iter() {
        budget.checkpoint()?;
        seen.insert(pointer.clone());
    }
    for pointer in additional {
        budget.checkpoint()?;
        if seen.insert(pointer.clone()) {
            into.push(pointer);
        }
    }
    Ok(())
}

fn function_name(value: &Value) -> Option<&str> {
    value
        .get("fn")
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)
}

fn definition(
    cx: &Context<'_>,
    function: &Value,
    name: &str,
    at: &str,
    dialect: bool,
    budget: &mut Budget,
) -> Result<(Option<Value>, Vec<String>)> {
    let mut deferred = Vec::new();
    let Some(blocks) = function.get("blocks").and_then(Value::as_array) else {
        return Ok((None, vec![at.into()]));
    };
    let mut owned = BTreeMap::new();
    let mut bodies = Vec::new();
    if blocks.is_empty() {
        deferred.push(format!("{at}/blocks"));
    }
    for (index, block) in blocks.iter().enumerate() {
        budget.checkpoint()?;
        let loc = format!("{at}/blocks/{index}");
        let Some(label) = block
            .get("name")
            .and_then(Value::as_str)
            .filter(|n| is_identifier(n))
        else {
            deferred.push(loc);
            continue;
        };
        if let Some(prior) = owned.insert(label.to_owned(), loc.clone()) {
            return Err(conflict(
                &format!("{loc}/name"),
                &format!("block `{label}` duplicates {prior} in function `{name}`"),
            ));
        }
        bodies.push((label.to_owned(), loc, block));
    }
    parameter_names(
        function.get("params"),
        &format!("{at}/params"),
        &owned,
        name,
        budget,
    )?;
    let mut edges = Vec::new();
    let mut unknown = Vec::new();
    let mut entry_binding = Vec::new();
    if let Some(entry) = function.get("entry") {
        let pointer = format!("{at}/entry");
        if let Some(label) = entry.as_str() {
            edge(
                name,
                &owned,
                label,
                &pointer,
                dialect,
                &mut entry_binding,
                &mut unknown,
                budget,
            )?;
        } else {
            unknown.push(pointer);
        }
    }
    for (label, loc, block) in bodies {
        let parsed = topology::block(cx, block, &label, &loc, &mut || budget.checkpoint())?;
        unknown.extend(parsed.deferred);
        for (target, pointer) in parsed.targets {
            edge(
                name,
                &owned,
                &target,
                &pointer,
                dialect,
                &mut edges,
                &mut unknown,
                budget,
            )?;
        }
    }
    let types =
        source_edges::definition(cx, function, name, at, dialect, &mut || budget.checkpoint())?;
    if let Some((pointer, decision)) = types.conflicts.first() {
        return Err(conflict(pointer, decision));
    }
    append_deferred(&mut unknown, types.deferred, budget)?;
    deferred.extend(unknown.iter().cloned());
    let report = json!({"at":at,"function":name,"source":"authored_definition","owned_blocks":owned,"entry_binding":entry_binding,"edges":edges,"edge_arguments":types.connections,"terminator_inputs":types.inputs,"value_reads":types.reads,"operation_signatures":types.operations,"control_projection":types.cfg,"deferred":unknown});
    Ok((Some(report), deferred))
}

#[allow(clippy::too_many_arguments)]
fn edge(
    name: &str,
    owned: &BTreeMap<String, String>,
    target: &str,
    at: &str,
    dialect: bool,
    edges: &mut Vec<Value>,
    deferred: &mut Vec<String>,
    budget: &mut Budget,
) -> Result<()> {
    budget.checkpoint()?;
    // Actual AF1-X expansion may regenerate/remove this name, including collisions.
    if dialect && target.contains("__") {
        deferred.push(at.into());
        return Ok(());
    }
    let definition=owned.get(target).ok_or_else(||conflict(at,&format!("target block `{target}` is not owned by function `{name}` after the authored source changes")))?;
    edges.push(json!({"at":at,"target":target,"definition":definition,"owner":name,"binding":"source_function_block","rewritten":false}));
    Ok(())
}

fn patch_body(
    cx: &Context<'_>,
    name: &str,
    patch: &Value,
    at: &str,
    dialect: bool,
    budget: &mut Budget,
) -> Result<(Value, Vec<String>)> {
    let Some((_, function)) = cx
        .live_function(name)
        .filter(|_| !cx.reference_deleted(name))
    else {
        return Ok((
            json!({"at":at,"function":name,"status":"deferred"}),
            vec![at.into()],
        ));
    };
    let empty = Map::new();
    let patches = match patch.get("blocks") {
        None => &empty,
        Some(Value::Object(blocks)) => blocks,
        _ => {
            return Ok((
                json!({"at":at,"function":name,"status":"deferred"}),
                vec![format!("{at}/blocks")],
            ));
        }
    };
    let BlockInventory {
        owned,
        retained,
        accepted,
    } = patch_inventory(cx, &function.blocks, patches, at, dialect, budget)?;
    let entry = cx.names.leaf(&function.entry_block);
    patch_parameters(cx, name, patch, at, &owned, &function.parameters, budget)?;
    let mut edges = Vec::new();
    let mut deferred = Vec::new();
    let mut entry_binding = Vec::new();
    match patch.get("entry") {
        Some(Value::String(label)) => edge(
            name,
            &owned,
            label,
            &format!("{at}/entry"),
            dialect,
            &mut entry_binding,
            &mut deferred,
            budget,
        )?,
        Some(_) => deferred.push(format!("{at}/entry")),
        None if owned.contains_key(&entry) => edge(
            name,
            &owned,
            &entry,
            at,
            dialect,
            &mut entry_binding,
            &mut deferred,
            budget,
        )?,
        // Ordinary AF1 may select a new first block when the old entry is gone.
        // Do not turn that supported reconstruction rule into a refusal here.
        None => deferred.push(format!("{at}/blocks")),
    }
    retained_edges(
        cx,
        name,
        &retained,
        &accepted,
        &owned,
        at,
        dialect,
        &mut edges,
        &mut deferred,
        budget,
    )?;
    for (label, body) in patches {
        budget.checkpoint()?;
        if body.is_null() {
            continue;
        }
        let loc = format!("{at}/blocks/{}", escape(label));
        let parsed = topology::block(cx, body, label, &loc, &mut || budget.checkpoint())?;
        deferred.extend(parsed.deferred);
        for (target, pointer) in parsed.targets {
            edge(
                name,
                &owned,
                &target,
                &pointer,
                dialect,
                &mut edges,
                &mut deferred,
                budget,
            )?;
        }
    }
    let types = source_edges::patch(cx, patch, name, at, dialect, &retained, &mut || {
        budget.checkpoint()
    })?;
    if let Some((pointer, decision)) = types.conflicts.first() {
        return Err(conflict(pointer, decision));
    }
    append_deferred(&mut deferred, types.deferred, budget)?;
    Ok((
        json!({"at":at,"function":name,"source":"accepted_graph_patch","owned_blocks":owned,"entry_binding":entry_binding,"edges":edges,"edge_arguments":types.connections,"terminator_inputs":types.inputs,"value_reads":types.reads,"operation_signatures":types.operations,"retained_operation_conflicts":types.retained_conflicts,"control_projection":types.cfg,"deferred":deferred}),
        deferred,
    ))
}

#[allow(clippy::too_many_arguments)]
fn patch_parameters(
    cx: &Context<'_>,
    name: &str,
    patch: &Value,
    at: &str,
    owned: &BTreeMap<String, String>,
    parameters: &[sley_id::EntityId],
    budget: &mut Budget,
) -> Result<()> {
    if patch.get("params").is_some() {
        parameter_names(
            patch.get("params"),
            &format!("{at}/params"),
            owned,
            name,
            budget,
        )?;
    } else {
        for id in parameters {
            budget.checkpoint()?;
            let label = cx.names.leaf(id);
            if let Some(block) = owned.get(&label) {
                return Err(conflict(
                    at,
                    &format!(
                        "block `{label}` at {block} conflicts with retained function parameter `{}` of `{name}`",
                        cx.names.name(id)
                    ),
                ));
            }
        }
    }
    Ok(())
}

struct BlockInventory {
    owned: BTreeMap<String, String>,
    retained: Vec<sley_id::EntityId>,
    accepted: BTreeSet<sley_id::EntityId>,
}

fn patch_inventory(
    cx: &Context<'_>,
    blocks: &[sley_id::EntityId],
    patches: &Map<String, Value>,
    at: &str,
    dialect: bool,
    budget: &mut Budget,
) -> Result<BlockInventory> {
    let removed = if dialect {
        topology::removed_pieces(cx, blocks, patches.keys().cloned(), &mut || {
            budget.checkpoint()
        })?
    } else {
        BTreeSet::new()
    };
    let mut owned = BTreeMap::new();
    let mut retained = Vec::new();
    let mut accepted = BTreeSet::new();
    for id in blocks {
        budget.checkpoint()?;
        accepted.insert(*id);
        let label = cx.names.leaf(id);
        if patches.contains_key(&label) || removed.contains(&label) {
            continue;
        }
        owned.insert(
            label.clone(),
            format!("accepted block `{}`", cx.names.name(id)),
        );
        retained.push(*id);
    }
    for (label, body) in patches {
        budget.checkpoint()?;
        if !body.is_null() {
            owned.insert(label.clone(), format!("{at}/blocks/{}", escape(label)));
        }
    }
    Ok(BlockInventory {
        owned,
        retained,
        accepted,
    })
}

#[allow(clippy::too_many_arguments)]
fn retained_edges(
    cx: &Context<'_>,
    name: &str,
    retained: &[sley_id::EntityId],
    accepted: &BTreeSet<sley_id::EntityId>,
    owned: &BTreeMap<String, String>,
    at: &str,
    dialect: bool,
    edges: &mut Vec<Value>,
    deferred: &mut Vec<String>,
    budget: &mut Budget,
) -> Result<()> {
    for id in retained {
        budget.checkpoint()?;
        let Some(EntityBodyValue::Block(body)) = cx.program.body(id) else {
            deferred.push(at.into());
            continue;
        };
        for target in targets(&body.terminator) {
            let loc = format!("{at} (retained block `{}` terminator)", cx.names.name(id));
            if !accepted.contains(&target) {
                return Err(conflict(
                    &loc,
                    &format!(
                        "retained target `{}` belongs outside function `{name}`",
                        cx.names.name(&target)
                    ),
                ));
            }
            edge(
                name,
                owned,
                &cx.names.leaf(&target),
                &loc,
                dialect,
                edges,
                deferred,
                budget,
            )?;
        }
    }
    Ok(())
}

fn parameter_names(
    value: Option<&Value>,
    at: &str,
    owned: &BTreeMap<String, String>,
    function: &str,
    budget: &mut Budget,
) -> Result<()> {
    for (index, pair) in value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        budget.checkpoint()?;
        let Some(label) = pair
            .as_array()
            .filter(|p| p.len() == 2)
            .and_then(|p| p[0].as_str())
        else {
            continue;
        };
        if let Some(block) = owned.get(label) {
            return Err(conflict(
                &format!("{at}/{index}/0"),
                &format!(
                    "function parameter `{label}` conflicts with block at {block} in function `{function}`; parameters and blocks share their owner namespace"
                ),
            ));
        }
    }
    Ok(())
}

fn escape(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}
fn targets(term: &Terminator) -> Vec<sley_id::EntityId> {
    match term {
        Terminator::Branch(t) => vec![t.edge.target],
        Terminator::CondBranch(t) => vec![t.if_true.target, t.if_false.target],
        Terminator::VariantSwitch(t) => t.cases.iter().map(|c| c.edge.target).collect(),
        Terminator::Return(_) | Terminator::Trap(_) => Vec::new(),
    }
}
