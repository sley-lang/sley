//! Authored locations for function-wide refusals: a phase 7 refusal of a
//! candidate made from a frame names the frame positions the advisory
//! analysis ties to the kernel's symbol, or says the location is the whole
//! function (`docs/spec/SLEY_AGENT_V1.md` section 8).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sley_agent::genesis;

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

fn workspace(label: &str) -> TempDir {
    let temp = TempDir::new(label);
    genesis::init(&temp.path, Some([5; 32]), genesis::INIT_CEILINGS).unwrap();
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

fn commit(dir: &Path, frame: &Value) {
    let (status, text) = run(dir, &["try", "--no-test", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    let (status, text) = run(dir, &["commit"]);
    assert_eq!(status, 0, "{text}");
}

/// Tries `frame` (refused at phase 7), and checks that `try` and `explain`
/// print the same `authored:` line, that the JSON verdicts carry the same
/// `authored` array, and that every pointer resolves in `resolve_in`
/// (the frame, or the layered frame for `try --on`). Returns the verdict.
fn refused(dir: &Path, args: &[&str], frame: &Value, symbol: &str, authored: &str) -> Value {
    let text_frame = frame.to_string();
    let mut words = vec!["try", "--no-test"];
    words.extend_from_slice(args);
    words.push(&text_frame);
    let (status, text) = run(dir, &words);
    assert_eq!(status, 1, "{text}");
    assert!(text.contains(&format!("  symbol: {symbol} (")), "{text}");
    assert!(text.contains("at phase 7 (control flow)"), "{text}");
    let line = format!("  authored: {authored}\n");
    assert!(text.contains(&line), "{text}");
    let handle = text.split(':').next().unwrap().to_owned();
    let (status, explained) = run(dir, &["explain", &handle]);
    assert_eq!(status, 1, "{explained}");
    assert!(explained.contains(&line), "{explained}");
    let (_, value) = run_json(dir, &words);
    let verdict = value["verdict"].clone();
    assert_eq!(verdict["symbol"], symbol, "{value}");
    assert_eq!(verdict["phase"], 7, "{value}");
    let (_, explained) = run_json(dir, &["explain", &handle]);
    assert_eq!(explained["verdict"]["authored"], verdict["authored"]);
    let resolve_in = if args.contains(&"--on") {
        let text = fs::read_to_string(dir.join(".sley/layered.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    } else {
        frame.clone()
    };
    for item in verdict["authored"].as_array().unwrap() {
        if let Some(at) = item["at"].as_str() {
            assert!(resolve_in.pointer(at).is_some(), "{at} in {resolve_in}");
        }
    }
    verdict
}

fn g_and_h(g_param: &str, h_body_arg: &str) -> Value {
    json!({"af1": 1, "fns": [
        {"fn": "g", "params": [["x", g_param]], "returns": g_param,
         "blocks": [{"name": "entry", "term": ["return", "x"]}]},
        {"fn": "h", "params": [["a", h_body_arg]], "returns": g_param,
         "blocks": [{"name": "entry", "ops": [["r", "call", "g", "a"]], "term": ["return", "r"]}]}]})
}

#[test]
fn a_function_wide_signature_mismatch_names_the_function_and_its_signature() {
    let temp = workspace("loc-wide");
    // The declared result contradicts `add`; nothing narrower is known.
    let frame = json!({"af1": 1, "fns": [{"fn": "total", "params": [["a", "i64"], ["b", "i64"]],
        "returns": "i32", "blocks": [{"name": "entry",
        "ops": [{"name": "s", "op": "add", "args": ["a", "b"], "type": "i32"}],
        "term": ["return", "s"]}]}]});
    let verdict = refused(
        &temp.path,
        &[],
        &frame,
        "VM_LOWER_SIGNATURE_MISMATCH",
        "/fns/0 (function-wide; the kernel names no smaller location), \
         /fns/0/params (parameters of total), /fns/0/returns (result of total)",
    );
    // The kernel's own locator is unchanged.
    assert_eq!(verdict["where"], "Function total");
    assert_eq!(verdict["decision"], "ControlFlowError");
    assert_eq!(
        verdict["authored"],
        json!([
            {"at": "/fns/0", "what": "function-wide; the kernel names no smaller location"},
            {"at": "/fns/0/params", "what": "parameters of total"},
            {"at": "/fns/0/returns", "what": "result of total"}])
    );
}

#[test]
fn a_call_argument_mismatch_names_the_call_and_the_callee_parameters() {
    let temp = workspace("loc-call");
    let frame = g_and_h("bool", "i64");
    let verdict = refused(
        &temp.path,
        &[],
        &frame,
        "VM_LOWER_SIGNATURE_MISMATCH",
        "/fns/1/blocks/0/ops/0 (h.entry.r), /fns/0/params (parameters of g)",
    );
    assert_eq!(
        verdict["where"],
        "Function h: operation h.entry.r passes `a` (i64) as argument 0 of g, which takes bool"
    );
}

#[test]
fn a_changed_callee_signature_is_named_when_the_refused_caller_is_not_in_the_frame() {
    let temp = workspace("loc-callee");
    commit(&temp.path, &g_and_h("i64", "i64"));
    let frame =
        json!({"af1": 1, "patch": [{"fn": "g", "params": [["x", "i32"]], "returns": "i32"}]});
    let verdict = refused(
        &temp.path,
        &[],
        &frame,
        "VM_LOWER_SIGNATURE_MISMATCH",
        "/patch/0/params (parameters of g), /patch/0/returns (result of g)",
    );
    assert!(
        verdict["where"]
            .as_str()
            .unwrap()
            .starts_with("Function h: operation h.entry.r passes `a` (i64) as argument 0 of g"),
        "{verdict}"
    );
}

#[test]
fn a_constant_type_change_is_named_at_the_constant() {
    let temp = workspace("loc-const");
    commit(
        &temp.path,
        &json!({"af1": 1, "consts": [{"name": "k", "type": "i64", "value": 3}],
          "fns": [{"fn": "h", "params": [["a", "i64"]], "returns": "i64",
                   "blocks": [{"name": "entry", "ops": [["v", "const", "k"]], "term": ["return", "v"]}]}]}),
    );
    let frame = json!({"af1": 1, "consts": [{"name": "k", "type": "bool", "value": true}]});
    let verdict = refused(
        &temp.path,
        &[],
        &frame,
        "VM_LOWER_SIGNATURE_MISMATCH",
        "/consts/0/type (type of constant k)",
    );
    assert_eq!(
        verdict["where"],
        "Function h: operation h.entry.v declares result i64 but constant k is bool"
    );
}

#[test]
fn a_return_type_refusal_names_the_return_and_the_result() {
    let temp = workspace("loc-return");
    let frame = json!({"af1": 1, "fns": [{"fn": "flag", "params": [["a", "i64"], ["b", "i64"]],
        "returns": "bool", "blocks": [
          {"name": "entry", "ops": [["c", "lt", "a", "b"]], "term": ["cond", "c", "yes", "no"]},
          {"name": "yes", "term": ["return", "entry.c"]},
          {"name": "no", "term": ["return", "a"]}]}]});
    let verdict = refused(
        &temp.path,
        &[],
        &frame,
        "CFG_RETURN_TYPE",
        "/fns/0/blocks/2/term (terminator of flag.no), /fns/0/returns (result of flag)",
    );
    assert_eq!(
        verdict["where"],
        "Function flag: the terminator of flag.no returns `a` (i64) but flag returns bool"
    );
}

#[test]
fn a_refused_function_outside_the_frame_with_nothing_narrower_says_so() {
    let temp = workspace("loc-none");
    commit(
        &temp.path,
        &json!({"af1": 1, "types": [{"name": "T", "variant": [["A", "i64"]]}],
          "fns": [{"fn": "h", "params": [["a", "i64"]], "returns": "T",
                   "blocks": [{"name": "entry", "ops": [["v", "variant", "T.A", "a"]], "term": ["return", "v"]}]}]}),
    );
    let frame = json!({"af1": 1, "types": [{"name": "T", "variant": [["A", "bool"]]}]});
    let verdict = refused(
        &temp.path,
        &[],
        &frame,
        "VM_LOWER_SIGNATURE_MISMATCH",
        "none (h is not in this frame; the kernel names the whole function)",
    );
    assert_eq!(verdict["where"], "Function h");
    assert_eq!(
        verdict["authored"],
        json!([{"at": null, "what": "h is not in this frame; the kernel names the whole function"}])
    );
}

#[test]
fn structural_findings_map_to_their_authored_blocks_and_layered_frames() {
    let temp = workspace("loc-layered");
    let frame = json!({"af1": 1, "fns": [{"fn": "pick", "params": [["a", "i64"], ["b", "i64"]], "returns": "i64",
        "blocks": [
          {"name": "entry", "ops": [["c", "gt", "a", "b"]], "term": ["cond", "c", "left", "right"]},
          {"name": "left", "ops": [["m", "const", 5]], "term": ["br", "join"]},
          {"name": "right", "term": ["br", "join"]},
          {"name": "join", "term": ["return", "left.m"]}]}]});
    refused(
        &temp.path,
        &[],
        &frame,
        "CFG_DOMINANCE",
        "/fns/0/blocks/3/term (terminator of pick.join)",
    );
    // Layered: the base is Valid; the patch changes g, and the layered
    // frame recompiles h, whose call now returns bool. Pointers refer to the
    // layered frame, where the patch merged into g's definition.
    let (status, text) = run(
        &temp.path,
        &["try", "--no-test", &g_and_h("i64", "i64").to_string()],
    );
    assert_eq!(status, 0, "{text}");
    let base = text.split(':').next().unwrap().to_owned();
    let patch =
        json!({"af1": 1, "patch": [{"fn": "g", "params": [["x", "bool"]], "returns": "bool"}]});
    refused(
        &temp.path,
        &["--on", &base],
        &patch,
        "CFG_RETURN_TYPE",
        "/fns/1/blocks/0/term (terminator of h.entry), /fns/1/returns (result of h)",
    );
}

#[test]
fn raw_candidates_and_narrow_locators_carry_no_authored_positions() {
    let temp = workspace("loc-raw");
    commit(
        &temp.path,
        &json!({"af1": 1, "consts": [{"name": "k", "type": "i64", "value": 3}],
          "fns": [{"fn": "h", "params": [["a", "i64"]], "returns": "i64",
                   "blocks": [{"name": "entry", "ops": [["v", "const", "k"]], "term": ["return", "v"]}]}]}),
    );
    let raw = json!([{"class": "ReplaceEntityVersion", "kind": 9, "target": "k",
        "payload": {"value": {"value_type": "bool", "data": {"variant": "Bool", "value": true}}}}]);
    let (status, text) = run(&temp.path, &["try", "--no-test", &raw.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert!(
        text.contains("symbol: VM_LOWER_SIGNATURE_MISMATCH"),
        "{text}"
    );
    assert!(!text.contains("authored:"), "{text}");
    let (_, value) = run_json(&temp.path, &["try", "--no-test", &raw.to_string()]);
    assert!(value["verdict"].get("authored").is_none(), "{value}");
    // A phase 12 locator names the test and the limit: nothing to add.
    let frame = json!({"af1": 1,
        "edit": [{"fn": "h", "replace_op": "entry.v", "with": ["const", 4]}],
        "tests": [{"name": "t", "fn": "h", "args": [1], "expect": 4,
                   "limits": {"memory_bytes": 100_000_000}}]});
    let (status, value) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 1, "{value}");
    assert_eq!(value["verdict"]["phase"], 12, "{value}");
    assert!(value["verdict"].get("authored").is_none(), "{value}");
}

#[test]
fn generated_names_resolve_through_the_source_map_names_table() {
    let temp = workspace("loc-sourcemap");
    let frame = json!({"af1": 1, "fns": [{"fn": "pick", "params": [["a", "i64"], ["b", "i64"]], "returns": "i64",
        "blocks": [
          {"name": "entry", "ops": [["c", "gt", "a", "b"]], "term": ["cond", "c", "left", "right"]},
          {"name": "left", "ops": [["m", "const", 5]], "term": ["br", "join__x"]},
          {"name": "right", "term": ["br", "join__x"]},
          {"name": "join__x", "term": ["return", "left.m"]}]}]});
    let (status, text) = run(&temp.path, &["try", "--no-test", &frame.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert!(
        text.contains("  authored: /fns/0/blocks/3/term (terminator of pick.join__x)\n"),
        "{text}"
    );
    // A candidate whose authored frame does not spell `join__x` (as an
    // authoring dialect's expansion would) maps it through the table.
    let meta_path = temp.path.join(".sley/candidates/c1.json");
    let mut meta: Value = serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
    let mut authored = frame.clone();
    authored["fns"][0]["blocks"]
        .as_array_mut()
        .unwrap()
        .truncate(3);
    meta["frame"] = authored;
    meta["sourcemap"] = json!({"names": {"pick": {"join__x": "/fns/0/blocks/1/term"}}});
    fs::write(&meta_path, meta.to_string()).unwrap();
    let (status, text) = run(&temp.path, &["explain", "c1"]);
    assert_eq!(status, 1, "{text}");
    assert!(
        text.contains("  authored: /fns/0/blocks/1/term (terminator of pick.join__x)\n"),
        "{text}"
    );
    // Without the table, the generated block is not in the authored frame:
    // the whole function is named instead of a guessed position.
    meta.as_object_mut().unwrap().remove("sourcemap");
    fs::write(&meta_path, meta.to_string()).unwrap();
    let (_, text) = run(&temp.path, &["explain", "c1"]);
    assert!(
        text.contains("  authored: /fns/0 (function-wide; the kernel names no smaller location)\n"),
        "{text}"
    );
}
