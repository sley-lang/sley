//! Nearest dominating declaration, hidden parameters and path rebinding.

use super::{Availability, Definition, Walk};
use crate::afx::Kind;
use crate::error::Result;

pub(super) fn resolve<F: FnMut() -> Result<()>>(
    walk: &mut Walk<'_, '_, '_, '_, F>,
    name: &str,
    text: &str,
    at: &str,
) -> Result<(Option<Definition>, Availability)> {
    (walk.checkpoint)()?;
    let candidates = declarations(walk, name)?;
    let here = &walk.view.blocks[walk.block].name;
    if candidates.is_empty() {
        return Ok((None, walk.unknown(at)));
    }
    if !walk.dialect {
        let locators: Vec<_> = candidates.iter().map(|(_, def)| def.at.as_str()).collect();
        walk.report.conflicts.push((at.into(), format!("plain AF1 cannot read `{text}` from another block without qualification; declarations at {}", locators.join(", "))));
        return Ok((None, ("other_block_far", None, "conflict")));
    }
    let Some(flow) = walk.report.flow.as_mut() else {
        return Ok((None, walk.unknown(at)));
    };
    let Some(using) = flow.point(here, at) else {
        return Ok((None, walk.unknown(at)));
    };
    let mut positions = Vec::new();
    for (owner, def) in &candidates {
        (walk.checkpoint)()?;
        let Some(node) = flow.point(owner, &def.at) else {
            return Ok((None, walk.unknown(at)));
        };
        positions.push(node);
    }
    let nearest = closest(flow, using, &positions, walk.checkpoint)?;
    let Some((index, from)) = nearest else {
        let locators: Vec<_> = candidates.iter().map(|(_, def)| def.at.as_str()).collect();
        walk.report.conflicts.push((at.into(), format!("each known definition of `{text}` does not dominate this use (a definition or use may be unreachable); declarations at {}", locators.join(", "))));
        return Ok((None, ("other_block_far", None, "conflict")));
    };
    let (owner, def) = &candidates[index];
    if def.kind != Kind::Op {
        walk.report.conflicts.push((at.into(), format!("nearest `{text}` is a parameter or checked continuation at {}, visible only in its own block `{owner}`; it hides outer definitions", def.at)));
        return Ok((
            Some(def.clone()),
            ("other_block_far", Some(def.at.clone()), "conflict"),
        ));
    }
    let paths = flow.path_nodes(from, using, walk.checkpoint)?;
    for (other, (other_owner, other_def)) in candidates.iter().enumerate() {
        (walk.checkpoint)()?;
        if other_owner != owner && positions[other] != from && paths[positions[other]] {
            walk.report.conflicts.push((at.into(), format!("plain name `{text}` would select {} but rebinds at {} on a path to this use; qualify the intended operation or pass an explicit parameter", def.at, other_def.at)));
            return Ok((
                Some(def.clone()),
                ("other_block_far", Some(def.at.clone()), "conflict"),
            ));
        }
    }
    if let (Some(found), Some(typed)) = (&def.ty, walk.view.far_type(walk.block, name))
        && *found != typed
    {
        walk.report.conflicts.push((at.into(), format!("plain name `{text}` selects {} of type {} but was typed with {}; qualify the intended result", def.at, walk.view.cx.render(found), walk.view.cx.render(&typed))));
        return Ok((
            Some(def.clone()),
            ("other_block_far", Some(def.at.clone()), "conflict"),
        ));
    }
    Ok((
        Some(def.clone()),
        (
            "other_block_far",
            Some(def.at.clone()),
            "nearest_definition_checked",
        ),
    ))
}

fn declarations<F: FnMut() -> Result<()>>(
    walk: &mut Walk<'_, '_, '_, '_, F>,
    name: &str,
) -> Result<Vec<(String, Definition)>> {
    let mut candidates = Vec::new();
    for (b, values) in walk.definitions.iter().enumerate() {
        (walk.checkpoint)()?;
        if b != walk.block
            && let Some(def) = values.get(name)
        {
            candidates.push((walk.view.blocks[b].name.clone(), def.clone()));
        }
    }
    for (owner, values) in &walk.kept {
        (walk.checkpoint)()?;
        if let Some(def) = values.get(name) {
            candidates.push((owner.clone(), def.clone()));
        }
    }
    Ok(candidates)
}

fn closest(
    flow: &mut crate::afx::operations::edges::flow::Flow,
    using: usize,
    positions: &[usize],
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Option<(usize, usize)>> {
    let mut nearest = None;
    for (index, &node) in positions.iter().enumerate() {
        checkpoint()?;
        if flow.node_dominates(node, using, checkpoint)? {
            let closer = match nearest {
                Some((_, previous)) => flow.node_dominates(previous, node, checkpoint)?,
                None => true,
            };
            if closer {
                nearest = Some((index, node));
            }
        }
    }
    Ok(nearest)
}
