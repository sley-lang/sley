//! Read ordinary operation-use hints from AF1-X, without lowering a block.
//! Within each continuation piece, terminal uses precede calls; pieces remain
//! in evaluation order. Only direct operation results receive ordinary hints.
use super::{Arg, Context, FnExp, Node, Operand, Parser, Result, Stmt, Target, Term, Value};
use crate::{frame::constants, types};
use serde_json::json;
use sley_ssmc::TypeExpr;
use std::collections::BTreeMap;

pub(crate) type Hints = BTreeMap<String, TypeExpr>;

pub(crate) struct End<'a> {
    pub value: &'a Value,
    pub at: String,
    pub hint: Option<TypeExpr>,
    pub context: Option<TypeExpr>,
}

pub(crate) fn region(
    cx: &Context<'_>,
    ops: &Value,
    at: &str,
    ends: &[End<'_>],
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Hints> {
    checkpoint()?;
    let mut obligations = Vec::new();
    let mut parser = Parser {
        obligations: &mut obligations,
    };
    let block = parser.block("region", &json!({"ops":ops,"term":["trap"]}), at, cx);
    let mut walk = Walk {
        cx,
        names: BTreeMap::new(),
        resolved: None,
        explicit: Hints::new(),
        pieces: vec![Piece::default()],
        checkpoint,
    };
    if let Some(block) = block {
        for stmt in &block.stmts {
            match stmt {
                Stmt::Op(node) => {
                    walk.node(node)?;
                }
                Stmt::Exit(exit) => {
                    walk.arg(&exit.cond)?;
                    if let Some(payload) = &exit.payload {
                        walk.arg(payload)?;
                    }
                    walk.pieces.push(Piece::default());
                }
                Stmt::Raw { .. } => {}
            }
        }
    }
    let mut terminal = Vec::new();
    for end in ends {
        let arg = parser.arg(end.value, end.at.clone());
        let key = walk.arg(&arg)?;
        if let (Some(key), Some(ty)) = (key, &end.hint) {
            terminal.push((key, ty.clone()));
        }
    }
    // AF1-X lowers every edge argument before emitting the final terminator.
    walk.pieces.last_mut().expect("initial piece").terminal = terminal;
    let mut hints = Hints::new();
    for piece in walk.pieces {
        for (at, ty) in piece.terminal.into_iter().chain(piece.calls) {
            constants::hint(&mut hints, at, &ty);
        }
    }
    // Explicit operation annotations override inferred use hints in build_op.
    hints.extend(walk.explicit);
    Ok(hints)
}

#[derive(Default)]
struct Piece {
    terminal: Vec<(String, TypeExpr)>,
    calls: Vec<(String, TypeExpr)>,
}

struct Walk<'a, 'p, F> {
    cx: &'a Context<'p>,
    names: BTreeMap<String, String>,
    resolved: Option<&'a BTreeMap<String, String>>,
    explicit: Hints,
    pieces: Vec<Piece>,
    checkpoint: &'a mut F,
}

impl<F: FnMut() -> Result<()>> Walk<'_, '_, F> {
    fn arg(&mut self, arg: &Arg) -> Result<Option<String>> {
        (self.checkpoint)()?;
        match &arg.operand {
            Operand::Nested(node) => self.node(node),
            Operand::Name(name) => {
                if let Some(resolved) = self.resolved {
                    return Ok(resolved.get(&arg.pointer).cloned());
                }
                let (name, index) = name
                    .split_once('#')
                    .map_or((name.as_str(), 0), |(name, index)| {
                        (name, index.parse().unwrap_or(u32::MAX))
                    });
                Ok((index == 0)
                    .then(|| self.names.get(name).cloned())
                    .flatten())
            }
            Operand::Literal { .. } | Operand::Raw(_) => Ok(None),
        }
    }

    fn node(&mut self, node: &Node) -> Result<Option<String>> {
        (self.checkpoint)()?;
        let mut args = Vec::new();
        for arg in &node.args {
            args.push(self.arg(arg)?);
        }
        if node.row.tag == 112
            && let Some(name) = node.immediate.as_ref().and_then(Value::as_str)
            && let Some((params, _)) = self.cx.signature(name)
        {
            for (key, ty) in args.into_iter().zip(params) {
                if let (Some(key), Some(ty)) = (key, ty) {
                    self.pieces
                        .last_mut()
                        .expect("initial piece")
                        .calls
                        .push((key, ty));
                }
            }
        }
        let key = node.check.is_none().then(|| node.pointer.clone());
        if let Some(key) = &key {
            if let Some(ty) = &node.ty {
                self.explicit.insert(
                    key.clone(),
                    types::read(ty, self.cx, &format!("{}/type", node.pointer))?,
                );
            }
            if node.check.is_none()
                && let Some(name) = &node.name
            {
                self.names.insert(name.clone(), key.clone());
            }
        }
        if node.check.is_some() {
            self.pieces.push(Piece::default());
        }
        // Checked results are continuation parameters, not raw operation results.
        Ok(if node.check.is_some() { None } else { key })
    }
}

/// Read first-use hints over parsed source blocks and resolved operation reads.
/// Parameters and checked continuation reads are absent from `resolved`.
pub(super) fn function(
    view: &FnExp<'_, '_>,
    resolved: &BTreeMap<String, String>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Hints> {
    let mut walk = Walk {
        cx: view.cx,
        names: BTreeMap::new(),
        resolved: Some(resolved),
        explicit: Hints::new(),
        pieces: vec![Piece::default()],
        checkpoint,
    };
    for block in &view.blocks {
        for stmt in &block.stmts {
            (walk.checkpoint)()?;
            match stmt {
                Stmt::Op(node) => {
                    walk.node(node)?;
                }
                Stmt::Exit(exit) => {
                    walk.arg(&exit.cond)?;
                    if let Some(arg) = &exit.payload {
                        walk.arg(arg)?;
                    }
                    walk.pieces.push(Piece::default());
                }
                Stmt::Raw { .. } => {}
            }
        }
        let mut terminal = Vec::new();
        match &block.term {
            Term::Return(arg) => {
                if let (Some(key), Some(ty)) = (walk.arg(arg)?, &view.result) {
                    terminal.push((key, ty.clone()));
                }
            }
            Term::Br { target, .. } => edge(&mut walk, view, target, &mut terminal)?,
            Term::Cond { cond, then, other } => {
                walk.arg(cond)?;
                edge(&mut walk, view, then, &mut terminal)?;
                edge(&mut walk, view, other, &mut terminal)?;
            }
            Term::Switch { value, cases } => {
                walk.arg(value)?;
                for (_, target) in cases {
                    edge(&mut walk, view, target, &mut terminal)?;
                }
            }
            Term::Ok(arg) => {
                walk.arg(arg)?;
            }
            Term::Fail { payload, .. } | Term::Trap { payload, .. } => {
                if let Some(arg) = payload {
                    walk.arg(arg)?;
                }
            }
            Term::LenientTrap { .. } | Term::Raw(_) => {}
        }
        walk.pieces.last_mut().expect("initial piece").terminal = terminal;
        walk.pieces.push(Piece::default());
    }
    let mut hints = Hints::new();
    for piece in walk.pieces {
        for (at, ty) in piece.terminal.into_iter().chain(piece.calls) {
            constants::hint(&mut hints, at, &ty);
        }
    }
    hints.extend(walk.explicit);
    Ok(hints)
}

fn edge<F: FnMut() -> Result<()>>(
    walk: &mut Walk<'_, '_, F>,
    view: &FnExp<'_, '_>,
    target: &Target,
    terminal: &mut Vec<(String, TypeExpr)>,
) -> Result<()> {
    let params = view.target_params(&target.block);
    for (index, arg) in target.args.iter().enumerate() {
        let key = walk.arg(arg)?;
        let ty = params
            .as_ref()
            .and_then(|params| params.get(index))
            .and_then(|(_, ty)| ty.as_ref());
        if let (Some(key), Some(ty)) = (key, ty) {
            terminal.push((key, ty.clone()));
        }
    }
    Ok(())
}
