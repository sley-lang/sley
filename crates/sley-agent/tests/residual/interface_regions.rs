use super::interface_tests::{check, predicate_request};
use super::{Fixture, branch_request, bytes, cli, interface_call_tests};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn at<'a>(report: &'a Value, pointer: &str) -> &'a Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["at"] == pointer)
        .unwrap_or_else(|| panic!("missing {pointer}: {report}"))
}

#[test]
fn known_return_and_ok_conflicts_refuse_before_publication() {
    let fixture = Fixture::new();
    for term in [
        json!(["return", "x"]),
        json!(["return", ["some", "ready"]]),
        json!(["ok", "x"]),
        json!(["return", ["some", 128]]),
        json!(["return", "x", "ready"]),
    ] {
        let mut request = predicate_request(json!(false));
        request["bindings"]["success"]["term"] = term;
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
                .contains("/bindings/success/term"),
            "{result}"
        );
        assert_eq!(bytes(&request), before);
    }
    for path in [".sley/candidates", ".sley/drafts", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}

#[test]
fn terminal_region_boolean_results_flow_to_constructor_and_match_vm() {
    for object_syntax in [false, true] {
        let fixture = Fixture::new();
        let mut request = predicate_request(json!(false));
        request["bindings"]["returns"] = json!("Option<bool>");
        request["bindings"]["success"] = json!({"ops":[["negative","lt","x",0],["selected","and","negative#0","ready"]],"term":["return",["some","selected"]]});
        if object_syntax {
            request["bindings"]["success"]["ops"][1] = json!({"name":"selected","opcode":"and","operands":["negative#0","ready"],"type":"bool"});
        }
        let report = check(&fixture, &request, &json!({})).unwrap();
        for pointer in [
            "/bindings/success/ops/0",
            "/bindings/success/ops/1",
            "/bindings/success/term/1",
        ] {
            assert_eq!(
                at(&report, pointer)["expression_types"],
                "connections_checked"
            );
        }
        assert_eq!(report["composition"], "partial");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        for (x, ready, expected) in [
            ("-2", "true", true),
            ("-2", "false", false),
            ("2", "true", false),
        ] {
            let (code, result) = cli(&fixture.dir, &["call", "checked", x, ready, "--on", "c1"]);
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], json!({"Some":expected}));
        }
    }
}

#[test]
fn case_operations_no_longer_suppress_independent_join_conflicts() {
    let fixture = Fixture::new();
    let mut request = branch_request();
    request["bindings"]["cases"][0]["ops"] =
        json!([["unresolved", "cell_get", ["future_expression"]]]);
    request["bindings"]["cases"][0]["values"] = json!(["x"]);
    request["bindings"]["join"]["params"][0][1] = json!("i16");
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(
        error.detail().contains("/bindings/cases/0/values/0"),
        "{error}"
    );
    assert!(
        error.detail().contains("/bindings/join/params/0"),
        "{error}"
    );
    request["bindings"]["cases"][0]["values"] = json!(["unresolved"]);
    request["bindings"]["returns"] = json!("Result<i16,ArithmeticError>");
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        at(&report, "/bindings/cases/0/values/0")["expression_types"],
        "partial"
    );
}

#[test]
fn branch_local_values_join_and_return_match_kernel_and_vm() {
    let fixture = Fixture::new();
    let mut request = branch_request();
    request["bindings"]["returns"] = json!("Option<bool>");
    request["bindings"]["cases"][0]["ops"] = json!([["flag", "lt", "x", 0]]);
    request["bindings"]["cases"][0]["values"] = json!(["flag"]);
    request["bindings"]["cases"][1]["values"] = json!([false]);
    request["bindings"]["join"] = json!({"params":[["flag","bool"]],"ops":[["inverted","not","flag"]],"term":["return",["some","inverted"]]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    for pointer in [
        "/bindings/cases/0/ops/0",
        "/bindings/cases/0/values/0",
        "/bindings/cases/1/values/0",
        "/bindings/join/ops/0",
        "/bindings/join/term/1",
    ] {
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
    for (input, expected) in [
        (json!({"Some":-2}), false),
        (json!({"Some":2}), true),
        (json!("None"), true),
    ] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", &input.to_string(), "--on", "c1"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!({"Some":expected}));
    }
}

#[test]
fn named_calls_report_actual_array_and_object_operand_locations() {
    let fixture = interface_call_tests::fixture(true);
    for (operation, pointer) in [
        (
            json!(["out", "call", "echo", "ready"]),
            "/bindings/success/ops/0/3",
        ),
        (
            json!({"name":"out","op":"call","args":["echo","ready"]}),
            "/bindings/success/ops/0/args/1",
        ),
        (
            json!({"name":"out","opcode":"call_direct","operands":["echo","ready"]}),
            "/bindings/success/ops/0/operands/1",
        ),
        (
            json!({"name":"out","op":"call","args":["echo","x"],"type":"bool"}),
            "/bindings/success/ops/0",
        ),
    ] {
        let mut request = predicate_request(json!(false));
        request["bindings"]["success"]["ops"] = json!([operation]);
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains(pointer), "{error}");
    }
    let mut request = predicate_request(json!(false));
    request["bindings"]["success"] =
        json!({"ops":[["out","call","echo","x"]],"term":["return",["some","out"]]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        at(&report, "/bindings/success/ops/0")["calls"][0]["signature_source"],
        "accepted_graph"
    );
    assert_eq!(
        at(&report, "/bindings/success/term/1")["expression_types"],
        "connections_checked"
    );
}

#[test]
fn unknown_local_results_mask_outer_parameters_and_forward_names() {
    let fixture = Fixture::new();
    let mut request = predicate_request(json!(false));
    request["bindings"]["returns"] = json!("Option<bool>");
    request["bindings"]["success"] =
        json!({"ops":[["x","cell_get",["future_expression"]]],"term":["return",["some","x"]]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        at(&report, "/bindings/success/term/1")["deferred_expressions"],
        json!(["/bindings/success/term/1/1"])
    );
    request["bindings"]["success"]["ops"] = json!([["first", "not", "x"], ["x", "not", "ready"]]);
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(error.detail().contains("/bindings/success/ops/0/2"));
    assert!(error.detail().contains("/bindings/success/ops/1"));
    assert!(error.detail().contains("before its definition"));
}

#[test]
fn case_names_do_not_leak_to_sibling_or_join_and_duplicate_locals_refuse() {
    let fixture = Fixture::new();
    for pointer in ["/bindings/cases/1/values/0", "/bindings/join/term/1"] {
        let mut request = branch_request();
        *request.pointer_mut(pointer).unwrap() = json!("negated");
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert!(error.detail().contains(pointer), "{error}");
    }
    for ops in [
        json!([["x", "not", true]]),
        json!([["out", "not", true], ["out", "not", false]]),
    ] {
        let mut request = branch_request();
        request["bindings"]["cases"][0]["ops"] = ops;
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert!(error.detail().contains("duplicates"), "{error}");
        assert!(error.detail().contains("/bindings/cases/0/"), "{error}");
    }
}

#[test]
fn constructor_payloads_and_annotated_wrappers_keep_exact_types() {
    let fixture = Fixture::new();
    for (returns, expression) in [
        ("Option<i8>", json!(["none"])),
        ("Option<i8>", json!(["some", "x"])),
        ("Result<i8,bool>", json!(["ok", "x"])),
        ("Result<i8,bool>", json!(["err", "ready"])),
    ] {
        let mut request = predicate_request(json!(false));
        request["bindings"]["guards"] = json!([]);
        request["bindings"]["returns"] = json!(returns);
        request["bindings"]["success"] = json!({"ops":[{"name":"wrapped","op":expression[0],"args":expression.as_array().unwrap()[1..],"type":returns}],"term":["return","wrapped"]});
        let report = check(&fixture, &request, &json!({})).unwrap();
        assert_eq!(
            at(&report, "/bindings/success/ops/0")["expression_types"],
            "connections_checked"
        );
        assert_eq!(
            at(&report, "/bindings/success/term/1")["expression_types"],
            "connections_checked"
        );
    }
    for expression in [
        json!(["none", 1]),
        json!(["some"]),
        json!(["some", 128]),
        json!(["err", "x"]),
    ] {
        let mut request = predicate_request(json!(false));
        request["bindings"]["success"]["term"] = json!(["return", expression]);
        assert_eq!(
            check(&fixture, &request, &json!({})).unwrap_err().code(),
            AgentErrorCode::ResidualConstraintConflict
        );
    }
}

#[test]
fn local_failure_payload_connections_are_checked_and_execute() {
    let fixture = Fixture::new();
    let mut declarations = interface_call_tests::helpers();
    declarations["types"] = json!([{"name":"Error","variant":[["Value","i8"]]}]);
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let mut request = predicate_request(json!(false));
    request["base"] = json!("d1@r1");
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("Result<i8,Error>");
    request["bindings"]["success"] =
        json!({"ops":[["out","call","echo","x"]],"term":["fail","Value","out"]});
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(
        at(&report, "/bindings/success/term")["payload_value"],
        "checked_operation_result"
    );
    assert_eq!(
        at(&report, "/bindings/success/term")["payload_interface"]["expression_types"],
        "connections_checked"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "7", "false", "--on", "c2"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], json!({"Err":{"Value":7}}));
}

#[test]
fn checked_annotation_is_not_mistaken_for_the_unwrapped_result() {
    let fixture = Fixture::new();
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
    request["bindings"]["success"] = json!({"ops":[{"name":"out","op":"neg?","args":["x"],"type":"Result<i8,ArithmeticError>"}],"term":["ok","out"]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        at(&report, "/bindings/success/ops/0")["expression_types"],
        "connections_checked"
    );
    assert_eq!(
        at(&report, "/bindings/success/term/1")["expression_types"],
        "connections_checked"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "7", "false", "--on", "c1"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], json!({"Ok":-7}));
}

#[test]
fn oversized_authored_regions_refuse_under_the_existing_bound() {
    let fixture = Fixture::new();
    let mut request = predicate_request(json!(false));
    request["bindings"]["success"]["ops"] = json!(
        (0..1025)
            .map(|index| json!([format!("v{index}"), "not", true]))
            .collect::<Vec<_>>()
    );
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("/bindings/success/ops"), "{error}");
    assert!(!fixture.dir.join(".sley").exists());
}
