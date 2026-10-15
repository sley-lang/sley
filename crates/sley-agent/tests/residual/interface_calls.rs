use super::interface_tests::{check, predicate_request};
use super::{Fixture, branch_request, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};

pub(super) fn helpers() -> Value {
    json!({"af1":1,"afx":1,"fns":[
        {"fn":"probe","params":[["z","i8"],["ready","bool"]],"returns":"bool",
         "blocks":[{"name":"entry","ops":[["negative","lt","z",0],
         ["both","and","negative","ready"]],"term":["return","both"]}]},
        {"fn":"echo","params":[["z","i8"]],"returns":"i8",
         "blocks":[{"name":"entry","ops":[],"term":["return","z"]}]},
        {"fn":"fetch","params":[["item","Option<i8>"]],"returns":"Option<i8>",
         "blocks":[{"name":"entry","ops":[],"term":["return","item"]}]}
    ]})
}

pub(super) fn fixture(accepted: bool) -> Fixture {
    let fixture = Fixture::new();
    let (code, report) = cli(&fixture.dir, &["try", &helpers().to_string(), "--no-test"]);
    assert_eq!(code, 0, "{report}");
    if accepted {
        let (code, report) = cli(&fixture.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{report}");
    }
    fixture
}

#[test]
fn call_conflicts_identify_arguments_results_and_unknown_callees_before_writes() {
    let fixture = fixture(true);
    let head = fixture.workspace.head().unwrap().transaction_id();
    for (predicate, detail) in [
        (json!(["call", "probe", "ready", "x"]), "parameter 0"),
        (json!(["call", "probe", 128, true]), "literal conflicts"),
        (json!(["call", "probe", "x"]), "requires 2"),
        (json!(["call", "probe", "x", true, true]), "supplies 3"),
        (json!(["call", "echo", "x"]), "guard condition"),
        (json!(["call", "absent"]), "no callable interface"),
        (json!(["call"]), "explicit callee name"),
        (json!(["call", ["call", "probe"]]), "explicit callee name"),
        (
            json!(["call", "probe", ["call", "absent"], true]),
            "no callable interface",
        ),
        (
            json!(["call", "probe", {"type":"u8","value":1}, true]),
            "parameter 0",
        ),
    ] {
        let request = predicate_request(predicate);
        let before = bytes(&request);
        let (code, report) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{report}");
        assert_eq!(
            report["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
            "{report}"
        );
        let message = report["detail"].as_str().unwrap();
        assert!(message.contains("/bindings/guards/0/when"), "{report}");
        assert!(message.contains(detail), "{report}");
        assert_eq!(before, bytes(&request));
    }
    assert_eq!(fixture.workspace.head().unwrap().transaction_id(), head);
    for path in [
        ".sley/drafts/d1/r2",
        ".sley/drafts/d2",
        ".sley/candidates/c2.hex",
        ".sley/candidates/c2.json",
        ".sley/residual",
    ] {
        assert!(!fixture.dir.join(path).exists(), "{path}");
    }
}

#[test]
fn declared_and_accepted_calls_keep_evidence_distinct_and_execute_identically() {
    for accepted in [false, true] {
        let fixture = fixture(accepted);
        for alias in ["call", "call_direct", "112"] {
            let mut request =
                predicate_request(json!([alias, "probe", ["call", "echo", "x"], "ready"]));
            let declarations = if accepted { json!({}) } else { helpers() };
            let report = check(&fixture, &request, &declarations).unwrap();
            let connection = &report["connections"][1];
            assert_eq!(connection["guard_result"], "bool_checked");
            assert_eq!(connection["expression_types"], "connections_checked");
            let calls = connection["calls"].as_array().unwrap();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0]["callee"], "echo");
            assert_eq!(calls[1]["callee"], "probe");
            for call in calls {
                assert_eq!(
                    call["signature_source"],
                    if accepted {
                        "accepted_graph"
                    } else {
                        "declared_interface"
                    }
                );
                assert_eq!(
                    call["effects"],
                    if accepted {
                        "empty_accepted_effect_set"
                    } else {
                        "empty_declared_effect_set"
                    }
                );
            }
            assert_eq!(report["composition"], "partial");
            // Run one spelling through the real expansion, kernel and VM too.
            if alias != "call" {
                continue;
            }
            if !accepted {
                request["base"] = json!("d1@r1");
            }
            let (code, result) = cli(
                &fixture.dir,
                &["residual", "try", &request.to_string(), "--no-test"],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["kernel"], "valid");
            for (x, ready, expected) in [
                ("-3", "true", json!("None")),
                ("-3", "false", json!({"Some":-3})),
                ("3", "true", json!({"Some":3})),
            ] {
                let (code, result) =
                    cli(&fixture.dir, &["call", "checked", x, ready, "--on", "c2"]);
                assert_eq!(code, 0, "{result}");
                assert_eq!(result["result"], expected);
            }
        }
    }
}

#[test]
fn draft_overrides_patches_deletions_and_recursive_target_use_authored_interfaces() {
    let fixture = fixture(true);
    let request = predicate_request(json!(["call", "probe", "x", "ready"]));
    for key in ["fns", "functions", "patch"] {
        let declarations =
            json!({key:[{"fn":"probe","params":[["z","bool"],["ready","i8"]],"returns":"bool"}]});
        let error = check(&fixture, &request, &declarations).unwrap_err();
        assert!(error.detail().contains("parameter 0"), "{error}");
    }
    let report = check(&fixture, &request, &json!({"patch":[{"fn":"probe"}]})).unwrap();
    assert_eq!(
        report["connections"][1]["calls"][0]["effects"],
        "empty_declared_effect_set"
    );
    for declarations in [
        json!({"delete":["probe"]}),
        json!({"consts":[{"name":"probe","type":"i8","value":1}]}),
    ] {
        let error = check(&fixture, &request, &declarations).unwrap_err();
        assert!(error.detail().contains("no callable interface"), "{error}");
    }
    let mut replacement = helpers();
    replacement["delete"] = json!(["probe"]);
    check(&fixture, &request, &replacement).unwrap();

    // Target replaces the old bool-returning probe with Option<i8>. Deliberate
    // recursion is checked only as an interface here, never executed.
    let mut recursive = predicate_request(json!([
        "eq",
        ["call", "probe", "z", "a"],
        ["call", "probe", "z", "a"]
    ]));
    recursive["scope"] = json!(["probe"]);
    recursive["bindings"]["params"] = json!([["z", "i8"], ["a", "bool"]]);
    recursive["bindings"]["success"]["term"] = json!(["return", ["some", "z"]]);
    let report = check(&fixture, &recursive, &json!({})).unwrap();
    assert_eq!(
        report["connections"][1]["calls"][0]["return_type"],
        "Option<i8>"
    );
    assert_eq!(
        report["connections"][1]["calls"][0]["signature_source"],
        "declared_interface"
    );
    recursive["bindings"]["guards"][0]["when"] = json!(["call", "probe", "z", "a"]);
    let error = check(&fixture, &recursive, &json!({})).unwrap_err();
    assert!(error.detail().contains("Option<i8>"), "{error}");
    assert!(error.detail().contains("guard condition"), "{error}");
}

#[test]
fn call_returned_branch_checks_coverage_payloads_and_matches_kernel_and_vm() {
    let fixture = fixture(true);
    let mut request = branch_request();
    request["bindings"]["input"] = json!(["call", "fetch", "maybe"]);
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(report["connections"][0]["branch_coverage"], "checked");
    assert_eq!(
        report["connections"][0]["expression_interface"]["calls"][0]["callee"],
        "fetch"
    );
    let mut missing = request.clone();
    missing["bindings"]["cases"].as_array_mut().unwrap().pop();
    let error = check(&fixture, &missing, &json!({})).unwrap_err();
    assert!(error.detail().contains("missing cases"), "{error}");
    let mut wrong = request.clone();
    wrong["bindings"]["cases"][0]["payload"][1] = json!("i16");
    assert_eq!(
        check(&fixture, &wrong, &json!({})).unwrap_err().code(),
        AgentErrorCode::ResidualConstraintConflict
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (input, expected) in [(json!({"Some":9}), -9), (json!("None"), 17)] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", &input.to_string(), "--on", "c2"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!({"Ok":expected}));
    }
}

#[test]
fn call_failure_payload_checks_result_and_literal_argument_before_expansion() {
    let fixture = Fixture::new();
    let mut declarations = helpers();
    declarations["types"] = json!([{"name":"Error","variant":[["WithValue","i8"]]}]);
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let mut request = predicate_request(json!("ready"));
    request["base"] = json!("d1@r1");
    request["bindings"]["returns"] = json!("Result<i8,Error>");
    request["bindings"]["guards"][0]["fail"] = json!(["fail", "WithValue", ["call", "echo", "x"]]);
    request["bindings"]["success"]["term"] = json!(["ok", "x"]);
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(
        report["connections"][0]["payload_value"],
        "checked_expression"
    );
    for payload in [
        json!(["call", "probe", "x", "ready"]),
        json!(["call", "echo", 128]),
    ] {
        let mut wrong = request.clone();
        wrong["bindings"]["guards"][0]["fail"][2] = payload;
        let error = check(&fixture, &wrong, &declarations).unwrap_err();
        assert!(
            error.detail().contains("/bindings/guards/0/fail/2"),
            "{error}"
        );
    }
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (ready, expected) in [
        ("true", json!({"Err":{"WithValue":5}})),
        ("false", json!({"Ok":5})),
    ] {
        let (code, result) = cli(&fixture.dir, &["call", "checked", "5", ready, "--on", "c2"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
}

#[test]
fn callee_effect_metadata_refuses_but_redeclarations_follow_the_af1_interface() {
    use sley_agent::workspace::Program;
    use sley_mutate::value::{EntityBodyValue, EntityIdSet};

    let fixture = fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let id = names.resolve("probe").unwrap();
    let mut record = head.program().object(&id).unwrap().record().clone();
    let EntityBodyValue::Function(body) = &mut record.body else {
        panic!("function")
    };
    body.effects =
        EntityIdSet::from_unsorted(vec![sley_id::EntityId::from_bytes([243; 32])]).unwrap();
    let replacement = sley_mutate::build_entity_object(head.program().epoch(), &record).unwrap();
    let objects = head
        .program()
        .objects()
        .iter()
        .map(|object| {
            if object.record().entity_id == id {
                replacement.clone()
            } else {
                object.clone()
            }
        })
        .collect();
    // Synthetic metadata fixture isolates the preflight rule. It is not a
    // kernel-valid accepted program and is never published or run in the VM.
    let program = Program::new(
        head.program().epoch(),
        head.program().root(),
        head.program().workspace(),
        objects,
    );
    let request = parse_request(&bytes(&predicate_request(json!([
        "call", "probe", "x", "ready"
    ]))))
    .unwrap();
    let error = interfaces::check(
        &program,
        &names,
        &serde_json::Map::new(),
        &request,
        &mut Budget::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(
        error.detail().contains("/bindings/guards/0/when"),
        "{error}"
    );
    assert!(error.detail().contains("empty effect set"), "{error}");
    for declarations in [helpers(), json!({"patch":[{"fn":"probe"}]})] {
        let report = interfaces::check(
            &program,
            &names,
            declarations.as_object().unwrap(),
            &request,
            &mut Budget::default(),
        )
        .unwrap();
        assert_eq!(
            report["connections"][1]["calls"][0]["effects"],
            "empty_declared_effect_set"
        );
        assert_eq!(
            report["connections"][1]["calls"][0]["effect_body_validation"],
            "ordinary_compiler_and_kernel"
        );
        assert_eq!(report["composition"], "partial");
    }
}

#[test]
fn known_call_result_does_not_hide_a_deferred_argument() {
    let fixture = fixture(true);
    let request = predicate_request(json!(["call", "probe", ["future_expression"], "ready"]));
    let report = check(&fixture, &request, &json!({})).unwrap();
    let connection = &report["connections"][1];
    assert_eq!(connection["guard_result"], "bool_checked");
    assert_eq!(connection["expression_types"], "partial");
    assert_eq!(
        connection["deferred_expressions"],
        json!(["/bindings/guards/0/when/2"])
    );
    assert_eq!(connection["calls"][0]["return_type"], "bool");
}

#[test]
fn nested_calls_obey_shared_work_and_expression_depth_limits() {
    let fixture = fixture(true);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let mut request = parse_request(&bytes(&predicate_request(json!([
        "call", "probe", "x", "ready"
    ]))))
    .unwrap();
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 1);
    for _ in 0..2 {
        let error = interfaces::check(
            head.program(),
            &names,
            &serde_json::Map::new(),
            &request,
            &mut budget,
        )
        .unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    }
    // Exercise the public interface API independently of the stricter envelope
    // JSON-depth bound; each nested call must charge/check the same budget.
    let mut argument = json!("x");
    for _ in 0..34 {
        argument = json!(["call", "echo", argument]);
    }
    request.bindings["guards"][0]["when"] = json!(["call", "probe", argument, "ready"]);
    let error = interfaces::check(
        head.program(),
        &names,
        &serde_json::Map::new(),
        &request,
        &mut Budget::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("nesting"), "{error}");
    assert!(!fixture.dir.join(".sley/drafts/d2").exists());
}
