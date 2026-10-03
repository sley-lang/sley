//! Bound named immediates share the literal reader's exact member query keys.
use super::{FnExp, Node, Result};
use crate::{
    error::{AgentError, AgentErrorCode},
    types::TypeNames,
    values::TypeDefs,
};
use sley_ssmc::{Immediate, NamedType, TypeExpr, VariantImmediate};

pub(super) fn resolve(
    view: &FnExp<'_, '_>,
    node: &Node,
    result: &TypeExpr,
    roots: &mut Vec<TypeExpr>,
) -> Result<Option<Immediate>> {
    let Some(text) = node.immediate.as_ref().and_then(serde_json::Value::as_str) else {
        return Ok(None);
    };
    let error = |detail: &str| {
        AgentError::new(
            AgentErrorCode::ResidualConstraintConflict,
            format!("{}: bound named immediate `{text}`: {detail}", node.pointer),
        )
    };
    let (definition, leaf) = if node.row.tag == 18 {
        (
            view.cx
                .type_definition(text)
                .ok_or_else(|| error("no bound type definition"))?,
            None,
        )
    } else if let Some((name, leaf)) = text.rsplit_once('.') {
        (
            view.cx
                .type_definition(name)
                .ok_or_else(|| error("no bound type definition"))?,
            Some(leaf),
        )
    } else if node.row.tag == 20 {
        let TypeExpr::Named(named) = result else {
            return Ok(None);
        };
        (named.definition, Some(text))
    } else {
        return Err(error("the ordinary operation requires Type.member"));
    };
    roots.push(TypeExpr::Named(NamedType {
        definition,
        arguments: vec![],
    }));
    let Some(leaf) = leaf else {
        return Ok(Some(Immediate::Entity(definition)));
    };
    let member_id = view
        .cx
        .member(&definition, leaf)
        .ok_or_else(|| error("no complete bound member"))?;
    Ok(Some(if node.row.tag == 19 {
        Immediate::Field(member_id)
    } else {
        Immediate::Variant(VariantImmediate {
            definition,
            member_id,
        })
    }))
}
