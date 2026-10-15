//! Load the exact draft artifacts covered by a captured binding. The bytes
//! actually consumed are checked, not merely files observed before/after them.

use serde_json::Value;

use super::{
    Binding, MAX_BOUND_ARTIFACT_BYTES, RuntimeIdentity, digest_bytes, read_names, read_required,
    stale,
};
use crate::draft::{Drafts, State};
use crate::error::{AgentError, AgentErrorCode, Result};
use crate::names::NameMap;
use crate::residual::{self, BaseRef, Operation, Request, edit, frontier::Budget};
use crate::workspace::{Head, STATE_DIR, Workspace};

/// An exact source revision, loaded without creating handles or claiming a
/// revision. Private fields prevent swapping its bytes, names or request after
/// capture. A later recheck detects drift; it is not a publication lock.
pub struct DraftSource {
    binding: Binding,
    input: Vec<u8>,
    request: Request,
    runtime: RuntimeIdentity,
    head: Head,
    frame: Value,
    status: Value,
    names: NameMap,
    candidate: Vec<u8>,
}

impl DraftSource {
    /// Capture a valid draft receipt for a literal edit or constrained derivation.
    /// The composition core independently validates the candidate itself.
    ///
    /// # Errors
    /// Refuses unsupported requests, incomplete/refused receipts, missing candidates,
    /// changed artifacts/names/heads, stale revisions and exhausted budgets.
    pub fn capture(
        workspace: &Workspace,
        input: &[u8],
        runtime: &RuntimeIdentity,
        budget: &mut Budget,
    ) -> Result<Self> {
        budget.checkpoint()?;
        let request = residual::parse_request(input)?;
        let BaseRef::Draft(reference) = &request.base else {
            return Err(edit_request(
                "draft edit source requires an explicit draft revision",
            ));
        };
        if request.operation != Operation::Edit && !residual::factored::applies(&request) {
            return Err(edit_request(
                "draft source requires an edit request or version-2 constrained derivation",
            ));
        }
        let revision = reference
            .revision
            .ok_or_else(|| edit_request("draft edit source requires an explicit revision"))?;
        let binding = Binding::capture(workspace, input, runtime)?;
        budget.checkpoint()?;
        let head = workspace.read_head()?;
        let snapshot = &binding.snapshot["workspace"];
        if snapshot["accepted_head"] != crate::hex::encode(head.transaction_id().as_bytes()) {
            return Err(stale("draft source head changed during loading"));
        }
        let selected = &snapshot["draft"];
        let directory = Drafts::read_only(workspace).revision_dir(&reference.handle, revision);
        let status = document(
            &directory.join("status.json"),
            "draft-status",
            &selected["status"],
        )?;
        if status["state"] != State::Valid.as_str() {
            return Err(AgentError::new(
                AgentErrorCode::DraftIncomplete,
                "draft graph edits currently require a valid candidate revision",
            ));
        }
        let frame = document(
            &directory.join("frame.json"),
            "draft-frame",
            &selected["frame"],
        )?;
        budget.checkpoint()?;
        let candidate = candidate_bytes(workspace, selected)?;
        let (names, names_snapshot) = read_names(workspace)?;
        if names_snapshot != snapshot["names"] {
            return Err(stale("draft source names changed during loading"));
        }
        budget.checkpoint()?;
        binding.recheck(workspace, input, runtime)?;
        budget.checkpoint()?;
        Ok(Self {
            binding,
            input: input.to_vec(),
            request,
            runtime: runtime.clone(),
            head,
            frame,
            status,
            names,
            candidate,
        })
    }

    /// Immutable evidence binding the request and loaded source artifacts.
    #[must_use]
    pub const fn binding(&self) -> &Binding {
        &self.binding
    }

    /// The exact layered authoring frame covered by the source receipt.
    #[must_use]
    pub const fn frame(&self) -> &Value {
        &self.frame
    }

    /// The exact source status, including test provenance and table metadata.
    #[must_use]
    pub const fn status(&self) -> &Value {
        &self.status
    }

    /// Recheck the source immediately before use or publication.
    ///
    /// # Errors
    /// Refuses any request dependency change, including copying the same
    /// logical head and draft files to another local directory.
    pub fn recheck(&self, workspace: &Workspace) -> Result<()> {
        self.binding.recheck(workspace, &self.input, &self.runtime)
    }

    /// Analyze a version-2 choice family against this exact draft's
    /// validated graph. Source values match sites; they never answer decisions.
    /// No complete candidate family is materialized and no tests are run.
    ///
    /// # Errors
    /// Refuses stale receipts, invalid source candidates, unsupported families,
    /// invalid domains, excessive coupled components and exhausted budgets.
    pub fn analyze_factors(
        &self,
        workspace: &Workspace,
        budget: &mut Budget,
    ) -> Result<residual::factored::Choices> {
        budget.checkpoint()?;
        self.recheck(workspace)?;
        if !residual::factored::applies(&self.request) {
            return Err(edit_request(
                "draft factoring requires a version-2 choice contract",
            ));
        }
        let authority = crate::candidate::Authority::of(&self.head)?;
        let output = crate::candidate::validate(&self.head, &authority, &self.candidate)?;
        budget.checkpoint()?;
        let program = crate::candidate::proposed_program(&self.head, &output).ok_or_else(|| {
            AgentError::new(
                AgentErrorCode::ResidualPreserve,
                "draft dependency source failed kernel validation",
            )
        })?;
        if program.objects().len() > edit::MAX_SOURCE_OBJECTS {
            return Err(AgentError::new(
                AgentErrorCode::ResidualLimit,
                "draft dependency source exceeds its object bound",
            ));
        }
        let assertions =
            edit::draft::receipt::check(&self.head, &program, &self.names, &self.frame, budget)?;
        let names = crate::names::Names::build(&program, &self.names);
        let mut choices =
            residual::factored::analyze_bound_draft(&self.request, &program, &names, budget)?;
        self.recheck(workspace)?;
        budget.checkpoint()?;
        choices.record_draft_source(serde_json::json!({
            "kind":"draft", "revision":self.request.value()["base"],
            "binding":self.binding.digest(),
            "candidate_sha256":crate::draft::sha256(&self.candidate),
            "kernel":"valid", "source_kernel_validations":1,
            "frame_assertions":assertions,
            "receipt_binding":"checked_before_and_after_dependency_analysis",
            "completion_kernel":"not_run", "public_checks":"not_run"
        }));
        Ok(choices)
    }

    /// Compose this request using only the captured candidate and names, with
    /// binding checks before and after work. Still no publication or claiming.
    ///
    /// # Errors
    /// Refuses unresolved choices, stale bindings, unsupported lenses and any
    /// ordinary compilation/kernel/preservation/budget failure.
    pub fn prepare(
        &self,
        workspace: &Workspace,
        budget: &mut Budget,
    ) -> Result<edit::draft::Prepared> {
        budget.checkpoint()?;
        self.recheck(workspace)?;
        if self.request.choices.is_some() {
            return Err(AgentError::new(
                AgentErrorCode::ResidualChoiceMissing,
                "resolve the declared relation before preparing a draft graph edit",
            ));
        }
        budget.checkpoint()?;
        let mut prepared = edit::draft::prepare(
            &self.head,
            &self.candidate,
            &self.names,
            &self.request,
            budget,
        )?;
        prepared.report["source_frame_assertions"] = edit::draft::receipt::check(
            &self.head,
            &prepared.source,
            &self.names,
            &self.frame,
            budget,
        )?;
        let mut retained = edit::draft::authoring::retain(
            self.head.program(),
            &crate::names::Names::build(self.head.program(), &self.names),
            &self.frame,
            &prepared.expansion.frame,
            budget,
        )?;
        let mut names = self.names.clone();
        names.extend(&prepared.compiled.names);
        prepared.report["composed_frame_assertions"] = if prepared.contract.no_change() {
            edit::draft::receipt::check(
                &self.head,
                &prepared.program,
                &names,
                &retained.frame,
                budget,
            )?
        } else {
            edit::draft::receipt::complete(
                &self.head,
                &prepared.source,
                &prepared.program,
                &names,
                &mut retained,
                budget,
            )?
        };
        prepared.report["inherited_constant_declarations"] =
            serde_json::json!(retained.inherited_constants.len());
        prepared.report["authoring_preservation"] =
            Value::from(if retained.inherited_constants.is_empty() {
                "reexpanded_only_selected_literal_immediates_changed"
            } else {
                "reexpanded_selected_literals_and_explicit_inherited_constants"
            });
        prepared.authoring = Some(retained);
        complete_metadata(&mut prepared, &self.names);
        self.recheck(workspace)?;
        budget.checkpoint()?;
        prepared.report["receipt_binding"] = Value::from("checked_before_and_after_composition");
        prepared.report["binding"] = Value::from(self.binding.digest());
        prepared.report["publication"] =
            Value::from("not_attempted; recheck_and_revision_claim_required");
        Ok(prepared)
    }
}

// A composed mutation list includes the source's tests and functions. Do not
// substitute metadata describing only the sparse literal delta at publication.
fn complete_metadata(prepared: &mut edit::draft::Prepared, map: &NameMap) {
    use sley_mutate::{MutationPayload, value::EntityBodyValue};
    let compiled = &mut prepared.compiled;
    let retained = prepared.authoring.as_ref().expect("retained source");
    let names = crate::names::Names::build(&prepared.program, map);
    for operation in &compiled.ops {
        let (MutationPayload::CreateEntity(body) | MutationPayload::ReplaceEntityVersion(body)) =
            &operation.payload
        else {
            continue;
        };
        match body {
            EntityBodyValue::Function(_) => compiled.functions.push(operation.target),
            EntityBodyValue::TestCase(_) => compiled.tests.push(operation.target),
            _ => {}
        }
    }
    for test in retained.expanded["tests"].as_array().into_iter().flatten() {
        if let Some(id) = test["name"].as_str().and_then(|name| names.resolve(name))
            && matches!(
                prepared.program.body(&id),
                Some(EntityBodyValue::TestCase(_))
            )
        {
            compiled.tests.push(id);
        }
    }
    compiled.tests.sort();
    compiled.tests.dedup();
    compiled.functions.sort();
    compiled.functions.dedup();
    if retained.frame.get("afx").is_some() {
        compiled.stats = retained.stats.clone();
        compiled
            .artifacts
            .push(("expanded.json".into(), retained.expanded.clone()));
        compiled
            .artifacts
            .push(("sourcemap.json".into(), retained.source_map.clone()));
        if let Some(ripple) = &retained.ripple {
            compiled
                .artifacts
                .push(("ripple.json".into(), ripple.clone()));
        }
    }
}

fn document(path: &std::path::Path, domain: &str, expected: &Value) -> Result<Value> {
    let bytes = read_required(path)?;
    if expected.as_str() != Some(digest_bytes(domain, &bytes).as_str()) {
        return Err(stale(format!(
            "{} differs from the captured draft receipt",
            path.display()
        )));
    }
    residual::strict_json_with_limit(&bytes, MAX_BOUND_ARTIFACT_BYTES)
}

fn candidate_bytes(workspace: &Workspace, selected: &Value) -> Result<Vec<u8>> {
    let handle = selected["candidate"]["handle"]
        .as_str()
        .filter(|handle| crate::candidate::is_handle(handle))
        .ok_or_else(|| edit_request("draft edit source has no captured candidate"))?;
    let path = workspace
        .dir()
        .join(STATE_DIR)
        .join("candidates")
        .join(format!("{handle}.hex"));
    let bytes = read_required(&path)?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| stale("draft candidate is not UTF-8 hex"))?;
    let stored = crate::hex::decode(text.trim())
        .ok_or_else(|| stale("draft candidate has malformed hex"))?;
    if selected["candidate"]["sha256"] != crate::draft::sha256(&stored) {
        return Err(stale(
            "loaded candidate differs from the captured draft receipt",
        ));
    }
    Ok(stored)
}

fn edit_request(detail: &str) -> AgentError {
    AgentError::new(AgentErrorCode::ResidualFragmentShape, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumed_document_must_match_the_selected_receipt_digest() {
        let directory = std::env::temp_dir().join(format!(
            "ghostweave-source-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("frame.json");
        let original = br#"{"af1":1}"#;
        let expected = Value::from(digest_bytes("draft-frame", original));
        std::fs::write(&path, br#"{"af1":1,"namespace":null}"#).unwrap();
        let error = document(&path, "draft-frame", &expected).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualBindingStale);
        std::fs::write(&path, original).unwrap();
        assert_eq!(document(&path, "draft-frame", &expected).unwrap()["af1"], 1);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
