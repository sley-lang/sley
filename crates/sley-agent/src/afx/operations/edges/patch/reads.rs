//! Retained operands keep accepted identity while consulting the current graph.

use super::{ValueState, value_type};
use crate::afx::operations::edges::Inventory;
use crate::afx::{Context, FnExp};
use crate::error::Result;
use serde_json::json;
use sley_id::EntityId;
use sley_mutate::value::EntityBodyValue;
use sley_ssmc::{ParameterRole, SwitchArgument, Terminator, ValueRef};
use std::collections::BTreeMap;

pub(super) fn check(
    cx: &Context<'_>,
    view: &FnExp<'_, '_>,
    states: &BTreeMap<EntityId, ValueState>,
    retained: &[EntityId],
    report: &mut Inventory,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    for id in retained {
        checkpoint()?;
        let Some(EntityBodyValue::Block(block)) = cx.program.body(id) else {
            continue;
        };
        let owner = cx.names.leaf(id);
        for op_id in &block.operations {
            checkpoint()?;
            let Some(EntityBodyValue::Operation(op)) = cx.program.body(op_id) else {
                continue;
            };
            let at = format!(
                "{} (retained operation `{}`)",
                view.fn_pointer,
                cx.names.name(op_id)
            );
            let mut arguments = Vec::new();
            for (index, value) in op.operands.iter().enumerate() {
                checkpoint()?;
                arguments.push(read(
                    cx,
                    states,
                    *value,
                    &owner,
                    &format!(
                        "{} (retained operation `{}` operand {index})",
                        view.fn_pointer,
                        cx.names.name(op_id)
                    ),
                    report,
                    checkpoint,
                )?);
            }
            let judgment = if let Some(args) = arguments.into_iter().collect::<Option<Vec<_>>>() {
                super::super::operation_types::retained(view, report, op, &at, &args, checkpoint)?
            } else {
                "type_deferred"
            };
            if judgment.ends_with("deferred") {
                report.deferred.push(at.clone());
            }
            report.operations.push(json!({"at":at,"source":"retained_canonical_operation","value_identity":crate::hex::encode(op_id.as_bytes()),"opcode":op.opcode,
                "result_types":op.result_types.iter().map(|ty|cx.render(ty)).collect::<Vec<_>>(),"signature":judgment,"immediate_rebound":false}));
        }
        let at = format!(
            "{} (retained block `{}` terminator)",
            view.fn_pointer,
            cx.names.name(id)
        );
        let mut values = Vec::new();
        let mut edge_values = |args: &[ValueRef]| {
            for (index, value) in args.iter().enumerate() {
                values.push((*value, format!("{at}/arguments/{index}")));
            }
        };
        match &block.terminator {
            Terminator::Return(t) => values.push((t.value, format!("{at}/return"))),
            Terminator::Trap(t) => {
                if let Some(value) = t.payload {
                    values.push((value, format!("{at}/payload")));
                }
            }
            Terminator::Branch(t) => edge_values(&t.edge.arguments),
            Terminator::CondBranch(t) => {
                edge_values(&t.if_true.arguments);
                edge_values(&t.if_false.arguments);
                values.push((t.condition, format!("{at}/condition")));
            }
            Terminator::VariantSwitch(t) => {
                values.push((t.value, at.clone()));
                for case in &t.cases {
                    checkpoint()?;
                    for (index, arg) in case.edge.arguments.iter().enumerate() {
                        if let SwitchArgument::Value(value) = arg {
                            values.push((*value, format!("{at}/arguments/{index}")));
                        }
                    }
                }
            }
        }
        for (value, at) in values {
            checkpoint()?;
            read(cx, states, value, &owner, &at, report, checkpoint)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn read(
    cx: &Context<'_>,
    states: &BTreeMap<EntityId, ValueState>,
    value: ValueRef,
    using: &str,
    at: &str,
    report: &mut Inventory,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Option<sley_ssmc::TypeExpr>> {
    checkpoint()?;
    let ty = value_type(cx, states, value, at, report);
    let (id, index) = match value {
        ValueRef::Parameter(id) => (id, None),
        ValueRef::OperationResult(r) => (r.operation, Some(r.result_index)),
    };
    let known = matches!(states.get(&id), Some(ValueState::Types(_))) && ty.is_some();
    let availability = if known {
        match cx.program.body(&id) {
            Some(EntityBodyValue::Parameter(p)) if p.role == ParameterRole::Function => {
                "function_parameter_checked"
            }
            Some(EntityBodyValue::Parameter(p)) => {
                if cx.names.leaf(&p.owner) == using {
                    "local_parameter_checked"
                } else {
                    report.conflicts.push((
                        at.into(),
                        format!(
                            "retained parameter `{}` is visible only in its own block",
                            cx.names.name(&id)
                        ),
                    ));
                    "conflict"
                }
            }
            Some(EntityBodyValue::Operation(op)) => {
                let owner = cx.names.leaf(&op.block);
                if owner == using {
                    "unchanged_local_order_checked"
                } else {
                    let judgment = if let Some(flow) = report.flow.as_mut() {
                        flow.dominates(&owner, using, checkpoint)?
                    } else {
                        None
                    };
                    match judgment {
                        Some(false) => {
                            report.conflicts.push((at.into(),format!("retained definition `{}` does not dominate this use in block `{using}` under the current entry (the definition or use may be unreachable)",cx.names.name(&id))));
                            "conflict"
                        }
                        Some(true) => "dominance_checked",
                        None => {
                            report.deferred.push(at.into());
                            "availability_deferred"
                        }
                    }
                }
            }
            _ => {
                report.deferred.push(at.into());
                "identity_or_type_deferred"
            }
        }
    } else {
        report.deferred.push(at.into());
        "identity_or_type_deferred"
    };
    report.reads.push(json!({"at":at,"name":cx.names.name(&id),"value_identity":crate::hex::encode(id.as_bytes()),"result_index":index,"scope":"retained_canonical_operand","availability":availability,"type":ty.as_ref().map(|ty|cx.render(ty)),"rewritten":false}));
    checkpoint()?;
    Ok(ty)
}
