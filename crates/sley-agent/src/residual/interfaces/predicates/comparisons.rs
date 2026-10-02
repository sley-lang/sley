//! Comparison eligibility without evaluating or rewriting either operand.

use super::{AgentError, AgentErrorCode, Check, Known, Result, TypeExpr, conflict, json};
use sley_check::TypeErrorCode;
use sley_ssmc::{Immediate, Opcode};
use sley_vm::extended::{LoweringContext, judge_extended_operation};

impl Check<'_, '_> {
    pub(super) fn comparison(
        &mut self,
        tag: u32,
        left: Option<&Known>,
        right: Option<&Known>,
        at: &str,
    ) -> Result<()> {
        self.budget.checkpoint()?;
        let known = left.or(right);
        let status = if let Some((ty, source)) = known {
            if tag >= 98 {
                // Map-key total ordering admits composites; these VM operations
                // admit only scalars. No named-definition lookup is needed.
                if !matches!(
                    ty,
                    TypeExpr::Bool
                        | TypeExpr::SInt(_)
                        | TypeExpr::UInt(_)
                        | TypeExpr::Bytes
                        | TypeExpr::Text
                        | TypeExpr::F32
                        | TypeExpr::F64
                ) {
                    return Err(conflict(
                        at,
                        &format!(
                            "{source} has {}; ordered comparison requires a scalar operand",
                            self.cx.render(ty)
                        ),
                    ));
                }
                "checked"
            } else {
                let result = super::type_traits::check(self.cx, ty, self.budget, |types| {
                    // Preserve canonical type/resource diagnostics before the VM
                    // judgment collapses invalid traits into SignatureMismatch.
                    types.require_hashable(ty)?;
                    // Equality reads only types. These inventories and the
                    // sentinel id are unused by the Equal/NotEqual judgment.
                    let context = LoweringContext {
                        types,
                        constants: &[],
                        globals: &[],
                        functions: &[],
                        parameters: &[],
                        contracts: &[],
                        adapters: &[],
                        function: sley_id::EntityId::from_bytes([0; 32]),
                    };
                    Ok(judge_extended_operation(
                        &context,
                        Opcode::from_tag(tag).expect("comparison opcode"),
                        &Immediate::None,
                        &[ty, ty],
                        &[TypeExpr::Bool],
                    )
                    .is_ok())
                })?;
                match result {
                    Some(Ok(true)) => "checked",
                    None => "definition_deferred",
                    Some(Ok(false)) => {
                        return Err(conflict(
                            at,
                            &format!(
                                "{source} has {}; equality is unsupported by the VM comparison profile",
                                self.cx.render(ty)
                            ),
                        ));
                    }
                    Some(Err(error)) => {
                        let detail = format!(
                            "{at}: comparison operand {source} ({}): {error}",
                            self.cx.render(ty)
                        );
                        return Err(AgentError::new(
                            if matches!(
                                error.code(),
                                TypeErrorCode::ResourceLimit | TypeErrorCode::DepthLimit
                            ) {
                                AgentErrorCode::ResidualLimit
                            } else {
                                AgentErrorCode::ResidualConstraintConflict
                            },
                            detail,
                        ));
                    }
                }
            }
        } else {
            "unresolved"
        };
        if status == "checked" {
            self.deferred.remove(at);
        } else {
            self.deferred.insert(at.into());
        }
        self.comparisons.insert(json!({"at":at,
            "operand_type":known.map(|(ty,_)|self.cx.render(ty)),
            "eligibility":status,
            "operands":if left.is_some() && right.is_some() {"checked"} else {"deferred"},
            "evaluated":false,"rewritten":false}));
        Ok(())
    }
}
