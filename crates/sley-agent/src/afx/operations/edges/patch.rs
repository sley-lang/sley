//! Patch declaration overlay and retained edges preserve accepted value identity.

use super::{Argument, Inventory, authored, destination, prepare};
use crate::{
    afx::{CasePayload, Context, FnExp, Kept, Kind, Parser, context},
    error::Result,
    types,
};
use serde_json::{Map, Value, json};
use sley_id::EntityId;
use sley_mutate::value::EntityBodyValue;
use sley_ssmc::{BuiltinCase, CaseKey, SwitchArgument, Terminator, TypeExpr, ValueRef};
use std::collections::{BTreeMap, BTreeSet};

mod reads;
mod retention;

enum ValueState {
    Types(Vec<Option<TypeExpr>>),
    Deleted,
    Deferred,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn patch(
    cx: &Context<'_>,
    value: &Value,
    name: &str,
    at: &str,
    dialect: bool,
    retained: &[EntityId],
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Inventory> {
    checkpoint()?;
    let mut report = Inventory::default();
    let Some((_, function)) = cx.live_function(name) else {
        report.deferred.push(at.into());
        return Ok(report);
    };
    let empty = Map::new();
    let patches = match value.get("blocks") {
        None => &empty,
        Some(Value::Object(blocks)) => blocks,
        _ => {
            report.deferred.push(format!("{at}/blocks"));
            return Ok(report);
        }
    };
    let params = if value.get("params").is_some() {
        let Some(params) =
            context::read_params_with_checkpoint(value.get("params"), cx, checkpoint)?
        else {
            report.deferred.push(format!("{at}/params"));
            return Ok(report);
        };
        params
    } else {
        let mut params = Vec::new();
        for id in &function.parameters {
            checkpoint()?;
            params.push((cx.names.leaf(id), cx.parameter_type(id), Value::Null));
        }
        params
    };
    let result = value.get("returns").map_or_else(
        || Some(function.result_type.clone()),
        |v| types::read(v, cx, "").ok(),
    );
    let (current_retained, deferred_exits) =
        retention::current(cx, &function.blocks, retained, dialect, checkpoint)?;
    let retained = current_retained.as_slice();
    let mut view = FnExp::new(cx, name, at, true, params, result);
    for id in retained {
        checkpoint()?;
        let Some(EntityBodyValue::Block(body)) = cx.program.body(id) else {
            report.deferred.push(at.into());
            continue;
        };
        let mut params = Vec::new();
        let mut ops = Vec::new();
        for id in &body.parameters {
            checkpoint()?;
            params.push((cx.names.written_leaf(id), cx.parameter_type(id)));
        }
        for id in &body.operations {
            checkpoint()?;
            let ty = match cx.program.body(id) {
                Some(EntityBodyValue::Operation(op)) => op.result_types.first().cloned(),
                _ => None,
            };
            ops.push((cx.names.leaf(id), ty));
        }
        view.kept.push(Kept {
            leaf: cx.names.leaf(id),
            params,
            ops,
            targets: super::flow::targets(&body.terminator)
                .iter()
                .map(|target| cx.names.leaf(target))
                .collect(),
        });
    }
    let mut obligations = Vec::new();
    for (label, body) in patches {
        checkpoint()?;
        if body.is_null() {
            continue;
        }
        let pointer = format!("{at}/blocks/{}", escape(label));
        let mut parser = Parser {
            obligations: &mut obligations,
        };
        if let Some(block) = parser.block(label, body, &pointer, cx) {
            view.blocks.push(block);
        } else {
            report.deferred.push(pointer);
        }
        checkpoint()?;
    }
    if !obligations.is_empty() || !report.deferred.is_empty() {
        report.deferred.push(at.into());
        return Ok(report);
    }
    report.deferred.extend(deferred_exits);
    view.entry = value.get("entry").map_or_else(
        || cx.names.leaf(&function.entry_block),
        |entry| entry.as_str().unwrap_or("").to_owned(),
    );
    prepare(&mut view, &mut report, checkpoint)?;
    if view.degraded || !view.obligations.is_empty() {
        return Ok(report);
    }
    authored(&view, &mut report, dialect, checkpoint)?;
    let states = overlay(
        cx,
        &view,
        &report,
        &function.parameters,
        &function.blocks,
        retained,
        dialect,
        checkpoint,
    )?;
    reads::check(cx, &view, &states, retained, &mut report, checkpoint)?;
    for id in retained {
        checkpoint()?;
        let Some(EntityBodyValue::Block(body)) = cx.program.body(id) else {
            continue;
        };
        let pointer = format!("{at} (retained block `{}` terminator)", cx.names.name(id));
        match &body.terminator {
            Terminator::Branch(term) => {
                let args = term
                    .edge
                    .arguments
                    .iter()
                    .copied()
                    .map(SwitchArgument::Value)
                    .collect::<Vec<_>>();
                retained_edge(
                    &view,
                    &states,
                    &mut report,
                    term.edge.target,
                    &args,
                    None,
                    &pointer,
                    dialect,
                    checkpoint,
                )?;
            }
            Terminator::CondBranch(term) => {
                let at = format!("{pointer}/condition");
                let ty = value_type(cx, &states, term.condition, &at, &mut report);
                report.condition(cx, ty.as_ref(), &at);
                for edge in [&term.if_true, &term.if_false] {
                    checkpoint()?;
                    let args = edge
                        .arguments
                        .iter()
                        .copied()
                        .map(SwitchArgument::Value)
                        .collect::<Vec<_>>();
                    retained_edge(
                        &view,
                        &states,
                        &mut report,
                        edge.target,
                        &args,
                        None,
                        &pointer,
                        dialect,
                        checkpoint,
                    )?;
                }
            }
            Terminator::VariantSwitch(term) => {
                let scrutinee = value_type(cx, &states, term.value, &pointer, &mut report);
                let mut keys = Vec::new();
                for (index, case) in term.cases.iter().enumerate() {
                    checkpoint()?;
                    keys.push((
                        super::inputs::Key::retained(cx, case.case_key, scrutinee.as_ref()),
                        format!("{pointer}/cases/{index}"),
                    ));
                }
                report.selector(cx, scrutinee.as_ref(), &keys, &pointer, checkpoint)?;
                for case in &term.cases {
                    checkpoint()?;
                    let key = case_name(cx, case.case_key, scrutinee.as_ref());
                    let payload = key.as_ref().map(|key| {
                        view.case_payload(&Value::String(key.clone()), scrutinee.as_ref())
                    });
                    retained_edge(
                        &view,
                        &states,
                        &mut report,
                        case.edge.target,
                        &case.edge.arguments,
                        payload.as_ref(),
                        &pointer,
                        dialect,
                        checkpoint,
                    )?;
                }
            }
            Terminator::Return(term) => {
                let at = format!("{pointer}/return");
                let ty = value_type(cx, &states, term.value, &at, &mut report);
                report.return_value(cx, ty.as_ref(), view.result.as_ref(), &at);
            }
            Terminator::Trap(term) => {
                if let Some(payload) = term.payload {
                    let at = format!("{pointer}/payload");
                    let ty = value_type(cx, &states, payload, &at, &mut report);
                    report.trap(cx, ty.as_ref(), &at, checkpoint)?;
                }
            }
        }
    }
    super::flow::reachability(&view, &mut report, checkpoint)?;
    checkpoint()?;
    Ok(report)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn overlay(
    cx: &Context<'_>,
    view: &FnExp<'_, '_>,
    report: &Inventory,
    parameters: &[EntityId],
    blocks: &[EntityId],
    retained: &[EntityId],
    dialect: bool,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<BTreeMap<EntityId, ValueState>> {
    let mut states = BTreeMap::new();
    let mut live = BTreeSet::new();
    for id in retained {
        checkpoint()?;
        live.insert(*id);
    }
    let mut function_params = BTreeMap::new();
    for (name, ty) in &view.params {
        checkpoint()?;
        function_params.insert(name, ty);
    }
    for id in parameters {
        checkpoint()?;
        states.insert(
            *id,
            function_params
                .get(&cx.names.leaf(id))
                .map_or(ValueState::Deleted, |ty| {
                    ValueState::Types(vec![(*ty).clone()])
                }),
        );
    }
    for block in blocks {
        checkpoint()?;
        let Some(EntityBodyValue::Block(body)) = cx.program.body(block) else {
            continue;
        };
        let label = cx.names.leaf(block);
        let authored = view.block_index(&label);
        let mut parameters = BTreeMap::new();
        if let Some(b) = authored {
            for (name, ty, _) in &view.blocks[b].params {
                checkpoint()?;
                parameters.insert(name, ty);
            }
        }
        for id in &body.parameters {
            checkpoint()?;
            let state = if live.contains(block) {
                ValueState::Types(vec![cx.parameter_type(id)])
            } else if authored.is_some() {
                parameters
                    .get(&cx.names.leaf(id))
                    .map_or(ValueState::Deleted, |ty| {
                        ValueState::Types(vec![(*ty).clone()])
                    })
            } else if dialect && label.contains("__") {
                ValueState::Deferred
            } else {
                ValueState::Deleted
            };
            states.insert(*id, state);
        }
        for id in &body.operations {
            checkpoint()?;
            let leaf = cx.names.leaf(id);
            let state = if live.contains(block) {
                match cx.program.body(id) {
                    Some(EntityBodyValue::Operation(op)) => {
                        ValueState::Types(op.result_types.iter().cloned().map(Some).collect())
                    }
                    _ => ValueState::Deferred,
                }
            } else if dialect && (label.contains("__") || leaf.contains("__")) {
                ValueState::Deferred
            } else if let Some(b) = authored {
                // Only an ordinary operation before every split is emitted in
                // the original block. Moving it does not move its old identity.
                match view.defs[b].get(&leaf) {
                    Some(def) if def.kind == Kind::Op && (!dialect || def.first) => {
                        ValueState::Types(vec![view.blocks[b].stmts.iter().find_map(|stmt| {
                            if let crate::afx::Stmt::Op(node) = stmt {
                                (node.name.as_deref() == Some(leaf.as_str()))
                                    .then(|| report.emitted_values.get(&node.pointer).cloned())
                                    .flatten()
                            } else {
                                None
                            }
                        })])
                    }
                    _ => ValueState::Deleted,
                }
            } else {
                ValueState::Deleted
            };
            states.insert(*id, state);
        }
    }
    Ok(states)
}

fn value_type(
    cx: &Context<'_>,
    states: &BTreeMap<EntityId, ValueState>,
    value: ValueRef,
    at: &str,
    report: &mut Inventory,
) -> Option<TypeExpr> {
    let (id, index) = match value {
        ValueRef::Parameter(id) => (id, 0),
        ValueRef::OperationResult(r) => (
            r.operation,
            usize::try_from(r.result_index).unwrap_or(usize::MAX),
        ),
    };
    match states.get(&id) {
        Some(ValueState::Types(types)) => {
            if let Some(ty) = types.get(index) {
                ty.clone()
            } else {
                report.conflicts.push((
                    at.into(),
                    format!(
                        "retained argument `{}` has no result {index} after this patch",
                        cx.names.name(&id)
                    ),
                ));
                None
            }
        }
        Some(ValueState::Deleted) => {
            report.conflicts.push((at.into(),format!("retained argument `{}` loses its original identity after this patch; restate its consuming block",cx.names.name(&id))));
            None
        }
        Some(ValueState::Deferred) | None => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn retained_edge(
    view: &FnExp<'_, '_>,
    states: &BTreeMap<EntityId, ValueState>,
    report: &mut Inventory,
    target: EntityId,
    args: &[SwitchArgument],
    payload: Option<&CasePayload>,
    at: &str,
    dialect: bool,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    checkpoint()?;
    let label = view.cx.names.leaf(&target);
    if dialect && label.contains("__") {
        report.deferred.push(at.into());
        return Ok(());
    }
    let Some(destination) = destination(view, &label) else {
        report.deferred.push(at.into());
        return Ok(());
    };
    if let Some(CasePayload::NotACase(reason)) = payload {
        report.conflicts.push((at.into(), reason.clone()));
        return Ok(());
    }
    let mut arguments = Vec::new();
    for (index, arg) in args.iter().enumerate() {
        checkpoint()?;
        let pointer = format!("{at}/arguments/{index}");
        let (ty, identity) = match arg {
            SwitchArgument::Value(value) => {
                let ty = value_type(view.cx, states, *value, &pointer, report);
                let identity = match value {
                    ValueRef::Parameter(id) => {
                        json!({"kind":"parameter","id":crate::hex::encode(id.as_bytes()),"name":view.cx.names.name(id)})
                    }
                    ValueRef::OperationResult(r) => {
                        json!({"kind":"operation_result","id":crate::hex::encode(r.operation.as_bytes()),"name":view.cx.names.name(&r.operation),"result_index":r.result_index})
                    }
                };
                (ty, Some(identity))
            }
            SwitchArgument::CasePayload => {
                match payload {
                    Some(CasePayload::Carries(ty)) => {
                        (ty.clone(), Some(json!({"kind":"case_payload"})))
                    }
                    Some(CasePayload::Unit) => {
                        report.conflicts.push((pointer.clone(),format!("retained argument for `{label}` requests a payload from a unit case")));
                        (None, None)
                    }
                    _ => (None, None),
                }
            }
        };
        arguments.push(Argument {
            at: pointer,
            ty,
            deferred: false,
            identity,
        });
    }
    // A retained canonical edge is already explicit; X4 never fills it.
    report.connect(view.cx, &destination, &arguments, at, false, checkpoint)
}

fn case_name(cx: &Context<'_>, key: CaseKey, scrutinee: Option<&TypeExpr>) -> Option<String> {
    match key {
        CaseKey::Builtin(case) => Some(
            match case {
                BuiltinCase::Some => "Some",
                BuiltinCase::None => "None",
                BuiltinCase::Ok => "Ok",
                BuiltinCase::Err => "Err",
            }
            .into(),
        ),
        CaseKey::Member(member) => match scrutinee {
            Some(ty @ TypeExpr::Named(named)) => Some(format!(
                "{}.{}",
                cx.render(ty),
                cx.names.member_leaf(&named.definition, &member)
            )),
            _ => None,
        },
    }
}
fn escape(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}
