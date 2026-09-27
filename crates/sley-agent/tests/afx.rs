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
        Err(error) => format!("refused {}", error),
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
