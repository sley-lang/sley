//! Author-declared finite constraints over checked program dependencies.

mod derived;

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

use super::{
    Request,
    choices::Resolution,
    dependencies,
    frontier::{Budget, factors},
};
use crate::{
    error::{AgentError, AgentErrorCode, Result},
    names::Names,
    workspace::Program,
};

/// One fully checked constraint family and its deterministic question batch.
pub struct Choices {
    template: Value,
    contract: Value,
    problem: factors::Problem,
    frontier: factors::FactoredFrontier,
    questions: Vec<Value>,
    dependency_report: Value,
}

pub(crate) fn applies(request: &Request) -> bool {
    request
        .choices
        .as_ref()
        .is_some_and(|value| value["version"].as_u64() == Some(2))
}

pub(crate) fn validate_contract(value: &Value) -> Result<()> {
    let object = value.as_object().ok_or_else(invalid)?;
    if object.len() != 4
        || !["version", "contract", "constraints", "dependencies"]
            .iter()
            .all(|key| object.contains_key(*key))
    {
        return Err(invalid());
    }
    if value["version"].as_u64() != Some(2) || value["contract"] != "author_closed_constraints" {
        return Err(error(
            AgentErrorCode::ResidualFamilyIncomplete,
            "choices v2 requires author_closed_constraints, not sampled candidates",
        ));
    }
    // This pure schema check happens before any workspace is opened. No join,
    // graph scan, or search is performed during request parsing.
    factors::Problem::parse(&encode(&declaration(value)?)?)?;
    Ok(())
}

fn declaration(contract: &Value) -> Result<Value> {
    let mut names = BTreeSet::new();
    for table in array(&contract["constraints"])? {
        for field in array(&table["fields"])? {
            names.insert(field.as_str().ok_or_else(invalid)?);
        }
    }
    Ok(
        json!({"fields":names.into_iter().map(|name| json!({"name":name,"cost":1,"eligible":true})).collect::<Vec<_>>(),
        "constraints":contract["constraints"],"dependencies":contract["dependencies"]}),
    )
}

/// Analyzes explicit finite constraints under the caller's captured graph.
/// The caller must retain and recheck its workspace binding before publication.
///
/// # Errors
/// Refuses unmatched sites, non-exact fields, invalid widths, stale sources,
/// contradictions, oversized coupled components and aggregate exhaustion.
pub fn analyze(
    request: &Request,
    program: &Program,
    names: &Names,
    budget: &mut Budget,
) -> Result<Choices> {
    analyze_graph(request, program, names, budget, false)
}

pub(crate) fn analyze_bound_draft(
    request: &Request,
    program: &Program,
    names: &Names,
    budget: &mut Budget,
) -> Result<Choices> {
    analyze_graph(request, program, names, budget, true)
}

fn analyze_graph(
    request: &Request,
    program: &Program,
    names: &Names,
    budget: &mut Budget,
    bound_draft: bool,
) -> Result<Choices> {
    budget.checkpoint()?;
    let contract = request.choices.as_ref().ok_or_else(invalid)?;
    validate_contract(contract)?;
    let mut template = request.value();
    template
        .as_object_mut()
        .ok_or_else(invalid)?
        .remove("choices");
    let mut declaration = declaration(contract)?;
    let mut questions = question_descriptors(request, contract, budget)?;
    let mut fields = Vec::new();
    for question in &questions {
        let cost = encode(question)?
            .len()
            .checked_add(1)
            .and_then(|cost| u32::try_from(cost).ok())
            .ok_or_else(invalid)?;
        fields.push(json!({"name":question["path"],"cost":cost,"eligible":true}));
    }
    declaration["fields"] = json!(fields);
    let (problem, frontier, dependency_report) = if request.operation == super::Operation::Derive {
        derived::analyze(
            program,
            names,
            request,
            &template,
            &declaration,
            budget,
            bound_draft,
        )?
    } else {
        let graph = if bound_draft {
            dependencies::extract_bound_draft(program, names, request, budget)?
        } else {
            dependencies::extract(program, names, request, budget)?
        };
        let problem = graph.constrain(program, names, request, &declaration, budget)?;
        let frontier = factors::plan(&problem, budget, false)?;
        (problem, frontier, graph.summary())
    };
    questions.retain(|question| {
        frontier
            .fields()
            .iter()
            .any(|field| question["path"] == *field)
    });
    budget.checkpoint()?;
    Ok(Choices {
        template,
        contract: contract.clone(),
        problem,
        frontier,
        questions,
        dependency_report,
    })
}

fn question_descriptors(
    request: &Request,
    contract: &Value,
    budget: &mut Budget,
) -> Result<Vec<Value>> {
    let mut questions = super::plan::explicit_decisions_with_budget(request, budget)?;
    for question in &mut questions {
        let path = question["path"].as_str().ok_or_else(invalid)?;
        let mut values = BTreeMap::new();
        for table in array(&contract["constraints"])? {
            for row in array(&table["rows"])? {
                budget.charge(1)?;
                if let Some(value) = row.get(path) {
                    values.insert(encode(value)?, value.clone());
                }
            }
        }
        question
            .as_object_mut()
            .ok_or_else(invalid)?
            .remove("supported_values");
        question["declared_values"] = json!(values.into_values().collect::<Vec<_>>());
        question["subject_to"] = json!("/choices/constraints");
        question["domain_note"] =
            json!("Literal table values; all declared constraints must hold together.");
    }
    Ok(questions)
}

impl Choices {
    pub(crate) fn record_draft_source(&mut self, evidence: Value) {
        self.dependency_report["source"] = evidence;
    }

    /// The complete single question batch, using real missing-site paths.
    #[must_use]
    pub fn decisions(&self) -> Vec<Value> {
        self.questions.clone()
    }

    /// Full local typed dependency evidence, excluded from compact reports.
    #[must_use]
    pub fn dependencies(&self) -> Value {
        self.dependency_report.clone()
    }

    /// Compact reconstruction guarantees, explicitly separate from validity.
    #[must_use]
    pub fn summary(&self) -> Value {
        let frontier = self.frontier.summary();
        json!({"rule":"closed-author-constraints-v1","contract":"/choices",
            "semantic_entitlement":"explicit author constraints plus checked reconstruction",
            "completeness":"exact conjunction of author-declared finite tables",
            "cost_model":"disclosed-field-json-bytes-v1",
            "dependency_profile":self.dependency_report["profile"],"dependency_binding":self.dependency_report["binding"],
            "components":frontier["components"].as_array().map(Vec::len),
            "description_count_decimal":frontier["description_count_decimal"],
            "count_exceeds_u128":frontier["count_exceeds_u128"],
            "materialized_component_rows":frontier["materialized_component_rows"],
            "global_product_materialized":false,"method":frontier["method"],"additive_cost":frontier["additive_cost"],
            "task_correctness":"not_established","external_clients":"not_modeled",
            "graph_validation":"not_asserted_by_reconstruction"})
    }

    /// Reconstructs a unique completion and records exact literal/answer sources.
    ///
    /// # Errors
    /// Refuses incomplete, extra or incompatible answers and exhausted budgets.
    pub fn resolve(
        &self,
        answers: &Map<String, Value>,
        source_document: &str,
        source_prefix: &str,
        filled: bool,
        budget: &mut Budget,
    ) -> Result<Resolution> {
        budget.checkpoint()?;
        let row = self.frontier.decode(&self.problem, answers)?;
        let request = super::plan::fill_with_budget(&self.template, &row, budget)?;
        let (sources, selected) =
            self.literal_sources(&row, source_document, source_prefix, budget)?;
        let frontier_summary = self.frontier.summary();
        let answer_sources: Vec<_> = answers.keys().map(|path| answer_origin(path)).collect();
        let mut origins = BTreeMap::new();
        for path in row.keys() {
            let origin = if filled && answers.contains_key(path) {
                answer_origin(path)
            } else {
                json!({"class":"DERIVED","rule":"closed-author-constraints-v1",
                "contract":{"class":"AUTHORED","document":source_document,"pointer":format!("{source_prefix}/choices")},
                "value_source":sources.get(path).ok_or_else(invalid)?,"answers":answer_sources,
                "problem":frontier_summary["problem"],"dependency_binding":self.dependency_report["binding"]})
            };
            origins.insert(path.clone(), origin);
        }
        let evidence = json!({"summary":self.summary(),"selected_constraint_rows":selected,
            "origins":origins,"request":request,"dependencies":self.dependency_report,
            "frontier":frontier_summary});
        budget.checkpoint()?;
        Ok(Resolution {
            request,
            origins,
            evidence,
        })
    }

    fn literal_sources(
        &self,
        row: &Map<String, Value>,
        document: &str,
        prefix: &str,
        budget: &mut Budget,
    ) -> Result<(BTreeMap<String, Value>, Vec<Value>)> {
        let mut sources = BTreeMap::new();
        let mut selected = Vec::new();
        for (table_index, table) in array(&self.contract["constraints"])?.iter().enumerate() {
            let mut found = None;
            for (row_index, candidate) in array(&table["rows"])?.iter().enumerate() {
                let candidate = candidate.as_object().ok_or_else(invalid)?;
                budget.charge(candidate.len())?;
                if candidate
                    .iter()
                    .all(|(key, value)| row.get(key) == Some(value))
                {
                    found = Some((row_index, candidate));
                    break;
                }
            }
            let (row_index, candidate) = found.ok_or_else(invalid)?;
            selected.push(json!({"constraint":table_index,"row":row_index}));
            for path in candidate.keys() {
                sources.entry(path.clone()).or_insert_with(|| json!({"document":document,
                    "pointer":format!("{prefix}/choices/constraints/{table_index}/rows/{row_index}/{}", super::edit::pointer_key(path))}));
            }
        }
        Ok((sources, selected))
    }
}

fn answer_origin(path: &str) -> Value {
    json!({"class":"AUTHORED","document":"residual-fill.json","pointer":format!("/choose/{}",super::edit::pointer_key(path))})
}
fn array(value: &Value) -> Result<&Vec<Value>> {
    value.as_array().ok_or_else(invalid)
}
fn encode(value: &Value) -> Result<Vec<u8>> {
    serde_json::to_vec(value)
        .map_err(|failure| error(AgentErrorCode::ResidualParse, &failure.to_string()))
}
fn invalid() -> AgentError {
    error(
        AgentErrorCode::ResidualParse,
        "choices v2 requires exact finite constraints and dependencies",
    )
}
fn error(code: AgentErrorCode, detail: &str) -> AgentError {
    AgentError::new(code, detail)
}
