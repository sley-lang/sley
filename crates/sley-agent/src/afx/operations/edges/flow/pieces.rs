//! Symbolic continuation slots; no generated names, entities or lowering.

use super::{FnExp, route};
use crate::afx::{Arg, Node, Operand, Stmt, Term, term_args};
use crate::error::Result;
use sley_check::cfg::MAX_CFG_BLOCKS;
use std::collections::BTreeMap;

pub(super) fn project(
    view: &FnExp<'_, '_>,
    names: &BTreeMap<String, usize>,
    successors: &mut Vec<Vec<usize>>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<BTreeMap<String, usize>> {
    let mut walk = Walk {
        view,
        names,
        successors,
        sites: BTreeMap::new(),
        current: 0,
        checkpoint,
    };
    for (b, block) in view.blocks.iter().enumerate() {
        (walk.checkpoint)()?;
        walk.current = b;
        walk.successors[b].clear();
        for stmt in &block.stmts {
            (walk.checkpoint)()?;
            match stmt {
                Stmt::Op(node) => walk.node(node)?,
                Stmt::Exit(exit) => {
                    walk.arg(&exit.cond)?;
                    if let Some(arg) = &exit.payload {
                        walk.arg(arg)?;
                    }
                    walk.reads(std::iter::once(&exit.cond).chain(exit.payload.iter()))?;
                    walk.split(&exit.target)?;
                }
                Stmt::Raw { .. } => unreachable!("complete projection excludes raw syntax"),
            }
        }
        let args = term_args(&block.term);
        for arg in &args {
            walk.arg(arg)?;
        }
        walk.reads(args.into_iter())?;
        let targets = match &block.term {
            Term::Br { target, .. } => vec![&target.block],
            Term::Cond { then, other, .. } => vec![&then.block, &other.block],
            Term::Switch { cases, .. } => cases.iter().map(|(_, target)| &target.block).collect(),
            _ => Vec::new(),
        };
        for name in targets {
            (walk.checkpoint)()?;
            walk.successors[walk.current].push(names[name]);
        }
    }
    Ok(walk.sites)
}

struct Walk<'v, 'c, 'a, 's, F> {
    view: &'v FnExp<'c, 'a>,
    names: &'s BTreeMap<String, usize>,
    successors: &'s mut Vec<Vec<usize>>,
    sites: BTreeMap<String, usize>,
    current: usize,
    checkpoint: &'s mut F,
}

impl<F: FnMut() -> Result<()>> Walk<'_, '_, '_, '_, F> {
    fn node(&mut self, node: &Node) -> Result<()> {
        (self.checkpoint)()?;
        for arg in &node.args {
            self.arg(arg)?;
        }
        // Direct operands are consumed by this operation after nested operands
        // have been evaluated, matching the ordinary expander's finish_node.
        self.reads(node.args.iter())?;
        if let Some(handler) = &node.check {
            self.split(handler)?;
        }
        self.sites.insert(node.pointer.clone(), self.current);
        (self.checkpoint)()
    }

    fn arg(&mut self, arg: &Arg) -> Result<()> {
        (self.checkpoint)()?;
        if let Operand::Nested(node) = &arg.operand {
            self.node(node)?;
        }
        Ok(())
    }

    fn reads<'b>(&mut self, args: impl Iterator<Item = &'b Arg>) -> Result<()> {
        for arg in args {
            (self.checkpoint)()?;
            if matches!(arg.operand, Operand::Name(_)) {
                self.sites.insert(arg.pointer.clone(), self.current);
            }
        }
        Ok(())
    }

    fn split(&mut self, handler: &str) -> Result<()> {
        (self.checkpoint)()?;
        let next = self.successors.len();
        if next >= MAX_CFG_BLOCKS {
            return Err(super::limit(
                "symbolic continuation projection exceeds the canonical CFG block bound",
            ));
        }
        let mut targets = Vec::new();
        assert!(
            route(self.view, handler, &mut targets),
            "complete projection qualified the route"
        );
        for name in targets {
            self.successors[self.current].push(self.names[&name]);
        }
        self.successors[self.current].push(next);
        self.successors.push(Vec::new());
        self.current = next;
        (self.checkpoint)()
    }
}
