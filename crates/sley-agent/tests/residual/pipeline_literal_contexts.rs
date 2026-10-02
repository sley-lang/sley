//! Pipeline constraints cannot invent ordinary literal materialization contexts.
use super::interface_tests::check;
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};

fn request(steps: Value, result: Value) -> Value {
    let mut request = json!({"residual":1,"base":"current","operation":"derive",
        "fragment":{"id":"checked_pipeline","version":1},"scope":["checked"],
        "bindings":{"params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>",
        "steps":null,"result":null,"rounding":"toward_zero",
        "arithmetic_failure":{"propagate":true}}});
    request["bindings"]["steps"] = steps;
    request["bindings"]["result"] = result;
    request
}

#[test]
fn pipeline_missing_ordinary_literal_contexts_refuse_before_drafts() {
    for (steps, result, at) in [
        (
            json!([["out", ["add", 1, 2]]]),
            json!("out"),
            "/bindings/steps/0/1/1",
        ),
        (
            json!([["unused", ["add", 1, 2]]]),
            json!("x"),
            "/bindings/steps/0/1/1",
        ),
        (
            json!([["a", ["add", 1, 2]], ["out", ["mul", "a", "x"]]]),
            json!("out"),
            "/bindings/steps/0/1/1",
        ),
        (
            json!([["out", ["add", ["neg", 1], 2]]]),
            json!("out"),
            "/bindings/steps/0/1/1/1",
        ),
    ] {
        let fixture = Fixture::new();
        let request = request(steps, result);
        let before = bytes(&request);
        let head = fixture.workspace.read_head().unwrap().transaction_id();
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(
            result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
            "{result}"
        );
        assert_eq!(result["kernel"], "not_run");
        assert!(result["detail"].as_str().unwrap().contains(at), "{result}");
        for path in [".sley/drafts", ".sley/candidates", ".sley/residual"] {
            assert!(!fixture.dir.join(path).exists(), "{path}");
        }
        assert_eq!(
            fixture.workspace.read_head().unwrap().transaction_id(),
            head
        );
        assert_eq!(bytes(&request), before);
    }
}

#[test]
fn pipeline_library_context_inventory_keeps_original_step_locators() {
    let fixture = Fixture::new();
    let request = request(json!([["out", ["add", 1, 2]]]), json!("out"));
    let before = bytes(&request);
    let report = check(&fixture, &request, &json!({})).unwrap();
    let pipeline = &report["connections"][0];
    assert_eq!(
        pipeline["untyped_literal_contexts"],
        json!(["/bindings/steps/0/1/1", "/bindings/steps/0/1/2"])
    );
    assert_eq!(pipeline["ordinary_literal_contexts"], "partial");
    assert_eq!(bytes(&request), before);
}

#[test]
fn pipeline_context_controls_preserve_explicit_nested_and_endpoint_vm_results() {
    for (steps, result, expected) in [
        (json!([]), json!(7), 7),
        (
            json!([["out",["add",{"type":"i8","value":1},2]]]),
            json!("out"),
            3,
        ),
        (
            json!([["out",["add",1,{"type":"i8","value":2}]]]),
            json!("out"),
            3,
        ),
        (
            json!([["out", ["mul", ["add", 1, 2], "x"]]]),
            json!("out"),
            15,
        ),
        (json!([["out", ["add", "x", 1]]]), json!("out"), 6),
    ] {
        let fixture = Fixture::new();
        let request = request(steps, result);
        let (code, trial) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{trial}");
        assert_eq!(trial["kernel"], "valid");
        let (code, called) = cli(
            &fixture.dir,
            &[
                "call",
                "checked",
                "5",
                "--on",
                trial["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{called}");
        assert_eq!(called["result"], json!({"Ok":expected}));
    }
}
