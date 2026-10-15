//! Conditional exits preserve their authored condition, payload and route.

use super::{Check, Checked, Result, Value, conflict};
use crate::opcodes;
use serde_json::json;
use sley_ssmc::TypeExpr;

pub(super) enum ErrorCase {
    Known(Option<TypeExpr>),
    Unresolved,
}

impl Check<'_, '_> {
    pub(super) fn conditional_exit(mut self, row: &[Value], at: &str) -> Result<Checked> {
        if !(3..=4).contains(&row.len()) || row[1] != "if" {
            return Err(conflict(
                at,
                "conditional exit requires [!target, if, condition, optional payload]",
            ));
        }
        let target = row[0]
            .as_str()
            .expect("checked exit word")
            .strip_prefix('!')
            .expect("checked exit prefix");
        if !target.is_empty() && !crate::names::is_identifier(target) {
            return Err(conflict(
                &format!("{at}/0"),
                "conditional exit requires an explicit case or handler name",
            ));
        }
        let condition_at = format!("{at}/2");
        let condition = self.expression(
            &row[2],
            &condition_at,
            Some(&(TypeExpr::Bool, format!("{condition_at} (exit condition)"))),
            0,
        )?;
        let payload = row.get(3);
        let payload_at = format!("{at}/3");
        if payload
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str)
            .is_some_and(|word| {
                opcodes::by_word(word.split_once('?').map_or(word, |(word, _)| word)).is_some()
            })
        {
            return Err(conflict(
                &payload_at,
                "conditional exit payload requires a name or literal, not a nested operation",
            ));
        }
        let route = if target.is_empty() {
            if !matches!(self.scope.output, TypeExpr::Option(_)) || payload.is_some() {
                return Err(conflict(
                    at,
                    "bare conditional exit returns None without a payload and requires Option at /bindings/returns",
                ));
            }
            "option_none_checked"
        } else if let ErrorCase::Known(expected) =
            self.error_case_payload(target, &format!("{at}/0"))?
        {
            if expected.is_some() != payload.is_some() {
                return Err(conflict(
                    &payload_at,
                    &format!(
                        "payload presence conflicts with case `{target}` of /bindings/returns"
                    ),
                ));
            }
            if let (Some(expected), Some(value)) = (expected, payload) {
                self.expression(
                    value,
                    &payload_at,
                    Some(&(expected, format!("case `{target}` of /bindings/returns"))),
                    0,
                )?;
            }
            "named_case_checked"
        } else {
            self.deferred.insert(format!("{at}/0"));
            if let Some(value) = payload {
                self.expression(value, &payload_at, None, 0)?;
            }
            "error_definition_deferred"
        };
        let mut checked = self.finish(None, at);
        checked.report["conditional_exit"] = json!({"target":target,
            "condition":if condition.is_some(){"bool_checked"}else{"deferred"},
            "failure_route":route,"return_type":"/bindings/returns",
            "payload":if payload.is_some(){Some(payload_at)}else{None}});
        Ok(checked)
    }

    /// A missing case is a conflict; only an unavailable definition is deferred.
    /// Fragment terminal regions expose no authored handler blocks.
    pub(super) fn error_case_payload(&mut self, target: &str, at: &str) -> Result<ErrorCase> {
        self.budget.checkpoint()?;
        let missing = || {
            conflict(
                at,
                &format!(
                    "failure route `{target}` is not a case of /bindings/returns; this fragment interface exposes no handler blocks"
                ),
            )
        };
        if !crate::names::is_identifier(target) {
            return Err(conflict(
                at,
                "failure route requires an explicit error case name",
            ));
        }
        let TypeExpr::Result { error, .. } = self.scope.output else {
            return Err(missing());
        };
        let TypeExpr::Named(named) = &**error else {
            return Err(missing());
        };
        // Context::members alone is a lossy view of incomplete draft syntax.
        // Do not infer absence or a unit payload from an incomplete declaration.
        if self.cx.trait_definition(&named.definition).is_none() {
            return Ok(ErrorCase::Unresolved);
        }
        let Some((variant, members)) = self.cx.members(&named.definition) else {
            return Ok(ErrorCase::Unresolved);
        };
        if !variant {
            return Err(missing());
        }
        members
            .into_iter()
            .find(|(name, _)| name == target)
            .map(|(_, payload)| {
                ErrorCase::Known(payload.map(|ty| crate::values::substitute(&ty, &named.arguments)))
            })
            .ok_or_else(missing)
    }
}
