//! Native parity fixtures for a bounded `Parameter` (entity kind 6)
//! program-codec profile.

use super::*;

fn parameter_stored(
    entity: [u8; 32],
    owner: [u8; 32],
    role: ParameterRole,
    ordinal: u32,
    value_type: TypeExpr,
) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, ParameterBody};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::Parameter(ParameterBody {
            owner: sley_id::EntityId::from_bytes(owner),
            role,
            ordinal,
            value_type,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds Parameter fixture")
        .stored_bytes()
        .to_vec()
}

fn build_parameter_body_check(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let mut template = vec![6, 45, 4, 1, 32];
    template.extend_from_slice(&[0; 32]);
    template.extend_from_slice(&[2, 1, 1, 3, 1, 0, 4, 2, 1, 0]);
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(5, 37), (47, 47)],
    )
}

fn decoded_parameter(outcome: &sley_vm::ExecutionOutcome) -> (Vec<u8>, Vec<u8>) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("Parameter decoder must return: {:?}", outcome.termination)
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &value.data else {
        panic!("Parameter decoder must succeed: {:?}", value.data)
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!("Parameter decoder returns an entity/body tuple")
    };
    let [entity, body] = fields.as_slice() else {
        panic!("Parameter decoder returns exactly two fields")
    };
    let ConstData::Bytes(entity) = &entity.data else {
        panic!("Parameter entity is Bytes")
    };
    let ConstData::Bytes(body) = &body.data else {
        panic!("Parameter body is Bytes")
    };
    (entity.clone(), body.clone())
}

#[test]
fn codec_parameter_function_zero_unit_profile_round_trips_native_bytes() {
    let stored = parameter_stored(
        [0xb1; 32],
        [0xb2; 32],
        ParameterRole::Function,
        0,
        TypeExpr::Unit,
    );
    let body = ns_body_of(&stored);
    let mut expected_body = vec![6, 45, 4, 1, 32];
    expected_body.extend_from_slice(&[0xb2; 32]);
    expected_body.extend_from_slice(&[2, 1, 1, 3, 1, 0, 4, 2, 1, 0]);
    assert_eq!(body, expected_body);

    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_parameter_body_check,
    ));
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = decoded_parameter(&decoded);
    assert_eq!(entity, vec![0xb1; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "PARAMETER_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_parameter_body_check,
        84,
        47,
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
        "PARAMETER_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_parameter_profile_rejects_other_shapes_and_short_entity() {
    let accepted = parameter_stored(
        [0xb1; 32],
        [0xb2; 32],
        ParameterRole::Function,
        0,
        TypeExpr::Unit,
    );
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_parameter_body_check,
    ));
    for (name, role, ordinal, value_type) in [
        ("block_role", ParameterRole::Block, 0, TypeExpr::Unit),
        ("ordinal_one", ParameterRole::Function, 1, TypeExpr::Unit),
        ("bool_type", ParameterRole::Function, 0, TypeExpr::Bool),
    ] {
        let stored = parameter_stored([0xb1; 32], [0xb2; 32], role, ordinal, value_type);
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "PARAMETER_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_parameter_body_check,
        84,
        47,
    ));
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0xb1; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
