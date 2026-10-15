use super::interface_tests::predicate_request;
use super::{Fixture, cli};
use serde_json::{Value, json};

fn source(ops: &Value, returned: &str) -> Value {
    json!({"af1":1,"afx":1,"fns":[
        {"fn":"echo","params":[["z","i8"]],"returns":"i8","blocks":[
            {"name":"entry","ops":[],"term":["return","z"]}]},
        {"fn":"helper","params":[["z","i8"],["flag","bool"]],"returns":returned,
         "blocks":[{"name":"entry","ops":ops,"term":["return","value"]}]}]})
}

fn request() -> Value {
    let mut request = predicate_request(json!(false));
    request["base"] = json!("d1@r1");
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("i8");
    request["bindings"]["success"] = json!({"ops":[],"term":["return","x"]});
    request
}

#[test]
fn source_helper_call_types_refuse_before_residual_revision_publication() {
    for (ops, returned) in [
        (json!([["value", "call", "echo", "flag"]]), "i8"),
        (json!([["value", "call", "echo"]]), "i8"),
        (json!([["value", "call", "echo", ["not", "flag"]]]), "i8"),
        (
            json!([{"name":"value","op":"call","args":["echo","z"],"type":"i64"}]),
            "i64",
        ),
    ] {
        let fixture = Fixture::new();
        let (code, result) = cli(
            &fixture.dir,
            &["try", &source(&ops, returned).to_string(), "--no-test"],
        );
        assert_eq!(code, 1, "{result}");
        assert_eq!(
            result["verdict"]["symbol"], "VM_LOWER_SIGNATURE_MISMATCH",
            "{result}"
        );
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
                .contains("/fns/1/blocks/0/ops/0"),
            "{result}"
        );
        assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
        assert_eq!(fixture.workspace.head().unwrap().transaction_id(), head);
    }
}

#[test]
fn source_helper_constant_use_hints_preserve_valid_calls_and_ordinary_vm_results() {
    let fixture = Fixture::new();
    let source = source(
        &json!([["k", "const", 7], ["value", "call", "echo", "k"]]),
        "i8",
    );
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["verdict"]["valid"], true);
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request().to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for z in ["3", "-7"] {
        let (code, called) = cli(
            &fixture.dir,
            &[
                "call",
                "helper",
                z,
                "false",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{called}");
        assert_eq!(called["result"], 7);
    }
}

#[test]
fn source_operation_families_reuse_canonical_signature_refusals_before_writes() {
    // Characterization of the generic signature path, beyond the call RED.
    for (op, returned) in [
        (json!(["value", "not", "z"]), "bool"),
        (json!(["value", "eq", "z", "flag"]), "bool"),
        (
            json!({"name":"value","op":"vec","args":["z","flag"],"type":"Vec<i8>"}),
            "Vec<i8>",
        ),
        (json!(["value", "vec_len", "z"]), "u64"),
        (
            json!({"name":"value","op":"tuple_get","args":[0,"z"],"type":"i8"}),
            "i8",
        ),
        (json!(["value", "map_has", "z", "flag"]), "bool"),
        (
            json!({"name":"value","op":"cell_get","args":["z"],"type":"i8"}),
            "i8",
        ),
        (
            json!({"name":"value","op":"fadd","args":["z","z"],"type":"i8"}),
            "i8",
        ),
    ] {
        let fixture = Fixture::new();
        let ordinary_frame_refusal = matches!(op[1].as_str(), Some("not" | "eq"));
        let (code, result) = cli(
            &fixture.dir,
            &[
                "try",
                &source(&json!([op]), returned).to_string(),
                "--no-test",
            ],
        );
        if ordinary_frame_refusal {
            assert_eq!(code, 2, "{result}");
            assert_eq!(result["error"], "AGENT_FRAME_INVALID", "{result}");
        } else {
            assert_eq!(code, 1, "{result}");
            assert_eq!(
                result["verdict"]["symbol"], "VM_LOWER_SIGNATURE_MISMATCH",
                "{result}"
            );
        }
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
                .contains("VM_LOWER_SIGNATURE_MISMATCH"),
            "{result}"
        );
        assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
        assert_eq!(fixture.workspace.head().unwrap().transaction_id(), head);
    }
}

#[test]
fn source_operation_signatures_keep_qualified_constant_reads_and_vm_values() {
    let fixture = Fixture::new();
    let mut input = source(&json!([]), "i8");
    input["fns"][1]["blocks"] = json!([
        {"name":"entry","ops":[["k","const",7]],"term":["br","tail"]},
        {"name":"tail","ops":[["value","call","echo","entry.k#0"]],"term":["return","value"]}
    ]);
    let (code, result) = cli(&fixture.dir, &["try", &input.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request().to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for z in ["3", "-7"] {
        let (code, called) = cli(
            &fixture.dir,
            &[
                "call",
                "helper",
                z,
                "true",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{called}");
        assert_eq!(called["result"], 7);
    }
}

#[test]
fn authored_patch_operations_check_inherited_signatures_and_preserve_valid_calls() {
    for valid in [false, true] {
        let fixture = Fixture::new();
        let input = source(&json!([["value", "call", "echo", "z"]]), "i8");
        let (code, result) = cli(&fixture.dir, &["try", &input.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{result}");
        let ops = if valid {
            json!([["k", "const", 7], ["value", "call", "echo", "k"]])
        } else {
            json!([["value", "call", "echo", "flag"]])
        };
        let patch = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{"entry":{"ops":ops,"term":["return","value"]}}}]});
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(code, i32::from(!valid), "{result}");
        if !valid {
            assert_eq!(
                result["verdict"]["symbol"], "VM_LOWER_SIGNATURE_MISMATCH",
                "{result}"
            );
        }
        let mut request = request();
        request["base"] = json!("d2@r1");
        let head = fixture.workspace.head().unwrap().transaction_id();
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        if valid {
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["kernel"], "valid");
            for z in ["3", "-7"] {
                let (code, called) = cli(
                    &fixture.dir,
                    &[
                        "call",
                        "helper",
                        z,
                        "false",
                        "--on",
                        result["handle"].as_str().unwrap(),
                    ],
                );
                assert_eq!(code, 0, "{called}");
                assert_eq!(called["result"], 7);
            }
        } else {
            assert_eq!(code, 2, "{result}");
            assert_eq!(
                result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
                "{result}"
            );
            assert!(
                result["detail"]
                    .as_str()
                    .unwrap()
                    .contains("/patch/0/blocks/entry/ops/0"),
                "{result}"
            );
            assert!(!fixture.dir.join(".sley/drafts/d2/r2").exists());
        }
        assert_eq!(fixture.workspace.head().unwrap().transaction_id(), head);
    }
}

#[test]
fn source_call_projection_preserves_generic_headers_and_ordinary_restatement() {
    use sley_agent::{
        AgentErrorCode,
        names::{NameMap, Names},
        residual::{frontier::Budget, interfaces, parse_request},
        workspace::Program,
    };
    use sley_mutate::value::EntityBodyValue;
    let fixture = super::interface_call_tests::fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let id = names.resolve("echo").unwrap();
    let mut record = head.program().object(&id).unwrap().record().clone();
    let EntityBodyValue::Function(body) = &mut record.body else {
        panic!("function")
    };
    // Synthetic metadata exercises the canonical signature projection, not
    // admission of a generic program or a runtime claim.
    body.type_parameters = vec![sley_ssmc::TypeParameterDef { ordinal: 0 }];
    let replacement = sley_mutate::build_entity_object(head.program().epoch(), &record).unwrap();
    let program = Program::new(
        head.program().epoch(),
        head.program().root(),
        head.program().workspace(),
        head.program()
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
    let mut source = json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["z","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[["value","call","echo","z"]],"term":["return","value"]}]}]});
    let request = parse_request(&serde_json::to_vec(&request()).unwrap()).unwrap();
    let error = interfaces::check(
        &program,
        &names,
        source.as_object().unwrap(),
        &request,
        &mut Budget::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(
        error.detail().contains("VM_LOWER_IMMEDIATE_MISMATCH")
            && error.detail().contains("/fns/0/blocks/0/ops/0"),
        "{error}"
    );
    // AF1 restatement deliberately emits an empty generic/effect header.
    source["fns"].as_array_mut().unwrap().push(json!({"fn":"echo","params":[["z","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[],"term":["return","z"]}]}));
    let report = interfaces::check(
        &program,
        &names,
        source.as_object().unwrap(),
        &request,
        &mut Budget::default(),
    )
    .unwrap();
    assert_eq!(
        report["declared_body_control_flow"]["checked_functions"][0]["operation_signatures"][0]["signature"],
        "canonical_vm_signature_checked",
        "{report}"
    );
}
