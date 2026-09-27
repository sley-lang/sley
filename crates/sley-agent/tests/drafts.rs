//! Drafts, delta repair, test import and the events ledger, end to end over
//! fresh `init` workspaces (`docs/spec/SLEY_AGENT_V1.md`, section 12).

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
            "sley-agent-drafts-{label}-{}-{}",
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

fn workspace(label: &str) -> TempDir {
    let temp = TempDir::new(label);
    genesis::init(&temp.path, Some([7; 32]), genesis::INIT_CEILINGS).unwrap();
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

fn status_of(dir: &Path, reference: &str) -> Value {
    let (status, value) = run_json(dir, &["draft", reference]);
    assert_eq!(status, 0, "{value}");
    value
}

fn revisions(dir: &Path, handle: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir.join(".sley/drafts").join(handle))
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('r'))
        .collect();
    names.sort();
    names
}

fn candidates(dir: &Path) -> usize {
    fs::read_dir(dir.join(".sley/candidates")).map_or(0, |entries| {
        entries
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".hex"))
            .count()
    })
}

/// A clamp with one wrong branch (`above` returns `low`) and two tests; the
/// second fails until the branch is fixed.
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
        {"name": "above", "ops": [["r", "ok", "low"]], "term": ["return", "r"]},
        {"name": "inside", "ops": [["r", "ok", "value"]], "term": ["return", "r"]}]}],
      "tests": [{"name": "t_in", "fn": "bound", "args": [5, 0, 10], "expect": {"Ok": 5}},
                {"name": "t_above", "fn": "bound", "args": [11, 0, 10], "expect": {"Ok": 10}}]})
}

const FIX: &str =
    r#"{"af1": 1, "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "high"]}]}"#;

/// A one-function frame (`f(a) = a`) without tests.
fn identity_frame() -> Value {
    json!({"af1": 1, "fns": [{"fn": "f", "params": [["a", "i64"]], "returns": "i64",
        "blocks": [{"name": "entry", "ops": [], "term": ["return", "a"]}]}]})
}

#[test]
fn every_try_records_a_revision_bound_to_its_candidate() {
    let temp = workspace("records");
    let (status, text) = run(&temp.path, &["try", &clamp_frame().to_string()]);
    assert_eq!(status, 1, "{text}");
    assert!(text.starts_with("c1: Valid ("), "{text}");
    assert!(
        text.lines().next().unwrap().ends_with(" draft d1@r1"),
        "{text}"
    );
    // Concise feedback: changed entities, failing tests only, draft steps.
    assert!(
        text.contains("changed: fn +bound; type +RangeError; test +t_above +t_in\n"),
        "{text}"
    );
    assert!(text.contains("tests: 1/2 passed [authored 2]\n"), "{text}");
    assert!(
        text.contains("  FAIL t_above (bound): expected Ok(10), got Ok(0)"),
        "{text}"
    );
    assert!(!text.contains("  ok   t_in"), "{text}");
    assert!(
        text.contains("next: fix only what failed on top of d1: sley-agent try --on d1"),
        "{text}"
    );
    let (_, verbose) = run(
        &temp.path,
        &["try", &clamp_frame().to_string(), "--verbose"],
    );
    assert!(verbose.contains("  ok   t_in = Ok(5)"), "{verbose}");
    // The revision keeps the exact input, the frame and the candidate's digest.
    let dir = temp.path.join(".sley/drafts/d1/r1");
    let input = fs::read_to_string(dir.join("input.txt")).unwrap();
    assert_eq!(input, clamp_frame().to_string());
    let frame: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("frame.json")).unwrap()).unwrap();
    assert_eq!(frame, clamp_frame());
    let status = status_of(&temp.path, "d1");
    assert_eq!(status["draft"], "d1@r1");
    assert_eq!(status["state"], "valid");
    assert_eq!(status["candidate"], "c1");
    assert_eq!(status["made_by"], "try");
    assert_eq!(status["parent"], Value::Null);
    let stored = fs::read_to_string(temp.path.join(".sley/candidates/c1.hex")).unwrap();
    let bytes = sley_agent::hex::decode(stored.trim()).unwrap();
    assert_eq!(
        status["candidate_sha256"],
        sley_agent::draft::sha256(&bytes)
    );
    let meta: Value = serde_json::from_str(
        &fs::read_to_string(temp.path.join(".sley/candidates/c1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(meta["draft"], "d1@r1");
    // JSON output is a superset of the earlier keys.
    let (_, value) = run_json(&temp.path, &["try", &clamp_frame().to_string()]);
    for key in [
        "handle",
        "ops",
        "verdict",
        "tests",
        "public",
        "notes",
        "also",
        "draft",
        "changed",
        "obligations",
        "provenance",
    ] {
        assert!(value.get(key).is_some(), "{key}: {value}");
    }
    assert_eq!(value["draft"], "d3@r1");
    assert_eq!(
        value["provenance"],
        json!({"provided": 0, "imported": 0, "authored": 2})
    );
    let (status, list) = run(&temp.path, &["draft"]);
    assert_eq!(status, 0);
    assert_eq!(list.lines().count(), 3, "{list}");
    assert!(
        list.starts_with("d1@r1: valid, candidate c1 (Valid)\n"),
        "{list}"
    );
}

#[test]
fn inherited_tests_survive_a_one_operation_correction() {
    let temp = workspace("inherit");
    assert_eq!(run(&temp.path, &["try", &clamp_frame().to_string()]).0, 1);
    // Only the edit: the definitions and both tests of r1 come along.
    let (status, text) = run(&temp.path, &["try", "--on", "d1", FIX]);
    assert_eq!(status, 0, "{text}");
    assert!(text.starts_with("c2: Valid"), "{text}");
    assert!(text.contains(" draft d1@r2\n"), "{text}");
    assert!(text.contains("tests: 2/2 passed"), "{text}");
    assert!(text.contains("next: sley-agent submit d1"), "{text}");
    let status = status_of(&temp.path, "d1");
    assert_eq!(status["made_by"], "try-on");
    assert_eq!(status["parent"], "d1@r1");
    assert_eq!(status["results"]["ran"], 2);
    // A tests-only follow-up adds a test without restating anything.
    let more = json!({"af1": 1, "tests": [{"name": "t_low", "fn": "bound", "args": [-3, 0, 10], "expect": {"Ok": 0}}]});
    let (status, text) = run(&temp.path, &["try", "--on", "d1", &more.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("tests: 3/3 passed"), "{text}");
    // `try --on c2` keeps its meaning and starts a new draft.
    let (status, text) = run(&temp.path, &["try", "--on", "c2", &more.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains(" draft d2@r1\n"), "{text}");
    assert_eq!(status_of(&temp.path, "d2")["on"], "c2");
    // An explicit earlier revision is a base too; the parent is recorded.
    let (status, text) = run(&temp.path, &["try", "--on", "d1@r1", FIX]);
    assert_eq!(status, 0, "{text}");
    let status = status_of(&temp.path, "d1");
    assert_eq!(status["draft"], "d1@r4");
    assert_eq!(status["parent"], "d1@r1");
}

#[test]
fn text_drafts_keep_malformed_input_until_filled_whole() {
    let temp = workspace("text");
    let broken = r#"{"af1": 1, "fns": [{"fn": "f", "params": [["a", "i64"]]"#;
    let (status, text) = run(&temp.path, &["try", broken]);
    assert_eq!(status, 2, "{text}");
    let mut lines = text.lines();
    assert!(
        lines
            .next()
            .unwrap()
            .starts_with("error AGENT_FRAME_INVALID: : not JSON:"),
        "{text}"
    );
    assert_eq!(
        lines.next().unwrap(),
        format!(
            "draft d1@r1: text (not JSON at line 1, column {}, byte {}); the input is kept",
            broken.len(),
            broken.len() - 1
        )
    );
    assert_eq!(candidates(&temp.path), 0, "no candidate handle");
    let status = status_of(&temp.path, "d1");
    assert_eq!(status["state"], "text");
    assert_eq!(status["text"]["line"], 1);
    assert_eq!(status["obligations"][0]["at"], "");
    assert!(!temp.path.join(".sley/drafts/d1/r1/frame.json").exists());
    let (_, input) = run(&temp.path, &["draft", "d1", "--input"]);
    assert_eq!(input.trim_end(), broken);
    let (status, text) = run(&temp.path, &["draft", "d1", "--frame"]);
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_DRAFT_INCOMPLETE"), "{text}");
    // Nothing is layered on a text revision, and nothing but "" repairs it.
    let (status, text) = run(&temp.path, &["try", "--on", "d1", FIX]);
    assert_eq!(status, 2);
    assert!(
        text.contains("error AGENT_DRAFT_INCOMPLETE: d1@r1 is a text draft"),
        "{text}"
    );
    assert!(text.contains("\"at\": \"\""), "{text}");
    let partial = json!({"set": [{"at": "/fns", "value": []}]});
    let (status, text) = run(
        &temp.path,
        &["fill", "d1", &partial.to_string(), "--revision", "1"],
    );
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_DELTA_INVALID"), "{text}");
    assert_eq!(revisions(&temp.path, "d1"), ["r1"]);
    // The whole-frame replacement is recorded as such.
    let whole = json!({"set": [{"at": "", "value": identity_frame()}]});
    let (status, text) = run(
        &temp.path,
        &["fill", "d1", &whole.to_string(), "--revision", "1"],
    );
    assert_eq!(status, 0, "{text}");
    assert!(text.starts_with("c1: Valid"), "{text}");
    let status = status_of(&temp.path, "d1");
    assert_eq!(status["revision"], 2);
    assert_eq!(status["made_by"], "fill");
    assert_eq!(status["whole_frame"], true);
    assert_eq!(status["delta"]["targets"], json!([""]));
    let events = ledger(&temp.path);
    let last = events
        .iter()
        .rev()
        .find(|event| event["cmd"] == "fill")
        .unwrap();
    assert_eq!(last["whole_frame"], true);
    assert_eq!(last["delta_targets"], 1);
    assert_eq!(last["delta_bytes"], whole.to_string().len());
}

#[test]
fn fill_refuses_stale_missing_equal_and_overlapping_targets() {
    let temp = workspace("fill");
    assert_eq!(run(&temp.path, &["try", &clamp_frame().to_string()]).0, 1);
    assert_eq!(run(&temp.path, &["try", "--on", "d1", FIX]).0, 0);
    let delta = |value: Value| value.to_string();
    let set_expect = delta(json!({"set": [{"at": "/tests/1/expect", "value": {"Ok": 0}}]}));
    let before = revisions(&temp.path, "d1");
    let handles = candidates(&temp.path);
    // Stale: r1 is no longer the latest revision.
    let (status, text) = run(&temp.path, &["fill", "d1", &set_expect, "--revision", "1"]);
    assert_eq!(status, 2);
    assert!(
        text.contains("error AGENT_DRAFT_STALE: d1 is at r2, not r1"),
        "{text}"
    );
    for (bad, needle) in [
        (
            json!({"set": [{"at": "/tests/7/expect", "value": 1}]}),
            "does not exist in d1@r2",
        ),
        (
            json!({"set": [{"at": "/tests/0/expect", "value": 1}, {"at": "/tests/0/expect", "value": 2}]}),
            "is replaced twice",
        ),
        (
            json!({"set": [{"at": "/tests/0", "value": 1}, {"at": "/tests/0/expect", "value": 2}]}),
            "overlap",
        ),
        (
            json!({"set": [{"at": "", "value": {}}, {"at": "/af1", "value": 1}]}),
            "overlap",
        ),
        (
            json!({"set": [{"at": "/af1", "value": 1, "note": "x"}]}),
            "unknown key `note`",
        ),
        (
            json!({"set": [{"at": "/af1", "value": 1}], "why": 1}),
            "unknown delta key `why`",
        ),
        (
            json!({"set": [{"at": "af1", "value": 1}]}),
            "is not a JSON pointer",
        ),
        (json!({"set": []}), "sets nothing"),
    ] {
        let (status, text) = run(
            &temp.path,
            &["fill", "d1", &delta(bad.clone()), "--revision", "2"],
        );
        assert_eq!(status, 2, "{bad}: {text}");
        assert!(text.contains("error AGENT_DELTA_INVALID"), "{bad}: {text}");
        assert!(text.contains(needle), "{bad}: {text}");
    }
    let (status, text) = run(&temp.path, &["fill", "d1", "{not json", "--revision", "2"]);
    assert_eq!(status, 2);
    assert!(
        text.contains("AGENT_DELTA_INVALID: the delta is not JSON"),
        "{text}"
    );
    // A refused fill writes nothing: no revision, no candidate.
    assert_eq!(revisions(&temp.path, "d1"), before);
    assert_eq!(candidates(&temp.path), handles);
    // Targets resolve against the pre-edit revision and apply together.
    let pair = json!({"set": [
        {"at": "/tests/0/expect", "value": {"Ok": 6}},
        {"at": "/tests/0/args", "value": [6, 0, 10]}]});
    let (status, text) = run(
        &temp.path,
        &["fill", "d1", &pair.to_string(), "--revision", "2"],
    );
    assert_eq!(status, 0, "{text}");
    assert!(text.contains(" draft d1@r3\n"), "{text}");
    let (_, frame) = run_json(&temp.path, &["draft", "d1", "--frame"]);
    assert_eq!(frame["frame"]["tests"][0]["args"], json!([6, 0, 10]));
    let status = status_of(&temp.path, "d1");
    assert_eq!(status["whole_frame"], false);
    assert_eq!(status["delta"]["targets"].as_array().unwrap().len(), 2);
}

#[test]
fn a_changed_head_needs_an_explicit_rebase() {
    let temp = workspace("head");
    assert_eq!(
        run(&temp.path, &["try", &identity_frame().to_string()]).0,
        0
    );
    let other = json!({"af1": 1, "fns": [{"fn": "g", "params": [], "returns": "i64",
        "blocks": [{"name": "entry", "ops": [["x", "const", 1]], "term": ["return", "x"]}]}]});
    assert_eq!(run(&temp.path, &["try", &other.to_string()]).0, 0);
    let before = status_of(&temp.path, "d1")["base_head"].clone();
    let (status, text) = run(&temp.path, &["commit", "c2"]);
    assert_eq!(status, 0, "{text}");
    let delta = json!({"set": [{"at": "/fns/0/blocks/0/term", "value": ["return", "a"]},
                              {"at": "/fns/0/params", "value": [["a", "i64"], ["b", "i64"]]}]});
    let (status, text) = run(
        &temp.path,
        &["fill", "d1", &delta.to_string(), "--revision", "1"],
    );
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("error AGENT_DRAFT_HEAD_CHANGED: d1@r1 was made on head"),
        "{text}"
    );
    assert!(text.contains("--rebase"), "{text}");
    let (status, text) = run(&temp.path, &["try", "--on", "d1", FIX]);
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_DRAFT_HEAD_CHANGED"), "{text}");
    assert_eq!(revisions(&temp.path, "d1"), ["r1"]);
    let (status, text) = run(
        &temp.path,
        &[
            "fill",
            "d1",
            &delta.to_string(),
            "--revision",
            "1",
            "--rebase",
        ],
    );
    assert_eq!(status, 0, "{text}");
    let status = status_of(&temp.path, "d1");
    assert_eq!(status["made_by"], "rebase");
    assert_eq!(status["rebase"]["from_head"], before);
    assert_eq!(status["rebase"]["via"], "fill");
    assert_eq!(status["rebase"]["to_head"], status["base_head"]);
    assert_ne!(status["base_head"], before);
    // The rebased revision is current: an ordinary fill follows.
    let expect = json!({"set": [{"at": "/fns/0/blocks/0/term", "value": ["return", "b"]}]});
    let (status, text) = run(
        &temp.path,
        &["fill", "d1", &expect.to_string(), "--revision", "2"],
    );
    assert_eq!(status, 0, "{text}");
    assert_eq!(status_of(&temp.path, "d1")["made_by"], "fill");
}

#[test]
fn submission_never_falls_back_to_an_older_revision() {
    let temp = workspace("submit");
    assert_eq!(run(&temp.path, &["try", &clamp_frame().to_string()]).0, 1);
    assert_eq!(run(&temp.path, &["try", "--on", "d1", FIX]).0, 0);
    // A newer, broken edit: r3 is incomplete, r2 stays valid.
    let broken = r#"{"af1": 1, "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "nowhere"]}]}"#;
    let (status, text) = run(&temp.path, &["try", "--on", "d1", broken]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.starts_with("error AGENT_FRAME_INVALID: /fns/0/blocks/5/ops/0: "),
        "{text}"
    );
    assert!(
        text.contains("  pointers refer to .sley/drafts/d1/r3/frame.json (sley-agent draft d1 --frame prints it)\n"),
        "{text}"
    );
    assert!(
        text.contains("draft d1@r3: incomplete, 1 obligation(s) (AGENT_FRAME_INVALID 1); list: sley-agent draft d1 --obligations\n"),
        "{text}"
    );
    assert!(
        text.contains("next: repair in place: sley-agent fill d1 <delta.json> --revision 3 with {\"set\": [{\"at\": \"/fns/0/blocks/5/ops/0\", \"value\": ...}]}"),
        "{text}"
    );
    let (status, text) = run(&temp.path, &["submit", "d1"]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("error AGENT_DRAFT_INCOMPLETE: d1@r3 is incomplete, not valid"),
        "{text}"
    );
    assert!(!temp.path.join("final_candidate.hex").exists());
    // The earlier Valid revision is submitted only when named.
    let (status, text) = run(&temp.path, &["submit", "d1@r2"]);
    assert_eq!(status, 0, "{text}");
    assert!(
        text.starts_with("submitted c2 from d1@r2 -> final_candidate.hex"),
        "{text}"
    );
    // A refused revision is not submitted either.
    let refused = json!({"af1": 1, "fns": [{"fn": "bad", "params": [["a", "i64"]], "returns": "bool", "blocks": [
        {"name": "entry", "ops": [["c", "lt", "a", "a"]], "term": ["cond", "c", "l", "r"]},
        {"name": "l", "ops": [["m", "lt", "a", "a"]], "term": ["br", "j"]},
        {"name": "r", "ops": [["c2", "lt", "a", "a"]], "term": ["cond", "c2", "j", "w"]},
        {"name": "w", "ops": [], "term": ["return", "a"]},
        {"name": "j", "ops": [], "term": ["return", "l.m"]}]}]});
    let (status, text) = run(&temp.path, &["try", &refused.to_string()]);
    assert_eq!(status, 1, "{text}");
    assert!(text.contains(" draft d2@r1\n"), "{text}");
    let status = status_of(&temp.path, "d2");
    assert_eq!(status["state"], "refused");
    assert_eq!(
        status["obligations"][0]["kernel"]["symbol"],
        "CFG_RETURN_TYPE"
    );
    assert_eq!(status["obligations"][0]["kernel"]["phase"], 7);
    let (status, text) = run(&temp.path, &["submit", "d2"]);
    assert_eq!(status, 2);
    assert!(text.contains("d2@r1 is refused, not valid"), "{text}");
    // The untested-function refusal still applies to a draft's candidate.
    assert_eq!(
        run(&temp.path, &["try", &identity_frame().to_string()]).0,
        0
    );
    let (status, text) = run(&temp.path, &["submit", "d3"]);
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_SUBMISSION_REFUSED"), "{text}");
    let (status, _) = run(&temp.path, &["submit", "d3", "--untested"]);
    assert_eq!(status, 0);
}

#[test]
fn candidate_handles_stay_monotonic_and_bound_to_their_revision() {
    let temp = workspace("handles");
    assert_eq!(run(&temp.path, &["try", &clamp_frame().to_string()]).0, 1);
    // Pre-kernel refusals allocate no candidate handle.
    let broken = r#"{"af1": 1, "fns": [{"fn": "f", "params": [], "returns": "i64", "blocks": [{"name": "entry", "ops": [["y", "frobnicate"]], "term": ["return", "y"]}]}]}"#;
    assert_eq!(run(&temp.path, &["try", broken]).0, 2);
    assert_eq!(run(&temp.path, &["try", "{\"af1\": 1,"]).0, 2);
    assert_eq!(candidates(&temp.path), 1);
    let (status, text) = run(&temp.path, &["try", "--on", "d1", FIX]);
    assert_eq!(status, 0, "{text}");
    assert!(text.starts_with("c2: "), "{text}");
    let status = status_of(&temp.path, "d1");
    assert_eq!(status["candidate"], "c2");
    assert_eq!(status_of(&temp.path, "d2")["candidate"], Value::Null);
    assert_eq!(status_of(&temp.path, "d3")["state"], "text");
    // The draft binds the exact bytes: altered bytes are not submitted.
    let path = temp.path.join(".sley/candidates/c2.hex");
    let c1 = fs::read_to_string(temp.path.join(".sley/candidates/c1.hex")).unwrap();
    fs::write(&path, c1).unwrap();
    let (status, text) = run(&temp.path, &["submit", "d1"]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains(
            "AGENT_DRAFT_INCOMPLETE: the stored bytes of c2 are not the ones d1@r2 recorded"
        ),
        "{text}"
    );
}

#[test]
fn named_entities_collide_across_revisions_only_by_kind() {
    let temp = workspace("collide");
    assert_eq!(run(&temp.path, &["try", &clamp_frame().to_string()]).0, 1);
    // The same name and kind replaces its definition: one `bound` remains.
    let again = json!({"af1": 1, "fns": [clamp_frame()["fns"][0].clone()]});
    let mut again = again;
    again["fns"][0]["blocks"][5]["ops"] = json!([["r", "ok", "high"]]);
    let (status, text) = run(&temp.path, &["try", "--on", "d1", &again.to_string()]);
    assert_eq!(status, 0, "{text}");
    let (_, frame) = run_json(&temp.path, &["draft", "d1", "--frame"]);
    assert_eq!(frame["frame"]["fns"].as_array().unwrap().len(), 1);
    // A type named like the function is a collision, found at its pointer.
    let clash = json!({"af1": 1, "types": [{"name": "bound", "variant": ["A"]}]});
    let (status, text) = run(&temp.path, &["try", "--on", "d1", &clash.to_string()]);
    assert_eq!(status, 2, "{text}");
    assert!(text.contains("`bound` is declared twice"), "{text}");
    let status = status_of(&temp.path, "d1");
    assert_eq!(status["state"], "incomplete");
    let obligation = &status["obligations"][0];
    assert_eq!(obligation["symbol"], "AGENT_FRAME_INVALID");
    assert!(
        obligation["at"].as_str().unwrap().starts_with('/'),
        "{obligation}"
    );
    // Repair in place: drop the colliding type with one replacement.
    let repair = json!({"set": [{"at": "/types", "value": [{"name": "RangeError", "variant": ["Inverted"]}]}]});
    let (status, text) = run(
        &temp.path,
        &["fill", "d1", &repair.to_string(), "--revision", "3"],
    );
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("tests: 2/2 passed"), "{text}");
}

#[test]
fn imported_cases_keep_their_source_and_never_count_as_authored() {
    let temp = workspace("import");
    // One TestCase at the head: `provided`.
    assert_eq!(
        run(&temp.path, &["try", &identity_frame().to_string()]).0,
        0
    );
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    let provided =
        json!({"af1": 1, "tests": [{"name": "t_head", "fn": "f", "args": [3], "expect": 3}]});
    assert_eq!(run(&temp.path, &["try", &provided.to_string()]).0, 0);
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    let cases = temp.path.join("public.json");
    let text = json!([
        {"name": "p_one", "function": "f", "args": [1], "expect": 1},
        {"name": "p_two", "function": "f", "args": [2], "expect": 2},
        {"name": "p_bad", "function": "f", "args": [2], "expect": 5}])
    .to_string();
    fs::write(&cases, &text).unwrap();
    let digest = sley_agent::draft::sha256(text.as_bytes());
    // A change plus one authored test, then the public cases on top.
    let change = json!({"af1": 1, "patch": [{"fn": "f", "blocks": {"entry": {"ops": [["z", "const", 0], ["s", "add", "a", "z"]],
        "term": ["switch", "s", ["Ok", "done", "$"], ["Err", "done", "a"]]}, "done": {"params": [["v", "i64"]], "ops": [], "term": ["return", "v"]}}}],
        "tests": [{"name": "t_mine", "fn": "f", "args": [4], "expect": 4}]});
    let (status, text) = run(&temp.path, &["try", &change.to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("[authored 1, provided 1]"), "{text}");
    let (status, text) = run(
        &temp.path,
        &[
            "import",
            cases.to_str().unwrap(),
            "--on",
            "d3",
            "--only",
            "p_one,p_two",
        ],
    );
    assert_eq!(status, 0, "{text}");
    assert!(
        text.contains("tests: 4/4 passed [authored 1, imported 2, provided 1]"),
        "{text}"
    );
    let status = status_of(&temp.path, "d3");
    assert_eq!(status["made_by"], "import");
    assert_eq!(
        status["tests"],
        json!({"provided": 1, "imported": 2, "authored": 1})
    );
    assert_eq!(status["import"]["sha256"], digest);
    assert_eq!(status["import"]["cases"], 2);
    let sources = status["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 2);
    assert!(
        sources.iter().all(|source| source["sha256"] == digest),
        "{status}"
    );
    assert_eq!(sources[0]["case"], "p_one");
    // The source stays in draft metadata, not in the frame.
    let (_, frame) = run_json(&temp.path, &["draft", "d3", "--frame"]);
    let tests = frame["frame"]["tests"].as_array().unwrap();
    assert!(
        tests.iter().all(|test| test.get("source").is_none()),
        "{frame}"
    );
    // A failing public expectation is reported, never replaced by the
    // candidate's answer.
    let (status, text) = run(
        &temp.path,
        &["import", cases.to_str().unwrap(), "--on", "d3"],
    );
    assert_eq!(status, 1, "{text}");
    assert!(text.contains("FAIL p_bad (f): expected 5, got 2"), "{text}");
    assert_eq!(status_of(&temp.path, "d3")["tests"]["imported"], 3);
    // Restating an imported test makes it authored.
    let mine = json!({"af1": 1, "tests": [{"name": "p_bad", "fn": "f", "args": [2], "expect": 2}]});
    let (status, text) = run(&temp.path, &["try", "--on", "d3", &mine.to_string()]);
    assert_eq!(status, 0, "{text}");
    let status = status_of(&temp.path, "d3");
    assert_eq!(
        status["tests"],
        json!({"provided": 1, "imported": 2, "authored": 2})
    );
    assert_eq!(status["sources"].as_array().unwrap().len(), 2);
}

#[test]
fn import_without_a_draft_starts_one_and_refuses_unsourced_cases() {
    let temp = workspace("import-new");
    assert_eq!(
        run(&temp.path, &["try", &identity_frame().to_string()]).0,
        0
    );
    assert_eq!(run(&temp.path, &["commit"]).0, 0);
    let cases = temp.path.join("public.json");
    fs::write(
        &cases,
        json!([{"name": "p_one", "function": "f", "args": [1], "expect": 1}]).to_string(),
    )
    .unwrap();
    let (status, text) = run(&temp.path, &["import", cases.to_str().unwrap()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains(" draft d2@r1\n"), "{text}");
    assert!(text.contains("tests: 1/1 passed [imported 1]"), "{text}");
    let status = status_of(&temp.path, "d2");
    assert_eq!(status["made_by"], "import");
    assert_eq!(status["parent"], Value::Null);
    let (status, text) = run(
        &temp.path,
        &["import", cases.to_str().unwrap(), "--only", "nope"],
    );
    assert_eq!(status, 2);
    assert!(
        text.contains("AGENT_INPUT_INVALID: no case named `nope`"),
        "{text}"
    );
    let unexpected = temp.path.join("unexpected.json");
    fs::write(
        &unexpected,
        r#"[{"name": "q", "function": "f", "args": [1]}]"#,
    )
    .unwrap();
    let (status, text) = run(&temp.path, &["import", unexpected.to_str().unwrap()]);
    assert_eq!(status, 2);
    assert!(text.contains("has no \"expect\""), "{text}");
}

#[test]
fn obligations_group_identical_decisions_and_bound_the_listing() {
    let temp = workspace("obligations");
    // Ten functions, each using its own unknown name, and three uses of one
    // unknown opcode: ten distinct decisions and one decision three times.
    let mut fns: Vec<Value> = (0..10)
        .map(|index| {
            json!({"fn": format!("f{index}"), "params": [["a", "i64"]], "returns": "i64",
                "blocks": [{"name": "entry", "ops": [], "term": ["return", format!("missing{index}")]}]})
        })
        .collect();
    for index in 0..3 {
        fns.push(json!({"fn": format!("g{index}"), "params": [["a", "i64"]], "returns": "i64",
            "blocks": [{"name": "entry", "ops": [["y", "frobnicate", "a"]], "term": ["return", "a"]}]}));
    }
    let frame = json!({"af1": 1, "fns": fns});
    let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("draft d1@r1: incomplete, 13 obligation(s) (AGENT_FRAME_INVALID 13)"),
        "{text}"
    );
    let status = status_of(&temp.path, "d1");
    let obligations = status["obligations"].as_array().unwrap();
    assert_eq!(obligations.len(), 11, "{status}");
    let grouped = obligations
        .iter()
        .find(|record| record["count"] == 3)
        .unwrap();
    assert_eq!(grouped["also_at"].as_array().unwrap().len(), 2);
    for key in [
        "id",
        "symbol",
        "at",
        "expected",
        "available",
        "decision",
        "kernel",
        "count",
    ] {
        assert!(grouped.get(key).is_some(), "{key}: {grouped}");
    }
    let (_, text) = run(&temp.path, &["draft", "d1"]);
    assert!(text.contains("obligations: 13\n"), "{text}");
    let listed = text.lines().filter(|line| line.starts_with("  o")).count();
    assert_eq!(listed, 8, "{text}");
    let hidden: u64 = obligations[8..]
        .iter()
        .map(|record| record["count"].as_u64().unwrap())
        .sum();
    assert!(
        text.contains(&format!(
            "{hidden} more: sley-agent draft d1@r1 --obligations\n"
        )),
        "{text}"
    );
    // JSON keeps the refusal keys and adds the draft, state and obligations.
    let (status, value) = run_json(&temp.path, &["try", &frame.to_string()]);
    assert_eq!(status, 2);
    assert_eq!(value["error"], "AGENT_FRAME_INVALID");
    assert!(
        value["detail"]
            .as_str()
            .unwrap()
            .contains("(1 of 13 problems)"),
        "{value}"
    );
    assert_eq!(value["draft"], "d2@r1");
    assert_eq!(value["state"], "incomplete");
    assert_eq!(value["obligations"].as_array().unwrap().len(), 11);
    let (_, all) = run(&temp.path, &["draft", "d1", "--obligations"]);
    assert_eq!(
        all.lines().filter(|line| line.starts_with("  o")).count(),
        11,
        "{all}"
    );
    assert!(
        all.contains(
            "(3x) at /fns/10/blocks/0/ops/0, /fns/11/blocks/0/ops/0, /fns/12/blocks/0/ops/0"
        ),
        "{all}"
    );
}

fn ledger(dir: &Path) -> Vec<Value> {
    fs::read_to_string(dir.join(".sley/events.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn the_events_ledger_counts_without_content() {
    let temp = workspace("events");
    let frame = clamp_frame().to_string();
    let (_, try_output) = run(&temp.path, &["try", &frame]);
    run(&temp.path, &["try", "--on", "d1", FIX]);
    let broken = r#"{"af1": 1, "edit": [{"fn": "bound", "replace_op": "above.r", "with": ["ok", "secret_name"]}]}"#;
    run(&temp.path, &["try", "--on", "d1", broken]);
    run(&temp.path, &["draft", "d1"]);
    run(&temp.path, &["submit", "d1@r2"]);
    run(&temp.path, &["help"]);
    run(&temp.path, &["version"]);
    let events = ledger(&temp.path);
    // One line per workspace command; help and version touch no workspace.
    assert_eq!(events.len(), 5, "{events:?}");
    let keys = [
        "seq",
        "cmd",
        "draft",
        "candidate",
        "input_bytes",
        "output_bytes",
        "whole_frame",
        "delta_targets",
        "delta_bytes",
        "afx",
        "table_rows",
        "tests",
        "refusal",
        "obligations",
        "valid",
    ];
    for (index, event) in events.iter().enumerate() {
        let object = event.as_object().unwrap();
        assert_eq!(object.len(), keys.len(), "{event}");
        for key in keys {
            assert!(object.contains_key(key), "{key}: {event}");
        }
        assert_eq!(event["seq"], index + 1);
    }
    let first = &events[0];
    assert_eq!(first["cmd"], "try");
    assert_eq!(first["draft"], "d1@r1");
    assert_eq!(first["candidate"], "c1");
    assert_eq!(first["input_bytes"], frame.len());
    assert_eq!(first["output_bytes"], try_output.len());
    assert_eq!(first["valid"], true);
    assert_eq!(
        first["tests"],
        json!({"provided": 0, "imported": 0, "authored": 2})
    );
    let refused = &events[2];
    assert_eq!(refused["refusal"], "AGENT_FRAME_INVALID");
    assert_eq!(refused["obligations"], 1);
    assert_eq!(refused["candidate"], Value::Null);
    assert_eq!(refused["valid"], Value::Null);
    assert_eq!(events[4]["cmd"], "submit");
    assert_eq!(events[4]["draft"], "d1@r2");
    assert_eq!(events[4]["candidate"], "c2");
    // No clock and no content: names from the frames never appear.
    let text = fs::read_to_string(temp.path.join(".sley/events.jsonl")).unwrap();
    for word in [
        "bound",
        "secret_name",
        "RangeError",
        "above",
        "time",
        "clock",
    ] {
        assert!(!text.contains(word), "{word}: {text}");
    }
    // Commands outside a workspace write no ledger there.
    let elsewhere = TempDir::new("no-ledger");
    let (status, _) = run(&elsewhere.path, &["view"]);
    assert_eq!(status, 2);
    assert!(!elsewhere.path.join(".sley").exists());
}
