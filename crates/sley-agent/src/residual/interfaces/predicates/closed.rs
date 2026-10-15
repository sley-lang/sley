//! Canonical closure for explicit annotations and bound reference/call types.

use super::{AgentError, AgentErrorCode, Check, Known, Result, json};
use sley_check::TypeErrorCode;

impl Check<'_, '_> {
    pub(super) fn closed_type(&mut self, known: &Known, at: &str) -> Result<()> {
        let (ty, source) = known;
        let status = match super::type_traits::check(self.cx, ty, self.budget, |types| {
            types.check_closed_type(ty)
        })? {
            Some(Ok(())) => "checked",
            None => {
                return Err(super::type_traits::incomplete(self.cx, ty, source, at));
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
                    format!("{at}: type from {source} is not closed and well-formed: {error}"),
                ));
            }
        };
        self.closed_types
            .insert(json!({"at":at,"source":source,"closure":status}));
        Ok(())
    }
}
