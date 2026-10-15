use super::Fixture;
use super::interface_tests::{check, predicate_request};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn helper(term: Value, dialect: bool) -> Value {
    let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":[
        {"name":"entry","ops":[],"term":null},
        {"name":"yes","params":[["value","i8"]],"ops":[],"term":["return","value"]},
        {"name":"no","params":[["value","i8"]],"ops":[],"term":["return","value"]}]}]});
    let single_target = matches!(term[0].as_str(), Some("br" | "jump"));
    source["fns"][0]["blocks"][2]["unreachable"] = json!(single_target);
    source["fns"][0]["blocks"][0]["term"] = term;
    if dialect {
        source["afx"] = json!(1);
    }
    source
}

#[test]
fn explicit_source_edges_connect_argument_arity_and_types_before_expansion() {
    let fixture = Fixture::new();
    let request = predicate_request(json!(false));
    for (dialect, term, pointer) in [
        (false, json!(["br", "yes"]), "/term"),
        (false, json!(["jump", ["yes", "x", "x"]]), "/term/1"),
        (true, json!(["br", "yes", "x", "x"]), "/term"),
        (false, json!(["br", "yes", "ready"]), "/term/2"),
        (true, json!(["jump", ["yes", "ready"]]), "/term/1/1"),
        (
            true,
            json!(["cond", "ready", ["yes", "ready"], ["no", "x"]]),
            "/term/2/1",
        ),
    ] {
        let source = helper(term, dialect);
        let error = check(&fixture, &request, &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error
                .detail()
                .contains(&format!("/fns/0/blocks/0{pointer}")),
            "{error}"
        );
        assert!(
            error.detail().contains("yes") && error.detail().contains("argument"),
            "{error}"
        );
    }
}

fn calling_request() -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("i8");
    request["bindings"]["success"] =
        json!({"ops":[],"term":["return",["call","helper","x","ready"]]});
    request
}

#[test]
fn valid_edge_forms_preserve_compilation_and_vm_behavior_including_afx_derivation() {
    use super::{bytes, cli};
    for mode in 0..9 {
        let fixture = Fixture::new();
        let mut source = helper(json!(["br", "yes", "x"]), mode >= 3);
        let mut expected = -7;
        match mode {
            0 => {}
            1 => source["fns"][0]["blocks"][0]["term"] = json!(["jump", ["yes", "x"]]),
            2 => {
                source["fns"][0]["blocks"][0]["ops"] =
                    json!([["value","const",{"type":"i8","value":9}]]);
                source["fns"][0]["blocks"][0]["term"] = json!(["br", "yes", "entry.value"]);
                expected = 9;
            }
            3 => {
                source["fns"][0]["blocks"][1]["params"][0][0] = json!("x");
                source["fns"][0]["blocks"][1]["term"] = json!(["return", "x"]);
                source["fns"][0]["blocks"][0]["term"] = json!(["jump", "yes"]);
            }
            4 => {
                source["fns"][0]["blocks"][0]["term"] =
                    json!(["br", ["yes", ["tuple_get", 0, ["tuple", "x", "ready"]]]]);
            }
            5 => {
                source["fns"][0]["blocks"][0]["term"] =
                    json!(["cond", "ready", ["yes", "x"], ["no", "x"]]);
            }
            6 => {
                source["fns"][0]["blocks"][0]["term"] = json!([
                    "switch",
                    ["some", "x"],
                    ["Some", "yes", "$"],
                    ["None", "no", "x"]
                ]);
            }
            7 => {
                source["fns"][0]["blocks"][0]["ops"] = json!([["value", "some?", "x"]]);
                source["fns"][0]["returns"] = json!("Option<i8>");
                source["fns"][0]["blocks"][1]["term"] = json!(["return", ["some", "value"]]);
                source["fns"][0]["blocks"][2]["term"] = json!(["return", ["some", "value"]]);
                source["fns"][0]["blocks"][0]["term"] = json!(["br", "yes", "value"]);
            }
            8 => {
                source["fns"][0]["blocks"][0]["term"] = json!(["br","yes",{"type":"i8","value":3}]);
                expected = 3;
            }
            _ => unreachable!(),
        }
        if mode == 5 || mode == 6 {
            source["fns"][0]["blocks"][2]["unreachable"] = json!(false);
        }
        if mode != 5 && mode != 6 {
            source["fns"][0]["blocks"].as_array_mut().unwrap().pop();
        }
        let mut request = calling_request();
        if mode == 7 {
            request["bindings"]["returns"] = json!("Option<i8>");
        }
        let original = bytes(&source);
        let report = check(&fixture, &request, &source).unwrap();
        assert_eq!(report["composition"], "partial");
        let args = &report["declared_body_control_flow"]["checked_functions"][0]["edge_arguments"];
        assert!(!args.as_array().unwrap().is_empty(), "{report}");
        if mode == 3 {
            assert_eq!(args[0]["trailing_arguments"], "afx_derivation_deferred");
        }
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "mode {mode}: {result}");
        request["base"] = json!("d1@r1");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "mode {mode}: {result}");
        assert_eq!(result["kernel"], "valid");
        for ready in [false, true] {
            let (code, result) = cli(
                &fixture.dir,
                &["call", "checked", "-7", &ready.to_string(), "--on", "c2"],
            );
            assert_eq!(code, 0, "mode {mode}: {result}");
            assert_eq!(
                result["result"],
                if mode == 7 {
                    json!({"Some":expected})
                } else {
                    json!(expected)
                }
            );
        }
        assert_eq!(bytes(&source), original);
    }
}

#[test]
fn switch_payloads_and_nested_operands_connect_to_the_target_parameter_type() {
    let fixture = Fixture::new();
    for (dialect, term, ops, pointer) in [
        (
            false,
            json!([
                "switch",
                "option",
                ["Some", "yes", "$"],
                ["None", "no", "x"]
            ]),
            json!([["option", "some", "ready"]]),
            "/term/2/2",
        ),
        (
            true,
            json!([
                "switch",
                ["some", "ready"],
                ["Some", ["yes", "$"]],
                ["None", "no", "x"]
            ]),
            json!([]),
            "/term/2/1/1",
        ),
        (
            true,
            json!(["br", "yes", ["not", "ready"]]),
            json!([]),
            "/term/2",
        ),
        (
            true,
            json!(["br","yes",{"type":"u8","value":1}]),
            json!([]),
            "/term/2",
        ),
    ] {
        let mut source = helper(term, dialect);
        source["fns"][0]["blocks"][0]["ops"] = ops;
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error
                .detail()
                .contains(&format!("/fns/0/blocks/0{pointer}")),
            "{error}"
        );
        assert!(
            error.detail().contains("yes") && error.detail().contains("requires i8"),
            "{error}"
        );
    }
}

#[test]
fn invalid_edge_source_draft_is_preserved_without_revision_or_candidate_publication() {
    use super::cli;
    let fixture = Fixture::new();
    let source = helper(json!(["br", "yes", "ready"]), false);
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["draft"], "d1@r1");
    let directory = fixture.dir.join(".sley/drafts/d1/r1");
    let snapshot: std::collections::BTreeMap<_, _> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_owned(),
                std::fs::read(path).unwrap(),
            )
        })
        .collect();
    let head = fixture.workspace.read_head().unwrap().transaction_id();
    let mut request = calling_request();
    request["base"] = json!("d1@r1");
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
            .contains("/fns/0/blocks/0/term/2")
    );
    for (name, bytes) in snapshot {
        assert_eq!(std::fs::read(directory.join(name)).unwrap(), bytes);
    }
    for path in [".sley/candidates", ".sley/drafts/d1/r2", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        head
    );
}

#[test]
fn unresolved_arguments_and_literal_admission_are_deferred_without_rewriting() {
    use super::bytes;
    let fixture = Fixture::new();
    for argument in [
        json!("unknown"),
        json!("x#1"),
        json!(3),
        json!({"type":"i8","value":3}),
    ] {
        let source = helper(json!(["br", "yes", argument]), true);
        let original = bytes(&source);
        let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        let control = &report["declared_body_control_flow"];
        if argument == json!("x#1") {
            assert_eq!(control["status"], "explicit_targets_checked", "{report}");
            let input = &control["checked_functions"][0]["edge_arguments"][0]["arguments"][0];
            assert_eq!(input["actual_type"], "i8");
            assert_eq!(input["expected_type"], "i8");
            assert_eq!(input["type_connection"], "checked");
            assert_eq!(control["deferred"], json!([]));
        } else {
            assert_eq!(control["status"], "partial", "{report}");
            assert!(
                control["deferred"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v == "/fns/0/blocks/0/term/2"),
                "{report}"
            );
        }
        assert_eq!(report["composition"], "partial");
        assert_eq!(bytes(&source), original);
    }
    let source = helper(
        json!([
            "switch",
            ["some", "x"],
            ["Some", "yes", "$"],
            ["None", "no", "$"]
        ]),
        true,
    );
    let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
    assert!(
        error.detail().contains("/term/3/2") && error.detail().contains("unit case"),
        "{error}"
    );
}
