use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::{
    AgentErrorCode,
    names::{NameMap, Names},
    residual::{frontier::Budget, interfaces, parse_request},
};

fn request(ty: &str, value: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!(ty);
    request["bindings"]["success"] = json!({"ops":[],"term":["return",{"type":ty,"value":null}]});
    request["bindings"]["success"]["term"][1]["value"] = value;
    request
}

fn terminal(report: &Value) -> &Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["at"] == "/bindings/success/term/1")
        .unwrap()
}

fn definitions() -> Value {
    json!({"af1":1,"types":[
        {"name":"Pack","record":[["value","i8"],["flag","bool"]]},
        {"name":"Choice","variant":["Empty",["Data","Pack"]]}
    ]})
}

fn execute(fixture: &Fixture, request: &Value) -> Value {
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid", "{result}");
    let (code, ran) = cli(
        &fixture.dir,
        &[
            "call",
            "checked",
            "3",
            "true",
            "--on",
            result["handle"].as_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{ran}");
    ran["result"].clone()
}

#[test]
fn composite_literal_contents_match_the_ordinary_compiler_and_vm() {
    for (ty, value, expected) in [
        (
            "Tuple<i8,bool,Vec<u8>>",
            json!([-7, true, [0, 255]]),
            json!([-7, true, [0, 255]]),
        ),
        (
            "Vec<Option<i8>>",
            json!([null,{"Some":7}]),
            json!(["None",{"Some":7}]),
        ),
        (
            "Result<i8,ArithmeticError>",
            json!({"Err":"Overflow"}),
            json!({"Err":{"ArithmeticError":"Overflow"}}),
        ),
        (
            "Map<i8,bool>",
            json!([[2, false], [1, true]]),
            json!([[1, true], [2, false]]),
        ),
    ] {
        let fixture = Fixture::new();
        let request = request(ty, value);
        let before = bytes(&request);
        let report = check(&fixture, &request, &json!({})).unwrap();
        assert_eq!(
            terminal(&report)["expression_types"],
            "connections_checked",
            "{report}"
        );
        assert_eq!(terminal(&report)["deferred_expressions"], json!([]));
        assert_eq!(execute(&fixture, &request), expected);
        assert_eq!(bytes(&request), before);
    }
}

#[test]
fn invalid_nested_literals_refuse_at_the_value_location_without_publication() {
    for (ty, value, locator) in [
        ("Vec<i8>", json!([1, 128]), "/value/1"),
        ("Tuple<i8,bool>", json!([1, 4]), "/value/1"),
        ("Option<Vec<u8>>", json!({"Some":[1,-1]}), "/value/Some/1"),
        ("Result<i8,bool>", json!({"Ok":128}), "/value/Ok"),
        ("Map<i8,bool>", json!([[1, true], ["1", false]]), "/value"),
        ("Map<i8,bool>", json!([[1, 2]]), "/value/0/1"),
        ("ArithmeticError", json!("Unknown"), "/value"),
    ] {
        let fixture = Fixture::new();
        let request = request(ty, value);
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
                .contains(&format!("/bindings/success/term/1{locator}")),
            "{result}"
        );
        for path in [".sley/drafts", ".sley/candidates", ".sley/residual"] {
            assert!(!fixture.dir.join(path).exists());
        }
    }
}

#[test]
fn named_literal_contents_use_complete_draft_or_accepted_definitions() {
    for accepted in [false, true] {
        let fixture = Fixture::new();
        let declarations = definitions();
        let (code, result) = cli(
            &fixture.dir,
            &["try", &declarations.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        if accepted {
            let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
            assert_eq!(code, 0, "{result}");
        }
        let mut request = request("Choice", json!({"Data":{"value":7,"flag":true}}));
        if !accepted {
            request["base"] = json!("d1@r1");
        }
        let bound = if accepted { json!({}) } else { declarations };
        let report = check(&fixture, &request, &bound).unwrap();
        assert_eq!(
            terminal(&report)["expression_types"],
            "connections_checked",
            "{report}"
        );
        assert_eq!(
            execute(&fixture, &request),
            json!({"Data":{"value":7,"flag":true}})
        );
        for value in [
            json!({"Data":{"value":128,"flag":true}}),
            json!("Data"),
            json!({"Empty":0}),
            json!({"Missing":0}),
        ] {
            request["bindings"]["success"]["term"][1]["value"] = value;
            // The direct read-only interface check uses the bound declaration
            // view; a prior successful trial can advance the source draft.
            let error = check(&fixture, &request, &bound).unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
            assert!(
                error.detail().contains("/bindings/success/term/1/value"),
                "{error}"
            );
        }
    }
}

#[test]
fn partial_definitions_refuse_contents_and_replacements_do_not_use_old_field_types() {
    let fixture = Fixture::new();
    let request = request("Choice", json!({"Data":{"value":7,"flag":true}}));
    let mut declarations = definitions();
    declarations["types"][0]["record"] = json!(["value"]);
    let error = check(&fixture, &request, &declarations).unwrap_err();
    super::interface_closure_tests::assert_incomplete(&error, "/bindings/success/term/1/value");
    let declarations = definitions();
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    let replacement = json!({"types":[{"name":"Pack","record":[["value","bool"],["flag","i8"]]}]});
    let error = check(&fixture, &request, &replacement).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(error.detail().contains("/value/Data/"), "{error}");
}

#[test]
fn literal_traversal_consumes_the_existing_shared_work_budget() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(head.program(), &NameMap::default());
    let decl = json!({});
    let mut measured = Budget::default();
    interfaces::check(
        head.program(),
        &names,
        decl.as_object().unwrap(),
        &parse_request(&bytes(&request("Vec<i8>", json!([])))).unwrap(),
        &mut measured,
    )
    .unwrap();
    let work = measured.usage()["charged_work"].as_u64().unwrap();
    let large = parse_request(&bytes(&request("Vec<i8>", json!(vec![1; 100])))).unwrap();
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), work + 120);
    let error = interfaces::check(
        head.program(),
        &names,
        decl.as_object().unwrap(),
        &large,
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(budget.usage()["charged_work"].as_u64().unwrap() > work);
}

#[test]
fn closed_relation_checks_literal_contents_before_publishing_any_plan() {
    let fixture = Fixture::new();
    let mut request = request("Vec<i8>", json!([128]));
    request["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("returns");
    request["choices"] = json!({"version":1,"contract":"author_closed_relation","rows":[
        {"/bindings/returns":"Vec<i8>"},{"/bindings/returns":"Vec<u8>"}
    ]});
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(
        result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
        "{result}"
    );
    let detail = result["detail"].as_str().unwrap();
    assert!(detail.contains("/choices/rows/0"), "{result}");
    assert!(
        detail.contains("/bindings/success/term/1/value/0"),
        "{result}"
    );
    for path in [".sley/drafts", ".sley/candidates", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}
