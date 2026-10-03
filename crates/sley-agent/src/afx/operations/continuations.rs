//! Read ordinary literal contexts and checked payloads without lowering blocks.
use super::super::constant_uses::End;
use super::super::{Arg, Context, FnExp, Node, Operand, Parser, Stmt};
use crate::error::Result;
use serde_json::{Value, json};
use sley_ssmc::TypeExpr;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(crate) struct Inventory {
    pub payloads: BTreeMap<String, TypeExpr>,
    pub literals: BTreeMap<String, TypeExpr>,
    pub untyped_literals: BTreeSet<String>,
}

pub(crate) fn failure_payload(
    cx: &Context<'_>,
    output: &TypeExpr,
    route: &str,
) -> Option<TypeExpr> {
    let TypeExpr::Result { error, .. } = output else {
        return None;
    };
    let TypeExpr::Named(named) = &**error else {
        return None;
    };
    cx.member_type(&named.definition, route)?
        .map(|ty| crate::values::substitute(&ty, &named.arguments))
}

pub(crate) fn region(
    cx: &Context<'_>,
    params: Vec<(String, Option<TypeExpr>, Value)>,
    output: &TypeExpr,
    ops: &Value,
    at: &str,
    ends: &[End<'_>],
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Inventory> {
    checkpoint()?;
    let mut obligations = Vec::new();
    let mut parser = Parser {
        obligations: &mut obligations,
    };
    let Some(block) = parser.block("region", &json!({"ops":ops,"term":["trap"]}), at, cx) else {
        return Ok(Inventory::default());
    };
    let endings = ends
        .iter()
        .map(|end| parser.arg(end.value, end.at.clone()))
        .collect::<Vec<_>>();
    if !obligations.is_empty() {
        return Ok(Inventory::default());
    }
    let mut view = FnExp::new(cx, "region", at, false, params, Some(output.clone()));
    view.entry = "region".into();
    view.blocks.push(block);
    inspect(
        view,
        endings
            .iter()
            .zip(ends)
            .map(|(arg, end)| (arg, end.context.as_ref())),
        checkpoint,
    )
}

/// Query the ordinary checked arithmetic AST using original pipeline locators.
/// This creates no fragment blocks, graph entities or inferred type annotations.
pub(crate) fn pipeline(
    cx: &Context<'_>,
    params: Vec<(String, Option<TypeExpr>, Value)>,
    output: &TypeExpr,
    steps: &[Value],
    result: &Value,
    at: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Inventory> {
    checkpoint()?;
    let mut obligations = Vec::new();
    let mut parser = Parser {
        obligations: &mut obligations,
    };
    let Some(mut block) = parser.block("region", &json!({"ops":[],"term":["trap"]}), at, cx) else {
        return Ok(Inventory::default());
    };
    for (index, step) in steps.iter().enumerate() {
        checkpoint()?;
        let Some(items) = step[1].as_array() else {
            return Ok(Inventory::default());
        };
        let Some(word) = items.first().and_then(Value::as_str) else {
            return Ok(Inventory::default());
        };
        let pointer = format!("{at}/steps/{index}/1");
        let Some(mut node) = parser.node(
            step[0].as_str(),
            word,
            &items[1..],
            None,
            &pointer,
            &|index| format!("{pointer}/{}", index + 1),
            0,
        ) else {
            return Ok(Inventory::default());
        };
        checked_arithmetic(&mut node, checkpoint)?;
        block.stmts.push(Stmt::Op(node));
    }
    let result = parser.arg(result, format!("{at}/result"));
    if !obligations.is_empty() {
        return Ok(Inventory::default());
    }
    let mut view = FnExp::new(cx, "region", at, false, params, Some(output.clone()));
    view.entry = "region".into();
    view.blocks.push(block);
    // The fragment emits `ok result`; the ordinary endpoint reader supplies
    // only its payload context, not a backward constraint on previous steps.
    let context = match output {
        TypeExpr::Result { ok, .. } => Some(ok.as_ref()),
        _ => None,
    };
    inspect(view, std::iter::once((&result, context)), checkpoint)
}

fn checked_arithmetic(node: &mut Node, checkpoint: &mut impl FnMut() -> Result<()>) -> Result<()> {
    checkpoint()?;
    // The closed pipeline grammar has already been checked. Like its expander,
    // every arithmetic node has a checked continuation; its named failure
    // mapping does not supply an integer literal context.
    node.check = Some(String::new());
    for arg in &mut node.args {
        checkpoint()?;
        if let Operand::Nested(nested) = &mut arg.operand {
            checked_arithmetic(nested, checkpoint)?;
        }
    }
    Ok(())
}

fn inspect<'a>(
    mut view: FnExp<'_, '_>,
    ends: impl IntoIterator<Item = (&'a Arg, Option<&'a TypeExpr>)>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Inventory> {
    view.declare_with_checkpoint(checkpoint)?;
    if view.degraded || !view.obligations.is_empty() {
        return Ok(Inventory::default());
    }
    view.infer_types_with_checkpoint(checkpoint)?;
    let mut report = Inventory::default();
    for stmt in &view.blocks[0].stmts {
        checkpoint()?;
        match stmt {
            Stmt::Op(node) => report.node(&view, node, None, checkpoint)?,
            Stmt::Exit(exit) => {
                report.arg(&view, &exit.cond, Some(&TypeExpr::Bool), checkpoint)?;
                if let Some(payload) = &exit.payload {
                    let ty = view
                        .result
                        .as_ref()
                        .and_then(|output| failure_payload(view.cx, output, &exit.target));
                    report.arg(&view, payload, ty.as_ref(), checkpoint)?;
                }
            }
            Stmt::Raw { .. } => {}
        }
    }
    for (arg, context) in ends {
        report.arg(&view, arg, context, checkpoint)?;
    }
    Ok(report)
}

impl Inventory {
    fn node(
        &mut self,
        view: &FnExp<'_, '_>,
        node: &Node,
        expected: Option<&TypeExpr>,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        checkpoint()?;
        let types = view.node_types(0, node, expected);
        for (arg, context) in node.args.iter().zip(&types.contexts) {
            self.arg(view, arg, context.as_ref(), checkpoint)?;
        }
        if node.check.is_some()
            && let Some(ty) = types.value
        {
            self.payloads.insert(node.pointer.clone(), ty);
        }
        Ok(())
    }

    fn arg(
        &mut self,
        view: &FnExp<'_, '_>,
        arg: &Arg,
        context: Option<&TypeExpr>,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        checkpoint()?;
        match &arg.operand {
            Operand::Nested(node) => self.node(view, node, context, checkpoint)?,
            Operand::Literal { value, typed: None } if !value.is_boolean() => {
                if let Some(ty) = context {
                    self.literals.insert(arg.pointer.clone(), ty.clone());
                } else {
                    self.untyped_literals.insert(arg.pointer.clone());
                }
            }
            _ => {}
        }
        Ok(())
    }
}
