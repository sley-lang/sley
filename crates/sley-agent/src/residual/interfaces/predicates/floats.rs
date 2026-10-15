//! Floating-point connections. Never calculate, round or reassociate operations.

use super::{Check, Known, Result, Value, conflict};
use sley_ssmc::TypeExpr;

impl Check<'_, '_> {
    pub(super) fn float_arithmetic(
        &mut self,
        tag: u32,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        let arity = match tag {
            84 => 1,
            85 => 3,
            _ => 2,
        };
        if items.len() != arity + 1 {
            return Err(conflict(
                at,
                "wrong floating-point arithmetic operand count",
            ));
        }
        let mut context = expected.cloned();
        if let Some(known) = &context {
            self.float_type(known, at)?;
        }
        let mut unresolved = Vec::new();
        for (index, value) in items.iter().enumerate().skip(1) {
            let actual = self.expression(value, &pointers[index], context.as_ref(), depth + 1)?;
            if let Some(known) = &actual {
                self.float_type(known, &pointers[index])?;
            } else {
                unresolved.push(index);
            }
            if context.is_none() {
                context = actual;
            }
        }
        if let Some(known) = &context {
            for index in unresolved {
                self.expression(&items[index], &pointers[index], Some(known), depth + 1)?;
            }
            self.deferred.remove(at);
            Ok(Some((known.0.clone(), at.into())))
        } else {
            self.deferred.insert(at.into());
            Ok(None)
        }
    }

    fn float_type(&self, known: &Known, at: &str) -> Result<()> {
        if !matches!(known.0, TypeExpr::F32 | TypeExpr::F64) {
            return Err(conflict(
                at,
                &format!(
                    "floating-point arithmetic requires f32 or f64, but {} has {}",
                    known.1,
                    self.cx.render(&known.0)
                ),
            ));
        }
        Ok(())
    }
}
