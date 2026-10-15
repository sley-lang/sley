//! Type eligibility for value hashing; the value and digest are never evaluated.

use super::{AgentError, AgentErrorCode, Check, Known, Result, TypeExpr, Value, conflict, json};
use sley_check::TypeErrorCode;

impl Check<'_, '_> {
    pub(super) fn value_hash(
        &mut self,
        items: &[Value],
        at: &str,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        if items.len() != 2 {
            return Err(conflict(at, "value hash requires exactly one operand"));
        }
        // A bytes result does not constrain the input's type or integer width.
        let operand = self.expression(&items[1], &pointers[1], None, depth + 1)?;
        let eligibility = match &operand {
            Some((ty, source)) => self.hash_eligibility(ty, source, at)?,
            None => "unresolved",
        };
        if eligibility == "checked" {
            self.deferred.remove(at);
        } else {
            self.deferred.insert(at.into());
        }
        self.hashes.insert(json!({"at":at,
            "operand_type":operand.as_ref().map(|(ty,_)|self.cx.render(ty)),
            "eligibility":eligibility,"result_type":"bytes",
            "evaluated":false,"rewritten":false}));
        Ok(Some((TypeExpr::Bytes, at.into())))
    }

    fn hash_eligibility(&mut self, ty: &TypeExpr, source: &str, at: &str) -> Result<&'static str> {
        // This is the VM's opcode-wide local-cell exclusion, including cells
        // nested in function-reference signatures. Persistable/hashable traits
        // alone do not encode that structural restriction.
        if sley_vm::extended::contains_cell(ty) {
            return Err(conflict(
                at,
                "value hash cannot consume an execution-local cell",
            ));
        }
        match super::type_traits::check(self.cx, ty, self.budget, |types| {
            types.require_hashable(ty)
        })? {
            Some(Ok(())) => Ok("checked"),
            None => Ok("definition_deferred"),
            Some(Err(error)) => Err(AgentError::new(
                if matches!(
                    error.code(),
                    TypeErrorCode::ResourceLimit | TypeErrorCode::DepthLimit
                ) {
                    AgentErrorCode::ResidualLimit
                } else {
                    AgentErrorCode::ResidualConstraintConflict
                },
                format!(
                    "{at}: hash operand {source} ({}): {error}",
                    self.cx.render(ty)
                ),
            )),
        }
    }
}
