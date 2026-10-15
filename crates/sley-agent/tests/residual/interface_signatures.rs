use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn call_request() -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["success"] =
        json!({"ops":[],"term":["return",["some",["call","echo","x"]]]});
    request
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                result.insert(path.clone(), std::fs::read(path).unwrap());
            }
        }
    }
    result
}

#[test]
fn incomplete_callee_refuses_before_trial_and_ordinary_signature_repair_executes() {
    let fixture = Fixture::new();
    let declarations = json!({"af1":1,"fns":[{"fn":"echo","params":[["z","i8"]],
        "blocks":[{"name":"entry","ops":[],"term":["return","z"]}]}]});
    let (code, source) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{source}");
    let mut request = call_request();
    request["base"] = source["draft"].clone();
    let root = fixture.dir.join(".sley");
    let events_path = root.join("events.jsonl");
    let mut before = snapshot(&root);
    let events_before = before.remove(&events_path).unwrap();
    let (code, refused) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{refused}");
    assert_eq!(
        refused["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
        "{refused}"
    );
    let detail = refused["detail"].as_str().unwrap();
    assert!(
        detail.contains("echo") && detail.contains("/fns/0") && detail.contains("returns"),
        "{refused}"
    );
    assert!(detail.contains("/bindings/success/term/1/1"), "{refused}");
    let mut after = snapshot(&root);
    let events_after = after.remove(&events_path).unwrap();
    assert!(events_after.starts_with(&events_before));
    let event: Value = serde_json::from_slice(&events_after[events_before.len()..]).unwrap();
    assert_eq!(event["refusal"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    assert_eq!(after, before);
    let mut repaired_declaration = declarations["fns"][0].clone();
    repaired_declaration["returns"] = json!("i8");
    let delta = json!({"set":[{"at":"/fns/0","value":repaired_declaration}]}).to_string();
    let (code, repaired) = cli(
        &fixture.dir,
        &["fill", "d1", &delta, "--revision", "1", "--no-test"],
    );
    assert_eq!(code, 0, "{repaired}");
    request["base"] = json!("d1@r2");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for input in ["-7", "19"] {
        let (code, result) = cli(
            &fixture.dir,
            &[
                "call",
                "checked",
                input,
                "false",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(
            result["result"],
            json!({"Some":input.parse::<i8>().unwrap()})
        );
    }
}

#[test]
fn canonical_signature_errors_refuse_calls_and_references_at_use_and_source() {
    let fixture = super::interface_call_tests::fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = sley_agent::names::Names::build(
        head.program(),
        &sley_agent::names::NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let mut reference = call_request();
    reference["bindings"]["returns"] = json!("fn(i8)->i8");
    reference["bindings"]["success"] = json!({"ops":[],"term":["return",["fnref","echo"]]});
    for key in ["fns", "functions", "patch"] {
        for (field, value, diagnostic) in [
            ("params", json!(null), "expected an array"),
            ("params", json!("broken"), "expected an array"),
            ("params", json!([["z"]]), "a parameter is"),
            ("params", json!([["z", "i8", "extra"]]), "a parameter is"),
            ("params", json!([[7, "i8"]]), "expected a string"),
            (
                "params",
                json!([["bad name", "i8"]]),
                "not a fresh parameter name",
            ),
            (
                "params",
                json!([["z", "i8"], ["z", "i8"]]),
                "not a fresh parameter name",
            ),
            ("params", json!([["z", "unknown_type"]]), "unknown"),
            ("returns", json!(null), "type"),
            ("returns", json!("unknown_type"), "unknown"),
        ] {
            let mut decl = json!({"name":"echo","params":[["z","i8"]],"returns":"i8"});
            decl[field] = value;
            if key == "patch" {
                decl["blocks"] = json!({"entry":{"ops":[],"term":["return",0]}});
            }
            let declarations = json!({"af1":1,key:[decl]});
            let ordinary = sley_agent::frame::compile(
                head.program(),
                &names,
                &sley_agent::candidate::Authority::of(&head)
                    .unwrap()
                    .ceilings,
                &declarations,
                sley_agent::candidate::fresh_nonce().unwrap(),
                &mut || Ok([44; 32]),
            )
            .unwrap_err();
            assert_eq!(ordinary.code(), sley_agent::AgentErrorCode::FrameInvalid);
            for request in [call_request(), reference.clone()] {
                let before = (request.clone(), declarations.clone());
                let error = check(&fixture, &request, &declarations).unwrap_err();
                assert_eq!(
                    error.code(),
                    sley_agent::AgentErrorCode::ResidualConstraintConflict
                );
                assert!(
                    error.detail().contains("/bindings/success/term/1"),
                    "{error}"
                );
                assert!(error.detail().contains(&format!("/{key}/0")), "{error}");
                assert!(
                    error.detail().contains("incomplete bound signature"),
                    "{error}"
                );
                assert!(error.detail().contains(diagnostic), "{error}");
                assert!(
                    error.detail().contains(ordinary.detail()),
                    "preflight: {error}; ordinary: {ordinary}"
                );
                assert_eq!((request, declarations.clone()), before);
            }
        }
    }
}

#[test]
fn incomplete_callee_in_source_helper_body_refuses_at_the_authored_call() {
    let fixture = Fixture::new();
    let declarations = json!({"af1":1,"afx":1,"fns":[
        {"fn":"echo","params":[["z","i8"]],"blocks":[{"name":"entry","ops":[],"term":["return","z"]}]},
        {"fn":"helper","params":[["z","i8"]],"returns":"i8",
         "blocks":[{"name":"entry","ops":[],"term":["return",["call","echo","z"]]}]}
    ]});
    let mut request = call_request();
    request["bindings"]["success"]["term"][1][1][1] = json!("helper");
    let before = declarations.clone();
    let error = check(&fixture, &request, &declarations).unwrap_err();
    assert_eq!(
        error.code(),
        sley_agent::AgentErrorCode::ResidualConstraintConflict
    );
    assert!(error.detail().contains("/fns/1/blocks/0/term/1"), "{error}");
    assert!(
        error.detail().contains("/fns/0: missing \"returns\""),
        "{error}"
    );
    assert_eq!(declarations, before);
}

#[test]
fn absent_parameters_inherited_patch_signatures_and_replaced_targets_remain_supported() {
    let fixture = super::interface_call_tests::fixture(true);
    let mut request = predicate_request(json!(["call", "zero"]));
    let declarations = json!({"fns":[{"fn":"zero","returns":"bool"}]});
    check(&fixture, &request, &declarations).unwrap();
    check(&fixture, &call_request(), &json!({"patch":[{"fn":"echo"}]})).unwrap();
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["success"] =
        json!({"ops":[],"term":["return",["call","checked","x",false]]});
    let declarations = json!({"fns":[{"fn":"checked","params":null}]});
    check(&fixture, &request, &declarations).unwrap();
}
