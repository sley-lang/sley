//! AF1-X: the authoring dialect (`"afx": 1`) and compact test tables.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sley_agent::genesis;
use sley_agent::names::{NameMap, Names};
use sley_agent::workspace::{NAMES_FILE, Workspace};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "sley-agent-afx-{label}-{}-{}",
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

/// FNV-1a, 64 bits: a small stable digest for pinned outputs.
fn fnv(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Compiles `frame` in-process against the workspace head with a fixed
/// nonce and member randomness; the digest of everything compiled.
fn compile_digest(dir: &Path, frame: &Value) -> String {
    let workspace = Workspace::at(dir);
    let head = workspace.head().unwrap();
    let map = NameMap::read(&dir.join(NAMES_FILE)).unwrap();
    let names = Names::build(head.program(), &map);
    let authority = sley_agent::candidate::Authority::of(&head).unwrap();
    let mut counter = 0_u8;
    let mut random = || {
        counter = counter.wrapping_add(1);
        Ok([counter; 32])
    };
    let result = sley_agent::frame::compile(
        head.program(),
        &names,
        &authority.ceilings,
        frame,
        sley_id::CandidateNonce::from_bytes([9; 32]),
        &mut random,
    );
    match result {
        Ok(compiled) => fnv(&format!(
            "{:?}|{:?}|{}|{}|{}|{:?}|{:?}|{:?}",
            compiled.ops,
            compiled.names,
            compiled.created,
            compiled.replaced,
            compiled.deleted,
            compiled.functions,
            compiled.tests,
            compiled.notes
        )),
        Err(error) => format!("refused {error}"),
    }
}

fn guide_example() -> Value {
    let example = sley_agent::help::GUIDE
        .split("```json\n")
        .nth(1)
        .and_then(|rest| rest.split("```").next())
        .unwrap();
    serde_json::from_str(example).unwrap()
}

fn clamp_frame() -> Value {
    json!({"af1": 1,
      "types": [{"name": "RangeError", "variant": ["Inverted"]}],
      "fns": [{"fn": "bound", "params": [["value", "i64"], ["low", "i64"], ["high", "i64"]],
               "returns": "Result<i64,RangeError>",
               "blocks": [
        {"name": "entry", "ops": [["inverted", "lt", "high", "low"]], "term": ["cond", "inverted", "invalid", "check_low"]},
        {"name": "invalid", "ops": [["e", "variant", "RangeError.Inverted"], ["r", "err", "e"]], "term": ["return", "r"]},
        {"name": "check_low", "ops": [["is_below", "lt", "value", "low"]], "term": ["cond", "is_below", "below", "check_high"]},
        {"name": "below", "ops": [["r", "ok", "low"]], "term": ["return", "r"]},
        {"name": "check_high", "ops": [["is_above", "gt", "value", "high"]], "term": ["cond", "is_above", "above", "inside"]},
        {"name": "above", "ops": [["r", "ok", "high"]], "term": ["return", "r"]},
        {"name": "inside", "ops": [["r", "ok", "value"]], "term": ["return", "r"]}]}],
      "tests": [{"fn": "bound", "args": [5, 0, 10], "expect": {"Ok": 5}}]})
}

fn shapes_frame() -> Value {
    json!({"af1": 1,
      "types": [{"name": "Shape", "variant": ["Empty", ["Circle", "i64"], ["Rect", "(i64,i64)"]]},
                {"name": "Point", "record": [["x", "i64"], ["y", "i64"]]}],
      "consts": [{"name": "limit", "type": "i64", "value": 10000}],
      "fns": [{"fn": "area", "params": [["s", "Shape"]], "returns": "Option<i64>", "blocks": [
          {"name": "entry", "term": ["switch", "s", ["Empty", "none_"], ["Circle", "circle", "$"], ["Rect", ["rect", "$"]]]},
          {"name": "none_", "ops": [{"name": "n", "op": "none", "type": "Option<i64>"}], "term": ["return", "n"]},
          {"name": "circle", "params": [["r", "i64"]], "ops": [["cap", "const", "limit"], ["ok_r", "lt", "r", "cap"]],
           "term": ["cond", "ok_r", ["small", "r"], "none_"]},
          {"name": "small", "params": [["v", "i64"]], "ops": [["x", "some", "v"]], "term": ["return", "x"]},
          {"name": "rect", "params": [["pair", "(i64,i64)"]], "ops": [["w", "tuple_get", 0, "pair"]], "term": ["br", "small", "w"]}]},
        {"fn": "origin", "params": [], "returns": "Point", "blocks": [
          {"name": "entry", "ops": [["z", "const", 0], ["p", "record", "Point", "z", "z"]], "term": ["return", "p"]}]}],
      "tests": [{"name": "t_empty", "fn": "area", "args": ["Empty"], "expect": "None"}]})
}

#[test]
fn plain_af1_frames_compile_exactly_as_before() {
    // Digests of the compiled output of representative plain AF1 frames,
    // taken before the authoring dialect existed: a frame without "afx"
    // takes the unchanged path, and AF1-X syntax without it is refused as
    // before.
    let temp = workspace("pin");
    let cases: Vec<(&str, Value)> = vec![
        ("guide", guide_example()),
        ("clamp", clamp_frame()),
        ("shapes", shapes_frame()),
        (
            "nested-refused",
            json!({"af1": 1, "fns": [{"fn": "f", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
                "blocks": [{"name": "entry", "ops": [["x", "add", "a", ["mul", "a", "a"]]], "term": ["return", "x"]}]}]}),
        ),
        (
            "literal-refused",
            json!({"af1": 1, "fns": [{"fn": "f", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
                "blocks": [{"name": "entry", "ops": [["x", "add", "a", 1]], "term": ["return", "x"]}]}]}),
        ),
        (
            "tables-refused",
            json!({"af1": 1, "test_tables": [{"name": "t", "fn": "f", "cases": []}]}),
        ),
        (
            "afx-key-refused-in-plain-order",
            json!({"af1": 2, "afx": 1}),
        ),
    ];
    let mut digests = Vec::new();
    for (label, frame) in &cases {
        digests.push(format!("{label}={}", compile_digest(&temp.path, frame)));
    }
    let expected = [
        "guide=38dfec510d3d96ba",
        "clamp=b16e8a51d88e241b",
        "shapes=334a74eefde9ef3a",
        "nested-refused=refused AGENT_FRAME_INVALID: /fns/0/blocks/0/ops/0: operations do not nest; add the operation to the block's ops under a name and use that name",
        "literal-refused=refused AGENT_FRAME_INVALID: /fns/0/blocks/0/ops/0: the literal 1 is not a value name; add an operation such as [\"k\", \"const\", 1] and use \"k\"",
        "tables-refused=refused AGENT_FRAME_INVALID: /test_tables: unknown frame key",
        "afx-key-refused-in-plain-order=refused AGENT_FRAME_INVALID: /af1: declare \"af1\": 1",
    ];
    assert_eq!(digests, expected, "{digests:#?}");
}

#[test]
fn valid_plain_af1_keeps_its_meaning_under_the_dialect() {
    // A valid plain AF1 frame compiles to the same candidate content with
    // "afx": 1: the dialect only adds forms.
    let temp = workspace("preserve");
    for frame in [guide_example(), clamp_frame(), shapes_frame()] {
        let plain = compile_digest(&temp.path, &frame);
        let mut extended = frame.clone();
        extended["afx"] = json!(1);
        assert_eq!(compile_digest(&temp.path, &extended), plain, "{frame}");
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn names_of(dir: &Path, program: &sley_agent::workspace::Program) -> Names {
    let mut map = NameMap::read(&dir.join(NAMES_FILE)).unwrap();
    map.extend(&NameMap::read(&dir.join(".sley").join(NAMES_FILE)).unwrap());
    Names::build(program, &map)
}

/// The expansion of `frame` against the workspace head.
fn expand(dir: &Path, frame: &Value) -> sley_agent::afx::Expansion {
    let head = Workspace::at(dir).head().unwrap();
    let names = names_of(dir, head.program());
    sley_agent::afx::expand(head.program(), &names, frame).unwrap()
}

/// A frame's refusal: (symbol, detail).
fn refused(dir: &Path, frame: &Value) -> (String, String) {
    let (status, value) = run_json(dir, &["try", &frame.to_string()]);
    assert_eq!(status, 2, "expected a refusal: {value}");
    (
        value["error"].as_str().unwrap().to_owned(),
        value["detail"].as_str().unwrap().to_owned(),
    )
}

/// A Valid candidate of `frame`, compiled and validated in process, with
/// an executor over the state it proposes.
struct Runner {
    executor: sley_agent::exec::Executor,
    program: sley_agent::workspace::Program,
    names: Names,
}

impl Runner {
    fn new(dir: &Path, frame: &Value) -> Self {
        let head = Workspace::at(dir).head().unwrap();
        let mut map = NameMap::read(&dir.join(NAMES_FILE)).unwrap();
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

const EDGES: [i64; 8] = [i64::MIN, -3, -1, 0, 1, 2, 7, i64::MAX];

fn triples() -> Vec<Vec<Value>> {
    let mut out = Vec::new();
    for a in EDGES {
        for b in EDGES {
            for c in [i64::MIN, -1, 0, 1, 7, i64::MAX] {
                out.push(vec![json!(a), json!(b), json!(c)]);
            }
        }
    }
    out
}

fn pairs() -> Vec<Vec<Value>> {
    let mut out = Vec::new();
    for a in EDGES {
        for b in EDGES {
            out.push(vec![json!(a), json!(b)]);
        }
    }
    out
}

fn singles() -> Vec<Vec<Value>> {
    EDGES.iter().map(|a| vec![json!(a)]).collect()
}

fn indexes() -> Vec<Vec<Value>> {
    [0_u64, 1, 2, 3, 100, u64::MAX]
        .iter()
        .map(|i| vec![json!(i)])
        .collect()
}

// ---------------------------------------------------------------------------
// Each construct, and its explicit AF1 equivalent
// ---------------------------------------------------------------------------

fn error_type() -> Value {
    json!({"name": "E", "variant": ["Ov", "Un", "Dz", "A", "B", "C", ["Arith", "ArithmeticError"], ["Code", "i64"]]})
}

fn params(names: &[&str], ty: &str) -> Value {
    Value::Array(names.iter().map(|name| json!([name, ty])).collect())
}

/// The dialect side: every construct in one frame.
fn constructs_afx() -> Value {
    let abc = params(&["a", "b", "c"], "i64");
    let ab = params(&["a", "b"], "i64");
    json!({"af1": 1, "afx": 1, "types": [error_type()], "fns": [
      {"fn": "arith", "params": abc, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["r", "mul?Ov", ["add?Ov", "a", "b"], ["sub?Un", "c", 1]]], "term": ["ok", "r"]}]},
      {"fn": "first", "params": abc, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["r", "add?A", ["mul?B", "a", "b"], ["div?C", "a", "c"]]], "term": ["ok", "r"]}]},
      {"fn": "payload", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["q", "div?Arith", "a", "b"]], "term": ["ok", "q"]}]},
      {"fn": "bare", "params": abc, "returns": "Result<i64,ArithmeticError>", "blocks": [
        {"name": "entry", "term": ["ok", ["add?", ["mul?", "a", "b"], "c"]]}]},
      {"fn": "safe_div", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["!Dz", "if", ["eq", "b", 0]], ["q", "div?Ov", "a", "b"]], "term": ["ok", "q"]}]},
      {"fn": "use_call", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["q", "call?", "safe_div", "a", "b"], ["r", "add?Ov", "q", "a"]], "term": ["ok", "r"]}]},
      {"fn": "recover", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["k", "mul?Ov", "a", 2], ["q", "call?onerr", "safe_div", "k", "b"]], "term": ["ok", "q"]},
        {"name": "onerr", "params": [["e", "E"], ["k", "i64"]], "ops": [["r", "sub?Un", "k", 1]], "term": ["ok", "r"]}]},
      {"fn": "pick", "params": [["i", "u64"]], "returns": "Option<i64>", "blocks": [
        {"name": "entry", "ops": [["v", "vec", {"type": "i64", "value": 10}, 20, 30], ["x", "vec_get?", "v", "i"]],
         "term": ["return", ["some", "x"]]}]},
      {"fn": "opt_call", "params": [["i", "u64"]], "returns": "Option<i64>", "blocks": [
        {"name": "entry", "ops": [["x", "call?", "pick", "i"]], "term": ["return", ["some", ["add?ovf", "x", 1]]]},
        {"name": "ovf", "term": ["fail"]}]},
      {"fn": "lookup", "params": [["k", "i64"]], "returns": "Option<i64>", "blocks": [
        {"name": "entry", "ops": [["m", "map?dup", {"type": "i64", "value": 1}, {"type": "i64", "value": 100}, 2, 200], ["y", "map_get?", "m", "k"]],
         "term": ["return", ["some", "y"]]},
        {"name": "dup", "term": ["fail"]}]},
      {"fn": "shadow", "params": [["x", "i64"], ["y", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["k", "lt", "x", "y"]], "term": ["br", "b1"]},
        {"name": "b1", "ops": [["x", "mul?Ov", "y", 3], ["z", "sub?Un", "x", "y"]], "term": ["br", "b2"]},
        {"name": "b2", "params": [["z", "i64"]], "ops": [["w", "add?Ov", "z", "x"]], "term": ["cond", "k", ["pos", "w"], ["neg", "w"]]},
        {"name": "pos", "params": [["v", "i64"]], "term": ["ok", "v"]},
        {"name": "neg", "params": [["v", "i64"]], "term": ["ok", ["neg?Ov", "v"]]}]},
      {"fn": "join", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["c", "lt", "a", "b"]], "term": ["cond", "c", "left", "right"]},
        {"name": "left", "ops": [["t", "sub?Un", "b", "a"]], "term": ["br", "done"]},
        {"name": "right", "ops": [["t", "sub?Un", "a", "b"]], "term": ["br", "done"]},
        {"name": "done", "params": [["t", "i64"]], "ops": [["r", "mul?Ov", "t", 2]], "term": ["ok", "r"]}]},
      {"fn": "sum", "params": [["n", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "term": ["br", "loop", 0, 1]},
        {"name": "loop", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["more", "le", "i", "n"]], "term": ["cond", "more", "body", "done"]},
        {"name": "body", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["acc2", "add?Ov", "acc", "i"], ["i2", "add?Ov", "i", 1]],
         "term": ["br", "loop", "acc2", "i2"]},
        {"name": "done", "params": [["acc", "i64"]], "term": ["ok", "acc"]}]},
      {"fn": "exits", "params": params(&["v", "lo", "hi"], "i64"), "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["!low", "if", ["lt", "v", 0]], ["!Code", "if", ["lt", "v", "lo"], "lo"], ["!B", "if", ["gt", "v", "hi"]]],
         "term": ["ok", "v"]},
        {"name": "low", "params": [["v", "i64"]], "term": ["ok", ["neg?Ov", "v"]]}]},
      {"fn": "failing", "params": [["x", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["c", "lt", "x", 0]], "term": ["cond", "c", "bad", "good"]},
        {"name": "bad", "term": ["fail", "Code", "x"]},
        {"name": "good", "ops": [["z", "eq", "x", 0]], "term": ["cond", "z", "zero", "fine"]},
        {"name": "zero", "term": ["fail", "A"]},
        {"name": "fine", "term": ["ok", "x"]}]},
      {"fn": "thread", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "term": ["br", "work", "a"]},
        {"name": "work", "params": [["p", "i64"]], "ops": [["s", "add?Ov", "p", "b"], ["t", "mul?Ov", "s", "p"], ["u", "sub?Un", "t", "p"]],
         "term": ["ok", "u"]}]}]})
}

fn done_block(name: &str, ty: &str, wrap: &str) -> Value {
    json!({"name": name, "params": [["v", ty]], "ops": [["o", wrap, "v"]], "term": ["return", "o"]})
}

fn fail_block(name: &str, case: &str) -> Value {
    json!({"name": name, "ops": [["e", "variant", format!("E.{case}")], ["o", "err", "e"]], "term": ["return", "o"]})
}

fn none_block() -> Value {
    json!({"name": "none_", "ops": [{"name": "o", "op": "none", "type": "Option<i64>"}], "term": ["return", "o"]})
}

/// The explicit side: the same functions written in plain AF1.
#[allow(clippy::too_many_lines)]
fn constructs_plain() -> Value {
    let abc = params(&["a", "b", "c"], "i64");
    let ab = params(&["a", "b"], "i64");
    let done = done_block("done", "i64", "ok");
    json!({"af1": 1, "types": [error_type()], "fns": [
      {"fn": "arith", "params": abc, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["s", "add", "a", "b"]], "term": ["switch", "s", ["Ok", "e1", "$"], ["Err", "ov"]]},
        {"name": "e1", "params": [["sv", "i64"]], "ops": [["one", "const", 1], ["t", "sub", "c", "one"]],
         "term": ["switch", "t", ["Ok", "e2", "sv", "$"], ["Err", "un"]]},
        {"name": "e2", "params": [["sv", "i64"], ["tv", "i64"]], "ops": [["r", "mul", "sv", "tv"]],
         "term": ["switch", "r", ["Ok", "done", "$"], ["Err", "ov"]]},
        done, fail_block("ov", "Ov"), fail_block("un", "Un")]},
      {"fn": "first", "params": abc, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["m", "mul", "a", "b"]], "term": ["switch", "m", ["Ok", "e1", "$"], ["Err", "fb"]]},
        {"name": "e1", "params": [["mv", "i64"]], "ops": [["d", "div", "a", "c"]], "term": ["switch", "d", ["Ok", "e2", "mv", "$"], ["Err", "fc"]]},
        {"name": "e2", "params": [["mv", "i64"], ["dv", "i64"]], "ops": [["r", "add", "mv", "dv"]],
         "term": ["switch", "r", ["Ok", "done", "$"], ["Err", "fa"]]},
        done, fail_block("fa", "A"), fail_block("fb", "B"), fail_block("fc", "C")]},
      {"fn": "payload", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["q", "div", "a", "b"]], "term": ["switch", "q", ["Ok", "done", "$"], ["Err", "arith", "$"]]},
        done,
        {"name": "arith", "params": [["p", "ArithmeticError"]], "ops": [["e", "variant", "E.Arith", "p"], ["o", "err", "e"]], "term": ["return", "o"]}]},
      {"fn": "bare", "params": abc, "returns": "Result<i64,ArithmeticError>", "blocks": [
        {"name": "entry", "ops": [["m", "mul", "a", "b"]], "term": ["switch", "m", ["Ok", "e1", "$"], ["Err", "fail", "$"]]},
        {"name": "e1", "params": [["mv", "i64"]], "ops": [["s", "add", "mv", "c"]], "term": ["switch", "s", ["Ok", "done", "$"], ["Err", "fail", "$"]]},
        done,
        {"name": "fail", "params": [["e", "ArithmeticError"]], "ops": [["o", "err", "e"]], "term": ["return", "o"]}]},
      {"fn": "safe_div", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["zero", "const", 0], ["z", "eq", "b", "zero"]], "term": ["cond", "z", "dz", "go"]},
        fail_block("dz", "Dz"),
        {"name": "go", "ops": [["q", "div", "a", "b"]], "term": ["switch", "q", ["Ok", "done", "$"], ["Err", "ov"]]},
        done, fail_block("ov", "Ov")]},
      {"fn": "use_call", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["q", "call", "safe_div", "a", "b"]], "term": ["switch", "q", ["Ok", "e1", "$"], ["Err", "fail", "$"]]},
        {"name": "e1", "params": [["qv", "i64"]], "ops": [["r", "add", "qv", "a"]], "term": ["switch", "r", ["Ok", "done", "$"], ["Err", "ov"]]},
        done, fail_block("ov", "Ov"),
        {"name": "fail", "params": [["e", "E"]], "ops": [["o", "err", "e"]], "term": ["return", "o"]}]},
      {"fn": "recover", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["two", "const", 2], ["k", "mul", "a", "two"]], "term": ["switch", "k", ["Ok", "e1", "$"], ["Err", "ov"]]},
        {"name": "e1", "params": [["kv", "i64"]], "ops": [["q", "call", "safe_div", "kv", "b"]],
         "term": ["switch", "q", ["Ok", "done", "$"], ["Err", "onerr", "$", "kv"]]},
        {"name": "onerr", "params": [["e", "E"], ["k", "i64"]], "ops": [["one", "const", 1], ["r", "sub", "k", "one"]],
         "term": ["switch", "r", ["Ok", "done", "$"], ["Err", "un"]]},
        done, fail_block("ov", "Ov"), fail_block("un", "Un")]},
      {"fn": "pick", "params": [["i", "u64"]], "returns": "Option<i64>", "blocks": [
        {"name": "entry", "ops": [["x10", "const", 10], ["x20", "const", 20], ["x30", "const", 30], ["v", "vec", "x10", "x20", "x30"], ["x", "vec_get", "v", "i"]],
         "term": ["switch", "x", ["Some", "done", "$"], ["None", "none_"]]},
        done_block("done", "i64", "some"), none_block()]},
      {"fn": "opt_call", "params": [["i", "u64"]], "returns": "Option<i64>", "blocks": [
        {"name": "entry", "ops": [["x", "call", "pick", "i"]], "term": ["switch", "x", ["Some", "e1", "$"], ["None", "none_"]]},
        {"name": "e1", "params": [["xv", "i64"]], "ops": [["one", "const", 1], ["s", "add", "xv", "one"]],
         "term": ["switch", "s", ["Ok", "done", "$"], ["Err", "none_"]]},
        done_block("done", "i64", "some"), none_block()]},
      {"fn": "lookup", "params": [["k", "i64"]], "returns": "Option<i64>", "blocks": [
        {"name": "entry", "ops": [["k1", "const", 1], ["v1", "const", 100], ["k2", "const", 2], ["v2", "const", 200],
            {"name": "m", "op": "map", "args": ["k1", "v1", "k2", "v2"], "type": "Result<Map<i64,i64>,DuplicateKeyError>"}],
         "term": ["switch", "m", ["Ok", "e1", "$"], ["Err", "none_"]]},
        {"name": "e1", "params": [["mv", "Map<i64,i64>"]], "ops": [["y", "map_get", "mv", "k"]],
         "term": ["switch", "y", ["Some", "done", "$"], ["None", "none_"]]},
        done_block("done", "i64", "some"), none_block()]},
      {"fn": "shadow", "params": [["x", "i64"], ["y", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["k", "lt", "x", "y"]], "term": ["br", "b1"]},
        {"name": "b1", "ops": [["three", "const", 3], ["xr", "mul", "y", "three"]], "term": ["switch", "xr", ["Ok", "b1x", "$"], ["Err", "ov"]]},
        {"name": "b1x", "params": [["x2", "i64"]], "ops": [["zr", "sub", "x2", "y"]], "term": ["switch", "zr", ["Ok", "b2", "$"], ["Err", "un"]]},
        {"name": "b2", "params": [["z", "i64"]], "ops": [["w", "add", "z", "x"]], "term": ["switch", "w", ["Ok", "b2w", "$"], ["Err", "ov"]]},
        {"name": "b2w", "params": [["w2", "i64"]], "term": ["cond", "entry.k", ["pos", "w2"], ["neg", "w2"]]},
        {"name": "pos", "params": [["v", "i64"]], "ops": [["o", "ok", "v"]], "term": ["return", "o"]},
        {"name": "neg", "params": [["v", "i64"]], "ops": [["n", "neg", "v"]], "term": ["switch", "n", ["Ok", "done", "$"], ["Err", "ov"]]},
        done, fail_block("ov", "Ov"), fail_block("un", "Un")]},
      {"fn": "join", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["c", "lt", "a", "b"]], "term": ["cond", "c", "left", "right"]},
        {"name": "left", "ops": [["t", "sub", "b", "a"]], "term": ["switch", "t", ["Ok", "done", "$"], ["Err", "un"]]},
        {"name": "right", "ops": [["t", "sub", "a", "b"]], "term": ["switch", "t", ["Ok", "done", "$"], ["Err", "un"]]},
        {"name": "done", "params": [["t", "i64"]], "ops": [["two", "const", 2], ["r", "mul", "t", "two"]],
         "term": ["switch", "r", ["Ok", "fin", "$"], ["Err", "ov"]]},
        done_block("fin", "i64", "ok"), fail_block("ov", "Ov"), fail_block("un", "Un")]},
      {"fn": "sum", "params": [["n", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["zero", "const", 0], ["one", "const", 1]], "term": ["br", "loop", "zero", "one"]},
        {"name": "loop", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["more", "le", "i", "n"]],
         "term": ["cond", "more", ["body", "acc", "i"], ["done", "acc"]]},
        {"name": "body", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["a2", "add", "acc", "i"]],
         "term": ["switch", "a2", ["Ok", "b2", "$", "i"], ["Err", "ov"]]},
        {"name": "b2", "params": [["acc2", "i64"], ["i", "i64"]], "ops": [["one", "const", 1], ["i2", "add", "i", "one"]],
         "term": ["switch", "i2", ["Ok", "loop", "acc2", "$"], ["Err", "ov"]]},
        done, fail_block("ov", "Ov")]},
      {"fn": "exits", "params": params(&["v", "lo", "hi"], "i64"), "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["zero", "const", 0], ["c1", "lt", "v", "zero"]], "term": ["cond", "c1", ["low", "v"], "e1"]},
        {"name": "e1", "ops": [["c2", "lt", "v", "lo"]], "term": ["cond", "c2", ["code", "lo"], "e2"]},
        {"name": "e2", "ops": [["c3", "gt", "v", "hi"]], "term": ["cond", "c3", "fb", "fine"]},
        {"name": "fine", "ops": [["o", "ok", "v"]], "term": ["return", "o"]},
        {"name": "low", "params": [["v", "i64"]], "ops": [["n", "neg", "v"]], "term": ["switch", "n", ["Ok", "done", "$"], ["Err", "ov"]]},
        {"name": "code", "params": [["p", "i64"]], "ops": [["e", "variant", "E.Code", "p"], ["o", "err", "e"]], "term": ["return", "o"]},
        fail_block("fb", "B"), done, fail_block("ov", "Ov")]},
      {"fn": "failing", "params": [["x", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["zero", "const", 0], ["c", "lt", "x", "zero"]], "term": ["cond", "c", "bad", "good"]},
        {"name": "bad", "ops": [["e", "variant", "E.Code", "x"], ["o", "err", "e"]], "term": ["return", "o"]},
        {"name": "good", "ops": [["zero", "const", 0], ["z", "eq", "x", "zero"]], "term": ["cond", "z", "zero_b", "fine"]},
        fail_block("zero_b", "A"),
        {"name": "fine", "ops": [["o", "ok", "x"]], "term": ["return", "o"]}]},
      {"fn": "thread", "params": ab, "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "term": ["br", "work", "a"]},
        {"name": "work", "params": [["p", "i64"]], "ops": [["s", "add", "p", "b"]], "term": ["switch", "s", ["Ok", "w1", "$", "p"], ["Err", "ov"]]},
        {"name": "w1", "params": [["sv", "i64"], ["p", "i64"]], "ops": [["t", "mul", "sv", "p"]], "term": ["switch", "t", ["Ok", "w2", "$", "p"], ["Err", "ov"]]},
        {"name": "w2", "params": [["tv", "i64"], ["p", "i64"]], "ops": [["u", "sub", "tv", "p"]], "term": ["switch", "u", ["Ok", "done", "$"], ["Err", "un"]]},
        done, fail_block("ov", "Ov"), fail_block("un", "Un")]}]})
}

#[test]
fn every_construct_matches_its_explicit_equivalent() {
    let temp = workspace("differential");
    let mut extended = Runner::new(&temp.path, &constructs_afx());
    let mut plain = Runner::new(&temp.path, &constructs_plain());
    let cases: Vec<(&str, Vec<Vec<Value>>)> = vec![
        ("arith", triples()),
        ("first", triples()),
        ("payload", pairs()),
        ("bare", triples()),
        ("safe_div", pairs()),
        ("use_call", pairs()),
        ("recover", pairs()),
        ("pick", indexes()),
        ("opt_call", indexes()),
        ("lookup", singles()),
        ("shadow", pairs()),
        ("join", pairs()),
        (
            "sum",
            vec![
                vec![json!(-1)],
                vec![json!(0)],
                vec![json!(1)],
                vec![json!(10)],
                vec![json!(100)],
            ],
        ),
        ("exits", triples()),
        ("failing", singles()),
        ("thread", pairs()),
    ];
    let mut compared = 0;
    for (function, inputs) in cases {
        let mut outcomes = std::collections::BTreeSet::new();
        for args in inputs {
            let got = extended.call(function, &args);
            let want = plain.call(function, &args);
            assert_eq!(got, want, "{function}{args:?}");
            outcomes.insert(got.to_string());
            compared += 1;
        }
        assert!(
            outcomes.len() > 1,
            "{function} only ever gives {outcomes:?}"
        );
    }
    assert!(compared > 2000, "{compared}");
    // A few outcomes stated independently of both sides.
    for (function, args, want) in [
        ("arith", json!([2, 3, 5]), json!({"Ok": 20})),
        ("arith", json!([i64::MAX, 1, 5]), json!({"Err": "Ov"})),
        ("arith", json!([1, 1, i64::MIN]), json!({"Err": "Un"})),
        // Both operands fail: the left one, evaluated first, wins.
        ("first", json!([i64::MAX, 2, 0]), json!({"Err": "B"})),
        ("first", json!([1, 2, 0]), json!({"Err": "C"})),
        (
            "payload",
            json!([1, 0]),
            json!({"Err": {"Arith": {"ArithmeticError": "DivideByZero"}}}),
        ),
        (
            "payload",
            json!([i64::MIN, -1]),
            json!({"Err": {"Arith": {"ArithmeticError": "Overflow"}}}),
        ),
        (
            "bare",
            json!([i64::MAX, 2, 0]),
            json!({"Err": {"ArithmeticError": "Overflow"}}),
        ),
        ("use_call", json!([7, 0]), json!({"Err": "Dz"})),
        ("recover", json!([3, 0]), json!({"Ok": 5})),
        ("pick", json!([1]), json!({"Some": 20})),
        ("pick", json!([3]), json!("None")),
        ("lookup", json!([2]), json!({"Some": 200})),
        ("sum", json!([10]), json!({"Ok": 55})),
        ("exits", json!([5, 7, 9]), json!({"Err": {"Code": 7}})),
        ("exits", json!([-4, 0, 9]), json!({"Ok": 4})),
        ("failing", json!([-2]), json!({"Err": {"Code": -2}})),
    ] {
        let args: Vec<Value> = args.as_array().unwrap().clone();
        assert_eq!(extended.call(function, &args), want, "{function}{args:?}");
    }
}
