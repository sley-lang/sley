use super::cli;
use super::interface_tests::predicate_request;
use serde_json::{Value, json};

fn linear(ops: Value, term: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("Result<i8,ArithmeticError>");
    request["bindings"]["success"]["ops"] = ops;
    request["bindings"]["success"]["term"] = term;
    request
}

fn branch(ops: Value, value: Value) -> Value {
    let mut request = json!({"residual":1,"base":"current","operation":"derive",
        "fragment":{"id":"typed_branch_result","version":1},"scope":["checked"],
        "bindings":{"params":[["maybe","Option<i8>"]],"returns":"Result<i8,ArithmeticError>","input":"maybe",
        "cases":[{"case":"Some","payload":["x","i8"],"ops":[],"values":[null]},
        {"case":"None","payload":null,"ops":[],"values":[["const",17]]}],
        "join":{"params":[["answer","i8"]],"ops":[],"term":["ok","answer"]}}});
    request["bindings"]["cases"][0]["ops"] = ops;
    request["bindings"]["cases"][0]["values"][0] = value;
    request
}

fn refuses(request: &Value, at: &str) {
    let fixture = super::interface_call_tests::fixture(true);
    let head = fixture.workspace.head().unwrap().transaction_id();
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(
        result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
        "{result}"
    );
    let detail = result["detail"].as_str().unwrap();
    assert!(
        detail.contains(at) && detail.contains("i8") && detail.contains("i64"),
        "{result}"
    );
    assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    assert_eq!(fixture.workspace.head().unwrap().transaction_id(), head);
}

#[test]
fn nested_checked_payloads_refuse_before_terminal_or_statement_expansion() {
    for (ops, term, at) in [
        (
            json!([["k", "const", 7], ["used", "call", "echo", "k"]]),
            json!(["ok", ["add?", "k", "x"]]),
            "/bindings/success/term/1",
        ),
        (
            json!([
                ["k", "const", 7],
                ["used", "call", "echo", ["add?", "k", "x"]],
                ["fix", "call", "echo", "k"]
            ]),
            json!(["ok", "used"]),
            "/bindings/success/ops/1/3",
        ),
    ] {
        refuses(&linear(ops, term), at);
    }
}

#[test]
fn ordinary_literal_materialization_conflicts_refuse_without_checked_syntax() {
    for literal in [json!(1), json!({"value":1})] {
        let request = linear(
            json!([
                ["k", "const", 7],
                ["sum", "add", "k", literal],
                ["used", "call", "echo", "k"]
            ]),
            json!(["return", "sum"]),
        );
        refuses(&request, "/bindings/success/ops/1/3");
    }
}

#[test]
fn branch_values_and_join_nested_continuations_use_ordinary_inference() {
    let request = branch(
        json!([["k", "const", 7], ["used", "call", "echo", "k"]]),
        json!(["add?", "k", "x"]),
    );
    refuses(&request, "/bindings/cases/0/values/0");
    let mut request = branch(json!([]), json!("x"));
    request["bindings"]["join"]["ops"] = json!([["k", "const", 7], ["used", "call", "echo", "k"]]);
    request["bindings"]["join"]["term"] = json!(["ok", ["add?", "k", "answer"]]);
    refuses(&request, "/bindings/join/term/1");
}

fn executes(request: &Value, inputs: &[(&[&str], Value)]) {
    let fixture = super::interface_call_tests::fixture(true);
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (inputs, expected) in inputs {
        let mut args = vec!["call", "checked"];
        args.extend_from_slice(inputs);
        args.extend_from_slice(&["--on", result["handle"].as_str().unwrap()]);
        let (code, called) = cli(&fixture.dir, &args);
        assert_eq!(code, 0, "{called}");
        assert_eq!(&called["result"], expected);
    }
}

#[test]
fn explicit_types_preserve_nested_checked_and_literal_materialization_vm_results() {
    for constant in [
        json!({"name":"k","op":"const","args":[7],"type":"i8"}),
        json!(["k","const",{"type":"i8","value":7}]),
    ] {
        let request = linear(
            json!([constant, ["used", "call", "echo", "k"]]),
            json!(["ok", ["call", "echo", ["add?", ["add?", "k", "x"], 1]]]),
        );
        executes(
            &request,
            &[
                (&["3", "false"], json!({"Ok":11})),
                (&["-7", "true"], json!({"Ok":1})),
            ],
        );
    }
    for (constant, literal) in [
        (json!(["k","const",{"type":"i8","value":7}]), json!(1)),
        (json!(["k", "const", 7]), json!({"type":"i8","value":1})),
    ] {
        let request = linear(
            json!([
                constant,
                ["sum", "add", "k", literal],
                ["used", "call", "echo", "k"]
            ]),
            json!(["return", "sum"]),
        );
        executes(
            &request,
            &[
                (&["3", "false"], json!({"Ok":8})),
                (&["-7", "true"], json!({"Ok":8})),
            ],
        );
    }
}

#[test]
fn explicit_nested_branch_and_join_payloads_keep_distinct_vm_results() {
    let typed = json!(["k","const",{"type":"i8","value":7}]);
    let request = branch(
        json!([typed, ["used", "call", "echo", "k"]]),
        json!(["add?", "k", "x"]),
    );
    executes(
        &request,
        &[
            (&["{\"Some\":3}"], json!({"Ok":10})),
            (&["\"None\""], json!({"Ok":17})),
        ],
    );
    let mut request = branch(json!([]), json!("x"));
    request["bindings"]["join"]["ops"] = json!([typed, ["used", "call", "echo", "k"]]);
    request["bindings"]["join"]["term"] = json!(["ok", ["add?", "k", "answer"]]);
    executes(
        &request,
        &[
            (&["{\"Some\":3}"], json!({"Ok":10})),
            (&["\"None\""], json!({"Ok":24})),
        ],
    );
}

#[test]
fn named_failure_payloads_keep_the_region_typing_view_and_execute_when_explicit() {
    for explicit in [false, true] {
        let fixture = super::Fixture::new();
        let mut source = super::interface_call_tests::helpers();
        source["types"] =
            json!([{"name":"Errors","variant":[["Value","i8"],["Math","ArithmeticError"]]}]);
        let (code, report) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{report}");
        let (code, report) = cli(&fixture.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{report}");
        let constant = if explicit {
            json!(["k","const",{"type":"i8","value":7}])
        } else {
            json!(["k", "const", 7])
        };
        let mut request = linear(
            json!([constant, ["used", "call", "echo", "k"]]),
            json!(["fail", "Value", ["add?Math", "k", "x"]]),
        );
        request["bindings"]["returns"] = json!("Result<i8,Errors>");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        if explicit {
            assert_eq!(code, 0, "{result}");
            let (code, called) = cli(
                &fixture.dir,
                &[
                    "call",
                    "checked",
                    "3",
                    "false",
                    "--on",
                    result["handle"].as_str().unwrap(),
                ],
            );
            assert_eq!(code, 0, "{called}");
            assert_eq!(called["result"], json!({"Err":{"Value":10}}));
        } else {
            assert_eq!(code, 2, "{result}");
            assert_eq!(
                result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
                "{result}"
            );
            assert!(
                result["detail"]
                    .as_str()
                    .unwrap()
                    .contains("/bindings/success/term/2"),
                "{result}"
            );
            assert!(!fixture.dir.join(".sley/drafts/d2").exists());
        }
    }
}

#[test]
fn absent_ordinary_literal_contexts_refuse_while_explicit_guards_execute() {
    let mut request = predicate_request(json!(["eq", 1, 1]));
    let fixture = super::interface_call_tests::fixture(true);
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
            .contains("/bindings/guards/0/when/1"),
        "{result}"
    );
    assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    request["bindings"]["guards"][0]["when"] =
        json!(["eq",{"type":"i8","value":1},{"type":"i8","value":2}]);
    executes(
        &request,
        &[
            (&["3", "false"], json!({"Some":3})),
            (&["-7", "true"], json!({"Some":-7})),
        ],
    );
}

#[test]
fn missing_literal_contexts_are_reported_once_in_the_owning_expression() {
    let fixture = super::Fixture::new();
    let request = linear(
        json!([
            ["n0", "some", 7],
            ["n1", "some", 7],
            ["n2", "some", 7],
            ["n3", "some", 7]
        ]),
        json!(["ok", "x"]),
    );
    let report = super::interface_tests::check(&fixture, &request, &json!({})).unwrap();
    for index in 0..4 {
        let at = format!("/bindings/success/ops/{index}");
        let entry = report["connections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["at"] == at)
            .unwrap();
        assert_eq!(
            entry["untyped_literal_contexts"],
            json!([format!("{at}/2")])
        );
    }
}
