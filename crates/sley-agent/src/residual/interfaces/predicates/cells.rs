//! Local-cell type connections; no runtime cells are allocated, read or written.

use super::{AgentError, AgentErrorCode, Check, Known, Result, TypeExpr, Value, conflict, json};
use sley_check::TypeErrorCode;

impl Check<'_, '_> {
    pub(super) fn cell_operand(&mut self, actual: Option<&Known>, at: &str) -> Result<()> {
        let Some((tag, operation_at)) = &self.operand_operation else {
            return Ok(());
        };
        self.budget.checkpoint()?;
        let exempt = matches!(tag, 177 | 178);
        if !exempt
            && let Some((ty, source)) = actual
            && sley_vm::extended::contains_cell(ty)
        {
            return Err(conflict(
                at,
                &format!(
                    "operand {source} contains a local cell; operation {tag} at {operation_at} permits no local-cell operands under the VM profile"
                ),
            ));
        }
        self.cell_operands.insert(json!({"at":at,"operation":operation_at,"opcode":tag,
            "local_cell_operand":if exempt {"cell_operation_exempt"} else if actual.is_some() {"absent_checked"} else {"type_deferred"},
            "type":actual.map(|(ty,_)|self.cx.render(ty)),"rule":"vm_structural_operand_check"}));
        Ok(())
    }

    pub(super) fn cell_operation(
        &mut self,
        tag: u32,
        items: &[Value],
        at: &str,
        expected: Option<&Known>,
        depth: usize,
        pointers: &[String],
    ) -> Result<Option<Known>> {
        if items.len() != if tag == 178 { 3 } else { 2 } {
            return Err(conflict(at, "wrong cell operation operand count"));
        }
        let context = if tag == 176 {
            expected
                .map(|known| self.cell_element(known, at))
                .transpose()?
        } else {
            None
        };
        let first = self.expression(&items[1], &pointers[1], context.as_ref(), depth + 1)?;
        let element = if tag == 176 {
            first
        } else {
            first
                .as_ref()
                .map(|known| self.cell_element(known, &pointers[1]))
                .transpose()?
        };
        if tag == 178 {
            self.expression(&items[2], &pointers[2], element.as_ref(), depth + 1)?;
        }
        let eligibility = if tag == 176 {
            match &element {
                Some((ty, source)) => self.cell_storage(ty, source, at)?,
                None => "unresolved",
            }
        } else {
            "not_applicable"
        };
        let ty = match tag {
            176 => element
                .as_ref()
                .map(|(ty, _)| TypeExpr::LocalCell(Box::new(ty.clone()))),
            177 => element.as_ref().map(|(ty, _)| ty.clone()),
            178 => Some(TypeExpr::Unit),
            _ => unreachable!("cell opcode"),
        };
        if ty.is_none() || eligibility == "definition_deferred" || element.is_none() {
            self.deferred.insert(at.into());
        } else {
            self.deferred.remove(at);
        }
        self.cells.insert(json!({"at":at,"opcode":tag,
            "element_type":element.as_ref().map(|(ty,_)|self.cx.render(ty)),
            "storage_eligibility":eligibility,"result_type":ty.as_ref().map(|ty|self.cx.render(ty)),
            "ownership":"ordinary_compiler","evaluated":false,"rewritten":false}));
        Ok(ty.map(|ty| (ty, at.into())))
    }

    fn cell_element(&self, known: &Known, at: &str) -> Result<Known> {
        let (TypeExpr::LocalCell(item), source) = known else {
            return Err(conflict(
                at,
                &format!(
                    "cell type required, but {} has {}",
                    known.1,
                    self.cx.render(&known.0)
                ),
            ));
        };
        Ok(((**item).clone(), format!("{source} (cell element)")))
    }

    fn cell_storage(&mut self, ty: &TypeExpr, source: &str, at: &str) -> Result<&'static str> {
        // Match the VM's structural local-cell exclusion, including function
        // signatures, before the canonical persistability judgment.
        if sley_vm::extended::contains_cell(ty) {
            return Err(conflict(
                at,
                "cell creation cannot store a value containing a local cell",
            ));
        }
        match super::type_traits::check(self.cx, ty, self.budget, |types| types.traits(ty))? {
            Some(Ok(traits)) if traits.persistable => Ok("persistable_checked"),
            None => Ok("definition_deferred"),
            Some(Ok(_)) => Err(conflict(
                at,
                &format!(
                    "cell initializer {source} ({}) is not persistable",
                    self.cx.render(ty)
                ),
            )),
            Some(Err(error)) => Err(AgentError::new(
                if matches!(
                    error.code(),
                    TypeErrorCode::ResourceLimit | TypeErrorCode::DepthLimit
                ) {
                    AgentErrorCode::ResidualLimit
                } else {
                    AgentErrorCode::ResidualConstraintConflict
                },
                format!("{at}: cell initializer {source}: {error}"),
            )),
        }
    }
}
