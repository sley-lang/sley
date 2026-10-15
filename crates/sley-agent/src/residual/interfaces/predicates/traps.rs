//! Trap payloads use the canonical CFG persistability rule, without execution.

use super::{Context, Result, Scope, Value, conflict, json};
use crate::error::{AgentError, AgentErrorCode};
use crate::residual::frontier::Budget;
use sley_check::TypeErrorCode;

pub(in super::super) fn trap(
    cx: &Context<'_>,
    scope: Scope<'_>,
    items: &[Value],
    at: &str,
    budget: &mut Budget,
) -> Result<Value> {
    budget.checkpoint()?;
    if !(1..=3).contains(&items.len()) {
        return Err(conflict(
            at,
            "trap takes an optional code and optional payload",
        ));
    }
    let code = match items.get(1) {
        None => "unreachable",
        Some(Value::String(code)) if crate::exec::trap_code(code).is_some() => code,
        Some(_) => {
            return Err(conflict(
                &format!("{at}/1"),
                "expected a supported trap code",
            ));
        }
    };
    let mut report = json!({"at":at,"terminal_interface":"checked",
        "control_flow":"terminal_without_successors","kind":"trap","code":code,
        "payload_eligibility":"absent","evaluated":false,"rewritten":false});
    if let Some(payload) = items.get(2) {
        let payload_at = format!("{at}/2");
        let checked = super::value(cx, scope, payload, &payload_at, None, budget)?;
        let eligibility = if let Some(ty) = &checked.ty {
            match super::type_traits::check(cx, ty, budget, |types| {
                types.check_closed_type(ty)?;
                types.traits(ty)
            })? {
                Some(Ok(traits)) if traits.persistable => "persistable_checked",
                None => "definition_deferred",
                Some(Ok(_)) => {
                    return Err(conflict(&payload_at, "trap payload must be persistable"));
                }
                Some(Err(error)) => {
                    return Err(AgentError::new(
                        if matches!(
                            error.code(),
                            TypeErrorCode::ResourceLimit | TypeErrorCode::DepthLimit
                        ) {
                            AgentErrorCode::ResidualLimit
                        } else {
                            AgentErrorCode::ResidualConstraintConflict
                        },
                        format!("{payload_at}: trap payload: {error}"),
                    ));
                }
            }
        } else {
            "unresolved"
        };
        report["payload_eligibility"] = json!(eligibility);
        report["payload_interface"] = checked.report;
    }
    Ok(report)
}
