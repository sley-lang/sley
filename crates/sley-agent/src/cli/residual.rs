//! Residual authoring delegates candidate work to the ordinary draft path.

mod draft_edit;
mod planning;

use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::rc::Rc;

use serde_json::{Value, json};

use super::{Global, Input, Origin, Proposal, Target, TrialOptions, Words};
use crate::draft::{self, DraftRef, Drafts};
use crate::error::{AgentError, AgentErrorCode, Result, io, usage};
use crate::residual::binding::{Binding, MAX_BOUND_ARTIFACT_BYTES, RuntimeIdentity};
use crate::residual::{self as authoring, BaseRef};
use crate::workspace::{Head, Workspace};

pub(super) struct Prepared {
    draft: Option<draft_edit::Context>,
    binding: Binding,
    runtime: RuntimeIdentity,
    fragment: Value,
    edit: Option<authoring::edit::Contract>,
    parent: Option<(Binding, Vec<u8>)>,
    resolution: Option<Value>,
    progress: Rc<RefCell<Value>>,
}

impl Prepared {
    pub(super) fn recheck(&self, workspace: &Workspace, head: &Head, input: &[u8]) -> Result<()> {
        if let Some(draft) = &self.draft {
            draft.source.recheck(workspace)?;
        }
        if let Some((binding, original)) = &self.parent {
            binding.recheck(workspace, original, &self.runtime)?;
        }
        self.binding.recheck(workspace, input, &self.runtime)?;
        if self.binding.to_json()["snapshot"]["workspace"]["accepted_head"]
            != crate::hex::encode(head.transaction_id().as_bytes())
        {
            return Err(AgentError::new(
                AgentErrorCode::ResidualBindingStale,
                "the trial's accepted head differs from the captured binding; replan",
            ));
        }
        Ok(())
    }

    pub(super) fn summary(&self) -> Value {
        let mut summary = json!({"binding":self.binding.digest(), "fragment":self.fragment});
        if let Some(edit) = &self.edit {
            summary["edit"] = edit.report();
            summary["edit"]["verification"] = json!("not_run");
        }
        if let Some(draft) = &self.draft {
            summary["draft_graph"] = draft.report.clone();
        }
        if let Some(resolution) = &self.resolution {
            summary["resolution"] = resolution.clone();
        }
        summary
    }

    pub(super) fn check(
        &self,
        program: &crate::workspace::Program,
        compiled: &crate::frame::Compiled,
    ) -> Result<()> {
        self.edit
            .as_ref()
            .map_or(Ok(()), |edit| edit.check(program, compiled))
    }

    pub(super) fn verify(
        &self,
        before: &crate::workspace::Program,
        after: &crate::workspace::Program,
    ) -> Result<()> {
        let before = self.draft.as_ref().map_or(before, |draft| &draft.graph);
        let result = self
            .edit
            .as_ref()
            .map_or(Ok(()), |edit| edit.verify(before, after));
        if result.is_err() {
            self.progress.borrow_mut()["construction"] = json!("refused");
        }
        result
    }

    pub(super) fn note_outcome(&self, outcome: Value) {
        self.progress.replace(outcome);
    }
}

pub(super) fn command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let result = match args.split_first() {
        Some((action, args)) if action == "try" => try_command(global, args, out),
        Some((action, args)) if action == "show" => show_command(global, args, out),
        Some((action, args)) if action == "plan" => planning::plan_command(global, args, out),
        Some((action, args)) if action == "fill" => planning::fill_command(global, args, out),
        _ => Err(usage(
            "residual try <request.json> [--no-test | --all-tests] [--public file] [--raw] [--verbose]; residual plan <request.json>; residual fill <rN@1> <decisions.json>; residual show <dN@rK | rN@1> [--expanded | --decisions | --provenance | --targets]",
        )),
    };
    match result {
        Ok(exit) => Ok(exit),
        Err(error) => {
            global.note("refusal", error.code().symbol());
            let event = global.event.borrow().line(0, "residual", 0);
            let kernel = if event["residual"]["kernel_validation_count_incomplete"] == true {
                "unknown"
            } else if event["residual"]["kernel_validations"]
                .as_u64()
                .is_some_and(|n| n > 0)
            {
                "valid"
            } else if event["residual"]["draft_graph_preparation"] == true {
                "unknown"
            } else {
                "not_run"
            };
            let value = json!({
                "construction":"refused", "kernel":kernel, "public_checks":"not_run",
                "native_admission":"not_attempted", "error":error.code().symbol(),
                "detail":error.detail(),
            });
            print(global, out, &value)?;
            Ok(super::EXIT_REFUSED)
        }
    }
}

fn read_limited(reader: impl Read, path: &Path, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io(path, &error))?;
    if bytes.len() > limit {
        return Err(AgentError::new(
            AgentErrorCode::ResidualLimit,
            format!("{} exceeds its {limit}-byte read bound", path.display()),
        ));
    }
    Ok(bytes)
}

fn input(argument: &str) -> Result<Vec<u8>> {
    let limit = authoring::MAX_REQUEST_BYTES;
    let path = Path::new(argument);
    if argument == "-" {
        read_limited(std::io::stdin().lock(), Path::new("<stdin>"), limit)
    } else if argument.trim_start().starts_with(['{', '[']) {
        read_limited(argument.as_bytes(), Path::new("<inline>"), limit)
    } else {
        read_limited(
            File::open(path).map_err(|error| io(path, &error))?,
            path,
            limit,
        )
    }
}

fn read_json(path: &Path) -> Result<Value> {
    let file = File::open(path).map_err(|error| io(path, &error))?;
    authoring::strict_json_with_limit(
        &read_limited(file, path, MAX_BOUND_ARTIFACT_BYTES)?,
        MAX_BOUND_ARTIFACT_BYTES,
    )
}

fn try_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = super::words(
        args,
        &["--public"],
        &["--no-test", "--all-tests", "--raw", "--verbose"],
    )?;
    let [argument] = words.positional.as_slice() else {
        return Err(usage(
            "residual try <request.json> [compatible try options]",
        ));
    };
    let input = input(argument)?;
    // No workspace or ledger is opened until strict parsing has succeeded.
    let mut inspection = planning::Inspection::new(&input)?;
    let workspace = super::workspace(global)?;
    global.note("input_bytes", input.len());
    let head = workspace.read_head()?;
    let options = TrialOptions::of(&words);
    inspection.bind_reported(global, &workspace, &head, &input)?;
    if !inspection.decisions.is_empty() {
        planning::publish(global, &workspace, &head, &input, inspection, &options, out)?;
        return Ok(super::EXIT_NEGATIVE);
    }
    let resolution =
        inspection.resolve(&serde_json::Map::new(), "residual-request.json", "", false)?;
    let original = input.clone();
    let mut proposal = prepare(
        global,
        &workspace,
        &head,
        &inspection.request,
        input,
        true,
        resolution.as_ref(),
        &mut inspection.budget,
    )?;
    if let Some(binding) = inspection.binding.take() {
        proposal.residual.as_mut().expect("prepared").parent = Some((binding, original));
    }
    attach_budget(&mut proposal, &mut inspection.budget)?;
    // The ordinary trial is outside the planner's resource scope.
    drop(inspection);
    execute_reported(global, &workspace, &head, proposal, &options, out)
}

fn execute_reported(
    global: &Global,
    workspace: &Workspace,
    head: &Head,
    proposal: Proposal,
    options: &TrialOptions,
    out: &mut dyn Write,
) -> Result<i32> {
    let progress = proposal
        .residual
        .as_ref()
        .expect("residual proposal")
        .progress
        .clone();
    match execute(global, workspace, head, proposal, options, out) {
        Ok(code) => Ok(code),
        Err(error) => {
            // An I/O failure after validation must not erase the kernel result.
            let mut value = progress.borrow().clone();
            value["error"] = json!(error.code().symbol());
            value["detail"] = json!(error.detail());
            global.note("refusal", error.code().symbol());
            print(global, out, &value)?;
            Ok(super::EXIT_REFUSED)
        }
    }
}

fn execute(
    global: &Global,
    workspace: &Workspace,
    head: &Head,
    proposal: Proposal,
    options: &TrialOptions,
    out: &mut dyn Write,
) -> Result<i32> {
    if proposal
        .residual
        .as_ref()
        .and_then(|prepared| prepared.edit.as_ref())
        .is_some_and(authoring::edit::Contract::no_change)
    {
        return unchanged(global, workspace, head, &proposal, options, out);
    }
    // Capture the existing structured result; retain its status, diagnostics,
    // tests and event counters while rendering a compact residual response.
    let inner = Global {
        json: true,
        ..global.clone()
    };
    let mut output = Vec::new();
    let result = super::run_trial(&inner, workspace, head, proposal, options, &mut output);
    global.event.replace(inner.event.into_inner());
    let code = result?;
    let full: Value = serde_json::from_slice(&output).map_err(|error| {
        AgentError::new(
            AgentErrorCode::Io,
            format!("invalid internal trial result: {error}"),
        )
    })?;
    let reference = full["draft"]
        .as_str()
        .and_then(DraftRef::parse)
        .ok_or_else(|| {
            AgentError::new(
                AgentErrorCode::Io,
                "trial result is missing its draft revision",
            )
        })?;
    let status = read_json(
        &Drafts::read_only(workspace)
            .revision_dir(
                &reference.handle,
                reference.revision.expect("trial revision"),
            )
            .join("status.json"),
    )?;
    let mut report = report(&status, &full["draft"]);
    for key in ["error", "detail", "next", "stored_hex", "notes"] {
        if let Some(value) = full.get(key) {
            report[key] = value.clone();
        }
    }
    let failures: Vec<_> = ["tests", "public"]
        .into_iter()
        .flat_map(|kind| {
            full[kind]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|test| test["pass"] == false)
                .map(move |test| json!({"kind":kind, "case":test}))
        })
        .collect();
    report["failures"] = json!(failures.iter().take(16).collect::<Vec<_>>());
    report["failures_omitted"] = json!(failures.len().saturating_sub(16));
    if options.verbose {
        if let Some(edit) = status["residual"].get("edit") {
            report["edit"] = edit.clone();
        }
        report["trial"] = full;
    }
    print(global, out, &report)?;
    Ok(code)
}

fn unchanged(
    global: &Global,
    workspace: &Workspace,
    head: &Head,
    proposal: &Proposal,
    options: &TrialOptions,
    out: &mut dyn Write,
) -> Result<i32> {
    let prepared = proposal.residual.as_ref().expect("residual proposal");
    prepared.recheck(workspace, head, &proposal.input)?;
    let program = prepared
        .draft
        .as_ref()
        .map_or(head.program(), |draft| &draft.graph);
    prepared.verify(program, program)?;
    let mut report = json!({
        "construction":"complete", "kernel":"not_run", "public_checks":"not_run",
        "native_admission":"not_attempted", "no_change":true,
        "binding":prepared.binding.digest(), "fragment":prepared.fragment,
        "edit":prepared.edit.as_ref().expect("edit").report(),
    });
    report["edit"]["verification"] = json!("passed");
    if let Some(draft) = &prepared.draft {
        report["kernel"] = json!("valid");
        report["draft_graph"] = draft.report.clone();
    }
    if let Some(resolution) = &prepared.resolution {
        report["resolution"] = resolution.clone();
    }
    prepared.note_outcome(report.clone());
    let selected = proposal
        .staged
        .as_ref()
        .map_or_else(Vec::new, |(compiled, _, output)| {
            compiled
                .tests
                .iter()
                .copied()
                .chain(output.result().record.selected_tests.iter().copied())
                .collect()
        });
    let (checks, refused) = match unchanged_checks(workspace, program, &selected, options) {
        Ok(checks) => {
            let refused = checks.get("error").is_some();
            if refused {
                report["error"] = checks["error"].clone();
                report["detail"] = checks["detail"].clone();
                global.note("refusal", checks["error"].clone());
            }
            (checks, refused)
        }
        Err(error) => {
            global.note("refusal", error.code().symbol());
            report["error"] = json!(error.code().symbol());
            report["detail"] = json!(error.detail());
            (
                json!({"outcome":"failed", "execution_refusal":error.detail()}),
                true,
            )
        }
    };
    report["public_checks"] = checks["outcome"].clone();
    report["checks"] = checks;
    prepared.note_outcome(report.clone());
    // Tests are observations of this exact bound head, never admission evidence.
    prepared.recheck(workspace, head, &proposal.input)?;
    let mut artifacts: serde_json::Map<String, Value> =
        proposal.artifacts.iter().cloned().collect();
    artifacts
        .get_mut("residual-provenance.json")
        .expect("residual provenance")["frame"] = json!(if prepared.draft.is_some() {
        "residual-retained-frame.json"
    } else {
        "residual-expanded.json"
    });
    let handle = authoring::store::save(
        workspace,
        &json!({
            "kind":"no_change", "report":report, "artifacts":artifacts,
        }),
    )?;
    report["residual"] = json!(handle);
    report["inspect"] = json!(format!("sley-agent residual show {handle} --expanded"));
    let failed = report["public_checks"] == "failed";
    if !options.verbose {
        compact_no_change(&mut report);
    }
    print(global, out, &report)?;
    Ok(if refused {
        super::EXIT_REFUSED
    } else if failed {
        super::EXIT_NEGATIVE
    } else {
        super::EXIT_OK
    })
}

fn unchanged_checks(
    workspace: &Workspace,
    program: &crate::workspace::Program,
    selected: &[sley_id::EntityId],
    options: &TrialOptions,
) -> Result<Value> {
    let cases = options
        .public
        .as_ref()
        .map(|path| super::read_public(Path::new(path)))
        .transpose()?;
    if options.no_test {
        return Ok(json!({"outcome":"not_run", "tests":[], "public":[], "failures":[]}));
    }
    let names = crate::names::Names::build(program, &super::name_map(workspace)?);
    let mut executor = super::Executor::new(program)?;
    let tests: Vec<_> = executor
        .tests()
        .to_vec()
        .iter()
        .filter(|test| options.all_tests || selected.contains(&test.entity_id))
        .map(|test| executor.run_test(test, &names))
        .collect();
    let public = cases
        .as_ref()
        .map(|cases| super::run_public(&mut executor, program, &names, cases))
        .transpose();
    let (public, refusal) = match public {
        Ok(public) => (public.unwrap_or_default(), None),
        Err(error) => (Vec::new(), Some(error)),
    };
    let tests = super::tests_json(&tests, &names);
    let public = super::public_json(&public);
    let failures: Vec<_> = [(&tests, "tests"), (&public, "public")]
        .into_iter()
        .flat_map(|(values, kind)| {
            values
                .as_array()
                .into_iter()
                .flatten()
                .filter(|test| test["pass"] == false)
                .map(move |test| json!({"kind":kind,"case":test}))
        })
        .collect();
    let count = tests.as_array().map_or(0, Vec::len) + public.as_array().map_or(0, Vec::len);
    let mut report = json!({"outcome":if refusal.is_some() || !failures.is_empty() {"failed"} else if count == 0 {"zero_ran"} else {"passed"},
        "count":count, "tests":tests, "public":public,
        "failures":failures.iter().take(16).collect::<Vec<_>>(),
        "failures_omitted":failures.len().saturating_sub(16)});
    if let Some(error) = refusal {
        report["error"] = json!(error.code().symbol());
        report["detail"] = json!(error.detail());
    }
    Ok(report)
}

fn compact_edit(report: &mut Value) {
    let reference = report
        .get("draft")
        .or_else(|| report.get("residual"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Some(Value::Object(edit)) = report.get_mut("edit") {
        if let Some(Value::Array(targets)) = edit.get("targets") {
            let count = targets.len();
            let changed = targets
                .iter()
                .filter(|target| target["changed"] == true)
                .count();
            edit.entry("targets_count").or_insert(json!(count));
            edit.entry("targets_changed").or_insert(json!(changed));
        }
        if let Some(reference) = reference {
            edit.insert(
                "inspect_targets".into(),
                json!(format!("sley-agent residual show {reference} --targets")),
            );
            edit.insert(
                "inspect".into(),
                json!(format!("sley-agent residual show {reference} --provenance")),
            );
        }
    }
    for pointer in [
        "/edit/targets",
        "/edit/boundaries/functions",
        "/edit/boundaries/possible_static_callers",
        "/edit/boundaries/exported",
        "/edit/boundaries/entry_points",
    ] {
        let (parent, key) = pointer.rsplit_once('/').expect("pointer");
        let omitted_key = format!("{key}_omitted");
        let previously_omitted = report
            .pointer(parent)
            .and_then(|value| value.get(&omitted_key))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if let Some(Value::Array(values)) = report.pointer_mut(pointer) {
            let omitted = previously_omitted + values.len().saturating_sub(16) as u64;
            values.truncate(16);
            if pointer == "/edit/targets" {
                for target in values {
                    if let Some(object) = target.as_object_mut() {
                        object.retain(|key, _| {
                            matches!(
                                key.as_str(),
                                "scope_index" | "name" | "changed" | "requested"
                            )
                        });
                    }
                }
                report["edit"]["target_details_omitted"] = json!(true);
            }
            report.pointer_mut(parent).expect("parent")[omitted_key] = json!(omitted);
        }
    }
}

fn compact_no_change(report: &mut Value) {
    compact_edit(report);
    let reference = report["residual"].as_str().map(str::to_owned);
    let Some(checks) = report.get_mut("checks").and_then(Value::as_object_mut) else {
        return;
    };
    for kind in ["tests", "public"] {
        if let Some(count) = checks.get(kind).and_then(Value::as_array).map(Vec::len) {
            checks.remove(kind);
            checks.insert(format!("{kind}_omitted"), json!(count));
            checks.insert("details_omitted".into(), json!(true));
        }
    }
    if checks.get("details_omitted").and_then(Value::as_bool) == Some(true)
        && let Some(reference) = reference
    {
        checks.insert(
            "inspect".into(),
            json!(format!("sley-agent residual show {reference} --provenance")),
        );
    }
}

#[allow(clippy::too_many_arguments)] // Shared invocation budget must not be recreated here.
fn prepare(
    global: &Global,
    workspace: &Workspace,
    head: &Head,
    request: &authoring::Request,
    input: Vec<u8>,
    attempt: bool,
    resolution: Option<&authoring::choices::Resolution>,
    budget: &mut authoring::frontier::Budget,
) -> Result<Proposal> {
    budget.checkpoint()?;
    let runtime = authoring::runtime::identity()?;
    let binding = Binding::capture(workspace, &input, &runtime)?;
    budget.checkpoint()?;
    let mut prepared = Prepared {
        draft: None,
        binding,
        runtime,
        fragment: json!({"id":request.fragment.id, "version":request.fragment.version}),
        edit: None,
        parent: None,
        resolution: None,
        progress: Rc::new(RefCell::new(json!({
            "construction":"refused", "kernel":"not_run", "public_checks":"not_run",
            "native_admission":"not_attempted",
        }))),
    };
    prepared.recheck(workspace, head, &input)?;
    budget.checkpoint()?;
    let effective = resolution
        .map(|resolution| {
            authoring::parse_request(
                &serde_json::to_vec(&resolution.request).expect("resolved request"),
            )
        })
        .transpose()?;
    let request = effective.as_ref().unwrap_or(request);
    if request.operation == authoring::Operation::Edit && matches!(request.base, BaseRef::Draft(_))
    {
        return draft_edit::prepare(
            global, workspace, head, request, input, attempt, resolution, budget, prepared,
        );
    }
    budget.checkpoint()?;
    let interfaces = check_interfaces(workspace, head, request, budget)?;
    let expansion = match request.operation {
        authoring::Operation::Derive => authoring::fragments::expand_with_budget(request, budget)?,
        authoring::Operation::Edit => {
            let names = crate::names::Names::build(head.program(), &super::name_map(workspace)?);
            budget.checkpoint()?;
            let edit =
                authoring::edit::expand_with_budget(head.program(), &names, request, budget)?;
            prepared.edit = Some(edit.contract);
            edit.expansion
        }
    };
    budget.checkpoint()?;
    let mut artifacts = vec![
        (
            "residual-request.json".into(),
            authoring::strict_json_with_limit(&input, authoring::MAX_REQUEST_BYTES)?,
        ),
        ("residual-binding.json".into(), prepared.binding.to_json()),
        ("residual-expanded.json".into(), expansion.frame.clone()),
        (
            "residual-provenance.json".into(),
            json!({
            "frame":"frame.json", "entries":expansion.provenance,
                "scope":request.scope,
            }),
        ),
    ];
    global.note(
        "residual",
        json!({
            "try":attempt, "plan":!attempt, "request_bytes":input.len(),
            "expanded_bytes":serde_json::to_vec(&expansion.frame).expect("JSON expansion").len(),
            "provenance_entries":expansion.provenance.len(),
        }),
    );
    budget.checkpoint()?;
    let mut proposal = proposal(workspace, &request.base, input, expansion.frame)?;
    budget.checkpoint()?;
    if let Some(entries) = relocated_provenance(&proposal, request, expansion.provenance) {
        artifacts[3].1["entries"] = Value::Object(entries);
    }
    if let Some(edit) = &prepared.edit {
        artifacts.push(("residual-edit.json".into(), edit.report()));
    }
    if let Some(interfaces) = interfaces {
        artifacts.push(("residual-interfaces.json".into(), interfaces));
    }
    proposal.residual = Some(prepared);
    proposal.artifacts = artifacts;
    if let Some(resolution) = resolution {
        attach_resolution(&mut proposal, resolution)?;
    }
    budget.checkpoint()?;
    Ok(proposal)
}

fn relocated_provenance(
    proposal: &Proposal,
    request: &authoring::Request,
    provenance: std::collections::BTreeMap<String, Value>,
) -> Option<serde_json::Map<String, Value>> {
    if let Input::Frame(frame) = &proposal.frame
        && request.operation == authoring::Operation::Derive
    {
        let target = request.scope[0].as_str().expect("expanded exact function");
        let index = frame["fns"]
            .as_array()
            .and_then(|functions| {
                functions
                    .iter()
                    .position(|function| function["fn"] == target)
            })
            .expect("layer retains the residual target");
        let relocated: serde_json::Map<String, Value> = provenance
            .into_iter()
            .map(|(at, origin)| {
                let at = at
                    .strip_prefix("/fns/0")
                    .map_or_else(|| at.clone(), |tail| format!("/fns/{index}{tail}"));
                (at, origin)
            })
            .collect();
        return Some(relocated);
    }
    None
}

fn check_interfaces(
    workspace: &Workspace,
    head: &Head,
    request: &authoring::Request,
    budget: &mut authoring::frontier::Budget,
) -> Result<Option<Value>> {
    if request.operation != authoring::Operation::Derive {
        return Ok(None);
    }
    budget.checkpoint()?;
    let declarations = match &request.base {
        BaseRef::Draft(reference) => {
            super::layer_base(
                &Drafts::read_only(workspace),
                &reference.handle,
                reference.revision.expect("explicit revision"),
            )?
            .0
        }
        _ => json!({}),
    };
    budget.checkpoint()?;
    let names = crate::names::Names::build(head.program(), &super::name_map(workspace)?);
    let report = authoring::interfaces::check(
        head.program(),
        &names,
        declarations
            .as_object()
            .ok_or_else(|| usage("source draft frame must be an object"))?,
        request,
        budget,
    )?;
    authoring::interfaces::require_literal_contexts(&report, budget)?;
    authoring::interfaces::require_expression_types(&report, budget)?;
    Ok(Some(report))
}

fn attach_budget(proposal: &mut Proposal, budget: &mut authoring::frontier::Budget) -> Result<()> {
    budget.checkpoint()?;
    proposal
        .artifacts
        .push(("residual-budget.json".into(), budget.usage()));
    // Refuse before entering the ordinary trial or consuming the fill marker.
    budget.checkpoint()
}

fn attach_resolution(
    proposal: &mut Proposal,
    resolution: &authoring::choices::Resolution,
) -> Result<()> {
    let provenance = proposal
        .artifacts
        .iter_mut()
        .find(|(name, _)| name == "residual-provenance.json")
        .expect("prepared provenance");
    let mut entries = serde_json::from_value(provenance.1["entries"].clone())
        .map_err(|error| AgentError::new(AgentErrorCode::ResidualParse, error.to_string()))?;
    resolution.apply(&mut entries);
    provenance.1["entries"] = json!(entries);
    proposal.artifacts.push((
        "residual-resolution.json".into(),
        resolution.evidence.clone(),
    ));
    proposal
        .residual
        .as_mut()
        .expect("prepared residual")
        .resolution = Some(resolution.evidence["summary"].clone());
    Ok(())
}

fn proposal(
    workspace: &Workspace,
    base: &BaseRef,
    input: Vec<u8>,
    frame: Value,
) -> Result<Proposal> {
    let BaseRef::Draft(reference) = base else {
        return Ok(Proposal::new(
            "residual-try",
            input,
            Input::Frame(frame),
            Target::New,
            Origin::Draft,
        ));
    };
    let revision = reference.revision.expect("strict residual draft revision");
    let drafts = Drafts::read_only(workspace);
    let (base_frame, status) = super::layer_base(&drafts, &reference.handle, revision)?;
    // Layering errors are refused before claiming or recording a revision.
    let layered = crate::layer::layer(&base_frame, &frame)?;
    let mut proposal = Proposal::new(
        "residual-try",
        input,
        Input::Frame(layered),
        Target::Next {
            handle: reference.handle.clone(),
            parent: revision,
            latest: true,
        },
        Origin::Draft,
    );
    proposal.on = Some(draft::spell(&reference.handle, revision));
    proposal.inherit(&status);
    Ok(proposal)
}

pub(super) fn attach_sources(artifacts: &mut Vec<(String, Value)>, frame: &Value) -> Result<()> {
    let get = |name: &str| {
        artifacts
            .iter()
            .find(|(file, _)| file == name)
            .map(|(_, value)| value)
    };
    let Some(provenance) = get("residual-provenance.json") else {
        return Ok(());
    };
    let expansion = authoring::fragments::Expansion {
        frame: frame.clone(),
        provenance: serde_json::from_value(provenance["entries"].clone()).map_err(|error| {
            AgentError::new(
                AgentErrorCode::Io,
                format!("invalid generated provenance: {error}"),
            )
        })?,
    };
    let Some(map) = get("sourcemap.json") else {
        return Ok(());
    };
    let source_entries = map["entries"].as_array().expect("generated source map");
    let entries: Vec<_> = source_entries
        .iter()
        .filter_map(|entry| {
            let origin = expansion.lowering_origin(entry["authored"].as_str()?)?;
            Some(json!({"expanded":entry["expanded"], "origin":origin}))
        })
        .collect();
    let composed = json!({
        "frame":"expanded.json", "coverage":"residual target only",
        "unmapped_entries":source_entries.len() - entries.len(), "entries":entries,
    });
    artifacts.push(("residual-source-map.json".into(), composed));
    Ok(())
}

/// Four independent outcomes; tests run by this workbench are never admission.
pub(super) fn outcomes(status: &Value, no_test: bool) -> Value {
    let kernel = match status["state"].as_str() {
        Some("valid") => "valid",
        Some("refused") => "refused",
        _ => "not_run",
    };
    let ran = status["results"]["ran"].as_u64().unwrap_or(0);
    let passed = status["results"]["passed"].as_u64().unwrap_or(0);
    let public = status["results"]["public"].as_u64().unwrap_or(0);
    let public_passed = status["results"]["public_passed"].as_u64().unwrap_or(0);
    let checks = if kernel != "valid" || no_test {
        "not_run"
    } else if status["results"].get("public_refusal").is_some()
        || status["results"].get("execution_refusal").is_some()
        || passed != ran
        || public_passed != public
    {
        "failed"
    } else if ran + public == 0 {
        "zero_ran"
    } else {
        "passed"
    };
    json!({
        "construction":if kernel == "not_run" {"incomplete"} else {"complete"},
        "kernel":kernel, "public_checks":checks, "native_admission":"not_attempted",
        "checks":{"cases":ran,"cases_passed":passed,"public":public,"public_passed":public_passed},
    })
}

fn report(status: &Value, reference: &Value) -> Value {
    let mut report = status["residual"]["outcome"].clone();
    // Compiler refusals are recorded before the final outcome calculation.
    if !report.is_object() {
        report = outcomes(status, true);
    }
    report["draft"] = reference.clone();
    report["handle"] = status["candidate"].clone();
    report["binding"] = status["residual"]["binding"].clone();
    report["fragment"] = status["residual"]["fragment"].clone();
    if let Some(resolution) = status["residual"].get("resolution") {
        report["resolution"] = resolution.clone();
    }
    if let Some(edit) = status["residual"].get("edit") {
        report["edit"] = edit.clone();
        compact_edit(&mut report);
    }
    if let Some(graph) = status["residual"].get("draft_graph") {
        report["draft_graph"] = graph.clone();
    }
    report["obligations"] = status["obligations"].clone();
    report["body_omitted"] = json!(true);
    report["inspect"] = json!(format!(
        "sley-agent residual show {} --expanded",
        reference.as_str().unwrap_or("")
    ));
    let changed = status["changed"].as_array().cloned().unwrap_or_default();
    report["changed"] = json!(changed.iter().take(16).collect::<Vec<_>>());
    report["changed_omitted"] = json!(changed.len().saturating_sub(16));
    if status["state"] == "refused" {
        report["verdict"] = status["verdict"].clone();
    }
    for key in ["public_refusal", "execution_refusal"] {
        if let Some(error) = status["results"].get(key) {
            report[key] = error.clone();
        }
    }
    report
}

fn literal_targets(edit: &Value) -> Result<Value> {
    let targets = edit["targets"]
        .as_array()
        .filter(|_| edit["lens"] == "integer_literal@1")
        .ok_or_else(|| usage("--targets requires a saved integer-literal edit; use --provenance for other records"))?;
    let mut rows = Vec::with_capacity(targets.len());
    for target in targets {
        let source = &target["source"];
        if !target["scope_index"].is_u64()
            || !target["name"].is_string()
            || !target["changed"].is_boolean()
            || target["requested"].is_null()
            || !source["width"].is_string()
            || source["previous"].is_null()
        {
            return Err(usage("saved integer-literal target details are incomplete"));
        }
        rows.push(
            json!({"scope_index":target["scope_index"], "name":target["name"],
            "changed":target["changed"], "requested":target["requested"],
            "width":source["width"], "previous":source["previous"]}),
        );
    }
    Ok(json!({"lens":edit["lens"], "targets_count":rows.len(),
        "targets_changed":rows.iter().filter(|row| row["changed"] == true).count(),
        "targets":rows, "source_details_omitted":true,
        "behavioral_preservation":edit["behavioral_preservation"], "preserve":edit["preserve"]}))
}

fn show_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words: Words = super::words(
        args,
        &[],
        &["--expanded", "--decisions", "--provenance", "--targets"],
    )?;
    let [reference] = words.positional.as_slice() else {
        return Err(usage(
            "residual show <dN@rK> [--expanded | --decisions | --provenance | --targets]",
        ));
    };
    if words.flags.len() > 1 {
        return Err(usage("choose one residual show view"));
    }
    if authoring::store::is_handle(reference) {
        return show_record(global, reference, &words, out);
    }
    let reference = DraftRef::parse(reference)
        .filter(|reference| reference.revision.is_some())
        .ok_or_else(|| usage("residual show requires an explicit draft revision dN@rK"))?;
    if words.flags.len() > 1 {
        return Err(usage("choose one residual show view"));
    }
    let workspace = super::workspace(global)?;
    let revision = reference.revision.expect("explicit revision");
    let dir = Drafts::read_only(&workspace).revision_dir(&reference.handle, revision);
    let status = read_json(&dir.join("status.json"))?;
    if !status["residual"].is_object() {
        return Err(usage(
            "that draft revision was not produced by residual authoring",
        ));
    }
    let spelled = draft::spell(&reference.handle, revision);
    let mut value = if words.has("--targets") {
        if !dir.join("residual-edit.json").is_file() {
            return Err(usage(
                "--targets requires a saved integer-literal edit; use --provenance for other records",
            ));
        }
        let summary = report(&status, &json!(spelled));
        json!({"draft":spelled, "view":"literal_targets",
            "binding":read_json(&dir.join("residual-binding.json"))?["digest"],
            "edit":literal_targets(&read_json(&dir.join("residual-edit.json"))?)?,
            "verification":summary["edit"]["verification"], "kernel":summary["kernel"],
            "checks":summary["checks"], "evidence":"saved requests and source literals; no checks rerun"})
    } else if words.has("--expanded") {
        let file = if dir.join("expanded.json").is_file() {
            "expanded.json"
        } else {
            "residual-expanded.json"
        };
        let expanded = read_json(&dir.join(file))?;
        let representation = if expanded.get("afx").is_some() {
            "AF1-X"
        } else {
            "AF1"
        };
        json!({"draft":spelled, "representation":representation, "expanded":expanded})
    } else if words.has("--decisions") {
        json!({"draft":spelled, "request":read_json(&dir.join("residual-request.json"))?})
    } else if words.has("--provenance") {
        json!({"draft":spelled, "provenance":read_json(&dir.join("residual-provenance.json"))?,
            "binding":read_json(&dir.join("residual-binding.json"))?,
            "interfaces":if dir.join("residual-interfaces.json").is_file() {
                read_json(&dir.join("residual-interfaces.json"))?
            } else {Value::Null},
            "planning_resources":if dir.join("residual-budget.json").is_file() {
                read_json(&dir.join("residual-budget.json"))?
            } else {Value::Null},
            "resolution":if dir.join("residual-resolution.json").is_file() {
                read_json(&dir.join("residual-resolution.json"))?
            } else {Value::Null},
            "edit":if dir.join("residual-edit.json").is_file() {
                read_json(&dir.join("residual-edit.json"))?
            } else {Value::Null},
            "afx_source_map":if dir.join("sourcemap.json").is_file() {
                read_json(&dir.join("sourcemap.json"))?
            } else {Value::Null},
            "composed_source_map":if dir.join("residual-source-map.json").is_file() {
                read_json(&dir.join("residual-source-map.json"))?
            } else {Value::Null}})
    } else {
        report(&status, &json!(spelled))
    };
    value["historical"] = json!(true);
    // Inspection is historical and output-only: a changed head does not rewrite
    // or invalidate access to the recorded evidence.
    if global.json {
        super::write_json(out, &value)?;
    } else if words.flags.is_empty() {
        print(global, out, &value)?;
    } else {
        super::write_text(
            out,
            &format!(
                "{}\n",
                serde_json::to_string_pretty(&value).expect("JSON view")
            ),
        )?;
    }
    Ok(super::EXIT_OK)
}

fn show_record(
    global: &Global,
    reference: &str,
    words: &Words,
    out: &mut dyn Write,
) -> Result<i32> {
    let workspace = super::workspace(global)?;
    let record = authoring::store::load(&workspace, reference)?;
    if record["kind"] == "plan" {
        if words.has("--targets") {
            return Err(usage(
                "--targets requires a saved literal edit, not a plan; use --decisions",
            ));
        }
        return planning::show(global, reference, &record, words, out);
    }
    if record["kind"] != "no_change"
        || !record["report"].is_object()
        || !record["artifacts"].is_object()
    {
        return Err(AgentError::new(
            AgentErrorCode::ResidualParse,
            "unsupported or malformed residual record",
        ));
    }
    let artifacts = &record["artifacts"];
    let mut value = if words.has("--targets") {
        json!({"view":"literal_targets", "binding":artifacts["residual-binding.json"]["digest"],
            "edit":literal_targets(&artifacts["residual-edit.json"])?,
            "verification":record["report"]["edit"]["verification"],
            "kernel":record["report"]["kernel"],
            "evidence":"saved requests and source literals; no checks rerun"})
    } else if words.has("--expanded") {
        json!({"representation":"AF1", "expanded":artifacts["residual-expanded.json"]})
    } else if words.has("--decisions") {
        json!({"request":artifacts["residual-request.json"]})
    } else if words.has("--provenance") {
        json!({"provenance":artifacts["residual-provenance.json"],
            "resolution":artifacts["residual-resolution.json"],
            "planning_resources":artifacts["residual-budget.json"],
            "checks":record["report"]["checks"],
            "binding":artifacts["residual-binding.json"], "edit":artifacts["residual-edit.json"]})
    } else {
        record["report"].clone()
    };
    value["residual"] = json!(reference);
    value["historical"] = json!(true);
    if words.flags.is_empty() {
        compact_no_change(&mut value);
    }
    print(global, out, &value)?;
    Ok(super::EXIT_OK)
}

fn print(global: &Global, out: &mut dyn Write, value: &Value) -> Result<()> {
    if global.json {
        return super::write_json(out, value);
    }
    // The summary is structured even in text mode so no refusal/check category
    // disappears behind a short success headline.
    super::write_text(
        out,
        &format!(
            "{}\n",
            serde_json::to_string_pretty(value).expect("JSON result")
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_is_rechecked_before_claim_and_after_compilation() {
        let dir = std::env::temp_dir().join(format!(
            "residual-candidate-guard-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace =
            crate::genesis::init(&dir, Some([205; 32]), crate::genesis::INIT_CEILINGS).unwrap();
        let head = workspace.head().unwrap();
        let global = Global::default();
        let input = crate::help::RESIDUAL
            .lines()
            .find(|line| line.starts_with("{\"residual\":"))
            .unwrap()
            .as_bytes()
            .to_vec();
        let request = authoring::parse_request(&input).unwrap();
        authoring::runtime::identity().unwrap();
        let proposal = prepare(
            &global,
            &workspace,
            &head,
            &request,
            input.clone(),
            true,
            None,
            &mut authoring::frontier::Budget::default(),
        )
        .unwrap();
        let prepared = proposal.residual.as_ref().unwrap();
        let parent_binding = prepared.binding.clone();
        let authority = crate::candidate::Authority::of(&head).unwrap();
        let map = super::super::name_map(&workspace).unwrap();
        let names = crate::names::Names::build(head.program(), &map);
        let Input::Frame(frame) = &proposal.frame else {
            panic!("prepared frame");
        };
        let path = dir.join(crate::workspace::NAMES_FILE);
        let changed = b"{}";
        let mut reached = false;
        let result = super::super::stage(
            &head,
            &authority,
            &names,
            frame,
            crate::candidate::fresh_nonce().unwrap(),
            |_| {
                reached = true;
                std::fs::write(&path, changed).unwrap();
                prepared.recheck(&workspace, &head, &input)
            },
        );
        assert!(
            reached,
            "ordinary compilation must precede the candidate guard: {:?}",
            result.as_ref().err()
        );
        assert_eq!(
            result.err().unwrap().code(),
            AgentErrorCode::ResidualBindingStale
        );
        // A fill may capture its completed request after the old plan was
        // observed. A fresh child binding must not silently accept that race.
        let mut fresh = prepare(
            &global,
            &workspace,
            &head,
            &request,
            input.clone(),
            true,
            None,
            &mut authoring::frontier::Budget::default(),
        )
        .unwrap();
        let fresh = fresh.residual.as_mut().unwrap();
        fresh.recheck(&workspace, &head, &input).unwrap();
        fresh.parent = Some((parent_binding, input.clone()));
        assert_eq!(
            fresh.recheck(&workspace, &head, &input).unwrap_err().code(),
            AgentErrorCode::ResidualBindingStale
        );
        let error = super::super::run_trial(
            &global,
            &workspace,
            &head,
            proposal,
            &TrialOptions::default(),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualBindingStale);
        assert!(
            !dir.join(".sley").exists(),
            "stale binding must refuse before claiming a draft"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
