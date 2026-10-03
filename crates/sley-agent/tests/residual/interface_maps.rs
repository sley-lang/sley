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
        ["y", "i8"],
        ["ready", "bool"],
        ["source", "Map<i8,bool>"]
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

fn run(fixture: &Fixture, candidate: &str, x: &str, y: &str) -> Value {
    let (code, result) = cli(
        &fixture.dir,
        &[
            "call",
            "checked",
            x,
            y,
            "true",
            "[[1,true],[2,false]]",
            "--on",
            candidate,
        ],
    );
    assert_eq!(code, 0, "{result}");
    result["result"].clone()
}

#[test]
fn map_constructor_aliases_preserve_duplicate_failures_and_pair_types() {
    for word in ["map", "map_new", "36"] {
        let fixture = Fixture::new();
        let request = request(
            "Result<Map<i8,bool>,DuplicateKeyError>",
            json!([["out", word, "x", "ready", "y", false]]),
            json!(["return", "out"]),
        );
        let before = bytes(&request);
        let report = check(&fixture, &request, &json!({})).unwrap();
        let connection = at(&report, "/bindings/success/ops/0");
        assert_eq!(connection["expression_types"], "connections_checked");
        assert_eq!(connection["maps"][0]["key_traits"], "checked");
        assert_eq!(connection["maps"][0]["entries"], "not_evaluated");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        assert_eq!(
            run(&fixture, "c1", "1", "2"),
            json!({"Ok":[[1,true],[2,false]]})
        );
        assert_eq!(
            run(&fixture, "c1", "1", "1"),
            json!({"Err":{"DuplicateKeyError":"DuplicateKey"}})
        );
        assert_eq!(bytes(&request), before);
    }
}

#[test]
fn map_access_aliases_execute_and_preserve_original_entries() {
    for (get, has, insert, remove) in [
        ("map_get", "map_has", "map_insert", "map_remove"),
        ("map_get", "map_contains", "map_insert", "map_remove"),
        ("37", "38", "39", "40"),
    ] {
        let fixture = Fixture::new();
        let request = request(
            "Tuple<Option<bool>,bool,Map<i8,bool>,Map<i8,bool>,Map<i8,bool>>",
            json!([
                ["found",get,"source","x"],["present",has,"source","x"],
                {"name":"updated","op":insert,"args":["source","x",false]},
                ["removed",remove,"source","x"]
            ]),
            json!([
                "return",
                ["tuple", "found", "present", "updated", "removed", "source"]
            ]),
        );
        let report = check(&fixture, &request, &json!({})).unwrap();
        for index in 0..4 {
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
            run(&fixture, "c1", "1", "2"),
            json!([{"Some":true},true,[[1,false],[2,false]],[[2,false]],[[1,true],[2,false]]])
        );
        assert_eq!(
            run(&fixture, "c1", "3", "2"),
            json!([
                "None",
                false,
                [[1, true], [2, false], [3, false]],
                [[1, true], [2, false]],
                [[1, true], [2, false]]
            ])
        );
    }
}

#[test]
fn map_context_types_empty_maps_and_later_key_value_anchors() {
    for (ops, term, expected) in [
        (
            json!([{"name":"empty","op":"map","args":[],"type":"Result<Map<i8,bool>,DuplicateKeyError>"}]),
            json!(["return", "empty"]),
            json!({"Ok":[]}),
        ),
        (
            json!([["out", "map", 1, true, "y", "ready"]]),
            json!(["return", "out"]),
            json!({"Ok":[[1,true],[2,true]]}),
        ),
    ] {
        let fixture = Fixture::new();
        let request = request("Result<Map<i8,bool>,DuplicateKeyError>", ops, term);
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
        assert_eq!(run(&fixture, "c1", "1", "2"), expected);
    }
    let fixture = Fixture::new();
    let request = request(
        "Result<Map<Tuple<i8,bool>,i8>,DuplicateKeyError>",
        json!([]),
        json!(["return", ["map", ["tuple", "x", "ready"], "y"]]),
    );
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(
        at(&report, "/bindings/success/term/1")["maps"][0]["key_traits"],
        "checked"
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(run(&fixture, "c1", "1", "2"), json!({"Ok":[[[1,true],2]]}));
}

#[test]
fn map_operand_and_wrapper_conflicts_refuse_before_publication() {
    let fixture = Fixture::new();
    for (op, pointer) in [
        (json!(["out", "map", "x"]), "/ops/0"),
        (json!(["out", "map", "x", true, "ready", false]), "/ops/0/4"),
        (json!(["out", "map", "x", true, "y", "x"]), "/ops/0/5"),
        (json!(["out", "map", 128, true, "x", false]), "/ops/0/2"),
        (json!(["out", "map_get", "source"]), "/ops/0"),
        (json!(["out", "map_get", "x", "y"]), "/ops/0/2"),
        (json!(["out", "map_get", "source", "ready"]), "/ops/0/3"),
        (json!(["out", "map_has", "source", "ready"]), "/ops/0/3"),
        (json!(["out", "map_insert", "source", "x"]), "/ops/0"),
        (json!(["out", "map_insert", "source", "x", "y"]), "/ops/0/4"),
        (
            json!(["out", "map_insert", "source", 128, true]),
            "/ops/0/3",
        ),
        (json!(["out", "map_remove", "source", true]), "/ops/0/3"),
        (json!(["out", "map_remove", "source", "x", true]), "/ops/0"),
        (json!(["out", "map_insert?", "source", "x", true]), "/ops/0"),
        (json!(["out", "map_remove?", "source", "x"]), "/ops/0"),
        (json!(["out", "map_has?", "source", "x"]), "/ops/0"),
        (
            json!({"name":"out","op":"map","type":"Map<i8,bool>","args":["x",true]}),
            "/ops/0",
        ),
        (
            json!({"name":"out","op":"map","type":"Result<Map<i8,bool>,IndexError>","args":["x",true]}),
            "/ops/0",
        ),
        (
            json!({"name":"out","op":"map_get","type":"bool","args":["source","x"]}),
            "/ops/0",
        ),
    ] {
        let request = request("i8", json!([op]), json!(["return", "x"]));
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
fn canonical_map_key_traits_reject_structural_nonkeys_and_resolve_named_keys() {
    let fixture = Fixture::new();
    for key in [
        "f32",
        "Vec<i8>",
        "Tuple<i8,f64>",
        "Cell<i8>",
        "Map<i8,bool>",
        "fn(i8)->i8",
    ] {
        let mut request = request(
            "i8",
            json!([["out", "map", "key", "x"]]),
            json!(["return", "x"]),
        );
        request["bindings"]["params"]
            .as_array_mut()
            .unwrap()
            .push(json!(["key", key]));
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(
            error.code(),
            AgentErrorCode::ResidualConstraintConflict,
            "{key}: {error}"
        );
        if key == "Cell<i8>" {
            assert!(error.detail().contains("local cell"), "{error}");
            assert!(
                error.detail().contains("/bindings/success/ops/0/2"),
                "{error}"
            );
        } else {
            assert!(error.detail().contains("map key"), "{error}");
            assert!(error.detail().contains("inadmissible"), "{error}");
        }
    }
    let declarations = json!({"types":[{"name":"Key","record":[["id","i8"]]}]});
    let mut request = request(
        "i8",
        json!([["out", "map", "key", "x"]]),
        json!(["return", "x"]),
    );
    request["bindings"]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!(["key", "Key"]));
    let report = check(&fixture, &request, &declarations).unwrap();
    let connection = at(&report, "/bindings/success/ops/0");
    assert_eq!(connection["expression_types"], "connections_checked");
    assert_eq!(connection["maps"][0]["key_traits"], "checked");
    assert_eq!(connection["deferred_expressions"], json!([]));
}

#[test]
fn checked_maps_preserve_named_failures_and_evaluate_values_before_duplicates() {
    let fixture = Fixture::new();
    let declarations = json!({"af1":1,"types":[{"name":"Errors","variant":["Missing",["Dup","DuplicateKeyError"],["Math","ArithmeticError"]]}]});
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let mut request = request(
        "Result<Map<i8,i8>,Errors>",
        json!([["out", "map?Dup", "x", "y", "x", ["neg?Math", "y"]]]),
        json!(["ok", "out"]),
    );
    request["base"] = json!("d1@r1");
    let report = check(&fixture, &request, &declarations).unwrap();
    let propagation = &at(&report, "/bindings/success/ops/0")["propagation"];
    assert_eq!(propagation[0]["route"], "Math");
    assert_eq!(propagation[1]["route"], "Dup");
    assert_eq!(propagation[1]["failure_route"], "preserve_in_named_case");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(
        run(&fixture, "c2", "1", "-128"),
        json!({"Err":{"Math":{"ArithmeticError":"Overflow"}}})
    );
    assert_eq!(
        run(&fixture, "c2", "1", "3"),
        json!({"Err":{"Dup":{"DuplicateKeyError":"DuplicateKey"}}})
    );
    request["bindings"]["success"]["ops"][0][1] = json!("map?Math");
    assert!(
        check(&fixture, &request, &declarations)
            .unwrap_err()
            .detail()
            .contains("failure payload conflicts")
    );

    let fixture = Fixture::new();
    let (code, result) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    request["bindings"]["returns"] = json!("Result<bool,Errors>");
    request["bindings"]["success"]["ops"] = json!([["out", "map_get?Missing", "source", "x"]]);
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(run(&fixture, "c2", "3", "2"), json!({"Err":"Missing"}));
    assert_eq!(run(&fixture, "c2", "1", "2"), json!({"Ok":true}));
    request["bindings"]["success"]["ops"][0][1] = json!("map_get?Dup");
    assert!(
        check(&fixture, &request, &declarations)
            .unwrap_err()
            .detail()
            .contains("failure payload conflicts")
    );
}

#[test]
fn map_inference_revisits_evidence_and_keeps_unknown_operands_partial() {
    let fixture = Fixture::new();
    let request = request(
        "Result<Map<i8,i8>,DuplicateKeyError>",
        json!([["out", "map", 1, 2, "x", "y"]]),
        json!(["return", "out"]),
    );
    let report = check(&fixture, &request, &json!({})).unwrap();
    let connection = at(&report, "/bindings/success/ops/0");
    assert_eq!(connection["maps"].as_array().unwrap().len(), 1);
    assert_eq!(connection["expression_types"], "connections_checked");
    assert_eq!(connection["deferred_expressions"], json!([]));
    let revisited = self::request(
        "bool",
        json!([]),
        json!(["return", ["eq", ["map", 1, true], ["map", "x", "ready"]]]),
    );
    let report = check(&fixture, &revisited, &json!({})).unwrap();
    let connection = at(&report, "/bindings/success/term/1");
    assert_eq!(connection["expression_types"], "connections_checked");
    assert_eq!(connection["maps"].as_array().unwrap().len(), 2);
    assert_eq!(connection["maps"][0]["at"], "/bindings/success/term/1/1");
    assert_eq!(connection["maps"][0]["key_type"], "i8");
    assert_eq!(connection["maps"][0]["key_traits"], "checked");
    for (returns, op) in [
        (
            "Result<Map<i8,bool>,DuplicateKeyError>",
            json!(["out", "map?"]),
        ),
        (
            "Option<bool>",
            json!(["out", "map_get?", ["future_expression"], "x"]),
        ),
    ] {
        let request = self::request(
            returns,
            json!([op]),
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
        let mut bad = request;
        bad["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
        assert!(
            check(&fixture, &bad, &json!({}))
                .unwrap_err()
                .detail()
                .contains("bare checked propagation")
        );
    }
}

#[test]
fn map_checked_annotations_preserve_bare_native_failure_routes() {
    for (returns, operation, term, good, bad) in [
        (
            "Result<Map<i8,bool>,DuplicateKeyError>",
            json!({"name":"out","op":"map?","args":["x",true,"y",false],"type":"Result<Map<i8,bool>,DuplicateKeyError>"}),
            json!(["ok", "out"]),
            json!({"Ok":[[1,true],[2,false]]}),
            json!({"Err":{"DuplicateKeyError":"DuplicateKey"}}),
        ),
        (
            "Option<bool>",
            json!({"name":"out","op":"map_get?","args":["source","x"],"type":"Option<bool>"}),
            json!(["return", ["some", "out"]]),
            json!({"Some":true}),
            json!("None"),
        ),
    ] {
        let fixture = Fixture::new();
        let mut request = request(returns, json!([operation]), term);
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
        assert_eq!(run(&fixture, "c1", "1", "2"), good);
        let (x, y) = if returns.starts_with("Result") {
            ("1", "1")
        } else {
            ("3", "2")
        };
        assert_eq!(run(&fixture, "c1", x, y), bad);
        request["bindings"]["success"]["ops"][0]["type"] =
            json!(if returns.starts_with("Result") {
                "Map<i8,bool>"
            } else {
                "bool"
            });
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert!(
            error.detail().contains("/bindings/success/ops/0"),
            "{error}"
        );
    }
}

#[test]
fn map_walks_enforce_shared_work_and_expression_depth() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let mut request = parse_request(&bytes(&request(
        "Option<bool>",
        json!([]),
        json!(["return", ["map_get", "source", "x"]]),
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
    let mut value = json!("source");
    for _ in 0..34 {
        value = json!(["map_remove", value, "x"]);
    }
    request.bindings["success"]["term"] = json!(["return", ["map_get", value, "x"]]);
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
