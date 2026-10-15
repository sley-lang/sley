use super::interface_tests::{check, predicate_request};
use super::{Fixture, branch_request, bytes, cli, expand};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn connection<'a>(report: &'a Value, at: &str) -> &'a Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == at)
        .unwrap()
}

#[test]
fn inaccessible_qualified_and_special_operands_refuse_before_fragment_publication() {
    for operand in [
        "source.entry.value",
        "gw_b0.x",
        "$",
        "#12345678",
        "x#bad",
        "x#4294967296",
    ] {
        let fixture = Fixture::new();
        let request = predicate_request(json!(operand));
        let before = bytes(&request);
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/bindings/guards/0/when"),
            "{error}"
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        for path in ["drafts", "candidates", "residual"] {
            assert!(!fixture.dir.join(".sley").join(path).exists());
        }
        assert_eq!(bytes(&request), before);
        // Exercise ordinary AF1-X without the fragment preflight as well.
        let ordinary = Fixture::new();
        let frame = expand(&request).frame;
        let (code, result) = cli(&ordinary.dir, &["try", &frame.to_string(), "--no-test"]);
        assert_ne!(code, 0, "{operand}: {result}");
        assert_ne!(result["kernel"], "valid", "{operand}: {result}");
    }
}

#[test]
fn local_parameter_and_region_result_bindings_preserve_zero_indexes_and_execution() {
    for suffix in ["", "#0", "#00"] {
        let fixture = Fixture::new();
        let mut request = predicate_request(json!(false));
        request["bindings"]["guards"] = json!([]);
        request["bindings"]["returns"] = json!("Tuple<i8,bool>");
        request["bindings"]["success"] = json!({"ops":[["v","not",format!("ready{suffix}")]],
            "term":["return",["tuple",format!("x{suffix}"),format!("v{suffix}")]]});
        let report = check(&fixture, &request, &json!({})).unwrap();
        let bindings = &connection(&report, "/bindings/success/term/1")["value_bindings"];
        assert_eq!(bindings.as_array().unwrap().len(), 2);
        assert_eq!(bindings[0]["definition"], "/bindings/params/0");
        assert_eq!(bindings[1]["definition"], "/bindings/success/ops/0");
        assert_eq!(bindings[1]["scope"], "current_region");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{suffix}: {result}");
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", "7", "true", "--on", "c1"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!([7, false]));
    }
}

#[test]
fn branch_values_cannot_bypass_region_ownership_with_generated_block_names() {
    for path in ["/bindings/cases/1/values/0", "/bindings/join/term/1"] {
        let fixture = Fixture::new();
        let mut request = branch_request();
        *request.pointer_mut(path).unwrap() = json!("gw_b2.negated");
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains(path), "{error}");
        assert!(
            error.detail().contains("generated blocks are private"),
            "{error}"
        );
        let frame = expand(&request).frame;
        assert!(
            frame["fns"][0]["blocks"]
                .as_array()
                .unwrap()
                .iter()
                .all(|block| block["name"] != "gw_b2")
        );
    }
}

#[test]
fn unknown_local_types_do_not_inherit_outer_bindings_or_result_zero() {
    let fixture = Fixture::new();
    for (ops, operand) in [
        (json!([["x", "future_operation"]]), "x"),
        (json!([["v", "future_operation"]]), "v#1"),
    ] {
        let mut request = predicate_request(json!(false));
        request["bindings"]["guards"] = json!([]);
        request["bindings"]["returns"] = json!("bool");
        request["bindings"]["success"] = json!({"ops":ops,"term":["return",operand]});
        let report = check(&fixture, &request, &json!({})).unwrap();
        let entry = connection(&report, "/bindings/success/term/1");
        assert_eq!(entry["expression_types"], "partial");
        assert_eq!(entry["value_bindings"][0]["type"], "deferred");
        assert_eq!(entry["value_bindings"][0]["scope"], "current_region");
        assert_eq!(report["composition"], "partial");
    }
}

#[test]
fn qualified_text_literals_keep_their_bytes_and_do_not_become_operand_references() {
    let fixture = Fixture::new();
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("text");
    request["bindings"]["success"] =
        json!({"ops":[],"term":["return",{"type":"text","value":"gw_b0.x#$"}]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/term/1")["value_bindings"],
        json!([])
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "7", "true", "--on", "c1"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], "gw_b0.x#$");
}

#[test]
fn constrained_derivations_refuse_inaccessible_value_domains_without_filtering_rows() {
    let fixture = Fixture::new();
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"][0]
        .as_object_mut()
        .unwrap()
        .remove("when");
    request["choices"] = json!({"version":2,"contract":"author_closed_constraints","dependencies":[],
        "constraints":[{"fields":["/bindings/guards/0/when"],"rows":[
            {"/bindings/guards/0/when":false},{"/bindings/guards/0/when":"external.value"}]}]});
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    assert!(!fixture.dir.join(".sley/residual").exists());
}
