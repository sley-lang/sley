//! Retained immediates keep their accepted identity; names only locate overlays.
use super::{FnExp, Result, bindings::Bindings, signature};
use crate::error::{AgentError, AgentErrorCode};
use sley_mutate::value::OperationBody;
use sley_ssmc::{ConstantDefinition, Immediate, NamedType, TypeExpr};

pub(super) fn read(
    view: &FnExp<'_, '_>,
    op: &OperationBody,
    at: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Option<Bindings>> {
    checkpoint()?;
    let mut bound = Bindings::new(op.immediate.clone());
    let error = |detail: String| {
        AgentError::new(
            AgentErrorCode::ResidualConstraintConflict,
            format!("{at}: retained bound immediate: {detail}"),
        )
    };
    match &op.immediate {
        Immediate::None | Immediate::Index(_) => {}
        Immediate::Function(reference) if matches!(op.opcode, 112 | 194) => {
            let name = view.cx.names.name(&reference.function);
            if view.cx.reference_deleted(&name)
                || view
                    .cx
                    .live_function(&name)
                    .is_none_or(|(id, _)| id != reference.function)
            {
                return Err(error(format!("callee `{name}` loses its bound identity")));
            }
            let Some((mut function, mut parameters)) = signature(view, &name, checkpoint)? else {
                return Err(error(format!(
                    "callee `{name}` has no complete current signature"
                )));
            };
            function.entity_id = reference.function;
            for parameter in &mut parameters {
                checkpoint()?;
                parameter.owner = reference.function;
            }
            bound.functions.push(function);
            bound.parameters = parameters;
        }
        Immediate::Entity(id) if op.opcode == 1 => {
            let name = view.cx.names.name(id);
            if view.cx.reference_initializer_type(id).is_none() {
                return Err(error(format!("constant `{name}` loses its bound identity")));
            }
            let Some(value) = view.cx.constant_value(&name, at, checkpoint)? else {
                return Err(error(format!("constant `{name}` is unresolved")));
            };
            bound.roots.push(value.value_type.clone());
            bound.constants.push(ConstantDefinition {
                entity_id: *id,
                value,
            });
        }
        Immediate::Entity(id) if op.opcode == 193 => {
            let name = view.cx.names.name(id);
            bound.global(view, &name, at, checkpoint)?;
        }
        Immediate::Entity(id) if op.opcode == 18 => {
            if view.cx.trait_definition(id).is_none() {
                return Err(error(format!(
                    "record definition `{}` is unavailable or incomplete",
                    view.cx.names.name(id)
                )));
            }
            bound.roots.push(TypeExpr::Named(NamedType {
                definition: *id,
                arguments: vec![],
            }));
        }
        Immediate::Field(_) if op.opcode == 19 => {}
        Immediate::Variant(variant) if matches!(op.opcode, 20 | 21) => {
            if view.cx.trait_definition(&variant.definition).is_none() {
                return Err(error(format!(
                    "variant definition `{}` is unavailable or incomplete",
                    view.cx.names.name(&variant.definition)
                )));
            }
            bound.roots.push(TypeExpr::Named(NamedType {
                definition: variant.definition,
                arguments: vec![],
            }));
        }
        _ => return Ok(None),
    }
    Ok(Some(bound))
}
