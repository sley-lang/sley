use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli, expand};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn request(suffix: &str, comparison: &str) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["params"] = json!([["x", "i8"], ["y", "i8"]]);
    request["bindings"]["returns"] = json!("Result<bool,ArithmeticError>");
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["success"] = json!({
        "ops":[["out","add?","x","y"],
               {"name":"same","op":comparison,"args":[format!("out{suffix}"),"x"],"type":"bool"}],
        "term":["ok","same"]});
    request
}

fn connection<'a>(report: &'a Value, at: &str) -> &'a Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["at"] == at)
        .unwrap()
}

#[test]
fn checked_continuation_indexes_keep_payload_types_and_ordinary_execution() {
    for suffix in ["#1", "#0007", "#4294967295"] {
        let request = request(suffix, "eq");
        let ordinary = Fixture::new();
        let frame = expand(&request).frame;
        let (code, result) = cli(&ordinary.dir, &["try", &frame.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        for (x, y, expected) in [
            (7, 0, json!({"Ok":true})),
            (7, 1, json!({"Ok":false})),
            (127, 1, json!({"Err":{"ArithmeticError":"Overflow"}})),
        ] {
            let (code, result) = cli(
                &ordinary.dir,
                &[
                    "call",
                    "checked",
                    &x.to_string(),
                    &y.to_string(),
                    "--on",
                    "c1",
                ],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], expected);
        }
        let fixture = Fixture::new();
        let before = request.clone();
        let report = check(&fixture, &request, &json!({})).unwrap();
        let comparison = connection(&report, "/bindings/success/ops/1");
        assert_eq!(
            comparison["expression_types"], "connections_checked",
            "{comparison}"
        );
        assert_eq!(
            comparison["value_bindings"][0]["binding_kind"],
            "continuation_parameter"
        );
        assert_eq!(comparison["value_bindings"][0]["type"], "i8");
        assert_eq!(request, before);
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(&fixture.dir, &["call", "checked", "7", "1", "--on", "c1"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!({"Ok":false}));
        let (code, result) = cli(&fixture.dir, &["call", "checked", "127", "1", "--on", "c1"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(
            result["result"],
            json!({"Err":{"ArithmeticError":"Overflow"}})
        );
    }
}

#[test]
fn checked_call_continuation_indexes_preserve_option_payloads_and_none_propagation() {
    for opcode in ["call?", "call_direct?", "112?"] {
        let mut request = predicate_request(json!(false));
        request["bindings"]["params"] = json!([["item", "Option<i8>"]]);
        request["bindings"]["returns"] = json!("Option<i8>");
        request["bindings"]["guards"] = json!([]);
        request["bindings"]["success"] = json!({"ops":[
            {"name":"out","opcode":opcode,"operands":["fetch","item"]},
            {"name":"ret","op":"some","args":["out#7"],"type":"Option<i8>"}
        ],"term":["return","ret"]});
        let before = request.clone();
        for residual in [false, true] {
            let fixture = super::interface_call_tests::fixture(true);
            let report = check(&fixture, &request, &json!({})).unwrap();
            let row = connection(&report, "/bindings/success/ops/1");
            assert_eq!(row["expression_types"], "connections_checked", "{row}");
            assert_eq!(
                row["value_bindings"][0]["binding_kind"],
                "continuation_parameter"
            );
            assert_eq!(row["value_bindings"][0]["type"], "i8");
            let authored = if residual {
                request.clone()
            } else {
                expand(&request).frame
            };
            let authored = authored.to_string();
            let args = if residual {
                vec!["residual", "try", &authored, "--no-test"]
            } else {
                vec!["try", &authored, "--no-test"]
            };
            let (code, result) = cli(&fixture.dir, &args);
            assert_eq!(code, 0, "{opcode}: {result}");
            for input in [json!({"Some":7}), json!("None")] {
                let (code, result) = cli(
                    &fixture.dir,
                    &["call", "checked", &input.to_string(), "--on", "c2"],
                );
                assert_eq!(code, 0, "{opcode}: {result}");
                assert_eq!(result["result"], input);
            }
        }
        assert_eq!(request, before);
    }
}

#[test]
fn unknown_checked_payloads_mask_outer_bindings_without_inheriting_wrapper_annotations() {
    let fixture = Fixture::new();
    for suffix in ["", "#0", "#7"] {
        let mut request = request(suffix, "eq");
        request["bindings"]["params"] = json!([["x", "i8"], ["out", "bool"]]);
        request["bindings"]["success"]["ops"][0] = json!({
            "name":"out","op":"future_operation?","args":[],"type":"Result<i8,ArithmeticError>"});
        let before = request.clone();
        let report = check(&fixture, &request, &json!({})).unwrap();
        let row = connection(&report, "/bindings/success/ops/1");
        assert_eq!(row["expression_types"], "partial", "{row}");
        assert_eq!(row["value_bindings"][0]["type"], "deferred");
        assert_eq!(report["composition"], "partial");
        assert_eq!(request, before);
    }
}

#[test]
fn invalid_indexed_continuations_refuse_whole_relations_without_pruning() {
    for version in [1, 2] {
        let fixture = Fixture::new();
        let mut request = request("#1", "eq");
        request["bindings"]["success"]
            .as_object_mut()
            .unwrap()
            .remove("term");
        let field = "/bindings/success/term";
        let rows = json!([{field:["ok",true]}, {field:["ok",["and","out#1",true]]}]);
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
        assert!(
            result.to_string().contains("/bindings/success/ops/0"),
            "{result}"
        );
        assert_eq!(request, before);
        for folder in ["residual", "drafts", "candidates"] {
            assert!(!fixture.dir.join(".sley").join(folder).exists());
        }
    }
}

#[test]
fn continuation_type_evidence_allows_unannotated_ordinary_comparisons() {
    let fixture = Fixture::new();
    let mut request = request("#1", "eq");
    request["bindings"]["success"]["ops"][1]
        .as_object_mut()
        .unwrap()
        .remove("type");
    let before = request.clone();
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/ops/1")["expression_types"],
        "connections_checked"
    );
    assert_eq!(report["composition"], "partial");
    let frame = expand(&request).frame;
    let (ordinary_code, ordinary) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(ordinary_code, 0, "{ordinary}");
    let residual = Fixture::new();
    let (code, result) = cli(
        &residual.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, ordinary_code, "{result}");
    assert_eq!(result["kernel"], "valid", "{result}");
    for workspace in [&fixture.dir, &residual.dir] {
        for (x, y, expected) in [
            ("7", "0", json!({"Ok":true})),
            ("7", "1", json!({"Ok":false})),
            ("127", "1", json!({"Err":{"ArithmeticError":"Overflow"}})),
        ] {
            let (code, result) = cli(workspace, &["call", "checked", x, y, "--on", "c1"]);
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], expected);
        }
    }
    assert_eq!(request, before);
}

#[test]
fn incompatible_checked_continuation_uses_refuse_before_expansion_publication() {
    let mut request = request("#1", "and");
    request["bindings"]["success"]["ops"][1]["args"][1] = json!(true);
    let fixture = Fixture::new();
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(
        error.detail().contains("/bindings/success/ops/0"),
        "{error}"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    for folder in ["residual", "drafts", "candidates"] {
        assert!(!fixture.dir.join(".sley").join(folder).exists());
    }
}
