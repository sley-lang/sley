use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};

fn request(ty: &str, expression: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["params"] = json!([["a", ty], ["b", ty]]);
    request["bindings"]["returns"] = json!("bool");
    request["bindings"]["success"] = json!({"ops":[],"term":["return",null]});
    request["bindings"]["success"]["term"][1] = expression;
    request
}

fn connection(report: &Value) -> &Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == "/bindings/success/term/1")
        .unwrap()
}

fn execute(fixture: &Fixture, request: &Value, candidate: &str, a: &Value, b: &Value) -> Value {
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let (code, result) = cli(
        &fixture.dir,
        &[
            "call",
            "checked",
            &a.to_string(),
            &b.to_string(),
            "--on",
            candidate,
        ],
    );
    assert_eq!(code, 0, "{result}");
    result["result"].clone()
}

#[test]
fn comparison_aliases_and_scalar_types_agree_with_actual_vm_results() {
    for words in [
        ["eq", "ne", "lt", "le", "gt", "ge"],
        [
            "equal",
            "not_equal",
            "less_than",
            "less_equal",
            "greater_than",
            "greater_equal",
        ],
        ["96", "97", "98", "99", "100", "101"],
    ] {
        for (ty, a, b) in [
            ("i8", json!(-1), json!(2)),
            ("u64", json!(1), json!(2)),
            ("bool", json!(false), json!(true)),
            ("text", json!("a"), json!("b")),
            ("bytes", json!("0x01"), json!("0x02")),
            ("f32", json!(1), json!(2)),
            ("f64", json!(1), json!(2)),
        ] {
            let fixture = Fixture::new();
            let mut request = request(
                ty,
                json!(["tuple", "eqv", "nev", "ltv", "lev", "gtv", "gev"]),
            );
            request["bindings"]["returns"] = json!("Tuple<bool,bool,bool,bool,bool,bool>");
            request["bindings"]["success"]["ops"] = json!([
                ["eqv",words[0],"a","b"], ["nev",words[1],"a","b"],
                ["ltv",words[2],"a","b"], ["lev",words[3],"a","b"],
                ["gtv",words[4],"a","b"],
                {"name":"gev","opcode":words[5],"operands":["a","b"],"type":"bool"}
            ]);
            let before = bytes(&request);
            let report = check(&fixture, &request, &json!({})).unwrap();
            let comparisons: Vec<_> = report["connections"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|entry| entry["comparisons"].as_array())
                .flatten()
                .collect();
            assert_eq!(comparisons.len(), 6);
            assert!(
                comparisons.iter().all(
                    |entry| entry["eligibility"] == "checked" && entry["operands"] == "checked"
                )
            );
            assert_eq!(
                execute(&fixture, &request, "c1", &a, &b),
                json!([false, true, true, true, false, false])
            );
            assert_eq!(bytes(&request), before);
        }
    }
}

#[test]
fn equality_accepts_hashable_composites_that_scalar_ordering_refuses() {
    for (ty, value) in [
        ("unit", json!(null)),
        ("Tuple<i8,bool>", json!([1, true])),
        ("Vec<i8>", json!([1, 2])),
        ("Option<i8>", json!({"Some":1})),
        ("Result<i8,bool>", json!({"Ok":1})),
        ("Map<i8,bool>", json!([[1, true]])),
    ] {
        let fixture = Fixture::new();
        let request = request(ty, json!(["eq", "a", "b"]));
        let report = check(&fixture, &request, &json!({})).unwrap();
        assert_eq!(
            connection(&report)["comparisons"][0]["eligibility"],
            "checked"
        );
        assert_eq!(
            execute(&fixture, &request, "c1", &value, &value),
            json!(true)
        );
        for word in ["lt", "le", "gt", "ge"] {
            let error = check(
                &fixture,
                &self::request(ty, json!([word, "a", "b"])),
                &json!({}),
            )
            .unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
            assert!(
                error
                    .detail()
                    .contains("ordered comparison requires a scalar"),
                "{error}"
            );
        }
    }
}

#[test]
fn float_composites_and_nonhashable_values_refuse_without_publication() {
    let fixture = Fixture::new();
    for ty in [
        "Tuple<f32>",
        "Vec<f64>",
        "Option<f32>",
        "Result<i8,f64>",
        "Map<i8,f32>",
        "Cell<i8>",
    ] {
        for word in ["eq", "ne"] {
            let request = request(ty, json!([word, "a", "b"]));
            let (code, result) = cli(
                &fixture.dir,
                &["residual", "try", &request.to_string(), "--no-test"],
            );
            assert_eq!(code, 2, "{ty}: {result}");
            assert_eq!(
                result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
                "{result}"
            );
            assert!(
                result["detail"]
                    .as_str()
                    .unwrap()
                    .contains("/bindings/success/term/1"),
                "{result}"
            );
        }
    }
    for path in [".sley/candidates", ".sley/drafts", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}

#[test]
fn scalar_nan_comparisons_preserve_ieee_results() {
    for ty in ["f32", "f64"] {
        for (word, expected) in [
            ("eq", false),
            ("ne", true),
            ("lt", false),
            ("le", false),
            ("gt", false),
            ("ge", false),
        ] {
            let fixture = Fixture::new();
            let request = request(ty, json!([word, "a", "b"]));
            assert_eq!(
                execute(&fixture, &request, "c1", &json!("NaN"), &json!(1)),
                json!(expected)
            );
        }
    }
}

#[test]
fn comparison_evidence_distinguishes_unknown_widths_from_unknown_expressions() {
    let fixture = Fixture::new();
    for (expr, eligibility, operands) in [
        (json!(["eq", 1, 2]), "unresolved", "deferred"),
        (json!(["lt", 1, "a"]), "checked", "checked"),
        (
            json!(["eq", ["future_expression"], "b"]),
            "checked",
            "deferred",
        ),
    ] {
        let report = check(&fixture, &request("i8", expr), &json!({})).unwrap();
        let entry = connection(&report);
        assert_eq!(entry["comparisons"][0]["eligibility"], eligibility);
        assert_eq!(entry["comparisons"][0]["operands"], operands);
        assert_eq!(entry["comparisons"].as_array().unwrap().len(), 1);
        assert_eq!(
            entry["expression_types"],
            if operands == "checked" {
                "connections_checked"
            } else {
                "partial"
            }
        );
        assert_eq!(entry["comparisons"][0]["evaluated"], false);
    }
    let error = check(
        &fixture,
        &request("i8", json!(["eq", 128, "a"])),
        &json!({}),
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
}

#[test]
fn named_equality_uses_accepted_and_draft_members_and_defers_incomplete_shapes() {
    for accepted in [false, true] {
        let fixture = Fixture::new();
        let declarations = json!({"af1":1,"types":[{"name":"Key","record":[["id","i8"]]}]});
        let (code, result) = cli(
            &fixture.dir,
            &["try", &declarations.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        if accepted {
            let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
            assert_eq!(code, 0, "{result}");
        }
        let mut request = request("Key", json!(["eq", "a", "b"]));
        if !accepted {
            request["base"] = json!("d1@r1");
        }
        let declared = if accepted { json!({}) } else { declarations };
        let report = check(&fixture, &request, &declared).unwrap();
        assert_eq!(
            connection(&report)["comparisons"][0]["eligibility"],
            "checked"
        );
        assert_eq!(
            execute(&fixture, &request, "c2", &json!({"id":7}), &json!({"id":7})),
            json!(true)
        );
        let bad = json!({"types":[{"name":"Key","record":[["id","Cell<i8>"]]}]});
        let error = check(&fixture, &request, &bad).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("TYPE_NOT_HASHABLE"), "{error}");
        let malformed = json!({"types":[{"name":"Key","record":["id"]}]});
        let error = check(&fixture, &request, &malformed).unwrap_err();
        super::interface_closure_tests::assert_incomplete(&error, "/bindings/params/");
    }
}

#[test]
fn comparison_named_traversal_obeys_shared_budget_and_type_depth() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(head.program(), &NameMap::default());
    let declarations = json!({"types":[{"name":"Key","record":[["id","i8"]]}]});
    let mut usage = Vec::new();
    for ty in ["i8", "Key"] {
        let request = parse_request(&bytes(&request(ty, json!(["eq", "a", "b"])))).unwrap();
        let mut budget = Budget::default();
        interfaces::check(
            head.program(),
            &names,
            declarations.as_object().unwrap(),
            &request,
            &mut budget,
        )
        .unwrap();
        usage.push(budget.usage()["charged_work"].as_u64().unwrap());
    }
    assert!(usage[1] > usage[0], "{usage:?}");
    let request = parse_request(&bytes(&request("Key", json!(["eq", "a", "b"])))).unwrap();
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), usage[0]);
    for _ in 0..2 {
        let error = interfaces::check(
            head.program(),
            &names,
            declarations.as_object().unwrap(),
            &request,
            &mut budget,
        )
        .unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    }
    let types:Vec<_> = (0..70).map(|i|json!({"name":format!("Key{i}"),"record":[["next",if i==69 {"i8".into()} else {format!("Key{}",i+1)}]]})).collect();
    let request = self::request("Key0", json!(["eq", "a", "b"]));
    let error = check(&fixture, &request, &json!({"types":types})).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit, "{error}");
    let declarations = json!({"types":[{"name":"Key","record":[["next","Key"]]}]});
    let error = check(
        &fixture,
        &self::request("Key", json!(["eq", "a", "b"])),
        &declarations,
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(error.detail().contains("TYPE_DEFINITION_CYCLE"), "{error}");
}

#[test]
fn named_float_fields_keep_the_existing_vm_eligibility_policy() {
    // The VM float exclusion walks type expressions and named arguments, not
    // named member definitions. Do not silently broaden it in preflight.
    let fixture = Fixture::new();
    let declarations = json!({"af1":1,"types":[{"name":"Reading","record":[["value","f32"]]}]});
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let mut request = request("Reading", json!(["eq", "a", "b"]));
    request["base"] = json!("d1@r1");
    let report = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(
        connection(&report)["comparisons"][0]["eligibility"],
        "checked"
    );
    assert_eq!(
        execute(
            &fixture,
            &request,
            "c2",
            &json!({"value":1}),
            &json!({"value":1})
        ),
        json!(true)
    );
}
