//! Namespace membership of frame-created entities: the omitted key joins
//! the only namespace, a name joins that namespace, and `null` joins none
//! (`docs/spec/SLEY_AGENT_V1.md` section 5, item 7).

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

/// Tries and commits `input`, asserting both succeed.
fn commit(dir: &Path, input: &str) {
    let (status, text) = run(dir, &["try", "--no-test", input]);
    assert_eq!(status, 0, "{text}");
    let (status, text) = run(dir, &["commit"]);
    assert_eq!(status, 0, "{text}");
}

fn create_namespace(key: &str) -> String {
    json!([{"class": "CreateEntity", "kind": 3, "key": key,
            "payload": {"parent": {"variant": "None"}, "members": []}}])
    .to_string()
}

/// A workspace with one namespace `ns` whose only member is `keep`.
fn one_namespace(label: &str) -> TempDir {
    let temp = TempDir::new(label);
    genesis::init(&temp.path, Some([9; 32]), genesis::INIT_CEILINGS).unwrap();
    commit(&temp.path, &create_namespace("ns"));
    commit(
        &temp.path,
        &json!({"af1": 1, "fns": [{"fn": "keep", "params": [["x", "i64"]], "returns": "i64",
                "blocks": [{"name": "entry", "term": ["return", "x"]}]}]})
        .to_string(),
    );
    assert_eq!(members(&temp.path, None, "ns"), ["keep"]);
    temp
}

/// The members of namespace `ns` at the head, or after a candidate.
fn members(dir: &Path, after: Option<&str>, ns: &str) -> Vec<String> {
    let mut args = vec!["view", ns];
    if let Some(handle) = after {
        args.extend(["--after", handle]);
    }
    let (status, text) = run(dir, &args);
    assert_eq!(status, 0, "{text}");
    let prefix = format!("ns {ns}:");
    let line = text
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no {prefix} in {text}"));
    let mut names: Vec<String> = line[prefix.len()..]
        .split(',')
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    names.sort();
    names
}

/// Tries a frame; returns its handle and `(created, replaced, deleted)`.
fn try_frame(dir: &Path, frame: &Value) -> (String, [u64; 3]) {
    let (status, value) = run_json(dir, &["try", "--no-test", &frame.to_string()]);
    assert_eq!(status, 0, "{value}");
    assert_eq!(value["verdict"]["valid"], true, "{value}");
    let ops = &value["ops"];
    let count = |key: &str| ops[key].as_u64().unwrap();
    (
        value["handle"].as_str().unwrap().to_owned(),
        [count("created"), count("replaced"), count("deleted")],
    )
}

fn limit_const(name: &str) -> Value {
    json!({"name": name, "type": "i64", "value": 3})
}

#[test]
fn an_omitted_namespace_joins_the_only_one() {
    let temp = one_namespace("ns-omitted");
    let (handle, ops) = try_frame(
        &temp.path,
        &json!({"af1": 1, "consts": [limit_const("lim")]}),
    );
    assert_eq!(
        ops,
        [1, 1, 0],
        "the const is created and the namespace replaced"
    );
    assert_eq!(members(&temp.path, Some(&handle), "ns"), ["keep", "lim"]);
}

#[test]
fn a_null_namespace_joins_none_and_keeps_existing_members() {
    let temp = one_namespace("ns-null");
    let (handle, ops) = try_frame(
        &temp.path,
        &json!({"af1": 1, "namespace": null, "consts": [limit_const("lim")],
                "types": [{"name": "State", "variant": ["Idle", ["Busy", "i64"]]}]}),
    );
    assert_eq!(ops, [2, 0, 0], "nothing but the two creates: {ops:?}");
    assert_eq!(members(&temp.path, Some(&handle), "ns"), ["keep"]);
    let (status, text) = run(&temp.path, &["view", "--after", &handle, "lim", "State"]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("const lim: i64 = 3"), "{text}");
}

#[test]
fn a_null_namespace_still_removes_deleted_members() {
    let temp = one_namespace("ns-null-delete");
    let (handle, ops) = try_frame(
        &temp.path,
        &json!({"af1": 1, "namespace": null, "consts": [limit_const("lim")],
                "delete": ["keep"]}),
    );
    // keep, its parameter and its block go; the namespace drops keep.
    assert_eq!(ops, [1, 1, 3], "{ops:?}");
    assert!(members(&temp.path, Some(&handle), "ns").is_empty());
    // The same deletion with the key omitted also adds the new const.
    let (handle, _) = try_frame(
        &temp.path,
        &json!({"af1": 1, "consts": [limit_const("lim")], "delete": ["keep"]}),
    );
    assert_eq!(members(&temp.path, Some(&handle), "ns"), ["lim"]);
}

#[test]
fn a_named_namespace_is_joined_and_omission_joins_none_of_several() {
    let temp = one_namespace("ns-named");
    commit(&temp.path, &create_namespace("extra"));
    assert!(members(&temp.path, None, "extra").is_empty());
    // Two namespaces: an omitted key joins neither (as before).
    let (handle, ops) = try_frame(&temp.path, &json!({"af1": 1, "consts": [limit_const("a")]}));
    assert_eq!(ops, [1, 0, 0]);
    assert_eq!(members(&temp.path, Some(&handle), "ns"), ["keep"]);
    assert!(members(&temp.path, Some(&handle), "extra").is_empty());
    // A name joins exactly that namespace.
    let (handle, ops) = try_frame(
        &temp.path,
        &json!({"af1": 1, "namespace": "extra", "consts": [limit_const("b")]}),
    );
    assert_eq!(ops, [1, 1, 0]);
    assert_eq!(members(&temp.path, Some(&handle), "extra"), ["b"]);
    assert_eq!(members(&temp.path, Some(&handle), "ns"), ["keep"]);
    // Null joins none of them.
    let (handle, ops) = try_frame(
        &temp.path,
        &json!({"af1": 1, "namespace": null, "consts": [limit_const("c")]}),
    );
    assert_eq!(ops, [1, 0, 0]);
    assert!(members(&temp.path, Some(&handle), "extra").is_empty());
}

#[test]
fn a_null_namespace_survives_layering_and_other_values_are_refused() {
    let temp = one_namespace("ns-layer");
    let (base, _) = try_frame(
        &temp.path,
        &json!({"af1": 1, "namespace": null, "consts": [limit_const("lim")]}),
    );
    let (status, value) = run_json(
        &temp.path,
        &[
            "try",
            "--no-test",
            "--on",
            &base,
            &json!({"af1": 1, "consts": [limit_const("other")]}).to_string(),
        ],
    );
    assert_eq!(status, 0, "{value}");
    let handle = value["handle"].as_str().unwrap();
    assert_eq!(value["ops"]["replaced"], 0, "{value}");
    assert_eq!(members(&temp.path, Some(handle), "ns"), ["keep"]);
    for bad in [json!(5), json!(["ns"]), json!("nowhere")] {
        let (status, text) = run(
            &temp.path,
            &[
                "try",
                &json!({"af1": 1, "namespace": bad, "consts": [limit_const("x")]}).to_string(),
            ],
        );
        assert_eq!(status, 2, "{text}");
        assert!(
            text.starts_with("error AGENT_FRAME_INVALID: /namespace"),
            "{text}"
        );
    }
}
