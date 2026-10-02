use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};

fn declarations() -> Value {
    let mut frame = super::interface_call_tests::helpers();
    frame["consts"] = json!([
        {"name":"limit","type":"i8","value":7},
        {"name":"maybe","type":"Option<i8>","value":{"Some":7}}
    ]);
    frame
}

pub(super) fn fixture(accepted: bool) -> Fixture {
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

fn run(fixture: &Fixture, candidate: &str) -> Value {
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "1", "true", "--on", candidate],
    );
    assert_eq!(code, 0, "{result}");
    result["result"].clone()
}

#[test]
fn named_constant_aliases_connect_accepted_and_draft_types_and_preserve_checked_values() {
    for accepted in [false, true] {
        for word in ["const", "constant_ref", "1"] {
            let fixture = fixture(accepted);
            let mut request = request(
                "Option<i8>",
                json!([
                    ["raw",word,"limit"],
                    {"name":"value","op":format!("{word}?"),"args":["maybe"],"type":"Option<i8>"}
                ]),
                json!(["return", ["some", "value"]]),
            );
            if !accepted {
                request["base"] = json!("d1@r1");
            }
            let declared = if accepted { json!({}) } else { declarations() };
            let before = bytes(&request);
            let report = check(&fixture, &request, &declared).unwrap();
            let entry = connection(&report, "/bindings/success/ops/0");
            assert_eq!(entry["expression_types"], "connections_checked");
            assert_eq!(
                entry["references"][0]["source"],
                if accepted {
                    "accepted_constant"
                } else {
                    "declared_constant"
                }
            );
            publish(&fixture, &request);
            assert_eq!(run(&fixture, "c2"), json!({"Some":7}));
            assert_eq!(bytes(&request), before);
        }
    }
}

#[test]
fn constant_literals_follow_context_and_explicit_types_without_guessing_sibling_widths() {
    for (returns, term, expected) in [
        (
            "i8",
            json!(["return", ["const", {"type":"i8","value":7}]]),
            json!(7),
        ),
        (
            "bool",
            json!(["return", ["eq", ["const", {"type":"i8","value":1}], "x"]]),
            json!(true),
        ),
        (
            "bool",
            json!(["return", ["eq", "x", ["const", {"type":"i8","value":1}]]]),
            json!(true),
        ),
        ("bool", json!(["return", ["const", true]]), json!(true)),
        (
            "f32",
            json!(["return",["const",{"type":"f32","value":"inf"}]]),
            json!("inf"),
        ),
        ("i64", json!(["return",["const",{"value":7}]]), json!(7)),
    ] {
        let fixture = fixture(true);
        let request = request(returns, json!([]), term);
        let report = check(&fixture, &request, &json!({})).unwrap();
        assert_eq!(
            connection(&report, "/bindings/success/term/1")["expression_types"],
            "connections_checked",
            "{report}"
        );
        publish(&fixture, &request);
        assert_eq!(run(&fixture, "c2"), expected);
    }
    let fixture = fixture(true);
    let request = request(
        "i8",
        json!([["unused", "const", 1]]),
        json!(["return", "x"]),
    );
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/ops/0")["expression_types"],
        "connections_checked"
    );
    let composite = self::request(
        "Option<i8>",
        json!([]),
        json!(["return",["const",{"type":"Option<i8>","value":{"Some":1}}]]),
    );
    let report = check(&fixture, &composite, &json!({})).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/term/1")["expression_types"],
        "connections_checked"
    );
    publish(&fixture, &composite);
    assert_eq!(run(&fixture, "c2"), json!({"Some":1}));
}

#[test]
fn reference_arity_identity_type_and_range_refusals_publish_nothing() {
    let fixture = fixture(true);
    for operation in [
        json!(["out", "const"]),
        json!(["out", "const", "limit", "x"]),
        json!(["out", "const", "x"]),
        json!(["out", "const", "echo"]),
        json!(["out", "global", "limit"]),
        json!(["out", "global", 1]),
        json!(["out", "fnref", "limit"]),
        json!(["out", "fnref", "echo", "x"]),
        json!(["out", "fnref?", "echo"]),
        json!({"name":"out","op":"const","args":[128],"type":"i8"}),
        json!({"name":"out","op":"const","args":[{"type":"i8","value":128}]}),
        json!({"name":"out","op":"const","args":["limit"],"type":"bool"}),
        json!({"name":"out","op":"fnref","args":["echo"],"type":"fn(bool)->i8"}),
    ] {
        let request = request("i8", json!([operation]), json!(["return", "x"]));
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
}

#[test]
fn deleted_shadowed_and_retyped_constants_do_not_fall_back_to_accepted_metadata() {
    let fixture = fixture(true);
    let request = request("i8", json!([]), json!(["return", ["const", "limit"]]));
    for declarations in [
        json!({"delete":["limit"]}),
        json!({"types":[{"name":"limit","record":[]}]}),
        json!({"consts":[{"name":"limit","type":"bool","value":true}]}),
    ] {
        let before = bytes(&declarations);
        let error = check(&fixture, &request, &declarations).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert_eq!(bytes(&declarations), before);
    }
}

#[test]
fn accepted_function_reference_aliases_preserve_signature_and_execute() {
    for word in ["fnref", "function_ref", "194"] {
        let fixture = fixture(true);
        let request = request("fn(i8)->i8", json!([]), json!(["return", [word, "echo"]]));
        let report = check(&fixture, &request, &json!({})).unwrap();
        let entry = connection(&report, "/bindings/success/term/1");
        assert_eq!(entry["expression_types"], "connections_checked");
        assert_eq!(entry["references"][0]["effect_ids"], json!([]));
        publish(&fixture, &request);
        let result = run(&fixture, "c2");
        assert_eq!(result, json!("fn echo"));
    }
}

#[test]
fn draft_and_recursive_function_references_use_the_af1_effect_interface() {
    let fixture = fixture(false);
    let mut request = request(
        "fn(i8)->i8",
        json!([]),
        json!(["return", ["fnref", "echo"]]),
    );
    request["base"] = json!("d1@r1");
    let report = check(&fixture, &request, &declarations()).unwrap();
    let entry = connection(&report, "/bindings/success/term/1");
    assert_eq!(entry["expression_types"], "connections_checked");
    assert_eq!(entry["references"][0]["source"], "declared_function");
    publish(&fixture, &request);
    assert_eq!(run(&fixture, "c2"), json!("fn echo"));
    let recursive = self::request(
        "i8",
        json!([["self", "fnref", "checked"]]),
        json!(["return", "x"]),
    );
    let report = check(&fixture, &recursive, &json!({})).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/ops/0")["expression_types"],
        "connections_checked"
    );
}

#[test]
fn accepted_globals_check_initializer_types_and_execute_all_aliases() {
    for word in ["global", "global_get", "193"] {
        let fixture = fixture(true);
        add_global(&fixture);
        let request = request("i8", json!([]), json!(["return", [word, "bound"]]));
        let report = check(&fixture, &request, &json!({})).unwrap();
        assert_eq!(
            connection(&report, "/bindings/success/term/1")["references"][0]["source"],
            "accepted_global"
        );
        publish(&fixture, &request);
        assert_eq!(run(&fixture, "c2"), json!(7));
        for declarations in [
            json!({"delete":["limit"]}),
            json!({"delete":["bound"]}),
            json!({"consts":[{"name":"limit","type":"bool","value":true}]}),
            json!({"delete":["limit"],"consts":[{"name":"limit","type":"i8","value":9}]}),
        ] {
            let error = check(&fixture, &request, &declarations).unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        }
    }
}

pub(super) fn add_global(fixture: &Fixture) {
    use sley_agent::candidate::{self, Authority, PlannedOp};
    use sley_mutate::{
        MutationPayload,
        value::{EntityBodyValue, GlobalValueBody},
    };
    use sley_ssmc::{IntegerWidth, TypeExpr, Visibility};

    // AF1 and the CLI raw path do not author globals. Build this fixture with
    // the public candidate API and require actual kernel validation/commit.
    let head = fixture.workspace.read_head().unwrap();
    let map_path = fixture.dir.join(".sley/names.json");
    let mut map = NameMap::read(&map_path).unwrap();
    let names = Names::build(head.program(), &map);
    let authority = Authority::of(&head).unwrap();
    let nonce = candidate::fresh_nonce().unwrap();
    let id = candidate::created_id(head.program(), nonce, 10, 0);
    let candidate = candidate::assemble(
        &head,
        &authority,
        nonce,
        vec![PlannedOp {
            kind: 10,
            target: id,
            field_tag: None,
            payload: MutationPayload::CreateEntity(EntityBodyValue::GlobalValue(GlobalValueBody {
                value_type: TypeExpr::SInt(IntegerWidth::from_bits(8)),
                initializer: names.resolve("limit").unwrap(),
                visibility: Visibility::Exported,
            })),
        }],
    )
    .unwrap();
    let output = candidate::validate(&head, &authority, &candidate.stored_bytes).unwrap();
    assert!(output.is_valid(), "{output:?}");
    sley_agent::genesis::commit(&head, &fixture.workspace.repo(), &candidate.stored_bytes).unwrap();
    map.insert(*id.as_bytes(), "bound");
    map.write(&map_path).unwrap();
}

#[test]
fn reference_checks_use_the_shared_budget() {
    let fixture = fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let request = parse_request(&bytes(&request(
        "i8",
        json!([]),
        json!(["return", ["const", "limit"]]),
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
}

#[test]
fn bare_numeric_constant_widths_follow_ordinary_use_hints() {
    let fixture = fixture(true);
    let request = request("i8", json!([]), json!(["return", ["const", 7]]));
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        connection(&report, "/bindings/success/term/1")["expression_types"],
        "connections_checked"
    );
    publish(&fixture, &request);
    assert_eq!(run(&fixture, "c2"), json!(7));
    let fixture = self::fixture(true);
    let request = self::request(
        "bool",
        json!([]),
        json!(["return", ["eq", ["const", 1], "x"]]),
    );
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    assert!(
        result["detail"].as_str().unwrap().contains("i64"),
        "{result}"
    );
}

#[test]
fn function_reference_metadata_preserves_effect_ids_and_refuses_generics() {
    use sley_agent::workspace::Program;
    use sley_mutate::value::{EntityBodyValue, EntityIdSet};
    use sley_ssmc::TypeParameterDef;
    let fixture = fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let id = names.resolve("echo").unwrap();
    let effect = sley_id::EntityId::from_bytes([231; 32]);
    for generic in [false, true] {
        let mut record = head.program().object(&id).unwrap().record().clone();
        let EntityBodyValue::Function(body) = &mut record.body else {
            panic!("function")
        };
        body.effects = EntityIdSet::from_unsorted(vec![effect]).unwrap();
        if generic {
            body.type_parameters = vec![TypeParameterDef { ordinal: 0 }];
        }
        let replacement =
            sley_mutate::build_entity_object(head.program().epoch(), &record).unwrap();
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
        // Synthetic metadata isolates type/effect handling. No effect definition
        // is supplied, so this is never published or claimed kernel-valid.
        let program = Program::new(
            head.program().epoch(),
            head.program().root(),
            head.program().workspace(),
            objects,
        );
        let request = parse_request(&bytes(&request(
            "i8",
            json!([["unused", "fnref", "echo"]]),
            json!(["return", "x"]),
        )))
        .unwrap();
        let result = interfaces::check(
            &program,
            &names,
            &serde_json::Map::new(),
            &request,
            &mut Budget::default(),
        );
        if generic {
            let error = result.unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
            assert!(error.detail().contains("non-generic"), "{error}");
        } else {
            let report = result.unwrap();
            assert_eq!(
                connection(&report, "/bindings/success/ops/0")["references"][0]["effect_ids"],
                json!([sley_agent::hex::encode(effect.as_bytes())])
            );
            let request = parse_request(&bytes(&self::request(
                "fn(i8)->i8",
                json!([]),
                json!(["return", ["fnref", "echo"]]),
            )))
            .unwrap();
            let error = interfaces::check(
                &program,
                &names,
                &serde_json::Map::new(),
                &request,
                &mut Budget::default(),
            )
            .unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
            assert!(error.detail().contains("effect identities"), "{error}");
        }
    }
}
