//! Shared member bindings for ordinary literal reads and VM type projections.
use super::{Context, EntityId, MemberId, TypeDefForm};
use crate::values::TypeDefs;
use sley_ssmc::TypeExpr;

impl Context<'_> {
    fn value_members(&self, id: &EntityId) -> Option<Vec<(String, MemberId)>> {
        let definition = self.trait_definition(id)?;
        let (_, names) = self.members(id)?;
        let ids: Vec<_> = match definition.form {
            TypeDefForm::Record(fields) => {
                fields.into_iter().map(|field| field.member_id).collect()
            }
            TypeDefForm::Variant(cases) => cases.into_iter().map(|case| case.member_id).collect(),
        };
        Some(names.into_iter().map(|(name, _)| name).zip(ids).collect())
    }
}

impl TypeDefs for Context<'_> {
    fn form(&self, id: &EntityId) -> Option<TypeDefForm> {
        self.trait_definition(id).map(|definition| definition.form)
    }
    fn member(&self, id: &EntityId, leaf: &str) -> Option<MemberId> {
        self.value_members(id)?
            .into_iter()
            .find(|(name, _)| name == leaf)
            .map(|(_, id)| id)
    }
    fn member_leaf(&self, id: &EntityId, member: &MemberId) -> String {
        self.value_members(id)
            .and_then(|members| members.into_iter().find(|(_, id)| id == member))
            .map_or_else(|| "?".into(), |(name, _)| name)
    }
    fn render(&self, ty: &TypeExpr) -> String {
        self.render(ty)
    }
}
