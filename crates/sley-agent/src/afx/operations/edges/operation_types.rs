//! Source operation signatures, using ordinary contexts and emitted-use hints.
//! This projects types only; it neither expands nor publishes a candidate graph.
use super::{Arg, FnExp, Inventory, Operand, Term};
use crate::{
    afx::{Node, Stmt, constant_uses},
    error::Result,
    types,
};
use serde_json::json;
use sley_ssmc::TypeExpr;
use std::collections::{BTreeMap, BTreeSet};

mod judge;
pub(super) use judge::retained;

struct Site<'a> {
    node: &'a Node,
    contexts: Vec<Option<TypeExpr>>,
    continuation: Option<TypeExpr>,
}

#[derive(Default)]
struct Sites<'a> {
    nodes: Vec<Site<'a>>,
    literals: Vec<(&'a Arg, Option<TypeExpr>)>,
}

pub(super) fn check(
    view: &FnExp<'_, '_>,
    report: &mut Inventory,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let mut reads = BTreeMap::new();
    for read in &report.reads {
        checkpoint()?;
        if read["result_index_judgment"] == "operation_result_index_checked"
            && read["availability"] != "conflict"
            && let (Some(at), Some(definition)) = (read["at"].as_str(), read["definition"].as_str())
        {
            reads.insert(at.to_owned(), definition.to_owned());
        }
    }
    let hints = constant_uses::function(view, &reads, checkpoint)?;
    let collected = sites(view, report, checkpoint)?;
    check_literals(view, report, &collected, checkpoint)?;
    let sites = collected.nodes;
    let authored: BTreeSet<_> = sites.iter().map(|site| site.node.pointer.clone()).collect();
    let mut values = BTreeMap::new();
    let mut raw = BTreeMap::new();
    // Each round settles at least one operation, or stops. Resolved reads of
    // authored operations never fall back to preparatory constant widths.
    loop {
        let mut changed = false;
        for site in &sites {
            checkpoint()?;
            if raw.contains_key(&site.node.pointer) {
                continue;
            }
            let args = arguments(view, report, site, &reads, &authored, &values);
            let mut projected = site.node.clone();
            for arg in &mut projected.args {
                arg.operand = Operand::Name(String::new());
            }
            let emitted =
                view.node_types_using(&projected, hints.get(&site.node.pointer), &|arg| {
                    site.node
                        .args
                        .iter()
                        .position(|a| a.pointer == arg.pointer)
                        .and_then(|i| args[i].clone())
                });
            if let Some(ty) = emitted.raw {
                raw.insert(site.node.pointer.clone(), ty);
                if let Some(value) = emitted.value {
                    values.insert(site.node.pointer.clone(), value);
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for site in &sites {
        checkpoint()?;
        let args = arguments(view, report, site, &reads, &authored, &values);
        let status = if let (Some(result), Some(args)) = (
            raw.get(&site.node.pointer),
            args.into_iter().collect::<Option<Vec<_>>>(),
        ) {
            judge::check(view, report, site.node, result, &args, checkpoint)?
        } else {
            "type_deferred"
        };
        if let (Some(actual), Some(expected)) = (values.get(&site.node.pointer), &site.continuation)
            && actual != expected
        {
            report.conflicts.push((site.node.pointer.clone(), format!(
                "checked operation emits payload {}, but ordinary continuation inference requires {}",
                view.cx.render(actual), view.cx.render(expected),
            )));
        }
        if status.ends_with("deferred") {
            report.deferred.push(site.node.pointer.clone());
        }
        report
            .operations
            .push(json!({"at":site.node.pointer,"opcode":site.node.row.tag,
            "result_type":raw.get(&site.node.pointer).map(|ty|view.cx.render(ty)),
            "signature":status,"projection":"readonly_types_and_bound_signatures",
            "literal_data_admission":"known_values_use_ordinary_reader_and_canonical_constant_check"}));
    }
    for read in &mut report.reads {
        checkpoint()?;
        if let Some((at, definition)) = read["at"]
            .as_str()
            .and_then(|at| reads.get(at).map(|definition| (at.to_owned(), definition)))
            && authored.contains(definition)
        {
            report.read_types.remove(&at);
            if let Some(ty) = values.get(definition) {
                report.read_types.insert(at.clone(), ty.clone());
            }
            read["actual_type"] = values
                .get(definition)
                .map_or(serde_json::Value::Null, |ty| json!(view.cx.render(ty)));
        }
    }
    report.emitted_values = values;
    Ok(())
}

fn check_literals(
    view: &FnExp<'_, '_>,
    report: &mut Inventory,
    collected: &Sites<'_>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    for (arg, ty) in &collected.literals {
        checkpoint()?;
        let Operand::Literal { value, .. } = &arg.operand else {
            continue;
        };
        if let Some(ty) = ty {
            if view
                .cx
                .literal_value(value, ty, &arg.pointer, checkpoint)?
                .is_none()
            {
                report.deferred.push(arg.pointer.clone());
            }
        } else {
            report.deferred.push(arg.pointer.clone());
        }
    }
    Ok(())
}

fn arguments(
    view: &FnExp<'_, '_>,
    report: &Inventory,
    site: &Site<'_>,
    reads: &BTreeMap<String, String>,
    authored: &BTreeSet<String>,
    values: &BTreeMap<String, TypeExpr>,
) -> Vec<Option<TypeExpr>> {
    site.node
        .args
        .iter()
        .enumerate()
        .map(|(i, arg)| match &arg.operand {
            Operand::Name(_) => {
                if let Some(def) = reads
                    .get(&arg.pointer)
                    .filter(|def| authored.contains(*def))
                {
                    values.get(def).cloned()
                } else {
                    report.read_types.get(&arg.pointer).cloned()
                }
            }
            Operand::Nested(node) => values.get(&node.pointer).cloned(),
            Operand::Literal {
                typed: Some(ty), ..
            } => types::read(ty, view.cx, "").ok(),
            Operand::Literal {
                value: serde_json::Value::Bool(_),
                ..
            } => Some(TypeExpr::Bool),
            Operand::Literal { .. } => site.contexts[i].clone(),
            Operand::Raw(_) => None,
        })
        .collect()
}

fn sites<'a>(
    view: &'a FnExp<'_, '_>,
    report: &Inventory,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Sites<'a>> {
    let mut sites = Sites::default();
    for block in &view.blocks {
        for stmt in &block.stmts {
            checkpoint()?;
            match stmt {
                Stmt::Op(n) => node(view, report, n, None, &mut sites, checkpoint)?,
                Stmt::Exit(exit) => {
                    argument(
                        view,
                        report,
                        &exit.cond,
                        Some(&TypeExpr::Bool),
                        &mut sites,
                        checkpoint,
                    )?;
                    if let Some(arg) = &exit.payload {
                        let ty = view
                            .target_params(&exit.target)
                            .and_then(|params| params.first().and_then(|(_, ty)| ty.clone()))
                            .or_else(|| view.error_case(&exit.target).flatten());
                        argument(view, report, arg, ty.as_ref(), &mut sites, checkpoint)?;
                    }
                }
                Stmt::Raw { .. } => {}
            }
        }
        match &block.term {
            Term::Return(arg) => argument(
                view,
                report,
                arg,
                view.result.as_ref(),
                &mut sites,
                checkpoint,
            )?,
            Term::Cond { cond, then, other } => {
                argument(
                    view,
                    report,
                    cond,
                    Some(&TypeExpr::Bool),
                    &mut sites,
                    checkpoint,
                )?;
                edge(view, report, then, &mut sites, checkpoint)?;
                edge(view, report, other, &mut sites, checkpoint)?;
            }
            Term::Br { target, .. } => edge(view, report, target, &mut sites, checkpoint)?,
            Term::Switch { value, cases } => {
                argument(view, report, value, None, &mut sites, checkpoint)?;
                for (_, target) in cases {
                    edge(view, report, target, &mut sites, checkpoint)?;
                }
            }
            Term::Ok(arg) => {
                let ty = view.result.as_ref().and_then(|result| {
                    if let TypeExpr::Result { ok, .. } = result {
                        Some(ok.as_ref())
                    } else {
                        None
                    }
                });
                argument(view, report, arg, ty, &mut sites, checkpoint)?;
            }
            Term::Fail { case, payload } => {
                let ty = case
                    .as_ref()
                    .and_then(|case| view.error_case(case))
                    .flatten();
                if let Some(arg) = payload {
                    argument(view, report, arg, ty.as_ref(), &mut sites, checkpoint)?;
                }
            }
            Term::Trap { payload, .. } => {
                if let Some(arg) = payload {
                    argument(view, report, arg, None, &mut sites, checkpoint)?;
                }
            }
            Term::Raw(_) | Term::LenientTrap { .. } => {}
        }
    }
    Ok(sites)
}

fn edge<'a>(
    view: &FnExp<'_, '_>,
    report: &Inventory,
    target: &'a super::Target,
    sites: &mut Sites<'a>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let params = view.target_params(&target.block);
    for (i, arg) in target.args.iter().enumerate() {
        let ty = params
            .as_ref()
            .and_then(|params| params.get(i))
            .and_then(|(_, ty)| ty.as_ref());
        argument(view, report, arg, ty, sites, checkpoint)?;
    }
    Ok(())
}

fn argument<'a>(
    view: &FnExp<'_, '_>,
    report: &Inventory,
    arg: &'a Arg,
    expected: Option<&TypeExpr>,
    sites: &mut Sites<'a>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    checkpoint()?;
    match &arg.operand {
        Operand::Nested(n) => node(view, report, n, expected, sites, checkpoint)?,
        Operand::Literal { value, typed } => {
            let ty = match typed {
                Some(ty) => types::read(ty, view.cx, "").ok(),
                None if value.is_boolean() => Some(TypeExpr::Bool),
                None => expected.cloned(),
            };
            sites.literals.push((arg, ty));
        }
        _ => {}
    }
    Ok(())
}

fn node<'a>(
    view: &FnExp<'_, '_>,
    report: &Inventory,
    node: &'a Node,
    expected: Option<&TypeExpr>,
    sites: &mut Sites<'a>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    checkpoint()?;
    let original = view.node_types_using(node, expected, &|arg| {
        report.read_types.get(&arg.pointer).cloned()
    });
    for (arg, context) in node.args.iter().zip(&original.contexts) {
        argument(view, report, arg, context.as_ref(), sites, checkpoint)?;
    }
    sites.nodes.push(Site {
        node,
        contexts: original.contexts,
        continuation: node.check.as_ref().and(original.value),
    });
    Ok(())
}
