//! Native parity fixtures for a bounded `Operation` (entity kind 8) codec.

use super::*;

fn operation_stored(
    entity: [u8; 32],
    block: [u8; 32],
    ordinal: u32,
    opcode: u32,
    immediate: Immediate,
) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, OperationBody};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::Operation(OperationBody {
            block: sley_id::EntityId::from_bytes(block),
            ordinal,
            opcode,
            operands: Vec::new(),
            result_types: Vec::new(),
            immediate,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds Operation fixture")
        .stored_bytes()
        .to_vec()
}

fn build_operation_body_check(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let mut template = vec![8, 51, 6, 1, 32];
    template.extend_from_slice(&[0; 32]);
    template.extend_from_slice(&[2, 1, 0, 3, 1, 1, 4, 1, 0, 5, 1, 0, 6, 2, 1, 0]);
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(5, 37), (53, 53)],
    )
}

#[test]
fn codec_operation_constant_ref_empty_profile_round_trips_native_bytes() {
    let stored = operation_stored(
        [0x81; 32],
        [0x82; 32],
        0,
        Opcode::ConstantRef.tag(),
        Immediate::None,
    );
    let body = ns_body_of(&stored);
    let mut expected = vec![8, 51, 6, 1, 32];
    expected.extend_from_slice(&[0x82; 32]);
    expected.extend_from_slice(&[2, 1, 0, 3, 1, 1, 4, 1, 0, 5, 1, 0, 6, 2, 1, 0]);
    assert_eq!(body, expected);

    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_operation_body_check,
    ));
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = super::workspace::decoded_fixed_body(&decoded, "Operation");
    assert_eq!(entity, vec![0x81; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "OPERATION_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_operation_body_check,
        90,
        53,
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
        "OPERATION_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_operation_profile_rejects_other_shapes_and_short_entity() {
    let accepted = operation_stored(
        [0x81; 32],
        [0x82; 32],
        0,
        Opcode::ConstantRef.tag(),
        Immediate::None,
    );
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_operation_body_check,
    ));
    for (name, ordinal, opcode, immediate) in [
        ("ordinal_one", 1, Opcode::ConstantRef.tag(), Immediate::None),
        ("tuple_new", 0, Opcode::TupleNew.tag(), Immediate::None),
        (
            "index_immediate",
            0,
            Opcode::ConstantRef.tag(),
            Immediate::Index(0),
        ),
    ] {
        let stored = operation_stored([0x81; 32], [0x82; 32], ordinal, opcode, immediate);
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "OPERATION_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_operation_body_check,
        90,
        53,
    ));
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0x81; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
