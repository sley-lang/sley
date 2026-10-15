//! A derived function is one conservative dependency component. Shared input,
//! result, binding, order and failure interfaces never justify splitting it.

use super::{Budget, Names, Program, Request, Value, encode, error, factors, json};
use crate::error::{AgentErrorCode, Result};
use crate::residual::{BaseRef, binding, interfaces, plan};

pub(super) fn analyze(
    program: &Program,
    names: &Names,
    request: &Request,
    template: &Value,
    declaration: &Value,
    budget: &mut Budget,
    bound_draft: bool,
) -> Result<(factors::Problem, factors::FactoredFrontier, Value)> {
    budget.checkpoint()?;
    if !request.scope.as_array().is_some_and(|scope| {
        scope.len() == 1
            && scope[0]
                .as_str()
                .is_some_and(|name| crate::names::is_identifier(name) && !name.contains("__"))
    }) {
        return Err(error(
            AgentErrorCode::ResidualScope,
            "fragment dependencies require exactly one AF1-X target function",
        ));
    }
    if matches!(request.base, BaseRef::Draft(_)) != bound_draft {
        return Err(error(
            AgentErrorCode::ResidualBindingStale,
            "fragment dependency source route differs from the bound base",
        ));
    }
    if let BaseRef::AcceptedRoot(root) = &request.base
        && root != program.root().as_bytes()
    {
        return Err(error(
            AgentErrorCode::ResidualBindingStale,
            "fragment dependency root differs from the bound base",
        ));
    }
    // Validate the author's original edges and exact field coverage before
    // adding mandatory coupling; this must not repair an invalid declaration.
    factors::Problem::parse_with_budget(&encode(declaration)?, budget)?;
    let fields: Vec<_> = declaration["fields"]
        .as_array()
        .expect("validated")
        .iter()
        .map(|field| field["name"].clone())
        .collect();
    let mut coupled = declaration.clone();
    let has_coupling = coupled["dependencies"]
        .as_array()
        .expect("validated")
        .iter()
        .any(|edge| {
            edge["kind"] == "binding"
                && edge["fields"].as_array().is_some_and(|scope| {
                    scope.len() == fields.len() && fields.iter().all(|field| scope.contains(field))
                })
        });
    if fields.len() > 1 && !has_coupling {
        coupled["dependencies"]
            .as_array_mut()
            .expect("validated")
            .push(json!({"kind":"binding","fields":fields}));
    }
    let context = context(program, names, request, budget)?;
    let mut problem = factors::Problem::parse_with_budget(&encode(&coupled)?, budget)?;
    problem.bind_context(&context)?;
    let frontier = factors::plan(&problem, budget, false)?;
    let rows = frontier.coupled_descriptions().ok_or_else(|| {
        error(
            AgentErrorCode::ResidualInconclusive,
            "derived function decisions did not form one coupled component",
        )
    })?;
    for row in rows {
        budget.checkpoint()?;
        let filled = plan::fill_with_budget(template, row, budget)?;
        let parsed = crate::residual::parse_request(&encode(&filled)?)?;
        budget.checkpoint()?;
        // For a bound draft, Program is the validated proposed graph, including
        // its definitions. No declarations from the accepted base replace it.
        interfaces::check(program, names, &serde_json::Map::new(), &parsed, budget)?;
        budget.checkpoint()?;
    }
    let report = json!({
        "profile":if bound_draft {"draft-derived-fragment-dependencies-v1"} else {"accepted-derived-fragment-dependencies-v1"},
        "binding":context,"components":[fields],
        "dependencies":coupled["dependencies"],
        "scope":"all decisions in one derived function conservatively coupled across shared interfaces",
        "coupling_basis":["function_signature","bindings","evaluation_order","failure_routes","effects"],
        "interface_preflight":{"completions":rows.len(),"status":"all_joined_rows_passed","composition":"partial"},
        "row_validity":"not_asserted","compiler":"not_run","kernel":"not_run",
        "external_clients":"not_modeled","task_correctness":"not_established"
    });
    Ok((problem, frontier, report))
}

fn context(
    program: &Program,
    names: &Names,
    request: &Request,
    budget: &mut Budget,
) -> Result<String> {
    if program.objects().len() > 65_535 {
        return Err(error(
            AgentErrorCode::ResidualLimit,
            "fragment dependency source exceeds 65535 objects",
        ));
    }
    let mut bytes = 0_usize;
    let mut objects = Vec::with_capacity(program.objects().len());
    for object in program.objects() {
        bytes = bytes
            .checked_add(object.stored_bytes().len())
            .filter(|bytes| *bytes <= 8 * 1024 * 1024)
            .ok_or_else(|| {
                error(
                    AgentErrorCode::ResidualLimit,
                    "fragment dependency source exceeds 8 MiB",
                )
            })?;
        budget.charge(object.stored_bytes().len())?;
        objects.push(json!([
            crate::hex::encode(object.record().entity_id.as_bytes()),
            crate::hex::encode(object.object_id().as_bytes())
        ]));
    }
    let mut symbols = Vec::new();
    for (name, id) in names.iter() {
        budget.charge(name.len().saturating_add(32))?;
        symbols.push(json!([name, crate::hex::encode(id.as_bytes())]));
    }
    let digest = binding::canonical_digest(
        "derived-fragment-dependency-binding-v1",
        &json!({
            "root":crate::hex::encode(program.root().as_bytes()),
            "workspace":crate::hex::encode(program.workspace().as_bytes()),
            "epoch":crate::hex::encode(program.epoch().as_bytes()),
            "objects":objects,"names":symbols,"request":request.value()
        }),
    )?;
    budget.checkpoint()?;
    Ok(digest)
}
