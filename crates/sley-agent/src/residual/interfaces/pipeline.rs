//! Integer type equalities for the closed checked-pipeline grammar.
//! This produces evidence only; it never annotates or rewrites authored operands.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use sley_ssmc::{IntegerWidth, TypeExpr};

use super::{Bindings, conflict, field};
use crate::afx::Context;
use crate::error::{AgentError, AgentErrorCode, Result};
use crate::residual::{fragments, frontier::Budget};
use crate::types;

struct Node {
    parent: usize,
    known: Option<(TypeExpr, String)>,
}

struct Check<'a, 'p> {
    cx: &'a Context<'p>,
    inputs: &'a Bindings,
    budget: &'a mut Budget,
    nodes: Vec<Node>,
    steps: BTreeMap<String, (usize, usize, String)>,
    operations: usize,
    negations: Vec<(usize, String)>,
    literals: Vec<(usize, Value, String)>,
}

pub(super) fn check(
    cx: &Context<'_>,
    inputs: &Bindings,
    output: &TypeExpr,
    returns: &TypeExpr,
    bindings: &Map<String, Value>,
    at: &str,
    budget: &mut Budget,
) -> Result<Value> {
    let steps = field(bindings, "steps")
        .as_array()
        .ok_or_else(|| conflict(at, "expected explicit pipeline steps"))?;
    if steps.len() > fragments::MAX_FRAGMENT_ITEMS {
        return Err(limit(at));
    }
    let mut check = Check {
        cx,
        inputs,
        budget,
        nodes: Vec::new(),
        steps: BTreeMap::new(),
        operations: 0,
        negations: Vec::new(),
        literals: Vec::new(),
    };
    // Declare all step names first: a later definition shadows a parameter in
    // AF1-X's authored block too, and cannot be mistaken for an earlier input.
    for (index, step) in steps.iter().enumerate() {
        let source = format!("{at}/steps/{index}");
        let pair = step
            .as_array()
            .filter(|pair| pair.len() == 2)
            .ok_or_else(|| conflict(&source, "expected [name, arithmetic expression]"))?;
        let name = pair[0]
            .as_str()
            .filter(|name| crate::names::is_identifier(name) && !name.contains("__"))
            .ok_or_else(|| conflict(&source, "expected a step binding name"))?;
        let node = check.node(None)?;
        if let Some((_, _, previous)) = check
            .steps
            .insert(name.into(), (node, index, source.clone()))
        {
            return Err(conflict(
                &source,
                &format!("duplicate step binding conflicts with {previous}"),
            ));
        }
    }
    for (index, step) in steps.iter().enumerate() {
        let source = format!("{at}/steps/{index}/1");
        if !step[1].is_array() {
            return Err(conflict(
                &source,
                "a pipeline step requires an arithmetic operation",
            ));
        }
        let value = check.expression(&step[1], &source, index, 0)?;
        let (node, _, _) = check.steps[step[0].as_str().expect("checked step name")];
        check.merge(node, value, &source)?;
    }
    let result_at = format!("{at}/result");
    if field(bindings, "result").is_array() {
        return Err(conflict(&result_at, "put arithmetic in a named step"));
    }
    let result = check.expression(field(bindings, "result"), &result_at, steps.len(), 0)?;
    let expected = check.node(Some((output.clone(), "/bindings/returns".into())))?;
    check.merge(result, expected, &result_at)?;
    check.signed_negations()?;
    let mut unresolved = 0;
    for index in 0..check.nodes.len() {
        let root = check.root(index)?;
        if check.nodes[root].known.is_none() {
            unresolved += 1;
        }
    }
    let checked_literals = check.literal_ranges()?;
    let ordinary = ordinary_contexts(
        cx,
        inputs,
        returns,
        steps,
        field(bindings, "result"),
        at,
        check.budget,
    )?;
    Ok(json!({"at":at,"pipeline_integer_connections":"checked",
        "operations":check.operations,"unresolved_type_nodes":unresolved,
        "expression_types":if unresolved==0 {"constraints_checked"} else {"unanchored_literals_deferred"},
        "checked_literals":checked_literals,
        "ordinary_literal_contexts":if ordinary.untyped_literals.is_empty(){"checked"}else{"partial"},
        "untyped_literal_contexts":ordinary.untyped_literals,
        "literal_contexts":ordinary.literals.iter().map(|(at,ty)|json!({"at":at,"type":cx.render(ty)})).collect::<Vec<_>>(),
        "rewritten":false}))
}

fn ordinary_contexts(
    cx: &Context<'_>,
    inputs: &Bindings,
    returns: &TypeExpr,
    steps: &[Value],
    result: &Value,
    at: &str,
    budget: &mut Budget,
) -> Result<crate::afx::operations::continuations::Inventory> {
    crate::afx::operations::continuations::pipeline(
        cx,
        inputs
            .iter()
            .map(|(name, binding)| {
                (
                    name.clone(),
                    Some(binding.ty.clone()),
                    Value::from(cx.render(&binding.ty)),
                )
            })
            .collect(),
        returns,
        steps,
        result,
        at,
        &mut || budget.checkpoint(),
    )
}

impl Check<'_, '_> {
    fn literal_ranges(&mut self) -> Result<usize> {
        let mut checked_literals = 0;
        for (node, value, source) in std::mem::take(&mut self.literals) {
            self.budget.checkpoint()?;
            let root = self.root(node)?;
            if let Some((ty, type_source)) = &self.nodes[root].known {
                crate::values::read(
                    &value,
                    ty,
                    &crate::values::ProgramTypes {
                        program: self.cx.program,
                        names: self.cx.names,
                    },
                    &source,
                )
                .map_err(|error| {
                    conflict(
                        &source,
                        &format!("literal conflicts with {type_source}: {}", error.detail()),
                    )
                })?;
                checked_literals += 1;
            }
        }
        Ok(checked_literals)
    }

    fn signed_negations(&mut self) -> Result<()> {
        for (node, source) in std::mem::take(&mut self.negations) {
            self.budget.checkpoint()?;
            let root = self.root(node)?;
            if let Some((TypeExpr::UInt(_), type_source)) = &self.nodes[root].known {
                return Err(conflict(
                    &source,
                    &format!("negation requires a signed integer, conflicting with {type_source}"),
                ));
            }
        }
        Ok(())
    }

    fn node(&mut self, known: Option<(TypeExpr, String)>) -> Result<usize> {
        self.budget.checkpoint()?;
        if let Some((ty, source)) = &known
            && !matches!(ty, TypeExpr::SInt(_) | TypeExpr::UInt(_))
        {
            return Err(conflict(
                source,
                &format!(
                    "pipeline operand requires an integer, found {}",
                    self.cx.render(ty)
                ),
            ));
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            parent: index,
            known,
        });
        Ok(index)
    }

    fn root(&mut self, mut index: usize) -> Result<usize> {
        while self.nodes[index].parent != index {
            self.budget.checkpoint()?;
            let parent = self.nodes[index].parent;
            self.nodes[index].parent = self.nodes[parent].parent;
            index = parent;
        }
        Ok(index)
    }

    fn merge(&mut self, left: usize, right: usize, at: &str) -> Result<()> {
        self.budget.checkpoint()?;
        let left = self.root(left)?;
        let right = self.root(right)?;
        if left == right {
            return Ok(());
        }
        if let (Some((a, a_at)), Some((b, b_at))) =
            (&self.nodes[left].known, &self.nodes[right].known)
            && a != b
        {
            return Err(conflict(
                at,
                &format!(
                    "{a_at} has {} but {b_at} requires {}",
                    self.cx.render(a),
                    self.cx.render(b)
                ),
            ));
        }
        let (root, child) = (left.min(right), left.max(right));
        if self.nodes[root].known.is_none() {
            self.nodes[root].known = self.nodes[child].known.take();
        }
        self.nodes[child].parent = root;
        Ok(())
    }

    fn expression(&mut self, value: &Value, at: &str, step: usize, depth: usize) -> Result<usize> {
        self.budget.checkpoint()?;
        if depth > crate::afx::MAX_EXPR_DEPTH {
            return Err(limit(at));
        }
        match value {
            Value::Number(_) => {
                let node = self.node(None)?;
                self.literals.push((node, value.clone(), at.into()));
                Ok(node)
            }
            Value::String(name) => {
                let (name, index) = match name.split_once('#') {
                    Some((name, index)) => (
                        name,
                        index
                            .parse::<u32>()
                            .map_err(|_| conflict(at, "invalid local result index"))?,
                    ),
                    None => (name.as_str(), 0),
                };
                if let Some((node, definition, source)) = self.steps.get(name) {
                    if *definition >= step {
                        return Err(conflict(
                            at,
                            &format!(
                                "step binding {source} is not available before its definition"
                            ),
                        ));
                    }
                    if index != 0 {
                        return Err(conflict(
                            at,
                            "nonzero pipeline step result indexes require explicit AF1-X",
                        ));
                    }
                    return Ok(*node);
                }
                let binding = self.inputs.get(name).ok_or_else(|| {
                    conflict(at, "pipeline value is not a parameter or earlier step")
                })?;
                self.node(Some(binding.known()))
            }
            Value::Object(object) => {
                if object.len() != 2
                    || !object.contains_key("type")
                    || !object.get("value").is_some_and(Value::is_number)
                {
                    return Err(conflict(at, "expected an explicitly typed integer literal"));
                }
                let ty = types::read(&object["type"], self.cx, &format!("{at}/type"))?;
                let node = self.node(Some((ty, format!("{at}/type"))))?;
                self.literals
                    .push((node, object["value"].clone(), format!("{at}/value")));
                Ok(node)
            }
            Value::Array(items) => {
                let opcode = items.first().and_then(Value::as_str).unwrap_or("");
                let row = fragments::pipeline_opcode(opcode).ok_or_else(|| {
                    conflict(at, "unsupported pipeline arithmetic opcode; use AF1-X")
                })?;
                let arity = if row.tag == 69 { 1 } else { 2 };
                if items.len() != arity + 1 {
                    return Err(conflict(at, "wrong arithmetic operand count"));
                }
                self.operations += 1;
                if self.operations > fragments::MAX_CONSTRUCTION_ITEMS {
                    return Err(limit(at));
                }
                let left = self.expression(&items[1], &format!("{at}/1"), step, depth + 1)?;
                if row.tag == 69 {
                    self.negations.push((left, at.into()));
                }
                if arity == 2 {
                    let right_at = format!("{at}/2");
                    let right = self.expression(&items[2], &right_at, step, depth + 1)?;
                    if matches!(row.tag, 70 | 71) {
                        let shift = self.node(Some((
                            TypeExpr::UInt(IntegerWidth::from_bits(32)),
                            format!("{at}/0 (shift count)"),
                        )))?;
                        self.merge(right, shift, &right_at)?;
                    } else {
                        self.merge(left, right, at)?;
                    }
                }
                Ok(left)
            }
            _ => Err(conflict(
                at,
                "pipeline operand must be an integer expression",
            )),
        }
    }
}

fn limit(at: &str) -> AgentError {
    AgentError::new(
        AgentErrorCode::ResidualLimit,
        format!("{at}: pipeline type-check budget exceeded"),
    )
}
