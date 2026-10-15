use serde_json::{Value, json};
use sley_agent::{
    candidate, hex,
    names::{NameMap, Names},
};
use std::fs;

use super::{Fixture, cli, literal_request};

fn setup() -> Fixture {
    let fixture = Fixture::new();
    let frame = json!({"af1":1,"afx":1,"fns":[{
        "fn":"adjust","params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>",
        "blocks":[{"name":"entry","ops":[["amount","const",{"type":"i8","value":3}],
            ["sum","add","x","amount"]],"term":["return","sum"]}]}],
        "test_tables":[{"name":"table","fn":"adjust","cases":[{"args":[5],"expect":{"Ok":8}}]}]});
    let (code, report) = cli(&fixture.dir, &["try", &frame.to_string()]);
    assert_eq!(code, 0, "{report}");
    fixture
}

fn read(fixture: &Fixture, path: &str) -> Value {
    serde_json::from_slice(&fs::read(fixture.dir.join(path)).unwrap()).unwrap()
}

fn stored(fixture: &Fixture, handle: &str) -> sley_mutate::ImportedCandidate {
    let hex =
        fs::read_to_string(fixture.dir.join(format!(".sley/candidates/{handle}.hex"))).unwrap();
    sley_mutate::import_candidate(&hex::decode(hex.trim()).unwrap()).unwrap()
}

fn request(value: i64, base: &str) -> Value {
    let mut request = literal_request(value);
    request["base"] = json!(base);
    request
}

#[test]
fn draft_cli_publishes_preserved_graph_and_inherited_table_tests() {
    let fixture = setup();
    let old = stored(&fixture, "c1");
    let head = fixture.workspace.read_head().unwrap();
    let before = candidate::applied_program(&head, &old).unwrap();
    let map = NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap();
    let names = Names::build(&before, &map);
    let target = names.resolve("adjust.entry.amount").unwrap();
    let source_status = read(&fixture, ".sley/drafts/d1/r1/status.json");
    let source_frame = read(&fixture, ".sley/drafts/d1/r1/frame.json");
    let (code, report) = cli(
        &fixture.dir,
        &[
            "residual",
            "try",
            &request(7, "d1@r1").to_string(),
            "--verbose",
        ],
    );
    assert_eq!(code, 1, "{report}");
    assert_eq!(report["draft"], "d1@r2");
    assert_eq!(report["kernel"], "valid");
    assert_eq!(report["public_checks"], "failed");
    assert_eq!(report["checks"]["cases"], 1);
    assert_eq!(report["trial"]["tests"][0]["actual"], "Ok(12)");
    assert_eq!(report["trial"]["tests"][0]["expected"], "Ok(8)");
    let next = stored(&fixture, report["handle"].as_str().unwrap());
    assert_eq!(old.record.candidate_nonce, next.record.candidate_nonce);
    let after = candidate::applied_program(&head, &next).unwrap();
    for object in before.objects() {
        if object.record().entity_id != target {
            assert_eq!(
                object.stored_bytes(),
                after
                    .object(&object.record().entity_id)
                    .unwrap()
                    .stored_bytes()
            );
        }
    }
    let status = read(&fixture, ".sley/drafts/d1/r2/status.json");
    assert_eq!(status["tables"], source_status["tables"]);
    assert_eq!(status["tests"], source_status["tests"]);
    assert_eq!(status["sources"], source_status["sources"]);
    assert_eq!(
        status["residual"]["draft_graph"]["publication"]["draft"],
        "d1@r2"
    );
    let mut expected = source_frame.clone();
    expected["fns"][0]["blocks"][0]["ops"][0][2]["value"] = json!(7);
    expected["consts"] = json!([{"name":"k_3","type":"i8","value":3}]);
    assert_eq!(read(&fixture, ".sley/drafts/d1/r2/frame.json"), expected);
    assert_eq!(
        read(&fixture, ".sley/drafts/d1/r1/frame.json"),
        source_frame
    );
    let origin = read(&fixture, ".sley/drafts/d1/r2/residual-provenance.json");
    assert_eq!(
        origin["entries"]["/fns/0/blocks/0/ops/0/2/value"]["pointer"],
        "/bindings/value"
    );
    let (code, report) = cli(&fixture.dir, &["try", r#"{"af1":1}"#, "--on", "d1@r2"]);
    assert_eq!(code, 1, "{report}");
    assert_eq!(report["tests"][0]["actual"], "Ok(12)");
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        head.transaction_id()
    );
}

#[test]
fn draft_cli_noop_checks_source_tests_without_creating_candidate_or_revision() {
    let fixture = setup();
    let original = fs::read(fixture.dir.join(".sley/candidates/c1.hex")).unwrap();
    let (code, report) = cli(
        &fixture.dir,
        &[
            "residual",
            "try",
            &request(3, "d1@r1").to_string(),
            "--verbose",
        ],
    );
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["no_change"], true);
    assert_eq!(report["kernel"], "valid");
    assert_eq!(report["public_checks"], "passed");
    assert_eq!(report["checks"]["count"], 1);
    assert_eq!(report["checks"]["tests"][0]["actual"], "Ok(8)");
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
    assert!(!fixture.dir.join(".sley/drafts/d1/r2/status.json").exists());
    assert_eq!(
        fs::read(fixture.dir.join(".sley/candidates/c1.hex")).unwrap(),
        original
    );
}

#[test]
fn draft_cli_explicit_plan_fill_and_stale_replay() {
    let fixture = setup();
    let mut request = request(7, "d1@r1");
    request["bindings"].as_object_mut().unwrap().remove("value");
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    let answer = json!({"residual":1,"plan":plan["plan"],"choose":{"/bindings/value":7}});
    let (code, report) = cli(
        &fixture.dir,
        &[
            "residual",
            "fill",
            plan["plan"].as_str().unwrap(),
            &answer.to_string(),
        ],
    );
    assert_eq!(code, 1, "{report}");
    assert_eq!(report["kernel"], "valid");
    assert_eq!(report["draft"], "d1@r2");
    let (code, stale) = cli(
        &fixture.dir,
        &["residual", "try", &self::request(9, "d1@r1").to_string()],
    );
    assert_eq!(code, 2, "{stale}");
    assert!(!fixture.dir.join(".sley/drafts/d1/r3/status.json").exists());
}

#[test]
fn draft_cli_ready_and_closed_relation_plans_keep_validation_evidence() {
    for relation in [false, true] {
        let fixture = setup();
        let mut request = request(7, "d1@r1");
        if relation {
            request["bindings"].as_object_mut().unwrap().remove("value");
            request["choices"] = json!({"version":1,"contract":"author_closed_relation",
                "rows":[{"/bindings/value":7},{"/bindings/value":9}]});
        }
        let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
        assert_eq!(code, 0, "{plan}");
        if relation {
            assert_eq!(plan["family_checks"]["kernel"], "all_rows_valid");
            assert_eq!(plan["family_checks"]["kernel_validations"], 4);
        } else {
            assert_eq!(plan["kernel"], "valid");
            assert_eq!(plan["draft_graph"]["source_revision"], "d1@r1");
        }
        let events = fs::read_to_string(fixture.dir.join(".sley/events.jsonl")).unwrap();
        let event: Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
        assert_eq!(
            event["residual"]["kernel_validations"],
            if relation { 4 } else { 2 }
        );
        assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
        assert!(!fixture.dir.join(".sley/drafts/d1/r2/status.json").exists());
        let choose = if relation {
            json!({"/bindings/value":7})
        } else {
            json!({})
        };
        let fill = json!({"residual":1,"plan":plan["plan"],"choose":choose});
        let (code, result) = cli(
            &fixture.dir,
            &[
                "residual",
                "fill",
                plan["plan"].as_str().unwrap(),
                &fill.to_string(),
            ],
        );
        assert_eq!(code, 1, "{result}");
        assert_eq!(result["draft"], "d1@r2");
        assert_eq!(result["kernel"], "valid");
    }
}

#[test]
fn draft_cli_inherits_imported_test_provenance_and_exact_table_versions() {
    let fixture = setup();
    let file = fixture.dir.join("public.json");
    fs::write(
        &file,
        json!([{"name":"external_case","function":"adjust","args":[1],"expect":{"Ok":4}}])
            .to_string(),
    )
    .unwrap();
    let (code, imported) = cli(
        &fixture.dir,
        &["import", file.to_str().unwrap(), "--on", "d1@r1"],
    );
    assert_eq!(code, 0, "{imported}");
    let source = read(&fixture, ".sley/drafts/d1/r2/status.json");
    assert_eq!(source["tests"]["imported"], 1);
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request(7, "d1@r2").to_string()],
    );
    assert_eq!(code, 1, "{result}");
    assert_eq!(result["checks"]["cases"], 2);
    let edited = read(&fixture, ".sley/drafts/d1/r3/status.json");
    assert_eq!(edited["sources"], source["sources"]);
    assert_eq!(edited["tests"], source["tests"]);
    assert_eq!(edited["tables"], source["tables"]);
}

#[test]
fn draft_cli_refuses_bad_relation_rows_without_pruning_or_draft_publication() {
    let fixture = setup();
    let mut request = request(7, "d1@r1");
    request["bindings"].as_object_mut().unwrap().remove("value");
    request["choices"] = json!({"version":1,"contract":"author_closed_relation",
        "rows":[{"/bindings/value":7},{"/bindings/value":128}]});
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["kernel"], "unknown");
    assert!(result.get("plan").is_none());
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
    assert!(!fixture.dir.join(".sley/drafts/d1/r2/status.json").exists());
}

#[test]
fn draft_cli_competing_edits_publish_only_one_successor() {
    let fixture = setup();
    let request = request(7, "d1@r1").to_string();
    let barrier = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let run = || {
            barrier.wait();
            cli(&fixture.dir, &["residual", "try", &request, "--no-test"])
        };
        let first = scope.spawn(run);
        let second = scope.spawn(run);
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(
        results.iter().filter(|(code, _)| *code == 0).count(),
        1,
        "{results:?}"
    );
    assert_eq!(
        results.iter().filter(|(code, _)| *code == 2).count(),
        1,
        "{results:?}"
    );
    assert!(fixture.dir.join(".sley/candidates/c2.hex").exists());
    assert!(!fixture.dir.join(".sley/candidates/c3.hex").exists());
    let revisions = fs::read_dir(fixture.dir.join(".sley/drafts/d1"))
        .unwrap()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().join("status.json").is_file())
        .count();
    assert_eq!(revisions, 2);
}
