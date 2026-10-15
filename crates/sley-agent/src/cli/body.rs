//! Signature binding for headerless proposals; no source reconstruction.
use serde_json::{Value, json};
use sley_id::EntityId;
use sley_mutate::value::EntityBodyValue;
use std::collections::BTreeSet;

use super::{Head, Names, Program, Words, Workspace, name_map};
use crate::error::{AgentError, AgentErrorCode, Result, unknown_name, usage};

pub(super) struct Target {
    id: EntityId,
    header: Value,
}

impl Target {
    pub(super) fn select(
        words: &Words,
        workspace: &Workspace,
        head: &Head,
    ) -> Result<Option<Self>> {
        let Some(name) = words.value("--body") else {
            return Ok(None);
        };
        if words.value("--base-root").is_none() {
            return Err(usage(
                "--body requires --base-root from the selected function context",
            ));
        }
        let names = Names::build(head.program(), &name_map(workspace)?);
        Self::from_program(head.program(), &names, name).map(Some)
    }

    fn from_program(program: &Program, names: &Names, name: &str) -> Result<Self> {
        let id = names.resolve(name).ok_or_else(|| unknown_name(name))?;
        let Some(EntityBodyValue::Function(function)) = program.body(&id) else {
            return Err(usage("--body must name a live function"));
        };
        if names.name(&id) != name || !crate::names::is_identifier(name) {
            return Err(usage(
                "--body needs the exact function name from the context",
            ));
        }
        // Ordinary AF1 cannot restate these interfaces. Never erase them as
        // a side effect of a proposal whose author supplied no new header.
        if !function.type_parameters.is_empty() || !function.effects.as_slice().is_empty() {
            return Err(usage(
                "--body cannot restate a generic or effect-declaring function; use a targeted graph edit",
            ));
        }
        let mut params = Vec::new();
        let mut seen = BTreeSet::new();
        for id in &function.parameters {
            let Some(EntityBodyValue::Parameter(parameter)) = program.body(id) else {
                return Err(usage("selected function has an unavailable parameter"));
            };
            let name = names.leaf(id);
            if !crate::names::is_identifier(&name) || !seen.insert(name.clone()) {
                return Err(usage(
                    "selected parameter names are not unique authoring identifiers",
                ));
            }
            params.push(json!([
                name,
                crate::types::render(&parameter.value_type, names)
            ]));
        }
        Ok(Self {
            id,
            header: json!({"fn":name, "params":params,
            "returns":crate::types::render(&function.result_type, names)}),
        })
    }

    pub(super) fn wrap(&self, input: &mut Value, familiar: bool) -> Result<()> {
        let body = if familiar {
            input["fns"][0]["body"].clone()
        } else {
            input.clone()
        };
        if !body.is_array() {
            return Err(usage(
                "--body reads a JSON statement list, or headerless text with --familiar",
            ));
        }
        let mut function = self.header.clone();
        function["body"] = body;
        *input = json!({"af1":1,"afx":1,"fns":[function]});
        Ok(())
    }

    pub(super) fn verify(&self, before: &Program, after: &Program) -> Result<()> {
        let (Some(EntityBodyValue::Function(old)), Some(EntityBodyValue::Function(new))) =
            (before.body(&self.id), after.body(&self.id))
        else {
            return Err(changed());
        };
        if old.parameters != new.parameters
            || old.result_type != new.result_type
            || old.effects != new.effects
            || old.type_parameters != new.type_parameters
            || old.contracts != new.contracts
            || old.visibility != new.visibility
        {
            return Err(changed());
        }
        for id in &old.parameters {
            if before.object(id).map(sley_mutate::EntityObject::object_id)
                != after.object(id).map(sley_mutate::EntityObject::object_id)
            {
                return Err(changed());
            }
        }
        Ok(())
    }
}

fn changed() -> AgentError {
    AgentError::new(
        AgentErrorCode::ProposalScope,
        "body-only lowering changed the selected interface; use an explicit function proposal",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::NameMap;
    use sley_mutate::value::{EntityIdSet, FunctionBody};

    fn program(body: FunctionBody) -> (Program, Names) {
        let epoch = crate::workspace::epoch().unwrap();
        let object = sley_mutate::build_entity_object(
            epoch,
            &sley_mutate::EntityObjectRecord {
                entity_id: EntityId::from_bytes([1; 32]),
                body: EntityBodyValue::Function(body),
                label: Some("selected".into()),
                semantic_fingerprint: None,
            },
        )
        .unwrap();
        let program = Program::new(
            epoch,
            sley_id::StateRoot::from_bytes([2; 32]),
            sley_id::WorkspaceId::from_bytes([3; 32]),
            vec![object],
        );
        let names = Names::build(&program, &NameMap::default());
        (program, names)
    }

    fn function() -> FunctionBody {
        FunctionBody {
            type_parameters: Vec::new(),
            parameters: Vec::new(),
            result_type: sley_ssmc::TypeExpr::Bool,
            effects: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
            entry_block: EntityId::from_bytes([4; 32]),
            blocks: vec![EntityId::from_bytes([4; 32])],
            contracts: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
            visibility: sley_ssmc::Visibility::Private,
        }
    }

    #[test]
    fn unsupported_interfaces_are_refused_and_bound_interfaces_cannot_change() {
        let (before, names) = program(function());
        let target = Target::from_program(&before, &names, "selected").unwrap();
        target.verify(&before, &before).unwrap();
        let mut variants = Vec::new();
        let mut effect = function();
        effect.effects = EntityIdSet::from_unsorted(vec![EntityId::from_bytes([5; 32])]).unwrap();
        variants.push(effect);
        let mut generic = function();
        generic
            .type_parameters
            .push(sley_ssmc::TypeParameterDef { ordinal: 0 });
        variants.push(generic);
        for changed in &variants {
            let (after, names) = program(changed.clone());
            assert!(
                matches!(Target::from_program(&after, &names, "selected"), Err(error)
                if error.code() == AgentErrorCode::Usage && error.detail().contains("generic or effect-declaring"))
            );
        }
        let mut result = function();
        result.result_type = sley_ssmc::TypeExpr::Text;
        variants.push(result);
        let mut visible = function();
        visible.visibility = sley_ssmc::Visibility::Exported;
        variants.push(visible);
        let mut contract = function();
        contract.contracts =
            EntityIdSet::from_unsorted(vec![EntityId::from_bytes([6; 32])]).unwrap();
        variants.push(contract);
        for changed in variants {
            let (after, _) = program(changed);
            assert_eq!(
                target.verify(&before, &after).unwrap_err().code(),
                AgentErrorCode::ProposalScope
            );
        }
    }
}
