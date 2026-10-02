use super::{Budget, Family, FamilyInput, analyze_with_budget, encoding};
use crate::error::AgentErrorCode;
use crate::residual::{MAX_REQUEST_BYTES, parse_request};
use serde_json::{Value, json};
use std::time::Duration;

fn request() -> crate::residual::Request {
    let mut value: Value = serde_json::from_str(
        crate::help::RESIDUAL
            .lines()
            .find(|line| line.starts_with("{\"residual\":"))
            .unwrap(),
    )
    .unwrap();
    for key in ["params", "returns", "rounding"] {
        value["bindings"].as_object_mut().unwrap().remove(key);
    }
    value["choices"] = json!({"version":1,"contract":"author_closed_relation","rows":[
        {"/bindings/params":[["x","i64"]],"/bindings/returns":"Result<i64,ArithmeticError>","/bindings/rounding":"toward_zero"},
        {"/bindings/params":[["x","i8"]],"/bindings/returns":"Result<i8,ArithmeticError>","/bindings/rounding":"toward_zero"}
    ]});
    parse_request(&serde_json::to_vec(&value).unwrap()).unwrap()
}

#[test]
fn borrowed_relation_serialization_preserves_bytes_and_shares_family_peak() {
    let fields = vec![json!({"name":"x","cost":1,"eligible":true})];
    let rows = [json!({"x":[null,true,1,"1","\0\n💡"]}), json!({"x":false})]
        .into_iter()
        .map(|value| value.as_object().unwrap().clone())
        .collect::<Vec<_>>();
    let source = FamilyInput {
        fields: &fields,
        descriptions: &rows,
    };
    let expected = serde_json::to_vec(&json!({"fields":fields,"descriptions":rows})).unwrap();
    let mut budget = Budget::default();
    let encoded = encoding::encode(&source, &mut budget, MAX_REQUEST_BYTES).unwrap();
    assert_eq!(encoded.bytes, expected);
    let family = Family::parse_with_budget(&encoded.bytes, &mut budget).unwrap();
    let live = budget.usage()["memory"]["reserved_bytes"].as_u64().unwrap();
    let peak = budget.usage()["memory"]["peak_reserved_bytes"]
        .as_u64()
        .unwrap();
    assert!(live > encoded.bytes.len() as u64);
    drop(family);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], expected.len());
    drop(encoded);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);

    let mut budget = Budget::limited_with_memory(
        Duration::from_secs(2),
        100_000,
        usize::try_from(peak).unwrap() - 1,
    );
    let encoded = encoding::encode(&source, &mut budget, MAX_REQUEST_BYTES).unwrap();
    assert_eq!(
        Family::parse_with_budget(&encoded.bytes, &mut budget)
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(
        budget.usage()["memory"]["reserved_bytes"],
        encoded.bytes.len()
    );
    drop(encoded);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(budget.checkpoint().is_err());
}

#[test]
fn relation_analysis_retains_only_family_reservations_and_keeps_authored_row_origins() {
    let request = request();
    let before = request.value();
    let mut budget = Budget::default();
    let relation = analyze_with_budget(&request, &mut budget).unwrap();
    assert_eq!(request.value(), before);
    let mut expected_cost = 0;
    for question in relation.decisions() {
        let cost = serde_json::to_vec(&question).unwrap().len() + 1;
        // The surrogate and typed-domain order match the unbounded byte encoder.
        expected_cost += cost as u64;
        let values = question["supported_values"].as_array().unwrap();
        let keys: Vec<_> = values
            .iter()
            .map(|value| serde_json::to_vec(value).unwrap())
            .collect();
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    }
    assert_eq!(relation.frontier.cost(), expected_cost);
    for (index, row) in relation.rows.iter().enumerate() {
        let answers = relation
            .frontier
            .fields()
            .iter()
            .map(|path| (path.clone(), row[path].clone()))
            .collect();
        let resolution = relation
            .resolve_with_budget(&answers, "request.json", "", false, &mut budget)
            .unwrap();
        assert_eq!(resolution.evidence["selected_row"], index);
        assert!(resolution.origins.values().all(|origin| {
            origin["value_source"]["pointer"]
                .as_str()
                .unwrap()
                .contains(&format!("/choices/rows/{index}/"))
        }));
    }
    assert!(budget.usage()["memory"]["reserved_bytes"].as_u64().unwrap() > 0);
    drop(relation);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
}

#[test]
fn relation_analysis_refuses_aggregate_memory_and_work_without_retained_buffers() {
    let request = request();
    let mut measured = Budget::default();
    let relation = analyze_with_budget(&request, &mut measured).unwrap();
    let peak = usize::try_from(
        measured.usage()["memory"]["peak_reserved_bytes"]
            .as_u64()
            .unwrap(),
    )
    .unwrap();
    let work = measured.usage()["charged_work"].as_u64().unwrap();
    drop(relation);
    for mut budget in [
        Budget::limited_with_memory(Duration::from_secs(2), 100_000, peak - 1),
        Budget::limited(Duration::from_secs(2), work - 1),
    ] {
        let error = analyze_with_budget(&request, &mut budget).err().unwrap();
        assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
        assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    }
}
