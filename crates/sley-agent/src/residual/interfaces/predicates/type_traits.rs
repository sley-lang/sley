//! Canonical type judgments share the AF1-X read-only reachable definition view.

use super::{Context, Result};
use crate::residual::frontier::Budget;
use sley_check::{TypeEnvironment, TypeError};
use sley_ssmc::TypeExpr;

pub(in super::super) fn check<T>(
    cx: &Context<'_>,
    ty: &TypeExpr,
    budget: &mut Budget,
    judge: impl FnOnce(&TypeEnvironment) -> std::result::Result<T, TypeError>,
) -> Result<Option<std::result::Result<T, TypeError>>> {
    cx.trait_check_with_checkpoint(ty, &mut || budget.checkpoint(), judge)
}

pub(super) fn environment(
    cx: &Context<'_>,
    ty: &TypeExpr,
    budget: &mut Budget,
) -> Result<Option<std::result::Result<TypeEnvironment, TypeError>>> {
    cx.type_environment_with_checkpoint(ty, &mut || budget.checkpoint())
}

/// An unavailable bound definition cannot establish a fragment interface.
/// Ordinary authoring can repair the declaration; no shape or type is guessed.
pub(in super::super) fn incomplete(
    cx: &Context<'_>,
    ty: &TypeExpr,
    source: &str,
    at: &str,
) -> crate::error::AgentError {
    super::conflict(
        at,
        &format!(
            "interface type {} from {source} has an incomplete or unavailable bound definition; repair the declaration with ordinary AF1-X before fragment composition",
            cx.render(ty)
        ),
    )
}
