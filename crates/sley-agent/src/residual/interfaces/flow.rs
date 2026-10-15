//! Explicit branch coverage and failure routes on the declared interfaces.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};
use sley_ssmc::{BuiltinFailureKind, TypeExpr};

use super::{Checker, conflict, field};
use crate::error::{AgentError, AgentErrorCode, Result};
use crate::residual::fragments::MAX_FRAGMENT_ITEMS;

type Cases = BTreeMap<String, Option<TypeExpr>>;

impl Checker<'_, '_> {
    fn cases(&mut self, ty: &TypeExpr, at: &str) -> Result<Option<Cases>> {
        self.budget.checkpoint()?;
        let members = match ty {
            TypeExpr::Option(item) => vec![
                ("Some".into(), Some((**item).clone())),
                ("None".into(), None),
            ],
            TypeExpr::Result { ok, error } => vec![
                ("Ok".into(), Some((**ok).clone())),
                ("Err".into(), Some((**error).clone())),
            ],
            TypeExpr::Named(named) => {
                // The member-name view can omit malformed draft rows or turn
                // missing payload syntax into None. It cannot establish an
                // exhaustive branch set or unit-case entitlement by itself.
                if self.cx.trait_definition(&named.definition).is_none() {
                    return Ok(None);
                }
                let (variant, members) = self
                    .cx
                    .members(&named.definition)
                    .ok_or_else(|| conflict(at, "bound variant definition is unavailable"))?;
                if !variant {
                    return Err(conflict(at, "a record has no switch/error cases"));
                }
                members
                    .into_iter()
                    .map(|(name, payload)| {
                        (
                            name,
                            payload.map(|ty| crate::values::substitute(&ty, &named.arguments)),
                        )
                    })
                    .collect()
            }
            _ => return Err(conflict(at, "expected Option, Result or a named variant")),
        };
        let mut cases = BTreeMap::new();
        for (name, payload) in members {
            self.budget.checkpoint()?;
            if cases.insert(name.clone(), payload).is_some() {
                return Err(conflict(
                    at,
                    &format!("bound variant repeats case `{name}`"),
                ));
            }
        }
        Ok(Some(cases))
    }

    pub(super) fn branch_cases(
        &mut self,
        input: &Value,
        cases: &[Value],
        at: &str,
    ) -> Result<Option<(Cases, String)>> {
        let mut selected = BTreeSet::new();
        for (index, case) in cases.iter().enumerate() {
            self.budget.checkpoint()?;
            let tag = case["case"]
                .as_str()
                .filter(|name| crate::names::is_identifier(name))
                .ok_or_else(|| {
                    conflict(&format!("{at}/cases/{index}/case"), "expected a case name")
                })?;
            if !selected.insert(tag.to_owned()) {
                return Err(conflict(
                    &format!("{at}/cases/{index}/case"),
                    "duplicate branch case",
                ));
            }
        }
        let checked = super::predicates::value(
            self.cx,
            super::predicates::Scope {
                inputs: self.inputs,
                unresolved: None,
                output: self.output,
                constant_hints: None,
                body_types: None,
            },
            input,
            &format!("{at}/input"),
            None,
            self.budget,
        )?;
        let Some(ty) = checked.ty else {
            self.connections.push(
                json!({"at":format!("{at}/input"),"branch_coverage":"deferred",
                "reason":"input_expression_requires_typing","expression_interface":checked.report}),
            );
            return Ok(None);
        };
        let source = input
            .as_str()
            .and_then(|name| {
                self.inputs
                    .get(name.split_once('#').map_or(name, |(base, _)| base))
            })
            .map_or_else(|| format!("{at}/input"), |binding| binding.source.clone());
        let Some(expected) = self.cases(&ty, &format!("{at}/input"))? else {
            self.connections.push(json!({"at":format!("{at}/cases"),
                "input":format!("{at}/input"),"source":source,"branch_coverage":"deferred",
                "reason":"variant_definition_incomplete","expression_interface":checked.report}));
            return Ok(None);
        };
        if expected.len() > MAX_FRAGMENT_ITEMS {
            return Err(AgentError::new(
                AgentErrorCode::ResidualLimit,
                format!(
                    "{at}/cases: exhaustive variant exceeds the {MAX_FRAGMENT_ITEMS}-case fragment ceiling"
                ),
            ));
        }
        for (index, case) in cases.iter().enumerate() {
            let tag = case["case"].as_str().expect("checked case name");
            if !expected.contains_key(tag) {
                return Err(conflict(
                    &format!("{at}/cases/{index}/case"),
                    &format!("`{tag}` is not a case of {source} selected by {at}/input"),
                ));
            }
        }
        let missing: Vec<_> = expected
            .keys()
            .filter(|name| !selected.contains(*name))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(conflict(
                &format!("{at}/cases"),
                &format!(
                    "missing cases {} required by {source} selected by {at}/input; supply explicit branches",
                    missing.join(", ")
                ),
            ));
        }
        self.connections.push(
            json!({"at":format!("{at}/cases"),"input":format!("{at}/input"),
            "source":source,"branch_coverage":"checked", "cases":selected,
            "expression_interface":checked.report}),
        );
        Ok(Some((expected, source)))
    }

    fn error_cases(&mut self, at: &str) -> Result<Option<Cases>> {
        let TypeExpr::Result { error, .. } = self.output else {
            return Err(conflict(
                at,
                "named failure requires Result with a variant error at /bindings/returns",
            ));
        };
        if !matches!(**error, TypeExpr::Named(_)) {
            return Err(conflict(
                at,
                "named failure requires a named variant error at /bindings/returns",
            ));
        }
        self.cases(error, at)
    }

    pub(super) fn arithmetic_route(&mut self, route: &str, at: &str) -> Result<()> {
        let Some(cases) = self.error_cases(at)? else {
            self.connections
                .push(json!({"at":at,"failure_route":"deferred",
                "return_type":"/bindings/returns","case":route,
                "reason":"variant_definition_incomplete"}));
            return Ok(());
        };
        let payload = cases.get(route).ok_or_else(|| conflict(at, &format!(
            "`{route}` is not a case of /bindings/returns; this fragment creates no authored handler block"
        )))?;
        let arithmetic = TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic);
        if payload.as_ref().is_some_and(|ty| ty != &arithmetic) {
            return Err(conflict(
                at,
                "arithmetic failure supplies ArithmeticError but the selected case of /bindings/returns requires a different payload",
            ));
        }
        self.connections.push(json!({"at":at,"failure_route":"checked", "return_type":"/bindings/returns",
            "case":route,"payload":if payload.is_some() {"preserve_ArithmeticError"} else {"drop_into_unit_case"}}));
        Ok(())
    }

    pub(super) fn guards(&mut self, bindings: &Map<String, Value>, at: &str) -> Result<()> {
        let guards = field(bindings, "guards")
            .as_array()
            .ok_or_else(|| conflict(at, "expected guards"))?;
        if guards.len() > MAX_FRAGMENT_ITEMS {
            return Err(AgentError::new(
                AgentErrorCode::ResidualLimit,
                format!("{at}/guards: too many guard interfaces"),
            ));
        }
        for (index, guard) in guards.iter().enumerate() {
            self.budget.checkpoint()?;
            self.failure(&guard["fail"], &format!("{at}/guards/{index}/fail"))?;
            let predicate = super::predicates::check(
                self.cx,
                super::predicates::Scope {
                    inputs: self.inputs,
                    unresolved: None,
                    output: self.output,
                    constant_hints: None,
                    body_types: None,
                },
                &guard["when"],
                &format!("{at}/guards/{index}/when"),
                self.budget,
            )?;
            self.connections.push(predicate);
        }
        Ok(())
    }

    fn failure(&mut self, value: &Value, at: &str) -> Result<()> {
        self.failure_in_scope(
            value,
            at,
            super::predicates::Scope {
                inputs: self.inputs,
                unresolved: None,
                output: self.output,
                constant_hints: None,
                body_types: None,
            },
        )
    }

    pub(super) fn failure_in_scope(
        &mut self,
        value: &Value,
        at: &str,
        scope: super::predicates::Scope<'_>,
    ) -> Result<()> {
        let inputs = scope.inputs;
        let term = value
            .as_array()
            .filter(|term| (1..=3).contains(&term.len()) && term[0] == "fail")
            .ok_or_else(|| conflict(at, "expected an explicit fail terminator"))?;
        if term.len() == 1 {
            if !matches!(self.output, TypeExpr::Option(_)) {
                return Err(conflict(
                    at,
                    "bare fail returns None but /bindings/returns is not Option",
                ));
            }
            self.connections.push(json!({"at":at,"failure_route":"checked", "return_type":"/bindings/returns", "case":"None"}));
            return Ok(());
        }
        let route = term[1]
            .as_str()
            .filter(|name| crate::names::is_identifier(name))
            .ok_or_else(|| conflict(at, "expected a failure case name"))?;
        let cases = self.error_cases(at)?;
        let expected = cases
            .as_ref()
            .map(|cases| {
                cases.get(route).ok_or_else(|| {
                    conflict(
                        at,
                        &format!("`{route}` is not a failure case of /bindings/returns"),
                    )
                })
            })
            .transpose()?;
        if expected.is_some_and(|payload| payload.is_some() != (term.len() == 3)) {
            return Err(conflict(
                at,
                "failure payload presence conflicts with the selected case of /bindings/returns",
            ));
        }
        let mut value_check = "not_applicable";
        let mut payload_interface = Value::Null;
        if let Some(value) = term.get(2) {
            let expected = expected.and_then(Option::as_ref).map(|expected| {
                (
                    expected.clone(),
                    format!("failure case `{route}` of /bindings/returns"),
                )
            });
            let checked = super::predicates::value(
                self.cx,
                scope,
                value,
                &format!("{at}/2"),
                expected.as_ref(),
                self.budget,
            )?;
            value_check = if cases.is_none() {
                "destination_deferred"
            } else if checked.report["expression_types"] != "connections_checked" {
                "deferred_expression"
            } else if let Some(binding) = value
                .as_str()
                .and_then(|name| inputs.get(name.split_once('#').map_or(name, |(base, _)| base)))
            {
                match binding.kind {
                    super::BindingKind::Operation => "checked_operation_result",
                    super::BindingKind::Parameter => "checked_parameter",
                    super::BindingKind::Continuation => "checked_continuation_parameter",
                }
            } else {
                "checked_expression"
            };
            payload_interface = checked.report;
        }
        self.connections.push(
            json!({"at":at,"failure_route":if cases.is_some(){"checked"}else{"deferred"},
            "return_type":"/bindings/returns","reason":if cases.is_some(){Value::Null}else{json!("variant_definition_incomplete")},
            "case":route,"payload_value":value_check,"payload_interface":payload_interface}),
        );
        Ok(())
    }
}
