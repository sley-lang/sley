//! Unchanged accepted declarations retain their result cardinalities.

use super::{Definition, FnExp};
use crate::afx::Kind;
use crate::error::Result;
use sley_mutate::value::EntityBodyValue;
use std::collections::BTreeMap;

type Blocks = BTreeMap<String, BTreeMap<String, Definition>>;

pub(super) fn declarations(
    view: &FnExp<'_, '_>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Blocks> {
    let mut blocks = BTreeMap::new();
    for block in &view.kept {
        checkpoint()?;
        let mut values = BTreeMap::new();
        for (name, ty) in &block.params {
            checkpoint()?;
            values.insert(
                name.clone(),
                Definition {
                    at: format!("accepted value `{}.{}.{name}`", view.fn_name, block.leaf),
                    position: None,
                    kind: Kind::Param,
                    ty: ty.clone(),
                    results: None,
                    retained: None,
                },
            );
        }
        for (name, ty) in &block.ops {
            checkpoint()?;
            let full = format!("{}.{}.{name}", view.fn_name, block.leaf);
            let retained = view.cx.names.resolve(&full);
            let results = retained.and_then(|id| match view.cx.program.body(&id) {
                Some(EntityBodyValue::Operation(op)) => Some(op.result_types.len()),
                _ => None,
            });
            values.insert(
                name.clone(),
                Definition {
                    at: format!("accepted value `{full}`"),
                    position: None,
                    kind: Kind::Op,
                    ty: ty.clone(),
                    results,
                    retained,
                },
            );
        }
        blocks.insert(block.leaf.clone(), values);
    }
    checkpoint()?;
    Ok(blocks)
}
