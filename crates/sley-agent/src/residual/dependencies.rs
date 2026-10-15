//! Conservative dependencies for the bound integer-literal lens.
//!
//! Reuses the validator's complete typed reference graph, never executions or
//! just the static caller report. This is local graph dependency evidence,
//! not a claim about unknown external clients or task correctness.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sley_id::EntityId;
use sley_mutate::value::EntityBodyValue;
use sley_policy::complete_entities::project_complete_entities;
use sley_ssmc::{ConstData, TypeExpr};

use super::frontier::{Budget, factors::Problem};
use super::{BaseRef, Operation, Request, binding, edit};
use crate::error::{AgentError, AgentErrorCode, Result};
use crate::names::Names;
use crate::workspace::Program;

// Bounds around the existing synchronous projection call. Its own
// bounds still apply, and the shared deadline is checked after each returns.
const MAX_GRAPH_BYTES: usize = 8 * 1024 * 1024;
const MAX_EDGES: usize = 262_144;

/// Dependency evidence constructed only from exact typed objects and matches.
/// It cannot be loaded from an author's asserted completeness certificate.
pub struct DependencyGraph {
    bound_draft: bool,
    digest: String,
    fields: BTreeSet<String>,
    dependencies: Vec<Value>,
    widths: BTreeMap<String, TypeExpr>,
    report: Value,
}

impl DependencyGraph {
    /// Bounded local evidence; no full graph disclosure or execution claim.
    #[must_use]
    pub fn summary(&self) -> Value {
        self.report.clone()
    }

    /// Conjoins mandatory graph edges with an author's finite constraints.
    /// Every field must be an actual missing per-site value. Caller-supplied
    /// dependencies can join more components, never split verified ones.
    /// Re-extraction checks names, exact objects and the request before use.
    ///
    /// # Errors
    /// Refuses stale evidence, a mismatched field vocabulary, malformed finite
    /// constraints, non-integer/out-of-width values and aggregate exhaustion.
    /// Does not run public tests or prune rows.
    pub fn constrain(
        &self,
        program: &Program,
        names: &Names,
        request: &Request,
        declaration: &Value,
        budget: &mut Budget,
    ) -> Result<Problem> {
        let current = extract_graph(program, names, request, budget, self.bound_draft)?;
        if current.digest != self.digest {
            return Err(error(
                AgentErrorCode::ResidualBindingStale,
                "literal dependency graph or request changed; replan explicitly",
            ));
        }
        // Validate the original declaration before modifying it: do not mask
        // duplicate/unknown fields or caller-provided dependency errors.
        let encoded = encode(declaration)?;
        budget.charge(encoded.len())?;
        Problem::parse_with_budget(&encoded, budget)?;
        let supplied: BTreeSet<_> = declaration["fields"]
            .as_array()
            .ok_or_else(invariant)?
            .iter()
            .map(|field| {
                field["name"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(invariant)
            })
            .collect::<Result<_>>()?;
        if supplied != self.fields {
            return Err(error(
                AgentErrorCode::ResidualConstraintConflict,
                "factor fields must be exactly the missing per-target values",
            ));
        }
        let defs = crate::values::ProgramTypes { program, names };
        for table in declaration["constraints"]
            .as_array()
            .ok_or_else(invariant)?
        {
            for row in table["rows"].as_array().ok_or_else(invariant)? {
                for (path, value) in row.as_object().ok_or_else(invariant)? {
                    budget.charge(1)?;
                    if value.as_i64().is_none() && value.as_u64().is_none() {
                        return Err(error(
                            AgentErrorCode::ResidualFragmentShape,
                            "literal domains require JSON integers",
                        ));
                    }
                    crate::values::read(
                        value,
                        self.widths.get(path).ok_or_else(invariant)?,
                        &defs,
                        path,
                    )?;
                }
            }
        }
        let mut value = declaration.clone();
        let edges = value["dependencies"].as_array_mut().ok_or_else(invariant)?;
        let mut keys = edges
            .iter()
            .map(edge_key)
            .collect::<Result<BTreeSet<_>>>()?;
        for edge in &self.dependencies {
            if keys.insert(edge_key(edge)?) {
                edges.push(edge.clone());
            }
        }
        let encoded = encode(&value)?;
        budget.charge(encoded.len())?;
        let mut problem = Problem::parse_with_budget(&encoded, budget)?;
        problem.bind_context(&self.digest)?;
        budget.checkpoint()?;
        Ok(problem)
    }
}

/// Extracts conservative components for exact accepted-graph literal edits.
/// All typed references join regions except administrative membership, test
/// observations and value reads of immutable integer constants. The lens
/// preserves those constants, replacing selected loads by copy-on-write.
/// Global policy/dependency relationships join every site conservatively.
///
/// # Errors
/// Refuses other lenses/bases, malformed scopes, unmatched targets, incomplete
/// projection/reference inventories and shared resource exhaustion. Missing
/// authored values remain missing; source values are used only for matching.
pub fn extract(
    program: &Program,
    names: &Names,
    request: &Request,
    budget: &mut Budget,
) -> Result<DependencyGraph> {
    extract_graph(program, names, request, budget, false)
}

/// Called only after the bound draft loader validates its exact candidate.
pub(crate) fn extract_bound_draft(
    program: &Program,
    names: &Names,
    request: &Request,
    budget: &mut Budget,
) -> Result<DependencyGraph> {
    extract_graph(program, names, request, budget, true)
}

fn extract_graph(
    program: &Program,
    names: &Names,
    request: &Request,
    budget: &mut Budget,
    bound_draft: bool,
) -> Result<DependencyGraph> {
    budget.checkpoint()?;
    check_source(program, request, bound_draft)?;
    let missing = super::plan::explicit_decisions_with_budget(request, budget)?;
    let fields: BTreeSet<_> = missing
        .iter()
        .map(|field| {
            field["path"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(invariant)
        })
        .collect::<Result<_>>()?;
    if fields.is_empty() {
        return Err(error(
            AgentErrorCode::ResidualConstraintConflict,
            "dependency planning requires missing site decisions",
        ));
    }
    let LocalGraph {
        positions,
        mut parents,
        global,
        mut report,
    } = project_graph(program, budget)?;
    let mut groups = BTreeMap::<usize, Vec<String>>::new();
    let mut targets = Vec::new();
    let mut widths = BTreeMap::new();
    for name in edit::scope_names(request)? {
        budget.charge(program.objects().len())?;
        let (id, owner, width) = edit::inspect_site(program, names, name, budget)?;
        let path = format!("/bindings/values/{}", edit::pointer_key(name));
        if let Some(value) = request.bindings["values"].get(name) {
            crate::values::read(
                value,
                &width,
                &crate::values::ProgramTypes { program, names },
                &path,
            )?;
        }
        // A fixed site can still bridge other decisions through the full graph.
        // It is simply absent from the question vocabulary.
        if fields.contains(&path) {
            let component = if global.is_empty() {
                root(&mut parents, positions[&id])
            } else {
                0
            };
            widths.insert(path.clone(), width);
            groups.entry(component).or_default().push(path);
        }
        targets.push(json!({"name":name,"entity":hex(id),"function":hex(owner)}));
    }
    let mut components: Vec<_> = groups.into_values().collect();
    for component in &mut components {
        component.sort();
    }
    components.sort();
    let dependencies: Vec<_> = components
        .iter()
        .filter(|fields| fields.len() > 1)
        .map(|fields| json!({"kind":"binding","fields":fields}))
        .collect();
    let graph = report["graph"].clone();
    let digest = binding::canonical_digest(
        "literal-dependency-binding-v1",
        &json!({
            "graph":graph,"request":request.value(),"targets":targets,"components":components
        }),
    )?;
    let details = json!({"profile":if bound_draft {"draft-integer-literal-dependencies-v1"} else {"accepted-integer-literal-dependencies-v1"},
        "binding":digest,"components":components,"targets":targets,"dependencies":dependencies,
        "scope":"conservative local typed graph for copy-on-write integer-load edits",
        "external_clients":"not_modeled","task_correctness":"not_established",
        "row_validity":"not_checked","kernel":"not_run"});
    report
        .as_object_mut()
        .ok_or_else(invariant)?
        .extend(details.as_object().ok_or_else(invariant)?.clone());
    budget.checkpoint()?;
    Ok(DependencyGraph {
        bound_draft,
        digest,
        fields,
        dependencies,
        widths,
        report,
    })
}

fn check_source(program: &Program, request: &Request, bound_draft: bool) -> Result<()> {
    if request.operation != Operation::Edit
        || matches!(request.base, BaseRef::Draft(_)) != bound_draft
        || !request.bindings.get("values").is_some_and(Value::is_object)
    {
        return Err(error(
            AgentErrorCode::ResidualFragmentShape,
            "dependency extraction requires per-target integer_literal edits and the matching bound source route",
        ));
    }
    if let BaseRef::AcceptedRoot(root) = &request.base
        && root != program.root().as_bytes()
    {
        return Err(error(
            AgentErrorCode::ResidualBindingStale,
            "requested root differs from dependency source",
        ));
    }
    Ok(())
}

struct LocalGraph {
    positions: BTreeMap<EntityId, usize>,
    parents: Vec<usize>,
    global: BTreeSet<EntityId>,
    report: Value,
}

fn project_graph(program: &Program, budget: &mut Budget) -> Result<LocalGraph> {
    let objects = program.objects();
    if objects.len() > 65_535 {
        return Err(limit("dependency graph exceeds 65535 objects"));
    }
    let mut source_bytes = 0_usize;
    for object in objects {
        source_bytes = source_bytes
            .checked_add(object.stored_bytes().len())
            .filter(|bytes| *bytes <= MAX_GRAPH_BYTES)
            .ok_or_else(|| limit("dependency source graph exceeds 8 MiB"))?;
        budget.charge(object.stored_bytes().len())?;
    }
    let entities = project_complete_entities(objects).map_err(|failure| {
        error(
            AgentErrorCode::ResidualInconclusive,
            &format!("dependency projection refused: {failure}"),
        )
    })?;
    budget.checkpoint()?;
    if entities.reference_edges.len() > MAX_EDGES {
        return Err(limit("dependency inventory exceeds 262144 edges"));
    }
    let positions: BTreeMap<_, _> = objects
        .iter()
        .enumerate()
        .map(|(position, object)| (object.record().entity_id, position))
        .collect();
    let mut parents: Vec<_> = (0..objects.len()).collect();
    let mut exclusions = BTreeMap::<&str, usize>::new();
    let mut included = BTreeMap::<u32, usize>::new();
    let mut global = BTreeSet::new();
    for object in objects {
        let id = object.record().entity_id;
        match &object.record().body {
            EntityBodyValue::Workspace(workspace)
                if !workspace.contracts.as_slice().is_empty()
                    || !workspace.capability_requirements.as_slice().is_empty() =>
            {
                global.insert(id);
            }
            EntityBodyValue::DependencyBinding(_) => {
                global.insert(id);
            }
            EntityBodyValue::PolicyBinding(policy)
                if administrative(program.body(&policy.subject)) =>
            {
                global.insert(id);
            }
            _ => {}
        }
        budget.charge(1)?;
    }
    for &(dependent, dependency, kind) in &entities.reference_edges {
        budget.charge(1)?;
        let from = program.body(&dependent);
        let to = program.body(&dependency);
        // Frozen reference-graph tags: 1 membership, 3 value reference.
        // Every other relationship is retained, including future known tags.
        let excluded = if matches!(from, Some(EntityBodyValue::TestCase(_)))
            || matches!(to, Some(EntityBodyValue::TestCase(_)))
        {
            Some("test_observation")
        } else if kind == 1 && (administrative(from) || administrative(to)) {
            Some("administrative_membership")
        } else if kind == 3 && immutable_integer(to) {
            Some("preserved_integer_constant")
        } else {
            None
        };
        if let Some(reason) = excluded {
            *exclusions.entry(reason).or_default() += 1;
        } else {
            join(&mut parents, positions[&dependent], positions[&dependency]);
            *included.entry(kind).or_default() += 1;
        }
    }
    let graph = graph_digest(program)?;
    let report = json!({"graph":graph,"source_objects":objects.len(),"source_bytes":source_bytes,
        "typed_edges":entities.reference_edges.len(),"included_relationship_tags":included,
        "excluded_edges":exclusions,"global_coupling_entities":global.iter().map(|id| hex(*id)).collect::<Vec<_>>()});
    Ok(LocalGraph {
        positions,
        parents,
        global,
        report,
    })
}

fn graph_digest(program: &Program) -> Result<String> {
    let inventory: Vec<_> = program
        .objects()
        .iter()
        .map(|object| {
            json!([
                hex(object.record().entity_id),
                crate::hex::encode(object.object_id().as_bytes())
            ])
        })
        .collect();
    let graph = binding::canonical_digest(
        "literal-dependency-objects-v1",
        &json!({
            "root":crate::hex::encode(program.root().as_bytes()),
            "workspace":crate::hex::encode(program.workspace().as_bytes()),
            "epoch":crate::hex::encode(program.epoch().as_bytes()),"objects":inventory
        }),
    )?;
    Ok(graph)
}

fn administrative(body: Option<&EntityBodyValue>) -> bool {
    matches!(
        body,
        Some(
            EntityBodyValue::Workspace(_)
                | EntityBodyValue::Package(_)
                | EntityBodyValue::Namespace(_)
        )
    )
}

fn immutable_integer(body: Option<&EntityBodyValue>) -> bool {
    matches!(body, Some(EntityBodyValue::Constant(constant)) if matches!(
        (&constant.value.value_type, &constant.value.data),
        (TypeExpr::SInt(_), ConstData::SInt(_)) | (TypeExpr::UInt(_), ConstData::UInt(_))))
}

fn root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        parents[index] = parents[parents[index]];
        index = parents[index];
    }
    index
}

fn join(parents: &mut [usize], left: usize, right: usize) {
    let left = root(parents, left);
    let right = root(parents, right);
    parents[left.max(right)] = left.min(right);
}

fn hex(id: EntityId) -> String {
    crate::hex::encode(id.as_bytes())
}

fn encode(value: &Value) -> Result<Vec<u8>> {
    serde_json::to_vec(value)
        .map_err(|failure| error(AgentErrorCode::ResidualParse, &failure.to_string()))
}

fn error(code: AgentErrorCode, detail: &str) -> AgentError {
    AgentError::new(code, detail)
}
fn limit(detail: &str) -> AgentError {
    error(AgentErrorCode::ResidualLimit, detail)
}

fn invariant() -> AgentError {
    error(
        AgentErrorCode::ResidualInconclusive,
        "validated dependency schema invariant failed",
    )
}

fn edge_key(edge: &Value) -> Result<(String, Vec<String>)> {
    let kind = edge["kind"].as_str().ok_or_else(invariant)?.to_owned();
    let mut fields = edge["fields"]
        .as_array()
        .ok_or_else(invariant)?
        .iter()
        .map(|field| field.as_str().map(str::to_owned).ok_or_else(invariant))
        .collect::<Result<Vec<_>>>()?;
    fields.sort();
    Ok((kind, fields))
}
