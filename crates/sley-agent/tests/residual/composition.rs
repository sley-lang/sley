//! Native composition controls for a guard entering a typed branch.

use super::interface_tests::check;
use super::{Fixture, bytes, cli, expand};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn source() -> Value {
    json!({"af1":1,"types":[{"name":"ComposeError","variant":["Skip","ZeroDelta",["Math","ArithmeticError"]]}]})
}

fn request() -> Value {
    json!({"residual":1,"base":"d1@r1","operation":"derive",
        "fragment":{"id":"ordered_guard_chain","version":1},"scope":["fold_state"],
        "bindings":{"params":[["maybe","Option<i8>"],["skip","bool"],["delta","i8"]],
        "returns":"Result<i8,ComposeError>","guards":[
            {"when":"skip","fail":["fail","Skip"]},
            {"when":["eq","delta",0],"fail":["fail","ZeroDelta"]}],
        "success":{"fragment":{"id":"typed_branch_result","version":1},"bindings":{
            "input":"maybe","cases":[
                {"case":"Some","payload":["value","i8"],"ops":[["adjusted","add?Math","value","delta"]],"values":["adjusted"]},
                {"case":"None","payload":null,"ops":[["fallback","neg?Math","delta"]],"values":["fallback"]}],
            "join":{"params":[["merged","i8"]],"ops":[],"term":["ok","merged"]}}}}})
}

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    let (code, result) = cli(&fixture.dir, &["try", &source().to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["draft"], "d1@r1");
    fixture
}

fn call(fixture: &Fixture, handle: &str, maybe: &Value, skip: bool, delta: i8) -> Value {
    let (code, result) = cli(
        &fixture.dir,
        &[
            "call",
            "fold_state",
            &maybe.to_string(),
            if skip { "true" } else { "false" },
            &delta.to_string(),
            "--on",
            handle,
        ],
    );
    assert_eq!(code, 0, "{result}");
    result["result"].clone()
}

#[test]
fn native_guard_to_branch_composition_preserves_priority_boundaries_and_source_maps() {
    let fixture = fixture();
    let before = fixture.workspace.read_head().unwrap().transaction_id();
    let request = request();
    let bytes_before = bytes(&request);
    let interfaces = check(&fixture, &request, &source()).unwrap();
    assert_eq!(interfaces["stage"], "before_fragment_expansion");
    assert_eq!(interfaces["declared_interface_types"], "checked");
    assert_eq!(interfaces["deferred_interface_types"], json!([]));
    assert_eq!(interfaces["interfaces"].as_array().unwrap().len(), 2);
    let coverage = interfaces["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["at"] == "/bindings/success/bindings/cases")
        .unwrap();
    assert_eq!(coverage["branch_coverage"], "checked");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(result["public_checks"], "not_run");
    let handle = result["handle"].as_str().unwrap();
    let overflow = json!({"Err":{"Math":{"ArithmeticError":"Overflow"}}});
    for (maybe, skip, delta, expected) in [
        (json!({"Some":127}), true, 1, json!({"Err":"Skip"})),
        (json!({"Some":127}), true, 0, json!({"Err":"Skip"})),
        (json!("None"), false, 0, json!({"Err":"ZeroDelta"})),
        (json!({"Some":127}), false, 1, overflow.clone()),
        (json!({"Some":-128}), false, -1, overflow.clone()),
        (json!("None"), false, -128, overflow),
        (json!({"Some":-128}), false, 1, json!({"Ok":-127})),
        (json!({"Some":126}), false, 1, json!({"Ok":127})),
        (json!({"Some":-7}), false, 3, json!({"Ok":-4})),
        (json!("None"), false, -7, json!({"Ok":7})),
    ] {
        assert_eq!(call(&fixture, handle, &maybe, skip, delta), expected);
    }
    let (code, shown) = cli(
        &fixture.dir,
        &[
            "residual",
            "show",
            result["draft"].as_str().unwrap(),
            "--provenance",
        ],
    );
    assert_eq!(code, 0, "{shown}");
    let entries = shown["composed_source_map"]["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert!(entries.iter().any(|v| {
        v.to_string()
            .contains("/bindings/success/bindings/cases/0/ops/0")
    }));
    assert_eq!(bytes(&request), bytes_before);
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
}

#[test]
fn reordered_guard_mutation_is_detected_by_independent_expected_behavior() {
    let original = request();
    let mut mutated = original.clone();
    mutated["bindings"]["guards"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    let original_expansion = expand(&original);
    let mutated_expansion = expand(&mutated);
    assert_ne!(original_expansion.frame, mutated_expansion.frame);
    for (request, expected) in [
        (original, json!({"Err":"Skip"})),
        (mutated, json!({"Err":"ZeroDelta"})),
    ] {
        let fixture = fixture();
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        assert_eq!(
            call(
                &fixture,
                result["handle"].as_str().unwrap(),
                &json!({"Some":127}),
                true,
                0
            ),
            expected
        );
    }
}

#[test]
fn nested_composition_conflicts_refuse_before_source_draft_publication() {
    let fixture = fixture();
    let dir = fixture.dir.join(".sley/drafts/d1");
    let original = snapshot(&dir);
    for (pointer, value, reason) in [
        (
            "/bindings/success/bindings/cases/1/values/0",
            json!("adjusted"),
            "binding",
        ),
        (
            "/bindings/success/bindings/join/params/0/1",
            json!("i16"),
            "requires i16",
        ),
        (
            "/bindings/success/bindings/cases/0/payload/1",
            json!("i16"),
            "payload type conflicts",
        ),
    ] {
        let mut request = request();
        *request.pointer_mut(pointer).unwrap() = value;
        let error = check(&fixture, &request, &source()).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains(reason), "{error}");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        assert_eq!(snapshot(&dir), original);
        for p in [
            ".sley/drafts/d2",
            ".sley/drafts/d1/r2",
            ".sley/candidates/c2.hex",
            ".sley/residual",
        ] {
            assert!(!fixture.dir.join(p).exists(), "{p}");
        }
    }
}

fn snapshot(dir: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut pending = vec![dir.to_path_buf()];
    let mut result = std::collections::BTreeMap::new();
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                result.insert(path.clone(), std::fs::read(path).unwrap());
            }
        }
    }
    result
}
