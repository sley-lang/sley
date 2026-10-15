//! Pre-expansion expression connections for predicates and authored regions. Other expressions
//! remain explicitly deferred; this pass never substitutes an expression.

pub(super) mod admission;
mod arithmetic;
mod bindings;
mod cells;
mod closed;
mod comparisons;
mod exits;
mod floats;
mod hashing;
mod literals;
mod maps;
mod named;
mod propagation;
mod references;
mod statements;
mod traps;
mod tuples;
pub(super) mod type_traits;
mod vectors;
pub(super) use statements::statement;
pub(super) use traps::trap;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sley_ssmc::TypeExpr;

use super::{Bindings, conflict, signatures};
use crate::afx::Context;
use crate::error::{AgentError, AgentErrorCode, Result};
use crate::residual::frontier::Budget;
use crate::{opcodes, types};

type Known = (TypeExpr, String);

/// Inference can revisit an expression when its sibling supplies a type. Keep
/// the final evidence once per authored location, in first-visit order.
#[derive(Default)]
struct Evidence {
    entries: Vec<Value>,
    locations: BTreeMap<String, usize>,
}

impl Evidence {
    fn insert(&mut self, value: Value) {
        let at = value["at"].as_str().expect("internal evidence locator");
        if let Some(index) = self.locations.get(at) {
            self.entries[*index] = value;
        } else {
            self.locations.insert(at.to_owned(), self.entries.len());
            self.entries.push(value);
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Scope<'a> {
    pub inputs: &'a Bindings,
    pub unresolved: Option<&'a super::regions::UnresolvedBindings>,
    pub output: &'a TypeExpr,
    pub constant_hints: Option<&'a crate::afx::constant_uses::Hints>,
    pub body_types: Option<&'a crate::afx::operations::continuations::Inventory>,
}

struct Check<'a, 'p> {
    cx: &'a Context<'p>,
    scope: Scope<'a>,
    budget: &'a mut Budget,
    deferred: BTreeSet<String>,
    untyped_literals: BTreeSet<String>,
    calls: Evidence,
    propagation: Evidence,
    maps: Evidence,
    comparisons: Evidence,
    named_values: Evidence,
    references: Evidence,
    cells: Evidence,
    hashes: Evidence,
    cell_operands: Evidence,
    closed_types: Evidence,
    value_bindings: Evidence,
    operand_operation: Option<(u32, String)>,
}

pub(super) struct Checked {
    pub ty: Option<TypeExpr>,
    pub report: Value,
    pub continuation: bool,
}

pub(super) fn check(
    cx: &Context<'_>,
    scope: Scope<'_>,
    value: &Value,
    at: &str,
    budget: &mut Budget,
) -> Result<Value> {
    let mut checked = self::value(
        cx,
        scope,
        value,
        at,
        Some(&(TypeExpr::Bool, format!("{at} (guard condition)"))),
        budget,
    )?;
    checked.report["guard_result"] = json!(if checked.ty.is_some() {
        "bool_checked"
    } else {
        "deferred"
    });
    Ok(checked.report)
}

pub(super) fn value(
    cx: &Context<'_>,
    scope: Scope<'_>,
    expression: &Value,
    at: &str,
    expected: Option<&Known>,
    budget: &mut Budget,
) -> Result<Checked> {
    let local_hints = if scope.constant_hints.is_none() {
        Some(crate::afx::constant_uses::region(
            cx,
            &json!([]),
            at,
            &[crate::afx::constant_uses::End {
                value: expression,
                at: at.into(),
                hint: None,
                context: expected.map(|known| known.0.clone()),
            }],
            &mut || budget.checkpoint(),
        )?)
    } else {
        None
    };
    let local_body = if scope.body_types.is_none() {
        Some(crate::afx::operations::continuations::region(
            cx,
            scope
                .inputs
                .iter()
                .map(|(name, binding)| {
                    (
                        name.clone(),
                        Some(binding.ty.clone()),
                        Value::from(cx.render(&binding.ty)),
                    )
                })
                .collect(),
            scope.output,
            &json!([]),
            at,
            &[crate::afx::constant_uses::End {
                value: expression,
                at: at.into(),
                hint: None,
                context: expected.map(|known| known.0.clone()),
            }],
            &mut || budget.checkpoint(),
        )?)
    } else {
        None
    };
    let scope = Scope {
        constant_hints: scope.constant_hints.or(local_hints.as_ref()),
        body_types: scope.body_types.or(local_body.as_ref()),
        ..scope
    };
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
    let actual = check.expression(expression, at, expected, 0)?;
    Ok(check.finish(actual, at))
}

impl Check<'_, '_> {
    fn emitted_hint(&self, at: &str) -> Option<Known> {
        self.scope
            .constant_hints
            .and_then(|hints| hints.get(at))
            .map(|ty| (ty.clone(), format!("{at} (ordinary emitted use hint)")))
    }

    fn finish(self, actual: Option<Known>, at: &str) -> Checked {
        Checked {
            ty: actual.map(|known| known.0),
            continuation: false,
            report: json!({"at":at,
            "expression_types":if self.deferred.is_empty() {"connections_checked"} else {"partial"},
            "deferred_expressions":self.deferred, "untyped_literal_contexts":self.untyped_literals, "rewritten":false, "calls":self.calls.entries, "propagation":self.propagation.entries, "maps":self.maps.entries,
            "comparisons":self.comparisons.entries,
            "named_values":self.named_values.entries,
            "references":self.references.entries,
            "cells":self.cells.entries,
            "hashes":self.hashes.entries,
            "cell_operands":self.cell_operands.entries,
            "closed_types":self.closed_types.entries,
            "value_bindings":self.value_bindings.entries,
            "value_scope":"local_bindings_checked; unknown_expressions_deferred",
            "effects":"callee_interfaces_checked; declared_bodies_and_other_expressions_deferred",
            "control_flow":"ordinary_compiler"}),
        }
    }

    fn compatible(&self, actual: &Known, expected: Option<&Known>, at: &str) -> Result<()> {
        if let Some(expected) = expected
            && actual.0 != expected.0
        {
            return Err(conflict(
                at,
                &format!(
                    "{} has {} but {} requires {}",
                    actual.1,
                    self.cx.render(&actual.0),
                    expected.1,
                    self.cx.render(&expected.0)
                ),
            ));
        }
        Ok(())
    }

    fn expression(
        &mut self,
        value: &Value,
        at: &str,
        expected: Option<&Known>,
        depth: usize,
    ) -> Result<Option<Known>> {
        self.budget.checkpoint()?;
        if depth > crate::afx::MAX_EXPR_DEPTH {
            return Err(AgentError::new(
                AgentErrorCode::ResidualLimit,
                format!("{at}: predicate expression nesting exceeds its bound"),
            ));
        }
        if self
            .scope
            .body_types
            .is_some_and(|types| types.untyped_literals.contains(at))
        {
            self.untyped_literals.insert(at.into());
        }
        let ordinary_literal = self
            .scope
            .body_types
            .and_then(|types| types.literals.get(at))
            .map(|ty| (ty.clone(), format!("{at} (ordinary AF1-X literal context)")));
        let actual = match value {
            Value::String(name) => self.binding(name, at)?,
            Value::Bool(_) => self.literal(value, at, Some(&(TypeExpr::Bool, at.into())))?,
            Value::Number(_) => self.literal(value, at, ordinary_literal.as_ref().or(expected))?,
            Value::Object(object)
                if object.contains_key("value")
                    && object.keys().all(|key| key == "value" || key == "type") =>
            {
                let explicit = object
                    .get("type")
                    .map(|ty| {
                        types::read(ty, self.cx, &format!("{at}/type"))
                            .map(|ty| (ty, format!("{at}/type")))
                    })
                    .transpose()?;
                let intrinsic = object["value"]
                    .is_boolean()
                    .then(|| (TypeExpr::Bool, format!("{at}/value")));
                self.literal(
                    &object["value"],
                    &format!("{at}/value"),
                    explicit
                        .as_ref()
                        .or(intrinsic.as_ref())
                        .or(ordinary_literal.as_ref())
                        .or(expected),
                )?
            }
            Value::Array(items) => self.operation(
                items,
                at,
                expected,
                None,
                depth,
                &(0..items.len())
                    .map(|index| format!("{at}/{index}"))
                    .collect::<Vec<_>>(),
            )?,
            _ => {
                self.deferred.insert(at.into());
                None
            }
        };
        if let Some(actual) = &actual {
            self.compatible(actual, expected, at)?;
        }
        self.cell_operand(actual.as_ref(), at)?;
        Ok(actual)
    }

    fn operation(
        &mut self,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        annotation: Option<&Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        let word = items.first().and_then(Value::as_str).unwrap_or("");
        let (word, route) = word
            .split_once('?')
            .map_or((word, None), |(word, route)| (word, Some(route)));
        let Some(row) = opcodes::by_word(word) else {
            self.deferred.insert(at.into());
            return Ok(None);
        };
        admission::check(row, at)?;
        if row.tag == 112
            && let Some(route) = route.filter(|route| !route.is_empty())
        {
            // The named destination is independently invalid even when the
            // bound callee declaration is incomplete.
            self.error_case_payload(route, at)?;
        }
        let context = annotation.cloned().or_else(|| {
            if route.is_none() {
                return expected.cloned();
            }
            expected.and_then(|(ty, source)| match row.tag {
                64..=71 => Some((arithmetic::wrapped(ty.clone()), source.clone())),
                21 | 34 | 37 | 128 => {
                    Some((TypeExpr::Option(Box::new(ty.clone())), source.clone()))
                }
                35 => Some((vectors::wrapped(ty.clone()), source.clone())),
                36 => Some((maps::wrapped(ty.clone()), source.clone())),
                _ => None,
            })
        });
        let parent = self.operand_operation.replace((row.tag, at.into()));
        let raw = self.raw_operation(row, items, at, context.as_ref(), depth, pointers);
        self.operand_operation = parent;
        let raw = raw?;
        if let Some(raw) = &raw {
            self.compatible(raw, context.as_ref(), at)?;
        }
        if let Some(route) = route {
            self.propagate(raw, route, row.tag, at)
        } else {
            Ok(raw)
        }
    }

    fn raw_operation(
        &mut self,
        row: &opcodes::OpcodeRow,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        match row.tag {
            1 | 193 | 194 => return self.reference(row.tag, items, at, expected, pointers),
            16 => return self.tuple(items, at, expected, depth, pointers),
            17 => return self.tuple_get(items, at, depth, pointers),
            18..=21 => return self.named_operation(row.tag, items, at, expected, depth, pointers),
            32 => return self.vector_new(items, at, expected, depth, pointers),
            33..=35 => return self.vector_access(row.tag, items, at, expected, depth, pointers),
            36 => return self.map_new(items, at, expected, depth, pointers),
            37..=40 => return self.map_access(row.tag, items, at, expected, depth, pointers),
            64..=71 => return self.arithmetic(row.tag, items, at, expected, depth, pointers),
            80..=85 => return self.float_arithmetic(row.tag, items, at, expected, depth, pointers),
            112 => return self.call(items, at, depth, pointers),
            128..=131 => return self.constructor(row.tag, items, at, expected, depth, pointers),
            176..=178 => return self.cell_operation(row.tag, items, at, expected, depth, pointers),
            192 => return self.value_hash(items, at, depth, pointers),
            96..=104 => {}
            _ => {
                self.deferred.insert(at.into());
                return Ok(None);
            }
        }
        let arity = if row.tag == 102 { 1 } else { 2 };
        if items.len() != arity + 1 {
            return Err(conflict(at, "wrong predicate operand count"));
        }
        let boolean = (
            TypeExpr::Bool,
            format!("{} ({})", pointers[0], row.mnemonic),
        );
        if row.tag >= 102 {
            for (index, operand) in items.iter().enumerate().skip(1) {
                self.expression(operand, &pointers[index], Some(&boolean), depth + 1)?;
            }
        } else {
            let left_at = &pointers[1];
            let right_at = &pointers[2];
            let mut left = self.expression(&items[1], left_at, None, depth + 1)?;
            let right = self.expression(&items[2], right_at, left.as_ref(), depth + 1)?;
            if left.is_none() && right.is_some() {
                left = self.expression(&items[1], left_at, right.as_ref(), depth + 1)?;
            }
            self.comparison(row.tag, left.as_ref(), right.as_ref(), at)?;
        }
        Ok(Some(boolean))
    }

    fn call(
        &mut self,
        items: &[Value],
        at: &str,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        let callee = items
            .get(1)
            .and_then(Value::as_str)
            .ok_or_else(|| conflict(at, "call needs an explicit callee name"))?;
        let (params, result) = signatures::read(self.cx, callee, at)?.ok_or_else(|| {
            conflict(
                &pointers[1],
                &format!("no callable interface for `{callee}`"),
            )
        })?;
        if items.len() - 2 != params.len() {
            return Err(conflict(
                at,
                &format!(
                    "call to `{callee}` supplies {} arguments but its interface requires {}",
                    items.len() - 2,
                    params.len()
                ),
            ));
        }
        let declared = self.cx.declared_signature(callee);
        let effects = self.cx.reference_function_effects(callee).ok_or_else(|| {
            conflict(
                at,
                &format!("no bound effect interface for callee `{callee}`"),
            )
        })?;
        if !effects.is_empty() {
            return Err(conflict(
                at,
                &format!(
                    "callee `{callee}` declares effects incompatible with the generated function's empty effect set"
                ),
            ));
        }
        for (index, (argument, expected)) in items[2..].iter().zip(&params).enumerate() {
            let expected = (
                expected.clone(),
                format!("callee `{callee}` parameter {index}"),
            );
            self.expression(argument, &pointers[index + 2], Some(&expected), depth + 1)?;
            self.closed_type(&expected, &pointers[index + 2])?;
        }
        self.closed_type(&(result.clone(), format!("callee `{callee}` result")), at)?;
        self.calls
            .insert(json!({"at":at,"callee":callee,"arguments":params.len(),
            "signature_source":if declared {"declared_interface"} else {"accepted_graph"},
            "effects":if declared {"empty_declared_effect_set"} else {"empty_accepted_effect_set"},
            "effect_body_validation":if declared {"ordinary_compiler_and_kernel"} else {"accepted_graph"},
            "return_type":self.cx.render(&result)}));
        Ok(Some((result, format!("callee `{callee}` result"))))
    }
}
