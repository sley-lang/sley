use super::interface_tests::{check, predicate_request};
use super::{Fixture, branch_request, bytes, cli, interface_call_tests};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn declarations() -> Value {
    let mut frame = interface_call_tests::helpers();
    frame["types"] = json!([{"name":"Errors","variant":["Early",["Value","i8"],["Wide","i16"],["Math","ArithmeticError"]]}]);
    frame
}

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations().to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    fixture
}

fn named_request(ops: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["base"] = json!("d1@r1");
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("Result<i8,Errors>");
    request["bindings"]["success"] = json!({"ops":[],"term":["ok","x"]});
    request["bindings"]["success"]["ops"] = ops;
    request
}

fn at<'a>(report: &'a Value, pointer: &str) -> &'a Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == pointer)
        .unwrap_or_else(|| panic!("missing {pointer}: {report}"))
}

#[test]
fn malformed_nonboolean_and_bare_payload_exits_refuse_before_publication() {
    let fixture = Fixture::new();
    for exit in [
        json!(["!", "if", 1]),
        json!(["!", "if", "x"]),
        json!(["!", "if", ["add", "x", 1]]),
        json!(["!", "if", true, "x"]),
        json!(["!", "unless", true]),
        json!(["!", "if"]),
        json!(["!", "if", true, 1, 2]),
        json!(["!bad target", "if", true]),
    ] {
        let mut request = predicate_request(json!(false));
        request["bindings"]["success"]["ops"] = json!([exit]);
        let before = bytes(&request);
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
                .contains("/bindings/success/ops/0"),
            "{result}"
        );
        assert_eq!(bytes(&request), before);
    }
    for path in [".sley/candidates", ".sley/drafts", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}

#[test]
fn bare_conditional_exit_matches_option_vm_semantics() {
    let fixture = Fixture::new();
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["success"] =
        json!({"ops":[["!","if",["lt","x",0]],["saved","some","x"]],"term":["return","saved"]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    let exit = at(&report, "/bindings/success/ops/0");
    assert_eq!(exit["conditional_exit"]["condition"], "bool_checked");
    assert_eq!(
        exit["conditional_exit"]["failure_route"],
        "option_none_checked"
    );
    assert_eq!(exit["expression_types"], "connections_checked");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (x, expected) in [("-128", json!("None")), ("3", json!({"Some":3}))] {
        let (code, result) = cli(&fixture.dir, &["call", "checked", x, "true", "--on", "c1"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
    request["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(error.detail().contains("requires Option"), "{error}");
}

#[test]
fn named_exits_preserve_order_and_thread_computed_payloads() {
    let fixture = fixture();
    let mut request = named_request(json!([
        ["first", "lt", "x", 0],
        ["!Early", "if", "first"],
        ["out", "neg?Math", "x"],
        ["!Value", "if", "ready", "out"]
    ]));
    request["bindings"]["success"]["term"] = json!(["ok", "out"]);
    let before = bytes(&request);
    let report = check(&fixture, &request, &declarations()).unwrap();
    for pointer in ["/bindings/success/ops/1", "/bindings/success/ops/3"] {
        assert_eq!(
            at(&report, pointer)["conditional_exit"]["failure_route"],
            "named_case_checked"
        );
        assert_eq!(
            at(&report, pointer)["expression_types"],
            "connections_checked"
        );
    }
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (x, ready, expected) in [
        ("-128", "true", json!({"Err":"Early"})),
        ("3", "true", json!({"Err":{"Value":-3}})),
        ("3", "false", json!({"Ok":-3})),
    ] {
        let (code, result) = cli(&fixture.dir, &["call", "checked", x, ready, "--on", "c2"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
    assert_eq!(bytes(&request), before);
}

#[test]
fn named_payload_presence_width_range_and_nesting_conflicts_are_exact() {
    let fixture = fixture();
    for exit in [
        json!(["!Early", "if", true, "x"]),
        json!(["!Value", "if", true]),
        json!(["!Value", "if", false, 128]),
        json!(["!Value", "if", true, "ready"]),
        json!(["!Wide", "if", true, "x"]),
        json!(["!Value", "if", true, ["call", "echo", "x"]]),
    ] {
        let request = named_request(json!([exit]));
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
                .contains("/bindings/success/ops/0/3"),
            "{result}"
        );
    }
    for path in [
        ".sley/candidates/c2.hex",
        ".sley/candidates/c2.json",
        ".sley/drafts/d1/r2",
        ".sley/drafts/d2",
    ] {
        assert!(!fixture.dir.join(path).exists());
    }
}

#[test]
fn checked_condition_failure_precedes_the_selected_exit() {
    let fixture = fixture();
    let mut request = named_request(json!([[
        "!Early",
        "if",
        ["gt", ["div?Math", "x", "divisor"], 0]
    ]]));
    request["bindings"]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!(["divisor", "i8"]));
    let report = check(&fixture, &request, &declarations()).unwrap();
    let exit = at(&report, "/bindings/success/ops/0");
    assert_eq!(exit["conditional_exit"]["condition"], "bool_checked");
    assert_eq!(
        exit["propagation"][0]["failure_route"],
        "preserve_in_named_case"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (x, divisor, expected) in [
        (
            "3",
            "0",
            json!({"Err":{"Math":{"ArithmeticError":"DivideByZero"}}}),
        ),
        ("3", "1", json!({"Err":"Early"})),
        ("-3", "1", json!({"Ok":-3})),
    ] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", x, "false", divisor, "--on", "c2"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
}

#[test]
fn cell_conditions_are_checked_and_missing_handler_edges_refuse() {
    let fixture = Fixture::new();
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["success"]["ops"] =
        json!([["flag", "cell_get", ["cell", "ready"]], ["!", "if", "flag"]]);
    let report = check(&fixture, &request, &json!({})).unwrap();
    let exit = at(&report, "/bindings/success/ops/1");
    assert_eq!(exit["conditional_exit"]["condition"], "bool_checked");
    assert_eq!(
        exit["conditional_exit"]["failure_route"],
        "option_none_checked"
    );
    assert_eq!(exit["expression_types"], "connections_checked");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    for (ready, expected) in [("true", json!("None")), ("false", json!({"Some":3}))] {
        let (code, result) = cli(&fixture.dir, &["call", "checked", "3", ready, "--on", "c1"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
    request["bindings"]["success"]["ops"] = json!([["!handler", "if", "ready", "x"]]);
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(
        error.detail().contains("exposes no handler blocks"),
        "{error}"
    );
    assert!(
        error.detail().contains("/bindings/success/ops/0/0"),
        "{error}"
    );
}

#[test]
fn exits_in_case_and_join_regions_keep_payload_scope_and_vm_results() {
    let fixture = fixture();
    let mut request = branch_request();
    request["base"] = json!("d1@r1");
    request["bindings"]["returns"] = json!("Result<i8,Errors>");
    request["bindings"]["cases"][0]["ops"] = json!([
        ["!Early", "if", ["lt", "x", 0]],
        ["negated", "neg?Math", "x"]
    ]);
    request["bindings"]["join"]["ops"] = json!([["!Value", "if", ["gt", "answer", 10], "answer"]]);
    let report = check(&fixture, &request, &declarations()).unwrap();
    for pointer in ["/bindings/cases/0/ops/0", "/bindings/join/ops/0"] {
        assert_eq!(
            at(&report, pointer)["conditional_exit"]["failure_route"],
            "named_case_checked"
        );
    }
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (input, expected) in [
        (json!({"Some":-4}), json!({"Err":"Early"})),
        (json!({"Some":4}), json!({"Ok":-4})),
        (json!("None"), json!({"Err":{"Value":17}})),
    ] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", &input.to_string(), "--on", "c2"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
    request["bindings"]["join"]["ops"][0][3] = json!("x");
    let error = check(&fixture, &request, &declarations()).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(error.detail().contains("/bindings/join/ops/0/3"), "{error}");
}
