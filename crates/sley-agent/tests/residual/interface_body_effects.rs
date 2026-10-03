use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli, interface_call_tests, interface_effect_tests};
use serde_json::{Value, json};
use sley_agent::{
    AgentErrorCode,
    names::{NameMap, Names},
    residual::{frontier::Budget, interfaces, parse_request},
};

fn helper(block: &Value) -> Value {
    json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["x","i8"]],
        "returns":"i8","blocks":[block]}]})
}

#[test]
fn draft_body_effect_conflicts_cover_statements_nested_expressions_and_edges() {
    let fixture = interface_call_tests::fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    // Metadata-only fixture: accepted `echo` is assigned a nonempty effect set.
    // This is an interface check, not a kernel-admitted effectful program.
    let program = interface_effect_tests::effectful_metadata(head.program(), &names);
    let request = parse_request(&bytes(&predicate_request(json!(false)))).unwrap();
    for (body, pointer) in [
        (
            json!({"name":"entry","ops":[["v","112","echo","x"]],"term":["return","v"]}),
            "/ops/0",
        ),
        (
            json!({"name":"entry","ops":[{"name":"v","opcode":"call_direct","operands":["echo","x"]}],"term":["return","v"]}),
            "/ops/0",
        ),
        (
            json!({"name":"entry","ops":[],"term":["return",["call","echo","x"]]}),
            "/term/1",
        ),
        (
            json!({"name":"entry","ops":[],"term":["cond",["eq",["call","echo","x"],0],"yes","no"]}),
            "/term/1/1",
        ),
        (
            json!({"name":"entry","ops":[],"term":["br","next",["call","echo","x"]]}),
            "/term/2",
        ),
        (
            json!({"name":"entry","ops":[],"term":["switch",["some","x"],["Some","next",["call","echo","x"]],["None","empty"]]}),
            "/term/2/2",
        ),
        (
            json!({"name":"entry","ops":[["!Bad","if",["eq",["call","echo","x"],0]]],"term":["return","x"]}),
            "/ops/0/2/1",
        ),
    ] {
        let declarations = helper(&body);
        let before = bytes(&declarations);
        let error = interfaces::check(
            &program,
            &names,
            declarations.as_object().unwrap(),
            &request,
            &mut Budget::default(),
        )
        .unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error
                .detail()
                .contains(&format!("/fns/0/blocks/0{pointer}")),
            "{error}"
        );
        assert!(
            error.detail().contains("`echo`") && error.detail().contains("`helper`"),
            "{error}"
        );
        assert_eq!(bytes(&declarations), before);
    }
}

#[test]
fn complete_draft_helper_chains_are_checked_and_execute_after_composition() {
    let fixture = Fixture::new();
    let declarations = json!({"af1":1,"afx":1,"functions":[
        {"fn":"first","params":[["x","bool"]],"returns":"bool","blocks":[
            {"name":"entry","ops":[],"term":["return",["call","second","x"]]}]},
        {"fn":"second","params":[["x","bool"]],"returns":"bool","blocks":[
            {"name":"entry","ops":[],"term":["return",["not","x"]]}]}
    ]});
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("bool");
    request["bindings"]["success"] = json!({"ops":[],"term":["return",["call","first","ready"]]});
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(report["declared_body_effects"]["status"], "checked");
    let functions = &report["declared_body_effects"]["checked_functions"];
    assert_eq!(functions.as_array().unwrap().len(), 2);
    assert_eq!(functions[0]["calls"][0]["callee"], "second");
    assert_eq!(functions[0]["calls"][0]["interface"], "declared_empty");
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    request["base"] = json!("d1@r1");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (input, expected) in [("true", false), ("false", true)] {
        let (code, result) = cli(&fixture.dir, &["call", "checked", "7", input, "--on", "c2"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!(expected));
    }
}

#[test]
fn draft_body_inventory_preserves_literal_data_and_function_reference_immediates() {
    let fixture = interface_call_tests::fixture(true);
    let declarations = helper(&json!({"name":"entry","ops":[
        ["data","const",{"type":"Vec<text>","value":["call","absent"]}],
        ["reference","fnref","echo"]],"term":["return","x"]}));
    let report = check(&fixture, &predicate_request(json!(false)), &declarations).unwrap();
    let effect = &report["declared_body_effects"];
    assert_eq!(effect["status"], "checked");
    assert_eq!(effect["checked_functions"][0]["calls"], json!([]));
}

#[test]
fn draft_body_unknown_syntax_and_later_rewrites_stay_deferred() {
    let fixture = Fixture::new();
    let request = predicate_request(json!(false));
    let mut declarations =
        helper(&json!({"name":"entry","ops":[["v","future_operation"]],"term":["return","x"]}));
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(report["declared_body_effects"]["status"], "partial");
    assert_eq!(
        report["declared_body_effects"]["deferred"],
        json!(["/fns/0/blocks/0/ops/0"])
    );
    declarations["fns"][0]["blocks"][0]["ops"] = json!([["v", "effect", "missing"]]);
    for key in ["edit", "ripple"] {
        let mut later = declarations.clone();
        later[key] = json!([{}]);
        let report = check(&fixture, &request, &later).unwrap();
        assert_eq!(report["declared_body_effects"]["status"], "deferred");
        assert_eq!(
            report["declared_body_effects"]["reason"],
            "later_body_transformations"
        );
    }
    declarations["patch"] = json!([{"fn":"helper","blocks":{"entry":null}}]);
    let error = check(&fixture, &request, &declarations).unwrap_err();
    assert!(
        error.detail().contains("restated more than once"),
        "{error}"
    );
    assert!(error.detail().contains("/fns/0"), "{error}");
    assert!(error.detail().contains("/patch/0"), "{error}");
    // The residual target's prior body is replaced by the new fragment.
    declarations.as_object_mut().unwrap().remove("patch");
    declarations["fns"][0]["fn"] = json!("checked");
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(
        report["declared_body_effects"]["checked_functions"],
        json!([])
    );
}

#[test]
fn draft_body_excluded_operations_refuse_at_the_original_source_locator() {
    let fixture = Fixture::new();
    for word in [
        "effect",
        "effect_request",
        "160",
        "adapter",
        "assert",
        "observe",
        "narrow",
    ] {
        let declarations =
            helper(&json!({"name":"entry","ops":[],"term":["return",[word,"target","x"]]}));
        let error = check(&fixture, &predicate_request(json!(false)), &declarations).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualFragmentShape);
        assert!(error.detail().contains("/fns/0/blocks/0/term/1"), "{error}");
        assert!(
            error
                .detail()
                .contains("CANDIDATE_OPERATION_ANALYSIS_UNSUPPORTED"),
            "{error}"
        );
    }
}

#[test]
fn mutually_recursive_declared_effect_interfaces_terminate_under_shared_budget() {
    let fixture = Fixture::new();
    let declarations = json!({"fns":[
        {"fn":"left","params":[["x","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[],"term":["return",["call","right","x"]]}]},
        {"fn":"right","params":[["x","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[],"term":["return",["call","left","x"]]}]}
    ]});
    let request = predicate_request(json!(false));
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(report["declared_body_effects"]["status"], "checked");
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(head.program(), &NameMap::default());
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 10);
    let error = interfaces::check(
        head.program(),
        &names,
        declarations.as_object().unwrap(),
        &parse_request(&bytes(&request)).unwrap(),
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
}
