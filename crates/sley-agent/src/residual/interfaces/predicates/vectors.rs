//! Homogeneous vector connections and native Option/IndexError results.

use super::{Check, Known, Result, Value, conflict};
use sley_ssmc::{BuiltinFailureKind, IntegerWidth, TypeExpr};

pub(super) fn wrapped(vector: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(vector),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
    }
}

impl Check<'_, '_> {
    pub(super) fn vector_new(
        &mut self,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        // Ordinary infer uses a direct use hint only when no element fixes
        // the result type. Nonempty vectors keep their operand-derived type.
        let emitted = (items.len() == 1).then(|| self.emitted_hint(at)).flatten();
        let expected = emitted.as_ref().or(expected);
        let mut element = expected
            .map(|known| self.vector_element(known, at))
            .transpose()?;
        let mut unresolved = Vec::new();
        for (index, value) in items.iter().enumerate().skip(1) {
            let actual = self.expression(value, &pointers[index], element.as_ref(), depth + 1)?;
            if actual.is_none() {
                unresolved.push(index);
            }
            if element.is_none() {
                element = actual;
            }
        }
        // A later element can constrain earlier literals/nested expressions.
        // Revisit their evidence without changing authored order or source.
        if let Some(element) = &element {
            for index in unresolved {
                self.expression(&items[index], &pointers[index], Some(element), depth + 1)?;
            }
            self.deferred.remove(at);
            Ok(Some((
                TypeExpr::Vector(Box::new(element.0.clone())),
                at.into(),
            )))
        } else {
            self.deferred.insert(at.into());
            Ok(None)
        }
    }

    pub(super) fn vector_access(
        &mut self,
        tag: u32,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        let arity = (tag - 32) as usize;
        if items.len() != arity + 1 {
            return Err(conflict(at, "wrong vector operation operand count"));
        }
        let context = match (tag, expected) {
            (34, Some((TypeExpr::Option(item), source))) => {
                Some((TypeExpr::Vector(item.clone()), source.clone()))
            }
            (35, Some((TypeExpr::Result { ok, error }, source)))
                if **error == TypeExpr::BuiltinFailure(BuiltinFailureKind::Index) =>
            {
                let vector = ((**ok).clone(), source.clone());
                self.vector_element(&vector, at)?;
                Some(vector)
            }
            (34 | 35, Some((ty, source))) => {
                return Err(conflict(
                    at,
                    &format!(
                        "{} produces {}, but {source} requires {}",
                        if tag == 34 { "vec_get" } else { "vec_set" },
                        if tag == 34 {
                            "Option<element>"
                        } else {
                            "Result<Vec<element>,IndexError>"
                        },
                        self.cx.render(ty)
                    ),
                ));
            }
            _ => None,
        };
        let mut vector = self
            .expression(&items[1], &pointers[1], context.as_ref(), depth + 1)?
            .or(context);
        let mut element = vector
            .as_ref()
            .map(|known| self.vector_element(known, &pointers[1]))
            .transpose()?;
        if tag >= 34 {
            let index_type = (
                TypeExpr::UInt(IntegerWidth::from_bits(64)),
                format!("{at} (vector index)"),
            );
            self.expression(&items[2], &pointers[2], Some(&index_type), depth + 1)?;
        }
        if tag == 35 {
            let replacement =
                self.expression(&items[3], &pointers[3], element.as_ref(), depth + 1)?;
            if vector.is_none()
                && let Some((ty, source)) = replacement
            {
                let context = (TypeExpr::Vector(Box::new(ty)), source);
                vector = self
                    .expression(&items[1], &pointers[1], Some(&context), depth + 1)?
                    .or(Some(context));
                element = vector
                    .as_ref()
                    .map(|known| self.vector_element(known, &pointers[1]))
                    .transpose()?;
            }
        }
        let result = match tag {
            33 => Some(TypeExpr::UInt(IntegerWidth::from_bits(64))),
            34 => element.map(|(ty, _)| TypeExpr::Option(Box::new(ty))),
            35 => vector.map(|(ty, _)| wrapped(ty)),
            _ => unreachable!("vector access opcode"),
        };
        if result.is_some() {
            self.deferred.remove(at);
        } else {
            self.deferred.insert(at.into());
        }
        Ok(result.map(|ty| (ty, at.into())))
    }

    fn vector_element(&self, known: &Known, at: &str) -> Result<Known> {
        let (TypeExpr::Vector(item), source) = known else {
            return Err(conflict(
                at,
                &format!(
                    "vector operand required, but {} has {}",
                    known.1,
                    self.cx.render(&known.0)
                ),
            ));
        };
        Ok(((**item).clone(), format!("{source} (vector element)")))
    }
}
