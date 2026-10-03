//! Bound reference metadata for residual preflight, without fallback through
//! a deleted or shadowed accepted name.

use super::{Context, EntityBodyValue, EntityId, Scope, TypeExpr};

impl Context<'_> {
    /// AF1 `define_function` emits an empty effect declaration for definitions
    /// and patches, including operation edits that restate their owner; the residual target overlay follows the same rule. This
    /// describes the emitted interface, not whether its body satisfies it.
    /// Unchanged functions retain their accepted effect identities.
    pub(crate) fn reference_function_effects(&self, name: &str) -> Option<&[EntityId]> {
        if self.declared_signature(name) {
            return Some(&[]);
        }
        if self.top.contains(name) || self.reference_deleted(name) {
            return None;
        }
        self.live_function(name)
            .map(|(_, body)| body.effects.as_slice())
    }

    /// Outer None means no constant; inner None means an unresolved draft type.
    #[allow(clippy::option_option)]
    pub(crate) fn reference_constant_type(&self, name: &str) -> Option<Option<TypeExpr>> {
        if let Some(ty) = self.consts.get(name) {
            return Some(ty.clone());
        }
        if self.top.contains(name) {
            return None;
        }
        let id = self.names.resolve(name)?;
        if self.names.scope(&id) != Scope::Top || self.deleted_references.contains(&id) {
            return None;
        }
        match self.program.body(&id) {
            Some(EntityBodyValue::Constant(body)) => Some(Some(body.value.value_type.clone())),
            _ => None,
        }
    }

    pub(crate) fn declared_constant(&self, name: &str) -> bool {
        self.consts.contains_key(name)
    }

    /// A global initializer is bound by identity. Deleting/recreating its leaf
    /// name does not retarget the global to the replacement constant.
    #[allow(clippy::option_option)]
    pub(crate) fn reference_initializer_type(&self, id: &EntityId) -> Option<Option<TypeExpr>> {
        if self.deleted_references.contains(id)
            || !matches!(self.program.body(id), Some(EntityBodyValue::Constant(_)))
        {
            return None;
        }
        self.reference_constant_type(&self.names.name(id))
    }

    pub(crate) fn reference_deleted(&self, name: &str) -> bool {
        self.names
            .resolve(name)
            .is_some_and(|id| self.deleted_references.contains(&id))
    }

    pub(crate) fn reference_global(
        &self,
        name: &str,
    ) -> Option<&sley_mutate::value::GlobalValueBody> {
        if self.top.contains(name) {
            return None;
        }
        let id = self.names.resolve(name)?;
        if self.names.scope(&id) != Scope::Top || self.deleted_references.contains(&id) {
            return None;
        }
        match self.program.body(&id) {
            Some(EntityBodyValue::GlobalValue(body)) => Some(body),
            _ => None,
        }
    }
}
