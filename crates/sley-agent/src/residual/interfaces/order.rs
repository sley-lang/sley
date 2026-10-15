//! Local use-before-definition checks independent of expression type inference.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::{Bindings, conflict};
use crate::error::{AgentError, AgentErrorCode, Result};
use crate::opcodes::{self, ImmediateKind};
use crate::residual::frontier::Budget;

struct Walk<'a> {
    pending: BTreeMap<String, String>,
    available: BTreeMap<String, String>,
    deferred: BTreeSet<String>,
    reads: Vec<Value>,
    budget: &'a mut Budget,
}

pub(super) fn check(
    ops: &[Value],
    inputs: &Bindings,
    locals: &Bindings,
    definitions: &BTreeMap<String, String>,
    at: &str,
    budget: &mut Budget,
) -> Result<Value> {
    let mut available: BTreeMap<_, _> = inputs
        .iter()
        .chain(locals)
        .map(|(name, binding)| (name.clone(), binding.source.clone()))
        .collect();
    let pending: BTreeMap<_, _> = definitions
        .iter()
        .filter(|(name, _)| !locals.contains_key(*name))
        .map(|(name, source)| {
            available.remove(name);
            (name.clone(), source.clone())
        })
        .collect();
    let mut walk = Walk {
        pending,
        available,
        deferred: BTreeSet::new(),
        reads: Vec::new(),
        budget,
    };
    for (index, op) in ops.iter().enumerate() {
        let source = format!("{at}/{index}");
        walk.statement(op, &source)?;
        let name = op
            .as_array()
            .and_then(|row| row.first())
            .or_else(|| op.get("name"))
            .and_then(Value::as_str);
        if let Some(name) = name
            && let Some(source) = walk.pending.remove(name)
        {
            walk.available.insert(name.into(), source);
        }
    }
    Ok(
        json!({"at":at,"local_definition_order":if walk.deferred.is_empty(){"checked"}else{"partial"},
        "reads":walk.reads,"deferred_operands":walk.deferred,"rewritten":false,
        "control_flow":"local_order_only; generated_edges_and_dominance_deferred"}),
    )
}

impl Walk<'_> {
    fn statement(&mut self, value: &Value, at: &str) -> Result<()> {
        self.budget.checkpoint()?;
        match value {
            Value::Array(row)
                if row
                    .first()
                    .and_then(Value::as_str)
                    .is_some_and(|word| word.starts_with('!')) =>
            {
                if (3..=4).contains(&row.len()) && row[1] == "if" {
                    for (index, operand) in row.iter().enumerate().skip(2) {
                        self.operand(operand, &format!("{at}/{index}"), 0)?;
                    }
                } else {
                    self.deferred.insert(at.into());
                }
            }
            Value::Array(row) if row.len() >= 2 => {
                self.operation(
                    row[1].as_str(),
                    &row[2..],
                    at,
                    &|index| format!("{at}/{}", index + 2),
                    0,
                )?;
            }
            Value::Object(object) => {
                let word = object
                    .get("op")
                    .or_else(|| object.get("opcode"))
                    .and_then(Value::as_str);
                let key = if object.contains_key("args") {
                    "args"
                } else {
                    "operands"
                };
                let args = match object.get(key) {
                    None => &[][..],
                    Some(Value::Array(args)) => args.as_slice(),
                    Some(_) => {
                        self.deferred.insert(at.into());
                        return Ok(());
                    }
                };
                self.operation(word, args, at, &|index| format!("{at}/{key}/{index}"), 0)?;
            }
            _ => {
                self.deferred.insert(at.into());
            }
        }
        Ok(())
    }

    fn operation(
        &mut self,
        word: Option<&str>,
        args: &[Value],
        at: &str,
        pointer: &dyn Fn(usize) -> String,
        depth: usize,
    ) -> Result<()> {
        self.budget.checkpoint()?;
        if depth > crate::afx::MAX_EXPR_DEPTH {
            return Err(AgentError::new(
                AgentErrorCode::ResidualLimit,
                format!("{at}: local-order expression nesting exceeds its bound"),
            ));
        }
        let row = word
            .and_then(|word| opcodes::by_word(word.split_once('?').map_or(word, |(base, _)| base)));
        let Some(row) = row else {
            self.deferred.insert(at.into());
            return Ok(());
        };
        let skip = usize::from(row.immediate != ImmediateKind::None);
        for (index, arg) in args.iter().enumerate().skip(skip) {
            self.operand(arg, &pointer(index), depth)?;
        }
        Ok(())
    }

    fn operand(&mut self, value: &Value, at: &str, depth: usize) -> Result<()> {
        self.budget.checkpoint()?;
        if depth > crate::afx::MAX_EXPR_DEPTH {
            return Err(AgentError::new(
                AgentErrorCode::ResidualLimit,
                format!("{at}: local-order expression nesting exceeds its bound"),
            ));
        }
        match value {
            Value::String(text) => {
                let name = text.split_once('#').map_or(text.as_str(), |(name, _)| name);
                if let Some(source) = self.pending.get(name) {
                    return Err(conflict(
                        at,
                        &format!("binding `{text}` is used before its definition at {source}"),
                    ));
                }
                if let Some(source) = self.available.get(name) {
                    self.reads
                        .push(json!({"at":at,"binding":text,"definition":source}));
                } else {
                    self.deferred.insert(at.into());
                }
            }
            Value::Number(_) | Value::Bool(_) => {}
            Value::Object(object)
                if object.contains_key("value")
                    && object.keys().all(|key| key == "value" || key == "type") => {}
            Value::Array(items) if !items.is_empty() => {
                self.operation(
                    items[0].as_str(),
                    &items[1..],
                    at,
                    &|index| format!("{at}/{}", index + 1),
                    depth + 1,
                )?;
            }
            _ => {
                self.deferred.insert(at.into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sley_ssmc::TypeExpr;

    #[test]
    fn opcode_schema_and_aliases_distinguish_immediates_from_value_operands() {
        let inputs = Bindings::from([(
            "ready".into(),
            super::super::Binding::parameter(TypeExpr::Bool, "/params/0".into()),
        )]);
        let definitions = BTreeMap::from([
            ("out".into(), "/ops/0".into()),
            ("later".into(), "/ops/1".into()),
        ]);
        // Metadata-only fixtures: an immediate need not be a valid entity/index
        // for this test of operand positions. No kernel-validity claim is made.
        for row in opcodes::OPCODES {
            for alias in [
                row.mnemonic.to_owned(),
                row.name.to_owned(),
                row.tag.to_string(),
            ] {
                let ops = vec![
                    json!(["out", format!("{alias}?Route"), "later", "ready"]),
                    json!(["later", "not", "ready"]),
                ];
                let result = check(
                    &ops,
                    &inputs,
                    &Bindings::new(),
                    &definitions,
                    "/ops",
                    &mut Budget::default(),
                );
                if row.immediate == ImmediateKind::None {
                    let error = result.unwrap_err();
                    assert!(error.detail().contains("/ops/0/2"), "{alias}: {error}");
                } else {
                    let report = result.unwrap();
                    assert_eq!(
                        report["local_definition_order"], "checked",
                        "{alias}: {report}"
                    );
                    assert_eq!(report["reads"][0]["at"], "/ops/0/3");
                }
            }
        }
    }

    #[test]
    fn local_order_walk_keeps_shared_work_and_nesting_bounds() {
        let definitions = BTreeMap::from([("out".into(), "/ops/0".into())]);
        let inputs = Bindings::new();
        let ops = vec![json!(["out", "not", true])];
        let mut budget = Budget::limited(std::time::Duration::from_secs(2), 1);
        for _ in 0..2 {
            let error =
                check(&ops, &inputs, &inputs, &definitions, "/ops", &mut budget).unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
        }
        let mut nested = json!(true);
        for _ in 0..33 {
            nested = json!(["tuple", nested]);
        }
        let error = check(
            &[json!(["out", "tuple", nested])],
            &inputs,
            &inputs,
            &definitions,
            "/ops",
            &mut Budget::default(),
        )
        .unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
        assert!(error.detail().contains("nesting"), "{error}");
    }
}
