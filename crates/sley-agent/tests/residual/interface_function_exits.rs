use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn function(returns: &str, term: &Value, dialect: bool) -> Value {
    let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":returns,"blocks":[{"name":"entry","ops":[],"term":term}]}]});
    if dialect {
        source["afx"] = json!(1);
    }
    source
}

#[test]
fn source_trap_payloads_follow_canonical_persistability() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        for ty in ["Cell<i8>", "Tuple<bool,Cell<i8>>"] {
            let mut source = function("i8", &json!(["trap", "unreachable", "payload"]), dialect);
            source["fns"][0]["params"] = json!([["payload", ty]]);
            let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
            assert!(
                error.detail().contains("/term/2")
                    && error.detail().contains("closed and persistable"),
                "{error}"
            );
        }
        // Canonical closed function references are persistable; do not invent a ban.
        let mut reference = function("i8", &json!(["trap", "unreachable", "payload"]), dialect);
        reference["fns"][0]["params"] = json!([["payload", "fn(i8)->i8"]]);
        check(&fixture, &predicate_request(json!(false)), &reference).unwrap();
        let source = function("i8", &json!(["trap", "unreachable", "x"]), dialect);
        check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(
            &fixture.dir,
            &[
                "call",
                "helper",
                "7",
                "false",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!({"trap":1,"payload":7}));
    }
}

#[test]
fn retained_return_types_and_parameter_ids_follow_the_patch_overlay() {
    let fixture = Fixture::new();
    let source = function("i8", &json!(["return", "x"]), false);
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    for dialect in [false, true] {
        let mut patch = json!({"af1":1,"patch":[{"fn":"helper","returns":"bool","blocks":{}}]});
        if dialect {
            patch["afx"] = json!(1);
        }
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("retained block `helper.entry`")
                && error.detail().contains("/return")
                && error.detail().contains("requires bool"),
            "{error}"
        );
        patch["patch"][0]["params"] = json!([["x", "bool"], ["ready", "bool"]]);
        check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(
            &fixture.dir,
            &[
                "call",
                "helper",
                "true",
                "false",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!(true));
        patch["patch"][0]["params"] = json!([["renamed", "bool"], ["ready", "bool"]]);
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("helper.x") && error.detail().contains("original identity"),
            "{error}"
        );
    }
}

#[test]
fn retained_traps_use_updated_named_definition_traits() {
    let fixture = Fixture::new();
    let mut source = function("i8", &json!(["trap", "unreachable", "payload"]), false);
    source["types"] = json!([{"name":"ExitPack","record":[["value","i8"]]}]);
    source["fns"][0]["params"] = json!([["payload", "ExitPack"]]);
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    for dialect in [false, true] {
        let mut patch = json!({"af1":1,"types":[{"name":"ExitPack","record":[["value","Cell<i8>"]]}],"patch":[{"fn":"helper","blocks":{}}]});
        if dialect {
            patch["afx"] = json!(1);
        }
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("retained block `helper.entry`")
                && error.detail().contains("/payload")
                && error.detail().contains("closed and persistable"),
            "{error}"
        );
        patch["types"][0]["record"][0][1] = json!("bool");
        check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
    }
}

#[test]
fn afx_ok_and_fail_keep_the_existing_result_and_option_routes() {
    let fixture = Fixture::new();
    for (returns, term, expected) in [
        ("Result<i8,ExitError>", json!(["ok", "x"]), json!({"Ok":7})),
        (
            "Result<i8,ExitError>",
            json!(["fail", "Stop"]),
            json!({"Err":"Stop"}),
        ),
        (
            "Result<i8,ExitError>",
            json!(["fail", "Value", "x"]),
            json!({"Err":{"Value":7}}),
        ),
        ("Option<i8>", json!(["fail"]), json!("None")),
    ] {
        let mut source = function(returns, &term, true);
        source["types"] = json!([{"name":"ExitError","variant":[["Stop",null],["Value","i8"]]}]);
        check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(
            &fixture.dir,
            &[
                "call",
                "helper",
                "7",
                "false",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
    for (returns, term, reason) in [
        (
            "Option<i8>",
            json!(["ok", "x"]),
            "ok terminator requires Result",
        ),
        (
            "Result<i8,ExitError>",
            json!(["ok", "ready"]),
            "requires i8",
        ),
        (
            "Result<i8,ExitError>",
            json!(["fail"]),
            "failure route is incompatible",
        ),
        (
            "Result<i8,ExitError>",
            json!(["fail", "Absent"]),
            "failure route is incompatible",
        ),
        (
            "Result<i8,ExitError>",
            json!(["fail", "Stop", "x"]),
            "payload presence",
        ),
        (
            "Result<i8,ExitError>",
            json!(["fail", "Value"]),
            "payload presence",
        ),
        (
            "Result<i8,ExitError>",
            json!(["fail", "Value", "ready"]),
            "requires i8",
        ),
    ] {
        let mut source = function(returns, &term, true);
        source["types"] = json!([{"name":"ExitError","variant":[["Stop",null],["Value","i8"]]}]);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/term") && error.detail().contains(reason),
            "{error}"
        );
    }
}

#[test]
fn unknown_exit_types_plain_sugar_and_lenient_traps_stay_deferred() {
    let fixture = Fixture::new();
    for (term, dialect, locator) in [
        (json!(["return", "unknown"]), true, "/term/1"),
        (json!(["trap", "unreachable", "unknown"]), true, "/term/2"),
        (json!(["trap", 7]), false, "/term"),
        (json!(["ok", "x"]), false, "/term"),
        (json!(["fail"]), false, "/term"),
    ] {
        let source = function("i8", &term, dialect);
        let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        assert!(
            report["declared_body_control_flow"]["deferred"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p.as_str().unwrap().ends_with(locator)),
            "{report}"
        );
        assert_eq!(report["composition"], "partial");
    }
}

#[test]
fn nested_return_expressions_are_checked_without_lowering() {
    let fixture = Fixture::new();
    for (index, valid) in [(0, true), (1, false)] {
        let source = function(
            "i8",
            &json!(["return", ["tuple_get", index, ["tuple", "x", "ready"]]]),
            true,
        );
        let result = check(&fixture, &predicate_request(json!(false)), &source);
        if valid {
            result.unwrap();
            let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
            assert_eq!(code, 0, "{result}");
        } else {
            let error = result.unwrap_err();
            assert!(
                error.detail().contains("/term/1") && error.detail().contains("requires i8"),
                "{error}"
            );
        }
    }
}

#[test]
fn source_function_return_values_match_the_declared_result_before_generation() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let source = function("i8", &json!(["return", "ready"]), dialect);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/fns/0/blocks/0/term/1")
                && error.detail().contains("return")
                && error.detail().contains("bool")
                && error.detail().contains("requires i8"),
            "{error}"
        );
    }
}

#[test]
fn incompatible_exit_drafts_refuse_without_publication_or_source_changes() {
    for dialect in [false, true] {
        for term in [
            json!(["return", "ready"]),
            json!(["trap", "unreachable", ["cell", "x"]]),
        ] {
            let fixture = Fixture::new();
            let source = function("i8", &term, dialect);
            let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
            assert!([1, 2].contains(&code), "{result}");
            let draft = result["draft"].as_str().unwrap();
            let before = inventory(&fixture.dir);
            let head = fixture.workspace.read_head().unwrap().transaction_id();
            let mut request = predicate_request(json!(false));
            request["base"] = json!(draft);
            let (code, result) = cli(
                &fixture.dir,
                &["residual", "try", &request.to_string(), "--no-test"],
            );
            assert_eq!(code, 2, "{result}");
            assert_eq!(
                result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
                "{result}"
            );
            assert!(
                result["detail"]
                    .as_str()
                    .unwrap()
                    .contains("/fns/0/blocks/0/term"),
                "{result}"
            );
            let after = inventory(&fixture.dir);
            let changed: std::collections::BTreeSet<_> = before
                .keys()
                .chain(after.keys())
                .filter(|path| before.get(*path) != after.get(*path))
                .collect();
            assert!(
                changed
                    .iter()
                    .all(|path| path.as_path() == std::path::Path::new(".sley/events.jsonl")),
                "{changed:?}"
            );
            assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
            assert_eq!(
                fixture.workspace.read_head().unwrap().transaction_id(),
                head
            );
        }
    }
}

fn inventory(root: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        at: &std::path::Path,
        files: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = std::collections::BTreeMap::new();
    walk(root, root, &mut files);
    files
}
