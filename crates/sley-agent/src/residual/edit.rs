//! Exact typed graph lenses. All emitted changes use the ordinary AF1 compiler.

pub mod draft;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};
use sley_id::EntityId;
use sley_mutate::MutationPayload;
use sley_mutate::value::{EntityBodyValue, OperationBody};
use sley_ssmc::{ConstValue, Immediate, TypeExpr, ValueRef};

use super::fragments::Expansion;
use super::frontier::Budget;
use super::{BaseRef, Operation, Request};
use crate::error::{AgentError, AgentErrorCode, Result};
use crate::frame::Compiled;
use crate::names::Names;
use crate::values::{self, ProgramTypes};
use crate::workspace::Program;

/// Maximum explicitly named sites in one literal edit.
pub const MAX_EDIT_TARGETS: usize = 64;
/// Bound on the typed graph inspected for consumers and caller impact.
pub const MAX_SOURCE_OBJECTS: usize = 65_536;

/// Exact, bounded target inventory shared by planning and expansion.
pub(crate) fn scope_names(request: &Request) -> Result<Vec<&str>> {
    let scope = request
        .scope
        .as_array()
        .ok_or_else(|| scope_error("scope must enumerate exact operation names"))?;
    if scope.is_empty() || scope.len() > MAX_EDIT_TARGETS {
        return Err(limit("scope needs 1..64 exact operations"));
    }
    let mut seen = BTreeSet::new();
    scope
        .iter()
        .map(|value| {
            let name = value
                .as_str()
                .filter(|name| name.split('.').all(crate::names::is_identifier))
                .ok_or_else(|| scope_error("each scope target must be an exact operation name"))?;
            if !seen.insert(name) {
                return Err(scope_error("duplicate scope target"));
            }
            Ok(name)
        })
        .collect()
}

/// A sparse expansion and the preservation contract checked before assembly.
pub struct Edit {
    /// Ordinary AF1 and decision provenance.
    pub expansion: Expansion,
    /// Exact allowed mutations and requested read-after-write values.
    pub contract: Contract,
}

#[derive(Clone)]
struct Target {
    id: EntityId,
    before: OperationBody,
    value: ConstValue,
    changed: bool,
}

/// A checked source match. A caller cannot construct this from unchecked JSON.
pub struct Contract {
    targets: Vec<Target>,
    report: Value,
}

impl Contract {
    /// All requested values already equal the selected source values.
    #[must_use]
    pub fn no_change(&self) -> bool {
        self.targets.iter().all(|target| !target.changed)
    }

    /// Full local source-version, preservation and behavioral-impact inventory.
    #[must_use]
    pub fn report(&self) -> Value {
        self.report.clone()
    }

    /// Checks the actual compiler mutations before candidate assembly.
    ///
    /// # Errors
    ///
    /// Refuses any unintended mutation, missing requested change, unrelated
    /// creation, or substitution of a different typed value.
    pub fn check(&self, program: &Program, compiled: &Compiled) -> Result<()> {
        let mut constants = BTreeMap::new();
        let mut replacements = BTreeMap::new();
        for mutation in &compiled.ops {
            match &mutation.payload {
                MutationPayload::CreateEntity(EntityBodyValue::Constant(value))
                    if !program.contains(&mutation.target) =>
                {
                    if constants.insert(mutation.target, &value.value).is_some() {
                        return Err(preserve("duplicate constant creation"));
                    }
                }
                MutationPayload::ReplaceEntityVersion(EntityBodyValue::Operation(operation)) => {
                    if replacements.insert(mutation.target, operation).is_some() {
                        return Err(preserve("duplicate operation replacement"));
                    }
                }
                _ => {
                    return Err(preserve(
                        "compiler proposed a mutation outside the literal lens",
                    ));
                }
            }
        }
        let changed: BTreeSet<_> = self
            .targets
            .iter()
            .filter(|target| target.changed)
            .map(|target| target.id)
            .collect();
        if replacements.keys().copied().collect::<BTreeSet<_>>() != changed {
            return Err(preserve(
                "compiler replacements differ from the exact changed target set",
            ));
        }
        let mut used = BTreeSet::new();
        for target in self.targets.iter().filter(|target| target.changed) {
            let operation = replacements[&target.id];
            check_operation(target, operation)?;
            let Immediate::Entity(constant) = operation.immediate else {
                unreachable!("checked constant load");
            };
            let value = if let Some(value) = constants.get(&constant) {
                used.insert(constant);
                (*value).clone()
            } else {
                constant_value(program, &constant)?
            };
            if value != target.value {
                return Err(preserve(
                    "read-after-write value differs from the requested typed integer",
                ));
            }
        }
        if used.len() != constants.len() {
            return Err(preserve("compiler created an unrelated constant"));
        }
        Ok(())
    }

    /// Checks the actual proposed graph, including byte-identical preservation
    /// of every existing non-target entity and of no-op targets.
    ///
    /// # Errors
    ///
    /// Refuses source-frame mutation, unrelated additions/deletions, or a
    /// requested value not present in the proposed graph.
    pub fn verify(&self, before: &Program, after: &Program) -> Result<()> {
        let changed: BTreeSet<_> = self
            .targets
            .iter()
            .filter(|target| target.changed)
            .map(|target| target.id)
            .collect();
        for object in before.objects() {
            let id = object.record().entity_id;
            let next = after
                .object(&id)
                .ok_or_else(|| preserve("an existing entity was deleted"))?;
            if !changed.contains(&id) && object.stored_bytes() != next.stored_bytes() {
                return Err(preserve(
                    "an entity outside the changed target set is not byte-identical",
                ));
            }
        }
        let mut constants = BTreeSet::new();
        for target in &self.targets {
            let Some(EntityBodyValue::Operation(operation)) = after.body(&target.id) else {
                return Err(preserve("a target operation is missing"));
            };
            check_operation(target, operation)?;
            let Immediate::Entity(constant) = operation.immediate else {
                unreachable!("checked constant load");
            };
            if constant_value(after, &constant)? != target.value {
                return Err(preserve("proposed graph fails read-after-write"));
            }
            constants.insert(constant);
        }
        for object in after
            .objects()
            .iter()
            .filter(|object| !before.contains(&object.record().entity_id))
        {
            if !matches!(object.record().body, EntityBodyValue::Constant(_))
                || !constants.contains(&object.record().entity_id)
            {
                return Err(preserve("proposed graph contains an unrelated new entity"));
            }
        }
        Ok(())
    }
}

fn check_operation(target: &Target, operation: &OperationBody) -> Result<()> {
    let mut expected = target.before.clone();
    if !matches!(operation.immediate, Immediate::Entity(_)) {
        return Err(preserve("a target ceased to be a constant load"));
    }
    expected.immediate = operation.immediate.clone();
    if expected != *operation {
        return Err(preserve(
            "a target's identity, position, opcode, operands or type changed",
        ));
    }
    Ok(())
}

/// Matches integer constant loads feeding checked arithmetic. Width and every
/// unchanged field are inherited from exact typed source objects. Equal values
/// emit no edit, avoiding even a switch to another equal constant identity.
///
/// # Errors
///
/// Refuses unknown lenses, non-matching shapes, unsupported bases, duplicate or
/// non-exact targets, out-of-range values, and missing preservation policies.
pub fn expand(program: &Program, names: &Names, request: &Request) -> Result<Edit> {
    expand_with_budget(program, names, request, &mut Budget::default())
}

/// Expands the literal lens using the caller's aggregate planning budget.
///
/// # Errors
/// The same refusals as [`expand`], including exhaustion during graph scans.
pub fn expand_with_budget(
    program: &Program,
    names: &Names,
    request: &Request,
    budget: &mut Budget,
) -> Result<Edit> {
    budget.checkpoint()?;
    if matches!(request.base, BaseRef::Draft(_)) {
        return Err(shape(
            "draft graph edits require a validated source candidate and identity-preserving composition",
        ));
    }
    expand_graph(program, names, request, budget)
}

fn expand_graph(
    program: &Program,
    names: &Names,
    request: &Request,
    budget: &mut Budget,
) -> Result<Edit> {
    budget.checkpoint()?;
    validate(request, program)?;
    let scope = scope_names(request)?;
    let values = request.bindings.get("values").and_then(Value::as_object);
    let overrides = request
        .bindings
        .get("overrides")
        .map(|value| {
            value
                .as_object()
                .ok_or_else(|| shape("overrides must map exact scope names to integer values"))
        })
        .transpose()?;
    let mut selected = BTreeSet::new();
    let mut targets = Vec::new();
    let mut edits = Vec::new();
    let mut provenance = BTreeMap::new();
    let mut inventory = Vec::new();
    let mut functions = BTreeSet::new();
    for (index, name) in scope.iter().copied().enumerate() {
        budget.checkpoint()?;
        let id = names
            .resolve(name)
            .filter(|id| names.name(id) == name)
            .ok_or_else(|| {
                scope_error("target must be an exact qualified name in the bound graph")
            })?;
        if !selected.insert(name) {
            return Err(scope_error("duplicate scope target"));
        }
        let (policy, policy_at) = if let Some(values) = values {
            let value = values
                .get(name)
                .ok_or_else(|| shape("values must specify every exact scope target"))?;
            (value, format!("/bindings/values/{}", pointer_key(name)))
        } else if let Some(value) = overrides.and_then(|overrides| overrides.get(name)) {
            (value, format!("/bindings/overrides/{}", pointer_key(name)))
        } else {
            (&request.bindings["value"], "/bindings/value".to_owned())
        };
        let (target, function, value, record) =
            match_target(program, names, id, policy, &policy_at, budget)?;
        functions.insert(function);
        if target.changed {
            let operation_at = format!("/edit/{}", edits.len());
            let source = json!({"class":"INHERITED","entity":record["entity"],"object":record["object"],"preserve":"/preserve"});
            provenance.insert(operation_at.clone(), source);
            provenance.insert(
                format!("{operation_at}/with/1/value"),
                json!({
                    "class":"AUTHORED", "pointer":policy_at
                }),
            );
            let block = target.before.block;
            edits.push(json!({"fn":names.name(&function),
                "replace_op":format!("{}.{}", names.leaf(&block), names.leaf(&id)),
                "with":["const",value]}));
        }
        inventory.push(
            json!({"scope_index":index,"name":name,"source":record,"changed":target.changed,
            "requested":values::to_json(&target.value,names)}),
        );
        targets.push(target);
    }
    if values.or(overrides).is_some_and(|policies| {
        policies
            .keys()
            .any(|name| !selected.contains(name.as_str()))
    }) {
        return Err(scope_error(
            "a value names an operation outside the explicit scope",
        ));
    }
    provenance.insert("/namespace".into(), json!({"class":"FRAGMENT_DEFINED","selection":"/fragment","rule":"literal-no-namespace-change-v1"}));
    provenance.insert(
        "/af1".into(),
        json!({"class":"FRAGMENT_DEFINED","selection":"/fragment","rule":"literal-edit-v1"}),
    );
    let report = json!({
        "lens":"integer_literal@1", "targets":inventory,
        "preserve":{"outside_targets":"byte-identical", "signatures":"byte-identical",
            "effects":"byte-identical", "control_flow":"byte-identical", "old_constants":"byte-identical"},
        "allowed_additions":"only integer constants read by changed target loads",
        "behavioral_preservation":"not_claimed",
        "boundaries":boundaries(program,names,&functions,budget)?,
    });
    budget.checkpoint()?;
    Ok(Edit {
        expansion: Expansion {
            frame: json!({"af1":1,"namespace":null,"edit":edits}),
            provenance,
        },
        contract: Contract { targets, report },
    })
}

fn validate(request: &Request, program: &Program) -> Result<()> {
    if request.operation != Operation::Edit
        || request.fragment.id != "checked_pipeline"
        || request.fragment.version != 1
    {
        return Err(AgentError::new(
            AgentErrorCode::ResidualFragmentUnknown,
            "this edit requires checked_pipeline@1",
        ));
    }
    if program.objects().len() > MAX_SOURCE_OBJECTS {
        return Err(limit("source graph exceeds the bounded lens inventory"));
    }
    if request.bindings.contains_key("values") {
        closed(&request.bindings, &["lens", "values"], &[])?;
        if !request.bindings["values"].is_object() {
            return Err(shape("values must map exact scope names to integer values"));
        }
    } else {
        closed(&request.bindings, &["lens", "value"], &["overrides"])?;
        if !request.bindings["value"].is_number() {
            return Err(shape("integer_literal default must be a JSON integer"));
        }
    }
    if request.bindings["lens"] != "integer_literal" {
        return Err(shape("unknown checked_pipeline edit lens"));
    }
    let preserve = request
        .preserve
        .as_ref()
        .and_then(Value::as_object)
        .ok_or_else(|| preserve("preserve must explicitly require outside_scope and boundaries"))?;
    if preserve.len() != 2
        || preserve.get("outside_scope").and_then(Value::as_bool) != Some(true)
        || preserve.get("boundaries").and_then(Value::as_bool) != Some(true)
    {
        return Err(self::preserve(
            "preserve must be exactly {outside_scope:true,boundaries:true}",
        ));
    }
    Ok(())
}

pub(crate) fn pointer_key(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

fn match_target(
    program: &Program,
    names: &Names,
    id: EntityId,
    policy: &Value,
    policy_at: &str,
    budget: &mut Budget,
) -> Result<(Target, EntityId, Value, Value)> {
    let Some(EntityBodyValue::Operation(operation)) = program.body(&id) else {
        return Err(shape("target is not an operation"));
    };
    let Immediate::Entity(constant) = operation.immediate else {
        return Err(shape("target is not a constant load"));
    };
    if operation.opcode != 1 || !operation.operands.is_empty() {
        return Err(shape("target must be a constant load"));
    }
    let old = constant_value(program, &constant)?;
    if !matches!(old.value_type, TypeExpr::SInt(_) | TypeExpr::UInt(_))
        || operation.result_types.as_slice() != [old.value_type.clone()]
    {
        return Err(shape(
            "target must load one integer of its unchanged source width",
        ));
    }
    let Some(EntityBodyValue::Block(block)) = program.body(&operation.block) else {
        return Err(shape("target has no live owning block"));
    };
    if !block.operations.contains(&id) {
        return Err(shape("target is not listed in its owning block"));
    }
    let mut consumers = Vec::new();
    for object in program.objects() {
        budget.charge(1)?;
        if let EntityBodyValue::Operation(consumer) = &object.record().body
            && (64..=71).contains(&consumer.opcode)
        {
            budget.charge(consumer.operands.len())?;
            if consumer.operands.iter().any(|operand| matches!(operand,
                ValueRef::OperationResult(result) if result.operation == id && result.result_index == 0))
            {
                consumers.push(object.record().entity_id);
            }
        }
    }
    if consumers.is_empty() {
        return Err(shape(
            "integer literal does not feed a checked arithmetic operation",
        ));
    }
    if !policy.is_number() {
        return Err(shape(
            "integer_literal values are JSON integers, never strings or booleans",
        ));
    }
    let defs = ProgramTypes { program, names };
    let value = values::read(policy, &old.value_type, &defs, policy_at)?;
    let typed = json!({"type":crate::types::render(&old.value_type,names),"value":policy});
    let record = json!({
        "entity":crate::hex::encode(id.as_bytes()),
        "object":crate::hex::encode(program.object(&id).expect("live operation").object_id().as_bytes()),
        "constant":crate::hex::encode(constant.as_bytes()),
        "constant_object":crate::hex::encode(program.object(&constant).expect("live constant").object_id().as_bytes()),
        "width":crate::types::render(&old.value_type,names), "previous":values::to_json(&old,names),
        "checked_consumers":consumers.iter().map(|id| names.name(id)).collect::<Vec<_>>(),
        "meaning":"changes every use of this load's value in its owning function",
    });
    Ok((
        Target {
            id,
            before: operation.clone(),
            changed: value != old,
            value,
        },
        block.function,
        typed,
        record,
    ))
}

fn constant_value(program: &Program, id: &EntityId) -> Result<ConstValue> {
    match program.body(id) {
        Some(EntityBodyValue::Constant(constant)) => Ok(constant.value.clone()),
        _ => Err(shape("constant load does not reference a live constant")),
    }
}

/// Matches a site using its existing value solely to inspect the lens shape.
/// This does not supply an answer to a missing author decision.
pub(crate) fn inspect_site(
    program: &Program,
    names: &Names,
    name: &str,
    budget: &mut Budget,
) -> Result<(EntityId, EntityId, TypeExpr)> {
    let id = names
        .resolve(name)
        .filter(|id| names.name(id) == name)
        .ok_or_else(|| scope_error("target must be an exact qualified name in the bound graph"))?;
    let Some(EntityBodyValue::Operation(operation)) = program.body(&id) else {
        return Err(shape("target is not an operation"));
    };
    let Immediate::Entity(constant) = operation.immediate else {
        return Err(shape("target is not a constant load"));
    };
    let old = constant_value(program, &constant)?;
    let (_, owner, _, _) = match_target(
        program,
        names,
        id,
        &values::to_json(&old, names),
        name,
        budget,
    )?;
    Ok((id, owner, old.value_type))
}

fn boundaries(
    program: &Program,
    names: &Names,
    functions: &BTreeSet<EntityId>,
    budget: &mut Budget,
) -> Result<Value> {
    let mut reverse: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();
    for object in program.objects() {
        budget.charge(1)?;
        if let EntityBodyValue::Operation(operation) = &object.record().body
            && let Immediate::Function(reference) = &operation.immediate
            && let Some(EntityBodyValue::Block(block)) = program.body(&operation.block)
        {
            reverse
                .entry(reference.function)
                .or_default()
                .insert(block.function);
        }
    }
    let mut affected = functions.clone();
    let mut pending: Vec<_> = functions.iter().copied().collect();
    while let Some(function) = pending.pop() {
        budget.charge(1)?;
        for caller in reverse.get(&function).into_iter().flatten() {
            budget.charge(1)?;
            if affected.insert(*caller) {
                pending.push(*caller);
            }
        }
    }
    let mut callers = Vec::new();
    let mut exported = Vec::new();
    for id in &affected {
        budget.charge(1)?;
        if !functions.contains(id) {
            callers.push(names.name(id));
        }
        if matches!(program.body(id), Some(EntityBodyValue::Function(function))
            if function.visibility == sley_ssmc::Visibility::Exported)
        {
            exported.push(names.name(id));
        }
    }
    let mut entrypoints = Vec::new();
    for object in program.objects() {
        budget.charge(1)?;
        if let EntityBodyValue::EntryPoint(entry) = &object.record().body
            && affected.contains(&entry.function)
        {
            entrypoints.push(names.name(&object.record().entity_id));
        }
    }
    budget.checkpoint()?;
    Ok(
        json!({"functions":functions.iter().map(|id| names.name(id)).collect::<Vec<_>>(),
        "possible_static_callers":callers,"exported":exported,"entry_points":entrypoints,
        "caller_bytes":"unchanged", "caller_behavior":"may_change",
        "dynamic_or_external_callers":"not_enumerated", "kernel_impact":"authoritative"}),
    )
}

fn closed(object: &Map<String, Value>, required: &[&str], optional: &[&str]) -> Result<()> {
    if object
        .keys()
        .any(|key| !required.contains(&key.as_str()) && !optional.contains(&key.as_str()))
    {
        return Err(shape("unknown lens parameter"));
    }
    if required.iter().any(|key| !object.contains_key(*key)) {
        return Err(AgentError::new(
            AgentErrorCode::ResidualChoiceMissing,
            "integer_literal is missing a required parameter for the selected value mode",
        ));
    }
    Ok(())
}
fn shape(message: &str) -> AgentError {
    AgentError::new(AgentErrorCode::ResidualFragmentShape, message)
}
fn preserve(message: &str) -> AgentError {
    AgentError::new(AgentErrorCode::ResidualPreserve, message)
}
fn scope_error(message: &str) -> AgentError {
    AgentError::new(AgentErrorCode::ResidualScope, message)
}
fn limit(message: &str) -> AgentError {
    AgentError::new(AgentErrorCode::ResidualLimit, message)
}
