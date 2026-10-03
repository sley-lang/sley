//! Entitlement from an explicitly authored, complete relation of bindings.
//!
//! The relation is itself the author's constraint. Its exact literal rows are
//! the whole permitted domain; no sampled search or test filtering constructs
//! it. This does not establish correctness for an external natural-language task.

use std::collections::BTreeMap;

use serde::{Serialize, Serializer, ser::SerializeMap};
use serde_json::{Map, Value, json};

use super::Request;
use super::frontier::{Budget, Family, Frontier, MAX_DESCRIPTIONS, encoding};
use crate::error::{AgentError, AgentErrorCode, Result};

/// A checked complete declaration and its separating projection.
pub struct Choices {
    template: Value,
    rows: Vec<Map<String, Value>>,
    family: Family,
    frontier: Frontier,
    questions: Vec<Value>,
}

/// A complete request plus auditable sources for every reconstructed field.
pub struct Resolution {
    /// Ordinary complete residual request, with the relation removed.
    pub request: Value,
    /// Exact missing-field paths mapped to authored/checked derived origins.
    pub origins: BTreeMap<String, Value>,
    /// Local reconstruction evidence, not authority or kernel evidence.
    pub evidence: Value,
}

struct FamilyInput<'a> {
    descriptions: &'a [Map<String, Value>],
    fields: &'a [Value],
}

impl Serialize for FamilyInput<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("descriptions", self.descriptions)?;
        map.serialize_entry("fields", self.fields)?;
        map.end()
    }
}

pub(crate) fn validate_contract(value: &Value) -> Result<()> {
    if value["version"].as_u64() == Some(2) {
        return super::factored::validate_contract(value);
    }
    let object = value.as_object().ok_or_else(|| {
        error(
            AgentErrorCode::ResidualParse,
            "choices must be a closed relation object",
        )
    })?;
    if object.len() != 3
        || !object.contains_key("version")
        || !object.contains_key("contract")
        || !object.contains_key("rows")
    {
        return Err(error(
            AgentErrorCode::ResidualParse,
            "choices requires exactly version, contract and rows",
        ));
    }
    if object["version"].as_u64() != Some(1) {
        return Err(error(
            AgentErrorCode::ResidualVersion,
            "choices version must be integer 1",
        ));
    }
    if object["contract"] != "author_closed_relation" {
        return Err(error(
            AgentErrorCode::ResidualFamilyIncomplete,
            "only an author_closed_relation declaration supplies a complete domain; sampled candidates do not",
        ));
    }
    let rows = object["rows"].as_array().ok_or_else(|| {
        error(
            AgentErrorCode::ResidualParse,
            "choices rows must be an array",
        )
    })?;
    if rows.is_empty() {
        return Err(error(
            AgentErrorCode::ResidualFamilyEmpty,
            "closed relation has no permitted completion",
        ));
    }
    if rows.len() > MAX_DESCRIPTIONS {
        return Err(error(
            AgentErrorCode::ResidualLimit,
            "closed relation exceeds 256 complete rows",
        ));
    }
    if rows.iter().any(|row| !row.is_object()) {
        return Err(error(
            AgentErrorCode::ResidualParse,
            "every choices row must map decision paths to values",
        ));
    }
    Ok(())
}

/// Checks the literal domain against the selected fragment's actual missing
/// fields and validates every row without pruning any alternative. Constructs
/// a greedy frontier with versioned byte-disclosure cost estimates.
///
/// # Errors
/// Refuses undeclared, empty, incomplete, duplicate, oversized or malformed
/// relations, answers to already-authored fields, incomplete fragment schemas,
/// and planning exhaustion. Bound interface checks, expansion, compilation and
/// kernel validity remain later checks; this analysis does not generate code.
pub fn analyze(request: &Request) -> Result<Choices> {
    analyze_with_budget(request, &mut Budget::default())
}

/// Checks a relation using the caller's aggregate invocation budget.
///
/// # Errors
/// The same refusals as [`analyze`], including exhaustion from earlier stages.
pub fn analyze_with_budget(request: &Request, budget: &mut Budget) -> Result<Choices> {
    budget.checkpoint()?;
    if super::factored::applies(request) {
        return Err(error(
            AgentErrorCode::ResidualInconclusive,
            "factored constraints require a bound typed graph; use residual plan or try",
        ));
    }
    let contract = request.choices.as_ref().ok_or_else(|| {
        error(
            AgentErrorCode::ResidualSemanticUnresolved,
            "no authored relation supplies the missing semantic values",
        )
    })?;
    validate_contract(contract)?;
    let mut template = request.value();
    template
        .as_object_mut()
        .ok_or_else(|| error(AgentErrorCode::ResidualParse, "request object required"))?
        .remove("choices");
    let plain = super::request_from_value(template.clone())?;
    let missing = super::plan::explicit_decisions_with_budget(&plain, budget)?;
    if missing.is_empty() {
        return Err(error(
            AgentErrorCode::ResidualConstraintConflict,
            "choices may bind only missing semantic fields; omit choices for a complete request",
        ));
    }
    let values = contract["rows"]
        .as_array()
        .ok_or_else(|| error(AgentErrorCode::ResidualParse, "rows array required"))?;
    let mut rows = Vec::new();
    for (index, row) in values.iter().enumerate() {
        budget.checkpoint()?;
        let row = row
            .as_object()
            .ok_or_else(|| error(AgentErrorCode::ResidualParse, "row object required"))?;
        // The ordinary fill checker enforces exact path coverage, forbids
        // overwrites and second-round holes, and preserves JSON types.
        let filled = super::plan::fill_with_budget(&template, row, budget)
            .map_err(|failure| row_error(index, &failure))?;
        super::request_from_value(filled).map_err(|failure| row_error(index, &failure))?;
        rows.push(row.clone());
    }
    let mut fields = Vec::new();
    let mut questions = Vec::new();
    for mut field in missing {
        budget.checkpoint()?;
        let path = field["path"]
            .as_str()
            .ok_or_else(|| error(AgentErrorCode::ResidualParse, "missing decision path"))?
            .to_owned();
        // Values are disclosed in full. These are byte proxies for the entire
        // field interface, not field counts or a provider tokenizer estimate.
        let domain = encoding::domain(
            rows.iter().map(|row| &row[&path]),
            budget,
            super::MAX_REQUEST_BYTES,
        )?;
        field["supported_values"] = Value::Array(domain.values().cloned().collect());
        drop(domain);
        field["contract"] = json!("/choices");
        let cost = encoding::size(&field, budget, super::MAX_REQUEST_BYTES)?
            .checked_add(1)
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or_else(|| {
                error(
                    AgentErrorCode::ResidualLimit,
                    "field disclosure cost exceeds u32",
                )
            })?;
        fields.push(json!({"name":path,"cost":cost,"eligible":true}));
        questions.push(field);
    }
    // Serialize borrowed rows directly. The encoded buffer stays reserved while
    // parsing/canonicalization acquires its own family and scratch reservations.
    let encoded = encoding::encode(
        &FamilyInput {
            descriptions: &rows,
            fields: &fields,
        },
        budget,
        super::MAX_REQUEST_BYTES,
    )?;
    let family = Family::parse_with_budget(&encoded.bytes, budget)?;
    drop(encoded);
    // The CLI uses deterministic greedy selection. Optional wall-limited exact
    // refinement remains a library operation until persistent frontier replay
    // can validate a saved selection without timing-dependent recomputation.
    let frontier = super::frontier::plan(&family, budget, false)?;
    questions.retain(|question| {
        frontier
            .fields()
            .iter()
            .any(|path| question["path"] == *path)
    });
    budget.checkpoint()?;
    Ok(Choices {
        template,
        rows,
        family,
        frontier,
        questions,
    })
}

impl Choices {
    /// Checks each declared completion in the caller's bound graph context.
    /// All rows are retained; failure never prunes or replaces a row.
    pub(crate) fn check_completions(
        &self,
        budget: &mut Budget,
        mut check: impl FnMut(&Request, &mut Budget) -> Result<()>,
    ) -> Result<()> {
        for (index, row) in self.rows.iter().enumerate() {
            let complete = super::plan::fill_with_budget(&self.template, row, budget)
                .map_err(|failure| row_error(index, &failure))?;
            let request = super::request_from_value(complete)
                .map_err(|failure| row_error(index, &failure))?;
            check(&request, budget).map_err(|failure| row_error(index, &failure))?;
        }
        Ok(())
    }
    /// All and only the distinguishing author questions, with complete domains.
    #[must_use]
    pub fn decisions(&self) -> Vec<Value> {
        self.questions.clone()
    }

    /// Explicitly scoped entitlement and cost model for plan inspection.
    #[must_use]
    pub fn summary(&self) -> Value {
        json!({"rule":"closed-author-relation-v1","contract":"/choices",
            "semantic_entitlement":"explicit author constraint plus checked reconstruction",
            "completeness":"exact literal author-declared relation","rows":self.rows.len(),
            "cost_model":"disclosed-field-json-bytes-v1","frontier":self.frontier.summary(),
            "task_correctness":"not_established","graph_validation":"not_asserted_by_reconstruction"})
    }

    /// Reconstructs a permitted row with provenance back to the literal relation
    /// and actual author answers. An empty answer succeeds only when the entire
    /// declared relation contains exactly one complete description.
    ///
    /// # Errors
    /// Refuses missing/unknown/out-of-family answers and any failed checked fill.
    pub fn resolve(
        &self,
        answers: &Map<String, Value>,
        source_document: &str,
        source_prefix: &str,
        filled: bool,
    ) -> Result<Resolution> {
        self.resolve_with_budget(
            answers,
            source_document,
            source_prefix,
            filled,
            &mut Budget::default(),
        )
    }

    /// Reconstructs under the caller's aggregate invocation budget.
    ///
    /// # Errors
    /// The same refusals as [`Self::resolve`], plus shared-budget exhaustion.
    pub fn resolve_with_budget(
        &self,
        answers: &Map<String, Value>,
        source_document: &str,
        source_prefix: &str,
        filled: bool,
        budget: &mut Budget,
    ) -> Result<Resolution> {
        budget.checkpoint()?;
        let row = self.frontier.decode(&self.family, answers)?;
        let index = self
            .rows
            .iter()
            .position(|candidate| *candidate == row)
            .ok_or_else(|| {
                error(
                    AgentErrorCode::ResidualInconclusive,
                    "decoded row is not in the declared relation",
                )
            })?;
        let request = super::plan::fill_with_budget(&self.template, &row, budget)?;
        let answers_origin: Vec<_> = answers.keys().map(|path| json!({"class":"AUTHORED","document":"residual-fill.json","pointer":format!("/choose/{}",escape(path))})).collect();
        let mut origins = BTreeMap::new();
        for path in row.keys() {
            budget.checkpoint()?;
            let origin = if filled && answers.contains_key(path) {
                json!({"class":"AUTHORED","document":"residual-fill.json","pointer":format!("/choose/{}",escape(path))})
            } else {
                json!({"class":"DERIVED","rule":"closed-author-relation-v1",
                    "contract":{"class":"AUTHORED","document":source_document,"pointer":format!("{source_prefix}/choices")},
                    "value_source":{"document":source_document,"pointer":format!("{source_prefix}/choices/rows/{index}/{}",escape(path))},
                    "answers":answers_origin,"family":self.frontier.summary()["family"]})
            };
            origins.insert(path.clone(), origin);
        }
        let evidence = json!({"summary":self.summary(),"selected_row":index,"origins":origins,"request":request});
        budget.checkpoint()?;
        Ok(Resolution {
            request,
            origins,
            evidence,
        })
    }
}

impl Resolution {
    /// Replaces apparent authored origins for reconstructed fields with their
    /// checked relation/answer origins. Existing fragment derivations retain
    /// all inputs and gain explicit origins for those decision dependencies.
    pub(crate) fn apply(&self, entries: &mut BTreeMap<String, Value>) {
        for origin in entries.values_mut() {
            if origin["class"] == "AUTHORED" {
                if let Some(replacement) = origin["pointer"]
                    .as_str()
                    .and_then(|path| self.origin(path))
                {
                    *origin = replacement;
                }
            } else if origin["class"] == "DERIVED"
                && let Some(inputs) = origin["inputs"].as_array()
            {
                let origins: Vec<_> = inputs
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|path| {
                        self.origin(path)
                            .unwrap_or_else(|| json!({"class":"AUTHORED","pointer":path}))
                    })
                    .collect();
                origin["decision_origins"] = json!(origins);
            }
        }
    }

    fn origin(&self, path: &str) -> Option<Value> {
        let (prefix, origin) = self
            .origins
            .iter()
            .filter(|(prefix, _)| {
                path == prefix.as_str()
                    || path
                        .strip_prefix(prefix.as_str())
                        .is_some_and(|tail| tail.starts_with('/'))
            })
            .max_by_key(|(prefix, _)| prefix.len())?;
        let mut origin = origin.clone();
        let suffix = &path[prefix.len()..];
        if origin["class"] == "AUTHORED" {
            origin["pointer"] = json!(format!("{}{suffix}", origin["pointer"].as_str()?));
        } else {
            origin["value_source"]["pointer"] = json!(format!(
                "{}{suffix}",
                origin["value_source"]["pointer"].as_str()?
            ));
        }
        Some(origin)
    }
}

fn escape(path: &str) -> String {
    path.replace('~', "~0").replace('/', "~1")
}
fn error(code: AgentErrorCode, detail: &str) -> AgentError {
    AgentError::new(code, detail)
}
fn row_error(index: usize, failure: &AgentError) -> AgentError {
    error(
        failure.code(),
        &format!("/choices/rows/{index}: {}", failure.detail()),
    )
}

#[cfg(test)]
mod tests;
