use super::interface_tests::{check, predicate_request};
use super::{Fixture, branch_request, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};

pub(super) fn request() -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("i8");
    request["bindings"]["success"] = json!({"ops":[],"term":["return","x"]});
    request
}

#[test]
fn unused_invalid_interface_types_refuse_before_publication() {
    let fixture = Fixture::new();
    for ty in [
        "$0",
        "Map<Vec<i8>,bool>",
        "Map<f32,bool>",
        "fn(Map<Cell<i8>,bool>)->i8",
    ] {
        let mut request = request();
        request["bindings"]["params"]
            .as_array_mut()
            .unwrap()
            .push(json!(["unused", ty]));
        let before = bytes(&request);
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("/bindings/params/2/1"), "{error}");
        assert!(
            error.detail().contains("declared interface type"),
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
fn named_interface_arguments_cycles_and_nested_map_keys_are_checked() {
    let fixture = Fixture::new();
    for (ty, definitions, code) in [
        (
            "Pack<i8>",
            json!([{ "name":"Pack","record":[["value","i8"]]}]),
            "TYPE_ARGUMENT_ARITY",
        ),
        (
            "Cycle",
            json!([{ "name":"Cycle","record":[["next","Option<Cycle>"]]}]),
            "TYPE_DEFINITION_CYCLE",
        ),
        (
            "Pack",
            json!([{ "name":"Pack","record":[["value","Map<Vec<i8>,bool>"]]}]),
            "TYPE_NOT_ORDERABLE",
        ),
    ] {
        let mut request = request();
        request["bindings"]["params"]
            .as_array_mut()
            .unwrap()
            .push(json!(["unused", ty]));
        let error = check(&fixture, &request, &json!({"types":definitions})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("/bindings/params/2/1"), "{error}");
        assert!(error.detail().contains(code), "{error}");
    }
}

#[test]
fn output_join_and_payload_types_close_even_when_values_are_unresolved() {
    let fixture = Fixture::new();
    let bad = "Map<Vec<i8>,bool>";
    let mut output = request();
    output["bindings"]["returns"] = json!(bad);
    output["bindings"]["success"]["term"] = json!(["return", ["future_expression"]]);
    let mut join = branch_request();
    join["bindings"]["join"]["params"] = json!([["unused", bad]]);
    join["bindings"]["join"]["term"] = json!(["ok", 1]);
    for case in join["bindings"]["cases"].as_array_mut().unwrap() {
        case["values"] = json!([["future_expression"]]);
    }
    let mut payload = branch_request();
    payload["bindings"]["input"] = json!(["future_expression"]);
    payload["bindings"]["cases"][0]["payload"] = json!(["unused", bad]);
    payload["bindings"]["cases"][0]["ops"] = json!([]);
    payload["bindings"]["cases"][0]["values"] = json!([1]);
    for (request, path) in [
        (output, "/bindings/returns"),
        (join, "/bindings/join/params/0/1"),
        (payload, "/bindings/cases/0/payload/1"),
    ] {
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert!(error.detail().contains(path), "{error}");
        assert!(error.detail().contains("TYPE_NOT_ORDERABLE"), "{error}");
    }
}

#[test]
fn incomplete_replacements_refuse_interface_closure_without_reusing_accepted_shapes() {
    let fixture = Fixture::new();
    let definitions = json!({"af1":1,"types":[{"name":"Pack","record":[["value","i8"]]}]});
    let (code, result) = cli(
        &fixture.dir,
        &["try", &definitions.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    let mut request = request();
    request["bindings"]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!(["unused", "Pack"]));
    for declarations in [json!({}), definitions] {
        let report = check(&fixture, &request, &declarations).unwrap();
        assert_eq!(report["declared_interface_types"], "checked");
        assert_eq!(report["closed_interface_type_count"], 4);
        assert_eq!(report["deferred_interface_types"], json!([]));
    }
    let partial = json!({"types":[{"name":"Pack","record":[["value"]]}]});
    let error = check(&fixture, &request, &partial).unwrap_err();
    super::interface_closure_tests::assert_incomplete(&error, "/bindings/params/2/1");
    request["bindings"]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!(["invalid", "$0"]));
    let error = check(&fixture, &request, &partial).unwrap_err();
    assert!(error.detail().contains("/bindings/params/3/1"), "{error}");
}

#[test]
fn valid_composite_interfaces_still_compile_and_execute() {
    let fixture = Fixture::new();
    let mut request = request();
    request["bindings"]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!(["unused", "Map<i8,bool>"]));
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(report["declared_interface_types"], "checked");
    assert_eq!(report["closed_interface_type_count"], 4);
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "7", "true", "[[1,true]]", "--on", "c1"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], 7);
}

#[test]
fn interface_closure_keeps_depth_failures_and_shared_work_exhaustion_as_limits() {
    let fixture = Fixture::new();
    let mut deep = request();
    let ty = format!("{}i8{}", "Option<".repeat(65), ">".repeat(65));
    deep["bindings"]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!(["unused", ty]));
    let error = check(&fixture, &deep, &json!({})).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("/bindings/params/2/1"), "{error}");
    assert!(error.detail().contains("TYPE_DEPTH_LIMIT"), "{error}");

    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let declarations = serde_json::Map::new();
    let baseline = parse_request(&bytes(&request())).unwrap();
    let mut measured = Budget::default();
    interfaces::check(
        head.program(),
        &names,
        &declarations,
        &baseline,
        &mut measured,
    )
    .unwrap();
    let work = measured.usage()["charged_work"].as_u64().unwrap();
    let mut larger = request();
    let ty = format!("Tuple<{}>", vec!["i8"; 128].join(","));
    larger["bindings"]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!(["unused", ty]));
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), work + 20);
    let error = interfaces::check(
        head.program(),
        &names,
        &declarations,
        &parse_request(&bytes(&larger)).unwrap(),
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(budget.usage()["charged_work"].as_u64().unwrap() > work);
}

#[test]
fn invalid_closed_relation_interface_refuses_before_publishing_a_plan() {
    let fixture = Fixture::new();
    let mut request = request();
    request["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("returns");
    request["bindings"]["success"]["term"] = json!(["return", ["future_expression"]]);
    request["choices"] = json!({"version":1,"contract":"author_closed_relation","rows":[
        {"/bindings/returns":"Map<Vec<i8>,bool>"},{"/bindings/returns":"i8"}
    ]});
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    let detail = result["detail"].as_str().unwrap();
    assert!(detail.contains("/choices/rows/0"), "{result}");
    assert!(detail.contains("/bindings/returns"), "{result}");
    assert!(detail.contains("TYPE_NOT_ORDERABLE"), "{result}");
    for artifact in ["drafts", "candidates", "residual"] {
        assert!(!fixture.dir.join(".sley").join(artifact).exists());
    }
}
