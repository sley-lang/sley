use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::residual::{binding::DraftSource, frontier::Budget};

use super::{Fixture, bytes, cli, literal_request, runtime};

fn source_fixture() -> (Fixture, Vec<u8>) {
    let fixture = Fixture::new();
    let frame = json!({"af1":1,"fns":[{
        "fn":"adjust","params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>",
        "blocks":[{"name":"entry","ops":[["amount","const",{"type":"i8","value":3}],
            ["sum","add","x","amount"]],"term":["return","sum"]}]
    }]});
    let (code, report) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["draft"], "d1@r1");
    let mut request = literal_request(7);
    request["base"] = json!("d1@r1");
    (fixture, bytes(&request))
}

pub(super) fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(tree(&path));
        } else {
            files.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    files
}

#[test]
fn captured_draft_source_composes_exact_receipt_without_writes() {
    let (fixture, input) = source_fixture();
    let before = tree(&fixture.dir);
    let source = DraftSource::capture(
        &fixture.workspace,
        &input,
        &runtime(),
        &mut Budget::default(),
    )
    .unwrap();
    let status: Value = serde_json::from_slice(
        &fs::read(fixture.dir.join(".sley/drafts/d1/r1/status.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(source.status(), &status);
    assert_eq!(source.frame()["fns"][0]["fn"], "adjust");
    let prepared = source
        .prepare(&fixture.workspace, &mut Budget::default())
        .unwrap();
    assert_eq!(
        prepared.report["receipt_binding"],
        "checked_before_and_after_composition"
    );
    assert_eq!(prepared.report["binding"], source.binding().digest());
    assert_eq!(
        prepared.report["source_candidate_sha256"],
        status["candidate_sha256"]
    );
    assert_eq!(prepared.report["source_graph_preservation"], "verified");
    let retained = prepared.authoring.as_ref().unwrap();
    let mut expected = source.frame().clone();
    expected["fns"][0]["blocks"][0]["ops"][0][2]["value"] = json!(7);
    expected["consts"] = json!([{"name":"k_3","type":"i8","value":3}]);
    assert_eq!(retained.frame, expected);
    assert_eq!(retained.expanded, expected);
    assert_eq!(tree(&fixture.dir), before);
    source.recheck(&fixture.workspace).unwrap();
}

#[test]
fn captured_draft_source_detects_each_consumed_artifact_change_before_composition() {
    for relative in [
        ".sley/drafts/d1/r1/status.json",
        ".sley/drafts/d1/r1/frame.json",
        ".sley/drafts/d1/r1/input.txt",
        ".sley/names.json",
        ".sley/candidates/c1.hex",
    ] {
        let (fixture, input) = source_fixture();
        let source = DraftSource::capture(
            &fixture.workspace,
            &input,
            &runtime(),
            &mut Budget::default(),
        )
        .unwrap();
        let path = fixture.dir.join(relative);
        let original = fs::read(&path).unwrap();
        let mut changed = original.clone();
        if relative == ".sley/candidates/c1.hex" {
            changed.extend_from_slice(b"00");
        } else {
            changed.push(b' ');
        }
        fs::write(&path, &changed).unwrap();
        let before = tree(&fixture.dir);
        let error = source
            .prepare(&fixture.workspace, &mut Budget::default())
            .err()
            .unwrap();
        assert_eq!(
            error.code(),
            AgentErrorCode::ResidualBindingStale,
            "{relative}: {error}"
        );
        assert_eq!(tree(&fixture.dir), before);
        fs::write(path, original).unwrap();
        source.recheck(&fixture.workspace).unwrap();
    }
}

#[test]
fn captured_draft_source_rejects_newer_revision_and_identical_cross_directory_copy() {
    let (fixture, input) = source_fixture();
    let source = DraftSource::capture(
        &fixture.workspace,
        &input,
        &runtime(),
        &mut Budget::default(),
    )
    .unwrap();
    let other = Fixture::new();
    for (path, content) in tree(&fixture.dir) {
        let destination = other.dir.join(path.strip_prefix(&fixture.dir).unwrap());
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(destination, content).unwrap();
    }
    assert_eq!(
        source.recheck(&other.workspace).unwrap_err().code(),
        AgentErrorCode::ResidualBindingStale
    );
    let (code, report) = cli(
        &fixture.dir,
        &["try", r#"{"af1":1}"#, "--on", "d1@r1", "--no-test"],
    );
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["draft"], "d1@r2");
    assert_eq!(
        source.recheck(&fixture.workspace).unwrap_err().code(),
        AgentErrorCode::ResidualBindingStale
    );
    assert!(
        DraftSource::capture(
            &fixture.workspace,
            &input,
            &runtime(),
            &mut Budget::default()
        )
        .is_err()
    );
}

#[test]
fn captured_draft_source_refuses_missing_graph_and_undecided_relation() {
    let (fixture, input) = source_fixture();
    let mut request: Value = serde_json::from_slice(&input).unwrap();
    request["bindings"].as_object_mut().unwrap().remove("value");
    request["choices"] = json!({"version":1,"contract":"author_closed_relation",
        "rows":[{"/bindings/value":7},{"/bindings/value":9}]});
    let source = DraftSource::capture(
        &fixture.workspace,
        &bytes(&request),
        &runtime(),
        &mut Budget::default(),
    )
    .unwrap();
    assert_eq!(
        source
            .prepare(&fixture.workspace, &mut Budget::default())
            .err()
            .unwrap()
            .code(),
        AgentErrorCode::ResidualChoiceMissing
    );
    let (code, report) = cli(
        &fixture.dir,
        &["try", r#"{"af1":1,"fns":[{"fn":"bad"}]}"#, "--no-test"],
    );
    assert_eq!(code, 2, "{report}");
    let mut request: Value = serde_json::from_slice(&input).unwrap();
    request["base"] = report["draft"].clone();
    let error = DraftSource::capture(
        &fixture.workspace,
        &bytes(&request),
        &runtime(),
        &mut Budget::default(),
    )
    .err()
    .unwrap();
    assert_eq!(error.code(), AgentErrorCode::DraftIncomplete);
}

#[test]
fn captured_draft_source_rechecks_head_and_retains_callers_budget() {
    let (fixture, input) = source_fixture();
    let source = DraftSource::capture(
        &fixture.workspace,
        &input,
        &runtime(),
        &mut Budget::default(),
    )
    .unwrap();
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 0);
    assert_eq!(
        source
            .prepare(&fixture.workspace, &mut budget)
            .err()
            .unwrap()
            .code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(
        DraftSource::capture(&fixture.workspace, &input, &runtime(), &mut budget)
            .err()
            .unwrap()
            .code(),
        AgentErrorCode::ResidualLimit
    );
    let (code, report) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{report}");
    assert_eq!(
        source.recheck(&fixture.workspace).unwrap_err().code(),
        AgentErrorCode::ResidualBindingStale
    );
}
