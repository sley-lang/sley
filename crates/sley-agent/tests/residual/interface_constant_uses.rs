use super::interface_tests::{check, predicate_request};
use super::{Fixture, branch_request, cli};
use serde_json::{Value, json};

fn request(returns: &str, ops: Value, term: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!(returns);
    request["bindings"]["success"] = json!({"ops":[],"term":[]});
    request["bindings"]["success"]["ops"] = ops;
    request["bindings"]["success"]["term"] = term;
    request
}

fn helpers() -> Fixture {
    let fixture = Fixture::new();
    let mut source = super::interface_call_tests::helpers();
    source["fns"]
        .as_array_mut()
        .unwrap()
        .push(json!({"fn":"wide","params":[["z","i64"]],
        "returns":"i64","blocks":[{"name":"entry","ops":[],"term":["return","z"]}]}));
    let (code, report) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{report}");
    let (code, report) = cli(
        &fixture.dir,
        &["commit", report["handle"].as_str().unwrap()],
    );
    assert_eq!(code, 0, "{report}");
    fixture
}

#[test]
fn direct_const_return_call_uses_and_explicit_annotations_preserve_actual_vm_results() {
    for (returns, ops, term) in [
        ("i8", json!([]), json!(["return", ["const", 7]])),
        (
            "i8",
            json!([]),
            json!(["return", ["call", "echo", ["const", 7]]]),
        ),
        (
            "i8",
            json!([["k", "constant_ref", 7]]),
            json!(["return", "k#0"]),
        ),
        (
            "i8",
            json!([["k", "1", 7], ["used", "call", "echo", "k"]]),
            json!(["return", "used"]),
        ),
        (
            "u8",
            json!([{"name":"k","op":"const","args":[7],"type":"u8"}]),
            json!(["return", "k"]),
        ),
        (
            "i8",
            json!([]),
            json!(["return",["const",{"type":"i8","value":7}]]),
        ),
        ("i64", json!([]), json!(["return",["const",{"value":7}]])),
    ] {
        let fixture = helpers();
        let request = request(returns, ops, term);
        let before = request.clone();
        check(&fixture, &request, &json!({})).unwrap();
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
        assert_eq!(called["result"], 7);
        assert_eq!(request, before);
    }
}

#[test]
fn return_before_calls_and_first_call_hints_reject_the_actual_conflicting_connection() {
    for (returns, ops, term, at) in [
        (
            "i8",
            json!([["k", "const", 7], ["used", "call", "wide", "k"]]),
            json!(["return", "k"]),
            "/bindings/success/ops/1/3",
        ),
        (
            "i64",
            json!([["k", "const", 7], ["used", "call", "echo", "k"]]),
            json!(["return", "k"]),
            "/bindings/success/ops/1/3",
        ),
        (
            "i8",
            json!([
                ["k", "const", 7],
                ["narrow", "call", "echo", "k"],
                ["wide", "call", "wide", "k"]
            ]),
            json!(["return", 0]),
            "/bindings/success/ops/2/3",
        ),
        (
            "i8",
            json!([]),
            json!(["return",["const",{"value":7}]]),
            "/bindings/success/term/1",
        ),
        (
            "Option<i8>",
            json!([]),
            json!(["return", ["some", ["const", 7]]]),
            "/bindings/success/term/1",
        ),
    ] {
        let fixture = helpers();
        let request = request(returns, ops, term);
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(
            error.code(),
            sley_agent::AgentErrorCode::ResidualConstraintConflict
        );
        assert!(error.detail().contains(at), "{error}");
        assert!(
            error.detail().contains("i8") && error.detail().contains("i64"),
            "{error}"
        );
    }
}

fn branch(ops: Value) -> Value {
    let mut request = branch_request();
    request["bindings"]["params"] = json!([["maybe", "Option<i8>"]]);
    request["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
    request["bindings"]["cases"] = json!([
        {"case":"Some","payload":["x","i8"],"ops":[],"values":["k"]},
        {"case":"None","payload":null,"ops":[],"values":[["const",17]]}
    ]);
    request["bindings"]["cases"][0]["ops"] = ops;
    request["bindings"]["join"]["params"] = json!([["answer", "i8"]]);
    request
}

#[test]
fn branch_edge_hints_after_checked_splits_preserve_the_ordinary_piece_order() {
    let fixture = helpers();
    let request = branch(json!([["k", "const", 7], ["step", "add?", "x", 1]]));
    check(&fixture, &request, &json!({})).unwrap();
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (input, expected) in [
        (json!({"Some":3}), json!({"Ok":7})),
        (json!("None"), json!({"Ok":17})),
    ] {
        let (code, called) = cli(
            &fixture.dir,
            &[
                "call",
                "checked",
                &input.to_string(),
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{called}");
        assert_eq!(called["result"], expected);
    }
    for (ops, at) in [
        (
            json!([["k", "const", 7], ["used", "call", "wide", "k"]]),
            "/bindings/cases/0/ops/1/3",
        ),
        (
            json!([
                ["k", "const", 7],
                ["used", "call", "wide", "k"],
                ["step", "add?", "x", 1]
            ]),
            "/bindings/cases/0/values/0",
        ),
    ] {
        let request = branch(ops);
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert!(error.detail().contains(at), "{error}");
        assert!(
            error.detail().contains("i8") && error.detail().contains("i64"),
            "{error}"
        );
    }
}

#[test]
fn every_branch_argument_is_lowered_before_the_final_edge_hint_order() {
    let fixture = helpers();
    let mut request = branch(json!([["k", "const", 7]]));
    request["bindings"]["params"] = json!([["maybe", "Option<i64>"]]);
    request["bindings"]["cases"][0]["payload"] = json!(["x", "i64"]);
    request["bindings"]["cases"][0]["values"] = json!(["k", ["add?", ["call", "wide", "k"], 1]]);
    request["bindings"]["cases"][1]["values"] = json!([["const", 17], ["const", 0]]);
    request["bindings"]["join"]["params"] = json!([["answer", "i8"], ["unused", "i64"]]);
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert_eq!(
        error.code(),
        sley_agent::AgentErrorCode::ResidualConstraintConflict
    );
    assert!(
        error.detail().contains("/bindings/cases/0/values/0"),
        "{error}"
    );
    assert!(
        error.detail().contains("i64") && error.detail().contains("i8"),
        "{error}"
    );
}

#[test]
fn default_const_width_conflict_refuses_before_expansion_or_draft_creation() {
    let fixture = Fixture::new();
    let request = request(
        "bool",
        json!([]),
        json!(["return", ["eq", ["const", 1], "x"]]),
    );
    let head = fixture.workspace.read_head().unwrap().transaction_id();
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
        detail.contains("/bindings/success/term/1")
            && detail.contains("i64")
            && detail.contains("i8"),
        "{result}"
    );
    for name in ["drafts", "candidates", "residual"] {
        assert!(!fixture.dir.join(".sley").join(name).exists(), "{name}");
    }
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        head
    );
}

#[test]
fn checked_statement_continuation_width_conflicts_refuse_before_expansion() {
    let fixture = helpers();
    let request = request(
        "Result<i8,ArithmeticError>",
        json!([
            ["k", "const", 7],
            ["sum", "add?", "k", "x"],
            ["used", "call", "echo", "k"]
        ]),
        json!(["ok", "sum"]),
    );
    let before = fixture.workspace.read_head().unwrap().transaction_id();
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
        detail.contains("/bindings/success/ops/1")
            && detail.contains("i8")
            && detail.contains("i64"),
        "{result}"
    );
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
    assert!(!fixture.dir.join(".sley/drafts/d2").exists());
}

#[test]
fn explicitly_typed_checked_statement_continuations_preserve_vm_results() {
    for constant in [
        json!({"name":"k","op":"const","args":[7],"type":"i8"}),
        json!(["k","const",{"type":"i8","value":7}]),
    ] {
        let fixture = helpers();
        let request = request(
            "Result<i8,ArithmeticError>",
            json!([
                constant,
                ["sum", "add?", "k", "x"],
                ["used", "call", "echo", "k"]
            ]),
            json!(["ok", "sum"]),
        );
        check(&fixture, &request, &json!({})).unwrap();
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        for (x, expected) in [("3", json!({"Ok":10})), ("-7", json!({"Ok":0}))] {
            let (code, called) = cli(
                &fixture.dir,
                &[
                    "call",
                    "checked",
                    x,
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
