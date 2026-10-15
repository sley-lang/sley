use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli};
use serde_json::{Value, json};
use sley_agent::names::{NameMap, Names};
use sley_agent::{AgentErrorCode, candidate, genesis};
use sley_mutate::{MutationPayload, value::EntityBodyValue};
use sley_ssmc::{FunctionType, TypeDefForm, TypeExpr, TypeParameterDef};

/// Publish a kernel-validated generic definition, rather than a synthetic graph.
fn fixture(payload: TypeExpr, parameters: u32) -> Fixture {
    let fixture = Fixture::new();
    let source = json!({"af1":1,"types":[{"name":"GenericChoice","variant":[["Present","i8"],["Empty",null]]}]});
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let id = names.resolve("GenericChoice").unwrap();
    let mut body = head.program().body(&id).unwrap().clone();
    let EntityBodyValue::TypeDef(definition) = &mut body else {
        panic!("type definition")
    };
    definition.type_parameters = (0..parameters)
        .map(|ordinal| TypeParameterDef { ordinal })
        .collect();
    let TypeDefForm::Variant(cases) = &mut definition.form else {
        panic!("variant")
    };
    cases
        .iter_mut()
        .find(|case| case.payload_type.is_some())
        .unwrap()
        .payload_type = Some(payload);
    let authority = candidate::Authority::of(&head).unwrap();
    let candidate = candidate::assemble(
        &head,
        &authority,
        candidate::fresh_nonce().unwrap(),
        vec![candidate::PlannedOp {
            kind: body.kind_tag(),
            target: id,
            payload: MutationPayload::ReplaceEntityVersion(body),
            field_tag: None,
        }],
    )
    .unwrap();
    let validated = candidate::validate(&head, &authority, &candidate.stored_bytes).unwrap();
    assert!(
        candidate::proposed_program(&head, &validated).is_some(),
        "{validated:?}"
    );
    genesis::commit(&head, &fixture.workspace.repo(), &candidate.stored_bytes).unwrap();
    fixture
}

fn source(item: &str, payload: &str, dialect: bool) -> Value {
    let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["item",item],["fallback",payload]],"returns":payload,"blocks":[
        {"name":"entry","ops":[],"term":["switch","item",["Present","yes","$"],["Empty","no","fallback"]]},
        {"name":"yes","params":[["value",payload]],"ops":[],"term":["return","value"]},
        {"name":"no","params":[["value",payload]],"ops":[],"term":["return","value"]}]}]});
    if dialect {
        source["afx"] = json!(1);
    }
    source
}

#[test]
fn generic_nominal_switch_payloads_use_instantiated_types_before_expansion() {
    let fixture = fixture(TypeExpr::TypeParameter(0), 1);
    for dialect in [false, true] {
        for (item, payload) in [("GenericChoice<i8>", "i8"), ("GenericChoice<bool>", "bool")] {
            let source = source(item, payload, dialect);
            let original = source.clone();
            let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
            assert_eq!(report["composition"], "partial");
            assert_eq!(source, original);
            let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["verdict"]["valid"], true, "{result}");
        }
    }
}

#[test]
fn nested_generic_payloads_substitute_parameter_ordinals_recursively() {
    let fixture = fixture(
        TypeExpr::Tuple(vec![
            TypeExpr::TypeParameter(1),
            TypeExpr::Option(Box::new(TypeExpr::TypeParameter(0))),
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Vector(Box::new(TypeExpr::TypeParameter(0)))),
                error: Box::new(TypeExpr::OrderedMap {
                    key: Box::new(TypeExpr::Bool),
                    value: Box::new(TypeExpr::TypeParameter(0)),
                }),
            },
        ]),
        2,
    );
    for dialect in [false, true] {
        for (item, payload) in [
            (
                "GenericChoice<i8,bool>",
                "(bool,Option<i8>,Result<Vec<i8>,Map<bool,i8>>)",
            ),
            (
                "GenericChoice<bool,i8>",
                "(i8,Option<bool>,Result<Vec<bool>,Map<bool,bool>>)",
            ),
        ] {
            let source = source(item, payload, dialect);
            check(&fixture, &predicate_request(json!(false)), &source).unwrap();
            let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["verdict"]["valid"], true, "{result}");
        }
    }
}

#[test]
fn generic_payload_mismatches_and_unit_payload_requests_refuse_at_source_arguments() {
    let fixture = fixture(TypeExpr::TypeParameter(0), 1);
    for dialect in [false, true] {
        let source = source("GenericChoice<bool>", "i8", dialect);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/fns/0/blocks/0/term/2/2")
                && error.detail().contains("type bool")
                && error.detail().contains("requires i8"),
            "{error}"
        );
        let mut source = self::source("GenericChoice<i8>", "i8", dialect);
        source["fns"][0]["blocks"][0]["term"][3][2] = json!("$");
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert!(
            error.detail().contains("/term/3/2") && error.detail().contains("unit case"),
            "{error}"
        );
    }
}

#[test]
fn retained_generic_payloads_follow_patch_type_arguments_and_restatements() {
    let fixture = fixture(TypeExpr::TypeParameter(0), 1);
    let definition = source("GenericChoice<i8>", "i8", false);
    let (code, result) = cli(&fixture.dir, &["try", &definition.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c2"]);
    assert_eq!(code, 0, "{result}");
    for dialect in [false, true] {
        let mut patch = json!({"af1":1,"patch":[{"fn":"helper","blocks":{}}]});
        if dialect {
            patch["afx"] = json!(1);
        }
        // A current definition view must retain accepted parameter ordinals.
        patch["types"] =
            json!([{"name":"GenericChoice","variant":[["Present","$0"],["Empty",null]]}]);
        check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
        patch["patch"][0]["params"] = json!([["item", "GenericChoice<bool>"], ["fallback", "i8"]]);
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("retained block `helper.entry`")
                && error.detail().contains("type bool")
                && error.detail().contains("requires i8"),
            "{error}"
        );
        patch["patch"][0]["params"] = json!([["item", "GenericChoice<i8>"], ["fallback", "i8"]]);
        patch["types"][0]["variant"][0][1] = json!("bool");
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("retained block `helper.entry`")
                && error.detail().contains("type bool"),
            "{error}"
        );
        patch["patch"][0]["params"] =
            json!([["item", "GenericChoice<bool>"], ["fallback", "bool"]]);
        patch["patch"][0]["returns"] = json!("bool");
        patch["patch"][0]["blocks"] = json!({
            "yes":{"params":[["value","bool"]],"ops":[],"term":["return","value"]},
            "no":{"params":[["value","bool"]],"ops":[],"term":["return","value"]}
        });
        check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["verdict"]["valid"], true, "{result}");
    }
}

#[test]
fn generic_function_reference_payloads_substitute_parameters_and_result() {
    let fixture = fixture(
        TypeExpr::FunctionRef(FunctionType {
            parameters: vec![TypeExpr::TypeParameter(1)],
            result: Box::new(TypeExpr::TypeParameter(0)),
            effects: vec![],
        }),
        2,
    );
    for dialect in [false, true] {
        let source = source("GenericChoice<i8,bool>", "fn(bool)->i8", dialect);
        check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["verdict"]["valid"], true, "{result}");
    }
}

#[test]
fn generic_switch_payloads_compile_and_execute_for_parameterized_nominal_types() {
    let fixture = fixture(TypeExpr::TypeParameter(0), 1);
    for dialect in [false, true] {
        let source = source("GenericChoice<i8>", "i8", dialect);
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["verdict"]["valid"], true, "{result}");
        let handle = result["handle"].as_str().unwrap();
        for value in [-128, 0, 127] {
            for (input, expected) in [(json!({"Present":value}), value), (json!("Empty"), 9)] {
                let (code, result) = cli(
                    &fixture.dir,
                    &["call", "helper", &input.to_string(), "9", "--on", handle],
                );
                assert_eq!(code, 0, "{result}");
                assert_eq!(result["result"], json!(expected), "{result}");
            }
        }
    }
}

#[test]
fn incompatible_generic_payload_draft_refuses_without_publishing_a_revision() {
    let fixture = fixture(TypeExpr::TypeParameter(0), 1);
    for dialect in [false, true] {
        let source = source("GenericChoice<bool>", "i8", dialect);
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 1, "{result}");
        let draft = result["draft"].as_str().unwrap();
        let before = inventory(&fixture.dir);
        let mut request = predicate_request(json!(false));
        request["base"] = json!(draft);
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
                .contains("/fns/0/blocks/0/term/2/2"),
            "{result}"
        );
        let after = inventory(&fixture.dir);
        assert_eq!(
            before.keys().collect::<Vec<_>>(),
            after.keys().collect::<Vec<_>>()
        );
        for (path, bytes) in &before {
            if path != std::path::Path::new(".sley/events.jsonl") {
                assert_eq!(after.get(path), Some(bytes), "{}", path.display());
            }
        }
    }
}

fn inventory(root: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        at: &std::path::Path,
        files: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = std::collections::BTreeMap::new();
    walk(root, root, &mut files);
    files
}
