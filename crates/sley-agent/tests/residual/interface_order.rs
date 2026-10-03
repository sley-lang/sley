use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli, interface_call_tests};
use serde_json::{Value, json};

fn order(report: &Value) -> &Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == "/bindings/success/ops")
        .unwrap()
}

#[test]
fn forward_and_self_reads_refuse_with_use_and_definition_before_writes() {
    let fixture = Fixture::new();
    for (ops, use_at, definition) in [
        (
            json!([["out", "not", "later"], ["later", "not", "ready"]]),
            "/bindings/success/ops/0/2",
            "/bindings/success/ops/1",
        ),
        (
            json!([["out", "not", "out#0"]]),
            "/bindings/success/ops/0/2",
            "/bindings/success/ops/0",
        ),
        (
            json!([
                ["out", "tuple_get", 0, ["tuple", "later"]],
                ["later", "not", "ready"]
            ]),
            "/bindings/success/ops/0/3/1",
            "/bindings/success/ops/1",
        ),
        (
            json!([{ "name":"out","opcode":"tuple_get","operands":[0,["tuple","later#0"]]},["later","not","ready"]]),
            "/bindings/success/ops/0/operands/1/1",
            "/bindings/success/ops/1",
        ),
        (
            json!([["!", "if", "later"], ["later", "not", "ready"]]),
            "/bindings/success/ops/0/2",
            "/bindings/success/ops/1",
        ),
        (
            json!([
                ["!WithValue", "if", true, "later"],
                ["later", "not", "ready"]
            ]),
            "/bindings/success/ops/0/3",
            "/bindings/success/ops/1",
        ),
    ] {
        let mut request = predicate_request(json!(false));
        request["bindings"]["success"]["ops"] = ops;
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
        let detail = result["detail"].as_str().unwrap();
        assert!(detail.contains(use_at), "{result}");
        assert!(detail.contains(definition), "{result}");
        assert!(detail.contains("before its definition"), "{result}");
        assert_eq!(bytes(&request), before);
    }
    for path in [".sley/candidates", ".sley/drafts", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}

#[test]
fn earlier_cell_results_have_checked_order_and_real_vm_execution() {
    let fixture = Fixture::new();
    let mut request = predicate_request(json!(false));
    request["bindings"]["returns"] = json!("Option<bool>");
    request["bindings"]["success"] = json!({"ops":[["packed","cell","ready"],["out","cell_get","packed#0"]],"term":["return",["some","out"]]});
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(order(&report)["local_definition_order"], "checked");
    assert_eq!(
        order(&report)["reads"][1],
        json!({"at":"/bindings/success/ops/1/2","binding":"packed#0","definition":"/bindings/success/ops/0"})
    );
    assert_eq!(report["composition"], "partial");
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
    assert_eq!(result["result"], json!({"Some":true}));
}

#[test]
fn callee_and_constant_immediates_are_not_future_local_reads() {
    for constant in [false, true] {
        let fixture = Fixture::new();
        let mut declarations = interface_call_tests::helpers();
        declarations["consts"] = json!([{"name":"later","type":"i8","value":9}]);
        let (code, result) = cli(
            &fixture.dir,
            &["try", &declarations.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        let mut request = predicate_request(json!(false));
        request["base"] = json!("d1@r1");
        request["bindings"]["success"] = if constant {
            json!({"ops":[["out","const","later"],["later","not",true]],"term":["return",["some","out"]]})
        } else {
            json!({"ops":[["out","call","echo","x"],["echo","not",true]],"term":["return",["some","out"]]})
        };
        let report = check(&fixture, &request, &declarations).unwrap();
        assert_eq!(order(&report)["local_definition_order"], "checked");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", "3", "true", "--on", "c2"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!({"Some":if constant {9}else{3}}));
    }
}

#[test]
fn typed_literal_payloads_do_not_create_local_reads() {
    let fixture = Fixture::new();
    let mut request = predicate_request(json!(false));
    request["bindings"]["success"]["ops"] =
        json!([["packed","tuple",{"type":"text","value":"later"}],["later","not",true]]);
    let report = check(&fixture, &request, &json!({})).unwrap();
    assert_eq!(order(&report)["local_definition_order"], "checked");
    assert_eq!(order(&report)["reads"], json!([]));
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
}

#[test]
fn unrecognized_syntax_keeps_partial_order_evidence() {
    let fixture = Fixture::new();
    for (op, deferred) in [
        (
            json!(["out", "future_opcode", "later"]),
            "/bindings/success/ops/0",
        ),
        (
            json!(["out", "tuple", ["future_expression"]]),
            "/bindings/success/ops/0/2",
        ),
    ] {
        let mut request = predicate_request(json!(false));
        request["bindings"]["success"]["ops"] = json!([op, ["later", "not", true]]);
        let report = check(&fixture, &request, &json!({})).unwrap();
        assert_eq!(order(&report)["local_definition_order"], "partial");
        assert_eq!(order(&report)["deferred_operands"], json!([deferred]));
        assert_eq!(report["composition"], "partial");
    }
}
