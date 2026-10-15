use super::{Budget, Row, Rows, join};
use crate::error::AgentErrorCode;
use serde_json::{Map, Value, json};
use std::time::Duration;

fn table(value: Value) -> Vec<Map<String, Value>> {
    let Value::Array(rows) = value else {
        panic!("array")
    };
    rows.into_iter()
        .map(|row| {
            let Value::Object(row) = row else {
                panic!("object")
            };
            row
        })
        .collect()
}

fn live(budget: &Budget) -> usize {
    usize::try_from(budget.usage()["memory"]["reserved_bytes"].as_u64().unwrap()).unwrap()
}

#[test]
fn joins_borrow_nested_values_and_preserve_exact_sorted_union_bytes() {
    let left = table(json!([{"z":[{"deep":"x".repeat(10000)}],"b":1}]));
    let right = table(json!([{"a":"\n","b":1,"c":true},{"a":null,"b":1,"c":false}]));
    let mut budget = Budget::default();
    let identity = Rows::identity(&mut budget).unwrap();
    let first = join(&identity, &left, &mut budget).unwrap();
    let result = join(&first, &right, &mut budget).unwrap();
    for (index, row) in result.values.iter().enumerate() {
        assert!(std::ptr::eq(
            row.get(&"z".into()).unwrap(),
            &raw const left[0]["z"]
        ));
        assert!(std::ptr::eq(
            row.get(&"a".into()).unwrap(),
            &raw const right[index]["a"]
        ));
        let mut expected = left[0].clone();
        expected.extend(right[index].clone());
        assert_eq!(
            serde_json::to_vec(row).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
    }
    // The 10 KiB payload is borrowed twice; reservations cover only buffers.
    assert!(live(&budget) < 1000);
    drop((identity, first, result));
    assert_eq!(live(&budget), 0);
}

#[test]
fn successive_joins_keep_old_and_new_buffers_charged_until_their_owners_drop() {
    let table = table(json!([{"x":1},{"x":2}]));
    let mut budget = Budget::default();
    let identity = Rows::identity(&mut budget).unwrap();
    let seed_bytes = live(&budget);
    assert_eq!(seed_bytes, std::mem::size_of::<Row<'_>>());
    let first = join(&identity, &table, &mut budget).unwrap();
    let first_bytes = live(&budget) - seed_bytes;
    let second = join(&first, &table, &mut budget).unwrap();
    let second_bytes = live(&budget) - seed_bytes - first_bytes;
    assert!(first_bytes > 0 && second_bytes > 0);
    drop(first);
    assert_eq!(live(&budget), seed_bytes + second_bytes);
    drop(identity);
    assert_eq!(live(&budget), second_bytes);
    // The result borrows input tables, never an intermediate row buffer.
    assert_eq!(
        serde_json::to_value(&second.values).unwrap(),
        json!([{"x":1},{"x":2}])
    );
    drop(second);
    assert_eq!(live(&budget), 0);
}

#[test]
fn join_memory_denial_is_sticky_and_releases_partial_rows() {
    let table = table(json!([{"x":1},{"x":2}]));
    let row_bytes = std::mem::size_of::<Row<'_>>();
    let entry_bytes = std::mem::size_of::<super::Entry<'_>>();
    // Enough for input, sizes, output headers and one output row's entries.
    let ceiling = 3 * row_bytes + 3 * std::mem::size_of::<usize>() + entry_bytes;
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 10000, ceiling);
    let identity = Rows::identity(&mut budget).unwrap();
    let error = join(&identity, &table, &mut budget).err().unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("planner memory"));
    assert_eq!(live(&budget), row_bytes);
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], ceiling);
    drop(identity);
    assert_eq!(live(&budget), 0);
    assert!(budget.checkpoint().is_err());
}

#[test]
fn conflicting_typed_values_refuse_and_release_all_output_capacity() {
    let left = table(json!([{"x":1}]));
    let right = table(json!([{"x":"1"}]));
    let mut budget = Budget::default();
    let identity = Rows::identity(&mut budget).unwrap();
    let first = join(&identity, &left, &mut budget).unwrap();
    let before = live(&budget);
    let error = join(&first, &right, &mut budget).err().unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualFamilyEmpty);
    assert_eq!(live(&budget), before);
    budget.checkpoint().unwrap();
    drop((identity, first));
    assert_eq!(live(&budget), 0);
}

#[test]
fn row_and_byte_limits_release_join_outputs_without_returning_partial_success() {
    for count in [256, 257] {
        let right = table(json!(
            (0..count).map(|x| json!({"x":x})).collect::<Vec<_>>()
        ));
        let mut budget = Budget::default();
        let identity = Rows::identity(&mut budget).unwrap();
        let joined = join(&identity, &right, &mut budget);
        if count == 256 {
            assert_eq!(joined.unwrap().values.len(), count);
        } else {
            assert_eq!(joined.err().unwrap().code(), AgentErrorCode::ResidualLimit);
        }
        assert_eq!(live(&budget), std::mem::size_of::<Row<'_>>());
    }
    let large = table(json!([{"x":"x".repeat(crate::residual::MAX_REQUEST_BYTES)}]));
    let mut budget = Budget::default();
    let identity = Rows::identity(&mut budget).unwrap();
    let error = join(&identity, &large, &mut budget).err().unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("byte ceiling"));
    assert_eq!(live(&budget), std::mem::size_of::<Row<'_>>());
}
