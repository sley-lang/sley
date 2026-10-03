use super::interface_tests::{check, predicate_request};
use super::{Fixture, branch_request, bytes, cli, interface_call_tests, planning_request};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn request(expression: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["params"] = json!([
        ["x", "i8"],
        ["y", "i8"],
        ["z", "i8"],
        ["shift", "u32"],
        ["ready", "bool"]
    ]);
    request["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
    request["bindings"]["success"] = json!({"ops":[],"term":["return",null]});
    request["bindings"]["success"]["term"][1] = expression;
    request
}

fn connection<'a>(report: &'a Value, at: &str) -> &'a Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["at"] == at)
        .unwrap_or_else(|| panic!("missing {at}: {report}"))
}

#[test]
fn arithmetic_connections_check_arity_width_wrapping_and_shift_contexts() {
    let fixture = Fixture::new();
    for expression in [
        json!(["add", "x", 1]),
        json!(["int_add_checked", 1, "x"]),
        json!(["64", "x", "y"]),
        json!(["shl", "x", "shift"]),
        json!(["shr", "x", 3]),
        json!(["neg", "x"]),
    ] {
        let request = request(expression);
        let before = bytes(&request);
        let report = check(&fixture, &request, &json!({})).unwrap();
        assert_eq!(
            connection(&report, "/bindings/success/term/1")["expression_types"],
            "connections_checked"
        );
        assert_eq!(bytes(&request), before);
    }
    for expression in [
        json!(["add", "x"]),
        json!(["neg", "x", "y"]),
        json!(["add", "x", 128]),
        json!(["add", 128, "x"]),
        json!(["add","x",{"type":"u8","value":1}]),
        json!(["shl", "x", "y"]),
        json!(["shr", "x", 4_294_967_296_u64]),
        json!(["add", "ready", true]),
        json!(["add", ["mul", "x", 2], 1]),
    ] {
        let request = request(expression);
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
                .contains("/bindings/success/term/1"),
            "{result}"
        );
    }
    for path in [".sley/drafts", ".sley/candidates", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}

#[test]
fn wrapped_annotations_and_bare_failure_types_are_checked_separately() {
    let fixture = Fixture::new();
    for annotation in ["i8", "Result<i8,IndexError>", "Result<u8,ArithmeticError>"] {
        let mut request = request(json!("x"));
        request["bindings"]["success"] = json!({"ops":[{"name":"out","op":"neg?","args":["x"],"type":annotation}],"term":["ok","out"]});
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert!(
            error.detail().contains("/bindings/success/ops/0"),
            "{error}"
        );
    }
    for returns in ["Option<i8>", "Result<i8,IndexError>", "i8"] {
        let mut request = request(json!(["add?", "x", 1]));
        request["bindings"]["returns"] = json!(returns);
        // Use an unconstrained named operation so the failure-route conflict is
        // checked independently of the eventual return expression.
        request["bindings"]["success"] =
            json!({"ops":[["out","add?","x",1]],"term":["return","out"]});
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert!(
            error.detail().contains("bare checked propagation"),
            "{error}"
        );
        assert!(error.detail().contains("/bindings/returns"), "{error}");
    }
}

#[test]
fn nested_checked_arithmetic_preserves_first_failure_and_vm_results() {
    let fixture = Fixture::new();
    let mut request = request(json!("x"));
    request["bindings"]["success"] =
        json!({"ops":[["out","add?",["div?","x","y"],["div?",1,"z"]]],"term":["ok","out"]});
    let before = bytes(&request);
    let report = check(&fixture, &request, &json!({})).unwrap();
    let operation = connection(&report, "/bindings/success/ops/0");
    assert_eq!(operation["expression_types"], "connections_checked");
    assert_eq!(operation["propagation"].as_array().unwrap().len(), 3);
    assert_eq!(
        operation["propagation"][0]["at"],
        "/bindings/success/ops/0/2"
    );
    assert_eq!(
        operation["propagation"][1]["at"],
        "/bindings/success/ops/0/3"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (x, y, z, expected) in [
        (
            "-128",
            "-1",
            "0",
            json!({"Err":{"ArithmeticError":"Overflow"}}),
        ),
        (
            "7",
            "1",
            "0",
            json!({"Err":{"ArithmeticError":"DivideByZero"}}),
        ),
        ("7", "1", "1", json!({"Ok":8})),
        (
            "127",
            "1",
            "1",
            json!({"Err":{"ArithmeticError":"Overflow"}}),
        ),
    ] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", x, y, z, "0", "false", "--on", "c1"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
    assert_eq!(bytes(&request), before);
}

#[test]
fn checked_values_flow_into_comparisons_and_branch_joins() {
    let fixture = Fixture::new();
    let mut request = request(json!("x"));
    request["bindings"]["returns"] = json!("Result<bool,ArithmeticError>");
    request["bindings"]["success"] =
        json!({"ops":[["flag","lt",["add?","x",1],0]],"term":["ok","flag"]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/ops/0")["expression_types"],
        "connections_checked"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(
        &fixture.dir,
        &[
            "call", "checked", "-2", "1", "1", "0", "false", "--on", "c1",
        ],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], json!({"Ok":true}));
    let mut branch = branch_request();
    branch["bindings"]["join"]["params"][0][1] = json!("i16");
    let error = check(&fixture, &branch, &json!({})).unwrap_err();
    assert!(
        error.detail().contains("/bindings/cases/0/values/0"),
        "{error}"
    );
    assert!(
        error.detail().contains("/bindings/join/params/0"),
        "{error}"
    );
}

#[test]
fn named_arithmetic_failure_routes_drop_or_preserve_the_exact_payload() {
    for (route, status, expected) in [
        ("Drop", "drop_into_unit_case", json!({"Err":"Drop"})),
        (
            "Keep",
            "preserve_in_named_case",
            json!({"Err":{"Keep":{"ArithmeticError":"Overflow"}}}),
        ),
    ] {
        let fixture = Fixture::new();
        let declarations = json!({"af1":1,"types":[{"name":"Errors","variant":["Drop",["Keep","ArithmeticError"],["Wrong","IndexError"]]}]});
        let (code, result) = cli(
            &fixture.dir,
            &["try", &declarations.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        let mut request = request(json!("x"));
        request["base"] = json!("d1@r1");
        request["bindings"]["returns"] = json!("Result<i8,Errors>");
        request["bindings"]["success"] =
            json!({"ops":[["out",format!("neg?{route}"),"x"]],"term":["ok","out"]});
        let report = check(&fixture, &request, &declarations).unwrap();
        assert_eq!(
            connection(&report, "/bindings/success/ops/0")["propagation"][0]["failure_route"],
            status
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        let (code, result) = cli(
            &fixture.dir,
            &[
                "call", "checked", "-128", "0", "0", "0", "false", "--on", "c2",
            ],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
        request["bindings"]["success"]["ops"][0][1] = json!("neg?Wrong");
        let error = check(&fixture, &request, &declarations).unwrap_err();
        assert!(error.detail().contains("case `Wrong`"), "{error}");
    }
}

#[test]
fn checked_calls_unwrap_options_and_check_failure_shape() {
    let fixture = interface_call_tests::fixture(true);
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["params"] = json!([["maybe", "Option<i8>"]]);
    request["bindings"]["success"] =
        json!({"ops":[["out","call?","fetch","maybe"]],"term":["return",["some","out"]]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/ops/0")["propagation"][0]["unwrapped_type"],
        "i8"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for input in [json!("None"), json!({"Some":9})] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", &input.to_string(), "--on", "c2"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], input);
    }
    request["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(
        error.detail().contains("bare checked propagation"),
        "{error}"
    );
    request["bindings"]["success"]["ops"][0] = json!(["out", "not?", true]);
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(
        error.detail().contains("requires Result or Option"),
        "{error}"
    );
}

#[test]
fn unknown_widths_stay_deferred_but_missing_handlers_refuse() {
    let fixture = Fixture::new();
    let mut request = request(json!("x"));
    request["bindings"]["success"] = json!({"ops":[["out","add?",1,2]],"term":["ok","x"]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    let operation = connection(&report, "/bindings/success/ops/0");
    assert_eq!(operation["expression_types"], "partial");
    assert_eq!(
        operation["propagation"][0]["failure_route"],
        "preserve_failure"
    );
    assert!(operation["propagation"][0]["unwrapped_type"].is_null());
    request["bindings"]["success"]["ops"][0] = json!(["out", "add?handler", "x", 1]);
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(
        error.detail().contains("exposes no handler blocks"),
        "{error}"
    );
    assert!(
        error.detail().contains("/bindings/success/ops/0"),
        "{error}"
    );
}

#[test]
fn unsigned_negation_refuses_in_regions_and_pipeline_after_constraint_resolution() {
    let fixture = Fixture::new();
    let mut request = request(json!(["neg", "x"]));
    request["bindings"]["params"][0][1] = json!("u8");
    request["bindings"]["returns"] = json!("Result<u8,ArithmeticError>");
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(error.detail().contains("signed integer"), "{error}");
    for operand in [json!("x"), json!(1)] {
        let mut request = planning_request();
        request["bindings"]["params"] = json!([["x", "u8"]]);
        request["bindings"]["returns"] = json!("Result<u8,ArithmeticError>");
        request["bindings"]["steps"] = json!([["out", ["neg", operand]]]);
        request["bindings"]["result"] = json!("out");
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("signed integer"), "{error}");
        assert!(error.detail().contains("/bindings/steps/0/1"), "{error}");
    }
}

#[test]
fn partner_inference_updates_each_checked_expression_evidence_once() {
    let fixture = Fixture::new();
    let mut request = request(json!("x"));
    request["bindings"]["returns"] = json!("Result<bool,ArithmeticError>");
    request["bindings"]["success"] =
        json!({"ops":[["flag","eq",["add?",["mul?",1,2],3],"x"]],"term":["ok","flag"]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    let operation = connection(&report, "/bindings/success/ops/0");
    assert_eq!(
        operation["expression_types"], "connections_checked",
        "{report}"
    );
    assert_eq!(operation["deferred_expressions"], json!([]));
    let propagation = operation["propagation"].as_array().unwrap();
    assert_eq!(propagation.len(), 2);
    assert_eq!(propagation[0]["at"], "/bindings/success/ops/0/2/1");
    assert_eq!(propagation[1]["at"], "/bindings/success/ops/0/2");
    for entry in propagation {
        assert_eq!(entry["unwrapped_type"], "i8");
    }
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "5", "0", "0", "0", "false", "--on", "c1"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], json!({"Ok":true}));
}

#[test]
fn option_failure_mapping_can_drop_none_but_cannot_invent_a_payload() {
    let fixture = Fixture::new();
    let mut declarations = interface_call_tests::helpers();
    declarations["types"] = json!([{"name":"Errors","variant":["Drop",["Carry","i8"]]}]);
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let mut request = request(json!("x"));
    request["base"] = json!("d1@r1");
    request["bindings"]["params"] = json!([["maybe", "Option<i8>"]]);
    request["bindings"]["returns"] = json!("Result<i8,Errors>");
    request["bindings"]["success"] =
        json!({"ops":[["out","call?Drop","fetch","maybe"]],"term":["ok","out"]});
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/ops/0")["propagation"][0]["failure_route"],
        "drop_into_unit_case"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (input, expected) in [
        (json!("None"), json!({"Err":"Drop"})),
        (json!({"Some":7}), json!({"Ok":7})),
    ] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", &input.to_string(), "--on", "c2"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
    request["bindings"]["success"]["ops"][0][1] = json!("call?Carry");
    let error = check(&fixture, &request, &declarations).unwrap_err();
    assert!(error.detail().contains("case `Carry`"), "{error}");
}

#[test]
fn shift_validation_keeps_runtime_overflow_and_invalid_shift_distinct() {
    let fixture = Fixture::new();
    let request = request(json!(["shl", "x", "shift"]));
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (x, shift, expected) in [
        ("1", "2", json!({"Ok":4})),
        ("1", "7", json!({"Err":{"ArithmeticError":"Overflow"}})),
        ("1", "8", json!({"Err":{"ArithmeticError":"InvalidShift"}})),
    ] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", x, "0", "0", shift, "false", "--on", "c1"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
}
