use super::*;
use crate::codec::{
    decode_const_value, encode_at_depth, encode_const_value, measure_const_value_bounded,
};
use sley_scb1::{encode_record, encode_text, encode_union};

fn value(data: ConstData) -> ConstValue {
    let value_type = match &data {
        ConstData::Bool(_) => TypeExpr::Bool,
        ConstData::Text(_) => TypeExpr::Text,
        ConstData::Bytes(_) => TypeExpr::Bytes,
        ConstData::F32Bits(_) => TypeExpr::F32,
        _ => TypeExpr::Unit, // Codec syntax tests; type admission has a separate owner.
    };
    ConstValue { value_type, data }
}

fn assert_caps(value: &ConstValue) {
    let encoded = encode_const_value(value).unwrap();
    let length = u64::try_from(encoded.len()).unwrap();
    for cap in [0, length - 1, length, length + 1, u64::MAX] {
        assert_eq!(
            measure_const_value_bounded(value, cap).unwrap(),
            if cap < length {
                ConstValueByteMeasure::OverLimit
            } else {
                ConstValueByteMeasure::Exact(length)
            }
        );
    }
    assert_eq!(decode_const_value(&encoded).unwrap(), *value);
}

fn assert_error_all_caps(value: &ConstValue, expected: ScbErrorCode) {
    for cap in [0, 1, 128, u64::MAX] {
        assert_eq!(
            measure_const_value_bounded(value, cap).unwrap_err().code(),
            expected
        );
    }
    assert_eq!(encode_const_value(value).unwrap_err().code(), expected);
}

#[test]
fn boundary_lengths_options_results_and_utf8() {
    for length in [0, 1, 126, 127, 128, 129, 16_382, 16_383, 16_384] {
        assert_caps(&value(ConstData::Bytes(vec![0x55; length])));
        assert_caps(&value(ConstData::Text("é".repeat(length))));
    }
    for width in [127, 128] {
        assert_caps(&value(ConstData::Sequence(vec![
            value(ConstData::Bool(
                true
            ));
            width
        ])));
    }
    for data in [
        ConstData::SInt(i128::MIN),
        ConstData::SInt(i128::MAX),
        ConstData::UInt(u128::MAX),
        ConstData::Option(None),
        ConstData::Option(Some(Box::new(value(ConstData::Unit)))),
        ConstData::Result(ResultConst::Ok(Box::new(value(ConstData::Unit)))),
        ConstData::Result(ResultConst::Err(Box::new(value(ConstData::Text(
            "e\u{301}".into(),
        ))))),
    ] {
        assert_caps(&value(data));
    }
}

// Independent primitive-owner composition equivalent to the pre-refactor Text record.
fn text_wire(text: &str) -> Vec<u8> {
    encode_record(&[
        (1, encode_union(8, &[]).unwrap()),
        (2, encode_union(8, &encode_text(text).unwrap()).unwrap()),
    ])
    .unwrap()
}

fn text_map(mut texts: Vec<String>) -> ConstValue {
    texts.sort_by_key(|text| text_wire(text));
    ConstValue {
        value_type: TypeExpr::OrderedMap {
            key: Box::new(TypeExpr::Text),
            value: Box::new(TypeExpr::Unit),
        },
        data: ConstData::Map(
            texts
                .into_iter()
                .map(|text| MapEntryConst {
                    key: value(ConstData::Text(text)),
                    value: value(ConstData::Unit),
                })
                .collect(),
        ),
    }
}

#[test]
fn map_cursor_order_agrees_with_primitive_owner_bytes() {
    let texts: Vec<String> = [0, 1, 126, 127, 128, 16_383, 16_384]
        .into_iter()
        .map(|n| "a".repeat(n))
        .collect();
    for left in &texts {
        for right in &texts {
            let a = value(ConstData::Text(left.clone()));
            let b = value(ConstData::Text(right.clone()));
            measure(Node::Value(&a), 0, true).unwrap();
            measure(Node::Value(&b), 0, true).unwrap();
            assert_eq!(
                compare_validated(Node::Value(&a), Node::Value(&b), 0).unwrap(),
                text_wire(left).cmp(&text_wire(right))
            );
        }
    }
    let map = text_map(texts);
    assert_caps(&map);
    let ConstData::Map(entries) = map.data else {
        unreachable!()
    };
    let mut reversed = entries.clone();
    reversed.reverse();
    assert_error_all_caps(&value(ConstData::Map(reversed)), ScbErrorCode::MapOrder);
    assert_error_all_caps(
        &value(ConstData::Map(vec![entries[0].clone(), entries[0].clone()])),
        ScbErrorCode::MapDuplicate,
    );
}

#[test]
fn canonical_errors_win_over_every_output_cap() {
    assert_error_all_caps(
        &value(ConstData::F32Bits(0x8000_0000)),
        ScbErrorCode::FloatNonCanonical,
    );
    assert_error_all_caps(
        &value(ConstData::F64Bits(0x7ff0_0000_0000_0001)),
        ScbErrorCode::FloatNonCanonical,
    );
    let invalid = value(ConstData::F32Bits(0x8000_0000));
    let mut map = text_map(vec!["a".into(), "b".into()]);
    let ConstData::Map(entries) = &mut map.data else {
        unreachable!()
    };
    entries[1].value = invalid.clone();
    assert_error_all_caps(&map, ScbErrorCode::FloatNonCanonical);
    let ConstData::Map(entries) = &mut map.data else {
        unreachable!()
    };
    entries[1].key = entries[0].key.clone();
    // The old map encoder checks key order before that entry's value.
    assert_error_all_caps(&map, ScbErrorCode::MapDuplicate);
    let ConstData::Map(entries) = &mut map.data else {
        unreachable!()
    };
    entries[1].key = invalid;
    assert_error_all_caps(&map, ScbErrorCode::FloatNonCanonical);

    let mut function = FunctionType {
        parameters: vec![],
        result: Box::new(TypeExpr::Unit),
        effects: vec![EntityId::from_bytes([2; 32]), EntityId::from_bytes([1; 32])],
    };
    assert_error_all_caps(
        &ConstValue {
            value_type: TypeExpr::FunctionRef(function.clone()),
            data: ConstData::Unit,
        },
        ScbErrorCode::MapOrder,
    );
    function.effects[1] = function.effects[0];
    assert_error_all_caps(
        &ConstValue {
            value_type: TypeExpr::FunctionRef(function),
            data: ConstData::Unit,
        },
        ScbErrorCode::MapDuplicate,
    );
}

#[test]
fn original_depth_and_enclosing_codec_depth_are_preserved() {
    let unit = value(ConstData::Unit);
    let encoded = encode_const_value(&unit).unwrap();
    assert_eq!(
        encode_at_depth(&unit, MAX_NESTING_DEPTH - 2).unwrap(),
        encoded
    );
    assert_eq!(
        encode_at_depth(&unit, MAX_NESTING_DEPTH - 1)
            .unwrap_err()
            .code(),
        ScbErrorCode::ResourceLimit
    );
    let mut nested = TypeExpr::Unit;
    for _ in 0..MAX_NESTING_DEPTH {
        nested = TypeExpr::Option(Box::new(nested));
    }
    assert_error_all_caps(
        &ConstValue {
            value_type: nested,
            data: ConstData::Unit,
        },
        ScbErrorCode::ResourceLimit,
    );
}

#[test]
fn codec_hard_limits_are_not_output_refusals() {
    assert_error_all_caps(
        &value(ConstData::Bytes(vec![0; MAX_BYTE_PAYLOAD + 1])),
        ScbErrorCode::ResourceLimit,
    );
    let too_many = ConstValue {
        value_type: TypeExpr::Tuple(vec![
            TypeExpr::Unit;
            usize::try_from(MAX_COLLECTION_ELEMENTS).unwrap() + 1
        ]),
        data: ConstData::Unit,
    };
    assert_error_all_caps(&too_many, ScbErrorCode::ResourceLimit);
    drop(too_many);
    // Each payload is legal; their containing value exceeds the standalone limit.
    let too_large = value(ConstData::Sequence(
        (0..5)
            .map(|_| value(ConstData::Bytes(vec![0; MAX_BYTE_PAYLOAD])))
            .collect(),
    ));
    assert_error_all_caps(&too_large, ScbErrorCode::ResourceLimit);
}

#[test]
fn large_payload_and_wide_values_use_borrowed_chunks_and_fixed_cursor_storage() {
    let bytes = value(ConstData::Bytes(vec![0x55; 2 * 1024 * 1024]));
    assert_eq!(
        measure_const_value_bounded(&bytes, 0).unwrap(),
        ConstValueByteMeasure::OverLimit
    );
    let ConstData::Bytes(payload) = &bytes.data else {
        unreachable!()
    };
    let mut cursor = WireCursor::new(Node::Value(&bytes), 0).unwrap();
    let capacity = cursor.stack.capacity();
    let mut borrowed_payload = false;
    while let Some(chunk) = cursor.next().unwrap() {
        if let Chunk::Borrowed(span) = chunk
            && span.len() == payload.len()
        {
            assert_eq!(span.as_ptr(), payload.as_ptr());
            borrowed_payload = true;
        }
        assert_eq!(cursor.stack.capacity(), capacity);
    }
    assert!(borrowed_payload);
    let wide = value(ConstData::Sequence(vec![value(ConstData::Unit); 16_384]));
    assert_eq!(
        measure_const_value_bounded(&wide, 0).unwrap(),
        ConstValueByteMeasure::OverLimit
    );
    let mut cursor = WireCursor::new(Node::Value(&wide), 0).unwrap();
    let mut peak = 0;
    while cursor.next().unwrap().is_some() {
        peak = peak.max(cursor.stack.len());
        assert_eq!(cursor.stack.capacity(), capacity);
    }
    assert!(peak < 20, "width must not grow the cursor stack: {peak}");
    let map = text_map(vec![
        "a".repeat(2 * 1024 * 1024),
        "b".repeat(2 * 1024 * 1024),
    ]);
    assert_eq!(
        measure_const_value_bounded(&map, 0).unwrap(),
        ConstValueByteMeasure::OverLimit
    );
}

#[test]
fn nested_map_keys_share_canonical_wire_order() {
    let a = text_map(vec!["a".into(), "b".into()]);
    let b = text_map(vec!["a".into(), "c".into()]);
    let mut entries = vec![
        MapEntryConst {
            key: a,
            value: value(ConstData::Unit),
        },
        MapEntryConst {
            key: b,
            value: value(ConstData::Unit),
        },
    ];
    entries.sort_by_key(|entry| encode_const_value(&entry.key).unwrap());
    assert_caps(&value(ConstData::Map(entries.clone())));
    entries.reverse();
    assert_error_all_caps(&value(ConstData::Map(entries)), ScbErrorCode::MapOrder);
}
