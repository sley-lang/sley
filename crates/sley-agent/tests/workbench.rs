//! End-to-end workbench tests over fresh `init` workspaces: AV1 views, AF1
//! frames, the raw path, decoded refusals with locators, the dev loop, and
//! the submission semantics (`docs/spec/SLEY_AGENT_V1.md`).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sley_agent::genesis;
use sley_policy::PolicyResourceCeilings;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "sley-agent-{label}-{}-{}",
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

/// A fresh workspace with the workbench policy (or `ceilings`).
fn workspace(label: &str, ceilings: Option<PolicyResourceCeilings>) -> TempDir {
    let temp = TempDir::new(label);
    genesis::init(
        &temp.path,
        Some(SEED),
        ceilings.unwrap_or(genesis::INIT_CEILINGS),
    )
    .unwrap();
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

/// A small program in the shape agents meet: a variant error type and a
/// clamping function with one wrong branch (`above` returns `low`).
fn clamp_frame(buggy: bool) -> Value {
    let above = if buggy { "low" } else { "high" };
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
        {"name": "above", "ops": [["r", "ok", above]], "term": ["return", "r"]},
        {"name": "inside", "ops": [["r", "ok", "value"]], "term": ["return", "r"]}]}]})
}

fn committed_program(label: &str, ceilings: Option<PolicyResourceCeilings>) -> TempDir {
    let temp = workspace(label, ceilings);
    let (status, text) = run(&temp.path, &["try", &clamp_frame(true).to_string()]);
    assert_eq!(status, 0, "{text}");
    let (status, text) = run(&temp.path, &["commit"]);
    assert_eq!(status, 0, "{text}");
    temp
}

#[test]
fn av1_renders_a_function_compactly_and_byte_stably() {
    let temp = committed_program("view", None);
    let (status, first) = run(&temp.path, &["view", "bound"]);
    assert_eq!(status, 0);
    assert!(
        first.starts_with("# sley view (AV1, non-canonical)"),
        "{first}"
    );
    assert!(first.contains("inverted = lt high, low"), "{first}");
    assert!(first.contains("cond is_above -> above, inside"), "{first}");
    assert!(
        first.len() <= 1024,
        "a small function renders in at most 1 KB: {}",
        first.len()
    );
    let (_, second) = run(&temp.path, &["view", "bound"]);
    assert_eq!(first, second, "rendering is byte-stable");
}

#[test]
fn av1_is_never_accepted_as_input() {
    // The product has no AV1 reader: a view fed back to `try` is refused as
    // not JSON, before any compilation.
    let temp = committed_program("no-parser", None);
    let (_, view) = run(&temp.path, &["view", "bound"]);
    let (status, text) = run(&temp.path, &["try", &view]);
    assert_eq!(status, 2);
    assert!(
        text.contains("AGENT_IO_FAILED") || text.contains("AGENT_FRAME_INVALID"),
        "{text}"
    );
    let (status, text) = run(
        &temp.path,
        &["try", "{\"af1\": 1, \"fns\": \"fn bound(x: i64)\"}"],
    );
    assert_eq!(status, 2);
    assert!(
        text.contains("AGENT_FRAME_INVALID: /fns: expected an array"),
        "{text}"
    );
}

#[test]
fn af1_edit_with_tests_is_valid_and_tests_run() {
    let temp = committed_program("edit", None);
    let (_, before) = run(&temp.path, &["call", "bound", "11", "0", "10"]);
    assert_eq!(before.trim(), "{\"Ok\":0}");
    let frame = json!({"af1": 1,
        "edit": [{"function": "bound", "replace_op": "above.r", "with": {"opcode": "result_ok", "operands": ["high"]}}],
        "tests": [{"target": "bound", "inputs": [11, 0, 10], "expect": {"Ok": 10}},
                  {"target": "bound", "inputs": [5, 10, 0], "expect": {"Err": "Inverted"}}]});
    let (status, result) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{result}");
    assert_eq!(result["verdict"]["valid"], true);
    assert_eq!(result["tests"].as_array().unwrap().len(), 2);
    assert!(
        result["tests"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["pass"] == true),
        "{result}"
    );
    let (_, after) = run(
        &temp.path,
        &["call", "bound", "11", "0", "10", "--on", "c2"],
    );
    assert_eq!(after.trim(), "{\"Ok\":10}");
}

#[test]
fn a_wrong_expectation_fails_with_both_values() {
    let temp = committed_program("wrong-expect", None);
    let frame = json!({"af1": 1, "tests": [{"name": "t_bad", "fn": "bound", "args": [11, 0, 10], "expect": {"Ok": 10}}]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert!(
        text.contains("FAIL t_bad (bound): expected Ok(10), got Ok(0)"),
        "{text}"
    );
    // Nothing is written to the repository by try or test.
    let (status, _) = run(&temp.path, &["test", "c2"]);
    assert_eq!(status, 1);
    let (_, view) = run(&temp.path, &["find", "--kind", "test"]);
    assert!(
        view.trim().is_empty(),
        "tests stay in the candidate: {view}"
    );
}

#[test]
fn test_limits_above_the_grant_are_refused_with_a_locator() {
    let ceilings = PolicyResourceCeilings::new(1_000, 1_000, 1_000, 100, 100, 100);
    let temp = committed_program("limits", Some(ceilings));
    let frame = json!({"af1": 1,
        "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "high"]}],
        "tests": [{"name": "t3", "fn": "bound", "args": [11, 0, 10], "expect": {"Ok": 10},
                   "limits": {"memory_bytes": 1_000_000}}]});
    let (status, result) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 1);
    let verdict = &result["verdict"];
    assert_eq!(verdict["decision"], "ResourceLimit");
    assert_eq!(verdict["phase"], 12);
    assert_eq!(verdict["symbol"], "CANDIDATE_TEST_RESOURCE_LIMIT");
    assert_eq!(
        verdict["where"],
        "TestCase t3 .resource_limits.memory_bytes 1000000 > grant ceiling 1000"
    );
    assert!(
        verdict["hint"]
            .as_str()
            .unwrap()
            .contains("omit \"limits\"")
    );
    // Omitted limits take the defaults clamped to the grant: Valid.
    let frame = json!({"af1": 1,
        "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "high"]}],
        "tests": [{"fn": "bound", "args": [11, 0, 10], "expect": {"Ok": 10}}]});
    let (status, result) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{result}");
}

#[test]
fn an_orphaned_block_names_the_unlisted_block() {
    let temp = committed_program("orphan", None);
    let raw = json!([
        {"class": "CreateEntity", "kind": 7, "key": "fresh", "payload": {"function": "bound",
          "parameters": [], "operations": [],
          "terminator": {"variant": "Trap", "value": {"code": "Unreachable", "payload": {"variant": "None"}}},
          "reachability": "Required"}},
        {"class": "ReplaceEntityVersion", "kind": 5, "target": "bound", "payload": {
          "type_parameters": [], "parameters": ["bound.value", "bound.low", "bound.high"],
          "result_type": "Result<i64,RangeError>", "effects": [], "entry_block": "@fresh",
          "blocks": ["@fresh"], "contracts": [], "visibility": "Exported"}}]);
    let (status, result) = run_json(&temp.path, &["try", &raw.to_string()]);
    assert_eq!(status, 1);
    let verdict = &result["verdict"];
    assert_eq!(verdict["phase"], 7);
    assert_eq!(verdict["symbol"], "GRAPH_INVENTORY_MISMATCH");
    let location = verdict["where"].as_str().unwrap();
    assert!(location.contains("bound.entry"), "{location}");
    assert!(
        location.contains("name this function but are not listed"),
        "{location}"
    );
}

#[test]
fn a_dominance_refusal_names_the_operand_and_both_blocks() {
    let temp = workspace("dominance", None);
    let frame = json!({"af1": 1, "fns": [{"fn": "pick", "params": [["a", "i64"], ["b", "i64"]], "returns": "i64",
        "blocks": [
          {"name": "entry", "ops": [["c", "gt", "a", "b"]], "term": ["cond", "c", "left", "right"]},
          {"name": "left", "ops": [["m", "const", 5]], "term": ["br", "join"]},
          {"name": "right", "term": ["br", "join"]},
          {"name": "join", "term": ["return", "left.m"]}]}]});
    let (status, result) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 1);
    let verdict = &result["verdict"];
    assert_eq!(verdict["symbol"], "CFG_DOMINANCE");
    let location = verdict["where"].as_str().unwrap();
    assert!(location.contains("`left.m`"), "{location}");
    assert!(location.contains("block pick.left"), "{location}");
    assert!(location.contains("block pick.join"), "{location}");
}

#[test]
fn block_parameters_stay_in_their_block_and_switch_targets_take_both_forms() {
    let temp = workspace("block-params", None);
    let frame = |sum_ops: Value, carry: bool| {
        let into_sum = if carry {
            json!(["Ok", ["sum", "sub", "$"]])
        } else {
            json!(["Ok", "sum", "$"])
        };
        let sum_params = if carry {
            json!([["sub", "i64"], ["t", "i64"]])
        } else {
            json!([["t", "i64"]])
        };
        json!({"af1": 1, "types": [{"name": "E", "variant": ["Overflow"]}],
          "fns": [{"fn": "f", "params": [["a", "i64"]], "returns": "Result<i64,E>", "blocks": [
            {"name": "entry", "ops": [["d", "add", "a", "a"]], "term": ["switch", "d", ["Ok", "mid", "$"], ["Err", "ovf"]]},
            {"name": "mid", "params": [["sub", "i64"]], "ops": [["m", "add", "sub", "sub"]],
             "term": ["switch", "m", into_sum, ["Err", "ovf"]]},
            {"name": "sum", "params": sum_params, "ops": sum_ops, "term": ["switch", "s", ["Ok", "done", "$"], ["Err", "ovf"]]},
            {"name": "done", "params": [["v", "i64"]], "ops": [["r", "ok", "v"]], "term": ["return", "r"]},
            {"name": "ovf", "ops": [["e", "variant", "E.Overflow"], ["r", "err", "e"]], "term": ["return", "r"]}]}]})
    };
    // Naming another block's parameter is refused before compilation.
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            &frame(json!([["s", "add", "mid.sub", "t"]]), false).to_string(),
        ],
    );
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("`mid.sub` is a parameter of block `mid`"),
        "{text}"
    );
    // Passing it on as an edge argument (bracketed switch target) is Valid.
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            &frame(json!([["s", "add", "sub", "t"]]), true).to_string(),
        ],
    );
    assert_eq!(status, 0, "{text}");
    let (_, value) = run(&temp.path, &["call", "f", "3", "--on", "latest"]);
    assert_eq!(value.trim(), "{\"Ok\":18}");
}

#[test]
fn one_refusal_lists_every_misplaced_name_and_case_key() {
    let temp = workspace("one-round", None);
    // `sub` is used unqualified in two later blocks: both uses in one round.
    let frame = |cases: Value| {
        json!({"af1": 1, "types": [{"name": "E", "variant": ["Overflow"]}],
          "fns": [{"fn": "f", "params": [["a", "i64"]], "returns": "Result<i64,E>", "blocks": [
            {"name": "entry", "ops": [["d", "add", "a", "a"]], "term": ["switch", "d", ["Ok", "mid", "$"], ["Err", "ovf"]]},
            {"name": "mid", "params": [["sub", "i64"]], "ops": [["m", "add", "sub", "sub"]],
             "term": ["switch", "m", ["Ok", "sum", "$"], ["Err", "ovf"]]},
            {"name": "sum", "params": [["t", "i64"]], "ops": [["s", "add", "sub", "t"]], "term": ["switch", "s", cases, ["Err", "ovf"]]},
            {"name": "done", "params": [["v", "i64"]], "ops": [["w", "add", "v", "sub"], ["r", "ok", "w"]], "term": ["return", "r"]},
            {"name": "ovf", "ops": [["e", "variant", "E.Overflow"], ["r", "err", "e"]], "term": ["return", "r"]}]}]})
    };
    let (status, text) = run(
        &temp.path,
        &["try", &frame(json!(["Ok", "done", "$"])).to_string()],
    );
    assert_eq!(status, 2, "{text}");
    // The headline carries the first problem's pointer, so a reader of the
    // first line alone can still locate it.
    let headline = text.lines().next().unwrap();
    assert!(
        headline.starts_with(
            "error AGENT_FRAME_INVALID: /fns/0/blocks/2/ops/0: `sub` is a parameter of block `mid`"
        ),
        "{text}"
    );
    assert!(headline.ends_with("(1 of 2 problems)"), "{text}");
    // Each further problem is one indented line with its own pointer.
    assert!(
        text.contains("\n  /fns/0/blocks/3/ops/0: `sub` is a parameter of block `mid`"),
        "{text}"
    );
    // A Result case written without its key names the expected keys.
    let fixed = |cases: Value| {
        let mut value = frame(cases);
        value["fns"][0]["blocks"][2]["ops"] = json!([["s", "add", "t", "t"]]);
        value["fns"][0]["blocks"][3]["ops"] = json!([["r", "ok", "v"]]);
        value
    };
    let (status, text) = run(
        &temp.path,
        &["try", &fixed(json!(["done", "$"])).to_string()],
    );
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("/fns/0/blocks/2/term/2: `done` is not a case of a Result; a Result switch lists [\"Ok\", block, args...]"),
        "{text}"
    );
    // A nested operation as an operand names the fix at its slot.
    let mut nested = fixed(json!(["Ok", "done", "$"]));
    nested["fns"][0]["blocks"][3]["ops"] = json!([]);
    nested["fns"][0]["blocks"][3]["term"] = json!(["return", ["ok", "v"]]);
    let (status, text) = run(&temp.path, &["try", &nested.to_string()]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("/fns/0/blocks/3/term/1: operations do not nest"),
        "{text}"
    );
    let (status, text) = run(
        &temp.path,
        &["try", &fixed(json!(["Ok", "done", "$"])).to_string()],
    );
    assert_eq!(status, 0, "{text}");
}

#[test]
fn test_names_may_carry_hyphens_like_the_public_tests() {
    let temp = committed_program("hyphen", None);
    let frame = json!({"af1": 1, "tests": [
        {"fn": "bound", "args": [5, 0, 10], "expect": {"Ok": 5}, "name": "bound-inside"}]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string(), "--verbose"]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("ok   bound-inside = Ok(5)"), "{text}");
    let (status, text) = run(&temp.path, &["view", "--after", "latest", "bound-inside"]);
    assert_eq!(status, 0, "{text}");
    assert!(
        text.contains("test bound-inside: bound(5, 0, 10) == Ok(5)"),
        "{text}"
    );
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            &json!({"af1": 1, "tests": [
        {"fn": "bound", "args": [1, 0, 2], "expect": {"Ok": 1}, "name": "-lead"}]})
            .to_string(),
        ],
    );
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("is not a name ([A-Za-z_][A-Za-z0-9_-]*, at most 64 bytes)"),
        "{text}"
    );
}

#[test]
fn the_refusal_corpus_decodes_every_reachable_phase() {
    // Phases 5, 7, 9 and 12 have their own tests above and below; this
    // corpus adds the stale base (3), a type refusal (6) and a test-plan
    // refusal (11). Every refusal carries its symbol and a hint.
    let temp = committed_program("corpus", None);
    let check = |result: &Value, phase: u64, symbol: &str| {
        let verdict = &result["verdict"];
        assert_eq!(verdict["phase"], phase, "{result}");
        assert_eq!(verdict["symbol"], symbol, "{result}");
        assert!(
            verdict["hint"]
                .as_str()
                .is_some_and(|hint| hint != "no hint"),
            "{result}"
        );
    };
    // Phase 6: a constant whose data disagrees with its type.
    let raw = json!([{"class": "CreateEntity", "kind": 9, "key": "bad_const", "payload": {
        "value": {"value_type": "i64", "data": {"variant": "Bool", "value": true}}}}]);
    let (status, result) = run_json(&temp.path, &["try", &raw.to_string()]);
    assert_eq!(status, 1, "{result}");
    check(&result, 6, "TYPE_CONST_SHAPE");
    // Phase 11: a TestCase whose input disagrees with its target.
    let raw = json!([{"class": "CreateEntity", "kind": 14, "key": "t_bad_input", "payload": {
        "target": "bound",
        "inputs": [{"value_type": "bool", "data": {"variant": "Bool", "value": true}}],
        "effect_environment": {"variant": "Replay", "value": []},
        "expected": {"variant": "FailureCode", "value": 1}, "observations": [],
        "resource_limits": {"fuel": 1, "memory_bytes": 1, "output_bytes": 1, "effect_count": 0,
                            "call_depth": 1, "wall_timeout_millis": 1}}}]);
    let (status, result) = run_json(&temp.path, &["try", &raw.to_string()]);
    assert_eq!(status, 1, "{result}");
    assert_eq!(result["verdict"]["phase"], 11, "{result}");
    assert!(
        result["verdict"]["symbol"]
            .as_str()
            .is_some_and(|symbol| symbol.starts_with("TEST_PLAN_")),
        "{result}"
    );
    // Phase 3: a Valid candidate goes stale when the head moves under it.
    let fix = json!({"af1": 1, "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "high"]}]});
    assert_eq!(run(&temp.path, &["try", &fix.to_string()]).0, 0);
    let stale = json!({"af1": 1, "edit": [{"fn": "bound", "replace_op": "below.r", "with": ["ok", "low"]},
                                         {"fn": "bound", "replace_op": "inside.r", "with": ["ok", "value"]}],
                       "consts": [{"name": "k_seven", "value": 7}]});
    let (status, first) = run_json(&temp.path, &["try", &stale.to_string()]);
    assert_eq!(status, 0, "{first}");
    let handle = first["handle"].as_str().unwrap().to_owned();
    let (fix_handle_status, _) = run(&temp.path, &["commit", "c4"]);
    assert_eq!(fix_handle_status, 0);
    let (status, result) = run_json(&temp.path, &["explain", &handle]);
    assert_eq!(status, 1, "{result}");
    check(&result, 3, "CANDIDATE_BASE_TRANSACTION_MISMATCH");
}

#[test]
fn an_unresolved_reference_names_both_entities() {
    let temp = committed_program("unresolved", None);
    let missing = "00".repeat(32);
    let raw = json!([{"class": "CreateEntity", "kind": 14, "key": "t_dangling", "payload": {
        "target": missing, "inputs": [], "effect_environment": {"variant": "Replay", "value": []},
        "expected": {"variant": "FailureCode", "value": 1}, "observations": [],
        "resource_limits": {"fuel": 1, "memory_bytes": 1, "output_bytes": 1, "effect_count": 0,
                            "call_depth": 1, "wall_timeout_millis": 1}}}]);
    let (status, result) = run_json(&temp.path, &["try", &raw.to_string()]);
    assert_eq!(status, 1);
    let verdict = &result["verdict"];
    assert_eq!(verdict["symbol"], "GRAPH_UNRESOLVED_REFERENCE");
    let location = verdict["where"].as_str().unwrap();
    assert!(
        location.starts_with("TestCase t_dangling test target -> #00000000 (not live)"),
        "{location}"
    );
}

#[test]
fn the_mutation_budget_refusal_names_the_count() {
    let ceilings = PolicyResourceCeilings::new(1_000_000, 1_000_000, 1_000_000, 0, 5, 0);
    let temp = workspace("budget", Some(ceilings));
    let (status, result) = run_json(&temp.path, &["try", &clamp_frame(false).to_string()]);
    assert_eq!(status, 1);
    assert_eq!(result["verdict"]["symbol"], "CAP_BUDGET_EXCEEDED");
    assert!(
        result["verdict"]["where"]
            .as_str()
            .unwrap()
            .ends_with("> grant ceiling 5"),
        "{result}"
    );
}

#[test]
fn submit_is_repeatable_and_status_matches_the_file() {
    let temp = committed_program("submit", None);
    let fix = json!({"af1": 1, "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "high"]}],
                     "tests": [{"fn": "bound", "args": [11, 0, 10], "expect": {"Ok": 10}}]});
    assert_eq!(run(&temp.path, &["try", &fix.to_string()]).0, 0);
    assert_eq!(run(&temp.path, &["submit", "c2"]).0, 0);
    let first = fs::read_to_string(temp.path.join("final_candidate.hex")).unwrap();
    let other = json!({"af1": 1, "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "high"]}]});
    assert_eq!(run(&temp.path, &["try", &other.to_string()]).0, 0);
    assert_eq!(run(&temp.path, &["submit", "c3", "--untested"]).0, 0);
    let second = fs::read_to_string(temp.path.join("final_candidate.hex")).unwrap();
    assert_ne!(first, second, "the last submission wins");
    let (status, text) = run(&temp.path, &["status"]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("submission: c3 (Valid)"), "{text}");
    // A refused candidate cannot be submitted.
    let refused = json!({"af1": 1, "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "high"]}],
        "tests": [{"fn": "bound", "args": [11, 0, 10], "expect": {"Ok": 10}, "limits": {"fuel": 2_000_000}}]});
    let (status, _) = run(&temp.path, &["try", &refused.to_string()]);
    assert_eq!(status, 1);
    let (status, text) = run(&temp.path, &["submit", "c4"]);
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_SUBMISSION_REFUSED"), "{text}");
    // A type mismatch AF1 can see is a frame error that names it.
    let broken = json!({"af1": 1, "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "entry.inverted"]}]});
    let (status, text) = run(&temp.path, &["try", &broken.to_string()]);
    assert_eq!(status, 2);
    assert!(
        text.contains("`r` passes a bool where the result's Ok type is i64"),
        "{text}"
    );
}

#[test]
fn a_three_hundred_operation_candidate_validates() {
    let temp = workspace("large", None);
    let mut ops = vec![json!(["v0", "eq", "a", "a"])];
    for index in 1..296 {
        ops.push(json!([
            format!("v{index}"),
            "and",
            format!("v{}", index - 1),
            "v0"
        ]));
    }
    let frame = json!({"af1": 1, "fns": [{"fn": "wide", "params": [["a", "i64"]], "returns": "bool",
        "blocks": [{"name": "entry", "ops": ops, "term": ["return", "v295"]}]}]});
    let (status, result) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{result}");
    assert!(
        result["ops"]["created"].as_u64().unwrap() >= 299,
        "{result}"
    );
    let (_, value) = run(&temp.path, &["call", "wide", "3", "--on", "c1"]);
    assert_eq!(value.trim(), "true");
}

#[test]
fn malformed_frames_fail_with_a_json_pointer() {
    let temp = workspace("malformed", None);
    let frame = json!({"af1": 1, "fns": [{"fn": "f", "params": [["x", "i64"]], "returns": "i64",
        "blocks": [{"name": "entry", "ops": [["y", "frobnicate", "x"]], "term": ["return", "y"]}]}]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 2);
    assert!(
        text.contains("AGENT_FRAME_INVALID: /fns/0/blocks/0/ops/0: unknown opcode `frobnicate`"),
        "{text}"
    );
    let (status, text) = run(&temp.path, &["try", "{\"fns\": []}"]);
    assert_eq!(status, 2);
    assert!(text.contains("/af1"), "{text}");
}

#[test]
fn call_batch_streams_one_result_per_input() {
    let temp = committed_program("batch", None);
    let batch = temp.path.join("batch.json");
    fs::write(&batch, "[[1,0,10],[-5,0,10],[50,0,10],[5,10,0]]").unwrap();
    let (status, text) = run(
        &temp.path,
        &["call", "bound", "--batch", batch.to_str().unwrap()],
    );
    assert_eq!(status, 0);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines,
        [
            "{\"Ok\":1}",
            "{\"Ok\":0}",
            "{\"Ok\":0}",
            "{\"Err\":\"Inverted\"}"
        ]
    );
}

#[test]
fn full_redefinition_reuses_names_and_deletes_the_rest() {
    let temp = committed_program("redefine", None);
    let mut frame = clamp_frame(false);
    // Drop the `inside` block: its operations go, everything else keeps
    // its identity and only changed bodies are replaced.
    let blocks = frame["fns"][0]["blocks"].as_array_mut().unwrap();
    blocks.retain(|block| block["name"] != "inside");
    blocks[4]["term"] = json!(["cond", "is_above", "above", "below"]);
    let (status, result) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{result}");
    assert_eq!(result["ops"]["created"], 0);
    assert_eq!(
        result["ops"]["deleted"], 2,
        "the inside block and its operation"
    );
    // A Valid candidate that runs no TestCase says so, and so does submit.
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("tests: 0 ran"), "{text}");
    assert!(
        text.contains(
            "next: add tests without restating the frame: sley-agent try --on d3 '{\"af1\": 1, \"tests\": [...]}' (submit refuses an untested change; --untested overrides)"
        ),
        "{text}"
    );
    let (status, text) = run(&temp.path, &["submit", "c3"]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains(
            "AGENT_SUBMISSION_REFUSED: c3 changes functions but no TestCase in it targets them"
        ),
        "{text}"
    );
    assert!(!temp.path.join("final_candidate.hex").exists());
    let (status, text) = run(&temp.path, &["submit", "c3", "--untested"]);
    assert_eq!(status, 0, "{text}");
    assert!(
        text.contains("note: no TestCase in c3 targets a function it changes"),
        "{text}"
    );
}

#[test]
fn the_guide_is_small_and_every_example_runs() {
    // The guide stays small (its size is recorded, not waived), and its
    // examples run in order: the frame with `try`, the follow-up with
    // `try --on d1`, and the repair delta with `fill`.
    let guide = sley_agent::help::GUIDE;
    assert!(guide.len() <= 3_500, "guide is {} bytes", guide.len());
    let temp = workspace("guide", None);
    let examples: Vec<&str> = guide
        .split("```json\n")
        .skip(1)
        .map(|rest| rest.split("```").next().unwrap())
        .collect();
    assert_eq!(examples.len(), 3, "a frame, a follow-up and a repair delta");
    let (status, text) = run(&temp.path, &["try", examples[0]]);
    assert_eq!(status, 0, "guide frame: {text}");
    assert!(text.contains("tests: 3/3 passed"), "{text}");
    assert!(text.contains("draft d1@r1"), "{text}");
    let (status, text) = run(&temp.path, &["try", "--on", "d1", examples[1]]);
    assert_eq!(status, 0, "guide follow-up: {text}");
    assert!(text.contains("tests: 3/3 passed"), "{text}");
    let fix = temp.path.join("fix.json");
    fs::write(&fix, examples[2]).unwrap();
    let (status, text) = run(
        &temp.path,
        &["fill", "d1", fix.to_str().unwrap(), "--revision", "2"],
    );
    assert_eq!(status, 0, "guide repair: {text}");
    assert!(text.contains("d1@r3"), "{text}");
    let (status, text) = run(&temp.path, &["submit", "d1"]);
    assert_eq!(status, 0, "{text}");
    // Every help topic the guide names exists.
    for line in guide.lines() {
        if let Some(rest) = line.split("sley-agent help ").nth(1) {
            let topics = rest.split('`').next().unwrap();
            for topic in topics.split('|') {
                assert!(
                    sley_agent::help::topic(topic).is_some(),
                    "the guide names an unknown help topic: {topic}"
                );
            }
        }
    }
    // Every command the guide shows names a real command.
    for line in guide.lines() {
        if let Some(command) = line.trim_start().strip_prefix("sley-agent ") {
            let word = command.split_whitespace().next().unwrap();
            assert!(
                sley_agent::cli::COMMANDS.contains(&word),
                "the guide shows an unknown command: {line}"
            );
        }
    }
}

#[test]
fn every_drafts_help_command_runs() {
    // The command lines of `help drafts` run in order on a draft made from
    // the guide's frame, with the guide's follow-up and repair as files.
    let guide = sley_agent::help::GUIDE;
    let examples: Vec<&str> = guide
        .split("```json\n")
        .skip(1)
        .map(|rest| rest.split("```").next().unwrap())
        .collect();
    let temp = workspace("drafts-help", None);
    assert_eq!(run(&temp.path, &["try", examples[0]]).0, 0);
    fs::write(temp.path.join("more.json"), examples[1]).unwrap();
    fs::write(temp.path.join("fix.json"), examples[2]).unwrap();
    let mut ran = 0;
    for line in sley_agent::help::DRAFTS.lines() {
        let Some(command) = line.trim_start().strip_prefix("sley-agent ") else {
            continue;
        };
        let words: Vec<String> = command
            .split("  ")
            .next()
            .unwrap()
            .split_whitespace()
            .map(|word| {
                if Path::new(word)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
                {
                    temp.path.join(word).display().to_string()
                } else {
                    word.to_owned()
                }
            })
            .collect();
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        let (status, text) = run(&temp.path, &words);
        assert_eq!(status, 0, "help drafts: {line}: {text}");
        ran += 1;
    }
    assert!(ran >= 8, "{ran} commands ran");
}

/// The worked AF1 example of the 2.0.2 guide (plain AF1), the program many
/// tests edit.
fn percent_frame() -> Value {
    serde_json::from_str(include_str!("fixtures/percent.json")).unwrap()
}

/// A workspace holding the guide's first example (without its tests) and
/// the `stub` the tests topic names, committed.
fn guide_context(label: &str) -> TempDir {
    let temp = workspace(label, None);
    let example = sley_agent::help::GUIDE
        .split("```json\n")
        .nth(1)
        .and_then(|rest| rest.split("```").next())
        .unwrap();
    let mut frame: Value = serde_json::from_str(example).unwrap();
    frame.as_object_mut().unwrap().remove("test_tables");
    let stub = json!({"fn": "stub", "params": [], "returns": "i64",
        "blocks": [{"name": "entry", "ops": [], "term": ["trap", "unreachable"]}]});
    frame["fns"].as_array_mut().unwrap().push(stub);
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    temp
}

#[test]
fn every_help_topic_example_runs() {
    // Every line of a help topic that is a whole JSON object is an example:
    // it runs as its section's frame key, against the guide's program.
    let temp = guide_context("help-examples");
    let mut ran = 0;
    for (topic, text) in [
        ("af1", sley_agent::help::AF1),
        ("tests", sley_agent::help::TESTS),
    ] {
        let mut section = topic;
        for line in text.lines() {
            if let Some(heading) = line.strip_prefix("## ") {
                section = heading.split_whitespace().next().unwrap();
                continue;
            }
            let Ok(Value::Object(example)) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            let key = match section {
                "types" | "consts" | "tests" => section,
                other => panic!("help {topic}: an example in section `{other}` has no runner"),
            };
            let frame = json!({"af1": 1, key: [Value::Object(example)]});
            let (status, output) = run(&temp.path, &["try", &frame.to_string()]);
            assert_eq!(status, 0, "help {topic} example {line}: {output}");
            if key == "tests" {
                assert!(output.contains("tests: 1/1 passed"), "{line}: {output}");
            }
            ran += 1;
        }
    }
    assert!(ran >= 7, "{ran} examples ran");
}

#[test]
fn every_value_form_in_help_types_round_trips() {
    // Each documented value form, read against its type and rendered back.
    let doc = sley_agent::help::TYPES;
    let forms: &[(&str, &str)] = &[
        ("i64", "5"),
        ("bool", "true"),
        ("unit", "null"),
        ("text", "\"hi\""),
        ("bytes", "\"0x00ff\""),
        ("i128", "\"170141183460469231731687303715884105727\""),
        ("(i64,i64)", "[1,2]"),
        ("Vec<i64>", "[1,2]"),
        ("Option<i64>", "\"None\""),
        ("Option<i64>", "{\"Some\":3}"),
        ("Result<i64,Shape>", "{\"Ok\":4}"),
        ("Result<i64,Shape>", "{\"Err\":\"Empty\"}"),
        ("Shape", "\"Empty\""),
        ("Shape", "{\"Circle\":5}"),
        ("Point", "{\"x\":1,\"y\":2}"),
        ("u128", "\"340282366920938463463374607431768211455\""),
        ("ArithmeticError", "{\"ArithmeticError\":\"Overflow\"}"),
    ];
    for needle in [
        "\"None\" | {\"Some\": v}",
        "{\"Ok\": v} | {\"Err\": e}",
        "\"Case\" | {\"Case\": payload}",
        "{\"field\": v, ...}",
        "\"0x00ff\"",
        "\"hi\"",
        "null",
        "[a, b]",
        "\"Overflow\" | {\"ArithmeticError\": \"Overflow\"}",
    ] {
        assert!(
            doc.contains(needle),
            "help types no longer documents {needle}"
        );
    }
    let temp = workspace("value-forms", None);
    let fns: Vec<Value> = forms
        .iter()
        .enumerate()
        .map(|(index, (ty, _))| {
            json!({"fn": format!("id{index}"), "params": [["x", ty]], "returns": ty,
                   "blocks": [{"name": "entry", "ops": [], "term": ["return", "x"]}]})
        })
        .collect();
    let frame = json!({"af1": 1, "types": [
        {"name": "Shape", "variant": ["Empty", ["Circle", "i64"], ["Rect", "(i64,i64)"]]},
        {"name": "Point", "record": [["x", "i64"], ["y", "i64"]]}], "fns": fns});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    for (index, (ty, value)) in forms.iter().enumerate() {
        let (status, got) = run(
            &temp.path,
            &["call", &format!("id{index}"), value, "--on", "latest"],
        );
        assert_eq!(status, 0, "{ty} {value}: {got}");
        assert_eq!(got.trim(), *value, "{ty}");
    }
}

/// The guide's first example without its tests, committed.
fn percent_program(label: &str) -> TempDir {
    let temp = workspace(label, None);
    let mut frame = percent_frame();
    frame.as_object_mut().unwrap().remove("tests");
    assert_eq!(run(&temp.path, &["try", &frame.to_string()]).0, 0);
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    temp
}

#[test]
fn delete_and_redefine_creates_a_fresh_entity() {
    // Review finding 1: a deleted name redefined in the same frame used to
    // revive the deleted identity and emit only deletes.
    let temp = percent_program("redefine-deleted");
    let frame = json!({"af1": 1, "delete": ["percent"], "fns": [{"fn": "percent",
        "params": [["part", "i64"], ["whole", "i64"]], "returns": "Result<i64,MathError>",
        "blocks": [{"name": "entry", "ops": [["zero", "const", 0], ["r", "ok", "zero"]], "term": ["return", "r"]}]}]});
    let (status, result) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{result}");
    assert!(result["ops"]["created"].as_u64().unwrap() > 0, "{result}");
    let (_, value) = run(&temp.path, &["call", "percent", "3", "4", "--on", "latest"]);
    assert_eq!(value.trim(), "{\"Ok\":0}");
}

#[test]
fn a_literal_never_reuses_a_constant_the_frame_changes() {
    // Review finding 2.
    let temp = percent_program("literal-reuse");
    let frame = json!({"af1": 1, "consts": [{"name": "k_100", "value": 7}],
        "fns": [{"fn": "g", "params": [], "returns": "i64",
                 "blocks": [{"name": "entry", "ops": [["h", "const", 100]], "term": ["return", "h"]}]}]});
    assert_eq!(run(&temp.path, &["try", &frame.to_string()]).0, 0);
    let (_, value) = run(&temp.path, &["call", "g", "--on", "latest"]);
    assert_eq!(value.trim(), "100");
}

#[test]
fn edits_in_one_function_all_apply_and_restating_twice_is_refused() {
    // Review finding 3.
    let temp = percent_program("edits");
    let frame = json!({"af1": 1, "edit": [
        {"fn": "percent", "replace_op": "entry.zero", "with": ["const", 5]},
        {"fn": "percent", "replace_op": "entry.is_zero", "with": ["ne", "whole", "zero"]},
        {"fn": "percent", "replace_op": "scale.hundred", "with": ["const", 1000]}]});
    assert_eq!(run(&temp.path, &["try", &frame.to_string()]).0, 0);
    let (_, view) = run(&temp.path, &["view", "percent", "--after", "latest"]);
    for line in [
        "zero = const k_5 (5)",
        "is_zero = ne whole, zero",
        "hundred = const k_1000 (1000)",
    ] {
        assert!(view.contains(line), "{line}: {view}");
    }
    let twice = json!({"af1": 1,
        "edit": [{"fn": "percent", "replace_op": "entry.zero", "with": ["const", 5]}],
        "patch": [{"fn": "percent", "blocks": {"done": {"params": [["v", "i64"]], "ops": [["r", "ok", "v"]], "term": ["return", "r"]}}}]});
    let (status, text) = run(&temp.path, &["try", &twice.to_string()]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("is restated more than once in this frame"),
        "{text}"
    );
    let same = json!({"af1": 1, "edit": [
        {"fn": "percent", "replace_op": "entry.zero", "with": ["const", 5]},
        {"fn": "percent", "replace_op": "entry.zero", "with": ["const", 6]}]});
    let (status, text) = run(&temp.path, &["try", &same.to_string()]);
    assert_eq!(status, 2, "{text}");
    assert!(text.contains("edited twice"), "{text}");
}

#[test]
fn values_round_trip_at_the_edges() {
    // Review findings 4 and 5: u128 above i128::MAX, and built-in failures
    // as call renders them.
    let temp = workspace("value-edges", None);
    let frame = json!({"af1": 1, "fns": [
        {"fn": "id128", "params": [["x", "u128"]], "returns": "u128",
         "blocks": [{"name": "entry", "ops": [], "term": ["return", "x"]}]},
        {"fn": "twice", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
         "blocks": [{"name": "entry", "ops": [["r", "add", "a", "a"]], "term": ["return", "r"]}]}]});
    assert_eq!(run(&temp.path, &["try", &frame.to_string()]).0, 0);
    let max = "\"340282366920938463463374607431768211455\"";
    let (_, value) = run(&temp.path, &["call", "id128", max, "--on", "latest"]);
    assert_eq!(value.trim(), max);
    let (_, failure) = run(
        &temp.path,
        &["call", "twice", "9223372036854775807", "--on", "latest"],
    );
    assert_eq!(
        failure.trim(),
        "{\"Err\":{\"ArithmeticError\":\"Overflow\"}}"
    );
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    let expect: Value = serde_json::from_str(failure.trim()).unwrap();
    let tests = json!({"af1": 1, "tests": [{"fn": "twice", "args": [9_223_372_036_854_775_807_i64], "expect": expect}]});
    let (status, text) = run(&temp.path, &["try", &tests.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("tests: 1/1 passed"), "{text}");
}

#[test]
fn restating_a_type_keeps_what_the_frame_does_not_state() {
    // Review finding 6, and a frame that changes nothing.
    let temp = workspace("type-visibility", None);
    let private = json!({"af1": 1, "types": [{"name": "Pv", "variant": ["A", "B"], "visibility": "private"}]});
    assert_eq!(run(&temp.path, &["try", &private.to_string()]).0, 0);
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    let restated = json!({"af1": 1, "types": [{"name": "Pv", "variant": ["A", "B"]}]});
    let (status, text) = run(&temp.path, &["try", &restated.to_string()]);
    assert_eq!(status, 2, "{text}");
    assert!(text.contains("the frame changes nothing"), "{text}");
}

#[test]
fn a_phase_five_locator_never_names_a_reference_that_resolved() {
    // Review finding 7: a malformed immediate on `eq`, whose operands resolve.
    let temp = percent_program("phase-five");
    let raw = json!([{"class": "ReplaceEntityVersion", "kind": 8, "target": "percent.entry.is_zero",
        "payload": {"block": "percent.entry", "ordinal": 1, "opcode": 96,
            "operands": [{"variant": "Parameter", "value": "percent.whole"},
                         {"variant": "OperationResult", "value": {"operation": "percent.entry.zero"}}],
            "result_types": ["bool"], "immediate": {"variant": "Entity", "value": "k_100"}}}]);
    let (status, text) = run(&temp.path, &["try", &raw.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert!(
        text.contains("where: Operation percent.entry.is_zero\n"),
        "{text}"
    );
}

#[test]
fn inference_and_names_cover_the_documented_cases() {
    // Review findings 8 and 9, and a block named like a parameter.
    let temp = workspace("inference", None);
    let frame = json!({"af1": 1,
        "types": [{"name": "my-err", "variant": ["Bad"]}, {"name": "Sh", "variant": [["Circle", "i64"], "Empty"]}],
        "fns": [
          {"fn": "f", "params": [["x", "i64"]], "returns": "Result<i64,my-err>",
           "blocks": [{"name": "entry", "ops": [["r", "ok", "x"]], "term": ["return", "r"]}]},
          {"fn": "second", "params": [["a", "i64"], ["b", "i64"]], "returns": "i64",
           "blocks": [{"name": "entry", "ops": [["t", "tuple", "a", "b"], ["x", "tuple_get", 1, "t"]], "term": ["return", "x"]}]},
          {"fn": "rad", "params": [["s", "Sh"]], "returns": "Option<i64>",
           "blocks": [{"name": "entry", "ops": [["r", "variant_get", "Sh.Circle", "s"]], "term": ["return", "r"]}]},
          {"fn": "sw", "params": [["a", "i64"]], "returns": "Option<i64>",
           "blocks": [{"name": "entry", "ops": [["d", "add", "a", "a"], ["n", "none"]],
                       "term": ["switch", "d", ["Ok", "done", "$"], ["Err", "fail", "n"]]},
                      {"name": "done", "params": [["v", "i64"]], "ops": [["r", "some", "v"]], "term": ["return", "r"]},
                      {"name": "fail", "params": [["m", "Option<i64>"]], "ops": [], "term": ["return", "m"]}]}],
        "tests": [{"fn": "f", "args": [3], "expect": {"Ok": 3}},
                  {"fn": "second", "args": [3, 4], "expect": 4},
                  {"fn": "rad", "args": [{"Circle": 5}], "expect": {"Some": 5}},
                  {"fn": "sw", "args": [9_223_372_036_854_775_807_i64], "expect": "None"}]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("tests: 4/4 passed"), "{text}");
    let clash = json!({"af1": 1, "fns": [{"fn": "q", "params": [["x", "i64"]], "returns": "i64",
        "blocks": [{"name": "entry", "ops": [], "term": ["br", "x"]}, {"name": "x", "ops": [], "term": ["return", "x"]}]}]});
    let (status, text) = run(&temp.path, &["try", &clash.to_string()]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("block `x` has the name of a parameter of `q`"),
        "{text}"
    );
}

#[test]
fn references_resolve_to_short_handles() {
    // Review findings 10, 11 and 12.
    let temp = committed_program("references", None);
    let frame =
        json!({"af1": 1, "tests": [{"fn": "bound", "args": [5, 0, 10], "expect": {"Ok": 5}}]});
    assert_eq!(run(&temp.path, &["try", &frame.to_string()]).0, 0);
    let (status, text) = run(&temp.path, &["submit", "latest"]);
    assert_eq!(status, 0, "{text}");
    assert!(text.starts_with("submitted c2 "), "{text}");
    let hex = fs::read_to_string(temp.path.join(".sley/candidates/c2.hex")).unwrap();
    let (status, text) = run(&temp.path, &["submit", hex.trim()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.starts_with("submitted c3 "), "{text}");
    let (_, text) = run(&temp.path, &["status"]);
    assert!(!text.contains(hex.trim()), "{text}");
    assert!(text.contains("after=c3"), "{text}");
    let odd = temp.path.join("aéééééééééé");
    fs::write(&odd, &hex).unwrap();
    let (status, text) = run(&temp.path, &["explain", odd.to_str().unwrap()]);
    assert_eq!(status, 0, "{text}");
    let code = json!({"af1": 1, "patch": [{"fn": "bound", "blocks": {"above": {"ops": [["r", "ok", "high"]], "term": ["return", "r"]}}}]});
    assert_eq!(run(&temp.path, &["try", &code.to_string()]).0, 0);
    let (status, text) = run(&temp.path, &["commit", "latest"]);
    assert_eq!(status, 0, "{text}");
    assert!(text.starts_with("committed c4 "), "{text}");
}

#[test]
fn a_raw_create_cannot_take_a_live_name() {
    // Second-pass finding 1: a raw create keyed like a live top-level
    // entity silently renamed it.
    let temp = committed_program("raw-collision", None);
    let create = |extra: Value| {
        let mut list = vec![json!({"class": "CreateEntity", "kind": 9, "key": "bound",
            "payload": {"value": {"type": "i64", "data": {"variant": "SInt", "value": 1}}}})];
        if !extra.is_null() {
            list.push(extra);
        }
        Value::Array(list).to_string()
    };
    let (status, text) = run(&temp.path, &["try", &create(Value::Null)]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("`bound` already names a live entity"),
        "{text}"
    );
}

#[test]
fn explain_never_blames_unrelated_code() {
    // Second-pass finding 2: a CFG_RETURN_TYPE refusal used to show a
    // dominance finding from another block as its `where`.
    let temp = workspace("explain-unrelated", None);
    let frame = json!({"af1": 1, "fns": [{"fn": "bad", "params": [["a", "i64"]], "returns": "bool", "blocks": [
        {"name": "entry", "ops": [["c", "lt", "a", "a"]], "term": ["cond", "c", "l", "r"]},
        {"name": "l", "ops": [["m", "lt", "a", "a"]], "term": ["br", "j"]},
        {"name": "r", "ops": [["c2", "lt", "a", "a"]], "term": ["cond", "c2", "j", "w"]},
        {"name": "w", "ops": [], "term": ["return", "a"]},
        {"name": "j", "ops": [], "term": ["return", "l.m"]}]}]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert!(text.contains("symbol: CFG_RETURN_TYPE"), "{text}");
    // The dominance problem is real, but it is listed under `also:`, never
    // as the refusal's own `where` or hint.
    let own: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("also:"))
        .collect();
    assert!(!own.contains("dominat"), "{text}");
    assert!(
        text.contains("  also: the terminator of bad.j uses `l.m` defined in block bad.l"),
        "{text}"
    );
    let (_, result) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(result["also"].as_array().map(Vec::len), Some(1), "{result}");
}

#[test]
fn public_cases_compare_typed_values() {
    // Second-pass finding 3: expectations in the forms AF1 tests accept.
    let temp = workspace("public-typed", None);
    let frame = json!({"af1": 1, "fns": [
        {"fn": "stub", "params": [], "returns": "i64", "blocks": [{"name": "entry", "ops": [], "term": ["trap", "unreachable"]}]},
        {"fn": "twice", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
         "blocks": [{"name": "entry", "ops": [["r", "add", "a", "a"]], "term": ["return", "r"]}]}]});
    let cases = temp.path.join("cases.json");
    fs::write(&cases, json!([
        {"name": "trap", "function": "stub", "args": [], "expect": {"trap": "unreachable"}},
        {"name": "short", "function": "twice", "args": [9_223_372_036_854_775_807_i64], "expect": {"Err": "Overflow"}},
        {"name": "long", "function": "twice", "args": [9_223_372_036_854_775_807_i64], "expect": {"Err": {"ArithmeticError": "Overflow"}}},
        {"name": "wrong", "function": "twice", "args": [2], "expect": {"Ok": 5}}]).to_string()).unwrap();
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            &frame.to_string(),
            "--public",
            cases.to_str().unwrap(),
        ],
    );
    assert_eq!(status, 1, "the wrong case fails: {text}");
    assert!(text.contains("public: 3/4 passed"), "{text}");
}

#[test]
fn a_refused_submission_is_never_stored() {
    // Second-pass finding 4 (a regression of the import fix).
    let temp = committed_program("refused-import", None);
    let stale = fs::read_to_string(temp.path.join(".sley/candidates/c1.hex")).unwrap();
    let frame =
        json!({"af1": 1, "tests": [{"fn": "bound", "args": [5, 0, 10], "expect": {"Ok": 5}}]});
    assert_eq!(run(&temp.path, &["try", &frame.to_string()]).0, 0);
    let (status, text) = run(&temp.path, &["submit", stale.trim()]);
    assert_eq!(status, 2, "{text}");
    assert!(!text.contains(stale.trim()), "{text}");
    let (status, text) = run(&temp.path, &["submit"]);
    assert_eq!(status, 0, "{text}");
    assert!(text.starts_with("submitted c2 "), "{text}");
}

#[test]
fn maps_floats_and_one_line_batches_read_back() {
    // Second-pass findings 5, 6 and 7.
    let temp = workspace("maps-floats", None);
    let frame = json!({"af1": 1, "fns": [
        {"fn": "idm", "params": [["m", "Map<i64,i64>"]], "returns": "Map<i64,i64>",
         "blocks": [{"name": "entry", "ops": [], "term": ["return", "m"]}]},
        {"fn": "idv", "params": [["v", "Vec<i64>"]], "returns": "Vec<i64>",
         "blocks": [{"name": "entry", "ops": [], "term": ["return", "v"]}]},
        {"fn": "idf", "params": [["x", "f64"]], "returns": "f64",
         "blocks": [{"name": "entry", "ops": [], "term": ["return", "x"]}]}]});
    assert_eq!(run(&temp.path, &["try", &frame.to_string()]).0, 0);
    let (_, map) = run(
        &temp.path,
        &["call", "idm", "[[2,20],[1,10]]", "--on", "latest"],
    );
    assert_eq!(map.trim(), "[[1,10],[2,20]]");
    let (status, text) = run(
        &temp.path,
        &["call", "idm", "[[1,1],[1,2]]", "--on", "latest"],
    );
    assert_eq!(status, 2, "{text}");
    assert!(text.contains("a map key appears twice"), "{text}");
    for (value, rendered) in [
        ("\"NaN\"", "\"NaN\""),
        ("\"-inf\"", "\"-inf\""),
        ("1.5", "1.5"),
    ] {
        let (_, out) = run(&temp.path, &["call", "idf", value, "--on", "latest"]);
        assert_eq!(out.trim(), rendered);
    }
    let batch = temp.path.join("batch.txt");
    fs::write(&batch, "[[1,2,3]]\n").unwrap();
    let (status, out) = run(
        &temp.path,
        &[
            "call",
            "idv",
            "--batch",
            batch.to_str().unwrap(),
            "--on",
            "latest",
        ],
    );
    assert_eq!(status, 0, "{out}");
    assert_eq!(out.trim(), "[1,2,3]");
}

#[test]
fn bytes_appear_only_with_raw() {
    // BR-10: no record, stored or body hex by default; `--raw` prints it.
    let temp = committed_program("raw", None);
    let frame =
        json!({"af1": 1, "tests": [{"fn": "bound", "args": [5, 0, 10], "expect": {"Ok": 5}}]});
    let hex64 = |text: &str| {
        text.split(|c: char| !c.is_ascii_hexdigit())
            .any(|word| word.len() >= 64)
    };
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(!hex64(&text), "{text}");
    let (_, value) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert!(!hex64(&value.to_string()), "{value}");
    let (_, value) = run_json(&temp.path, &["try", &frame.to_string(), "--raw"]);
    let stored = value["stored_hex"].as_str().unwrap();
    let handle = value["handle"].as_str().unwrap();
    let file = fs::read_to_string(
        temp.path
            .join(".sley/candidates")
            .join(format!("{handle}.hex")),
    )
    .unwrap();
    assert_eq!(stored, file.trim());
    assert_eq!(run(&temp.path, &["submit", handle]).0, 0);
    let (status, text) = run(&temp.path, &["status"]);
    assert_eq!(status, 0, "{text}");
    assert!(!hex64(&text), "{text}");
    let (_, text) = run(&temp.path, &["status", "--raw"]);
    assert!(text.contains(&format!("stored: {stored}")), "{text}");
}

#[test]
fn init_grants_the_benchmark_fixture_ceilings() {
    // BR-03(b), ADR-0051 decision 9: pinned values, stated by `init`.
    let temp = TempDir::new("init-ceilings");
    let dir = temp.path.join("ws");
    let mut out = Vec::new();
    let status = sley_agent::cli::run(&["init".to_owned(), dir.display().to_string()], &mut out);
    let text = String::from_utf8(out).unwrap();
    assert_eq!(status, 0, "{text}");
    assert!(
        text.contains(
            "(policy: fuel 1000000, memory 16777216, output 65536, 10000 mutations per candidate)"
        ),
        "{text}"
    );
    assert_eq!(
        genesis::INIT_CEILINGS,
        PolicyResourceCeilings::new(1_000_000, 16_777_216, 65_536, 0, 10_000, 0)
    );
}

/// Every file under `dir`, with its bytes.
fn snapshot(dir: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut files = std::collections::BTreeMap::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(path.clone(), fs::read(&path).unwrap());
            }
        }
    }
    files
}

#[test]
fn call_and_test_write_nothing_to_the_repository() {
    // BR-04: results are advisory; the repository is untouched.
    let temp = committed_program("advisory", None);
    let frame = json!({"af1": 1, "tests": [
        {"fn": "bound", "args": [5, 0, 10], "expect": {"Ok": 5}},
        {"fn": "bound", "args": [15, 0, 10], "expect": {"Ok": 10}}]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(status, 0, "{text}");
    let before = snapshot(&temp.path.join("repo"));
    assert_eq!(run(&temp.path, &["call", "bound", "15", "0", "10"]).0, 0);
    assert_eq!(
        run(
            &temp.path,
            &["call", "bound", "15", "0", "10", "--on", "c2"]
        )
        .0,
        0
    );
    let (status, text) = run(&temp.path, &["test", "c2"]);
    assert_eq!(status, 1, "the buggy bound fails its second test: {text}");
    assert!(text.contains("tests: 1/2 passed"), "{text}");
    assert_eq!(snapshot(&temp.path.join("repo")), before);
}

#[test]
fn help_topics_and_hints_cover_the_common_refusals() {
    for topic in sley_agent::help::TOPICS {
        assert!(sley_agent::help::topic(topic).is_some(), "{topic}");
    }
    for symbol in [
        "CANDIDATE_TEST_RESOURCE_LIMIT",
        "GRAPH_UNRESOLVED_REFERENCE",
        "GRAPH_INVENTORY_MISMATCH",
        "CFG_DOMINANCE",
    ] {
        assert!(sley_agent::catalog::has_symbol_hint(symbol), "{symbol}");
    }
    assert_eq!(
        sley_agent::catalog::hint("NOT_A_REAL_SYMBOL", "NotADecision"),
        "no hint"
    );
}

#[test]
fn workbench_refusal_symbols_are_emitted() {
    let temp = TempDir::new("refusals");
    let (status, text) = run(&temp.path, &["view"]);
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_WORKSPACE_INVALID"), "{text}");
    let temp = workspace("symbols", None);
    for (args, symbol) in [
        (vec!["frobnicate"], "AGENT_USAGE_INVALID"),
        (vec!["view", "nothing_here"], "AGENT_NAME_UNKNOWN"),
        (vec!["submit", "c9"], "AGENT_HANDLE_UNKNOWN"),
        (vec!["try", "/nonexistent/frame.json"], "AGENT_IO_FAILED"),
    ] {
        let (status, text) = run(&temp.path, &args);
        assert_eq!(status, 2, "{args:?}");
        assert!(text.contains(symbol), "{args:?}: {text}");
    }
    let frame = json!({"af1": 1, "fns": [{"fn": "f", "params": [["x", "i64"]], "returns": "i64",
        "blocks": [{"name": "entry", "term": ["return", "x"]}]}]});
    assert_eq!(run(&temp.path, &["try", &frame.to_string()]).0, 0);
    let (status, text) = run(&temp.path, &["call", "f", "\"text\"", "--on", "c1"]);
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_INPUT_INVALID"), "{text}");
    let (status, text) = run(&temp.path, &["call", "f", "1"]);
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_NAME_UNKNOWN"), "{text}");
    assert!(
        text.contains("AGENT_NAME_UNKNOWN") || text.contains("AGENT_EXECUTION_REFUSED"),
        "{text}"
    );
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            "{\"af1\": 1, \"tests\": [{\"fn\": \"f\", \"args\": [], \"expect\": 1}]}",
        ],
    );
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_FRAME_INVALID"), "{text}");
    let raw = json!([{"class": "ReplaceEntityVersion", "kind": 5, "target": "00".repeat(32), "payload": {}}]);
    let (status, text) = run(&temp.path, &["try", &raw.to_string()]);
    assert_eq!(status, 2);
    assert!(
        text.contains("AGENT_FRAME_INVALID") || text.contains("AGENT_CANDIDATE_INVALID"),
        "{text}"
    );
    let (status, text) = run(&temp.path, &["init", temp.path.to_str().unwrap()]);
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_WORKSPACE_INVALID"), "{text}");
    let not_a_workspace = TempDir::new("nowhere");
    let mut out = Vec::new();
    let previous = std::env::current_dir().unwrap();
    let _ = previous;
    let status = sley_agent::cli::run(
        &[
            "--workspace".into(),
            not_a_workspace.path.join("x").display().to_string(),
            "status".into(),
        ],
        &mut out,
    );
    assert_eq!(status, 2);
    let _ = sley_agent::AgentErrorCode::WorkspaceNotFound.symbol();
    assert_eq!(
        sley_agent::AgentErrorCode::ExecutionRefused.symbol(),
        "AGENT_EXECUTION_REFUSED"
    );
    assert_eq!(
        sley_agent::AgentErrorCode::CandidateInvalid.symbol(),
        "AGENT_CANDIDATE_INVALID"
    );
    assert_eq!(
        sley_agent::AgentErrorCode::WorkspaceNotFound.symbol(),
        "AGENT_WORKSPACE_NOT_FOUND"
    );
}

#[test]
fn the_guides_inline_edge_examples_compile_and_run() {
    // Every inline terminator example in the guide runs in one program, so
    // the guide and the frame compiler cannot drift apart. (An earlier
    // guide taught `["br", ["join", "x"]]`, which was refused.)
    let guide = sley_agent::help::GUIDE;
    let mut examples = Vec::new();
    for prefix in ["`[\"br\"", "`[\"cond\"", "`[\"switch\""] {
        for (start, _) in guide.match_indices(prefix) {
            let rest = &guide[start + 1..];
            let snippet = &rest[..rest.find('`').unwrap()];
            // The guide elides the switch's other cases.
            let snippet = snippet.replace(", ...]", ", [\"Err\", \"bad\", \"$\"]]");
            examples.push(serde_json::from_str::<Value>(&snippet).unwrap());
        }
    }
    let term = |op: &str| {
        let found: Vec<&Value> = examples.iter().filter(|example| example[0] == op).collect();
        assert_eq!(found.len(), 1, "one inline `{op}` example in the guide");
        found[0].clone()
    };
    let frame = json!({"af1": 1, "fns": [{"fn": "edges", "params": [["x", "i64"], ["c", "bool"]],
        "returns": "Result<i64,ArithmeticError>", "blocks": [
        {"name": "entry", "ops": [], "term": term("cond")},
        {"name": "t", "params": [["y", "i64"]], "ops": [["v", "add", "y", "y"]], "term": term("switch")},
        {"name": "next", "params": [["s", "i64"], ["z", "i64"]], "ops": [["r", "ok", "s"]], "term": ["return", "r"]},
        {"name": "f", "ops": [], "term": term("br")},
        {"name": "join", "params": [["j", "i64"]], "ops": [["r", "ok", "j"]], "term": ["return", "r"]},
        {"name": "bad", "params": [["e", "ArithmeticError"]], "ops": [["r", "err", "e"]], "term": ["return", "r"]}]}],
        "tests": [{"fn": "edges", "args": [3, true], "expect": {"Ok": 6}},
                  {"fn": "edges", "args": [3, false], "expect": {"Ok": 3}}]});
    let temp = workspace("guide-edges", None);
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("tests: 2/2 passed"), "{text}");
}

/// A function whose entry block ends in `term`, over `join(j: i64)`.
fn edge_frame(term: &Value) -> Value {
    json!({"af1": 1, "fns": [{"fn": "g", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
        "blocks": [
        {"name": "entry", "ops": [["d", "add", "a", "a"]], "term": term},
        {"name": "join", "params": [["j", "i64"]], "ops": [["r", "ok", "j"]], "term": ["return", "r"]}]}]})
}

#[test]
fn edge_mistakes_name_the_block_and_the_fix() {
    let temp = workspace("edges", None);
    for term in [json!(["br", "join", "a"]), json!(["br", ["join", "a"]])] {
        let (status, text) = run(&temp.path, &["try", &edge_frame(&term).to_string()]);
        assert_eq!(status, 0, "{term}: {text}");
    }
    for (term, message) in [
        (
            json!(["br", "join", ["a"]]),
            "/fns/0/blocks/0/term: edge arguments are value names: write [\"join\", \"x\", \"y\"], not [\"join\", [\"x\", \"y\"]]",
        ),
        (
            json!(["br", "join"]),
            "/fns/0/blocks/0/term: block `join` takes 1 argument(s) (j: i64); this edge passes 0",
        ),
        (
            json!(["br", ["join", "d"]]),
            "/fns/0/blocks/0/term: `d` is Result<i64,ArithmeticError> but parameter `j` of block `join` is i64; switch on it and pass the case's `$`",
        ),
    ] {
        let (status, text) = run(&temp.path, &["try", &edge_frame(&term).to_string()]);
        assert_eq!(status, 2, "{term}: {text}");
        assert!(text.contains(message), "{term}: {text}");
    }
}

#[test]
fn operand_type_mistakes_are_named_at_the_operation() {
    let temp = workspace("operands", None);
    let frame = |ops: Value| {
        json!({"af1": 1, "fns": [{"fn": "g", "params": [["a", "i64"], ["flag", "bool"]],
            "returns": "bool", "blocks": [{"name": "entry", "ops": ops, "term": ["return", "flag"]}]}]})
    };
    for (ops, message) in [
        (
            json!([["d", "add", "a", "a"], ["e", "add", "d", "a"]]),
            "/fns/0/blocks/0/ops/1: `d` is a Result<i64,ArithmeticError>, not a value `add` can use: switch on it first",
        ),
        (
            json!([["c", "lt", "a", "flag"]]),
            "/fns/0/blocks/0/ops/0: the operands of `lt` must have one type: `a` is i64, `flag` is bool",
        ),
        (
            json!([["c", "and", "flag", "a"]]),
            "/fns/0/blocks/0/ops/0: `and` takes bool operands; `a` is i64",
        ),
    ] {
        let (status, text) = run(&temp.path, &["try", &frame(ops.clone()).to_string()]);
        assert_eq!(status, 2, "{ops}: {text}");
        assert!(text.contains(message), "{ops}: {text}");
    }
}

#[test]
fn one_refusal_covers_every_function_of_the_frame() {
    // Problems in different functions arrive in one refusal; the headline
    // carries the first pointer, so its first line alone locates it.
    let temp = workspace("every-fn", None);
    let function = |name: &str| {
        json!({"fn": name, "params": [["a", "i64"]], "returns": "bool", "blocks": [
            {"name": "entry", "ops": [["d", "add", "a", "a"], ["c", "lt", "d", "a"]], "term": ["return", "c"]}]})
    };
    let frame = json!({"af1": 1, "fns": [function("f"), function("g")]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 2, "{text}");
    let mut lines = text.lines();
    let headline = lines.next().unwrap();
    assert!(
        headline.starts_with("error AGENT_FRAME_INVALID: /fns/0/blocks/0/ops/1: `d` is a Result"),
        "{text}"
    );
    assert!(headline.ends_with("(1 of 2 problems)"), "{text}");
    assert!(
        lines
            .next()
            .unwrap()
            .starts_with("  /fns/1/blocks/0/ops/1: `d` is a Result"),
        "{text}"
    );
}

#[test]
fn functions_that_call_functions_run_from_one_loaded_image() {
    // The executor loads each image once and runs every input against it;
    // an entry with callees must answer as if each call loaded it afresh.
    let temp = workspace("callees", None);
    let frame = json!({"af1": 1, "fns": [
        {"fn": "double", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
         "blocks": [{"name": "entry", "ops": [["r", "add", "a", "a"]], "term": ["return", "r"]}]},
        {"fn": "quad", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>", "blocks": [
            {"name": "entry", "ops": [["d", "call", "double", "a"]], "term": ["switch", "d", ["Ok", "again", "$"], ["Err", "bad", "$"]]},
            {"name": "again", "params": [["h", "i64"]], "ops": [["q", "call", "double", "h"]], "term": ["return", "q"]},
            {"name": "bad", "params": [["e", "ArithmeticError"]], "ops": [["r", "err", "e"]], "term": ["return", "r"]}]}],
        "tests": [{"fn": "quad", "args": [3], "expect": {"Ok": 12}},
                  {"fn": "quad", "args": [4_611_686_018_427_387_904_i64], "expect": {"Err": "Overflow"}}]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("tests: 2/2 passed"), "{text}");
    let batch = temp.path.join("batch.txt");
    fs::write(&batch, "[1]\n[-5]\n[2305843009213693952]\n[0]\n").unwrap();
    let (status, text) = run(
        &temp.path,
        &[
            "call",
            "quad",
            "--batch",
            batch.to_str().unwrap(),
            "--on",
            "c1",
        ],
    );
    assert_eq!(status, 0, "{text}");
    assert_eq!(
        text.lines().collect::<Vec<_>>(),
        [
            "{\"Ok\":4}",
            "{\"Ok\":-20}",
            "{\"Err\":{\"ArithmeticError\":\"Overflow\"}}",
            "{\"Ok\":0}"
        ]
    );
}

#[test]
fn try_on_a_handle_layers_a_small_frame_on_its_frame() {
    // A follow-up adds tests or fixes one operation without restating the
    // frame.
    let temp = workspace("layer", None);
    let (status, text) = run(&temp.path, &["try", &clamp_frame(true).to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(
        text.contains("next: add tests without restating the frame: sley-agent try --on d1"),
        "{text}"
    );
    let tests = json!({"af1": 1, "tests": [
        {"name": "t_in", "fn": "bound", "args": [5, 0, 10], "expect": {"Ok": 5}},
        {"name": "t_above", "fn": "bound", "args": [11, 0, 10], "expect": {"Ok": 10}}]});
    let (status, text) = run(&temp.path, &["try", "--on", "c1", &tests.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert!(text.starts_with("c2: Valid"), "{text}");
    assert!(text.contains("FAIL t_above"), "{text}");
    assert!(
        text.contains("next: fix only what failed on top of d2: sley-agent try --on d2"),
        "{text}"
    );
    // The fix names one operation of a function the base frame defines.
    let fix = json!({"af1": 1, "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "high"]}]});
    let (status, text) = run(&temp.path, &["try", "--on", "c2", &fix.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.starts_with("c3: Valid"), "{text}");
    assert!(text.contains("tests: 2/2 passed"), "{text}");
    assert_eq!(run(&temp.path, &["submit", "c3"]).0, 0);
    // A frame refusal in a layered frame says where its pointers point.
    let broken = json!({"af1": 1, "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "nowhere"]}]});
    let (status, text) = run(&temp.path, &["try", "--on", "c3", &broken.to_string()]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains(
            "pointers refer to .sley/drafts/d4/r1/frame.json (the frame of c3 with yours on top"
        ),
        "{text}"
    );
    assert!(temp.path.join(".sley/layered.json").is_file());
    // Raw operations leave no frame to build on.
    let raw = json!([{"class": "CreateEntity", "kind": 9, "key": "limit",
        "payload": {"value": {"value_type": "i64", "data": {"variant": "SInt", "value": 1}}}}]);
    let (status, text) = run(&temp.path, &["try", &raw.to_string()]);
    assert_eq!(status, 0, "{text}");
    let handle = text.split(':').next().unwrap().to_owned();
    let (status, text) = run(&temp.path, &["try", "--on", &handle, &tests.to_string()]);
    assert_eq!(status, 2, "{text}");
    assert!(text.contains("was not made from an AF1 frame"), "{text}");
}

#[test]
fn a_frame_file_refusal_says_to_edit_the_file_in_place() {
    let temp = workspace("edit-in-place", None);
    let path = temp.path.join("frame.json");
    let mut frame = clamp_frame(false);
    frame["fns"][0]["blocks"][1]["ops"] = json!([
        ["e", "variant", "RangeError.Inverted"],
        ["r", "err", "nope"]
    ]);
    fs::write(&path, frame.to_string()).unwrap();
    let (status, text) = run(&temp.path, &["try", path.to_str().unwrap()]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains(&format!(
            "fix: edit {} in place at those pointers (no need to rewrite it)",
            path.display()
        )),
        "{text}"
    );
}

#[test]
fn malformed_terminators_and_cases_name_the_fix() {
    let temp = workspace("shapes", None);
    let frame = |entry_term: Value, ops: Value| {
        json!({"af1": 1, "types": [{"name": "E", "variant": ["Bad"]}],
          "fns": [{"fn": "f", "params": [["a", "i64"], ["c", "bool"]], "returns": "Result<i64,E>", "blocks": [
            {"name": "entry", "comment": "a note for the reader", "ops": ops, "term": entry_term},
            {"name": "t", "params": [["x", "i64"]], "ops": [["r", "ok", "x"]], "term": ["return", "r"]},
            {"name": "f", "ops": [["e", "variant", "E.Bad"], ["r", "err", "e"]], "term": ["return", "r"]}]}],
          "comment": "frames may carry comments"})
    };
    // A bracketed target whose block is named like a case stays valid.
    let mut named_like_a_case = frame(
        json!(["switch", "d", ["Ok", ["t", "$"]], ["Err", ["f"]]]),
        json!([["d", "add", "a", "a"]]),
    );
    named_like_a_case["fns"][0]["blocks"][1]["name"] = json!("Ok");
    named_like_a_case["fns"][0]["blocks"][0]["term"] =
        json!(["switch", "d", ["Ok", ["Ok", "$"]], ["Err", ["f"]]]);
    let (status, text) = run(&temp.path, &["try", &named_like_a_case.to_string()]);
    assert_eq!(status, 0, "{text}");
    // Comments are accepted, and the well-formed frame is Valid.
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            &frame(json!(["cond", "c", ["t", "a"], "f"]), json!([])).to_string(),
        ],
    );
    assert_eq!(status, 0, "{text}");
    for (term, ops, message) in [
        (
            json!(["cond", "c", "f", "t", "a"]),
            json!([]),
            "/fns/0/blocks/0/term: `cond` takes [\"cond\", c, then, else], not 5 items; a target with arguments is bracketed, e.g. [\"cond\",\"c\",\"f\",[\"t\",\"a\"]]",
        ),
        (
            json!(["return"]),
            json!([]),
            "`return` takes one value: [\"return\", v], not 1 items",
        ),
        (
            json!(["switch", "d", ["Ok", ["t", "$"], ["Err", "f"]]]),
            json!([["d", "add", "a", "a"]]),
            "/fns/0/blocks/0/term/2: the `Ok` case contains the `Err` case: close [\"Ok\", ...] before [\"Err\", ...]",
        ),
        (
            json!(["return", "a"]),
            json!([["x", "cond", "c", "t", "f"]]),
            "/fns/0/blocks/0/ops/0: `cond` is a terminator, not an operation: it goes in the block's \"term\"",
        ),
    ] {
        let (status, text) = run(&temp.path, &["try", &frame(term.clone(), ops).to_string()]);
        assert_eq!(status, 2, "{term}: {text}");
        assert!(text.contains(message), "{term}: {text}");
    }
}

#[test]
fn a_misspelled_terminator_is_named_before_the_types_it_hides() {
    // `retrun` hides the use that types `r`; the refusal names the
    // terminator first and the inference failure as its consequence.
    let temp = workspace("root-cause", None);
    let frame = json!({"af1": 1, "fns": [{"fn": "f", "params": [["x", "i64"]],
        "returns": "Result<i64,ArithmeticError>",
        "blocks": [{"name": "entry", "ops": [["r", "ok", "x"]], "term": ["retrun", "r"]}]}]});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 2, "{text}");
    let first = text.lines().next().unwrap();
    assert!(
        first.contains("/fns/0/blocks/0/term: bad terminator `retrun`"),
        "{text}"
    );
    assert!(text.contains("(1 of 2 problems)"), "{text}");
    assert!(
        text.contains("/fns/0/blocks/0/ops/0: cannot infer the result type of `r`"),
        "{text}"
    );
}
