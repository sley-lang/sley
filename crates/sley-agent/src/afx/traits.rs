//! Read-only definition views for canonical trait checks. No graph identities
//! are allocated: draft member query keys bind their definition and ordinal.

use super::{Context, EntityBodyValue, EntityId, Members, Value, is_identifier};
use sha2::{Digest, Sha256};
use sley_ssmc::{MemberId, RecordField, TypeDefForm, TypeDefinition, VariantCase, Visibility};
use std::collections::BTreeSet;

mod constants;
mod environment;
mod values;

pub(super) fn shape_complete(decl: &Value, members: &Members) -> bool {
    let (record, rows) = match (decl.get("record"), decl.get("variant")) {
        (Some(Value::Array(rows)), None) => (true, rows),
        (None, Some(Value::Array(rows))) => (false, rows),
        _ => return false,
    };
    if rows.len() != members.len() {
        return false;
    }
    let mut names = BTreeSet::new();
    rows.iter().zip(members).all(|(raw, (name, ty))| {
        is_identifier(name)
            && names.insert(name)
            && match raw {
                Value::String(_) => !record,
                Value::Array(pair) if pair.len() == 2 => {
                    if record || !pair[1].is_null() {
                        ty.is_some()
                    } else {
                        true
                    }
                }
                _ => false,
            }
    })
}

impl Context<'_> {
    /// Complete member/type-parameter view for read-only type checks. None keeps
    /// malformed or shadowed declarations unresolved instead of treating a
    /// lossy parser view as an empty, admissible definition.
    pub(crate) fn trait_definition(&self, id: &EntityId) -> Option<TypeDefinition> {
        let old = match self.program.body(id) {
            Some(EntityBodyValue::TypeDef(body)) => Some(body),
            _ => None,
        };
        let (form, parameters, invariants, visibility) = if let Some(name) = self.type_names.get(id)
        {
            let declared = &self.types[name];
            if declared.id != *id || !declared.trait_shape_complete {
                return None;
            }
            let member = |index: usize| {
                if old.is_some()
                    && let Some(member) = self
                        .names
                        .resolve_member_leaf(id, &declared.members[index].0)
                {
                    return member;
                }
                let mut hash = Sha256::new();
                hash.update(b"afxtrait-member-query-v1");
                hash.update(id.as_bytes());
                hash.update((index as u64).to_be_bytes());
                MemberId::from_bytes(hash.finalize().into())
            };
            let form = if declared.variant {
                TypeDefForm::Variant(
                    declared
                        .members
                        .iter()
                        .enumerate()
                        .map(|(index, (_, ty))| VariantCase {
                            member_id: member(index),
                            payload_type: ty.clone(),
                        })
                        .collect(),
                )
            } else {
                TypeDefForm::Record(
                    declared
                        .members
                        .iter()
                        .enumerate()
                        .map(|(index, (_, ty))| RecordField {
                            member_id: member(index),
                            value_type: ty.clone().expect("complete record field"),
                            visibility: Visibility::Exported,
                        })
                        .collect(),
                )
            };
            (
                form,
                old.map_or_else(Vec::new, |body| body.type_parameters.clone()),
                old.map_or_else(Vec::new, |body| body.invariants.as_slice().to_vec()),
                old.map_or(Visibility::Exported, |body| body.visibility),
            )
        } else {
            if self.top.contains(&self.names.name(id)) {
                return None;
            }
            let old = old?;
            (
                old.form.clone(),
                old.type_parameters.clone(),
                old.invariants.as_slice().to_vec(),
                old.visibility,
            )
        };
        Some(TypeDefinition {
            entity_id: *id,
            type_parameters: parameters,
            form,
            invariants,
            visibility,
        })
    }
}
