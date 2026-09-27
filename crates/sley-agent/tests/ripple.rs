//! Ripple: `arity` and `guard` derivations in AF1-X frames, and the intents
//! this build does not enable.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sley_agent::genesis;
use sley_agent::names::{NameMap, Names};
use sley_agent::workspace::{NAMES_FILE, Program, Workspace};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "sley-agent-ripple-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// The suite runs on the binary's allocator (ADR-0052).
#[path = "../src/allocator.rs"]
mod allocator;
#[global_allocator]
static ALLOCATOR: allocator::SizeClassCache = allocator::SizeClassCache;

const SEED: [u8; 32] = [7; 32];

fn workspace(label: &str) -> TempDir {
    let temp = TempDir::new(label);
    genesis::init(&temp.path, Some(SEED), genesis::INIT_CEILINGS).unwrap();
    temp
}

fn run(dir: &Path, args: &[&str]) -> (i32, String) {
    let mut words = vec!["--workspace".to_owned(), dir.display().to_string()];
    words.extend(args.iter().map(|arg| (*arg).to_owned()));
    let mut out = Vec::new();
    let status = sley_agent::cli::run(&words, &mut out);
    (status, String::from_utf8(out).unwrap())
}

fn run_json(dir: &Path, args: &[&str]) -> (i32, Value) {
    let mut words = vec!["--json"];
    words.extend_from_slice(args);
    let (status, text) = run(dir, &words);
    (
        status,
        serde_json::from_str(text.trim()).unwrap_or_else(|_| panic!("not JSON: {text}")),
    )
}

/// Commits a frame (functions, or tests alone) as the new head.
fn commit(dir: &Path, frame: &Value) {
    let (status, text) = run(dir, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    let (status, text) = run(dir, &["commit"]);
    assert_eq!(status, 0, "{text}");
}

/// A Valid `try`: its JSON report.
fn valid(dir: &Path, frame: &Value) -> Value {
    let (status, value) = run_json(dir, &["try", &frame.to_string()]);
    assert_eq!(value["state"], "valid", "{value:#}");
    assert_eq!(status, 0, "{value:#}");
    value
}

/// A refused frame: (symbol, detail, obligations).
fn refused(dir: &Path, frame: &Value) -> (String, String, Vec<Value>) {
    let (status, value) = run_json(dir, &["try", &frame.to_string()]);
    assert_eq!(status, 2, "expected a refusal: {value:#}");
    assert_eq!(value["state"], "incomplete", "{value:#}");
    (
        value["error"].as_str().unwrap().to_owned(),
        value["detail"].as_str().unwrap().to_owned(),
        value["obligations"].as_array().cloned().unwrap_or_default(),
    )
}

fn assert_refused(dir: &Path, frame: &Value, symbol: &str, needles: &[&str]) -> Vec<Value> {
    let (got, detail, obligations) = refused(dir, frame);
    assert_eq!(got, symbol, "{detail}");
    for needle in needles {
        assert!(detail.contains(needle), "missing `{needle}` in: {detail}");
    }
    obligations
}

/// A derived artifact of a draft revision (`ripple.json`, `expanded.json`).
fn artifact(dir: &Path, draft: &str, file: &str) -> Value {
    let (handle, revision) = draft.split_once("@r").unwrap();
    let path = dir
        .join(".sley")
        .join("drafts")
        .join(handle)
        .join(format!("r{revision}"))
        .join(file);
    serde_json::from_str(&fs::read_to_string(&path).unwrap_or_else(|_| panic!("{path:?}")))
        .unwrap()
}

fn names_of(dir: &Path, program: &Program) -> Names {
    let mut map = NameMap::read(&dir.join(NAMES_FILE)).unwrap();
    map.extend(&NameMap::read(&dir.join(".sley").join(NAMES_FILE)).unwrap());
    Names::build(program, &map)
}

fn name_map(dir: &Path) -> NameMap {
    let mut map = NameMap::read(&dir.join(NAMES_FILE)).unwrap();
    map.extend(&NameMap::read(&dir.join(".sley").join(NAMES_FILE)).unwrap());
    map
}

/// The expansion of `frame` against the workspace head.
fn expand(dir: &Path, frame: &Value) -> sley_agent::afx::Expansion {
    let head = Workspace::at(dir).head().unwrap();
    let names = names_of(dir, head.program());
    sley_agent::afx::expand(head.program(), &names, frame).unwrap()
}

/// An executor over the head, or over the Valid candidate of a frame.
struct Machine {
    executor: sley_agent::exec::Executor,
    program: Program,
    names: Names,
}

impl Machine {
    fn head(dir: &Path) -> Self {
        let head = Workspace::at(dir).head().unwrap();
        let program = head.program().clone();
        let names = names_of(dir, &program);
        let executor = sley_agent::exec::Executor::new(&program).unwrap();
        Self {
            executor,
            program,
            names,
        }
    }

    fn candidate(dir: &Path, frame: &Value) -> Self {
        let head = Workspace::at(dir).head().unwrap();
        let mut map = name_map(dir);
        let names = Names::build(head.program(), &map);
        let authority = sley_agent::candidate::Authority::of(&head).unwrap();
        let nonce = sley_agent::candidate::fresh_nonce().unwrap();
        let compiled = sley_agent::frame::compile(
            head.program(),
            &names,
            &authority.ceilings,
            frame,
            nonce,
            &mut sley_agent::candidate::random32,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        map.extend(&compiled.names);
        let imported =
            sley_agent::candidate::assemble(&head, &authority, nonce, compiled.ops).unwrap();
        let output =
            sley_agent::candidate::validate(&head, &authority, &imported.stored_bytes).unwrap();
        let program =
            sley_agent::candidate::proposed_program(&head, &output).unwrap_or_else(|| {
                let applied = sley_agent::candidate::applied_program(&head, &imported).unwrap();
                let names = Names::build(&applied, &map);
                panic!(
                    "not Valid: {}",
                    sley_agent::catalog::Verdict::of(&output, &applied, &names).to_text()
                )
            });
        let names = Names::build(&program, &map);
        let executor = sley_agent::exec::Executor::new(&program).unwrap();
        Self {
            executor,
            program,
            names,
        }
    }

    fn call(&mut self, function: &str, args: &[Value]) -> Value {
        let id = self
            .names
            .resolve(function)
            .unwrap_or_else(|| panic!("no {function}"));
        let types = self.executor.parameter_types(&id);
        let defs = sley_agent::values::ProgramTypes {
            program: &self.program,
            names: &self.names,
        };
        let inputs = args
            .iter()
            .zip(&types)
            .map(|(arg, ty)| sley_agent::values::read(arg, ty, &defs, "").unwrap())
            .collect();
        let outcome = self
            .executor
            .run(&id, inputs, sley_agent::exec::call_limits())
            .unwrap();
        sley_agent::exec::termination_json(&outcome.termination, &self.names)
    }
}

const INTS: [i64; 7] = [i64::MIN, -3, -1, 0, 1, 7, i64::MAX];

fn grid(columns: &[&[Value]]) -> Vec<Vec<Value>> {
    let mut rows: Vec<Vec<Value>> = vec![Vec::new()];
    for column in columns {
        let mut next = Vec::new();
        for row in &rows {
            for value in *column {
                let mut row = row.clone();
                row.push(value.clone());
                next.push(row);
            }
        }
        rows = next;
    }
    rows
}

fn ints() -> Vec<Value> {
    INTS.iter().map(|n| json!(n)).collect()
}

fn bools() -> Vec<Value> {
    vec![json!(true), json!(false)]
}

/// `function` behaves the same on the head and in the candidate for every
/// row.
fn same_behavior(dir: &Path, frame: &Value, function: &str, rows: &[Vec<Value>]) {
    let mut before = Machine::head(dir);
    let mut after = Machine::candidate(dir, frame);
    for row in rows {
        assert_eq!(
            before.call(function, row),
            after.call(function, row),
            "{function}{row:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// arity
// ---------------------------------------------------------------------------

/// `f(a, b) = a - b` and three callers: a literal argument, swapped
/// arguments, and calls in two branches (one checked, two nested).
fn arity_base() -> Value {
    json!({"af1": 1, "afx": 1, "fns": [
      {"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,ArithmeticError>",
       "blocks": [{"name": "entry", "term": ["return", ["sub", "a", "b"]]}]},
      {"fn": "g", "params": [["x", "i64"]], "returns": "Result<i64,ArithmeticError>",
       "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x", 1]], "term": ["return", "r"]}]},
      {"fn": "h", "params": [["x", "i64"], ["y", "i64"]], "returns": "Result<i64,ArithmeticError>",
       "blocks": [{"name": "entry", "ops": [["r", "call", "f", "y", "x"]], "term": ["return", "r"]}]},
      {"fn": "twice", "params": [["x", "i64"], ["flag", "bool"]], "returns": "Result<i64,ArithmeticError>",
       "blocks": [{"name": "entry", "term": ["cond", "flag", "left", "right"]},
                  {"name": "left", "ops": [["r", "call?", "f", "x", "x"]], "term": ["return", ["call", "f", "r", 2]]},
                  {"name": "right", "term": ["return", ["call", "f", 3, "x"]]}]}]})
}

fn arity_workspace(label: &str) -> TempDir {
    let temp = workspace(label);
    commit(&temp.path, &arity_base());
    commit(
        &temp.path,
        &json!({"af1": 1, "tests": [{"name": "t_f", "fn": "f", "args": [5, 2], "expect": {"Ok": 3}},
                                     {"name": "t_g", "fn": "g", "args": [5], "expect": {"Ok": 4}}]}),
    );
    temp
}

/// `f` becomes `f(b, a, c) = a - b + c`.
fn new_f(ripple: &Value) -> Value {
    json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "f", "params": [["b", "i64"], ["a", "i64"], ["c", "i64"]],
                 "blocks": {"entry": {"ops": [["d", "sub?", "a", "b"]], "term": ["return", ["add", "d", "c"]]}}}],
      "ripple": ripple})
}

/// The call operations of `function` in a derived patch, as
/// `(name, arguments after the callee)`.
fn patched_calls(expanded: &Value, function: &str) -> Vec<(String, Vec<Value>)> {
    let patch = expanded["patch"]
        .as_array()
        .unwrap()
        .iter()
        .find(|patch| patch["fn"] == function)
        .unwrap_or_else(|| panic!("no patch of {function}: {expanded:#}"));
    let mut out = Vec::new();
    for block in patch["blocks"].as_object().unwrap().values() {
        for op in block["ops"].as_array().into_iter().flatten() {
            if op["op"] == "call" {
                let args = op["args"].as_array().unwrap();
                out.push((op["name"].as_str().unwrap().to_owned(), args[1..].to_vec()));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn arity_rewrites_every_caller_and_test_by_parameter_name() {
    let temp = arity_workspace("arity");
    let frame = new_f(&json!([{"arity": "f", "value": 0}]));
    let report = valid(&temp.path, &frame);
    let draft = report["draft"].as_str().unwrap();
    // Collateral changes are visible in the ordinary report.
    let changed: Vec<String> = report["changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| format!("{} {}", change["kind"].as_str().unwrap(), change["name"].as_str().unwrap()))
        .collect();
    for entity in ["fn f", "fn g", "fn h", "fn twice", "test t_f"] {
        assert!(changed.contains(&entity.to_owned()), "{changed:?}");
    }
    assert!(!changed.contains(&"test t_g".to_owned()), "{changed:?}");
    // Every argument goes by name: b <- old b, a <- old a, c <- 0.
    let expanded = artifact(&temp.path, draft, "expanded.json");
    assert_eq!(
        patched_calls(&expanded, "g"),
        [("r".to_owned(), vec![json!("r__a1"), json!("x"), json!("r__a2")])]
    );
    assert_eq!(
        patched_calls(&expanded, "h"),
        [("r".to_owned(), vec![json!("x"), json!("y"), json!("r__a2")])]
    );
    assert_eq!(patched_calls(&expanded, "twice").len(), 3);
    let g = expanded["patch"]
        .as_array()
        .unwrap()
        .iter()
        .find(|patch| patch["fn"] == "g")
        .unwrap();
    assert_eq!(
        g["blocks"]["entry"]["ops"][1],
        json!({"name": "r__a2", "op": "const", "args": [{"type": "i64", "value": 0}], "type": "i64"})
    );
    // The live test gets the same treatment; its expectation is kept.
    let test = expanded["tests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|test| test["name"] == "t_f")
        .unwrap();
    assert_eq!(test["args"], json!([2, 5, 0]));
    assert_eq!(test["expect"], json!({"Ok": 3}));
    // The inventory lists every site, the test and the boundary.
    let inventory = artifact(&temp.path, draft, "ripple.json");
    let intent = &inventory["intents"][0];
    assert_eq!(intent["intent"], "arity");
    assert_eq!(intent["old"], json!(["a: i64", "b: i64"]));
    assert_eq!(intent["new"], json!(["b: i64", "a: i64", "c: i64"]));
    assert_eq!(intent["calls"].as_array().unwrap().len(), 5);
    assert!(
        intent["calls"]
            .as_array()
            .unwrap()
            .iter()
            .all(|call| call["edit"] == "rewritten" && call["origin"] == "live"),
        "{intent:#}"
    );
    assert_eq!(intent["tests"], json!([{"test": "t_f", "origin": "live", "edit": "rewritten"}]));
    assert_eq!(intent["boundary"]["references"], json!([]));
    assert_eq!(inventory["changed"]["functions"], json!(["g", "h", "twice"]));
    assert_eq!(inventory["changed"]["tests"], json!(["t_f"]));
    let status = artifact(&temp.path, draft, "status.json");
    assert_eq!(status["stats"]["ripple_intents"], 1);
    assert_eq!(status["stats"]["ripple_edits"], 6);
    assert_eq!(status["stats"]["ripple_holes"], 0);
    // Tests of the changed functions ran and pass; every caller computes
    // what it did before.
    assert_eq!(report["tests"].as_array().unwrap().len(), 2);
    same_behavior(&temp.path, &frame, "g", &grid(&[&ints()]));
    same_behavior(&temp.path, &frame, "h", &grid(&[&ints(), &ints()]));
    same_behavior(&temp.path, &frame, "twice", &grid(&[&ints(), &bools()]));
}

#[test]
fn removed_parameters_drop_their_argument_but_not_its_evaluation() {
    let temp = workspace("removed");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Zero", "Ov"]}], "fns": [
          {"fn": "scaled", "params": [["a", "i64"], ["scale", "i64"]], "returns": "Result<i64,E>",
           "blocks": [{"name": "entry", "ops": [["m", "mul?Ov", "a", "scale"]], "term": ["ok", "m"]}]},
          {"fn": "use_scaled", "params": [["x", "i64"], ["d", "i64"]], "returns": "Result<i64,E>",
           "blocks": [{"name": "entry", "ops": [["s", "div?Zero", 100, "d"], ["r", "call?", "scaled", "x", "s"]],
                       "term": ["ok", "r"]}]}]}),
    );
    let frame = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "scaled", "params": [["a", "i64"]], "blocks": {"entry": {"term": ["ok", "a"]}}}],
      "ripple": [{"arity": "scaled"}]});
    let report = valid(&temp.path, &frame);
    let draft = report["draft"].as_str().unwrap();
    let expanded = artifact(&temp.path, draft, "expanded.json");
    assert_eq!(
        patched_calls(&expanded, "use_scaled"),
        [("r__r".to_owned(), vec![json!("x")])]
    );
    let inventory = artifact(&temp.path, draft, "ripple.json");
    assert_eq!(inventory["intents"][0]["removed"], json!(["scale"]));
    // The dropped argument's division still runs first: its failure stays.
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(after.call("use_scaled", &[json!(3), json!(0)]), json!({"Err": "Zero"}));
    assert_eq!(after.call("use_scaled", &[json!(3), json!(7)]), json!({"Ok": 3}));
    assert_eq!(
        after.call("use_scaled", &[json!(i64::MAX), json!(1)]),
        json!({"Ok": i64::MAX})
    );
}

#[test]
fn a_missing_value_is_a_hole_at_each_site() {
    let temp = arity_workspace("holes");
    let obligations = assert_refused(
        &temp.path,
        &new_f(&json!([{"arity": "f"}])),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &["/ripple/0: `g.entry.r` gives `f` no argument for its new parameter `c` (expected i64)"],
    );
    // One hole per call site and per test, each pointing into the intent.
    assert_eq!(obligations.len(), 6, "{obligations:#?}");
    for obligation in &obligations {
        assert_eq!(obligation["symbol"], "AGENT_RIPPLE_HOLE_UNFILLED");
        assert_eq!(obligation["at"], "/ripple/0");
        assert_eq!(obligation["expected"], "i64");
    }
    let decisions: Vec<&str> = obligations
        .iter()
        .map(|obligation| obligation["decision"].as_str().unwrap())
        .collect();
    for site in ["`g.entry.r`", "`h.entry.r`", "`twice.left.r__r`", "TestCase `t_f`"] {
        assert!(
            decisions.iter().any(|decision| decision.starts_with(site)),
            "{site}: {decisions:#?}"
        );
    }
}

#[test]
fn the_value_must_be_a_literal_of_the_one_new_parameter() {
    let temp = arity_workspace("values");
    for (value, needle) in [
        (json!("zero"), "/ripple/0/value: state the value of new parameter `c` (expected i64) as a literal"),
        (json!(2.5), "/ripple/0/value: 2.5 is not a literal of new parameter `c`'s type (expected i64)"),
        (json!({"type": "u8", "value": 3}), "/ripple/0/value: the value's type is \"u8\", but new parameter `c` of `f` is i64"),
        (json!({"type": "i64", "value": "x"}), "/ripple/0/value: the value does not fit new parameter `c` (expected i64)"),
    ] {
        assert_refused(
            &temp.path,
            &new_f(&json!([{"arity": "f", "value": value}])),
            "AGENT_RIPPLE_HOLE_UNFILLED",
            &[needle],
        );
    }
    // Two new parameters: "value" fills neither.
    let two = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"], ["c", "i64"], ["d", "i64"]]}],
      "ripple": [{"arity": "f", "value": 0}]});
    assert_refused(
        &temp.path,
        &two,
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &["/ripple/0/value: \"value\" fills one new parameter; `f` gains 2 (c: i64, d: i64)"],
    );
    // No new parameter: "value" fills nothing.
    let none = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "f", "params": [["b", "i64"], ["a", "i64"]]}],
      "ripple": [{"arity": "f", "value": 0}]});
    assert_refused(
        &temp.path,
        &none,
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &["/ripple/0/value: `f` gains no parameter, so \"value\" fills nothing"],
    );
    // Unchanged parameters, or none restated.
    let same = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]]}],
      "ripple": [{"arity": "f"}]});
    assert_refused(
        &temp.path,
        &same,
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &["/ripple/0/arity: the frame restates the parameters of `f` unchanged (a: i64, b: i64)"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"arity": "f"}]}),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &["/ripple/0/arity: the frame does not restate the parameters of `f`"],
    );
}

#[test]
fn calls_the_frame_writes_are_read_against_the_new_parameters() {
    let temp = workspace("frame-calls");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [
          {"fn": "f", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
           "blocks": [{"name": "entry", "term": ["return", ["add", "a", 0]]}]},
          {"fn": "m", "params": [["x", "i64"]], "returns": "Result<i64,ArithmeticError>",
           "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"]], "term": ["return", "r"]}]},
          {"fn": "n", "params": [["x", "i64"]], "returns": "Result<i64,ArithmeticError>",
           "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"]], "term": ["return", "r"]}]}]}),
    );
    let frame = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]],
                 "blocks": {"entry": {"term": ["return", ["add", "a", "b"]]}}},
                // Restated with the old argument count: rewritten.
                {"fn": "m", "blocks": {"entry": {"ops": [["one", "const", 1], ["r", "call", "f", "x"]],
                                                 "term": ["return", "r"]}}}],
      // Written for the new parameters: kept as written.
      "fns": [{"fn": "k", "params": [["x", "i64"]], "returns": "Result<i64,ArithmeticError>",
               "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x", 5]], "term": ["return", "r"]}]}],
      "tests": [{"name": "t_old", "fn": "f", "args": [4], "expect": {"Ok": 4}},
                {"name": "t_new", "fn": "f", "args": [4, 1], "expect": {"Ok": 5}},
                {"name": "t_k", "fn": "k", "args": [1], "expect": {"Ok": 6}}],
      "ripple": [{"arity": "f", "value": 0}]});
    let report = valid(&temp.path, &frame);
    assert_eq!(report["tests"].as_array().unwrap().len(), 3, "{report:#}");
    let draft = report["draft"].as_str().unwrap();
    let expanded = artifact(&temp.path, draft, "expanded.json");
    // m's authored block: the constant goes before the call, and the source
    // map follows the move.
    let m = &expanded["patch"][1]["blocks"]["entry"]["ops"];
    assert_eq!(m[1], json!(["r__a1", "const", {"type": "i64", "value": 0}]));
    assert_eq!(m[2], json!(["r", "call", "f", "x", "r__a1"]));
    assert_eq!(expanded["fns"][0]["blocks"][0]["ops"][0], json!(["r__a1", "const", {"type": "i64", "value": 5}]));
    assert_eq!(expanded["tests"][0]["args"], json!([4, 0]));
    assert_eq!(expanded["tests"][1]["args"], json!([4, 1]));
    let map = artifact(&temp.path, draft, "sourcemap.json");
    let entry = |expanded: &str| {
        map["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["expanded"] == expanded)
            .cloned()
    };
    assert_eq!(entry("/patch/1/blocks/entry/ops/1").unwrap()["authored"], "/ripple/0");
    assert_eq!(entry("/patch/1/blocks/entry/ops/1").unwrap()["role"], "ripple");
    assert_eq!(map["names"]["m"]["r__a1"], "/ripple/0");
    let inventory = artifact(&temp.path, draft, "ripple.json");
    let calls = &inventory["intents"][0]["calls"];
    let edits: Vec<(String, String, String)> = calls
        .as_array()
        .unwrap()
        .iter()
        .map(|call| {
            (
                call["site"].as_str().or(call["test"].as_str()).unwrap().to_owned(),
                call["origin"].as_str().unwrap().to_owned(),
                call["edit"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(edits.contains(&("/fns/0/blocks/0/ops/0 (k)".to_owned(), "frame".to_owned(), "as written".to_owned())), "{edits:?}");
    assert!(edits.contains(&("/patch/1/blocks/entry/ops/1 (m)".to_owned(), "frame".to_owned(), "rewritten".to_owned())), "{edits:?}");
    assert!(edits.contains(&("n.entry.r".to_owned(), "live".to_owned(), "rewritten".to_owned())), "{edits:?}");
    assert!(edits.contains(&("t_old".to_owned(), "frame".to_owned(), "rewritten".to_owned())), "{edits:?}");
    assert!(edits.contains(&("t_new".to_owned(), "frame".to_owned(), "as written".to_owned())), "{edits:?}");
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(after.call("m", &[json!(4)]), json!({"Ok": 4}));
    assert_eq!(after.call("n", &[json!(4)]), json!({"Ok": 4}));
    assert_eq!(after.call("k", &[json!(4)]), json!({"Ok": 9}));
}

#[test]
fn overloaded_names_and_reordered_parameters_are_resolved_by_identity_and_name() {
    let temp = workspace("ambiguity");
    let body = |name: &str| {
        json!({"fn": name, "params": [["a", "i64"], ["flag", "bool"]], "returns": "i64",
               "blocks": [{"name": "entry", "term": ["cond", "flag", ["yes", "a"], "no"]},
                          {"name": "yes", "params": [["v", "i64"]], "term": ["return", "v"]},
                          {"name": "no", "term": ["return", {"type": "i64", "value": -1}]}]})
    };
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [body("f"), body("f2"),
          // A value named `f`, a call of `f2` on it, and a call of `f`.
          {"fn": "g", "params": [["x", "i64"], ["t", "bool"]], "returns": "(i64,i64)",
           "blocks": [{"name": "entry", "ops": [["f", "const", {"type": "i64", "value": 7}],
                                                ["r2", "call", "f2", "f", "t"],
                                                ["r", "call", "f", "x", "t"]],
                       "term": ["return", ["tuple", "r", "r2"]]}]}]}),
    );
    // Same names, reordered, with the types moving along.
    let frame = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "f", "params": [["flag", "bool"], ["a", "i64"]]}],
      "ripple": [{"arity": "f"}]});
    let report = valid(&temp.path, &frame);
    let expanded = artifact(&temp.path, report["draft"].as_str().unwrap(), "expanded.json");
    assert_eq!(
        patched_calls(&expanded, "g"),
        [
            ("r".to_owned(), vec![json!("t"), json!("x")]),
            ("r2".to_owned(), vec![json!("f"), json!("t")]),
        ]
    );
    same_behavior(&temp.path, &frame, "g", &grid(&[&ints(), &bools()]));
    // A parameter kept by name with another type is never coerced.
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1,
          "patch": [{"fn": "f", "params": [["a", "u8"], ["flag", "bool"]],
                     "blocks": {"yes": {"params": [["v", "u8"]], "term": ["return", {"type": "i64", "value": 0}]}}}],
          "ripple": [{"arity": "f"}]}),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &["/ripple/0: `g.entry.r` passes an argument of type i64 for parameter `a`, which `f` now takes as u8 (expected u8)"],
    );
}

#[test]
fn a_function_value_is_unresolved_dispatch() {
    let temp = workspace("fnref");
    commit(
        &temp.path,
        &json!({"af1": 1, "fns": [
          {"fn": "f", "params": [["a", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
          {"fn": "g", "params": [["x", "i64"]], "returns": "i64",
           "blocks": [{"name": "entry", "ops": [["fr", "fnref", "f"]], "term": ["return", "x"]}]}]}),
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]]}],
                "ripple": [{"arity": "f", "value": 0}]}),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &["/ripple/0: `g.entry.fr` uses `f` as a function value (fnref): calls through it are unresolved dispatch"],
    );
}

#[test]
fn callers_in_another_namespace_are_an_exported_boundary() {
    let temp = workspace("namespaces");
    commit(
        &temp.path,
        &json!({"af1": 1, "fns": [
          {"fn": "f", "params": [["a", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
          {"fn": "g", "params": [["x", "i64"]], "returns": "i64",
           "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"]], "term": ["return", "r"]}]},
          {"fn": "h", "params": [["x", "i64"]], "returns": "i64",
           "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"]], "term": ["return", "r"]}]}]}),
    );
    commit(
        &temp.path,
        &json!([
          {"class": "CreateEntity", "kind": 3, "key": "ns_a", "payload": {"parent": {"variant": "None"}, "members": ["f", "h"]}},
          {"class": "CreateEntity", "kind": 3, "key": "ns_b", "payload": {"parent": {"variant": "None"}, "members": ["g"]}}]),
    );
    let obligations = assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]]}],
                "ripple": [{"arity": "f", "value": 0}]}),
        "AGENT_RIPPLE_EXPORTED_BOUNDARY",
        &["/ripple/0: `g.entry.r` calls `f` from namespace ns_b while `f` is in ns_a: ripple does not edit code across a namespace boundary"],
    );
    // The caller in f's own namespace is not a boundary.
    assert_eq!(obligations.len(), 1, "{obligations:#?}");
}

#[test]
fn an_entry_point_is_an_exported_boundary() {
    use sley_mutate::value::{EntityBodyValue, EntryPointBody};
    let temp = arity_workspace("entry-point");
    let head = Workspace::at(&temp.path).head().unwrap();
    let names = names_of(&temp.path, head.program());
    let f = names.resolve("f").unwrap();
    // The workbench cannot author an entry point; add one to the state the
    // expansion reads.
    let mut objects = head.program().objects().to_vec();
    objects.push(
        sley_mutate::build_entity_object(
            head.program().epoch(),
            &sley_mutate::EntityObjectRecord {
                entity_id: sley_id::EntityId::from_bytes([0xe9; 32]),
                body: EntityBodyValue::EntryPoint(EntryPointBody {
                    function: f,
                    exposure: sley_ssmc::EntryExposure::Protocol,
                }),
                label: Some("serve".to_owned()),
                semantic_fingerprint: None,
            },
        )
        .unwrap(),
    );
    let program = Program::new(
        head.program().epoch(),
        head.program().root(),
        head.program().workspace(),
        objects,
    );
    let names = Names::build(&program, &name_map(&temp.path));
    let expansion = sley_agent::afx::expand(
        &program,
        &names,
        &new_f(&json!([{"arity": "f", "value": 0}])),
    )
    .unwrap();
    assert_eq!(expansion.obligations.len(), 1, "{:?}", expansion.obligations);
    let obligation = &expansion.obligations[0];
    assert_eq!(obligation.symbol.symbol(), "AGENT_RIPPLE_EXPORTED_BOUNDARY");
    assert_eq!(obligation.at, "/ripple/0/arity");
    assert!(
        obligation.decision.starts_with("`f` is the entry point `serve`"),
        "{}",
        obligation.decision
    );
    // Nothing was derived: no patch of a caller entered the frame.
    assert_eq!(expansion.frame["patch"].as_array().unwrap().len(), 1);
}

#[test]
fn bounds_are_refused_never_truncated() {
    let temp = workspace("limits");
    let intents: Vec<Value> = (0..33).map(|_| json!({"arity": "f"})).collect();
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": intents}),
        "AGENT_RIPPLE_LIMIT",
        &["/ripple: 33 intents; a frame lists at most 32"],
    );
    let functions: Vec<String> = (0..65).map(|n| format!("f{n}")).collect();
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"guard": "g", "arg": "p", "in": functions}]}),
        "AGENT_RIPPLE_LIMIT",
        &["/ripple/0/in: 65 functions; a guard names at most 64"],
    );
    let mut ops: Vec<Value> = (0..257).map(|n| json!([format!("r{n}"), "call", "f", "x"])).collect();
    ops.push(json!(["z", "tuple", "r0", "r256"]));
    commit(
        &temp.path,
        &json!({"af1": 1, "fns": [
          {"fn": "f", "params": [["a", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
          {"fn": "g", "params": [["x", "i64"]], "returns": "(i64,i64)",
           "blocks": [{"name": "entry", "ops": ops, "term": ["return", "z"]}]}]}),
    );
    let obligations = assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]]}],
                "ripple": [{"arity": "f", "value": 0}]}),
        "AGENT_RIPPLE_LIMIT",
        &["/ripple/0: `f` has 257 call sites and tests; one intent rewrites at most 256"],
    );
    assert_eq!(obligations.len(), 1);
}

#[test]
fn targets_of_the_wrong_kind_and_disabled_intents_are_refused() {
    let temp = arity_workspace("kinds");
    commit(
        &temp.path,
        &json!({"af1": 1, "types": [{"name": "Shape", "variant": ["Empty"]}],
                "consts": [{"name": "limit", "type": "i64", "value": 3}]}),
    );
    for (target, needle) in [
        ("Shape", "/ripple/0/arity: `Shape` is a TypeDef, not a function"),
        ("limit", "/ripple/0/arity: `limit` is a Constant, not a function"),
        ("nothing", "/ripple/0/arity: no function named `nothing`"),
    ] {
        assert_refused(
            &temp.path,
            &json!({"af1": 1, "afx": 1, "ripple": [{"arity": target}]}),
            "AGENT_RIPPLE_TARGET_KIND",
            &[needle],
        );
    }
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [{"fn": "fresh", "params": [], "returns": "i64",
                  "blocks": [{"name": "entry", "term": ["return", {"type": "i64", "value": 1}]}]}],
                "ripple": [{"arity": "fresh"}]}),
        "AGENT_RIPPLE_TARGET_KIND",
        &["/ripple/0/arity: `fresh` is created by this frame"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "delete": ["h"], "ripple": [{"arity": "h"}]}),
        "AGENT_RIPPLE_TARGET_KIND",
        &["/ripple/0/arity: `h` is deleted by this frame"],
    );
    // Specified intents this build does not enable, and unknown ones.
    for intent in [
        json!({"effect": "f", "add": "E"}),
        json!({"member": "Shape", "add": "Round"}),
        json!({"retype": "f.a", "to": "u8"}),
        json!({"move": "f", "to": "ns"}),
        json!({"prune": "f"}),
    ] {
        let word = intent.as_object().unwrap().keys().find(|key| *key != "add" && *key != "to").unwrap().clone();
        assert_refused(
            &temp.path,
            &json!({"af1": 1, "afx": 1, "ripple": [intent]}),
            "AGENT_RIPPLE_INTENT_UNKNOWN",
            &[&format!("/ripple/0: `{word}` is not enabled in this build; the enabled intents are arity and guard")],
        );
    }
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"rename": "f"}]}),
        "AGENT_RIPPLE_INTENT_UNKNOWN",
        &["/ripple/0: unknown intent"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"arity": "f", "guard": "g"}]}),
        "AGENT_RIPPLE_INTENT_UNKNOWN",
        &["/ripple/0: one intent per entry, not arity and guard"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": {"arity": "f"}}),
        "AGENT_RIPPLE_INTENT_UNKNOWN",
        &["/ripple: ripple is a list of intents"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"arity": "f", "values": 0}]}),
        "AGENT_FRAME_INVALID",
        &["/ripple/0/values: unknown key in a arity intent"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"guard": "g", "arg": "p", "in": []}]}),
        "AGENT_FRAME_INVALID",
        &["/ripple/0/in: a guard names the functions it applies to"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"guard": "g", "arg": "p", "in": ["f"], "mode": "first"}]}),
        "AGENT_FRAME_INVALID",
        &["/ripple/0/mode: the mode is \"preserve\" (the default) or \"entry\""],
    );
    // Without "afx": 1 the key is unknown, as before.
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "ripple": [{"arity": "f"}]}),
        "AGENT_FRAME_INVALID",
        &["/ripple: unknown frame key"],
    );
    // One intent per function.
    assert_refused(
        &temp.path,
        &new_f(&json!([{"arity": "f", "value": 0}, {"arity": "f", "value": 1}])),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &["/ripple/1/arity: the parameters of `f` are already propagated by the intent at /ripple/0"],
    );
}

#[test]
fn derivations_are_deterministic() {
    let temp = arity_workspace("determinism");
    let frame = new_f(&json!([{"arity": "f", "value": 0}]));
    let first = expand(&temp.path, &frame);
    let second = expand(&temp.path, &frame);
    assert!(first.obligations.is_empty(), "{:?}", first.obligations);
    assert_eq!(first.frame, second.frame);
    assert_eq!(first.map, second.map);
    assert_eq!(first.stats, second.stats);
    assert_eq!(first.ripple, second.ripple);
    assert_eq!(first.frame.to_string(), second.frame.to_string());
    assert_eq!(first.stats.ripple_intents, 1);
    assert_eq!(first.stats.ripple_edits, 6);
    // A hole-bearing derivation is deterministic too.
    let holes = new_f(&json!([{"arity": "f"}]));
    let (a, b) = (expand(&temp.path, &holes), expand(&temp.path, &holes));
    assert_eq!(a.obligations, b.obligations);
    assert_eq!(a.obligations.len(), 6);
    // The same frame layered on its own draft derives the same edits.
    let report = valid(&temp.path, &frame);
    let draft = report["draft"].as_str().unwrap().split('@').next().unwrap().to_owned();
    let more = json!({"af1": 1, "tests": [{"name": "t_h", "fn": "h", "args": [1, 3], "expect": {"Ok": 2}}]});
    let (status, layered) = run_json(&temp.path, &["try", "--on", &draft, &more.to_string()]);
    assert_eq!(status, 0, "{layered:#}");
    let revision = layered["draft"].as_str().unwrap();
    let expanded = artifact(&temp.path, revision, "expanded.json");
    let original = artifact(&temp.path, report["draft"].as_str().unwrap(), "expanded.json");
    assert_eq!(expanded["patch"], original["patch"]);
}

#[test]
fn derived_edits_go_through_the_unchanged_kernel() {
    let temp = workspace("kernel");
    commit(
        &temp.path,
        &json!({"af1": 1, "fns": [
          {"fn": "scale_it", "params": [["a", "f64"]], "returns": "f64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
          {"fn": "use_it", "params": [["x", "f64"]], "returns": "f64",
           "blocks": [{"name": "entry", "ops": [["r", "call", "scale_it", "x"]], "term": ["return", "r"]}]}]}),
    );
    // A type-correct literal the kernel refuses: negative zero is not a
    // canonical float. The derived constant reaches the kernel and is
    // refused there; nothing hides it.
    let frame = json!({"af1": 1, "afx": 1, "patch": [{"fn": "scale_it", "params": [["a", "f64"], ["s", "f64"]]}],
                       "ripple": [{"arity": "scale_it", "value": {"type": "f64", "value": -0.0}}]});
    let expansion = expand(&temp.path, &frame);
    assert!(expansion.obligations.is_empty());
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 2, "{text}");
    assert!(text.starts_with("error AGENT_CANDIDATE_INVALID: SCB_FLOAT_NON_CANONICAL"), "{text}");
    // A candidate carrying derived edits is judged whole: the kernel's own
    // refusal of the frame's change is reported with the derived collateral.
    let frame = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "scale_it", "params": [["a", "f64"], ["s", "f64"]], "blocks": {"orphan": {"term": ["return", "a"]}}}],
      "ripple": [{"arity": "scale_it", "value": 1.5}]});
    let (status, report) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 1, "{report:#}");
    assert_eq!(report["state"], "refused");
    assert_eq!(report["verdict"]["symbol"], "CFG_REACHABILITY");
    assert!(
        report["changed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change["name"] == "use_it"),
        "{report:#}"
    );
    // Without the orphan, the kernel accepts it and the caller passes 1.5.
    let frame = json!({"af1": 1, "afx": 1, "patch": [{"fn": "scale_it", "params": [["a", "f64"], ["s", "f64"]],
                        "blocks": {"entry": {"term": ["return", "s"]}}}],
                       "ripple": [{"arity": "scale_it", "value": 1.5}]});
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(after.call("use_it", &[json!(4.0)]), json!(1.5));
}

// ---------------------------------------------------------------------------
// guard
// ---------------------------------------------------------------------------

fn order_error() -> Value {
    json!({"name": "OrderError", "variant": ["InvalidQuantity", "InvalidPrice", "Overflow"]})
}

/// Live functions that check `quantity` inline in several ways.
fn orders() -> Value {
    json!({"af1": 1, "afx": 1, "types": [order_error()], "fns": [
      // The price check comes first.
      {"fn": "line_total", "params": [["quantity", "i64"], ["price", "i64"]], "returns": "Result<i64,OrderError>",
       "blocks": [{"name": "entry", "ops": [["!InvalidPrice", "if", ["lt", "price", 0]],
                                             ["!InvalidQuantity", "if", ["lt", "quantity", 1]],
                                             ["total", "mul?Overflow", "quantity", "price"]],
                   "term": ["ok", "total"]}]},
      {"fn": "discounted", "params": [["quantity", "i64"], ["price", "i64"], ["off", "i64"]], "returns": "Result<i64,OrderError>",
       "blocks": [{"name": "entry", "ops": [["!InvalidQuantity", "if", ["lt", "quantity", 1]],
                                             ["total", "mul?Overflow", "quantity", "price"],
                                             ["net", "sub?Overflow", "total", "off"]],
                   "term": ["ok", "net"]}]},
      // The same check on two paths, after an early return.
      {"fn": "shipped", "params": [["quantity", "i64"], ["express", "bool"], ["free", "bool"]], "returns": "Result<i64,OrderError>",
       "blocks": [{"name": "entry", "term": ["cond", "free", "gratis", "paid"]},
                  {"name": "gratis", "term": ["ok", 0]},
                  {"name": "paid", "term": ["cond", "express", "fast", "slow"]},
                  {"name": "fast", "ops": [["!InvalidQuantity", "if", ["lt", "quantity", 1]]], "term": ["ok", ["mul?Overflow", "quantity", 2]]},
                  {"name": "slow", "ops": [["!InvalidQuantity", "if", ["lt", "quantity", 1]]], "term": ["ok", "quantity"]}]},
      // A call that can trap runs before the check.
      {"fn": "boom", "params": [["a", "i64"]], "returns": "i64",
       "blocks": [{"name": "entry", "term": ["cond", ["eq", "a", 0], "bad", "good"]},
                  {"name": "bad", "term": ["trap"]},
                  {"name": "good", "term": ["return", "a"]}]},
      {"fn": "after_call", "params": [["a", "i64"], ["quantity", "i64"]], "returns": "Result<i64,OrderError>",
       "blocks": [{"name": "entry", "ops": [["z", "call", "boom", "a"],
                                             ["!InvalidQuantity", "if", ["lt", "quantity", 1]]],
                   "term": ["ok", ["add?Overflow", "z", "quantity"]]}]}]})
}

fn check_quantity() -> Value {
    json!({"fn": "check_quantity", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["!InvalidQuantity", "if", ["lt", "q", 1]]], "term": ["ok", "q"]}]})
}

fn orders_workspace(label: &str) -> TempDir {
    let temp = workspace(label);
    commit(&temp.path, &orders());
    temp
}

fn guard_frame(checker: &Value, intent: &Value) -> Value {
    json!({"af1": 1, "afx": 1, "fns": [checker], "ripple": [intent]})
}

fn quantities() -> Vec<Value> {
    [i64::MIN, -1, 0, 1, 2, 7, i64::MAX].iter().map(|n| json!(n)).collect()
}

fn prices() -> Vec<Value> {
    [i64::MIN, -1, 0, 5, i64::MAX].iter().map(|n| json!(n)).collect()
}

#[test]
fn preserve_replaces_the_identical_check_where_it_is() {
    let temp = orders_workspace("preserve");
    let frame = guard_frame(
        &check_quantity(),
        &json!({"guard": "check_quantity", "arg": "quantity",
                "in": ["line_total", "discounted", "shipped", "after_call"]}),
    );
    let report = valid(&temp.path, &frame);
    let draft = report["draft"].as_str().unwrap();
    let inventory = artifact(&temp.path, draft, "ripple.json");
    let intent = &inventory["intents"][0];
    assert_eq!(intent["mode"], "preserve");
    let functions = intent["functions"].as_array().unwrap();
    assert_eq!(functions.len(), 4);
    assert!(functions.iter().all(|f| f["edit"] == "replaced"), "{intent:#}");
    // line_total's check stays after the price check; shipped has two.
    assert_eq!(functions[0]["checks"], json!(["line_total.entry__if0"]));
    assert_eq!(functions[2]["checks"], json!(["shipped.fast", "shipped.slow"]));
    assert_eq!(functions[0]["deleted"], json!(["__fail_InvalidQuantity"]));
    let expanded = artifact(&temp.path, draft, "expanded.json");
    let line = expanded["patch"]
        .as_array()
        .unwrap()
        .iter()
        .find(|patch| patch["fn"] == "line_total")
        .unwrap();
    assert_eq!(line["blocks"]["__fail_InvalidQuantity"], Value::Null);
    assert_eq!(
        line["blocks"]["entry__if0"]["ops"],
        json!([{"name": "quantity__check_quantity", "op": "call", "args": ["check_quantity", "quantity"],
                "type": "Result<i64,OrderError>"}])
    );
    assert_eq!(
        line["blocks"]["entry__if0"]["term"],
        json!(["switch", "quantity__check_quantity", ["Ok", "entry__if1"], ["Err", "__err", "$"]])
    );
    // Same results for every input, simultaneous invalid ones included:
    // the price error still wins in line_total, the early return and the
    // trapping call still come first.
    same_behavior(&temp.path, &frame, "line_total", &grid(&[&quantities(), &prices()]));
    same_behavior(&temp.path, &frame, "discounted", &grid(&[&quantities(), &prices(), &prices()]));
    same_behavior(&temp.path, &frame, "shipped", &grid(&[&quantities(), &bools(), &bools()]));
    same_behavior(&temp.path, &frame, "after_call", &grid(&[&[json!(0), json!(3)], &quantities()]));
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(
        after.call("line_total", &[json!(0), json!(-1)]),
        json!({"Err": "InvalidPrice"})
    );
    assert_eq!(after.call("shipped", &[json!(0), json!(true), json!(true)]), json!({"Ok": 0}));
    assert_eq!(after.call("after_call", &[json!(0), json!(0)]), json!({"trap": 1}));
}

#[test]
fn preserve_matches_an_alpha_renamed_hand_written_check_of_a_live_checker() {
    let temp = workspace("alpha");
    commit(
        &temp.path,
        &json!({"af1": 1, "types": [order_error()], "fns": [
          {"fn": "check_quantity", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["one", "const", 1], ["low", "lt", "q", "one"]], "term": ["cond", "low", "bad", "good"]},
                      {"name": "bad", "ops": [["e", "variant", "OrderError.InvalidQuantity"], ["r", "err", "e"]], "term": ["return", "r"]},
                      {"name": "good", "ops": [["r", "ok", "q"]], "term": ["return", "r"]}]},
          {"fn": "unit_price", "params": [["total", "i64"], ["n", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["k", "const", 1], ["small", "lt", "n", "k"]], "term": ["cond", "small", "reject", "divide"]},
                      {"name": "reject", "ops": [["why", "variant", "OrderError.InvalidQuantity"], ["out", "err", "why"]], "term": ["return", "out"]},
                      {"name": "divide", "ops": [["d", "div", "total", "n"]], "term": ["switch", "d", ["Ok", "fine", "$"], ["Err", "over"]]},
                      {"name": "fine", "params": [["v", "i64"]], "ops": [["o", "ok", "v"]], "term": ["return", "o"]},
                      {"name": "over", "ops": [["w", "variant", "OrderError.Overflow"], ["x", "err", "w"]], "term": ["return", "x"]}]}]}),
    );
    let frame = json!({"af1": 1, "afx": 1,
      "ripple": [{"guard": "check_quantity", "arg": "n", "in": ["unit_price"], "mode": "preserve"}]});
    let report = valid(&temp.path, &frame);
    let inventory = artifact(&temp.path, report["draft"].as_str().unwrap(), "ripple.json");
    assert_eq!(inventory["intents"][0]["functions"][0]["checks"], json!(["unit_price.entry"]));
    assert_eq!(inventory["intents"][0]["functions"][0]["deleted"], json!(["reject"]));
    same_behavior(&temp.path, &frame, "unit_price", &grid(&[&prices(), &quantities()]));
}

#[test]
fn preserve_refuses_what_it_cannot_prove() {
    let temp = orders_workspace("preserve-refusals");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "Other", "variant": ["Bad"]}], "fns": [
          // Another comparison: `le quantity 0` is not `lt q 1`.
          {"fn": "le_check", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["!InvalidQuantity", "if", ["le", "quantity", 0]]], "term": ["ok", "quantity"]}]},
          // The same test with another error.
          {"fn": "price_error", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["!InvalidPrice", "if", ["lt", "quantity", 1]]], "term": ["ok", "quantity"]}]},
          // Another error type.
          {"fn": "other_error", "params": [["quantity", "i64"]], "returns": "Result<i64,Other>",
           "blocks": [{"name": "entry", "ops": [["!Bad", "if", ["lt", "quantity", 1]]], "term": ["ok", "quantity"]}]},
          // The check's value is used after it.
          {"fn": "reused", "params": [["quantity", "i64"]], "returns": "Result<(i64,bool),OrderError>",
           "blocks": [{"name": "entry", "ops": [["low", "lt", "quantity", 1], ["!InvalidQuantity", "if", "low"]],
                       "term": ["ok", ["tuple", "quantity", "low"]]}]}]}),
    );
    let guard = |function: &str| {
        guard_frame(
            &check_quantity(),
            &json!({"guard": "check_quantity", "arg": "quantity", "in": [function]}),
        )
    };
    assert_refused(
        &temp.path,
        &guard("le_check"),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["/ripple/0/in/0: no check in `le_check` is the same as `check_quantity` on `quantity`", "use \"mode\": \"entry\""],
    );
    assert_refused(
        &temp.path,
        &guard("price_error"),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["/ripple/0/in/0: no check in `price_error` is the same as `check_quantity`"],
    );
    assert_refused(
        &temp.path,
        &guard("other_error"),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["/ripple/0/in/0: no check in `other_error` is the same as `check_quantity`"],
    );
    assert_refused(
        &temp.path,
        &guard("reused"),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["/ripple/0/in/0: a check in `reused` has the shape of `check_quantity` on `quantity` but defines entry.low, which block `entry__if0` uses"],
    );
    // A checker that changes the value it checks is outside preserve.
    let normalizing = json!({"fn": "to_index", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>",
      "blocks": [{"name": "entry", "ops": [["!InvalidQuantity", "if", ["lt", "q", 1]]], "term": ["ok", ["sub?Overflow", "q", 1]]}]});
    assert_refused(
        &temp.path,
        &guard_frame(&normalizing, &json!({"guard": "to_index", "arg": "quantity", "in": ["discounted"]})),
        "AGENT_RIPPLE_GUARD_ORDER",
        &[
            "preserve replaces a check that is the same pure check as `to_index`'s body, but block `",
            "` succeeds with a value other than its parameter",
        ],
    );
    // A checker that loops is outside it too.
    let looping = json!({"fn": "spin", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>",
      "blocks": [{"name": "entry", "term": ["br", "again", "q"]},
                 {"name": "again", "params": [["v", "i64"]], "term": ["cond", ["lt", "v", 1], ["again", "v"], "done"]},
                 {"name": "done", "term": ["ok", "q"]}]});
    assert_refused(
        &temp.path,
        &guard_frame(&looping, &json!({"guard": "spin", "arg": "quantity", "in": ["discounted"]})),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["its body loops"],
    );
}

/// `to_index(q)`: 1-based to 0-based, refusing quantities below 1.
fn to_index() -> Value {
    json!({"fn": "to_index", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["!InvalidQuantity", "if", ["lt", "q", 1]]],
                       "term": ["ok", ["sub?Overflow", "q", 1]]}]})
}

fn err(case: &str) -> Value {
    json!({"Err": case})
}

#[test]
fn entry_checks_first_and_routes_uses_through_the_payload() {
    let temp = orders_workspace("entry");
    let frame = guard_frame(
        &to_index(),
        &json!({"guard": "to_index", "arg": "quantity", "in": ["line_total", "shipped", "after_call"], "mode": "entry"}),
    );
    let report = valid(&temp.path, &frame);
    let inventory = artifact(&temp.path, report["draft"].as_str().unwrap(), "ripple.json");
    let functions = inventory["intents"][0]["functions"].as_array().unwrap();
    assert_eq!(functions[0], json!({"fn": "line_total", "edit": "entry", "uses": 2, "error": "__err"}));
    let expanded = artifact(&temp.path, report["draft"].as_str().unwrap(), "expanded.json");
    let line = expanded["patch"]
        .as_array()
        .unwrap()
        .iter()
        .find(|patch| patch["fn"] == "line_total")
        .unwrap();
    assert_eq!(line["entry"], "quantity__guard");
    // The authored intent: to_index runs once, first; its error wins over
    // the price error; everything after reads the 0-based quantity, and the
    // old quantity check stays (it now sees that value).
    let line_total = |quantity: i64, price: i64| -> Value {
        if quantity < 1 {
            return err("InvalidQuantity");
        }
        let index = quantity - 1;
        if price < 0 {
            return err("InvalidPrice");
        }
        if index < 1 {
            return err("InvalidQuantity");
        }
        index.checked_mul(price).map_or_else(|| err("Overflow"), |total| json!({"Ok": total}))
    };
    let mut after = Machine::candidate(&temp.path, &frame);
    for quantity in [i64::MIN, -1, 0, 1, 2, 7, i64::MAX] {
        for price in [i64::MIN, -1, 0, 5, i64::MAX] {
            assert_eq!(
                after.call("line_total", &[json!(quantity), json!(price)]),
                line_total(quantity, price),
                "line_total({quantity}, {price})"
            );
        }
    }
    // Early returns no longer come first: the check is at entry.
    assert_eq!(after.call("shipped", &[json!(0), json!(true), json!(true)]), err("InvalidQuantity"));
    assert_eq!(after.call("shipped", &[json!(3), json!(true), json!(true)]), json!({"Ok": 0}));
    assert_eq!(after.call("shipped", &[json!(3), json!(false), json!(false)]), json!({"Ok": 2}));
    // A trapping call that ran first now runs after the check.
    assert_eq!(after.call("after_call", &[json!(0), json!(0)]), err("InvalidQuantity"));
    assert_eq!(after.call("after_call", &[json!(0), json!(3)]), json!({"trap": 1}));
    assert_eq!(after.call("after_call", &[json!(4), json!(3)]), json!({"Ok": 6}));
}

#[test]
fn entry_errors_take_the_one_compatible_route_or_leave_a_hole() {
    let temp = orders_workspace("routes");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "AppError", "variant": [["Order", "OrderError"], "Other"]}], "fns": [
          {"fn": "validate", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["!Overflow", "if", ["gt", "quantity", 100]]], "term": ["ok", "quantity"]}]},
          // Another error type, and one block that takes an OrderError.
          {"fn": "wrapped", "params": [["quantity", "i64"]], "returns": "Result<i64,AppError>",
           "blocks": [{"name": "entry", "ops": [["v", "call?wrap", "validate", "quantity"]], "term": ["ok", "v"]},
                      {"name": "wrap", "params": [["e", "OrderError"]], "term": ["fail", "Order", "e"]}]},
          // Another error type, and nowhere to put an OrderError.
          {"fn": "unrelated", "params": [["quantity", "i64"]], "returns": "Result<i64,AppError>",
           "blocks": [{"name": "entry", "ops": [["!Other", "if", ["gt", "quantity", 100]]], "term": ["ok", "quantity"]}]},
          // The compatible result, and a handler block as well: two routes.
          {"fn": "two_routes", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["v", "call?log", "validate", "quantity"]], "term": ["ok", "v"]},
                      {"name": "log", "params": [["e", "OrderError"]], "term": ["fail", "Overflow"]}]}]}),
    );
    let guard = |function: &str| {
        guard_frame(
            &check_quantity(),
            &json!({"guard": "check_quantity", "arg": "quantity", "in": [function], "mode": "entry"}),
        )
    };
    let frame = guard("wrapped");
    valid(&temp.path, &frame);
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(after.call("wrapped", &[json!(0)]), json!({"Err": {"Order": "InvalidQuantity"}}));
    assert_eq!(after.call("wrapped", &[json!(500)]), json!({"Err": {"Order": "Overflow"}}));
    assert_eq!(after.call("wrapped", &[json!(5)]), json!({"Ok": 5}));
    let obligations = assert_refused(
        &temp.path,
        &guard("unrelated"),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &["/ripple/0/in/0: `check_quantity` fails with OrderError, and `unrelated` has no route for it"],
    );
    assert_eq!(obligations[0]["expected"], "OrderError");
    assert_refused(
        &temp.path,
        &guard("two_routes"),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &["`two_routes` has 2 candidate route for it: block `log`, returning it (Result<i64,OrderError>)"],
    );
}

#[test]
fn entry_leaves_uses_it_does_not_dominate() {
    let temp = workspace("dominated");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [order_error()], "fns": [
          {"fn": "kept", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "term": ["ok", "quantity"]},
                      {"name": "dead", "unreachable": true, "term": ["ok", "quantity"]}]}]}),
    );
    let frame = guard_frame(
        &to_index(),
        &json!({"guard": "to_index", "arg": "quantity", "in": ["kept"], "mode": "entry"}),
    );
    let report = valid(&temp.path, &frame);
    let expanded = artifact(&temp.path, report["draft"].as_str().unwrap(), "expanded.json");
    let patch = &expanded["patch"][0];
    // The reachable use reads the checked value; the unreachable block,
    // which entry does not dominate, keeps the parameter and is not restated.
    assert!(patch["blocks"].get("dead").is_none(), "{patch:#}");
    let entry = serde_json::to_string(&patch["blocks"]["entry"]).unwrap();
    assert!(entry.contains("quantity__guarded.quantity__ok"), "{entry}");
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(after.call("kept", &[json!(5)]), json!({"Ok": 4}));
}

#[test]
fn a_guard_never_runs_twice_or_twice_over() {
    let temp = orders_workspace("repeated");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [check_quantity(),
          {"fn": "checked", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["q", "call?", "check_quantity", "quantity"]], "term": ["ok", "q"]}]}]}),
    );
    // It already calls the checker: entry would run it again.
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"guard": "check_quantity", "arg": "quantity", "in": ["checked"], "mode": "entry"}]}),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["/ripple/0/in/0: `checked` already calls `check_quantity` on `quantity` at `checked.entry.q__r`; evaluating it again at entry would run it twice"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"guard": "check_quantity", "arg": "quantity", "in": ["checked"]}]}),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["(`checked` already calls `check_quantity` on `quantity` at `checked.entry.q__r`)"],
    );
    // A function listed twice is guarded once.
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"guard": "check_quantity", "arg": "quantity", "in": ["discounted", "discounted"]}]}),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["/ripple/0/in/1: `discounted` is listed twice"],
    );
    // A second guard of the same function finds the first one's call.
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [
          {"guard": "check_quantity", "arg": "quantity", "in": ["discounted"]},
          {"guard": "check_quantity", "arg": "quantity", "in": ["discounted"], "mode": "entry"}]}),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["/ripple/1/in/0: `discounted` already calls `check_quantity` on `quantity`"],
    );
}

#[test]
fn guards_compose_in_written_order() {
    let temp = orders_workspace("compose");
    let check_price = json!({"fn": "check_price", "params": [["p", "i64"]], "returns": "Result<i64,OrderError>",
      "blocks": [{"name": "entry", "ops": [["!InvalidPrice", "if", ["lt", "p", 0]]], "term": ["ok", "p"]}]});
    let frame = json!({"af1": 1, "afx": 1, "fns": [check_quantity(), check_price], "ripple": [
      {"guard": "check_quantity", "arg": "quantity", "in": ["line_total"]},
      {"guard": "check_price", "arg": "price", "in": ["line_total"]}]});
    let report = valid(&temp.path, &frame);
    let inventory = artifact(&temp.path, report["draft"].as_str().unwrap(), "ripple.json");
    assert_eq!(inventory["intents"][0]["functions"][0]["checks"], json!(["line_total.entry__if0"]));
    assert_eq!(inventory["intents"][1]["functions"][0]["checks"], json!(["line_total.entry"]));
    // One patch of line_total carries both; the error exit is shared.
    let expanded = artifact(&temp.path, report["draft"].as_str().unwrap(), "expanded.json");
    let patches: Vec<&Value> = expanded["patch"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|patch| patch["fn"] == "line_total")
        .collect();
    assert_eq!(patches.len(), 1);
    same_behavior(&temp.path, &frame, "line_total", &grid(&[&quantities(), &prices()]));
}

#[test]
fn guard_shapes_and_targets_are_checked() {
    let temp = orders_workspace("shapes");
    let guard = |checker: Value, function: &str, arg: &str| {
        let name = checker["fn"].as_str().unwrap().to_owned();
        guard_frame(&checker, &json!({"guard": name, "arg": arg, "in": [function]}))
    };
    let option = json!({"fn": "maybe", "params": [["q", "i64"]], "returns": "Option<i64>",
      "blocks": [{"name": "entry", "term": ["return", ["some", "q"]]}]});
    assert_refused(
        &temp.path,
        &guard(option, "discounted", "quantity"),
        "AGENT_RIPPLE_GUARD_SHAPE",
        &["/ripple/0/guard: `maybe` returns Option<i64>; a guard is P -> Result<P,E>, and no error case is inferred for None"],
    );
    let two = json!({"fn": "both", "params": [["q", "i64"], ["p", "i64"]], "returns": "Result<i64,OrderError>",
      "blocks": [{"name": "entry", "term": ["ok", "q"]}]});
    assert_refused(
        &temp.path,
        &guard(two, "discounted", "quantity"),
        "AGENT_RIPPLE_GUARD_SHAPE",
        &["/ripple/0/guard: `both` takes (i64, i64) and returns Result<i64,OrderError>; a guard is one parameter P -> Result<P,E>"],
    );
    let widening = json!({"fn": "widen", "params": [["q", "i64"]], "returns": "Result<(i64,i64),OrderError>",
      "blocks": [{"name": "entry", "term": ["ok", ["tuple", "q", "q"]]}]});
    assert_refused(
        &temp.path,
        &guard(widening, "discounted", "quantity"),
        "AGENT_RIPPLE_GUARD_SHAPE",
        &["`widen` returns Result<(i64,i64),OrderError> for a i64 parameter; a guard keeps the checked type"],
    );
    let small = json!({"fn": "small", "params": [["q", "u8"]], "returns": "Result<u8,OrderError>",
      "blocks": [{"name": "entry", "term": ["ok", "q"]}]});
    assert_refused(
        &temp.path,
        &guard(small, "discounted", "quantity"),
        "AGENT_RIPPLE_GUARD_SHAPE",
        &["/ripple/0/in/0: `small` checks u8, but parameter `quantity` of `discounted` is i64 (expected u8)"],
    );
    assert_refused(
        &temp.path,
        &guard(check_quantity(), "discounted", "qty"),
        "AGENT_RIPPLE_TARGET_KIND",
        &["/ripple/0/in/0: `discounted` has no parameter `qty` (it takes quantity, price, off)"],
    );
    assert_refused(
        &temp.path,
        &guard(check_quantity(), "OrderError", "quantity"),
        "AGENT_RIPPLE_TARGET_KIND",
        &["/ripple/0/in/0: `OrderError` is a TypeDef, not a function"],
    );
    assert_refused(
        &temp.path,
        &guard(check_quantity(), "check_quantity", "q"),
        "AGENT_RIPPLE_TARGET_KIND",
        &["/ripple/0/in/0: `check_quantity` is the checker itself"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"guard": "absent", "arg": "quantity", "in": ["discounted"]}]}),
        "AGENT_RIPPLE_TARGET_KIND",
        &["/ripple/0/guard: no function named `absent`"],
    );
    // A function the frame restates is the author's: guard leaves it alone.
    let restated = json!({"af1": 1, "afx": 1, "fns": [check_quantity()],
      "patch": [{"fn": "discounted", "blocks": {"entry": {"ops": [["!InvalidQuantity", "if", ["lt", "quantity", 1]]], "term": ["ok", "price"]}}}],
      "ripple": [{"guard": "check_quantity", "arg": "quantity", "in": ["discounted"]}]});
    assert_refused(
        &temp.path,
        &restated,
        "AGENT_RIPPLE_TARGET_KIND",
        &["/ripple/0/in/0: `discounted` is restated by this frame"],
    );
}

#[test]
fn effects_before_a_check_keep_entry_from_moving_it() {
    use sley_mutate::value::EntityBodyValue;
    let temp = orders_workspace("effects");
    let head = Workspace::at(&temp.path).head().unwrap();
    let names = names_of(&temp.path, head.program());
    let target = names.resolve("after_call").unwrap();
    // The workbench cannot author effects; give `after_call` a declared
    // effect in the state the expansion reads.
    let objects: Vec<sley_mutate::EntityObject> = head
        .program()
        .objects()
        .iter()
        .map(|object| {
            let record = object.record();
            if record.entity_id != target {
                return object.clone();
            }
            let EntityBodyValue::Function(mut function) = record.body.clone() else {
                unreachable!()
            };
            function.effects = sley_mutate::value::EntityIdSet::from_unsorted(vec![
                sley_id::EntityId::from_bytes([0xef; 32]),
            ])
            .unwrap();
            sley_mutate::build_entity_object(
                head.program().epoch(),
                &sley_mutate::EntityObjectRecord {
                    entity_id: record.entity_id,
                    body: EntityBodyValue::Function(function),
                    label: record.label.clone(),
                    semantic_fingerprint: record.semantic_fingerprint,
                },
            )
            .unwrap()
        })
        .collect();
    let program = Program::new(
        head.program().epoch(),
        head.program().root(),
        head.program().workspace(),
        objects,
    );
    let names = Names::build(&program, &name_map(&temp.path));
    let frame = guard_frame(
        &check_quantity(),
        &json!({"guard": "check_quantity", "arg": "quantity", "in": ["after_call"], "mode": "entry"}),
    );
    let expansion = sley_agent::afx::expand(&program, &names, &frame).unwrap();
    assert_eq!(expansion.obligations.len(), 1, "{:?}", expansion.obligations);
    assert_eq!(expansion.obligations[0].symbol.symbol(), "AGENT_RIPPLE_GUARD_ORDER");
    assert!(
        expansion.obligations[0]
            .decision
            .starts_with("`after_call` performs effects (it declares effects); evaluating `check_quantity` first could skip them"),
        "{}",
        expansion.obligations[0].decision
    );
    // Preserve keeps the order, so it still applies there.
    let preserve = guard_frame(
        &check_quantity(),
        &json!({"guard": "check_quantity", "arg": "quantity", "in": ["after_call"]}),
    );
    let expansion = sley_agent::afx::expand(&program, &names, &preserve).unwrap();
    assert!(expansion.obligations.is_empty(), "{:?}", expansion.obligations);
}
