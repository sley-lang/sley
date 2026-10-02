use super::{Budget, Family, digest, from_value};
use crate::error::AgentErrorCode;
use serde_json::{Value, json};
use std::time::Duration;

fn declaration() -> Value {
    json!({"fields":[{"name":"b","cost":2,"eligible":false},{"name":"a","cost":1,"eligible":true}],
        "descriptions":[{"b":null,"a":{"z":["nested",true,false,null],"a":i64::MIN}},
            {"a":u64::MAX,"b":"\0\n é 💡"}, {"a":[{},[],0],"b":false}]})
}

fn reference(family: &Family) -> String {
    let value = json!({"fields":family.fields.iter().map(|field| json!({
        "name":field.name,"cost":field.cost,"eligible":field.eligible})).collect::<Vec<_>>(),
        "descriptions":family.rows.as_ref()});
    crate::residual::binding::canonical_digest("finite-description-family-v1", &value).unwrap()
}

#[test]
fn consuming_family_construction_preserves_nested_allocations_and_canonical_identity() {
    let value = declaration();
    let original = value["descriptions"][0]["a"]["z"][0]
        .as_str()
        .unwrap()
        .as_ptr();
    let mut budget = Budget::default();
    let family = from_value(value, &mut budget).unwrap();
    let row = family.rows.iter().find(|row| row["a"].is_object()).unwrap();
    assert_eq!(row["a"]["z"][0].as_str().unwrap().as_ptr(), original);
    assert_eq!(family.digest, reference(&family));
    assert!(budget.usage()["memory"]["reserved_bytes"].as_u64().unwrap() > 0);
    assert!(
        budget.usage()["memory"]["peak_reserved_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    drop(family);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
}

#[test]
fn streamed_family_digests_match_legacy_encoding_for_typed_values_and_permutations() {
    let mut value = declaration();
    let original = Family::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    for _ in 0..3 {
        value["fields"].as_array_mut().unwrap().reverse();
        value["descriptions"].as_array_mut().unwrap().rotate_left(1);
        let parsed =
            Family::parse_with_budget(&serde_json::to_vec(&value).unwrap(), &mut Budget::default())
                .unwrap();
        assert_eq!(parsed.digest, original.digest);
        assert_eq!(parsed.digest, reference(&parsed));
        let keys: Vec<_> = parsed
            .rows
            .iter()
            .map(|row| serde_json::to_vec(row).unwrap())
            .collect();
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    }
    for item in [
        json!(null),
        json!(false),
        json!(true),
        json!(0),
        json!("0"),
        json!([]),
        json!({}),
    ] {
        value["descriptions"] = json!([{"a":item,"b":null}]);
        let parsed = Family::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(parsed.digest, reference(&parsed));
    }
}

#[test]
fn canonical_hashing_has_no_payload_sized_buffer_and_retains_parent_indexes() {
    let fields = vec![super::super::Field {
        name: "x".into(),
        cost: u32::MAX,
        eligible: true,
    }];
    let rows = vec![
        json!({"x":{"inner":["x".repeat(200_000)]}})
            .as_object()
            .unwrap()
            .clone(),
    ];
    let mut budget = Budget::default();
    let hash = digest::family(&fields, &rows, &mut budget).unwrap();
    let value = json!({"fields":[{"name":"x","cost":u32::MAX,"eligible":true}],
        "descriptions":rows});
    assert_eq!(
        hash,
        crate::residual::binding::canonical_digest("finite-description-family-v1", &value).unwrap()
    );
    assert_eq!(
        budget.usage()["memory"]["peak_reserved_bytes"],
        2 * std::mem::size_of::<(&String, &Value)>()
    );
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
}

#[test]
fn canonical_traversal_charges_shared_work_and_refuses_without_leaking_indexes() {
    let family = Family::parse(&serde_json::to_vec(&declaration()).unwrap()).unwrap();
    let mut budget = Budget::limited(Duration::from_secs(2), 40);
    let error = digest::family(&family.fields, &family.rows, &mut budget).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(budget.usage()["charged_work"].as_u64().unwrap() > 0);
}

#[test]
fn family_sort_keys_share_memory_and_refusal_drops_every_partial_key() {
    let mut budget = Budget::default();
    let family = from_value(declaration(), &mut budget).unwrap();
    let peak = usize::try_from(
        budget.usage()["memory"]["peak_reserved_bytes"]
            .as_u64()
            .unwrap(),
    )
    .unwrap();
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 100_000, peak - 1);
    let error = from_value(declaration(), &mut budget).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(budget.checkpoint().is_err());
    assert_eq!(family.digest, reference(&family));
}

#[test]
fn duplicate_and_incomplete_family_errors_release_reserved_keys() {
    for duplicate in [false, true] {
        let mut value = declaration();
        if duplicate {
            value["descriptions"][2] = value["descriptions"][0].clone();
        } else {
            value["descriptions"][2]
                .as_object_mut()
                .unwrap()
                .remove("b");
        }
        let mut budget = Budget::default();
        let error = from_value(value, &mut budget).unwrap_err();
        assert_eq!(
            error.code(),
            if duplicate {
                AgentErrorCode::ResidualConstraintConflict
            } else {
                AgentErrorCode::ResidualFamilyIncomplete
            }
        );
        assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
        budget.checkpoint().unwrap();
    }
}

#[test]
fn retained_payload_counts_capacity_including_unused_string_and_array_storage() {
    let mut text = String::with_capacity(4096);
    text.push('v');
    let mut key = String::with_capacity(512);
    key.push('x');
    let mut array = Vec::with_capacity(128);
    let expected = text.capacity()
        + key.capacity()
        + array.capacity() * size_of::<Value>()
        + size_of::<super::super::Field>()
        + 1
        + size_of::<serde_json::Map<String, Value>>();
    array.push(Value::String(text));
    let row = Value::Object(serde_json::Map::from_iter([(key, Value::Array(array))]));
    let mut value = json!({"fields":[{"name":"x","cost":1,"eligible":true}]});
    value["descriptions"] = Value::Array(vec![row]);
    let mut budget = Budget::default();
    let family = from_value(value, &mut budget).unwrap();
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], expected);
    assert_eq!(family.digest, reference(&family));
    drop(family);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
}

#[test]
fn family_clones_share_payload_and_release_only_after_the_last_owner() {
    let mut budget = Budget::default();
    let original = from_value(declaration(), &mut budget).unwrap();
    let live = budget.usage()["memory"]["reserved_bytes"].clone();
    let cloned = original.clone();
    assert!(std::sync::Arc::ptr_eq(&original.rows, &cloned.rows));
    assert!(std::sync::Arc::ptr_eq(&original.fields, &cloned.fields));
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], live);
    drop(original);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], live);
    assert_eq!(cloned.digest, reference(&cloned));
    std::thread::spawn(move || drop(cloned)).join().unwrap();
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    budget.checkpoint().unwrap();
}

#[test]
fn simultaneously_retained_families_share_the_construction_ceiling() {
    let mut measured = Budget::default();
    let first = from_value(declaration(), &mut measured).unwrap();
    let live = measured.usage()["memory"]["reserved_bytes"].clone();
    let ceiling = usize::try_from(
        measured.usage()["memory"]["peak_reserved_bytes"]
            .as_u64()
            .unwrap(),
    )
    .unwrap();
    drop(first);
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 100_000, ceiling);
    let first = from_value(declaration(), &mut budget).unwrap();
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], live);
    let error = from_value(declaration(), &mut budget).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], live);
    drop(first);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(budget.checkpoint().is_err());
}

#[test]
fn retained_payload_refusals_release_capacity_and_charge_traversal_work() {
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 100_000, 1);
    assert_eq!(
        from_value(declaration(), &mut budget).unwrap_err().code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 0);
    assert!(budget.checkpoint().is_err());

    let mut budget = Budget::limited(Duration::from_secs(2), 2);
    assert_eq!(
        from_value(declaration(), &mut budget).unwrap_err().code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);

    let mut value = declaration();
    value["fields"][1]["cost"] = json!(0);
    let mut budget = Budget::default();
    assert_eq!(
        from_value(value, &mut budget).unwrap_err().code(),
        AgentErrorCode::ResidualParse
    );
    assert!(
        budget.usage()["memory"]["peak_reserved_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    budget.checkpoint().unwrap();
}
