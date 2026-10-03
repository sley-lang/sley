//! Failure-route connections of checked expressions in the enclosing function.

use super::exits::ErrorCase;
use super::{Check, Known, Result, conflict};
use serde_json::json;
use sley_ssmc::{BuiltinFailureKind, TypeExpr};

impl Check<'_, '_> {
    pub(super) fn propagate(
        &mut self,
        raw: Option<Known>,
        route: &str,
        tag: u32,
        at: &str,
    ) -> Result<Option<Known>> {
        // Resolve the named destination even when the operation's result type
        // is unknown: missing source typing cannot create a handler block.
        let named = if route.is_empty() {
            None
        } else {
            Some(self.error_case_payload(route, at)?)
        };
        let (payload, failure, option) = match raw {
            Some((TypeExpr::Result { ok, error }, _)) => (Some(*ok), Some(*error), false),
            Some((TypeExpr::Option(item), _)) => (Some(*item), None, true),
            Some((ty, source)) => {
                return Err(conflict(
                    at,
                    &format!(
                        "checked propagation requires Result or Option, but {source} has {}",
                        self.cx.render(&ty)
                    ),
                ));
            }
            None if (64..=71).contains(&tag) => (
                None,
                Some(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
                false,
            ),
            None if tag == 35 => (
                None,
                Some(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
                false,
            ),
            None if tag == 36 => (
                None,
                Some(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
                false,
            ),
            None if matches!(tag, 21 | 34 | 37 | 128 | 129) => (None, None, true),
            None if matches!(tag, 18 | 20) => {
                return Err(conflict(
                    at,
                    "named value construction cannot use checked propagation",
                ));
            }
            None if tag == 194 => {
                return Err(conflict(
                    at,
                    "function references cannot use checked propagation",
                ));
            }
            None if matches!(tag, 39 | 40) => {
                return Err(conflict(
                    at,
                    "map insert/remove cannot use checked propagation; they produce a Map",
                ));
            }
            None if tag == 176 => {
                return Err(conflict(
                    at,
                    "cell creation cannot use checked propagation; it produces a Cell",
                ));
            }
            None if tag == 32 => {
                return Err(conflict(
                    at,
                    "vector construction cannot use checked propagation; it produces a Vec",
                ));
            }
            None if (80..=85).contains(&tag) => {
                return Err(conflict(
                    at,
                    "floating-point arithmetic cannot use checked propagation; it produces f32 or f64",
                ));
            }
            None => {
                self.deferred.insert(at.into());
                self.propagation.insert(json!({"at":at,"route":route,"failure_route":"deferred","reason":"operation_result_unresolved"}));
                return Ok(None);
            }
        };
        if let (Some(actual), Some(inferred)) = (
            payload.as_ref(),
            self.scope
                .body_types
                .and_then(|types| types.payloads.get(at)),
        ) && actual != inferred
        {
            return Err(conflict(
                at,
                &format!(
                    "authored operands produce {} but ordinary AF1-X continuation typing requires {}",
                    self.cx.render(actual),
                    self.cx.render(inferred)
                ),
            ));
        }
        let status = self.propagation_route(route, named, failure.as_ref(), option, at)?;
        self.propagation.insert(json!({"at":at,"route":route,"failure_route":status,
            "source_failure":failure.as_ref().map(|ty|self.cx.render(ty)),
            "output":"/bindings/returns", "unwrapped_type":payload.as_ref().map(|ty|self.cx.render(ty)),
            "control_flow":"ordinary_compiler"}));
        Ok(payload.map(|ty| (ty, format!("{at} (unwrapped result)"))))
    }

    fn propagation_route(
        &mut self,
        route: &str,
        named: Option<ErrorCase>,
        failure: Option<&TypeExpr>,
        option: bool,
        at: &str,
    ) -> Result<&'static str> {
        self.budget.checkpoint()?;
        if route.is_empty() {
            let compatible = match (self.scope.output, option) {
                (TypeExpr::Option(_), true) => true,
                (TypeExpr::Result { error, .. }, false) => failure == Some(&**error),
                _ => false,
            };
            if !compatible {
                return Err(conflict(
                    at,
                    &format!(
                        "bare checked propagation passes {} unchanged, conflicting with /bindings/returns ({})",
                        failure.map_or_else(|| "None".into(), |ty| self.cx.render(ty)),
                        self.cx.render(self.scope.output)
                    ),
                ));
            }
            return Ok("preserve_failure");
        }
        if let Some(ErrorCase::Known(expected)) = named {
            if expected.is_none() {
                return Ok("drop_into_unit_case");
            }
            if !option && expected.as_ref() == failure {
                return Ok("preserve_in_named_case");
            }
            return Err(conflict(
                at,
                &format!(
                    "checked failure payload conflicts with case `{route}` of /bindings/returns"
                ),
            ));
        }
        self.deferred.insert(at.into());
        Ok("error_definition_deferred")
    }
}
