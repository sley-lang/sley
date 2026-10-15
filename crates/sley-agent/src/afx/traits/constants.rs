//! Read real literal/constant values through the ordinary reader and checker.
use super::Context;
use crate::{
    error::{AgentError, AgentErrorCode, Result},
    values,
};
use serde_json::Value;
use sley_check::{TypeEnvironment, TypeError, TypeErrorCode};
use sley_mutate::value::EntityBodyValue;
use sley_ssmc::{ConstValue, TypeExpr};

fn error(at: &str, detail: impl std::fmt::Display) -> AgentError {
    AgentError::new(
        AgentErrorCode::ResidualConstraintConflict,
        format!("{at}: constant/literal compatibility: {detail}"),
    )
}
fn type_error(at: &str, detail: &TypeError) -> AgentError {
    if matches!(
        detail.code(),
        TypeErrorCode::ResourceLimit | TypeErrorCode::DepthLimit
    ) {
        AgentError::new(
            AgentErrorCode::ResidualLimit,
            format!("{at}: constant/literal compatibility: {detail}"),
        )
    } else {
        error(at, detail)
    }
}

impl Context<'_> {
    fn value_environment(
        &self,
        ty: &TypeExpr,
        at: &str,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<TypeEnvironment>> {
        let Some(types) = self.type_environment_with_checkpoint(ty, checkpoint)? else {
            return Ok(None);
        };
        let types = types.map_err(|e| type_error(at, &e))?;
        types
            .check_closed_type(ty)
            .map_err(|e| type_error(at, &e))?;
        checkpoint()?;
        Ok(Some(types))
    }
    pub(crate) fn literal_value(
        &self,
        value: &Value,
        ty: &TypeExpr,
        at: &str,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<ConstValue>> {
        let Some(types) = self.value_environment(ty, at, checkpoint)? else {
            return Ok(None);
        };
        let value =
            values::read_with_check(value, ty, self, at, &mut |_| checkpoint()).map_err(|e| {
                if e.code() == AgentErrorCode::ResidualLimit {
                    e
                } else {
                    error(at, e.detail())
                }
            })?;
        checkpoint()?;
        types
            .check_constant(&value)
            .map_err(|e| type_error(at, &e))?;
        checkpoint()?;
        Ok(Some(value))
    }
    pub(crate) fn constant_value(
        &self,
        name: &str,
        at: &str,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<ConstValue>> {
        checkpoint()?;
        if let Some((data, source)) = self.constant_values.get(name) {
            let Some(Some(ty)) = self.consts.get(name) else {
                return Ok(None);
            };
            let value = data
                .as_ref()
                .ok_or_else(|| error(source, "missing value"))?;
            return self
                .literal_value(value, ty, source, checkpoint)
                .map_err(|e| {
                    AgentError::new(e.code(), format!("constant `{name}`: {}", e.detail()))
                });
        }
        if self.top.contains(name) || self.reference_deleted(name) {
            return Err(error(at, format!("no bound constant `{name}`")));
        }
        let value = self
            .names
            .resolve(name)
            .and_then(|id| self.program.body(&id))
            .and_then(|body| {
                if let EntityBodyValue::Constant(body) = body {
                    Some(&body.value)
                } else {
                    None
                }
            })
            .ok_or_else(|| error(at, format!("no bound constant `{name}`")))?;
        let Some(types) = self.value_environment(&value.value_type, at, checkpoint)? else {
            return Ok(None);
        };
        types
            .check_constant(value)
            .map_err(|e| type_error(at, &e))?;
        checkpoint()?;
        Ok(Some(value.clone()))
    }
    pub(crate) fn check_declared_constants(
        &self,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        for (name, (_, at)) in &self.constant_values {
            checkpoint()?;
            self.constant_value(name, at, checkpoint)?;
        }
        Ok(())
    }
}
