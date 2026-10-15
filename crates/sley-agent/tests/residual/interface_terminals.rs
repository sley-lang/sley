use super::interface_tests::{check, predicate_request};
use super::{Fixture, branch_request, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn request(term: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["success"]["term"] = term;
    request
}

fn terminal(report: &Value) -> &Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == "/bindings/success/term")
        .unwrap()
}

#[test]
fn terminal_edges_and_unknown_forms_refuse_before_publication() {
    let fixture = Fixture::new();
    for term in [
        json!(["br", "entry"]),
        json!(["jump", ["gw_b0", "x"]]),
        json!(["cond", "ready", "left", "right"]),
        json!(["switch", ["some", "x"], ["Some", "arm", "$"]]),
        json!(["br", "checked.gw_b0"]),
        json!(["ret", "x"]),
        json!([]),
        json!([1]),
        json!(["trap", 1]),
        json!(["trap", "invalid"]),
        json!(["trap", "unreachable", "x", "ignored"]),
        json!(["trap", ["add", "x", 1]]),
    ] {
        let request = request(term);
        let before = bytes(&request);
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
                .contains("/bindings/success/term"),
            "{result}"
        );
        assert_eq!(bytes(&request), before);
    }
    for path in [".sley/candidates", ".sley/drafts", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}

#[test]
fn trap_codes_and_nested_payloads_preserve_actual_vm_termination() {
    for (term, expected) in [
        (json!(["trap"]), json!({"trap":1})),
        (
            json!(["trap", "unreachable", "x"]),
            json!({"trap":1,"payload":7}),
        ),
        (
            json!(["trap", "resource_exhausted", "ready"]),
            json!({"trap":2,"payload":true}),
        ),
        (
            json!(["trap", "adapter_contract_violation", ["some", "x"]]),
            json!({"trap":3,"payload":{"Some":7}}),
        ),
        (
            json!(["trap", "internal_invariant", ["lt", "x", 0]]),
            json!({"trap":4,"payload":false}),
        ),
    ] {
        let fixture = Fixture::new();
        let request = request(term.clone());
        let report = check(&fixture, &request, &json!({})).unwrap();
        let terminal = terminal(&report);
        assert_eq!(terminal["control_flow"], "terminal_without_successors");
        assert_eq!(
            terminal["payload_eligibility"],
            if term.as_array().unwrap().len() == 3 {
                "persistable_checked"
            } else {
                "absent"
            }
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", "7", "true", "--on", "c1"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
}

#[test]
fn nonpersistable_and_invalid_nested_trap_payloads_refuse_early() {
    let fixture = Fixture::new();
    for payload in [
        json!(["cell", "x"]),
        json!(["tuple", "ready", ["cell", "x"]]),
        json!(["and", "x", true]),
        json!(["assert", "condition", "ready"]),
    ] {
        let request = request(json!(["trap", "unreachable", payload]));
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert!(
            error.detail().contains("/bindings/success/term/2"),
            "{error}"
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
    }
    assert!(!fixture.dir.join(".sley/candidates").exists());
}

#[test]
fn unresolved_trap_payload_keeps_exact_expression_obligations() {
    let fixture = Fixture::new();
    for payload in [
        json!(7),
        json!(["future_expression"]),
        json!(["hash", ["future_expression"]]),
    ] {
        let request = request(json!(["trap", "unreachable", payload]));
        let report = check(&fixture, &request, &json!({})).unwrap();
        let terminal = terminal(&report);
        assert_eq!(terminal["payload_interface"]["expression_types"], "partial");
        assert!(
            !terminal["payload_interface"]["deferred_expressions"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(report["composition"], "partial");
    }
}

#[test]
fn nested_terminal_and_join_conflicts_have_authored_locations() {
    let fixture = Fixture::new();
    let mut nested = request(json!(["trap"]));
    nested["bindings"]["success"] = json!({"fragment":{"id":"ordered_guard_chain","version":1},
        "bindings":{"guards":[],"success":{"ops":[],"term":["jump","entry"]}}});
    let error = check(&fixture, &nested, &json!({})).unwrap_err();
    assert!(
        error
            .detail()
            .contains("/bindings/success/bindings/success/term"),
        "{error}"
    );
    let mut branch = branch_request();
    branch["bindings"]["join"]["term"] = json!(["br", "entry"]);
    let error = check(&fixture, &branch, &json!({})).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(error.detail().contains("/bindings/join/term"), "{error}");
    branch["bindings"]["join"]["term"] = json!(["trap"]);
    assert!(
        check(&fixture, &branch, &json!({}))
            .unwrap_err()
            .detail()
            .contains("join must explicitly return, ok, or fail")
    );
}

#[test]
fn traps_use_bound_named_traits_and_preserve_unavailable_definitions() {
    for accepted in [false, true] {
        let fixture = Fixture::new();
        let declarations = json!({"af1":1,"types":[{"name":"Pack","record":[["value","i8"]]}]});
        let (code, result) = cli(
            &fixture.dir,
            &["try", &declarations.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        if accepted {
            let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
            assert_eq!(code, 0, "{result}");
        }
        let mut request = request(json!(["trap", "unreachable", "pack"]));
        request["bindings"]["params"] = json!([["pack", "Pack"]]);
        if !accepted {
            request["base"] = json!("d1@r1");
        }
        let report = check(
            &fixture,
            &request,
            &if accepted { json!({}) } else { declarations },
        )
        .unwrap();
        assert_eq!(
            terminal(&report)["payload_eligibility"],
            "persistable_checked"
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", "{\"value\":9}", "--on", "c2"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!({"trap":1,"payload":{"value":9}}));
        let bad = json!({"types":[{"name":"Pack","record":[["value","Cell<i8>"]]}]});
        assert!(
            check(&fixture, &request, &bad)
                .unwrap_err()
                .detail()
                .contains("persistable")
        );
        let incomplete = json!({"types":[{"name":"Pack","record":["value"]}]});
        let error = check(&fixture, &request, &incomplete).unwrap_err();
        super::interface_closure_tests::assert_incomplete(&error, "/bindings/params/");
    }
}

#[test]
fn checked_payload_failure_happens_before_trap_and_shared_budget_is_not_reset() {
    use sley_agent::{
        names::{NameMap, Names},
        residual::{frontier::Budget, interfaces, parse_request},
    };
    let fixture = Fixture::new();
    let mut request = request(json!([
        "trap",
        "internal_invariant",
        ["div?", "x", "divisor"]
    ]));
    request["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
    request["bindings"]["params"] = json!([["x", "i8"], ["divisor", "i8"]]);
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    for (divisor, expected) in [
        ("0", json!({"Err":{"ArithmeticError":"DivideByZero"}})),
        ("2", json!({"trap":4,"payload":3})),
    ] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", "7", divisor, "--on", "c1"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 0);
    let error = interfaces::check(
        head.program(),
        &names,
        &serde_json::Map::new(),
        &parse_request(&bytes(&request)).unwrap(),
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
}
