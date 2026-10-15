use std::{cell::Cell, time::Duration};

use serde::{Serialize, Serializer, ser::Error};
use serde_json::json;

use super::{domain, encode, size};
use crate::{error::AgentErrorCode, residual::frontier::Budget};

#[test]
fn streamed_size_and_reserved_encoding_match_json_bytes_exactly() {
    for value in [
        json!(null),
        json!(true),
        json!(u64::MAX),
        json!(i64::MIN),
        json!("quotes\" slash\\ newline\n\0 é 💡"),
        json!({"escaped\nkey":[null, false, {}, [], {"x":"\t"}]}),
    ] {
        let expected = serde_json::to_vec(&value).unwrap();
        let mut budget = Budget::default();
        assert_eq!(
            size(&value, &mut budget, expected.len()).unwrap(),
            expected.len()
        );
        assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 0);
        let encoded = encode(&value, &mut budget, expected.len()).unwrap();
        assert_eq!(encoded.bytes, expected);
        assert_eq!(budget.usage()["memory"]["reserved_bytes"], expected.len());
        drop(encoded);
        assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    }
}

#[test]
fn byte_refusal_precedes_output_allocation_including_escape_expansion() {
    let value = json!("\0".repeat(1000));
    let length = serde_json::to_vec(&value).unwrap().len();
    let mut budget = Budget::default();
    let error = encode(&value, &mut budget, length - 1).err().unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("byte ceiling"));
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 0);
}

#[test]
fn serialization_consumes_existing_work_before_reserving_output() {
    let mut budget = Budget::limited(Duration::from_secs(2), 4);
    let error = encode(&json!([0, 1, 2, 3, 4]), &mut budget, 100)
        .err()
        .unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 0);
    assert!(budget.usage()["charged_work"].as_u64().unwrap() > 0);
}

#[test]
fn encoded_buffers_share_the_memory_ceiling_and_release_on_drop() {
    let value = json!("x".repeat(30));
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 1000, 63);
    let first = encode(&value, &mut budget, 100).unwrap();
    let error = encode(&value, &mut budget, 100).err().unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("planner memory"));
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 32);
    drop(first);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(budget.checkpoint().is_err());
}

struct ChangesOnSecondPass {
    calls: Cell<usize>,
    fail: bool,
}

impl Serialize for ChangesOnSecondPass {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let call = self.calls.replace(self.calls.get() + 1);
        if call != 0 && self.fail {
            return Err(S::Error::custom("injected second-pass failure"));
        }
        serializer.serialize_str(if call == 0 { "a" } else { "longer" })
    }
}

#[test]
fn second_pass_refusals_release_capacity_and_never_grow_past_measured_size() {
    for fail in [true, false] {
        let value = ChangesOnSecondPass {
            calls: Cell::new(0),
            fail,
        };
        let mut budget = Budget::default();
        let error = encode(&value, &mut budget, 100).err().unwrap();
        assert_eq!(
            error.code(),
            if fail {
                AgentErrorCode::ResidualParse
            } else {
                AgentErrorCode::ResidualLimit
            }
        );
        assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 3);
        assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    }
}

#[test]
fn decision_domains_match_legacy_typed_order_without_copying_values() {
    let values = [
        json!(true),
        json!(1),
        json!("1"),
        json!(null),
        json!({"z":["\0",false]}),
        json!(true),
        json!([]),
        json!(-1),
    ];
    let legacy: std::collections::BTreeMap<_, _> = values
        .iter()
        .map(|value| (serde_json::to_vec(value).unwrap(), value))
        .collect();
    let mut budget = Budget::default();
    let result = domain(values.iter(), &mut budget, 1000).unwrap();
    assert_eq!(
        result.values().collect::<Vec<_>>(),
        legacy.values().copied().collect::<Vec<_>>()
    );
    for borrowed in result.values() {
        assert!(
            values
                .iter()
                .any(|original| std::ptr::eq(original, borrowed))
        );
    }
    let expected = values.len() * size_of::<(super::Encoded, &serde_json::Value)>()
        + legacy.keys().map(Vec::len).sum::<usize>();
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], expected);
    drop(result);
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
}

#[test]
fn domain_key_memory_denial_releases_partial_keys_and_remains_exhausted() {
    let values = [json!("a".repeat(100)), json!("b".repeat(100))];
    let vector = values.len() * size_of::<(super::Encoded, &serde_json::Value)>();
    let mut budget = Budget::limited_with_memory(Duration::from_secs(2), 100_000, vector + 203);
    let error = domain(values.iter(), &mut budget, 1000).err().unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert_eq!(
        budget.usage()["memory"]["peak_reserved_bytes"],
        vector + 102
    );
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    assert!(budget.checkpoint().is_err());
}

#[test]
fn domains_enforce_count_byte_and_existing_work_bounds_without_leaks() {
    let values = vec![json!(false); 257];
    let mut budget = Budget::default();
    assert_eq!(
        domain(values.iter(), &mut budget, 1000)
            .err()
            .unwrap()
            .code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(budget.usage()["memory"]["peak_reserved_bytes"], 0);
    let values = [json!("\0".repeat(100))];
    let error = domain(values.iter(), &mut budget, 100).err().unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    assert!(error.detail().contains("byte ceiling"));
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
    let mut budget = Budget::limited(Duration::from_secs(2), 1);
    assert_eq!(
        domain(values.iter(), &mut budget, 1000)
            .err()
            .unwrap()
            .code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(budget.usage()["memory"]["reserved_bytes"], 0);
}
