use super::{Budget, Coverage, Family, Method, memory, plan};
use crate::error::AgentErrorCode;
use serde_json::json;
use std::time::Duration;

fn family() -> Family {
    Family::parse(
        &serde_json::to_vec(&json!({
            "fields":[{"name":"a","cost":1,"eligible":true},{"name":"b","cost":2,"eligible":true}],
            "descriptions":[{"a":0,"b":0},{"a":1,"b":1}]
        }))
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn reservations_share_capacity_release_on_drop_and_retain_the_peak() {
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 1000, 64);
    let first = budget.reserve::<u64>(4).unwrap();
    let second = budget.reserve::<u8>(32).unwrap();
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 64);
    drop(first);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 32);
    let third = budget.reserve::<u64>(4).unwrap();
    drop(second);
    drop(third);
    budget.checkpoint().unwrap();
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 64);
    assert_eq!(budget.usage()["memory"]["exhausted"], false);
}

#[test]
fn denial_and_size_overflow_are_sticky_without_charging_unallocated_bytes() {
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 1000, 8);
    let held = budget.reserve::<u64>(1).unwrap();
    let error = budget.reserve::<u8>(1).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("planner memory"));
    drop(held);
    for _ in 0..2 {
        assert_eq!(
            budget.checkpoint().unwrap_err().code(),
            AgentErrorCode::ResidualLimit
        );
        assert_eq!(
            budget.reserve::<u8>(0).unwrap_err().code(),
            AgentErrorCode::ResidualLimit
        );
    }
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 8);
    let mut overflow = Budget::default();
    assert_eq!(
        overflow.reserve::<u64>(usize::MAX).unwrap_err().code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(overflow.usage()["memory"]["peak_reserved_bytes"], 0);
    assert_eq!(overflow.usage()["memory"]["exhausted"], true);
}

#[test]
fn memory_ceiling_cannot_be_raised_or_reset_by_fast_path_selection() {
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 1000, usize::MAX);
    assert_eq!(budget.usage()["memory"]["limit_bytes"], memory::MAX_BYTES);
    let held = budget.reserve::<u8>(17).unwrap();
    budget.use_fast_path().unwrap();
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 17);
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 17);
    assert_eq!(
        budget.reserve::<u8>(memory::MAX_BYTES).unwrap_err().code(),
        AgentErrorCode::ResidualLimit
    );
    drop(held);
    assert!(budget.use_fast_path().is_err());
}

#[test]
fn coverage_reserves_before_allocation_and_releases_after_vocabulary_refusal() {
    let family = family();
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 1000, 1);
    let error = plan(&family, &mut budget, true).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("planner memory"));
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 0);

    let mut invalid = family;
    for field in std::sync::Arc::get_mut(&mut invalid.fields).unwrap() {
        field.eligible = false;
    }
    let mut budget = Budget::default();
    let error = plan(&invalid, &mut budget, true).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualVocabularyIncomplete);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(
        budget.usage()["memory"]["peak_reserved_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    budget.checkpoint().unwrap();
}

#[test]
fn simultaneously_live_component_coverage_cannot_reset_the_memory_budget() {
    let family = family();
    let mut measured = Budget::default();
    let coverage = Coverage::new(&family, &mut measured).unwrap();
    let peak = measured.usage()["memory"]["peak_reserved_bytes"]
        .as_u64()
        .unwrap();
    let live = measured.usage()["memory"]["reserved_bytes"]
        .as_u64()
        .unwrap();
    assert!(peak > live && live > 0);
    drop(coverage);
    let mut budget =
        Budget::limited_with_memory(Duration::from_secs(2), 1000, usize::try_from(peak).unwrap());
    let first = Coverage::new(&family, &mut budget).unwrap();
    let error = Coverage::new(&family, &mut budget).err().unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], live);
    drop(first);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(budget.checkpoint().is_err());
}

#[test]
fn accounted_plan_scratch_releases_on_success_and_matches_requested_capacities() {
    let family = family();
    let mut budget = Budget::default();
    let frontier = plan(&family, &mut budget, true).unwrap();
    assert_eq!(frontier.method(), Method::ExactAdditive);
    assert_eq!(frontier.fields(), ["a"]);
    // Two columns and one full mask (one word each), plus two outer
    // column Vec headers. Construction's borrowed-value index has two
    // rows of two references and two outer Vec headers, simultaneously live.
    let output_bound = 2 * size_of::<String>() + family.digest.len() + 2;
    let expected = output_bound
        + 2 * std::mem::size_of::<Vec<u64>>()
        + 3 * std::mem::size_of::<u64>()
        + 2 * (std::mem::size_of::<Vec<&serde_json::Value>>()
            + 2 * std::mem::size_of::<&serde_json::Value>());
    let usage = budget.usage();
    assert_eq!(usage["memory"]["peak_reserved_bytes"], expected);
    assert_eq!(usage["memory"]["reserved_bytes"], output_bound - 1);
    drop(frontier);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert_eq!(usage["aggregate_memory_enforcement"], "not_implemented");
    assert!(
        usage["memory"]["excludes"]
            .as_str()
            .unwrap()
            .contains("graph/compiler")
    );
}

#[test]
fn reservations_can_be_released_elsewhere_but_planning_cannot_move_workers() {
    let mut budget = Budget::default();
    let held = budget.reserve::<u8>(8).unwrap();
    std::thread::spawn(move || drop(held)).join().unwrap();
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    let error = std::thread::spawn(move || budget.reserve::<u8>(1).unwrap_err())
        .join()
        .unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("another worker thread"));
}

#[test]
fn unused_output_allowance_releases_capacity_without_resetting_exhaustion() {
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 1000, 64);
    let mut reservation = budget.reserve::<u8>(64).unwrap();
    reservation.shrink(16);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 16);
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 64);
    let rest = budget.reserve::<u8>(48).unwrap();
    assert!(budget.reserve::<u8>(1).is_err());
    reservation.shrink(0);
    drop(rest);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(budget.checkpoint().is_err());
}

#[test]
fn retained_frontier_clones_share_exact_capacity_until_the_last_owner_drops() {
    let family = family();
    let mut budget = Budget::default();
    let frontier = plan(&family, &mut budget, true).unwrap();
    let expected = frontier.fields.capacity() * size_of::<String>()
        + frontier.fields.iter().map(String::capacity).sum::<usize>()
        + frontier.family.capacity();
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], expected);
    let cloned = frontier.clone();
    assert!(std::sync::Arc::ptr_eq(&frontier.fields, &cloned.fields));
    assert!(std::sync::Arc::ptr_eq(&frontier.family, &cloned.family));
    drop(frontier);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], expected);
    for row in family.descriptions() {
        assert_eq!(
            cloned
                .decode(&family, &cloned.encode(&family, row).unwrap())
                .unwrap(),
            *row
        );
    }
    std::thread::spawn(move || drop(cloned)).join().unwrap();
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
}

#[test]
fn simultaneous_frontier_outputs_share_the_invocation_memory_ceiling() {
    let family = family();
    let mut measured = Budget::default();
    let frontier = plan(&family, &mut measured, true).unwrap();
    let ceiling = usize::try_from(
        measured.usage()["memory"]["peak_reserved_bytes"]
            .as_u64()
            .unwrap(),
    )
    .unwrap();
    drop(frontier);
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 100_000, ceiling);
    let first = plan(&family, &mut budget, true).unwrap();
    let live = budget.usage()["memory"]["reserved_bytes"].clone();
    let error = plan(&family, &mut budget, true).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("planner memory"));
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], live);
    drop(first);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(budget.checkpoint().is_err());
}
