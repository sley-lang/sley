use super::interface_tests::predicate_request;
use super::{Fixture, cli};
use serde_json::{Value, json};

fn source(ops: &Value, returned: &str) -> Value {
    json!({"af1":1,"afx":1,"types":[
        {"name":"Box","record":[["payload","i8"]]},
        {"name":"Other","record":[["payload","i8"]]},
        {"name":"Choice","variant":[["Item","i8"],"Empty"]}
    ],"fns":[{"fn":"helper","params":[["z","i8"],["flag","bool"],["other","Other"]],"returns":returned,
        "blocks":[{"name":"entry","ops":ops,"term":["return","value"]}]}]})
}

fn request() -> Value {
    let mut value = predicate_request(json!(false));
    value["base"] = json!("d1@r1");
    value["bindings"]["guards"] = json!([]);
    value["bindings"]["returns"] = json!("i8");
    value["bindings"]["success"] = json!({"ops":[],"term":["return","x"]});
    value
}

#[test]
fn bound_source_record_variant_and_member_conflicts_refuse_before_revision_creation() {
    for (op, returned, symbol) in [
        (
            json!(["value", "record", "Box", "flag"]),
            "Box",
            "VM_LOWER_SIGNATURE_MISMATCH",
        ),
        (
            json!(["value", "record", "Box"]),
            "Box",
            "VM_LOWER_SIGNATURE_MISMATCH",
        ),
        (
            json!(["value", "record", "Box", "z", "z"]),
            "Box",
            "VM_LOWER_SIGNATURE_MISMATCH",
        ),
        (
            json!(["value", "variant", "Choice.Item", "flag"]),
            "Choice",
            "VM_LOWER_SIGNATURE_MISMATCH",
        ),
        (
            json!(["value", "variant", "Choice.Empty", "z"]),
            "Choice",
            "VM_LOWER_SIGNATURE_MISMATCH",
        ),
        (
            json!(["value", "variant", "Item", "flag"]),
            "Choice",
            "VM_LOWER_SIGNATURE_MISMATCH",
        ),
        (
            json!({"name":"value","op":"variant_get","args":["Choice.Item","other"],"type":"Option<i8>"}),
            "Option<i8>",
            "VM_LOWER_SIGNATURE_MISMATCH",
        ),
        (
            json!(["value", "field", "Box.payload", "other"]),
            "i8",
            "VM_LOWER_IMMEDIATE_MISMATCH",
        ),
    ] {
        let fixture = Fixture::new();
        let (code, result) = cli(
            &fixture.dir,
            &[
                "try",
                &source(&json!([op]), returned).to_string(),
                "--no-test",
            ],
        );
        assert_eq!(code, 1, "{result}");
        assert_eq!(result["verdict"]["symbol"], symbol, "{result}");
        let head = fixture.workspace.head().unwrap().transaction_id();
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request().to_string(), "--no-test"],
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
                .contains("/fns/0/blocks/0/ops/0"),
            "{result}"
        );
        assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
        assert_eq!(fixture.workspace.head().unwrap().transaction_id(), head);
    }
}

#[test]
fn bound_source_named_operations_keep_actual_member_and_payload_vm_results() {
    for (ops, returned, kind) in [
        (
            json!([
                ["packed", "record", "Box", "z"],
                ["value", "field", "Box.payload", "packed"]
            ]),
            "i8",
            "scalar",
        ),
        (json!([["value", "record", "Box", "z"]]), "Box", "record"),
        (
            json!([["value", "variant", "Choice.Item", "z"]]),
            "Choice",
            "variant",
        ),
        (
            json!([["value", "variant", "Item", "z"]]),
            "Choice",
            "variant",
        ),
        (
            json!([
                ["packed", "variant", "Choice.Item", "z"],
                ["value", "variant_get", "Choice.Item", "packed"]
            ]),
            "Option<i8>",
            "option",
        ),
        (
            json!([["value", "variant", "Choice.Empty"]]),
            "Choice",
            "unit",
        ),
    ] {
        let fixture = Fixture::new();
        let (code, result) = cli(
            &fixture.dir,
            &["try", &source(&ops, returned).to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request().to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        for z in [3, -7] {
            let (code, called) = cli(
                &fixture.dir,
                &[
                    "call",
                    "helper",
                    &z.to_string(),
                    "false",
                    "{\"payload\":5}",
                    "--on",
                    result["handle"].as_str().unwrap(),
                ],
            );
            assert_eq!(code, 0, "{called}");
            let expected = match kind {
                "scalar" => json!(z),
                "record" => json!({"payload":z}),
                "variant" => json!({"Item":z}),
                "option" => json!({"Some":z}),
                _ => json!("Empty"),
            };
            assert_eq!(called["result"], expected);
        }
    }
}

#[test]
fn source_literal_and_declared_constant_data_refuse_before_residual_revision_creation() {
    let mut declared = source(&json!([["value", "const", "limit"]]), "i8");
    declared["consts"] = json!([{ "name":"limit","type":"i8","value":999 }]);
    for input in [
        declared,
        source(&json!([["value","const",{"type":"i8","value":999}]]), "i8"),
        source(
            &json!([["value","record","Box",{"type":"i8","value":999}]]),
            "Box",
        ),
        source(
            &json!([["value","const",{"type":"Box","value":{"payload":true}}]]),
            "Box",
        ),
    ] {
        let fixture = Fixture::new();
        let (code, result) = cli(&fixture.dir, &["try", &input.to_string(), "--no-test"]);
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_FRAME_INVALID", "{result}");
        let head = fixture.workspace.head().unwrap().transaction_id();
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request().to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(
            result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
            "{result}"
        );
        assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
        assert_eq!(fixture.workspace.head().unwrap().transaction_id(), head);
    }
}

fn endpoint(term: Value, exit: Option<&str>, value: i32) -> Value {
    let returned = if term[0] == "return" {
        "i8"
    } else {
        "Result<i8,Choice>"
    };
    let mut input = source(&json!([]), returned);
    input["fns"][0]["blocks"][0]["term"] = term;
    if let Some(target) = exit {
        input["fns"][0]["blocks"][0]["ops"] = json!([[format!("!{target}"), "if", "flag", value]]);
        if target == "stop" {
            input["fns"][0]["blocks"].as_array_mut().unwrap().push(
                json!({"name":"stop","params":[["payload","i8"]],"ops":[],"term":["ok","payload"]}),
            );
        }
    }
    input
}

#[test]
fn source_terminal_and_conditional_exit_literals_use_ordinary_endpoint_contexts() {
    for (term, exit, at) in [
        (json!(["return", 999]), None, "/term/1"),
        (json!(["ok", 999]), None, "/term/1"),
        (json!(["fail", "Item", 999]), None, "/term/2"),
        (json!(["ok", "z"]), Some("Item"), "/ops/0/3"),
        (json!(["ok", "z"]), Some("stop"), "/ops/0/3"),
    ] {
        let fixture = Fixture::new();
        let input = endpoint(term, exit, 999);
        let (code, result) = cli(&fixture.dir, &["try", &input.to_string(), "--no-test"]);
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_FRAME_INVALID", "{result}");
        let head = fixture.workspace.head().unwrap().transaction_id();
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request().to_string(), "--no-test"],
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
                .contains(&format!("/fns/0/blocks/0{at}")),
            "{result}"
        );
        assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
        assert_eq!(fixture.workspace.head().unwrap().transaction_id(), head);
    }
}

#[test]
fn source_literal_endpoints_and_conditional_exits_keep_vm_results() {
    for (term, exit, expected) in [
        (json!(["return", 7]), None, json!(7)),
        (json!(["ok", 7]), None, json!({"Ok":7})),
        (json!(["fail", "Item", 7]), None, json!({"Err":{"Item":7}})),
        (json!(["ok", "z"]), Some("Item"), json!({"Err":{"Item":7}})),
        (json!(["ok", "z"]), Some("stop"), json!({"Ok":7})),
    ] {
        let fixture = Fixture::new();
        let input = endpoint(term, exit, 7);
        let (code, result) = cli(&fixture.dir, &["try", &input.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request().to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        for flag in [false, true] {
            let (code, called) = cli(
                &fixture.dir,
                &[
                    "call",
                    "helper",
                    "3",
                    &flag.to_string(),
                    "{\"payload\":5}",
                    "--on",
                    result["handle"].as_str().unwrap(),
                ],
            );
            assert_eq!(code, 0, "{called}");
            let expected = if exit.is_some() && !flag {
                json!({"Ok":3})
            } else {
                expected.clone()
            };
            assert_eq!(called["result"], expected, "{called}");
        }
    }
}

fn global_source(returned: &str) -> Value {
    json!({"af1":1,"afx":1,"fns":[{"fn":"global_reader","params":[],"returns":returned,
        "blocks":[{"name":"entry","ops":[{"name":"value","op":"global","args":["bound"],"type":returned}],"term":["return","value"]}]}]})
}

#[test]
fn source_global_immediates_check_real_result_and_initializer_identity() {
    for change in [
        json!({}),
        json!({"delete":["limit"]}),
        json!({"delete":["limit"],"consts":[{"name":"limit","type":"i8","value":9}]}),
    ] {
        let fixture = super::interface_reference_tests::fixture(true);
        super::interface_reference_tests::add_global(&fixture);
        let mut input = global_source(if change.as_object().unwrap().is_empty() {
            "bool"
        } else {
            "i8"
        });
        for (key, value) in change.as_object().unwrap() {
            input[key] = value.clone();
        }
        let (code, result) = cli(&fixture.dir, &["try", &input.to_string(), "--no-test"]);
        assert_ne!(code, 0, "{result}");
        let head = fixture.workspace.head().unwrap().transaction_id();
        let mut request = request();
        request["base"] = json!("d2@r1");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(
            result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
            "{result}"
        );
        assert!(!fixture.dir.join(".sley/drafts/d2/r2").exists());
        assert_eq!(fixture.workspace.head().unwrap().transaction_id(), head);
    }
}

#[test]
fn source_globals_and_restatement_keep_actual_initializer_values() {
    for value in [7, 9] {
        let fixture = super::interface_reference_tests::fixture(true);
        super::interface_reference_tests::add_global(&fixture);
        let mut input = global_source("i8");
        if value == 9 {
            input["consts"] = json!([{"name":"limit","type":"i8","value":value}]);
        }
        let (code, result) = cli(&fixture.dir, &["try", &input.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let mut request = request();
        request["base"] = json!("d2@r1");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        let (code, called) = cli(
            &fixture.dir,
            &[
                "call",
                "global_reader",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{called}");
        assert_eq!(called["result"], json!(value));
    }
}

#[test]
fn accepted_record_constants_preserve_existing_member_identity_when_restatement_is_valid() {
    let fixture = Fixture::new();
    let initial = json!({"af1":1,"afx":1,"types":[{"name":"Box","record":[["payload","i8"]]}],"consts":[{"name":"seed","type":"Box","value":{"payload":7}}]});
    let (code, result) = cli(&fixture.dir, &["try", &initial.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    let input = source(&json!([["value", "const", "seed"]]), "Box");
    let (code, result) = cli(&fixture.dir, &["try", &input.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let mut request = request();
    request["base"] = json!("d2@r1");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let (code, called) = cli(
        &fixture.dir,
        &[
            "call",
            "helper",
            "3",
            "false",
            "{\"payload\":5}",
            "--on",
            result["handle"].as_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{called}");
    assert_eq!(called["result"], json!({"payload":7}));
}
