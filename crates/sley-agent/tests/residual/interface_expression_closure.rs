use super::interface_declaration_tests::request;
use super::interface_tests::check;
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn with_op(op: Value) -> Value {
    let mut request = request();
    request["bindings"]["success"]["ops"] = json!([null]);
    request["bindings"]["success"]["ops"][0] = op;
    request
}

fn op_report(report: &Value) -> &Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == "/bindings/success/ops/0")
        .unwrap()
}

#[test]
fn unused_operation_annotations_require_closed_types_before_publication() {
    let fixture = Fixture::new();
    for (op, ty) in [
        ("none", "Option<$0>"),
        ("none", "Option<Map<Vec<i8>,bool>>"),
        ("future_operation", "$0"),
    ] {
        let request = with_op(json!({"name":"unused","op":op,"args":[],"type":ty}));
        let before = bytes(&request);
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/bindings/success/ops/0/type"),
            "{error}"
        );
        assert!(
            error.detail().contains("not closed and well-formed"),
            "{error}"
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        assert_eq!(bytes(&request), before);
    }
    for artifact in ["drafts", "candidates", "residual"] {
        assert!(!fixture.dir.join(".sley").join(artifact).exists());
    }
}

#[test]
fn incomplete_annotation_definitions_refuse_at_authored_type() {
    let fixture = Fixture::new();
    let request = with_op(
        json!({"name":"unused","opcode":"option_none","operands":[],"type":"Option<Pack>"}),
    );
    let incomplete = json!({"types":[{"name":"Pack","record":[["value"]]}]});
    let error = check(&fixture, &request, &incomplete).unwrap_err();
    super::interface_closure_tests::assert_incomplete(&error, "/bindings/success/ops/0/type");
    let complete = json!({"types":[{"name":"Pack","record":[["value","i8"]]}]});
    let report = check(&fixture, &request, &complete).unwrap();
    assert_eq!(
        op_report(&report)["expression_types"],
        "connections_checked"
    );
    assert_eq!(op_report(&report)["closed_types"][0]["closure"], "checked");
}

#[test]
fn callee_and_reference_types_close_independently_of_operand_resolution() {
    let fixture = Fixture::new();
    for (params, returns, operation, location, source) in [
        (
            json!([["arg", "Map<Vec<i8>,bool>"]]),
            "bool",
            json!(["unused", "call", "bad", ["future_expression"]]),
            "/3",
            "parameter 0",
        ),
        (
            json!([]),
            "Map<Vec<i8>,bool>",
            json!(["unused", "call", "bad"]),
            "",
            "callee `bad` result",
        ),
        (
            json!([["arg", "$0"]]),
            "bool",
            json!(["unused", "fnref", "bad"]),
            "",
            "function",
        ),
    ] {
        let declarations = json!({"af1":1,"afx":1,"fns":[{"fn":"bad","params":params,"returns":returns,"blocks":[]}]});
        let error = check(&fixture, &with_op(operation), &declarations).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error
                .detail()
                .contains(&format!("/bindings/success/ops/0{location}")),
            "{error}"
        );
        assert!(error.detail().contains(source), "{error}");
        assert!(
            error.detail().contains("not closed and well-formed"),
            "{error}"
        );
    }
    let declarations = json!({"consts":[{"name":"bad","type":"Map<Vec<i8>,bool>","value":[]}]});
    let error = check(
        &fixture,
        &with_op(json!(["unused", "const", "bad"])),
        &declarations,
    )
    .unwrap_err();
    assert!(error.detail().contains("constant `bad`"), "{error}");
    assert!(error.detail().contains("TYPE_NOT_ORDERABLE"), "{error}");
}

#[test]
fn incomplete_callee_results_and_reference_types_cannot_claim_complete_connections() {
    let fixture = Fixture::new();
    let declarations = json!({"types":[{"name":"Pack","record":[["value"]]}],
        "fns":[{"fn":"make","params":[],"returns":"Pack","blocks":[]}],
        "consts":[{"name":"saved","type":"Pack","value":{}}]});
    for op in [
        json!(["unused", "call", "make"]),
        json!(["unused", "fnref", "make"]),
        json!(["unused", "const", "saved"]),
    ] {
        let error = check(&fixture, &with_op(op), &declarations).unwrap_err();
        super::interface_closure_tests::assert_incomplete(&error, "/bindings/success/ops/0");
    }
}

#[test]
fn valid_annotations_calls_and_function_references_preserve_vm_execution() {
    let fixture = Fixture::new();
    let declarations = super::interface_call_tests::helpers();
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    let mut request = with_op(json!({"name":"empty","op":"none","args":[],"type":"Option<i8>"}));
    request["bindings"]["success"]["ops"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!(["function", "fnref", "echo"]),
            json!(["result", "call", "echo", "x"]),
        ]);
    request["bindings"]["success"]["term"] = json!(["return", "result"]);
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(op_report(&report)["closed_types"][0]["closure"], "checked");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "7", "true", "--on", "c2"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], 7);
}

#[test]
fn annotation_depth_limits_remain_resource_refusals() {
    let fixture = Fixture::new();
    let ty = format!("{}i8{}", "Option<".repeat(65), ">".repeat(65));
    let request = with_op(json!({"name":"unused","op":"none","args":[],"type":ty}));
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(
        error.detail().contains("/bindings/success/ops/0/type"),
        "{error}"
    );
}
