//! Canonical shape recognition of old shared exits; generated reuse is deferred.

use crate::afx::{Context, LiveGraph};
use crate::error::Result;
use sley_id::EntityId;
use sley_mutate::value::EntityBodyValue;
use std::collections::BTreeSet;

pub(super) fn current(
    cx: &Context<'_>,
    blocks: &[EntityId],
    retained: &[EntityId],
    dialect: bool,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<(Vec<EntityId>, Vec<String>)> {
    checkpoint()?;
    if !dialect {
        return Ok((retained.to_vec(), Vec::new()));
    }
    let live = LiveGraph::new(cx, blocks);
    checkpoint()?;
    let mut generated = BTreeSet::new();
    for (leaf, id, body) in &live.blocks {
        checkpoint()?;
        if live.is_generated_exit(leaf, body) {
            generated.insert(*id);
        }
        checkpoint()?;
    }
    let mut referenced = BTreeSet::new();
    for id in retained {
        checkpoint()?;
        if generated.contains(id) {
            continue;
        }
        if let Some(EntityBodyValue::Block(body)) = cx.program.body(id) {
            for target in super::super::flow::targets(&body.terminator) {
                checkpoint()?;
                referenced.insert(target);
            }
        }
    }
    let mut current = Vec::new();
    let mut deferred = Vec::new();
    for id in retained {
        checkpoint()?;
        if generated.contains(id) && !referenced.contains(id) {
            // Ordinary expansion drops an unreferenced old shared exit unless
            // its generated output restates the name. Do not check the old body
            // as an unchanged consumer; future allocation/reuse stays deferred.
            deferred.push(format!(
                "generated shared exit `{}` current body/identity deferred",
                cx.names.name(id)
            ));
        } else {
            current.push(*id);
        }
    }
    checkpoint()?;
    Ok((current, deferred))
}
