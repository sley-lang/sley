//! Function exits use known declaration types and canonical trap traits.

use super::{Context, FnExp, Inventory, Term, operand_type};
use crate::error::{AgentError, AgentErrorCode, Result};
use serde_json::json;
use sley_check::TypeErrorCode;
use sley_ssmc::TypeExpr;

impl Inventory {
    pub(super) fn return_value(
        &mut self,
        cx: &Context<'_>,
        actual: Option<&TypeExpr>,
        expected: Option<&TypeExpr>,
        at: &str,
    ) {
        if let (Some(actual), Some(expected)) = (actual, expected) {
            if actual != expected {
                self.conflicts.push((
                    at.into(),
                    format!(
                        "return value has type {}, but the function result requires {}",
                        cx.render(actual),
                        cx.render(expected)
                    ),
                ));
            }
        } else {
            self.deferred.push(at.into());
        }
        self.inputs.push(json!({"at":at,"kind":"return","actual_type":actual.map(|ty|cx.render(ty)),"required_type":expected.map(|ty|cx.render(ty)),"type_connection":if actual.is_some()&&expected.is_some(){"checked"}else{"deferred"}}));
    }

    pub(super) fn trap(
        &mut self,
        cx: &Context<'_>,
        ty: Option<&TypeExpr>,
        at: &str,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        checkpoint()?;
        let eligibility = if let Some(ty) = ty {
            match cx.trait_check_with_checkpoint(ty, checkpoint, |types| {
                types.check_closed_type(ty)?;
                types.traits(ty)
            })? {
                Some(Ok(traits)) if traits.persistable => "persistable_checked",
                Some(Ok(_)) => {
                    self.conflicts.push((
                        at.into(),
                        "trap payload must be closed and persistable".into(),
                    ));
                    "conflict"
                }
                None => {
                    self.deferred.push(at.into());
                    "definition_deferred"
                }
                Some(Err(error)) => {
                    if matches!(
                        error.code(),
                        TypeErrorCode::DepthLimit | TypeErrorCode::ResourceLimit
                    ) {
                        return Err(AgentError::new(
                            AgentErrorCode::ResidualLimit,
                            format!("{at}: trap payload: {error}"),
                        ));
                    }
                    self.conflicts.push((
                        at.into(),
                        format!("trap payload must be closed and persistable: {error}"),
                    ));
                    "conflict"
                }
            }
        } else {
            self.deferred.push(at.into());
            "unresolved"
        };
        self.inputs.push(json!({"at":at,"kind":"trap_payload","actual_type":ty.map(|ty|cx.render(ty)),"payload_eligibility":eligibility}));
        checkpoint()
    }

    pub(super) fn sugar(
        &mut self,
        view: &FnExp<'_, '_>,
        term: &Term,
        at: &str,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        checkpoint()?;
        let Some(result) = view.result.as_ref() else {
            self.deferred.push(at.into());
            return Ok(());
        };
        match term {
            Term::Ok(arg) => {
                let TypeExpr::Result { ok, .. } = result else {
                    self.conflicts.push((
                        at.into(),
                        format!(
                            "ok terminator requires Result, but the function result is {}",
                            view.cx.render(result)
                        ),
                    ));
                    return Ok(());
                };
                self.return_value(
                    view.cx,
                    operand_type(view, self, arg, Some(ok)).as_ref(),
                    Some(ok),
                    &arg.pointer,
                );
            }
            Term::Fail {
                case: None,
                payload: None,
            } if matches!(result, TypeExpr::Option(_)) => {
                self.inputs
                    .push(json!({"at":at,"kind":"fail","route":"option_none_checked"}));
            }
            Term::Fail {
                case: Some(case),
                payload,
            } => {
                let TypeExpr::Result { error, .. } = result else {
                    self.invalid_failure(view, at);
                    return Ok(());
                };
                let TypeExpr::Named(named) = &**error else {
                    self.invalid_failure(view, at);
                    return Ok(());
                };
                if view.cx.trait_definition(&named.definition).is_none()
                    || !named.arguments.is_empty()
                {
                    self.deferred.push(at.into());
                    return Ok(());
                }
                let Some(expected) = view.error_case(case) else {
                    self.invalid_failure(view, at);
                    return Ok(());
                };
                match (expected,payload) {
                    (None,None) => self.inputs.push(json!({"at":at,"kind":"fail","route":"named_unit_case_checked","case":case})),
                    (Some(expected),Some(arg)) => self.return_value(view.cx,operand_type(view,self,arg,Some(&expected)).as_ref(),Some(&expected),&arg.pointer),
                    _ => self.conflicts.push((at.into(),format!("failure payload presence conflicts with case `{case}` of the function result"))),
                }
            }
            Term::Fail { .. } => self.invalid_failure(view, at),
            _ => {}
        }
        checkpoint()
    }

    fn invalid_failure(&mut self, view: &FnExp<'_, '_>, at: &str) {
        self.conflicts.push((
            at.into(),
            format!(
                "failure route is incompatible with the function result {}",
                view.result
                    .as_ref()
                    .map_or_else(|| "unknown".into(), |ty| view.cx.render(ty))
            ),
        ));
    }
}
