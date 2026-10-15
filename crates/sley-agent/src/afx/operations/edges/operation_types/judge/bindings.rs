//! Read-only VM inventories use actual constant data and bound global identity.
use super::{FnExp, Node, Result, named, signature};
use crate::{
    error::{AgentError, AgentErrorCode},
    frame::constants,
    opcodes::ImmediateKind,
};
use sley_id::EntityId;
use sley_ssmc::{
    ConstantDefinition, FunctionGraph, FunctionRefValue, GlobalValueDefinition, Immediate,
    Parameter, TypeExpr,
};

pub(super) struct Bindings {
    pub immediate: Immediate,
    pub functions: Vec<FunctionGraph>,
    pub parameters: Vec<Parameter>,
    pub constants: Vec<ConstantDefinition>,
    pub globals: Vec<GlobalValueDefinition>,
    pub roots: Vec<TypeExpr>,
}
impl Bindings {
    pub(super) fn new(immediate: Immediate) -> Self {
        Self {
            immediate,
            functions: vec![],
            parameters: vec![],
            constants: vec![],
            globals: vec![],
            roots: vec![],
        }
    }

    pub(super) fn read(
        view: &FnExp<'_, '_>,
        node: &Node,
        result: &TypeExpr,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<Self>> {
        let mut inventory = Self::new(Immediate::None);
        inventory.immediate = match node.row.immediate {
            ImmediateKind::None => Immediate::None,
            ImmediateKind::Index => {
                let Some(index) = node
                    .immediate
                    .as_ref()
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
                else {
                    return Ok(None);
                };
                Immediate::Index(index)
            }
            ImmediateKind::Function if matches!(node.row.tag, 112 | 194) => {
                let Some(name) = node.immediate.as_ref().and_then(serde_json::Value::as_str) else {
                    return Ok(None);
                };
                let Some((function, params)) = signature(view, name, checkpoint)? else {
                    return Ok(None);
                };
                let immediate = Immediate::Function(FunctionRefValue {
                    function: function.entity_id,
                    type_arguments: vec![],
                });
                inventory.parameters = params;
                inventory.functions.push(function);
                immediate
            }
            _ if matches!(node.row.tag, 18..=21) => {
                let Some(immediate) = named::resolve(view, node, result, &mut inventory.roots)?
                else {
                    return Ok(None);
                };
                immediate
            }
            _ if node.row.tag == 1 => {
                let Some(value) = node.immediate.as_ref() else {
                    return Ok(None);
                };
                let constant = if let Some(name) = value.as_str() {
                    view.cx.constant_value(name, &node.pointer, checkpoint)?
                } else {
                    let ty =
                        constants::immediate_type(value, Some(result), view.cx, &node.pointer)?;
                    let data = value
                        .as_object()
                        .and_then(|object| object.get("value"))
                        .unwrap_or(value);
                    view.cx
                        .literal_value(data, &ty, &node.pointer, checkpoint)?
                };
                let Some(value) = constant else {
                    return Ok(None);
                };
                inventory.roots.push(value.value_type.clone());
                // Private lookup key only: no identity is allocated or published.
                let id = EntityId::from_bytes([0; 32]);
                inventory.constants.push(ConstantDefinition {
                    entity_id: id,
                    value,
                });
                Immediate::Entity(id)
            }
            _ if node.row.tag == 193 => {
                let Some(name) = node.immediate.as_ref().and_then(serde_json::Value::as_str) else {
                    return Ok(None);
                };
                inventory.global(view, name, &node.pointer, checkpoint)?
            }
            _ => return Ok(None),
        };
        Ok(Some(inventory))
    }
    pub(super) fn global(
        &mut self,
        view: &FnExp<'_, '_>,
        name: &str,
        at: &str,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Immediate> {
        let error = |text: &str| {
            AgentError::new(
                AgentErrorCode::ResidualConstraintConflict,
                format!("{at}: global `{name}`: {text}"),
            )
        };
        let global = view
            .cx
            .reference_global(name)
            .ok_or_else(|| error("no bound global"))?;
        if view
            .cx
            .reference_initializer_type(&global.initializer)
            .is_none()
        {
            return Err(error("bound initializer is deleted or unavailable"));
        }
        let initializer = view.cx.names.name(&global.initializer);
        let value = view
            .cx
            .constant_value(&initializer, at, checkpoint)?
            .ok_or_else(|| error("initializer type is unresolved"))?;
        let id = view
            .cx
            .names
            .resolve(name)
            .ok_or_else(|| error("no bound identity"))?;
        self.roots.push(value.value_type.clone());
        self.constants.push(ConstantDefinition {
            entity_id: global.initializer,
            value,
        });
        self.globals.push(GlobalValueDefinition {
            entity_id: id,
            value_type: global.value_type.clone(),
            initializer: global.initializer,
            visibility: global.visibility,
        });
        Ok(Immediate::Entity(id))
    }
}
