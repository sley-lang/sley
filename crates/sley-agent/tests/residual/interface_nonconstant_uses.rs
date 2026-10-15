use super::cli;
use super::interface_tests::predicate_request;
use serde_json::{Value, json};

fn request(returns: &str, ops: Value, term: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!(returns);
    request["bindings"]["success"]["ops"] = ops;
    request["bindings"]["success"]["term"] = term;
    request
}

#[test]
fn nonconstant_none_return_hint_refuses_conflicting_call_before_draft_creation() {
    let fixture = super::interface_call_tests::fixture(true);
    let head = fixture.workspace.head().unwrap().transaction_id();
    let request = request(
        "Option<i64>",
        json!([["n", "none"], ["v", "call", "fetch", "n"]]),
        json!(["return", "n"]),
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
            && detail.contains("Option<i64>")
            && detail.contains("Option<i8>"),
        "{result}"
    );
    assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    assert_eq!(fixture.workspace.head().unwrap().transaction_id(), head);
}

fn executes(request: &Value, expected: &Value) {
    let fixture = super::interface_call_tests::fixture(true);
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for input in ["3", "-7"] {
        let (code, called) = cli(
            &fixture.dir,
            &[
                "call",
                "checked",
                input,
                "false",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{called}");
        assert_eq!(&called["result"], expected);
    }
}

#[test]
fn none_call_return_alias_and_annotation_controls_keep_native_none_results() {
    for (ops, term) in [
        (
            json!([["n", "none"], ["v", "call", "fetch", "n"]]),
            json!(["return", "v"]),
        ),
        (
            json!([["n", "129"], ["v", "call", "fetch", "n#0"]]),
            json!(["return", "n#0"]),
        ),
        (
            json!([{"name":"n","op":"none","type":"Option<i8>"},["v","call","fetch","n"]]),
            json!(["return", "n"]),
        ),
        (
            json!([["n", "none"], ["v", "call?", "fetch", "n"]]),
            json!(["return", "n"]),
        ),
    ] {
        executes(&request("Option<i8>", ops, term), &json!("None"));
    }
}

#[test]
fn earlier_checked_call_hint_precedes_later_return_and_explicit_type_overrides_hints() {
    for (ops, at) in [
        (
            json!([["n", "none"], ["v", "call?", "fetch", "n"]]),
            "/bindings/success/term/1",
        ),
        (
            json!([{"name":"n","op":"none","type":"Option<i8>"},["v","call","fetch","n"]]),
            "/bindings/success/term/1",
        ),
    ] {
        let fixture = super::interface_call_tests::fixture(true);
        let request = request("Option<i64>", ops, json!(["return", "n"]));
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        let detail = result["detail"].as_str().unwrap();
        assert!(
            detail.contains(at) && detail.contains("Option<i8>") && detail.contains("Option<i64>"),
            "{result}"
        );
        assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    }
}

#[test]
fn result_constructor_return_hints_resolve_named_ok_and_err_without_changing_values() {
    for (ops, result) in [
        (json!([["r", "ok", "x"]]), json!({"Ok":3})),
        (json!([["r", "err", "x"]]), json!({"Err":3})),
    ] {
        let fixture = super::interface_call_tests::fixture(true);
        let request = request("Result<i8,i8>", ops, json!(["return", "r"]));
        let report = super::interface_tests::check(&fixture, &request, &json!({})).unwrap();
        let operation = report["connections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["at"] == "/bindings/success/ops/0")
            .unwrap();
        assert_eq!(
            operation["expression_types"], "connections_checked",
            "{report}"
        );
        let (code, tried) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{tried}");
        assert_eq!(tried["kernel"], "valid");
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
        assert_eq!(called["result"], result);
    }
}
