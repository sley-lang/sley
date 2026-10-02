//! Complete callee declarations are required before fragment connections.
use super::conflict;
use crate::{afx::Context, error::Result};
use sley_ssmc::TypeExpr;

pub(super) fn read(
    cx: &Context<'_>,
    name: &str,
    at: &str,
) -> Result<Option<(Vec<TypeExpr>, TypeExpr)>> {
    if let Some(error) = cx.signature_issue(name) {
        return Err(conflict(
            at,
            &format!(
                "function `{name}` has an incomplete bound signature: {}; repair the declaration with ordinary AF1-X before residual expansion",
                error.detail()
            ),
        ));
    }
    let Some((params, result)) = cx.signature(name) else {
        return Ok(None);
    };
    let incomplete = || {
        conflict(
            at,
            &format!(
                "function `{name}` has an incomplete bound signature; repair its parameter/result declarations with ordinary AF1-X before residual expansion"
            ),
        )
    };
    let params = params
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(incomplete)?;
    let result = result.ok_or_else(incomplete)?;
    Ok(Some((params, result)))
}
