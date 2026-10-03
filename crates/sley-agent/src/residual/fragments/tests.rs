use std::{collections::BTreeMap, time::Duration};

use serde_json::{Value, json};

use super::{Builder, expand, expand_with_budget, mentions_prefix};
use crate::{
    AgentErrorCode,
    residual::{self, frontier::Budget},
};

fn request(fragment: &str, bindings: Value) -> residual::Request {
    let mut value = json!({"residual":1,"base":"current","operation":"derive",
        "fragment":{"id":fragment,"version":1},"scope":["sample"]});
    value["bindings"] = bindings;
    residual::request_from_value(value).unwrap()
}

fn examples() -> Vec<residual::Request> {
    vec![
        request(
            "checked_pipeline",
            json!({"params":[["x","i16"]],"returns":"Result<i16,ArithmeticError>",
            "steps":[["answer",["mul",["add","x",1],2]]],"arithmetic_failure":{"propagate":true},
            "rounding":"toward_zero","result":"answer"}),
        ),
        request(
            "ordered_guard_chain",
            json!({"params":[["x","i16"]],"returns":"Result<i16,Error>",
            "guards":[{"when":["lt","x",0],"fail":["fail","Negative"]}],
            "success":{"ops":[],"term":["ok","x"]}}),
        ),
        request(
            "typed_branch_result",
            json!({"params":[["x","Option<i16>"]],"returns":"i16",
            "input":"x","cases":[{"case":"None","payload":null,"ops":[],"values":[0]},
            {"case":"Some","payload":["v","i16"],"ops":[],"values":["v"]}],
            "join":{"params":[["v","i16"]],"ops":[],"term":["return","v"]}}),
        ),
    ]
}

#[test]
fn all_fragment_families_share_work_and_retain_frames_and_provenance() {
    for request in examples() {
        let original = request.value();
        let expected = expand(&request).unwrap();
        let mut measured = Budget::default();
        let actual = expand_with_budget(&request, &mut measured).unwrap();
        assert_eq!(actual.frame, expected.frame);
        assert_eq!(actual.provenance, expected.provenance);
        let work = measured.usage()["charged_work"].as_u64().unwrap();
        assert!(work > 20);
        let mut shared = Budget::limited(Duration::from_secs(2), 2 * work - 1);
        expand_with_budget(&request, &mut shared).unwrap();
        assert_eq!(
            expand_with_budget(&request, &mut shared)
                .unwrap_err()
                .code(),
            AgentErrorCode::ResidualLimit
        );
        assert_eq!(request.value(), original);
    }
}

#[test]
fn elapsed_or_spent_budget_refuses_before_fragment_construction() {
    for mut budget in [
        Budget::limited(Duration::ZERO, u64::MAX),
        Budget::limited(Duration::from_secs(2), 0),
    ] {
        assert_eq!(
            expand_with_budget(&examples()[0], &mut budget)
                .unwrap_err()
                .code(),
            AgentErrorCode::ResidualLimit
        );
    }
}

#[test]
fn name_capture_scan_charges_each_visited_value_and_keeps_deterministic_names() {
    let value = json!(["first",{"nested":["second","gw_capture"]}]);
    let mut measured = Budget::default();
    assert!(mentions_prefix(&value, "gw", &mut measured).unwrap());
    assert_eq!(measured.usage()["charged_work"], 6);
    let mut limited = Budget::limited(Duration::from_secs(2), 5);
    assert_eq!(
        mentions_prefix(&value, "gw", &mut limited)
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualLimit
    );
    let request = request(
        "ordered_guard_chain",
        json!({"params":[["gw","i16"],["gwg","i16"]],"returns":"i16",
        "guards":[],"success":{"ops":[],"term":["return","gw"]}}),
    );
    let expansion = expand_with_budget(&request, &mut Budget::default()).unwrap();
    assert_eq!(expansion.frame["fns"][0]["blocks"][0]["name"], "gwgg_b0");
}

#[test]
fn generated_block_ceiling_is_checked_before_pushing_another_block() {
    let mut budget = Budget::default();
    let mut builder = Builder {
        budget: &mut budget,
        blocks: vec![Value::Null; crate::afx::MAX_GENERATED_BLOCKS_PER_FUNCTION],
        provenance: BTreeMap::new(),
        items: 0,
        prefix: "gw".into(),
    };
    let before = builder.blocks.len();
    assert_eq!(
        builder
            .block(json!([]), json!(["trap"]), json!({}))
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(builder.blocks.len(), before);
    assert!(builder.provenance.is_empty());
}

#[test]
fn nested_guards_cannot_expand_beyond_the_existing_af1x_block_limit() {
    let mut success = json!({"ops":[],"term":["return",0]});
    for _ in 0..8 {
        success = json!({"fragment":{"id":"ordered_guard_chain","version":1},"bindings":{
            "guards":vec![json!({"when":true,"fail":["fail","Stop"]});64],"success":success}});
    }
    let mut bindings = success["bindings"].clone();
    bindings["params"] = json!([]);
    bindings["returns"] = json!("i16");
    let request = request("ordered_guard_chain", bindings);
    let error = expand_with_budget(&request, &mut Budget::default()).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("fragment construction budget"));
}
