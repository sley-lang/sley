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
    serde_json::from_str(
        &fs::read_to_string(&path).unwrap_or_else(|_| panic!("{}", path.display())),
    )
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
      // Two calls in one block.
      {"fn": "pair", "params": [["x", "i64"], ["y", "i64"]],
       "returns": "(Result<i64,ArithmeticError>,Result<i64,ArithmeticError>)",
       "blocks": [{"name": "entry", "ops": [["r1", "call", "f", "x", "y"], ["r2", "call", "f", "y", "x"]],
                   "term": ["return", ["tuple", "r1", "r2"]]}]},
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
        .map(|change| {
            format!(
                "{} {}",
                change["kind"].as_str().unwrap(),
                change["name"].as_str().unwrap()
            )
        })
        .collect();
    for entity in ["fn f", "fn g", "fn h", "fn pair", "fn twice", "test t_f"] {
        assert!(changed.contains(&entity.to_owned()), "{changed:?}");
    }
    assert!(!changed.contains(&"test t_g".to_owned()), "{changed:?}");
    // Every argument goes by name: b <- old b, a <- old a, c <- 0.
    let expanded = artifact(&temp.path, draft, "expanded.json");
    assert_eq!(
        patched_calls(&expanded, "g"),
        [(
            "r".to_owned(),
            vec![json!("r__a1"), json!("x"), json!("r__v2")]
        )]
    );
    assert_eq!(
        patched_calls(&expanded, "h"),
        [("r".to_owned(), vec![json!("x"), json!("y"), json!("r__v2")])]
    );
    assert_eq!(
        patched_calls(&expanded, "pair"),
        [
            (
                "r1".to_owned(),
                vec![json!("y"), json!("x"), json!("r1__v2")]
            ),
            (
                "r2".to_owned(),
                vec![json!("x"), json!("y"), json!("r2__v2")]
            ),
        ]
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
        json!({"name": "r__v2", "op": "const", "args": [{"type": "i64", "value": 0}], "type": "i64"})
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
    assert_eq!(intent["calls"].as_array().unwrap().len(), 7);
    assert!(
        intent["calls"]
            .as_array()
            .unwrap()
            .iter()
            .all(|call| call["edit"] == "rewritten" && call["origin"] == "live"),
        "{intent:#}"
    );
    assert_eq!(
        intent["tests"],
        json!([{"test": "t_f", "origin": "live", "edit": "rewritten"}])
    );
    assert_eq!(intent["boundary"]["references"], json!([]));
    assert_eq!(
        inventory["changed"]["functions"],
        json!(["g", "h", "pair", "twice"])
    );
    assert_eq!(inventory["changed"]["tests"], json!(["t_f"]));
    let status = artifact(&temp.path, draft, "status.json");
    assert_eq!(status["stats"]["ripple_intents"], 1);
    assert_eq!(status["stats"]["ripple_edits"], 8);
    assert_eq!(status["stats"]["ripple_holes"], 0);
    // Tests of the changed functions ran and pass; every caller computes
    // what it did before.
    assert_eq!(report["tests"].as_array().unwrap().len(), 2);
    same_behavior(&temp.path, &frame, "g", &grid(&[&ints()]));
    same_behavior(&temp.path, &frame, "h", &grid(&[&ints(), &ints()]));
    same_behavior(&temp.path, &frame, "pair", &grid(&[&ints(), &ints()]));
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
    assert_eq!(
        after.call("use_scaled", &[json!(3), json!(0)]),
        json!({"Err": "Zero"})
    );
    assert_eq!(
        after.call("use_scaled", &[json!(3), json!(7)]),
        json!({"Ok": 3})
    );
    assert_eq!(
        after.call("use_scaled", &[json!(i64::MAX), json!(1)]),
        json!({"Ok": i64::MAX})
    );
}

#[test]
fn a_rewritten_block_is_restated_exactly_but_for_the_call() {
    let temp = workspace("fidelity");
    let out = "(i64,u64,Option<i64>,bool,Option<i64>,Shape,Point,i64,bytes,bool,Option<i64>,i64,Option<i64>)";
    commit(
        &temp.path,
        &json!({"af1": 1,
          "types": [{"name": "Shape", "variant": ["Empty", ["Circle", "i64"]]}, {"name": "Point", "record": [["x", "i64"], ["y", "i64"]]}],
          "consts": [{"name": "limit", "type": "i64", "value": 10}],
          "fns": [
            {"fn": "f", "params": [["a", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
            {"fn": "wide", "params": [["s", "Shape"], ["p", "Point"], ["v", "Vec<i64>"], ["m", "Map<i64,i64>"], ["flag", "bool"]],
             "returns": out,
             "blocks": [
              {"name": "entry", "ops": [
                ["k", "const", "limit"], ["px", "field", "Point.x", "p"], ["q", "record", "Point", "px", "k"],
                ["n", "vec_len", "v"], ["g0", "vec_get", "v", "n"], ["has", "map_has", "m", "k"], ["mg", "map_get", "m", "k"],
                ["c", "variant", "Shape.Circle", "k"], ["t", "tuple", "k", "px"], ["t0", "tuple_get", 0, "t"],
                ["h", "hash", "k"], ["nf", "not", "flag"], ["both", "and", "nf", "has"],
                {"name": "nothing", "op": "none", "type": "Option<i64>"},
                ["cell", "cell", "k"], ["cv", "cell_get", "cell"], ["vg", "variant_get", "Shape.Circle", "s"],
                ["r", "call", "f", "cv"],
                ["o", "tuple", "r", "n", "g0", "has", "mg", "c", "q", "t0", "h", "both", "nothing", "px", "vg"]],
               "term": ["switch", "s", ["Empty", "done", "o"], ["Circle", "round", "$", "o"]]},
              {"name": "done", "params": [["x", out]], "term": ["return", "x"]},
              {"name": "round", "params": [["radius", "i64"], ["x", out]], "term": ["return", "x"]}]}]}),
    );
    let frame = json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]]}],
                       "ripple": [{"arity": "f", "value": 0}]});
    // Only the call, its new constant operation and their block change in
    // `wide`: every other operation is restated as it is, except that the
    // operations after the new one move down one ordinal.
    let head = Workspace::at(&temp.path).head().unwrap();
    let names = names_of(&temp.path, head.program());
    let compiled = sley_agent::frame::compile(
        head.program(),
        &names,
        &sley_agent::candidate::Authority::of(&head)
            .unwrap()
            .ceilings,
        &frame,
        sley_id::CandidateNonce::from_bytes([3; 32]),
        &mut || Ok([4; 32]),
    )
    .unwrap();
    let count = |kind: u16, class: &str| {
        compiled
            .ops
            .iter()
            .filter(|op| op.kind == kind && format!("{:?}", op.payload).starts_with(class))
            .count()
    };
    assert_eq!(
        count(8, "ReplaceEntityVersion"),
        2,
        "the call and the operation after it"
    );
    for op in &compiled.ops {
        let sley_mutate::MutationPayload::ReplaceEntityVersion(
            sley_mutate::value::EntityBodyValue::Operation(new),
        ) = &op.payload
        else {
            continue;
        };
        let Some(sley_mutate::value::EntityBodyValue::Operation(old)) =
            head.program().body(&op.target)
        else {
            panic!("replaced a missing operation");
        };
        assert_eq!(new.ordinal, old.ordinal + 1);
        if new.opcode != 112 {
            let mut moved = old.clone();
            moved.ordinal += 1;
            assert_eq!(*new, moved, "only the ordinal moves");
        }
    }
    assert_eq!(count(8, "CreateEntity"), 1, "one operation created");
    assert_eq!(count(7, "ReplaceEntityVersion"), 1, "one block replaced");
    assert_eq!(count(5, "ReplaceEntityVersion"), 1, "only f's signature");
    assert_eq!(count(6, "CreateEntity"), 1, "f's new parameter");
    assert_eq!(compiled.deleted, 0);
    let rows = [
        vec![
            json!("Empty"),
            json!({"x": 1, "y": 2}),
            json!([4, 5]),
            json!([[10, 3]]),
            json!(true),
        ],
        vec![
            json!({"Circle": 7}),
            json!({"x": -1, "y": 0}),
            json!([]),
            json!([]),
            json!(false),
        ],
    ];
    same_behavior(&temp.path, &frame, "wide", &rows);
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
    assert_eq!(obligations.len(), 8, "{obligations:#?}");
    for obligation in &obligations {
        assert_eq!(obligation["symbol"], "AGENT_RIPPLE_HOLE_UNFILLED");
        assert_eq!(obligation["at"], "/ripple/0");
        assert_eq!(obligation["expected"], "i64");
    }
    let decisions: Vec<&str> = obligations
        .iter()
        .map(|obligation| obligation["decision"].as_str().unwrap())
        .collect();
    for site in [
        "`g.entry.r`",
        "`h.entry.r`",
        "`twice.left.r__r`",
        "TestCase `t_f`",
    ] {
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
        (
            json!("zero"),
            "/ripple/0/value: state the value of new parameter `c` (expected i64) as a literal",
        ),
        (
            json!(2.5),
            "/ripple/0/value: 2.5 is not a literal of new parameter `c`'s type (expected i64)",
        ),
        (
            json!({"type": "u8", "value": 3}),
            "/ripple/0/value: the value's type is \"u8\", but new parameter `c` of `f` is i64",
        ),
        (
            json!({"type": "i64", "value": "x"}),
            "/ripple/0/value: the value does not fit new parameter `c` (expected i64)",
        ),
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
    // Parameters the head already has: the intent is applied, nothing is
    // derived (and "old" cannot be honoured); or none restated.
    let same = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]]}],
      "ripple": [{"arity": "f"}]});
    let expansion = expand(&temp.path, &same);
    assert!(
        expansion.obligations.is_empty(),
        "{:?}",
        expansion.obligations
    );
    let inventory = expansion.ripple.unwrap();
    assert_eq!(inventory["intents"][0]["edit"], "already applied");
    assert_eq!(inventory["edits"], 0);
    assert_eq!(expansion.frame["patch"].as_array().unwrap().len(), 1);
    // "old" then only reads the frame's own calls as the head has them;
    // this frame writes none.
    let mut old = same.clone();
    old["ripple"][0]["frame_calls"] = json!("old");
    let expansion = expand(&temp.path, &old);
    assert!(
        expansion.obligations.is_empty(),
        "{:?}",
        expansion.obligations
    );
    assert_eq!(
        expansion.ripple.unwrap()["intents"][0]["edit"],
        "already applied"
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"arity": "f"}]}),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &["/ripple/0/arity: the frame does not restate the parameters of `f`"],
    );
}

#[test]
#[allow(clippy::too_many_lines)]
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
           "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"]], "term": ["return", "r"]}]},
          {"fn": "w", "params": [["x", "i64"], ["flag", "bool"]], "returns": "Result<i64,ArithmeticError>",
           "blocks": [{"name": "entry", "term": ["cond", "flag", "kept", "restated"]},
                      {"name": "kept", "ops": [["r", "call", "f", "x"]], "term": ["return", "r"]},
                      {"name": "restated", "ops": [["r", "call", "f", 1]], "term": ["return", "r"]}]}]}),
    );
    let frame = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]],
                 "blocks": {"entry": {"term": ["return", ["add", "a", "b"]]}}},
                // Restated with the old argument count: rewritten.
                {"fn": "m", "blocks": {"entry": {"ops": [["one", "const", 1], ["r", "call", "f", "x"]],
                                                 "term": ["return", "r"]}}},
                // One block restated with the old count, one kept live.
                {"fn": "w", "blocks": {"restated": {"ops": [["r", "call", "f", 2]], "term": ["return", "r"]}}}],
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
    assert_eq!(m[1], json!(["r__v1", "const", {"type": "i64", "value": 0}]));
    assert_eq!(m[2], json!(["r", "call", "f", "x", "r__v1"]));
    assert_eq!(
        expanded["fns"][0]["blocks"][0]["ops"][0],
        json!(["r__a1", "const", {"type": "i64", "value": 5}])
    );
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
    assert_eq!(
        entry("/patch/1/blocks/entry/ops/1").unwrap()["authored"],
        "/ripple/0"
    );
    assert_eq!(
        entry("/patch/1/blocks/entry/ops/1").unwrap()["role"],
        "ripple"
    );
    assert_eq!(map["names"]["m"]["r__v1"], "/ripple/0");
    let inventory = artifact(&temp.path, draft, "ripple.json");
    let calls = &inventory["intents"][0]["calls"];
    let edits: Vec<(String, String, String)> = calls
        .as_array()
        .unwrap()
        .iter()
        .map(|call| {
            (
                call["site"]
                    .as_str()
                    .or(call["test"].as_str())
                    .unwrap()
                    .to_owned(),
                call["origin"].as_str().unwrap().to_owned(),
                call["edit"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(
        edits.contains(&(
            "/fns/0/blocks/0/ops/0 (k)".to_owned(),
            "frame".to_owned(),
            "as written".to_owned()
        )),
        "{edits:?}"
    );
    assert!(
        edits.contains(&(
            "/patch/1/blocks/entry/ops/1 (m)".to_owned(),
            "frame".to_owned(),
            "rewritten".to_owned()
        )),
        "{edits:?}"
    );
    assert!(
        edits.contains(&(
            "n.entry.r".to_owned(),
            "live".to_owned(),
            "rewritten".to_owned()
        )),
        "{edits:?}"
    );
    assert!(
        edits.contains(&(
            "t_old".to_owned(),
            "frame".to_owned(),
            "rewritten".to_owned()
        )),
        "{edits:?}"
    );
    assert!(
        edits.contains(&(
            "t_new".to_owned(),
            "frame".to_owned(),
            "as written".to_owned()
        )),
        "{edits:?}"
    );
    // w's kept block joins the author's patch of w, restated with the call
    // rewritten; the author's own block keeps its (rewritten) statement.
    let w = &expanded["patch"][2];
    assert_eq!(w["fn"], "w");
    assert_eq!(
        w["blocks"]["kept"]["ops"][1],
        json!({"name": "r", "op": "call", "args": ["f", "x", "r__v1"], "type": "Result<i64,ArithmeticError>"})
    );
    assert_eq!(
        w["blocks"]["restated"]["ops"][2],
        json!(["r", "call", "f", "r__a0", "r__v1"])
    );
    assert!(
        edits.contains(&(
            "w.kept.r".to_owned(),
            "live".to_owned(),
            "rewritten".to_owned()
        )),
        "{edits:?}"
    );
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(after.call("w", &[json!(4), json!(true)]), json!({"Ok": 4}));
    assert_eq!(after.call("w", &[json!(4), json!(false)]), json!({"Ok": 2}));
    assert_eq!(after.call("m", &[json!(4)]), json!({"Ok": 4}));
    assert_eq!(after.call("n", &[json!(4)]), json!({"Ok": 4}));
    assert_eq!(after.call("k", &[json!(4)]), json!({"Ok": 9}));
}

#[test]
fn an_edit_that_calls_the_function_is_rewritten_in_place_or_left_as_a_hole() {
    let temp = workspace("edits");
    commit(
        &temp.path,
        &json!({"af1": 1, "fns": [
          {"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
          {"fn": "one", "params": [["x", "i64"], ["y", "i64"]], "returns": "i64",
           "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x", "y"]], "term": ["return", "r"]}]},
          {"fn": "two", "params": [["x", "i64"], ["y", "i64"]], "returns": "i64",
           "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x", "y"], ["s", "call", "f", "y", "x"]], "term": ["return", "r"]}]}]}),
    );
    // Dropping a parameter needs no new operation: the edit is rewritten.
    let frame = json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"]]}],
      "edit": [{"fn": "one", "replace_op": "entry.r", "with": ["call", "f", "y", "x"]}],
      "ripple": [{"arity": "f"}]});
    let report = valid(&temp.path, &frame);
    let expanded = artifact(
        &temp.path,
        report["draft"].as_str().unwrap(),
        "expanded.json",
    );
    assert_eq!(expanded["edit"][0]["with"], json!(["call", "f", "y"]));
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(after.call("one", &[json!(5), json!(9)]), json!(9));
    assert_eq!(after.call("two", &[json!(5), json!(9)]), json!(5));
    // A new argument needs an operation an edit cannot add; and another call
    // in a function the frame edits is the author's to restate.
    let obligations = assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"], ["c", "i64"]]}],
          "edit": [{"fn": "one", "replace_op": "entry.r", "with": ["call", "f", "y", "x"]},
                   {"fn": "two", "replace_op": "entry.r", "with": ["call", "f", "y", "x"]}],
          "ripple": [{"arity": "f", "value": 1}]}),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &[
            "/ripple/0: /edit/0/with (one) is an edit, which cannot add the operation for the new argument: restate its block with patch",
            "/ripple/0: `two.entry.s` calls `f`, and this frame changes `two` with edit: restate block `entry` with patch",
        ],
    );
    assert_eq!(obligations.len(), 3, "{obligations:#?}");
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
    let expanded = artifact(
        &temp.path,
        report["draft"].as_str().unwrap(),
        "expanded.json",
    );
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
        &[
            "/ripple/0: `g.entry.r` passes an argument of type i64 for parameter `a`, which `f` now takes as u8 (expected u8)",
        ],
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
        &[
            "/ripple/0: `g.entry.fr` uses `f` as a function value (fnref): calls through it are unresolved dispatch",
        ],
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
        &[
            "/ripple/0: `g.entry.r` calls `f` from namespace ns_b while `f` is in ns_a: ripple does not edit code across a namespace boundary",
        ],
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
    assert_eq!(
        expansion.obligations.len(),
        1,
        "{:?}",
        expansion.obligations
    );
    let obligation = &expansion.obligations[0];
    assert_eq!(obligation.symbol.symbol(), "AGENT_RIPPLE_EXPORTED_BOUNDARY");
    assert_eq!(obligation.at, "/ripple/0/arity");
    assert!(
        obligation
            .decision
            .starts_with("`f` is the entry point `serve`"),
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
    let mut ops: Vec<Value> = (0..257)
        .map(|n| json!([format!("r{n}"), "call", "f", "x"]))
        .collect();
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
#[allow(clippy::too_many_lines)]
fn targets_of_the_wrong_kind_and_disabled_intents_are_refused() {
    let temp = arity_workspace("kinds");
    commit(
        &temp.path,
        &json!({"af1": 1, "types": [{"name": "Shape", "variant": ["Empty"]}],
                "consts": [{"name": "limit", "type": "i64", "value": 3}]}),
    );
    for (target, needle) in [
        (
            "Shape",
            "/ripple/0/arity: `Shape` is a TypeDef, not a function",
        ),
        (
            "limit",
            "/ripple/0/arity: `limit` is a Constant, not a function",
        ),
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
        let word = intent
            .as_object()
            .unwrap()
            .keys()
            .find(|key| *key != "add" && *key != "to")
            .unwrap()
            .clone();
        assert_refused(
            &temp.path,
            &json!({"af1": 1, "afx": 1, "ripple": [intent]}),
            "AGENT_RIPPLE_INTENT_UNKNOWN",
            &[&format!(
                "/ripple/0: `{word}` is not enabled in this build; the enabled intents are arity and guard"
            )],
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
        &[
            "/ripple/1/arity: the parameters of `f` are already propagated by the intent at /ripple/0",
        ],
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
    assert_eq!(first.stats.ripple_edits, 8);
    // A hole-bearing derivation is deterministic too.
    let holes = new_f(&json!([{"arity": "f"}]));
    let (a, b) = (expand(&temp.path, &holes), expand(&temp.path, &holes));
    assert_eq!(a.obligations, b.obligations);
    assert_eq!(a.obligations.len(), 8);
    // The same frame layered on its own draft derives the same edits.
    let report = valid(&temp.path, &frame);
    let draft = report["draft"]
        .as_str()
        .unwrap()
        .split('@')
        .next()
        .unwrap()
        .to_owned();
    let more = json!({"af1": 1, "tests": [{"name": "t_h", "fn": "h", "args": [1, 3], "expect": {"Ok": 2}}]});
    let (status, layered) = run_json(&temp.path, &["try", "--on", &draft, &more.to_string()]);
    assert_eq!(status, 0, "{layered:#}");
    let revision = layered["draft"].as_str().unwrap();
    let expanded = artifact(&temp.path, revision, "expanded.json");
    let original = artifact(
        &temp.path,
        report["draft"].as_str().unwrap(),
        "expanded.json",
    );
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
    assert!(
        text.starts_with("error AGENT_CANDIDATE_INVALID: SCB_FLOAT_NON_CANONICAL"),
        "{text}"
    );
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
    [i64::MIN, -1, 0, 1, 2, 7, i64::MAX]
        .iter()
        .map(|n| json!(n))
        .collect()
}

fn prices() -> Vec<Value> {
    [i64::MIN, -1, 0, 5, i64::MAX]
        .iter()
        .map(|n| json!(n))
        .collect()
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
    assert!(
        functions.iter().all(|f| f["edit"] == "replaced"),
        "{intent:#}"
    );
    // line_total's check stays after the price check; shipped has two.
    assert_eq!(functions[0]["checks"], json!(["line_total.entry__if0"]));
    assert_eq!(
        functions[2]["checks"],
        json!(["shipped.fast", "shipped.slow"])
    );
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
        json!([
            "switch",
            "quantity__check_quantity",
            ["Ok", "entry__if1"],
            ["Err", "__err", "$"]
        ])
    );
    // Same results for every input, simultaneous invalid ones included:
    // the price error still wins in line_total, the early return and the
    // trapping call still come first.
    same_behavior(
        &temp.path,
        &frame,
        "line_total",
        &grid(&[&quantities(), &prices()]),
    );
    same_behavior(
        &temp.path,
        &frame,
        "discounted",
        &grid(&[&quantities(), &prices(), &prices()]),
    );
    same_behavior(
        &temp.path,
        &frame,
        "shipped",
        &grid(&[&quantities(), &bools(), &bools()]),
    );
    same_behavior(
        &temp.path,
        &frame,
        "after_call",
        &grid(&[&[json!(0), json!(3)], &quantities()]),
    );
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(
        after.call("line_total", &[json!(0), json!(-1)]),
        json!({"Err": "InvalidPrice"})
    );
    assert_eq!(
        after.call("shipped", &[json!(0), json!(true), json!(true)]),
        json!({"Ok": 0})
    );
    assert_eq!(
        after.call("after_call", &[json!(0), json!(0)]),
        json!({"trap": 1})
    );
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
    assert_eq!(
        inventory["intents"][0]["functions"][0]["checks"],
        json!(["unit_price.entry"])
    );
    assert_eq!(
        inventory["intents"][0]["functions"][0]["deleted"],
        json!(["reject"])
    );
    same_behavior(
        &temp.path,
        &frame,
        "unit_price",
        &grid(&[&prices(), &quantities()]),
    );
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
        &[
            "/ripple/0/in/0: no check in `le_check` is the same as `check_quantity` on `quantity`",
            "use \"mode\": \"entry\"",
        ],
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
        &[
            "/ripple/0/in/0: a check in `reused` has the shape of `check_quantity` on `quantity` but defines entry.low, which block `entry__if0` uses",
        ],
    );
    // A checker that changes the value it checks is outside preserve.
    let normalizing = json!({"fn": "to_index", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>",
      "blocks": [{"name": "entry", "ops": [["!InvalidQuantity", "if", ["lt", "q", 1]]], "term": ["ok", ["sub?Overflow", "q", 1]]}]});
    assert_refused(
        &temp.path,
        &guard_frame(
            &normalizing,
            &json!({"guard": "to_index", "arg": "quantity", "in": ["discounted"]}),
        ),
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
        &guard_frame(
            &looping,
            &json!({"guard": "spin", "arg": "quantity", "in": ["discounted"]}),
        ),
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
    assert_eq!(
        functions[0],
        json!({"fn": "line_total", "edit": "entry", "uses": 2, "error": "__err"})
    );
    let expanded = artifact(
        &temp.path,
        report["draft"].as_str().unwrap(),
        "expanded.json",
    );
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
        index
            .checked_mul(price)
            .map_or_else(|| err("Overflow"), |total| json!({"Ok": total}))
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
    // Early returns no longer come first: the check is at entry. A success
    // on a path that never read the quantity, and a trap before its check,
    // both become the checker's error; the reference texts say so.
    for text in [
        sley_agent::help::AFX,
        include_str!("../../../docs/spec/SLEY_AGENT_V1.md"),
    ] {
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(flat.contains("returned early, trapped"), "entry documented");
    }
    assert_eq!(
        after.call("shipped", &[json!(0), json!(true), json!(true)]),
        err("InvalidQuantity")
    );
    assert_eq!(
        after.call("shipped", &[json!(3), json!(true), json!(true)]),
        json!({"Ok": 0})
    );
    assert_eq!(
        after.call("shipped", &[json!(3), json!(false), json!(false)]),
        json!({"Ok": 2})
    );
    // A trapping call that ran first now runs after the check.
    assert_eq!(
        after.call("after_call", &[json!(0), json!(0)]),
        err("InvalidQuantity")
    );
    assert_eq!(
        after.call("after_call", &[json!(0), json!(3)]),
        json!({"trap": 1})
    );
    assert_eq!(
        after.call("after_call", &[json!(4), json!(3)]),
        json!({"Ok": 6})
    );
}

#[test]
fn entry_errors_go_to_the_declared_result_or_a_named_handler_only() {
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
          // The compatible result, and a block taking an OrderError that
          // the author does not name.
          {"fn": "two_routes", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["v", "call?log", "validate", "quantity"]], "term": ["ok", "v"]},
                      {"name": "log", "params": [["e", "OrderError"]], "term": ["fail", "Overflow"]}]}]}),
    );
    let guard = |function: &str, handler: Option<&str>| {
        let mut intent = json!({"guard": "check_quantity", "arg": "quantity", "in": [function], "mode": "entry"});
        if let Some(handler) = handler {
            intent["handler"] = json!(handler);
        }
        guard_frame(&check_quantity(), &intent)
    };
    // A block that takes the error type is not a route until it is named.
    assert_refused(
        &temp.path,
        &guard("wrapped", None),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &[
            "/ripple/0/in/0: `check_quantity` fails with OrderError, which `wrapped` (returning Result<i64,AppError>) cannot return: name the block of `wrapped` that takes the error with \"handler\"",
        ],
    );
    let frame = guard("wrapped", Some("wrap"));
    valid(&temp.path, &frame);
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(
        after.call("wrapped", &[json!(0)]),
        json!({"Err": {"Order": "InvalidQuantity"}})
    );
    assert_eq!(
        after.call("wrapped", &[json!(500)]),
        json!({"Err": {"Order": "Overflow"}})
    );
    assert_eq!(after.call("wrapped", &[json!(5)]), json!({"Ok": 5}));
    let obligations = assert_refused(
        &temp.path,
        &guard("unrelated", None),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &[
            "/ripple/0/in/0: `check_quantity` fails with OrderError, which `unrelated` (returning Result<i64,AppError>) cannot return",
        ],
    );
    assert_eq!(obligations[0]["expected"], "OrderError");
    assert_refused(
        &temp.path,
        &guard("unrelated", Some("nowhere")),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &[
            "/ripple/0/in/0: `unrelated` has no block `nowhere` that takes one OrderError (the error of `check_quantity`)",
        ],
    );
    // With the declared result, the error is returned: the unnamed block
    // `log` is not a route.
    let frame = guard("two_routes", None);
    valid(&temp.path, &frame);
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(
        after.call("two_routes", &[json!(0)]),
        json!({"Err": "InvalidQuantity"})
    );
    assert_eq!(
        after.call("two_routes", &[json!(500)]),
        json!({"Err": "Overflow"})
    );
    // A handler applies to entry mode only.
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [check_quantity()], "ripple": [
          {"guard": "check_quantity", "arg": "quantity", "in": ["wrapped"], "handler": "wrap"}]}),
        "AGENT_FRAME_INVALID",
        &["/ripple/0/handler: a handler takes the checker's error in \"mode\": \"entry\""],
    );
}

#[test]
fn a_join_block_of_the_error_type_is_never_an_error_route() {
    // An ordinary join block that happens to take one value of the error
    // type must not receive the checker's failure.
    let temp = workspace("join");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [
          {"fn": "nonneg", "params": [["q", "i64"]], "returns": "Result<i64,i64>",
           "blocks": [{"name": "entry", "term": ["cond", ["lt", "q", 0], "bad", "good"]},
                      {"name": "bad", "term": ["return", ["err", "q"]]},
                      {"name": "good", "term": ["return", ["ok", "q"]]}]},
          {"fn": "pick", "params": [["p", "i64"], ["flag", "bool"]], "returns": "i64",
           "blocks": [{"name": "entry", "term": ["cond", "flag", "a", "b"]},
                      {"name": "a", "term": ["br", "done", "p"]},
                      {"name": "b", "term": ["br", "done", 0]},
                      {"name": "done", "params": [["r", "i64"]], "term": ["return", "r"]}]}]}),
    );
    let obligations = assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"guard": "nonneg", "arg": "p", "in": ["pick"], "mode": "entry"}]}),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &[
            "/ripple/0/in/0: `nonneg` fails with i64, which `pick` (returning i64) cannot return: name the block of `pick` that takes the error with \"handler\"",
        ],
    );
    assert_eq!(obligations.len(), 1);
    // Named explicitly, the join block is the author's decision.
    let frame = json!({"af1": 1, "afx": 1,
      "ripple": [{"guard": "nonneg", "arg": "p", "in": ["pick"], "mode": "entry", "handler": "done"}]});
    valid(&temp.path, &frame);
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(after.call("pick", &[json!(-5), json!(false)]), json!(-5));
    assert_eq!(after.call("pick", &[json!(5), json!(false)]), json!(0));
}

#[test]
fn entry_leaves_uses_it_does_not_dominate() {
    let temp = workspace("dominated");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [order_error()], "fns": [
          {"fn": "kept", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "term": ["ok", "quantity"]},
                      {"name": "dead", "unreachable": true, "term": ["ok", "quantity"]}]},
          {"fn": "validate", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["!Overflow", "if", ["gt", "quantity", 100]]], "term": ["ok", "quantity"]}]},
          // The error handler reads the parameter too: the checker's error
          // reaches it without the checked value.
          {"fn": "handled", "params": [["quantity", "i64"]], "returns": "Result<(i64,i64),i64>",
           "blocks": [{"name": "entry", "ops": [["v", "call?log", "validate", "quantity"]],
                       "term": ["return", ["ok", ["tuple", "v", "quantity"]]]},
                      {"name": "log", "params": [["e", "OrderError"]], "term": ["return", ["err", "quantity"]]}]}]}),
    );
    let frame = guard_frame(
        &to_index(),
        &json!({"guard": "to_index", "arg": "quantity", "in": ["kept"], "mode": "entry"}),
    );
    let report = valid(&temp.path, &frame);
    let expanded = artifact(
        &temp.path,
        report["draft"].as_str().unwrap(),
        "expanded.json",
    );
    let patch = &expanded["patch"][0];
    // The reachable use reads the checked value; the unreachable block,
    // which entry does not dominate, keeps the parameter and is not restated.
    assert!(patch["blocks"].get("dead").is_none(), "{patch:#}");
    let entry = serde_json::to_string(&patch["blocks"]["entry"]).unwrap();
    assert!(entry.contains("quantity__guarded.quantity__ok"), "{entry}");
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(after.call("kept", &[json!(5)]), json!({"Ok": 4}));
    let frame = guard_frame(
        &to_index(),
        &json!({"guard": "to_index", "arg": "quantity", "in": ["handled"], "mode": "entry", "handler": "log"}),
    );
    let report = valid(&temp.path, &frame);
    let inventory = artifact(&temp.path, report["draft"].as_str().unwrap(), "ripple.json");
    assert_eq!(
        inventory["intents"][0]["functions"][0],
        json!({"fn": "handled", "edit": "entry", "uses": 2, "error": "log"})
    );
    let expanded = artifact(
        &temp.path,
        report["draft"].as_str().unwrap(),
        "expanded.json",
    );
    assert!(
        expanded["patch"][0]["blocks"].get("log").is_none(),
        "{expanded:#}"
    );
    let mut after = Machine::candidate(&temp.path, &frame);
    // The checker's error reaches `log`, which reads the parameter as given.
    assert_eq!(after.call("handled", &[json!(0)]), json!({"Err": 0}));
    // On the checked path everything reads the 0-based value, including
    // validate's own error, which also goes to `log`.
    assert_eq!(after.call("handled", &[json!(5)]), json!({"Ok": [4, 4]}));
    assert_eq!(after.call("handled", &[json!(102)]), json!({"Err": 102}));
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
    // It checks the quantity first itself, but not in the exact shape an
    // entry guard derives: entry would evaluate the checker again.
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"guard": "check_quantity", "arg": "quantity", "in": ["checked"], "mode": "entry"}]}),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["/ripple/0/in/0: `checked` already evaluates `check_quantity` at `checked.entry.q__r`"],
    );

    // It evaluates the checker in another shape: entry would run it again.
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [
          {"fn": "checked_late", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["d", "add?Overflow", "quantity", 1], ["q", "call?", "check_quantity", "quantity"]],
                       "term": ["ok", "q"]}]}]}),
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"guard": "check_quantity", "arg": "quantity", "in": ["checked_late"], "mode": "entry"}]}),
        "AGENT_RIPPLE_GUARD_ORDER",
        &[
            "/ripple/0/in/0: `checked_late` already evaluates `check_quantity` at `checked_late.entry__d.q__r`; evaluating it again at entry could run it twice",
        ],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"guard": "check_quantity", "arg": "quantity", "in": ["checked"]}]}),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["(`checked` already evaluates `check_quantity` at `checked.entry.q__r`)"],
    );
    // A checker that calls the function would call itself without end.
    let calls_back = json!({"fn": "calls_back", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>",
      "blocks": [{"name": "entry", "ops": [["r", "call?", "discounted", "q", 1, 0]], "term": ["ok", "q"]}]});
    assert_refused(
        &temp.path,
        &guard_frame(
            &calls_back,
            &json!({"guard": "calls_back", "arg": "quantity", "in": ["discounted"], "mode": "entry"}),
        ),
        "AGENT_RIPPLE_GUARD_ORDER",
        &[
            "/ripple/0/in/0: `calls_back` calls `discounted` (calls_back -> discounted): evaluating it at the entry of `discounted` would never end",
        ],
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
        &[
            "/ripple/1/in/0: `discounted` already evaluates `check_quantity` at `discounted.entry.quantity__check_quantity`",
        ],
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
    assert_eq!(
        inventory["intents"][0]["functions"][0]["checks"],
        json!(["line_total.entry__if0"])
    );
    assert_eq!(
        inventory["intents"][1]["functions"][0]["checks"],
        json!(["line_total.entry"])
    );
    // One patch of line_total carries both; the error exit is shared.
    let expanded = artifact(
        &temp.path,
        report["draft"].as_str().unwrap(),
        "expanded.json",
    );
    let patches: Vec<&Value> = expanded["patch"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|patch| patch["fn"] == "line_total")
        .collect();
    assert_eq!(patches.len(), 1);
    same_behavior(
        &temp.path,
        &frame,
        "line_total",
        &grid(&[&quantities(), &prices()]),
    );
}

#[test]
fn guard_shapes_and_targets_are_checked() {
    let temp = orders_workspace("shapes");
    let guard = |checker: Value, function: &str, arg: &str| {
        let name = checker["fn"].as_str().unwrap().to_owned();
        guard_frame(
            &checker,
            &json!({"guard": name, "arg": arg, "in": [function]}),
        )
    };
    let option = json!({"fn": "maybe", "params": [["q", "i64"]], "returns": "Option<i64>",
      "blocks": [{"name": "entry", "term": ["return", ["some", "q"]]}]});
    assert_refused(
        &temp.path,
        &guard(option, "discounted", "quantity"),
        "AGENT_RIPPLE_GUARD_SHAPE",
        &[
            "/ripple/0/guard: `maybe` returns Option<i64>; a guard is P -> Result<P,E>, and no error case is inferred for None",
        ],
    );
    let two = json!({"fn": "both", "params": [["q", "i64"], ["p", "i64"]], "returns": "Result<i64,OrderError>",
      "blocks": [{"name": "entry", "term": ["ok", "q"]}]});
    assert_refused(
        &temp.path,
        &guard(two, "discounted", "quantity"),
        "AGENT_RIPPLE_GUARD_SHAPE",
        &[
            "/ripple/0/guard: `both` takes (i64, i64) and returns Result<i64,OrderError>; a guard is one parameter P -> Result<P,E>",
        ],
    );
    let widening = json!({"fn": "widen", "params": [["q", "i64"]], "returns": "Result<(i64,i64),OrderError>",
      "blocks": [{"name": "entry", "term": ["ok", ["tuple", "q", "q"]]}]});
    assert_refused(
        &temp.path,
        &guard(widening, "discounted", "quantity"),
        "AGENT_RIPPLE_GUARD_SHAPE",
        &[
            "`widen` returns Result<(i64,i64),OrderError> for a i64 parameter; a guard keeps the checked type",
        ],
    );
    let small = json!({"fn": "small", "params": [["q", "u8"]], "returns": "Result<u8,OrderError>",
      "blocks": [{"name": "entry", "term": ["ok", "q"]}]});
    assert_refused(
        &temp.path,
        &guard(small, "discounted", "quantity"),
        "AGENT_RIPPLE_GUARD_SHAPE",
        &[
            "/ripple/0/in/0: `small` checks u8, but parameter `quantity` of `discounted` is i64 (expected u8)",
        ],
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
    assert_eq!(
        expansion.obligations.len(),
        1,
        "{:?}",
        expansion.obligations
    );
    assert_eq!(
        expansion.obligations[0].symbol.symbol(),
        "AGENT_RIPPLE_GUARD_ORDER"
    );
    assert!(
        expansion.obligations[0]
            .decision
            .starts_with("`after_call` performs effects (it declares effects); evaluating `check_quantity` first could skip them"),
        "{}",
        expansion.obligations[0].decision
    );
    // Preserve would keep the order, but a patch cannot restate declared
    // effects: the function is left alone, not silently changed.
    let preserve = guard_frame(
        &check_quantity(),
        &json!({"guard": "check_quantity", "arg": "quantity", "in": ["after_call"]}),
    );
    let expansion = sley_agent::afx::expand(&program, &names, &preserve).unwrap();
    assert_eq!(
        expansion.obligations.len(),
        1,
        "{:?}",
        expansion.obligations
    );
    assert_eq!(
        expansion.obligations[0].symbol.symbol(),
        "AGENT_RIPPLE_TARGET_KIND"
    );
    assert_eq!(
        expansion.obligations[0].decision,
        "`after_call` declares effects, which a patch cannot restate: guard leaves it"
    );
    // So is a caller that `arity` would have to restate.
    let arity = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "boom", "params": [["a", "i64"], ["b", "i64"]]}],
      "ripple": [{"arity": "boom", "value": 0}]});
    let expansion = sley_agent::afx::expand(&program, &names, &arity).unwrap();
    assert_eq!(
        expansion.obligations.len(),
        1,
        "{:?}",
        expansion.obligations
    );
    assert_eq!(
        expansion.obligations[0].decision,
        "`after_call.entry.z` calls `boom`, but `after_call` declares effects, which a patch cannot restate: change the call yourself"
    );
}

// ---------------------------------------------------------------------------
// Generated checkers: preserve keeps behavior, entry checks first
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[usize::try_from(self.next() % items.len() as u64).unwrap()]
    }
}

/// One to three exits `["!Case", "if", [cmp, v, k]]` on the value `v`,
/// sometimes over a combined condition.
fn generated_exits(rng: &mut Rng, v: &str) -> Vec<Value> {
    let count = 1 + rng.next() % 3;
    (0..count)
        .map(|_| {
            let case = rng.pick(&["InvalidQuantity", "InvalidPrice", "Overflow"]);
            let cmp = |rng: &mut Rng| {
                json!([
                    rng.pick(&["lt", "gt", "eq", "le", "ge", "ne"]),
                    v,
                    rng.pick(&[-2_i64, 0, 1, 7, 100])
                ])
            };
            let cond = match rng.next() % 3 {
                0 => json!(["and", cmp(rng), cmp(rng)]),
                1 => json!(["or", cmp(rng), cmp(rng)]),
                _ => cmp(rng),
            };
            json!([format!("!{case}"), "if", cond])
        })
        .collect()
}

#[test]
#[allow(clippy::too_many_lines)]
fn generated_checks_keep_their_behavior_and_entry_checks_first() {
    let temp = workspace("generated");
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    let count = 16;
    let mut live = Vec::new();
    let mut checkers = Vec::new();
    for index in 0..count {
        let exits = generated_exits(&mut rng, "q");
        // The function has the checker's exits written with its own names,
        // after a price check, before its own work.
        let own: Vec<Value> = exits
            .iter()
            .map(|exit| serde_json::from_str(&exit.to_string().replace("\"q\"", "\"n\"")).unwrap())
            .collect();
        let mut ops = vec![json!(["!InvalidPrice", "if", ["lt", "price", 0]])];
        ops.extend(own);
        ops.push(json!(["total", "mul?Overflow", "n", "price"]));
        live.push(
            json!({"fn": format!("f{index}"), "params": [["n", "i64"], ["price", "i64"]],
                         "returns": "Result<i64,OrderError>",
                         "blocks": [{"name": "entry", "ops": ops, "term": ["ok", "total"]}]}),
        );
        checkers.push(json!({"fn": format!("g{index}"), "params": [["q", "i64"]], "returns": "Result<i64,OrderError>",
                             "blocks": [{"name": "entry", "ops": exits, "term": ["ok", "q"]}]}));
    }
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [order_error()], "fns": live}),
    );
    let values: Vec<Value> = [i64::MIN, -3, -2, -1, 0, 1, 2, 7, 8, 99, 100, 101, i64::MAX]
        .iter()
        .map(|n| json!(n))
        .collect();
    let prices: Vec<Value> = [-1_i64, 0, 3, i64::MAX].iter().map(|n| json!(n)).collect();
    let rows = grid(&[&values, &prices]);
    for entry in [false, true] {
        let intents: Vec<Value> = (0..count)
            .map(|index| {
                json!({"guard": format!("g{index}"), "arg": "n", "in": [format!("f{index}")],
                       "mode": if entry { "entry" } else { "preserve" }})
            })
            .collect();
        let frame = json!({"af1": 1, "afx": 1, "fns": checkers, "ripple": intents});
        let report = valid(&temp.path, &frame);
        let inventory = artifact(&temp.path, report["draft"].as_str().unwrap(), "ripple.json");
        let edits: Vec<&str> = inventory["intents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|intent| intent["functions"][0]["edit"].as_str().unwrap())
            .collect();
        assert_eq!(
            edits,
            vec![if entry { "entry" } else { "replaced" }; count],
            "{inventory:#}"
        );
        let mut before = Machine::head(&temp.path);
        let mut after = Machine::candidate(&temp.path, &frame);
        for index in 0..count {
            let (f, g) = (format!("f{index}"), format!("g{index}"));
            for row in &rows {
                let old = before.call(&f, row);
                let new = after.call(&f, row);
                if entry {
                    // The checker first; on success, the function as it was.
                    let checked = after.call(&g, &row[..1]);
                    let expected = if checked.get("Err").is_some() {
                        checked
                    } else {
                        old
                    };
                    assert_eq!(new, expected, "entry {f}{row:?}");
                } else {
                    assert_eq!(new, old, "preserve {f}{row:?}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Regressions: the frame's own definitions, repeated evaluation,
// equal-count signatures, boundaries, bounds and the events ledger
// ---------------------------------------------------------------------------

/// The head's program with a frame's creations added, compiled in process
/// without the kernel: for expansion-level tests of large functions.
fn staged(dir: &Path, frame: &Value) -> (Program, Names) {
    let head = Workspace::at(dir).head().unwrap();
    let mut map = name_map(dir);
    let names = Names::build(head.program(), &map);
    let compiled = sley_agent::frame::compile(
        head.program(),
        &names,
        &sley_agent::candidate::Authority::of(&head)
            .unwrap()
            .ceilings,
        frame,
        sley_id::CandidateNonce::from_bytes([5; 32]),
        &mut || Ok([6; 32]),
    )
    .unwrap();
    map.extend(&compiled.names);
    let mut objects = head.program().objects().to_vec();
    for op in compiled.ops {
        if let sley_mutate::MutationPayload::CreateEntity(body) = op.payload {
            objects.push(
                sley_mutate::build_entity_object(
                    head.program().epoch(),
                    &sley_mutate::EntityObjectRecord {
                        entity_id: op.target,
                        body,
                        label: None,
                        semantic_fingerprint: None,
                    },
                )
                .unwrap(),
            );
        }
    }
    let program = Program::new(
        head.program().epoch(),
        head.program().root(),
        head.program().workspace(),
        objects,
    );
    let names = Names::build(&program, &map);
    (program, names)
}

/// Adds objects to the head's program, for states the workbench cannot
/// author (packages, entry points, effects).
fn with_objects(dir: &Path, extra: Vec<sley_mutate::EntityObject>) -> (Program, Names) {
    let head = Workspace::at(dir).head().unwrap();
    let mut objects = head.program().objects().to_vec();
    objects.extend(extra);
    let program = Program::new(
        head.program().epoch(),
        head.program().root(),
        head.program().workspace(),
        objects,
    );
    let names = Names::build(&program, &name_map(dir));
    (program, names)
}

fn object(
    dir: &Path,
    byte: u8,
    label: &str,
    body: sley_mutate::value::EntityBodyValue,
) -> sley_mutate::EntityObject {
    let head = Workspace::at(dir).head().unwrap();
    sley_mutate::build_entity_object(
        head.program().epoch(),
        &sley_mutate::EntityObjectRecord {
            entity_id: sley_id::EntityId::from_bytes([byte; 32]),
            body,
            label: Some(label.to_owned()),
            semantic_fingerprint: None,
        },
    )
    .unwrap()
}

#[test]
fn preserve_compares_constants_as_the_frame_defines_them() {
    let temp = workspace("frame-constants");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [order_error()],
          "consts": [{"name": "min_q", "type": "i64", "value": 1}, {"name": "one", "type": "i64", "value": 1}],
          "fns": [
            {"fn": "check_quantity", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>",
             "blocks": [{"name": "entry", "ops": [["m", "const", "min_q"], ["!InvalidQuantity", "if", ["lt", "q", "m"]]], "term": ["ok", "q"]}]},
            {"fn": "line_total", "params": [["quantity", "i64"], ["price", "i64"]], "returns": "Result<i64,OrderError>",
             "blocks": [{"name": "entry", "ops": [["!InvalidPrice", "if", ["lt", "price", 0]],
                                                   ["m", "const", "one"],
                                                   ["!InvalidQuantity", "if", ["lt", "quantity", "m"]],
                                                   ["total", "mul?Overflow", "quantity", "price"]],
                         "term": ["ok", "total"]}]}]}),
    );
    // Live, min_q and one are equal: the checks match.
    let guard = json!({"guard": "check_quantity", "arg": "quantity", "in": ["line_total"]});
    valid(&temp.path, &json!({"af1": 1, "afx": 1, "ripple": [guard]}));
    // The frame makes min_q 5: in the candidate the checker tests q < 5,
    // line_total tests quantity < 1. Not the same check.
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "consts": [{"name": "min_q", "type": "i64", "value": 5}], "ripple": [guard]}),
        "AGENT_RIPPLE_GUARD_ORDER",
        &["/ripple/0/in/0: no check in `line_total` is the same as `check_quantity` on `quantity`"],
    );
    // Changing both keeps them equal, and the behavior with them.
    let both = json!({"af1": 1, "afx": 1,
      "consts": [{"name": "min_q", "type": "i64", "value": 2}, {"name": "one", "type": "i64", "value": 2}],
      "ripple": [guard]});
    valid(&temp.path, &both);
    let without = json!({"af1": 1, "afx": 1,
      "consts": [{"name": "min_q", "type": "i64", "value": 2}, {"name": "one", "type": "i64", "value": 2}]});
    let mut expected = Machine::candidate(&temp.path, &without);
    let mut after = Machine::candidate(&temp.path, &both);
    for row in grid(&[&quantities(), &prices()]) {
        assert_eq!(
            after.call("line_total", &row),
            expected.call("line_total", &row),
            "{row:?}"
        );
    }
}

#[test]
fn entry_reads_the_call_graph_as_the_frame_leaves_it() {
    let temp = workspace("frame-calls-graph");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "OrderError", "variant": ["InvalidQuantity", "Overflow"]}],
          "fns": [
            {"fn": "helper", "params": [["n", "i64"]], "returns": "Result<i64,OrderError>",
             "blocks": [{"name": "entry", "ops": [["!InvalidQuantity", "if", ["lt", "n", 1]]], "term": ["ok", "n"]}]},
            {"fn": "check", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>",
             "blocks": [{"name": "entry", "ops": [["v", "call?", "helper", "q"]], "term": ["ok", "v"]}]},
            {"fn": "total", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
             "blocks": [{"name": "entry", "term": ["ok", ["mul?Overflow", "quantity", 2]]}]}]}),
    );
    let guard = json!({"guard": "check", "arg": "quantity", "in": ["total"], "mode": "entry"});
    // Live, check does not reach total: the guard applies.
    valid(&temp.path, &json!({"af1": 1, "afx": 1, "ripple": [guard]}));
    // The frame redefines helper to call total: check would recurse.
    let helper = json!({"fn": "helper", "params": [["n", "i64"]], "returns": "Result<i64,OrderError>",
      "blocks": [{"name": "entry", "ops": [["!InvalidQuantity", "if", ["lt", "n", 1]]], "term": ["cond", ["gt", "n", 100], "big", "small"]},
                 {"name": "big", "term": ["return", ["call", "total", "n"]]},
                 {"name": "small", "term": ["ok", "n"]}]});
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [helper], "ripple": [guard]}),
        "AGENT_RIPPLE_GUARD_ORDER",
        &[
            "/ripple/0/in/0: `check` calls `total` (check -> helper -> total): evaluating it at the entry of `total` would never end",
        ],
    );
}

#[test]
fn entry_refuses_a_function_that_may_already_evaluate_the_checker() {
    let temp = orders_workspace("double");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [check_quantity(),
          {"fn": "helper", "params": [["n", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["v", "call?", "check_quantity", "n"]], "term": ["ok", "v"]}]},
          // Through a block parameter.
          {"fn": "viaparam", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "term": ["br", "next", "quantity"]},
                      {"name": "next", "params": [["q", "i64"]], "ops": [["v", "call?", "check_quantity", "q"]], "term": ["ok", "v"]}]},
          // Through a helper.
          {"fn": "viahelper", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["v", "call?", "helper", "quantity"]], "term": ["ok", "v"]}]}]}),
    );
    let obligations = assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [
          {"guard": "check_quantity", "arg": "quantity", "in": ["viaparam", "viahelper"], "mode": "entry"}]}),
        "AGENT_RIPPLE_GUARD_ORDER",
        &[
            "/ripple/0/in/0: `viaparam` already evaluates `check_quantity` at `viaparam.next.v__r`; evaluating it again at entry could run it twice",
            "/ripple/0/in/1: `viahelper` already evaluates `check_quantity` at `viahelper.entry.v__r`, through helper -> check_quantity",
        ],
    );
    assert_eq!(obligations.len(), 2);
    // A helper the frame redefines to call the checker counts too.
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [
          {"fn": "plain", "params": [["n", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "term": ["ok", "n"]}]},
          {"fn": "viaplain", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["v", "call?", "plain", "quantity"]], "term": ["ok", "v"]}]}]}),
    );
    let guard =
        json!({"guard": "check_quantity", "arg": "quantity", "in": ["viaplain"], "mode": "entry"});
    valid(&temp.path, &json!({"af1": 1, "afx": 1, "ripple": [guard]}));
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [
          {"fn": "plain", "params": [["n", "i64"]], "returns": "Result<i64,OrderError>",
           "blocks": [{"name": "entry", "ops": [["v", "call?", "check_quantity", "n"]], "term": ["ok", "v"]}]}],
          "ripple": [guard]}),
        "AGENT_RIPPLE_GUARD_ORDER",
        &[
            "`viaplain` already evaluates `check_quantity` at `viaplain.entry.v__r`, through plain -> check_quantity",
        ],
    );
}

#[test]
fn equal_count_signatures_never_reinterpret_calls_the_frame_writes() {
    let temp = workspace("equal-count");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [
          {"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,ArithmeticError>",
           "blocks": [{"name": "entry", "term": ["return", ["sub", "a", "b"]]}]},
          {"fn": "g", "params": [["x", "i64"]], "returns": "Result<i64,ArithmeticError>",
           "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x", 1]], "term": ["return", "r"]}]}]}),
    );
    // r1 writes k(x, y) = f(x, y) and a table row against f(a, b).
    let r1 = json!({"af1": 1, "afx": 1, "fns": [
      {"fn": "k", "params": [["x", "i64"], ["y", "i64"]], "returns": "Result<i64,ArithmeticError>",
       "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x", "y"]], "term": ["return", "r"]}]}],
      "test_tables": [{"name": "t_k", "fn": "k", "cases": [{"args": [5, 2], "expect": {"Ok": 3}}]},
                      {"name": "t_f", "fn": "f", "cases": [{"args": [9, 4], "expect": {"Ok": 5}}]}]});
    let report = valid(&temp.path, &r1);
    // Each variant is layered on r1's candidate (a new draft each time).
    let draft = report["handle"].as_str().unwrap().to_owned();
    // r2 reorders f to (b, a): the carried call and row fit both lists.
    let reorder = |frame_calls: Option<&str>| {
        let mut intent = json!({"arity": "f"});
        if let Some(word) = frame_calls {
            intent["frame_calls"] = json!(word);
        }
        json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["b", "i64"], ["a", "i64"]]}], "ripple": [intent]})
    };
    let (status, refusal) = run_json(
        &temp.path,
        &["try", "--on", &draft, &reorder(None).to_string()],
    );
    assert_eq!(status, 2, "{refusal:#}");
    assert_eq!(refusal["error"], "AGENT_RIPPLE_HOLE_UNFILLED");
    let detail = refusal["detail"].as_str().unwrap();
    for needle in [
        "/fns/0/blocks/0/ops/0 (k) passes 2 arguments to `f`, which fits both its old parameters (a: i64, b: i64) and its new ones (b: i64, a: i64): say how this frame's calls and tests of `f` are written with \"frame_calls\"",
        "/test_tables/1/cases/0 (`t_f_0`) passes 2 arguments to `f`",
    ] {
        assert!(detail.contains(needle), "missing `{needle}` in {detail}");
    }
    // Declared old: rewritten by name, like live code; k keeps its meaning.
    let (status, report) = run_json(
        &temp.path,
        &["try", "--on", &draft, &reorder(Some("old")).to_string()],
    );
    assert_eq!(status, 0, "{report:#}");
    let revision = report["draft"].as_str().unwrap().to_owned();
    let expanded = artifact(&temp.path, &revision, "expanded.json");
    assert_eq!(
        expanded["fns"][0]["blocks"][0]["ops"][0],
        json!(["r", "call", "f", "y", "x"])
    );
    let inventory = artifact(&temp.path, &revision, "ripple.json");
    let edits: Vec<&str> = inventory["intents"][0]["calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|call| call["edit"].as_str().unwrap())
        .collect();
    assert_eq!(
        edits,
        ["rewritten", "rewritten", "rewritten"],
        "{inventory:#}"
    );
    let layered = artifact(&temp.path, &revision, "frame.json");
    let mut after = Machine::candidate(&temp.path, &layered);
    assert_eq!(after.call("k", &[json!(5), json!(2)]), json!({"Ok": 3}));
    assert_eq!(after.call("f", &[json!(4), json!(9)]), json!({"Ok": 5}));
    assert_eq!(after.call("g", &[json!(5)]), json!({"Ok": 4}));
    // Declared new: kept as written (the author's decision), so k flips.
    let (status, report) = run_json(
        &temp.path,
        &["try", "--on", &draft, &reorder(Some("new")).to_string()],
    );
    assert_eq!(status, 1, "the old row now fails: {report:#}");
    let layered = artifact(&temp.path, report["draft"].as_str().unwrap(), "frame.json");
    let mut after = Machine::candidate(&temp.path, &layered);
    assert_eq!(after.call("k", &[json!(5), json!(2)]), json!({"Ok": -3}));
    // Declared old for a call that passes another count is a hole.
    let grow = json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"], ["c", "i64"]]}],
      "fns": [{"fn": "k2", "params": [["x", "i64"]], "returns": "Result<i64,ArithmeticError>",
               "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x", 1, 2]], "term": ["return", "r"]}]}],
      "ripple": [{"arity": "f", "value": 0, "frame_calls": "old"}]});
    assert_refused(
        &temp.path,
        &grow,
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &[
            "/fns/0/blocks/0/ops/0 (k2) passes 3 arguments to `f`, but \"frame_calls\": \"old\" says it is written for the old parameters (a: i64, b: i64)",
        ],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"arity": "f", "frame_calls": "both"}]}),
        "AGENT_FRAME_INVALID",
        &[
            "/ripple/0/frame_calls: \"frame_calls\" says how the calls and tests this frame writes are read",
        ],
    );
}

#[test]
fn a_restated_caller_in_another_namespace_is_an_exported_boundary() {
    let temp = workspace("frame-namespace");
    commit(
        &temp.path,
        &json!({"af1": 1, "fns": [
          {"fn": "f", "params": [["a", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
          {"fn": "g", "params": [["x", "i64"]], "returns": "i64",
           "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"]], "term": ["return", "r"]}]}]}),
    );
    commit(
        &temp.path,
        &json!([
          {"class": "CreateEntity", "kind": 3, "key": "ns_a", "payload": {"parent": {"variant": "None"}, "members": ["f"]}},
          {"class": "CreateEntity", "kind": 3, "key": "ns_b", "payload": {"parent": {"variant": "None"}, "members": ["g"]}}]),
    );
    let obligations = assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]]}],
          "fns": [{"fn": "g", "params": [["x", "i64"]], "returns": "i64",
                   "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"], ["unused", "const", {"type": "i64", "value": 3}]],
                               "term": ["return", "r"]}]}],
          "ripple": [{"arity": "f", "value": 0}]}),
        "AGENT_RIPPLE_EXPORTED_BOUNDARY",
        &[
            "/ripple/0: /fns/0/blocks/0/ops/0 (g) calls `f` from namespace ns_b while `f` is in ns_a: ripple does not edit code across a namespace boundary",
        ],
    );
    assert_eq!(obligations.len(), 1);
    // A call written for the new parameters is the author's, not a crossing.
    valid(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]]}],
          "fns": [{"fn": "g", "params": [["x", "i64"]], "returns": "i64",
                   "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x", 5]], "term": ["return", "r"]}]}],
          "ripple": [{"arity": "f", "value": 0}]}),
    );
}

#[test]
fn refused_tries_record_their_ripple_counts_in_the_ledger() {
    let temp = workspace("ledger");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [
          {"fn": "f", "params": [["a", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
          {"fn": "h", "params": [["a", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
          {"fn": "g", "params": [["x", "i64"]], "returns": "(i64,i64)",
           "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"], ["s", "call", "h", "x"]], "term": ["return", ["tuple", "r", "s"]]}]}]}),
    );
    let mut frame = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]]}, {"fn": "h", "params": [["a", "i64"], ["b", "i64"]]}],
      "fns": [{"fn": "k", "params": [["y", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "ops": [["t", "call", "f", "y"]], "term": ["return", "t"]}]}],
      "ripple": [{"arity": "f", "value": 0}, {"arity": "h"}]});
    let last = |dir: &Path| -> Value {
        let text = fs::read_to_string(dir.join(".sley/events.jsonl")).unwrap();
        serde_json::from_str(text.lines().last().unwrap()).unwrap()
    };
    let (status, _) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 2);
    let event = last(&temp.path);
    assert_eq!(event["refusal"], "AGENT_RIPPLE_HOLE_UNFILLED");
    assert_eq!(event["afx"]["ripple_intents"], 2, "{event:#}");
    assert_eq!(event["afx"]["ripple_holes"], 1, "{event:#}");
    assert_eq!(event["afx"]["ripple_edits"], 2, "{event:#}");
    frame["ripple"][1]["value"] = json!(0);
    let (status, _) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0);
    let event = last(&temp.path);
    assert_eq!(event["afx"]["ripple_holes"], 0);
    assert_eq!(event["afx"]["ripple_edits"], 3);
}

#[test]
fn the_checker_and_matcher_bounds_are_limits() {
    let temp = orders_workspace("guard-bounds");
    // A checker with more than 64 blocks, in either mode.
    let mut blocks: Vec<Value> = (0..65)
        .map(|n| json!({"name": format!("b{n}"), "term": ["br", format!("b{}", n + 1)]}))
        .collect();
    blocks.push(json!({"name": "b65", "term": ["ok", "q"]}));
    let big = json!({"fn": "big_check", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>", "blocks": blocks});
    for mode in ["preserve", "entry"] {
        assert_refused(
            &temp.path,
            &guard_frame(
                &big,
                &json!({"guard": "big_check", "arg": "quantity", "in": ["discounted"], "mode": mode}),
            ),
            "AGENT_RIPPLE_LIMIT",
            &["/ripple/0/guard: `big_check` has 66 blocks; a checker has at most 64"],
        );
    }
    // A function whose many copies of the check take more than 1024
    // comparisons to find: a limit, not "no match".
    let chain = |copies: usize| {
        let check_ops = |v: &str| {
            let mut ops = vec![json!(["one", "const", 1]), json!(["t0", "lt", v, "one"])];
            ops.extend((1..39).map(|i| json!([format!("t{i}"), "not", format!("t{}", i - 1)])));
            ops
        };
        let mut blocks: Vec<Value> = (0..copies)
            .map(|i| {
                let next = if i + 1 < copies { format!("b{}", i + 1) } else { "done".to_owned() };
                json!({"name": format!("b{i}"), "ops": check_ops("quantity"), "term": ["cond", "t38", "bad", next]})
            })
            .collect();
        blocks.push(json!({"name": "bad", "ops": [["v", "variant", "OrderError.InvalidQuantity"], ["r", "err", "v"]], "term": ["return", "r"]}));
        blocks.push(
            json!({"name": "done", "ops": [["r", "ok", "quantity"]], "term": ["return", "r"]}),
        );
        let checker = json!({"fn": "chk", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>", "blocks": [
          {"name": "entry", "ops": check_ops("q"), "term": ["cond", "t38", "bad", "good"]},
          {"name": "bad", "ops": [["v", "variant", "OrderError.InvalidQuantity"], ["r", "err", "v"]], "term": ["return", "r"]},
          {"name": "good", "ops": [["r", "ok", "q"]], "term": ["return", "r"]}]});
        (blocks, checker)
    };
    // The long function is staged in process: the bound is the
    // expansion's, and committing 1,200 operations only costs time.
    let (blocks, checker) = chain(30);
    let (program, names) = staged(
        &temp.path,
        &json!({"af1": 1, "fns": [{"fn": "long_check", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>", "blocks": blocks}]}),
    );
    let expansion = sley_agent::afx::expand(
        &program,
        &names,
        &guard_frame(
            &checker,
            &json!({"guard": "chk", "arg": "quantity", "in": ["long_check"]}),
        ),
    )
    .unwrap();
    assert_eq!(
        expansion.obligations.len(),
        1,
        "{:?}",
        expansion.obligations
    );
    assert_eq!(
        expansion.obligations[0].symbol.symbol(),
        "AGENT_RIPPLE_LIMIT"
    );
    assert_eq!(expansion.obligations[0].at, "/ripple/0/in/0");
    assert!(
        expansion.obligations[0].decision.starts_with(
            "comparing `chk` with the checks of `long_check` takes more than 1024 steps"
        ),
        "{}",
        expansion.obligations[0].decision
    );
    // Fewer copies stay within the bound, and every one is replaced.
    let (blocks, checker) = chain(5);
    commit(
        &temp.path,
        &json!({"af1": 1, "fns": [{"fn": "short_check", "params": [["quantity", "i64"]], "returns": "Result<i64,OrderError>", "blocks": blocks}]}),
    );
    let frame = guard_frame(
        &checker,
        &json!({"guard": "chk", "arg": "quantity", "in": ["short_check"]}),
    );
    let report = valid(&temp.path, &frame);
    let inventory = artifact(&temp.path, report["draft"].as_str().unwrap(), "ripple.json");
    assert_eq!(
        inventory["intents"][0]["functions"][0]["checks"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    same_behavior(&temp.path, &frame, "short_check", &grid(&[&quantities()]));
}

#[test]
fn a_constant_holding_the_function_is_unresolved_dispatch() {
    let temp = workspace("function-value");
    commit(
        &temp.path,
        &json!({"af1": 1, "fns": [
          {"fn": "f", "params": [["a", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
          {"fn": "g", "params": [["x", "i64"]], "returns": "i64",
           "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"]], "term": ["return", "r"]}]}]}),
    );
    commit(
        &temp.path,
        &json!([{"class": "CreateEntity", "kind": 9, "key": "fc",
                 "payload": {"value": {"value_type": "fn(i64)->i64",
                                       "data": {"variant": "FunctionRef", "value": {"function": "f", "type_arguments": []}}}}}]),
    );
    let obligations = assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]]}],
                "ripple": [{"arity": "f", "value": 0}]}),
        "AGENT_RIPPLE_HOLE_UNFILLED",
        &[
            "/ripple/0: constant `fc` holds `f` as a function value: calls through it are unresolved dispatch",
        ],
    );
    assert_eq!(obligations.len(), 1);
}

#[test]
fn a_package_export_of_the_namespace_is_an_exported_boundary() {
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, NamespaceBody, PackageBody};
    let temp = arity_workspace("package-namespace");
    let head = Workspace::at(&temp.path).head().unwrap();
    let names = names_of(&temp.path, head.program());
    let f = names.resolve("f").unwrap();
    let set = |ids: Vec<sley_id::EntityId>| EntityIdSet::from_unsorted(ids).unwrap();
    let inner = sley_id::EntityId::from_bytes([0xa1; 32]);
    let outer = sley_id::EntityId::from_bytes([0xa2; 32]);
    let (program, names) = with_objects(
        &temp.path,
        vec![
            object(
                &temp.path,
                0xa1,
                "inner",
                EntityBodyValue::Namespace(NamespaceBody {
                    parent: Some(outer),
                    members: set(vec![f]),
                }),
            ),
            object(
                &temp.path,
                0xa2,
                "outer",
                EntityBodyValue::Namespace(NamespaceBody {
                    parent: None,
                    members: set(vec![inner]),
                }),
            ),
            object(
                &temp.path,
                0xa3,
                "pkg",
                EntityBodyValue::Package(PackageBody {
                    workspace: sley_id::EntityId::from_bytes([0xa4; 32]),
                    root_namespace: outer,
                    dependencies: set(Vec::new()),
                    exports: set(vec![outer]),
                }),
            ),
        ],
    );
    let expansion = sley_agent::afx::expand(
        &program,
        &names,
        &new_f(&json!([{"arity": "f", "value": 0}])),
    )
    .unwrap();
    assert_eq!(
        expansion.obligations.len(),
        1,
        "{:?}",
        expansion.obligations
    );
    assert_eq!(
        expansion.obligations[0].symbol.symbol(),
        "AGENT_RIPPLE_EXPORTED_BOUNDARY"
    );
    assert!(
        expansion.obligations[0]
            .decision
            .starts_with("`f` is exported (in namespace `outer`) by the package `pkg`"),
        "{}",
        expansion.obligations[0].decision
    );
}

// ---------------------------------------------------------------------------
// Follow-ups: committed intents, restated blocks and restated intents
// ---------------------------------------------------------------------------

/// The last line of the events ledger.
fn last_event(dir: &Path) -> Value {
    let text = fs::read_to_string(dir.join(".sley/events.jsonl")).unwrap();
    serde_json::from_str(text.lines().last().unwrap()).unwrap()
}

#[test]
fn a_tests_only_rebase_after_committing_an_intent_derives_nothing_again() {
    // The recommended flow: commit the change first, then add its tests with
    // `try --on dN --rebase`. The layered frame still carries the intent; the
    // head already reflects it, so it is applied and derives nothing.
    let temp = workspace("rebase-arity");
    commit(&temp.path, &help_example(0));
    let arity = help_example(1);
    let (status, text) = run(&temp.path, &["try", &arity.to_string()]);
    assert_eq!(status, 0, "{text}");
    let (status, text) = run(&temp.path, &["commit"]);
    assert_eq!(status, 0, "{text}");
    let tests = json!({"af1": 1, "afx": 1, "test_tables": [{"name": "t_fee", "fn": "line_total",
      "cases": [{"args": [2, 5, 1], "expect": {"Ok": 11}}, {"args": [0, 5, 1], "expect": {"Err": "InvalidQuantity"}}]}]});
    let (status, report) = run_json(
        &temp.path,
        &["try", "--on", "d2", "--rebase", &tests.to_string()],
    );
    assert_eq!(status, 0, "{report:#}");
    let revision = report["draft"].as_str().unwrap();
    assert_eq!(revision, "d2@r2");
    let inventory = artifact(&temp.path, revision, "ripple.json");
    assert_eq!(
        inventory["intents"][0]["edit"], "already applied",
        "{inventory:#}"
    );
    assert_eq!(inventory["edits"], 0);
    assert_eq!(
        report["changed"],
        json!([{"change": "created", "exported": false, "kind": "test", "name": "t_fee_0"},
               {"change": "created", "exported": false, "kind": "test", "name": "t_fee_1"}])
    );
    let (status, text) = run(&temp.path, &["submit", "d2"]);
    assert_eq!(status, 0, "{text}");
    // An entry guard, the same way.
    let temp = workspace("rebase-guard");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "SumError", "variant": ["Negative", "Overflow", "TooBig"]}],
          "fns": [{"fn": "sum_to", "params": [["n", "i64"]], "returns": "Result<i64,SumError>", "blocks": [
            {"name": "entry", "ops": [["!Negative", "if", ["lt", "n", 0]]], "term": ["br", "loop", 0, 1]},
            {"name": "loop", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["more", "le", "i", "n"]], "term": ["cond", "more", "body", "done"]},
            {"name": "body", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["next", "add?Overflow", "acc", "i"]],
             "term": ["br", "loop", "next", ["add?Overflow", "i", 1]]},
            {"name": "done", "params": [["acc", "i64"]], "term": ["ok", "acc"]}]}]}),
    );
    let guard = json!({"af1": 1, "afx": 1,
      "fns": [{"fn": "cap", "params": [["n", "i64"]], "returns": "Result<i64,SumError>",
               "blocks": [{"name": "entry", "ops": [["!TooBig", "if", ["gt", "n", 100]]], "term": ["ok", ["sub?Overflow", "n", 1]]}]}],
      "ripple": [{"guard": "cap", "arg": "n", "in": ["sum_to"], "mode": "entry"}]});
    let (status, text) = run(&temp.path, &["try", &guard.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    let tests = json!({"af1": 1, "afx": 1, "test_tables": [{"name": "t_sum", "fn": "sum_to",
      "cases": [{"args": [3], "expect": {"Ok": 3}}, {"args": [500], "expect": {"Err": "TooBig"}}]}]});
    let (status, report) = run_json(
        &temp.path,
        &["try", "--on", "d2", "--rebase", &tests.to_string()],
    );
    assert_eq!(status, 0, "{report:#}");
    let inventory = artifact(&temp.path, report["draft"].as_str().unwrap(), "ripple.json");
    assert_eq!(
        inventory["intents"][0]["functions"],
        json!([{"fn": "sum_to", "edit": "already applied", "site": "sum_to.n__guard.n__r"}])
    );
    assert_eq!(last_event(&temp.path)["afx"]["ripple_edits"], 0);
    // A committed preserve guard, too.
    let temp = workspace("rebase-preserve");
    commit(&temp.path, &help_example(0));
    let (status, text) = run(&temp.path, &["try", &help_example(2).to_string()]);
    assert_eq!(status, 0, "{text}");
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    let (status, report) = run_json(
        &temp.path,
        &["try", "--on", "d2", "--rebase", &tests_for_line_total()],
    );
    assert_eq!(status, 0, "{report:#}");
    let inventory = artifact(&temp.path, report["draft"].as_str().unwrap(), "ripple.json");
    assert_eq!(
        inventory["intents"][0]["functions"][0]["edit"],
        "already applied"
    );
}

/// The JSON examples of the help's ripple section: the live functions, the
/// arity frame and the guard frame (without their test tables).
fn help_example(index: usize) -> Value {
    let examples: Vec<Value> = sley_agent::help::AFX
        .split("```json\n")
        .skip(1)
        .map(|rest| serde_json::from_str(rest.split("```").next().unwrap()).unwrap())
        .collect();
    let live = examples
        .iter()
        .position(|example: &Value| example.get("test_tables").is_none())
        .unwrap();
    let mut frame = examples[live + index].clone();
    if index > 0 {
        frame.as_object_mut().unwrap().remove("test_tables");
    }
    frame
}

fn tests_for_line_total() -> String {
    json!({"af1": 1, "afx": 1, "test_tables": [{"name": "t_line", "fn": "line_total",
      "cases": [{"args": [0, -1], "expect": {"Err": "InvalidPrice"}}, {"args": [2, 5], "expect": {"Ok": 10}}]}]})
    .to_string()
}

#[test]
fn restating_a_block_after_a_committed_guard_leaves_no_orphans() {
    let temp = workspace("guard-restate");
    commit(&temp.path, &help_example(0));
    let (status, text) = run(&temp.path, &["try", &help_example(2).to_string()]);
    assert_eq!(status, 0, "{text}");
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    // The guard left `entry__if0` switching on its call, `Ok -> entry__if1`
    // without the payload. Restating `entry` replaces its pieces through
    // that edge too.
    let restate = json!({"af1": 1, "afx": 1, "patch": [{"fn": "line_total", "blocks": {"entry": {
      "ops": [["!InvalidPrice", "if", ["lt", "price", 0]],
              ["q", "call?", "check_quantity", "quantity"],
              ["total", "mul?Overflow", "q", "price"]],
      "term": ["ok", "total"]}}}],
      "test_tables": [{"name": "t_restated", "fn": "line_total",
        "cases": [{"args": [0, -1], "expect": {"Err": "InvalidPrice"}},
                  {"args": [0, 5], "expect": {"Err": "InvalidQuantity"}},
                  {"args": [2, 5], "expect": {"Ok": 10}}]}]});
    let report = valid(&temp.path, &restate);
    let expanded = artifact(
        &temp.path,
        report["draft"].as_str().unwrap(),
        "expanded.json",
    );
    let blocks = expanded["patch"][0]["blocks"].as_object().unwrap();
    assert_eq!(blocks["entry__if1"], Value::Null, "{blocks:#?}");
    assert!(
        blocks.keys().all(|leaf| !leaf.ends_with("_2")),
        "{blocks:#?}"
    );
}

#[test]
fn a_follow_up_intent_replaces_the_same_intent() {
    let temp = workspace("relayer");
    commit(&temp.path, &help_example(0));
    let (status, text) = run(&temp.path, &["try", &help_example(1).to_string()]);
    assert_eq!(status, 0, "{text}");
    // Restating the intent with another value replaces it.
    let (status, report) = run_json(
        &temp.path,
        &[
            "try",
            "--on",
            "d2",
            &json!({"af1": 1, "afx": 1, "ripple": [{"arity": "line_total", "value": 5}]})
                .to_string(),
        ],
    );
    assert_eq!(status, 0, "{report:#}");
    let revision = report["draft"].as_str().unwrap();
    let frame = artifact(&temp.path, revision, "frame.json");
    assert_eq!(
        frame["ripple"],
        json!([{"arity": "line_total", "value": 5}])
    );
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(
        after.call("order_total", &[json!(2), json!(5)]),
        json!({"Ok": 15})
    );
    // A hole's hint says how to drop an intent.
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            "--on",
            "d2",
            &json!({"af1": 1, "afx": 1, "ripple": [{"arity": "line_total", "value": "five"}]})
                .to_string(),
        ],
    );
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("to drop an intent instead, set \"/ripple\" to the intents to keep (a follow-up's intent replaces the same intent)"),
        "{text}"
    );
}

// ---------------------------------------------------------------------------
// Applied intents compare their whole effect; guard order; provenance
// ---------------------------------------------------------------------------

/// Commits `base`, then tries `change` and commits it: the draft of the
/// change is `d2`.
fn committed_change(label: &str, base: &Value, change: &Value) -> TempDir {
    let temp = workspace(label);
    commit(&temp.path, base);
    let (status, text) = run(&temp.path, &["try", &change.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    temp
}

/// The Valid candidate `try --on d2 --rebase` makes of `follow_up`, and its
/// ripple inventory.
fn rebased(dir: &Path, follow_up: &Value) -> (Machine, Value) {
    let (status, report) = run_json(
        dir,
        &["try", "--on", "d2", "--rebase", &follow_up.to_string()],
    );
    assert_eq!(report["state"], "valid", "{report:#}");
    assert!(status <= 1, "{report:#}");
    let revision = report["draft"].as_str().unwrap();
    let frame = artifact(dir, revision, "frame.json");
    (
        Machine::candidate(dir, &frame),
        artifact(dir, revision, "ripple.json"),
    )
}

#[test]
fn a_tests_only_rebase_keeps_what_the_committed_intent_derived() {
    // An equal-count reorder whose frame wrote k against the old order.
    let temp = committed_change(
        "rebase-reorder",
        &json!({"af1": 1, "afx": 1, "fns": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,ArithmeticError>",
          "blocks": [{"name": "entry", "term": ["return", ["sub", "a", "b"]]}]}]}),
        &json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["b", "i64"], ["a", "i64"]]}],
          "fns": [{"fn": "k", "params": [["x", "i64"], ["y", "i64"]], "returns": "Result<i64,ArithmeticError>",
                   "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x", "y"]], "term": ["return", "r"]}]}],
          "ripple": [{"arity": "f", "frame_calls": "old"}]}),
    );
    let mut head = Machine::head(&temp.path);
    assert_eq!(head.call("k", &[json!(5), json!(2)]), json!({"Ok": 3}));
    let tests = json!({"af1": 1, "afx": 1, "tests": [{"name": "tf", "fn": "f", "args": [2, 5], "expect": {"Ok": 3}},
                                                   {"name": "tk", "fn": "k", "args": [5, 2], "expect": {"Ok": 3}}]});
    let (mut after, inventory) = rebased(&temp.path, &tests);
    assert_eq!(after.call("k", &[json!(5), json!(2)]), json!({"Ok": 3}));
    assert_eq!(
        inventory["intents"][0]["calls"],
        json!([{"site": "/fns/0/blocks/0/ops/0 (k)", "origin": "frame", "edit": "as committed"}])
    );
    // Without "old", the carried call is not known to be either: a hole,
    // never a flip.
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            "--on",
            "d2",
            "--rebase",
            &json!({"af1": 1, "afx": 1, "ripple": [{"arity": "f"}]}).to_string(),
        ],
    );
    assert_eq!(status, 2, "{text}");
    assert!(text.contains("/fns/0/blocks/0/ops/0 (k) passes 2 arguments to `f`, which fits its parameters (b: i64, a: i64), but it differs from the committed call"), "{text}");
    // A new parameter with a value, and a frame call with the old count.
    let temp = committed_change(
        "rebase-value",
        &json!({"af1": 1, "fns": [
          {"fn": "f", "params": [["a", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
          {"fn": "g", "params": [["x", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"]], "term": ["return", "r"]}]}]}),
        &json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "blocks": {"entry": {"term": ["return", "b"]}}}],
          "fns": [{"fn": "k", "params": [["y", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "ops": [["r", "call", "f", "y"]], "term": ["return", "r"]}]}],
          "ripple": [{"arity": "f", "value": 7}]}),
    );
    let (mut after, inventory) = rebased(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "tests": [{"name": "tk", "fn": "k", "args": [1], "expect": 7}]}),
    );
    assert_eq!(after.call("k", &[json!(1)]), json!(7));
    assert_eq!(after.call("g", &[json!(1)]), json!(7));
    assert_eq!(inventory["intents"][0]["calls"][0]["edit"], "as committed");
}

#[test]
fn a_changed_value_after_the_commit_reaches_the_calls() {
    let base = json!({"af1": 1, "fns": [
      {"fn": "f", "params": [["a", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
      {"fn": "g", "params": [["x", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"]], "term": ["return", "r"]}]}]});
    let change = json!({"af1": 1, "afx": 1,
      "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "blocks": {"entry": {"term": ["return", "b"]}}}],
      "ripple": [{"arity": "f", "value": 0}]});
    let follow_up = |value: i64| {
        json!({"af1": 1, "afx": 1, "ripple": [{"arity": "f", "value": value}],
               "tests": [{"name": "tf", "fn": "f", "args": [1, 2], "expect": 2}]})
    };
    // Before the commit, a follow-up's value replaces the draft's.
    let temp = workspace("value-before");
    commit(&temp.path, &base);
    let (status, text) = run(&temp.path, &["try", &change.to_string()]);
    assert_eq!(status, 0, "{text}");
    let (status, report) = run_json(
        &temp.path,
        &["try", "--on", "d2", &follow_up(5).to_string()],
    );
    assert_eq!(status, 0, "{report:#}");
    let frame = artifact(&temp.path, report["draft"].as_str().unwrap(), "frame.json");
    assert_eq!(
        Machine::candidate(&temp.path, &frame).call("g", &[json!(3)]),
        json!(5)
    );
    // After it, the same: the committed fill loads the new value.
    let temp = committed_change("value-after", &base, &change);
    let (mut after, inventory) = rebased(&temp.path, &follow_up(5));
    assert_eq!(after.call("g", &[json!(3)]), json!(5));
    assert_eq!(inventory["intents"][0]["edit"], "applied again");
    assert_eq!(
        inventory["intents"][0]["calls"],
        json!([{"site": "g.entry.r", "origin": "live", "edit": "value"}])
    );
    // The value it already loads: nothing to derive.
    let (_, inventory) = rebased(&temp.path, &follow_up(0));
    assert_eq!(inventory["intents"][0]["edit"], "already applied");
    // A value that does not fit the parameter is refused as before.
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            "--on",
            "d2",
            "--rebase",
            &json!({"af1": 1, "afx": 1, "ripple": [{"arity": "f", "value": 2.5}]}).to_string(),
        ],
    );
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("/ripple/0/value: 2.5 is not a literal of new parameter `b`'s type"),
        "{text}"
    );
}

#[test]
fn entry_guards_run_in_written_order() {
    let temp = workspace("guard-order");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["BadP", "BadQ"]}],
          "fns": [{"fn": "f", "params": [["p", "i64"], ["q", "i64"]], "returns": "Result<i64,E>",
                   "blocks": [{"name": "entry", "term": ["ok", ["add?BadP", "p", "q"]]}]}]}),
    );
    let checker = |name: &str, case: &str, step: i64| {
        json!({"fn": name, "params": [["x", "i64"]], "returns": "Result<i64,E>",
               "blocks": [{"name": "entry", "ops": [[format!("!{case}"), "if", ["lt", "x", 0]]], "term": ["ok", ["add?BadP", "x", step]]}]})
    };
    let frame = json!({"af1": 1, "afx": 1, "fns": [checker("cp", "BadP", 10), checker("cq", "BadQ", 100), checker("cs", "BadQ", 1000)],
      "ripple": [{"guard": "cp", "arg": "p", "in": ["f"], "mode": "entry"},
                 {"guard": "cq", "arg": "q", "in": ["f"], "mode": "entry"},
                 {"guard": "cs", "arg": "p", "in": ["f"], "mode": "entry"}]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    let mut after = Machine::candidate(&temp.path, &frame);
    // cp first, then cq on q, then cs on the value cp left for p.
    assert_eq!(
        after.call("f", &[json!(-1), json!(-1)]),
        json!({"Err": "BadP"})
    );
    assert_eq!(
        after.call("f", &[json!(1), json!(-1)]),
        json!({"Err": "BadQ"})
    );
    assert_eq!(after.call("f", &[json!(1), json!(2)]), json!({"Ok": 1113}));
    // Committed, the chain is recognized whole: a tests-only rebase derives
    // nothing.
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    let (_, inventory) = rebased(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "tests": [{"name": "tf", "fn": "f", "args": [-1, -1], "expect": {"Err": "BadP"}}]}),
    );
    let edits: Vec<&Value> = inventory["intents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|intent| &intent["functions"][0]["edit"])
        .collect();
    assert_eq!(edits, [&json!("already applied"); 3], "{inventory:#}");
}

#[test]
fn live_tests_an_intent_restates_stay_provided() {
    let temp = workspace("provenance");
    commit(
        &temp.path,
        &json!({"af1": 1, "fns": [
          {"fn": "f", "params": [["a", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "term": ["return", "a"]}]},
          {"fn": "g", "params": [["x", "i64"]], "returns": "i64", "blocks": [{"name": "entry", "ops": [["r", "call", "f", "x"]], "term": ["return", "r"]}]}]}),
    );
    commit(
        &temp.path,
        &json!({"af1": 1, "tests": [{"name": "tf", "fn": "f", "args": [3], "expect": 3}, {"name": "tg", "fn": "g", "args": [4], "expect": 4}]}),
    );
    let (status, report) = run_json(
        &temp.path,
        &["try", &json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]]}],
          "ripple": [{"arity": "f", "value": 0}], "tests": [{"name": "t_new", "fn": "f", "args": [1, 2], "expect": 1}]}).to_string()],
    );
    assert_eq!(status, 0, "{report:#}");
    assert_eq!(
        report["provenance"],
        json!({"authored": 1, "imported": 0, "provided": 2})
    );
    assert_eq!(report["tests"].as_array().unwrap().len(), 3);
}

#[test]
fn frame_calls_old_covers_only_what_was_written_with_the_intent() {
    let temp = workspace("provenance-calls");
    commit(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,ArithmeticError>",
          "blocks": [{"name": "entry", "term": ["return", ["sub", "a", "b"]]}]}]}),
    );
    let caller = |name: &str, args: [&str; 2]| {
        json!({"fn": name, "params": [["x", "i64"], ["y", "i64"]], "returns": "Result<i64,ArithmeticError>",
               "blocks": [{"name": "entry", "ops": [["r", "call", "f", args[0], args[1]]], "term": ["return", "r"]}]})
    };
    // r1: the reorder, and k written for the old order.
    let r1 = json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["b", "i64"], ["a", "i64"]]}],
      "fns": [caller("k", ["x", "y"])], "ripple": [{"arity": "f", "frame_calls": "old"}]});
    let (status, text) = run(&temp.path, &["try", &r1.to_string()]);
    assert_eq!(status, 0, "{text}");
    // r2 adds m, written for the new order (as `view --after` shows it):
    // "old" covers only what r1 stated, and the count fits both orders, so
    // m's call is a hole, never flipped.
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            "--on",
            "d2",
            &json!({"af1": 1, "afx": 1, "fns": [caller("m", ["y", "x"])]}).to_string(),
        ],
    );
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("/fns/1/blocks/0/ops/0 (m) passes 2 arguments to `f`, which fits both its old parameters (a: i64, b: i64) and its new ones (b: i64, a: i64), and it was stated in a revision after the intent, which \"frame_calls\" does not cover"),
        "{text}"
    );
    let frame = artifact(&temp.path, "d2@r2", "frame.json");
    assert_eq!(
        frame["ripple"],
        json!([{"arity": "f", "frame_calls": "old", "after": ["m"]}])
    );
    // A count that decides needs no declaration: a call added later with
    // the old count is rewritten.
    let grow = json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"], ["c", "i64"]]}],
      "ripple": [{"arity": "f", "value": 0}]});
    let temp2 = workspace("provenance-count");
    commit(
        &temp2.path,
        &json!({"af1": 1, "afx": 1, "fns": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,ArithmeticError>",
          "blocks": [{"name": "entry", "term": ["return", ["sub", "a", "b"]]}]}]}),
    );
    assert_eq!(run(&temp2.path, &["try", &grow.to_string()]).0, 0);
    let (status, report) = run_json(
        &temp2.path,
        &[
            "try",
            "--on",
            "d2",
            &json!({"af1": 1, "afx": 1, "fns": [caller("m", ["x", "y"])]}).to_string(),
        ],
    );
    assert_eq!(status, 0, "{report:#}");
    let frame = artifact(&temp2.path, report["draft"].as_str().unwrap(), "frame.json");
    assert_eq!(
        Machine::candidate(&temp2.path, &frame).call("m", &[json!(5), json!(2)]),
        json!({"Ok": 3})
    );
    // Restating the intent with "frame_calls" in a revision covers what the
    // frame states then: with k written for the new order too, "new".
    let (status, report) = run_json(
        &temp.path,
        &[
            "try",
            "--on",
            "d2",
            &json!({"af1": 1, "afx": 1, "fns": [caller("k", ["y", "x"])],
                                       "ripple": [{"arity": "f", "frame_calls": "new"}]})
            .to_string(),
        ],
    );
    assert_eq!(status, 0, "{report:#}");
    let frame = artifact(&temp.path, report["draft"].as_str().unwrap(), "frame.json");
    assert_eq!(
        frame["ripple"],
        json!([{"arity": "f", "frame_calls": "new"}])
    );
    let mut after = Machine::candidate(&temp.path, &frame);
    assert_eq!(after.call("k", &[json!(5), json!(2)]), json!({"Ok": 3}));
    assert_eq!(after.call("m", &[json!(5), json!(2)]), json!({"Ok": 3}));
}
