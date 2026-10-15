use std::fs;

use serde_json::json;
use sley_agent::AgentErrorCode;
use sley_agent::residual::frontier::Budget;

use super::dependency_tests::literal_function;
use super::draft_receipt_tests::{capture, replace_frame, setup};
use super::draft_source_tests::tree;
use super::{cli, literal_request};

#[test]
fn draft_replay_detects_omitted_created_definitions() {
    let frame = json!({"af1":1,"types":[{"name":"E","variant":["Bad"]}],
        "fns":[literal_function("adjust"),literal_function("other")],
        "tests":[{"name":"test","fn":"adjust","args":[5],"expect":{"Ok":8}}]});
    for field in ["types", "fns", "tests"] {
        let fixture = setup(&frame);
        let mut changed = frame.clone();
        changed[field].as_array_mut().unwrap().pop();
        replace_frame(&fixture, &changed);
        let before = tree(&fixture.dir);
        let error = capture(&fixture, "d1@r1")
            .prepare(&fixture.workspace, &mut Budget::default())
            .err()
            .unwrap();
        assert_eq!(
            error.code(),
            AgentErrorCode::ResidualPreserve,
            "{field}: {error}"
        );
        assert!(error.detail().contains("not described"), "{error}");
        assert_eq!(tree(&fixture.dir), before);
    }
}

#[test]
fn draft_replay_detects_omitted_accepted_graph_changes_and_deletions() {
    for delete in [false, true] {
        let fixture = setup(&json!({"af1":1,
            "fns":[literal_function("adjust"),literal_function("other")]}));
        let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{result}");
        let mut frame = json!({"af1":1,"edit":[{"fn":"adjust","replace_op":"entry.amount",
            "with":["const",{"type":"i8","value":4}]}]});
        if delete {
            frame["delete"] = json!(["other"]);
        } else {
            frame["edit"]
                .as_array_mut()
                .unwrap()
                .push(json!({"fn":"other",
                "replace_op":"entry.amount","with":["const",{"type":"i8","value":9}]}));
        }
        let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        if delete {
            frame.as_object_mut().unwrap().remove("delete");
        } else {
            frame["edit"].as_array_mut().unwrap().pop();
        }
        fs::write(
            fixture.dir.join(".sley/drafts/d2/r1/frame.json"),
            frame.to_string(),
        )
        .unwrap();
        let before = tree(&fixture.dir);
        let error = capture(&fixture, "d2@r1")
            .prepare(&fixture.workspace, &mut Budget::default())
            .err()
            .unwrap();
        assert_eq!(error.code(), AgentErrorCode::ResidualPreserve, "{error}");
        assert!(error.detail().contains("not described"), "{error}");
        assert_eq!(tree(&fixture.dir), before);
    }
}

#[test]
fn draft_replay_keeps_typed_constant_collisions_after_repair() {
    let mut other = literal_function("other");
    other["params"][0][1] = json!("i64");
    other["returns"] = json!("Result<i64,ArithmeticError>");
    other["blocks"][0]["ops"][0][2]["type"] = json!("i64");
    let fixture = setup(&json!({"af1":1,"fns":[literal_function("adjust"),other]}));
    for (revision, value) in [(1, 7), (2, 9), (3, 3)] {
        let mut request = literal_request(value);
        request["base"] = json!(format!("d1@r{revision}"));
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        let (_, ran) = cli(
            &fixture.dir,
            &[
                "call",
                "other",
                "5",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(ran["result"], json!({"Ok":8}));
    }
}

#[test]
fn draft_replay_does_not_claim_provenance_for_unrepresented_created_constants() {
    let mut frame = json!({"af1":1,"fns":[literal_function("adjust")],
        "consts":[{"name":"unused","type":"i8","value":99}]});
    let fixture = setup(&frame);
    frame.as_object_mut().unwrap().remove("consts");
    replace_frame(&fixture, &frame);
    let prepared = capture(&fixture, "d1@r1")
        .prepare(&fixture.workspace, &mut Budget::default())
        .unwrap();
    let checked = &prepared.report["source_frame_assertions"]["accepted_head_replay"];
    assert_eq!(checked["unrepresented_created_constants"], 1);
    assert_eq!(
        checked["complete_candidate_correspondence"],
        "not_established"
    );
}

#[test]
fn draft_replay_checks_inherited_fields_against_the_accepted_head() {
    for accepted in [false, true] {
        let mut frame = json!({"af1":1,"fns":[literal_function("adjust")]});
        frame["fns"][0]["visibility"] = json!("private");
        let fixture = setup(&frame);
        let base = if accepted {
            let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
            assert_eq!(code, 0, "{result}");
            frame["fns"][0]
                .as_object_mut()
                .unwrap()
                .remove("visibility");
            frame["fns"][0]["blocks"][0]["ops"][0][2]["value"] = json!(4);
            let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
            assert_eq!(code, 0, "{result}");
            "d2@r1"
        } else {
            frame["fns"][0]
                .as_object_mut()
                .unwrap()
                .remove("visibility");
            replace_frame(&fixture, &frame);
            "d1@r1"
        };
        let before = tree(&fixture.dir);
        let prepared = capture(&fixture, base).prepare(&fixture.workspace, &mut Budget::default());
        if accepted {
            // Omission legitimately inherits the private visibility from head.
            assert!(prepared.is_ok(), "{}", prepared.err().unwrap());
        } else {
            // Omission on a newly created function means the compiler default,
            // even though restating against the source would inherit private.
            let error = prepared.err().unwrap();
            assert_eq!(error.code(), AgentErrorCode::ResidualPreserve);
            assert!(error.detail().contains("not described"), "{error}");
        }
        assert_eq!(tree(&fixture.dir), before);
    }
}
