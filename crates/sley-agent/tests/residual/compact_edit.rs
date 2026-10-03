use super::{Fixture, cli, literal_fixture, literal_request};
use serde_json::{Value, json};
use std::fs;

#[test]
fn target_view_refuses_non_literal_drafts_and_plans() {
    let fixture = Fixture::new();
    let before = fixture.workspace.read_head().unwrap().transaction_id();
    let request = json!({"residual":1,"base":"current","operation":"derive",
        "fragment":{"id":"checked_pipeline","version":1},"scope":["scaled"],
        "bindings":{"params":[["x","i64"]],"returns":"Result<i64,ArithmeticError>",
            "steps":[["r",["mul","x",2]]],"arithmetic_failure":{"propagate":true},
            "rounding":"toward_zero","result":"r"}});
    let (code, trial) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{trial}");
    let (code, refused) = cli(
        &fixture.dir,
        &[
            "residual",
            "show",
            trial["draft"].as_str().unwrap(),
            "--targets",
        ],
    );
    assert_eq!(code, 2, "{refused}");
    assert_eq!(refused["error"], "AGENT_USAGE_INVALID");
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    let (code, refused) = cli(
        &fixture.dir,
        &[
            "residual",
            "show",
            plan["plan"].as_str().unwrap(),
            "--targets",
        ],
    );
    assert_eq!(code, 2, "{refused}");
    assert_eq!(refused["error"], "AGENT_USAGE_INVALID");
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
}

#[test]
fn compact_edit_keeps_full_source_evidence_available_without_echoing_it() {
    let fixture = literal_fixture();
    let before = fixture.workspace.read_head().unwrap().transaction_id();
    let (code, trial) = cli(
        &fixture.dir,
        &[
            "residual",
            "try",
            &literal_request(7).to_string(),
            "--no-test",
        ],
    );
    assert_eq!(code, 0, "{trial}");
    assert_eq!(trial["kernel"], "valid");
    assert_eq!(trial["edit"]["verification"], "passed");
    assert_eq!(trial["edit"]["targets_count"], 1);
    assert_eq!(trial["edit"]["targets_changed"], 1);
    assert_eq!(trial["edit"]["target_details_omitted"], true);
    assert_eq!(
        trial["edit"]["targets"][0],
        json!({
            "scope_index":0,"name":"adjust.entry.amount","changed":true,"requested":7
        })
    );
    let reference = trial["draft"].as_str().unwrap();
    assert_eq!(
        trial["edit"]["inspect"],
        format!("sley-agent residual show {reference} --provenance")
    );
    let (code, full) = cli(
        &fixture.dir,
        &["residual", "show", reference, "--provenance"],
    );
    assert_eq!(code, 0, "{full}");
    let source = &full["edit"]["targets"][0]["source"];
    for key in ["entity", "object", "constant", "constant_object"] {
        assert_eq!(source[key].as_str().unwrap().len(), 64);
    }
    assert_eq!(source["previous"], 3);
    assert_eq!(source["checked_consumers"], json!(["adjust.entry.sum"]));
    assert_eq!(full["edit"]["target_details_omitted"], Value::Null);
    let (code, targets) = cli(&fixture.dir, &["residual", "show", reference, "--targets"]);
    assert_eq!(code, 0, "{targets}");
    assert_eq!(targets["historical"], true);
    assert_eq!(targets["edit"]["targets_count"], 1);
    assert_eq!(
        targets["edit"]["targets"][0],
        json!({
            "scope_index":0,"name":"adjust.entry.amount","changed":true,
            "width":"i8","previous":3,"requested":7
        })
    );
    assert_eq!(targets["edit"]["targets"][0]["source"], Value::Null);
    let (code, refused) = cli(
        &fixture.dir,
        &["residual", "show", reference, "--targets", "--provenance"],
    );
    assert_eq!(code, 2, "{refused}");
    assert_eq!(refused["error"], "AGENT_USAGE_INVALID");
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
}

#[test]
fn verbose_edit_and_noop_provenance_keep_complete_target_details() {
    let fixture = literal_fixture();
    let (code, verbose) = cli(
        &fixture.dir,
        &[
            "residual",
            "try",
            &literal_request(7).to_string(),
            "--no-test",
            "--verbose",
        ],
    );
    assert_eq!(code, 0, "{verbose}");
    assert!(verbose["edit"]["targets"][0]["source"].is_object());
    assert_eq!(verbose["edit"]["target_details_omitted"], Value::Null);
    let (code, noop) = cli(
        &fixture.dir,
        &["residual", "try", &literal_request(3).to_string()],
    );
    assert_eq!(code, 0, "{noop}");
    assert_eq!(noop["no_change"], true);
    assert_eq!(noop["edit"]["targets_changed"], 0);
    assert_eq!(noop["edit"]["target_details_omitted"], true);
    let reference = noop["residual"].as_str().unwrap();
    let (code, shown) = cli(&fixture.dir, &["residual", "show", reference]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["edit"]["targets_count"], 1);
    assert_eq!(shown["edit"]["targets_changed"], 0);
    assert_eq!(shown["edit"]["targets"][0]["source"], Value::Null);
    let (code, full) = cli(
        &fixture.dir,
        &["residual", "show", reference, "--provenance"],
    );
    assert_eq!(code, 0, "{full}");
    assert!(full["edit"]["targets"][0]["source"].is_object());
    assert_eq!(full["edit"]["targets"][0]["changed"], false);
    let (code, targets) = cli(&fixture.dir, &["residual", "show", reference, "--targets"]);
    assert_eq!(code, 0, "{targets}");
    assert_eq!(targets["edit"]["targets_changed"], 0);
    assert_eq!(targets["edit"]["targets"][0]["previous"], 3);
    assert_eq!(targets["edit"]["targets"][0]["requested"], 3);
}

#[test]
fn noop_historical_summary_omits_repeated_checks_but_preserves_failures_and_inspection() {
    let fixture = literal_fixture();
    let before = fixture.workspace.read_head().unwrap().transaction_id();
    let cases: Vec<_> = (0..128)
        .map(|index| {
            json!({
                "name":format!("case_{index}"),"function":"adjust","args":[5],
                "expect":{"Ok":if index == 127 {9} else {8}}
            })
        })
        .collect();
    let path = fixture.dir.join("public.json");
    fs::write(&path, json!(cases).to_string()).unwrap();
    let (code, trial) = cli(
        &fixture.dir,
        &[
            "residual",
            "try",
            &literal_request(3).to_string(),
            "--public",
            path.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 1, "{trial}");
    let (code, verbose) = cli(
        &fixture.dir,
        &[
            "residual",
            "try",
            &literal_request(3).to_string(),
            "--public",
            path.to_str().unwrap(),
            "--verbose",
        ],
    );
    assert_eq!(code, 1, "{verbose}");
    assert_eq!(verbose["checks"]["public"].as_array().unwrap().len(), 128);
    assert_eq!(verbose["checks"]["details_omitted"], Value::Null);
    fs::remove_file(&path).unwrap(); // Historical inspection cannot rerun this file.
    let reference = trial["residual"].as_str().unwrap();
    let (code, shown) = cli(&fixture.dir, &["residual", "show", reference]);
    assert_eq!(code, 0, "{shown}");
    assert!(
        shown["checks"]["public"].is_null(),
        "historical summary repeats all public results"
    );
    for value in [&trial, &shown] {
        assert_eq!(value["public_checks"], "failed");
        assert_eq!(value["checks"]["count"], 128);
        assert_eq!(value["checks"]["details_omitted"], true);
        assert_eq!(value["checks"]["public_omitted"], 128);
        assert_eq!(value["checks"]["failures"][0]["case"]["name"], "case_127");
        assert_eq!(
            value["checks"]["failures"][0]["case"]["actual"],
            json!({"Ok":8})
        );
        assert_eq!(
            value["checks"]["failures"][0]["case"]["expected"],
            json!({"Ok":9})
        );
        assert_eq!(
            value["checks"]["inspect"],
            format!("sley-agent residual show {reference} --provenance")
        );
    }
    let (code, full) = cli(
        &fixture.dir,
        &["residual", "show", reference, "--provenance"],
    );
    assert_eq!(code, 0, "{full}");
    assert_eq!(full["checks"]["public"].as_array().unwrap().len(), 128);
    assert_eq!(full["checks"]["public"][127]["pass"], false);
    assert_eq!(full["checks"]["details_omitted"], Value::Null);
    assert_eq!(full["checks"], verbose["checks"]);
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
}

#[test]
fn maximum_batch_reply_is_small_and_complete_provenance_remains_local() {
    let fixture = Fixture::new();
    let functions: Vec<_> = (0..64).map(|index| json!({
        "fn":format!("f{index}"),"params":[["x","i16"]],"returns":"Result<i16,ArithmeticError>",
        "blocks":[{"name":"entry","ops":[["amount","const",{"type":"i16","value":3}],
            ["sum","add","x","amount"]],"term":["return","sum"]}]
    })).collect();
    let (code, initial) = cli(
        &fixture.dir,
        &[
            "try",
            &json!({"af1":1,"fns":functions}).to_string(),
            "--no-test",
        ],
    );
    assert_eq!(code, 0, "{initial}");
    let (code, committed) = cli(
        &fixture.dir,
        &["commit", initial["handle"].as_str().unwrap()],
    );
    assert_eq!(code, 0, "{committed}");
    let mut request = literal_request(7);
    request["scope"] = json!(
        (0..64)
            .map(|index| format!("f{index}.entry.amount"))
            .collect::<Vec<_>>()
    );
    let (code, trial) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{trial}");
    assert_eq!(trial["edit"]["targets_count"], 64);
    assert_eq!(trial["edit"]["targets_changed"], 64);
    assert_eq!(trial["edit"]["targets"].as_array().unwrap().len(), 16);
    assert_eq!(trial["edit"]["targets_omitted"], 48);
    assert!(
        serde_json::to_vec(&trial).unwrap().len() < 6_000,
        "compact reply must not repeat sixteen full source records"
    );
    let (code, full) = cli(
        &fixture.dir,
        &[
            "residual",
            "show",
            trial["draft"].as_str().unwrap(),
            "--provenance",
        ],
    );
    assert_eq!(code, 0, "{full}");
    assert_eq!(full["edit"]["targets"].as_array().unwrap().len(), 64);
    assert!(
        full["edit"]["targets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|target| target["source"].is_object())
    );
    let (code, targets) = cli(
        &fixture.dir,
        &[
            "residual",
            "show",
            trial["draft"].as_str().unwrap(),
            "--targets",
        ],
    );
    assert_eq!(code, 0, "{targets}");
    assert_eq!(targets["edit"]["targets_count"], 64);
    assert_eq!(targets["edit"]["targets"].as_array().unwrap().len(), 64);
    for (index, target) in targets["edit"]["targets"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(target["name"], format!("f{index}.entry.amount"));
        assert_eq!(target["width"], "i16");
        assert_eq!(target["previous"], 3);
        assert_eq!(target["requested"], 7);
        assert_eq!(target["source"], Value::Null);
    }
    assert!(serde_json::to_vec(&targets).unwrap().len() < serde_json::to_vec(&full).unwrap().len());
}
