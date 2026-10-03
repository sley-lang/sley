use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};

fn request(key: &str) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["params"] = json!([["key", key], ["x", "i8"]]);
    request["bindings"]["returns"] = json!(format!("Result<Map<{key},i8>,DuplicateKeyError>"));
    request["bindings"]["success"] =
        json!({"ops":[["out","map","key","x"]],"term":["return","out"]});
    request
}

fn connection(report: &Value) -> &Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == "/bindings/success/ops/0")
        .unwrap()
}

fn declarations() -> Value {
    json!({"af1":1,"types":[
        {"name":"Inner","record":[["id","i8"]]},
        {"name":"Key","variant":["Empty",["Data","Inner"]]}
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

#[test]
fn accepted_and_draft_named_keys_check_nested_shapes_and_execute_in_vm() {
    for accepted in [false, true] {
        let fixture = fixture(accepted);
        let mut request = request("Key");
        if !accepted {
            request["base"] = json!("d1@r1");
        }
        let declared = if accepted { json!({}) } else { declarations() };
        let before = bytes(&request);
        let report = check(&fixture, &request, &declared).unwrap();
        assert_eq!(
            connection(&report)["expression_types"],
            "connections_checked"
        );
        assert_eq!(connection(&report)["maps"][0]["key_traits"], "checked");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        for key in [json!("Empty"), json!({"Data":{"id":7}})] {
            let (code, result) = cli(
                &fixture.dir,
                &["call", "checked", &key.to_string(), "3", "--on", "c2"],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], json!({"Ok":[[key,3]]}));
        }
        assert_eq!(bytes(&request), before);
    }
}

#[test]
fn draft_overrides_replace_accepted_member_traits_in_reachable_dependencies() {
    let fixture = fixture(true);
    let request = request("Key");
    check(&fixture, &request, &json!({})).unwrap();
    for ty in ["f32", "Vec<i8>", "Cell<i8>"] {
        let declarations = json!({"types":[{"name":"Inner","record":[["id",ty]]}]});
        let before = bytes(&declarations);
        let error = check(&fixture, &request, &declarations).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/bindings/success/ops/0/1"),
            "{error}"
        );
        assert!(error.detail().contains("Key"), "{error}");
        assert!(error.detail().contains("TYPE_NOT_ORDERABLE"), "{error}");
        assert_eq!(bytes(&declarations), before);
    }
    // Restating a variant must consider every case, including an unused one.
    let invalid = json!({"types":[{"name":"Key","variant":["Empty",["Bad","Vec<i8>"]]}]});
    assert!(
        check(&fixture, &request, &invalid)
            .unwrap_err()
            .detail()
            .contains("inadmissible")
    );
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
}

#[test]
fn malformed_and_shadowed_definitions_do_not_become_empty_admissible_keys() {
    let fixture = Fixture::new();
    for key in [
        json!({"name":"Key","record":["id"]}),
        json!({"name":"Key","record":[["id",null]]}),
        json!({"name":"Key","record":[["id","i8"],["id","i8"]]}),
        json!({"name":"Key","record":[["bad name","i8"]]}),
        json!({"name":"Key","variant":[["id","i8",true]]}),
        json!({"name":"Key","record":[],"variant":[]}),
        json!({"name":"Key","record":false}),
    ] {
        let error = check(&fixture, &request("Key"), &json!({"types":[key]})).unwrap_err();
        super::interface_closure_tests::assert_incomplete(&error, "/bindings/params/");
    }
    let duplicate =
        json!({"types":[{"name":"Key","record":[["id","f32"]]},{"name":"Key","record":[]}]});
    let error = check(&fixture, &request("Key"), &duplicate).unwrap_err();
    super::interface_closure_tests::assert_incomplete(&error, "/bindings/params/");
    let fixture = self::fixture(true);
    let error = check(&fixture, &request("Key"), &json!({"delete":["Inner"]})).unwrap_err();
    super::interface_closure_tests::assert_incomplete(&error, "/bindings/params/");
}

#[test]
fn named_definition_cycles_and_excessive_depth_are_bounded() {
    let fixture = Fixture::new();
    for declarations in [
        json!({"types":[{"name":"Key","record":[["next","Key"]]}]}),
        json!({"types":[{"name":"Key","variant":["End",["Next","Inner"]]},{"name":"Inner","record":[["next","Key"]]}]}),
    ] {
        let error = check(&fixture, &request("Key"), &declarations).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("TYPE_DEFINITION_CYCLE"), "{error}");
    }
    let types:Vec<_> = (0..70).map(|index| json!({"name":format!("Key{index}"),"record":[["next",if index == 69 {"i8".into()} else {format!("Key{}",index+1)}]]})).collect();
    let error = check(&fixture, &request("Key0"), &json!({"types":types})).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit, "{error}");
}

#[test]
fn accepted_generic_parameters_survive_draft_restatement_and_check_arguments() {
    use sley_agent::workspace::Program;
    use sley_mutate::value::EntityBodyValue;
    use sley_ssmc::{TypeDefForm, TypeExpr, TypeParameterDef};

    let fixture = fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let id = names.resolve("Inner").unwrap();
    let mut record = head.program().object(&id).unwrap().record().clone();
    let EntityBodyValue::TypeDef(body) = &mut record.body else {
        panic!("type definition")
    };
    body.type_parameters = vec![TypeParameterDef { ordinal: 0 }];
    let TypeDefForm::Record(fields) = &mut body.form else {
        panic!("record")
    };
    fields[0].value_type = TypeExpr::TypeParameter(0);
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
    // Metadata-only fixture isolates generic hydration. The unchanged Key still
    // names Inner without arguments; this synthetic program is never published
    // or represented as a kernel-valid accepted graph.
    let program = Program::new(
        head.program().epoch(),
        head.program().root(),
        head.program().workspace(),
        objects,
    );
    for declarations in [
        json!({}),
        json!({"types":[{"name":"Inner","record":[["id","$0"]]}]}),
    ] {
        for (key, valid) in [
            ("Inner<i8>", true),
            ("Inner<f32>", false),
            ("Inner", false),
            ("Inner<i8,bool>", false),
        ] {
            let request = parse_request(&bytes(&request(key))).unwrap();
            let result = interfaces::check(
                &program,
                &names,
                declarations.as_object().unwrap(),
                &request,
                &mut Budget::default(),
            );
            if valid {
                assert_eq!(
                    connection(&result.unwrap())["maps"][0]["key_traits"],
                    "checked"
                );
            } else {
                let error = result.unwrap_err();
                assert_eq!(
                    error.code(),
                    AgentErrorCode::ResidualConstraintConflict,
                    "{error}"
                );
                assert!(
                    error.detail().contains(if key.contains("f32") {
                        "TYPE_NOT_ORDERABLE"
                    } else {
                        "TYPE_ARGUMENT_ARITY"
                    }),
                    "{error}"
                );
            }
        }
    }
}

#[test]
fn named_key_walk_uses_the_invocation_budget_without_resetting_it() {
    let fixture = fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let mut usage = Vec::new();
    for key in ["i8", "Key"] {
        let request = parse_request(&bytes(&request(key))).unwrap();
        let mut budget = Budget::default();
        interfaces::check(
            head.program(),
            &names,
            &serde_json::Map::new(),
            &request,
            &mut budget,
        )
        .unwrap();
        usage.push(budget.usage()["charged_work"].as_u64().unwrap());
    }
    // Equal operation/parameter counts isolate the additional named-definition
    // traversal. A scalar-sized budget must not suffice for the named closure.
    assert!(usage[1] > usage[0], "{usage:?}");
    let request = parse_request(&bytes(&request("Key"))).unwrap();
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), usage[0]);
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
}
