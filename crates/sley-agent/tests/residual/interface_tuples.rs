use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};

fn request(returns: &str, ops: Value, term: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!(returns);
    request["bindings"]["success"] = json!({"ops":[],"term":[]});
    request["bindings"]["success"]["ops"] = ops;
    request["bindings"]["success"]["term"] = term;
    request
}

fn at<'a>(report: &'a Value, pointer: &str) -> &'a Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == pointer)
        .unwrap_or_else(|| panic!("missing {pointer}: {report}"))
}

#[test]
fn tuple_aliases_and_object_operands_connect_to_local_results_and_vm() {
    for (construct, extract) in [
        ("tuple", "tuple_get"),
        ("tuple_new", "tuple_get"),
        ("16", "17"),
    ] {
        let fixture = Fixture::new();
        let request = request(
            "Option<bool>",
            json!([
                ["pair",construct,"x","ready"],
                {"name":"flag","opcode":extract,"operands":[1,"pair#0"]}
            ]),
            json!(["return", ["some", "flag"]]),
        );
        let before = bytes(&request);
        let report = check(&fixture, &request, &json!({})).unwrap();
        for pointer in [
            "/bindings/success/ops/0",
            "/bindings/success/ops/1",
            "/bindings/success/term/1",
        ] {
            assert_eq!(
                at(&report, pointer)["expression_types"],
                "connections_checked",
                "{report}"
            );
        }
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        for ready in [true, false] {
            let (code, result) = cli(
                &fixture.dir,
                &["call", "checked", "7", &ready.to_string(), "--on", "c1"],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], json!({"Some":ready}));
        }
        assert_eq!(bytes(&request), before);
    }
}

#[test]
fn tuple_context_checks_nested_literals_and_empty_shape_without_rewriting() {
    for (returns, value, expected) in [
        (
            "Option<Tuple<i8,bool,Tuple<u16>>>",
            json!(["tuple", 127, "ready", ["tuple", 65535]]),
            json!([127, true, [65535]]),
        ),
        ("Option<Tuple<>>", json!(["tuple"]), json!([])),
    ] {
        let fixture = Fixture::new();
        let request = request(returns, json!([]), json!(["return", ["some", value]]));
        let before = bytes(&request);
        let report = check(&fixture, &request, &json!({})).unwrap();
        assert_eq!(
            at(&report, "/bindings/success/term/1")["expression_types"],
            "connections_checked"
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", "3", "true", "--on", "c1"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!({"Some":expected}));
        assert_eq!(bytes(&request), before);
    }
}

#[test]
fn tuple_conflicts_refuse_before_publication_at_authored_operands() {
    let fixture = Fixture::new();
    for (returns, value, pointer) in [
        (
            "Option<bool>",
            json!(["tuple_get", 0, "x"]),
            "/bindings/success/term/1/1/2",
        ),
        (
            "Option<bool>",
            json!(["tuple_get", 1, ["tuple", "ready"]]),
            "/bindings/success/term/1/1/1",
        ),
        (
            "Option<bool>",
            json!(["tuple_get", -1, ["tuple", "ready"]]),
            "/bindings/success/term/1/1/1",
        ),
        (
            "Option<bool>",
            json!(["tuple_get", 4_294_967_296_u64, ["tuple", "ready"]]),
            "/bindings/success/term/1/1/1",
        ),
        (
            "Option<bool>",
            json!(["tuple_get", "0", ["tuple", "ready"]]),
            "/bindings/success/term/1/1/1",
        ),
        (
            "Option<bool>",
            json!(["tuple_get", true, ["tuple", "ready"]]),
            "/bindings/success/term/1/1/1",
        ),
        (
            "Option<bool>",
            json!(["tuple_get", 0]),
            "/bindings/success/term/1/1",
        ),
        (
            "Option<bool>",
            json!(["tuple_get", 0, ["tuple", "ready"], "x"]),
            "/bindings/success/term/1/1",
        ),
        (
            "Option<bool>",
            json!(["tuple_get", 0, ["tuple", "x"]]),
            "/bindings/success/term/1/1",
        ),
        (
            "Option<bool>",
            json!(["tuple", "ready"]),
            "/bindings/success/term/1/1",
        ),
        (
            "Option<Tuple<i8,bool>>",
            json!(["tuple", "x"]),
            "/bindings/success/term/1/1",
        ),
        (
            "Option<Tuple<i8,bool>>",
            json!(["tuple", 128, "ready"]),
            "/bindings/success/term/1/1/1",
        ),
        (
            "Option<Tuple<i8,bool>>",
            json!(["tuple", "ready", "x"]),
            "/bindings/success/term/1/1/1",
        ),
        (
            "Option<bool>",
            json!(["tuple_get?", 0, ["tuple", "ready"]]),
            "/bindings/success/term/1/1",
        ),
    ] {
        let request = request(returns, json!([]), json!(["return", ["some", value]]));
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{request}: {result}");
        assert_eq!(
            result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
            "{result}"
        );
        assert!(
            result["detail"].as_str().unwrap().contains(pointer),
            "{result}"
        );
    }
    for path in [".sley/candidates", ".sley/drafts", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}

#[test]
fn tuple_extraction_preserves_option_propagation_and_operand_failure_order() {
    let fixture = Fixture::new();
    let mut option = request(
        "Option<i8>",
        json!([]),
        json!(["return", ["some", ["tuple_get?", 0, ["tuple", "maybe"]]]]),
    );
    option["bindings"]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!(["maybe", "Option<i8>"]));
    let report = check(&fixture, &option, &json!({})).unwrap();
    assert_eq!(
        at(&report, "/bindings/success/term/1")["propagation"][0]["failure_route"],
        "preserve_failure"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &option.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    for value in [json!("None"), json!({"Some":12})] {
        let (code, result) = cli(
            &fixture.dir,
            &[
                "call",
                "checked",
                "3",
                "true",
                &value.to_string(),
                "--on",
                "c1",
            ],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], value);
    }

    let fixture = Fixture::new();
    let mut ordered = request(
        "Result<i8,ArithmeticError>",
        json!([]),
        json!([
            "ok",
            [
                "tuple_get",
                1,
                ["tuple", ["div?", "x", "divisor"], ["neg?", "x"]]
            ]
        ]),
    );
    ordered["bindings"]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!(["divisor", "i8"]));
    let report = check(&fixture, &ordered, &json!({})).unwrap();
    let term = at(&report, "/bindings/success/term/1");
    assert_eq!(term["expression_types"], "connections_checked");
    assert_eq!(term["propagation"][0]["at"], "/bindings/success/term/1/2/1");
    assert_eq!(term["propagation"][1]["at"], "/bindings/success/term/1/2/2");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &ordered.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (x, divisor, expected) in [
        (
            "-128",
            "0",
            json!({"Err":{"ArithmeticError":"DivideByZero"}}),
        ),
        ("-128", "1", json!({"Err":{"ArithmeticError":"Overflow"}})),
        ("4", "2", json!({"Ok":-4})),
    ] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", x, "true", divisor, "--on", "c1"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
}

#[test]
fn unknown_tuple_elements_keep_deferred_evidence_even_with_result_context() {
    let fixture = Fixture::new();
    for (returns, value, deferred) in [
        (
            "Option<i8>",
            json!(["tuple_get", 0, ["tuple", 1]]),
            "/bindings/success/term/1/1/2/1",
        ),
        (
            "Option<Tuple<i8,bool>>",
            json!(["tuple", ["future_expression"], "ready"]),
            "/bindings/success/term/1/1/1",
        ),
        (
            "Option<bool>",
            json!(["tuple_get", 0, ["tuple", "ready", ["future_expression"]]]),
            "/bindings/success/term/1/1/2/2",
        ),
    ] {
        let request = request(returns, json!([]), json!(["return", ["some", value]]));
        let report = check(&fixture, &request, &json!({})).unwrap();
        let term = at(&report, "/bindings/success/term/1");
        assert_eq!(term["expression_types"], "partial");
        assert!(
            term["deferred_expressions"]
                .as_array()
                .unwrap()
                .contains(&json!(deferred)),
            "{report}"
        );
        assert_eq!(report["composition"], "partial");
    }
}

#[test]
fn tuple_annotations_keep_wrapped_and_continuation_types_distinct() {
    let fixture = Fixture::new();
    let mut request = request(
        "Option<i8>",
        json!([
            {"name":"packed","op":"tuple","type":"Tuple<Option<i8>>","args":["maybe"]},
            {"name":"out","op":"tuple_get?","type":"Option<i8>","args":[0,"packed"]}
        ]),
        json!(["return", ["some", "out"]]),
    );
    request["bindings"]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!(["maybe", "Option<i8>"]));
    let report = check(&fixture, &request, &json!({})).unwrap();
    for pointer in [
        "/bindings/success/ops/0",
        "/bindings/success/ops/1",
        "/bindings/success/term/1",
    ] {
        assert_eq!(
            at(&report, pointer)["expression_types"],
            "connections_checked"
        );
    }
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for value in [json!("None"), json!({"Some":12})] {
        let (code, result) = cli(
            &fixture.dir,
            &[
                "call",
                "checked",
                "3",
                "true",
                &value.to_string(),
                "--on",
                "c1",
            ],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], value);
    }
    request["bindings"]["success"]["ops"][1]["type"] = json!("i8");
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(
        error.detail().contains("/bindings/success/ops/1/type"),
        "{error}"
    );
    request["bindings"]["success"]["ops"][1]["type"] = json!("Option<i8>");
    request["bindings"]["success"]["ops"][0]["type"] = json!("Tuple<Option<u8>>");
    let error = check(&fixture, &request, &json!({})).unwrap_err();
    assert!(
        error.detail().contains("/bindings/success/ops/0/args/0"),
        "{error}"
    );
    assert!(
        error.detail().contains("/bindings/success/ops/0/type"),
        "{error}"
    );
}

#[test]
fn tuple_walks_charge_shared_budget_and_enforce_expression_depth() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let mut request = parse_request(&bytes(&request(
        "Option<bool>",
        json!([]),
        json!(["return", ["some", ["tuple_get", 0, ["tuple", "ready"]]]]),
    )))
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
    // Bypass the request envelope's JSON-depth bound to exercise the checker.
    let mut value = json!("ready");
    for _ in 0..34 {
        value = json!(["tuple_get", 0, ["tuple", value]]);
    }
    request.bindings["success"]["term"] = json!(["return", ["some", value]]);
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
}
