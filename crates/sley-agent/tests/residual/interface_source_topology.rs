use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn helper(blocks: Value) -> Value {
    let mut source = json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":[]}]});
    source["fns"][0]["blocks"] = blocks;
    source
}

#[test]
fn source_definition_branch_targets_must_belong_to_the_declared_function() {
    let fixture = Fixture::new();
    let request = predicate_request(json!(false));
    for (term, pointer) in [
        (json!(["br", "absent"]), "/term/1"),
        (json!(["jump", ["absent", "x"]]), "/term/1/0"),
        (
            json!(["cond", "ready", ["absent", "x"], "entry"]),
            "/term/2/0",
        ),
        (
            json!([
                "switch",
                ["some", "x"],
                ["Some", "absent", "$"],
                ["None", "entry"]
            ]),
            "/term/2/1",
        ),
        (
            json!([
                "switch",
                ["some", "x"],
                ["Some", ["absent", "$"]],
                ["None", "entry"]
            ]),
            "/term/2/1/0",
        ),
    ] {
        let declaration = helper(json!([{"name":"entry","ops":[],"term":term}]));
        let original = bytes(&declaration);
        let error = check(&fixture, &request, &declaration).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error
                .detail()
                .contains(&format!("/fns/0/blocks/0{pointer}")),
            "{error}"
        );
        assert!(
            error.detail().contains("absent") && error.detail().contains("helper"),
            "{error}"
        );
        assert_eq!(bytes(&declaration), original);
    }
}

fn complete_helper() -> Value {
    helper(json!([
        {"name":"entry","ops":[],"term":["cond","ready","positive","negative"]},
        {"name":"positive","ops":[],"term":["return","x"]},
        {"name":"negative","ops":[["other","const",{"type":"i8","value":-5}]],"term":["return","other"]}
    ]))
}

fn calling_request() -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("i8");
    request["bindings"]["success"] =
        json!({"ops":[],"term":["return",["call","helper","x","ready"]]});
    request
}

#[test]
fn complete_source_topology_preserves_ordinary_compilation_and_actual_execution() {
    for dialect in [false, true] {
        for key in ["fns", "functions"] {
            let fixture = Fixture::new();
            let mut source = complete_helper();
            if !dialect {
                source.as_object_mut().unwrap().remove("afx");
            }
            if key == "functions" {
                let functions = source.as_object_mut().unwrap().remove("fns").unwrap();
                source[key] = functions;
            }
            let mut request = calling_request();
            let original = bytes(&source);
            let report = check(&fixture, &request, &source).unwrap();
            let control = &report["declared_body_control_flow"];
            assert_eq!(control["status"], "explicit_targets_checked", "{report}");
            assert_eq!(
                control["checked_functions"][0]["edges"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
            assert_eq!(
                control["checked_functions"][0]["owned_blocks"]
                    .as_object()
                    .unwrap()
                    .len(),
                3
            );
            assert_eq!(report["composition"], "partial");
            let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
            assert_eq!(code, 0, "{result}");
            request["base"] = json!("d1@r1");
            let (code, result) = cli(
                &fixture.dir,
                &["residual", "try", &request.to_string(), "--no-test"],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["kernel"], "valid");
            for x in [-128, -7, 0, 1, 7, 126, 127] {
                for ready in [true, false] {
                    let (code, result) = cli(
                        &fixture.dir,
                        &[
                            "call",
                            "checked",
                            &x.to_string(),
                            &ready.to_string(),
                            "--on",
                            "c2",
                        ],
                    );
                    assert_eq!(code, 0, "{result}");
                    assert_eq!(result["result"], json!(if ready { x } else { -5 }));
                }
            }
            assert_eq!(bytes(&source), original);
        }
    }
}

#[test]
fn duplicate_blocks_and_parameter_block_ownership_conflicts_are_not_deferred() {
    let fixture = Fixture::new();
    let mut duplicate = complete_helper();
    duplicate["fns"][0]["blocks"][1]["name"] = json!("entry");
    let error = check(&fixture, &calling_request(), &duplicate).unwrap_err();
    assert!(
        error.detail().contains("/fns/0/blocks/1/name")
            && error.detail().contains("/fns/0/blocks/0"),
        "{error}"
    );
    let mut collision = complete_helper();
    collision["fns"][0]["params"][0][0] = json!("positive");
    let error = check(&fixture, &calling_request(), &collision).unwrap_err();
    assert!(
        error.detail().contains("/fns/0/params/0/0") && error.detail().contains("/fns/0/blocks/1"),
        "{error}"
    );
    let mut absent_entry = complete_helper();
    absent_entry["fns"][0]["entry"] = json!("absent");
    let error = check(&fixture, &calling_request(), &absent_entry).unwrap_err();
    assert!(
        error.detail().contains("/fns/0/entry") && error.detail().contains("absent"),
        "{error}"
    );
    let mut empty = complete_helper();
    empty["fns"][0]["blocks"] = json!([]);
    let report = check(&fixture, &predicate_request(json!(false)), &empty).unwrap();
    assert_eq!(report["declared_body_control_flow"]["status"], "partial");
    assert_eq!(
        report["declared_body_control_flow"]["deferred"],
        json!(["/fns/0/blocks"])
    );
}

#[test]
fn patches_check_retained_edges_entry_and_parameter_names_after_deletions() {
    for dialect in [false, true] {
        let fixture = Fixture::new();
        let source = complete_helper();
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{result}");
        {
            let blocks = json!({"positive":null});
            let mut patch = json!({"af1":1,"patch":[{"fn":"helper","blocks":blocks}]});
            if dialect {
                patch["afx"] = json!(1);
            }
            let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
            assert!(
                error.detail().contains("/patch/0") && error.detail().contains("helper"),
                "{error}"
            );
        }
        let absent_entry = json!({"af1":1,"patch":[{"fn":"helper","entry":"absent"}]});
        let error = check(&fixture, &predicate_request(json!(false)), &absent_entry).unwrap_err();
        assert!(
            error.detail().contains("/patch/0/entry") && error.detail().contains("absent"),
            "{error}"
        );
        let collision = json!({"af1":1,"patch":[{"fn":"helper","params":[["positive","i8"],["ready","bool"]]}]});
        let error = check(&fixture, &predicate_request(json!(false)), &collision).unwrap_err();
        assert!(
            error.detail().contains("/patch/0/params/0/0") && error.detail().contains("positive"),
            "{error}"
        );
        let new_collision = json!({"af1":1,"patch":[{"fn":"helper","blocks":{"x":{"ops":[],"term":["return","x"]}}}]});
        let error = check(&fixture, &predicate_request(json!(false)), &new_collision).unwrap_err();
        assert!(
            error.detail().contains("retained function parameter")
                && error.detail().contains("helper.x"),
            "{error}"
        );
        let mut patch = json!({"af1":1,"patch":[{"fn":"helper","blocks":{
            "positive":null,"entry":{"ops":[],"term":["cond","ready","replacement","negative"]},
            "replacement":{"ops":[],"term":["return","x"]}}}]});
        if dialect {
            patch["afx"] = json!(1);
        }
        let mut request = calling_request();
        let report = check(&fixture, &request, &patch).unwrap();
        assert_eq!(
            report["declared_body_control_flow"]["status"], "explicit_targets_checked",
            "{report}"
        );
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        request["base"] = json!("d2@r1");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        for (ready, expected) in [(true, 7), (false, -5)] {
            let (code, result) = cli(
                &fixture.dir,
                &["call", "checked", "7", &ready.to_string(), "--on", "c3"],
            );
            assert_eq!(code, 0, "{result}");
            assert_eq!(result["result"], json!(expected));
        }
    }
}

#[test]
fn source_generated_targets_transforms_and_replaced_bodies_keep_explicit_boundaries() {
    let fixture = Fixture::new();
    let source = helper(json!([{"name":"entry","ops":[],"term":["br","entry__generated"]}]));
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    assert_eq!(report["declared_body_control_flow"]["status"], "partial");
    assert_eq!(
        report["declared_body_control_flow"]["deferred"],
        json!(["/fns/0/blocks/0/term/1"])
    );
    let mut replaced = source.clone();
    replaced["fns"][0]["fn"] = json!("checked");
    let report = check(&fixture, &predicate_request(json!(false)), &replaced).unwrap();
    assert_eq!(
        report["declared_body_control_flow"]["checked_functions"],
        json!([])
    );
    let mut transformed = source.clone();
    transformed["ripple"] = json!([{}]);
    let report = check(&fixture, &predicate_request(json!(false)), &transformed).unwrap();
    assert_eq!(report["declared_body_control_flow"]["status"], "deferred");
    let mut malformed = source.clone();
    malformed["fns"][0]["blocks"][0]["term"] = json!(["future_terminator"]);
    let report = check(&fixture, &predicate_request(json!(false)), &malformed).unwrap();
    assert_eq!(
        report["declared_body_control_flow"]["deferred"],
        json!(["/fns/0/blocks/0/term"])
    );
}

#[test]
fn deleting_an_entry_preserves_ordinary_explicit_and_implicit_replacement_semantics() {
    for explicit in [false, true] {
        let fixture = Fixture::new();
        let (code, result) = cli(
            &fixture.dir,
            &["try", &complete_helper().to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{result}");
        let mut patch =
            json!({"af1":1,"patch":[{"fn":"helper","blocks":{"entry":null,"negative":null}}]});
        if explicit {
            patch["patch"][0]["entry"] = json!("positive");
        }
        let request = calling_request();
        let report = check(&fixture, &request, &patch).unwrap();
        assert_eq!(
            report["declared_body_control_flow"]["status"],
            if explicit {
                "explicit_targets_checked"
            } else {
                "partial"
            },
            "{report}"
        );
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let mut request = request;
        request["base"] = json!("d2@r1");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", "-7", "false", "--on", "c3"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!(-7));
    }
}

#[test]
fn real_source_draft_refusal_preserves_revision_and_publishes_no_candidate() {
    let fixture = Fixture::new();
    let source = helper(json!([{"name":"entry","ops":[],"term":["br","absent"]}]));
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["draft"], "d1@r1", "{result}");
    let directory = fixture.dir.join(".sley/drafts/d1/r1");
    let snapshot: std::collections::BTreeMap<_, _> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_owned(),
                std::fs::read(path).unwrap(),
            )
        })
        .collect();
    let head = fixture.workspace.read_head().unwrap().transaction_id();
    let mut request = calling_request();
    request["base"] = json!("d1@r1");
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
            .contains("/fns/0/blocks/0/term/1"),
        "{result}"
    );
    for (name, bytes) in snapshot {
        assert_eq!(std::fs::read(directory.join(name)).unwrap(), bytes);
    }
    assert!(!fixture.dir.join(".sley/candidates").exists());
    assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
    assert!(!fixture.dir.join(".sley/residual").exists());
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        head
    );
}
