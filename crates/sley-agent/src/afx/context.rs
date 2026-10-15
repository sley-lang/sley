//! Cooperative cancellation while constructing declaration interfaces.
//! Ordinary AF1-X supplies an infallible callback; residual preflight shares its
//! invocation budget. Unknown type syntax still produces the same partial view.

use super::{Context, FrameType, Signature, list, placeholder, traits};
use crate::{
    error::Result,
    frame::signatures,
    names::{Names, Scope},
    types,
    workspace::Program,
};
use serde_json::{Map, Value};
use sley_mutate::value::EntityBodyValue;
use sley_ssmc::TypeExpr;
use std::collections::{BTreeMap, BTreeSet};

impl<'a> Context<'a> {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn with_checkpoint(
        program: &'a Program,
        names: &'a Names,
        frame: &Map<String, Value>,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Self> {
        checkpoint()?;
        let mut cx = Self {
            program,
            names,
            types: BTreeMap::new(),
            type_names: BTreeMap::new(),
            consts: BTreeMap::new(),
            constant_values: BTreeMap::new(),
            deleted_references: BTreeSet::new(),
            signatures: BTreeMap::new(),
            signature_issues: BTreeMap::new(),
            top: BTreeSet::new(),
        };
        for value in list(frame, "delete") {
            checkpoint()?;
            let Some(name) = value.as_str() else { continue };
            cx.top.insert(name.to_owned());
            if let Some(id) = names.resolve(name) {
                cx.deleted_references.insert(id);
            }
        }
        for (index, decl) in list(frame, "types").iter().enumerate() {
            checkpoint()?;
            let Some(name) = decl.get("name").and_then(Value::as_str) else {
                continue;
            };
            cx.top.insert(name.to_owned());
            let id = names
                .resolve(name)
                .filter(|id| {
                    !cx.deleted_references.contains(id)
                        && names.scope(id) == Scope::Top
                        && matches!(program.body(id), Some(EntityBodyValue::TypeDef(_)))
                })
                .unwrap_or_else(|| placeholder(index));
            let unique = !cx.types.contains_key(name);
            cx.types.insert(
                name.to_owned(),
                FrameType {
                    id,
                    variant: decl.get("variant").is_some(),
                    members: Vec::new(),
                    trait_shape_complete: unique,
                },
            );
            cx.type_names.insert(id, name.to_owned());
        }
        let mut members = Vec::new();
        for decl in list(frame, "types") {
            checkpoint()?;
            let Some(name) = decl.get("name").and_then(Value::as_str) else {
                continue;
            };
            let cases = decl
                .get("variant")
                .or_else(|| decl.get("record"))
                .and_then(Value::as_array);
            let mut read = Vec::new();
            for case in cases.into_iter().flatten() {
                checkpoint()?;
                match case {
                    Value::String(leaf) => read.push((leaf.clone(), None)),
                    Value::Array(pair) if pair.len() == 2 => {
                        if let Some(name) = pair[0].as_str() {
                            let ty = if pair[1].is_null() {
                                None
                            } else {
                                types::read(&pair[1], &cx, "").ok()
                            };
                            read.push((name.to_owned(), ty));
                        }
                    }
                    _ => {}
                }
            }
            let complete = traits::shape_complete(decl, &read);
            members.push((name.to_owned(), read, complete));
        }
        for (name, read, complete) in members {
            checkpoint()?;
            if let Some(declared) = cx.types.get_mut(&name) {
                declared.members = read;
                declared.trait_shape_complete &= complete;
            }
        }
        for (index, decl) in list(frame, "consts").iter().enumerate() {
            checkpoint()?;
            let Some(name) = decl.get("name").and_then(Value::as_str) else {
                continue;
            };
            let ty = types::read(decl.get("type").unwrap_or(&Value::from("i64")), &cx, "").ok();
            cx.top.insert(name.to_owned());
            cx.consts.insert(name.to_owned(), ty);
            cx.constant_values.insert(
                name.to_owned(),
                (decl.get("value").cloned(), format!("/consts/{index}/value")),
            );
        }
        for key in ["fns", "functions"] {
            for (index, decl) in list(frame, key).iter().enumerate() {
                checkpoint()?;
                let Some(name) = decl
                    .get("fn")
                    .or_else(|| decl.get("name"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                let params = read_params_with_checkpoint(decl.get("params"), &cx, checkpoint)?
                    .map(|params| params.into_iter().map(|(_, ty, _)| ty).collect())
                    .unwrap_or_default();
                let result = decl
                    .get("returns")
                    .and_then(|ty| types::read(ty, &cx, "").ok());
                cx.top.insert(name.to_owned());
                cx.signatures.insert(name.to_owned(), (params, result));
                cx.check_signature_declaration(
                    name,
                    decl,
                    &format!("/{key}/{index}"),
                    true,
                    checkpoint,
                )?;
            }
        }
        for (index, decl) in list(frame, "patch").iter().enumerate() {
            checkpoint()?;
            let Some(name) = decl
                .get("fn")
                .or_else(|| decl.get("name"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            let Some((live_params, live_result)) =
                cx.signature_with_checkpoint(name, checkpoint)?
            else {
                continue;
            };
            let params = match decl.get("params") {
                Some(value) => read_params_with_checkpoint(Some(value), &cx, checkpoint)?
                    .map(|params| params.into_iter().map(|(_, ty, _)| ty).collect())
                    .unwrap_or_default(),
                None => live_params,
            };
            let result = match decl.get("returns") {
                Some(ty) => types::read(ty, &cx, "").ok(),
                None => live_result,
            };
            cx.signatures.insert(name.to_owned(), (params, result));
            cx.check_signature_declaration(
                name,
                decl,
                &format!("/patch/{index}"),
                false,
                checkpoint,
            )?;
        }
        for decl in list(frame, "tests") {
            checkpoint()?;
            if let Some(name) = decl.get("name").and_then(Value::as_str) {
                cx.top.insert(name.to_owned());
            }
        }
        // Ordinary operation edits restate their owner through define_function,
        // which emits the same empty effect declaration as a full definition.
        // Their parameter/result interface is inherited unless already stated.
        for decl in list(frame, "edit") {
            checkpoint()?;
            let Some(name) = decl
                .get("fn")
                .or_else(|| decl.get("function"))
                .or_else(|| decl.get("name"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            if let Some(signature) = cx.signature_with_checkpoint(name, checkpoint)? {
                cx.signatures.entry(name.to_owned()).or_insert(signature);
            }
        }
        checkpoint()?;
        Ok(cx)
    }

    fn check_signature_declaration(
        &mut self,
        name: &str,
        declaration: &Value,
        pointer: &str,
        full: bool,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        checkpoint()?;
        // Syntax/type errors belong to the bound declaration. Cancellation must
        // still abort context construction rather than becoming deferred data.
        let result = signatures::read_params(
            declaration.get("params"),
            &format!("{pointer}/params"),
            checkpoint,
            &mut |value, at| types::read(value, self, at),
        )
        .and_then(|_| {
            if full || declaration.get("returns").is_some() {
                signatures::read_result(
                    declaration.as_object().expect("named declaration object"),
                    pointer,
                    &mut |value, at| types::read(value, self, at),
                )
                .map_err(signatures::ParameterError::Declaration)?;
            }
            Ok(())
        });
        match result {
            Err(signatures::ParameterError::Checkpoint(error)) => return Err(error),
            Err(signatures::ParameterError::Declaration(error)) => {
                self.signature_issues
                    .entry(name.to_owned())
                    .or_insert(error);
            }
            Ok(()) => {}
        }
        Ok(())
    }

    fn signature_with_checkpoint(
        &self,
        name: &str,
        checkpoint: &mut impl FnMut() -> Result<()>,
    ) -> Result<Option<Signature>> {
        let Some((_, body)) = self.live_function(name) else {
            return Ok(None);
        };
        let mut params = Vec::new();
        for param in &body.parameters {
            checkpoint()?;
            params.push(self.parameter_type(param));
        }
        Ok(Some((params, Some(body.result_type.clone()))))
    }
}

type Parameters = Vec<(String, Option<TypeExpr>, Value)>;

pub(super) fn read_params_with_checkpoint(
    value: Option<&Value>,
    cx: &Context<'_>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Option<Parameters>> {
    let Some(value) = value else {
        return Ok(Some(Vec::new()));
    };
    let Some(params) = value.as_array() else {
        return Ok(None);
    };
    let mut out = Vec::new();
    for param in params {
        checkpoint()?;
        let Some(pair) = param.as_array().filter(|pair| pair.len() == 2) else {
            return Ok(None);
        };
        let Some(name) = pair[0].as_str() else {
            return Ok(None);
        };
        out.push((
            name.to_owned(),
            types::read(&pair[1], cx, "").ok(),
            pair[1].clone(),
        ));
    }
    Ok(Some(out))
}
