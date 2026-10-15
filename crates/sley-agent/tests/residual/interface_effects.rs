use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli, interface_call_tests};
use serde_json::{Value, json};
use sley_agent::{
    AgentErrorCode, candidate, frame, hex,
    names::{NameMap, Names},
    residual::{frontier::Budget, interfaces, parse_request},
    workspace::Program,
};
use sley_mutate::{
    MutationPayload,
    value::{EntityBodyValue, EntityIdSet},
};

fn function_reference(word: &str) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("fn(i8)->i8");
    request["bindings"]["success"] = json!({"ops":[],"term":["return",[word,"echo"]]});
    request
}

pub(super) fn effectful_metadata(program: &Program, names: &Names) -> Program {
    let id = names.resolve("echo").unwrap();
    let mut record = program.object(&id).unwrap().record().clone();
    let EntityBodyValue::Function(body) = &mut record.body else {
        panic!("function")
    };
    body.effects =
        EntityIdSet::from_unsorted(vec![sley_id::EntityId::from_bytes([241; 32])]).unwrap();
    let replacement = sley_mutate::build_entity_object(program.epoch(), &record).unwrap();
    Program::new(
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
    )
}

#[test]
fn declared_effect_interfaces_match_actual_definition_and_patch_compiler_output() {
    let fixture = interface_call_tests::fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    // Deliberately synthetic metadata: no effect definition is supplied. This
    // exercises the compiler's interface rule, not kernel admission or execution.
    let program = effectful_metadata(head.program(), &names);
    for key in ["fns", "functions", "patch"] {
        let mut declaration = json!({"fn":"echo","params":[["z","i8"]],"returns":"i8"});
        declaration["blocks"] = if key == "patch" {
            json!({"entry":{"ops":[],"term":["return","z"]}})
        } else {
            json!([{"name":"entry","ops":[],"term":["return","z"]}])
        };
        let frame = json!({"af1":1,key:[declaration]});
        let before = bytes(&frame);
        let compiled = frame::compile(
            &program,
            &names,
            &candidate::Authority::of(&head).unwrap().ceilings,
            &frame,
            candidate::fresh_nonce().unwrap(),
            &mut || Ok([211; 32]),
        )
        .unwrap();
        let functions: Vec<_> = compiled
            .ops
            .iter()
            .filter_map(|op| match &op.payload {
                MutationPayload::CreateEntity(EntityBodyValue::Function(body))
                | MutationPayload::ReplaceEntityVersion(EntityBodyValue::Function(body)) => {
                    Some(body)
                }
                _ => None,
            })
            .collect();
        assert_eq!(functions.len(), 1);
        assert!(functions[0].effects.as_slice().is_empty());
        let report = interfaces::check(
            &program,
            &names,
            frame.as_object().unwrap(),
            &parse_request(&bytes(&function_reference("fnref"))).unwrap(),
            &mut Budget::default(),
        )
        .unwrap();
        let reference = report["connections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["at"] == "/bindings/success/term/1")
            .unwrap();
        assert_eq!(reference["expression_types"], "connections_checked");
        assert_eq!(reference["references"][0]["source"], "declared_function");
        assert_eq!(reference["references"][0]["effect_ids"], json!([]));
        assert_eq!(bytes(&frame), before);
    }
}

#[test]
fn draft_reference_aliases_check_complete_signatures_before_publication() {
    let fixture = interface_call_tests::fixture(false);
    for word in ["fnref", "function_ref", "194"] {
        let mut request = function_reference(word);
        request["base"] = json!("d1@r1");
        for returns in ["fn(bool)->i8", "fn(i8)->bool", "fn()->i8"] {
            request["bindings"]["returns"] = json!(returns);
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
                    .contains("/bindings/success/term/1"),
                "{result}"
            );
        }
    }
    for path in [
        ".sley/candidates/c2.hex",
        ".sley/drafts/d2",
        ".sley/residual",
    ] {
        assert!(!fixture.dir.join(path).exists(), "{path}");
    }
}

#[test]
fn incomplete_draft_signature_refuses_despite_known_effect_declaration() {
    let fixture = Fixture::new();
    let declarations = json!({"fns":[{"fn":"echo","params":[["z","i8"]]}]});
    let error = check(&fixture, &function_reference("fnref"), &declarations).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(
        error.detail().contains("/bindings/success/term/1/1"),
        "{error}"
    );
    assert!(
        error.detail().contains("/fns/0: missing \"returns\""),
        "{error}"
    );
    let mut deleted = declarations;
    deleted.as_object_mut().unwrap().remove("fns");
    deleted["delete"] = json!(["echo"]);
    let error = check(&fixture, &function_reference("fnref"), &deleted).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
}

#[test]
fn recursive_target_call_and_reference_use_the_emitted_interface_and_execute() {
    let fixture = Fixture::new();
    let mut request = predicate_request(json!(["not", "ready"]));
    request["bindings"]["success"] = json!({"ops":[["self","fnref","checked"]],
        "term":["return",["call","checked","x",false]]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    let connections = report["connections"].as_array().unwrap();
    let reference = connections
        .iter()
        .find(|entry| entry["at"] == "/bindings/success/ops/0")
        .unwrap();
    assert_eq!(reference["expression_types"], "connections_checked");
    assert_eq!(reference["references"][0]["effect_ids"], json!([]));
    let call = connections
        .iter()
        .find(|entry| entry["at"] == "/bindings/success/term/1")
        .unwrap();
    assert_eq!(call["calls"][0]["effects"], "empty_declared_effect_set");
    assert_eq!(
        call["calls"][0]["effect_body_validation"],
        "ordinary_compiler_and_kernel"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let stored = std::fs::read_to_string(fixture.dir.join(".sley/candidates/c1.hex")).unwrap();
    let candidate = sley_mutate::import_candidate(&hex::decode(stored.trim()).unwrap()).unwrap();
    let program =
        candidate::applied_program(&fixture.workspace.read_head().unwrap(), &candidate).unwrap();
    let functions: Vec<_> = program
        .objects()
        .iter()
        .filter_map(|object| match &object.record().body {
            EntityBodyValue::Function(body) => Some(body),
            _ => None,
        })
        .collect();
    assert_eq!(functions.len(), 1);
    assert!(functions[0].effects.as_slice().is_empty());
    for ready in ["false", "true"] {
        let (code, result) = cli(&fixture.dir, &["call", "checked", "7", ready, "--on", "c1"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], "None");
    }
}
