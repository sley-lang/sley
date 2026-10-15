use super::{Fixture, branch_request, bytes, cli, guarded_request, planning_request};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};

pub(super) fn check(
    fixture: &Fixture,
    request: &Value,
    declarations: &Value,
) -> sley_agent::Result<Value> {
    let head = fixture.workspace.read_head()?;
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json"))?,
    );
    interfaces::check(
        head.program(),
        &names,
        declarations.as_object().unwrap(),
        &parse_request(&bytes(request))?,
        &mut Budget::default(),
    )
}

pub(super) fn predicate_request(predicate: Value) -> Value {
    let mut request = guarded_request();
    request["bindings"]["params"] = json!([["x", "i8"], ["ready", "bool"]]);
    request["bindings"]["returns"] = json!("Option<i8>");
    request["bindings"]["guards"] = json!([{"when":null,"fail":["fail"]}]);
    request["bindings"]["guards"][0]["when"] = predicate;
    request["bindings"]["success"] = json!({"ops":[],"term":["return",["some","x"]]});
    request
}

#[test]
fn guard_boolean_connections_check_both_comparison_directions_and_literal_types() {
    let fixture = Fixture::new();
    for predicate in [
        json!(true),
        json!({"value":true}),
        json!("ready"),
        json!(["tuple_get", 1, ["tuple", "x", "ready"]]),
        json!(["eq", ["tuple", 1, "ready"], ["tuple", "x", true]]),
        json!(["and", ["lt", "x", 7], ["not", "ready"]]),
        json!(["or",["ge",-5,"x"],["equal",{"type":"i8","value":3},"x"]]),
    ] {
        let request = predicate_request(predicate);
        let before = request.clone();
        let report = check(&fixture, &request, &json!({})).unwrap();
        let connection = &report["connections"][1];
        assert_eq!(connection["guard_result"], "bool_checked");
        assert_eq!(
            connection["expression_types"], "connections_checked",
            "{report}"
        );
        assert_eq!(connection["deferred_expressions"], json!([]));
        assert_eq!(connection["rewritten"], false);
        assert_eq!(request, before);
    }
}

#[test]
fn guard_predicate_conflicts_refuse_at_authored_uses_before_publication() {
    let fixture = Fixture::new();
    for predicate in [
        json!("x"),
        json!(1),
        json!(["and", true, "x"]),
        json!(["eq","x",{"type":"u8","value":2}]),
        json!(["lt", 128, "x"]),
        json!(["not", true, false]),
        json!(["add", "x", 1]),
        json!("missing"),
    ] {
        let request = predicate_request(predicate);
        let (code, report) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{report}");
        assert_eq!(
            report["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
            "{report}"
        );
        assert!(
            report["detail"]
                .as_str()
                .unwrap()
                .contains("/bindings/guards/0/when"),
            "{report}"
        );
    }
    for path in [".sley/drafts", ".sley/candidates", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}

#[test]
fn guard_unknown_expressions_and_unanchored_literals_remain_explicitly_deferred() {
    let fixture = Fixture::new();
    for (predicate, result) in [
        (json!(["future_expression"]), "deferred"),
        (json!(["and", true, ["future_expression"]]), "bool_checked"),
        (json!(["lt", 1, 2]), "bool_checked"),
        (json!(["eq", ["future_expression"], "x"]), "bool_checked"),
    ] {
        let report = check(&fixture, &predicate_request(predicate), &json!({})).unwrap();
        let connection = &report["connections"][1];
        assert_eq!(connection["guard_result"], result);
        assert_eq!(connection["expression_types"], "partial");
        assert!(
            !connection["deferred_expressions"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(report["composition"], "partial");
    }
}

#[test]
fn guard_predicate_preflight_agrees_with_kernel_and_actual_execution() {
    let fixture = Fixture::new();
    let request = predicate_request(json!(["or", ["lt", "x", 0], ["not", "ready"]]));
    let (code, report) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["kernel"], "valid");
    for (x, ready, expected) in [
        ("-3", "true", json!("None")),
        ("3", "false", json!("None")),
        ("3", "true", json!({"Some":3})),
    ] {
        let (code, report) = cli(&fixture.dir, &["call", "checked", x, ready, "--on", "c1"]);
        assert_eq!(code, 0, "{report}");
        assert_eq!(report["result"], expected);
    }
}

#[test]
fn pipeline_checks_nested_widths_shifts_and_result_before_expansion() {
    let fixture = Fixture::new();
    let mut request = planning_request();
    request["bindings"]["params"] = json!([["x", "i8"], ["amount", "u32"]]);
    request["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
    request["bindings"]["steps"] = json!([["out", ["shl", ["add", "x", 1], "amount"]]]);
    request["bindings"]["result"] = json!("out#0");
    let report = check(&fixture, &request, &json!({})).unwrap();
    let pipeline = report["connections"].as_array().unwrap().last().unwrap();
    assert_eq!(pipeline["expression_types"], "constraints_checked");
    assert_eq!(pipeline["checked_literals"], 1);
    assert_eq!(pipeline["rewritten"], false);
    request["bindings"]["params"][1][1] = json!("i8");
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(error.detail().contains("/bindings/steps/0/1/2"), "{error}");
    assert!(error.detail().contains("/bindings/params/1"), "{error}");
    assert!(error.detail().contains("u32"), "{error}");
    request["bindings"]["params"][1][1] = json!("u32");
    request["bindings"]["returns"] = json!("Result<i16,ArithmeticError>");
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(error.detail().contains("/bindings/result"), "{error}");
    assert!(error.detail().contains("/bindings/returns"), "{error}");
}

#[test]
fn pipeline_constraints_follow_intermediate_values_and_check_literal_ranges() {
    let fixture = Fixture::new();
    for operand in [json!({"type":"u8","value":1}), json!(128)] {
        let mut request = planning_request();
        request["bindings"]["params"] = json!([["x", "i8"]]);
        request["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
        request["bindings"]["steps"] =
            json!([["a", ["add", 1, operand]], ["out", ["mul", "a", "x"]]]);
        request["bindings"]["result"] = json!("out");
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("/bindings/steps/0/1/2"), "{error}");
        assert!(error.detail().contains("/bindings/params/0"), "{error}");
    }
}

#[test]
fn pipeline_refuses_noninteger_reads_forward_references_and_duplicate_steps() {
    let fixture = Fixture::new();
    for (steps, parameter) in [
        (json!([["out", ["add", "x", 1]]]), "bool"),
        (
            json!([["a", ["add", "out", 1]], ["out", ["neg", "x"]]]),
            "i64",
        ),
        (json!([["out", ["neg", "x"]], ["out", ["neg", "x"]]]), "i64"),
        (json!([["x", ["add", "x", 1]]]), "i64"),
    ] {
        let mut request = planning_request();
        request["bindings"]["params"] = json!([["x", parameter]]);
        request["bindings"]["steps"] = steps;
        request["bindings"]["result"] = json!("out");
        assert_eq!(
            check(&fixture, &request, &json!({})).unwrap_err().code(),
            AgentErrorCode::ResidualConstraintConflict
        );
    }
}

#[test]
fn pipeline_unanchored_unused_literals_remain_deferred_without_rewriting() {
    let fixture = Fixture::new();
    let mut request = planning_request();
    request["bindings"]["steps"] = json!([["unused", ["add", 1, 2]]]);
    request["bindings"]["result"] = json!("x");
    let before = request.clone();
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        report["connections"][0]["expression_types"],
        "unanchored_literals_deferred"
    );
    assert_eq!(request, before);
}

#[test]
fn pipeline_cli_preserves_checked_shift_execution_and_refuses_bad_width_without_artifacts() {
    let fixture = Fixture::new();
    let mut request = planning_request();
    request["bindings"]["params"] = json!([["x", "i8"], ["amount", "u32"]]);
    request["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
    request["bindings"]["steps"] = json!([["out", ["shl", ["add", "x", 1], "amount"]]]);
    request["bindings"]["result"] = json!("out");
    let (code, report) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["kernel"], "valid");
    let (code, report) = cli(
        &fixture.dir,
        &["call", "scaled_half", "2", "1", "--on", "c1"],
    );
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["result"], json!({"Ok":6}), "{report}");
    let other = Fixture::new();
    request["bindings"]["params"][1][1] = json!("i8");
    let (code, report) = cli(
        &other.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{report}");
    assert_eq!(report["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    for path in [".sley/drafts", ".sley/candidates", ".sley/residual"] {
        assert!(!other.dir.join(path).exists());
    }
}

#[test]
fn declared_interfaces_resolve_draft_types_and_retain_nested_locators() {
    let fixture = Fixture::new();
    let declarations =
        json!({"types":[{"name":"Error","variant":["ZeroInput","ZeroDivisor","Arithmetic"]}]});
    let request = guarded_request();
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(report["stage"], "before_fragment_expansion");
    assert_eq!(report["composition"], "partial");
    assert_eq!(
        report["interfaces"][1]["bindings"],
        "/bindings/success/bindings"
    );
    assert_eq!(
        report["interfaces"][1]["output"]["source"],
        "/bindings/returns"
    );
    assert_eq!(
        report["interfaces"][1]["output"]["type"],
        "Result<i8,Error>"
    );
    assert!(
        report["deferred"]
            .as_array()
            .unwrap()
            .contains(&json!("ownership"))
    );
    assert!(!fixture.dir.join(".sley").exists());
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(error.detail().contains("/bindings/returns"));
}

#[test]
fn incompatible_pipeline_outputs_and_propagation_fail_at_author_bindings() {
    let fixture = Fixture::new();
    for result in [
        "i8",
        "Result<bool,ArithmeticError>",
        "Result<i8,IndexError>",
    ] {
        let mut request = planning_request();
        request["bindings"]["returns"] = json!(result);
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("/bindings/returns"));
    }
    let mut request = planning_request();
    request["bindings"]["params"] = json!([["x", "i8"], ["x", "i16"]]);
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(error.detail().contains("/bindings/params/0"));
    assert!(error.detail().contains("/bindings/params/1"));
}

#[test]
fn branch_payload_and_direct_join_connections_use_resolved_types() {
    let fixture = Fixture::new();
    let mut request = branch_request();
    check(&fixture, &request, &json!({})).unwrap();
    request["bindings"]["cases"][0]["payload"][1] = json!("i16");
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(error.detail().contains("/bindings/cases/0/payload"));
    assert!(error.detail().contains("/bindings/params/0"));
    request["bindings"]["cases"][0]["payload"][1] = json!("i8");
    request["bindings"]["cases"][0]["ops"] = json!([]);
    request["bindings"]["cases"][0]["values"] = json!(["x"]);
    request["bindings"]["join"]["params"] = json!([["out", "i16"]]);
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(error.detail().contains("/bindings/cases/0/values/0"));
    assert!(error.detail().contains("/bindings/join/params/0"));
}

#[test]
fn interface_checks_refuse_missing_parameters_and_share_the_callers_budget() {
    let fixture = Fixture::new();
    let mut request = planning_request();
    request["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("params");
    assert!(check(&fixture, &request, &json!({})).is_err());
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(head.program(), &NameMap::default());
    let request = parse_request(&bytes(&planning_request())).unwrap();
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 0);
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
    assert!(!fixture.dir.join(".sley").exists());
}

#[test]
fn cli_checks_interfaces_before_expansion_and_retains_honest_evidence() {
    let fixture = Fixture::new();
    let mut request = planning_request();
    request["bindings"]["returns"] = json!("bool");
    // Grammar accepts an explicit expression, but expansion cannot lower it.
    // The interface conflict must be reported first, before generation begins.
    request["bindings"]["steps"] = json!([["out", ["unsupported", "x", 2]]]);
    let (code, report) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{report}");
    assert!(
        report["detail"]
            .as_str()
            .unwrap()
            .contains("/bindings/returns"),
        "{report}"
    );
    for path in [".sley/drafts", ".sley/candidates", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
    let request = planning_request();
    let (code, report) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{report}");
    let (code, shown) = cli(&fixture.dir, &["residual", "show", "d1@r1", "--provenance"]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["interfaces"]["composition"], "partial");
    assert_eq!(shown["interfaces"]["authority"], "none");
}

#[test]
fn cli_resolves_named_interfaces_from_the_exact_source_draft() {
    let fixture = Fixture::new();
    let mut base = super::frame();
    base["types"] = json!([{"name":"Error","variant":["ZeroInput","ZeroDivisor","Arithmetic"]}]);
    let (code, report) = cli(&fixture.dir, &["try", &base.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{report}");
    let mut request = guarded_request();
    request["base"] = json!("d1@r1");
    let (code, report) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["kernel"], "valid");
    assert_eq!(report["draft"], "d1@r2");
    let (code, shown) = cli(&fixture.dir, &["residual", "show", "d1@r2", "--provenance"]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(
        shown["interfaces"]["interfaces"][1]["output"]["type"],
        "Result<i8,Error>"
    );
}

#[test]
fn relation_rows_do_not_expand_before_the_bound_interface_check() {
    let fixture = Fixture::new();
    let mut request = planning_request();
    request["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("returns");
    request["bindings"]["steps"] = json!([["out", ["unsupported", "x", 2]]]);
    request["choices"] = json!({"version":1,"contract":"author_closed_relation",
        "rows":[{"/bindings/returns":"bool"},{"/bindings/returns":"Result<i64,ArithmeticError>"}]});
    let (code, report) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{report}");
    let detail = report["detail"].as_str().unwrap();
    assert!(detail.contains("/choices/rows/0"), "{report}");
    assert!(detail.contains("/bindings/returns"), "{report}");
    assert_eq!(report["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    for path in [".sley/drafts", ".sley/candidates", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}

fn named_branch() -> (Value, Value) {
    let mut request = branch_request();
    request["bindings"]["params"] = json!([["maybe", "Input"]]);
    request["bindings"]["cases"] = json!([
        {"case":"Present","payload":["x","i8"],"ops":[],"values":["x"]},
        {"case":"Absent","payload":null,"ops":[],"values":[17]}
    ]);
    let declarations = json!({"types":[{"name":"Input","variant":[["Present","i8"],"Absent"]}]});
    (request, declarations)
}

#[test]
fn named_branches_require_exact_coverage_and_preserve_payload_types() {
    let fixture = Fixture::new();
    let (request, declarations) = named_branch();
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(report["connections"][0]["branch_coverage"], "checked");
    assert_eq!(report["connections"][0]["source"], "/bindings/params/0");
    for (change, expected) in [
        (0, "missing cases Absent"),
        (1, "not a case"),
        (2, "duplicate"),
        (3, "payload type conflicts"),
    ] {
        let mut bad = request.clone();
        match change {
            0 => {
                bad["bindings"]["cases"].as_array_mut().unwrap().pop();
            }
            1 => bad["bindings"]["cases"][1]["case"] = json!("Invented"),
            2 => bad["bindings"]["cases"][1]["case"] = json!("Present"),
            3 => bad["bindings"]["cases"][0]["payload"][1] = json!("u8"),
            _ => unreachable!(),
        }
        let error = check(&fixture, &bad, &declarations).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains(expected), "{error}");
        assert!(error.detail().contains("/bindings/cases"));
    }
    let records = json!({"types":[{"name":"Input","record":[["Present","i8"]]}]});
    assert!(
        check(&fixture, &request, &records)
            .unwrap_err()
            .detail()
            .contains("record")
    );
    let unknown = json!({"types":[{"name":"Input","variant":[["Present","Missing"],"Absent"]}]});
    let error = check(&fixture, &request, &unknown).unwrap_err();
    assert!(error.detail().contains("/types/0/variant/0"));
}

#[test]
fn builtin_branch_coverage_and_untyped_input_reports_are_distinct() {
    let fixture = Fixture::new();
    for ty in ["Option<i8>", "Result<i8,i8>"] {
        let mut request = branch_request();
        request["bindings"]["params"][0][1] = json!(ty);
        if ty.starts_with("Result") {
            request["bindings"]["cases"][0]["case"] = json!("Ok");
            request["bindings"]["cases"][1]["case"] = json!("Err");
            request["bindings"]["cases"][1]["payload"] = json!(["error", "i8"]);
        }
        check(&fixture, &request, &json!({})).unwrap();
        request["bindings"]["cases"].as_array_mut().unwrap().pop();
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert!(error.detail().contains("missing cases"));
        request["bindings"]["input"] = json!(["tuple_get", 0, ["tuple", "maybe"]]);
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert!(error.detail().contains("missing cases"));
        request["bindings"]["input"] = json!(["future_expression"]);
        let report = check(&fixture, &request, &json!({})).unwrap();
        assert_eq!(report["connections"][0]["branch_coverage"], "deferred");
    }
}

#[test]
fn arithmetic_mappings_preserve_or_explicitly_drop_the_native_payload() {
    let fixture = Fixture::new();
    let mut request = planning_request();
    request["bindings"]["returns"] = json!("Result<i64,Errors>");
    let declarations = json!({"types":[{"name":"Errors","variant":["Drop",["Keep","ArithmeticError"],["Wrong","IndexError"]]}]});
    for (route, payload) in [
        ("Drop", "drop_into_unit_case"),
        ("Keep", "preserve_ArithmeticError"),
    ] {
        request["bindings"]["arithmetic_failure"] = json!(route);
        let report = check(&fixture, &request, &declarations).unwrap();
        assert_eq!(report["connections"][0]["failure_route"], "checked");
        assert_eq!(report["connections"][0]["payload"], payload);
    }
    for route in ["Wrong", "Missing"] {
        request["bindings"]["arithmetic_failure"] = json!(route);
        let error = check(&fixture, &request, &declarations).unwrap_err();
        assert!(error.detail().contains("/bindings/arithmetic_failure"));
        assert!(error.detail().contains("/bindings/returns"));
    }
}

#[test]
fn guard_failures_check_option_shape_case_presence_and_parameter_payloads() {
    let fixture = Fixture::new();
    let mut request = guarded_request();
    request["bindings"]["returns"] = json!("Result<i8,Error>");
    request["bindings"]["guards"] = json!([{"when":true,"fail":["fail","WithValue","x"]}]);
    request["bindings"]["success"] = json!({"ops":[],"term":["ok",1]});
    let declarations = json!({"types":[{"name":"Error","variant":["Unit",["WithValue","i8"],["BadWidth","i16"]]}]});
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(
        report["connections"][0]["payload_value"],
        "checked_parameter"
    );
    for fail in [
        json!(["fail"]),
        json!(["fail", "Missing"]),
        json!(["fail", "Unit", "x"]),
        json!(["fail", "WithValue"]),
        json!(["fail", "BadWidth", "x"]),
    ] {
        request["bindings"]["guards"][0]["fail"] = fail;
        let error = check(&fixture, &request, &declarations).unwrap_err();
        assert!(error.detail().contains("/bindings/guards/0/fail"));
        assert!(error.detail().contains("/bindings/returns"));
    }
    request["bindings"]["returns"] = json!("Option<i8>");
    request["bindings"]["guards"][0]["fail"] = json!(["fail"]);
    request["bindings"]["success"]["term"] = json!(["return", "x"]);
    let error = check(&fixture, &request, &declarations).unwrap_err();
    assert!(error.detail().contains("/bindings/success/term/1"));
    assert!(error.detail().contains("/bindings/returns"));
    request["bindings"]["success"]["term"] = json!(["return", ["some", "x"]]);
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(report["connections"][0]["case"], "None");
    // Checking this return connection does not establish full composition.
    assert_eq!(report["composition"], "partial");
}

#[test]
fn named_branch_preflight_agrees_with_actual_compilation_and_execution() {
    for accepted in [false, true] {
        let fixture = Fixture::new();
        let (mut request, declarations) = named_branch();
        let mut base = super::frame();
        base["types"] = declarations["types"].clone();
        let (code, report) = cli(&fixture.dir, &["try", &base.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{report}");
        if accepted {
            let (code, report) = cli(&fixture.dir, &["commit", "c1"]);
            assert_eq!(code, 0, "{report}");
        } else {
            request["base"] = json!("d1@r1");
        }
        let (code, report) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{report}");
        assert_eq!(report["kernel"], "valid");
        for (arg, expected) in [(json!({"Present":9}), 9), (json!("Absent"), 17)] {
            let (code, output) = cli(
                &fixture.dir,
                &["call", "checked", &arg.to_string(), "--on", "c2"],
            );
            assert_eq!(code, 0, "{output}");
            assert_eq!(output["result"], json!({"Ok":expected}));
        }
    }
}

#[test]
fn checked_arithmetic_error_routes_agree_with_kernel_and_vm() {
    for (route, expected) in [
        ("Drop", json!({"Err":"Drop"})),
        (
            "Keep",
            json!({"Err":{"Keep":{"ArithmeticError":"Overflow"}}}),
        ),
    ] {
        let fixture = Fixture::new();
        let mut base = super::frame();
        base["types"] = json!([{"name":"Errors","variant":["Drop",["Keep","ArithmeticError"]]}]);
        let (code, report) = cli(&fixture.dir, &["try", &base.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{report}");
        let mut request = planning_request();
        request["base"] = json!("d1@r1");
        request["bindings"]["params"] = json!([["x", "i8"]]);
        request["bindings"]["returns"] = json!("Result<i8,Errors>");
        request["bindings"]["arithmetic_failure"] = json!(route);
        let (code, report) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{report}");
        assert_eq!(report["kernel"], "valid");
        let name = request["scope"][0].as_str().unwrap();
        for (argument, expected) in [("100", expected), ("5", json!({"Ok":7}))] {
            let (code, output) = cli(&fixture.dir, &["call", name, argument, "--on", "c2"]);
            assert_eq!(code, 0, "{output}");
            assert_eq!(output["result"], expected);
        }
    }
}

#[test]
fn branch_coverage_obeys_fragment_case_ceiling_without_limiting_error_lookup() {
    let fixture = Fixture::new();
    let cases: Vec<_> = (0..65).map(|index| json!(format!("Case{index}"))).collect();
    let declarations = json!({"types":[{"name":"Many","variant":cases}]});
    let mut branch = branch_request();
    branch["bindings"]["params"][0][1] = json!("Many");
    let error = check(&fixture, &branch, &declarations).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("64-case"));
    let mut pipeline = planning_request();
    pipeline["bindings"]["returns"] = json!("Result<i64,Many>");
    pipeline["bindings"]["arithmetic_failure"] = json!("Case64");
    let report = check(&fixture, &pipeline, &declarations).unwrap();
    assert_eq!(report["connections"][0]["case"], "Case64");
}

#[test]
fn interface_context_construction_charges_large_and_malformed_declaration_lists() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let request = parse_request(&bytes(&predicate_request(json!(false)))).unwrap();
    let mut cases = Vec::new();
    for key in [
        "delete",
        "types",
        "consts",
        "fns",
        "functions",
        "patch",
        "edit",
        "tests",
    ] {
        cases.push(json!({key:vec![json!(false);64]}));
    }
    cases.push(json!({"types":[{"name":"Large", "record":(0..64).map(|i|json!([format!("v{i}"),"i8"])).collect::<Vec<_>>()}]}));
    cases.push(json!({"fns":[{"fn":"wide", "params":(0..64).map(|i|json!([format!("v{i}"),"i8"])).collect::<Vec<_>>(),"returns":"i8"}]}));
    for declarations in cases {
        let before = declarations.clone();
        let mut budget = Budget::limited(std::time::Duration::from_secs(2), 16);
        let error = interfaces::check(
            head.program(),
            &names,
            declarations.as_object().unwrap(),
            &request,
            &mut budget,
        )
        .unwrap_err();
        assert_eq!(
            error.code(),
            AgentErrorCode::ResidualLimit,
            "{declarations}: {error}"
        );
        assert_eq!(budget.usage()["charged_work"], 16);
        assert!(budget.checkpoint().is_err());
        assert_eq!(declarations, before);
    }
}

#[test]
fn interface_context_cancellation_precedes_later_declared_type_errors() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let request = parse_request(&bytes(&predicate_request(json!(false)))).unwrap();
    let declarations = json!({"types":[{"name":"Large","variant":(0..64).map(|i|json!(format!("V{i}"))).collect::<Vec<_>>()}],
        "consts":[{"name":"broken","type":"MissingType","value":0}]});
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 16);
    let error = interfaces::check(
        head.program(),
        &names,
        declarations.as_object().unwrap(),
        &request,
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    let error = interfaces::check(
        head.program(),
        &names,
        declarations.as_object().unwrap(),
        &request,
        &mut Budget::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::FrameInvalid);
    assert!(error.detail().contains("MissingType"), "{error}");
}
