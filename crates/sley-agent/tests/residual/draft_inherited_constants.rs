use std::fs;

use serde_json::{Value, json};
use sley_agent::{
    candidate, hex,
    names::{NameMap, Names},
};
use sley_mutate::value::EntityBodyValue;

use super::dependency_tests::literal_function;
use super::draft_receipt_tests::{capture, replace_frame, setup};
use super::{Fixture, cli, literal_request};

fn read(fixture: &Fixture, path: &str) -> Value {
    serde_json::from_slice(&fs::read(fixture.dir.join(path)).unwrap()).unwrap()
}

fn graph(fixture: &Fixture, handle: &str) -> sley_agent::workspace::Program {
    let stored =
        fs::read_to_string(fixture.dir.join(format!(".sley/candidates/{handle}.hex"))).unwrap();
    let candidate = sley_mutate::import_candidate(&hex::decode(stored.trim()).unwrap()).unwrap();
    candidate::applied_program(&fixture.workspace.read_head().unwrap(), &candidate).unwrap()
}

fn symbols(fixture: &Fixture, program: &sley_agent::workspace::Program) -> Names {
    Names::build(
        program,
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    )
}

#[test]
fn draft_inherited_constants_keep_source_origins_and_survive_ordinary_layering() {
    let fixture = setup(&json!({"af1":1,"afx":1,"fns":[literal_function("adjust")]}));
    let mut source_handle = "c1".to_owned();
    for (revision, value) in [(1, 7), (2, 9)] {
        let source = graph(&fixture, &source_handle);
        let mut request = literal_request(value);
        request["base"] = json!(format!("d1@r{revision}"));
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        let events = fs::read_to_string(fixture.dir.join(".sley/events.jsonl")).unwrap();
        let event: Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
        assert_eq!(event["residual"]["inherited_constant_declarations"], 1);
        source_handle = result["handle"].as_str().unwrap().to_owned();
        let directory = format!(".sley/drafts/d1/r{}", revision + 1);
        let status = read(&fixture, &format!("{directory}/status.json"));
        assert_eq!(
            status["residual"]["draft_graph"]["composed_frame_assertions"]["complete_candidate_correspondence"],
            "verified_with_inline_constant_aliases"
        );
        let inheritance = read(
            &fixture,
            &format!("{directory}/residual-inherited-constants.json"),
        );
        assert_eq!(inheritance["revision"], format!("d1@r{revision}"));
        assert_eq!(inheritance["entries"].as_array().unwrap().len(), 1);
        let provenance = read(&fixture, &format!("{directory}/residual-provenance.json"));
        let frame = read(&fixture, &format!("{directory}/frame.json"));
        for entry in inheritance["entries"].as_array().unwrap() {
            let id = sley_id::EntityId::from_bytes(
                hex::decode32(entry["entity"].as_str().unwrap()).unwrap(),
            );
            assert_eq!(
                entry["object"],
                hex::encode(source.object(&id).unwrap().object_id().as_bytes())
            );
            let pointer = entry["authored"].as_str().unwrap();
            assert!(frame.pointer(pointer).is_some());
            assert_eq!(provenance["entries"][pointer]["class"], "INHERITED");
            assert_eq!(
                provenance["entries"][pointer]["binding"],
                inheritance["binding"]
            );
            assert_eq!(
                provenance["entries"][pointer]["historical_authorship"],
                "not_asserted"
            );
        }
    }
    let (code, result) = cli(
        &fixture.dir,
        &["try", r#"{"af1":1}"#, "--on", "d1@r3", "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let after = graph(&fixture, result["handle"].as_str().unwrap());
    let names = symbols(&fixture, &after);
    for (name, value) in [("k_3", 3), ("k_7", 7)] {
        let EntityBodyValue::Constant(constant) =
            after.body(&names.resolve(name).unwrap()).unwrap()
        else {
            panic!("constant")
        };
        assert_eq!(
            sley_agent::values::to_json(&constant.value, &names),
            json!(value)
        );
    }
    let (_, called) = cli(
        &fixture.dir,
        &[
            "call",
            "adjust",
            "5",
            "--on",
            result["handle"].as_str().unwrap(),
        ],
    );
    assert_eq!(called["result"], json!({"Ok":14}));
}

#[test]
fn draft_inherited_constants_close_coverage_after_equal_alias_selection_changes() {
    let mut frame = json!({"af1":1,
        "consts":[{"name":"left","type":"i8","value":3},{"name":"right","type":"i8","value":3}],
        "fns":[literal_function("adjust"),literal_function("other")]});
    frame["fns"][0]["blocks"][0]["ops"][0][2] = json!("left");
    frame["fns"][1]["blocks"][0]["ops"][0][2] = json!("right");
    let fixture = setup(&frame);
    frame.as_object_mut().unwrap().remove("consts");
    for function in frame["fns"].as_array_mut().unwrap() {
        function["blocks"][0]["ops"][0][2] = json!({"type":"i8","value":3});
    }
    replace_frame(&fixture, &frame);
    let prepared = capture(&fixture, "d1@r1")
        .prepare(
            &fixture.workspace,
            &mut sley_agent::residual::frontier::Budget::default(),
        )
        .unwrap();
    assert_eq!(
        prepared.report["source_frame_assertions"]["complete_candidate_correspondence"],
        "not_established"
    );
    assert_eq!(
        prepared.report["composed_frame_assertions"]["complete_candidate_correspondence"],
        "verified_with_inline_constant_aliases"
    );
    let retained = prepared.authoring.unwrap();
    assert_eq!(retained.inherited_constants.len(), 2);
    let mut constants = retained.frame["consts"].as_array().unwrap().clone();
    constants.sort_by_key(|constant| constant["name"].as_str().unwrap().to_owned());
    assert_eq!(
        constants,
        json!([{"name":"left","type":"i8","value":3},{"name":"right","type":"i8","value":3}])
            .as_array()
            .unwrap()
            .clone()
    );
}

#[test]
fn draft_inherited_constants_render_exact_typed_values() {
    let constants = json!([
        {"name":"wide","type":"i128","value":"170141183460469231731687303715884105727"},
        {"name":"unsigned","type":"u128","value":"340282366920938463463374607431768211455"},
        {"name":"fraction","type":"f64","value":1.25},
        {"name":"bytes","type":"bytes","value":"0x00ff"},
        {"name":"point","type":"Point","value":{"x":3,"y":4}},
        {"name":"error","type":"E","value":{"Code":7}}
    ]);
    let mut frame = json!({"af1":1,"consts":constants,
        "types":[{"name":"Point","record":[["x","i8"],["y","i8"]]},
            {"name":"E","variant":[["Code","i8"]]}],
        "fns":[literal_function("adjust")]});
    let fixture = setup(&frame);
    frame.as_object_mut().unwrap().remove("consts");
    replace_frame(&fixture, &frame);
    let prepared = capture(&fixture, "d1@r1")
        .prepare(
            &fixture.workspace,
            &mut sley_agent::residual::frontier::Budget::default(),
        )
        .unwrap();
    let retained = prepared.authoring.unwrap();
    for original in constants.as_array().unwrap() {
        let restored = retained.frame["consts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == original["name"])
            .unwrap();
        assert_eq!(restored, original);
    }
    let (code, result) = cli(
        &fixture.dir,
        &["try", &retained.frame.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    let after = graph(&fixture, result["handle"].as_str().unwrap());
    let names = symbols(&fixture, &after);
    let EntityBodyValue::Constant(constant) =
        after.body(&names.resolve("fraction").unwrap()).unwrap()
    else {
        panic!("constant")
    };
    assert_eq!(
        constant.value.data,
        sley_ssmc::ConstData::F64Bits(1.25_f64.to_bits())
    );
}

#[test]
fn draft_inherited_constants_noop_does_not_rewrite_an_incomplete_receipt() {
    let mut frame = json!({"af1":1,"consts":[{"name":"unused","type":"i8","value":99}],
        "fns":[literal_function("adjust")]});
    let fixture = setup(&frame);
    frame.as_object_mut().unwrap().remove("consts");
    replace_frame(&fixture, &frame);
    let bytes = fs::read(fixture.dir.join(".sley/drafts/d1/r1/frame.json")).unwrap();
    let mut request = literal_request(3);
    request["base"] = json!("d1@r1");
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["no_change"], true);
    assert_eq!(
        fs::read(fixture.dir.join(".sley/drafts/d1/r1/frame.json")).unwrap(),
        bytes
    );
    assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
}
