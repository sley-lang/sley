use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};
use sley_agent::{
    AgentErrorCode,
    names::{NameMap, Names},
    types, values,
};

fn request(ty: &str, expression: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["params"] = json!([["x", ty]]);
    request["bindings"]["returns"] = json!("bytes");
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["success"] = json!({"ops":[],"term":["return",null]});
    request["bindings"]["success"]["term"][1] = expression;
    request
}

fn connection(report: &Value) -> &Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == "/bindings/success/term/1")
        .unwrap()
}

fn execute(fixture: &Fixture, request: &Value, candidate: &str, input: &Value) -> Value {
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", &input.to_string(), "--on", candidate],
    );
    assert_eq!(code, 0, "{result}");
    result["result"].clone()
}

struct TypeNames<'a>(&'a Names);
impl types::TypeNames for TypeNames<'_> {
    fn type_definition(&self, name: &str) -> Option<sley_id::EntityId> {
        self.0.resolve(name)
    }
}

fn canonical_hash(fixture: &Fixture, ty: &str, input: &Value) -> Value {
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let ty = types::read(&json!(ty), &TypeNames(&names), "/oracle/type").unwrap();
    let value = values::read(
        input,
        &ty,
        &values::ProgramTypes {
            program: head.program(),
            names: &names,
        },
        "/oracle/value",
    )
    .unwrap();
    let hash =
        sley_ssmc::fingerprint::hash_validated_value(head.program().epoch(), &value).unwrap();
    json!(format!("0x{}", sley_agent::hex::encode(hash.as_bytes())))
}

#[test]
fn hash_aliases_match_canonical_value_hashing_through_the_vm() {
    for word in ["hash", "value_hash", "192"] {
        for (ty, input) in [
            ("i8", json!(-7)),
            ("bool", json!(true)),
            ("text", json!("é\n")),
            ("bytes", json!("0x00ff")),
            ("unit", Value::Null),
            ("Vec<f32>", json!(["NaN", 0, 1])),
            ("Tuple<f64,bool>", json!(["NaN", true])),
        ] {
            let fixture = Fixture::new();
            let request = request(ty, json!([word, "x"]));
            let before = bytes(&request);
            let report = check(&fixture, &request, &json!({})).unwrap();
            let entry = connection(&report);
            assert_eq!(entry["expression_types"], "connections_checked");
            assert_eq!(entry["hashes"][0]["eligibility"], "checked");
            assert_eq!(entry["hashes"][0]["evaluated"], false);
            assert_eq!(
                execute(&fixture, &request, "c1", &input),
                canonical_hash(&fixture, ty, &input)
            );
            assert_eq!(bytes(&request), before);
        }
    }
}

#[test]
fn nested_and_object_hash_operations_keep_exact_types_and_locations() {
    let fixture = Fixture::new();
    let mut request = request("i8", json!(["hash", "first"]));
    request["bindings"]["success"]["ops"] = json!([
        {"name":"first","opcode":"value_hash","operands":["x"],"type":"bytes"}
    ]);
    let input = json!(7);
    let first = canonical_hash(&fixture, "i8", &input);
    assert_eq!(
        execute(&fixture, &request, "c1", &input),
        canonical_hash(&fixture, "bytes", &first)
    );
    request["bindings"]["success"]["ops"] = json!([]);
    request["bindings"]["success"]["term"][1] = json!(["hash", ["hash", "x"]]);
    let report = check(&fixture, &request, &json!({})).unwrap();
    let hashes = &connection(&report)["hashes"];
    assert_eq!(hashes.as_array().unwrap().len(), 2);
    assert_eq!(hashes[0]["at"], "/bindings/success/term/1/1");
    assert_eq!(hashes[1]["at"], "/bindings/success/term/1");
}

#[test]
fn hash_shape_type_and_checked_route_conflicts_refuse_before_publication() {
    for (ty, expression) in [
        ("i8", json!(["hash"])),
        ("i8", json!(["hash", "x", "x"])),
        ("Cell<i8>", json!(["hash", "x"])),
        ("Tuple<Cell<i8>,bool>", json!(["hash", "x"])),
        ("i8", json!(["hash", ["cell", "x"]])),
        ("i8", json!(["hash?", ["future_expression"]])),
    ] {
        let fixture = Fixture::new();
        let request = request(ty, expression);
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
                .contains("/bindings/success/term")
        );
        for artifact in ["drafts", "candidates", "residual"] {
            assert!(!fixture.dir.join(".sley").join(artifact).exists());
        }
    }
}

#[test]
fn known_bytes_results_never_hide_unresolved_operands_or_guess_integer_widths() {
    let fixture = Fixture::new();
    for operand in [json!(1), json!(["future_expression"]), json!(["vec"])] {
        let request = request("i8", json!(["hash", operand]));
        let report = check(&fixture, &request, &json!({})).unwrap();
        let entry = connection(&report);
        assert_eq!(entry["expression_types"], "partial");
        assert_eq!(entry["hashes"][0]["eligibility"], "unresolved");
        assert_eq!(entry["hashes"][0]["result_type"], "bytes");
        assert!(entry["hashes"][0]["operand_type"].is_null());
    }
    let mut request = request("i8", json!(["hash", ["future_expression"]]));
    request["bindings"]["returns"] = json!("i8");
    assert!(
        check(&fixture, &request, &json!({}))
            .unwrap_err()
            .detail()
            .contains("bytes")
    );
}

#[test]
fn named_hash_eligibility_uses_bound_definitions_and_retains_deferred_shapes() {
    for accepted in [false, true] {
        let fixture = Fixture::new();
        let declarations = json!({"af1":1,"types":[{"name":"Pack","record":[["value","f32"]]}]});
        let (code, result) = cli(
            &fixture.dir,
            &["try", &declarations.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        if accepted {
            let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
            assert_eq!(code, 0, "{result}");
        }
        let mut request = request("Pack", json!(["hash", "x"]));
        if !accepted {
            request["base"] = json!("d1@r1");
        }
        let report = check(
            &fixture,
            &request,
            &if accepted { json!({}) } else { declarations },
        )
        .unwrap();
        assert_eq!(connection(&report)["hashes"][0]["eligibility"], "checked");
        let input = json!({"value":"NaN"});
        let result = execute(&fixture, &request, "c2", &input);
        assert_eq!(result.as_str().unwrap().len(), 66);
        if accepted {
            assert_eq!(result, canonical_hash(&fixture, "Pack", &input));
        }
        let bad = json!({"types":[{"name":"Pack","record":[["value","Cell<i8>"]]}]});
        assert!(
            check(&fixture, &request, &bad)
                .unwrap_err()
                .detail()
                .contains("TYPE_NOT_HASHABLE")
        );
        let malformed = json!({"types":[{"name":"Pack","record":["value"]}]});
        let error = check(&fixture, &request, &malformed).unwrap_err();
        super::interface_closure_tests::assert_incomplete(&error, "/bindings/params/");
    }
}

#[test]
fn hash_named_cycles_and_shared_work_exhaustion_refuse() {
    let fixture = Fixture::new();
    let declarations = json!({"types":[{"name":"Cycle","record":[["again","Cycle"]]}]});
    let cyclic = request("Cycle", json!(["hash", "x"]));
    let error = check(&fixture, &cyclic, &declarations).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    let request = request("i8", json!(["hash", "x"]));
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let parsed = parse_request(&bytes(&request)).unwrap();
    let mut measured = Budget::default();
    interfaces::check(
        head.program(),
        &names,
        &serde_json::Map::new(),
        &parsed,
        &mut measured,
    )
    .unwrap();
    let work = measured.usage()["charged_work"].as_u64().unwrap();
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), work - 1);
    let error = interfaces::check(
        head.program(),
        &names,
        &serde_json::Map::new(),
        &parsed,
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
}
