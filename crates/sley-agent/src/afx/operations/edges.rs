//! Connect authored edges with declaration types using AF1-X's preparatory
//! inference and complete block-start dominance. No lowering or implicit fill.

use super::super::{Arg, CasePayload, Context, FnExp, Operand, Parser, Target, Term, context};
use crate::{error::Result, types};
use serde_json::{Value, json};
use sley_ssmc::TypeExpr;

mod exits;
mod flow;
mod inputs;
mod operation_types;
mod order;
mod patch;
pub(crate) use patch::patch;

#[derive(Default)]
pub(crate) struct Inventory {
    pub connections: Vec<Value>,
    pub inputs: Vec<Value>,
    pub reads: Vec<Value>,
    pub operations: Vec<Value>,
    read_types: std::collections::BTreeMap<String, TypeExpr>,
    emitted_values: std::collections::BTreeMap<String, TypeExpr>,
    pub cfg: Value,
    flow: Option<flow::Flow>,
    pub deferred: Vec<String>,
    pub conflicts: Vec<(String, String)>,
    pub retained_conflicts: Vec<(String, String)>,
}

pub(crate) fn definition(
    cx: &Context<'_>,
    function: &Value,
    name: &str,
    at: &str,
    dialect: bool,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Inventory> {
    let mut report = Inventory::default();
    let Some(params) =
        context::read_params_with_checkpoint(function.get("params"), cx, checkpoint)?
    else {
        report.deferred.push(format!("{at}/params"));
        return Ok(report);
    };
    let result = function
        .get("returns")
        .and_then(|v| types::read(v, cx, "").ok());
    let mut view = FnExp::new(cx, name, at, false, params, result);
    let Some(blocks) = function.get("blocks").and_then(Value::as_array) else {
        report.deferred.push(format!("{at}/blocks"));
        return Ok(report);
    };
    let mut obligations = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        checkpoint()?;
        let pointer = format!("{at}/blocks/{index}");
        let Some(label) = block.get("name").and_then(Value::as_str) else {
            report.deferred.push(pointer);
            continue;
        };
        let mut parser = Parser {
            obligations: &mut obligations,
        };
        if let Some(block) = parser.block(label, block, &pointer, cx) {
            view.blocks.push(block);
        } else {
            report.deferred.push(pointer);
        }
        checkpoint()?;
    }
    // Parsing/duplicate failures cannot establish a unique declaration view.
    if !obligations.is_empty() || !report.deferred.is_empty() {
        report.deferred.push(at.into());
        return Ok(report);
    }
    view.entry = function.get("entry").map_or_else(
        || {
            view.blocks
                .first()
                .map(|b| b.name.clone())
                .unwrap_or_default()
        },
        |entry| entry.as_str().unwrap_or("").to_owned(),
    );
    prepare(&mut view, &mut report, checkpoint)?;
    if !view.degraded && view.obligations.is_empty() {
        authored(&view, &mut report, dialect, checkpoint)?;
    }
    flow::reachability(&view, &mut report, checkpoint)?;
    Ok(report)
}

fn prepare(
    view: &mut FnExp<'_, '_>,
    report: &mut Inventory,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    view.declare_with_checkpoint(checkpoint)?;
    if view.degraded || !view.obligations.is_empty() {
        report.deferred.push(view.fn_pointer.clone());
        return Ok(());
    }
    // Shared inference requires a common far-candidate type; not availability.
    view.infer_types_with_checkpoint(checkpoint)
}

fn authored(
    view: &FnExp<'_, '_>,
    report: &mut Inventory,
    dialect: bool,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    flow::prepare(view, report, dialect, checkpoint)?;
    order::check(view, report, dialect, checkpoint)?;
    operation_types::check(view, report, checkpoint)?;
    for block in &view.blocks {
        checkpoint()?;
        match &block.term {
            Term::Br { target, .. } => report.edge(view, target, None, dialect, checkpoint)?,
            Term::Cond { cond, then, other } => {
                report.condition(
                    view.cx,
                    operand_type(view, report, cond, Some(&TypeExpr::Bool)).as_ref(),
                    &cond.pointer,
                );
                report.edge(view, then, None, dialect, checkpoint)?;
                report.edge(view, other, None, dialect, checkpoint)?;
            }
            Term::Switch { value, cases } => {
                let scrutinee = operand_type(view, report, value, None);
                let mut keys = Vec::new();
                for (index, (key, _)) in cases.iter().enumerate() {
                    checkpoint()?;
                    keys.push((
                        inputs::Key::authored(key),
                        format!("{}/term/{}/0", block.pointer, index + 2),
                    ));
                }
                report.selector(
                    view.cx,
                    scrutinee.as_ref(),
                    &keys,
                    &value.pointer,
                    checkpoint,
                )?;
                for (key, target) in cases {
                    checkpoint()?;
                    let payload = view.case_payload(key, scrutinee.as_ref());
                    report.edge(view, target, Some(&payload), dialect, checkpoint)?;
                }
            }
            Term::Return(arg) => {
                report.return_value(
                    view.cx,
                    operand_type(view, report, arg, view.result.as_ref()).as_ref(),
                    view.result.as_ref(),
                    &arg.pointer,
                );
            }
            Term::Ok(_) | Term::Fail { .. } if dialect => {
                report.sugar(
                    view,
                    &block.term,
                    &format!("{}/term", block.pointer),
                    checkpoint,
                )?;
            }
            Term::Trap {
                payload: Some(arg), ..
            } => {
                report.trap(
                    view.cx,
                    operand_type(view, report, arg, None).as_ref(),
                    &arg.pointer,
                    checkpoint,
                )?;
            }
            Term::LenientTrap { .. } | Term::Ok(_) | Term::Fail { .. } => {
                report.deferred.push(format!("{}/term", block.pointer));
            }
            Term::Raw(_) => report.deferred.push(format!("{}/term", block.pointer)),
            Term::Trap { payload: None, .. } => {}
        }
    }
    checkpoint()
}

struct Argument {
    at: String,
    ty: Option<TypeExpr>,
    deferred: bool,
    identity: Option<Value>,
}

struct Destination {
    name: String,
    at: String,
    params: Vec<(String, Option<TypeExpr>)>,
}

fn destination(view: &FnExp<'_, '_>, name: &str) -> Option<Destination> {
    let params = view.target_params(name)?;
    let at = view.block_index(name).map_or_else(
        || format!("accepted block `{}.{name}`", view.fn_name),
        |b| view.blocks[b].pointer.clone(),
    );
    Some(Destination {
        name: name.to_owned(),
        at,
        params,
    })
}

impl Inventory {
    #[allow(clippy::too_many_arguments)]
    fn edge(
        &mut self,
        view: &FnExp<'_, '_>,
        target: &Target,
        payload: Option<&CasePayload>,
        dialect: bool,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        checkpoint()?;
        if dialect && target.block.contains("__") {
            self.deferred
                .push(super::topology::target_pointer(target, 1));
            return Ok(());
        }
        let Some(destination) = destination(view, &target.block) else {
            self.deferred.push(target.pointer.clone());
            return Ok(());
        };
        if let Some(CasePayload::NotACase(reason)) = payload {
            self.conflicts
                .push((target.pointer.clone(), reason.clone()));
            return Ok(());
        }
        let mut arguments = Vec::new();
        for (index, arg) in target.args.iter().enumerate() {
            checkpoint()?;
            let expected = destination
                .params
                .get(index)
                .and_then(|(_, ty)| ty.as_ref());
            let actual = if matches!(&arg.operand, Operand::Name(name) if name == "$") {
                match payload {
                    Some(CasePayload::Carries(ty)) => ty.clone(),
                    Some(CasePayload::Unit) => {
                        self.conflicts.push((arg.pointer.clone(), format!(
                            "argument {index} for target block `{}` requests a switch payload from a unit case", target.block
                        )));
                        continue;
                    }
                    _ => None,
                }
            } else {
                operand_type(view, self, arg, expected)
            };
            arguments.push(Argument {
                at: arg.pointer.clone(),
                ty: actual,
                deferred: matches!(arg.operand, Operand::Literal { .. } | Operand::Raw(_)),
                identity: None,
            });
        }
        self.connect(
            view.cx,
            &destination,
            &arguments,
            &target.pointer,
            dialect,
            checkpoint,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn connect(
        &mut self,
        cx: &Context<'_>,
        destination: &Destination,
        args: &[Argument],
        at: &str,
        allow_fill: bool,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        checkpoint()?;
        let supplied = args.len();
        let params = &destination.params;
        if supplied > params.len() || (!allow_fill && supplied != params.len()) {
            self.conflicts.push((at.into(),format!(
                "target block `{}` takes {} arguments, but this edge explicitly supplies {supplied}",destination.name,params.len())));
            return Ok(());
        }
        let mut arguments = Vec::new();
        for (index, (arg, (_, expected))) in args.iter().zip(params).enumerate() {
            checkpoint()?;
            let actual = &arg.ty;
            if let (Some(actual), Some(expected)) = (actual, expected)
                && actual != expected
            {
                self.conflicts.push((arg.at.clone(),format!(
                    "argument {index} for target block `{}` has type {}, but its parameter at {}/params/{index}/1 requires {}",
                    destination.name,cx.render(actual),destination.at,cx.render(expected))));
            }
            if actual.is_none() || expected.is_none() || arg.deferred {
                self.deferred.push(arg.at.clone());
            }
            arguments.push(json!({"at":arg.at,"parameter":format!("{}/params/{index}",destination.at),
                "actual_type":actual.as_ref().map(|ty|cx.render(ty)),"expected_type":expected.as_ref().map(|ty|cx.render(ty)),
                "argument_identity":arg.identity,
                "type_connection":if actual.is_some()&&expected.is_some(){"checked"}else{"deferred"}}));
        }
        if supplied < params.len() {
            self.deferred.push(at.into());
        }
        self.connections.push(json!({"at":at,"target":destination.name,"parameters":params.len(),"explicit_arguments":supplied,
            "arguments":arguments,"trailing_arguments":if supplied<params.len(){"afx_derivation_deferred"}else{"none"},"rewritten":false}));
        Ok(())
    }
}

fn operand_type(
    view: &FnExp<'_, '_>,
    report: &Inventory,
    arg: &Arg,
    expected: Option<&TypeExpr>,
) -> Option<TypeExpr> {
    match &arg.operand {
        Operand::Name(_) => report.read_types.get(&arg.pointer).cloned(),
        Operand::Literal {
            typed: Some(ty), ..
        } => types::read(ty, view.cx, "").ok(),
        Operand::Literal {
            value: Value::Bool(_),
            ..
        } => Some(TypeExpr::Bool),
        Operand::Nested(node) => {
            view.node_types_using(node, expected, &|read| {
                report.read_types.get(&read.pointer).cloned()
            })
            .value
        }
        // Untyped literals require admission and bounds checks, not assumed types.
        Operand::Literal { .. } | Operand::Raw(_) => None,
    }
}
