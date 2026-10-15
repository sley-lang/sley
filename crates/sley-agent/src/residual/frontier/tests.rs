use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::{Budget, Family, Method, plan};
use crate::error::AgentErrorCode;

fn family(costs: &[u32], rows: &[Vec<u8>]) -> Family {
    let fields: Vec<_> = costs
        .iter()
        .enumerate()
        .map(|(index, cost)| json!({"name":format!("x{index}"),"cost":cost,"eligible":true}))
        .collect();
    let rows: Vec<_> = rows
        .iter()
        .map(|row| {
            Value::Object(
                row.iter()
                    .enumerate()
                    .map(|(index, value)| (format!("x{index}"), json!(value)))
                    .collect(),
            )
        })
        .collect();
    parse(&json!({"fields":fields,"descriptions":rows}))
}

fn parse(value: &Value) -> Family {
    Family::parse(&serde_json::to_vec(value).unwrap()).unwrap()
}

fn counterexample() -> Family {
    // Preserved verbatim values from source/research/frontier-audit.json,
    // audit-124. Its costs are artificial additive units, not tokens/dollars.
    family(
        &[6, 5, 2, 3, 1, 3, 5],
        &[
            vec![0, 0, 0, 0, 1, 0, 1],
            vec![0, 0, 1, 1, 0, 0, 0],
            vec![0, 0, 1, 1, 1, 0, 0],
            vec![0, 0, 1, 1, 1, 0, 1],
            vec![0, 1, 0, 0, 1, 0, 0],
            vec![0, 1, 0, 1, 0, 0, 1],
            vec![0, 1, 1, 1, 1, 1, 0],
            vec![1, 0, 1, 0, 1, 1, 0],
        ],
    )
}

// Independent oracle: enumerate subsets and compare serialized projections.
// It does not use pair bitsets, the greedy procedure, or the production decoder.
pub(super) fn oracle(family: &Family) -> (u64, Vec<String>) {
    let eligible: Vec<_> = family
        .fields
        .iter()
        .filter(|field| field.eligible)
        .collect();
    assert!(eligible.len() <= 8);
    let mut best = None;
    for mask in 0..(1_usize << eligible.len()) {
        let selected: Vec<_> = eligible
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, field)| *field)
            .collect();
        let cost = selected.iter().map(|field| u64::from(field.cost)).sum();
        let names = selected
            .iter()
            .map(|field| field.name.clone())
            .collect::<Vec<_>>();
        let projections: BTreeSet<_> = family
            .descriptions()
            .iter()
            .map(|row| {
                serde_json::to_string(
                    &selected
                        .iter()
                        .map(|field| &row[&field.name])
                        .collect::<Vec<_>>(),
                )
                .unwrap()
            })
            .collect();
        if projections.len() == family.descriptions().len()
            && best
                .as_ref()
                .is_none_or(|best| &(cost, names.clone()) < best)
        {
            best = Some((cost, names));
        }
    }
    best.expect("complete vocabulary")
}

#[test]
fn retained_greedy_counterexample_and_exact_additive_optimum() {
    let family = counterexample();
    let greedy = plan(&family, &mut Budget::default(), false).unwrap();
    assert_eq!(greedy.fields(), ["x1", "x2", "x4", "x5", "x6"]);
    assert_eq!(greedy.cost(), 16);
    assert_eq!(greedy.method(), Method::Greedy);
    let exact = plan(&family, &mut Budget::default(), true).unwrap();
    assert_eq!(exact.fields(), ["x3", "x4", "x5", "x6"]);
    assert_eq!(exact.cost(), 12);
    assert_eq!(exact.method(), Method::ExactAdditive);
    assert_eq!((exact.cost(), exact.fields().to_vec()), oracle(&family));
    for row in family.descriptions() {
        for frontier in [&greedy, &exact] {
            assert_eq!(
                frontier
                    .decode(&family, &frontier.encode(&family, row).unwrap())
                    .unwrap(),
                *row
            );
        }
    }
    assert_eq!(exact.summary()["billed_cost_optimality"], "not_established");
}

#[test]
fn singleton_projection_does_not_establish_semantic_entitlement() {
    let family = family(&[1], &[vec![0]]);
    let frontier = plan(&family, &mut Budget::default(), true).unwrap();
    assert!(frontier.fields().is_empty());
    assert_eq!(frontier.cost(), 0);
    assert_eq!(
        frontier.decode(&family, &Map::new()).unwrap(),
        family.rows[0]
    );
    assert_eq!(
        frontier.summary()["semantic_entitlement"],
        "not_established"
    );
    assert_eq!(
        frontier.summary()["family_completeness_for_task"],
        "not_established"
    );
    assert!(frontier.summary().get("construction").is_none());
}

#[test]
fn deterministic_field_ties_and_row_order_bind_the_same_relation() {
    let value = json!({"fields":[{"name":"z","cost":1,"eligible":true},{"name":"a","cost":1,"eligible":true}],"descriptions":[{"z":0,"a":0},{"z":1,"a":1}]});
    let first = parse(&value);
    let mut permuted = value.clone();
    permuted["fields"].as_array_mut().unwrap().reverse();
    permuted["descriptions"].as_array_mut().unwrap().reverse();
    let second = parse(&permuted);
    for exact in [false, true] {
        let left = plan(&first, &mut Budget::default(), exact).unwrap();
        let right = plan(&second, &mut Budget::default(), exact).unwrap();
        assert_eq!(left.fields(), ["a"]);
        assert_eq!(left.summary(), right.summary());
    }
}

#[test]
fn typed_domains_and_exact_existence_are_required_on_decode() {
    let family = parse(
        &json!({"fields":[{"name":"x","cost":1,"eligible":true}],"descriptions":[{"x":true},{"x":1},{"x":"1"}]}),
    );
    let frontier = plan(&family, &mut Budget::default(), false).unwrap();
    for row in family.descriptions() {
        assert_eq!(frontier.decode(&family, row).unwrap(), *row);
    }
    assert_eq!(
        frontier.decode(&family, &Map::new()).unwrap_err().code(),
        AgentErrorCode::ResidualChoiceMissing
    );
    for answers in [
        json!({"x":false}),
        json!({"x":1.0}),
        json!({"x":true,"extra":0}),
        json!({"extra":0}),
    ] {
        assert_eq!(
            frontier
                .decode(&family, answers.as_object().unwrap())
                .unwrap_err()
                .code(),
            AgentErrorCode::ResidualChoiceUnknown
        );
    }
    assert_eq!(
        frontier
            .encode(&family, json!({"x":2}).as_object().unwrap())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualChoiceUnknown
    );
}

#[test]
fn costs_and_vocabulary_and_rows_are_part_of_the_binding() {
    let value =
        json!({"fields":[{"name":"x","cost":1,"eligible":true}],"descriptions":[{"x":0},{"x":1}]});
    let family = parse(&value);
    let frontier = plan(&family, &mut Budget::default(), false).unwrap();
    for (pointer, replacement) in [
        ("/fields/0/cost", json!(2)),
        ("/fields/0/eligible", json!(false)),
        ("/descriptions/0/x", json!(2)),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        assert_eq!(
            frontier
                .decode(&parse(&changed), json!({"x":1}).as_object().unwrap())
                .unwrap_err()
                .code(),
            AgentErrorCode::ResidualBindingStale
        );
    }
}

#[test]
fn ineligible_fields_are_never_questions_and_inadequate_vocabulary_refuses() {
    let value = json!({"fields":[{"name":"x","cost":1,"eligible":false},{"name":"y","cost":2,"eligible":true}],"descriptions":[{"x":0,"y":0},{"x":1,"y":1}]});
    let family = parse(&value);
    let frontier = plan(&family, &mut Budget::default(), true).unwrap();
    assert_eq!(frontier.fields(), ["y"]);
    let mut inadequate = value;
    inadequate["descriptions"][1]["y"] = json!(0);
    assert_eq!(
        plan(&parse(&inadequate), &mut Budget::default(), true)
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualVocabularyIncomplete
    );
}

#[test]
fn exhausted_refinement_retains_feasibility_without_claiming_optimality() {
    let family = counterexample();
    // Measure the deterministic charged work of the greedy path; then leave
    // exactly one more unit so exact enumeration must terminate early.
    let mut baseline = Budget::default();
    let greedy = plan(&family, &mut baseline, false).unwrap();
    let used = super::MAX_WORK - baseline.remaining;
    let mut budget = Budget::limited(Duration::from_secs(2), used + 1);
    let partial = plan(&family, &mut budget, true).unwrap();
    assert_eq!(partial.method(), Method::BoundedBest);
    assert_eq!(partial.fields(), greedy.fields());
    for row in family.descriptions() {
        assert_eq!(
            partial
                .decode(&family, &partial.encode(&family, row).unwrap())
                .unwrap(),
            *row
        );
    }
    assert_eq!(
        plan(&family, &mut budget, false).unwrap_err().code(),
        AgentErrorCode::ResidualLimit,
        "the same aggregate budget cannot restart"
    );
    assert_eq!(
        plan(
            &family,
            &mut Budget::limited(Duration::ZERO, super::MAX_WORK),
            false
        )
        .unwrap_err()
        .code(),
        AgentErrorCode::ResidualLimit
    );
}

#[test]
fn strict_family_schema_limits_and_empty_or_duplicate_descriptions_refuse() {
    let value = json!({"fields":[{"name":"x","cost":1,"eligible":true}],"descriptions":[{"x":0}]});
    for cost in [
        json!(0),
        json!(-1),
        json!(true),
        json!("1"),
        json!(1.5),
        json!(u64::MAX),
    ] {
        let mut invalid = value.clone();
        invalid["fields"][0]["cost"] = cost;
        assert!(Family::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    for rows in [
        json!([]),
        json!([{}]),
        json!([{"x":0,"unknown":1}]),
        json!([{"x":0},{"x":0}]),
        json!(vec![json!({"x":0}); 257]),
    ] {
        let mut invalid = value.clone();
        invalid["descriptions"] = rows;
        assert!(Family::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    let mut invalid = value.clone();
    invalid["fields"][0]["extra"] = json!(false);
    assert!(Family::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
    let duplicate =
        br#"{"fields":[{"name":"x","cost":1,"eligible":true}],"descriptions":[{"x":0,"x":1}]}"#;
    assert_eq!(
        Family::parse(duplicate).unwrap_err().code(),
        AgentErrorCode::ResidualParse
    );
}

#[test]
fn maximum_component_and_u32_costs_do_not_overflow_or_sample_rows() {
    let rows: Vec<_> = (0_u16..256)
        .map(|value| {
            (0..64)
                .map(|bit| u8::from(bit < 8 && value & (1 << bit) != 0))
                .collect()
        })
        .collect();
    let family = family(&[u32::MAX; 64], &rows);
    let frontier = plan(&family, &mut Budget::default(), true).unwrap();
    assert_eq!(frontier.fields().len(), 8);
    assert_eq!(frontier.cost(), 8 * u64::from(u32::MAX));
    assert_eq!(
        frontier.method(),
        Method::Greedy,
        "64 fields exceed the exhaustive refinement ceiling"
    );
    for row in family.descriptions() {
        assert_eq!(
            frontier
                .decode(&family, &frontier.encode(&family, row).unwrap())
                .unwrap(),
            *row
        );
    }
}

#[test]
fn field_limit_is_aggregate_and_cannot_be_raised_by_callers() {
    let family = family(&[1; 33], &[vec![0; 33], vec![1; 33]]);
    let mut budget = Budget::limited(Duration::from_secs(200), u64::MAX);
    assert_eq!(budget.wall, Duration::from_secs(2));
    assert_eq!(budget.remaining, super::MAX_WORK);
    plan(&family, &mut budget, false).unwrap();
    assert_eq!(
        plan(&family, &mut budget, false).unwrap_err().code(),
        AgentErrorCode::ResidualLimit
    );
}

#[test]
fn five_hundred_seeded_families_match_an_independent_exact_oracle() {
    // Separate deterministic generator from the retained Python audit. This is
    // a new mathematical check, not a claim to reproduce its 3,538 samples.
    let mut state = 20_526_928_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut roundtrips = 0;
    for _ in 0..500 {
        let width = 4 + (next() % 5) as usize;
        let count = 4 + (next() % 7) as usize;
        let mut rows = BTreeSet::new();
        while rows.len() < count {
            rows.insert(
                (0..width)
                    .map(|_| u8::from(next() % 2 == 1))
                    .collect::<Vec<_>>(),
            );
        }
        let costs: Vec<_> = (0..width)
            .map(|_| u32::try_from(1 + next() % 9).unwrap())
            .collect();
        let family = family(&costs, &rows.into_iter().collect::<Vec<_>>());
        let expected = oracle(&family);
        let greedy = plan(&family, &mut Budget::default(), false).unwrap();
        let exact = plan(&family, &mut Budget::default(), true).unwrap();
        assert_eq!(exact.method(), Method::ExactAdditive);
        assert_eq!((exact.cost(), exact.fields().to_vec()), expected);
        assert!(greedy.cost() >= exact.cost());
        for omitted in greedy.fields() {
            let projections: BTreeSet<_> = family
                .descriptions()
                .iter()
                .map(|row| {
                    serde_json::to_string(
                        &greedy
                            .fields()
                            .iter()
                            .filter(|field| *field != omitted)
                            .map(|field| &row[field])
                            .collect::<Vec<_>>(),
                    )
                    .unwrap()
                })
                .collect();
            assert!(
                projections.len() < family.descriptions().len(),
                "redundant greedy field {omitted}"
            );
        }
        for row in family.descriptions() {
            for frontier in [&greedy, &exact] {
                assert_eq!(
                    frontier
                        .decode(&family, &frontier.encode(&family, row).unwrap())
                        .unwrap(),
                    *row
                );
            }
            roundtrips += 1;
        }
    }
    assert!(roundtrips >= 2_000);
}

#[test]
fn budget_cannot_move_between_worker_threads() {
    let budget = Budget::default();
    let error = std::thread::spawn(move || {
        let mut budget = budget;
        budget.checkpoint().unwrap_err()
    })
    .join()
    .unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("another worker thread"));
}

#[cfg(target_os = "linux")]
#[test]
fn cpu_exhaustion_and_invalid_observations_refuse_at_stage_boundaries() {
    let mut budget = Budget::default();
    assert!(
        budget.cpu_start.is_some(),
        "Linux test requires the existing procfs CPU source"
    );
    budget.cpu_limit = Duration::ZERO;
    let error = budget.checkpoint().unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("CPU budget"));

    let mut budget = Budget {
        cpu_start: Some(u64::MAX),
        ..Budget::default()
    };
    let error = budget.checkpoint().unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("observation became unavailable"));
}

#[test]
fn budget_usage_counts_work_from_the_actual_tightened_ceiling() {
    let mut budget = Budget::limited(Duration::from_secs(1), 10);
    budget.checkpoint().unwrap();
    let usage = budget.usage();
    assert_eq!(usage["charged_work"], 1);
    assert_eq!(usage["work_limit"], 10);
    assert_eq!(usage["wall_limit_micros"], 1_000_000);
    assert_eq!(
        usage["memory_enforcement"],
        "frontier_scratch_and_retained_payload_reservations"
    );
    assert_eq!(usage["aggregate_memory_enforcement"], "not_implemented");
    // Missing platform CPU measurements use the synchronous wall upper bound,
    // and must never be rendered as zero measured CPU.
    budget.cpu_start = None;
    let usage = budget.usage();
    assert!(usage["cpu_micros"].is_null());
    assert_eq!(usage["cpu_accounting"], "single_thread_wall_upper_bound");
}

#[test]
fn fast_path_tightens_the_original_clock_and_preserves_consumed_work() {
    let mut budget = Budget::default();
    let start = budget.start;
    budget.charge(17).unwrap();
    budget.fields_used = 3;
    budget.use_fast_path().unwrap();
    assert_eq!(budget.start, start);
    assert_eq!(budget.wall, Duration::from_millis(250));
    assert_eq!(budget.usage()["charged_work"], 18);
    assert_eq!(budget.usage()["planned_fields"], 3);
    assert_eq!(budget.usage()["planning_route"], "explicit_fast_path");
    budget.use_fast_path().unwrap();
    assert_eq!(budget.start, start);
    assert_eq!(budget.usage()["charged_work"], 19);

    let mut tighter = Budget::limited(Duration::from_millis(100), 10);
    tighter.use_fast_path().unwrap();
    assert_eq!(tighter.wall, Duration::from_millis(100));
}

#[test]
fn selecting_fast_path_cannot_hide_time_already_spent() {
    let mut budget = Budget {
        start: Instant::now()
            .checked_sub(Duration::from_millis(251))
            .unwrap(),
        ..Budget::default()
    };
    let start = budget.start;
    let failure = budget.use_fast_path().unwrap_err();
    assert_eq!(failure.code(), AgentErrorCode::ResidualLimit);
    assert!(failure.detail().contains("fast-path"));
    assert!(failure.detail().contains("AF1-X"));
    assert_eq!(budget.start, start);
    assert_eq!(
        budget.checkpoint().unwrap_err().code(),
        AgentErrorCode::ResidualLimit
    );
}
