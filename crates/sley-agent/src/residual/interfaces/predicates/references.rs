//! Reference result types; this pass neither creates nor evaluates constants.

use super::{Check, Known, Result, TypeExpr, Value, conflict, json};
use sley_ssmc::FunctionType;

impl Check<'_, '_> {
    pub(super) fn reference(
        &mut self,
        tag: u32,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        self.budget.checkpoint()?;
        if items.len() != 2 {
            return Err(conflict(
                at,
                "reference operation requires exactly one immediate and no operands",
            ));
        }
        let (result, source) = if tag == 1 {
            let hint = self.scope.constant_hints.and_then(|hints| hints.get(at));
            self.constant_reference(&items[1], &pointers[1], hint)?
        } else {
            let name = items[1].as_str().ok_or_else(|| {
                conflict(&pointers[1], "reference requires an explicit entity name")
            })?;
            if tag == 193 {
                self.global_reference(name, &pointers[1])?
            } else {
                self.function_reference(name, &pointers[1])?
            }
        };
        if result.is_some() {
            self.deferred.remove(at);
        } else {
            self.deferred.insert(at.into());
        }
        if let Some(result) = &result {
            self.closed_type(result, at)?;
        }
        let effects = match &result {
            Some((TypeExpr::FunctionRef(function), _)) => Some(
                function
                    .effects
                    .iter()
                    .map(|id| crate::hex::encode(id.as_bytes()))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        };
        if let (
            Some((TypeExpr::FunctionRef(actual), source)),
            Some((TypeExpr::FunctionRef(wanted), required)),
        ) = (&result, expected)
            && actual.effects != wanted.effects
        {
            let actual_effects = effects.as_ref().expect("function reference effects");
            let required_effects: Vec<_> = wanted
                .effects
                .iter()
                .map(|id| crate::hex::encode(id.as_bytes()))
                .collect();
            return Err(conflict(
                at,
                &format!(
                    "{source} has effect identities {actual_effects:?}, but {required} requires {required_effects:?}"
                ),
            ));
        }
        self.references.insert(json!({"at":at,"source":source,
            "target":items[1].as_str(),"result_type":result.as_ref().map(|(ty,_)|self.cx.render(ty)),
            "effect_ids":effects,"referenced_value":"ordinary_compiler","rewritten":false}));
        Ok(result)
    }

    fn constant_reference(
        &mut self,
        value: &Value,
        at: &str,
        hint: Option<&TypeExpr>,
    ) -> Result<(Option<Known>, &'static str)> {
        if let Some(name) = value.as_str() {
            let ty = self.cx.reference_constant_type(name).ok_or_else(|| {
                conflict(at, &format!("no constant `{name}` in the bound interface"))
            })?;
            return Ok((
                ty.map(|ty| (ty, format!("constant `{name}`"))),
                if self.cx.declared_constant(name) {
                    "declared_constant"
                } else {
                    "accepted_constant"
                },
            ));
        }
        let ty = crate::frame::constants::immediate_type(value, hint, self.cx, at)?;
        let (data, literal_at) = if let Value::Object(object) = value
            && object.contains_key("value")
        {
            (&object["value"], format!("{at}/value"))
        } else {
            (value, at.to_owned())
        };
        Ok((
            self.literal(data, &literal_at, Some(&(ty, at.into())))?,
            "literal_immediate",
        ))
    }

    fn global_reference(&self, name: &str, at: &str) -> Result<(Option<Known>, &'static str)> {
        let global = self
            .cx
            .reference_global(name)
            .ok_or_else(|| conflict(at, &format!("no global `{name}` in the bound interface")))?;
        let initializer = self.cx.names.name(&global.initializer);
        let ty = self
            .cx
            .reference_initializer_type(&global.initializer)
            .ok_or_else(|| {
                conflict(
                    at,
                    &format!("global `{name}` has no bound constant initializer `{initializer}`"),
                )
            })?;
        let Some(ty) = ty else {
            return Ok((None, "global_initializer_deferred"));
        };
        if ty != global.value_type {
            return Err(conflict(
                at,
                &format!(
                    "initializer `{initializer}` has {} but global `{name}` requires {}",
                    self.cx.render(&ty),
                    self.cx.render(&global.value_type)
                ),
            ));
        }
        Ok((
            Some((global.value_type.clone(), format!("global `{name}`"))),
            "accepted_global",
        ))
    }

    fn function_reference(&self, name: &str, at: &str) -> Result<(Option<Known>, &'static str)> {
        if !self.cx.declared_signature(name) && self.cx.reference_deleted(name) {
            return Err(conflict(
                at,
                &format!("function `{name}` is deleted from the bound interface"),
            ));
        }
        let (parameters, result) = super::signatures::read(self.cx, name, at)?
            .ok_or_else(|| conflict(at, &format!("no function `{name}` in the bound interface")))?;
        let declared = self.cx.declared_signature(name);
        if !declared
            && self
                .cx
                .live_function(name)
                .is_some_and(|(_, body)| !body.type_parameters.is_empty())
        {
            return Err(conflict(
                at,
                "function references require a non-generic function",
            ));
        }
        let effects = self.cx.reference_function_effects(name).ok_or_else(|| {
            conflict(
                at,
                &format!("no bound effect interface for function `{name}`"),
            )
        })?;
        Ok((
            Some((
                TypeExpr::FunctionRef(FunctionType {
                    parameters,
                    result: Box::new(result),
                    effects: effects.to_vec(),
                }),
                format!("function `{name}`"),
            )),
            if declared {
                "declared_function"
            } else {
                "accepted_function"
            },
        ))
    }
}
