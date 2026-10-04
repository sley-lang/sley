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

/// The worked AF1 example the 2.0.2 guide taught (plain AF1).
fn guide_example() -> Value {
    serde_json::from_str(include_str!("fixtures/percent.json")).unwrap()
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
        "tables-refused=refused AGENT_FRAME_INVALID: /test_tables: `test_tables` belongs to the authoring dialect: add \"afx\": 1 to the frame (the AF1-X envelope) to use it",
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

#[test]
fn scalar_operations_report_wrapped_operands_before_partner_literals() {
    let temp = workspace("wrapped-operands");
    let frame = |ops: Value| {
        json!({"af1": 1, "afx": 1,
            "types": [{"name": "MathError", "variant": ["Overflow"]}],
            "fns": [{"fn": "scale", "params": [["x", "i64"]],
                "returns": "Result<i64,MathError>",
                "blocks": [{"name": "entry", "ops": ops, "term": ["ok", "y"]}]}]})
    };
    for (ops, pointer) in [
        (
            json!([["n", "add", "x", 9], ["y", "div?Overflow", "n", 10]]),
            "/fns/0/blocks/0/ops/1/2",
        ),
        (
            json!([["n", "add", "x", 9], ["y", "div?Overflow", 10, "n"]]),
            "/fns/0/blocks/0/ops/1/3",
        ),
        (
            json!([["y", "div?Overflow", ["add", "x", 9], 10]]),
            "/fns/0/blocks/0/ops/0/2",
        ),
    ] {
        let (symbol, detail) = refused(&temp.path, &frame(ops));
        assert_eq!(symbol, "AGENT_FRAME_INVALID", "{detail}");
        assert!(detail.starts_with(pointer), "{detail}");
        assert!(detail.contains("Result<i64,ArithmeticError>"), "{detail}");
        assert!(detail.contains("unwrap"), "{detail}");
        assert!(detail.contains("?Case"), "{detail}");
        assert!(!detail.contains("expected {\"Ok\""), "{detail}");
    }
    let fixed = frame(json!([
        ["n", "add?Overflow", "x", 9],
        ["y", "div?Overflow", "n", 10]
    ]));
    let mut runner = Runner::new(&temp.path, &fixed);
    assert_eq!(runner.call("scale", &[json!(11)]), json!({"Ok": 2}));
    assert_eq!(
        runner.call("scale", &[json!(i64::MAX)]),
        json!({"Err": "Overflow"})
    );
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

#[test]
fn a_checked_operand_of_a_cell_write_matches_explicit_control_flow() {
    let temp = workspace("cell-order");
    let dialect = json!({"af1": 1, "afx": 1, "fns": [{
        "fn": "f", "params": [["x", "i64"], ["y", "i64"]],
        "returns": "Result<i64,ArithmeticError>", "blocks": [{
            "name": "entry", "ops": [
                ["c", "cell", "x"],
                ["s", "cell_set", "c", ["div?", "x", "y"]],
                ["v", "cell_get", "c"]],
            "term": ["ok", "v"]
        }]
    }]});
    let explicit = json!({"af1": 1, "fns": [{
        "fn": "f", "params": [["x", "i64"], ["y", "i64"]],
        "returns": "Result<i64,ArithmeticError>", "blocks": [
            {"name": "entry", "ops": [["c", "cell", "x"], ["q", "div", "x", "y"]],
             "term": ["switch", "q", ["Ok", "write", "$"], ["Err", "fail", "$"]]},
            {"name": "write", "params": [["t", "i64"]],
             "ops": [["s", "cell_set", "entry.c", "t"],
                     ["v", "cell_get", "entry.c"], ["o", "ok", "v"]],
             "term": ["return", "o"]},
            {"name": "fail", "params": [["e", "ArithmeticError"]],
             "ops": [["r", "err", "e"]], "term": ["return", "r"]}
        ]
    }]});
    let mut expanded = Runner::new(&temp.path, &dialect);
    let mut reference = Runner::new(&temp.path, &explicit);
    for (x, y) in [(9, 3), (-9, 3), (0, 3), (9, 0), (i64::MIN, -1)] {
        let args = [json!(x), json!(y)];
        assert_eq!(
            expanded.call("f", &args),
            reference.call("f", &args),
            "{args:?}"
        );
    }
    assert_eq!(expanded.call("f", &[json!(9), json!(3)]), json!({"Ok": 3}));
    assert!(
        expanded
            .call("f", &[json!(9), json!(0)])
            .get("Err")
            .is_some()
    );
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
    // `__` is reserved for generated names in blocks that use the
    // dialect's forms (here a literal operand).
    assert_refused(
        &temp.path,
        &one_function(
            "Result<i64,ArithmeticError>",
            &json!([["a__b", "i64"]]),
            &json!([
            {"name": "entry__x", "ops": [["y__z", "add", "a__b", 1]], "term": ["return", "y__z"]}]),
        ),
        "AGENT_FRAME_INVALID",
        &[
            "/fns/0/params/0: `a__b` contains `__`, which is reserved for generated names",
            "/fns/0/blocks/0: `entry__x` contains `__`",
            "/fns/0/blocks/0/ops/0: `y__z` contains `__`",
        ],
    );
    // W3-D2: a plain block keeps the names AF1 allows, next to a block that
    // uses the dialect.
    let frame = one_function(
        "Result<i64,ArithmeticError>",
        &json!([["a", "i64"]]),
        &json!([
            {"name": "entry", "ops": [["z", "const", {"type": "i64", "value": 0}], ["c", "lt", "a", "z"]], "term": ["cond", "c", "is__neg", "sum"]},
            {"name": "is__neg", "ops": [["n__v", "neg", "a"]], "term": ["return", "n__v"]},
            {"name": "sum", "term": ["ok", ["add?", "a", 1]]}]),
    );
    let mut runner = Runner::new(&temp.path, &frame);
    assert_eq!(runner.call("f", &[json!(-2)]), json!({"Ok": 2}));
    assert_eq!(runner.call("f", &[json!(2)]), json!({"Ok": 3}));
    // `edit.with` stays plain AF1; an unknown ripple intent is refused;
    // "afx" is 1.
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
        &["/ripple/0: unknown intent"],
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
            "/fns/0/blocks/3/ops/0/2: `t` is defined in blocks left, right, none of which dominates this point of `done` (paths join here)",
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
            "/fns/0/blocks/2/ops/0/3: `s` is the value a checked operation of block `entry` unwraps, visible only in its own block",
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
            "/fns/0/blocks/1/term: block `spin` takes `i: i64`, and this edge goes back into `spin`, which it is inside of (a loop edge), and a loop edge never takes an omitted argument by name",
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
fn continuation_inputs_keep_suffix_first_use_order() {
    let temp = workspace("thread-order");
    let frame = json!({"af1": 1, "afx": 1, "fns": [{
        "fn": "f", "params": params(&["a", "b", "c"], "i64"),
        "returns": "Result<i64,ArithmeticError>", "blocks": [
            {"name": "entry", "term": ["br", "work", "a", "b", "c"]},
            {"name": "work", "params": params(&["p", "q", "r"], "i64"),
             "ops": [["x", "add?", "p", "q"], ["y", "add?", "r", "p"],
                     ["z", "add?", "q", "x"]], "term": ["ok", "y"]}
        ]
    }]});
    let expansion = expand(&temp.path, &frame);
    assert!(
        expansion.obligations.is_empty(),
        "{:?}",
        expansion.obligations
    );
    let blocks = expansion.frame["fns"][0]["blocks"].as_array().unwrap();
    for (name, expected) in [
        (
            "work__x",
            json!([["x", "i64"], ["r", "i64"], ["p", "i64"], ["q", "i64"]]),
        ),
        ("work__y", json!([["y", "i64"], ["q", "i64"], ["x", "i64"]])),
        ("work__z", json!([["z", "i64"], ["y", "i64"]])),
    ] {
        let block = blocks.iter().find(|block| block["name"] == name).unwrap();
        assert_eq!(block["params"], expected, "{name}");
    }
    let mut runner = Runner::new(&temp.path, &frame);
    assert_eq!(
        runner.call("f", &[json!(1), json!(2), json!(3)]),
        json!({"Ok": 4})
    );
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
        &["/fns/0/blocks/0/term: block `done` takes 1 argument(s) (v: i64); this edge passes 2"],
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
            {"name": "entry", "ops": [["x", "add", "a", ["not", "a"]]], "term": ["return", "x"]}]),
        ),
        "AGENT_FRAME_INVALID",
        &[
            "/fns/0/blocks/0/ops/0/3: `not` takes bool operands; `a` is i64 [expanded /fns/0/blocks/0/ops/0]",
        ],
    );
    // An unknown name is the root cause, named where it is written.
    assert_refused(
        &temp.path,
        &one_function(
            "Result<i64,ArithmeticError>",
            &ab,
            &json!([
            {"name": "entry", "ops": [["x", "add", "a", ["field", "Nope.x", "a"]]], "term": ["return", "x"]}]),
        ),
        "AGENT_FRAME_INVALID",
        &["/fns/0/blocks/0/ops/0/3: no type `Nope`"],
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
        &[
            "/fns/0: the expanded function has at least 4200 operations, more than the bound of 4096",
        ],
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
        &[
            "/fns/0: the expanded function has at least 1030 generated blocks, more than the bound of 1024",
        ],
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
    // A frame without tests is the live program the ripple examples after
    // it change: it is committed in its own workspace.
    let live = workspace("help-afx-ripple");
    let examples: Vec<&str> = sley_agent::help::AFX
        .split("```json\n")
        .skip(1)
        .map(|rest| rest.split("```").next().unwrap())
        .collect();
    assert!(examples.len() >= 6);
    let mut ripples = 0;
    for (index, example) in examples.iter().enumerate() {
        let frame: Value = serde_json::from_str(example)
            .unwrap_or_else(|error| panic!("example {index} is not JSON: {error}"));
        if frame.get("test_tables").is_none() {
            let (status, text) = run(&live.path, &["try", &frame.to_string()]);
            assert_eq!(status, 0, "help afx example {index}: {text}");
            let (status, text) = run(&live.path, &["commit"]);
            assert_eq!(status, 0, "help afx example {index}: {text}");
            continue;
        }
        let dir = if frame.get("ripple").is_some() {
            ripples += 1;
            &live.path
        } else {
            &temp.path
        };
        let (status, text) = run(dir, &["try", &frame.to_string()]);
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
    assert_eq!(ripples, 2, "both enabled intents have an example");
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

#[test]
fn an_undefined_operand_of_a_checked_operation_is_named() {
    // `q` names nothing: the refusal says so at the operand, not that the
    // checked operation's type is unknown; a missing edge argument lists
    // the values of its type visible at the edge.
    let temp = workspace("undefined-operand");
    let frame = json!({"af1": 1, "afx": 1,
        "fns": [{"fn": "g", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
          "blocks": [{"name": "entry", "ops": [["b", "add?", "a", 1]], "term": ["br", "next"]},
                     {"name": "next", "params": [["c", "i64"], ["b", "i64"]],
                      "ops": [["d", "mul?", "c", "q"]], "term": ["ok", "d"]}]}]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.starts_with("error AGENT_X_SCOPE: /fns/0/blocks/1/ops/0/3: no value named `q`"),
        "{text}"
    );
    assert!(!text.contains("is not known here"), "{text}");
    let (_, draft) = run_json(&temp.path, &["draft", "d1", "--obligations"]);
    let edge = draft["obligations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["at"] == "/fns/0/blocks/0/term")
        .unwrap()
        .clone();
    assert_eq!(edge["expected"], "i64", "{edge}");
    assert_eq!(edge["available"], json!(["b: i64", "a: i64"]), "{edge}");
}

// ---------------------------------------------------------------------------
// Review regressions: loops, payloads, explicit qualification
// ---------------------------------------------------------------------------

/// `sum_evens(n)`: the sum of the even numbers up to `n`; `skip` ends with
/// `skip_term` (reproducer W2-A1).
fn sum_evens(skip_params: &Value, skip_edge: &Value, skip_term: &Value) -> Value {
    json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Overflow"]}],
      "fns": [{"fn": "sum_evens", "params": [["n", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["acc", "const", {"type": "i64", "value": 0}], ["i", "const", {"type": "i64", "value": 1}]],
         "term": ["br", "loop"]},
        {"name": "loop", "params": [["i", "i64"], ["acc", "i64"]], "ops": [["more", "le", "i", "n"]],
         "term": ["cond", "more", ["body", "i", "acc"], ["done", "acc"]]},
        {"name": "body", "params": [["k", "i64"], ["a", "i64"]], "ops": [["even", "eq", ["rem?Overflow", "k", 2], 0]],
         "term": ["cond", "even", ["addit", "k", "a"], skip_edge]},
        {"name": "addit", "params": [["j", "i64"], ["b", "i64"]], "ops": [["s", "add?Overflow", "b", "j"]],
         "term": ["br", "loop", ["add?Overflow", "j", 1], "s"]},
        {"name": "skip", "params": skip_params, "term": skip_term},
        {"name": "done", "params": [["r", "i64"]], "term": ["ok", "r"]}]}]})
}

#[test]
fn a_loop_edge_never_takes_a_shadowed_or_passed_through_value() {
    let temp = workspace("loops");
    let skip = json!(["skip", "k"]);
    // W2-A1: the omitted `acc` must not become the pre-loop `entry.acc`.
    assert_refused(
        &temp.path,
        &sum_evens(
            &json!([["m", "i64"]]),
            &skip,
            &json!(["br", "loop", ["add?Overflow", "m", 1]]),
        ),
        "AGENT_X_SCOPE",
        &[
            "/fns/0/blocks/4/term: block `loop` takes `acc: i64`, and this edge goes back into `loop`, which it is inside of (a loop edge), and a loop edge never takes an omitted argument by name: pass it explicitly",
        ],
    );
    // Written by name, `acc` is shadowed by the loop's own parameter: the
    // refusal names the exact alternatives instead of calling it ambiguous.
    let (symbol, detail) = refused(
        &temp.path,
        &sum_evens(
            &json!([["m", "i64"]]),
            &skip,
            &json!(["br", "loop", ["add?Overflow", "m", 1], "acc"]),
        ),
    );
    assert_eq!(symbol, "AGENT_X_SCOPE", "{detail}");
    assert!(
        detail.contains("/fns/0/blocks/4/term/3: `acc` here is shadowed: the nearest `acc` above `skip` is a parameter of block `loop`, visible only there; write `entry.acc` for the value `entry` defines, or declare `acc` as a parameter of `skip` and pass it on each edge"),
        "{detail}"
    );
    assert!(!detail.contains("ambiguous"), "{detail}");
    // Passing the loop's value on explicitly is the working program.
    let fixed = sum_evens(
        &json!([["m", "i64"], ["acc", "i64"]]),
        &json!(["skip", "k", "a"]),
        &json!(["br", "loop", ["add?Overflow", "m", 1], "acc"]),
    );
    let mut runner = Runner::new(&temp.path, &fixed);
    for (n, sum) in [(1, 0), (2, 2), (3, 2), (5, 6), (6, 12)] {
        assert_eq!(
            runner.call("sum_evens", &[json!(n)]),
            json!({"Ok": sum}),
            "{n}"
        );
    }
    // W2-A3: a body edge back to the loop header does not pass the body's
    // own copy of the header's value (which would never advance).
    let frame = json!({"af1": 1, "afx": 1, "types": [{"name": "SumError", "variant": ["Negative", "Overflow"]}],
      "fns": [{"fn": "sum_to2", "params": [["n", "i64"]], "returns": "Result<i64,SumError>", "blocks": [
        {"name": "entry", "ops": [["!Negative", "if", ["lt", "n", 0]]], "term": ["br", "loop", 0, 1]},
        {"name": "loop", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["more", "le", "i", "n"]], "term": ["cond", "more", "body", "done"]},
        {"name": "body", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["next", "add?Overflow", "acc", "i"]], "term": ["br", "loop", "next"]},
        {"name": "done", "params": [["acc", "i64"]], "term": ["ok", "acc"]}]}]});
    assert_refused(
        &temp.path,
        &frame,
        "AGENT_X_SCOPE",
        &[
            "/fns/0/blocks/2/term: block `loop` takes `i: i64`, and this edge goes back into `loop`, which it is inside of (a loop edge)",
        ],
    );
}

#[test]
fn a_plain_name_means_the_nearest_dominating_definition() {
    // `t` is defined by `entry` (which dominates `right`) and by `left`
    // (which does not): at `right` the name is unambiguous.
    let temp = workspace("nearest");
    let frame = json!({"af1": 1, "afx": 1, "fns": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "i64", "blocks": [
        {"name": "entry", "ops": [["t", "lt", "a", "b"]], "term": ["cond", "t", "left", "right"]},
        {"name": "left", "ops": [["t", "gt", "a", 0]], "term": ["cond", "t", ["out", "a"], ["out", "b"]]},
        {"name": "right", "term": ["cond", "t", ["out", 1], ["out", 2]]},
        {"name": "out", "params": [["v", "i64"]], "term": ["return", "v"]}]}]});
    let expansion = expand(&temp.path, &frame);
    assert!(
        expansion.obligations.is_empty(),
        "{:?}",
        expansion.obligations
    );
    assert_eq!(
        expansion.frame["fns"][0]["blocks"][2]["term"][1],
        json!("entry.t")
    );
    let mut runner = Runner::new(&temp.path, &frame);
    assert_eq!(runner.call("f", &[json!(5), json!(1)]), json!(2));
    assert_eq!(runner.call("f", &[json!(3), json!(4)]), json!(3));
    // When `left` redefines `t` on a path from `entry` to `join`, the plain
    // name would silently mean `entry.t` on that path too: refused, with
    // the exact alternatives.
    let join = |uses: &str, params: Value, edge: &str| {
        json!({"af1": 1, "afx": 1, "fns": [{"fn": "g", "params": [["a", "i64"], ["b", "i64"]], "returns": "bool", "blocks": [
            {"name": "entry", "ops": [["t", "lt", "a", "b"]], "term": ["cond", "t", "left", edge]},
            {"name": "left", "ops": [["t", "gt", "a", 0]], "term": ["br", "join"]},
            {"name": "right", "term": ["br", "join"]},
            {"name": "join", "params": params, "ops": [["u", "not", uses]], "term": ["return", "u"]}]}]})
    };
    let (symbol, detail) = refused(&temp.path, &join("t", json!([]), "right"));
    assert_eq!(symbol, "AGENT_X_SCOPE", "{detail}");
    assert!(
        detail.starts_with("/fns/0/blocks/3/ops/0/2: `t` here would be the result of `entry`, but block `left` also defines `t` on a path from `entry` to here: write `entry.t` for the value of `entry`, or declare `t` as a parameter of `join` and pass it on each edge"),
        "{detail}"
    );
    // The value of each path, as a parameter derived at each edge.
    let mut runner = Runner::new(&temp.path, &join("t", json!([["t", "bool"]]), "right"));
    assert_eq!(runner.call("g", &[json!(1), json!(5)]), json!(false));
    assert_eq!(runner.call("g", &[json!(-1), json!(5)]), json!(true));
    assert_eq!(runner.call("g", &[json!(5), json!(1)]), json!(true));
}

#[test]
fn a_case_payload_is_never_replaced_by_a_value_found_by_name() {
    // W2-A2: without `$` the Err or Some payload would be dropped unseen.
    let temp = workspace("payload-cases");
    let frame = |err_case: Value, some_case: Value| {
        json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": [["Math", "ArithmeticError"]]}], "fns": [
          {"fn": "ratio", "params": [["a", "i64"], ["b", "i64"], ["e", "ArithmeticError"]], "returns": "Result<i64,E>", "blocks": [
            {"name": "entry", "term": ["switch", ["div", "a", "b"], ["Ok", "done", "$"], err_case]},
            {"name": "done", "params": [["q", "i64"]], "term": ["ok", "q"]},
            {"name": "bad", "params": [["e", "ArithmeticError"]], "term": ["fail", "Math", "e"]}]},
          {"fn": "lookup", "params": [["m", "Map<i64,i64>"], ["k", "i64"], ["v", "i64"]], "returns": "i64", "blocks": [
            {"name": "entry", "term": ["switch", ["map_get", "m", "k"], some_case, ["None", "done"]]},
            {"name": "done", "params": [["v", "i64"]], "term": ["return", "v"]}]}]})
    };
    let (symbol, detail) = refused(
        &temp.path,
        &frame(json!(["Err", "bad"]), json!(["Some", "done"])),
    );
    assert_eq!(symbol, "AGENT_X_SCOPE", "{detail}");
    for needle in [
        "/fns/0/blocks/0/term/3: case `Err` carries a payload (ArithmeticError) that this edge does not pass, and `bad` takes more arguments than the edge gives: pass the payload with \"$\" where `bad` takes it (e.g. [\"Err\", \"bad\", \"$\"]), or write every argument",
        "/fns/1/blocks/0/term/2: case `Some` carries a payload (i64) that this edge does not pass",
    ] {
        assert!(detail.contains(needle), "{needle}\n{detail}");
    }
    // With `$` the payload is passed; the None case still derives `v`.
    let mut runner = Runner::new(
        &temp.path,
        &frame(json!(["Err", "bad", "$"]), json!(["Some", "done", "$"])),
    );
    assert_eq!(
        runner.call(
            "ratio",
            &[json!(1), json!(0), json!({"ArithmeticError": "Overflow"})]
        ),
        json!({"Err": {"Math": {"ArithmeticError": "DivideByZero"}}})
    );
    assert_eq!(
        runner.call("lookup", &[json!([[1, 5]]), json!(1), json!(0)]),
        json!(5)
    );
    assert_eq!(
        runner.call("lookup", &[json!([[1, 5]]), json!(2), json!(9)]),
        json!(9)
    );
}

#[test]
fn an_explicit_block_qualification_resolves_as_in_plain_af1() {
    // W2-A4: `mid.lo` inside `mid`, where `mid` defines no `lo`, is refused
    // as plain AF1 refuses it; it is never re-qualified to `entry.lo`.
    let temp = workspace("self-qualified");
    let blocks = json!([
        {"name": "entry", "ops": [["lo", "lt", "a", "b"]], "term": ["cond", "lo", "left", "right"]},
        {"name": "left", "ops": [["m", "add", "a", "a"]], "term": ["br", "mid"]},
        {"name": "right", "term": ["br", "mid"]},
        {"name": "mid", "term": ["br", "fin", "mid.lo"]},
        {"name": "fin", "params": [["flag", "bool"]], "term": ["cond", "flag", ["out", "a"], ["out", "b"]]},
        {"name": "out", "params": [["v", "i64"]], "term": ["return", "v"]}]);
    let plain = json!({"af1": 1, "fns": [{"fn": "pick", "params": [["a", "i64"], ["b", "i64"]], "returns": "i64", "blocks": blocks}]});
    let mut extended = plain.clone();
    extended["afx"] = json!(1);
    assert_eq!(
        expand(&temp.path, &extended).frame["fns"][0]["blocks"][3]["term"],
        json!(["br", "fin", "mid.lo"])
    );
    let (_, plain_detail) = refused(&temp.path, &plain);
    let (symbol, detail) = refused(&temp.path, &extended);
    assert_eq!(symbol, "AGENT_FRAME_INVALID", "{detail}");
    assert_eq!(detail, plain_detail);
    assert!(detail.contains("mid.lo` in scope"), "{detail}");
}

#[test]
fn a_root_cause_is_reported_before_the_literals_it_leaves_untyped() {
    let temp = workspace("root-causes");
    // W2-V3: a misspelled name, not the literal it leaves without a type.
    let (symbol, detail) = refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Neg"]}],
          "fns": [{"fn": "n10", "params": [["a", "i64"]], "returns": "Result<i64,E>",
            "blocks": [{"name": "entry", "ops": [["!Neg", "if", ["lt", "aa", 0]]], "term": ["ok", "a"]}]}]}),
    );
    assert_eq!(symbol, "AGENT_X_SCOPE", "{detail}");
    assert_eq!(
        detail,
        "/fns/0/blocks/0/ops/0/2/1: no value named `aa` in this function"
    );
    let (symbol, detail) = refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [{"fn": "n9", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
            "blocks": [{"name": "entry", "ops": [["x", "add?", "a", "zz"]], "term": ["ok", "x"]}]}]}),
    );
    assert_eq!(symbol, "AGENT_X_SCOPE", "{detail}");
    assert!(
        detail.starts_with("/fns/0/blocks/0/ops/0/3: no value named `zz` in this function"),
        "{detail}"
    );
    assert!(!detail.contains("not known here"), "{detail}");
    // A surplus edge argument: the arity, as for a named argument, not the
    // type of a literal that has no parameter.
    for (term, at) in [
        (
            json!(["cond", "c", ["t", "a", 5], ["t", "a"]]),
            "/fns/0/blocks/0/term/2",
        ),
        (json!(["br", "t", "a", 5]), "/fns/0/blocks/0/term"),
        (
            json!(["switch", ["lt", "a", 0], ["true", "t", "a", 5]]),
            "/fns/0/blocks/0/term/2",
        ),
    ] {
        let frame = json!({"af1": 1, "afx": 1, "fns": [{"fn": "f", "params": [["a", "i64"]], "returns": "i64",
            "blocks": [{"name": "entry", "ops": [["c", "lt", "a", "a"]], "term": term},
                       {"name": "t", "params": [["x", "i64"]], "term": ["return", "x"]}]}]});
        let (symbol, detail) = refused(&temp.path, &frame);
        assert_eq!(symbol, "AGENT_FRAME_INVALID", "{detail}");
        assert!(
            detail.starts_with(&format!(
                "{at}: block `t` takes 1 argument(s) (x: i64); this edge passes 2"
            )),
            "{term}: {detail}"
        );
        assert!(!detail.contains("literal"), "{term}: {detail}");
    }
}

// ---------------------------------------------------------------------------
// Review regressions: patches recognize generated blocks by their shape
// ---------------------------------------------------------------------------

fn commit_frame(dir: &Path, frame: &Value) {
    let (status, text) = run(dir, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    let (status, text) = run(dir, &["commit"]);
    assert_eq!(status, 0, "{text}");
}

fn patch_blocks(dir: &Path, patch: &Value) -> Value {
    let expansion = expand(dir, patch);
    assert!(
        expansion.obligations.is_empty(),
        "{:?}",
        expansion.obligations
    );
    expansion.frame["patch"][0]["blocks"].clone()
}

#[test]
fn restating_a_block_deletes_its_shortened_pieces() {
    // W2-A5: a continuation whose name was shortened is still recognized.
    let temp = workspace("shortened");
    let long = "block_with_a_very_long_descriptive_name_for_the_checked_step";
    commit_frame(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Overflow"]}],
          "fns": [{"fn": "g", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,E>", "blocks": [
            {"name": "entry", "term": ["br", long]},
            {"name": long, "ops": [["quotient", "div?Overflow", "a", "b"]], "term": ["ok", "quotient"]}]}]}),
    );
    let (_, view) = run(&temp.path, &["view", "g"]);
    let old: Vec<String> = block_names(&view)
        .into_iter()
        .filter(|name| name.contains("__h"))
        .collect();
    assert_eq!(old.len(), 1, "{view}");
    let patch = json!({"af1": 1, "afx": 1, "patch": [{"fn": "g", "blocks": {
        long: {"ops": [["product", "mul?Overflow", "a", "b"]], "term": ["ok", "product"]}}}]});
    let blocks = patch_blocks(&temp.path, &patch);
    assert_eq!(blocks[&old[0]], Value::Null, "{blocks}");
    let (status, text) = run(&temp.path, &["try", &patch.to_string()]);
    assert_eq!(status, 0, "{text}");
    let (_, value) = run(&temp.path, &["call", "g", "6", "7", "--on", "latest"]);
    assert_eq!(value.trim(), "{\"Ok\":42}");
}

#[test]
fn a_patch_never_rewrites_or_deletes_blocks_it_did_not_generate() {
    // W2-A6: a plain AF1 block named like a shared exit, and one named like
    // a piece, are the author's: kept as they are.
    let temp = workspace("user-blocks");
    commit_frame(
        &temp.path,
        &json!({"af1": 1, "types": [{"name": "E", "variant": ["Zero", "Other"]}],
          "fns": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,E>", "blocks": [
            {"name": "entry", "ops": [["neg", "lt", "a", "b"]], "term": ["cond", "neg", "k", "__fail_Zero"]},
            {"name": "k", "ops": [["r", "ok", "a"]], "term": ["return", "r"]},
            {"name": "__fail_Zero", "ops": [["v", "variant", "E.Other"], ["r", "err", "v"]], "term": ["return", "r"]}]},
          {"fn": "h", "params": [["a", "i64"]], "returns": "Result<i64,E>", "blocks": [
            {"name": "entry", "ops": [["zero", "const", 0], ["pos", "gt", "a", "zero"]], "term": ["cond", "pos", "k", "entry__pos"]},
            {"name": "k", "ops": [["r", "ok", "a"]], "term": ["return", "r"]},
            {"name": "entry__pos", "ops": [["v", "variant", "E.Other"], ["r", "err", "v"]], "term": ["return", "r"]}]}]}),
    );
    let (_, before) = run(&temp.path, &["call", "f", "5", "1"]);
    assert_eq!(before.trim(), "{\"Err\":\"Other\"}");
    let patch = json!({"af1": 1, "afx": 1, "patch": [
        {"fn": "f", "blocks": {"k": {"ops": [["!Zero", "if", ["eq", "a", 0]]], "term": ["ok", "a"]}}},
        {"fn": "h", "blocks": {"entry": {"ops": [["!Zero", "if", ["eq", "a", 0]], ["pos", "gt", "a", 0]],
                                         "term": ["cond", "pos", "k", "entry__pos"]}}}]});
    let expansion = expand(&temp.path, &patch);
    assert!(
        expansion.obligations.is_empty(),
        "{:?}",
        expansion.obligations
    );
    let f_blocks = &expansion.frame["patch"][0]["blocks"];
    assert!(f_blocks.get("__fail_Zero").is_none(), "{f_blocks}");
    assert!(f_blocks["__fail_Zero_2"].is_object(), "{f_blocks}");
    assert_eq!(
        f_blocks["k"]["term"][2],
        json!("__fail_Zero_2"),
        "{f_blocks}"
    );
    let h_blocks = &expansion.frame["patch"][1]["blocks"];
    assert!(h_blocks.get("entry__pos").is_none(), "{h_blocks}");
    let (status, text) = run(&temp.path, &["try", &patch.to_string()]);
    assert_eq!(status, 0, "{text}");
    for (function, args, want) in [
        ("f", ["5", "1"], "{\"Err\":\"Other\"}"),
        ("f", ["0", "1"], "{\"Err\":\"Zero\"}"),
        ("f", ["1", "5"], "{\"Ok\":1}"),
        ("h", ["-3", "0"], "{\"Err\":\"Other\"}"),
        ("h", ["0", "0"], "{\"Err\":\"Zero\"}"),
        ("h", ["4", "0"], "{\"Ok\":4}"),
    ] {
        let mut words = vec!["call", function, args[0]];
        if function == "f" {
            words.push(args[1]);
        }
        words.extend(["--on", "latest"]);
        let (_, value) = run(&temp.path, &words);
        assert_eq!(value.trim(), want, "{function}{args:?}");
    }
}

#[test]
fn a_suffixed_shared_exit_is_recognized_and_deleted_when_unused() {
    // W2-P2: with a top-level `__err`, the exit is `__err_2`; a later patch
    // that no longer needs it deletes it.
    let temp = workspace("suffixed-exit");
    commit_frame(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [
          {"fn": "__err", "params": [], "returns": "i64", "blocks": [{"name": "entry", "ops": [["z", "const", 0]], "term": ["return", "z"]}]},
          {"fn": "g", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>", "blocks": [
            {"name": "entry", "ops": [["s", "add?", "a", 1]], "term": ["ok", "s"]}]}]}),
    );
    let (_, view) = run(&temp.path, &["view", "g"]);
    assert!(block_names(&view).contains(&"__err_2".to_owned()), "{view}");
    let patch = json!({"af1": 1, "afx": 1, "patch": [{"fn": "g", "blocks": {
        "entry": {"ops": [["s", "add", "a", 1]], "term": ["return", "s"]}}}]});
    let blocks = patch_blocks(&temp.path, &patch);
    assert_eq!(blocks["__err_2"], Value::Null, "{blocks}");
    assert_eq!(blocks["entry__s"], Value::Null, "{blocks}");
    commit_frame(&temp.path, &patch);
    let (_, view) = run(&temp.path, &["view", "g"]);
    assert_eq!(block_names(&view), ["entry"], "{view}");
}

#[test]
fn generated_block_and_value_names_have_separate_tables() {
    // W2-P1: the exit condition value and the block after the exit share
    // the name `entry__if0`; each table maps it to its own construct.
    let temp = workspace("name-tables");
    let frame = json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Neg"]}],
      "fns": [{"fn": "f", "params": [["a", "i64"]], "returns": "Result<i64,E>",
        "blocks": [{"name": "entry", "ops": [["!Neg", "if", ["lt", "a", 0]]], "term": ["ok", "a"]}]}]});
    let map = expand(&temp.path, &frame).map.to_json();
    assert_eq!(
        map["values"]["f"]["entry__if0"],
        json!("/fns/0/blocks/0/ops/0/2")
    );
    assert_eq!(
        map["blocks"]["f"]["entry__if0"],
        json!("/fns/0/blocks/0/ops/0")
    );
    assert_eq!(
        map["blocks"]["f"]["__fail_Neg"],
        json!("/fns/0/blocks/0/ops/0")
    );
    assert!(map["values"]["f"].get("__fail_Neg").is_none());
}

// ---------------------------------------------------------------------------
// Review regressions: locators and diagnostics through the source map
// ---------------------------------------------------------------------------

fn authored_line(text: &str) -> String {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("authored: "))
        .unwrap_or_else(|| panic!("no authored line: {text}"))
        .to_owned()
}

#[test]
fn kernel_locators_in_generated_blocks_name_the_authored_construct() {
    let temp = workspace("locators");
    let neg = json!([{"name": "E", "variant": ["Neg"]}]);
    // W2-V1a: a return in the piece after an exit is the authored return.
    let frame = json!({"af1": 1, "afx": 1, "types": neg,
      "fns": [{"fn": "flagx", "params": [["a", "i64"]], "returns": "Result<bool,E>",
        "blocks": [{"name": "entry", "ops": [["!Neg", "if", ["lt", "a", 0]], ["s", "add", "a", "a"]], "term": ["return", "s"]}]}]});
    let (status, text) = run(&temp.path, &["try", "--no-test", &frame.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert_eq!(
        authored_line(&text),
        "/fns/0/blocks/0/term (terminator of flagx.entry__if0), /fns/0/returns (result of flagx)"
    );
    // W2-V1b: the generated `cond` of an exit is the exit operation, not
    // the block's authored terminator.
    let frame = json!({"af1": 1, "afx": 1, "types": neg,
      "fns": [{"fn": "domx", "params": [["a", "i64"]], "returns": "Result<i64,E>",
        "blocks": [{"name": "entry", "ops": [["z", "const", {"type": "i64", "value": 0}], ["c0", "lt", "a", "z"]], "term": ["cond", "c0", "left", "right"]},
                   {"name": "left", "ops": [["c", "gt", "a", "z"]], "term": ["br", "right"]},
                   {"name": "right", "ops": [["!Neg", "if", "left.c"]], "term": ["ok", "a"]}]}]});
    let (status, text) = run(&temp.path, &["try", "--no-test", &frame.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert_eq!(
        authored_line(&text),
        "/fns/0/blocks/2/ops/0 (the exit that ends domx.right)"
    );
    // W2-V2: a call in a continuation piece keeps its operation pointer.
    let frame = json!({"af1": 1, "afx": 1, "types": neg,
      "fns": [{"fn": "g", "params": [["x", "bool"]], "returns": "bool", "blocks": [{"name": "entry", "ops": [], "term": ["return", "x"]}]},
              {"fn": "hx", "params": [["a", "i64"]], "returns": "Result<bool,E>",
        "blocks": [{"name": "entry", "ops": [["!Neg", "if", ["lt", "a", 0]], ["r", "call", "g", "a"]], "term": ["ok", "r"]}]}]});
    let (status, text) = run(&temp.path, &["try", "--no-test", &frame.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert_eq!(
        authored_line(&text),
        "/fns/1/blocks/0/ops/1 (hx.entry__if0.r), /fns/0/params (parameters of g)"
    );
}

#[test]
fn a_pointer_inside_a_problem_detail_is_mapped_too() {
    // W2-V4: the detail's own (expanded) pointer is not left behind.
    let temp = workspace("embedded-pointer");
    let (_, detail) = refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Neg"]}],
          "fns": [{"fn": "p3", "params": [["a", "u8"]], "returns": "Result<u8,E>",
            "blocks": [{"name": "entry", "ops": [["!Neg", "if", ["lt", "a", 1]], ["s", "add?Neg", "a", 300]], "term": ["ok", "s"]}]}]}),
    );
    assert_eq!(
        detail,
        "/fns/0/blocks/0/ops/1/3: 300 does not fit u8 [expanded /fns/0/blocks/1/ops/0]"
    );
    let (_, detail) = refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [{"fn": "idt", "params": [["a", "i64"]], "returns": "i64",
            "blocks": [{"name": "entry", "term": ["return", "a"]}]}],
          "test_tables": [{"name": "t_x", "fn": "idt", "cases": [{"args": [1], "expect": 1}, {"args": [true], "expect": 1}]}]}),
    );
    assert_eq!(
        detail,
        "/test_tables/0/cases/1/args/0: expected an integer [expanded /tests/1/args/0]"
    );
}

// ---------------------------------------------------------------------------
// Review regression: expansion time
// ---------------------------------------------------------------------------

/// A chain of `n` blocks listed opposite to dominance order, each using the
/// value of the block before it by plain name (reproducer W2-A7).
fn reverse_chain(n: usize) -> Value {
    let blocks: Vec<Value> = (0..n)
        .map(|i| {
            let ops = if i == n - 1 {
                json!([[format!("v{i}"), "eq", "a", "a"]])
            } else {
                let previous = format!("v{}", i + 1);
                json!([[format!("v{i}"), "eq", previous, previous]])
            };
            let term = if i == 0 {
                json!(["return", "v0"])
            } else {
                json!(["br", format!("b{}", i - 1)])
            };
            json!({"name": format!("b{i}"), "ops": ops, "term": term})
        })
        .collect();
    json!({"af1": 1, "afx": 1, "fns": [{"fn": "h", "params": [["a", "i64"]], "returns": "bool",
        "entry": format!("b{}", n - 1), "blocks": blocks}]})
}

#[test]
fn expansion_time_stays_near_linear() {
    let temp = workspace("scale");
    let head = Workspace::at(&temp.path).head().unwrap();
    let names = names_of(&temp.path, head.program());
    for n in [250, 1000, 4000] {
        let frame = reverse_chain(n);
        let started = std::time::Instant::now();
        let expansion = sley_agent::afx::expand(head.program(), &names, &frame).unwrap();
        let elapsed = started.elapsed();
        assert!(
            expansion.obligations.is_empty(),
            "{:?}",
            expansion.obligations
        );
        assert_eq!(
            expansion.frame["fns"][0]["blocks"][0]["ops"][0],
            json!(["v0", "eq", "b1.v1", "b1.v1"])
        );
        eprintln!("reverse chain of {n} blocks: {elapsed:?}");
        assert!(elapsed.as_millis() < 1000, "{n} blocks took {elapsed:?}");
    }
    // One block of 1,000 checked operations that each read a block
    // parameter: 1,000 pieces, the parameter threaded through all of them.
    let ops: Vec<Value> = (0..1000)
        .map(|i| {
            let previous = if i == 0 {
                "p".to_owned()
            } else {
                format!("x{}", i - 1)
            };
            json!([format!("x{i}"), "add?", previous, "p"])
        })
        .collect();
    let frame = json!({"af1": 1, "afx": 1, "fns": [{"fn": "w", "params": [["a", "i64"]],
        "returns": "Result<i64,ArithmeticError>", "blocks": [
          {"name": "entry", "term": ["br", "work", "a"]},
          {"name": "work", "params": [["p", "i64"]], "ops": ops, "term": ["ok", "x999"]}]}]});
    let started = std::time::Instant::now();
    let expansion = sley_agent::afx::expand(head.program(), &names, &frame).unwrap();
    let elapsed = started.elapsed();
    assert!(
        expansion.obligations.is_empty(),
        "{:?}",
        expansion.obligations
    );
    eprintln!("1,000 checked operations in one block: {elapsed:?}");
    assert!(elapsed.as_millis() < 1000, "took {elapsed:?}");
    // A bound is refused before the expensive passes.
    let ops: Vec<Value> = (0..5000)
        .map(|i| json!([format!("x{i}"), "eq", "a", "a"]))
        .collect();
    let frame = json!({"af1": 1, "afx": 1, "fns": [{"fn": "w", "params": [["a", "i64"]], "returns": "bool",
        "blocks": [{"name": "entry", "ops": ops, "term": ["return", "x0"]}]}]});
    let started = std::time::Instant::now();
    let expansion = sley_agent::afx::expand(head.program(), &names, &frame).unwrap();
    assert!(started.elapsed().as_millis() < 1000);
    assert_eq!(
        expansion.obligations[0].symbol,
        sley_agent::AgentErrorCode::XLimit
    );
    assert!(
        expansion.obligations[0]
            .decision
            .starts_with("the expanded function has at least 5000 operations"),
        "{:?}",
        expansion.obligations
    );
}

// ---------------------------------------------------------------------------
// Wave-3 regressions
// ---------------------------------------------------------------------------

#[test]
fn an_unknown_type_callee_or_constant_is_the_reported_root_cause() {
    // W3-A1: named as plain AF1 names it, with none of the obligations it
    // would cause (an unknown failure route, "does not return a Result",
    // an untyped literal).
    let temp = workspace("unknown-entities");
    let e = json!([{"name": "E", "variant": ["Ov", "Bad", ["Code", "i64"]]}]);
    let f = |types: &Value, returns: &str, blocks: Value| {
        json!({"af1": 1, "afx": 1, "types": types,
               "fns": [{"fn": "f", "params": [["a", "i64"]], "returns": returns, "blocks": blocks}]})
    };
    let simple = json!([{"name": "entry", "ops": [["!Ov", "if", ["lt", "a", 0]], ["x", "add?Ov", "a", 1]], "term": ["ok", "x"]}]);
    for (frame, want) in [
        (
            f(&json!([]), "Result<i64,E>", simple.clone()),
            "/fns/0/returns: unknown type `E`",
        ),
        (
            f(
                &json!([{"name": "MathError", "variant": ["Ov"]}]),
                "Result<i64,MathErr>",
                simple.clone(),
            ),
            "/fns/0/returns: unknown type `MathErr`",
        ),
        (
            f(&e, "Result<Amount,E>", simple),
            "/fns/0/returns: unknown type `Amount`",
        ),
        (
            f(
                &e,
                "Result<i64,E>",
                json!([{"name": "entry", "term": ["br", "nx", 1]},
                {"name": "nx", "params": [["q", "Amt"]], "ops": [["x", "add?Ov", "a", 1]], "term": ["ok", "x"]}]),
            ),
            "/fns/0/blocks/1/params/0: unknown type `Amt`",
        ),
        (
            f(
                &e,
                "Result<i64,E>",
                json!([{"name": "entry", "ops": [["x", "call?Bad", "gg", ["add?Ov", "a", 1]]], "term": ["ok", ["mul?Ov", "x", 2]]}]),
            ),
            "/fns/0/blocks/0/ops/0: no Function named `gg`",
        ),
        (
            f(
                &e,
                "Result<i64,E>",
                json!([{"name": "entry", "ops": [["v", "variant", "Shape.Circle", 1], ["x", "add?Ov", "a", 1]], "term": ["ok", "x"]}]),
            ),
            "/fns/0/blocks/0/ops/0: no type `Shape`",
        ),
        (
            f(
                &e,
                "Result<i64,E>",
                json!([{"name": "entry", "ops": [["v", "variant", "E.Nope"], ["x", "add?Ov", "a", 1]], "term": ["ok", "x"]}]),
            ),
            "/fns/0/blocks/0/ops/0: type `E` has no member `Nope`",
        ),
        (
            f(
                &e,
                "Result<i64,E>",
                json!([{"name": "entry", "ops": [["k", "const", "LIMIT"], ["x", "add?Ov", "a", "k"]], "term": ["ok", ["mul?Ov", "x", 2]]}]),
            ),
            "/fns/0/blocks/0/ops/0: no Constant named `LIMIT`",
        ),
        (
            f(
                &e,
                "Result<i64,E>",
                json!([{"name": "entry", "ops": [["x", "add?Ov", "a", {"type": "Int", "value": 1}]], "term": ["ok", "x"]}]),
            ),
            "/fns/0/blocks/0/ops/0/3: unknown type `Int`",
        ),
        (
            f(
                &json!([{"name": "E", "variant": ["Ov", ["Code", "Amt"]]}]),
                "Result<i64,E>",
                json!([{"name": "entry", "ops": [["x", "add?Code", "a", 1]], "term": ["ok", "x"]}]),
            ),
            "/types/0/variant/1: unknown type `Amt`",
        ),
    ] {
        let (symbol, detail) = refused(&temp.path, &frame);
        assert_eq!(symbol, "AGENT_FRAME_INVALID", "{frame}: {detail}");
        assert_eq!(detail, want, "{frame}");
    }
}

#[test]
fn a_trap_never_hides_content_or_relaxes_the_loop_edge_rule() {
    let temp = workspace("strict-trap");
    // W3-A2: an extra trap item used to put the function in a lenient mode
    // that skipped the loop-edge rule (the W2-A3 loop that never advances).
    let frame = json!({"af1": 1, "afx": 1, "types": [{"name": "SumError", "variant": ["Negative", "Overflow"]}],
      "fns": [{"fn": "sum_to2", "params": [["n", "i64"]], "returns": "Result<i64,SumError>", "blocks": [
        {"name": "entry", "ops": [["!Negative", "if", ["lt", "n", 0]]], "term": ["br", "loop", 0, 1]},
        {"name": "loop", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["more", "le", "i", "n"]], "term": ["cond", "more", "body", "done"]},
        {"name": "body", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["next", "add?Overflow", "acc", "i"]], "term": ["br", "loop", "next"]},
        {"name": "done", "params": [["acc", "i64"]], "term": ["ok", "acc"]},
        {"name": "never", "unreachable": true, "term": ["trap", "unreachable", "n", "ignored"]}]}]});
    // The trap block uses no dialect form, so it keeps plain AF1's reading
    // (W4-A1); the loop-edge rule holds regardless.
    let (symbol, detail) = refused(&temp.path, &frame);
    assert_eq!(symbol, "AGENT_X_SCOPE", "{detail}");
    assert!(
        detail.starts_with(
            "/fns/0/blocks/2/term: block `loop` takes `i: i64`, and this edge goes back into `loop`"
        ),
        "{detail}"
    );
    // In a block that uses the dialect the same trap is refused.
    let mut strict = frame.clone();
    strict["fns"][0]["blocks"][4] = json!({"name": "never", "unreachable": true,
        "ops": [["m", "add", "n", 1]], "term": ["trap", "unreachable", "m", "ignored"]});
    let (_, detail) = refused(&temp.path, &strict);
    assert!(
        detail.contains("/fns/0/blocks/4/term: `trap` takes [\"trap\"], [\"trap\", code] or [\"trap\", code, payload], not 4 items"),
        "{detail}"
    );
    // W3-DOC8: an operation where the code goes is refused, not dropped; a
    // nested payload is lowered and kept.
    let trap = |term: Value| {
        json!({"af1": 1, "afx": 1, "fns": [{"fn": "tt", "params": [["a", "i64"]], "returns": "i64",
            "blocks": [{"name": "entry", "term": term}]}]})
    };
    assert_refused(
        &temp.path,
        &trap(json!(["trap", ["add", "a", 1]])),
        "AGENT_FRAME_INVALID",
        &["/fns/0/blocks/0/term/1: a trap code is a word"],
    );
    let expansion = expand(
        &temp.path,
        &trap(json!(["trap", "unreachable", ["add", "a", 1]])),
    );
    assert!(
        expansion.obligations.is_empty(),
        "{:?}",
        expansion.obligations
    );
    let block = &expansion.frame["fns"][0]["blocks"][0];
    assert_eq!(block["ops"][1][0], json!("entry__t0"), "{block}");
    assert_eq!(
        block["term"],
        json!(["trap", "unreachable", "entry__t0"]),
        "{block}"
    );
    // Plain AF1 keeps its behavior.
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            &json!({"af1": 1, "fns": [{"fn": "tp", "params": [["a", "i64"]], "returns": "i64",
            "blocks": [{"name": "entry", "term": ["trap", "unreachable", "a", "ignored"]}]}]})
            .to_string(),
        ],
    );
    assert_ne!(status, 2, "{text}");
}

#[test]
fn a_dialect_follow_up_keeps_the_plain_names_of_its_base() {
    // W3-D2: tests added as a table on top of a plain draft whose blocks
    // are named `is__neg` and `not__neg`.
    let temp = workspace("plain-base");
    let plain = json!({"af1": 1, "fns": [{"fn": "neg", "params": [["a", "i64"]], "returns": "bool", "blocks": [
        {"name": "entry", "ops": [["z", "const", {"type": "i64", "value": 0}], ["c", "lt", "a", "z"]], "term": ["cond", "c", "is__neg", "not__neg"]},
        {"name": "is__neg", "ops": [["t", "eq", "entry.z", "entry.z"]], "term": ["return", "t"]},
        {"name": "not__neg", "ops": [["f", "ne", "entry.z", "entry.z"]], "term": ["return", "f"]}]}]});
    let (status, text) = run(&temp.path, &["try", &plain.to_string()]);
    assert_eq!(status, 0, "{text}");
    let follow_up = json!({"af1": 1, "afx": 1, "test_tables": [{"name": "t", "fn": "neg",
        "cases": [{"args": [-1], "expect": true}, {"args": [1], "expect": false}]}]});
    let (status, text) = run(&temp.path, &["try", "--on", "d1", &follow_up.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("tests: 2/2 passed"), "{text}");
}

#[test]
fn an_edit_of_an_expanded_operation_names_where_it_lives() {
    // W3-A3: after a commit, `b` lives in a generated block.
    let temp = workspace("edit-expanded");
    commit_frame(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "ShapeError", "variant": ["BadSide", "Overflow"]}],
          "fns": [{"fn": "area", "params": [["w", "i64"], ["h", "i64"]], "returns": "Result<i64,ShapeError>",
            "blocks": [{"name": "entry", "ops": [["!BadSide", "if", ["lt", "w", 1]], ["a", "mul?Overflow", "w", "h"], ["b", "add?Overflow", "a", 1]],
                        "term": ["ok", "b"]}]}]}),
    );
    let (symbol, detail) = refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "edit": [{"fn": "area", "replace_op": "entry.b", "with": ["mul", "a", "h"]}]}),
    );
    assert_eq!(symbol, "AGENT_FRAME_INVALID", "{detail}");
    assert!(
        detail.starts_with("/edit/0: no operation `b` in `entry`; the authoring dialect's expansion holds it as `entry__a.b__r` (block `entry` was split into generated blocks): to change it, restate block `entry` with patch"),
        "{detail}"
    );
    // A name that is nowhere keeps the plain refusal.
    let (_, detail) = refused(
        &temp.path,
        &json!({"af1": 1, "edit": [{"fn": "area", "replace_op": "entry.zz", "with": ["mul", "w", "h"]}]}),
    );
    assert_eq!(detail, "/edit/0: no operation `zz` in `entry`");
}

#[test]
fn a_kernel_refusal_analyzes_its_function_once_and_names_its_frame() {
    let temp = workspace("refusal-analysis");
    // W3-RP1: the locator detail, the authored positions and the other
    // findings share one analysis of the refused function.
    let frame = json!({"af1": 1, "fns": [{"fn": "f", "params": [["a", "i64"], ["c", "bool"]], "returns": "bool", "blocks": [
        {"name": "entry", "ops": [["x", "lt", "a", "a"]], "term": ["cond", "c", "l", "r"]},
        {"name": "l", "ops": [["y", "not", "entry.x"]], "term": ["return", "y"]},
        {"name": "r", "term": ["return", "l.y"]}]}]});
    let before = sley_agent::explain::analyses_run();
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert!(text.contains("CFG_DOMINANCE"), "{text}");
    assert!(
        text.contains("  authored: /fns/0/blocks/2/term (terminator of f.r)\n"),
        "{text}"
    );
    assert_eq!(sley_agent::explain::analyses_run() - before, 1, "{text}");
    // W3-L1: for a layered revision the authored pointers name the frame
    // they index, as a frame refusal's pointers do.
    let base = json!({"af1": 1, "afx": 1, "fns": [{"fn": "g", "params": [["a", "i64"], ["c", "bool"]], "returns": "bool", "blocks": [
        {"name": "entry", "ops": [["x", "lt", "a", 0]], "term": ["cond", "c", "l", "r"]},
        {"name": "l", "ops": [["y", "not", "x"]], "term": ["return", "y"]},
        {"name": "r", "term": ["return", "x"]}]}]});
    let (status, text) = run(&temp.path, &["try", &base.to_string()]);
    assert_eq!(status, 0, "{text}");
    let draft = text
        .split_whitespace()
        .skip_while(|word| *word != "draft")
        .nth(1)
        .unwrap()
        .split('@')
        .next()
        .unwrap()
        .to_owned();
    let patch = json!({"af1": 1, "afx": 1, "patch": [{"fn": "g", "blocks": {"r": {"term": ["return", "l.y"]}}}]});
    let (status, text) = run(&temp.path, &["try", "--on", &draft, &patch.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert!(
        text.contains(&format!(
            "  authored: /fns/0/blocks/2/term (terminator of g.r); pointers refer to .sley/drafts/{draft}/r2/frame.json\n"
        )),
        "{text}"
    );
    let (_, value) = run_json(&temp.path, &["explain", "latest"]);
    assert_eq!(
        value["verdict"]["authored_frame"],
        json!(format!(".sley/drafts/{draft}/r2/frame.json")),
        "{value}"
    );
}

#[test]
fn a_plain_block_keeps_plain_af1_trap_reading_under_a_dialect_follow_up() {
    // W4-A1: a plain draft whose block ends ["trap", 5], then tests added
    // as a table: the untouched plain block is not held to the dialect.
    let temp = workspace("plain-trap");
    let plain = json!({"af1": 1, "fns": [{"fn": "p1", "params": [["a", "i64"]], "returns": "i64", "blocks": [
        {"name": "entry", "ops": [["z", "const", {"type": "i64", "value": 0}], ["c", "lt", "a", "z"]], "term": ["cond", "c", "bad", "good"]},
        {"name": "bad", "term": ["trap", 5]},
        {"name": "good", "term": ["return", "a"]}]}]});
    let (status, text) = run(&temp.path, &["try", &plain.to_string()]);
    assert_eq!(status, 0, "{text}");
    let follow_up = json!({"af1": 1, "afx": 1, "test_tables": [{"name": "t", "fn": "p1",
        "cases": [{"args": [3], "expect": 3}]}]});
    let (status, text) = run(&temp.path, &["try", "--on", "d1", &follow_up.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("tests: 1/1 passed"), "{text}");
    // A block that uses the dialect is held to the strict form.
    let mut extended = plain.clone();
    extended["afx"] = json!(1);
    extended["fns"][0]["blocks"][1] =
        json!({"name": "bad", "ops": [["m", "add", "a", 1]], "term": ["trap", 5]});
    assert_refused(
        &temp.path,
        &extended,
        "AGENT_FRAME_INVALID",
        &["/fns/0/blocks/1/term/1: a trap code is a word"],
    );
}

#[test]
fn a_fail_case_refusal_names_the_function() {
    // W4-A2: the function's name, not a placeholder.
    let temp = workspace("fail-no-variant");
    let (symbol, detail) = refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [{"fn": "o2", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
            "blocks": [{"name": "entry", "term": ["fail", "X"]}]}]}),
    );
    assert_eq!(symbol, "AGENT_X_PROPAGATION", "{detail}");
    assert_eq!(
        detail,
        "/fns/0/blocks/0/term: `X` is not a case of the error type of `o2` (`o2` has no variant error type)"
    );
}

// ---------------------------------------------------------------------------
// Review regressions: names written like a function parameter
// ---------------------------------------------------------------------------

#[test]
fn a_piece_whose_value_shadows_a_function_parameter_is_still_a_piece() {
    // W5-AFX-6: the continuation `entry__x` takes `x`, which view renders
    // `x_<hex>` beside the function parameter `x`; restating `entry` still
    // recognizes and replaces it.
    let temp = workspace("shadow-piece");
    commit_frame(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Big"]}],
          "fns": [{"fn": "fx", "params": [["a", "i64"], ["x", "i64"]], "returns": "Result<i64,E>", "blocks": [
            {"name": "entry", "ops": [["x", "add?Big", "a", 1]], "term": ["ok", "x"]}]}]}),
    );
    let (_, view) = run(&temp.path, &["view", "fx"]);
    assert!(view.contains("entry__x(x_"), "{view}");
    for value in ["x", "z"] {
        let patch = json!({"af1": 1, "afx": 1, "patch": [{"fn": "fx", "blocks": {
            "entry": {"ops": [[value, "add?Big", "a", 2]], "term": ["ok", value]}}}]});
        let blocks = patch_blocks(&temp.path, &patch);
        if value == "z" {
            assert_eq!(blocks["entry__x"], Value::Null, "{blocks}");
        }
        let (status, text) = run(&temp.path, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(status, 0, "{text}");
        let (_, result) = run(&temp.path, &["call", "fx", "5", "100", "--on", "latest"]);
        assert_eq!(result.trim(), "{\"Ok\":7}", "{value}");
    }
}

#[test]
fn a_kept_block_parameter_is_derived_by_the_name_it_was_written_with() {
    // W5-AFX-3: after a commit, view renders the handler's `a` as `a_<hex>`
    // (it shares the function parameter's name); an edge into the kept
    // handler still derives `a`, as before the commit.
    let temp = workspace("shadow-kept");
    commit_frame(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": [["Code", "i64"]]}],
          "fns": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,E>", "blocks": [
            {"name": "entry", "ops": [["x", "mul?h", "a", "b"]], "term": ["ok", "x"]},
            {"name": "h", "params": [["e", "ArithmeticError"], ["a", "i64"]], "term": ["fail", "Code", "a"]}]}]}),
    );
    let (_, view) = run(&temp.path, &["view", "f"]);
    assert!(view.contains("h(e: ArithmeticError, a_"), "{view}");
    let patch = json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "blocks": {
        "entry": {"ops": [["x", "mul?h", "a", "b"], ["y", "add?h", "x", 1]], "term": ["ok", "y"]}}}]});
    let (status, text) = run(&temp.path, &["try", &patch.to_string(), "--no-test"]);
    assert_eq!(status, 0, "{text}");
    for (args, want) in [
        (["3", "4"], "{\"Ok\":13}"),
        (
            ["4611686018427387904", "2"],
            "{\"Err\":{\"Code\":4611686018427387904}}",
        ),
        (
            ["9223372036854775807", "1"],
            "{\"Err\":{\"Code\":9223372036854775807}}",
        ),
    ] {
        let (_, result) = run(
            &temp.path,
            &["call", "f", args[0], args[1], "--on", "latest"],
        );
        assert_eq!(result.trim(), want, "{args:?}");
    }
    // A plain base: `loop(n, acc)` passes the function parameter `n` on.
    let temp = workspace("shadow-loop");
    commit_frame(
        &temp.path,
        &json!({"af1": 1, "fns": [{"fn": "sum_to", "params": [["n", "i64"]], "returns": "i64", "blocks": [
            {"name": "entry", "ops": [["z", "const", {"type": "i64", "value": 0}]], "term": ["br", "loop", "n", "z"]},
            {"name": "loop", "params": [["n", "i64"], ["acc", "i64"]], "ops": [["zero", "const", {"type": "i64", "value": 0}], ["more", "gt", "n", "zero"]],
             "term": ["cond", "more", ["body", "n", "acc"], ["done", "acc"]]},
            {"name": "body", "params": [["n", "i64"], ["acc", "i64"]], "ops": [["one", "const", {"type": "i64", "value": 1}], ["s", "add", "acc", "n"], ["m", "sub", "n", "one"]],
             "term": ["switch", "s", ["Ok", "step", "$", "m"], ["Err", "ovf"]]},
            {"name": "step", "params": [["acc2", "i64"], ["m", "Result<i64,ArithmeticError>"]], "term": ["switch", "m", ["Ok", "loop", "$", "acc2"], ["Err", "ovf"]]},
            {"name": "ovf", "ops": [["k", "const", {"type": "i64", "value": -1}]], "term": ["return", "k"]},
            {"name": "done", "params": [["acc", "i64"]], "term": ["return", "acc"]}]}]}),
    );
    let (_, view) = run(&temp.path, &["view", "sum_to"]);
    assert!(view.contains("loop(n_"), "{view}");
    let patch = json!({"af1": 1, "afx": 1, "patch": [{"fn": "sum_to", "blocks": {
        "entry": {"ops": [["acc", "const", {"type": "i64", "value": 100}]], "term": ["br", "loop"]}}}]});
    let (status, text) = run(&temp.path, &["try", &patch.to_string(), "--no-test"]);
    assert_eq!(status, 0, "{text}");
    let (_, result) = run(&temp.path, &["call", "sum_to", "4", "--on", "latest"]);
    assert_eq!(result.trim(), "110");
}

// ---------------------------------------------------------------------------
// Review regressions: the type of a name found in another block
// ---------------------------------------------------------------------------

/// A function whose `next` block uses `q`, the result `entry` defines,
/// while `other` (reached on another path) defines a `q` of its own.
fn far_q(name: &str, next_ops: &Value, next_term: &Value, other: &Value) -> Value {
    json!({"fn": name, "params": [["a", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["q", "const", {"type": "i64", "value": 5}]],
         "term": ["cond", ["lt", "a", 0], "other", "next"]},
        {"name": "next", "ops": next_ops, "term": next_term},
        other]})
}

#[test]
fn a_name_another_block_also_defines_keeps_the_type_of_its_definition() {
    // W5-AFX-1: `q` in `next` is `entry.q` (X4). Another block's parameter
    // or non-dominating result named `q` does not make its type unknown,
    // so checked operations and literal partners using it are typed.
    let temp = workspace("far-type");
    let base = json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Neg", "Big"]}],
        "fns": [{"fn": "hr", "params": [["x", "i64"]], "returns": "Result<i64,E>",
                 "blocks": [{"name": "entry", "ops": [["!Neg", "if", ["lt", "x", 0]]], "term": ["ok", "x"]}]}]});
    commit_frame(&temp.path, &base);
    let param_q = json!({"name": "other", "params": [["q", "i64"]], "term": ["ok", "q"]});
    let result_q = json!({"name": "other", "ops": [["q", "lt", "a", -10]], "term": ["cond", "q", ["done", 1], ["done", 2]]});
    let done = json!({"name": "done", "params": [["v", "i64"]], "term": ["ok", "v"]});
    let cases = [
        (
            "checked_add",
            json!([["x", "add?Big", "q", "a"]]),
            json!(["ok", "x"]),
        ),
        (
            "checked_call",
            json!([["x", "call?Neg", "hr", "q"]]),
            json!(["ok", "x"]),
        ),
        (
            "literal",
            json!([["x", "mul?Big", "q", 3]]),
            json!(["ok", ["sub?Big", "x", "q"]]),
        ),
    ];
    let mut fns = Vec::new();
    for (name, ops, term) in &cases {
        fns.push(far_q(&format!("{name}_p"), ops, term, &param_q));
        let mut shape = far_q(&format!("{name}_r"), ops, term, &result_q);
        shape["blocks"].as_array_mut().unwrap().push(done.clone());
        fns.push(shape);
    }
    // A handler that receives `q` by name, and a later block adding `q`.
    fns.push(json!({"fn": "handler_shape", "params": [["a", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["q", "const", {"type": "i64", "value": 4}], ["x", "add?h", "a", 1]], "term": ["br", "j", "x"]},
        {"name": "j", "params": [["x", "i64"]], "ops": [["z", "add?Big", "x", "q"]], "term": ["ok", "z"]},
        {"name": "h", "params": [["e", "ArithmeticError"], ["q", "i64"]], "term": ["ok", "q"]}]}));
    let frame = json!({"af1": 1, "afx": 1, "fns": fns});
    let (status, text) = run(&temp.path, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(status, 0, "{text}");
    for (function, arg, want) in [
        ("checked_add_p", "7", "{\"Ok\":12}"),
        ("checked_add_p", "-1", "{\"Ok\":5}"),
        ("checked_add_r", "7", "{\"Ok\":12}"),
        ("checked_add_r", "-20", "{\"Ok\":1}"),
        ("checked_add_r", "-1", "{\"Ok\":2}"),
        ("checked_call_p", "7", "{\"Ok\":5}"),
        ("checked_call_r", "7", "{\"Ok\":5}"),
        ("literal_p", "7", "{\"Ok\":10}"),
        ("literal_r", "7", "{\"Ok\":10}"),
        ("handler_shape", "1", "{\"Ok\":6}"),
        ("handler_shape", "9223372036854775807", "{\"Ok\":4}"),
    ] {
        let (_, result) = run(&temp.path, &["call", function, arg, "--on", "latest"]);
        assert_eq!(result.trim(), want, "{function}({arg})");
    }
    // The hand-written plain equivalent of `checked_add_p` agrees.
    let plain = json!({"af1": 1, "fns": [{"fn": "plain_add", "params": [["a", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["q", "const", {"type": "i64", "value": 5}], ["z", "const", {"type": "i64", "value": 0}], ["n", "lt", "a", "z"]],
         "term": ["cond", "n", ["other", "q"], "next"]},
        {"name": "next", "ops": [["x__r", "add", "entry.q", "a"]], "term": ["switch", "x__r", ["Ok", "next__x", "$"], ["Err", "fb"]]},
        {"name": "next__x", "params": [["x", "i64"]], "ops": [["o", "ok", "x"]], "term": ["return", "o"]},
        {"name": "fb", "ops": [["v", "variant", "E.Big"], ["e", "err", "v"]], "term": ["return", "e"]},
        {"name": "other", "params": [["q", "i64"]], "ops": [["o", "ok", "q"]], "term": ["return", "o"]}]}]});
    let (status, text) = run(&temp.path, &["try", &plain.to_string(), "--no-test"]);
    assert_eq!(status, 0, "{text}");
    for (arg, want) in [("7", "{\"Ok\":12}"), ("-1", "{\"Ok\":5}")] {
        let (_, result) = run(&temp.path, &["call", "plain_add", arg, "--on", "latest"]);
        assert_eq!(result.trim(), want, "plain_add({arg})");
    }
}

#[test]
fn a_name_whose_definition_x4_cannot_fix_stays_refused() {
    // The nearest definition decides: when blocks on joining paths define
    // `q` with different types, nothing types it and X4 refuses the name.
    let temp = workspace("far-type-join");
    let frame = json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Big"]}],
      "fns": [{"fn": "j", "params": [["a", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "term": ["cond", ["lt", "a", 0], "l", "r"]},
        {"name": "l", "ops": [["q", "const", {"type": "i64", "value": 5}]], "term": ["br", "m"]},
        {"name": "r", "ops": [["q", "lt", "a", 3]], "term": ["br", "m"]},
        {"name": "m", "ops": [["x", "add?Big", "q", 1]], "term": ["ok", "x"]}]}]});
    assert_refused(
        &temp.path,
        &frame,
        "AGENT_X_SCOPE",
        &["`q` is defined in blocks l, r, none of which dominates this point of `m`"],
    );
}

#[test]
fn a_value_defined_after_the_check_that_reaches_a_handler_is_named_as_such() {
    // W5-AFX-8: every path to `h` passes through `entry`, but `r` exists
    // only after the check that reaches `h`: say so, and how to fix it.
    let temp = workspace("defined-after");
    let frame = |ops: Value| {
        json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": [["Code", "i64"]]}],
          "fns": [{"fn": "da", "params": [["a", "i64"]], "returns": "Result<i64,E>", "blocks": [
            {"name": "entry", "ops": ops, "term": ["ok", "y"]},
            {"name": "h", "params": [["e", "ArithmeticError"]], "term": ["fail", "Code", "r"]}]}]})
    };
    let (symbol, detail) = refused(
        &temp.path,
        &frame(
            json!([["x", "add?h", "a", 1], ["r", "const", {"type": "i64", "value": 2}], ["y", "mul?h", "x", "r"]]),
        ),
    );
    assert_eq!(symbol, "AGENT_X_SCOPE", "{detail}");
    assert_eq!(
        detail,
        "/fns/0/blocks/1/term/2: `r` is defined in block `entry` only after /fns/0/blocks/0/ops/0, from which a failure or exit route reaches this point of `h`: define `r` before it"
    );
    // Following it is Valid.
    let fixed = frame(
        json!([["r", "const", {"type": "i64", "value": 2}], ["x", "add?h", "a", 1], ["y", "mul?h", "x", "r"]]),
    );
    let (status, text) = run(&temp.path, &["try", &fixed.to_string(), "--no-test"]);
    assert_eq!(status, 0, "{text}");
    let (_, result) = run(
        &temp.path,
        &["call", "da", "9223372036854775807", "--on", "latest"],
    );
    assert_eq!(result.trim(), "{\"Err\":{\"Code\":2}}");
    // A block that does not dominate the use keeps its own message.
    let (_, detail) = refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "fns": [{"fn": "nd", "params": [["a", "i64"]], "returns": "i64", "blocks": [
            {"name": "entry", "term": ["cond", ["lt", "a", 0], "l", "m"]},
            {"name": "l", "ops": [["r", "const", {"type": "i64", "value": 2}]], "term": ["br", "m"]},
            {"name": "m", "term": ["return", "r"]}]}]}),
    );
    assert!(
        detail.contains("which does not dominate this point of `m` (another path reaches it without passing through `l`)"),
        "{detail}"
    );
}

#[test]
fn a_switch_key_the_value_has_no_case_for_is_the_reported_cause() {
    // W5-AFX-4: a case key the switched value's type has no case for is the
    // problem, not the arguments its edge leaves to be derived.
    let temp = workspace("switch-not-a-case");
    let frame = json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Big", ["Code", "i64"]]}],
      "fns": [{"fn": "sw", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,E>", "blocks": [
        {"name": "entry", "ops": [["x", "add?h", "a", "b"]], "term": ["ok", "x"]},
        {"name": "h", "params": [["e", "ArithmeticError"], ["a", "i64"]],
         "term": ["switch", "e", ["Overflow", "ho"], ["DivideByZero", ["hz"]], ["InvalidShift", "hz"]]},
        {"name": "ho", "params": [["a", "i64"]], "term": ["fail", "Code", "a"]},
        {"name": "hz", "params": [["a", "i64"]], "term": ["fail", "Big"]}]}]});
    let (symbol, detail) = refused(&temp.path, &frame);
    assert_eq!(symbol, "AGENT_FRAME_INVALID", "{detail}");
    let lines: Vec<&str> = detail.lines().map(str::trim).collect();
    assert_eq!(lines.len(), 3, "{detail}");
    for (line, (at, key)) in lines.iter().zip([
        ("/term/2", "Overflow"),
        ("/term/3", "DivideByZero"),
        ("/term/4", "InvalidShift"),
    ]) {
        assert!(
            line.starts_with(&format!(
                "/fns/0/blocks/1{at}: `{key}` needs a variant scrutinee"
            )),
            "{detail}"
        );
    }
    assert!(!detail.contains("payload"), "{detail}");
    // The same key written with explicit arguments: the same cause.
    let mut explicit = frame.clone();
    explicit["fns"][0]["blocks"][1]["term"] = json!([
        "switch",
        "e",
        ["Overflow", "ho", "a"],
        ["DivideByZero", "hz", "a"],
        ["InvalidShift", "hz", "a"]
    ]);
    let (_, detail) = refused(&temp.path, &explicit);
    assert!(
        detail.starts_with("/fns/0/blocks/1/term/2: `Overflow` needs a variant scrutinee"),
        "{detail}"
    );
    // A variant without that case, and Result keys on an Option.
    let (_, detail) = refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Big", ["Code", "i64"]]}],
          "fns": [{"fn": "sv", "params": [["e", "E"], ["o", "Option<i64>"], ["a", "i64"]], "returns": "i64", "blocks": [
            {"name": "entry", "term": ["cond", ["lt", "a", 0], "s", "t"]},
            {"name": "s", "term": ["switch", "e", ["Bog", "u"], ["Code", "u", "$"]]},
            {"name": "t", "term": ["switch", "o", ["Ok", "u"], ["Err", "u"]]},
            {"name": "u", "params": [["a", "i64"]], "term": ["return", "a"]}]}]}),
    );
    let lines: Vec<&str> = detail.lines().map(str::trim).collect();
    assert_eq!(
        lines[0],
        "/fns/0/blocks/1/term/2: `Bog` is not a case of E (its cases: Big, Code) (1 of 3 problems)"
    );
    assert!(
        lines[1].starts_with("/fns/0/blocks/2/term/2: `Ok` is not a case of an Option"),
        "{detail}"
    );
    assert!(
        lines[2].starts_with("/fns/0/blocks/2/term/3: `Err` is not a case of an Option"),
        "{detail}"
    );
}

#[test]
fn restating_a_block_never_removes_a_result_a_kept_block_reads() {
    // W5-AFX-2: `next` (kept) reads `entry__x.r`, a piece restating `entry`
    // deletes. The patch is refused at the restated block, naming the kept
    // block to restate, instead of reaching the kernel with a dangling
    // reference; restating both is Valid.
    let temp = workspace("kept-reads");
    commit_frame(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "types": [{"name": "E", "variant": ["Big"]}],
          "fns": [{"fn": "f", "params": [["a", "i64"]], "returns": "Result<i64,E>", "blocks": [
            {"name": "entry", "ops": [["q", "const", {"type": "i64", "value": 3}], ["x", "add?Big", "a", 1],
                                      ["r", "const", {"type": "i64", "value": 7}]], "term": ["br", "next"]},
            {"name": "next", "ops": [["y", "add?Big", "r", "q"]], "term": ["ok", "y"]}]}]}),
    );
    let (_, view) = run(&temp.path, &["view", "f"]);
    assert!(view.contains("entry__x.r"), "{view}");
    let entry = json!({"ops": [["q", "const", {"type": "i64", "value": 3}], ["r", "const", {"type": "i64", "value": 8}]], "term": ["br", "next"]});
    let (symbol, detail) = refused(
        &temp.path,
        &json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "blocks": {"entry": entry}}]}),
    );
    assert_eq!(symbol, "AGENT_X_SCOPE", "{detail}");
    assert_eq!(
        detail,
        "/patch/0/blocks/entry: block `next`, which this patch keeps, reads `entry__x.r`, which restating `entry` removes: restate `next` too, so its names are resolved again"
    );
    // A restatement that keeps the piece and its value is not affected.
    let same = json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "blocks": {"entry":
        {"ops": [["q", "const", {"type": "i64", "value": 3}], ["x", "add?Big", "a", 2],
                 ["r", "const", {"type": "i64", "value": 7}]], "term": ["br", "next"]}}}]});
    let (status, text) = run(&temp.path, &["try", &same.to_string(), "--no-test"]);
    assert_eq!(status, 0, "{text}");
    let both = json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "blocks": {"entry": entry,
        "next": {"ops": [["y", "add?Big", "r", "q"]], "term": ["ok", "y"]}}}]});
    let (status, text) = run(&temp.path, &["try", &both.to_string(), "--no-test"]);
    assert_eq!(status, 0, "{text}");
    let (_, result) = run(&temp.path, &["call", "f", "1", "--on", "latest"]);
    assert_eq!(result.trim(), "{\"Ok\":11}");
}
