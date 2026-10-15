use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    let source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":[
        {"name":"entry","ops":[],"term":["cond","ready",["yes","x"],["no","x"]]},
        {"name":"yes","params":[["value","i8"]],"ops":[],"term":["return","value"]},
        {"name":"no","params":[["value","i8"]],"ops":[],"term":["return","value"]}]}]});
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    fixture
}

fn source(blocks: &Value, dialect: bool) -> Value {
    let mut patch = json!({"af1":1,"patch":[{"fn":"helper","blocks":blocks}]});
    if dialect {
        patch["afx"] = json!(1);
    }
    patch
}

#[test]
fn retained_edges_connect_to_changed_destination_arity_and_types_before_generation() {
    let fixture = fixture();
    let request = predicate_request(json!(false));
    for dialect in [false, true] {
        for replacement in [
            json!({"params":[["value","bool"]],"ops":[["out","const",{"type":"i8","value":9}]],"term":["return","out"]}),
            json!({"params":[],"ops":[],"term":["return","x"]}),
            json!({"params":[["value","i8"],["extra","i8"]],"ops":[],"term":["return","value"]}),
        ] {
            let patch = source(&json!({"yes":replacement}), dialect);
            let error = check(&fixture, &request, &patch).unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
            assert!(
                error.detail().contains("/patch/0")
                    && error.detail().contains("yes")
                    && error.detail().contains("argument"),
                "{error}"
            );
            assert!(
                error.detail().contains("retained block `helper.entry`"),
                "{error}"
            );
        }
    }
}

fn calling_request(returns: &str) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!(returns);
    request["bindings"]["success"] =
        json!({"ops":[],"term":["return",["call","helper","x","ready"]]});
    request
}

#[test]
fn authored_patch_edges_use_retained_block_parameter_types_and_function_overlay() {
    let fixture = fixture();
    for dialect in [false, true] {
        for (term, pointer) in [
            (json!(["br", "yes", "ready"]), "/term/2"),
            (json!(["jump", ["yes", "x", "x"]]), "/term/1"),
        ] {
            let patch = source(&json!({"entry":{"ops":[],"term":term}}), dialect);
            let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
            assert!(
                error
                    .detail()
                    .contains(&format!("/patch/0/blocks/entry{pointer}"))
                    && error.detail().contains("yes"),
                "{error}"
            );
        }
        let mut patch = source(&json!({}), dialect);
        patch["patch"][0]["params"] = json!([["x", "bool"], ["ready", "bool"]]);
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("retained block `helper.entry`")
                && error.detail().contains("type bool")
                && error.detail().contains("requires i8"),
            "{error}"
        );
        patch["patch"][0]["params"] = json!([["other", "i8"], ["ready", "bool"]]);
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("helper.x") && error.detail().contains("original identity"),
            "{error}"
        );
    }
}

#[test]
fn destination_rename_and_function_parameter_reorder_preserve_compilation_and_vm_values() {
    for dialect in [false, true] {
        for reorder in [false, true] {
            let fixture = fixture();
            let mut patch = source(
                &json!({"yes":{"params":[["renamed","i8"]],"ops":[],"term":["return","renamed"]}}),
                dialect,
            );
            let mut request = calling_request("i8");
            if reorder {
                patch["patch"][0]["params"] = json!([["ready", "bool"], ["x", "i8"]]);
                request["bindings"]["success"]["term"] =
                    json!(["return", ["call", "helper", "ready", "x"]]);
            }
            let report = check(&fixture, &request, &patch).unwrap();
            assert_eq!(report["composition"], "partial");
            assert!(
                report["declared_body_control_flow"]["checked_functions"][0]["edge_arguments"]
                    .as_array()
                    .unwrap()
                    .len()
                    >= 2
            );
            let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
            assert_eq!(code, 0, "{result}");
            request["base"] = json!("d2@r1");
            let (code, result) = cli(
                &fixture.dir,
                &["residual", "try", &request.to_string(), "--no-test"],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["kernel"], "valid");
            for x in [-128, 0, 127] {
                for ready in [false, true] {
                    let (code, result) = cli(
                        &fixture.dir,
                        &[
                            "call",
                            "checked",
                            &x.to_string(),
                            &ready.to_string(),
                            "--on",
                            "c3",
                        ],
                    );
                    assert_eq!(code, 0, "{result}");
                    assert_eq!(result["result"], json!(x));
                }
            }
        }
    }
}

fn operation_fixture() -> Fixture {
    let fixture = Fixture::new();
    let frame = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":[
        {"name":"start","ops":[["value","const",{"type":"i8","value":9}]],"term":["br","entry"]},
        {"name":"entry","ops":[],"term":["cond","ready",["yes","start.value"],["no","start.value"]]},
        {"name":"yes","params":[["value","i8"]],"ops":[],"term":["return","value"]},
        {"name":"no","params":[["value","i8"]],"ops":[],"term":["return","value"]}]}]});
    let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    fixture
}

#[test]
fn retained_operation_results_use_new_types_without_rebinding_removed_identities() {
    for dialect in [false, true] {
        let fixture = operation_fixture();
        let mut patch = source(
            &json!({"start":{"ops":[["value","const",true]],"term":["br","entry"]}}),
            dialect,
        );
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("retained block `helper.entry`")
                && error.detail().contains("type bool"),
            "{error}"
        );
        patch["patch"][0]["blocks"]["start"]["ops"] =
            json!([["replacement","const",{"type":"i8","value":9}]]);
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("helper.start.value")
                && error.detail().contains("original identity"),
            "{error}"
        );
        patch["patch"][0]["blocks"]["start"]["ops"] = json!([["value", "const", true]]);
        for name in ["yes", "no"] {
            patch["patch"][0]["blocks"][name] =
                json!({"params":[["value","bool"]],"ops":[],"term":["return","value"]});
        }
        patch["patch"][0]["returns"] = json!("bool");
        let mut request = calling_request("bool");
        let report = check(&fixture, &request, &patch).unwrap();
        assert_eq!(report["composition"], "partial");
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        request["base"] = json!("d2@r1");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        for ready in [false, true] {
            let (code, result) = cli(
                &fixture.dir,
                &["call", "checked", "-7", &ready.to_string(), "--on", "c3"],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], true);
        }
    }
}

#[test]
fn retained_switch_payloads_follow_current_option_result_and_named_variant_types() {
    for kind in 0..3 {
        let fixture = Fixture::new();
        let (ty, first, second) = match kind {
            0 => ("Option<i8>", "Some", "None"),
            1 => ("Result<i8,bool>", "Ok", "Err"),
            _ => ("Choice", "Present", "Empty"),
        };
        let mut frame = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"],["item",ty]],"returns":"i8","blocks":[
            {"name":"entry","ops":[],"term":["switch","item",[first,"yes","$"],[second,"no","x"]]},
            {"name":"yes","params":[["value","i8"]],"ops":[],"term":["return","value"]},
            {"name":"no","params":[["value","i8"]],"ops":[],"term":["return","value"]}]}]});
        if kind == 2 {
            frame["types"] = json!([{"name":"Choice","variant":[["Present","i8"],["Empty",null]]}]);
        }
        let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{result}");
        for dialect in [false, true] {
            let mut patch = source(&json!({}), dialect);
            if kind == 2 {
                patch["types"] =
                    json!([{"name":"Choice","variant":[["Present","bool"],["Empty",null]]}]);
            } else {
                patch["patch"][0]["params"] = json!([
                    ["x", "i8"],
                    ["ready", "bool"],
                    [
                        "item",
                        if kind == 0 {
                            "Option<bool>"
                        } else {
                            "Result<bool,bool>"
                        }
                    ]
                ]);
            }
            let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
            assert!(
                error.detail().contains("retained block `helper.entry`")
                    && error.detail().contains("requires i8"),
                "kind {kind}: {error}"
            );
        }
    }
}

fn inventory(root: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        path: &std::path::Path,
        out: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(path).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(p).unwrap(),
                );
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn malformed_patch_draft_refusal_preserves_all_state_except_the_event_journal() {
    let fixture = fixture();
    let patch = source(
        &json!({"yes":{"params":[["value","bool"]],"ops":[["out","const",{"type":"i8","value":9}]],"term":["return","out"]}}),
        false,
    );
    let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
    assert_eq!(code, 1, "{result}");
    assert_eq!(result["draft"], "d2@r1");
    let before = inventory(&fixture.dir);
    let mut request = calling_request("i8");
    request["base"] = json!("d2@r1");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    assert!(
        result["detail"]
            .as_str()
            .unwrap()
            .contains("retained block `helper.entry`")
    );
    let after = inventory(&fixture.dir);
    for (path, bytes) in &before {
        if path != std::path::Path::new(".sley/events.jsonl") {
            assert_eq!(after.get(path), Some(bytes), "{}", path.display());
        }
    }
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );
    assert!(!fixture.dir.join(".sley/drafts/d2/r2").exists());
    assert!(!fixture.dir.join(".sley/candidates/c3.hex").exists());
    assert!(!fixture.dir.join(".sley/residual").exists());
}

#[test]
fn checked_replacement_keeps_generation_deferred_and_never_moves_old_result_identity() {
    use sley_agent::names::{NameMap, Names};
    let fixture = operation_fixture();
    let mut patch = source(
        &json!({"start":{"ops":[["value","some?","x"]],"term":["br","entry"]}}),
        true,
    );
    patch["patch"][0]["returns"] = json!("Option<i8>");
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let expanded = sley_agent::afx::expand(head.program(), &names, &patch).unwrap();
    assert!(
        expanded
            .obligations
            .iter()
            .any(|o| o.decision.contains("reads `start.value`") && o.decision.contains("removes")),
        "{:?}",
        expanded.obligations
    );
    let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
    assert!(
        error.detail().contains("helper.start.value")
            && error.detail().contains("original identity"),
        "{error}"
    );
    let fixture = Fixture::new();
    let definition = json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"Option<i8>","blocks":[
        {"name":"entry","ops":[["value","some?","x"]],"term":["return",["some","value"]]}]}]});
    let (code, result) = cli(&fixture.dir, &["try", &definition.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    let patch = source(
        &json!({"entry":{"ops":[["value","some?","x"]],"term":["return",["some",["tuple_get",0,["tuple","value","ready"]]]]}}),
        true,
    );
    let mut request = calling_request("Option<i8>");
    let report = check(&fixture, &request, &patch).unwrap();
    assert_eq!(report["composition"], "partial");
    let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    request["base"] = json!("d2@r1");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for x in [-128, 0, 127] {
        for ready in [false, true] {
            let (code, result) = cli(
                &fixture.dir,
                &[
                    "call",
                    "checked",
                    &x.to_string(),
                    &ready.to_string(),
                    "--on",
                    "c3",
                ],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], json!({"Some":x}));
        }
    }
}
