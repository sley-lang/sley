use super::interface_tests::predicate_request;
use super::{Fixture, cli};
use serde_json::{Value, json};

fn fixture() -> Fixture {
    let f = Fixture::new();
    let source = json!({"af1":1,"afx":1,"fns":[
        {"fn":"echo","params":[["z","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[],"term":["return","z"]}]},
        {"fn":"helper","params":[["z","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[["value","call","echo","z"]],"term":["return","value"]}]}
    ]});
    let (code, result) = cli(&f.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&f.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    f
}
fn request(params: Value, returns: &str) -> Value {
    let mut r = predicate_request(json!(false));
    r["scope"] = json!(["echo"]);
    r["bindings"]["params"] = params;
    r["bindings"]["returns"] = json!(returns);
    r["bindings"]["guards"] = json!([]);
    r["bindings"]["success"] =
        json!({"ops":[["value","const",{"type":returns,"value":7}]],"term":["return","value"]});
    r
}
#[test]
fn retained_accepted_callers_refuse_changed_callee_types_arity_and_results_before_drafts() {
    for (params, returns) in [
        (json!([["z", "bool"]]), "i8"),
        (json!([["z", "i8"], ["ready", "bool"]]), "i8"),
        (json!([["z", "i8"]]), "i64"),
    ] {
        let f = fixture();
        let head = f.workspace.head().unwrap().transaction_id();
        let (code, result) = cli(
            &f.dir,
            &[
                "residual",
                "try",
                &request(params, returns).to_string(),
                "--no-test",
            ],
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
                .contains("helper.entry.value"),
            "{result}"
        );
        assert!(
            result["detail"]
                .as_str()
                .unwrap()
                .contains("VM_LOWER_SIGNATURE_MISMATCH"),
            "{result}"
        );
        assert!(!f.dir.join(".sley/drafts/d2").exists());
        assert_eq!(f.workspace.head().unwrap().transaction_id(), head);
    }
}
#[test]
fn retained_accepted_callers_keep_valid_replacement_vm_results() {
    let f = fixture();
    let mut r = request(json!([["z", "i8"]]), "i8");
    r["bindings"]["success"] = json!({"ops":[],"term":["return","z"]});
    let (code, result) = cli(&f.dir, &["residual", "try", &r.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for z in [3, -7] {
        let (code, called) = cli(
            &f.dir,
            &[
                "call",
                "helper",
                &z.to_string(),
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{called}");
        assert_eq!(called["result"], json!(z));
    }
}
#[test]
fn retained_accepted_callers_refuse_source_callee_patches_before_new_revisions() {
    let f = fixture();
    let source = json!({"af1":1,"afx":1,"patch":[{"fn":"echo","params":[["z","bool"]],"blocks":{"entry":{"ops":[["value","const",{"type":"i8","value":7}]],"term":["return","value"]}}}]});
    let (code, result) = cli(&f.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 1, "{result}");
    assert_eq!(result["verdict"]["symbol"], "VM_LOWER_SIGNATURE_MISMATCH");
    let head = f.workspace.head().unwrap().transaction_id();
    let mut r = request(json!([["z", "i8"]]), "i8");
    r["scope"] = json!(["checked"]);
    r["base"] = json!("d2@r1");
    let (code, result) = cli(&f.dir, &["residual", "try", &r.to_string(), "--no-test"]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    assert!(
        result["detail"]
            .as_str()
            .unwrap()
            .contains("helper.entry.value"),
        "{result}"
    );
    assert!(!f.dir.join(".sley/drafts/d2/r2").exists());
    assert_eq!(f.workspace.head().unwrap().transaction_id(), head);
}

#[test]
fn retained_call_consumers_keep_ordinary_width_refusals_and_explicit_vm_values() {
    for (immediate, authored_use, valid) in [
        (json!(9), false, false),
        (json!(9), true, true),
        (json!({"type":"i8","value":9}), false, true),
    ] {
        let f = Fixture::new();
        let initial = json!({"af1":1,"afx":1,"fns":[
            {"fn":"echo","params":[["z","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[],"term":["return","z"]}]},
            {"fn":"helper","params":[],"returns":"i8","blocks":[
                {"name":"entry","ops":[["k","const",{"type":"i8","value":7}]],"term":["br","tail"]},
                {"name":"tail","ops":[["value","call","echo","entry.k"]],"term":["return","value"]}
            ]}
        ]});
        let (code, result) = cli(&f.dir, &["try", &initial.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(&f.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{result}");
        let mut ops = json!([["k", "const", immediate]]);
        if authored_use {
            ops.as_array_mut()
                .unwrap()
                .push(json!(["unused", "call", "echo", "k"]));
        }
        let source = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{"entry":{"ops":ops,"term":["br","tail"]}}}]});
        let (code, result) = cli(&f.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, i32::from(!valid), "ordinary control: {result}");
        if !valid {
            assert_eq!(result["verdict"]["symbol"], "VM_LOWER_SIGNATURE_MISMATCH");
        }
        let mut r = request(json!([["z", "i8"]]), "i8");
        r["scope"] = json!(["checked"]);
        r["base"] = json!("d2@r1");
        let (code, result) = cli(&f.dir, &["residual", "try", &r.to_string(), "--no-test"]);
        if !valid {
            assert_eq!(code, 2, "{result}");
            assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
            assert!(!f.dir.join(".sley/drafts/d2/r2").exists());
            continue;
        }
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        let (code, called) = cli(
            &f.dir,
            &["call", "helper", "--on", result["handle"].as_str().unwrap()],
        );
        assert_eq!(code, 0, "{called}");
        assert_eq!(called["result"], 9);
    }
}

fn retained_frame(
    types: Value,
    constants: Value,
    params: Value,
    returns: &str,
    ops: Value,
) -> Value {
    let mut frame = json!({"af1":1,"afx":1,"fns":[{"fn":"helper","returns":returns,"blocks":[{"name":"entry","term":["return","value"]}]}]});
    frame["types"] = types;
    frame["consts"] = constants;
    frame["fns"][0]["params"] = params;
    frame["fns"][0]["blocks"][0]["ops"] = ops;
    frame
}
#[test]
fn retained_constants_records_fields_and_variants_check_current_data_and_members() {
    let boxed = json!([{ "name":"Box","record":[["payload","i8"]] }]);
    let choice = json!([{ "name":"Choice","variant":[["Item","i8"],"Empty"] }]);
    for (initial, change) in [
        (
            retained_frame(
                json!([]),
                json!([{ "name":"limit","type":"i8","value":7 }]),
                json!([]),
                "i8",
                json!([["value", "const", "limit"]]),
            ),
            json!({"consts":[{"name":"limit","type":"bool","value":true}]}),
        ),
        (
            retained_frame(
                boxed.clone(),
                json!([]),
                json!([["z", "i8"]]),
                "Box",
                json!([["value", "record", "Box", "z"]]),
            ),
            json!({"types":[{"name":"Box","record":[["payload","bool"]]}]}),
        ),
        (
            retained_frame(
                boxed.clone(),
                json!([]),
                json!([["z", "i8"]]),
                "i8",
                json!([
                    ["packed", "record", "Box", "z"],
                    ["value", "field", "Box.payload", "packed"]
                ]),
            ),
            json!({"types":[{"name":"Box","record":[["replacement","i8"]]}]}),
        ),
        (
            retained_frame(
                choice.clone(),
                json!([]),
                json!([["z", "i8"]]),
                "Choice",
                json!([["value", "variant", "Choice.Item", "z"]]),
            ),
            json!({"types":[{"name":"Choice","variant":[["Item","bool"],"Empty"]}]}),
        ),
        (
            retained_frame(
                choice,
                json!([]),
                json!([["input", "Choice"]]),
                "Option<i8>",
                json!([["value", "variant_get", "Choice.Item", "input"]]),
            ),
            json!({"types":[{"name":"Choice","variant":[["Item","bool"],"Empty"]}]}),
        ),
        (
            retained_frame(
                boxed,
                json!([]),
                json!([["input", "Box"]]),
                "i8",
                json!([["value", "field", "Box.payload", "input"]]),
            ),
            json!({"delete":["Box"]}),
        ),
    ] {
        let f = Fixture::new();
        let (code, result) = cli(&f.dir, &["try", &initial.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(&f.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{result}");
        let mut change = change;
        change["af1"] = json!(1);
        change["afx"] = json!(1);
        let (code, result) = cli(&f.dir, &["try", &change.to_string(), "--no-test"]);
        assert_ne!(code, 0, "{result}");
        let head = f.workspace.head().unwrap().transaction_id();
        let mut r = request(json!([["z", "i8"]]), "i8");
        r["scope"] = json!(["checked"]);
        r["base"] = json!("d2@r1");
        let (code, result) = cli(&f.dir, &["residual", "try", &r.to_string(), "--no-test"]);
        assert_eq!(code, 2, "{result}");
        assert_eq!(
            result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
            "{result}"
        );
        assert!(
            result["detail"]
                .as_str()
                .unwrap()
                .contains("retained operation"),
            "{result}"
        );
        assert!(!f.dir.join(".sley/drafts/d2/r2").exists());
        assert_eq!(f.workspace.head().unwrap().transaction_id(), head);
    }
}
#[test]
fn retained_constant_restatement_keeps_real_new_value_and_caller_execution() {
    let f = Fixture::new();
    let initial = retained_frame(
        json!([]),
        json!([{"name":"limit","type":"i8","value":7}]),
        json!([]),
        "i8",
        json!([["value", "const", "limit"]]),
    );
    let (code, result) = cli(&f.dir, &["try", &initial.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&f.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    let source = json!({"af1":1,"afx":1,"consts":[{"name":"limit","type":"i8","value":9}]});
    let (code, result) = cli(&f.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let mut r = request(json!([["z", "i8"]]), "i8");
    r["scope"] = json!(["checked"]);
    r["base"] = json!("d2@r1");
    let (code, result) = cli(&f.dir, &["residual", "try", &r.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let (code, called) = cli(
        &f.dir,
        &["call", "helper", "--on", result["handle"].as_str().unwrap()],
    );
    assert_eq!(code, 0, "{called}");
    assert_eq!(called["result"], 9);
}

#[test]
fn retained_function_references_keep_actual_callee_identity_and_current_signature() {
    for mode in ["wrong-type", "valid", "recreated"] {
        let f = Fixture::new();
        let initial = json!({"af1":1,"afx":1,"fns":[
            {"fn":"echo","params":[["z","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[],"term":["return","z"]}]},
            {"fn":"helper","params":[["z","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[["reference","fnref","echo"],["value","call","echo","z"]],"term":["return","value"]}]}
        ]});
        let (code, result) = cli(&f.dir, &["try", &initial.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(&f.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{result}");
        let mut r = request(
            json!([["z", if mode == "wrong-type" { "bool" } else { "i8" }]]),
            "i8",
        );
        if mode == "recreated" {
            let source =
                json!({"af1":1,"afx":1,"delete":["echo"],"fns":[initial["fns"][0].clone()]});
            let (code, result) = cli(&f.dir, &["try", &source.to_string(), "--no-test"]);
            assert_ne!(code, 0, "{result}");
            r["base"] = result["draft"].clone();
            r["scope"] = json!(["checked"]);
        }
        let head = f.workspace.head().unwrap().transaction_id();
        let (code, result) = cli(&f.dir, &["residual", "try", &r.to_string(), "--no-test"]);
        if mode == "valid" {
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["kernel"], "valid");
            for z in [3, -7] {
                let (code, called) = cli(
                    &f.dir,
                    &[
                        "call",
                        "helper",
                        &z.to_string(),
                        "--on",
                        result["handle"].as_str().unwrap(),
                    ],
                );
                assert_eq!(code, 0, "{called}");
                assert_eq!(called["result"], 7);
            }
        } else {
            assert_eq!(code, 2, "{result}");
            assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
            assert!(
                result["detail"]
                    .as_str()
                    .unwrap()
                    .contains("helper.entry.reference"),
                "{result}"
            );
            assert!(
                !f.dir
                    .join(if mode == "recreated" {
                        ".sley/drafts/d2/r2"
                    } else {
                        ".sley/drafts/d2"
                    })
                    .exists()
            );
        }
        assert_eq!(f.workspace.head().unwrap().transaction_id(), head);
    }
}
#[test]
fn retained_operations_use_changed_owning_parameters_before_source_patch_expansion() {
    let f = fixture();
    let source =
        json!({"af1":1,"afx":1,"patch":[{"fn":"helper","params":[["z","bool"]],"blocks":{}}]});
    let (code, result) = cli(&f.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 1, "{result}");
    assert_eq!(result["verdict"]["symbol"], "VM_LOWER_SIGNATURE_MISMATCH");
    let mut r = request(json!([["z", "i8"]]), "i8");
    r["scope"] = json!(["checked"]);
    r["base"] = result["draft"].clone();
    let head = f.workspace.head().unwrap().transaction_id();
    let (code, result) = cli(&f.dir, &["residual", "try", &r.to_string(), "--no-test"]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    assert!(
        result["detail"]
            .as_str()
            .unwrap()
            .contains("helper.entry.value"),
        "{result}"
    );
    assert!(!f.dir.join(".sley/drafts/d2/r2").exists());
    assert_eq!(f.workspace.head().unwrap().transaction_id(), head);
}
#[test]
fn retained_globals_use_real_current_data_without_retargeting_deleted_initializers() {
    for mode in ["wrong-type", "recreated", "valid"] {
        let f = super::interface_reference_tests::fixture(true);
        super::interface_reference_tests::add_global(&f);
        let initial = json!({"af1":1,"afx":1,"fns":[{"fn":"global_reader","params":[],"returns":"i8","blocks":[{"name":"entry","ops":[["value","global","bound"]],"term":["return","value"]}]}]});
        let (code, result) = cli(&f.dir, &["try", &initial.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(&f.dir, &["commit", result["handle"].as_str().unwrap()]);
        assert_eq!(code, 0, "{result}");
        let mut source = json!({"af1":1,"afx":1,"consts":[{"name":"limit","type":if mode=="wrong-type" {"bool"}else{"i8"},"value":if mode=="wrong-type" {json!(true)}else{json!(9)}}]});
        if mode == "recreated" {
            source["delete"] = json!(["limit"]);
        }
        let (code, result) = cli(&f.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, i32::from(mode != "valid"), "{result}");
        let mut r = request(json!([["z", "i8"]]), "i8");
        r["scope"] = json!(["checked"]);
        r["base"] = result["draft"].clone();
        let head = f.workspace.head().unwrap().transaction_id();
        let (code, result) = cli(&f.dir, &["residual", "try", &r.to_string(), "--no-test"]);
        if mode == "valid" {
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["kernel"], "valid");
            let (code, called) = cli(
                &f.dir,
                &[
                    "call",
                    "global_reader",
                    "--on",
                    result["handle"].as_str().unwrap(),
                ],
            );
            assert_eq!(code, 0, "{called}");
            assert_eq!(called["result"], 9);
        } else {
            assert_eq!(code, 2, "{result}");
            assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
            assert!(
                result["detail"]
                    .as_str()
                    .unwrap()
                    .contains("global_reader.entry.value"),
                "{result}"
            );
            assert!(!f.dir.join(".sley/drafts/d3/r2").exists());
        }
        assert_eq!(f.workspace.head().unwrap().transaction_id(), head);
    }
}
