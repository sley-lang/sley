//! Shared canonical judgments over the reachable read-only definition view.

use super::super::Context;
use crate::error::Result;
use sley_check::{TypeEnvironment, TypeError};
use sley_ssmc::{TypeDefForm, TypeExpr};
use std::collections::BTreeMap;

impl Context<'_> {
    pub(crate) fn trait_check_with_checkpoint<T>(
        &self,
        ty: &TypeExpr,
        checkpoint: &mut impl FnMut() -> Result<()>,
        judge: impl FnOnce(&TypeEnvironment) -> std::result::Result<T, TypeError>,
    ) -> Result<Option<std::result::Result<T, TypeError>>> {
        let result = self
            .type_environment_with_checkpoint(ty, checkpoint)?
            .map(|types| types.and_then(|types| judge(&types)));
        checkpoint()?;
        Ok(result)
    }

    pub(crate) fn type_environment_with_checkpoint(
        &self,
        ty: &TypeExpr,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<std::result::Result<TypeEnvironment, TypeError>>> {
        let mut definitions = BTreeMap::new();
        let mut pending = vec![ty.clone()];
        while let Some(ty) = pending.pop() {
            checkpoint()?;
            match ty {
                TypeExpr::Named(named) => {
                    pending.extend(named.arguments);
                    if definitions.contains_key(&named.definition) {
                        continue;
                    }
                    let Some(definition) = self.trait_definition(&named.definition) else {
                        return Ok(None);
                    };
                    match &definition.form {
                        TypeDefForm::Record(fields) => {
                            pending.extend(fields.iter().map(|field| field.value_type.clone()));
                        }
                        TypeDefForm::Variant(cases) => {
                            pending
                                .extend(cases.iter().filter_map(|case| case.payload_type.clone()));
                        }
                    }
                    definitions.insert(named.definition, definition);
                }
                TypeExpr::Tuple(items) => pending.extend(items),
                TypeExpr::Vector(item) | TypeExpr::Option(item) | TypeExpr::LocalCell(item) => {
                    pending.push(*item);
                }
                TypeExpr::OrderedMap { key, value } => {
                    pending.push(*key);
                    pending.push(*value);
                }
                TypeExpr::Result { ok, error } => {
                    pending.push(*ok);
                    pending.push(*error);
                }
                TypeExpr::FunctionRef(function) => {
                    pending.extend(function.parameters);
                    pending.push(*function.result);
                }
                _ => {}
            }
        }
        checkpoint()?;
        let result = TypeEnvironment::new(definitions.into_values().collect());
        checkpoint()?;
        Ok(Some(result))
    }
}
