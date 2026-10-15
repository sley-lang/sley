use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli, interface_call_tests, interface_effect_tests};
use serde_json::{Value, json};
use sley_agent::{
    AgentErrorCode,
    names::{NameMap, Names},
    residual::{frontier::Budget, interfaces, parse_request},
};
use sley_mutate::value::EntityBodyValue;
use sley_ssmc::Immediate;

fn fixture(checked: bool) -> Fixture {
    let fixture = interface_call_tests::fixture(true);
    let function = if checked {
        json!({"fn":"helper","params":[["item","Option<i8>"]],"returns":"Option<i8>","blocks":[
            {"name":"entry","ops":[["value","call?","fetch","item"],["out","call","echo","value"]],
                "term":["return",["some","out"]]}]})
    } else {
        json!({"fn":"helper","params":[["x","i8"]],"returns":"i8","blocks":[
            {"name":"entry","ops":[["out","call","echo","x"]],"term":["return","out"]}]})
    };
    let frame = json!({"af1":1,"afx":1,"fns":[function]});
    let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c2"]);
    assert_eq!(code, 0, "{result}");
    fixture
}

fn metadata_check(fixture: &Fixture, declarations: &Value) -> sley_agent::Result<Value> {
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    // Synthetic effect metadata isolates the interface constraint. Only the
    // unmodified accepted fixture is used for kernel and VM checks.
    let program = interface_effect_tests::effectful_metadata(head.program(), &names);
    interfaces::check(
        &program,
        &names,
        declarations.as_object().unwrap(),
        &parse_request(&bytes(&predicate_request(json!(false)))).unwrap(),
        &mut Budget::default(),
    )
}

#[test]
fn retained_patch_calls_check_bound_effects_and_name_the_original_operation() {
    let fixture = fixture(false);
    for patch in [
        json!({"fn":"helper","blocks":{}}),
        json!({"fn":"helper","returns":"i8"}),
    ] {
        let source = json!({"af1":1,"afx":1,"patch":[patch]});
        let before = bytes(&source);
        let error = metadata_check(&fixture, &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("/patch/0"), "{error}");
        assert!(
            error.detail().contains("helper.entry.out") && error.detail().contains("`echo`"),
            "{error}"
        );
        assert_eq!(bytes(&source), before);
        let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        let patch = &report["declared_body_effects"]["checked_functions"][0];
        assert_eq!(patch["status"], "empty_effect_connections_checked");
        assert_eq!(patch["source"], "accepted_graph_patch");
        assert_eq!(
            patch["calls"][0]["retained_operation"]["name"],
            "helper.entry.out"
        );
    }
}

#[test]
fn replaced_deleted_and_new_patch_blocks_use_only_surviving_call_connections() {
    let fixture = fixture(false);
    for blocks in [
        json!({"entry":{"ops":[],"term":["return","x"]}}),
        json!({"entry":null,"finish":{"ops":[],"term":["return","x"]}}),
    ] {
        let source = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":blocks}]});
        let report = metadata_check(&fixture, &source).unwrap();
        assert_eq!(report["declared_body_effects"]["status"], "checked");
        assert_eq!(
            report["declared_body_effects"]["checked_functions"][0]["calls"],
            json!([])
        );
    }
    let source = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{
        "entry":{"ops":[],"term":["return",["call_direct","echo","x"]]}}}]});
    let error = metadata_check(&fixture, &source).unwrap_err();
    assert!(
        error.detail().contains("/patch/0/blocks/entry/term/1"),
        "{error}"
    );
}

#[test]
fn checked_continuation_removal_matches_afx_and_plain_af1_keeps_its_calls() {
    let fixture = fixture(true);
    let source = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{
        "entry":{"ops":[],"term":["return","item"]}}}]});
    let report = metadata_check(&fixture, &source).unwrap();
    assert_eq!(report["declared_body_effects"]["status"], "checked");
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let echo = names.resolve("echo").unwrap();
    let retained_call = head.program().objects().iter().find_map(|object| match &object.record().body {
        EntityBodyValue::Operation(op) if matches!(&op.immediate,Immediate::Function(reference) if reference.function==echo) => Some(op),
        _ => None,
    }).unwrap();
    let continuation = names.leaf(&retained_call.block);
    assert_ne!(continuation, "entry");
    let expanded = sley_agent::afx::expand(head.program(), &names, &source).unwrap();
    assert!(
        expanded.obligations.is_empty(),
        "{:?}",
        expanded.obligations
    );
    assert_eq!(
        expanded.frame["patch"][0]["blocks"][&continuation],
        Value::Null
    );
    assert!(
        expanded.frame["patch"][0]["blocks"]
            .as_object()
            .unwrap()
            .contains_key(&continuation)
    );
    let mut plain = source.clone();
    plain.as_object_mut().unwrap().remove("afx");
    let error = metadata_check(&fixture, &plain).unwrap_err();
    assert!(
        error.detail().contains("retained operation") && error.detail().contains("`echo`"),
        "{error}"
    );
    // Ordinary AF1-X confirms that the proposed surviving body is admitted.
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(
        &fixture.dir,
        &["call", "helper", "{\"Some\":5}", "--on", "c3"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], json!({"Some":5}));
}

#[test]
fn retained_call_identity_cannot_be_rebound_after_delete_and_recreate() {
    let fixture = fixture(false);
    let source = json!({"af1":1,"afx":1,"delete":["echo"],
        "fns":[{"fn":"echo","params":[["z","i8"]],"returns":"i8","blocks":[
            {"name":"entry","ops":[],"term":["return","z"]}]}],
        "patch":[{"fn":"helper","blocks":{}}]});
    let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(error.detail().contains("original identity"), "{error}");
    assert!(error.detail().contains("helper.entry.out"), "{error}");
}

#[test]
fn pure_patch_draft_composes_and_executes_through_the_residual_cli() {
    let fixture = fixture(false);
    let source = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{
        "entry":{"ops":[],"term":["return",["call","echo",{"type":"i8","value":11}]]}}}]});
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let mut request = predicate_request(json!(false));
    request["base"] = json!("d3@r1");
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("i8");
    request["bindings"]["success"] = json!({"ops":[],"term":["return",["call","helper","x"]]});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "7", "true", "--on", "c4"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], json!(11));
}

#[test]
fn missing_and_malformed_patch_targets_refuse_while_transforms_remain_deferred() {
    let fixture = fixture(false);
    for patches in [
        json!([{"fn":"missing","blocks":{}}]),
        json!([{"fn":"helper","blocks":[]}]),
    ] {
        let source = json!({"af1":1,"afx":1,"patch":patches});
        let error = metadata_check(&fixture, &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    }
    for key in ["edit", "ripple"] {
        let source = json!({"af1":1,"afx":1,key:[{}],"patch":[{"fn":"helper","blocks":{}}]});
        let report = metadata_check(&fixture, &source).unwrap();
        assert_eq!(report["declared_body_effects"]["status"], "deferred");
    }
}

#[test]
fn patch_target_conflicts_match_ordinary_refusals_and_preserve_source() {
    for dialect in [false, true] {
        for scenario in 0..7 {
            let fixture = fixture(false);
            let mut source = json!({"af1":1,"patch":[{"fn":"helper","blocks":{}}]});
            if dialect {
                source["afx"] = json!(1);
            }
            let at = match scenario {
                0 => {
                    source["patch"][0]["fn"] = json!("missing");
                    "/patch/0"
                }
                1 => {
                    source["delete"] = json!(["helper"]);
                    "/patch/0"
                }
                2 => {
                    source["patch"][0]["blocks"] = json!([]);
                    "/patch/0/blocks"
                }
                3 => {
                    source["patch"][0]["blocks"] = json!({"absent":null});
                    "/patch/0/blocks/absent"
                }
                4 => {
                    source["patch"][0]["blocks"] = json!({"entry":true});
                    "/patch/0/blocks/entry"
                }
                5 => {
                    source["patch"][0]["blocks"] = json!({"newblock":[]});
                    "/patch/0/blocks/newblock"
                }
                _ => {
                    source["patch"][0]["blocks"] = json!({"a~/b":null});
                    "/patch/0/blocks/a~0~1b"
                }
            };
            let before = source.clone();
            let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
            assert!(error.detail().contains(at), "{scenario}/{dialect}: {error}");
            let (code, ordinary) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
            assert_ne!(code, 0, "{scenario}/{dialect}: {ordinary}");
            assert_eq!(source, before);
        }
    }
}

#[test]
fn patch_target_evidence_distinguishes_creation_restatement_and_deletion() {
    let fixture = fixture(false);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    for (index, (blocks, expected)) in [
        (
            json!({"entry":{"ops":[],"term":["return","x"]}}),
            vec![("entry", "restate")],
        ),
        (
            json!({"entry":null,"finish":{"ops":[],"term":["return","x"]}}),
            vec![("entry", "delete"), ("finish", "create")],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let source = json!({"af1":1,"afx":1,"patch":[{"name":"helper","blocks":blocks}]});
        let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        let targets = report["declared_body_effects"]["checked_functions"][0]["patch_targets"]
            .as_array()
            .unwrap();
        assert_eq!(targets.len(), expected.len());
        for (target, (block, action)) in targets.iter().zip(expected) {
            assert_eq!(target["action"], action);
            assert_eq!(target["block"], block);
            assert_eq!(target["authority"], "none");
            assert_eq!(
                target["owner_id"],
                sley_agent::hex::encode(names.resolve("helper").unwrap().as_bytes())
            );
            let id = names
                .resolve(&format!("helper.{block}"))
                .map(|id| sley_agent::hex::encode(id.as_bytes()));
            assert_eq!(target["block_id"], json!(id));
        }
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let candidate = format!("c{}", index + 3);
        let (code, result) = cli(&fixture.dir, &["call", "helper", "7", "--on", &candidate]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], 7);
    }
}

#[test]
fn unknown_patch_bodies_do_not_hide_invalid_targets_or_claim_complete_checks() {
    let fixture = fixture(false);
    let mut source = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{
        "entry":{"ops":[["out","future_operation"]],"term":["return","out"]},"missing":null}}]});
    let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
    assert!(
        error.detail().contains("/patch/0/blocks/missing"),
        "{error}"
    );
    source["patch"][0]["blocks"]
        .as_object_mut()
        .unwrap()
        .remove("missing");
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    let body = &report["declared_body_effects"]["checked_functions"][0];
    assert_eq!(body["status"], "partial");
    assert_eq!(body["patch_targets"][0]["action"], "restate");
}

#[test]
fn generated_patch_deletion_collisions_remain_deferred_until_actual_expansion() {
    let fixture = fixture(true);
    let source = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{
        "entry":{"ops":[["fresh","call?","fetch","item"]],"term":["return",["some","fresh"]]},
        "entry__fresh":null}}]});
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    let body = &report["declared_body_effects"]["checked_functions"][0];
    assert_eq!(body["status"], "partial");
    assert_eq!(body["patch_targets"][1]["action"], "pending_expansion");
    assert_eq!(body["patch_targets"][1]["block_id"], Value::Null);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    assert!(names.resolve("helper.entry__fresh").is_none());
    let expanded = sley_agent::afx::expand(head.program(), &names, &source).unwrap();
    assert!(
        expanded.obligations.is_empty(),
        "{:?}",
        expanded.obligations
    );
    assert!(expanded.frame["patch"][0]["blocks"]["entry__fresh"].is_object());
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(
        &fixture.dir,
        &["call", "helper", "{\"Some\":5}", "--on", "c3"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], json!({"Some":5}));

    let mut plain = source;
    plain.as_object_mut().unwrap().remove("afx");
    let error = check(&fixture, &predicate_request(json!(false)), &plain).unwrap_err();
    assert!(
        error.detail().contains("/patch/0/blocks/entry__fresh"),
        "{error}"
    );
}
