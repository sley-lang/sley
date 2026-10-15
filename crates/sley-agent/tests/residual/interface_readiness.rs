use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli};
use serde_json::{Value, json};

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    let mut source = json!({"af1":1,"afx":1,"types":[
        {"name":"ChoiceA","variant":[["Item","i8"]]},
        {"name":"ChoiceB","variant":[["Item","i8"]]}],"fns":[]});
    for (name, ty) in [
        ("fetchVec", "Vec<i8>"),
        ("fetchMap", "Result<Map<i8,bool>,DuplicateKeyError>"),
        ("fetchChoice", "ChoiceA"),
    ] {
        source["fns"].as_array_mut().unwrap().push(json!({"fn":name,
            "params":[["item",ty]],"returns":ty,"blocks":[
                {"name":"entry","ops":[],"term":["return","item"]}]}));
    }
    let (code, report) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{report}");
    let (code, report) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{report}");
    fixture
}

fn request(returns: &str, ops: Value, term: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!(returns);
    request["bindings"]["success"]["ops"] = ops;
    request["bindings"]["success"]["term"] = term;
    request
}

#[test]
fn container_and_short_variant_hints_refuse_actual_conflicting_connections_before_expansion() {
    for (returns, operation, callee, actual, wanted) in [
        (
            "Vec<i64>",
            json!(["value", "vec"]),
            "fetchVec",
            "Vec<i64>",
            "Vec<i8>",
        ),
        (
            "Result<Map<i64,bool>,DuplicateKeyError>",
            json!(["value", "map"]),
            "fetchMap",
            "Map<i64,bool>",
            "Map<i8,bool>",
        ),
        (
            "ChoiceB",
            json!(["value", "variant", "Item", "x"]),
            "fetchChoice",
            "ChoiceB",
            "ChoiceA",
        ),
    ] {
        let fixture = fixture();
        let request = request(
            returns,
            json!([operation, ["used", "call", callee, "value"]]),
            json!(["return", "value"]),
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(
            result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
            "{result}"
        );
        let detail = result["detail"].as_str().unwrap();
        assert!(
            detail.contains("/bindings/success/ops/1/3")
                && detail.contains(actual)
                && detail.contains(wanted),
            "{result}"
        );
        assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    }
}

#[test]
fn emitted_container_and_short_variant_contexts_are_known_and_keep_native_results() {
    for (returns, operation, callee, expected) in [
        ("Vec<i8>", json!(["value", "vec"]), "fetchVec", json!([])),
        (
            "Result<Map<i8,bool>,DuplicateKeyError>",
            json!(["value", "map"]),
            "fetchMap",
            json!({"Ok":[]}),
        ),
        (
            "ChoiceA",
            json!(["value", "variant", "Item", "x"]),
            "fetchChoice",
            json!({"Item":3}),
        ),
    ] {
        for returned in ["value", "used"] {
            let fixture = fixture();
            let request = request(
                returns,
                json!([operation, ["used", "call", callee, "value"]]),
                json!(["return", returned]),
            );
            let report = check(&fixture, &request, &json!({})).unwrap();
            for at in ["/bindings/success/ops/0", "/bindings/success/ops/1"] {
                let row = report["connections"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["at"] == at)
                    .unwrap();
                assert_eq!(row["expression_types"], "connections_checked", "{report}");
            }
            let (code, result) = cli(
                &fixture.dir,
                &["residual", "try", &request.to_string(), "--no-test"],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["kernel"], "valid");
            let (code, called) = cli(
                &fixture.dir,
                &[
                    "call",
                    "checked",
                    "3",
                    "false",
                    "--on",
                    result["handle"].as_str().unwrap(),
                ],
            );
            assert_eq!(code, 0, "{called}");
            assert_eq!(called["result"], expected);
        }
    }
}

#[test]
fn unresolved_expressions_keep_library_analysis_but_cli_refuses_before_drafts() {
    let fixture = fixture();
    let request = request(
        "i8",
        json!([["value", "future-op", "x"]]),
        json!(["return", "x"]),
    );
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(report["composition"], "partial");
    assert!(
        report["connections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["expression_types"] == "partial")
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    assert!(
        result["detail"]
            .as_str()
            .unwrap()
            .contains("/bindings/success/ops/0"),
        "{result}"
    );
    assert!(
        result["detail"]
            .as_str()
            .unwrap()
            .contains("ordinary AF1-X"),
        "{result}"
    );
    assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    // The author can supply the intended operation through the unchanged
    // ordinary path. The readiness refusal does not create an automatic fix.
    let ordinary = json!({"af1":1,"afx":1,"fns":[{"fn":"checked",
        "params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":[
            {"name":"entry","ops":[["value","const",{"type":"i8","value":7}]],
             "term":["return","value"]}]}]});
    let (code, tried) = cli(&fixture.dir, &["try", &ordinary.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{tried}");
    assert_eq!(tried["verdict"]["valid"], true);
    let (code, called) = cli(
        &fixture.dir,
        &[
            "call",
            "checked",
            "3",
            "false",
            "--on",
            tried["handle"].as_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{called}");
    assert_eq!(called["result"], 7);
}

#[test]
fn annotated_unknown_syntax_stays_unresolved_and_known_conflicts_keep_priority() {
    let fixture = fixture();
    for operation in [
        json!(["value", "future-op", "x"]),
        json!({"name":"value","op":"future-op","args":["x"],"type":"i8"}),
    ] {
        let request = request("i8", json!([operation]), json!(["return", "x"]));
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        assert!(
            result["detail"]
                .as_str()
                .unwrap()
                .contains("/bindings/success/ops/0")
        );
        assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    }
    let request = request(
        "i8",
        json!([
            ["value", "future-op", "x"],
            ["used", "call", "fetchVec", "ready"]
        ]),
        json!(["return", "x"]),
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{result}");
    let detail = result["detail"].as_str().unwrap();
    assert!(
        detail.contains("/bindings/success/ops/1/3")
            && detail.contains("Vec<i8>")
            && detail.contains("bool"),
        "{result}"
    );
}
