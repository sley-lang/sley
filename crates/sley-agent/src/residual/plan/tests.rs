use std::{cell::Cell, time::Duration};

use serde_json::{Map, Value, json};

use super::{explicit_decisions_with_budget, fill, fill_with_budget};
use crate::{
    AgentErrorCode,
    residual::{self, choices, frontier::Budget},
};

fn input() -> Value {
    let mut request: Value = serde_json::from_str(
        crate::help::RESIDUAL
            .lines()
            .find(|line| line.starts_with("{\"residual\":"))
            .unwrap(),
    )
    .unwrap();
    request["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("rounding");
    request
}

fn answers() -> Map<String, Value> {
    json!({"/bindings/rounding":"toward_zero"})
        .as_object()
        .unwrap()
        .clone()
}

fn work(budget: &Budget) -> u64 {
    budget.usage()["charged_work"].as_u64().unwrap()
}

#[test]
fn repeated_inventory_consumes_one_budget_without_resetting_it() {
    let request = residual::request_from_value(input()).unwrap();
    let mut measured = Budget::default();
    let expected = explicit_decisions_with_budget(&request, &mut measured).unwrap();
    let cost = work(&measured);
    assert!(cost > 3, "inventory fields must themselves be charged");
    let mut budget = Budget::limited(Duration::from_secs(2), 2 * cost - 1);
    assert_eq!(
        explicit_decisions_with_budget(&request, &mut budget).unwrap(),
        expected
    );
    let error = explicit_decisions_with_budget(&request, &mut budget).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert_eq!(work(&budget), 2 * cost - 1);
}

#[test]
fn fill_preserves_values_and_original_while_charging_serialization() {
    let original = input();
    let unchanged = original.clone();
    let mut budget = Budget::default();
    let filled = fill_with_budget(&original, &answers(), &mut budget).unwrap();
    assert_eq!(filled, fill(&original, &answers()).unwrap());
    assert_eq!(filled["bindings"]["rounding"], "toward_zero");
    assert_eq!(original, unchanged);
    assert!(work(&budget) > 30);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert_eq!(
        budget.usage()["memory"]["peak_reserved_bytes"],
        serde_json::to_vec(&filled).unwrap().len()
    );
    let cost = work(&budget);
    let mut shared = Budget::limited(Duration::from_secs(2), 2 * cost - 1);
    fill_with_budget(&original, &answers(), &mut shared).unwrap();
    assert_eq!(
        fill_with_budget(&original, &answers(), &mut shared)
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(original, unchanged);
    assert_eq!(shared.usage()["memory"]["reserved_bytes"], 0);
}

#[test]
fn fill_encoding_refuses_shared_memory_exhaustion_and_releases_reservations() {
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), u64::MAX, 1);
    let error = fill_with_budget(&input(), &answers(), &mut budget).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("planner memory"));
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(budget.checkpoint().is_err(), "exhaustion stays sticky");
}

#[test]
fn fill_refuses_expired_clock_and_work_before_reconstruction() {
    for mut budget in [
        Budget::limited(Duration::ZERO, u64::MAX),
        Budget::limited(Duration::from_secs(2), 0),
    ] {
        assert_eq!(
            fill_with_budget(&input(), &answers(), &mut budget)
                .unwrap_err()
                .code(),
            AgentErrorCode::ResidualLimit
        );
        assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 0);
    }
}

#[test]
fn choices_resolution_keeps_the_analysis_budget_and_exact_answer() {
    let mut original = input();
    original["choices"] =
        json!({"version":1,"contract":"author_closed_relation","rows":[answers()]});
    let request = residual::request_from_value(original.clone()).unwrap();
    let mut measured = Budget::default();
    let relation = choices::analyze_with_budget(&request, &mut measured).unwrap();
    let analysis_work = work(&measured);
    let resolution = relation
        .resolve_with_budget(&Map::new(), "request.json", "", false, &mut measured)
        .unwrap();
    assert!(work(&measured) > analysis_work + 1);
    assert_eq!(resolution.request["bindings"]["rounding"], "toward_zero");
    assert!(resolution.request.get("choices").is_none());
    let mut tight = Budget::limited(Duration::from_secs(2), analysis_work + 1);
    let relation = choices::analyze_with_budget(&request, &mut tight).unwrap();
    assert_eq!(
        relation
            .resolve_with_budget(&Map::new(), "request.json", "", false, &mut tight)
            .err()
            .unwrap()
            .code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(fill(&original, &Map::new()).unwrap(), resolution.request);
}

#[test]
fn completion_fill_exhaustion_is_attributed_and_never_reaches_compiler_callback() {
    let mut original = input();
    original["choices"] =
        json!({"version":1,"contract":"author_closed_relation","rows":[answers()]});
    let request = residual::request_from_value(original).unwrap();
    let relation = choices::analyze(&request).unwrap();
    let called = Cell::new(0);
    let mut budget = Budget::limited(Duration::from_secs(2), 2);
    let error = relation
        .check_completions(&mut budget, |_, _| {
            called.set(called.get() + 1);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(called.get(), 0);
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("/choices/rows/0"));
    let mut measured = Budget::default();
    relation
        .check_completions(&mut measured, |_, shared| {
            assert!(work(shared) > 2);
            called.set(called.get() + 1);
            shared.checkpoint()
        })
        .unwrap();
    assert_eq!(called.get(), 1);
}

#[test]
fn oversized_json_refuses_without_allocating_an_encoding_buffer() {
    let mut original = input();
    original["bindings"]["result"] = json!("x".repeat(residual::MAX_REQUEST_BYTES));
    let mut budget = Budget::default();
    let error = fill_with_budget(&original, &answers(), &mut budget).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("byte ceiling"));
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 0);
}
