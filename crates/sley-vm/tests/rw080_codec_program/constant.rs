//! Native parity fixtures for a bounded `Constant` (entity kind 9)
//! program-codec profile.

use super::*;

fn constant_stored(entity: [u8; 32], value: ConstValue) -> Vec<u8> {
    use sley_mutate::value::{ConstantBody, EntityBodyValue};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::Constant(ConstantBody { value }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds Constant fixture")
        .stored_bytes()
        .to_vec()
}

fn unit_constant() -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    }
}

fn bool_constant(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

fn build_constant_body_check(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let template = vec![9, 12, 1, 1, 9, 2, 1, 2, 1, 0, 2, 2, 1, 0];
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(14, 14), (14, 14)],
    )
}

fn decoded_constant(outcome: &sley_vm::ExecutionOutcome) -> (Vec<u8>, Vec<u8>) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("Constant decoder must return: {:?}", outcome.termination)
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &value.data else {
        panic!("Constant decoder must succeed: {:?}", value.data)
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!("Constant decoder returns an entity/body tuple")
    };
    let [entity, body] = fields.as_slice() else {
        panic!("Constant decoder returns exactly two fields")
    };
    let ConstData::Bytes(entity) = &entity.data else {
        panic!("Constant entity is Bytes")
    };
    let ConstData::Bytes(body) = &body.data else {
        panic!("Constant body is Bytes")
    };
    (entity.clone(), body.clone())
}

#[test]
fn codec_constant_unit_profile_round_trips_native_bytes() {
    let stored = constant_stored([0xd1; 32], unit_constant());
    let body = ns_body_of(&stored);
    assert_eq!(body, vec![9, 12, 1, 1, 9, 2, 1, 2, 1, 0, 2, 2, 1, 0]);

    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_constant_body_check,
    ));
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = decoded_constant(&decoded);
    assert_eq!(entity, vec![0xd1; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "CONSTANT_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_constant_body_check,
        51,
        14,
    ));
    let encoded = execute(
        &encode_package,
        &encode_approved,
        vec![
            bytes_input(&entity),
            bytes_input(&decoded_body),
            unit_input(),
        ],
    );
    assert_encode_ok(&encoded, &stored);
    eprintln!(
        "CONSTANT_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_constant_profile_rejects_other_values_and_short_entity() {
    let accepted = constant_stored([0xd1; 32], unit_constant());
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_constant_body_check,
    ));
    for (name, value) in [
        ("bool_false", bool_constant(false)),
        ("bool_true", bool_constant(true)),
    ] {
        let stored = constant_stored([0xd1; 32], value);
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "CONSTANT_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_constant_body_check,
        51,
        14,
    ));
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0xd1; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
