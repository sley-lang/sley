//! Bound previews and a single explicit answer round, using ordinary trials.

use super::{
    AgentError, AgentErrorCode, Binding, Global, Head, Result, TrialOptions, Value, Words,
    Workspace, Write, attach_budget, attach_resolution, authoring, execute_reported, input, json,
    prepare, print, usage,
};

/// One analysis and one budget for the invocation's planning stages.
pub(super) struct Inspection {
    pub request: authoring::Request,
    pub decisions: Vec<Value>,
    pub relation: Option<authoring::choices::Choices>,
    pub factors: Option<authoring::factored::Choices>,
    pub binding: Option<Binding>,
    pub budget: authoring::frontier::Budget,
}

impl Inspection {
    pub fn new(input: &[u8]) -> Result<Self> {
        let request = authoring::parse_request(input)?;
        // Build hashing is startup/binding work, not frontier search. Initialize
        // it before starting the shared planning clock, including in debug builds.
        authoring::runtime::identity()?;
        Self::with_budget(request, authoring::frontier::Budget::default())
    }

    fn with_budget(
        request: authoring::Request,
        mut budget: authoring::frontier::Budget,
    ) -> Result<Self> {
        if request.choices.is_none() {
            budget.use_fast_path()?;
        }
        budget.checkpoint()?;
        let relation = request
            .choices
            .as_ref()
            .filter(|_| !authoring::factored::applies(&request))
            .map(|_| authoring::choices::analyze_with_budget(&request, &mut budget))
            .transpose()?;
        let decisions = match &relation {
            Some(relation) => relation.decisions(),
            None => authoring::plan::explicit_decisions_with_budget(&request, &mut budget)?,
        };
        budget.checkpoint()?;
        Ok(Self {
            request,
            decisions,
            relation,
            factors: None,
            binding: None,
            budget,
        })
    }

    /// Capture before looking at typed graph objects and retain this exact
    /// observation through publication or trial. Reentry never starts a new
    /// graph analysis or replaces a captured binding.
    pub fn bind(&mut self, workspace: &Workspace, head: &Head, input: &[u8]) -> Result<()> {
        if !authoring::factored::applies(&self.request) {
            return Ok(());
        }
        self.budget.checkpoint()?;
        let runtime = authoring::runtime::identity()?;
        if let Some(binding) = &self.binding {
            binding.recheck(workspace, input, &runtime)?;
            return check_head(binding, head);
        }
        let binding = Binding::capture(workspace, input, &runtime)?;
        check_head(&binding, head)?;
        let factors = if matches!(self.request.base, authoring::BaseRef::Draft(_)) {
            let source = authoring::binding::DraftSource::capture(
                workspace,
                input,
                &runtime,
                &mut self.budget,
            )?;
            if source.binding().to_json() != binding.to_json() {
                return Err(stale("draft dependency receipt changed during capture"));
            }
            source.analyze_factors(workspace, &mut self.budget)?
        } else {
            let names =
                crate::names::Names::build(head.program(), &super::super::name_map(workspace)?);
            authoring::factored::analyze(&self.request, head.program(), &names, &mut self.budget)?
        };
        binding.recheck(workspace, input, &runtime)?;
        self.decisions = factors.decisions();
        self.factors = Some(factors);
        self.binding = Some(binding);
        self.budget.checkpoint()
    }

    /// Expose source validation separately from validation of a selected
    /// completion. Cached reentry only rechecks the original binding.
    pub fn bind_reported(
        &mut self,
        global: &Global,
        workspace: &Workspace,
        head: &Head,
        input: &[u8],
    ) -> Result<()> {
        let draft = authoring::factored::applies(&self.request)
            && matches!(self.request.base, authoring::BaseRef::Draft(_));
        if draft && self.binding.is_none() {
            global.note(
                "residual",
                json!({"draft_dependency_analysis":true,
                "kernel_validation_count_incomplete":true}),
            );
        }
        self.bind(workspace, head, input)?;
        if draft {
            global.note(
                "residual",
                json!({"draft_dependency_analysis":true,
                "source_kernel_validations":1,"kernel_validation_count_incomplete":false}),
            );
        }
        Ok(())
    }

    pub fn resolve(
        &mut self,
        answers: &serde_json::Map<String, Value>,
        document: &str,
        prefix: &str,
        filled: bool,
    ) -> Result<Option<authoring::choices::Resolution>> {
        self.budget.checkpoint()?;
        if let Some(factors) = &self.factors {
            return factors
                .resolve(answers, document, prefix, filled, &mut self.budget)
                .map(Some);
        }
        self.relation
            .as_ref()
            .map(|relation| {
                relation.resolve_with_budget(answers, document, prefix, filled, &mut self.budget)
            })
            .transpose()
    }
}

fn check_head(binding: &Binding, head: &Head) -> Result<()> {
    if binding.to_json()["snapshot"]["workspace"]["accepted_head"]
        != crate::hex::encode(head.transaction_id().as_bytes())
    {
        return Err(stale(
            "dependency graph head differs from the original binding; replan",
        ));
    }
    Ok(())
}

pub(super) fn plan_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = super::super::words(args, &[], &[])?;
    let [argument] = words.positional.as_slice() else {
        return Err(usage("residual plan <request.json>"));
    };
    let input = input(argument)?;
    let inspection = Inspection::new(&input)?;
    let workspace = super::super::workspace(global)?;
    let head = workspace.read_head()?;
    global.note("input_bytes", input.len());
    publish(
        global,
        &workspace,
        &head,
        &input,
        inspection,
        &TrialOptions::default(),
        out,
    )
}

pub(super) fn publish(
    global: &Global,
    workspace: &Workspace,
    head: &Head,
    input: &[u8],
    mut inspection: Inspection,
    options: &TrialOptions,
    out: &mut dyn Write,
) -> Result<i32> {
    inspection.bind_reported(global, workspace, head, input)?;
    let resolution = if inspection.decisions.is_empty() {
        inspection.resolve(&serde_json::Map::new(), "residual-request.json", "", false)?
    } else {
        None
    };
    let Inspection {
        request,
        decisions,
        relation,
        factors,
        binding,
        mut budget,
    } = inspection;
    budget.checkpoint()?;
    let runtime = authoring::runtime::identity()?;
    let binding = match binding {
        Some(binding) => binding,
        None => Binding::capture(workspace, input, &runtime)?,
    };
    check_head(&binding, head)?;
    let mut artifacts = serde_json::Map::new();
    if decisions.is_empty() {
        let proposal = prepare(
            global,
            workspace,
            head,
            &request,
            input.to_vec(),
            false,
            resolution.as_ref(),
            &mut budget,
        )?;
        budget.checkpoint()?;
        artifacts.extend(proposal.artifacts);
    } else {
        global.note("residual",json!({"try":false,"plan":true,"request_bytes":input.len(),"unresolved_fields":decisions.len()}));
        artifacts.insert(
            "residual-request.json".into(),
            authoring::strict_json(input)?,
        );
        artifacts.insert("residual-binding.json".into(), binding.to_json());
    }
    let contract = authoring::fragments::manifest()["families"]
        .as_array()
        .expect("manifest")
        .iter()
        .find(|family| family["id"] == request.fragment.id)
        .cloned()
        .expect("inspected fragment");
    let mut report = json!({
        "construction":if decisions.is_empty() {"complete"} else {"incomplete"},
        "kernel":"not_run","public_checks":"not_run","native_admission":"not_attempted",
        "binding":binding.digest(),"fragment":{"id":request.fragment.id,"version":request.fragment.version},
        "route":"explicit_author_decisions", "optimization":"none", "unresolved":decisions,
        "contract":contract, "question_rounds":u8::from(!decisions.is_empty()),
        "body_omitted":true,
        "escape":"Supply a complete revised residual request or use ordinary AF1-X; no source draft has changed.",
    });
    if let Some(graph) = artifacts.get("residual-draft-graph.json") {
        report["kernel"] = json!("valid");
        report["draft_graph"] = graph.clone();
        artifacts
            .get_mut("residual-provenance.json")
            .expect("provenance")["frame"] = json!("residual-retained-frame.json");
    }
    if let Some(relation) = &relation {
        let checks = relation_evidence(global, workspace, head, &request, relation, &mut budget)?;
        report["route"] = json!("closed_author_relation");
        report["optimization"] = json!("greedy_additive_byte_surrogate");
        report["entitlement"] = relation.summary();
        report["relation_omitted"] = json!(true);
        report["family_checks"] = checks;
    }
    if let Some(factors) = &factors {
        report["route"] = json!("closed_author_constraints");
        report["optimization"] = json!("factored_greedy_additive_byte_surrogate");
        report["entitlement"] = factors.summary();
        report["relation_omitted"] = json!(true);
        let (checks, dependencies) = factor_evidence(global, factors);
        report["family_checks"] = checks;
        artifacts.insert("residual-dependencies.json".into(), dependencies);
    }
    binding.recheck(workspace, input, &runtime)?;
    let options = options_json(options)?;
    budget.checkpoint()?;
    artifacts.insert("residual-budget.json".into(), budget.usage());
    let record = json!({"kind":"plan", "report":report, "artifacts":artifacts, "options":options});
    budget.checkpoint()?;
    // Persistence is outside preparation; close the observer before I/O.
    drop(budget);
    let handle = authoring::store::save(workspace, &record)?;
    report["plan"] = json!(handle);
    report["next"] = json!(format!("sley-agent residual fill {handle} decisions.json"));
    report["inspect"] = json!(format!("sley-agent residual show {handle} --decisions"));
    print(global, out, &report)?;
    Ok(super::super::EXIT_OK)
}

pub(super) fn fill_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = super::super::words(args, &[], &[])?;
    let [handle, argument] = words.positional.as_slice() else {
        return Err(usage("residual fill <rN@1> <decisions.json>"));
    };
    let input = input(argument)?;
    let choices = authoring::plan::parse_fill(&input, handle)?;
    let workspace = super::super::workspace(global)?;
    global.note("input_bytes", input.len());
    let record = authoring::store::load(&workspace, handle)?;
    if record["kind"] != "plan" {
        return Err(usage("that residual record is not a plan"));
    }
    let original = &record["artifacts"]["residual-request.json"];
    let original_bytes = serde_json::to_vec(original).expect("JSON request");
    let mut inspection = Inspection::new(&original_bytes)?;
    let runtime = authoring::runtime::identity()?;
    let binding = Binding::capture(&workspace, &original_bytes, &runtime)?;
    if binding.to_json() != record["artifacts"]["residual-binding.json"] {
        return Err(stale("plan dependencies changed; explicitly replan"));
    }
    let head = workspace.read_head()?;
    inspection.bind_reported(global, &workspace, &head, &original_bytes)?;
    binding.recheck(&workspace, &original_bytes, &runtime)?;
    if json!(inspection.decisions) != record["report"]["unresolved"] {
        return Err(stale("plan decision inventory differs from its request"));
    }
    let resolution = inspection.resolve(
        &choices,
        "residual-plan.json",
        "/artifacts/residual-request.json",
        true,
    )?;
    let filled = match &resolution {
        Some(resolution) => resolution.request.clone(),
        None => authoring::plan::fill_with_budget(original, &choices, &mut inspection.budget)?,
    };
    let bytes = serde_json::to_vec(&filled).expect("filled JSON");
    let request = authoring::parse_request(&bytes)?;
    let mut proposal = prepare(
        global,
        &workspace,
        &head,
        &request,
        bytes,
        true,
        None,
        &mut inspection.budget,
    )?;
    proposal.residual.as_mut().expect("prepared").parent = Some((binding, original_bytes));
    proposal
        .artifacts
        .push(("residual-plan.json".into(), record.clone()));
    proposal
        .artifacts
        .push(("residual-fill.json".into(), authoring::strict_json(&input)?));
    if let Some(resolution) = &resolution {
        attach_resolution(&mut proposal, resolution)?;
    }
    let options = read_options(&record["options"])?;
    proposal
        .residual
        .as_ref()
        .expect("prepared")
        .recheck(&workspace, &head, &proposal.input)?;
    // Consume only after strict fill/schema validation and successful expansion.
    // The ordinary trial still arbitrates source draft revisions and kernel
    // preconditions. The atomic marker also excludes duplicate current-base fills.
    attach_budget(&mut proposal, &mut inspection.budget)?;
    drop(inspection);
    authoring::store::claim_fill(&workspace, handle, &input)?;
    execute_reported(global, &workspace, &head, proposal, &options, out)
}

fn factor_evidence(global: &Global, factors: &authoring::factored::Choices) -> (Value, Value) {
    let dependencies = factors.dependencies();
    let mut checks = json!({"inherited_widths":"all_declared_values_passed",
        "compiler":"not_run","compilation_policy":"selected_completion_on_trial","kernel":"not_run"});
    if let Some(preflight) = dependencies.get("interface_preflight") {
        checks
            .as_object_mut()
            .expect("object")
            .remove("inherited_widths");
        checks["interface_preflight"] = preflight.clone();
    }
    if dependencies["source"]["kind"] == "draft" {
        checks["source_kernel"] = json!("valid");
        checks["source_kernel_validations"] = json!(1);
        let mut event = global.event.borrow().line(0, "residual", 0)["residual"].clone();
        event["source_kernel_validations"] = json!(1);
        global.note("residual", event);
    }
    (checks, dependencies)
}

fn relation_evidence(
    global: &Global,
    workspace: &Workspace,
    head: &Head,
    request: &authoring::Request,
    relation: &authoring::choices::Choices,
    budget: &mut authoring::frontier::Budget,
) -> Result<Value> {
    let draft_edit = request.operation == authoring::Operation::Edit
        && matches!(request.base, authoring::BaseRef::Draft(_));
    let mut event = global.event.borrow().line(0, "residual", 0)["residual"].clone();
    if draft_edit {
        event["draft_graph_preparation"] = json!(true);
        event["kernel_validation_count_incomplete"] = json!(true);
        global.note("residual", event.clone());
    }
    let validations = validate_relation(workspace, head, request, relation, budget)?;
    let mut checks = json!({"compiler":"all_rows_passed","kernel":"not_run"});
    if draft_edit {
        checks["kernel"] = json!("all_rows_valid");
        checks["kernel_validations"] = json!(validations);
        event["kernel_validations"] =
            json!(event["kernel_validations"].as_u64().unwrap_or(0) + validations as u64);
        event["kernel_validation_count_incomplete"] = json!(false);
        global.note("residual", event);
    }
    Ok(checks)
}

fn validate_relation(
    workspace: &Workspace,
    head: &Head,
    request: &authoring::Request,
    relation: &authoring::choices::Choices,
    budget: &mut authoring::frontier::Budget,
) -> Result<usize> {
    budget.checkpoint()?;
    let names = crate::names::Names::build(head.program(), &super::super::name_map(workspace)?);
    budget.checkpoint()?;
    let authority = crate::candidate::Authority::of(head)?;
    budget.checkpoint()?;
    let base_frame = if let authoring::BaseRef::Draft(reference) = &request.base {
        Some(
            super::super::layer_base(
                &crate::draft::Drafts::read_only(workspace),
                &reference.handle,
                reference.revision.expect("explicit revision"),
            )?
            .0,
        )
    } else {
        None
    };
    let empty = serde_json::Map::new();
    let declarations = base_frame
        .as_ref()
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let mut validations = 0;
    relation.check_completions(budget, |complete, budget| {
        budget.checkpoint()?;
        if complete.operation == authoring::Operation::Edit
            && matches!(complete.base, authoring::BaseRef::Draft(_))
        {
            let input = serde_json::to_vec(&complete.value()).expect("complete row");
            let source = authoring::binding::DraftSource::capture(
                workspace,
                &input,
                &authoring::runtime::identity()?,
                budget,
            )?;
            source.prepare(workspace, budget)?;
            validations += 2;
            return budget.checkpoint();
        }
        if complete.operation == authoring::Operation::Derive {
            authoring::interfaces::check(head.program(), &names, declarations, complete, budget)?;
        }
        let expansion = match complete.operation {
            authoring::Operation::Derive => {
                authoring::fragments::expand_with_budget(complete, budget)?
            }
            authoring::Operation::Edit => {
                authoring::edit::expand_with_budget(head.program(), &names, complete, budget)?
                    .expansion
            }
        };
        budget.checkpoint()?;
        let frame = if let Some(base) = &base_frame {
            crate::layer::layer(base, &expansion.frame)?
        } else {
            expansion.frame
        };
        budget.checkpoint()?;
        crate::frame::compile(
            head.program(),
            &names,
            &authority.ceilings,
            &frame,
            crate::candidate::fresh_nonce()?,
            &mut crate::candidate::random32,
        )?;
        budget.checkpoint()
    })?;
    Ok(validations)
}

pub(super) fn show(
    global: &Global,
    handle: &str,
    record: &Value,
    words: &Words,
    out: &mut dyn Write,
) -> Result<i32> {
    let artifacts = &record["artifacts"];
    let mut value = if words.has("--expanded") {
        let expanded = &artifacts["residual-expanded.json"];
        json!({"expanded":expanded,"available":!expanded.is_null(),
            "unresolved":record["report"]["unresolved"],
            "representation":if expanded.is_null() {Value::Null} else if expanded.get("afx").is_some() {json!("AF1-X")} else {json!("AF1")}})
    } else if words.has("--decisions") {
        json!({"request":artifacts["residual-request.json"],"unresolved":record["report"]["unresolved"],"contract":record["report"]["contract"]})
    } else if words.has("--provenance") {
        json!({"binding":artifacts["residual-binding.json"],"provenance":artifacts["residual-provenance.json"],
            "dependencies":artifacts["residual-dependencies.json"],
            "interfaces":artifacts["residual-interfaces.json"],
            "resolution":artifacts["residual-resolution.json"], "planning_resources":artifacts["residual-budget.json"], "entitlement":record["report"]["entitlement"],
            "unresolved":record["report"]["unresolved"]})
    } else {
        record["report"].clone()
    };
    if !value.is_object() {
        return Err(usage("malformed plan report"));
    }
    value["plan"] = json!(handle);
    value["historical"] = json!(true);
    if words.flags.is_empty() {
        value["inspect"] = json!(format!("sley-agent residual show {handle} --decisions"));
        value["answer_context"] = json!(
            "Read --decisions to recover the complete author contract before answering from a new context."
        );
    }
    print(global, out, &value)?;
    Ok(super::super::EXIT_OK)
}

fn options_json(options: &TrialOptions) -> Result<Value> {
    let public = options
        .public
        .as_ref()
        .map(|path| {
            let path = std::path::Path::new(path);
            super::super::read_public(path)?;
            std::fs::canonicalize(path).map_err(|error| crate::error::io(path, &error))
        })
        .transpose()?;
    Ok(
        json!({"no_test":options.no_test,"all_tests":options.all_tests,"raw":options.raw,"verbose":options.verbose,"public":public}),
    )
}

fn read_options(value: &Value) -> Result<TrialOptions> {
    let object = value
        .as_object()
        .filter(|object| object.len() == 5)
        .ok_or_else(|| usage("malformed stored trial options"))?;
    let boolean = |key| {
        object
            .get(key)
            .and_then(Value::as_bool)
            .ok_or_else(|| usage("malformed stored trial option"))
    };
    let public = match object.get("public") {
        Some(Value::Null) => None,
        Some(Value::String(path)) => Some(path.clone()),
        _ => return Err(usage("malformed stored public-case path")),
    };
    Ok(TrialOptions {
        no_test: boolean("no_test")?,
        all_tests: boolean("all_tests")?,
        raw: boolean("raw")?,
        verbose: boolean("verbose")?,
        public,
    })
}

fn stale(detail: &str) -> AgentError {
    AgentError::new(AgentErrorCode::ResidualBindingStale, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    struct Fixture {
        path: std::path::PathBuf,
        workspace: Workspace,
    }

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ghostweave-planning-budget-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
            ));
            let workspace =
                crate::genesis::init(&path, Some([205; 32]), crate::genesis::INIT_CEILINGS)
                    .unwrap();
            Self { path, workspace }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn input() -> Vec<u8> {
        let mut request: Value = serde_json::from_str(
            crate::help::RESIDUAL
                .lines()
                .find(|line| line.starts_with("{\"residual\":"))
                .unwrap(),
        )
        .unwrap();
        request["bindings"]
            .as_object_mut()
            .unwrap()
            .remove("rounding");
        request["choices"] = json!({"version":1,"contract":"author_closed_relation",
            "rows":[{"/bindings/rounding":"toward_zero"}]});
        serde_json::to_vec(&request).unwrap()
    }

    #[test]
    fn scratch_memory_exhaustion_refuses_analysis_without_workspace_artifacts() {
        let fixture = Fixture::new();
        let before = fixture.workspace.read_head().unwrap().transaction_id();
        let error = Inspection::with_budget(
            authoring::parse_request(&input()).unwrap(),
            authoring::frontier::Budget::limited_with_memory(Duration::from_secs(2), u64::MAX, 1),
        )
        .err()
        .unwrap();
        assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
        assert!(error.detail().contains("planner memory"), "{error}");
        assert_eq!(
            fixture.workspace.read_head().unwrap().transaction_id(),
            before
        );
        assert!(!fixture.path.join(".sley").exists());
    }

    #[test]
    fn budget_is_not_reset_between_analysis_compilation_and_publication() {
        let fixture = Fixture::new();
        let head = fixture.workspace.read_head().unwrap();
        let input = input();
        let measured = Inspection::new(&input).unwrap();
        let work = measured.budget.usage()["charged_work"].as_u64().unwrap();
        let inspect = |extra| {
            Inspection::with_budget(
                authoring::parse_request(&input).unwrap(),
                authoring::frontier::Budget::limited(Duration::from_secs(2), work + extra),
            )
            .unwrap()
        };

        let mut inspection = inspect(3);
        let error = validate_relation(
            &fixture.workspace,
            &head,
            &inspection.request,
            inspection.relation.as_ref().unwrap(),
            &mut inspection.budget,
        )
        .unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
        assert!(error.detail().contains("/choices/rows/0"));

        let global = Global {
            json: true,
            ..Global::default()
        };
        let mut output = Vec::new();
        let error = publish(
            &global,
            &fixture.workspace,
            &head,
            &input,
            inspect(0),
            &TrialOptions::default(),
            &mut output,
        )
        .unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
        assert!(output.is_empty());
        assert!(
            !fixture.path.join(".sley").exists(),
            "exhaustion must not create a plan, candidate or draft"
        );
    }

    #[test]
    fn preparation_checks_the_shared_budget_at_each_stage_without_writes() {
        let fixture = Fixture::new();
        let head = fixture.workspace.read_head().unwrap();
        let mut value = authoring::strict_json(&input()).unwrap();
        value.as_object_mut().unwrap().remove("choices");
        value["bindings"]["rounding"] = json!("toward_zero");
        let input = serde_json::to_vec(&value).unwrap();
        let request = authoring::parse_request(&input).unwrap();
        authoring::runtime::identity().unwrap();
        let global = Global::default();
        let mut measured = authoring::frontier::Budget::default();
        prepare(
            &global,
            &fixture.workspace,
            &head,
            &request,
            input.clone(),
            false,
            None,
            &mut measured,
        )
        .unwrap();
        let work = measured.usage()["charged_work"].as_u64().unwrap();
        assert!(
            work >= 8,
            "binding, reconstruction, expansion and layering checkpoints"
        );
        for allowance in 0..work {
            let mut budget =
                authoring::frontier::Budget::limited(Duration::from_secs(2), allowance);
            let error = prepare(
                &global,
                &fixture.workspace,
                &head,
                &request,
                input.clone(),
                false,
                None,
                &mut budget,
            )
            .err()
            .expect("each stage must retain the caller's work ceiling");
            assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
            assert_eq!(budget.usage()["charged_work"], allowance);
            assert!(!fixture.path.join(".sley").exists());
        }
    }

    #[test]
    fn analysis_does_not_receive_a_new_budget_on_reentry() {
        let input = input();
        let request = authoring::parse_request(&input).unwrap();
        let mut measured = authoring::frontier::Budget::default();
        authoring::choices::analyze_with_budget(&request, &mut measured).unwrap();
        let work = measured.usage()["charged_work"].as_u64().unwrap();
        let mut budget = authoring::frontier::Budget::limited(Duration::from_secs(2), work + 1);
        authoring::choices::analyze_with_budget(&request, &mut budget).unwrap();
        let result = authoring::choices::analyze_with_budget(&request, &mut budget);
        assert_eq!(result.err().unwrap().code(), AgentErrorCode::ResidualLimit);
    }

    #[test]
    fn route_selects_one_shared_fast_or_frontier_ceiling() {
        let finite = input();
        let relation = Inspection::new(&finite).unwrap();
        assert_eq!(relation.budget.usage()["wall_limit_micros"], 2_000_000);
        assert_eq!(relation.budget.usage()["planning_route"], "finite_frontier");
        let mut plain = authoring::strict_json(&finite).unwrap();
        plain.as_object_mut().unwrap().remove("choices");
        for complete in [false, true] {
            if complete {
                plain["bindings"]["rounding"] = json!("toward_zero");
            }
            let bytes = serde_json::to_vec(&plain).unwrap();
            let inspection = Inspection::new(&bytes).unwrap();
            assert_eq!(inspection.budget.usage()["wall_limit_micros"], 250_000);
            assert_eq!(
                inspection.budget.usage()["planning_route"],
                "explicit_fast_path"
            );
            assert_eq!(inspection.decisions.is_empty(), complete);
        }
        let factored = Inspection::new(&factored_input()).unwrap();
        assert_eq!(factored.budget.usage()["wall_limit_micros"], 2_000_000);
        assert_eq!(factored.budget.usage()["planning_route"], "finite_frontier");
    }

    #[test]
    fn fast_deadline_spans_analysis_and_publication_without_creating_records() {
        let fixture = Fixture::new();
        let head = fixture.workspace.read_head().unwrap();
        let global = Global {
            json: true,
            ..Global::default()
        };
        let mut request = authoring::strict_json(&input()).unwrap();
        request.as_object_mut().unwrap().remove("choices");
        for complete in [false, true] {
            if complete {
                request["bindings"]["rounding"] = json!("toward_zero");
            }
            let bytes = serde_json::to_vec(&request).unwrap();
            let inspection = Inspection::new(&bytes).unwrap();
            // An actual wall-clock delay between stages must not receive the
            // frontier allowance or a new inventory-local 250 ms clock.
            std::thread::sleep(Duration::from_millis(260));
            let mut output = Vec::new();
            let failure = publish(
                &global,
                &fixture.workspace,
                &head,
                &bytes,
                inspection,
                &TrialOptions::default(),
                &mut output,
            )
            .unwrap_err();
            assert_eq!(failure.code(), AgentErrorCode::ResidualLimit);
            assert!(failure.detail().contains("fast-path"));
            assert!(output.is_empty());
            assert!(!fixture.path.join(".sley").exists());
        }
    }

    fn factored_input() -> Vec<u8> {
        serde_json::to_vec(&json!({"residual":1,"base":"current","operation":"edit",
            "fragment":{"id":"checked_pipeline","version":1},
            "bindings":{"lens":"integer_literal","values":{}},"scope":["adjust.entry.amount"],
            "preserve":{"outside_scope":true,"boundaries":true},
            "choices":{"version":2,"contract":"author_closed_constraints","dependencies":[],
                "constraints":[{"fields":["/bindings/values/adjust.entry.amount"],
                    "rows":[{"/bindings/values/adjust.entry.amount":3},{"/bindings/values/adjust.entry.amount":7}]}]}})).unwrap()
    }

    #[test]
    fn factored_graph_analysis_uses_the_original_invocation_budget() {
        let fixture = Fixture::new();
        let head = fixture.workspace.read_head().unwrap();
        let input = factored_input();
        let measured = Inspection::new(&input).unwrap();
        let work = measured.budget.usage()["charged_work"].as_u64().unwrap();
        let mut inspection = Inspection::with_budget(
            authoring::parse_request(&input).unwrap(),
            authoring::frontier::Budget::limited(Duration::from_secs(2), work + 1),
        )
        .unwrap();
        for _ in 0..2 {
            let failure = inspection
                .bind(&fixture.workspace, &head, &input)
                .unwrap_err();
            assert_eq!(failure.code(), AgentErrorCode::ResidualLimit);
            assert!(inspection.binding.is_none());
        }
        assert!(!fixture.path.join(".sley").exists());
    }

    #[test]
    fn factored_publication_cannot_replace_the_binding_captured_before_graph_analysis() {
        let fixture = Fixture::new();
        let frame = json!({"af1":1,"fns":[{"fn":"adjust","params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>",
            "blocks":[{"name":"entry","ops":[["amount","const",{"type":"i8","value":3}],
            ["sum","add","x","amount"]],"term":["return","sum"]}]}]});
        for command in [
            vec!["try".into(), frame.to_string(), "--no-test".into()],
            vec!["commit".into(), "c1".into()],
        ] {
            let mut words = vec![
                "--workspace".into(),
                fixture.path.display().to_string(),
                "--json".into(),
            ];
            words.extend(command);
            let mut output = Vec::new();
            assert_eq!(
                crate::cli::run(&words, &mut output),
                0,
                "{}",
                String::from_utf8_lossy(&output)
            );
        }
        let input = factored_input();
        let head = fixture.workspace.read_head().unwrap();
        let mut inspection = Inspection::new(&input).unwrap();
        inspection.bind(&fixture.workspace, &head, &input).unwrap();
        let digest = inspection.binding.as_ref().unwrap().digest().to_owned();
        let path = fixture.path.join(".sley/names.json");
        let mut names = std::fs::read(&path).unwrap();
        names.push(b'\n');
        std::fs::write(&path, names).unwrap();
        let failure = inspection
            .bind(&fixture.workspace, &head, &input)
            .unwrap_err();
        assert_eq!(failure.code(), AgentErrorCode::ResidualBindingStale);
        assert_eq!(inspection.binding.as_ref().unwrap().digest(), digest);
        let mut output = Vec::new();
        let global = Global {
            json: true,
            ..Global::default()
        };
        let failure = publish(
            &global,
            &fixture.workspace,
            &head,
            &input,
            inspection,
            &TrialOptions::default(),
            &mut output,
        )
        .unwrap_err();
        assert_eq!(failure.code(), AgentErrorCode::ResidualBindingStale);
        assert!(output.is_empty());
        assert!(!fixture.path.join(".sley/residual").exists());
    }
}
