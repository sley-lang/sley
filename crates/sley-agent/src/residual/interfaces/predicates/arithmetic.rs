//! Integer arithmetic type connections; never evaluate or simplify operands.

use super::{Check, Known, Result, Value, conflict};
use sley_ssmc::{BuiltinFailureKind, IntegerWidth, TypeExpr};

pub(super) fn wrapped(payload: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(payload),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    }
}

impl Check<'_, '_> {
    pub(super) fn arithmetic(
        &mut self,
        tag: u32,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        let arity = if tag == 69 { 1 } else { 2 };
        if items.len() != arity + 1 {
            return Err(conflict(at, "wrong integer arithmetic operand count"));
        }
        let payload = match expected {
            Some((TypeExpr::Result { ok, error }, source))
                if **error == TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic) =>
            {
                Some(((**ok).clone(), source.clone()))
            }
            Some((ty, source)) => {
                return Err(conflict(
                    at,
                    &format!(
                        "integer arithmetic produces Result<integer,ArithmeticError>, but {source} requires {}",
                        self.cx.render(ty)
                    ),
                ));
            }
            None => None,
        };
        if let Some(payload) = &payload {
            self.integer(payload, tag, at)?;
        }
        let mut left = self.expression(&items[1], &pointers[1], payload.as_ref(), depth + 1)?;
        if let Some(left) = &left {
            self.integer(left, tag, &pointers[1])?;
        }
        let mut right = None;
        if arity == 2 {
            let shift = (
                TypeExpr::UInt(IntegerWidth::from_bits(32)),
                format!("{} (shift count)", pointers[0]),
            );
            let expected = if tag >= 70 {
                Some(&shift)
            } else {
                left.as_ref().or(payload.as_ref())
            };
            right = self.expression(&items[2], &pointers[2], expected, depth + 1)?;
            if tag < 70
                && let Some(right) = &right
            {
                self.integer(right, tag, &pointers[2])?;
                if left.is_none() {
                    left = self.expression(&items[1], &pointers[1], Some(right), depth + 1)?;
                }
            }
        }
        let payload = left
            .or_else(|| (tag < 70).then_some(right).flatten())
            .or(payload);
        if let Some((payload, _)) = payload {
            self.deferred.remove(at);
            Ok(Some((wrapped(payload), at.into())))
        } else {
            self.deferred.insert(at.into());
            Ok(None)
        }
    }

    fn integer(&self, known: &Known, tag: u32, at: &str) -> Result<()> {
        let valid = matches!(known.0, TypeExpr::SInt(_))
            || (tag != 69 && matches!(known.0, TypeExpr::UInt(_)));
        if !valid {
            return Err(conflict(
                at,
                &format!(
                    "{} has {}, but {} requires {}",
                    known.1,
                    self.cx.render(&known.0),
                    if tag == 69 {
                        "negation"
                    } else {
                        "integer arithmetic"
                    },
                    if tag == 69 {
                        "a signed integer"
                    } else {
                        "an integer"
                    }
                ),
            ));
        }
        Ok(())
    }
}
