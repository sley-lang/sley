//! Validate literal contents through the ordinary reader and canonical checker.
//! Draft member identities exist only in this read-only validation view.

use super::{AgentError, AgentErrorCode, Check, Known, Result, Value, conflict};
use crate::values;
use sley_check::{TypeError, TypeErrorCode};

fn type_error(at: &str, source: &str, error: &TypeError) -> AgentError {
    AgentError::new(
        if matches!(
            error.code(),
            TypeErrorCode::ResourceLimit | TypeErrorCode::DepthLimit
        ) {
            AgentErrorCode::ResidualLimit
        } else {
            AgentErrorCode::ResidualConstraintConflict
        },
        format!("{at}: literal conflicts with {source}: {error}"),
    )
}

impl Check<'_, '_> {
    pub(super) fn literal(
        &mut self,
        value: &Value,
        at: &str,
        ty: Option<&Known>,
    ) -> Result<Option<Known>> {
        let Some(ty) = ty else {
            self.deferred.insert(at.into());
            return Ok(None);
        };
        let Some(types) = super::type_traits::environment(self.cx, &ty.0, self.budget)? else {
            return Err(super::type_traits::incomplete(self.cx, &ty.0, &ty.1, at));
        };
        let types = types.map_err(|error| type_error(at, &ty.1, &error))?;
        types
            .check_closed_type(&ty.0)
            .map_err(|error| type_error(at, &ty.1, &error))?;
        let constant = values::read_with_check(value, &ty.0, self.cx, at, &mut |value| {
            self.budget.charge(match value {
                Value::String(text) => text.len().div_ceil(64).max(1),
                Value::Array(items) => items.len().max(1),
                Value::Object(members) => members.len().max(1),
                _ => 1,
            })
        })
        .map_err(|error| {
            if error.code() == AgentErrorCode::ResidualLimit {
                error
            } else {
                conflict(
                    at,
                    &format!("literal conflicts with {}: {}", ty.1, error.detail()),
                )
            }
        })?;
        self.budget.checkpoint()?;
        types
            .check_constant(&constant)
            .map_err(|error| type_error(at, &ty.1, &error))?;
        self.budget.checkpoint()?;
        self.deferred.remove(at);
        Ok(Some(ty.clone()))
    }
}
