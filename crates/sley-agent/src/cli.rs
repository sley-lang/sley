//! Command-line dispatch for `sley-agent`.
//!
//! Every command prints compact text by default and one JSON object under
//! `--json`. Exit status: 0 success, 1 a negative outcome (refused
//! candidate, failing test), 2 a workbench refusal (`AGENT_*`).

use std::fmt::Write as _;
use std::fs;
use std::io::{Read as _, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sley_id::EntityId;
use sley_ssmc::{ConstValue, TypeExpr};

use crate::candidate::{self, Authority, Store};
use crate::catalog::Verdict;
use crate::error::{AgentError, AgentErrorCode, Result, io, unknown_name, usage};
use crate::exec::{self, Executor, TestOutcome};
use crate::frame;
use crate::help;
use crate::names::{NameMap, Names, Scope};
use crate::values::{self, ProgramTypes};
use crate::view::{self, ViewOptions};
use crate::workspace::{Head, NAMES_FILE, Program, STATE_DIR, SUBMISSION, Workspace};

/// Exit status for success.
pub const EXIT_OK: i32 = 0;
/// Exit status for a negative outcome (refused candidate, failing test).
pub const EXIT_NEGATIVE: i32 = 1;
/// Exit status for a workbench refusal.
pub const EXIT_REFUSED: i32 = 2;

/// Parsed global options.
#[derive(Clone, Debug, Default)]
struct Global {
    workspace: Option<PathBuf>,
    json: bool,
}

/// Runs one command line and returns the exit status.
pub fn run(args: &[String], out: &mut dyn Write) -> i32 {
    let mut global = Global::default();
    let mut rest = Vec::new();
    let mut words = args.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--workspace" | "-C" => match words.next() {
                Some(dir) => global.workspace = Some(PathBuf::from(dir)),
                None => return refuse(out, &usage("--workspace needs a directory"), false),
            },
            "--json" => global.json = true,
            _ => rest.push(word.clone()),
        }
    }
    match dispatch(&global, &rest, out) {
        Ok(status) => status,
        Err(error) => refuse(out, &error, global.json),
    }
}

fn refuse(out: &mut dyn Write, error: &AgentError, json: bool) -> i32 {
    let text = if json {
        json!({"error": error.code().symbol(), "detail": error.detail()}).to_string()
    } else {
        format!("error {}: {}", error.code().symbol(), error.detail())
    };
    let _ = writeln!(out, "{text}");
    EXIT_REFUSED
}

fn workspace(global: &Global) -> Result<Workspace> {
    if let Some(dir) = &global.workspace {
        Ok(Workspace::at(dir.clone()))
    } else {
        let cwd = std::env::current_dir().map_err(|error| io(Path::new("."), &error))?;
        Workspace::locate(&cwd)
    }
}

/// Loads the workspace name maps (starter map, then workbench map).
pub(crate) fn name_map(workspace: &Workspace) -> Result<NameMap> {
    let mut map = NameMap::read(&workspace.dir().join(NAMES_FILE))?;
    map.extend(&NameMap::read(
        &workspace.dir().join(STATE_DIR).join(NAMES_FILE),
    )?);
    Ok(map)
}

fn remember_names(workspace: &Workspace, names: &NameMap) -> Result<()> {
    if names.is_empty() {
        return Ok(());
    }
    let path = workspace.state_dir()?.join(NAMES_FILE);
    let mut map = NameMap::read(&path)?;
    map.extend(names);
    map.write(&path)
}

fn dispatch(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let Some((command, rest)) = args.split_first() else {
        return help_command(&[], out);
    };
    match command.as_str() {
        "view" => view_command(global, rest, out),
        "find" => find_command(global, rest, out),
        "try" => try_command(global, rest, out),
        "submit" => submit_command(global, rest, out),
        "status" => status_command(global, rest, out),
        "call" => call_command(global, rest, out),
        "test" => test_command(global, rest, out),
        "explain" => explain_command(global, rest, out),
        "init" => init_command(global, rest, out),
        "commit" => commit_command(global, rest, out),
        "export" => export_command(global, rest, out),
        "help" | "--help" | "-h" => help_command(rest, out),
        "version" | "--version" => {
            write_text(out, &format!("sley-agent {}\n", env!("CARGO_PKG_VERSION")))?;
            Ok(EXIT_OK)
        }
        other => Err(usage(format!(
            "unknown command `{other}` (try `sley-agent help`)"
        ))),
    }
}

fn write_text(out: &mut dyn Write, text: &str) -> Result<()> {
    out.write_all(text.as_bytes())
        .map_err(|error| io(Path::new("<stdout>"), &error))
}

fn write_json(out: &mut dyn Write, value: &Value) -> Result<()> {
    write_text(out, &format!("{value}\n"))
}

/// Flags and positional words of one command.
struct Words {
    flags: Vec<(String, Option<String>)>,
    positional: Vec<String>,
}

fn words(args: &[String], valued: &[&str], switches: &[&str]) -> Result<Words> {
    let mut flags = Vec::new();
    let mut positional = Vec::new();
    let mut iter = args.iter();
    while let Some(word) = iter.next() {
        if valued.contains(&word.as_str()) {
            let value = iter
                .next()
                .ok_or_else(|| usage(format!("{word} needs a value")))?;
            flags.push((word.clone(), Some(value.clone())));
        } else if switches.contains(&word.as_str()) {
            flags.push((word.clone(), None));
        } else if word.starts_with("--") {
            return Err(usage(format!("unknown flag `{word}`")));
        } else {
            positional.push(word.clone());
        }
    }
    Ok(Words { flags, positional })
}

impl Words {
    fn has(&self, flag: &str) -> bool {
        self.flags.iter().any(|(name, _)| name == flag)
    }

    fn value(&self, flag: &str) -> Option<&str> {
        self.flags
            .iter()
            .find(|(name, _)| name == flag)
            .and_then(|(_, value)| value.as_deref())
    }
}

/// A resolved program state: the head, or a candidate's proposed state.
struct Selected {
    program: Program,
    names: Names,
    label: Option<String>,
    verdict: Option<Verdict>,
    valid: bool,
    chosen_tests: Vec<EntityId>,
}

/// Selects the head, or the state a candidate reference proposes (a
/// refused candidate shows its pure apply).
fn select(
    workspace: &Workspace,
    head: &Head,
    map: &NameMap,
    reference: Option<&str>,
) -> Result<Selected> {
    let Some(reference) = reference else {
        let program = head.program().clone();
        let names = Names::build(&program, map);
        return Ok(Selected {
            program,
            names,
            label: None,
            verdict: None,
            valid: true,
            chosen_tests: Vec::new(),
        });
    };
    let store = Store::open(workspace)?;
    let reference = if reference == "latest" {
        store
            .latest()?
            .ok_or_else(|| AgentError::new(AgentErrorCode::HandleUnknown, "no candidates yet"))?
    } else {
        reference.to_owned()
    };
    let stored = store.load(&reference)?;
    let authority = Authority::of(head)?;
    let output = candidate::validate(head, &authority, &stored)?;
    let candidate = sley_mutate::import_candidate(&stored).ok();
    let program = candidate::proposed_program(head, &output)
        .or_else(|| {
            candidate
                .as_ref()
                .and_then(|c| candidate::applied_program(head, c))
        })
        .unwrap_or_else(|| head.program().clone());
    let names = Names::build(&program, map);
    let verdict = Verdict::of(&output, &program, &names);
    Ok(Selected {
        valid: output.is_valid(),
        chosen_tests: output.result().record.selected_tests.clone(),
        program,
        names,
        label: Some(reference),
        verdict: Some(verdict),
    })
}

fn view_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(
        args,
        &["--after"],
        &["--ids", "--types", "--limits", "--package"],
    )?;
    let options = ViewOptions {
        ids: words.has("--ids"),
        types: words.has("--types"),
        limits: words.has("--limits"),
    };
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let map = name_map(&workspace)?;
    let selected = select(&workspace, &head, &map, words.value("--after"))?;
    let mut text = view::header(&selected.program, selected.label.as_deref());
    if words.positional.is_empty() && words.value("--after").is_some() && !words.has("--package") {
        // The affected view: every function and test the candidate touches.
        let store = Store::open(&workspace)?;
        let affected = affected(
            &store,
            &head,
            selected.label.as_deref().unwrap_or(""),
            &selected,
        )?;
        for id in affected {
            text.push_str(&view::entity(
                &selected.program,
                &selected.names,
                &id,
                options,
            ));
        }
    } else if words.has("--package") || words.positional.is_empty() {
        text.push_str(&view::package(&selected.program, &selected.names, options));
    } else {
        for target in &words.positional {
            let id = selected
                .names
                .resolve(target)
                .ok_or_else(|| unknown_name(target))?;
            text.push_str(&view::entity(
                &selected.program,
                &selected.names,
                &id,
                options,
            ));
        }
    }
    if global.json {
        write_json(out, &json!({"view": text}))?;
    } else {
        write_text(out, &text)?;
    }
    Ok(EXIT_OK)
}

/// Top-level entities a candidate touches: functions owning any changed
/// entity, and changed top-level entities themselves.
fn affected(
    store: &Store,
    head: &Head,
    reference: &str,
    selected: &Selected,
) -> Result<Vec<EntityId>> {
    let stored = store.load(reference)?;
    let candidate = sley_mutate::import_candidate(&stored)
        .map_err(|error| AgentError::new(AgentErrorCode::CandidateInvalid, error.to_string()))?;
    let mut out = Vec::new();
    let push = |id: EntityId, out: &mut Vec<EntityId>| {
        if !out.contains(&id) {
            out.push(id);
        }
    };
    for operation in &candidate.record.operations {
        let target = operation.target_entity;
        let owner = owner_function(&selected.program, &selected.names, &target)
            .or_else(|| owner_function(head.program(), &Names::default(), &target));
        match owner {
            Some(function) if selected.program.contains(&function) => push(function, &mut out),
            _ if selected.program.contains(&target) => push(target, &mut out),
            _ => {}
        }
    }
    Ok(out)
}

fn owner_function(program: &Program, names: &Names, id: &EntityId) -> Option<EntityId> {
    use sley_mutate::value::EntityBodyValue;
    let _ = names;
    match program.body(id)? {
        EntityBodyValue::Function(_) => Some(*id),
        EntityBodyValue::Block(block) => Some(block.function),
        EntityBodyValue::Operation(operation) => match program.body(&operation.block)? {
            EntityBodyValue::Block(block) => Some(block.function),
            _ => None,
        },
        EntityBodyValue::Parameter(parameter) => match program.body(&parameter.owner)? {
            EntityBodyValue::Function(_) => Some(parameter.owner),
            EntityBodyValue::Block(block) => Some(block.function),
            _ => None,
        },
        _ => None,
    }
}

fn find_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &["--kind", "--after"], &[])?;
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let map = name_map(&workspace)?;
    let selected = select(&workspace, &head, &map, words.value("--after"))?;
    let pattern = words.positional.first().map_or("", String::as_str);
    let kind = words.value("--kind");
    let mut rows = Vec::new();
    for (name, id) in selected.names.iter() {
        if selected.names.scope(id) != Scope::Top || !name.contains(pattern) {
            continue;
        }
        let Some(body) = selected.program.body(id) else {
            continue;
        };
        let prefix = crate::names::kind_prefix(body.kind_tag());
        if kind.is_some_and(|kind| kind != prefix) {
            continue;
        }
        let summary = match body {
            sley_mutate::value::EntityBodyValue::Function(function) => {
                let params = function
                    .parameters
                    .iter()
                    .map(|param| match selected.program.body(param) {
                        Some(sley_mutate::value::EntityBodyValue::Parameter(p)) => {
                            crate::types::render(&p.value_type, &selected.names)
                        }
                        _ => "?".to_owned(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "({params}) -> {}",
                    crate::types::render(&function.result_type, &selected.names)
                )
            }
            _ => String::new(),
        };
        rows.push((prefix, name.clone(), summary));
    }
    if global.json {
        let rows: Vec<Value> = rows
            .iter()
            .map(|(kind, name, summary)| json!({"kind": kind, "name": name, "signature": summary}))
            .collect();
        write_json(out, &json!({"entities": rows}))?;
    } else {
        let mut text = String::new();
        for (kind, name, summary) in rows {
            let _ = writeln!(text, "{kind} {name}{summary}");
        }
        write_text(out, &text)?;
    }
    Ok(EXIT_OK)
}

/// Reads a frame argument: a path, `-` for standard input, or inline JSON.
fn read_json_argument(argument: &str) -> Result<Value> {
    let text = if argument == "-" {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| io(Path::new("<stdin>"), &error))?;
        text
    } else if argument.trim_start().starts_with('{') || argument.trim_start().starts_with('[') {
        argument.to_owned()
    } else {
        fs::read_to_string(argument).map_err(|error| io(Path::new(argument), &error))?
    };
    serde_json::from_str(&text)
        .map_err(|error| crate::error::frame("", format!("not JSON: {error}")))
}

#[allow(clippy::too_many_lines)]
fn try_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &["--public"], &["--no-test", "--all-tests"])?;
    let [frame_argument] = words.positional.as_slice() else {
        return Err(usage(
            "try <frame.json | - | '{\"af1\":1,...}'> [--no-test] [--all-tests] [--public file]",
        ));
    };
    let frame_value = read_json_argument(frame_argument)?;
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let authority = Authority::of(&head)?;
    let mut map = name_map(&workspace)?;
    let names = Names::build(head.program(), &map);
    let nonce = candidate::fresh_nonce()?;
    let compiled = if frame_value.is_array() {
        let (ops, new_names) = crate::raw::compile(head.program(), &names, &frame_value, nonce)?;
        let count = |class: sley_mutate::MutationClass| {
            ops.iter().filter(|op| op.payload.class() == class).count()
        };
        frame::Compiled {
            created: count(sley_mutate::MutationClass::CreateEntity),
            replaced: count(sley_mutate::MutationClass::ReplaceEntityVersion),
            deleted: count(sley_mutate::MutationClass::DeleteEntityBinding),
            ops,
            names: new_names,
            ..frame::Compiled::default()
        }
    } else {
        frame::compile(
            head.program(),
            &names,
            &authority.ceilings,
            &frame_value,
            nonce,
            &mut candidate::random32,
        )?
    };
    let counts = (compiled.created, compiled.replaced, compiled.deleted);
    let imported = candidate::assemble(&head, &authority, nonce, compiled.ops)?;
    let output = candidate::validate(&head, &authority, &imported.stored_bytes)?;
    remember_names(&workspace, &compiled.names)?;
    map.extend(&compiled.names);
    let program = candidate::proposed_program(&head, &output)
        .or_else(|| candidate::applied_program(&head, &imported))
        .unwrap_or_else(|| head.program().clone());
    let after_names = Names::build(&program, &map);
    let verdict = Verdict::of(&output, &program, &after_names);
    let store = Store::open(&workspace)?;
    let handle = store.save(
        &imported.stored_bytes,
        &json!({
            "base": crate::hex::encode(head.transaction_id().as_bytes()),
            "ops": {"created": counts.0, "replaced": counts.1, "deleted": counts.2},
            "verdict": verdict.to_json(),
            "notes": compiled.notes,
        }),
    )?;
    let mut tests = Vec::new();
    let mut public = Vec::new();
    if output.is_valid() && !words.has("--no-test") {
        let mut executor = Executor::new(&program)?;
        let chosen: Vec<_> = executor
            .tests()
            .iter()
            .filter(|test| {
                words.has("--all-tests")
                    || output
                        .result()
                        .record
                        .selected_tests
                        .contains(&test.entity_id)
                    || compiled.tests.contains(&test.entity_id)
            })
            .cloned()
            .collect();
        for test in &chosen {
            tests.push(executor.run_test(test, &after_names));
        }
        if let Some(path) = words.value("--public") {
            public = run_public(&mut executor, &program, &after_names, Path::new(path))?;
        }
    }
    let failed =
        tests.iter().filter(|t| !t.passed()).count() + public.iter().filter(|p| !p.passed).count();
    let status = if verdict.valid && failed == 0 {
        EXIT_OK
    } else {
        EXIT_NEGATIVE
    };
    if global.json {
        write_json(
            out,
            &json!({
                "handle": handle,
                "ops": {"created": counts.0, "replaced": counts.1, "deleted": counts.2},
                "verdict": verdict.to_json(),
                "tests": tests_json(&tests, &after_names),
                "public": public_json(&public),
                "notes": compiled.notes,
            }),
        )?;
    } else {
        let mut text = format!(
            "{handle}: {} (+{} created, {} replaced, {} deleted)\n{}",
            verdict.headline(),
            counts.0,
            counts.1,
            counts.2,
            verdict.details()
        );
        for note in &compiled.notes {
            let _ = writeln!(text, "  note: {note}");
        }
        text.push_str(&tests_text(&tests, &after_names));
        text.push_str(&public_text(&public));
        if verdict.valid && failed == 0 {
            let _ = writeln!(text, "next: sley-agent submit {handle}");
        } else if !verdict.valid {
            let _ = writeln!(text, "more: sley-agent explain {handle}");
        }
        write_text(out, &text)?;
    }
    Ok(status)
}

fn sorted<'a>(tests: &'a [TestOutcome], names: &Names) -> Vec<&'a TestOutcome> {
    let mut sorted: Vec<&TestOutcome> = tests.iter().collect();
    sorted.sort_by_key(|test| names.name(&test.test));
    sorted
}

fn tests_json(tests: &[TestOutcome], names: &Names) -> Value {
    Value::Array(
        sorted(tests, names)
            .into_iter()
            .map(|test| {
                json!({
                    "test": names.name(&test.test),
                    "target": names.name(&test.target),
                    "pass": test.passed(),
                    "expected": test.expected,
                    "actual": test.actual,
                })
            })
            .collect(),
    )
}

fn tests_text(tests: &[TestOutcome], names: &Names) -> String {
    if tests.is_empty() {
        return String::new();
    }
    let passed = tests.iter().filter(|t| t.passed()).count();
    let mut text = format!("tests: {passed}/{} passed\n", tests.len());
    for test in sorted(tests, names) {
        if test.passed() {
            let _ = writeln!(text, "  ok   {} = {}", names.name(&test.test), test.actual);
        } else {
            let _ = writeln!(
                text,
                "  FAIL {} ({}): expected {}, got {}",
                names.name(&test.test),
                names.name(&test.target),
                test.expected,
                test.actual
            );
        }
    }
    text
}

/// One public case from a `public_tests.json` file.
struct PublicOutcome {
    name: String,
    passed: bool,
    expected: Value,
    actual: Value,
}

fn run_public(
    executor: &mut Executor,
    program: &Program,
    names: &Names,
    path: &Path,
) -> Result<Vec<PublicOutcome>> {
    let text = fs::read_to_string(path).map_err(|error| io(path, &error))?;
    let cases: Value = serde_json::from_str(&text).map_err(|error| {
        AgentError::new(
            AgentErrorCode::InputInvalid,
            format!("{}: {error}", path.display()),
        )
    })?;
    let cases = cases.as_array().ok_or_else(|| {
        AgentError::new(
            AgentErrorCode::InputInvalid,
            "public cases are a JSON array",
        )
    })?;
    let mut outcomes = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let name = case
            .get("name")
            .and_then(Value::as_str)
            .map_or_else(|| format!("case{index}"), str::to_owned);
        let function = case
            .get("function")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AgentError::new(
                    AgentErrorCode::InputInvalid,
                    format!("case {index}: missing \"function\""),
                )
            })?;
        let id = names
            .resolve(function)
            .ok_or_else(|| unknown_name(function))?;
        let args = case
            .get("args")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let expected = case.get("expect").cloned().unwrap_or(Value::Null);
        let inputs = typed_inputs(executor, program, names, &id, &args)?;
        let outcome = executor.run(&id, inputs, exec::call_limits())?;
        let actual = exec::termination_json(&outcome.termination, names);
        outcomes.push(PublicOutcome {
            passed: actual == expected,
            name,
            expected,
            actual,
        });
    }
    Ok(outcomes)
}

fn public_json(outcomes: &[PublicOutcome]) -> Value {
    Value::Array(
        outcomes
            .iter()
            .map(|p| json!({"name": p.name, "pass": p.passed, "expected": p.expected, "actual": p.actual}))
            .collect(),
    )
}

fn public_text(outcomes: &[PublicOutcome]) -> String {
    if outcomes.is_empty() {
        return String::new();
    }
    let passed = outcomes.iter().filter(|p| p.passed).count();
    let mut text = format!("public: {passed}/{} passed\n", outcomes.len());
    for outcome in outcomes.iter().filter(|p| !p.passed) {
        let _ = writeln!(
            text,
            "  FAIL {}: expected {}, got {}",
            outcome.name, outcome.expected, outcome.actual
        );
    }
    text
}

fn typed_inputs(
    executor: &Executor,
    program: &Program,
    names: &Names,
    function: &EntityId,
    args: &[Value],
) -> Result<Vec<ConstValue>> {
    let types: Vec<TypeExpr> = executor.parameter_types(function);
    if types.len() != args.len() {
        return Err(AgentError::new(
            AgentErrorCode::InputInvalid,
            format!(
                "{} takes {} argument(s), got {}",
                names.name(function),
                types.len(),
                args.len()
            ),
        ));
    }
    let defs = ProgramTypes { program, names };
    args.iter()
        .zip(&types)
        .enumerate()
        .map(|(index, (arg, ty))| values::read(arg, ty, &defs, &format!("arg {index}")))
        .collect()
}

fn submit_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &[], &[])?;
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let store = Store::open(&workspace)?;
    let reference = match words.positional.as_slice() {
        [] => store
            .latest()?
            .ok_or_else(|| AgentError::new(AgentErrorCode::HandleUnknown, "no candidates yet"))?,
        [reference] => reference.clone(),
        _ => return Err(usage("submit [<handle>]")),
    };
    let stored = store.load(&reference)?;
    let authority = Authority::of(&head)?;
    let output = candidate::validate(&head, &authority, &stored)?;
    if !output.is_valid() {
        return Err(AgentError::new(
            AgentErrorCode::SubmissionRefused,
            format!(
                "{reference} is not Valid ({:?}); only Valid candidates are submitted",
                output.result().record.decision
            ),
        ));
    }
    let path = workspace.dir().join(SUBMISSION);
    fs::write(&path, format!("{}\n", crate::hex::encode(&stored)))
        .map_err(|error| io(&path, &error))?;
    let marker = workspace.state_dir()?.join("submission");
    fs::write(&marker, format!("{reference}\n")).map_err(|error| io(&marker, &error))?;
    let tests = output.result().record.selected_tests.len();
    if global.json {
        write_json(
            out,
            &json!({"submitted": reference, "file": SUBMISSION, "bytes": stored.len(), "selected_tests": tests}),
        )?;
    } else {
        write_text(
            out,
            &format!(
                "submitted {reference} -> {SUBMISSION} ({} bytes); resubmit any time, the last submission wins\n",
                stored.len()
            ),
        )?;
    }
    Ok(EXIT_OK)
}

fn status_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let _ = words(args, &[], &[])?;
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let map = name_map(&workspace)?;
    let path = workspace.dir().join(SUBMISSION);
    if !path.is_file() {
        let text = "no submission yet (sley-agent submit <handle>)\n";
        if global.json {
            write_json(out, &json!({"submission": null}))?;
        } else {
            write_text(out, text)?;
        }
        return Ok(EXIT_NEGATIVE);
    }
    let handle = fs::read_to_string(workspace.dir().join(STATE_DIR).join("submission"))
        .ok()
        .map(|text| text.trim().to_owned());
    let reference = path.to_string_lossy().into_owned();
    let selected = select(&workspace, &head, &map, Some(&reference))?;
    let mut executor = if selected.valid {
        Some(Executor::new(&selected.program)?)
    } else {
        None
    };
    let tests: Vec<TestOutcome> = match executor.as_mut() {
        Some(executor) => executor
            .tests()
            .to_vec()
            .iter()
            .filter(|test| selected.chosen_tests.contains(&test.entity_id))
            .map(|test| executor.run_test(test, &selected.names))
            .collect(),
        None => Vec::new(),
    };
    let store = Store::open(&workspace)?;
    let mut text = view::header(&selected.program, handle.as_deref());
    if let Some(handle) = &handle {
        for id in affected(&store, &head, handle, &selected)? {
            text.push_str(&view::entity(
                &selected.program,
                &selected.names,
                &id,
                ViewOptions::default(),
            ));
        }
    }
    let verdict = selected
        .verdict
        .as_ref()
        .map_or_else(|| "unknown".to_owned(), Verdict::to_text);
    if global.json {
        write_json(
            out,
            &json!({
                "submission": handle,
                "verdict": selected.verdict.as_ref().map(Verdict::to_json),
                "tests": tests_json(&tests, &selected.names),
                "view": text,
            }),
        )?;
    } else {
        let _ = writeln!(
            text,
            "submission: {} ({verdict})",
            handle.as_deref().unwrap_or(SUBMISSION)
        );
        text.push_str(&tests_text(&tests, &selected.names));
        write_text(out, &text)?;
    }
    Ok(if selected.valid {
        EXIT_OK
    } else {
        EXIT_NEGATIVE
    })
}

fn call_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &["--on", "--batch"], &["--stats"])?;
    let Some((function_name, raw_args)) = words.positional.split_first() else {
        return Err(usage(
            "call <fn> <arg-json>... [--on <handle|file>] [--batch file]",
        ));
    };
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let map = name_map(&workspace)?;
    let selected = select(&workspace, &head, &map, words.value("--on"))?;
    if !selected.valid {
        let verdict = selected
            .verdict
            .as_ref()
            .map_or_else(String::new, Verdict::to_text);
        return Err(AgentError::new(
            AgentErrorCode::ExecutionRefused,
            format!("the candidate is not Valid: {verdict}"),
        ));
    }
    let function = selected
        .names
        .resolve(function_name)
        .ok_or_else(|| unknown_name(function_name))?;
    let mut executor = Executor::new(&selected.program)?;
    if executor.function(&function).is_none() {
        return Err(AgentError::new(
            AgentErrorCode::ExecutionRefused,
            format!("`{function_name}` is not a function"),
        ));
    }
    if let Some(batch) = words.value("--batch") {
        return call_batch(
            &mut executor,
            &selected,
            &function,
            Path::new(batch),
            words.has("--stats"),
            out,
        );
    }
    let parsed: Vec<Value> = raw_args
        .iter()
        .map(|arg| serde_json::from_str(arg).unwrap_or_else(|_| Value::from(arg.as_str())))
        .collect();
    let arity = executor.parameter_types(&function).len();
    let parsed = match parsed.as_slice() {
        [Value::Array(items)] if arity != 1 => items.clone(),
        _ => parsed,
    };
    let inputs = typed_inputs(
        &executor,
        &selected.program,
        &selected.names,
        &function,
        &parsed,
    )?;
    let outcome = executor.run(&function, inputs, exec::call_limits())?;
    if global.json {
        write_json(
            out,
            &json!({
                "result": exec::termination_json(&outcome.termination, &selected.names),
                "fuel": outcome.fuel,
                "instructions": outcome.instructions,
                "vm_micros": outcome.micros,
            }),
        )?;
    } else {
        write_text(
            out,
            &format!(
                "{}\n",
                exec::termination_json(&outcome.termination, &selected.names)
            ),
        )?;
    }
    Ok(EXIT_OK)
}

/// Streams one JSON result line per input (a JSON array of argument lists,
/// or one argument list per line).
fn call_batch(
    executor: &mut Executor,
    selected: &Selected,
    function: &EntityId,
    path: &Path,
    stats: bool,
    out: &mut dyn Write,
) -> Result<i32> {
    let text = if path == Path::new("-") {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| io(path, &error))?;
        text
    } else {
        fs::read_to_string(path).map_err(|error| io(path, &error))?
    };
    let rows: Vec<Value> = match serde_json::from_str::<Value>(&text) {
        Ok(Value::Array(rows)) if rows.iter().all(Value::is_array) => rows,
        _ => text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str(line).map_err(|error| {
                    AgentError::new(AgentErrorCode::InputInvalid, format!("batch line: {error}"))
                })
            })
            .collect::<Result<_>>()?,
    };
    let mut buffer = String::with_capacity(rows.len() * 16);
    let mut vm_micros = 0_u64;
    for row in &rows {
        let args = row.as_array().ok_or_else(|| {
            AgentError::new(
                AgentErrorCode::InputInvalid,
                "each batch row is an argument list",
            )
        })?;
        let inputs = typed_inputs(executor, &selected.program, &selected.names, function, args)?;
        let outcome = executor.run(function, inputs, exec::call_limits())?;
        vm_micros += outcome.micros;
        buffer.push_str(&exec::termination_json(&outcome.termination, &selected.names).to_string());
        buffer.push('\n');
    }
    if stats {
        buffer.push_str(&json!({"inputs": rows.len(), "vm_micros": vm_micros}).to_string());
        buffer.push('\n');
    }
    write_text(out, &buffer)?;
    Ok(EXIT_OK)
}

fn test_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &["--public"], &[])?;
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let map = name_map(&workspace)?;
    let reference = words.positional.first().map(String::as_str);
    let selected = select(&workspace, &head, &map, reference)?;
    if !selected.valid {
        let verdict = selected
            .verdict
            .as_ref()
            .map_or_else(String::new, Verdict::to_text);
        return Err(AgentError::new(
            AgentErrorCode::ExecutionRefused,
            format!("the candidate is not Valid: {verdict}"),
        ));
    }
    let mut executor = Executor::new(&selected.program)?;
    let tests: Vec<TestOutcome> = executor
        .tests()
        .to_vec()
        .iter()
        .map(|test| executor.run_test(test, &selected.names))
        .collect();
    let public = match words.value("--public") {
        Some(path) => run_public(
            &mut executor,
            &selected.program,
            &selected.names,
            Path::new(path),
        )?,
        None => Vec::new(),
    };
    let failed =
        tests.iter().filter(|t| !t.passed()).count() + public.iter().filter(|p| !p.passed).count();
    if global.json {
        write_json(
            out,
            &json!({"tests": tests_json(&tests, &selected.names), "public": public_json(&public)}),
        )?;
    } else {
        let mut text = tests_text(&tests, &selected.names);
        if tests.is_empty() {
            text.push_str("tests: none in this state\n");
        }
        text.push_str(&public_text(&public));
        write_text(out, &text)?;
    }
    Ok(if failed == 0 { EXIT_OK } else { EXIT_NEGATIVE })
}

fn explain_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &[], &[])?;
    let reference = words.positional.first().map_or("latest", String::as_str);
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let map = name_map(&workspace)?;
    let selected = select(&workspace, &head, &map, Some(reference))?;
    let verdict = selected
        .verdict
        .clone()
        .ok_or_else(|| AgentError::new(AgentErrorCode::HandleUnknown, "no candidate"))?;
    let store = Store::open(&workspace)?;
    let label = selected.label.clone().unwrap_or_default();
    let mut view_text = String::new();
    if !verdict.valid {
        for id in affected(&store, &head, &label, &selected)?
            .into_iter()
            .take(3)
        {
            view_text.push_str(&view::entity(
                &selected.program,
                &selected.names,
                &id,
                ViewOptions::default(),
            ));
        }
    }
    if global.json {
        write_json(
            out,
            &json!({"handle": label, "verdict": verdict.to_json(), "view": view_text}),
        )?;
    } else {
        write_text(out, &format!("{label}: {}\n{view_text}", verdict.to_text()))?;
    }
    Ok(if verdict.valid {
        EXIT_OK
    } else {
        EXIT_NEGATIVE
    })
}

fn init_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &["--seed"], &[])?;
    let dir = match (words.positional.as_slice(), &global.workspace) {
        ([dir], _) => PathBuf::from(dir),
        ([], Some(dir)) => dir.clone(),
        ([], None) => std::env::current_dir().map_err(|error| io(Path::new("."), &error))?,
        _ => return Err(usage("init [<dir>] [--seed <64-hex>]")),
    };
    let seed = match words.value("--seed") {
        Some(text) => {
            Some(crate::hex::decode32(text).ok_or_else(|| usage("--seed takes 64 lowercase hex"))?)
        }
        None => None,
    };
    let workspace = crate::genesis::init(&dir, seed, crate::genesis::INIT_CEILINGS)?;
    let head = workspace.head()?;
    let authority = Authority::of(&head)?;
    let ceilings = authority.ceilings;
    if global.json {
        write_json(
            out,
            &json!({
                "workspace": workspace.dir().display().to_string(),
                "ceilings": {"fuel": ceilings.max_fuel, "memory_bytes": ceilings.max_memory_bytes,
                             "output_bytes": ceilings.max_output_bytes, "mutations": ceilings.max_mutation_count},
            }),
        )?;
    } else {
        write_text(
            out,
            &format!(
                "initialized {} (policy: fuel {}, memory {}, output {}, {} mutations per candidate)\n",
                workspace.dir().display(),
                ceilings.max_fuel,
                ceilings.max_memory_bytes,
                ceilings.max_output_bytes,
                ceilings.max_mutation_count
            ),
        )?;
    }
    Ok(EXIT_OK)
}

fn commit_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &[], &[])?;
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let store = Store::open(&workspace)?;
    let reference = match words.positional.as_slice() {
        [] => store
            .latest()?
            .ok_or_else(|| AgentError::new(AgentErrorCode::HandleUnknown, "no candidates yet"))?,
        [reference] => reference.clone(),
        _ => return Err(usage("commit [<handle>]")),
    };
    let stored = store.load(&reference)?;
    let transaction = crate::genesis::commit(&head, &workspace.repo(), &stored)?;
    let transaction = crate::hex::encode(transaction.as_bytes());
    if global.json {
        write_json(
            out,
            &json!({"committed": reference, "transaction": transaction}),
        )?;
    } else {
        write_text(
            out,
            &format!(
                "committed {reference} as transaction {}\n",
                &transaction[..8]
            ),
        )?;
    }
    Ok(EXIT_OK)
}

fn export_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &[], &[])?;
    let [path] = words.positional.as_slice() else {
        return Err(usage("export <file.pack>"));
    };
    let workspace = workspace(global)?;
    workspace.ensure_seeded()?;
    let bytes = crate::genesis::export(&workspace, Path::new(path))?;
    if global.json {
        write_json(out, &json!({"exported": path, "bytes": bytes}))?;
    } else {
        write_text(out, &format!("exported {path} ({bytes} bytes)\n"))?;
    }
    Ok(EXIT_OK)
}

fn help_command(args: &[String], out: &mut dyn Write) -> Result<i32> {
    let topic = args.first().map_or("", String::as_str);
    let text = help::topic(topic).ok_or_else(|| {
        usage(format!(
            "unknown help topic `{topic}`; topics: {}",
            help::TOPICS.join(", ")
        ))
    })?;
    write_text(out, &text)?;
    Ok(EXIT_OK)
}
