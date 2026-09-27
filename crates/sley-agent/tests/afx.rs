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

// ---------------------------------------------------------------------------
// Generated expressions against a reference evaluator
// ---------------------------------------------------------------------------

/// xorshift64: a small deterministic generator (no external crates).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap()
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

#[derive(Clone, Copy, Debug)]
enum Arith {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Debug)]
enum Expr {
    Param(usize),
    Lit(i64, bool),
    /// Operation, failure case (`None`: bare `?`), operands.
    Op(Arith, Option<usize>, Box<Expr>, Box<Expr>),
}

const LITERALS: [i64; 14] = [
    0,
    1,
    -1,
    2,
    3,
    -7,
    1000,
    1 << 62,
    -(1 << 62),
    3_037_000_500,
    i64::MAX,
    i64::MIN,
    i64::MAX - 1,
    i64::MIN + 1,
];
const INPUTS: [i64; 10] = [
    i64::MIN,
    i64::MAX,
    0,
    -1,
    1,
    2,
    -2,
    1 << 32,
    3_037_000_500,
    1 << 62,
];

fn generate(rng: &mut Rng, depth: usize, bare: bool) -> Expr {
    let leaf = depth == 0 || (depth < 4 && rng.below(3) == 0);
    if leaf {
        return if rng.below(2) == 0 {
            Expr::Param(rng.below(3))
        } else {
            Expr::Lit(rng.pick(&LITERALS), rng.below(4) == 0)
        };
    }
    let op = rng.pick(&[Arith::Add, Arith::Sub, Arith::Mul, Arith::Div]);
    let case = (!bare).then(|| rng.below(4));
    let left = generate(rng, depth - 1, bare);
    let right = generate(rng, depth - 1, bare);
    Expr::Op(op, case, Box::new(left), Box::new(right))
}

/// Whether anything in the tree fixes the integer type.
fn anchored(expr: &Expr) -> bool {
    match expr {
        Expr::Param(_) | Expr::Lit(_, true) => true,
        Expr::Lit(_, false) => false,
        Expr::Op(_, _, left, right) => anchored(left) || anchored(right),
    }
}

/// Types the leftmost literal, so the tree has a known type.
fn anchor(expr: &mut Expr) -> bool {
    match expr {
        Expr::Lit(_, typed) => {
            *typed = true;
            true
        }
        Expr::Param(_) => false,
        Expr::Op(_, _, left, right) => anchor(left) || anchor(right),
    }
}

fn operand_json(expr: &Expr) -> Value {
    match expr {
        Expr::Param(index) => json!(["a", "b", "c"][*index]),
        Expr::Lit(value, false) => json!(value),
        Expr::Lit(value, true) => json!({"type": "i64", "value": value}),
        Expr::Op(op, case, left, right) => {
            let mut items = vec![json!(op_word(*op, *case))];
            items.push(operand_json(left));
            items.push(operand_json(right));
            Value::Array(items)
        }
    }
}

fn op_word(op: Arith, case: Option<usize>) -> String {
    let word = match op {
        Arith::Add => "add",
        Arith::Sub => "sub",
        Arith::Mul => "mul",
        Arith::Div => "div",
    };
    match case {
        None => format!("{word}?"),
        Some(3) => format!("{word}?P"),
        Some(case) => format!("{word}?C{case}"),
    }
}

/// Left to right, depth first; the first failure takes its operation's
/// route.
fn evaluate(expr: &Expr, args: &[i64; 3]) -> Result<i64, (Option<usize>, &'static str)> {
    match expr {
        Expr::Param(index) => Ok(args[*index]),
        Expr::Lit(value, _) => Ok(*value),
        Expr::Op(op, case, left, right) => {
            let left = evaluate(left, args)?;
            let right = evaluate(right, args)?;
            let result = match op {
                Arith::Add => left.checked_add(right).ok_or("Overflow"),
                Arith::Sub => left.checked_sub(right).ok_or("Overflow"),
                Arith::Mul => left.checked_mul(right).ok_or("Overflow"),
                Arith::Div if right == 0 => Err("DivideByZero"),
                Arith::Div => left.checked_div(right).ok_or("Overflow"),
            };
            result.map_err(|kind| (*case, kind))
        }
    }
}

fn expected_json(result: Result<i64, (Option<usize>, &'static str)>) -> Value {
    match result {
        Ok(value) => json!({"Ok": value}),
        Err((None, kind)) => json!({"Err": {"ArithmeticError": kind}}),
        Err((Some(3), kind)) => json!({"Err": {"P": {"ArithmeticError": kind}}}),
        Err((Some(case), _)) => json!({"Err": format!("C{case}")}),
    }
}

/// One generated function in one of three written forms.
fn generated_function(name: &str, expr: &Expr, bare: bool, form: usize) -> Value {
    let returns = if bare {
        "Result<i64,ArithmeticError>"
    } else {
        "Result<i64,E>"
    };
    // Forms 3 and 4 name the operands' operations in the block (their
    // results are referenced across the generated pieces); form 4 runs in a
    // second block whose parameters are derived edge arguments and are
    // threaded through every piece.
    let named = |block: &str| -> Value {
        let Expr::Op(op, case, left, right) = expr else {
            unreachable!("the root is an operation")
        };
        let mut ops = Vec::new();
        let mut operand = |label: &str, side: &Expr| match side {
            Expr::Op(op, case, l, r) => {
                ops.push(json!([
                    label,
                    op_word(*op, *case),
                    operand_json(l),
                    operand_json(r)
                ]));
                json!(label)
            }
            other => operand_json(other),
        };
        let left = operand("l", left);
        let right = operand("m", right);
        ops.push(json!(["r", op_word(*op, *case), left, right]));
        json!({"name": block, "params": if block == "entry" { json!([]) } else { params(&["a", "b", "c"], "i64") },
               "ops": ops, "term": ["ok", "r"]})
    };
    let blocks = match (form, expr) {
        (0, Expr::Op(op, case, left, right)) => json!([{"name": "entry",
            "ops": [["r", op_word(*op, *case), operand_json(left), operand_json(right)]],
            "term": ["ok", "r"]}]),
        (1, _) => json!([{"name": "entry", "term": ["ok", operand_json(expr)]}]),
        (2, _) => json!([{"name": "entry", "term": ["return", ["ok", operand_json(expr)]]}]),
        (3, _) => json!([named("entry")]),
        _ => json!([{"name": "entry", "term": ["br", "work"]}, named("work")]),
    };
    json!({"fn": name, "params": params(&["a", "b", "c"], "i64"), "returns": returns, "blocks": blocks})
}

#[test]
fn generated_expressions_match_a_reference_evaluator() {
    let temp = workspace("generated");
    let mut rng = Rng(0x5eed_2026_0927_af1d);
    let mut cases = 0;
    let mut failures = std::collections::BTreeMap::new();
    for _frame in 0..8 {
        let mut functions = Vec::new();
        let mut trees = Vec::new();
        for index in 0..40 {
            let bare = rng.below(4) == 0;
            let depth = 1 + rng.below(5);
            let mut expr = generate(&mut rng, depth, bare);
            if !matches!(expr, Expr::Op(..)) {
                expr = Expr::Op(
                    Arith::Add,
                    (!bare).then_some(0),
                    Box::new(expr),
                    Box::new(Expr::Param(0)),
                );
            }
            let form = rng.below(5);
            if form >= 3
                && let Expr::Op(_, _, left, right) = &mut expr
            {
                // A named operation takes no type from its user: each named
                // subtree needs its own anchor.
                for side in [left, right] {
                    if matches!(**side, Expr::Op(..)) && !anchored(side) {
                        anchor(side);
                    }
                }
            }
            if !anchored(&expr) {
                anchor(&mut expr);
            }
            let name = format!("g{index}");
            functions.push(generated_function(&name, &expr, bare, form));
            trees.push((name, expr));
        }
        let frame = json!({"af1": 1, "afx": 1,
            "types": [{"name": "E", "variant": ["C0", "C1", "C2", ["P", "ArithmeticError"]]}],
            "fns": functions});
        let mut runner = Runner::new(&temp.path, &frame);
        for (name, expr) in &trees {
            for _ in 0..12 {
                let args = [
                    rng.pick(&INPUTS),
                    rng.pick(&INPUTS),
                    if rng.below(2) == 0 {
                        rng.pick(&INPUTS)
                    } else {
                        i64::from_ne_bytes(rng.next().to_ne_bytes())
                    },
                ];
                let want = expected_json(evaluate(expr, &args));
                let got = runner.call(name, &args.map(Value::from));
                assert_eq!(got, want, "{name} {args:?}: {}", operand_json(expr));
                *failures.entry(want.get("Err").is_some()).or_insert(0) += 1;
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 8 * 40 * 12);
    // Both outcomes are well represented.
    assert!(
        failures[&true] > 300 && failures[&false] > 300,
        "{failures:?}"
    );
}

// ---------------------------------------------------------------------------
// Refusals: ambiguity, scope, bounds
// ---------------------------------------------------------------------------

fn one_function(returns: &str, fn_params: &Value, blocks: &Value) -> Value {
    json!({"af1": 1, "afx": 1, "types": [error_type()],
           "fns": [{"fn": "f", "params": fn_params, "returns": returns, "blocks": blocks}]})
}

fn assert_refused(dir: &Path, frame: &Value, symbol: &str, needles: &[&str]) {
    let (got, detail) = refused(dir, frame);
    assert_eq!(got, symbol, "{detail}");
    for needle in needles {
        assert!(detail.contains(needle), "missing `{needle}` in: {detail}");
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn ambiguous_or_unsupported_failure_routes_are_refused() {
    let temp = workspace("routes");
    let ab = params(&["a", "b"], "i64");
    let result = "Result<i64,E>";
    // A name that is both a block and a case.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "ops": [["x", "add?A", "a", "b"]], "term": ["ok", "x"]},
            {"name": "A", "term": ["ok", "a"]}]),
        ),
        "AGENT_X_PROPAGATION",
        &["/fns/0/blocks/0/ops/0: `A` names both a block of `f` and a case of E"],
    );
    // A handler whose first parameter is not the failure payload.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "ops": [["x", "add?h", "a", "b"]], "term": ["ok", "x"]},
            {"name": "h", "params": [["n", "i64"]], "term": ["ok", "n"]}]),
        ),
        "AGENT_X_PROPAGATION",
        &["handler `h` starts with `n: i64`, but the failure payload is ArithmeticError"],
    );
    // A payload case of another type, and None into a payload case.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "ops": [["x", "add?Code", "a", "b"]], "term": ["ok", "x"]}]),
        ),
        "AGENT_X_PROPAGATION",
        &["case `Code` carries a i64, but the failure payload here is ArithmeticError"],
    );
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &json!([["i", "u64"]]),
            &json!([
            {"name": "entry", "ops": [["v", "vec", {"type": "i64", "value": 1}], ["x", "vec_get?Code", "v", "i"]], "term": ["ok", "x"]}]),
        ),
        "AGENT_X_PROPAGATION",
        &["a None has no payload to pass"],
    );
    // `?` on a value that is not a Result or an Option.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "ops": [["c", "lt?A", "a", "b"]], "term": ["ok", "a"]}]),
        ),
        "AGENT_X_PROPAGATION",
        &["`lt?` propagates the failure of a Result or an Option, but `lt` gives bool"],
    );
    // A bare `?` whose failure type is not the function's.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "ops": [["x", "add?", "a", "b"]], "term": ["ok", "x"]}]),
        ),
        "AGENT_X_PROPAGATION",
        &["name a case: op?Case, or a handler block"],
    );
    // Neither a case nor a block; `fail` of a block; exit payload rules.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "ops": [["x", "add?Nope", "a", "b"], ["!A", "if", ["lt", "a", 0], "a"],
                                      ["!Code", "if", ["gt", "a", 9]]],
             "term": ["cond", ["lt", "a", "b"], "l", "m"]},
            {"name": "l", "term": ["fail", "m"]},
            {"name": "m", "term": ["fail", "Code"]}]),
        ),
        "AGENT_X_PROPAGATION",
        &[
            "`Nope` is neither a block of `f` nor a case of its error type (cases of E: Ov, Un, Dz, A, B, C, Arith, Code)",
            "/fns/0/blocks/0/ops/1: case `A` carries no payload",
            "/fns/0/blocks/0/ops/2: case `Code` carries a i64: add it",
            "/fns/0/blocks/1/term: `m` is not a case of the error type of `f`",
            "to go to block `m`, use [\"br\", \"m\"]",
            "/fns/0/blocks/2/term: case `Code` carries a i64: write [\"fail\", \"Code\", payload]",
        ],
    );
    // `ok` needs a Result function.
    assert_refused(
        &temp.path,
        &one_function(
            "Option<i64>",
            &ab,
            &json!([{"name": "entry", "term": ["ok", "a"]}]),
        ),
        "AGENT_X_PROPAGATION",
        &["[\"ok\", v] returns a Result, but `f` does not return one"],
    );
}

#[test]
fn literals_nesting_and_names_follow_the_dialect_rules() {
    let temp = workspace("forms");
    let ab = params(&["a", "b"], "i64");
    // A literal nothing types.
    assert_refused(
        &temp.path,
        &one_function(
            "Result<i64,E>",
            &ab,
            &json!([
            {"name": "entry", "ops": [["x", "add?A", 1, 2]], "term": ["ok", "x"]}]),
        ),
        "AGENT_FRAME_INVALID",
        &[
            "/fns/0/blocks/0/ops/0/2: nothing here fixes the type of the literal 1: state it, e.g. {\"type\": \"i64\", \"value\": 1}",
        ],
    );
    // A nested operation where it would run on one path only.
    assert_refused(
        &temp.path,
        &one_function(
            "Result<i64,E>",
            &ab,
            &json!([
            {"name": "entry", "term": ["cond", ["lt", "a", "b"], ["t", ["add?A", "a", "b"]], ["t", 0]]},
            {"name": "t", "params": [["v", "i64"]], "term": ["ok", "v"]}]),
        ),
        "AGENT_FRAME_INVALID",
        &[
            "/fns/0/blocks/0/term/2/1: only names and literals may appear here",
            "name the operation in the target block",
        ],
    );
    // `__` is reserved for generated names.
    assert_refused(
        &temp.path,
        &one_function(
            "i64",
            &json!([["a__b", "i64"]]),
            &json!([
            {"name": "entry__x", "ops": [["y__z", "const", 1]], "term": ["return", "a__b"]}]),
        ),
        "AGENT_FRAME_INVALID",
        &[
            "/fns/0/params/0: `a__b` contains `__`, which is reserved for generated names",
            "/fns/0/blocks/0: `entry__x` contains `__`",
            "/fns/0/blocks/0/ops/0: `y__z` contains `__`",
        ],
    );
    // `edit.with` stays plain AF1; `ripple` is not enabled; "afx" is 1.
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "edit": [{"fn": "f", "replace_op": "entry.x", "with": ["add", "a", 1]}]}),
        "AGENT_FRAME_INVALID",
        &[
            "/edit/0/with: an edit replaces one operation with plain AF1",
            "use patch to restate the block",
        ],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "ripple": [{"intent": "arity"}]}),
        "AGENT_RIPPLE_INTENT_UNKNOWN",
        &["/ripple: ripple is not enabled in this build"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 2}),
        "AGENT_FRAME_INVALID",
        &["/afx: declare \"afx\": 1"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "bogus": []}),
        "AGENT_FRAME_INVALID",
        &["/bogus: unknown frame key"],
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn names_resolve_only_to_unique_available_values() {
    let temp = workspace("scope");
    let ab = params(&["a", "b"], "i64");
    let result = "Result<i64,E>";
    // Several definers at a join.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "term": ["cond", ["lt", "a", "b"], "left", "right"]},
            {"name": "left", "ops": [["t", "sub", "b", "a"]], "term": ["br", "done"]},
            {"name": "right", "ops": [["t", "sub", "a", "b"]], "term": ["br", "done"]},
            {"name": "done", "ops": [["r", "add", "t", "a"]], "term": ["return", "t"]}]),
        ),
        "AGENT_X_SCOPE",
        &[
            "/fns/0/blocks/3/ops/0/2: several blocks define `t` (left, right), so a plain `t` is ambiguous in `done`",
        ],
    );
    // A definer that does not dominate.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "term": ["cond", ["lt", "a", "b"], "left", "done"]},
            {"name": "left", "ops": [["t", "lt", "b", "a"]], "term": ["br", "done"]},
            {"name": "done", "term": ["cond", "t", ["fin", "a"], ["fin", "b"]]},
            {"name": "fin", "params": [["v", "i64"]], "term": ["ok", "v"]}]),
        ),
        "AGENT_X_SCOPE",
        &["`t` is defined in block `left`, which does not dominate this point of `done`"],
    );
    // A definer where the name is a parameter or an unwrapped value.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "ops": [["s", "add?Ov", "a", "b"]], "term": ["br", "next", "a"]},
            {"name": "next", "params": [["q", "i64"]], "term": ["br", "last"]},
            {"name": "last", "ops": [["x", "add?Ov", "q", "s"]], "term": ["ok", "x"]}]),
        ),
        "AGENT_X_SCOPE",
        &[
            "/fns/0/blocks/2/ops/0/2: `q` is a parameter of block `next`",
            "declare `q` as a parameter of `last`; its edge argument is derived",
            "/fns/0/blocks/2/ops/0/3: `s` is a parameter of block `entry` (or the value a checked operation unwraps there)",
        ],
    );
    // Use before definition in the same block.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "ops": [["y", "add?Ov", "x", "a"], ["x", "add?Ov", "a", "b"]], "term": ["ok", "y"]}]),
        ),
        "AGENT_X_SCOPE",
        &["/fns/0/blocks/0/ops/0/2: `x` is used before its definition in block `entry`"],
    );
    // A missing predecessor value: an obligation naming the parameter, its
    // type and the values of that type available there.
    let frame = one_function(
        result,
        &ab,
        &json!([
        {"name": "entry", "ops": [["s", "add?Ov", "a", "b"], ["flag", "lt", "a", "b"]], "term": ["br", "next"]},
        {"name": "next", "params": [["total", "i64"]], "term": ["ok", "total"]}]),
    );
    assert_refused(
        &temp.path,
        &frame,
        "AGENT_X_SCOPE",
        &[
            "/fns/0/blocks/0/term: block `next` takes `total: i64`, and no value named `total` is available here: pass it explicitly (values of that type here: s: i64, a: i64, b: i64)",
        ],
    );
    let expansion = expand(&temp.path, &frame);
    let obligation = &expansion.obligations[0];
    assert_eq!(obligation.expected.as_deref(), Some("i64"));
    assert_eq!(
        obligation.available.clone().unwrap(),
        ["s: i64", "a: i64", "b: i64"]
    );
    // A loop edge never passes a value back to the block that defines it.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &json!([["n", "i64"]]),
            &json!([
            {"name": "entry", "term": ["br", "spin", 0]},
            {"name": "spin", "params": [["i", "i64"]], "ops": [["j", "add?Ov", "i", 1]],
             "term": ["cond", ["gt", "j", "n"], ["fin", "j"], "spin"]},
            {"name": "fin", "params": [["v", "i64"]], "term": ["ok", "v"]}]),
        ),
        "AGENT_X_SCOPE",
        &[
            "/fns/0/blocks/1/term: the edge back to `spin` would pass `i` from `spin` itself (a loop edge)",
        ],
    );
    // An explicit `b.x` of another block's parameter.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "term": ["br", "next", "a"]},
            {"name": "next", "params": [["q", "i64"]], "term": ["br", "last"]},
            {"name": "last", "term": ["ok", "next.q"]}]),
        ),
        "AGENT_X_SCOPE",
        &["/fns/0/blocks/2/term/1: `next.q` is a parameter of block `next`"],
    );
    // Problems of several symbols: the first decides the refusal, the
    // others carry their own symbol.
    let (symbol, detail) = refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "ops": [["y", "add?Ov", "x", "a"], ["x", "add?Ov", "a", 1], ["z", "sub?Un", 1, 2]], "term": ["ok", "y"]}]),
        ),
    );
    assert_eq!(symbol, "AGENT_X_SCOPE");
    assert!(detail.contains("(1 of 3 problems)"), "{detail}");
    assert!(
        detail.contains(
            "\n  [AGENT_FRAME_INVALID] /fns/0/blocks/0/ops/2/2: nothing here fixes the type"
        ),
        "{detail}"
    );
}

#[test]
fn qualification_threading_and_generated_names_are_visible_in_the_expansion() {
    let temp = workspace("naming");
    let frame = json!({"af1": 1, "afx": 1, "types": [error_type()], "fns": [
      {"fn": "b__t0", "params": [], "returns": "i64", "blocks": [{"name": "entry", "ops": [["z", "const", 0]], "term": ["return", "z"]}]},
      {"fn": "f", "params": params(&["x", "y"], "i64"), "returns": "Result<i64,E>", "blocks": [
        {"name": "b", "params": [], "ops": [["k", "lt", "x", "y"], ["if0", "add?Ov", "x", 1], ["big", "gt", "if0", 100],
                                           ["!A", "if", ["lt", "if0", 0]]],
         "term": ["br", "next", ["add?Ov", "if0", 2]]},
        {"name": "next", "params": [["w", "i64"]], "term": ["cond", "k", ["fin", "w"], ["other", "w"]]},
        {"name": "other", "params": [["w", "i64"]], "term": ["cond", "big", ["fin", 7], ["fin", "w"]]},
        {"name": "fin", "params": [["v", "i64"]], "term": ["ok", "v"]}]}]});
    let expansion = expand(&temp.path, &frame);
    assert!(
        expansion.obligations.is_empty(),
        "{:?}",
        expansion.obligations
    );
    let blocks = &expansion.frame["fns"][1]["blocks"];
    let names: Vec<&str> = blocks
        .as_array()
        .unwrap()
        .iter()
        .map(|block| block["name"].as_str().unwrap())
        .collect();
    // Authored blocks keep their places; generated pieces follow; a
    // collision takes the next suffix.
    assert_eq!(
        names,
        [
            "b",
            "next",
            "other",
            "fin",
            "b__if0",
            "b__if0_2",
            "b__b__t0_2",
            "__fail_Ov",
            "__fail_A"
        ]
    );
    // The terminator's nested value would collide with the function `b__t0`.
    let last = &blocks[5];
    assert_eq!(last["ops"][1][0], json!("b__t0_2__r"), "{last}");
    assert_eq!(
        last["ops"][0],
        json!(["b__t0_2__a1", "const", {"type": "i64", "value": 2}])
    );
    // X4: `k` from the first piece, `big` from the piece that holds it.
    assert_eq!(blocks[1]["term"][1], json!("b.k"));
    assert_eq!(blocks[2]["term"][1], json!("b__if0.big"));
    // The unwrapped value is threaded into the next piece.
    assert_eq!(blocks[5]["params"], json!([["if0", "i64"]]));
    assert_eq!(
        expansion.map.origin("f", "b__if0_2"),
        Some("/fns/1/blocks/0/ops/3")
    );
    assert_eq!(expansion.stats.qualified, 2);
    let mut runner = Runner::new(&temp.path, &frame);
    assert_eq!(runner.call("f", &[json!(1), json!(5)]), json!({"Ok": 4}));
    assert_eq!(runner.call("f", &[json!(500), json!(5)]), json!({"Ok": 7}));
    assert_eq!(runner.call("f", &[json!(9), json!(5)]), json!({"Ok": 12}));
    assert_eq!(
        runner.call("f", &[json!(-9), json!(5)]),
        json!({"Err": "A"})
    );
    assert_eq!(
        runner.call("f", &[json!(i64::MAX), json!(5)]),
        json!({"Err": "Ov"})
    );
    assert_eq!(runner.call("b__t0", &[]), json!(0));
}

#[test]
fn explicit_edges_are_never_repaired() {
    let temp = workspace("edges");
    let ab = params(&["a", "b"], "i64");
    let result = "Result<i64,E>";
    // Too many arguments: refused as authored, with the expanded pointer.
    let frame = one_function(
        result,
        &ab,
        &json!([
        {"name": "entry", "ops": [["s", "add?Ov", "a", "b"]], "term": ["br", "done", "s", "a"]},
        {"name": "done", "params": [["v", "i64"]], "term": ["ok", "v"]}]),
    );
    assert_eq!(
        expand(&temp.path, &frame).frame["fns"][0]["blocks"][2]["term"],
        json!(["br", "done", "s", "a"])
    );
    assert_refused(
        &temp.path,
        &frame,
        "AGENT_FRAME_INVALID",
        &[
            "/fns/0/blocks/0/term: block `done` takes 1 argument(s) (v: i64); this edge passes 2 [expanded /fns/0/blocks/2/term]",
        ],
    );
    // A wrongly typed explicit argument is not replaced by a fitting value.
    let frame = one_function(
        result,
        &ab,
        &json!([
        {"name": "entry", "ops": [["s", "add?Ov", "a", "b"], ["c", "lt", "a", "b"]], "term": ["br", "done", "c"]},
        {"name": "done", "params": [["s", "i64"]], "term": ["ok", "s"]}]),
    );
    assert_refused(
        &temp.path,
        &frame,
        "AGENT_FRAME_INVALID",
        &["/fns/0/blocks/0/term: `c` is bool but parameter `s` of block `done` is i64"],
    );
    // A list-wrapped argument keeps its refusal.
    assert_refused(
        &temp.path,
        &one_function(
            result,
            &ab,
            &json!([
            {"name": "entry", "term": ["br", ["done", ["a"]]]},
            {"name": "done", "params": [["v", "i64"]], "term": ["ok", "v"]}]),
        ),
        "AGENT_FRAME_INVALID",
        &["edge arguments are value names"],
    );
}

#[test]
fn diagnostics_point_at_the_authored_frame() {
    let temp = workspace("diagnostics");
    let ab = params(&["a", "b"], "i64");
    // A problem in a hoisted nested operation names the nested operand.
    assert_refused(
        &temp.path,
        &one_function(
            "Result<i64,ArithmeticError>",
            &ab,
            &json!([
            {"name": "entry", "ops": [["x", "add", "a", ["field", "Nope.x", "a"]]], "term": ["return", "x"]}]),
        ),
        "AGENT_FRAME_INVALID",
        &["/fns/0/blocks/0/ops/0/3: no type `Nope` [expanded /fns/0/blocks/0/ops/0]"],
    );
    // A problem in a checked operation names the authored operation.
    assert_refused(
        &temp.path,
        &one_function(
            "Result<i64,E>",
            &ab,
            &json!([
            {"name": "entry", "ops": [["x", "add?Ov", "a", ["lt", "a", "b"]]], "term": ["ok", "x"]}]),
        ),
        "AGENT_FRAME_INVALID",
        &[
            "/fns/0/blocks/0/ops/0: the operands of `add` must have one type: `a` is i64, `x__a1` is bool [expanded /fns/0/blocks/0/ops/1]",
        ],
    );
}

#[test]
fn bounds_are_refused_never_truncated() {
    let temp = workspace("bounds");
    let mut nested = json!("a");
    for _ in 0..40 {
        nested = json!(["add?", nested, 1]);
    }
    assert_refused(
        &temp.path,
        &one_function(
            "Result<i64,ArithmeticError>",
            &json!([["a", "i64"]]),
            &json!([
            {"name": "entry", "term": ["ok", nested]}]),
        ),
        "AGENT_X_LIMIT",
        &["operations nest more than 32 levels deep"],
    );
    let ops: Vec<Value> = (0..2100)
        .map(|index| json!([format!("x{index}"), "add", "a", 1]))
        .collect();
    assert_refused(
        &temp.path,
        &one_function(
            "Result<i64,ArithmeticError>",
            &json!([["a", "i64"]]),
            &json!([
            {"name": "entry", "ops": ops, "term": ["return", "x0"]}]),
        ),
        "AGENT_X_LIMIT",
        &["/fns/0: the expanded function has 4200 operations, more than the bound of 4096"],
    );
}

#[test]
fn the_generated_block_bound_is_refused() {
    let temp = workspace("block-bound");
    let exits: Vec<Value> = (0..1030).map(|_| json!(["!A", "if", "c"])).collect();
    assert_refused(
        &temp.path,
        &one_function(
            "Result<i64,E>",
            &json!([["c", "bool"], ["a", "i64"]]),
            &json!([
            {"name": "entry", "ops": exits, "term": ["ok", "a"]}]),
        ),
        "AGENT_X_LIMIT",
        &["/fns/0: the expanded function has 1031 generated blocks, more than the bound of 1024"],
    );
}

#[test]
fn expansion_and_compilation_are_deterministic() {
    let temp = workspace("determinism");
    let frame = constructs_afx();
    let first = expand(&temp.path, &frame);
    let second = expand(&temp.path, &frame);
    assert_eq!(first.frame, second.frame);
    assert_eq!(first.map, second.map);
    assert_eq!(first.stats, second.stats);
    assert_eq!(
        first.map.to_json().to_string(),
        second.map.to_json().to_string()
    );
    assert_eq!(
        compile_digest(&temp.path, &frame),
        compile_digest(&temp.path, &frame)
    );
    // The compiled frame carries the expansion, its map and the counts.
    let head = Workspace::at(&temp.path).head().unwrap();
    let names = names_of(&temp.path, head.program());
    let authority = sley_agent::candidate::Authority::of(&head).unwrap();
    let compiled = sley_agent::frame::compile(
        head.program(),
        &names,
        &authority.ceilings,
        &frame,
        sley_id::CandidateNonce::from_bytes([3; 32]),
        &mut || Ok([4; 32]),
    )
    .unwrap();
    let artifacts: Vec<&str> = compiled
        .artifacts
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(artifacts, ["expanded.json", "sourcemap.json"]);
    assert_eq!(compiled.artifacts[0].1, first.frame);
    assert_eq!(compiled.artifacts[1].1, first.map.to_json());
    let keys: Vec<&str> = compiled.stats.keys().map(String::as_str).collect();
    for key in [
        "nested",
        "literals",
        "checked",
        "exits",
        "ok_fail",
        "derived_args",
        "qualified",
        "table_rows",
        "generated_blocks",
    ] {
        assert!(keys.contains(&key), "{key}");
        if key != "table_rows" {
            assert!(compiled.stats[key].as_u64().unwrap() > 0, "{key}");
        }
    }
}

// ---------------------------------------------------------------------------
// Patches, test tables, help
// ---------------------------------------------------------------------------

fn block_names(view: &str) -> Vec<String> {
    view.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (name, _) = line.split_once(':')?;
            (line.ends_with(':') || name.ends_with(')'))
                .then(|| name.split('(').next().unwrap().to_owned())
        })
        .collect()
}

#[test]
fn restating_a_block_deletes_the_pieces_it_no_longer_makes() {
    let temp = workspace("patch");
    let frame = json!({"af1": 1, "afx": 1, "types": [error_type()], "fns": [
        {"fn": "pf", "params": params(&["a", "b"], "i64"), "returns": "Result<i64,E>", "blocks": [
          {"name": "entry", "ops": [["s", "add?Ov", "a", "b"], ["t", "mul?Un", "s", 2]], "term": ["ok", "t"]}]},
        {"fn": "kx", "params": [["a", "i64"]], "returns": "Result<i64,E>", "blocks": [
          {"name": "entry", "ops": [["k", "mul?Ov", "a", 3], ["m", "gt", "k", 10]], "term": ["cond", "m", "hi", "lo"]},
          {"name": "hi", "term": ["ok", 1]},
          {"name": "lo", "term": ["ok", 2]}]}]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    let (_, view) = run(&temp.path, &["view", "pf"]);
    assert_eq!(
        block_names(&view),
        ["entry", "entry__s", "entry__t", "__fail_Ov", "__fail_Un"],
        "{view}"
    );
    // Restating `entry` with one checked operation: the second piece and
    // the exit only it used are deleted; the rest keep their identities.
    let patch = json!({"af1": 1, "afx": 1, "patch": [
        {"fn": "pf", "blocks": {"entry": {"ops": [["s", "add?Ov", "a", "b"]], "term": ["ok", ["neg?Ov", "s"]]}}},
        {"fn": "kx", "blocks": {
            "hi": {"ops": [["x", "add?Ov", "a", 5]], "term": ["cond", "m", ["fin", "x"], "lo"]},
            "fin": {"params": [["x", "i64"]], "term": ["ok", "x"]}}}]});
    let expansion = expand(&temp.path, &patch);
    assert!(
        expansion.obligations.is_empty(),
        "{:?}",
        expansion.obligations
    );
    let blocks = &expansion.frame["patch"][0]["blocks"];
    assert_eq!(blocks["entry__t"], Value::Null, "{blocks}");
    assert_eq!(blocks["__fail_Un"], Value::Null, "{blocks}");
    assert!(blocks["entry__s"].is_object() && blocks["__fail_Ov"].is_object());
    // X4 reaches into a kept live block that dominates the use.
    let hi = &expansion.frame["patch"][1]["blocks"]["hi__x"];
    assert_eq!(hi["term"][1], json!("entry__k.m"), "{hi}");
    let (status, text) = run(&temp.path, &["try", &patch.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    let (_, view) = run(&temp.path, &["view", "pf"]);
    assert_eq!(
        block_names(&view),
        ["entry", "entry__s", "__fail_Ov", "entry__entry__t0"],
        "{view}"
    );
    let (_, value) = run(&temp.path, &["call", "pf", "2", "3"]);
    assert_eq!(value.trim(), "{\"Ok\":-5}");
    let (_, value) = run(&temp.path, &["call", "kx", "5"]);
    assert_eq!(value.trim(), "{\"Ok\":10}");
    let (_, value) = run(&temp.path, &["call", "kx", "1"]);
    assert_eq!(value.trim(), "{\"Ok\":2}");
    // Deleting a block deletes its pieces too.
    let delete = json!({"af1": 1, "afx": 1, "patch": [{"fn": "kx", "blocks": {
        "hi": null, "fin": null, "entry": {"ops": [["k", "mul?Ov", "a", 3], ["m", "gt", "k", 10]], "term": ["cond", "m", "lo", "lo"]}}}]});
    let expansion = expand(&temp.path, &delete);
    let blocks = &expansion.frame["patch"][0]["blocks"];
    assert_eq!(blocks["hi__x"], Value::Null, "{blocks}");
    assert_eq!(blocks["hi"], Value::Null, "{blocks}");
}

#[test]
fn test_tables_lower_to_tests_that_point_at_their_rows() {
    let temp = workspace("tables");
    let function = json!({"fn": "twice", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
        "blocks": [{"name": "entry", "term": ["ok", ["mul?", "a", 2]]}]});
    let table = |cases: Value| {
        json!({"af1": 1, "afx": 1, "fns": [function.clone()],
               "test_tables": [{"name": "t_twice", "fn": "twice", "defaults": {"limits": {"fuel": 100_000}}, "cases": cases}]})
    };
    let frame = table(json!([
        {"args": [2], "expect": {"Ok": 4}},
        {"args": [-3], "expect": {"Ok": -6}},
        {"name": "t_big", "args": [i64::MAX], "expect": {"Err": {"ArithmeticError": "Overflow"}}}]));
    let (status, result) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{result}");
    let tests: Vec<&str> = result["tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|test| test["test"].as_str().unwrap())
        .collect();
    assert_eq!(tests, ["t_big", "t_twice_0", "t_twice_1"]);
    // Layering replaces the table by name; it never accumulates rows.
    let smaller = json!({"af1": 1, "afx": 1, "test_tables": [
        {"name": "t_twice", "fn": "twice", "cases": [{"args": [5], "expect": {"Ok": 10}}]}]});
    let (status, result) = run_json(&temp.path, &["try", "--on", "c1", &smaller.to_string()]);
    assert_eq!(status, 0, "{result}");
    assert_eq!(result["tests"].as_array().unwrap().len(), 1, "{result}");
    assert_eq!(result["tests"][0]["test"], json!("t_twice_0"));
    // An encoding error points at the row.
    assert_refused(
        &temp.path,
        &table(json!([{"args": [1], "expect": {"Ok": 2}}, {"args": ["one"], "expect": {"Ok": 2}}])),
        "AGENT_FRAME_INVALID",
        &[
            "/test_tables/0/cases/1/args/0: ",
            "[expanded /tests/1/args/0]",
        ],
    );
    // Duplicate rows, colliding names, unknown keys and missing parts.
    assert_refused(
        &temp.path,
        &table(json!([{"args": [1], "expect": {"Ok": 2}}, {"args": [1], "expect": {"Ok": 3}}])),
        "AGENT_TEST_TABLE_INVALID",
        &["/test_tables/0/cases/1: rows 0 and 1 of table `t_twice` have the same args: keep one"],
    );
    let mut colliding = table(json!([{"args": [1], "expect": {"Ok": 2}}]));
    colliding["tests"] =
        json!([{"name": "t_twice_0", "fn": "twice", "args": [7], "expect": {"Ok": 14}}]);
    assert_refused(
        &temp.path,
        &colliding,
        "AGENT_TEST_TABLE_INVALID",
        &[
            "/test_tables/0/cases/0: the row's test name `t_twice_0` is taken by another test of this frame",
        ],
    );
    assert_refused(
        &temp.path,
        &table(json!([{"args": [1], "expect": {"Ok": 2}, "expected": 3}])),
        "AGENT_TEST_TABLE_INVALID",
        &["/test_tables/0/cases/0/expected: unknown row key"],
    );
    assert_refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "test_tables": [{"name": "t", "cases": []}, {"name": "u", "fn": "twice"}]}),
        "AGENT_TEST_TABLE_INVALID",
        &[
            "/test_tables/0: missing \"fn\"",
            "/test_tables/1: missing \"cases\"",
        ],
    );
}

#[test]
fn every_example_in_help_afx_runs() {
    let temp = workspace("help-afx");
    let examples: Vec<&str> = sley_agent::help::AFX
        .split("```json\n")
        .skip(1)
        .map(|rest| rest.split("```").next().unwrap())
        .collect();
    assert!(examples.len() >= 3);
    for (index, example) in examples.iter().enumerate() {
        let frame: Value = serde_json::from_str(example)
            .unwrap_or_else(|error| panic!("example {index} is not JSON: {error}"));
        let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
        assert_eq!(status, 0, "help afx example {index}: {text}");
        let rows: usize = frame["test_tables"]
            .as_array()
            .unwrap()
            .iter()
            .map(|table| table["cases"].as_array().unwrap().len())
            .sum();
        assert!(
            text.contains(&format!("tests: {rows}/{rows} passed")),
            "help afx example {index}: {text}"
        );
    }
    assert_eq!(
        sley_agent::help::topic("afx").as_deref(),
        Some(sley_agent::help::AFX)
    );
    let (status, text) = run(&temp.path, &["help", "afx"]);
    assert_eq!(status, 0);
    assert!(text.starts_with("# AF1-X"), "{text}");
}

#[test]
fn a_malformed_terminator_is_reported_by_the_compiler_as_authored() {
    // The expander passes a terminator it does not understand through; the
    // compiler's own fix is reported, not a guess about names downstream.
    let temp = workspace("malformed");
    let (symbol, detail) = refused(
        &temp.path,
        &one_function(
            "Result<i64,E>",
            &params(&["a", "b"], "i64"),
            &json!([
            {"name": "entry", "ops": [["k", "lt", "a", "b"], ["s", "add?Ov", "a", "b"]],
             "term": ["cond", "k", "yes", "no", "s"]},
            {"name": "yes", "term": ["ok", ["add?Ov", "m", 1]]},
            {"name": "no", "ops": [["m", "const", 5]], "term": ["ok", 0]}]),
        ),
    );
    assert_eq!(symbol, "AGENT_FRAME_INVALID", "{detail}");
    assert!(
        detail
            .contains("/fns/0/blocks/0/term: `cond` takes [\"cond\", c, then, else], not 5 items"),
        "{detail}"
    );
    assert!(!detail.contains("AGENT_X_SCOPE"), "{detail}");
}

#[test]
fn derived_arguments_are_threaded_and_literals_take_member_types() {
    let temp = workspace("threaded-derive");
    let frame = json!({"af1": 1, "afx": 1,
      "types": [error_type(),
                {"name": "Shape", "variant": ["Empty", ["Circle", "i64"]]},
                {"name": "Point", "record": [["x", "i64"], ["y", "u8"]]}],
      "fns": [
        {"fn": "f", "params": [["a", "i64"]], "returns": "Result<i64,E>", "blocks": [
          {"name": "entry", "term": ["br", "work", "a"]},
          {"name": "work", "params": [["p", "i64"]],
           "ops": [["s", "add?Ov", "p", 1], ["t", "mul?Ov", "s", 3]], "term": ["br", "next"]},
          {"name": "next", "params": [["p", "i64"], ["t", "i64"]], "ops": [["d", "sub?Un", "t", "p"]], "term": ["ok", "d"]}]},
        {"fn": "circle", "params": [], "returns": "Shape", "blocks": [
          {"name": "entry", "term": ["return", ["variant", "Shape.Circle", 5]]}]},
        {"fn": "point", "params": [], "returns": "Point", "blocks": [
          {"name": "entry", "term": ["return", ["record", "Point", -2, 200]]}]}]});
    let expansion = expand(&temp.path, &frame);
    assert!(
        expansion.obligations.is_empty(),
        "{:?}",
        expansion.obligations
    );
    let blocks = &expansion.frame["fns"][0]["blocks"];
    // `p` rides through both continuations; `next` gets it and `t` derived.
    assert_eq!(blocks[3]["name"], json!("work__s"));
    assert_eq!(blocks[3]["params"], json!([["s", "i64"], ["p", "i64"]]));
    assert_eq!(blocks[4]["params"], json!([["t", "i64"], ["p", "i64"]]));
    assert_eq!(blocks[4]["term"], json!(["br", "next", "p", "t"]));
    assert_eq!(expansion.stats.derived_args, 2);
    let mut runner = Runner::new(&temp.path, &frame);
    assert_eq!(runner.call("f", &[json!(4)]), json!({"Ok": 11}));
    assert_eq!(runner.call("f", &[json!(i64::MAX)]), json!({"Err": "Ov"}));
    assert_eq!(runner.call("circle", &[]), json!({"Circle": 5}));
    assert_eq!(runner.call("point", &[]), json!({"x": -2, "y": 200}));
}
