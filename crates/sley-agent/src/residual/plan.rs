//! Missing author decisions, with no semantic completion from sampled families.
//!
//! This inventory is the explicit-authoring fallback of the frontier planner.
//! It never supplies a value, even for a singleton supported policy domain.

use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::frontier::{Budget, encoding};
use super::{FragmentRef, Operation, Request};
use crate::error::{AgentError, AgentErrorCode, Result};

/// Aggregate question-field bound, including nested fragments.
pub const MAX_DECISIONS: usize = 64;

#[derive(Clone, Copy)]
enum Shape {
    Any,
    List,
    Object,
    Type,
    Text,
    Integer,
    Payload,
    Rounding,
    Failure,
}

impl Shape {
    fn description(self) -> &'static str {
        match self {
            Self::Any => "explicit AF1-X value or expression",
            Self::List => "ordered JSON array; preserve evaluation order",
            Self::Object => "explicit object conforming to the selected fragment",
            Self::Type => "explicit AF1 type",
            Self::Text => "explicit name",
            Self::Integer => "JSON integer within the inherited width",
            Self::Payload => "null or [name, AF1 type]",
            Self::Rounding => {
                "signed division rounding; supported: toward_zero; use AF1-X for another policy"
            }
            Self::Failure => {
                "explicit error variant case name or {propagate:true} for native arithmetic errors"
            }
        }
    }

    fn accepts(self, value: &Value) -> bool {
        match self {
            Self::Any => true,
            Self::List => value.is_array(),
            Self::Object => value.is_object(),
            Self::Type => value.is_string() || value.is_object(),
            Self::Text => value.as_str().is_some_and(|text| !text.is_empty()),
            Self::Integer => value.as_i64().is_some() || value.as_u64().is_some(),
            Self::Payload => {
                value.is_null()
                    || value.as_array().is_some_and(|pair| {
                        pair.len() == 2 && pair[0].is_string() && Self::Type.accepts(&pair[1])
                    })
            }
            Self::Rounding => value == "toward_zero",
            Self::Failure => {
                value
                    .as_str()
                    .is_some_and(|name| crate::names::is_identifier(name) && !name.contains("__"))
                    || value.as_object().is_some_and(|object| {
                        object.len() == 1
                            && object.get("propagate").and_then(Value::as_bool) == Some(true)
                    })
            }
        }
    }
}

struct Inventory<'a> {
    budget: &'a mut Budget,
    decisions: Vec<Value>,
    visited: usize,
    start: Instant,
}

impl Inventory<'_> {
    fn charge(&mut self) -> Result<()> {
        self.budget.checkpoint()?;
        self.visited += 1;
        if self.visited > super::fragments::MAX_CONSTRUCTION_ITEMS
            || self.start.elapsed() > Duration::from_millis(250)
        {
            return Err(error(
                AgentErrorCode::ResidualLimit,
                "decision inventory exhausted; use explicit AF1-X",
            ));
        }
        Ok(())
    }

    fn object(
        &mut self,
        object: &Map<String, Value>,
        at: &str,
        fields: &[(&str, Shape)],
        optional: &[(&str, Shape)],
    ) -> Result<()> {
        self.charge()?;
        for (name, value) in object {
            self.budget.charge(1)?;
            let Some((_, shape)) = fields
                .iter()
                .chain(optional)
                .find(|(field, _)| *field == name)
            else {
                return Err(error(
                    AgentErrorCode::ResidualFragmentShape,
                    &format!("{at}/{name}: unknown field"),
                ));
            };
            if !shape.accepts(value) {
                return Err(error(
                    if matches!(shape, Shape::Rounding) {
                        AgentErrorCode::ResidualConstraintConflict
                    } else {
                        AgentErrorCode::ResidualFragmentShape
                    },
                    &format!("{at}/{name}: {}", shape.description()),
                ));
            }
        }
        for (name, shape) in fields {
            self.budget.charge(1)?;
            if !object.contains_key(*name) {
                if self.decisions.len() == MAX_DECISIONS {
                    return Err(error(
                        AgentErrorCode::ResidualLimit,
                        "more than 64 unresolved fields; use explicit AF1-X",
                    ));
                }
                self.decisions.push(json!({"path":format!("{at}/{}", super::edit::pointer_key(name)),"class":"UNRESOLVED", "requires":shape.description(),
                    "supported_values":if matches!(shape, Shape::Rounding) {json!(["toward_zero"])} else {Value::Null}}));
            }
        }
        Ok(())
    }

    fn fragment(
        &mut self,
        fragment: &FragmentRef,
        bindings: &Map<String, Value>,
        at: &str,
        top: bool,
        depth: usize,
    ) -> Result<()> {
        use Shape::{Any, Failure, List, Object, Rounding, Type};
        if depth > super::fragments::MAX_FRAGMENT_DEPTH {
            return Err(error(
                AgentErrorCode::ResidualLimit,
                "fragment nesting exceeds eight",
            ));
        }
        if fragment.version != 1 {
            return Err(error(
                AgentErrorCode::ResidualFragmentUnknown,
                "unsupported fragment version",
            ));
        }
        let body: &[(&str, Shape)] = match fragment.id.as_str() {
            "ordered_guard_chain" => &[("guards", List), ("success", Object)],
            "checked_pipeline" => &[
                ("steps", List),
                ("arithmetic_failure", Failure),
                ("rounding", Rounding),
                ("result", Any),
            ],
            "typed_branch_result" => &[("input", Any), ("cases", List), ("join", Object)],
            _ => {
                return Err(error(
                    AgentErrorCode::ResidualFragmentUnknown,
                    "unknown fragment family",
                ));
            }
        };
        let mut fields = body.to_vec();
        if top {
            fields.extend([("params", List), ("returns", Type)]);
        }
        self.object(bindings, at, &fields, &[])?;
        match fragment.id.as_str() {
            "ordered_guard_chain" => self.guards(bindings, at, depth),
            "typed_branch_result" => self.branch(bindings, at),
            _ => Ok(()),
        }
    }

    fn guards(&mut self, bindings: &Map<String, Value>, at: &str, depth: usize) -> Result<()> {
        if let Some(guards) = bindings.get("guards") {
            for (index, guard) in bounded_list(guards)?.iter().enumerate() {
                self.object(
                    object(guard)?,
                    &format!("{at}/guards/{index}"),
                    &[("when", Shape::Any), ("fail", Shape::List)],
                    &[],
                )?;
            }
        }
        if let Some(success) = bindings.get("success") {
            let success = object(success)?;
            let path = format!("{at}/success");
            if let Some(fragment) = success.get("fragment") {
                // The selection itself must already be explicit. Missing nested
                // bindings are one authored subtree, never an inferred program.
                self.object(
                    success,
                    &path,
                    &[("fragment", Shape::Object), ("bindings", Shape::Object)],
                    &[],
                )?;
                let fragment = super::parse_fragment(fragment.clone())?;
                if fragment.version != 1
                    || !matches!(
                        fragment.id.as_str(),
                        "ordered_guard_chain" | "checked_pipeline" | "typed_branch_result"
                    )
                {
                    return Err(error(
                        AgentErrorCode::ResidualFragmentUnknown,
                        "unsupported nested fragment",
                    ));
                }
                if let Some(bindings) = success.get("bindings") {
                    self.fragment(
                        &fragment,
                        object(bindings)?,
                        &format!("{path}/bindings"),
                        false,
                        depth + 1,
                    )?;
                }
            } else {
                self.object(
                    success,
                    &path,
                    &[("ops", Shape::List), ("term", Shape::List)],
                    &[],
                )?;
            }
        }
        Ok(())
    }

    fn branch(&mut self, bindings: &Map<String, Value>, at: &str) -> Result<()> {
        if let Some(cases) = bindings.get("cases") {
            let cases = bounded_list(cases)?;
            if cases.is_empty() {
                return Err(error(
                    AgentErrorCode::ResidualChoiceMissing,
                    "empty branch cases do not define a result; supply exhaustive cases",
                ));
            }
            for (index, case) in cases.iter().enumerate() {
                self.object(
                    object(case)?,
                    &format!("{at}/cases/{index}"),
                    &[
                        ("case", Shape::Text),
                        ("payload", Shape::Payload),
                        ("ops", Shape::List),
                        ("values", Shape::List),
                    ],
                    &[],
                )?;
            }
        }
        if let Some(join) = bindings.get("join") {
            self.object(
                object(join)?,
                &format!("{at}/join"),
                &[
                    ("params", Shape::List),
                    ("ops", Shape::List),
                    ("term", Shape::List),
                ],
                &[],
            )?;
        }
        Ok(())
    }
}

/// Lists missing author choices across the selected fragment composition.
/// Already supplied values are never overwritten or guessed.
///
/// # Errors
/// Refuses unknown fragment schemas, malformed supplied fields and inventory
/// bounds. Typed graph and compiler validation still follow after filling.
pub fn decisions(request: &Request) -> Result<Vec<Value>> {
    decisions_with_budget(request, &mut Budget::default())
}

fn decisions_with_budget(request: &Request, budget: &mut Budget) -> Result<Vec<Value>> {
    if request.choices.is_some() {
        return Ok(super::choices::analyze_with_budget(request, budget)?.decisions());
    }
    explicit_decisions_with_budget(request, budget)
}

pub(crate) fn explicit_decisions_with_budget(
    request: &Request,
    budget: &mut Budget,
) -> Result<Vec<Value>> {
    budget.checkpoint()?;
    let mut inventory = Inventory {
        budget,
        decisions: Vec::new(),
        visited: 0,
        start: Instant::now(),
    };
    match request.operation {
        Operation::Derive => {
            if !request.scope.as_array().is_some_and(|scope| {
                scope.len() == 1
                    && scope[0].as_str().is_some_and(|name| {
                        crate::names::is_identifier(name) && !name.contains("__")
                    })
            }) {
                return Err(error(
                    AgentErrorCode::ResidualScope,
                    "derive scope must name exactly one function",
                ));
            }
            inventory.fragment(&request.fragment, &request.bindings, "/bindings", true, 1)?;
        }
        Operation::Edit => {
            if !request
                .preserve
                .as_ref()
                .and_then(Value::as_object)
                .is_some_and(|preserve| {
                    preserve.len() == 2
                        && preserve.get("outside_scope") == Some(&Value::Bool(true))
                        && preserve.get("boundaries") == Some(&Value::Bool(true))
                })
            {
                return Err(error(
                    AgentErrorCode::ResidualPreserve,
                    "integer_literal requires outside_scope:true and boundaries:true",
                ));
            }
            if request.fragment.id != "checked_pipeline"
                || request.fragment.version != 1
                || request.bindings.get("lens").and_then(Value::as_str) != Some("integer_literal")
            {
                return Err(error(
                    AgentErrorCode::ResidualFragmentUnknown,
                    "select the supported integer_literal edit lens explicitly",
                ));
            }
            let targets = super::edit::scope_names(request)?;
            if request.bindings.contains_key("values") {
                inventory.object(
                    &request.bindings,
                    "/bindings",
                    &[("lens", Shape::Text), ("values", Shape::Object)],
                    &[],
                )?;
                let fields: Vec<_> = targets.iter().map(|name| (*name, Shape::Integer)).collect();
                inventory.object(
                    request.bindings["values"]
                        .as_object()
                        .expect("checked shape"),
                    "/bindings/values",
                    &fields,
                    &[],
                )?;
            } else {
                inventory.object(
                    &request.bindings,
                    "/bindings",
                    &[("lens", Shape::Text), ("value", Shape::Integer)],
                    &[("overrides", Shape::Object)],
                )?;
            }
        }
    }
    inventory
        .decisions
        .sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    inventory.budget.checkpoint()?;
    Ok(inventory.decisions)
}

/// Strict fill envelope. Parsing precedes workspace access and any state write.
///
/// # Errors
/// Refuses duplicate keys, unknown fields, malformed handles and invalid versions.
pub fn parse_fill(bytes: &[u8], handle: &str) -> Result<Map<String, Value>> {
    let value = super::strict_json(bytes)?;
    let object = value
        .as_object()
        .ok_or_else(|| error(AgentErrorCode::ResidualParse, "fill must be an object"))?;
    if object.len() != 3
        || !object.contains_key("residual")
        || !object.contains_key("plan")
        || !object.contains_key("choose")
    {
        return Err(error(
            AgentErrorCode::ResidualParse,
            "fill requires exactly residual, plan and choose",
        ));
    }
    if object["residual"].as_u64() != Some(1) {
        return Err(error(
            AgentErrorCode::ResidualVersion,
            "fill residual version must be integer 1",
        ));
    }
    if !super::store::is_handle(handle) || object["plan"].as_str() != Some(handle) {
        return Err(error(
            AgentErrorCode::ResidualBindingStale,
            "fill must name the exact plan rN@1",
        ));
    }
    object["choose"].as_object().cloned().ok_or_else(|| {
        error(
            AgentErrorCode::ResidualParse,
            "choose must be an object of exact unresolved paths",
        )
    })
}

/// Inserts precisely the missing author fields. The selected values undergo
/// the same strict request and fragment checks as direct authoring.
///
/// # Errors
/// Rejects unknown paths, partial answers, changed authored values, and an
/// answer that introduces another unresolved decision (one question round).
pub fn fill(original: &Value, choose: &Map<String, Value>) -> Result<Value> {
    fill_with_budget(original, choose, &mut Budget::default())
}

/// Fills using the caller's aggregate clock, work and serialization budget.
///
/// # Errors
/// The same refusals as [`fill`], including exhaustion in earlier stages.
pub fn fill_with_budget(
    original: &Value,
    choose: &Map<String, Value>,
    budget: &mut Budget,
) -> Result<Value> {
    // Measure without an output allocation before cloning an arbitrary caller's
    // JSON. Existing tree/clone allocations are not claimed as reserved memory.
    encoding::size(original, budget, super::MAX_REQUEST_BYTES)?;
    encoding::size(choose, budget, super::MAX_REQUEST_BYTES)?;
    let request = super::request_from_value(original.clone())?;
    if request.choices.is_some() {
        return Ok(super::choices::analyze_with_budget(&request, budget)?
            .resolve_with_budget(
                choose,
                "residual-plan.json",
                "/artifacts/residual-request.json",
                true,
                budget,
            )?
            .request);
    }
    let required = decisions_with_budget(&request, budget)?;
    for path in choose.keys() {
        budget.checkpoint()?;
        if !required.iter().any(|field| field["path"] == *path) {
            return Err(error(
                AgentErrorCode::ResidualChoiceUnknown,
                &format!("{path}: not an unresolved decision in this plan"),
            ));
        }
    }
    if choose.len() != required.len() {
        return Err(error(
            AgentErrorCode::ResidualChoiceMissing,
            "answer every listed decision in one fill, or use explicit AF1-X",
        ));
    }
    let mut filled = original.clone();
    for (path, value) in choose {
        budget.checkpoint()?;
        let (parent, key) = path.rsplit_once('/').ok_or_else(|| {
            error(
                AgentErrorCode::ResidualChoiceUnknown,
                "invalid decision path",
            )
        })?;
        let parent = filled
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| {
                error(
                    AgentErrorCode::ResidualChoiceUnknown,
                    "decision parent is not an object",
                )
            })?;
        // Paths are exactly inventory-produced pointers; decode their final
        // token once, in RFC 6901 order, before inserting the object key.
        let key = key.replace("~1", "/").replace("~0", "~");
        if parent.insert(key, value.clone()).is_some() {
            return Err(error(
                AgentErrorCode::ResidualConstraintConflict,
                "fill attempted to overwrite an authored value",
            ));
        }
    }
    let encoded = encoding::encode(&filled, budget, super::MAX_REQUEST_BYTES)?;
    let request = super::parse_request(&encoded.bytes)?;
    if !decisions_with_budget(&request, budget)?.is_empty() {
        return Err(error(
            AgentErrorCode::ResidualChoiceMissing,
            "fill introduced further choices; use an explicit revised request or AF1-X",
        ));
    }
    budget.checkpoint()?;
    Ok(filled)
}

fn object(value: &Value) -> Result<&Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        error(
            AgentErrorCode::ResidualFragmentShape,
            "expected a fragment object",
        )
    })
}
fn bounded_list(value: &Value) -> Result<&Vec<Value>> {
    let values = value.as_array().ok_or_else(|| {
        error(
            AgentErrorCode::ResidualFragmentShape,
            "expected an ordered list",
        )
    })?;
    if values.len() > super::fragments::MAX_FRAGMENT_ITEMS {
        return Err(error(
            AgentErrorCode::ResidualLimit,
            "fragment list exceeds 64 items",
        ));
    }
    Ok(values)
}
fn error(code: AgentErrorCode, detail: &str) -> AgentError {
    AgentError::new(code, detail)
}

#[cfg(test)]
mod tests;
