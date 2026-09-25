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
            verdict["hint"].as_str().is_some_and(|hint| hint != "no hint"),
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
    assert_eq!(run(&temp.path, &["submit", "c3"]).0, 0);
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
}

#[test]
fn the_guide_is_small_and_every_example_runs() {
    let guide = sley_agent::help::GUIDE;
    assert!(guide.len() <= 8 * 1024, "guide is {} bytes", guide.len());
    let temp = workspace("guide", None);
    let examples: Vec<&str> = guide
        .split("```json\n")
        .skip(1)
        .map(|rest| rest.split("```").next().unwrap())
        .collect();
    assert!(examples.len() >= 3);
    for (index, example) in examples.iter().enumerate() {
        let (status, text) = run(&temp.path, &["try", example]);
        assert_eq!(status, 0, "guide example {index}: {text}");
        if index == 0 {
            assert!(text.contains("tests: 3/3 passed"), "{text}");
            // Commit the definitions so the edit and patch examples apply.
            let mut frame: Value = serde_json::from_str(example).unwrap();
            frame.as_object_mut().unwrap().remove("tests");
            assert_eq!(run(&temp.path, &["try", &frame.to_string()]).0, 0);
            assert_eq!(run(&temp.path, &["commit"]).0, 0);
        }
    }
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
