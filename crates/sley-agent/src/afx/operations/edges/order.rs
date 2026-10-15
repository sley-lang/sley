//! Authored local order and parameter ownership, independent of inferred types.

use super::{FnExp, Inventory};
use crate::afx::{Arg, Kind, Node, Operand, Stmt, Term, split_suffix};
use crate::error::Result;
use serde_json::json;
use std::collections::BTreeMap;

mod far;
mod indexes;
mod kept;

type Availability = (&'static str, Option<String>, &'static str);

#[derive(Clone)]
struct Definition {
    at: String,
    position: Option<usize>,
    kind: Kind,
    ty: Option<sley_ssmc::TypeExpr>,
    results: Option<usize>,
    retained: Option<sley_id::EntityId>,
}

struct Walk<'v, 'r, 'c, 'a, F> {
    view: &'v FnExp<'c, 'a>,
    definitions: Vec<BTreeMap<String, Definition>>,
    parameters: BTreeMap<String, String>,
    kept: BTreeMap<String, BTreeMap<String, Definition>>,
    block: usize,
    position: usize,
    dialect: bool,
    report: &'r mut Inventory,
    checkpoint: &'r mut F,
}

pub(super) fn check(
    view: &FnExp<'_, '_>,
    report: &mut Inventory,
    dialect: bool,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let mut definitions = Vec::new();
    for (b, block) in view.blocks.iter().enumerate() {
        checkpoint()?;
        let mut values = BTreeMap::new();
        for (index, (name, _, _)) in block.params.iter().enumerate() {
            checkpoint()?;
            values.insert(
                name.clone(),
                Definition {
                    at: format!("{}/params/{index}", block.pointer),
                    position: None,
                    kind: Kind::Param,
                    ty: view.defs[b][name].ty.clone(),
                    results: None,
                    retained: None,
                },
            );
        }
        for (position, stmt) in block.stmts.iter().enumerate() {
            checkpoint()?;
            let (name, at) = match stmt {
                Stmt::Op(node) => (node.name.as_ref(), &node.pointer),
                Stmt::Raw { name, pointer, .. } => (name.as_ref(), pointer),
                Stmt::Exit(_) => continue,
            };
            if let Some(name) = name {
                values.insert(
                    name.clone(),
                    Definition {
                        at: at.clone(),
                        position: Some(position),
                        kind: view.defs[b][name].kind,
                        ty: view.defs[b][name].ty.clone(),
                        results: (view.defs[b][name].kind == Kind::Op).then_some(1),
                        retained: None,
                    },
                );
            }
        }
        definitions.push(values);
    }
    let mut parameters = BTreeMap::new();
    for (index, (name, _)) in view.params.iter().enumerate() {
        checkpoint()?;
        let at = if view.patch {
            format!("current function parameter `{}.{name}`", view.fn_name)
        } else {
            format!("{}/params/{index}", view.fn_pointer)
        };
        parameters.insert(name.clone(), at);
    }
    let kept = kept::declarations(view, checkpoint)?;
    let mut walk = Walk {
        view,
        definitions,
        parameters,
        kept,
        block: 0,
        position: 0,
        dialect,
        report,
        checkpoint,
    };
    for (b, block) in view.blocks.iter().enumerate() {
        walk.block = b;
        for (position, stmt) in block.stmts.iter().enumerate() {
            (walk.checkpoint)()?;
            walk.position = position;
            match stmt {
                Stmt::Op(node) => walk.node(node)?,
                Stmt::Exit(exit) => {
                    walk.arg(&exit.cond, false)?;
                    if let Some(arg) = &exit.payload {
                        walk.arg(arg, false)?;
                    }
                }
                Stmt::Raw { pointer, .. } => walk.report.deferred.push(pointer.clone()),
            }
        }
        walk.position = block.stmts.len();
        walk.term(&block.term, &block.pointer)?;
    }
    (walk.checkpoint)()
}

impl<F: FnMut() -> Result<()>> Walk<'_, '_, '_, '_, F> {
    fn node(&mut self, node: &Node) -> Result<()> {
        (self.checkpoint)()?;
        // The parser separates opcode immediates, type annotations and literals.
        for arg in &node.args {
            self.arg(arg, false)?;
        }
        Ok(())
    }

    fn arg(&mut self, arg: &Arg, case_payload: bool) -> Result<()> {
        (self.checkpoint)()?;
        match &arg.operand {
            Operand::Name(name) if name == "$" && case_payload => {}
            Operand::Name(name) => self.name(name, &arg.pointer)?,
            Operand::Nested(node) => self.node(node)?,
            Operand::Literal { .. } => {}
            Operand::Raw(_) => self.report.deferred.push(arg.pointer.clone()),
        }
        Ok(())
    }

    fn term(&mut self, term: &Term, at: &str) -> Result<()> {
        (self.checkpoint)()?;
        match term {
            Term::Return(arg) | Term::Ok(arg) => self.arg(arg, false)?,
            Term::Br { target, .. } => {
                for arg in &target.args {
                    self.arg(arg, false)?;
                }
            }
            Term::Cond { cond, then, other } => {
                self.arg(cond, false)?;
                for arg in then.args.iter().chain(&other.args) {
                    self.arg(arg, false)?;
                }
            }
            Term::Switch { value, cases } => {
                self.arg(value, false)?;
                for (_, target) in cases {
                    for arg in &target.args {
                        self.arg(arg, true)?;
                    }
                }
            }
            Term::Trap { payload, .. } | Term::Fail { payload, .. } => {
                if let Some(arg) = payload {
                    self.arg(arg, false)?;
                }
            }
            Term::Raw(_) | Term::LenientTrap { .. } => {
                self.report.deferred.push(format!("{at}/term"));
            }
        }
        Ok(())
    }

    fn local(
        &mut self,
        text: &str,
        at: &str,
        def: &Definition,
    ) -> (&'static str, Option<String>, &'static str) {
        if def
            .position
            .is_some_and(|position| position >= self.position)
        {
            self.report.conflicts.push((at.into(),format!("`{text}` is used before its definition at {} in block `{}`: a block's values are defined in order",def.at,self.view.blocks[self.block].name)));
            ("current_block", Some(def.at.clone()), "conflict")
        } else {
            ("current_block", Some(def.at.clone()), "local_order_checked")
        }
    }

    fn foreign(
        &mut self,
        text: &str,
        at: &str,
        owner: &str,
        def: &Definition,
    ) -> Result<(&'static str, Option<String>, &'static str)> {
        if def.kind == Kind::Param || (self.dialect && def.kind == Kind::Unwrapped) {
            self.report.conflicts.push((at.into(),format!("`{text}` names a block parameter or checked continuation at {}, visible only in its own block; pass it as an edge argument",def.at)));
            return Ok(("other_block", Some(def.at.clone()), "conflict"));
        }
        let judgment = if let Some(flow) = self.report.flow.as_mut() {
            flow.value_dominates(
                owner,
                &def.at,
                &self.view.blocks[self.block].name,
                at,
                self.checkpoint,
            )?
        } else {
            None
        };
        match judgment {
            Some(false) => {
                self.report.conflicts.push((at.into(),format!("definition of `{text}` at {} does not dominate this use in block `{}` under the current entry (the definition or use may be unreachable)",def.at,self.view.blocks[self.block].name)));
                Ok(("other_block", Some(def.at.clone()), "conflict"))
            }
            Some(true) => Ok(("other_block", Some(def.at.clone()), "dominance_checked")),
            None => {
                self.report.deferred.push(at.into());
                Ok(("other_block", Some(def.at.clone()), "availability_deferred"))
            }
        }
    }

    fn name(&mut self, text: &str, at: &str) -> Result<()> {
        let (base, suffix) = split_suffix(text);
        let mut resolved = None;
        let (scope, definition, availability) = if let Some((owner, leaf)) = base.split_once('.') {
            if let Some(b) = self.view.block_index(owner) {
                if let Some(def) = self.definitions[b].get(leaf).cloned() {
                    resolved = Some(def.clone());
                    if b == self.block {
                        self.local(text, at, &def)
                    } else {
                        self.foreign(text, at, owner, &def)?
                    }
                } else {
                    self.unknown(at)
                }
            } else if let Some(def) = self
                .kept
                .get(owner)
                .and_then(|values| values.get(leaf))
                .cloned()
            {
                resolved = Some(def.clone());
                self.foreign(text, at, owner, &def)?
            } else {
                self.unknown(at)
            }
        } else if let Some(def) = self.definitions[self.block].get(base).cloned() {
            resolved = Some(def.clone());
            self.local(text, at, &def)
        } else if let Some(def) = self.parameters.get(base) {
            resolved = Some(Definition {
                at: def.clone(),
                position: None,
                kind: Kind::Param,
                ty: self
                    .view
                    .params
                    .iter()
                    .find(|(name, _)| name == base)
                    .and_then(|(_, ty)| ty.clone()),
                results: None,
                retained: None,
            });
            ("function", Some(def.clone()), "function_parameter_checked")
        } else {
            let (chosen, availability) = far::resolve(self, base, text, at)?;
            resolved = chosen;
            availability
        };
        let result_index_judgment = indexes::check(self.report, text, at, resolved.as_ref());
        let ty = (availability != "conflict"
            && matches!(
                result_index_judgment,
                "operation_result_index_checked"
                    | "parameter_reference_checked"
                    | "parameter_suffix_ignored_by_ordinary_compiler"
            ))
        .then(|| resolved.as_ref().and_then(|def| self.read_type(text, def)))
        .flatten();
        if let Some(ty) = &ty {
            self.report.read_types.insert(at.into(), ty.clone());
        }
        self.report.reads.push(json!({"at":at,"name":text,"scope":scope,"definition":definition,"availability":availability,"result_index_suffix":suffix,"result_index_judgment":result_index_judgment,"actual_type":ty.as_ref().map(|ty|self.view.cx.render(ty)),"rewritten":false}));
        Ok(())
    }

    fn read_type(&self, text: &str, def: &Definition) -> Option<sley_ssmc::TypeExpr> {
        if def.kind != Kind::Op {
            return def.ty.clone();
        }
        let index = text
            .split_once('#')
            .map_or(Some(0), |(_, suffix)| suffix.parse::<u32>().ok())?;
        if let Some(id) = def.retained {
            return match self.view.cx.program.body(&id) {
                Some(sley_mutate::value::EntityBodyValue::Operation(op)) => {
                    op.result_types.get(usize::try_from(index).ok()?).cloned()
                }
                _ => None,
            };
        }
        (index == 0).then(|| def.ty.clone()).flatten()
    }

    fn unknown(&mut self, at: &str) -> (&'static str, Option<String>, &'static str) {
        self.report.deferred.push(at.into());
        ("unresolved_or_far", None, "availability_deferred")
    }
}
