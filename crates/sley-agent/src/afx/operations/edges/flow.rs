//! Read-only named block starts and symbolic authored continuation slots.

use super::{FnExp, Inventory};
use crate::afx::{ABlock, Arg, Node, Operand, Stmt, Term, term_args};
use crate::error::{AgentError, AgentErrorCode, Result};
use serde_json::json;
use sley_check::cfg::{MAX_CFG_BLOCKS, MAX_CFG_EDGES};
use sley_ssmc::{Terminator, TypeExpr};
use std::collections::{BTreeMap, VecDeque};

mod pieces;
mod reach;

pub(super) struct Flow {
    names: BTreeMap<String, usize>,
    successors: Vec<Vec<usize>>,
    sites: BTreeMap<String, usize>,
    entry: usize,
    reachable: Vec<bool>,
    without: BTreeMap<usize, Vec<bool>>,
}

pub(super) fn targets(term: &Terminator) -> Vec<sley_id::EntityId> {
    match term {
        Terminator::Branch(t) => vec![t.edge.target],
        Terminator::CondBranch(t) => vec![t.if_true.target, t.if_false.target],
        Terminator::VariantSwitch(t) => t.cases.iter().map(|case| case.edge.target).collect(),
        Terminator::Return(_) | Terminator::Trap(_) => Vec::new(),
    }
}

pub(super) fn prepare(
    view: &FnExp<'_, '_>,
    report: &mut Inventory,
    dialect: bool,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    report.flow = Flow::build(view, dialect, checkpoint)?;
    report.cfg=report.flow.as_ref().map_or_else(||json!({"status":"deferred","authority":"none"}),|flow|json!({"status":"complete_block_start_projection","entry":view.entry,"blocks":flow.names,"edges":flow.successors,"reachable":flow.reachable,"definition_scope":"authored_symbolic_split_slots_or_retained_canonical_operation","symbolic_sites":flow.sites,"split_boundaries":"symbolic_continuations_checked; shared_terminal_identity_and_generated_flags_deferred","authority":"none"}));
    Ok(())
}

pub(super) fn reachability(
    view: &FnExp<'_, '_>,
    report: &mut Inventory,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let declarations = reach::check(view, report, checkpoint)?;
    if let Some(cfg) = report.cfg.as_object_mut() {
        cfg.insert("reachability_declarations".into(), json!(declarations));
    }
    Ok(())
}

impl Flow {
    fn build(
        view: &FnExp<'_, '_>,
        dialect: bool,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<Self>> {
        checkpoint()?;
        if view.entry.is_empty() || (dialect && view.entry.contains("__")) {
            return Ok(None);
        }
        let mut names = BTreeMap::new();
        for name in view
            .blocks
            .iter()
            .map(|b| &b.name)
            .chain(view.kept.iter().map(|b| &b.leaf))
        {
            checkpoint()?;
            let index = names.len();
            if names.insert(name.clone(), index).is_some() {
                return Ok(None);
            }
            if names.len() > MAX_CFG_BLOCKS {
                return Err(limit(
                    "block projection exceeds the canonical CFG block bound",
                ));
            }
        }
        let Some(&entry) = names.get(&view.entry) else {
            return Ok(None);
        };
        let mut successors = vec![Vec::new(); names.len()];
        let mut edges = 0;
        for (b, block) in view.blocks.iter().enumerate() {
            checkpoint()?;
            let Some(out) = authored_targets(view, block, dialect, checkpoint)? else {
                return Ok(None);
            };
            for name in out {
                checkpoint()?;
                if dialect && name.contains("__") {
                    return Ok(None);
                }
                let Some(&target) = names.get(&name) else {
                    return Ok(None);
                };
                successors[b].push(target);
                edges += 1;
                if edges > MAX_CFG_EDGES {
                    return Err(limit(
                        "block projection exceeds the canonical CFG edge bound",
                    ));
                }
            }
        }
        for (k, block) in view.kept.iter().enumerate() {
            checkpoint()?;
            for name in &block.targets {
                checkpoint()?;
                let Some(&target) = names.get(name) else {
                    return Ok(None);
                };
                successors[view.blocks.len() + k].push(target);
                edges += 1;
                if edges > MAX_CFG_EDGES {
                    return Err(limit(
                        "block projection exceeds the canonical CFG edge bound",
                    ));
                }
            }
        }
        let sites = pieces::project(view, &names, &mut successors, checkpoint)?;
        let mut edge_count = 0;
        for targets in &successors {
            checkpoint()?;
            edge_count += targets.len();
            if edge_count > MAX_CFG_EDGES {
                return Err(limit(
                    "symbolic continuation projection exceeds the canonical CFG edge bound",
                ));
            }
        }
        let reachable = walk(&successors, entry, None, checkpoint)?;
        Ok(Some(Self {
            names,
            sites,
            successors,
            entry,
            reachable,
            without: BTreeMap::new(),
        }))
    }

    pub(super) fn dominates(
        &mut self,
        owner: &str,
        using: &str,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<bool>> {
        let (Some(&owner), Some(&using)) = (self.names.get(owner), self.names.get(using)) else {
            return Ok(None);
        };
        self.judgment(owner, using, checkpoint)
    }

    pub(super) fn value_dominates(
        &mut self,
        owner: &str,
        definition: &str,
        using: &str,
        at: &str,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<bool>> {
        let owner = self.sites.get(definition).or_else(|| self.names.get(owner));
        let using = self.sites.get(at).or_else(|| self.names.get(using));
        let (Some(&owner), Some(&using)) = (owner, using) else {
            return Ok(None);
        };
        self.judgment(owner, using, checkpoint)
    }

    pub(super) fn point(&self, block: &str, at: &str) -> Option<usize> {
        self.sites
            .get(at)
            .or_else(|| self.names.get(block))
            .copied()
    }

    pub(super) fn node_dominates(
        &mut self,
        owner: usize,
        using: usize,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<bool> {
        Ok(self.judgment(owner, using, checkpoint)? == Some(true))
    }

    pub(super) fn path_nodes(
        &self,
        from: usize,
        to: usize,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Vec<bool>> {
        let mut predecessors = vec![Vec::new(); self.successors.len()];
        for (node, targets) in self.successors.iter().enumerate() {
            checkpoint()?;
            for &target in targets {
                checkpoint()?;
                predecessors[target].push(node);
            }
        }
        let forward = walk(&self.successors, from, None, checkpoint)?;
        let backward = walk(&predecessors, to, None, checkpoint)?;
        let mut paths = Vec::with_capacity(forward.len());
        for (forward, backward) in forward.into_iter().zip(backward) {
            checkpoint()?;
            paths.push(forward && backward);
        }
        Ok(paths)
    }

    fn judgment(
        &mut self,
        owner: usize,
        using: usize,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<bool>> {
        checkpoint()?;
        if !self.reachable[owner] || !self.reachable[using] {
            return Ok(Some(false));
        }
        if !self.without.contains_key(&owner) {
            let reachable = walk(&self.successors, self.entry, Some(owner), checkpoint)?;
            self.without.insert(owner, reachable);
        }
        checkpoint()?;
        Ok(Some(!self.without[&owner][using]))
    }
}

fn authored_targets(
    view: &FnExp<'_, '_>,
    block: &ABlock,
    dialect: bool,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Option<Vec<String>>> {
    let mut out = Vec::new();
    for stmt in &block.stmts {
        checkpoint()?;
        let complete = match stmt {
            Stmt::Op(node) => node_targets(view, node, &mut out, dialect, checkpoint)?,
            Stmt::Exit(exit) if dialect => {
                arg_targets(view, &exit.cond, &mut out, dialect, checkpoint)?
                    && match &exit.payload {
                        Some(arg) => arg_targets(view, arg, &mut out, dialect, checkpoint)?,
                        None => true,
                    }
                    && route(view, &exit.target, &mut out)
            }
            Stmt::Exit(_) | Stmt::Raw { .. } => false,
        };
        if !complete {
            return Ok(None);
        }
    }
    for arg in term_args(&block.term) {
        if !arg_targets(view, arg, &mut out, dialect, checkpoint)? {
            return Ok(None);
        }
    }
    match &block.term {
        Term::Br { target, .. } => out.push(target.block.clone()),
        Term::Cond { then, other, .. } => {
            out.push(then.block.clone());
            out.push(other.block.clone());
        }
        Term::Switch { cases, .. } => {
            out.extend(cases.iter().map(|(_, t)| t.block.clone()));
        }
        Term::Raw(_) => return Ok(None),
        Term::Return(_)
        | Term::Trap { .. }
        | Term::LenientTrap { .. }
        | Term::Ok(_)
        | Term::Fail { .. } => {}
    }
    Ok(Some(out))
}

fn route(view: &FnExp<'_, '_>, name: &str, out: &mut Vec<String>) -> bool {
    if name.is_empty() {
        return true;
    }
    // An incomplete named error may hide a case/block ambiguity.
    if let Some(TypeExpr::Result { error, .. }) = &view.result {
        if let TypeExpr::Named(named) = &**error
            && view.cx.trait_definition(&named.definition).is_none()
        {
            return false;
        }
    } else if view.result.is_none() {
        return false;
    }
    match (view.is_block(name), view.error_case(name).is_some()) {
        (true, false) => {
            out.push(name.into());
            true
        }
        (false, true) => true,
        (true, true) | (false, false) => false,
    }
}

fn node_targets(
    view: &FnExp<'_, '_>,
    node: &Node,
    out: &mut Vec<String>,
    dialect: bool,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<bool> {
    checkpoint()?;
    if let Some(name) = &node.check
        && (!dialect || !route(view, name, out))
    {
        return Ok(false);
    }
    for arg in &node.args {
        if !arg_targets(view, arg, out, dialect, checkpoint)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn arg_targets(
    view: &FnExp<'_, '_>,
    arg: &Arg,
    out: &mut Vec<String>,
    dialect: bool,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<bool> {
    checkpoint()?;
    match &arg.operand {
        Operand::Nested(node) => {
            if dialect {
                node_targets(view, node, out, dialect, checkpoint)
            } else {
                Ok(false)
            }
        }
        Operand::Raw(_) => Ok(false),
        Operand::Name(_) | Operand::Literal { .. } => Ok(true),
    }
}

fn walk(
    successors: &[Vec<usize>],
    entry: usize,
    skip: Option<usize>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<bool>> {
    checkpoint()?;
    let mut seen = vec![false; successors.len()];
    if skip == Some(entry) {
        return Ok(seen);
    }
    let mut pending = VecDeque::from([entry]);
    seen[entry] = true;
    while let Some(node) = pending.pop_front() {
        checkpoint()?;
        for &target in &successors[node] {
            checkpoint()?;
            if skip != Some(target) && !seen[target] {
                seen[target] = true;
                pending.push_back(target);
            }
        }
    }
    checkpoint()?;
    Ok(seen)
}

fn limit(detail: &str) -> AgentError {
    AgentError::new(AgentErrorCode::ResidualLimit, detail)
}
