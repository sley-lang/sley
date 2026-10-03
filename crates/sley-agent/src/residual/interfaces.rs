//! Resolve declared fragment interfaces before generating any blocks or operations.
//! This is an initial composition check, not an ownership/effect or kernel proof.

mod declarations;
mod effects;
mod flow;
mod order;
mod pipeline;
mod predicates;
mod regions;
mod signatures;
mod source_topology;

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use sley_ssmc::{BuiltinFailureKind, TypeExpr};

use super::{Operation, Request, fragments, frontier::Budget};
use crate::afx::Context;
use crate::error::{AgentError, AgentErrorCode, Result};
use crate::{names::Names, types, workspace::Program};

#[derive(Clone, Copy, Eq, PartialEq)]
enum BindingKind {
    Parameter,
    Operation,
    Continuation,
}

#[derive(Clone)]
struct Binding {
    ty: TypeExpr,
    source: String,
    kind: BindingKind,
}

impl Binding {
    fn parameter(ty: TypeExpr, source: String) -> Self {
        Self {
            ty,
            source,
            kind: BindingKind::Parameter,
        }
    }

    fn operation(ty: TypeExpr, source: String, continuation: bool) -> Self {
        Self {
            ty,
            source,
            kind: if continuation {
                BindingKind::Continuation
            } else {
                BindingKind::Operation
            },
        }
    }

    fn known(&self) -> (TypeExpr, String) {
        (self.ty.clone(), self.source.clone())
    }
}

type Bindings = BTreeMap<String, Binding>;

/// Checks declared types and supported interface connections without expansion.
/// `declarations` is the bound source draft frame, or an empty object for an
/// accepted base. Type names resolve through the existing AF1-X context.
///
/// # Errors
/// Refuses malformed or conflicting interfaces, unknown declared types, and
/// exhausted shared budgets. Successful evidence explicitly retains the checks
/// that still require the ordinary compiler and kernel.
pub fn check(
    program: &Program,
    names: &Names,
    declarations: &Map<String, Value>,
    request: &Request,
    budget: &mut Budget,
) -> Result<Value> {
    budget.checkpoint()?;
    if request.operation != Operation::Derive {
        return Err(conflict(
            "/operation",
            "interface checks require a derive request",
        ));
    }
    let mut cx =
        Context::with_checkpoint(program, names, declarations, &mut || budget.checkpoint())?;
    let obligations = crate::afx::declared_types(declarations, &cx);
    if !obligations.is_empty() {
        return Err(crate::afx::refusal(&obligations));
    }
    budget.checkpoint()?;
    let inputs = parameters(
        field(&request.bindings, "params"),
        "/bindings/params",
        &cx,
        budget,
    )?;
    let output = types::read(
        field(&request.bindings, "returns"),
        &cx,
        "/bindings/returns",
    )?;
    sley_vm::extended::check_result_type(&output).map_err(|_| {
        conflict(
            "/bindings/returns",
            "the VM result interface cannot return an execution-local cell or host handle",
        )
    })?;
    if let Some(name) = request
        .scope
        .as_array()
        .filter(|scope| scope.len() == 1)
        .and_then(|scope| scope[0].as_str())
    {
        let ordered = field(&request.bindings, "params")
            .as_array()
            .into_iter()
            .flatten()
            .map(|pair| {
                pair[0]
                    .as_str()
                    .and_then(|name| inputs.get(name))
                    .map(|binding| binding.ty.clone())
            })
            .collect();
        cx.set_signature(name, (ordered, Some(output.clone())));
    }
    let replaced = request
        .scope
        .as_array()
        .filter(|scope| scope.len() == 1)
        .and_then(|scope| scope[0].as_str());
    let body_effects = effects::check(&cx, declarations, replaced, budget)?;
    let mut checker = Checker {
        cx: &cx,
        inputs: &inputs,
        output: &output,
        budget,
        entries: Vec::new(),
        connections: Vec::new(),
        declared_types: inputs
            .values()
            .map(|binding| (format!("{}/1", binding.source), binding.ty.clone()))
            .chain(std::iter::once((
                "/bindings/returns".into(),
                output.clone(),
            )))
            .collect(),
    };
    let body_control_flow = source_topology::check(&cx, declarations, replaced, checker.budget)?;
    checker.fragment(&request.fragment, &request.bindings, "/bindings", 1)?;
    let deferred_types = checker.check_declared_types()?;
    source_topology::retained_operations(&body_control_flow, checker.budget)?;
    checker.budget.checkpoint()?;
    Ok(json!({
        "interface_check":1, "stage":"before_fragment_expansion",
        "declared_interface_types":if deferred_types.is_empty(){"checked"}else{"partial"}, "composition":"partial",
        "closed_interface_type_count":checker.declared_types.len()-deferred_types.len(),
        "deferred_interface_types":deferred_types,
        "execution_local_return":"vm_boundary_checked",
        "declared_body_effects":body_effects,
        "declared_body_control_flow":body_control_flow,
        "interfaces":checker.entries, "connections":checker.connections,
        "deferred":["expression_types", "ownership", "effects", "full_control_flow", "kernel"],
        "authority":"none"
    }))
}

fn parameters(value: &Value, at: &str, cx: &Context<'_>, budget: &mut Budget) -> Result<Bindings> {
    let values = value
        .as_array()
        .ok_or_else(|| conflict(at, "expected typed parameter pairs"))?;
    if values.len() > fragments::MAX_CONSTRUCTION_ITEMS {
        return Err(AgentError::new(
            AgentErrorCode::ResidualLimit,
            format!("{at}: interface parameter bound exceeded"),
        ));
    }
    let mut bindings = BTreeMap::new();
    for (index, value) in values.iter().enumerate() {
        budget.checkpoint()?;
        let pointer = format!("{at}/{index}");
        let (name, ty) = parameter(value, &pointer, cx)?;
        if let Some(previous) =
            bindings.insert(name.clone(), Binding::parameter(ty, pointer.clone()))
        {
            return Err(conflict(
                &pointer,
                &format!("binding `{name}` duplicates {}", previous.source),
            ));
        }
    }
    Ok(bindings)
}

fn parameter(value: &Value, at: &str, cx: &Context<'_>) -> Result<(String, TypeExpr)> {
    let pair = value
        .as_array()
        .filter(|pair| pair.len() == 2)
        .ok_or_else(|| conflict(at, "expected [name, type]"))?;
    let name = pair[0]
        .as_str()
        .filter(|name| crate::names::is_identifier(name) && !name.contains("__"))
        .ok_or_else(|| conflict(at, "expected an explicit parameter name"))?;
    Ok((
        name.to_owned(),
        types::read(&pair[1], cx, &format!("{at}/1"))?,
    ))
}

struct Checker<'a, 'p> {
    cx: &'a Context<'p>,
    inputs: &'a Bindings,
    output: &'a TypeExpr,
    budget: &'a mut Budget,
    entries: Vec<Value>,
    connections: Vec<Value>,
    declared_types: BTreeMap<String, TypeExpr>,
}

impl Checker<'_, '_> {
    fn fragment(
        &mut self,
        fragment: &super::FragmentRef,
        bindings: &Map<String, Value>,
        at: &str,
        depth: usize,
    ) -> Result<()> {
        self.budget.checkpoint()?;
        if depth > fragments::MAX_FRAGMENT_DEPTH {
            return Err(AgentError::new(
                AgentErrorCode::ResidualLimit,
                format!("{at}: fragment interface nesting exceeds its bound"),
            ));
        }
        if fragment.version != 1 {
            return Err(conflict(at, "unknown fragment interface version"));
        }
        self.entries.push(json!({"bindings":at,"fragment":{"id":fragment.id,"version":fragment.version},
            "inputs":self.inputs.iter().map(|(name,binding)| json!({"name":name,"type":self.cx.render(&binding.ty),"source":binding.source})).collect::<Vec<_>>(),
            "output":{"type":self.cx.render(self.output),"source":"/bindings/returns"}}));
        match fragment.id.as_str() {
            "ordered_guard_chain" => {
                self.guards(bindings, at)?;
                if let Some(nested) = bindings.get("success").and_then(Value::as_object)
                    && let Some(fragment) = nested.get("fragment")
                {
                    let fragment = super::parse_fragment(fragment.clone())?;
                    let next = nested
                        .get("bindings")
                        .and_then(Value::as_object)
                        .ok_or_else(|| {
                            conflict(
                                &format!("{at}/success/bindings"),
                                "expected nested bindings",
                            )
                        })?;
                    self.fragment(
                        &fragment,
                        next,
                        &format!("{at}/success/bindings"),
                        depth + 1,
                    )?;
                } else {
                    self.term_region(
                        &Bindings::new(),
                        field(bindings, "success"),
                        &format!("{at}/success"),
                    )?;
                }
            }
            "checked_pipeline" => self.pipeline(bindings, at)?,
            "typed_branch_result" => self.branch(bindings, at)?,
            _ => return Err(conflict(at, "unknown fragment interface")),
        }
        Ok(())
    }

    fn pipeline(&mut self, bindings: &Map<String, Value>, at: &str) -> Result<()> {
        let TypeExpr::Result { ok, error } = self.output else {
            return Err(conflict(
                at,
                "checked_pipeline requires /bindings/returns to be Result<integer,error>",
            ));
        };
        if !matches!(**ok, TypeExpr::SInt(_) | TypeExpr::UInt(_)) {
            return Err(conflict(
                at,
                "checked_pipeline integer output conflicts with /bindings/returns",
            ));
        }
        if bindings
            .get("arithmetic_failure")
            .and_then(|route| route.get("propagate"))
            == Some(&Value::Bool(true))
            && **error != TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)
        {
            return Err(conflict(
                &format!("{at}/arithmetic_failure"),
                "propagated ArithmeticError conflicts with /bindings/returns; supply the intended error mapping",
            ));
        }
        if let Some(route) = bindings.get("arithmetic_failure").and_then(Value::as_str) {
            self.arithmetic_route(route, &format!("{at}/arithmetic_failure"))?;
        }
        self.connections.push(pipeline::check(
            self.cx,
            self.inputs,
            ok,
            self.output,
            bindings,
            at,
            self.budget,
        )?);
        Ok(())
    }

    fn branch(&mut self, bindings: &Map<String, Value>, at: &str) -> Result<()> {
        let join_at = format!("{at}/join/params");
        if !field(bindings, "join")["term"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_str)
            .is_some_and(|word| matches!(word, "return" | "ok" | "fail"))
        {
            return Err(conflict(
                &format!("{at}/join/term"),
                "join must explicitly return, ok, or fail",
            ));
        }
        let join = parameters(
            &field(bindings, "join")["params"],
            &join_at,
            self.cx,
            self.budget,
        )?;
        self.declared_types.extend(
            join.values()
                .map(|binding| (format!("{}/1", binding.source), binding.ty.clone())),
        );
        // Preserve written argument order; the name map is only for duplicate checks.
        let join_names = field(bindings, "join")["params"]
            .as_array()
            .expect("checked parameters");
        let cases = field(bindings, "cases")
            .as_array()
            .ok_or_else(|| conflict(at, "expected explicit branch cases"))?;
        if cases.len() > fragments::MAX_FRAGMENT_ITEMS {
            return Err(AgentError::new(
                AgentErrorCode::ResidualLimit,
                format!("{at}/cases: interface case bound exceeded"),
            ));
        }
        let expected_cases = self.branch_cases(field(bindings, "input"), cases, at)?;
        for (index, case) in cases.iter().enumerate() {
            self.budget.checkpoint()?;
            let case_at = format!("{at}/cases/{index}");
            let payload_at = format!("{case_at}/payload");
            let payload = if case["payload"].is_null() {
                None
            } else {
                Some(parameter(&case["payload"], &payload_at, self.cx)?)
            };
            if let Some((_, ty)) = &payload {
                self.declared_types
                    .insert(format!("{payload_at}/1"), ty.clone());
            }
            if let Some((expected, source)) = &expected_cases {
                let expected = &expected[case["case"].as_str().expect("checked case")];
                if expected.as_ref() != payload.as_ref().map(|(_, ty)| ty) {
                    return Err(conflict(
                        &payload_at,
                        &format!("payload type conflicts with {source} selected by {at}/input"),
                    ));
                }
            }
            let values = case["values"]
                .as_array()
                .ok_or_else(|| conflict(&case_at, "expected join values"))?;
            if values.len() != join_names.len() {
                return Err(conflict(
                    &format!("{case_at}/values"),
                    &format!("argument count conflicts with {join_at}"),
                ));
            }
            let locals = payload
                .into_iter()
                .map(|(name, ty)| (name, Binding::parameter(ty, payload_at.clone())))
                .collect();
            let ends = regions::edge_ends(values, &join, join_names, &case_at);
            let region =
                self.region_ops(&locals, &case["ops"], &format!("{case_at}/ops"), &ends)?;
            for (index, (value, declared)) in values.iter().zip(join_names).enumerate() {
                let expected = &join[declared[0].as_str().expect("parameter name")];
                self.region_value(
                    &region,
                    value,
                    &format!("{case_at}/values/{index}"),
                    &expected.known(),
                )?;
            }
        }
        self.term_region(&join, field(bindings, "join"), &format!("{at}/join"))?;
        Ok(())
    }
}

fn conflict(at: &str, detail: &str) -> AgentError {
    AgentError::new(
        AgentErrorCode::ResidualConstraintConflict,
        format!("{at}: {detail}"),
    )
}

fn field<'a>(bindings: &'a Map<String, Value>, key: &str) -> &'a Value {
    bindings.get(key).unwrap_or(&Value::Null)
}

/// CLI readiness is stricter than a read-only, explicitly partial analysis.
///
/// # Errors
/// Reports a missing ordinary literal context before expansion, after known
/// interface/index/route conflicts have had their existing diagnostic priority.
pub fn require_literal_contexts(report: &Value, budget: &mut Budget) -> Result<()> {
    if let Some(at) = first_pending(report, "untyped_literal_contexts", budget)? {
        return Err(conflict(
            at,
            "ordinary AF1-X has no type context for this literal; state an explicit type or use ordinary AF1-X to supply the missing declaration",
        ));
    }
    Ok(())
}

/// Requires resolved expression connections before CLI fragment expansion.
/// Library analyses remain inspectable when types or supported syntax are missing.
///
/// # Errors
/// Refuses an unresolved expression or an exhausted shared budget. This is not
/// a kernel validity or whole control-flow certificate.
pub fn require_expression_types(report: &Value, budget: &mut Budget) -> Result<()> {
    if let Some(at) = first_pending(report, "deferred_expressions", budget)? {
        return Err(conflict(
            at,
            "expression compatibility is unresolved before expansion; supply a complete typed supported expression or use ordinary AF1-X to inspect and repair the missing declaration or unsupported syntax",
        ));
    }
    Ok(())
}

fn first_pending<'a>(report: &'a Value, key: &str, budget: &mut Budget) -> Result<Option<&'a str>> {
    let mut pending = vec![report];
    let mut missing = std::collections::BTreeSet::new();
    while let Some(value) = pending.pop() {
        budget.checkpoint()?;
        match value {
            Value::Object(object) => {
                if let Some(items) = object.get(key).and_then(Value::as_array) {
                    for item in items {
                        if let Some(at) = item.as_str() {
                            missing.insert(at);
                        }
                    }
                }
                pending.extend(object.values());
            }
            Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
    Ok(missing.first().copied())
}
