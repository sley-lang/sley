//! Ordered-map type connections without sorting, evaluating or folding entries.

use super::{AgentError, AgentErrorCode, Check, Known, Result, Value, conflict};
use serde_json::json;
use sley_check::TypeErrorCode;
use sley_ssmc::{BuiltinFailureKind, TypeExpr};

type Parts = [Option<Known>; 2];

pub(super) fn wrapped(map: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(map),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    }
}

fn map_type(parts: &Parts, at: &str) -> Option<Known> {
    Some((
        TypeExpr::OrderedMap {
            key: Box::new(parts[0].as_ref()?.0.clone()),
            value: Box::new(parts[1].as_ref()?.0.clone()),
        },
        at.into(),
    ))
}

impl Check<'_, '_> {
    pub(super) fn map_new(
        &mut self,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        if !(items.len() - 1).is_multiple_of(2) {
            return Err(conflict(
                at,
                "map construction requires alternating key/value pairs",
            ));
        }
        let emitted = (items.len() == 1).then(|| self.emitted_hint(at)).flatten();
        let expected = emitted.as_ref().or(expected);
        let mut parts = match expected {
            Some((TypeExpr::Result { ok, error }, source))
                if **error == TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey) =>
            {
                self.map_parts(&((**ok).clone(), source.clone()), at)?
            }
            Some((ty, source)) => {
                return Err(conflict(
                    at,
                    &format!(
                        "map construction produces Result<Map<key,value>,DuplicateKeyError>, but {source} requires {}",
                        self.cx.render(ty)
                    ),
                ));
            }
            None => [None, None],
        };
        let mut unresolved = Vec::new();
        for (index, value) in items.iter().enumerate().skip(1) {
            let slot = (index - 1) % 2;
            let actual =
                self.expression(value, &pointers[index], parts[slot].as_ref(), depth + 1)?;
            if actual.is_none() {
                unresolved.push(index);
            }
            if parts[slot].is_none() {
                parts[slot] = actual;
            }
        }
        for index in unresolved {
            if let Some(context) = &parts[(index - 1) % 2] {
                self.expression(&items[index], &pointers[index], Some(context), depth + 1)?;
            }
        }
        let result = map_type(&parts, at).map(|(ty, source)| (wrapped(ty), source));
        self.map_evidence(&parts, at, &pointers[0])?;
        Ok(self.map_result(result, at))
    }

    pub(super) fn map_access(
        &mut self,
        tag: u32,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        let arity = if tag == 39 { 3 } else { 2 };
        if items.len() != arity + 1 {
            return Err(conflict(at, "wrong map operation operand count"));
        }
        let mut parts = match (tag, expected) {
            (39 | 40, Some(known)) => self.map_parts(known, at)?,
            (37, Some((TypeExpr::Option(value), source))) => {
                [None, Some(((**value).clone(), source.clone()))]
            }
            (37, Some((ty, source))) => {
                return Err(conflict(
                    at,
                    &format!(
                        "map_get produces Option<value>, but {source} requires {}",
                        self.cx.render(ty)
                    ),
                ));
            }
            _ => [None, None],
        };
        let context = map_type(&parts, at);
        let source = self.expression(&items[1], &pointers[1], context.as_ref(), depth + 1)?;
        if let Some(source) = &source {
            parts = self.map_parts(source, &pointers[1])?;
        }
        for index in 2..items.len() {
            let slot = index - 2;
            let actual = self.expression(
                &items[index],
                &pointers[index],
                parts[slot].as_ref(),
                depth + 1,
            )?;
            if parts[slot].is_none() {
                parts[slot] = actual;
            }
        }
        if source.is_none()
            && let Some(context) = map_type(&parts, at)
        {
            self.expression(&items[1], &pointers[1], Some(&context), depth + 1)?;
        }
        let result = match tag {
            37 => parts[1]
                .as_ref()
                .map(|(ty, _)| (TypeExpr::Option(Box::new(ty.clone())), at.into())),
            38 => Some((TypeExpr::Bool, at.into())),
            39 | 40 => map_type(&parts, at),
            _ => unreachable!("map access opcode"),
        };
        self.map_evidence(&parts, at, &pointers[0])?;
        Ok(self.map_result(result, at))
    }

    fn map_parts(&self, known: &Known, at: &str) -> Result<Parts> {
        let (TypeExpr::OrderedMap { key, value }, source) = known else {
            return Err(conflict(
                at,
                &format!(
                    "map operand required, but {} has {}",
                    known.1,
                    self.cx.render(&known.0)
                ),
            ));
        };
        Ok([
            Some(((**key).clone(), format!("{source} (map key)"))),
            Some(((**value).clone(), format!("{source} (map value)"))),
        ])
    }

    fn map_evidence(&mut self, parts: &Parts, at: &str, key_at: &str) -> Result<()> {
        self.budget.checkpoint()?;
        let status = if let Some((key, source)) = &parts[0] {
            match super::type_traits::check(self.cx, key, self.budget, |types| {
                types.check_closed_type(&TypeExpr::OrderedMap {
                    key: Box::new(key.clone()),
                    value: Box::new(TypeExpr::Unit),
                })
            })? {
                Some(Ok(())) => "checked",
                None => "definition_deferred",
                Some(Err(error))
                    if matches!(
                        error.code(),
                        TypeErrorCode::ResourceLimit | TypeErrorCode::DepthLimit
                    ) =>
                {
                    return Err(AgentError::new(
                        AgentErrorCode::ResidualLimit,
                        format!("{key_at}: map key type check: {error}"),
                    ));
                }
                Some(Err(error)) => {
                    return Err(conflict(
                        key_at,
                        &format!(
                            "map key {source} ({}) is inadmissible: {error}",
                            self.cx.render(key)
                        ),
                    ));
                }
            }
        } else {
            "unresolved"
        };
        if status == "checked" {
            self.deferred.remove(key_at);
        } else {
            self.deferred.insert(key_at.into());
        }
        self.maps.insert(
            json!({"at":at,"key_type":parts[0].as_ref().map(|(ty,_)|self.cx.render(ty)),
            "value_type":parts[1].as_ref().map(|(ty,_)|self.cx.render(ty)),"key_traits":status,
            "entries":"not_evaluated","rewritten":false}),
        );
        Ok(())
    }

    fn map_result(&mut self, result: Option<Known>, at: &str) -> Option<Known> {
        if result.is_some() {
            self.deferred.remove(at);
        } else {
            self.deferred.insert(at.into());
        }
        result
    }
}
