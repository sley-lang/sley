//! Scoped authored operation results, branch edges and return connections.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sley_ssmc::TypeExpr;

use super::{Binding, BindingKind, Bindings, Checker, conflict, predicates};
use crate::afx::constant_uses::{self, End, Hints};
use crate::error::{AgentError, AgentErrorCode, Result};
use crate::residual::fragments::MAX_CONSTRUCTION_ITEMS;

pub(super) struct Region {
    pub inputs: Bindings,
    pub unresolved: UnresolvedBindings,
    pub constant_hints: Hints,
    pub body_types: crate::afx::operations::continuations::Inventory,
}

pub(super) struct UnresolvedBinding {
    pub source: String,
    pub kind: Option<BindingKind>,
}

pub(super) type UnresolvedBindings = BTreeMap<String, UnresolvedBinding>;

fn binding_kind(value: &Value) -> Option<BindingKind> {
    let word = value
        .as_array()
        .and_then(|items| items.get(1))
        .or_else(|| value.get("op").or_else(|| value.get("opcode")))
        .and_then(Value::as_str)?;
    let (word, checked) = word
        .split_once('?')
        .map_or((word, false), |(word, _)| (word, true));
    crate::opcodes::by_word(word)?;
    Some(if checked {
        BindingKind::Continuation
    } else {
        BindingKind::Operation
    })
}

fn result_name(value: &Value) -> Option<&str> {
    value
        .as_array()
        .and_then(|items| items.first())
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)
        .filter(|name| crate::names::is_identifier(name))
}

impl Checker<'_, '_> {
    pub(super) fn term_region(&mut self, locals: &Bindings, body: &Value, at: &str) -> Result<()> {
        let region = self.region_ops(
            locals,
            &body["ops"],
            &format!("{at}/ops"),
            &term_ends(&body["term"], &format!("{at}/term"), self.output, self.cx),
        )?;
        self.region_term(&region, &body["term"], &format!("{at}/term"))
    }

    pub(super) fn region_ops(
        &mut self,
        locals: &Bindings,
        ops: &Value,
        at: &str,
        ends: &[End<'_>],
    ) -> Result<Region> {
        let ops = ops
            .as_array()
            .ok_or_else(|| conflict(at, "expected authored operation list"))?;
        if ops.len() > MAX_CONSTRUCTION_ITEMS {
            return Err(AgentError::new(
                AgentErrorCode::ResidualLimit,
                format!("{at}: region operation bound exceeded"),
            ));
        }
        let mut region = Region {
            inputs: self.inputs.clone(),
            unresolved: BTreeMap::new(),
            body_types: crate::afx::operations::continuations::Inventory::default(),
            constant_hints: constant_uses::region(
                self.cx,
                &Value::Array(ops.clone()),
                at.strip_suffix("/ops").expect("region operation locator"),
                ends,
                &mut || self.budget.checkpoint(),
            )?,
        };
        region.inputs.extend(locals.clone());
        let mut definitions: BTreeMap<_, _> = locals
            .iter()
            .map(|(name, binding)| (name.clone(), binding.source.clone()))
            .collect();
        // All authored names mask outer bindings for the entire region, including
        // uses before a definition. Unknown results must never inherit an outer type.
        for (index, op) in ops.iter().enumerate() {
            self.budget.checkpoint()?;
            if let Some(name) = result_name(op) {
                let source = format!("{at}/{index}");
                if let Some(previous) = definitions.insert(name.into(), source.clone()) {
                    return Err(conflict(
                        &source,
                        &format!("binding `{name}` duplicates {previous}"),
                    ));
                }
                region.inputs.remove(name);
                // Result cardinality and continuation ownership follow from
                // recognized operation syntax even when its value type does
                // not yet resolve. Unknown syntax remains deferred.
                region.unresolved.insert(
                    name.into(),
                    UnresolvedBinding {
                        source,
                        kind: binding_kind(op),
                    },
                );
            }
        }
        self.connections.push(super::order::check(
            ops,
            self.inputs,
            locals,
            &definitions,
            at,
            self.budget,
        )?);
        region.body_types =
            self.operation_types(&region.inputs, &Value::Array(ops.clone()), at, ends)?;
        for (index, op) in ops.iter().enumerate() {
            let source = format!("{at}/{index}");
            let checked = predicates::statement(
                self.cx,
                predicates::Scope {
                    inputs: &region.inputs,
                    unresolved: Some(&region.unresolved),
                    output: self.output,
                    constant_hints: Some(&region.constant_hints),
                    body_types: Some(&region.body_types),
                },
                op,
                &source,
                self.budget,
            )?;
            if let (Some(name), Some(ty)) = (result_name(op), checked.ty) {
                region.inputs.insert(
                    name.into(),
                    Binding::operation(ty, source, checked.continuation),
                );
                region.unresolved.remove(name);
            }
            self.connections.push(checked.report);
        }
        Ok(region)
    }

    fn operation_types(
        &mut self,
        inputs: &Bindings,
        ops: &Value,
        at: &str,
        ends: &[End<'_>],
    ) -> Result<crate::afx::operations::continuations::Inventory> {
        crate::afx::operations::continuations::region(
            self.cx,
            inputs
                .iter()
                .map(|(name, binding)| {
                    (
                        name.clone(),
                        Some(binding.ty.clone()),
                        Value::from(self.cx.render(&binding.ty)),
                    )
                })
                .collect(),
            self.output,
            ops,
            at.strip_suffix("/ops").expect("region operation locator"),
            ends,
            &mut || self.budget.checkpoint(),
        )
    }

    pub(super) fn region_value(
        &mut self,
        region: &Region,
        value: &Value,
        at: &str,
        expected: &(TypeExpr, String),
    ) -> Result<()> {
        let checked = predicates::value(
            self.cx,
            predicates::Scope {
                inputs: &region.inputs,
                unresolved: Some(&region.unresolved),
                output: self.output,
                constant_hints: Some(&region.constant_hints),
                body_types: Some(&region.body_types),
            },
            value,
            at,
            Some(expected),
            self.budget,
        )?;
        self.connections.push(checked.report);
        Ok(())
    }

    pub(super) fn region_term(&mut self, region: &Region, term: &Value, at: &str) -> Result<()> {
        self.budget.checkpoint()?;
        let items = term
            .as_array()
            .ok_or_else(|| conflict(at, "expected explicit terminator"))?;
        let word = items.first().and_then(Value::as_str).unwrap_or("");
        let expected = match word {
            "return" => Some((self.output.clone(), "/bindings/returns".into())),
            "ok" => {
                let TypeExpr::Result { ok, .. } = self.output else {
                    return Err(conflict(
                        at,
                        "ok terminator requires Result at /bindings/returns",
                    ));
                };
                Some((
                    (**ok).clone(),
                    "successful payload of /bindings/returns".into(),
                ))
            }
            "fail" => {
                return self.failure_in_scope(
                    term,
                    at,
                    predicates::Scope {
                        inputs: &region.inputs,
                        unresolved: Some(&region.unresolved),
                        output: self.output,
                        constant_hints: Some(&region.constant_hints),
                        body_types: Some(&region.body_types),
                    },
                );
            }
            "trap" => {
                self.connections.push(predicates::trap(
                    self.cx,
                    predicates::Scope {
                        inputs: &region.inputs,
                        unresolved: Some(&region.unresolved),
                        output: self.output,
                        constant_hints: Some(&region.constant_hints),
                        body_types: Some(&region.body_types),
                    },
                    items,
                    at,
                    self.budget,
                )?);
                return Ok(());
            }
            "br" | "jump" | "cond" | "switch" => {
                return Err(conflict(
                    at,
                    "terminal region has no authored block targets; compose an explicit fragment or use ordinary AF1-X for named control flow",
                ));
            }
            _ => {
                return Err(conflict(
                    at,
                    "expected return, ok, fail, or trap terminator",
                ));
            }
        };
        if let Some(expected) = expected {
            if items.len() != 2 {
                return Err(conflict(at, "return/ok needs exactly one value"));
            }
            self.region_value(region, &items[1], &format!("{at}/1"), &expected)?;
        }
        self.connections
            .push(json!({"at":at,"terminal_interface":"checked",
            "control_flow":"terminal_without_successors","kind":word}));
        Ok(())
    }
}

pub(super) fn term_ends<'a>(
    term: &'a Value,
    at: &str,
    output: &TypeExpr,
    cx: &crate::afx::Context<'_>,
) -> Vec<End<'a>> {
    let Some(items) = term.as_array() else {
        return Vec::new();
    };
    let word = items.first().and_then(Value::as_str).unwrap_or("");
    let index = match word {
        "return" | "ok" => 1,
        "fail" | "trap" => 2,
        _ => return Vec::new(),
    };
    items
        .get(index)
        .map(|value| {
            vec![End {
                value,
                at: format!("{at}/{index}"),
                hint: (word == "return").then(|| output.clone()),
                context: match (word, output) {
                    ("return", _) => Some(output.clone()),
                    ("ok", TypeExpr::Result { ok, .. }) => Some((**ok).clone()),
                    ("fail", _) => items.get(1).and_then(Value::as_str).and_then(|route| {
                        crate::afx::operations::continuations::failure_payload(cx, output, route)
                    }),
                    _ => None,
                },
            }]
        })
        .unwrap_or_default()
}

pub(super) fn edge_ends<'a>(
    values: &'a [Value],
    join: &Bindings,
    names: &[Value],
    at: &str,
) -> Vec<End<'a>> {
    values
        .iter()
        .zip(names)
        .enumerate()
        .map(|(index, (value, declared))| {
            let ty = join[declared[0].as_str().expect("parameter name")]
                .ty
                .clone();
            End {
                value,
                at: format!("{at}/values/{index}"),
                hint: Some(ty.clone()),
                context: Some(ty),
            }
        })
        .collect()
}
