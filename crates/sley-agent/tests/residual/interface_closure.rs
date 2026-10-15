use super::interface_tests::{check, predicate_request};
use super::{Fixture, branch_request, bytes, cli, planning_request};
use serde_json::{Value, json};

fn branch() -> Value {
    let mut request = branch_request();
    request["bindings"]["params"] = json!([["maybe", "Input"]]);
    request["bindings"]["cases"] = json!([
        {"case":"Present","payload":["x","i8"],"ops":[],"values":["x"]},
        {"case":"Absent","payload":null,"ops":[],"values":[17]}
    ]);
    request
}

fn at<'a>(report: &'a Value, path: &str) -> &'a Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == path)
        .unwrap()
}

#[test]
fn incomplete_variant_shapes_do_not_establish_branch_coverage_or_unit_payloads() {
    let fixture = Fixture::new();
    for variant in [
        json!(["Absent", ["Present"]]),
        json!(["Absent", "Absent"]),
        json!(["Absent", ["Present", "i8", "extra"]]),
    ] {
        let declarations = json!({"types":[{"name":"Input","variant":variant}]});
        let request = branch();
        let before = bytes(&request);
        let error = check(&fixture, &request, &declarations).unwrap_err();
        assert_incomplete(&error, "/bindings/params/0/1");
        assert_eq!(bytes(&request), before);
        // Definition uncertainty does not hide an independently known bad join edge.
        let mut bad = request;
        bad["bindings"]["cases"][0]["values"] = json!([true]);
        let error = check(&fixture, &bad, &declarations).unwrap_err();
        assert!(
            error.detail().contains("/bindings/cases/0/values/0"),
            "{error}"
        );
    }
}

#[test]
fn incomplete_named_failures_refuse_destinations_but_still_check_authored_payloads() {
    let fixture = Fixture::new();
    for variant in [json!(["Stop", ["Math"]]), json!(["Stop", "Stop"])] {
        let declarations = json!({"types":[{"name":"Errors","variant":variant}]});
        for term in [json!(["fail", "Math"]), json!(["fail", "Math", "x"])] {
            let mut request = predicate_request(json!(false));
            request["bindings"]["returns"] = json!("Result<i8,Errors>");
            request["bindings"]["guards"] = json!([{ "when":false,"fail":term }]);
            request["bindings"]["success"] = json!({"ops":[],"term":term});
            let error = check(&fixture, &request, &declarations).unwrap_err();
            assert_incomplete(&error, "/bindings/returns");
            request["bindings"]["guards"][0]["fail"] = json!(["fail", "Math", ["not", "x"]]);
            let error = check(&fixture, &request, &declarations).unwrap_err();
            assert!(
                error.detail().contains("/bindings/guards/0/fail/2"),
                "{error}"
            );
        }
        let mut pipeline = planning_request();
        pipeline["bindings"]["returns"] = json!("Result<i64,Errors>");
        pipeline["bindings"]["arithmetic_failure"] = json!("Math");
        let error = check(&fixture, &pipeline, &declarations).unwrap_err();
        assert_incomplete(&error, "/bindings/returns");
    }
}

#[test]
fn partial_draft_replacement_cannot_inherit_accepted_variant_closure() {
    let fixture = Fixture::new();
    let complete =
        json!({"af1":1,"types":[{"name":"Input","variant":[["Present","i8"],"Absent"]}]});
    let (code, result) = cli(&fixture.dir, &["try", &complete.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    let accepted = check(&fixture, &branch(), &json!({})).unwrap();
    assert_eq!(
        at(&accepted, "/bindings/cases")["branch_coverage"],
        "checked"
    );
    let replacement = json!({"types":[{"name":"Input","variant":["Absent",["Present"]]}]});
    let error = check(&fixture, &branch(), &replacement).unwrap_err();
    assert_incomplete(&error, "/bindings/params/0/1");
    let complete = json!({"types":[{"name":"Input","variant":["Absent"]}]});
    let error = check(&fixture, &branch(), &complete).unwrap_err();
    assert!(error.detail().contains("not a case"), "{error}");
}

#[test]
fn incomplete_source_variant_refuses_before_actual_residual_trial() {
    let fixture = Fixture::new();
    let before = fixture.workspace.read_head().unwrap().transaction_id();
    let declarations = json!({"af1":1,"types":[{"name":"Input","variant":["Absent",["Present"]]}]});
    let (code, source) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_ne!(code, 0, "{source}");
    let draft = source["draft"].as_str().unwrap();
    let mut request = branch();
    request["base"] = json!(draft);
    let (code, report) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_ne!(code, 0, "{report}");
    assert_eq!(
        report["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
        "{report}"
    );
    assert!(
        report["detail"].as_str().unwrap().contains("incomplete"),
        "{report}"
    );
    assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
    assert!(!fixture.dir.join("final_candidate.hex").exists());
}

#[test]
fn nested_branch_refuses_the_incomplete_input_interface() {
    let fixture = Fixture::new();
    let child = branch();
    let mut nested_bindings = child["bindings"].clone();
    nested_bindings.as_object_mut().unwrap().remove("params");
    nested_bindings.as_object_mut().unwrap().remove("returns");
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["params"] = child["bindings"]["params"].clone();
    request["bindings"]["returns"] = child["bindings"]["returns"].clone();
    request["bindings"]["success"] =
        json!({"fragment":child["fragment"],"bindings":nested_bindings});
    let declarations = json!({"types":[{"name":"Input","variant":["Absent",["Present"]]}]});
    let error = check(&fixture, &request, &declarations).unwrap_err();
    assert_incomplete(&error, "/bindings/params/0/1");
}

#[test]
fn incomplete_source_interfaces_refuse_without_publication_and_allow_ordinary_repair() {
    let fixture = Fixture::new();
    let declarations = json!({"af1":1,"types":[{"name":"Input","variant":["Absent",["Present"]]}]});
    let (code, source) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{source}");
    let mut request = branch();
    request["base"] = source["draft"].clone();
    let mut before = closure_snapshot(&fixture.dir.join(".sley"));
    let events_path = fixture.dir.join(".sley/events.jsonl");
    let events_before = before.remove(&events_path).unwrap();
    let (code, refused) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{refused}");
    assert_eq!(
        refused["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
        "{refused}"
    );
    assert!(
        refused["detail"]
            .as_str()
            .unwrap()
            .contains("/bindings/params/0/1"),
        "{refused}"
    );
    assert!(
        refused["detail"].as_str().unwrap().contains("incomplete"),
        "{refused}"
    );
    let mut after = closure_snapshot(&fixture.dir.join(".sley"));
    let events_after = after.remove(&events_path).unwrap();
    assert!(events_after.starts_with(&events_before));
    let event: Value = serde_json::from_slice(&events_after[events_before.len()..]).unwrap();
    assert_eq!(
        event["refusal"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
        "{event}"
    );
    assert_eq!(after, before);
    let delta = json!({"set":[{"at":"/types/0/variant/1","value":["Present","i8"]}]}).to_string();
    let (code, repaired) = cli(
        &fixture.dir,
        &["fill", "d1", &delta, "--revision", "1", "--no-test"],
    );
    assert_eq!(code, 0, "{repaired}");
    request["base"] = json!("d1@r2");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid", "{result}");
    let handle = result["handle"].as_str().unwrap();
    for (input, expected) in [
        (json!({"Present":7}), json!({"Ok":7})),
        (json!("Absent"), json!({"Ok":17})),
    ] {
        let (code, called) = cli(
            &fixture.dir,
            &["call", "checked", &input.to_string(), "--on", handle],
        );
        assert_eq!(code, 0, "{called}");
        assert_eq!(called["result"], expected);
    }
}

fn closure_snapshot(
    root: &std::path::Path,
) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut pending = vec![root.to_owned()];
    let mut result = std::collections::BTreeMap::new();
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                result.insert(path.clone(), std::fs::read(path).unwrap());
            }
        }
    }
    result
}

pub(super) fn assert_incomplete(error: &sley_agent::AgentError, at: &str) {
    assert_eq!(
        error.code(),
        sley_agent::AgentErrorCode::ResidualConstraintConflict
    );
    assert!(error.detail().contains(at), "{error}");
    assert!(
        error
            .detail()
            .contains("incomplete or unavailable bound definition"),
        "{error}"
    );
}

#[test]
fn incomplete_bound_interfaces_and_draft_gate_refuse_both_relation_versions_before_plan_publication()
 {
    for version in [1, 2] {
        let fixture = Fixture::new();
        let declarations =
            json!({"af1":1,"types":[{"name":"Input","variant":["Absent",["Present"]]}]});
        let (code, source) = cli(
            &fixture.dir,
            &["try", &declarations.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{source}");
        let mut request = branch();
        request["base"] = source["draft"].clone();
        request["bindings"]["join"]
            .as_object_mut()
            .unwrap()
            .remove("term");
        let field = "/bindings/join/term";
        let rows = json!([{field:["ok","answer"]},{field:["return",["ok","answer"]]}]);
        request["choices"] = if version == 1 {
            json!({"version":1,"contract":"author_closed_relation","rows":rows})
        } else {
            json!({"version":2,"contract":"author_closed_constraints","dependencies":[],"constraints":[{"fields":[field],"rows":rows}]})
        };
        let dir = fixture.dir.join(".sley/drafts/d1");
        let before = closure_snapshot(&dir);
        let names_path = fixture.dir.join(".sley/names.json");
        let names = names_path
            .exists()
            .then(|| std::fs::read(&names_path).unwrap());
        let (code, refused) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
        assert_eq!(code, 2, "{refused}");
        if version == 1 {
            assert_eq!(
                refused["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
                "{refused}"
            );
            assert!(
                refused["detail"].as_str().unwrap().contains("incomplete"),
                "{refused}"
            );
        } else {
            // Factored draft planning already requires a valid candidate source.
            // This older gate runs before interface preflight; do not relabel it.
            assert_eq!(refused["error"], "AGENT_DRAFT_INCOMPLETE", "{refused}");
        }
        assert_eq!(closure_snapshot(&dir), before);
        assert_eq!(
            names_path
                .exists()
                .then(|| std::fs::read(&names_path).unwrap()),
            names
        );
        for path in [".sley/residual", ".sley/drafts/d2", ".sley/candidates"] {
            assert!(!fixture.dir.join(path).exists(), "{path}");
        }
    }
}
