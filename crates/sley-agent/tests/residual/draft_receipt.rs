use std::fs;

use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::residual::{binding::DraftSource, frontier::Budget};

use super::dependency_tests::literal_function;
use super::{Fixture, bytes, cli, literal_request, runtime};

pub(super) fn setup(frame: &Value) -> Fixture {
    let fixture = Fixture::new();
    let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    fixture
}

pub(super) fn capture(fixture: &Fixture, base: &str) -> DraftSource {
    let mut request = literal_request(7);
    request["base"] = json!(base);
    DraftSource::capture(
        &fixture.workspace,
        &bytes(&request),
        &runtime(),
        &mut Budget::default(),
    )
    .unwrap()
}

pub(super) fn replace_frame(fixture: &Fixture, frame: &Value) {
    fs::write(
        fixture.dir.join(".sley/drafts/d1/r1/frame.json"),
        frame.to_string(),
    )
    .unwrap();
}

#[test]
fn draft_receipt_rejects_assertions_changed_before_capture() {
    let frame = json!({"af1":1,
        "types":[{"name":"E","variant":["Bad"]}],
        "fns":[literal_function("adjust"),literal_function("other")],
        "tests":[{"name":"test","fn":"adjust","args":[5],"expect":{"Ok":8}}]});
    for (pointer, changed) in [
        ("/fns/0/blocks/0/ops/0/2/value", json!(4)),
        ("/fns/1/blocks/0/ops/0/2/value", json!(4)),
        ("/fns/1/blocks/0/ops/1/1", json!("sub")),
        ("/tests/0/expect/Ok", json!(9)),
        ("/types/0/variant", json!(["Bad", "New"])),
    ] {
        let fixture = setup(&frame);
        let mut changed_frame = frame.clone();
        *changed_frame.pointer_mut(pointer).unwrap() = changed;
        replace_frame(&fixture, &changed_frame);
        let before = super::draft_source_tests::tree(&fixture.dir);
        let source = capture(&fixture, "d1@r1");
        let error = source
            .prepare(&fixture.workspace, &mut Budget::default())
            .err()
            .unwrap();
        assert_eq!(
            error.code(),
            AgentErrorCode::ResidualPreserve,
            "{pointer}: {error}"
        );
        assert!(error.detail().contains("source frame"), "{error}");
        assert_eq!(super::draft_source_tests::tree(&fixture.dir), before);
    }
}

#[test]
fn draft_receipt_keeps_anonymous_tests_and_type_members_across_edits() {
    let frame = json!({"af1":1,
        "types":[{"name":"E","variant":["Bad"]},
            {"name":"Point","record":[["x","i8"],["y","i8"]]}],
        "fns":[literal_function("adjust")],
        "tests":[{"fn":"adjust","args":[5],"expect":{"Ok":8}}]});
    let fixture = setup(&frame);
    for (revision, value) in [(1, 7), (2, 9)] {
        let mut request = literal_request(value);
        request["base"] = json!(format!("d1@r{revision}"));
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        let status: Value = serde_json::from_slice(
            &fs::read(
                fixture
                    .dir
                    .join(format!(".sley/drafts/d1/r{}/status.json", revision + 1)),
            )
            .unwrap(),
        )
        .unwrap();
        for key in ["source_frame_assertions", "composed_frame_assertions"] {
            let assertions = &status["residual"]["draft_graph"][key];
            assert_eq!(assertions["anonymous_tests_named"], 1, "{status}");
            assert_eq!(assertions["authored_assertions"], "match_validated_graph");
            assert_eq!(
                assertions["complete_candidate_correspondence"],
                "verified_with_inline_constant_aliases"
            );
        }
    }
}

#[test]
fn draft_receipt_equal_values_do_not_erase_named_constant_identity() {
    let mut aliases = 0;
    for referenced in ["left", "right"] {
        let other = if referenced == "left" {
            "right"
        } else {
            "left"
        };
        let mut frame = json!({"af1":1,
            "consts":[{"name":"left","type":"i8","value":3},
                {"name":"right","type":"i8","value":3}],
            "fns":[literal_function("adjust"),literal_function("other")]});
        frame["fns"][0]["blocks"][0]["ops"][0][2] = json!(referenced);
        frame["fns"][1]["blocks"][0]["ops"][0][2] = json!(other);
        let fixture = setup(&frame);
        // Both named constants have the same value, but the source operation
        // still names only one of them. Change the receipt, not the candidate.
        frame["fns"][0]["blocks"][0]["ops"][0][2] = json!(other);
        replace_frame(&fixture, &frame);
        let error = capture(&fixture, "d1@r1")
            .prepare(&fixture.workspace, &mut Budget::default())
            .err()
            .unwrap();
        assert_eq!(error.code(), AgentErrorCode::ResidualPreserve);
        assert!(error.detail().contains("source frame disagrees"), "{error}");
        // Inline syntax asserts a value. Either equal constant can satisfy it.
        frame["fns"][0]["blocks"][0]["ops"][0][2] = json!({"type":"i8","value":3});
        frame["fns"][1]["blocks"][0]["ops"][0][2] = json!({"type":"i8","value":3});
        replace_frame(&fixture, &frame);
        let prepared = capture(&fixture, "d1@r1")
            .prepare(&fixture.workspace, &mut Budget::default())
            .unwrap();
        aliases += prepared.report["source_frame_assertions"]["equal_constant_aliases"]
            .as_u64()
            .unwrap();
    }
    assert!(aliases > 0, "exercise actual compiler alias selection");
}

#[test]
fn draft_receipt_checks_already_applied_top_and_block_deletions() {
    let mut other = literal_function("other");
    let mut spare = other["blocks"][0].clone();
    spare["name"] = json!("spare");
    spare["unreachable"] = json!(true);
    other["blocks"].as_array_mut().unwrap().push(spare);
    let fixture = setup(&json!({"af1":1,
        "fns":[literal_function("adjust"),literal_function("obsolete"),other]}));
    let (code, report) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{report}");
    let frame = json!({"af1":1,"delete":["obsolete"],
        "patch":[{"fn":"other","blocks":{"spare":null}}],
        "edit":[{"fn":"adjust","replace_op":"entry.amount",
            "with":["const",{"type":"i8","value":4}]}]});
    let (code, report) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{report}");
    let before = super::draft_source_tests::tree(&fixture.dir);
    let prepared = capture(&fixture, "d2@r1")
        .prepare(&fixture.workspace, &mut Budget::default())
        .unwrap();
    assert_eq!(
        prepared.report["source_frame_assertions"]["applied_deletions_checked"],
        2
    );
    assert_eq!(
        prepared.report["composed_frame_assertions"]["applied_deletions_checked"],
        2
    );
    let mut expected = frame.clone();
    expected["edit"][0]["with"][1]["value"] = json!(7);
    expected["consts"] = json!([{"name":"k_4","type":"i8","value":4}]);
    assert_eq!(prepared.authoring.unwrap().frame, expected);
    assert_eq!(super::draft_source_tests::tree(&fixture.dir), before);
    // A receipt cannot claim a deletion that the candidate did not perform,
    // nor reinterpret a removed child as an AF1 top-level delete directive.
    for name in ["other", "obsolete.entry.amount"] {
        let mut changed = frame.clone();
        changed["delete"] = json!([name]);
        fs::write(
            fixture.dir.join(".sley/drafts/d2/r1/frame.json"),
            changed.to_string(),
        )
        .unwrap();
        let before = super::draft_source_tests::tree(&fixture.dir);
        let error = capture(&fixture, "d2@r1")
            .prepare(&fixture.workspace, &mut Budget::default())
            .err()
            .unwrap();
        assert_eq!(error.code(), AgentErrorCode::ResidualPreserve, "{error}");
        assert!(error.detail().contains("deletion"), "{error}");
        assert_eq!(super::draft_source_tests::tree(&fixture.dir), before);
    }
}

#[test]
fn draft_receipt_factoring_refuses_disagreement_before_plan_publication() {
    let mut frame = json!({"af1":1,"fns":[literal_function("adjust")]});
    let fixture = setup(&frame);
    frame["fns"][0]["blocks"][0]["ops"][0][2]["value"] = json!(4);
    replace_frame(&fixture, &frame);
    let mut request = literal_request(7);
    request["base"] = json!("d1@r1");
    request["bindings"].as_object_mut().unwrap().remove("value");
    request["choices"] = json!({"version":2,"contract":"author_closed_constraints",
        "constraints":[{"fields":["/bindings/value"],
            "rows":[{"/bindings/value":7},{"/bindings/value":9}]}],"dependencies":[]});
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_PRESERVE");
    assert!(!fixture.dir.join(".sley/residual").exists());
    assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
}
