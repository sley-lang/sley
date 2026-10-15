//! Read authored operations using the AF1-X parser, without lowering a block.

use super::{Arg, Context, EntityId, Node, Operand, Parser, Result, Stmt, Term, Value};

pub(crate) mod continuations;
pub(crate) mod edges;
mod edit;
mod patch;
pub(crate) mod topology;
pub(crate) use edit::edits;
pub(crate) use patch::patch;

pub(crate) struct Operation {
    pub tag: u32,
    pub callee: Option<String>,
    pub callee_id: Option<EntityId>,
    pub retained: Option<EntityId>,
    pub restated: bool,
    pub at: String,
}

#[derive(Default)]
pub(crate) struct Inventory {
    pub operations: Vec<Operation>,
    pub deferred: Vec<String>,
}

/// Parse one complete authored block. Unknown syntax and parser obligations
/// remain deferred; literal payloads and opcode immediates are never traversed.
pub(crate) fn block(
    cx: &Context<'_>,
    value: &Value,
    at: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Inventory> {
    let name = value.get("name").and_then(Value::as_str).unwrap_or("");
    named_block(cx, value, name, at, checkpoint)
}

fn named_block(
    cx: &Context<'_>,
    value: &Value,
    name: &str,
    at: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Inventory> {
    checkpoint()?;
    let mut obligations = Vec::new();
    let mut parser = Parser {
        obligations: &mut obligations,
    };
    let parsed = parser.block(name, value, at, cx);
    let mut inventory = Inventory::default();
    checkpoint()?;
    if !obligations.is_empty() {
        inventory.deferred.push(at.into());
    }
    let Some(parsed) = parsed else {
        inventory.deferred.push(at.into());
        return Ok(inventory);
    };
    for statement in &parsed.stmts {
        checkpoint()?;
        match statement {
            Stmt::Op(node) => inventory.node(node, checkpoint)?,
            Stmt::Exit(exit) => {
                inventory.arg(&exit.cond, checkpoint)?;
                if let Some(payload) = &exit.payload {
                    inventory.arg(payload, checkpoint)?;
                }
            }
            Stmt::Raw { pointer, .. } => inventory.deferred.push(pointer.clone()),
        }
    }
    let args: Vec<&Arg> = match &parsed.term {
        Term::Return(arg) | Term::Ok(arg) => vec![arg],
        Term::Br { target, .. } => target.args.iter().collect(),
        Term::Cond { cond, then, other } => std::iter::once(cond)
            .chain(&then.args)
            .chain(&other.args)
            .collect(),
        Term::Switch { value, cases } => std::iter::once(value)
            .chain(cases.iter().flat_map(|(_, target)| &target.args))
            .collect(),
        Term::Trap { payload, .. } | Term::Fail { payload, .. } => payload.iter().collect(),
        Term::LenientTrap { .. } | Term::Raw(_) => {
            inventory.deferred.push(format!("{at}/term"));
            Vec::new()
        }
    };
    for arg in args {
        inventory.arg(arg, checkpoint)?;
    }
    Ok(inventory)
}

impl Inventory {
    fn node(&mut self, node: &Node, checkpoint: &mut impl FnMut() -> Result<()>) -> Result<()> {
        checkpoint()?;
        self.operations.push(Operation {
            tag: node.row.tag,
            callee: (node.row.tag == 112)
                .then(|| {
                    node.immediate
                        .as_ref()
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .flatten(),
            at: node.pointer.clone(),
            callee_id: None,
            retained: None,
            restated: false,
        });
        for arg in &node.args {
            self.arg(arg, checkpoint)?;
        }
        Ok(())
    }

    fn arg(&mut self, arg: &Arg, checkpoint: &mut impl FnMut() -> Result<()>) -> Result<()> {
        checkpoint()?;
        match &arg.operand {
            Operand::Nested(node) => self.node(node, checkpoint)?,
            Operand::Raw(_) => self.deferred.push(arg.pointer.clone()),
            Operand::Name(_) | Operand::Literal { .. } => {}
        }
        Ok(())
    }
}
