use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn conditional(condition: &Value, dialect: bool) -> Value {
    let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":[
        {"name":"entry","ops":[],"term":["cond",condition,["yes","x"],["no","x"]]},
        {"name":"yes","params":[["value","i8"]],"ops":[],"term":["return","value"]},
        {"name":"no","params":[["value","i8"]],"ops":[],"term":["return","value"]}]}]});
    if dialect {
        source["afx"] = json!(1);
    }
    source
}

#[test]
fn source_conditional_inputs_require_bool_before_generation() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let source = conditional(&json!("x"), dialect);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/fns/0/blocks/0/term/1")
                && error.detail().contains("requires bool")
                && error.detail().contains("i8"),
            "{error}"
        );
    }
}

fn switched(ty: &str, cases: &Value, dialect: bool) -> Value {
    let mut source = conditional(&json!("ready"), dialect);
    source["fns"][0]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!(["item", ty]));
    let mut term = json!(["switch", "item"]);
    term.as_array_mut()
        .unwrap()
        .extend(cases.as_array().unwrap().iter().cloned());
    source["fns"][0]["blocks"][0]["term"] = term;
    source
}

#[test]
fn switch_case_sets_reject_missing_duplicate_and_unexpected_keys_at_authored_uses() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        for (cases, locator, reason) in [
            (
                json!([["Some", "yes", "$"]]),
                "/term/1",
                "missing cases: builtin None",
            ),
            (
                json!([["Some", "yes", "$"], ["Some", ["no", "x"]]]),
                "/term/3/0",
                "duplicate switch case",
            ),
            (
                json!([["Some", "yes", "$"], ["Ok", "no", "x"]]),
                "/term/3/0",
                "not a case of Option<i8>",
            ),
        ] {
            let error = check(
                &fixture,
                &predicate_request(json!(false)),
                &switched("Option<i8>", &cases, dialect),
            )
            .unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
            assert!(
                error.detail().contains(locator) && error.detail().contains(reason),
                "{error}"
            );
        }
    }
}

#[test]
fn switch_selector_records_and_scalars_cannot_be_treated_as_variants() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        for ty in ["i8", "bool", "Pack"] {
            let mut source = switched(ty, &json!([["Slot", "yes", "x"]]), dialect);
            source["types"] = json!([{"name":"Pack","record":[["Slot","i8"]]}]);
            let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
            assert!(
                error.detail().contains("/term/1")
                    && error.detail().contains("switch selector has type")
                    && error.detail().contains("nominal variant"),
                "{error}"
            );
        }
    }
}

#[test]
fn nominal_case_qualifiers_follow_ordinary_rules_and_keep_reserved_member_names() {
    for dialect in [false, true] {
        let fixture = Fixture::new();
        let mut source = switched(
            "Choice",
            &json!([["Choice.Some", "yes", "$"], ["Other.None", ["no", "x"]]]),
            dialect,
        );
        source["types"] = json!([{"name":"Choice","variant":[["Some","i8"],["None",null]]}]);
        check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let handle = result["handle"].as_str().unwrap();
        let (code, result) = cli(&fixture.dir, &["commit", handle]);
        assert_eq!(code, 0, "{result}");
        let mut patch = json!({"af1":1,"patch":[{"fn":"helper","blocks":{}}]});
        if dialect {
            patch["afx"] = json!(1);
        }
        // Canonical Member(Some) must remain distinct from Builtin(Some).
        check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
        for item in [json!({"Some":-7}), json!("None")] {
            let (code, result) = cli(
                &fixture.dir,
                &["call", "helper", "9", "false", &item.to_string()],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(
                result["result"],
                if item.is_object() {
                    json!(-7)
                } else {
                    json!(9)
                }
            );
        }
        let mut duplicate = source.clone();
        duplicate["fns"][0]["blocks"][0]["term"][3][0] = json!("Other.Some");
        let error = check(&fixture, &predicate_request(json!(false)), &duplicate).unwrap_err();
        assert!(
            error.detail().contains("duplicate switch case member Some"),
            "{error}"
        );
    }
}

#[test]
fn retained_conditions_use_current_types_and_exact_parameter_identity() {
    let fixture = Fixture::new();
    let definition = conditional(&json!("ready"), false);
    let (code, result) = cli(&fixture.dir, &["try", &definition.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    for dialect in [false, true] {
        let mut patch = json!({"af1":1,"patch":[{"fn":"helper","params":[["x","i8"],["ready","i8"]],"blocks":{}}]});
        if dialect {
            patch["afx"] = json!(1);
        }
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("retained block `helper.entry`")
                && error.detail().contains("/condition")
                && error.detail().contains("requires bool"),
            "{error}"
        );
        patch["patch"][0]["params"] = json!([["x", "i8"], ["flag", "bool"]]);
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("helper.ready") && error.detail().contains("original identity"),
            "{error}"
        );
        patch["patch"][0]["blocks"] =
            json!({"entry":{"ops":[],"term":["cond","flag",["yes","x"],["no","x"]]}});
        check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
    }
}

#[test]
fn retained_switch_cases_follow_current_members_without_rebinding_old_ids() {
    let fixture = Fixture::new();
    let mut definition = switched(
        "Choice",
        &json!([["Present", "yes", "$"], ["Empty", "no", "x"]]),
        false,
    );
    definition["types"] = json!([
        {"name":"Choice","variant":[["Present","i8"],["Empty",null]]},
        {"name":"Other","variant":[["Present","i8"],["Empty",null]]}
    ]);
    let (code, result) = cli(&fixture.dir, &["try", &definition.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    for dialect in [false, true] {
        let mut patch = json!({"af1":1,"patch":[{"fn":"helper","blocks":{}}],"types":[{"name":"Choice","variant":[["Present","i8"],["Empty",null],["Extra",null]]}]});
        if dialect {
            patch["afx"] = json!(1);
        }
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("retained block `helper.entry`")
                && error.detail().contains("missing cases: member Extra"),
            "{error}"
        );
        patch["types"][0]["variant"] = json!([["Present", "i8"]]);
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("/cases/")
                && error.detail().contains("member Empty is not a case"),
            "{error}"
        );
        patch.as_object_mut().unwrap().remove("types");
        patch["patch"][0]["params"] = json!([["x", "i8"], ["ready", "bool"], ["item", "Other"]]);
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("retained block `helper.entry`")
                && error.detail().contains("not a case of Other"),
            "{error}"
        );
        patch["patch"][0]["params"][2][1] = json!("Option<i8>");
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("member identity")
                && error.detail().contains("not a case of Option<i8>"),
            "{error}"
        );
    }
}

#[test]
fn unknown_input_types_and_incomplete_case_declarations_stay_deferred() {
    let fixture = Fixture::new();
    let source = conditional(&json!("unknown"), true);
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    assert!(
        report["declared_body_control_flow"]["deferred"]
            .as_array()
            .unwrap()
            .contains(&json!("/fns/0/blocks/0/term/1"))
    );
    let mut source = switched("Choice", &json!([["Empty", "yes", "x"]]), true);
    source["types"] = json!([{"name":"Choice","variant":[["Empty",null],["Present"]]}]);
    source["fns"][0]["blocks"][2]["unreachable"] = json!(true);
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    assert!(
        report["declared_body_control_flow"]["deferred"]
            .as_array()
            .unwrap()
            .contains(&json!("/fns/0/blocks/0/term/1"))
    );
    assert_eq!(report["composition"], "partial");
}

#[test]
fn nested_conditions_and_retained_operation_results_use_known_types() {
    let fixture = Fixture::new();
    for (condition, valid) in [
        (json!(["tuple_get", 1, ["tuple", "x", "ready"]]), true),
        (json!(["tuple_get", 0, ["tuple", "x", "ready"]]), false),
    ] {
        let source = conditional(&condition, true);
        let result = check(&fixture, &predicate_request(json!(false)), &source);
        if valid {
            result.unwrap();
            let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
            assert_eq!(code, 0, "{result}");
        } else {
            let error = result.unwrap_err();
            assert!(
                error.detail().contains("/term/1") && error.detail().contains("requires bool"),
                "{error}"
            );
        }
    }
    let mut source = conditional(&json!("start.flag"), false);
    source["fns"][0]["blocks"].as_array_mut().unwrap().insert(0,json!({"name":"start","ops":[["flag","const",{"type":"bool","value":true}]],"term":["br","entry"]}));
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let handle = result["handle"].as_str().unwrap();
    let (code, result) = cli(&fixture.dir, &["commit", handle]);
    assert_eq!(code, 0, "{result}");
    for dialect in [false, true] {
        let mut patch = json!({"af1":1,"patch":[{"fn":"helper","blocks":{"start":{"ops":[["flag","const",{"type":"i8","value":1}]],"term":["br","entry"]}}}]});
        if dialect {
            patch["afx"] = json!(1);
        }
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("retained block `helper.entry`")
                && error.detail().contains("/condition")
                && error.detail().contains("requires bool"),
            "{error}"
        );
        patch["patch"][0]["blocks"]["start"]["ops"][0][2] = json!({"type":"bool","value":false});
        check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
    }
}
