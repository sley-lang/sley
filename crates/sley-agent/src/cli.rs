//! Command-line dispatch for `sley-agent`.
//!
//! Every command prints compact text by default and one JSON object under
//! `--json`. Exit status: 0 success, 1 a negative outcome (refused
//! candidate, failing test), 2 a workbench refusal (`AGENT_*`).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::{Read as _, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};
use sley_id::EntityId;
use sley_ssmc::{ConstValue, TypeExpr};

use crate::candidate::{self, Authority, Store};
use crate::catalog::Verdict;
use crate::draft::{self, DraftRef, Drafts, Revision, State};
use crate::error::{AgentError, AgentErrorCode, Result, io, unknown_name, usage};
use crate::events::Event;
use crate::exec::{self, Executor, TestOutcome};
use crate::focus;
use crate::frame;
use crate::help;
use crate::locate::Source;
use crate::names::{NameMap, Names, Scope};
use crate::values::{self, ProgramTypes};
use crate::view::{self, ViewOptions};
use crate::workspace::{Head, NAMES_FILE, Program, STATE_DIR, SUBMISSION, Workspace};
use crate::xview;

/// Every command `dispatch` accepts (help examples are checked against it).
pub const COMMANDS: &[&str] = &[
    "view", "find", "try", "fill", "import", "draft", "submit", "status", "call", "test",
    "explain", "search", "init", "commit", "export", "help", "version",
];

/// Exit status for success.
pub const EXIT_OK: i32 = 0;
/// Exit status for a negative outcome (refused candidate, failing test).
pub const EXIT_NEGATIVE: i32 = 1;
/// Exit status for a workbench refusal.
pub const EXIT_REFUSED: i32 = 2;

/// Parsed global options, and the command's events-ledger counters.
#[derive(Clone, Debug, Default)]
struct Global {
    workspace: Option<PathBuf>,
    json: bool,
    event: RefCell<Event>,
}

impl Global {
    /// Sets one events-ledger counter of this command.
    fn note(&self, key: &str, value: impl Into<Value>) {
        self.event.borrow_mut().set(key, value);
    }
}

/// Counts the bytes a command prints (the ledger's `output_bytes`).
struct Counted<'a> {
    out: &'a mut dyn Write,
    bytes: usize,
}

impl Write for Counted<'_> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let written = self.out.write(buffer)?;
        self.bytes += written;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}

/// Set when the process exits as soon as the command returns.
static EXITING: AtomicBool = AtomicBool::new(false);

/// `run` for a process that exits with the returned status right away (the
/// binary): a command may then leave its in-memory state to the exit
/// instead of freeing it piece by piece. Output and status are `run`'s.
pub fn run_then_exit(args: &[String], out: &mut dyn Write) -> i32 {
    EXITING.store(true, Ordering::Relaxed);
    run(args, out)
}

/// Frees command state, or leaves it to the imminent process exit.
fn release<T>(state: T) {
    if EXITING.load(Ordering::Relaxed) {
        std::mem::forget(state);
    }
}

/// Runs one command line and returns the exit status.
pub fn run(args: &[String], out: &mut dyn Write) -> i32 {
    let mut global = Global::default();
    let mut rest = Vec::new();
    let mut words = args.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--workspace" | "-C" if global.workspace.is_some() => {
                return refuse(
                    out,
                    &usage("--workspace is given twice; give it once"),
                    false,
                );
            }
            "--workspace" | "-C" => match words.next() {
                Some(dir) => global.workspace = Some(PathBuf::from(dir)),
                None => return refuse(out, &usage("--workspace needs a directory"), false),
            },
            "--json" if global.json => {
                return refuse(out, &usage("--json is given twice; give it once"), true);
            }
            "--json" => global.json = true,
            _ => rest.push(word.clone()),
        }
    }
    let words: usize = rest.iter().map(String::len).sum();
    global.note("input_bytes", words + rest.len().saturating_sub(1));
    let mut counted = Counted { out, bytes: 0 };
    let status = match dispatch(&global, &rest, &mut counted) {
        Ok(status) => status,
        Err(error) => {
            global.note("refusal", error.code().symbol());
            refuse(&mut counted, &error, global.json)
        }
    };
    let command = rest.first().map_or("", String::as_str);
    crate::events::append(&global.event.borrow(), command, counted.bytes);
    status
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
    let workspace = if let Some(dir) = &global.workspace {
        Workspace::at(dir.clone())
    } else {
        let cwd = std::env::current_dir().map_err(|error| io(Path::new("."), &error))?;
        Workspace::locate(&cwd)?
    };
    global.event.borrow_mut().at(workspace.dir().to_path_buf());
    Ok(workspace)
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
    // Concurrent commands each merge into the map; a merge another command
    // overwrote is merged again.
    let path = workspace.state_dir()?.join(NAMES_FILE);
    for _ in 0..8 {
        let mut map = NameMap::read(&path)?;
        map.extend(names);
        map.write(&path)?;
        let written = NameMap::read(&path)?;
        let mut merged = written.clone();
        merged.extend(names);
        if merged == written {
            return Ok(());
        }
    }
    Ok(())
}

fn dispatch(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let Some((command, rest)) = args.split_first() else {
        return help_command(&[], out);
    };
    match command.as_str() {
        "view" => view_command(global, rest, out),
        "find" => find_command(global, rest, out),
        "try" => try_command(global, rest, out),
        "fill" => fill_command(global, rest, out),
        "import" => import_command(global, rest, out),
        "draft" => draft_command(global, rest, out),
        "submit" => submit_command(global, rest, out),
        "status" => status_command(global, rest, out),
        "call" => call_command(global, rest, out),
        "test" => test_command(global, rest, out),
        "explain" => explain_command(global, rest, out),
        "search" => search_command(global, rest, out),
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
        let known = valued.contains(&word.as_str()) || switches.contains(&word.as_str());
        if known && flags.iter().any(|(name, _): &(String, _)| name == word) {
            return Err(usage(format!("{word} is given twice; give it once")));
        }
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
    let reference = store.resolve(Some(reference))?;
    let stored = store.load(&reference)?;
    let authority = Authority::of(head)?;
    let output = candidate::validate(head, &authority, &stored)?;
    // Only a refused candidate needs its own import (for the pure apply).
    let program = candidate::proposed_program(head, &output)
        .or_else(|| {
            sley_mutate::import_candidate(&stored)
                .ok()
                .and_then(|c| candidate::applied_program(head, &c))
        })
        .unwrap_or_else(|| head.program().clone());
    let names = Names::build(&program, map);
    let mut verdict = Verdict::of(&output, &program, &names);
    let meta = store.meta(&reference).unwrap_or_default();
    verdict.locate(&output, &Source::of_meta(&meta), &program, &names);
    Ok(Selected {
        valid: output.is_valid(),
        chosen_tests: output.result().record.selected_tests.clone(),
        program,
        names,
        label: Some(reference),
        verdict: Some(verdict),
    })
}

/// A candidate reference as shown to the reader: a handle as is, a file by
/// its name, raw hex by its first eight digits.
fn shown(reference: &str) -> String {
    let path = Path::new(reference);
    if reference.contains('/') {
        return path.file_name().map_or_else(
            || reference.to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
    }
    if reference.chars().count() > 16 {
        return format!("{}…", reference.chars().take(8).collect::<String>());
    }
    reference.to_owned()
}

fn view_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(
        args,
        &["--after", "--focus"],
        &["--ids", "--types", "--limits", "--package", "--x"],
    )?;
    let options = ViewOptions {
        ids: words.has("--ids"),
        types: words.has("--types"),
        limits: words.has("--limits"),
    };
    let x = words.has("--x");
    let focus = words.value("--focus");
    if focus.is_some() && (!words.positional.is_empty() || words.has("--package")) {
        return Err(usage(
            "--focus takes one name; view further names without --focus",
        ));
    }
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let map = name_map(&workspace)?;
    let selected = select(&workspace, &head, &map, words.value("--after"))?;
    let shown_label = selected.label.as_deref().map(shown);
    let mut text = if x {
        view::header_x(&selected.program, shown_label.as_deref())
    } else {
        view::header(&selected.program, shown_label.as_deref())
    };
    let render = |id: &EntityId| {
        if x {
            xview::entity(&selected.program, &selected.names, id, options)
        } else {
            view::entity(&selected.program, &selected.names, id, options)
        }
    };
    if let Some(target) = focus {
        let id = selected
            .names
            .resolve(target)
            .ok_or_else(|| unknown_name(target))?;
        // Expansion commands repeat the reference: the resolved handle, or
        // the reference as given when it is a plain word.
        let after = selected.label.as_deref().map(|label| {
            let plain = label.len() <= 80
                && label
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c));
            if plain {
                label.to_owned()
            } else {
                "<ref>".to_owned()
            }
        });
        let focused = focus::render(
            &selected.program,
            &selected.names,
            &id,
            &focus::FocusRequest {
                header: text,
                options,
                x,
                after,
            },
        );
        if global.json {
            write_json(out, &json!({"view": focused.text, "focus": focused.json}))?;
        } else {
            write_text(out, &focused.text)?;
        }
        return Ok(EXIT_OK);
    }
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
            text.push_str(&render(&id));
        }
    } else if words.has("--package") || words.positional.is_empty() {
        if x {
            text.push_str(&xview::package(&selected.program, &selected.names, options));
        } else {
            text.push_str(&view::package(&selected.program, &selected.names, options));
        }
    } else {
        for target in &words.positional {
            let id = selected
                .names
                .resolve(target)
                .ok_or_else(|| unknown_name(target))?;
            text.push_str(&render(&id));
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

/// Whether a candidate creates, replaces or deletes any part of a function.
fn changes_a_function(
    head: &Head,
    output: &sley_policy::CandidateValidationOutput,
    stored: &[u8],
) -> bool {
    let Ok(candidate) = sley_mutate::import_candidate(stored) else {
        return false;
    };
    let after = candidate::proposed_program(head, output);
    candidate.record.operations.iter().any(|operation| {
        let target = operation.target_entity;
        after
            .as_ref()
            .and_then(|program| owner_function(program, &Names::default(), &target))
            .or_else(|| owner_function(head.program(), &Names::default(), &target))
            .is_some()
    })
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

/// Reads an input argument's exact bytes: a path, `-` for standard input,
/// or inline JSON.
fn read_argument(argument: &str) -> Result<Vec<u8>> {
    if argument == "-" {
        let mut bytes = Vec::new();
        std::io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| io(Path::new("<stdin>"), &error))?;
        Ok(bytes)
    } else if argument.trim_start().starts_with('{') || argument.trim_start().starts_with('[') {
        Ok(argument.as_bytes().to_vec())
    } else {
        fs::read(argument).map_err(|error| io(Path::new(argument), &error))
    }
}

/// Where an input failed to parse as JSON.
struct TextFailure {
    detail: String,
    line: usize,
    column: usize,
    byte: usize,
}

fn parse_input(bytes: &[u8]) -> std::result::Result<Value, TextFailure> {
    serde_json::from_slice(bytes).map_err(|error| {
        let (line, column, byte) = draft::parse_location(bytes, &error);
        TextFailure {
            detail: error.to_string(),
            line,
            column,
            byte,
        }
    })
}

/// The switches of every command that makes a draft revision.
const TRIAL_SWITCHES: [&str; 5] = ["--no-test", "--all-tests", "--raw", "--verbose", "--rebase"];

/// The options of the commands that make a draft revision.
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
struct TrialOptions {
    no_test: bool,
    all_tests: bool,
    raw: bool,
    verbose: bool,
    public: Option<String>,
}

impl TrialOptions {
    fn of(words: &Words) -> Self {
        Self {
            no_test: words.has("--no-test"),
            all_tests: words.has("--all-tests"),
            raw: words.has("--raw"),
            verbose: words.has("--verbose"),
            public: words.value("--public").map(str::to_owned),
        }
    }
}

/// The draft a revision goes to.
enum Target {
    /// A new draft (`dN@r1`).
    New,
    /// The next revision of `handle`, built on its revision `parent`; with
    /// `latest`, `parent` was the latest revision and the new revision must
    /// directly follow it.
    Next {
        handle: String,
        parent: u64,
        latest: bool,
    },
}

/// What a revision is made of.
enum Input {
    /// The complete frame (layered, when the revision builds on a base).
    Frame(Value),
    /// Input that is not JSON.
    Text(TextFailure),
    /// A parseable follow-up that could not be layered on its base.
    Unlayered { follow_up: Value, error: AgentError },
}

impl Input {
    fn of(parsed: std::result::Result<Value, TextFailure>) -> Self {
        match parsed {
            Ok(frame) => Self::Frame(frame),
            Err(failure) => Self::Text(failure),
        }
    }

    /// A follow-up layered on `base`; a follow-up that is not JSON or that
    /// layering refuses is kept as it is.
    fn layered(base: &Value, parsed: std::result::Result<Value, TextFailure>) -> Self {
        match parsed {
            Ok(follow_up) => match crate::layer::layer(base, &follow_up) {
                Ok(frame) => Self::Frame(frame),
                Err(error) => Self::Unlayered { follow_up, error },
            },
            Err(failure) => Self::Text(failure),
        }
    }
}

/// Where the pointers of a frame refusal point.
enum Origin {
    /// An inline frame: the frame as given.
    Inline,
    /// A frame file, best edited in place.
    File(String),
    /// A frame layered on the frame of a candidate (`.sley/layered.json`).
    Candidate(String),
    /// The draft revision's `frame.json`.
    Draft,
}

/// One draft revision to record and try.
struct Proposal {
    made_by: &'static str,
    input: Vec<u8>,
    frame: Input,
    target: Target,
    origin: Origin,
    on: Option<String>,
    delta: Option<Value>,
    whole_frame: bool,
    rebase: Option<Value>,
    sources: Vec<Value>,
    tables: Value,
    import: Option<Value>,
}

impl Proposal {
    fn new(
        made_by: &'static str,
        input: Vec<u8>,
        frame: Input,
        target: Target,
        origin: Origin,
    ) -> Self {
        Self {
            made_by,
            input,
            frame,
            target,
            origin,
            on: None,
            delta: None,
            whole_frame: false,
            rebase: None,
            sources: Vec::new(),
            tables: json!({}),
            import: None,
        }
    }

    /// Carries a lineage's imported-test sources and table-made tests.
    fn inherit(&mut self, status: &Value) {
        self.sources = status["sources"].as_array().cloned().unwrap_or_default();
        if status["tables"].is_object() {
            self.tables = status["tables"].clone();
        }
    }
}

/// Refuses to build on a revision made on another head unless the author
/// asked to rebase; the rebase record names both heads.
fn head_check(
    status: &Value,
    head: &Head,
    revision: &str,
    rebase: bool,
    via: &str,
) -> Result<Option<Value>> {
    let base = status["base_head"].as_str().unwrap_or("");
    let now = crate::hex::encode(head.transaction_id().as_bytes());
    if base == now {
        return Ok(None);
    }
    if !rebase {
        let short = |hex: &str| hex.chars().take(8).collect::<String>();
        return Err(AgentError::new(
            AgentErrorCode::DraftHeadChanged,
            format!(
                "{revision} was made on head {} and the head is now {}; build on the new head explicitly with --rebase",
                short(base),
                short(&now)
            ),
        ));
    }
    Ok(Some(json!({"from_head": base, "to_head": now, "via": via})))
}

/// The frame and status of a revision to build on. A text revision, and a
/// follow-up that was never layered, have no complete frame to build on.
fn layer_base(drafts: &Drafts, handle: &str, revision: u64) -> Result<(Value, Value)> {
    let status = drafts.status(handle, revision)?;
    let frame = drafts.frame(handle, revision)?;
    let unlayered = status["unlayered"] == true;
    if let (Some(frame), false) = (&frame, unlayered) {
        return Ok((frame.clone(), status));
    }
    let spelled = draft::spell(handle, revision);
    let repair = format!("sley-agent fill {handle} <delta.json> --revision {revision}");
    let detail = match (status["on"].as_str().filter(|_| unlayered), frame) {
        (Some(on), None) => format!(
            "{spelled} holds a follow-up that is not JSON, so nothing can be layered on it; fix the JSON: {repair} with {{\"set\": [{{\"at\": \"\", \"value\": <the follow-up>}}]}} (it is layered on {on} again), or layer on {on} again: sley-agent try --on {on} <follow-up>"
        ),
        (Some(on), Some(_)) => format!(
            "{spelled} holds a follow-up that could not be layered on {on}, so nothing can be layered on it; repair the follow-up: {repair} (it is layered on {on} again), or layer on {on} again: sley-agent try --on {on} <follow-up>"
        ),
        _ => format!(
            "{spelled} is a text draft (its input is not JSON), so nothing can be layered on it; fix the JSON: {repair} with {{\"set\": [{{\"at\": \"\", \"value\": <the frame>}}]}}"
        ),
    };
    Err(AgentError::new(AgentErrorCode::DraftIncomplete, detail))
}

/// A follow-up refused before it could be recorded (its base cannot be
/// layered on, or the head changed): the refusal says that nothing was
/// recorded and how to send the follow-up again.
fn not_recorded(error: AgentError, what: &str) -> AgentError {
    let again = match error.code() {
        AgentErrorCode::DraftHeadChanged => "send it again with --rebase",
        AgentErrorCode::DraftIncomplete => {
            "send it again once that revision is repaired, or with --on the revision named above"
        }
        _ => return error,
    };
    AgentError::new(
        error.code(),
        format!("{}; this {what} was not recorded: {again}", error.detail()),
    )
}

/// The frame a follow-up builds on: a draft revision's complete frame, or
/// the frame a candidate was made from.
fn base_frame(workspace: &Workspace, on: &str) -> Result<Value> {
    if let Some(reference) = DraftRef::parse(on) {
        let drafts = Drafts::open(workspace)?;
        let (handle, revision) = drafts.resolve(&reference)?;
        return layer_base(&drafts, &handle, revision).map(|(frame, _)| frame);
    }
    let store = Store::open(workspace)?;
    let handle = store.resolve(Some(on))?;
    store
        .meta(&handle)
        .and_then(|meta| meta.get("frame").cloned())
        .filter(|frame| !frame.is_null())
        .ok_or_else(|| {
            AgentError::new(
                AgentErrorCode::Usage,
                format!("{handle} was not made from an AF1 frame, so try --on cannot build on it"),
            )
        })
}

fn try_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &["--public", "--on"], &TRIAL_SWITCHES)?;
    let [frame_argument] = words.positional.as_slice() else {
        return Err(usage(
            "try <frame.json | - | '{\"af1\":1,...}'> [--on <handle|draft>] [--rebase] [--no-test] [--all-tests] [--public file] [--raw] [--verbose]",
        ));
    };
    let on = words.value("--on");
    if words.has("--rebase") && !on.is_some_and(draft::is_draft_ref) {
        return Err(usage("--rebase goes with try --on <draft>"));
    }
    let options = TrialOptions::of(&words);
    let input = read_argument(frame_argument)?;
    global.note("input_bytes", input.len());
    let parsed = parse_input(&input);
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let proposal = match on {
        None => {
            // A whole new frame while drafts exist restates work that a
            // layered frame or a fill would not: the ledger counts it.
            let drafted = Drafts::open(&workspace)
                .and_then(|drafts| drafts.handles())
                .is_ok_and(|handles| !handles.is_empty());
            global.note("rewrite", drafted);
            let origin = if Path::new(frame_argument).is_file() {
                Origin::File(frame_argument.clone())
            } else {
                Origin::Inline
            };
            Proposal::new("try", input, Input::of(parsed), Target::New, origin)
        }
        // `--on d1`: the next revision of d1, this frame layered on the frame
        // of its latest (or named) revision.
        Some(reference) if draft::is_draft_ref(reference) => {
            let drafts = Drafts::open(&workspace)?;
            let reference = DraftRef::parse(reference).expect("a draft reference");
            let (handle, base) = drafts.resolve(&reference)?;
            let spelled = draft::spell(&handle, base);
            let (base_frame, status) = layer_base(&drafts, &handle, base)
                .map_err(|error| not_recorded(error, "follow-up"))?;
            let rebase = head_check(&status, &head, &spelled, words.has("--rebase"), "try-on")
                .map_err(|error| not_recorded(error, "follow-up"))?;
            let target = Target::Next {
                handle,
                parent: base,
                latest: reference.revision.is_none(),
            };
            let frame = Input::layered(&base_frame, parsed);
            let mut proposal = Proposal::new("try-on", input, frame, target, Origin::Draft);
            proposal.on = Some(spelled);
            proposal.rebase = rebase;
            proposal.inherit(&status);
            proposal
        }
        // `--on c1`: this frame goes on top of the frame c1 was made from, so
        // a follow-up states only what it adds or changes; a new draft.
        Some(reference) => {
            let store = Store::open(&workspace)?;
            let handle = store.resolve(Some(reference))?;
            let base = base_frame(&workspace, &handle)?;
            let frame = Input::layered(&base, parsed);
            if let Input::Frame(layered) = &frame {
                let path = workspace.state_dir()?.join("layered.json");
                let mut text = serde_json::to_string_pretty(layered).unwrap_or_default();
                text.push('\n');
                candidate::replace_file(&path, text.as_bytes())?;
            }
            let origin = Origin::Candidate(handle.clone());
            let mut proposal = Proposal::new("try-on", input, frame, Target::New, origin);
            // Imported tests and table-made tests keep their provenance
            // through the candidate's draft.
            let lineage = store
                .meta(&handle)
                .and_then(|meta| meta["draft"].as_str().and_then(DraftRef::parse))
                .and_then(|reference| {
                    let drafts = Drafts::open(&workspace).ok()?;
                    let (handle, revision) = drafts.resolve(&reference).ok()?;
                    drafts.status(&handle, revision).ok()
                });
            if let Some(status) = lineage {
                proposal.inherit(&status);
            }
            proposal.on = Some(handle);
            proposal
        }
    };
    run_trial(global, &workspace, &head, proposal, &options, out)
}

fn fill_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    const USAGE: &str = "fill <draft> <delta.json | - | '{\"set\": [...]}'> --revision <N> [--rebase] [--no-test] [--all-tests] [--public file] [--raw] [--verbose]";
    let words = words(args, &["--revision", "--public"], &TRIAL_SWITCHES)?;
    let [reference, delta_argument] = words.positional.as_slice() else {
        return Err(usage(USAGE));
    };
    let reference = DraftRef::parse(reference)
        .ok_or_else(|| usage(format!("`{reference}` is not a draft (d1, d1@r2): {USAGE}")))?;
    let revision = match (words.value("--revision"), reference.revision) {
        (Some(text), spelled) => {
            let revision: u64 = text
                .strip_prefix('r')
                .unwrap_or(text)
                .parse()
                .map_err(|_| usage("--revision takes a revision number"))?;
            if spelled.is_some_and(|spelled| spelled != revision) {
                return Err(usage(
                    "the draft reference and --revision name different revisions",
                ));
            }
            revision
        }
        (None, Some(revision)) => revision,
        (None, None) => {
            return Err(usage(format!(
                "fill needs --revision <N>, the revision the delta was written against: {USAGE}"
            )));
        }
    };
    let options = TrialOptions::of(&words);
    let input = read_argument(delta_argument)?;
    global.note("input_bytes", input.len());
    global.note("delta_bytes", input.len());
    global.note("draft", draft::spell(&reference.handle, revision));
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let drafts = Drafts::open(&workspace)?;
    let handle = reference.handle;
    let latest = drafts.latest(&handle)?;
    if revision != latest {
        return Err(AgentError::new(
            AgentErrorCode::DraftStale,
            format!(
                "{handle} is at r{latest}, not r{revision}: read it (sley-agent draft {handle}) and write the delta against --revision {latest}"
            ),
        ));
    }
    let spelled = draft::spell(&handle, revision);
    let status = drafts.status(&handle, revision)?;
    let rebase = head_check(&status, &head, &spelled, words.has("--rebase"), "fill")?;
    let delta_value: Value = serde_json::from_slice(&input).map_err(|error| {
        AgentError::new(
            AgentErrorCode::DeltaInvalid,
            format!("the delta is not JSON: {error}"),
        )
    })?;
    let delta = draft::parse_delta(&delta_value)?;
    let base = drafts.frame(&handle, revision)?;
    let applied = draft::apply_delta(base.as_ref(), &delta, &spelled)?;
    // A follow-up that was never layered (not JSON, or refused by its base)
    // is layered on that base again once repaired.
    let on = status["on"]
        .as_str()
        .filter(|_| status["unlayered"] == true)
        .map(str::to_owned);
    let frame = match &on {
        Some(on) => Input::layered(&base_frame(&workspace, on)?, Ok(applied)),
        None => Input::Frame(applied),
    };
    let bytes = input.len();
    let target = Target::Next {
        handle,
        parent: revision,
        latest: true,
    };
    let mut proposal = Proposal::new("fill", input, frame, target, Origin::Draft);
    proposal.delta = Some(json!({"targets": delta.targets(), "bytes": bytes}));
    proposal.whole_frame = delta.whole_frame();
    proposal.rebase = rebase;
    proposal.on = on;
    proposal.inherit(&status);
    run_trial(global, &workspace, &head, proposal, &options, out)
}

/// Public cases as AF1 tests, each with its source record. Expected values
/// come from the case file only, never from running a candidate.
fn import_cases(
    bytes: &[u8],
    digest: &str,
    only: Option<&str>,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let invalid = |detail: String| AgentError::new(AgentErrorCode::InputInvalid, detail);
    let cases: Value = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("the case file is not JSON: {error}")))?;
    let cases = cases.as_array().ok_or_else(|| {
        invalid(
            "public cases are a JSON array of {\"name\", \"function\", \"args\", \"expect\"}"
                .to_owned(),
        )
    })?;
    let wanted: Option<Vec<&str>> = only.map(|list| {
        list.split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .collect()
    });
    let mut entries: Vec<Value> = Vec::new();
    let mut sources = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let name = case
            .get("name")
            .and_then(Value::as_str)
            .map_or_else(|| format!("case{index}"), str::to_owned);
        if wanted
            .as_ref()
            .is_some_and(|wanted| !wanted.contains(&name.as_str()))
        {
            continue;
        }
        if seen.contains(&name) {
            return Err(invalid(format!("case `{name}` appears twice")));
        }
        let function = case
            .get("function")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid(format!("case {index} ({name}): missing \"function\"")))?;
        let args = case.get("args").cloned().unwrap_or_else(|| json!([]));
        if !args.is_array() {
            return Err(invalid(format!(
                "case {index} ({name}): \"args\" is an array"
            )));
        }
        let expect = case.get("expect").cloned().ok_or_else(|| {
            invalid(format!(
                "case {index} ({name}) has no \"expect\": expected values come from the case file"
            ))
        })?;
        let entry = json!({"name": name, "fn": function, "args": args, "expect": expect});
        sources.push(json!({"test": name, "case": name, "sha256": digest, "entry": draft::entry_digest(&entry)}));
        entries.push(entry);
        seen.push(name);
    }
    for name in wanted.iter().flatten() {
        if !seen.iter().any(|seen| seen == name) {
            return Err(invalid(format!("no case named `{name}` in the case file")));
        }
    }
    if entries.is_empty() {
        return Err(invalid("the case file holds no case to import".to_owned()));
    }
    Ok((entries, sources))
}

fn import_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    const USAGE: &str = "import <cases.json | -> [--on <draft>] [--only name,name] [--rebase] [--no-test] [--all-tests] [--public file] [--raw] [--verbose]";
    let words = words(args, &["--on", "--only", "--public"], &TRIAL_SWITCHES)?;
    let [cases_argument] = words.positional.as_slice() else {
        return Err(usage(USAGE));
    };
    let on = match words.value("--on") {
        Some(reference) => Some(
            DraftRef::parse(reference)
                .ok_or_else(|| usage(format!("import --on takes a draft (d1, d1@r2): {USAGE}")))?,
        ),
        None => None,
    };
    if words.has("--rebase") && on.is_none() {
        return Err(usage("--rebase goes with import --on <draft>"));
    }
    let options = TrialOptions::of(&words);
    let input = read_argument(cases_argument)?;
    global.note("input_bytes", input.len());
    let digest = draft::sha256(&input);
    let (entries, mut sources) = import_cases(&input, &digest, words.value("--only"))?;
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let import = json!({"sha256": digest, "bytes": input.len(), "cases": entries.len()});
    let tests = json!({"af1": 1, "tests": entries});
    let mut proposal = match on {
        None => Proposal::new(
            "import",
            input,
            Input::Frame(tests),
            Target::New,
            Origin::Draft,
        ),
        Some(reference) => {
            let drafts = Drafts::open(&workspace)?;
            let (handle, base) = drafts.resolve(&reference)?;
            let spelled = draft::spell(&handle, base);
            let (base_frame, status) = layer_base(&drafts, &handle, base)
                .map_err(|error| not_recorded(error, "import"))?;
            let rebase = head_check(&status, &head, &spelled, words.has("--rebase"), "import")
                .map_err(|error| not_recorded(error, "import"))?;
            let frame = Input::layered(&base_frame, Ok(tests));
            let target = Target::Next {
                handle,
                parent: base,
                latest: reference.revision.is_none(),
            };
            let mut proposal = Proposal::new("import", input, frame, target, Origin::Draft);
            proposal.inherit(&status);
            // A re-imported test replaces its earlier source.
            let mut inherited = std::mem::take(&mut proposal.sources);
            inherited.retain(|old| !sources.iter().any(|new| new["test"] == old["test"]));
            inherited.append(&mut sources);
            sources = inherited;
            proposal.on = Some(spelled);
            proposal.rebase = rebase;
            proposal
        }
    };
    proposal.sources = sources;
    proposal.import = Some(import);
    run_trial(global, &workspace, &head, proposal, &options, out)
}

/// Everything before a candidate handle: compile the frame (or raw
/// operation list), assemble the record over the head, validate it.
fn stage(
    head: &Head,
    authority: &Authority,
    names: &Names,
    frame_value: &Value,
    nonce: sley_id::CandidateNonce,
) -> Result<(
    frame::Compiled,
    sley_mutate::ImportedCandidate,
    sley_policy::CandidateValidationOutput,
)> {
    let mut compiled = if frame_value.is_array() {
        let (ops, new_names) = crate::raw::compile(head.program(), names, frame_value, nonce)?;
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
            names,
            &authority.ceilings,
            frame_value,
            nonce,
            &mut candidate::random32,
        )?
    };
    if compiled.ops.is_empty() {
        return Err(AgentError::new(
            AgentErrorCode::FrameInvalid,
            "the frame changes nothing: everything it states is already live as stated",
        ));
    }
    let ops = std::mem::take(&mut compiled.ops);
    let imported = candidate::assemble(head, authority, nonce, ops)?;
    let output = candidate::validate(head, authority, &imported.stored_bytes)?;
    Ok((compiled, imported, output))
}

/// Appends where a frame refusal's pointers point, and how to fix it
/// cheaply.
fn pointer_hint(error: AgentError, origin: &Origin, frame_path: &str, draft: &str) -> AgentError {
    let hint = match origin {
        Origin::Inline => return error,
        Origin::File(path) => format!(
            "fix: edit {path} in place at those pointers (no need to rewrite it) and run try again"
        ),
        Origin::Candidate(handle) => format!(
            "pointers refer to .sley/layered.json (the frame of {handle} with yours on top); fix your frame and run try --on {handle} again"
        ),
        Origin::Draft => {
            format!("pointers refer to {frame_path} (sley-agent draft {draft} --frame prints it)")
        }
    };
    AgentError::new(error.code(), format!("{}\n  {hint}", error.detail()))
}

/// Reports a revision that made no candidate: the refusal exactly as a
/// workbench refusal prints (exit status 2), then the draft line and the
/// next step.
fn unfinished(
    global: &Global,
    out: &mut dyn Write,
    error: &AgentError,
    reference: &str,
    state: State,
    obligations: &[Value],
    lines: [String; 2],
) -> Result<i32> {
    let symbol = error.code().symbol();
    global.note("refusal", symbol);
    global.note("obligations", draft::obligation_count(obligations));
    let [summary, next] = lines;
    if global.json {
        write_json(
            out,
            &json!({
                "error": symbol,
                "detail": error.detail(),
                "draft": reference,
                "state": state.as_str(),
                "obligations": obligations,
                "next": next.strip_prefix("next: ").unwrap_or(&next),
            }),
        )?;
    } else {
        write_text(
            out,
            &format!("error {symbol}: {}\n{summary}\n{next}\n", error.detail()),
        )?;
    }
    Ok(EXIT_REFUSED)
}

/// `TestCases` live at the head; with `after`, only those it keeps and
/// does not replace (a replaced test counts where its new entry comes from).
fn provided_tests(head: &Head, after: Option<&Program>, replaced: &[EntityId]) -> usize {
    head.program()
        .objects()
        .iter()
        .filter(|object| object.record().body.kind_tag() == 14)
        .map(|object| object.record().entity_id)
        .filter(|id| after.is_none_or(|program| program.contains(id)))
        .filter(|id| !replaced.contains(id))
        .count()
}

/// The live `TestCase` a name denotes at the head, if any.
fn live_test(head: &Head, names: &Names, name: &str) -> Option<EntityId> {
    names.resolve(name).filter(|id| {
        names.scope(id) == Scope::Top
            && head
                .program()
                .body(id)
                .is_some_and(|body| body.kind_tag() == 14)
    })
}

/// One refusal holding the problems of two: `first`'s code and headline,
/// then every other line, prefixed with its own symbol where it differs.
fn merge_refusals(first: &AgentError, second: &AgentError) -> AgentError {
    let code = first.code();
    let mut lines = frame::problem_lines(first);
    for line in frame::problem_lines(second) {
        let line = if second.code() == code || line.starts_with('[') {
            line
        } else {
            format!("[{}] {line}", second.code().symbol())
        };
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    let headline = lines.remove(0);
    if lines.is_empty() {
        return AgentError::new(code, headline);
    }
    AgentError::new(
        code,
        format!(
            "{headline} (1 of {} problems)\n  {}",
            lines.len() + 1,
            lines.join("\n  ")
        ),
    )
}

/// What a frame's test tables do to the `TestCases` live at the head.
#[derive(Debug, Default)]
struct TableCheck {
    /// Rows that would take over a live test their table did not make.
    problems: Vec<crate::afx::Obligation>,
    /// Live tests a table made in this draft's lineage and no longer has:
    /// `(table, test)`.
    stale: Vec<(String, String)>,
}

/// Whether `owned` (a lineage's `tables`) records `id` for `table`'s `test`.
fn owns(owned: &Value, table: &str, test: &str, id: &EntityId) -> bool {
    let id = crate::hex::encode(id.as_bytes());
    owned[table][test]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|known| known.as_str() == Some(id.as_str())))
}

/// The draft and table whose latest revision records `id` as a table-made
/// test.
fn table_maker(drafts: &Drafts, id: &EntityId) -> Option<(String, String)> {
    for handle in drafts.handles().ok()? {
        let Ok(status) = drafts
            .latest(&handle)
            .and_then(|latest| drafts.status(&handle, latest))
        else {
            continue;
        };
        for (table, tests) in status["tables"].as_object().into_iter().flatten() {
            let made = tests
                .as_object()
                .into_iter()
                .flatten()
                .any(|(test, _)| owns(&status["tables"], table, test, id));
            if made {
                return Some((handle, table.clone()));
            }
        }
    }
    None
}

/// A table row may name a live `TestCase` only when this draft's lineage
/// made that very test from the same table (then the row updates it), or
/// when the frame deletes it explicitly. A table restated without a row it
/// once made deletes the live test that row made.
fn check_tables(
    head: &Head,
    names: &Names,
    frame_value: &Value,
    owned: &Value,
    drafts: &Drafts,
) -> TableCheck {
    let mut check = TableCheck::default();
    if frame_value.get("afx") != Some(&Value::from(1)) {
        return check;
    }
    let (tables, rows) = crate::tables::row_tests(frame_value);
    let listed = |key: &str, field: Option<&str>| -> Vec<String> {
        frame_value[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|entry| match field {
                Some(field) => entry.get(field).and_then(Value::as_str),
                None => entry.as_str(),
            })
            .map(str::to_owned)
            .collect()
    };
    let deleted = listed("delete", None);
    let frame_tests = listed("tests", Some("name"));
    let live_test = |name: &str| live_test(head, names, name);
    for row in &rows {
        if deleted.contains(&row.name) {
            continue;
        }
        let Some(id) = live_test(&row.name) else {
            continue;
        };
        if owns(owned, &row.table, &row.name, &id) {
            continue;
        }
        let target = match head.program().body(&id) {
            Some(sley_mutate::value::EntityBodyValue::TestCase(test)) => names.name(&test.target),
            _ => "?".to_owned(),
        };
        let maker = table_maker(drafts, &id).map_or_else(String::new, |(draft, table)| {
            format!("; table `{table}` of {draft} made it: update it there with sley-agent try --on {draft} (with --rebase after a commit)")
        });
        check.problems.push(crate::afx::Obligation::new(
            AgentErrorCode::TestTableInvalid,
            &row.at,
            format!(
                "the row's test `{name}` would replace the live TestCase `{name}` (a test of `{target}`), which table `{table}` did not make in this draft: give the row another \"name\", or delete `{name}` explicitly (\"delete\": [\"{name}\"]){maker}",
                name = row.name,
                table = row.table
            ),
        ));
    }
    for table in &tables {
        for (test, _) in owned[table.as_str()].as_object().into_iter().flatten() {
            let kept = rows.iter().any(|row| &row.name == test)
                || deleted.contains(test)
                || frame_tests.contains(test);
            if kept {
                continue;
            }
            if let Some(id) = live_test(test)
                && owns(owned, table, test, &id)
            {
                check.stale.push((table.clone(), test.clone()));
            }
        }
    }
    check
}

/// The table-made tests of a lineage after this revision: every row test
/// the candidate holds is recorded under its table.
fn record_tables(
    inherited: &Value,
    frame_value: &Value,
    program: &Program,
    names: &Names,
) -> Value {
    let mut owned = inherited.as_object().cloned().unwrap_or_default();
    if frame_value.get("afx") != Some(&Value::from(1)) {
        return Value::Object(owned);
    }
    for row in crate::tables::row_tests(frame_value).1 {
        let Some(id) = names
            .resolve(&row.name)
            .filter(|id| program.body(id).is_some_and(|body| body.kind_tag() == 14))
        else {
            continue;
        };
        let id = json!(crate::hex::encode(id.as_bytes()));
        let tests = owned
            .entry(row.table)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .map(|tests| tests.entry(row.name).or_insert_with(|| json!([])));
        if let Some(Value::Array(ids)) = tests
            && !ids.contains(&id)
        {
            ids.push(id);
        }
    }
    Value::Object(owned)
}

/// The authored positions of a kernel refusal about a `TestCase`: for a
/// refused limit, that limit, then the test's entry in the frame, through
/// the source map for a test made from a table row (its row, and the row's
/// `limits` or the table's `defaults`).
fn test_positions(
    output: &sley_policy::CandidateValidationOutput,
    program: &Program,
    names: &Names,
    frame_value: &Value,
    artifacts: &[(String, Value)],
) -> Option<crate::locate::Authored> {
    let locator = output.refusal_locator()?;
    let subject = locator.subject?;
    if program.body(&subject)?.kind_tag() != 14 {
        return None;
    }
    let name = names.name(&subject);
    let artifact = |file: &str| {
        artifacts
            .iter()
            .find(|(name, _)| name == file)
            .map(|(_, value)| value)
    };
    let expanded = artifact("expanded.json").unwrap_or(frame_value);
    let index = expanded
        .get("tests")?
        .as_array()?
        .iter()
        .position(|test| test.get("name").and_then(Value::as_str) == Some(name.as_str()))?;
    let authored = |pointer: String| {
        artifact("sourcemap.json")
            .and_then(|map| draft::authored_pointer(map, &pointer))
            .unwrap_or(pointer)
    };
    // The refused limit first: it is what a repair changes.
    let entry = format!("/tests/{index}");
    let mut items = Vec::new();
    if let Some(field) = locator
        .field
        .and_then(|field| field.strip_prefix("resource_limits."))
    {
        let limit = format!("{entry}/limits/{field}");
        if expanded.pointer(&limit).is_some() {
            items.push((authored(limit), format!("{field} limit of test {name}")));
        }
    }
    items.push((authored(entry), format!("test {name}")));
    Some(crate::locate::Authored { items, none: None })
}

/// The `next:` repair of an incomplete revision: a fill at the first
/// obligation's pointer, and when that pointer had to move to an existing
/// ancestor, which one and why.
fn repair_hint(
    obligations: &[Value],
    handle: &str,
    revision: u64,
    relayer: Option<&str>,
) -> String {
    let first = obligations.iter().find(|record| record["at"].is_string());
    let at = first.and_then(|record| record["at"].as_str()).unwrap_or("");
    let mut text = format!(
        "next: repair in place: sley-agent fill {handle} <delta.json> --revision {revision} with {{\"set\": [{{\"at\": {}, \"value\": ...}}]}}",
        json!(at)
    );
    if let Some(missing) = first.and_then(|record| record["missing"].as_str()) {
        let whole = if at.is_empty() { "the frame" } else { at };
        let _ = write!(
            text,
            " ({missing} does not exist: replace {whole} whole, with it included)"
        );
    }
    if let Some(on) = relayer {
        let _ = write!(
            text,
            "; the repaired follow-up is layered on {on} again (or: sley-agent try --on {on} <follow-up>)"
        );
    }
    text
}

/// One top-level entity a candidate changes.
struct Change {
    kind: u16,
    name: String,
    /// `+` created, `~` replaced (or a part of it changed), `-` deleted.
    mark: char,
    /// An exported function that existed before the candidate.
    exported: bool,
}

/// The functions, types, constants and `TestCases` a candidate creates,
/// changes or deletes, by kind and name.
fn changes(
    head: &Head,
    head_names: &Names,
    after: &Program,
    after_names: &Names,
    record: &sley_mutate::CandidateRecord,
) -> Vec<Change> {
    use sley_mutate::MutationClass;
    use sley_mutate::value::EntityBodyValue;
    let mut marks: BTreeMap<EntityId, (u16, char)> = BTreeMap::new();
    for operation in &record.operations {
        let target = operation.target_entity;
        let (entity, kind, mark) = match operation.target_kind {
            4 | 5 | 9 | 14 => {
                let mark = match operation.class {
                    MutationClass::CreateEntity => '+',
                    MutationClass::DeleteEntityBinding => '-',
                    _ => '~',
                };
                (target, operation.target_kind, mark)
            }
            6..=8 => {
                let owner = owner_function(after, &Names::default(), &target)
                    .or_else(|| owner_function(head.program(), &Names::default(), &target));
                match owner {
                    Some(owner) => (owner, 5, '~'),
                    None => continue,
                }
            }
            _ => continue,
        };
        let slot = marks.entry(entity).or_insert((kind, mark));
        if slot.1 == '~' {
            slot.1 = mark;
        }
    }
    let rank = |kind: u16| match kind {
        5 => 0,
        4 => 1,
        9 => 2,
        _ => 3,
    };
    let mut changes: Vec<Change> = marks
        .into_iter()
        .map(|(id, (kind, mark))| {
            let name = if after.contains(&id) {
                after_names.name(&id)
            } else {
                head_names.name(&id)
            };
            let exported = mark != '+'
                && matches!(
                    head.program().body(&id),
                    Some(EntityBodyValue::Function(function))
                        if function.visibility == sley_ssmc::Visibility::Exported
                );
            Change {
                kind,
                name,
                mark,
                exported,
            }
        })
        .collect();
    changes.sort_by(|a, b| (rank(a.kind), &a.name, a.mark).cmp(&(rank(b.kind), &b.name, b.mark)));
    changes
}

fn changes_json(changes: &[Change]) -> Value {
    Value::Array(
        changes
            .iter()
            .map(|change| {
                json!({
                    "kind": crate::names::kind_prefix(change.kind),
                    "name": change.name,
                    "change": match change.mark {
                        '+' => "created",
                        '-' => "deleted",
                        _ => "replaced",
                    },
                    "exported": change.exported,
                })
            })
            .collect(),
    )
}

/// `changed: fn ~f +g; type +E; test +t1` and, when an exported function
/// changed, `exported: ~f`.
fn changes_text(changes: &[Change]) -> String {
    if changes.is_empty() {
        return String::new();
    }
    let mut groups: Vec<(u16, Vec<String>)> = Vec::new();
    for change in changes {
        let item = format!("{}{}", change.mark, change.name);
        match groups.last_mut() {
            Some((kind, items)) if *kind == change.kind => items.push(item),
            _ => groups.push((change.kind, vec![item])),
        }
    }
    let body: Vec<String> = groups
        .iter()
        .map(|(kind, items)| format!("{} {}", crate::names::kind_prefix(*kind), items.join(" ")))
        .collect();
    let mut text = format!("changed: {}\n", body.join("; "));
    let exported: Vec<String> = changes
        .iter()
        .filter(|change| change.exported)
        .map(|change| format!("{}{}", change.mark, change.name))
        .collect();
    if !exported.is_empty() {
        let _ = writeln!(text, "exported: {}", exported.join(" "));
    }
    text
}

/// ` [authored 2, imported 1, provided 3]` (non-zero counts only), and the
/// live tests the candidate replaces: `; replaces provided t_1`.
fn provenance_text(provenance: &Value) -> String {
    let parts: Vec<String> = ["authored", "imported", "provided"]
        .iter()
        .filter_map(|key| {
            let count = provenance[*key].as_u64().unwrap_or(0);
            (count > 0).then(|| format!("{key} {count}"))
        })
        .collect();
    let replaced: Vec<&str> = provenance["replaced"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut text = parts.join(", ");
    if !replaced.is_empty() {
        if !text.is_empty() {
            text.push_str("; ");
        }
        let _ = write!(text, "replaces provided {}", replaced.join(" "));
    }
    if text.is_empty() {
        text
    } else {
        format!(" [{text}]")
    }
}

/// Records one draft revision and tries it: compile, assemble, validate,
/// store the candidate, run the tests, report. `try`, `fill` and `import`
/// share it, so every revision goes through the same loop.
#[allow(clippy::too_many_lines)]
fn run_trial(
    global: &Global,
    workspace: &Workspace,
    head: &Head,
    proposal: Proposal,
    options: &TrialOptions,
    out: &mut dyn Write,
) -> Result<i32> {
    let drafts = Drafts::open(workspace)?;
    let (mut claim, parent) = match &proposal.target {
        Target::New => (drafts.claim_new()?, None),
        Target::Next {
            handle,
            parent,
            latest,
        } => {
            let claim = drafts.claim_next(handle)?;
            if *latest && claim.number() != parent + 1 {
                return Err(AgentError::new(
                    AgentErrorCode::DraftStale,
                    format!(
                        "{handle} gained r{} while this command ran on r{parent}: read it (sley-agent draft {handle}) and run the command again",
                        claim.number() - 1
                    ),
                ));
            }
            (claim, Some(draft::spell(handle, *parent)))
        }
    };
    let handle = claim.handle().to_owned();
    let revision = claim.number();
    let reference = draft::spell(&handle, revision);
    let frame_path = format!(
        "{STATE_DIR}/{}/{handle}/r{revision}/frame.json",
        draft::DRAFTS_DIR
    );
    // A follow-up kept as given (not JSON, or refused by its base) is
    // layered on its base again when it is repaired.
    let unlayered = proposal.on.is_some() && !matches!(proposal.frame, Input::Frame(_));
    let mut status = json!({
        "revision": revision,
        "base_head": crate::hex::encode(head.transaction_id().as_bytes()),
        "parent": parent,
        "made_by": if proposal.rebase.is_some() { "rebase" } else { proposal.made_by },
        "on": proposal.on,
        "unlayered": unlayered,
        "delta": proposal.delta,
        "whole_frame": proposal.whole_frame,
        "state": State::Incomplete.as_str(),
        "candidate": null,
        "candidate_sha256": null,
        "verdict": null,
        "obligations": [],
        "tests": null,
        "sources": proposal.sources,
        "tables": proposal.tables,
    });
    if let Some(rebase) = &proposal.rebase {
        status["rebase"] = rebase.clone();
    }
    if let Some(import) = &proposal.import {
        status["import"] = import.clone();
    }
    global.note("draft", reference.as_str());
    global.note("input_bytes", proposal.input.len());
    global.note("whole_frame", proposal.whole_frame);
    if let Some(delta) = &proposal.delta {
        global.note(
            "delta_targets",
            delta["targets"].as_array().map_or(0, Vec::len),
        );
        global.note("delta_bytes", proposal.input.len());
    }
    let relayer = proposal.on.as_deref().filter(|_| unlayered);
    let frame_value = match proposal.frame {
        Input::Frame(value) => value,
        Input::Text(failure) => {
            let message = failure
                .detail
                .rsplit_once(" at line ")
                .map_or(failure.detail.as_str(), |(message, _)| message);
            let obligations = vec![draft::text_obligation(
                message,
                failure.line,
                failure.column,
                failure.byte,
            )];
            status["state"] = json!(State::Text.as_str());
            status["text"] =
                json!({"line": failure.line, "column": failure.column, "byte": failure.byte});
            status["obligations"] = json!(obligations);
            drafts.record(
                &mut claim,
                &Revision {
                    input: &proposal.input,
                    frame: None,
                    artifacts: &[],
                    status: &status,
                },
            )?;
            let error = crate::error::frame("", format!("not JSON: {}", failure.detail));
            let summary = format!(
                "draft {reference}: text (not JSON at line {}, column {}, byte {}); the input is kept",
                failure.line, failure.column, failure.byte
            );
            let next = match relayer {
                Some(on) => format!(
                    "next: fix the JSON: sley-agent fill {handle} <delta.json> --revision {revision} with {{\"set\": [{{\"at\": \"\", \"value\": <the follow-up>}}]}}; the follow-up is layered on {on} again (or: sley-agent try --on {on} <follow-up>)"
                ),
                None => format!(
                    "next: fix the JSON and try again, or replace it whole: sley-agent fill {handle} <delta.json> --revision {revision} with {{\"set\": [{{\"at\": \"\", \"value\": <the frame>}}]}}"
                ),
            };
            return unfinished(
                global,
                out,
                &error,
                &reference,
                State::Text,
                &obligations,
                [summary, next],
            );
        }
        Input::Unlayered { follow_up, error } => {
            let mut obligations = draft::obligations_of(&error);
            draft::anchor(&mut obligations, Some(&follow_up));
            status["obligations"] = json!(obligations);
            drafts.record(
                &mut claim,
                &Revision {
                    input: &proposal.input,
                    frame: Some(&follow_up),
                    artifacts: &[],
                    status: &status,
                },
            )?;
            let on = relayer.unwrap_or("its base");
            let error = AgentError::new(
                error.code(),
                format!(
                    "{}\n  pointers refer to {frame_path}, the follow-up as given: it is not yet layered on {on}",
                    error.detail()
                ),
            );
            let summary = format!(
                "draft {reference}: incomplete, {} obligation(s) ({}); the follow-up is kept, not yet layered on {on}",
                draft::obligation_count(&obligations),
                draft::obligation_symbols(&obligations)
            );
            let next = repair_hint(&obligations, &handle, revision, relayer);
            return unfinished(
                global,
                out,
                &error,
                &reference,
                State::Incomplete,
                &obligations,
                [summary, next],
            );
        }
    };
    global.note("table_rows", draft::table_rows(&frame_value));
    let sources = draft::live_sources(
        &frame_value,
        &status["sources"].as_array().cloned().unwrap_or_default(),
    );
    let (imported_tests, authored) = draft::frame_tests(&frame_value, &sources, &[]);
    status["sources"] = json!(sources);
    let authority = Authority::of(head)?;
    let mut map = name_map(workspace)?;
    let names = Names::build(head.program(), &map);
    // Until a candidate exists, a frame test that restates a live test by
    // name counts where its entry comes from, not also as provided.
    let restated: Vec<EntityId> = frame_value["tests"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("name").and_then(Value::as_str).map(str::to_owned))
        .chain(
            crate::tables::row_tests(&frame_value)
                .1
                .into_iter()
                .map(|row| row.name),
        )
        .filter_map(|name| live_test(head, &names, &name))
        .collect();
    let nonce = candidate::fresh_nonce()?;
    let tables = check_tables(head, &names, &frame_value, &proposal.tables, &drafts);
    // A table restated without a row it made deletes that row's live test.
    let mut compile_frame = frame_value.clone();
    let mut notes: Vec<String> = Vec::new();
    for (table, test) in &tables.stale {
        if let Some(object) = compile_frame.as_object_mut() {
            let deletes = object
                .entry("delete")
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(deletes) = deletes {
                deletes.push(json!(test));
            }
        }
        notes.push(format!(
            "table `{table}` no longer has a row for its live test `{test}`: the test is deleted"
        ));
    }
    let staged = match (
        stage(head, &authority, &names, &compile_frame, nonce),
        tables.problems.is_empty(),
    ) {
        (Ok(staged), true) => Ok(staged),
        (Ok(_), false) => Err(crate::afx::refusal(&tables.problems)),
        (Err(error), true) => Err(error),
        (Err(error), false) => Err(merge_refusals(
            &crate::afx::refusal(&tables.problems),
            &error,
        )),
    };
    let (compiled, imported, output) = match staged {
        Ok(staged) => staged,
        Err(error) => {
            let mut obligations = draft::obligations_of(&error);
            draft::anchor(&mut obligations, Some(&frame_value));
            let provenance = json!({"provided": provided_tests(head, None, &restated), "imported": imported_tests, "authored": authored});
            status["obligations"] = json!(obligations);
            status["tests"] = provenance.clone();
            global.note("tests", provenance);
            drafts.record(
                &mut claim,
                &Revision {
                    input: &proposal.input,
                    frame: Some(&frame_value),
                    artifacts: &[],
                    status: &status,
                },
            )?;
            let error = if obligations.iter().any(|record| record["at"].is_string()) {
                pointer_hint(error, &proposal.origin, &frame_path, &handle)
            } else {
                error
            };
            let summary = format!(
                "draft {reference}: incomplete, {} obligation(s) ({}); list: sley-agent draft {handle} --obligations",
                draft::obligation_count(&obligations),
                draft::obligation_symbols(&obligations)
            );
            let next = repair_hint(&obligations, &handle, revision, None);
            return unfinished(
                global,
                out,
                &error,
                &reference,
                State::Incomplete,
                &obligations,
                [summary, next],
            );
        }
    };
    let counts = (compiled.created, compiled.replaced, compiled.deleted);
    remember_names(workspace, &compiled.names)?;
    map.extend(&compiled.names);
    let program = candidate::proposed_program(head, &output)
        .or_else(|| candidate::applied_program(head, &imported))
        .unwrap_or_else(|| head.program().clone());
    let after_names = Names::build(&program, &map);
    let mut verdict = Verdict::of(&output, &program, &after_names);
    let source = Source::of_try(&frame_value, &compiled.artifacts);
    verdict.locate(&output, &source, &program, &after_names);
    // A refusal about a TestCase: its authored limit and entry, for the
    // trial's `authored:` line and the obligation (the verdict's own
    // `authored` list stays the phase-7 analysis).
    let test_authored = if !verdict.valid && verdict.authored.is_none() {
        test_positions(
            &output,
            &program,
            &after_names,
            &frame_value,
            &compiled.artifacts,
        )
    } else {
        None
    };
    let afx_stats = Value::Object(compiled.stats.clone());
    let store = Store::open(workspace)?;
    let mut meta = json!({
        "base": crate::hex::encode(head.transaction_id().as_bytes()),
        "ops": {"created": counts.0, "replaced": counts.1, "deleted": counts.2},
        "verdict": verdict.to_json(),
        "notes": compiled.notes,
        // The frame (layered, with --on) this candidate was made from, for a
        // later try --on.
        "frame": if frame_value.is_object() { frame_value.clone() } else { Value::Null },
        "draft": reference,
    });
    if !compiled.stats.is_empty() {
        meta["afx"] = json!({"stats": afx_stats});
    }
    // The source map travels with the candidate, for `explain` locators.
    if let Some((_, sourcemap)) = compiled
        .artifacts
        .iter()
        .find(|(file, _)| file == "sourcemap.json")
    {
        meta["sourcemap"] = sourcemap.clone();
    }
    let candidate_handle = store.save(&imported.stored_bytes, &meta)?;
    let ran = (|| -> Result<(Vec<TestOutcome>, Vec<PublicOutcome>)> {
        let (mut tests, mut public) = (Vec::new(), Vec::new());
        if output.is_valid() && !options.no_test {
            let mut executor = Executor::new(&program)?;
            let chosen: Vec<_> = executor
                .tests()
                .iter()
                .filter(|test| {
                    options.all_tests
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
            if let Some(path) = &options.public {
                public = run_public(&mut executor, &program, &after_names, Path::new(path))?;
            }
        }
        Ok((tests, public))
    })();
    let changes = changes(head, &names, &program, &after_names, &imported.record);
    let mut obligations = if verdict.valid {
        Vec::new()
    } else {
        let positions: Vec<String> = test_authored
            .iter()
            .flat_map(|authored| authored.items.iter().map(|(at, _)| at.clone()))
            .collect();
        vec![draft::kernel_obligation(&verdict, &positions)]
    };
    draft::anchor(&mut obligations, Some(&frame_value));
    // Live tests the candidate replaces count where their new entry comes
    // from, never also as provided.
    let replaced: Vec<EntityId> = imported
        .record
        .operations
        .iter()
        .filter(|operation| {
            operation.target_kind == 14
                && operation.class == sley_mutate::MutationClass::ReplaceEntityVersion
                && head.program().contains(&operation.target_entity)
        })
        .map(|operation| operation.target_entity)
        .collect();
    let mut replaced_names: Vec<String> = replaced.iter().map(|id| names.name(id)).collect();
    replaced_names.sort();
    // A frame test the candidate keeps unchanged (its compiled TestCase is
    // the live one) is provided, and counts once; a changed one replaces
    // the provided test and counts where its entry comes from.
    let kept: Vec<String> = head
        .program()
        .objects()
        .iter()
        .filter(|object| object.record().body.kind_tag() == 14)
        .map(|object| object.record().entity_id)
        .filter(|id| program.contains(id) && !replaced.contains(id))
        .map(|id| names.name(&id))
        .collect();
    let (imported_tests, authored) = draft::frame_tests(&frame_value, &sources, &kept);
    let mut provenance = json!({
        "provided": provided_tests(head, Some(&program), &replaced),
        "imported": imported_tests,
        "authored": authored,
    });
    if !replaced_names.is_empty() {
        provenance["replaced"] = json!(replaced_names);
    }
    let state = if verdict.valid {
        State::Valid
    } else {
        State::Refused
    };
    status["state"] = json!(state.as_str());
    status["candidate"] = json!(candidate_handle);
    status["candidate_sha256"] = json!(draft::sha256(&imported.stored_bytes));
    status["verdict"] = verdict.to_json();
    status["obligations"] = json!(obligations);
    status["tests"] = provenance.clone();
    status["changed"] = changes_json(&changes);
    status["tables"] = record_tables(&proposal.tables, &frame_value, &program, &after_names);
    if !compiled.stats.is_empty() {
        status["stats"] = afx_stats.clone();
    }
    if let Ok((tests, public)) = &ran {
        status["results"] = json!({
            "ran": tests.len(),
            "passed": tests.iter().filter(|test| test.passed()).count(),
            "public": public.len(),
            "public_passed": public.iter().filter(|case| case.passed).count(),
        });
    }
    drafts.record(
        &mut claim,
        &Revision {
            input: &proposal.input,
            frame: Some(&frame_value),
            artifacts: &compiled.artifacts,
            status: &status,
        },
    )?;
    global.note("candidate", candidate_handle.as_str());
    global.note("valid", verdict.valid);
    global.note("tests", provenance.clone());
    global.note("afx", afx_stats);
    global.note("obligations", draft::obligation_count(&obligations));
    if !verdict.valid {
        global.note(
            "refusal",
            verdict
                .symbol
                .clone()
                .unwrap_or_else(|| verdict.decision.clone()),
        );
    }
    notes.splice(0..0, compiled.notes.iter().cloned());
    let (tests, public) = ran?;
    let failed =
        tests.iter().filter(|t| !t.passed()).count() + public.iter().filter(|p| !p.passed).count();
    let exit = if verdict.valid && failed == 0 {
        EXIT_OK
    } else {
        EXIT_NEGATIVE
    };
    // After a control-flow refusal, every other finding the advisory analysis
    // sees, so the next frame can fix them all at once.
    let also = if !verdict.valid && verdict.phase == Some(7) {
        crate::explain::also(&program, &after_names, verdict.location.as_deref())
    } else {
        Vec::new()
    };
    // Bytes only on request (BR-10): the handle names them otherwise.
    let raw = options
        .raw
        .then(|| crate::hex::encode(&imported.stored_bytes));
    if global.json {
        let mut value = json!({
            "handle": candidate_handle,
            "draft": reference,
            "state": state.as_str(),
            "ops": {"created": counts.0, "replaced": counts.1, "deleted": counts.2},
            "verdict": verdict.to_json(),
            "tests": tests_json(&tests, &after_names),
            "public": public_json(&public),
            "notes": notes,
            "also": also,
            "changed": changes_json(&changes),
            "obligations": obligations,
            "provenance": provenance,
        });
        if let Some(raw) = &raw {
            value["stored_hex"] = json!(raw);
        }
        write_json(out, &value)?;
        return Ok(exit);
    }
    let mut details = verdict.details();
    if let Some(authored) = &test_authored {
        // After `where:`, as the verdict's own `authored:` line would be.
        let line = format!("  authored: {}\n", authored.text());
        let at = details.find("  hint:").unwrap_or(details.len());
        details.insert_str(at, &line);
    }
    let mut text = format!(
        "{candidate_handle}: {} (+{} created, {} replaced, {} deleted) draft {reference}\n{details}",
        verdict.headline(),
        counts.0,
        counts.1,
        counts.2,
    );
    for note in &notes {
        let _ = writeln!(text, "  note: {note}");
    }
    for finding in &also {
        let _ = writeln!(text, "  also: {finding}");
    }
    text.push_str(&changes_text(&changes));
    let provenance = provenance_text(&provenance);
    if !tests.is_empty() {
        let passed = tests.iter().filter(|t| t.passed()).count();
        let _ = writeln!(text, "tests: {passed}/{} passed{provenance}", tests.len());
        text.push_str(&test_lines(&tests, &after_names, options.verbose));
    }
    if let Some(raw) = &raw {
        let _ = writeln!(text, "stored: {raw}");
    }
    if verdict.valid && tests.is_empty() && !options.no_test {
        // Like a test runner's "running 0 tests": say that none ran.
        let _ = writeln!(
            text,
            "tests: 0 ran (no TestCase in this candidate targets a function it changes){provenance}"
        );
    }
    text.push_str(&public_text(&public));
    let layerable = frame_value.is_object();
    // A follow-up keeps the dialect of the frame it is layered on.
    let envelope = if frame_value.get("afx") == Some(&Value::from(1)) {
        "\"af1\": 1, \"afx\": 1"
    } else {
        "\"af1\": 1"
    };
    if verdict.valid && failed == 0 && tests.is_empty() && !options.no_test {
        if layerable {
            let _ = writeln!(
                text,
                "next: add tests without restating the frame: sley-agent try --on {handle} '{{{envelope}, \"tests\": [...]}}' (submit refuses an untested change; --untested overrides)"
            );
        } else {
            let _ = writeln!(
                text,
                "next: add AF1 \"tests\" for what {candidate_handle} changes and try again (submit refuses an untested change; --untested overrides)"
            );
        }
    } else if verdict.valid && failed == 0 {
        let _ = writeln!(text, "next: sley-agent submit {handle}");
    } else if verdict.valid && layerable {
        let _ = writeln!(
            text,
            "next: fix only what failed on top of {handle}: sley-agent try --on {handle} '{{{envelope}, \"edit\": [...]}}' (or \"patch\", \"tests\")"
        );
    } else if !verdict.valid {
        let _ = writeln!(text, "more: sley-agent explain {candidate_handle}");
        if layerable {
            let _ = writeln!(
                text,
                "fix: layer only the change on {handle}: sley-agent try --on {handle} '{{{envelope}, \"patch\": [...]}}'"
            );
        }
    }
    write_text(out, &text)?;
    Ok(exit)
}

/// A draft revision's state line: `d1@r3: valid, candidate c5 (Valid)`.
fn draft_headline(spelled: &str, status: &Value) -> String {
    let state = status["state"].as_str().unwrap_or("unknown");
    let mut text = format!("{spelled}: {state}");
    if let Some(candidate) = status["candidate"].as_str() {
        let verdict = &status["verdict"];
        let decision = if verdict["valid"] == true {
            "Valid".to_owned()
        } else {
            format!(
                "REFUSED {} at phase {} ({})",
                verdict["decision"].as_str().unwrap_or("?"),
                verdict["phase"].as_u64().unwrap_or(0),
                verdict["phase_name"].as_str().unwrap_or("unknown")
            )
        };
        let _ = write!(text, ", candidate {candidate} ({decision})");
    } else if state == State::Text.as_str() {
        let location = &status["text"];
        let _ = write!(
            text,
            " (not JSON at line {}, column {}, byte {})",
            location["line"], location["column"], location["byte"]
        );
    }
    text
}

#[allow(clippy::too_many_lines)]
fn draft_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    const VIEWS: [&str; 4] = ["--obligations", "--expanded", "--input", "--frame"];
    let words = words(args, &[], &VIEWS)?;
    let asked: Vec<&str> = VIEWS.into_iter().filter(|view| words.has(view)).collect();
    if asked.len() > 1 {
        return Err(usage(
            "draft takes one of --obligations, --expanded, --input, --frame",
        ));
    }
    let workspace = workspace(global)?;
    let drafts = Drafts::open(&workspace)?;
    let reference = match words.positional.as_slice() {
        [] if asked.is_empty() => return list_drafts(global, &drafts, out),
        [reference] => DraftRef::parse(reference)
            .ok_or_else(|| usage(format!("`{reference}` is not a draft (d1, d1@r2)")))?,
        _ => {
            return Err(usage(
                "draft [<draft>[@r<N>]] [--obligations | --expanded | --input | --frame]",
            ));
        }
    };
    let (handle, revision) = drafts.resolve(&reference)?;
    let spelled = draft::spell(&handle, revision);
    global.note("draft", spelled.as_str());
    let status = drafts.status(&handle, revision)?;
    let obligations = status["obligations"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    match asked.first().copied() {
        Some("--input") => {
            let input = drafts.input(&handle, revision)?;
            if global.json {
                write_json(
                    out,
                    &json!({"draft": spelled, "input": String::from_utf8_lossy(&input)}),
                )?;
            } else {
                let mut text = String::from_utf8_lossy(&input).into_owned();
                if !text.ends_with('\n') {
                    text.push('\n');
                }
                write_text(out, &text)?;
            }
        }
        Some("--frame") => {
            let frame = drafts.frame(&handle, revision)?.ok_or_else(|| {
                AgentError::new(
                    AgentErrorCode::DraftIncomplete,
                    format!("{spelled} is a text draft: it has no frame (sley-agent draft {spelled} --input prints its text)"),
                )
            })?;
            if global.json {
                write_json(out, &json!({"draft": spelled, "frame": frame}))?;
            } else {
                let mut text = serde_json::to_string_pretty(&frame).unwrap_or_default();
                text.push('\n');
                write_text(out, &text)?;
            }
        }
        Some("--expanded") => {
            let artifacts = drafts.artifacts(&handle, revision)?;
            if global.json {
                let map: serde_json::Map<String, Value> = artifacts.into_iter().collect();
                write_json(out, &json!({"draft": spelled, "artifacts": map}))?;
            } else if artifacts.is_empty() {
                write_text(
                    out,
                    &format!(
                        "{spelled} has no expanded form (its frame needed no expansion, or made no candidate)\n"
                    ),
                )?;
            } else {
                let mut text = String::new();
                for (name, value) in artifacts {
                    let _ = writeln!(
                        text,
                        "# {name}\n{}",
                        serde_json::to_string_pretty(&value).unwrap_or_default()
                    );
                }
                write_text(out, &text)?;
            }
        }
        Some(_) => {
            if global.json {
                write_json(out, &json!({"draft": spelled, "obligations": obligations}))?;
            } else {
                write_text(out, &draft::obligations_text(&obligations, &spelled, true))?;
            }
        }
        None => {
            let latest = drafts.latest(&handle)?;
            if global.json {
                let mut value = status.clone();
                value["draft"] = json!(spelled);
                value["latest"] = json!(latest);
                write_json(out, &value)?;
            } else {
                write_text(out, &draft_text(&spelled, &status, latest))?;
            }
        }
    }
    Ok(EXIT_OK)
}

/// The default text of `draft <d>`: state, origin, tests, obligations.
fn draft_text(spelled: &str, status: &Value, latest: u64) -> String {
    let mut text = draft_headline(spelled, status);
    text.push('\n');
    let _ = write!(
        text,
        "made by {}",
        status["made_by"].as_str().unwrap_or("?")
    );
    if let Some(parent) = status["parent"].as_str() {
        let _ = write!(text, " from {parent}");
    }
    if let Some(on) = status["on"].as_str() {
        if status["unlayered"] == true {
            let _ = write!(text, ", a follow-up not yet layered on {on}");
        } else if candidate::is_handle(on) {
            let _ = write!(text, " on the frame of {on}");
        } else if status["parent"].as_str() != Some(on) {
            let _ = write!(text, ", layered on {on}");
        }
    }
    if let Some(targets) = status["delta"]["targets"].as_array() {
        let _ = write!(text, ", {} target(s)", targets.len());
    }
    if status["whole_frame"] == true {
        text.push_str(", whole frame");
    }
    if let Some(import) = status.get("import") {
        let digest = import["sha256"].as_str().unwrap_or("");
        let _ = write!(
            text,
            ", {} case(s) imported from sha256 {}",
            import["cases"],
            &digest[..digest.len().min(12)]
        );
    }
    let short = |key: &str, value: &Value| {
        value[key]
            .as_str()
            .map(|hex| hex.chars().take(8).collect::<String>())
            .unwrap_or_default()
    };
    if let Some(rebase) = status.get("rebase") {
        let _ = write!(
            text,
            ", rebased from head {} to {}",
            short("from_head", rebase),
            short("to_head", rebase)
        );
    }
    let _ = write!(text, "; head {}", short("base_head", status));
    let revision = status["revision"].as_u64().unwrap_or(0);
    if revision != latest {
        let _ = write!(text, "; the latest revision is r{latest}");
    }
    text.push('\n');
    if status["tests"].is_object() {
        let provenance = provenance_text(&status["tests"]);
        match status.get("results") {
            Some(results) => {
                let _ = writeln!(
                    text,
                    "tests: {}/{} passed{provenance}",
                    results["passed"], results["ran"]
                );
            }
            None => {
                let _ = writeln!(text, "tests: not run{provenance}");
            }
        }
    }
    let obligations = status["obligations"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    text.push_str(&draft::obligations_text(&obligations, spelled, false));
    text
}

fn list_drafts(global: &Global, drafts: &Drafts, out: &mut dyn Write) -> Result<i32> {
    let last = drafts
        .last_recorded()?
        .map(|(handle, revision)| draft::spell(&handle, revision));
    let mut rows = Vec::new();
    for handle in drafts.handles()? {
        let latest = drafts.latest(&handle)?;
        let status = drafts.status(&handle, latest)?;
        let obligations = draft::obligation_count(
            status["obligations"]
                .as_array()
                .map_or(&[][..], Vec::as_slice),
        );
        rows.push((draft::spell(&handle, latest), status, obligations));
    }
    if global.json {
        let rows: Vec<Value> = rows
            .iter()
            .map(|(spelled, status, obligations)| {
                json!({"draft": spelled, "state": status["state"], "candidate": status["candidate"], "obligations": obligations, "last": last.as_deref() == Some(spelled.as_str())})
            })
            .collect();
        write_json(out, &json!({"drafts": rows}))?;
        return Ok(EXIT_OK);
    }
    if rows.is_empty() {
        write_text(out, "no drafts yet (sley-agent try <frame> makes one)\n")?;
        return Ok(EXIT_OK);
    }
    let mut text = String::new();
    for (spelled, status, obligations) in &rows {
        text.push_str(&draft_headline(spelled, status));
        if *obligations > 0 {
            let _ = write!(text, "; {obligations} obligation(s)");
        }
        if last.as_deref() == Some(spelled.as_str()) {
            text.push_str("; recorded last (a bare submit takes it)");
        }
        text.push('\n');
    }
    write_text(out, &text)?;
    Ok(EXIT_OK)
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
    text.push_str(&test_lines(tests, names, true));
    text
}

/// One line per failing test, and per passing test when `passing`.
fn test_lines(tests: &[TestOutcome], names: &Names, passing: bool) -> String {
    let mut text = String::new();
    for test in sorted(tests, names) {
        if test.passed() && !passing {
            continue;
        }
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
            passed: actual == canonical_expectation(&expected, program, names, &id),
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

/// A public case's `expect` in the form execution renders: read against
/// the function's result type (as AF1 tests read it) and rendered back, and
/// a trap by name or number as its code. An expectation that does not read
/// is compared as written, and fails.
pub(crate) fn canonical_expectation(
    expected: &Value,
    program: &Program,
    names: &Names,
    function: &EntityId,
) -> Value {
    if let Some(trap) = expected.as_object().and_then(|object| object.get("trap")) {
        let code = match trap {
            Value::String(name) => exec::trap_code(name).map(Value::from),
            Value::Number(number) => number.as_u64().map(Value::from),
            _ => None,
        };
        let Some(code) = code else {
            return expected.clone();
        };
        let mut canonical = json!({"trap": code});
        if let Some(payload) = expected.get("payload") {
            canonical["payload"] = payload.clone();
        }
        return canonical;
    }
    let Some(sley_mutate::value::EntityBodyValue::Function(body)) = program.body(function) else {
        return expected.clone();
    };
    let defs = ProgramTypes { program, names };
    values::read(expected, &body.result_type, &defs, "expect")
        .map_or_else(|_| expected.clone(), |value| values::to_json(&value, names))
}

/// A call's or case's JSON arguments read against the function's parameter
/// types.
pub(crate) fn typed_inputs(
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
    let words = words(args, &[], &["--untested"])?;
    let workspace = workspace(global)?;
    let head = workspace.head()?;
    let store = Store::open(&workspace)?;
    let (reference, from_draft) = match words.positional.as_slice() {
        [] => bare_submission(global, &workspace, &store)?,
        [latest] if latest == "latest" => bare_submission(global, &workspace, &store)?,
        [reference] => match DraftRef::parse(reference) {
            Some(reference) => {
                let drafts = Drafts::open(&workspace)?;
                let (handle, revision) = drafts.resolve(&reference)?;
                let spelled = draft::spell(&handle, revision);
                global.note("draft", spelled.as_str());
                let candidate = draft_candidate(&drafts, &store, &handle, revision)?;
                (candidate, Some(spelled))
            }
            None => (store.resolve(Some(reference))?, None),
        },
        _ => return Err(usage("submit [<handle> | <draft>[@r<N>]] [--untested]")),
    };
    let stored = store.load(&reference)?;
    let authority = Authority::of(&head)?;
    let output = candidate::validate(&head, &authority, &stored)?;
    global.note("valid", output.is_valid());
    // A change to a function travels with a test of it: a candidate that
    // changes functions while no TestCase targets them is refused unless
    // the agent says so explicitly.
    if output.is_valid()
        && output.result().record.selected_tests.is_empty()
        && !words.has("--untested")
        && changes_a_function(&head, &output, &stored)
    {
        return Err(AgentError::new(
            AgentErrorCode::SubmissionRefused,
            format!(
                "{} changes functions but no TestCase in it targets them; add AF1 \"tests\" for them to the frame and try again, or submit --untested",
                shown(&reference)
            ),
        ));
    }
    if !output.is_valid() {
        return Err(AgentError::new(
            AgentErrorCode::SubmissionRefused,
            format!(
                "{} is not Valid ({:?}); only Valid candidates are submitted",
                shown(&reference),
                output.result().record.decision
            ),
        ));
    }
    // A Valid file or stored hex is kept under a handle, so the submission
    // and every message name a short handle, never the bytes. A refused one
    // is never stored, so it cannot become `latest`.
    let reference = if candidate::is_handle(&reference) {
        reference
    } else {
        store.save(&stored, &json!({"imported_from": shown(&reference)}))?
    };
    let path = workspace.dir().join(SUBMISSION);
    fs::write(&path, format!("{}\n", crate::hex::encode(&stored)))
        .map_err(|error| io(&path, &error))?;
    let marker = workspace.state_dir()?.join("submission");
    fs::write(&marker, format!("{reference}\n")).map_err(|error| io(&marker, &error))?;
    global.note("candidate", reference.as_str());
    let tests = output.result().record.selected_tests.len();
    if global.json {
        write_json(
            out,
            &json!({"submitted": reference, "draft": from_draft, "file": SUBMISSION, "bytes": stored.len(), "selected_tests": tests}),
        )?;
    } else {
        let from = from_draft
            .as_ref()
            .map_or_else(String::new, |spelled| format!(" from {spelled}"));
        let mut text = format!(
            "submitted {reference}{from} -> {SUBMISSION} ({}-byte candidate, hex); resubmit any time, the last submission wins\n",
            stored.len()
        );
        if tests == 0 {
            let _ = writeln!(
                text,
                "note: no TestCase in {reference} targets a function it changes (submitted with --untested, or it changes no function)"
            );
        }
        write_text(out, &text)?;
    }
    Ok(EXIT_OK)
}

/// What a bare `submit` (or `submit latest`) submits: the draft revision
/// recorded last in this workspace, over every draft, when it is valid. Any
/// other state refuses, naming that revision: an earlier revision or
/// candidate is submitted only when named. Without drafts, the latest
/// candidate.
fn bare_submission(
    global: &Global,
    workspace: &Workspace,
    store: &Store,
) -> Result<(String, Option<String>)> {
    let drafts = Drafts::open(workspace)?;
    let Some((handle, revision)) = drafts.last_recorded()? else {
        return Ok((store.resolve(None)?, None));
    };
    let spelled = draft::spell(&handle, revision);
    global.note("draft", spelled.as_str());
    let status = drafts.status(&handle, revision)?;
    let state = status["state"].as_str().unwrap_or("unknown");
    if state != State::Valid.as_str() {
        let earlier = (1..revision).rev().find_map(|earlier| {
            let status = drafts.status(&handle, earlier).ok()?;
            (status["state"] == State::Valid.as_str()).then(|| {
                format!(
                    " (the latest valid revision is {}, candidate {})",
                    draft::spell(&handle, earlier),
                    status["candidate"].as_str().unwrap_or("?")
                )
            })
        });
        return Err(AgentError::new(
            AgentErrorCode::DraftIncomplete,
            format!(
                "the last recorded draft revision, {spelled}, is {state}: a bare submit takes only that revision when it is valid, never an earlier one; repair it (sley-agent draft {spelled}), or name what to submit: sley-agent submit <draft>@r<N> or <handle>{}",
                earlier.unwrap_or_default()
            ),
        ));
    }
    let candidate = draft_candidate(&drafts, store, &handle, revision)?;
    Ok((candidate, Some(spelled)))
}

/// The candidate a draft revision made: only a Valid revision resolves,
/// and only to the stored bytes it recorded. An older revision is never
/// used in its place.
fn draft_candidate(drafts: &Drafts, store: &Store, handle: &str, revision: u64) -> Result<String> {
    let spelled = draft::spell(handle, revision);
    let status = drafts.status(handle, revision)?;
    let state = status["state"].as_str().unwrap_or("unknown");
    if state != State::Valid.as_str() {
        let fix = match State::parse(state) {
            Some(State::Text) => {
                format!("fix the JSON: sley-agent fill {handle} <delta.json> --revision {revision}")
            }
            Some(State::Refused) => format!(
                "sley-agent explain {} says why",
                status["candidate"].as_str().unwrap_or("latest")
            ),
            _ => format!("sley-agent draft {spelled} --obligations lists what is open"),
        };
        return Err(AgentError::new(
            AgentErrorCode::DraftIncomplete,
            format!(
                "{spelled} is {state}, not valid: only a revision with a Valid candidate is submitted; {fix}"
            ),
        ));
    }
    let candidate = status["candidate"]
        .as_str()
        .filter(|handle| candidate::is_handle(handle))
        .ok_or_else(|| {
            AgentError::new(
                AgentErrorCode::DraftIncomplete,
                format!("{spelled} names no candidate"),
            )
        })?;
    let stored = store.load(candidate)?;
    if status["candidate_sha256"].as_str() != Some(draft::sha256(&stored).as_str()) {
        return Err(AgentError::new(
            AgentErrorCode::DraftIncomplete,
            format!("the stored bytes of {candidate} are not the ones {spelled} recorded"),
        ));
    }
    Ok(candidate.to_owned())
}

fn status_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(args, &[], &["--raw"])?;
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
    let raw = if words.has("--raw") {
        Some(
            fs::read_to_string(&path)
                .map_err(|error| io(&path, &error))?
                .trim()
                .to_owned(),
        )
    } else {
        None
    };
    if global.json {
        let mut value = json!({
            "submission": handle,
            "verdict": selected.verdict.as_ref().map(Verdict::to_json),
            "tests": tests_json(&tests, &selected.names),
            "view": text,
        });
        if let Some(raw) = &raw {
            value["stored_hex"] = json!(raw);
        }
        write_json(out, &value)?;
    } else {
        let _ = writeln!(
            text,
            "submission: {} ({verdict})",
            handle.as_deref().unwrap_or(SUBMISSION)
        );
        text.push_str(&tests_text(&tests, &selected.names));
        if let Some(raw) = &raw {
            let _ = writeln!(text, "stored: {raw}");
        }
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
        let status = call_batch(
            &mut executor,
            &selected,
            &function,
            Path::new(batch),
            words.has("--stats"),
            out,
        );
        release((executor, selected, map, head));
        return status;
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
    release((executor, selected, map, head));
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
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let rows: Vec<Value> = match serde_json::from_str::<Value>(&text) {
        // One line of arrays is several argument lists or one list whose
        // arguments are arrays: several only when every row type-checks.
        Ok(Value::Array(items))
            if lines.len() == 1 && !items.is_empty() && items.iter().all(Value::is_array) =>
        {
            let as_rows = items.iter().all(|row| {
                row.as_array().is_some_and(|args| {
                    typed_inputs(executor, &selected.program, &selected.names, function, args)
                        .is_ok()
                })
            });
            if as_rows {
                items
            } else {
                vec![Value::Array(items)]
            }
        }
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
    release((buffer, rows, text));
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

fn search_command(global: &Global, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let words = words(
        args,
        &["--public", "--from", "--max-neighbors", "--max-millis"],
        &[],
    )?;
    let ([function], Some(public)) = (words.positional.as_slice(), words.value("--public")) else {
        return Err(usage(crate::search::USAGE));
    };
    let (max_neighbors, max_millis) =
        crate::search::limits(words.value("--max-neighbors"), words.value("--max-millis"))?;
    let workspace = workspace(global)?;
    let report = crate::search::run(
        &workspace,
        &crate::search::Request {
            function,
            public,
            from: words.value("--from"),
            max_neighbors,
            max_millis,
        },
    )?;
    global.note("input_bytes", report.input_bytes);
    global.note("draft", report.draft.clone());
    global.note("candidate", report.candidate.clone());
    global.note("afx", Value::Object(report.stats.clone()));
    if global.json {
        write_json(out, &report.json)?;
    } else {
        write_text(out, &report.text)?;
    }
    Ok(report.exit)
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
        write_text(
            out,
            &format!("{}: {}\n{view_text}", shown(&label), verdict.to_text()),
        )?;
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
    global.event.borrow_mut().at(workspace.dir().to_path_buf());
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
        [] => store.resolve(None)?,
        [reference] => store.resolve(Some(reference))?,
        _ => return Err(usage("commit [<handle>]")),
    };
    let stored = store.load(&reference)?;
    let transaction = crate::genesis::commit(&head, &workspace.repo(), &stored)?;
    let transaction = crate::hex::encode(transaction.as_bytes());
    if global.json {
        write_json(
            out,
            &json!({"committed": shown(&reference), "transaction": transaction}),
        )?;
    } else {
        write_text(
            out,
            &format!(
                "committed {} as transaction {}\n",
                shown(&reference),
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
