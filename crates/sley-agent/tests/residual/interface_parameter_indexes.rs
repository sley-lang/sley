use super::interface_tests::{check, predicate_request};
use super::{Fixture, branch_request, cli, expand, planning_request};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn returned(operand: Value, ty: &str) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!(ty);
    request["bindings"]["success"] = json!({"ops":[],"term":["return",null]});
    request["bindings"]["success"]["term"][1] = operand;
    request
}

#[test]
fn indexed_parameters_keep_their_declared_type_and_match_ordinary_execution() {
    for suffix in ["#1", "#0007", "#4294967295"] {
        let fixture = Fixture::new();
        let request = returned(json!(format!("x{suffix}")), "i8");
        // Establish behavior through the unchanged authoring/compiler/VM route.
        let ordinary = Fixture::new();
        let frame = expand(&request).frame;
        let (code, result) = cli(&ordinary.dir, &["try", &frame.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(
            &ordinary.dir,
            &["call", "checked", "7", "true", "--on", "c1"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!(7));
        let report = check(&fixture, &request, &json!({})).unwrap();
        let connection = report["connections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["at"] == "/bindings/success/term/1")
            .unwrap();
        assert_eq!(connection["expression_types"], "connections_checked");
        assert_eq!(connection["value_bindings"][0]["type"], "i8");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
    }
}

#[test]
fn incompatible_indexed_parameters_refuse_before_publication() {
    for request in [
        returned(json!("x#1"), "bool"),
        predicate_request(json!("x#1")),
        returned(json!(["and", "ready#7", "x#9"]), "bool"),
    ] {
        let fixture = Fixture::new();
        let before = request.clone();
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("/bindings/params/0"), "{error}");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        for folder in ["residual", "drafts", "candidates"] {
            assert!(!fixture.dir.join(".sley").join(folder).exists());
        }
        assert_eq!(request, before);
        let ordinary = Fixture::new();
        let frame = expand(&request).frame;
        let (code, result) = cli(&ordinary.dir, &["try", &frame.to_string(), "--no-test"]);
        assert_ne!(code, 0, "{result}");
    }
}

#[test]
fn indexed_branch_parameters_keep_local_types_and_binding_ownership() {
    let fixture = Fixture::new();
    let mut request = branch_request();
    let case_name = request["bindings"]["cases"][0]["payload"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let join_name = request["bindings"]["join"]["params"][0][0]
        .as_str()
        .unwrap()
        .to_owned();
    request["bindings"]["cases"][0]["ops"] = json!([]);
    request["bindings"]["cases"][0]["values"][0] = json!(format!("{case_name}#9"));
    request["bindings"]["returns"] = json!("i8");
    request["bindings"]["join"]["term"] = json!(["return", format!("{join_name}#3")]);
    let report = check(&fixture, &request, &json!({})).unwrap();
    for path in ["/bindings/cases/0/values/0", "/bindings/join/term/1"] {
        let row = report["connections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["at"] == path)
            .unwrap();
        assert_eq!(row["expression_types"], "connections_checked");
        assert_eq!(row["value_bindings"][0]["binding_kind"], "parameter");
    }
    let ordinary = Fixture::new();
    let frame = expand(&request).frame;
    let (code, result) = cli(&ordinary.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    for workspace in [&ordinary.dir, &fixture.dir] {
        for (input, expected) in [(json!({"Some":7}), json!(7)), (json!("None"), json!(17))] {
            let (code, result) = cli(
                workspace,
                &["call", "checked", &input.to_string(), "--on", "c1"],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], expected);
        }
    }
}

#[test]
fn indexed_pipeline_parameters_keep_type_constraints_and_ordinary_outcomes() {
    for suffix in ["#0", "#00", "#1", "#0007", "#4294967295"] {
        let fixture = Fixture::new();
        let mut request = planning_request();
        request["bindings"]["steps"][0][1] =
            json!(["mul",{"type":"i64","value":3},format!("x{suffix}")]);
        let ordinary = Fixture::new();
        let frame = expand(&request).frame;
        let (ordinary_code, ordinary_result) =
            cli(&ordinary.dir, &["try", &frame.to_string(), "--no-test"]);
        assert_eq!(ordinary_code, 0, "{ordinary_result}");
        let report = check(&fixture, &request, &json!({})).unwrap();
        assert!(
            report["connections"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["pipeline_integer_connections"] == "checked")
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(
            code, ordinary_code,
            "{suffix}: {result}; ordinary: {ordinary_result}"
        );
        for workspace in [&ordinary.dir, &fixture.dir] {
            let (code, result) = cli(workspace, &["call", "scaled_half", "8", "--on", "c1"]);
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], json!({"Ok":12}));
        }
    }
}

#[test]
fn indexed_parameter_type_checks_allow_ordinary_wrapper_inference() {
    let fixture = Fixture::new();
    let mut request = returned(json!("x#1"), "Result<i8,ArithmeticError>");
    request["bindings"]["success"]["term"] = json!(["ok", "x#1"]);
    let report = check(&fixture, &request, &json!({})).unwrap();
    // Interface analysis is still partial; successful ordinary compilation
    // and kernel validation establish the completed program's validity.
    assert_eq!(report["composition"], "partial");
    let frame = expand(&request).frame;
    let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let residual = Fixture::new();
    let (code, result) = cli(
        &residual.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid", "{result}");
    for workspace in [&fixture.dir, &residual.dir] {
        for input in ["-128", "7"] {
            let (code, result) = cli(workspace, &["call", "checked", input, "true", "--on", "c1"]);
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], json!({"Ok":input.parse::<i8>().unwrap()}));
        }
    }
}

#[test]
fn indexed_parameter_conflicts_refuse_complete_relations_without_pruning() {
    for version in [1, 2] {
        let fixture = Fixture::new();
        let mut request = predicate_request(json!(false));
        request["bindings"]["guards"][0]
            .as_object_mut()
            .unwrap()
            .remove("when");
        let rows = json!([
            {"/bindings/guards/0/when":false},
            {"/bindings/guards/0/when":"x#1"}
        ]);
        request["choices"] = if version == 1 {
            json!({"version":1,"contract":"author_closed_relation","rows":rows})
        } else {
            json!({"version":2,"contract":"author_closed_constraints","dependencies":[],
                "constraints":[{"fields":["/bindings/guards/0/when"],"rows":rows}]})
        };
        let before = request.clone();
        let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        assert!(
            result.to_string().contains("/bindings/params/0"),
            "{result}"
        );
        assert_eq!(request, before);
        for folder in ["residual", "drafts", "candidates"] {
            assert!(!fixture.dir.join(".sley").join(folder).exists());
        }
    }
}

#[test]
fn indexed_pipeline_parameters_cannot_hide_width_or_shadowing_conflicts() {
    for (operand, ty, step_name, source) in [
        ("x#1", "bool", "scaled", "/bindings/params/0"),
        ("x#7", "i8", "scaled", "/bindings/params/0"),
        ("x#1", "i64", "x", "/bindings/steps/0"),
        ("x#bad", "i64", "scaled", "/bindings/steps/0/1/1"),
        ("x#4294967296", "i64", "scaled", "/bindings/steps/0/1/1"),
    ] {
        let fixture = Fixture::new();
        let mut request = planning_request();
        request["bindings"]["params"][0][1] = json!(ty);
        request["bindings"]["steps"][0] =
            json!([step_name,["mul",operand,{"type":"i64","value":3}]]);
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains(source), "{error}");
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
}
