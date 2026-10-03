//! Tuple shape and element connections without evaluating or rewriting values.

use super::{Check, Known, Result, Value, conflict};
use sley_ssmc::TypeExpr;

impl Check<'_, '_> {
    pub(super) fn tuple(
        &mut self,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        let context = match expected {
            Some((TypeExpr::Tuple(types), source)) => {
                if types.len() != items.len() - 1 {
                    return Err(conflict(
                        at,
                        &format!(
                            "tuple has {} elements but {source} requires {}",
                            items.len() - 1,
                            types.len()
                        ),
                    ));
                }
                Some((types, source))
            }
            Some((ty, source)) => {
                return Err(conflict(
                    at,
                    &format!(
                        "tuple conflicts with {source}, which requires {}",
                        self.cx.render(ty)
                    ),
                ));
            }
            None => None,
        };
        let mut types = Vec::with_capacity(items.len() - 1);
        for (index, value) in items.iter().enumerate().skip(1) {
            let element = context.map(|(types, source)| {
                (
                    types[index - 1].clone(),
                    format!("{source} (tuple element {})", index - 1),
                )
            });
            let actual = self.expression(value, &pointers[index], element.as_ref(), depth + 1)?;
            types.push(actual.map(|(ty, _)| ty));
        }
        let known = expected.cloned().or_else(|| {
            types
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .map(|types| (TypeExpr::Tuple(types), at.into()))
        });
        if known.is_some() {
            self.deferred.remove(at);
        } else {
            self.deferred.insert(at.into());
        }
        Ok(known)
    }

    pub(super) fn tuple_get(
        &mut self,
        items: &[Value],
        at: &str,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        if items.len() != 3 {
            return Err(conflict(
                at,
                "tuple_get requires an index and one tuple operand",
            ));
        }
        let index = items[1]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| {
                conflict(
                    &pointers[1],
                    "tuple_get index must be an unsigned 32-bit integer",
                )
            })?;
        let Some((ty, source)) = self.expression(&items[2], &pointers[2], None, depth + 1)? else {
            self.deferred.insert(at.into());
            return Ok(None);
        };
        let TypeExpr::Tuple(types) = ty else {
            return Err(conflict(
                &pointers[2],
                &format!(
                    "tuple_get requires a tuple, but {source} has {}",
                    self.cx.render(&ty)
                ),
            ));
        };
        let ty = types.get(index as usize).ok_or_else(|| {
            conflict(
                &pointers[1],
                &format!(
                    "tuple_get index {index} is outside {source}, which has {} elements",
                    types.len()
                ),
            )
        })?;
        self.deferred.remove(at);
        Ok(Some((
            ty.clone(),
            format!("{source} (tuple element {index})"),
        )))
    }
}
