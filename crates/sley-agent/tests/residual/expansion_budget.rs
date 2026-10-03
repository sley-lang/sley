use std::time::Duration;

use super::{bytes, fixture_names, literal_fixture, literal_request, per_target_request};
use serde_json::json;
use sley_agent::{
    AgentErrorCode,
    residual::{edit, frontier::Budget, parse_request},
};

#[test]
fn literal_expansion_charges_consumer_and_caller_scans_without_changing_output() {
    let fixture = literal_fixture();
    let head = fixture.workspace.read_head().unwrap();
    let names = fixture_names(&fixture, head.program());
    let request = parse_request(&bytes(&literal_request(7))).unwrap();
    let expected = edit::expand(head.program(), &names, &request).unwrap();
    let mut measured = Budget::default();
    let actual = edit::expand_with_budget(head.program(), &names, &request, &mut measured).unwrap();
    assert_eq!(actual.expansion.frame, expected.expansion.frame);
    assert_eq!(actual.expansion.provenance, expected.expansion.provenance);
    assert_eq!(actual.contract.report(), expected.contract.report());
    let report = actual.contract.report();
    let callers = report["boundaries"]["possible_static_callers"]
        .as_array()
        .unwrap();
    assert_eq!(callers.len(), 2);
    assert!(callers.contains(&json!("caller")) && callers.contains(&json!("outer")));
    let work = measured.usage()["charged_work"].as_u64().unwrap();
    assert!(work > 2 * head.program().objects().len() as u64);
    let before = request.bindings.clone();
    for allowance in [0, 2, (head.program().objects().len() / 2) as u64, work - 1] {
        let mut budget = Budget::limited(Duration::from_secs(2), allowance);
        let error = edit::expand_with_budget(head.program(), &names, &request, &mut budget)
            .err()
            .unwrap();
        assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
        assert_eq!(request.bindings, before);
        assert_eq!(
            fixture.workspace.read_head().unwrap().transaction_id(),
            head.transaction_id()
        );
    }
}

#[test]
fn repeated_literal_expansions_and_multiple_targets_do_not_reset_the_budget() {
    let fixture = literal_fixture();
    let head = fixture.workspace.read_head().unwrap();
    let names = fixture_names(&fixture, head.program());
    let one = parse_request(&bytes(&literal_request(7))).unwrap();
    let two = parse_request(&bytes(&per_target_request(
        json!({"adjust.entry.amount":7,"other.entry.amount":9}),
    )))
    .unwrap();
    let mut first = Budget::default();
    edit::expand_with_budget(head.program(), &names, &one, &mut first).unwrap();
    let first_work = first.usage()["charged_work"].as_u64().unwrap();
    let mut second = Budget::default();
    edit::expand_with_budget(head.program(), &names, &two, &mut second).unwrap();
    let second_work = second.usage()["charged_work"].as_u64().unwrap();
    assert!(second_work >= first_work + head.program().objects().len() as u64);
    let mut shared = Budget::limited(Duration::from_secs(2), first_work + second_work - 1);
    edit::expand_with_budget(head.program(), &names, &one, &mut shared).unwrap();
    assert_eq!(
        edit::expand_with_budget(head.program(), &names, &two, &mut shared)
            .err()
            .unwrap()
            .code(),
        AgentErrorCode::ResidualLimit
    );
}
