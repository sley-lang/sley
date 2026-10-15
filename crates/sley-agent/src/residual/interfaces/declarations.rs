//! Canonical closure of declared interfaces, including unused bindings.

use super::{Checker, predicates};
use crate::error::{AgentError, AgentErrorCode, Result};
use sley_check::TypeErrorCode;

impl Checker<'_, '_> {
    pub(super) fn check_declared_types(&mut self) -> Result<Vec<String>> {
        let mut deferred = Vec::new();
        for (at, ty) in &self.declared_types {
            match predicates::type_traits::check(self.cx, ty, self.budget, |types| {
                types.check_closed_type(ty)
            })? {
                Some(Ok(())) => {}
                None => deferred.push(at.clone()),
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
                        format!(
                            "{at}: declared interface type {} is not closed and well-formed: {error}",
                            self.cx.render(ty)
                        ),
                    ));
                }
            }
        }
        if let Some(at) = deferred.first() {
            return Err(predicates::type_traits::incomplete(
                self.cx,
                &self.declared_types[at],
                at,
                at,
            ));
        }
        Ok(deferred)
    }
}
