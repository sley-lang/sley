//! Named record/variant connections using the bound AF1-X definition view.

use super::{AgentError, AgentErrorCode, Check, Known, Result, TypeExpr, Value, conflict, json};
use crate::types::TypeNames;
use sley_check::TypeErrorCode;
use sley_ssmc::{NamedType, TypeDefForm};

impl Check<'_, '_> {
    pub(super) fn named_operation(
        &mut self,
        tag: u32,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        if items.len() < 2
            || (matches!(tag, 19 | 21) && items.len() != 3)
            || (tag == 20 && items.len() > 3)
        {
            return Err(conflict(at, "wrong named operation operand count"));
        }
        let immediate = items[1].as_str().ok_or_else(|| {
            conflict(
                &pointers[1],
                "named operation requires a type or member name",
            )
        })?;
        // A short variant constructor case resolves through the emitted
        // result hint; a qualified member keeps its own definition.
        let emitted = (tag == 20 && !immediate.contains('.'))
            .then(|| self.emitted_hint(at))
            .flatten();
        let expected = emitted.as_ref().or(expected);
        let resolved = if tag == 18 {
            Some((self.named_type(immediate, &pointers[1])?, None))
        } else if let Some((name, leaf)) = immediate.rsplit_once('.') {
            Some((self.named_type(name, &pointers[1])?, Some(leaf)))
        } else if tag == 19 {
            return Err(conflict(
                &pointers[1],
                "field requires a qualified Type.field name",
            ));
        } else if let Some((TypeExpr::Named(named), _)) = expected {
            Some((TypeExpr::Named(named.clone()), Some(immediate)))
        } else {
            // AF1-X resolves a short case through a named result annotation.
            // The operand of variant_get does not provide that annotation.
            None
        };
        let (result, status) = if let Some((ty, leaf)) = &resolved {
            if let Some(members) = self.named_shape(ty, tag >= 20, &pointers[1])? {
                let result = if tag == 18 {
                    self.record_operands(items, pointers, &members, depth)?;
                    ty.clone()
                } else {
                    let leaf = leaf.expect("member operation");
                    let payload =
                        members
                            .iter()
                            .find(|(name, _)| name == leaf)
                            .ok_or_else(|| {
                                conflict(
                                    &pointers[1],
                                    &format!("{} has no member `{leaf}`", self.cx.render(ty)),
                                )
                            })?;
                    self.member_operands(tag, items, pointers, ty, payload, depth)?
                };
                (Some((result, at.into())), "checked")
            } else {
                self.named_unknown_operands(items, pointers, depth)?;
                (
                    (matches!(tag, 18 | 20)).then(|| (ty.clone(), at.into())),
                    "definition_deferred",
                )
            }
        } else {
            self.named_unknown_operands(items, pointers, depth)?;
            (None, "immediate_deferred")
        };
        if status == "checked" {
            self.deferred.remove(at);
        } else {
            self.deferred.insert(at.into());
        }
        self.named_values
            .insert(json!({"at":at,"immediate":immediate,
            "members":status,"result_type":result.as_ref().map(|(ty,_)|self.cx.render(ty)),
            "rewritten":false}));
        Ok(result)
    }

    fn named_type(&self, name: &str, at: &str) -> Result<TypeExpr> {
        let definition = self.cx.type_definition(name).ok_or_else(|| {
            conflict(
                at,
                &format!("no named type `{name}` in the bound interface"),
            )
        })?;
        Ok(TypeExpr::Named(NamedType {
            definition,
            arguments: Vec::new(),
        }))
    }

    fn named_shape(
        &mut self,
        ty: &TypeExpr,
        variant: bool,
        at: &str,
    ) -> Result<Option<crate::afx::Members>> {
        self.budget.checkpoint()?;
        let TypeExpr::Named(named) = ty else {
            unreachable!("resolved named type")
        };
        let Some(definition) = self.cx.trait_definition(&named.definition) else {
            return Err(super::type_traits::incomplete(self.cx, ty, at, at));
        };
        if !named.arguments.is_empty() || !definition.type_parameters.is_empty() {
            return Err(conflict(
                at,
                "record/variant operations require a non-generic definition",
            ));
        }
        if matches!(definition.form, TypeDefForm::Variant(_)) != variant {
            return Err(conflict(
                at,
                if variant {
                    "operation requires a variant definition"
                } else {
                    "operation requires a record definition"
                },
            ));
        }
        match super::type_traits::check(self.cx, ty, self.budget, |types| {
            types.check_closed_type(ty)
        })? {
            Some(Ok(())) => Ok(self
                .cx
                .members(&named.definition)
                .map(|(_, members)| members)),
            None => Err(super::type_traits::incomplete(self.cx, ty, at, at)),
            Some(Err(error)) => Err(AgentError::new(
                if matches!(
                    error.code(),
                    TypeErrorCode::ResourceLimit | TypeErrorCode::DepthLimit
                ) {
                    AgentErrorCode::ResidualLimit
                } else {
                    AgentErrorCode::ResidualConstraintConflict
                },
                format!("{at}: named definition {}: {error}", self.cx.render(ty)),
            )),
        }
    }

    fn record_operands(
        &mut self,
        items: &[Value],
        pointers: &[String],
        members: &crate::afx::Members,
        depth: usize,
    ) -> Result<()> {
        if items.len() - 2 != members.len() {
            return Err(conflict(
                &pointers[0],
                "record operand count conflicts with its field count",
            ));
        }
        for (index, (name, ty)) in members.iter().enumerate() {
            let expected = (
                ty.clone().expect("complete record"),
                format!("{} (field `{name}`)", pointers[1]),
            );
            self.expression(
                &items[index + 2],
                &pointers[index + 2],
                Some(&expected),
                depth + 1,
            )?;
        }
        Ok(())
    }

    fn member_operands(
        &mut self,
        tag: u32,
        items: &[Value],
        pointers: &[String],
        ty: &TypeExpr,
        member: &(String, Option<TypeExpr>),
        depth: usize,
    ) -> Result<TypeExpr> {
        let (name, payload) = member;
        if tag == 20 {
            if items.len() - 2 != usize::from(payload.is_some()) {
                return Err(conflict(
                    &pointers[0],
                    "variant payload presence conflicts with its case",
                ));
            }
            if let Some(payload) = payload {
                let expected = (payload.clone(), format!("{} (case `{name}`)", pointers[1]));
                self.expression(&items[2], &pointers[2], Some(&expected), depth + 1)?;
            }
            return Ok(ty.clone());
        }
        let payload = payload
            .as_ref()
            .ok_or_else(|| conflict(&pointers[1], "cannot extract a payload from a unit case"))?;
        self.expression(
            &items[2],
            &pointers[2],
            Some(&(ty.clone(), pointers[1].clone())),
            depth + 1,
        )?;
        Ok(if tag == 21 {
            TypeExpr::Option(Box::new(payload.clone()))
        } else {
            payload.clone()
        })
    }

    fn named_unknown_operands(
        &mut self,
        items: &[Value],
        pointers: &[String],
        depth: usize,
    ) -> Result<()> {
        for index in 2..items.len() {
            self.expression(&items[index], &pointers[index], None, depth + 1)?;
        }
        Ok(())
    }
}
