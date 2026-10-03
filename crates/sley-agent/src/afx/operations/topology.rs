//! Read authored successor names with the existing parser; generate no blocks.

use super::super::{Context, Parser, Shape, Term};
use crate::error::Result;
use serde_json::Value;

#[derive(Default)]
pub(crate) struct Topology {
    pub targets: Vec<(String, String)>,
    pub deferred: Vec<String>,
}

pub(crate) fn block(
    cx: &Context<'_>,
    body: &Value,
    name: &str,
    at: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Topology> {
    checkpoint()?;
    let mut obligations = Vec::new();
    let mut parser = Parser {
        obligations: &mut obligations,
    };
    let parsed = parser.block(name, body, at, cx);
    checkpoint()?;
    let mut result = Topology::default();
    let Some(parsed) = parsed else {
        result.deferred.push(at.into());
        return Ok(result);
    };
    let mut target = |value: &super::super::Target, flat_index: usize| {
        let pointer = target_pointer(value, flat_index);
        result.targets.push((value.block.clone(), pointer));
    };
    match &parsed.term {
        Term::Br { target: edge, .. } => target(edge, 1),
        Term::Cond { then, other, .. } => {
            target(then, 0);
            target(other, 0);
        }
        Term::Switch { cases, .. } => {
            for (_, edge) in cases {
                checkpoint()?;
                target(edge, 1);
            }
        }
        Term::Raw(_) => result.deferred.push(format!("{at}/term")),
        Term::Return(_)
        | Term::Ok(_)
        | Term::Fail { .. }
        | Term::Trap { .. }
        | Term::LenientTrap { .. } => {}
    }
    // These are explicit terminator targets only. Generated checked/exit edges,
    // argument types, dominance and the body parser's other obligations remain
    // separate judgments, never inferred from this inventory.
    Ok(result)
}

pub(super) fn target_pointer(value: &super::super::Target, flat_index: usize) -> String {
    match value.shape {
        Shape::Name => value.pointer.clone(),
        Shape::Bracket => format!("{}/0", value.pointer),
        Shape::Flat => format!("{}/{flat_index}", value.pointer),
    }
}

pub(crate) fn removed_pieces(
    cx: &Context<'_>,
    blocks: &[super::super::EntityId],
    keys: impl Iterator<Item = String>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<std::collections::BTreeSet<String>> {
    checkpoint()?;
    let live = super::super::LiveGraph::new(cx, blocks);
    checkpoint()?;
    let mut removed = std::collections::BTreeSet::new();
    for key in keys {
        checkpoint()?;
        removed.extend(live.pieces_of(&key));
        checkpoint()?;
    }
    Ok(removed)
}
