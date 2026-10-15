//! Check authored assertions and accepted-head replay against the actual graph
//! without publication. Report unproven historical constants explicitly.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use sley_id::{CandidateNonce, EntityId};
use sley_mutate::{MutationPayload, value::EntityBodyValue};
use sley_ssmc::Immediate;

use super::super::preserve;
use crate::candidate::Authority;
use crate::error::Result;
use crate::names::{NameMap, Names, Scope};
use crate::residual::frontier::Budget;
use crate::workspace::{Head, Program};

mod coverage;
mod inherit;

pub(crate) use inherit::complete;

pub(crate) fn check(
    head: &Head,
    program: &Program,
    map: &NameMap,
    frame: &Value,
    budget: &mut Budget,
) -> Result<Value> {
    Ok(inspect(head, program, map, frame, budget)?.report)
}

fn inspect(
    head: &Head,
    program: &Program,
    map: &NameMap,
    frame: &Value,
    budget: &mut Budget,
) -> Result<coverage::Checked> {
    budget.checkpoint()?;
    let accepted_names = Names::build(head.program(), map);
    // Lower against the original head: reapplying Ripple to its result can
    // have a different meaning. The resulting AF1 is checked against the graph.
    let mut plain = if frame.get("afx").is_some() {
        let expanded = crate::afx::expand(head.program(), &accepted_names, frame)?;
        if !expanded.obligations.is_empty() {
            return Err(preserve("source frame no longer has a complete expansion"));
        }
        expanded.frame
    } else {
        frame.clone()
    };
    let object = plain
        .as_object_mut()
        .ok_or_else(|| preserve("source frame is not an object"))?;
    let anonymous = name_tests(object, &accepted_names, budget)?;
    let replay_frame = Value::Object(object.clone());
    let deleted = applied_deletions(head.program(), program, &accepted_names, object, budget)?;
    budget.checkpoint()?;
    let names = Names::build(program, map);
    let authority = Authority::of(head)?;
    let compiled = crate::frame::compile(
        program,
        &names,
        &authority.ceilings,
        &plain,
        CandidateNonce::from_bytes([0x5c; 32]),
        &mut || Err(preserve("source frame requests a new type member")),
    )
    .map_err(|error| {
        preserve(&format!(
            "source frame cannot be asserted against its graph: {}",
            error.detail()
        ))
    })?;
    budget.checkpoint()?;
    let mut aliases = 0;
    for operation in &compiled.ops {
        budget.charge(1)?;
        if equal_constant_alias(program, operation.target, &operation.payload)
            && inline_constant(program, &names, &plain, operation.target, budget)?
        {
            aliases += 1;
        } else {
            return Err(preserve(&format!(
                "source frame disagrees with graph entity `{}`",
                names.name(&operation.target)
            )));
        }
    }
    let mut checked = coverage::check(head, program, map, &replay_frame, budget)?;
    checked.report = json!({"authored_assertions":"match_validated_graph",
        "complete_candidate_correspondence":checked.report["complete_candidate_correspondence"],
        "accepted_head_replay":checked.report,
        "functions":compiled.functions.len(),"tests":compiled.tests.len(),
        "anonymous_tests_named":anonymous,"applied_deletions_checked":deleted,
        "equal_constant_aliases":aliases,"publication":"not_attempted"});
    Ok(checked)
}

// The compiler may choose a different equal constant for an inline value.
// A named reference asserts identity as well as value and cannot use this
// exception: changing that named constant later must affect the same loads.
fn inline_constant(
    program: &Program,
    names: &Names,
    frame: &Value,
    id: EntityId,
    budget: &mut Budget,
) -> Result<bool> {
    let Some(EntityBodyValue::Operation(operation)) = program.body(&id) else {
        return Ok(false);
    };
    let Some(EntityBodyValue::Block(block)) = program.body(&operation.block) else {
        return Ok(false);
    };
    let selector = json!({"fn":names.name(&block.function),
        "replace_op":format!("{}.{}",names.leaf(&operation.block),names.leaf(&id))});
    let sites = super::authoring::sites(frame, &selector, budget)?;
    Ok(sites.len() == 1
        && frame.pointer(&sites[0]).is_some_and(|value| {
            value.is_number() || value.is_boolean() || value.get("value").is_some()
        }))
}

fn equal_constant_alias(program: &Program, id: EntityId, payload: &MutationPayload) -> bool {
    let MutationPayload::ReplaceEntityVersion(EntityBodyValue::Operation(next)) = payload else {
        return false;
    };
    let Some(EntityBodyValue::Operation(old)) = program.body(&id) else {
        return false;
    };
    if old.opcode != 1 || next.opcode != 1 {
        return false;
    }
    let (Immediate::Entity(before), Immediate::Entity(after)) = (&old.immediate, &next.immediate)
    else {
        return false;
    };
    let (Some(EntityBodyValue::Constant(before)), Some(EntityBodyValue::Constant(after))) =
        (program.body(before), program.body(after))
    else {
        return false;
    };
    let mut expected = old.clone();
    expected.immediate = next.immediate.clone();
    before == after && expected == *next
}

// Mirror the compiler's anonymous test allocation over the original accepted
// names and already declared top-level entries. Explicit names stay unchanged.
fn name_tests(
    frame: &mut serde_json::Map<String, Value>,
    names: &Names,
    budget: &mut Budget,
) -> Result<usize> {
    let mut declared = BTreeSet::new();
    for key in ["types", "consts", "fns", "functions"] {
        for entry in frame
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            budget.charge(1)?;
            if let Some(name) = entry
                .get("fn")
                .or_else(|| entry.get("name"))
                .and_then(Value::as_str)
            {
                declared.insert(name.to_owned());
            }
        }
    }
    let mut count = 0;
    for test in frame
        .get_mut("tests")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        budget.charge(1)?;
        if let Some(name) = test["name"].as_str() {
            declared.insert(name.to_owned());
            continue;
        }
        let target = ["fn", "target", "function"]
            .iter()
            .find_map(|key| test[*key].as_str())
            .ok_or_else(|| preserve("anonymous source test has no target"))?;
        let mut index = 1_u32;
        let name = loop {
            budget.charge(1)?;
            let name = format!("t_{target}_{index}");
            if !declared.contains(&name) && names.resolve(&name).is_none() {
                break name;
            }
            index = index
                .checked_add(1)
                .ok_or_else(|| preserve("anonymous test name range exhausted"))?;
        };
        declared.insert(name.clone());
        test["name"] = json!(name);
        count += 1;
    }
    Ok(count)
}

fn applied_deletions(
    accepted: &Program,
    source: &Program,
    names: &Names,
    frame: &mut serde_json::Map<String, Value>,
    budget: &mut Budget,
) -> Result<usize> {
    let mut count = 0;
    if let Some(deletes) = frame.remove("delete") {
        for name in deletes
            .as_array()
            .ok_or_else(|| preserve("source deletions are not a list"))?
        {
            let id = name
                .as_str()
                .and_then(|name| names.resolve(name))
                .filter(|id| names.scope(id) == Scope::Top)
                .ok_or_else(|| preserve("source deletion has no accepted top-level entity"))?;
            absent_tree(accepted, source, id, budget)?;
            count += 1;
        }
    }
    for patch in frame
        .get_mut("patch")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        let function = patch
            .get("fn")
            .or_else(|| patch.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| preserve("source patch has no function"))?
            .to_owned();
        if let Some(blocks) = patch.get_mut("blocks").and_then(Value::as_object_mut) {
            let deletes: Vec<_> = blocks
                .iter()
                .filter(|(_, value)| value.is_null())
                .map(|(name, _)| name.clone())
                .collect();
            for block in deletes {
                let id = names
                    .resolve(&format!("{function}.{block}"))
                    .ok_or_else(|| preserve("source block deletion has no accepted block"))?;
                absent_tree(accepted, source, id, budget)?;
                blocks.remove(&block);
                count += 1;
            }
        }
    }
    Ok(count)
}

fn absent_tree(
    accepted: &Program,
    source: &Program,
    id: EntityId,
    budget: &mut Budget,
) -> Result<()> {
    let mut pending = vec![id];
    let mut seen = BTreeSet::new();
    while let Some(id) = pending.pop() {
        budget.charge(1)?;
        if !seen.insert(id) {
            continue;
        }
        if source.contains(&id) {
            return Err(preserve("an authored deletion remains in the source graph"));
        }
        match accepted.body(&id) {
            Some(EntityBodyValue::Function(body)) => {
                pending.extend(&body.parameters);
                pending.extend(&body.blocks);
                for object in accepted.objects() {
                    budget.charge(1)?;
                    if let EntityBodyValue::TestCase(test) = &object.record().body
                        && test.target == id
                    {
                        pending.push(object.record().entity_id);
                    }
                }
            }
            Some(EntityBodyValue::Block(body)) => {
                pending.extend(&body.parameters);
                pending.extend(&body.operations);
            }
            _ => {}
        }
    }
    Ok(())
}
