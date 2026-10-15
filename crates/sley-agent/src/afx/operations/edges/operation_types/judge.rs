//! Reuse the VM signature judge with a read-only callee header projection.
use super::{FnExp, Inventory, Node, Result};
use crate::error::{AgentError, AgentErrorCode};
use sley_check::TypeErrorCode;
use sley_id::EntityId;
use sley_ssmc::{FunctionGraph, Opcode, Parameter, ParameterRole, TypeExpr, Visibility};
use sley_vm::extended::{LoweringContext, judge_extended_operation};
mod bindings;
mod named;
mod retained_bindings;

pub(super) fn check(
    view: &FnExp<'_, '_>,
    report: &mut Inventory,
    node: &Node,
    result: &TypeExpr,
    args: &[TypeExpr],
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<&'static str> {
    checkpoint()?;
    let Some(bound) = bindings::Bindings::read(view, node, result, checkpoint)? else {
        return Ok("immediate_deferred");
    };
    judge_bound(
        view,
        report,
        Operation {
            at: &node.pointer,
            tag: node.row.tag,
            results: std::slice::from_ref(result),
            retained: false,
            name: Some(&node.word),
        },
        args,
        bound,
        checkpoint,
    )
}

pub(in crate::afx::operations::edges) fn retained(
    view: &FnExp<'_, '_>,
    report: &mut Inventory,
    op: &sley_mutate::value::OperationBody,
    at: &str,
    args: &[TypeExpr],
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<&'static str> {
    checkpoint()?;
    let bound = match retained_bindings::read(view, op, at, checkpoint) {
        Ok(Some(bound)) => bound,
        Ok(None) => return Ok("immediate_deferred"),
        Err(error) if error.code() == AgentErrorCode::ResidualConstraintConflict => {
            let prefix = format!("{at}: ");
            let detail = error
                .detail()
                .strip_prefix(&prefix)
                .unwrap_or_else(|| error.detail());
            report
                .retained_conflicts
                .push((at.to_owned(), detail.to_owned()));
            return Ok("conflict");
        }
        Err(error) => return Err(error),
    };
    let mut checked = Inventory::default();
    let status = judge_bound(
        view,
        &mut checked,
        Operation {
            at,
            tag: op.opcode,
            results: &op.result_types,
            retained: true,
            name: None,
        },
        args,
        bound,
        checkpoint,
    )?;
    report.retained_conflicts.extend(checked.conflicts);
    Ok(status)
}

#[derive(Clone, Copy)]
struct Operation<'a> {
    at: &'a str,
    tag: u32,
    results: &'a [TypeExpr],
    retained: bool,
    name: Option<&'a str>,
}

fn judge_bound(
    view: &FnExp<'_, '_>,
    report: &mut Inventory,
    operation: Operation<'_>,
    args: &[TypeExpr],
    bound: bindings::Bindings,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<&'static str> {
    let mut roots = args.to_vec();
    roots.extend(bound.roots);
    roots.extend_from_slice(operation.results);
    for parameter in &bound.parameters {
        checkpoint()?;
        roots.push(parameter.value_type.clone());
    }
    for function in &bound.functions {
        checkpoint()?;
        roots.push(function.result_type.clone());
    }
    let Some(types) = view
        .cx
        .type_environment_with_checkpoint(&TypeExpr::Tuple(roots), checkpoint)?
    else {
        if operation.retained {
            report.conflicts.push((operation.at.to_owned(),"retained operation's bound type definitions are unavailable or incomplete under the current source".into()));
            return Ok("conflict");
        }
        return Ok("definition_deferred");
    };
    let types = match types {
        Ok(types) => types,
        Err(error) => {
            if matches!(
                error.code(),
                TypeErrorCode::ResourceLimit | TypeErrorCode::DepthLimit
            ) {
                return Err(AgentError::new(
                    AgentErrorCode::ResidualLimit,
                    format!("{}: source operation types: {error}", operation.at),
                ));
            }
            report.conflicts.push((
                operation.at.to_owned(),
                format!("source operation type environment: {error}"),
            ));
            return Ok("conflict");
        }
    };
    let context = LoweringContext {
        types: &types,
        constants: &bound.constants,
        globals: &bound.globals,
        functions: &bound.functions,
        parameters: &bound.parameters,
        contracts: &[],
        adapters: &[],
        function: EntityId::from_bytes([0; 32]),
    };
    let Some(opcode) = Opcode::from_tag(operation.tag) else {
        return Ok("opcode_deferred");
    };
    let operands: Vec<_> = args.iter().collect();
    let judgment = judge_extended_operation(
        &context,
        opcode,
        &bound.immediate,
        &operands,
        operation.results,
    );
    checkpoint()?;
    match judgment {
        Ok(_) => Ok("canonical_vm_signature_checked"),
        Err(error) => {
            let label = operation.name.map_or_else(
                || format!("retained operation opcode {}", operation.tag),
                |name| format!("source operation `{name}`"),
            );
            let operands = type_list(view, args);
            let results = type_list(view, operation.results);
            let callee = bound
                .functions
                .first()
                .map_or_else(String::new, |function| {
                    let parameters: Vec<_> = bound
                        .parameters
                        .iter()
                        .map(|p| p.value_type.clone())
                        .collect();
                    format!(
                        "; bound callee parameters [{}], result [{}]",
                        type_list(view, &parameters),
                        type_list(view, std::slice::from_ref(&function.result_type))
                    )
                });
            report.conflicts.push((operation.at.to_owned(),format!("{error}: {label} operands [{operands}] and declared results [{results}] do not match its canonical VM signature{callee}")));
            Ok("conflict")
        }
    }
}

fn type_list(view: &FnExp<'_, '_>, types: &[TypeExpr]) -> String {
    let mut rendered: Vec<_> = types
        .iter()
        .take(4)
        .map(|ty| {
            let text = view.cx.render(ty);
            if text.chars().count() > 128 {
                format!("{}…", text.chars().take(128).collect::<String>())
            } else {
                text
            }
        })
        .collect();
    if types.len() > 4 {
        rendered.push(format!("… {} further types", types.len() - 4));
    }
    rendered.join(", ")
}

fn signature(
    view: &FnExp<'_, '_>,
    name: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Option<(FunctionGraph, Vec<Parameter>)>> {
    let mut parameters = Vec::new();
    let Some((params, Some(result))) = view.cx.signature(name) else {
        return Ok(None);
    };
    let Some(params) = params.into_iter().collect::<Option<Vec<_>>>() else {
        return Ok(None);
    };
    let Some(effects) = view.cx.reference_function_effects(name) else {
        return Ok(None);
    };
    // These private query keys are never allocated/published entity IDs.
    // The canonical judge reads only the signature fields of this graph.
    let owner = EntityId::from_bytes([0; 32]);
    for (ordinal, ty) in params.into_iter().enumerate() {
        checkpoint()?;
        let ordinal = u32::try_from(ordinal).map_err(|_| {
            AgentError::new(AgentErrorCode::ResidualLimit, "callee parameter count")
        })?;
        let mut key = [1; 32];
        key[..4].copy_from_slice(&ordinal.to_le_bytes());
        parameters.push(Parameter {
            entity_id: EntityId::from_bytes(key),
            owner,
            role: ParameterRole::Function,
            ordinal,
            value_type: ty,
        });
    }
    let function = FunctionGraph {
        entity_id: owner,
        // Ordinary definitions/patches reset this header; untouched accepted
        // callees retain it, including unused generic parameters.
        type_parameters: if view.cx.declared_signature(name) {
            vec![]
        } else {
            view.cx
                .live_function(name)
                .map_or_else(Vec::new, |(_, body)| body.type_parameters.clone())
        },
        parameters: parameters.iter().map(|p| p.entity_id).collect(),
        result_type: result,
        effects: effects.to_vec(),
        entry_block: owner,
        blocks: vec![],
        contracts: vec![],
        visibility: Visibility::Private,
    };
    Ok(Some((function, parameters)))
}
