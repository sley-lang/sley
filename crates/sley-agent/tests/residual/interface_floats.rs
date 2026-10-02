use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};

fn request(ty: &str, ops: Value, term: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["params"] = json!([
        ["a", ty],
        ["b", ty],
        ["c", ty],
        ["integer", "i8"],
        ["ready", "bool"]
    ]);
    request["bindings"]["returns"] = json!(ty);
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
        .unwrap()
}

fn run(fixture: &Fixture, a: &Value, b: &Value, c: &Value) -> Value {
    let (code, result) = cli(
        &fixture.dir,
        &[
            "call",
            "checked",
            &a.to_string(),
            &b.to_string(),
            &c.to_string(),
            "3",
            "true",
            "--on",
            "c1",
        ],
    );
    assert_eq!(code, 0, "{result}");
    result["result"].clone()
}

#[test]
fn floating_aliases_check_exact_widths_and_execute_all_six_operations() {
    for ty in ["f32", "f64"] {
        for words in [
            ["fadd", "fsub", "fmul", "fdiv", "fneg", "fma"],
            [
                "float_add",
                "float_sub",
                "float_mul",
                "float_div",
                "float_neg",
                "float_fma",
            ],
            ["80", "81", "82", "83", "84", "85"],
        ] {
            let fixture = Fixture::new();
            let mut request = request(
                ty,
                json!([
                    ["add",words[0],"a","b"],["sub",words[1],"a","b"],
                    ["mul",words[2],"a","b"],["div",words[3],"a","b"],
                    ["neg",words[4],"a"],{"name":"fused","opcode":words[5],"operands":["a","b","c"],"type":ty}
                ]),
                json!([
                    "return",
                    ["tuple", "add", "sub", "mul", "div", "neg", "fused"]
                ]),
            );
            request["bindings"]["returns"] = json!(format!("Tuple<{ty},{ty},{ty},{ty},{ty},{ty}>"));
            let before = bytes(&request);
            let report = check(&fixture, &request, &json!({})).unwrap();
            for index in 0..6 {
                assert_eq!(
                    at(&report, &format!("/bindings/success/ops/{index}"))["expression_types"],
                    "connections_checked"
                );
            }
            let (code, result) = cli(
                &fixture.dir,
                &["residual", "try", &request.to_string(), "--no-test"],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["kernel"], "valid");
            assert_eq!(
                run(&fixture, &json!(6), &json!(2), &json!(3)),
                json!([8.0, 4.0, 12.0, 3.0, -6.0, 15.0])
            );
            assert_eq!(bytes(&request), before);
        }
    }
}

#[test]
fn floating_arity_width_annotation_and_checked_suffix_conflicts_refuse_before_writes() {
    let fixture = Fixture::new();
    for (op, pointer) in [
        (json!(["out", "fadd", "a"]), "/ops/0"),
        (json!(["out", "fdiv", "a", "b", "c"]), "/ops/0"),
        (json!(["out", "fneg", "a", "b"]), "/ops/0"),
        (json!(["out", "fma", "a", "b"]), "/ops/0"),
        (json!(["out", "fma", "a", "b", "integer"]), "/ops/0/4"),
        (json!(["out", "fmul", "integer", "a"]), "/ops/0/2"),
        (json!(["out", "fadd", "a", "ready"]), "/ops/0/3"),
        (
            json!(["out","fadd","a",{"type":"f64","value":1}]),
            "/ops/0/3",
        ),
        (
            json!(["out","fadd","a",{"type":"f32","value":"bad"}]),
            "/ops/0/3/value",
        ),
        (
            json!({"name":"out","op":"fadd","args":["a","b"],"type":"f64"}),
            "/ops/0/args/0",
        ),
        (
            json!({"name":"out","op":"fadd","args":["a","b"],"type":"Result<f32,ArithmeticError>"}),
            "/ops/0",
        ),
        (json!(["out", "fdiv?", "a", "b"]), "/ops/0"),
        (json!(["out", "fadd?", 1, 2]), "/ops/0"),
    ] {
        let request = request("f32", json!([op]), json!(["return", "a"]));
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
fn floating_result_and_sibling_context_resolve_literals_without_defaulting_widths() {
    let fixture = Fixture::new();
    let request = request(
        "f32",
        json!([["out", "fma", ["fadd", 1, 2], 2, "a"]]),
        json!(["return", "out"]),
    );
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        at(&report, "/bindings/success/ops/0")["expression_types"],
        "connections_checked"
    );
    assert_eq!(
        at(&report, "/bindings/success/ops/0")["deferred_expressions"],
        json!([])
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(run(&fixture, &json!(3), &json!(2), &json!(1)), json!(9.0));

    let fixture = Fixture::new();
    let unanchored = self::request(
        "f64",
        json!([["unused", "fadd", 1, 2]]),
        json!(["return", ["fadd", 1, 2]]),
    );
    let report = check(&fixture, &unanchored, &json!({})).unwrap();
    assert_eq!(
        at(&report, "/bindings/success/ops/0")["expression_types"],
        "partial"
    );
    assert_eq!(
        at(&report, "/bindings/success/term/1")["expression_types"],
        "connections_checked"
    );
    let unresolved = self::request(
        "f32",
        json!([]),
        json!(["return", ["fadd", ["future_expression"], 1]]),
    );
    let report = check(&fixture, &unresolved, &json!({})).unwrap();
    assert_eq!(
        at(&report, "/bindings/success/term/1")["expression_types"],
        "partial"
    );
    assert_eq!(
        at(&report, "/bindings/success/term/1")["deferred_expressions"],
        json!(["/bindings/success/term/1/1"])
    );
}

#[test]
fn division_retains_profile_nonfinite_results_and_canonical_zero() {
    for ty in ["f32", "f64"] {
        let fixture = Fixture::new();
        let request = request(ty, json!([]), json!(["return", ["fdiv", "a", "b"]]));
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        for (a, b, expected) in [
            (json!(1), json!(0), json!("inf")),
            (json!(-1), json!(0), json!("-inf")),
            (json!(0), json!(0), json!("NaN")),
            (json!("inf"), json!("inf"), json!("NaN")),
        ] {
            assert_eq!(run(&fixture, &a, &b, &json!(0)), expected);
        }
        let fixture = Fixture::new();
        let request = self::request(ty, json!([]), json!(["return", ["fneg", "a"]]));
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        let result = run(&fixture, &json!(0.0), &json!(0), &json!(0));
        // VM_EXTENDED_OPCODE_PROFILE_V1 / S20-210 canonicalizes every zero
        // result to positive zero. Preflight must retain that existing policy.
        assert_eq!(result.as_f64().unwrap().to_bits(), 0.0_f64.to_bits());
    }
}

#[test]
fn fused_multiply_add_remains_distinct_from_multiply_then_add() {
    for (ty, a, c) in [
        ("f32", 4_097_u64, -16_785_408_i64),
        ("f64", 134_217_729, -18_014_398_777_917_440),
    ] {
        let fixture = Fixture::new();
        let mut request = request(
            ty,
            json!([]),
            json!([
                "return",
                [
                    "tuple",
                    ["fma", "a", "b", "c"],
                    ["fadd", ["fmul", "a", "b"], "c"]
                ]
            ]),
        );
        request["bindings"]["returns"] = json!(format!("Tuple<{ty},{ty}>"));
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
        assert_eq!(
            run(&fixture, &json!(a), &json!(a), &json!(c)),
            json!([1.0, 0.0])
        );
        assert_eq!(bytes(&request), before);
    }
}

#[test]
fn typed_nonfinite_literals_are_checked_without_changing_the_request_grammar() {
    for literal in ["NaN", "inf", "-inf"] {
        let fixture = Fixture::new();
        let request = request(
            "f32",
            json!([]),
            json!(["return",["fadd",{"type":"f32","value":literal},1]]),
        );
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
        assert_eq!(
            run(&fixture, &json!(0), &json!(0), &json!(0)),
            json!(literal)
        );
        assert_eq!(bytes(&request), before);
    }
    let fixture = Fixture::new();
    let request = request("f32", json!([]), json!(["return", ["fadd", 1.5, "a"]]));
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_PARSE");
}

#[test]
fn floating_expressions_obey_shared_work_and_depth_limits() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let mut request = parse_request(&bytes(&request(
        "f32",
        json!([]),
        json!(["return", ["fadd", "a", "b"]]),
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
    let mut value = json!("a");
    for _ in 0..34 {
        value = json!(["fneg", value]);
    }
    request.bindings["success"]["term"] = json!(["return", value]);
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
