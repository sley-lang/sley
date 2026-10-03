use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::{Map, Value, json};

use super::{Problem, plan};
use crate::error::AgentErrorCode;
use crate::residual::frontier::{Budget, Family};

fn binary(count: usize) -> Value {
    let names: Vec<_> = (0..count).map(|index| format!("x{index:02}")).collect();
    json!({"fields":names.iter().map(|name| json!({"name":name,"cost":1,"eligible":true})).collect::<Vec<_>>(),
        "constraints":names.iter().map(|name| json!({"fields":[name],"rows":[{name:0},{name:1}]})).collect::<Vec<_>>(),
        "dependencies":[]})
}
fn parse(value: &Value) -> Problem {
    Problem::parse(&serde_json::to_vec(value).unwrap()).unwrap()
}

fn legacy_digest(problem: &Problem) -> String {
    let value = json!({
        "fields":problem.fields.iter().map(super::field_value).collect::<Vec<_>>(),
        "constraints":problem.constraints.iter().map(|family| &family.digest).collect::<Vec<_>>(),
        "dependencies":problem.dependencies.iter().map(|edge| json!({
            "kind":edge.kind,"fields":edge.fields.iter().map(|index| &problem.fields[*index].name).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    });
    crate::residual::binding::canonical_digest("factored-constraint-graph-v1", &value).unwrap()
}

#[test]
fn streamed_problem_digest_matches_legacy_encoding_for_every_dependency_kind() {
    for count in [2, 8, 64] {
        for kind in super::DEPENDENCY_KINDS {
            let mut value = binary(count);
            value["fields"][0]["cost"] = json!(u32::MAX);
            value["fields"][1]["eligible"] = json!(false);
            value["constraints"][0]["rows"] =
                json!([{"x00":[null,true,"\0\n é 💡",i64::MIN,u64::MAX]}]);
            value["dependencies"] =
                json!([{"kind":kind,"fields":["x00",format!("x{:02}",count-1)]}]);
            let problem = parse(&value);
            assert_eq!(problem.digest, legacy_digest(&problem), "{count} {kind}");
            value["fields"].as_array_mut().unwrap().reverse();
            value["constraints"].as_array_mut().unwrap().reverse();
            let reordered = parse(&value);
            assert_eq!(problem.digest, reordered.digest);
            assert_eq!(reordered.digest, legacy_digest(&reordered));
        }
    }
}

#[test]
fn streamed_problem_identity_binds_cost_eligibility_rows_and_dependency_kinds() {
    let original = binary(2);
    let baseline = parse(&original).digest;
    let mut digests = BTreeSet::new();
    for change in 0..5 {
        let mut value = original.clone();
        match change {
            0 => value["fields"][0]["cost"] = json!(2),
            1 => value["fields"][0]["eligible"] = json!(false),
            2 => value["constraints"][0]["rows"][0]["x00"] = json!(false),
            3 => value["dependencies"] = json!([{"kind":"effect","fields":["x00","x01"]}]),
            _ => value["dependencies"] = json!([{"kind":"order","fields":["x00","x01"]}]),
        }
        let problem = parse(&value);
        assert_eq!(problem.digest, legacy_digest(&problem));
        assert_ne!(problem.digest, baseline);
        assert!(digests.insert(problem.digest));
    }
}

#[test]
fn problem_identity_stream_charges_both_passes_without_canonical_tree_storage() {
    let problem = parse(&binary(8));
    let digest = |budget: &mut Budget| {
        super::digest::problem(
            &problem.fields,
            &problem.constraints,
            &problem.dependencies,
            budget,
        )
    };
    let mut measured = Budget::default();
    assert_eq!(digest(&mut measured).unwrap(), legacy_digest(&problem));
    let work = measured.usage()["charged_work"].as_u64().unwrap();
    assert!(work > 100);
    // This only establishes that this stream needs no reserved scratch buffer;
    // the returned digest string and allocator overhead remain unaccounted.
    assert_eq!(measured.usage()["memory"]["peak_reserved_bytes"], 0);
    let mut exact = Budget::limited_with_memory(Duration::from_secs(2), work, 0);
    assert_eq!(digest(&mut exact).unwrap(), problem.digest);
    assert_eq!(
        digest(&mut exact).unwrap_err().code(),
        AgentErrorCode::ResidualLimit
    );
    for allowance in [1, work / 2, work - 1] {
        let mut budget = Budget::limited(Duration::from_secs(2), allowance);
        assert_eq!(
            digest(&mut budget).unwrap_err().code(),
            AgentErrorCode::ResidualLimit
        );
        assert!(digest(&mut budget).is_err());
        assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    }
}

#[test]
fn constraint_family_parsing_cannot_restart_the_callers_work_budget() {
    let bytes = serde_json::to_vec(&binary(2)).unwrap();
    let mut measured = Budget::default();
    let problem = Problem::parse_with_budget(&bytes, &mut measured).unwrap();
    assert_eq!(problem.digest, parse(&binary(2)).digest);
    let work = measured.usage()["charged_work"].as_u64().unwrap();
    assert!(work > 100);
    let mut budget = Budget::limited(Duration::from_secs(2), work);
    let first = Problem::parse_with_budget(&bytes, &mut budget).unwrap();
    assert_eq!(first.digest, problem.digest);
    let retained = budget.usage()["memory"]["reserved_bytes"].clone();
    assert!(retained.as_u64().unwrap() > 0);
    let error = Problem::parse_with_budget(&bytes, &mut budget).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], retained);
    drop(first);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    let mut budget = Budget::limited(Duration::from_secs(2), work);
    let error = Problem::parse_with_budget(&serde_json::to_vec(&binary(4)).unwrap(), &mut budget)
        .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
}

#[test]
fn factored_frontier_retains_component_capacity_until_its_last_clone_drops() {
    let bytes = serde_json::to_vec(&binary(4)).unwrap();
    let mut budget = Budget::default();
    let problem = Problem::parse_with_budget(&bytes, &mut budget).unwrap();
    let source_bytes = budget.usage()["memory"]["reserved_bytes"].as_u64().unwrap();
    assert!(source_bytes > 0);
    let frontier = plan(&problem, &mut budget, true).unwrap();
    let combined = budget.usage()["memory"]["reserved_bytes"].as_u64().unwrap();
    assert!(combined > source_bytes);
    let clone = frontier.clone();
    assert!(std::sync::Arc::ptr_eq(
        &frontier.components,
        &clone.components
    ));
    assert!(std::sync::Arc::ptr_eq(&frontier.fields, &clone.fields));
    assert!(std::sync::Arc::ptr_eq(&frontier.problem, &clone.problem));
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], combined);
    drop((problem, frontier));
    assert_eq!(
        budget.usage()["memory"]["reserved_bytes"],
        combined - source_bytes
    );
    assert_eq!(clone.summary()["description_count_decimal"], "16");
    drop(clone);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    budget.checkpoint().unwrap();
}

#[test]
fn factored_output_storage_is_reserved_before_any_component_is_planned() {
    let problem = parse(&binary(4));
    let bound =
        4 * size_of::<super::Component>() + 4 * size_of::<String>() + problem.digest.len() + 12;
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 100_000, bound - 1);
    let error = plan(&problem, &mut budget, true).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("planner memory"));
    assert_eq!(budget.usage()["planned_fields"], 0);
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 0);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
}

#[test]
fn simultaneous_factored_outputs_cannot_reset_retained_capacity() {
    let problem = parse(&binary(4));
    let mut measured = Budget::default();
    let frontier = plan(&problem, &mut measured, true).unwrap();
    let ceiling = usize::try_from(
        measured.usage()["memory"]["peak_reserved_bytes"]
            .as_u64()
            .unwrap(),
    )
    .unwrap();
    drop(frontier);
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 100_000, ceiling);
    let first = plan(&problem, &mut budget, true).unwrap();
    let live = budget.usage()["memory"]["reserved_bytes"].clone();
    let error = plan(&problem, &mut budget, true).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("planner memory"));
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], live);
    drop(first);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(budget.checkpoint().is_err());
}

#[test]
fn twenty_independent_binary_fields_never_materialize_the_million_row_product() {
    let value = binary(20);
    let problem = parse(&value);
    let mut budget = Budget::default();
    let frontier = plan(&problem, &mut budget, true).unwrap();
    let summary = frontier.summary();
    assert_eq!(summary["description_count_decimal"], "1048576");
    assert_eq!(summary["materialized_component_rows"], 40);
    assert_eq!(summary["components"].as_array().unwrap().len(), 20);
    assert_eq!(summary["global_product_materialized"], false);
    assert_eq!(
        summary["program_dependency_completeness"],
        "not_established"
    );
    assert_eq!(budget.usage()["planned_fields"], 20);
    for seed in 0..32 {
        let row: Map<_, _> = (0..20)
            .map(|index| (format!("x{index:02}"), json!((seed + index) % 2)))
            .collect();
        assert_eq!(
            frontier
                .decode(&problem, &frontier.encode(&problem, &row).unwrap())
                .unwrap(),
            row
        );
    }
}

#[test]
fn every_declared_dependency_kind_joins_components_even_when_values_are_independent() {
    for kind in super::DEPENDENCY_KINDS {
        let mut value = binary(3);
        value["dependencies"] = json!([{"kind":kind,"fields":["x00","x01"]}]);
        let problem = parse(&value);
        let summary = plan(&problem, &mut Budget::default(), true)
            .unwrap()
            .summary();
        let components = summary["components"].as_array().unwrap();
        assert_eq!(components.len(), 2, "{kind}");
        assert_eq!(components[0]["fields"], json!(["x00", "x01"]));
        assert_eq!(components[0]["rows"], 4);
        assert_eq!(components[1]["rows"], 2);
    }
}

#[test]
fn constraint_scopes_are_never_split_by_observed_row_patterns() {
    let mut value = binary(2);
    value["constraints"] = json!([{"fields":["x00","x01"],"rows":[
        {"x00":0,"x01":0},{"x00":0,"x01":1},{"x00":1,"x01":0},{"x00":1,"x01":1}]}]);
    let summary = plan(&parse(&value), &mut Budget::default(), true)
        .unwrap()
        .summary();
    assert_eq!(summary["components"].as_array().unwrap().len(), 1);
    assert_eq!(summary["materialized_component_rows"], 4);
    value["constraints"][0]["rows"] = json!([{"x00":0,"x01":0},{"x00":1,"x01":1}]);
    let problem = parse(&value);
    let frontier = plan(&problem, &mut Budget::default(), true).unwrap();
    assert_eq!(frontier.fields().len(), 1);
    assert!(
        frontier
            .encode(&problem, json!({"x00":0,"x01":1}).as_object().unwrap())
            .is_err()
    );
    let encoded = frontier
        .encode(&problem, json!({"x00":1,"x01":1}).as_object().unwrap())
        .unwrap();
    assert_eq!(
        frontier.decode(&problem, &encoded).unwrap(),
        json!({"x00":1,"x01":1}).as_object().unwrap().clone()
    );
}

#[test]
fn shared_variables_and_transitive_dependencies_merge_before_joining() {
    let mut value = binary(3);
    value["constraints"]
        .as_array_mut()
        .unwrap()
        .push(json!({"fields":["x00","x01"],"rows":[{"x00":0,"x01":0},{"x00":1,"x01":1}]}));
    value["dependencies"] = json!([{"kind":"error_route","fields":["x01","x02"]}]);
    let problem = parse(&value);
    let summary = plan(&problem, &mut Budget::default(), true)
        .unwrap()
        .summary();
    assert_eq!(summary["components"].as_array().unwrap().len(), 1);
    assert_eq!(summary["materialized_component_rows"], 4);
}

#[test]
fn contradiction_is_not_a_vacuous_frontier_and_large_coupled_products_refuse() {
    let mut empty = binary(1);
    empty["constraints"]
        .as_array_mut()
        .unwrap()
        .push(json!({"fields":["x00"],"rows":[{"x00":2}]}));
    assert_eq!(
        plan(&parse(&empty), &mut Budget::default(), false)
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualFamilyEmpty
    );
    let mut large = binary(9);
    large["dependencies"] = json!([{"kind":"order","fields":(0..9).map(|index| format!("x{index:02}")).collect::<Vec<_>>()}]);
    let error = plan(&parse(&large), &mut Budget::default(), false).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("256"));
}

#[test]
fn strict_graph_schema_and_missing_domains_refuse() {
    let original = binary(2);
    let mut invalid = Vec::new();
    let mut value = original.clone();
    value["dependencies"] = json!([{"kind":"sampled_correlation","fields":["x00","x01"]}]);
    invalid.push(value);
    let mut value = original.clone();
    value["constraints"].as_array_mut().unwrap().pop();
    invalid.push(value);
    let mut value = original.clone();
    value["constraints"][0]["fields"] = json!(["x00", "x00"]);
    invalid.push(value);
    let mut value = original.clone();
    value["dependencies"] = json!([{"kind":"binding","fields":["x00","missing"]}]);
    invalid.push(value);
    let mut value = original.clone();
    value["constraints"][0]["rows"] = json!([]);
    invalid.push(value);
    let mut value = original.clone();
    value["constraints"][0]["rows"] = json!([{"x00":0},{"x00":0}]);
    invalid.push(value);
    let mut value = original;
    value["independent"] = json!(true);
    invalid.push(value);
    invalid.push(binary(65));
    for value in invalid {
        assert!(
            Problem::parse(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{value}"
        );
    }
    assert!(
        Problem::parse(br#"{"fields":[],"fields":[],"constraints":[],"dependencies":[]}"#).is_err()
    );
}

#[test]
fn canonical_permutations_keep_identity_but_missing_dependency_is_stale() {
    let mut value = binary(3);
    value["dependencies"] = json!([{"kind":"order","fields":["x00","x01"]}]);
    let problem = parse(&value);
    let frontier = plan(&problem, &mut Budget::default(), true).unwrap();
    let row = json!({"x00":0,"x01":1,"x02":0})
        .as_object()
        .unwrap()
        .clone();
    let answers = frontier.encode(&problem, &row).unwrap();
    let mut reordered = value.clone();
    reordered["fields"].as_array_mut().unwrap().reverse();
    reordered["constraints"].as_array_mut().unwrap().reverse();
    for constraint in reordered["constraints"].as_array_mut().unwrap() {
        constraint["rows"].as_array_mut().unwrap().reverse();
    }
    reordered["dependencies"][0]["fields"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_eq!(frontier.decode(&parse(&reordered), &answers).unwrap(), row);
    for pointer in ["/dependencies", "/constraints/0/rows", "/fields/0/cost"] {
        let mut changed = value.clone();
        *changed.pointer_mut(pointer).unwrap() = match pointer {
            "/dependencies" => json!([]),
            "/fields/0/cost" => json!(2),
            _ => json!([{"x00":0}]),
        };
        assert_eq!(
            frontier
                .decode(&parse(&changed), &answers)
                .unwrap_err()
                .code(),
            AgentErrorCode::ResidualBindingStale
        );
    }
}

#[test]
fn typed_answers_and_all_component_fields_are_checked() {
    let mut value = binary(2);
    value["constraints"][0]["rows"] = json!([{"x00":true},{"x00":1},{"x00":"1"}]);
    let problem = parse(&value);
    let frontier = plan(&problem, &mut Budget::default(), true).unwrap();
    for x in [json!(true), json!(1), json!("1")] {
        let row = json!({"x00":x,"x01":0}).as_object().unwrap().clone();
        assert_eq!(
            frontier
                .decode(&problem, &frontier.encode(&problem, &row).unwrap())
                .unwrap(),
            row
        );
    }
    for answers in [
        json!({"x00":true}),
        json!({"x00":false,"x01":0}),
        json!({"x00":true,"x01":0,"index":0}),
    ] {
        assert!(
            frontier
                .decode(&problem, answers.as_object().unwrap())
                .is_err()
        );
    }
}

#[test]
fn shared_budget_cannot_reset_for_the_next_component() {
    let problem = parse(&binary(2));
    assert_eq!(
        plan(
            &problem,
            &mut Budget::limited(Duration::ZERO, 12_000_000),
            false
        )
        .unwrap_err()
        .code(),
        AgentErrorCode::ResidualLimit
    );
    let mut budget = Budget::limited(Duration::from_secs(2), 12);
    assert_eq!(
        plan(&problem, &mut budget, false).unwrap_err().code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(
        plan(&problem, &mut budget, false).unwrap_err().code(),
        AgentErrorCode::ResidualLimit
    );
}

#[test]
fn small_graphs_match_an_independent_exhaustive_conjunction_oracle() {
    // Exhaustive test-only product: production must not take this route.
    for seed in 0..128_u64 {
        let mut value = binary(4);
        for (index, field) in value["fields"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            field["cost"] = json!(1 + ((seed >> (2 * index)) & 3));
        }
        let mut tables = Vec::new();
        for edge in 0..3 {
            if seed & (1 << edge) != 0 {
                let a = format!("x{edge:02}");
                let b = format!("x{:02}", edge + 1);
                let rows: Vec<_> = (0..4)
                    .filter(|bits| (bits + seed) % 3 != 0)
                    .map(|bits| json!({a.clone():bits/2,b.clone():bits%2}))
                    .collect();
                tables.push(json!({"fields":[a,b],"rows":rows}));
            }
        }
        value["constraints"].as_array_mut().unwrap().extend(tables);
        let rows: Vec<Map<String, Value>> = (0..16)
            .map(|bits| {
                (0..4)
                    .map(|index| (format!("x{index:02}"), json!((bits >> index) & 1)))
                    .collect()
            })
            .filter(|row: &Map<String, Value>| {
                value["constraints"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|table| {
                        table["rows"].as_array().unwrap().iter().any(|allowed| {
                            allowed
                                .as_object()
                                .unwrap()
                                .iter()
                                .all(|(name, value)| row.get(name) == Some(value))
                        })
                    })
            })
            .collect();
        let problem = parse(&value);
        let result = plan(&problem, &mut Budget::default(), true);
        if rows.is_empty() {
            assert_eq!(
                result.unwrap_err().code(),
                AgentErrorCode::ResidualFamilyEmpty
            );
            continue;
        }
        let frontier = result.unwrap();
        assert_eq!(
            frontier.summary()["description_count_decimal"],
            rows.len().to_string()
        );
        let mut projections = BTreeSet::new();
        for row in &rows {
            let answers = frontier.encode(&problem, row).unwrap();
            assert!(projections.insert(serde_json::to_vec(&answers).unwrap()));
            assert_eq!(frontier.decode(&problem, &answers).unwrap(), *row);
        }
        // Compare the exact additive optimum against independent subset/projection
        // enumeration in the existing test oracle, using the full tiny relation.
        let family = Family::parse(
            &serde_json::to_vec(&json!({"fields":value["fields"],"descriptions":rows})).unwrap(),
        )
        .unwrap();
        let expected = crate::residual::frontier::tests::oracle(&family).0;
        assert_eq!(frontier.summary()["additive_cost"], expected);
    }
}

#[test]
fn large_constants_cannot_amplify_join_storage_past_the_byte_ceiling() {
    let mut value = binary(2);
    value["constraints"][0]["rows"] = json!([{"x00":"x".repeat(9000)}]);
    value["constraints"][1]["rows"] = json!(
        (0..256)
            .map(|index| json!({"x01":index}))
            .collect::<Vec<_>>()
    );
    value["dependencies"] = json!([{"kind":"effect","fields":["x00","x01"]}]);
    let error = plan(&parse(&value), &mut Budget::default(), false).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("byte ceiling"));
}

#[test]
fn borrowed_component_encoding_preserves_the_existing_canonical_envelope() {
    let descriptors = vec![json!({"name":"x","cost":1,"eligible":true})];
    let fields = [super::Field {
        name: "x".into(),
        cost: 1,
        eligible: true,
    }];
    let rows = vec![json!({"x":["\n",true,1]}).as_object().unwrap().clone()];
    let expected = serde_json::to_vec(&json!({"fields":descriptors,"descriptions":rows})).unwrap();
    let mut budget = Budget::default();
    let identity = super::rows::Rows::identity(&mut budget).unwrap();
    let joined = super::rows::join(&identity, &rows, &mut budget).unwrap();
    let borrowed = super::ComponentEncoding {
        descriptors: super::Descriptors {
            fields: &fields,
            mask: 1,
        },
        rows: &joined.values,
    };
    let encoded = super::super::encoding::encode(&borrowed, &mut budget, expected.len()).unwrap();
    assert_eq!(encoded.bytes, expected);
    assert_eq!(
        Family::parse(&encoded.bytes).unwrap().digest,
        Family::parse(&expected).unwrap().digest
    );
}

#[test]
fn borrowed_descriptors_match_legacy_bytes_for_sparse_masks_and_escaped_names() {
    let fields = [
        super::Field {
            name: "a\\\"\n".into(),
            cost: u32::MAX,
            eligible: false,
        },
        super::Field {
            name: "zé".into(),
            cost: 1,
            eligible: true,
        },
    ];
    for mask in [0, 1, 2, 3] {
        let expected: Vec<_> = super::super::bits(mask)
            .map(|index| super::field_value(&fields[index]))
            .collect();
        let descriptors = super::Descriptors {
            fields: &fields,
            mask,
        };
        assert_eq!(
            serde_json::to_vec(&descriptors).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
    }
}

#[test]
fn fixed_topology_handles_all_sixty_four_bits_and_preserves_component_order() {
    let mut value = binary(64);
    let problem = parse(&value);
    let groups = super::topology::Groups::new(&problem, &mut Budget::default()).unwrap();
    assert_eq!(groups.len(), 64);
    assert_eq!(
        groups.iter().collect::<Vec<_>>(),
        (0..64).map(|i| (i, 1_u64 << i)).collect::<Vec<_>>()
    );
    value["dependencies"] = json!([{"kind":"order","fields":["x00","x63"]}]);
    let problem = parse(&value);
    let groups = super::topology::Groups::new(&problem, &mut Budget::default()).unwrap();
    assert_eq!(groups.len(), 63);
    assert_eq!(groups.iter().next(), Some((0, 1 | (1_u64 << 63))));
    assert!(!groups.iter().any(|(root, _)| root == 63));
    for (index, table) in problem.constraints.iter().enumerate() {
        let field = super::index_of(&problem.fields, &table.fields[0].name).unwrap();
        assert_eq!(
            groups.table_root(index),
            if field == 63 { 0 } else { field }
        );
    }
}

#[test]
fn fixed_topology_matches_independent_transitive_closure_for_seeded_graphs() {
    for seed in 0_u64..64 {
        let mut state = seed + 1;
        let mut value = binary(8);
        let mut edges = Vec::new();
        let mut reach: [u64; 8] = std::array::from_fn(|i| 1 << i);
        for left in 0..8 {
            for right in left + 1..8 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                if state >> 60 == 0 {
                    edges.push(json!({"kind":"binding","fields":[format!("x{left:02}"),format!("x{right:02}")]}));
                    reach[left] |= 1 << right;
                    reach[right] |= 1 << left;
                }
            }
        }
        value["dependencies"] = json!(edges);
        for via in 0..8 {
            for from in 0..8 {
                if reach[from] & (1 << via) != 0 {
                    reach[from] |= reach[via];
                }
            }
        }
        let expected: Vec<_> = reach
            .iter()
            .copied()
            .enumerate()
            .filter(|(index, mask)| mask.trailing_zeros() as usize == *index)
            .collect();
        let problem = parse(&value);
        let groups = super::topology::Groups::new(&problem, &mut Budget::default()).unwrap();
        assert_eq!(groups.iter().collect::<Vec<_>>(), expected, "seed {seed}");
        for (index, table) in problem.constraints.iter().enumerate() {
            let field = super::index_of(&problem.fields, &table.fields[0].name).unwrap();
            assert_eq!(
                groups.table_root(index),
                reach[field].trailing_zeros() as usize
            );
        }
    }
}

#[test]
fn topology_walk_uses_shared_work_without_heap_reservations_or_budget_restart() {
    let problem = parse(&binary(8));
    let mut measured = Budget::default();
    super::topology::Groups::new(&problem, &mut measured).unwrap();
    assert_eq!(measured.usage()["memory"]["peak_reserved_bytes"], 0);
    let work = measured.usage()["charged_work"].as_u64().unwrap();
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), work, 0);
    super::topology::Groups::new(&problem, &mut budget).unwrap();
    let error = super::topology::Groups::new(&problem, &mut budget)
        .err()
        .unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert_eq!(budget.usage()["charged_work"], work);
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 0);
}

#[test]
fn separate_components_share_the_materialization_byte_ceiling() {
    let mut value = binary(4);
    for index in [0, 2] {
        let name = format!("x{index:02}");
        value["constraints"][index]["rows"] = json!([{name:"x".repeat(6000)}]);
        let name = format!("x{:02}", index + 1);
        value["constraints"][index + 1]["rows"] = json!(
            (0..100)
                .map(|number| json!({name.clone():number}))
                .collect::<Vec<_>>()
        );
    }
    value["dependencies"] = json!([
        {"kind":"binding","fields":["x00","x01"]},
        {"kind":"binding","fields":["x02","x03"]}
    ]);
    let mut budget = Budget::default();
    let error = plan(&parse(&value), &mut budget, false).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("byte ceiling"), "{error}");
    assert_eq!(
        budget.usage()["planned_fields"],
        2,
        "first component completed, second did not escape"
    );
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(
        budget.usage()["memory"]["peak_reserved_bytes"]
            .as_u64()
            .unwrap()
            > 600_000
    );
}

#[test]
fn component_encoding_reserves_before_parsing_or_planning_its_family() {
    let mut value = binary(1);
    value["constraints"][0]["rows"] = json!([{"x00":"x".repeat(200)}]);
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 100_000, 128);
    let error = plan(&parse(&value), &mut budget, false).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("planner memory"), "{error}");
    assert_eq!(budget.usage()["planned_fields"], 0);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(
        budget.usage()["memory"]["peak_reserved_bytes"]
            .as_u64()
            .unwrap()
            <= 128
    );
    assert!(budget.checkpoint().is_err());
}

#[test]
fn exhausted_component_refinement_never_claims_global_optimality() {
    let mut value = binary(3);
    value["dependencies"] = json!([{"kind":"binding","fields":["x00","x01","x02"]}]);
    let problem = parse(&value);
    let mut measured = Budget::default();
    let greedy = plan(&problem, &mut measured, false).unwrap();
    let work = measured.usage()["charged_work"].as_u64().unwrap();
    let mut budget = Budget::limited(Duration::from_secs(2), work + 1);
    let partial = plan(&problem, &mut budget, true).unwrap();
    assert_eq!(partial.summary()["method"], "checked_component_frontiers");
    assert_eq!(
        partial.summary()["components"][0]["frontier"]["method"],
        "bounded_best"
    );
    assert_eq!(partial.fields(), greedy.fields());
    let row = json!({"x00":1,"x01":0,"x02":1})
        .as_object()
        .unwrap()
        .clone();
    assert_eq!(
        partial
            .decode(&problem, &partial.encode(&problem, &row).unwrap())
            .unwrap(),
        row
    );
}

#[test]
fn huge_implicit_cardinality_is_neither_overflowed_nor_materialized() {
    let mut value = binary(64);
    for (index, table) in value["constraints"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        let name = format!("x{index:02}");
        table["rows"] = json!(
            (0..8)
                .map(|choice| json!({name.clone():choice}))
                .collect::<Vec<_>>()
        );
    }
    let summary = plan(&parse(&value), &mut Budget::default(), true)
        .unwrap()
        .summary();
    assert_eq!(summary["materialized_component_rows"], 512);
    assert_eq!(summary["count_exceeds_u128"], true);
    assert!(summary["description_count_decimal"].is_null());
}
