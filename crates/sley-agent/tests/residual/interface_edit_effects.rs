use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli, interface_call_tests, interface_effect_tests};
use serde_json::{Value, json};
use sley_agent::{
    AgentErrorCode, candidate, frame,
    names::{NameMap, Names},
    residual::{frontier::Budget, interfaces, parse_request},
};
use sley_mutate::{MutationPayload, value::EntityBodyValue};

fn fixture() -> Fixture {
    let fixture = interface_call_tests::fixture(true);
    let source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"i8","blocks":[
        {"name":"entry","ops":[["n","const",{"type":"i8","value":0}]],"term":["br","tail"]},
        {"name":"tail","ops":[["pad","const",{"type":"i8","value":0}],["out","call","echo","x"]],"term":["return","out"]}]}]});
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c2"]);
    assert_eq!(code, 0, "{result}");
    fixture
}

fn edit(path: &str, with: &Value) -> Value {
    json!({"af1":1,"afx":1,"edit":[{"fn":"helper","replace_op":path,"with":with}]})
}

fn metadata_check(fixture: &Fixture, source: &Value) -> sley_agent::Result<Value> {
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    // Synthetic nonempty callee effect set: interface-only, never executed.
    let program = interface_effect_tests::effectful_metadata(head.program(), &names);
    interfaces::check(
        &program,
        &names,
        source.as_object().unwrap(),
        &parse_request(&bytes(&predicate_request(json!(false)))).unwrap(),
        &mut Budget::default(),
    )
}

#[test]
fn edits_remove_replaced_effectful_calls_but_check_surviving_and_new_calls() {
    let fixture = fixture();
    let source = edit("tail.out", &json!(["const",{"type":"i8","value":9}]));
    let report = metadata_check(&fixture, &source).unwrap();
    assert_eq!(report["declared_body_effects"]["status"], "checked");
    let body = &report["declared_body_effects"]["checked_functions"][0];
    assert_eq!(body["source"], "accepted_graph_edit");
    assert_eq!(body["calls"], json!([]));
    for path in ["entry.n", "tail.pad"] {
        let source = edit(path, &json!(["const",{"type":"i8","value":9}]));
        let before = bytes(&source);
        let error = metadata_check(&fixture, &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/edit/0") && error.detail().contains("helper.tail.out"),
            "{error}"
        );
        assert_eq!(bytes(&source), before);
    }
    let source = edit("tail.out", &json!(["call_direct", "echo", "x"]));
    let error = metadata_check(&fixture, &source).unwrap_err();
    assert!(error.detail().contains("/edit/0/with"), "{error}");
}

#[test]
fn edited_blocks_restate_call_names_while_untouched_blocks_keep_entity_bindings() {
    let fixture = fixture();
    for (path, binding, restated) in [
        ("entry.n", "retained_entity", false),
        ("tail.pad", "authoring_name", true),
    ] {
        let source = edit(path, &json!(["const",{"type":"i8","value":9}]));
        let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        let call = &report["declared_body_effects"]["checked_functions"][0]["calls"][0];
        assert_eq!(call["callee_binding"], binding);
        assert_eq!(call["restated_operation"], restated);
        assert_eq!(call["retained_operation"]["name"], "helper.tail.out");
    }
    for (path, restated) in [("entry.n", false), ("tail.pad", true)] {
        let scenario = self::fixture();
        let mut source = edit(path, &json!(["const", {"type":"i8","value":9}]));
        source["delete"] = json!(["echo"]);
        source["fns"] = json!([{"fn":"echo","params":[["z","i8"]],"returns":"i8","blocks":[
            {"name":"entry","ops":[["n","const",{"type":"i8","value":12}]],"term":["return","n"]}]}]);
        let preflight = check(&scenario, &predicate_request(json!(false)), &source);
        if restated {
            assert!(preflight.is_ok(), "{preflight:?}");
        } else {
            assert!(
                preflight
                    .unwrap_err()
                    .detail()
                    .contains("original identity")
            );
        }
        // Compare against real ordinary authoring: only the restated block's
        // existing call can resolve the newly created function name.
        let (code, result) = cli(&scenario.dir, &["try", &source.to_string(), "--no-test"]);
        if restated {
            assert_eq!(code, 0, "{result}");
            let (code, result) = cli(&scenario.dir, &["call", "helper", "7", "--on", "c3"]);
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], json!(12));
        } else {
            assert_ne!(code, 0, "{result}");
            assert_ne!(result["kernel"], "valid", "{result}");
        }
    }
}

#[test]
fn edit_owner_effect_interface_matches_actual_compiler_restatement() {
    let fixture = fixture();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let program = interface_effect_tests::effectful_metadata(head.program(), &names);
    // Give the edited owner its own synthetic nonempty effect declaration.
    // Compare actual compiler metadata without claiming kernel admission.
    let id = names.resolve("helper").unwrap();
    let mut record = program.object(&id).unwrap().record().clone();
    let EntityBodyValue::Function(body) = &mut record.body else {
        panic!("function")
    };
    body.effects =
        sley_mutate::value::EntityIdSet::from_unsorted(vec![sley_id::EntityId::from_bytes(
            [245; 32],
        )])
        .unwrap();
    let replacement = sley_mutate::build_entity_object(program.epoch(), &record).unwrap();
    let program = sley_agent::workspace::Program::new(
        program.epoch(),
        program.root(),
        program.workspace(),
        program
            .objects()
            .iter()
            .map(|object| {
                if object.record().entity_id == id {
                    replacement.clone()
                } else {
                    object.clone()
                }
            })
            .collect(),
    );
    let source = edit("tail.out", &json!(["const",{"type":"i8","value":9}]));
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("i8");
    request["bindings"]["success"] = json!({"ops":[],"term":["return",["call","helper","x"]]});
    let report = interfaces::check(
        &program,
        &names,
        source.as_object().unwrap(),
        &parse_request(&bytes(&request)).unwrap(),
        &mut Budget::default(),
    )
    .unwrap();
    let calls = report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["at"] == "/bindings/success/term/1")
        .unwrap();
    assert_eq!(calls["calls"][0]["effects"], "empty_declared_effect_set");
    let compiled = frame::compile(
        &program,
        &names,
        &candidate::Authority::of(&head).unwrap().ceilings,
        &source,
        candidate::fresh_nonce().unwrap(),
        &mut || Ok([211; 32]),
    )
    .unwrap();
    let functions: Vec<_> = compiled
        .ops
        .iter()
        .filter_map(|op| match &op.payload {
            MutationPayload::ReplaceEntityVersion(EntityBodyValue::Function(body)) => Some(body),
            _ => None,
        })
        .collect();
    assert_eq!(functions.len(), 1);
    assert!(functions[0].effects.as_slice().is_empty());
}

#[test]
fn grouped_plain_edits_compose_and_run_as_a_residual_source_draft() {
    let fixture = fixture();
    let mut source = edit("tail.out", &json!(["const",{"type":"i8","value":9}]));
    source["edit"].as_array_mut().unwrap().push(json!({"function":"helper","replace_op":"entry.n","with":["const",{"type":"i8","value":3}]}));
    assert_eq!(
        metadata_check(&fixture, &source).unwrap()["declared_body_effects"]["status"],
        "checked"
    );
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let mut request = predicate_request(json!(false));
    request["base"] = json!("d3@r1");
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("i8");
    request["bindings"]["success"] = json!({"ops":[],"term":["return",["call","helper","x"]]});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "7", "true", "--on", "c4"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], json!(9));
}

#[test]
fn invalid_duplicate_and_extended_operation_edits_cannot_claim_checked_effects() {
    let fixture = fixture();
    let mut duplicate = edit("tail.out", &json!(["const",{"type":"i8","value":9}]));
    let first = duplicate["edit"][0].clone();
    duplicate["edit"].as_array_mut().unwrap().push(first);
    for source in [
        duplicate,
        edit("missing.out", &json!(["call", "echo", "x"])),
    ] {
        let error = metadata_check(&fixture, &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    }
    for source in [
        edit("tail.out", &json!(["call?", "echo", "x"])),
        edit("tail.out", &json!(["call", "echo", ["call", "echo", "x"]])),
    ] {
        let report = metadata_check(&fixture, &source).unwrap();
        assert_eq!(report["declared_body_effects"]["status"], "partial");
    }
    let mut combined = edit("tail.out", &json!(["const",{"type":"i8","value":9}]));
    combined["patch"] = json!([{"fn":"helper","blocks":{}}]);
    let error = metadata_check(&fixture, &combined).unwrap_err();
    assert!(
        error.detail().contains("restated more than once"),
        "{error}"
    );
    assert!(error.detail().contains("/patch/0"), "{error}");
    assert!(error.detail().contains("/edit/0"), "{error}");
}

#[test]
fn edit_target_conflicts_match_ordinary_refusal_without_changing_source() {
    let replacement = json!(["const",{"type":"i8","value":9}]);
    for scenario in 0..8 {
        let fixture = fixture();
        let mut source = edit("tail.out", &replacement);
        let at = match scenario {
            0 => {
                source["edit"][0]["fn"] = json!("absent");
                "/edit/0"
            }
            1 => {
                source["delete"] = json!(["helper"]);
                "/edit/0"
            }
            2 => {
                source["edit"][0]["replace_op"] = json!("missing.out");
                "/edit/0/replace_op"
            }
            3 => {
                source["edit"][0]["replace_op"] = json!("tail.absent");
                "/edit/0/replace_op"
            }
            4 => {
                source["edit"][0]["replace_op"] = json!("out");
                "/edit/0/replace_op"
            }
            5 => {
                source["edit"][0]["replace_op"] = json!(true);
                "/edit/0/replace_op"
            }
            6 => {
                source["edit"][0].as_object_mut().unwrap().remove("with");
                "/edit/0/with"
            }
            _ => {
                source["edit"][0]["with"] = json!(true);
                "/edit/0/with"
            }
        };
        let before = source.clone();
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains(at), "scenario {scenario}: {error}");
        let (code, ordinary) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_ne!(code, 0, "scenario {scenario}: {ordinary}");
        assert_eq!(source, before);
    }
}

#[test]
fn duplicate_targets_are_resolved_before_unknown_replacement_inventory() {
    for unknown in [false, true] {
        let fixture = fixture();
        let mut source = edit("tail.out", &json!(["const",{"type":"i8","value":9}]));
        if unknown {
            source["edit"][0]["with"] = json!(["future_operation"]);
        }
        source["edit"].as_array_mut().unwrap().push(json!({"name":"helper","replace_op":"tail.out","with":["const",{"type":"i8","value":3}]}));
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        for text in [
            "edited twice",
            "/edit/0/replace_op",
            "/edit/1/replace_op",
            "helper.tail.out",
        ] {
            assert!(error.detail().contains(text), "{error}");
        }
        if !unknown {
            let (code, ordinary) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
            assert_ne!(code, 0, "{ordinary}");
            assert!(ordinary.to_string().contains("edited twice"), "{ordinary}");
        }
    }
}

#[test]
fn edit_target_evidence_identifies_exact_accepted_operations_and_retains_deferral() {
    let fixture = fixture();
    let source = edit("tail.out", &json!(["future_operation"]));
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    let body = &report["declared_body_effects"]["checked_functions"][0];
    assert_eq!(body["status"], "partial");
    let target = &body["edit_targets"][0];
    assert_eq!(target["at"], "/edit/0/replace_op");
    assert_eq!(target["operation"], "helper.tail.out");
    assert_eq!(target["authority"], "none");
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    for (key, name) in [
        ("owner_id", "helper"),
        ("block_id", "helper.tail"),
        ("operation_id", "helper.tail.out"),
    ] {
        assert_eq!(
            target[key],
            sley_agent::hex::encode(names.resolve(name).unwrap().as_bytes())
        );
    }
}

#[test]
fn duplicate_function_restatements_match_ordinary_refusal_and_locate_both_sources() {
    let definition = json!({"name":"helper","params":[["x","i8"]],"returns":"i8",
        "blocks":[{"name":"entry","ops":[],"term":["return","x"]}]});
    let patch = json!({"name":"helper","blocks":{}});
    let edit = json!({"function":"helper","replace_op":"tail.out","with":["const",{"type":"i8","value":9}]});
    for (source, left, right) in [
        (
            json!({"af1":1,"fns":[definition],"patch":[patch]}),
            "/fns/0",
            "/patch/0",
        ),
        (
            json!({"af1":1,"functions":[definition],"edit":[edit]}),
            "/functions/0",
            "/edit/0",
        ),
        (
            json!({"af1":1,"patch":[patch],"edit":[edit]}),
            "/patch/0",
            "/edit/0",
        ),
        (
            json!({"af1":1,"patch":[patch,patch]}),
            "/patch/0",
            "/patch/1",
        ),
        (
            json!({"af1":1,"fns":[definition],"functions":[definition]}),
            "/fns/0",
            "/functions/0",
        ),
    ] {
        let fixture = fixture();
        let before = source.clone();
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        for text in ["helper", left, right, "restated more than once"] {
            assert!(error.detail().contains(text), "{error}");
        }
        let (code, ordinary) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_ne!(code, 0, "{ordinary}");
        let detail = ordinary.to_string();
        assert!(
            detail.contains("restated more than once") || detail.contains("declared twice"),
            "{ordinary}"
        );
        assert_eq!(source, before);
    }
}

#[test]
fn transformations_on_distinct_owners_and_grouped_edits_still_compile_and_execute() {
    let fixture = fixture();
    let mut source = edit("tail.out", &json!(["const",{"type":"i8","value":9}]));
    source["edit"].as_array_mut().unwrap().push(
        json!({"name":"helper","replace_op":"entry.n","with":["const",{"type":"i8","value":3}]}),
    );
    source["patch"] = json!([{"fn":"echo","blocks":{}}]);
    source["functions"] = json!([{"fn":"other","params":[["x","i8"]],"returns":"i8",
        "blocks":[{"name":"entry","ops":[],"term":["return","x"]}]}]);
    let before = source.clone();
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    assert_eq!(report["declared_body_effects"]["status"], "checked");
    assert_eq!(
        report["declared_body_effects"]["checked_functions"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    for (function, expected) in [("helper", 9), ("other", 7), ("echo", 7)] {
        let (code, result) = cli(&fixture.dir, &["call", function, "7", "--on", "c3"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
    assert_eq!(source, before);
}

#[test]
fn edit_replacements_cannot_introduce_excluded_opcodes() {
    let fixture = fixture();
    for word in [
        "effect",
        "effect_request",
        "160",
        "adapter",
        "assert",
        "observe",
        "narrow",
    ] {
        let source = edit("tail.out", &json!([word, "unbound", "x"]));
        let error = metadata_check(&fixture, &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualFragmentShape);
        assert!(error.detail().contains("/edit/0/with"), "{error}");
    }
}
