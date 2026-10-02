use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn declarations() -> Value {
    json!({"af1":1,"types":[
        {"name":"Errors","variant":["Stop",["Math","ArithmeticError"]]},
        {"name":"Record","record":[["Math","ArithmeticError"]]}
    ]})
}

fn request(ops: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["returns"] = json!("Result<i8,Errors>");
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["success"] = json!({"ops":[],"term":["ok","x"]});
    request["bindings"]["success"]["ops"] = ops;
    request
}

fn operation(report: &Value) -> &Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["at"] == "/bindings/success/ops/0")
        .unwrap()
}

#[test]
fn missing_cases_and_nonvariant_returns_refuse_without_candidate_publication() {
    for accepted in [false, true] {
        let fixture = Fixture::new();
        let declarations = declarations();
        let (code, result) = cli(
            &fixture.dir,
            &["try", &declarations.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        if accepted {
            let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
            assert_eq!(code, 0, "{result}");
        }
        for returns in [
            "Result<i8,Errors>",
            "Result<i8,Record>",
            "Result<i8,ArithmeticError>",
            "Option<i8>",
            "i8",
        ] {
            for ops in [
                json!([["!missing", "if", false]]),
                json!([["out", "add?missing", "x", 1]]),
            ] {
                let mut request = request(ops);
                request["bindings"]["returns"] = json!(returns);
                if !accepted {
                    request["base"] = json!("d1@r1");
                }
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
                let detail = result["detail"].as_str().unwrap();
                assert!(
                    detail.contains("failure route `missing`")
                        && detail.contains("/bindings/returns")
                        && detail.contains("/bindings/success/ops/0"),
                    "{result}"
                );
                assert_eq!(bytes(&request), before);
            }
        }
        for path in [
            ".sley/candidates/c2.hex",
            ".sley/candidates/c2.json",
            ".sley/drafts/d2",
            ".sley/drafts/d1/r2",
            ".sley/residual",
        ] {
            assert!(!fixture.dir.join(path).exists(), "{path}");
        }
    }
}

#[test]
fn unknown_source_type_cannot_hide_a_missing_named_destination() {
    let fixture = Fixture::new();
    for expression in [
        json!(["cell_get?missing", ["future_expression"]]),
        json!(["add?missing", 1, 2]),
        json!(["call?missing", "untyped"]),
    ] {
        let mut declarations = declarations();
        declarations["fns"] = json!([{"fn":"untyped","params":[]}]);
        let mut request = request(json!([]));
        request["bindings"]["guards"] =
            json!([{"when":["eq",expression,"x"],"fail":["fail","Stop"]}]);
        let error = check(&fixture, &request, &declarations).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/bindings/guards/0/when/1"),
            "{error}"
        );
        assert!(
            error.detail().contains("failure route `missing`"),
            "{error}"
        );
    }
}

#[test]
fn incomplete_error_definitions_refuse_without_inventing_unit_cases() {
    let fixture = Fixture::new();
    for variant in [json!(["Stop", ["Math"]]), json!(["Stop", "Stop"])] {
        let declarations = json!({"types":[{"name":"Errors","variant":variant}]});
        for ops in [
            json!([["!Math", "if", "ready"]]),
            json!([["out", "add?Math", "x", 1]]),
        ] {
            let request = request(ops);
            let error = check(&fixture, &request, &declarations).unwrap_err();
            super::interface_closure_tests::assert_incomplete(&error, "/bindings/returns");
        }
    }
}

#[test]
fn draft_replacement_controls_case_presence_and_payload_connections() {
    let fixture = Fixture::new();
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations().to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    let request = request(json!([["out", "add?Math", "x", 1]]));
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        operation(&report)["propagation"][0]["failure_route"],
        "preserve_in_named_case"
    );
    let replacement = json!({"types":[{"name":"Errors","variant":["Stop"]}]});
    assert!(
        check(&fixture, &request, &replacement)
            .unwrap_err()
            .detail()
            .contains("failure route `Math`")
    );
    let replacement = json!({"types":[{"name":"Errors","variant":[["Math","i8"]]}]});
    assert!(
        check(&fixture, &request, &replacement)
            .unwrap_err()
            .detail()
            .contains("checked failure payload conflicts")
    );
    let replacement = json!({"types":[{"name":"Errors","variant":["Math"]}]});
    let report = check(&fixture, &request, &replacement).unwrap();
    assert_eq!(
        operation(&report)["propagation"][0]["failure_route"],
        "drop_into_unit_case"
    );
}

#[test]
fn valid_named_failure_routes_keep_payload_or_explicitly_drop_it_in_the_vm() {
    for accepted in [false, true] {
        for (route, failure) in [
            (
                "Math",
                json!({"Err":{"Math":{"ArithmeticError":"Overflow"}}}),
            ),
            ("Stop", json!({"Err":"Stop"})),
        ] {
            let fixture = Fixture::new();
            let (code, result) = cli(
                &fixture.dir,
                &["try", &declarations().to_string(), "--no-test"],
            );
            assert_eq!(code, 0, "{result}");
            if accepted {
                let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
                assert_eq!(code, 0, "{result}");
            }
            let mut request = request(json!([["out", format!("add?{route}"), "x", 1]]));
            request["bindings"]["success"]["term"] = json!(["ok", "out"]);
            if !accepted {
                request["base"] = json!("d1@r1");
            }
            let before = bytes(&request);
            let (code, result) = cli(
                &fixture.dir,
                &["residual", "try", &request.to_string(), "--no-test"],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["kernel"], "valid");
            for (input, expected) in [("127", failure), ("3", json!({"Ok":4}))] {
                let (code, result) = cli(
                    &fixture.dir,
                    &["call", "checked", input, "false", "--on", "c2"],
                );
                assert_eq!(code, 0, "{result}");
                assert_eq!(result["result"], expected);
            }
            assert_eq!(bytes(&request), before);
        }
    }
}

#[test]
fn missing_failure_route_refuses_a_closed_relation_before_question_publication() {
    let fixture = Fixture::new();
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations().to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let mut request = request(json!([["out", "add?missing", "x", 1]]));
    request["base"] = json!("d1@r1");
    request["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("returns");
    request["choices"] = json!({"version":1,"contract":"author_closed_relation","rows":[
        {"/bindings/returns":"Result<i8,Errors>"},
        {"/bindings/returns":"Result<i8,Record>"}
    ]});
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(
        result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
        "{result}"
    );
    assert!(
        result["detail"]
            .as_str()
            .unwrap()
            .contains("failure route `missing`"),
        "{result}"
    );
    for path in [
        ".sley/residual",
        ".sley/candidates/c2.hex",
        ".sley/drafts/d2",
    ] {
        assert!(!fixture.dir.join(path).exists(), "{path}");
    }
}
