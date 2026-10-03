use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};

fn declarations() -> Value {
    json!({"af1":1,"types":[
        {"name":"Pack","record":[["value","i8"],["flag","bool"]]},
        {"name":"Choice","variant":["Empty",["Data","Pack"]]},
        {"name":"Errors","variant":["Missing",["Math","ArithmeticError"]]}
    ]})
}

fn fixture(accepted: bool) -> Fixture {
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
    fixture
}

fn request(returns: &str, ops: Value, term: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["params"] = json!([["x", "i8"], ["ready", "bool"], ["choice", "Choice"]]);
    request["bindings"]["returns"] = json!(returns);
    request["bindings"]["success"] = json!({"ops":[],"term":[]});
    request["bindings"]["success"]["ops"] = ops;
    request["bindings"]["success"]["term"] = term;
    request
}

fn connection<'a>(report: &'a Value, at: &str) -> &'a Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == at)
        .unwrap()
}

fn publish(fixture: &Fixture, request: &Value) {
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
}

fn run(fixture: &Fixture, x: i64, choice: &Value) -> Value {
    let (code, result) = cli(
        &fixture.dir,
        &[
            "call",
            "checked",
            &x.to_string(),
            "true",
            &choice.to_string(),
            "--on",
            "c2",
        ],
    );
    assert_eq!(code, 0, "{result}");
    result["result"].clone()
}

#[test]
fn named_operations_and_aliases_check_accepted_and_draft_members_and_execute() {
    for accepted in [false, true] {
        for words in [
            ["record", "field", "variant", "variant_get"],
            ["record_new", "record_get", "variant_new", "variant_get"],
            ["18", "19", "20", "21"],
        ] {
            let fixture = fixture(accepted);
            let mut request = request(
                "Option<i8>",
                json!([
                    ["pack",words[0],"Pack","x","ready"],
                    ["wrapped",words[2],"Choice.Data","pack"],
                    ["extracted",format!("{}?",words[3]),"Choice.Data","wrapped"],
                    {"name":"value","opcode":words[1],"operands":["Pack.value","extracted"],"type":"i8"}
                ]),
                json!(["return", ["some", "value"]]),
            );
            if !accepted {
                request["base"] = json!("d1@r1");
            }
            let declared = if accepted { json!({}) } else { declarations() };
            let before = bytes(&request);
            let report = check(&fixture, &request, &declared).unwrap();
            for index in 0..4 {
                let entry = connection(&report, &format!("/bindings/success/ops/{index}"));
                assert_eq!(entry["expression_types"], "connections_checked", "{report}");
                assert_eq!(entry["named_values"][0]["members"], "checked");
            }
            publish(&fixture, &request);
            assert_eq!(run(&fixture, 7, &json!("Empty")), json!({"Some":7}));
            assert_eq!(bytes(&request), before);
        }
    }
}

#[test]
fn named_arity_member_kind_and_operand_conflicts_refuse_before_publication() {
    let fixture = fixture(true);
    for operation in [
        json!(["out", "record"]),
        json!(["out", "record", 17]),
        json!(["out", "record", "Missing"]),
        json!(["out", "record", "Choice"]),
        json!(["out", "record", "Pack", "x"]),
        json!(["out", "record", "Pack", "ready", "x"]),
        json!(["out", "record", "Pack", 128, true]),
        json!(["out", "record?", "Pack", "x", "ready"]),
        json!(["out", "field", "value", "choice"]),
        json!(["out", "field", "Choice.Data", "choice"]),
        json!(["out", "field", "Pack.missing", "choice"]),
        json!(["out", "field", "Pack.value", "choice"]),
        json!(["out", "field", "Pack.value"]),
        json!(["out", "variant", "Pack.value", "x"]),
        json!(["out", "variant", "Choice.Empty", "x"]),
        json!(["out", "variant", "Choice.Data"]),
        json!(["out", "variant", "Choice.Data", "x"]),
        json!(["out", "variant_get", "Choice.Empty", "choice"]),
        json!(["out", "variant_get", "Choice.Data", "x"]),
        json!(["out", "variant_get", "Choice.Data", "choice", "choice"]),
        json!({"name":"out","op":"record","args":["Pack","x","ready"],"type":"Choice"}),
    ] {
        let request = request(
            "Option<i8>",
            json!([operation]),
            json!(["return", ["some", "x"]]),
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{request}: {result}");
        assert_eq!(
            result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
            "{result}"
        );
        assert!(
            result["detail"]
                .as_str()
                .unwrap()
                .contains("/bindings/success/ops/0"),
            "{result}"
        );
    }
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
    assert!(!fixture.dir.join(".sley/residual").exists());
    let (code, result) = cli(&fixture.dir, &["draft", "d1"]);
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["draft"], "d1@r1");
}

#[test]
fn variant_extraction_retains_native_none_and_explicit_checked_failure_routes() {
    for (returns, ops, term, empty, data) in [
        (
            "Option<Pack>",
            json!([]),
            json!(["return", ["variant_get", "Choice.Data", "choice"]]),
            json!("None"),
            json!({"Some":{"value":5,"flag":true}}),
        ),
        (
            "Option<i8>",
            json!([]),
            json!([
                "return",
                [
                    "some",
                    [
                        "field",
                        "Pack.value",
                        ["variant_get?", "Choice.Data", "choice"]
                    ]
                ]
            ]),
            json!("None"),
            json!({"Some":5}),
        ),
        (
            "Result<i8,Errors>",
            json!([]),
            json!([
                "ok",
                [
                    "field",
                    "Pack.value",
                    ["variant_get?Missing", "Choice.Data", "choice"]
                ]
            ]),
            json!({"Err":"Missing"}),
            json!({"Ok":5}),
        ),
    ] {
        let fixture = fixture(true);
        let request = request(returns, ops, term);
        check(&fixture, &request, &json!({})).unwrap();
        publish(&fixture, &request);
        assert_eq!(run(&fixture, 1, &json!("Empty")), empty);
        assert_eq!(
            run(&fixture, 1, &json!({"Data":{"value":5,"flag":true}})),
            data
        );
    }
}

#[test]
fn named_payload_context_keeps_nested_arithmetic_failure_before_later_extraction() {
    let fixture = fixture(true);
    let request = request(
        "Result<i8,Errors>",
        json!([
            ["pack", "record", "Pack", ["add?Math", "x", 1], "ready"],
            ["selected", "variant_get?Missing", "Choice.Data", "choice"]
        ]),
        json!(["ok", ["field", "Pack.value", "pack"]]),
    );
    check(&fixture, &request, &json!({})).unwrap();
    publish(&fixture, &request);
    assert_eq!(
        run(&fixture, 127, &json!("Empty")),
        json!({"Err":{"Math":{"ArithmeticError":"Overflow"}}})
    );
    assert_eq!(run(&fixture, 1, &json!("Empty")), json!({"Err":"Missing"}));
    assert_eq!(
        run(&fixture, 1, &json!({"Data":{"value":5,"flag":true}})),
        json!({"Ok":2})
    );
}

#[test]
fn short_variant_cases_use_named_result_context_and_do_not_guess_missing_context() {
    let fixture = fixture(true);
    let request = request(
        "Choice",
        json!([]),
        json!(["return", ["variant", "Data", ["record", "Pack", 1, true]]]),
    );
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/term/1")["expression_types"],
        "connections_checked"
    );
    publish(&fixture, &request);
    assert_eq!(
        run(&fixture, 0, &json!("Empty")),
        json!({"Data":{"value":1,"flag":true}})
    );
    let unresolved = self::request(
        "Option<i8>",
        json!([["out", "variant", "Empty"]]),
        json!(["return", ["some", "x"]]),
    );
    let report = check(&fixture, &unresolved, &json!({})).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/ops/0")["named_values"][0]["members"],
        "immediate_deferred"
    );
}

#[test]
fn incomplete_named_shapes_refuse_and_unknown_operands_stay_partial() {
    let fixture = fixture(true);
    let request = request(
        "Pack",
        json!([]),
        json!(["return", ["record", "Pack", "x", true]]),
    );
    for decl in [
        json!({"types":[{"name":"Pack","record":["value"]}]}),
        json!({"types":[{"name":"Pack","record":[["value","i8"],["value","i8"]]}]}),
    ] {
        let before = bytes(&decl);
        let error = check(&fixture, &request, &decl).unwrap_err();
        super::interface_closure_tests::assert_incomplete(&error, "/bindings/success/term/1/1");
        assert_eq!(bytes(&decl), before);
    }
    let request = self::request(
        "Pack",
        json!([]),
        json!(["return", ["record", "Pack", ["future_expression"], true]]),
    );
    let report = check(&fixture, &request, &json!({})).unwrap();
    let entry = connection(&report, "/bindings/success/term/1");
    assert_eq!(entry["named_values"][0]["members"], "checked");
    assert_eq!(entry["expression_types"], "partial");
    assert_eq!(
        entry["deferred_expressions"],
        json!(["/bindings/success/term/1/2"])
    );
    let override_fields =
        json!({"types":[{"name":"Pack","record":[["value","bool"],["flag","i8"]]}]});
    let error = check(
        &fixture,
        &self::request(
            "Pack",
            json!([]),
            json!(["return", ["record", "Pack", "x", "ready"]]),
        ),
        &override_fields,
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(
        error.detail().contains("/bindings/success/term/1/2"),
        "{error}"
    );
    let error = check(
        &fixture,
        &self::request(
            "Option<i8>",
            json!([["out", "record", "Pack", "x", "ready"]]),
            json!(["return", ["some", "x"]]),
        ),
        &json!({"delete":["Pack"]}),
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
}

#[test]
fn empty_and_unit_constructors_and_optional_record_fields_execute() {
    let fixture = fixture(true);
    let additions = json!({"af1":1,"types":[
        {"name":"EmptyRecord","record":[]},
        {"name":"Optional","record":[["value","Option<i8>"]]}
    ]});
    let (code, result) = cli(&fixture.dir, &["try", &additions.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let mut request = request(
        "Option<i8>",
        json!([
            ["empty","record","EmptyRecord"],
            {"name":"unit","op":"variant","args":["Empty"],"type":"Choice"},
            ["optional","record","Optional",["some","x"]],
            ["extracted","field?","Optional.value","optional"]
        ]),
        json!(["return", ["some", "extracted"]]),
    );
    request["base"] = result["draft"].clone();
    let report = check(&fixture, &request, &additions).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/ops/3")["propagation"][0]["failure_route"],
        "preserve_failure"
    );
    publish(&fixture, &request);
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "7", "true", "\"Empty\"", "--on", "c3"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], json!({"Some":7}));
}

#[test]
fn generic_named_operations_refuse_against_the_vm_non_generic_boundary() {
    use sley_agent::workspace::Program;
    use sley_mutate::value::EntityBodyValue;
    use sley_ssmc::TypeParameterDef;

    let fixture = fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let id = names.resolve("Pack").unwrap();
    let mut record = head.program().object(&id).unwrap().record().clone();
    let EntityBodyValue::TypeDef(body) = &mut record.body else {
        panic!("definition")
    };
    body.type_parameters = vec![TypeParameterDef { ordinal: 0 }];
    let replacement = sley_mutate::build_entity_object(head.program().epoch(), &record).unwrap();
    let objects = head
        .program()
        .objects()
        .iter()
        .map(|object| {
            if object.record().entity_id == id {
                replacement.clone()
            } else {
                object.clone()
            }
        })
        .collect();
    // Metadata fixture only. Unchanged Choice still refers to unparameterized
    // Pack; this synthetic graph is never published or called kernel-valid.
    let program = Program::new(
        head.program().epoch(),
        head.program().root(),
        head.program().workspace(),
        objects,
    );
    let mut request = request(
        "Option<i8>",
        json!([["out", "record", "Pack", "x", "ready"]]),
        json!(["return", ["some", "x"]]),
    );
    request["bindings"]["params"] = json!([["x", "i8"], ["ready", "bool"]]);
    let request = parse_request(&bytes(&request)).unwrap();
    for declarations in [
        json!({}),
        json!({"types":[{"name":"Pack","record":[["value","i8"],["flag","bool"]]}]}),
    ] {
        let error = interfaces::check(
            &program,
            &names,
            declarations.as_object().unwrap(),
            &request,
            &mut Budget::default(),
        )
        .unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("non-generic definition"), "{error}");
    }
}

#[test]
fn named_connections_obey_the_shared_work_and_expression_depth_limits() {
    let fixture = fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let mut request = parse_request(&bytes(&request(
        "Pack",
        json!([]),
        json!(["return", ["record", "Pack", "x", "ready"]]),
    )))
    .unwrap();
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 1);
    for _ in 0..2 {
        let error = interfaces::check(
            head.program(),
            &names,
            &serde_json::Map::new(),
            &request,
            &mut budget,
        )
        .unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    }
    let mut value = json!("x");
    for _ in 0..18 {
        value = json!(["field", "Pack.value", ["record", "Pack", value, true]]);
    }
    request.bindings["success"]["term"] = json!(["return", ["record", "Pack", value, true]]);
    let error = interfaces::check(
        head.program(),
        &names,
        &serde_json::Map::new(),
        &request,
        &mut Budget::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("nesting"), "{error}");
}
