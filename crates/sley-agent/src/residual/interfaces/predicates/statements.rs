//! Read named AF1-X operations without emitting or renaming them.

use super::{Check, Checked, Context, Evidence, Result, Scope, Value, conflict, types};
use std::collections::BTreeSet;

pub(in super::super) fn statement(
    cx: &Context<'_>,
    scope: Scope<'_>,
    statement: &Value,
    at: &str,
    budget: &mut crate::residual::frontier::Budget,
) -> Result<Checked> {
    let mut check = Check {
        cx,
        scope,
        budget,
        deferred: BTreeSet::new(),
        untyped_literals: BTreeSet::new(),
        calls: Evidence::default(),
        propagation: Evidence::default(),
        maps: Evidence::default(),
        comparisons: Evidence::default(),
        named_values: Evidence::default(),
        references: Evidence::default(),
        cells: Evidence::default(),
        hashes: Evidence::default(),
        cell_operands: Evidence::default(),
        closed_types: Evidence::default(),
        value_bindings: Evidence::default(),
        operand_operation: None,
    };
    check.budget.checkpoint()?;
    if let Some(row) = statement.as_array()
        && row
            .first()
            .and_then(Value::as_str)
            .is_some_and(|word| word.starts_with('!'))
    {
        return check.conditional_exit(row, at);
    }
    let (items, pointers, annotation) = match statement {
        Value::Array(row)
            if row.len() >= 2 && !row[0].as_str().is_some_and(|name| name.starts_with('!')) =>
        {
            (
                row[1..].to_vec(),
                (1..row.len())
                    .map(|index| format!("{at}/{index}"))
                    .collect::<Vec<_>>(),
                None,
            )
        }
        Value::Object(object) if object.get("op").or_else(|| object.get("opcode")).is_some() => {
            let word = if object.contains_key("op") {
                "op"
            } else {
                "opcode"
            };
            let key = if object.contains_key("args") {
                "args"
            } else {
                "operands"
            };
            let args = match object.get(key) {
                None => &[][..],
                Some(Value::Array(args)) => args.as_slice(),
                Some(_) => {
                    return Err(conflict(
                        &format!("{at}/{key}"),
                        "expected operation operands",
                    ));
                }
            };
            let mut items = vec![object[word].clone()];
            items.extend_from_slice(args);
            let mut pointers = vec![format!("{at}/{word}")];
            pointers.extend((0..args.len()).map(|index| format!("{at}/{key}/{index}")));
            let annotation = object
                .get("type")
                .map(|value| {
                    let pointer = format!("{at}/type");
                    types::read(value, cx, &pointer).map(|ty| (ty, pointer))
                })
                .transpose()?;
            (items, pointers, annotation)
        }
        _ => {
            check.deferred.insert(at.into());
            return Ok(check.finish(None, at));
        }
    };
    let actual = check.operation(&items, at, None, annotation.as_ref(), 0, &pointers)?;
    if let Some(annotation) = &annotation {
        check.closed_type(annotation, &annotation.1)?;
    }
    // Checked operations annotate their wrapped result, not the continuation
    // payload. Do not use that annotation as the local result's type.
    let checked = items[0].as_str().is_some_and(|word| word.contains('?'));
    let mut result = check.finish(
        actual.or_else(|| (!checked).then_some(annotation).flatten()),
        at,
    );
    // AF1-X split() binds the unwrapped value as a continuation parameter,
    // rather than as result zero of the operation that produced the wrapper.
    // Unknown payload types remain unresolved and never create a typed binding.
    result.continuation = checked;
    Ok(result)
}

impl Check<'_, '_> {
    pub(super) fn constructor(
        &mut self,
        tag: u32,
        items: &[Value],
        at: &str,
        expected: Option<&super::Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<super::Known>> {
        use sley_ssmc::TypeExpr;
        let arity = if tag == 129 { 1 } else { 2 };
        if items.len() != arity {
            return Err(conflict(
                at,
                "wrong Option/Result constructor operand count",
            ));
        }
        // Ordinary build_op uses the first direct return/edge/call hint for
        // unannotated constructors. A named result keeps that emitted type
        // when a later use supplies a different context.
        let emitted = self.emitted_hint(at);
        let expected = emitted.as_ref().or(expected);
        let payload = match (tag, expected) {
            (128 | 129, Some((TypeExpr::Option(item), source))) => {
                Some(((**item).clone(), source.clone()))
            }
            (130, Some((TypeExpr::Result { ok, .. }, source))) => {
                Some(((**ok).clone(), source.clone()))
            }
            (131, Some((TypeExpr::Result { error, .. }, source))) => {
                Some(((**error).clone(), source.clone()))
            }
            (_, Some((ty, source))) => {
                return Err(conflict(
                    at,
                    &format!(
                        "constructor conflicts with {source}, which requires {}",
                        self.cx.render(ty)
                    ),
                ));
            }
            (_, None) => None,
        };
        let actual = if tag == 129 {
            None
        } else {
            self.expression(&items[1], &pointers[1], payload.as_ref(), depth + 1)?
        };
        if let Some(expected) = expected {
            self.deferred.remove(at);
            return Ok(Some(expected.clone()));
        }
        if tag == 128
            && let Some((ty, _)) = actual
        {
            self.deferred.remove(at);
            return Ok(Some((TypeExpr::Option(Box::new(ty)), at.into())));
        }
        self.deferred.insert(at.into());
        Ok(None)
    }
}
