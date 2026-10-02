//! Canonical AF1 signature readers shared with the read-only interface context.

use super::{NAME_GRAMMAR, Result, TypeExpr, Value, array, frame, is_identifier, string};
use crate::error::AgentError;
use serde_json::Map;

pub(crate) enum ParameterError {
    Declaration(AgentError),
    Checkpoint(AgentError),
}

impl ParameterError {
    pub(crate) fn into_error(self) -> AgentError {
        match self {
            Self::Declaration(error) | Self::Checkpoint(error) => error,
        }
    }
}

pub(crate) fn read_params(
    value: Option<&Value>,
    pointer: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
    read_type: &mut impl FnMut(&Value, &str) -> Result<TypeExpr>,
) -> core::result::Result<Vec<(String, TypeExpr)>, ParameterError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let mut out: Vec<(String, TypeExpr)> = Vec::new();
    for (index, param) in array(value, pointer)
        .map_err(ParameterError::Declaration)?
        .iter()
        .enumerate()
    {
        checkpoint().map_err(ParameterError::Checkpoint)?;
        let param_pointer = format!("{pointer}/{index}");
        let pair = param
            .as_array()
            .filter(|pair| pair.len() == 2)
            .ok_or_else(|| {
                ParameterError::Declaration(frame(
                    &param_pointer,
                    "a parameter is [\"name\", type]",
                ))
            })?;
        let name = string(&pair[0], &param_pointer).map_err(ParameterError::Declaration)?;
        if !is_identifier(name) || out.iter().any(|(existing, _)| existing == name) {
            return Err(ParameterError::Declaration(frame(
                &param_pointer,
                format!("`{name}` is not a fresh parameter name ({NAME_GRAMMAR}, distinct)"),
            )));
        }
        out.push((
            name.to_owned(),
            read_type(&pair[1], &param_pointer).map_err(ParameterError::Declaration)?,
        ));
    }
    Ok(out)
}

pub(crate) fn read_result(
    declaration: &Map<String, Value>,
    pointer: &str,
    read_type: &mut impl FnMut(&Value, &str) -> Result<TypeExpr>,
) -> Result<TypeExpr> {
    let value = declaration
        .get("returns")
        .ok_or_else(|| frame(pointer, "missing \"returns\""))?;
    read_type(value, &format!("{pointer}/returns"))
}
