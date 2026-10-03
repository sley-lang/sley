use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};

fn request(returns: &str, ops: Value, term: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["params"] = json!([
        ["x", "i8"],
        ["ready", "bool"],
        ["values", "Vec<i8>"],
        ["index", "u64"]
    ]);
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

fn run(fixture: &Fixture, candidate: &str, x: &str, index: &str) -> Value {
    let (code, result) = cli(
        &fixture.dir,
        &[
            "call", "checked", x, "true", "[1,2]", index, "--on", candidate,
        ],
    );
    assert_eq!(code, 0, "{result}");
    result["result"].clone()
}

#[test]
fn vector_aliases_infer_later_element_context_and_execute_reads_and_lengths() {
    for (new, len, get) in [
        ("vec", "vec_len", "vec_get"),
        ("vector_new", "vector_len", "vector_get"),
        ("32", "33", "34"),
    ] {
        let fixture = Fixture::new();
        let request = request(
            "Tuple<Option<i8>,u64>",
            json!([
                ["packed",new,1,"x",3],
                ["length",len,"packed"],
                {"name":"found","opcode":get,"operands":["packed","index"],"type":"Option<i8>"}
            ]),
            json!(["return", ["tuple", "found", "length"]]),
        );
        let before = bytes(&request);
        let report = check(&fixture, &request, &json!({})).unwrap();
        for pointer in [
            "/bindings/success/ops/0",
            "/bindings/success/ops/1",
            "/bindings/success/ops/2",
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
        for (index, expected) in [
            ("0", json!({"Some":1})),
            ("1", json!({"Some":7})),
            ("2", json!({"Some":3})),
            ("9", json!("None")),
        ] {
            assert_eq!(run(&fixture, "c1", "7", index), json!([expected, 3]));
        }
        assert_eq!(bytes(&request), before);
    }
}

#[test]
fn contextual_empty_and_nested_vectors_preserve_exact_vm_values() {
    for (returns, value, expected) in [
        ("Vec<i8>", json!(["vec"]), json!([])),
        (
            "Vec<Vec<i8>>",
            json!(["vec", ["vec"], ["vec", 127, -128]]),
            json!([[], [127, -128]]),
        ),
        (
            "Vec<Tuple<i8,bool>>",
            json!(["vec", ["tuple", 127, true], ["tuple", -128, false]]),
            json!([[127, true], [-128, false]]),
        ),
    ] {
        let fixture = Fixture::new();
        let request = request(returns, json!([]), json!(["return", value]));
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
        assert_eq!(run(&fixture, "c1", "3", "0"), expected);
    }
}

#[test]
fn vector_arity_type_index_and_replacement_conflicts_refuse_before_writes() {
    let fixture = Fixture::new();
    for (operation, pointer) in [
        (json!(["out", "vec", "x", "ready"]), "/ops/0/3"),
        (json!(["out", "vec", 128, "x"]), "/ops/0/2"),
        (json!(["out", "vec_len"]), "/ops/0"),
        (json!(["out", "vec_len", "x"]), "/ops/0/2"),
        (json!(["out", "vec_len", "values", 0]), "/ops/0"),
        (json!(["out", "vec_get", "values"]), "/ops/0"),
        (json!(["out", "vec_get", "values", 0, "x"]), "/ops/0"),
        (json!(["out", "vec_get", "x", 0]), "/ops/0/2"),
        (json!(["out", "vec_get", "values", -1]), "/ops/0/3"),
        (
            json!(["out","vec_get","values",{"type":"u32","value":0}]),
            "/ops/0/3",
        ),
        (json!(["out", "vec_set", "values", 0]), "/ops/0"),
        (json!(["out", "vec_set", "values", 0, "ready"]), "/ops/0/4"),
        (json!(["out", "vec_set", "values", 0, 128]), "/ops/0/4"),
        (
            json!(["out", "vec_set", ["vec", 128], 0, "x"]),
            "/ops/0/2/1",
        ),
        (json!(["out", "vec_set", "values", true, "x"]), "/ops/0/3"),
        (
            json!({"name":"out","op":"vec_set","args":["values",0,"x"],"type":"Result<Vec<i8>,ArithmeticError>"}),
            "/ops/0",
        ),
        (
            json!({"name":"out","op":"vec_get","args":["values",0],"type":"i8"}),
            "/ops/0",
        ),
        (
            json!({"name":"out","op":"vec","args":["x"],"type":"Tuple<i8>"}),
            "/ops/0",
        ),
        (json!(["out", "vec?"]), "/ops/0"),
        (json!(["out", "vec_len?", "values"]), "/ops/0"),
    ] {
        let request = request("i8", json!([operation]), json!(["return", "x"]));
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
            result["detail"]
                .as_str()
                .unwrap()
                .contains(&format!("/bindings/success{pointer}")),
            "{result}"
        );
    }
    for path in [".sley/candidates", ".sley/drafts", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}

#[test]
fn checked_vector_access_keeps_native_failures_and_raw_annotation_types() {
    for (returns, op, annotation, term, good, bad) in [
        (
            "Option<i8>",
            "vec_get?",
            "Option<i8>",
            json!(["return", ["some", "out"]]),
            json!({"Some":1}),
            json!("None"),
        ),
        (
            "Result<Vec<i8>,IndexError>",
            "vec_set?",
            "Result<Vec<i8>,IndexError>",
            json!(["ok", "out"]),
            json!({"Ok":[9,2]}),
            json!({"Err":{"IndexError":"OutOfBounds"}}),
        ),
    ] {
        let fixture = Fixture::new();
        let mut args = json!(["values", "index"]);
        if op.starts_with("vec_set") {
            args.as_array_mut().unwrap().push(json!("x"));
        }
        let request = request(
            returns,
            json!([{"name":"out","op":op,"args":args,"type":annotation}]),
            term,
        );
        let before = bytes(&request);
        let report = check(&fixture, &request, &json!({})).unwrap();
        let connection = at(&report, "/bindings/success/ops/0");
        assert_eq!(connection["expression_types"], "connections_checked");
        assert_eq!(
            connection["propagation"][0]["failure_route"],
            "preserve_failure"
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        assert_eq!(run(&fixture, "c1", "9", "0"), good);
        assert_eq!(run(&fixture, "c1", "9", "2"), bad);
        assert_eq!(bytes(&request), before);
    }
}

#[test]
fn named_vector_failures_preserve_payloads_and_replacement_evaluation_order() {
    let fixture = Fixture::new();
    let declarations = json!({"af1":1,"types":[{"name":"Errors","variant":["Missing",["Bounds","IndexError"],["Math","ArithmeticError"]]}]});
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let mut request = request(
        "Result<Vec<i8>,Errors>",
        json!([[
            "out",
            "vec_set?Bounds",
            "values",
            "index",
            ["neg?Math", "x"]
        ]]),
        json!(["ok", "out"]),
    );
    request["base"] = json!("d1@r1");
    let report = check(&fixture, &request, &declarations).unwrap();
    let propagation = &at(&report, "/bindings/success/ops/0")["propagation"];
    assert_eq!(propagation[0]["route"], "Math");
    assert_eq!(propagation[1]["route"], "Bounds");
    assert_eq!(propagation[1]["failure_route"], "preserve_in_named_case");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(
        run(&fixture, "c2", "-128", "9"),
        json!({"Err":{"Math":{"ArithmeticError":"Overflow"}}})
    );
    assert_eq!(
        run(&fixture, "c2", "3", "9"),
        json!({"Err":{"Bounds":{"IndexError":"OutOfBounds"}}})
    );
    assert_eq!(run(&fixture, "c2", "3", "0"), json!({"Ok":[-3,2]}));
    request["bindings"]["success"]["ops"][0][1] = json!("vec_set?Math");
    assert!(
        check(&fixture, &request, &declarations)
            .unwrap_err()
            .detail()
            .contains("failure payload conflicts")
    );

    // The preceding residual trial advanced its draft. Use a fresh bound draft
    // for this independently authored function with a different return type.
    let fixture = Fixture::new();
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    request["bindings"]["returns"] = json!("Result<i8,Errors>");
    request["bindings"]["success"]["ops"] = json!([["out", "vec_get?Missing", "values", "index"]]);
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(
        at(&report, "/bindings/success/ops/0")["propagation"][0]["failure_route"],
        "drop_into_unit_case"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(run(&fixture, "c2", "3", "9"), json!({"Err":"Missing"}));
    request["bindings"]["success"]["ops"][0][1] = json!("vec_get?Bounds");
    assert!(
        check(&fixture, &request, &declarations)
            .unwrap_err()
            .detail()
            .contains("failure payload conflicts")
    );
}

#[test]
fn later_vector_elements_resolve_evidence_once_without_rewriting_arithmetic() {
    let fixture = Fixture::new();
    let request = request(
        "Result<Vec<i8>,ArithmeticError>",
        json!([["packed", "vec", ["add?", 1, 2], ["neg?", "x"]]]),
        json!(["ok", "packed"]),
    );
    let before = bytes(&request);
    let report = check(&fixture, &request, &json!({})).unwrap();
    let connection = at(&report, "/bindings/success/ops/0");
    assert_eq!(connection["expression_types"], "connections_checked");
    assert_eq!(connection["deferred_expressions"], json!([]));
    assert_eq!(connection["propagation"].as_array().unwrap().len(), 2);
    assert_eq!(
        connection["propagation"][0]["at"],
        "/bindings/success/ops/0/2"
    );
    assert_eq!(connection["propagation"][0]["unwrapped_type"], "i8");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(run(&fixture, "c1", "3", "0"), json!({"Ok":[3,-3]}));
    assert_eq!(bytes(&request), before);
}

#[test]
fn unresolved_vectors_keep_partial_evidence_but_check_intrinsic_failure_routes() {
    let fixture = Fixture::new();
    for (returns, operation) in [
        ("Option<i8>", json!(["out", "vec_get?", ["vec"], 0])),
        (
            "Result<Vec<i8>,IndexError>",
            json!(["out", "vec_set?", ["vec"], 0, ["future_expression"]]),
        ),
    ] {
        let request = request(
            returns,
            json!([operation]),
            json!(["return", ["future_expression"]]),
        );
        let report = check(&fixture, &request, &json!({})).unwrap();
        let connection = at(&report, "/bindings/success/ops/0");
        assert_eq!(connection["expression_types"], "partial");
        assert_eq!(
            connection["propagation"][0]["failure_route"],
            "preserve_failure"
        );
        assert!(connection["propagation"][0]["unwrapped_type"].is_null());
        let mut bad = request.clone();
        bad["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
        assert!(
            check(&fixture, &bad, &json!({}))
                .unwrap_err()
                .detail()
                .contains("bare checked propagation")
        );
    }
    let request = request(
        "Vec<i8>",
        json!([]),
        json!(["return", ["vec", ["future_expression"], 1]]),
    );
    let report = check(&fixture, &request, &json!({})).unwrap();
    let connection = at(&report, "/bindings/success/term/1");
    assert_eq!(connection["expression_types"], "partial");
    assert_eq!(
        connection["deferred_expressions"],
        json!(["/bindings/success/term/1/1"])
    );
}

#[test]
fn vector_set_aliases_preserve_the_original_vector_with_raw_results() {
    for word in ["vec_set", "vector_set", "35"] {
        let fixture = Fixture::new();
        let request = request(
            "Tuple<Vec<i8>,Result<Vec<i8>,IndexError>>",
            json!([["updated", word, "values", "index", "x"]]),
            json!(["return", ["tuple", "values", "updated"]]),
        );
        let report = check(&fixture, &request, &json!({})).unwrap();
        assert_eq!(
            at(&report, "/bindings/success/ops/0")["expression_types"],
            "connections_checked"
        );
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        assert_eq!(run(&fixture, "c1", "9", "0"), json!([[1,2],{"Ok":[9,2]}]));
        assert_eq!(
            run(&fixture, "c1", "9", "9"),
            json!([[1,2],{"Err":{"IndexError":"OutOfBounds"}}])
        );
    }
}

#[test]
fn vector_walks_enforce_shared_work_and_nesting_limits() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let mut request = parse_request(&bytes(&request(
        "Vec<i8>",
        json!([]),
        json!(["return", ["vec", 1, "x"]]),
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
    let mut value = json!("x");
    for _ in 0..34 {
        value = json!(["vec_get?", ["vec", value], 0]);
    }
    request.bindings["returns"] = json!("Option<i8>");
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
