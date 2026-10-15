use super::interface_tests::{check, predicate_request};
use super::{Fixture, branch_request, cli, expand};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn request(operand: &Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("bool");
    request["bindings"]["success"] = json!({
        "ops":[["out","not","ready"]], "term":["return",operand]});
    request
}

#[test]
fn operation_result_indexes_refuse_with_use_and_definition_before_publication() {
    for suffix in ["#1", "#0007", "#4294967295"] {
        for object in [false, true] {
            let fixture = Fixture::new();
            let mut request = request(&json!(format!("out{suffix}")));
            if object {
                request["bindings"]["success"]["ops"][0] =
                    json!({"name":"out","opcode":"bool_not","operands":["ready"],"type":"bool"});
            }
            let before = request.clone();
            let error = check(&fixture, &request, &json!({})).unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
            for text in [
                "/bindings/success/term/1",
                "/bindings/success/ops/0",
                "only result 0",
            ] {
                assert!(error.detail().contains(text), "{error}");
            }
            let (code, result) = cli(
                &fixture.dir,
                &["residual", "try", &request.to_string(), "--no-test"],
            );
            assert_eq!(code, 2, "{result}");
            assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
            assert_eq!(request, before);
            for folder in ["residual", "drafts", "candidates"] {
                assert!(!fixture.dir.join(".sley").join(folder).exists());
            }
        }
    }
}

#[test]
fn unresolved_operation_types_cannot_hide_nonexistent_results() {
    for operation in [
        json!(["out", "const", 7]),
        json!(["out", "some", 7]),
        json!({"name":"out","opcode":"constant_ref","operands":[7]}),
    ] {
        let fixture = Fixture::new();
        let mut request = request(&json!("out#1"));
        request["bindings"]["returns"] = json!("Option<i64>");
        request["bindings"]["success"]["ops"] = json!([operation]);
        let before = request.clone();
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        for text in [
            "/bindings/success/term/1",
            "/bindings/success/ops/0",
            "only result 0",
        ] {
            assert!(error.detail().contains(text), "{error}");
        }
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        assert_eq!(request, before);
        for folder in ["residual", "drafts", "candidates"] {
            assert!(!fixture.dir.join(".sley").join(folder).exists());
        }
        let ordinary = Fixture::new();
        let (code, result) = cli(
            &ordinary.dir,
            &["try", &expand(&request).frame.to_string(), "--no-test"],
        );
        assert_ne!(code, 0, "{result}");
    }
}

#[test]
fn unresolved_result_cardinality_is_checked_in_operations_and_branch_edges() {
    let fixture = Fixture::new();
    let mut nested = request(&json!(["some", "ret"]));
    nested["bindings"]["returns"] = json!("Option<i64>");
    nested["bindings"]["success"]["ops"] = json!([
        ["out", "const", 7],
        ["ret", "tuple_get", 0, ["tuple", "out#1"]]
    ]);
    let mut branch = branch_request();
    branch["bindings"]["cases"][0]["ops"] = json!([["out", "const", 7]]);
    branch["bindings"]["cases"][0]["values"] = json!(["out#1"]);
    for (request, use_at, definition) in [
        (
            nested,
            "/bindings/success/ops/1/3/1",
            "/bindings/success/ops/0",
        ),
        (
            branch,
            "/bindings/cases/0/values/0",
            "/bindings/cases/0/ops/0",
        ),
    ] {
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        for text in [use_at, definition, "only result 0"] {
            assert!(error.detail().contains(text), "{error}");
        }
    }
}

#[test]
fn unresolved_zero_indexes_preserve_ordinary_literal_obligations_and_execution() {
    for suffix in ["", "#0", "#0000"] {
        for some in [false, true] {
            let name = format!("out{suffix}");
            let mut request = request(&json!(name));
            request["bindings"]["returns"] = json!("Option<i64>");
            request["bindings"]["success"]["ops"] =
                json!([["out", if some { "some" } else { "const" }, 7]]);
            if !some {
                request["bindings"]["success"]["term"] = json!(["return", ["some", name]]);
            }
            for residual in [false, true] {
                let fixture = Fixture::new();
                check(&fixture, &request, &json!({})).unwrap();
                let text = if residual {
                    request.clone()
                } else {
                    expand(&request).frame
                }
                .to_string();
                let args = if residual {
                    vec!["residual", "try", &text, "--no-test"]
                } else {
                    vec!["try", &text, "--no-test"]
                };
                let (code, result) = cli(&fixture.dir, &args);
                if some {
                    // AF1-X needs an explicit type for this standalone Some
                    // literal. A valid zero index must retain that obligation,
                    // rather than acquiring a type from the preflight.
                    assert_eq!(code, 2, "{result}");
                    if residual {
                        assert_eq!(
                            result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
                            "{result}"
                        );
                        assert!(
                            result["detail"]
                                .as_str()
                                .unwrap()
                                .contains("no type context"),
                            "{result}"
                        );
                        assert!(!fixture.dir.join(".sley/drafts/d1").exists());
                        request["bindings"]["success"]["ops"][0][2] =
                            json!({"type":"i64","value":7});
                        let (code, result) = cli(
                            &fixture.dir,
                            &["residual", "try", &request.to_string(), "--no-test"],
                        );
                        assert_eq!(code, 0, "{result}");
                    } else {
                        assert_eq!(result["error"], "AGENT_FRAME_INVALID");
                        assert!(
                            result["detail"]
                                .as_str()
                                .unwrap()
                                .contains("fixes the type")
                        );
                        assert!(!fixture.dir.join(".sley/candidates/c1.hex").exists());
                        let delta = json!({"set":[{"at":"/fns/0/blocks/0/ops/0/2",
                        "value":{"type":"i64","value":7}}]})
                        .to_string();
                        let (code, result) = cli(
                            &fixture.dir,
                            &["fill", "d1", &delta, "--revision", "1", "--no-test"],
                        );
                        assert_eq!(code, 0, "{result}");
                    }
                } else {
                    assert_eq!(code, 0, "{result}");
                    if residual {
                        assert_eq!(result["kernel"], "valid", "{result}");
                    }
                }
                let (code, result) = cli(
                    &fixture.dir,
                    &["call", "checked", "7", "true", "--on", "c1"],
                );
                assert_eq!(code, 0, "{result}");
                assert_eq!(result["result"], json!({"Some":7}));
            }
        }
    }
}

#[test]
fn ordinary_expansion_rejects_nonexistent_results_and_zero_indexes_execute() {
    for suffix in ["", "#0", "#0000", "#1", "#4294967295"] {
        let fixture = Fixture::new();
        let request = request(&json!(format!("out{suffix}")));
        let frame = expand(&request).frame;
        let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
        if matches!(suffix, "#1" | "#4294967295") {
            assert_ne!(code, 0, "{result}");
            assert!(check(&fixture, &request, &json!({})).is_err());
        } else {
            assert_eq!(code, 0, "{result}");
            let report = check(&fixture, &request, &json!({})).unwrap();
            assert_eq!(report["composition"], "partial");
            for (ready, expected) in [("true", false), ("false", true)] {
                let (code, result) =
                    cli(&fixture.dir, &["call", "checked", "7", ready, "--on", "c1"]);
                assert_eq!(code, 0, "{result}");
                assert_eq!(result["result"], expected);
            }
        }
    }
}

#[test]
fn nested_operands_and_tuple_results_cannot_select_a_nonexistent_result() {
    for operation in [
        json!(["out", "not", "ready"]),
        json!(["out", "tuple", "ready", "ready"]),
    ] {
        let fixture = Fixture::new();
        let mut request = request(&json!(["and", "out#1", true]));
        request["bindings"]["success"]["ops"][0] = operation;
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert!(
            error.detail().contains("/bindings/success/term/1/1"),
            "{error}"
        );
        assert!(error.detail().contains("only result 0"), "{error}");
    }
}

#[test]
fn branch_case_edges_and_join_operations_check_result_indexes() {
    for case_edge in [false, true] {
        let fixture = Fixture::new();
        let mut request = branch_request();
        request["bindings"]["returns"] = json!("bool");
        request["bindings"]["cases"][0]["ops"] = json!([["flag", "lt", "x", 0]]);
        request["bindings"]["cases"][0]["values"] =
            json!([if case_edge { "flag#1" } else { "flag" }]);
        request["bindings"]["cases"][1]["values"] = json!([false]);
        request["bindings"]["join"] = json!({"params":[["flag","bool"]],
            "ops":[["inverted","not","flag"]],"term":["return","inverted#1"]});
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        let (use_at, definition) = if case_edge {
            ("/bindings/cases/0/values/0", "/bindings/cases/0/ops/0")
        } else {
            ("/bindings/join/term/1", "/bindings/join/ops/0")
        };
        assert!(error.detail().contains(use_at), "{error}");
        assert!(error.detail().contains(definition), "{error}");
    }
}

#[test]
fn invalid_operation_indexes_refuse_whole_relations_without_pruning() {
    for version in [1, 2] {
        let fixture = Fixture::new();
        let mut request = request(&json!(true));
        request["bindings"]["success"]
            .as_object_mut()
            .unwrap()
            .remove("term");
        let field = "/bindings/success/term";
        let rows = json!([{field:["return","out#0"]},{field:["return","out#1"]}]);
        request["choices"] = if version == 1 {
            json!({"version":1,"contract":"author_closed_relation","rows":rows})
        } else {
            json!({"version":2,"contract":"author_closed_constraints","dependencies":[],
                "constraints":[{"fields":[field],"rows":rows}]})
        };
        let before = request.clone();
        let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        assert!(result.to_string().contains("only result 0"), "{result}");
        assert_eq!(request, before);
        for folder in ["residual", "drafts", "candidates"] {
            assert!(!fixture.dir.join(".sley").join(folder).exists());
        }
    }
}
