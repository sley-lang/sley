//! Native parity fixtures for a bounded `Function` (entity kind 5) codec.

use super::*;

fn function_stored(
    entity: [u8; 32],
    result_type: TypeExpr,
    entry_block: [u8; 32],
    blocks: &[[u8; 32]],
    visibility: Visibility,
) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, FunctionBody};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::Function(FunctionBody {
            type_parameters: Vec::new(),
            parameters: Vec::new(),
            result_type,
            effects: EntityIdSet::from_unsorted(Vec::new()).expect("empty set is canonical"),
            entry_block: sley_id::EntityId::from_bytes(entry_block),
            blocks: blocks
                .iter()
                .copied()
                .map(sley_id::EntityId::from_bytes)
                .collect(),
            contracts: EntityIdSet::from_unsorted(Vec::new()).expect("empty set is canonical"),
            visibility,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds Function fixture")
        .stored_bytes()
        .to_vec()
}

fn build_function_body_check(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let mut template = vec![5, 57, 8, 1, 1, 0, 2, 1, 0, 3, 2, 1, 0, 4, 1, 0, 5, 32];
    template.extend_from_slice(&[0; 32]);
    template.extend_from_slice(&[6, 1, 0, 7, 1, 0, 8, 1, 1]);
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(18, 50), (59, 59)],
    )
}

#[test]
fn codec_function_empty_unit_private_profile_round_trips_native_bytes() {
    let stored = function_stored(
        [0x51; 32],
        TypeExpr::Unit,
        [0x52; 32],
        &[],
        Visibility::Private,
    );
    let body = ns_body_of(&stored);
    let mut expected = vec![5, 57, 8, 1, 1, 0, 2, 1, 0, 3, 2, 1, 0, 4, 1, 0, 5, 32];
    expected.extend_from_slice(&[0x52; 32]);
    expected.extend_from_slice(&[6, 1, 0, 7, 1, 0, 8, 1, 1]);
    assert_eq!(body, expected);

    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_function_body_check,
    ));
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = super::workspace::decoded_fixed_body(&decoded, "Function");
    assert_eq!(entity, vec![0x51; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "FUNCTION_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_function_body_check,
        96,
        59,
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
        "FUNCTION_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_function_profile_rejects_other_shapes_and_short_entity() {
    let accepted = function_stored(
        [0x51; 32],
        TypeExpr::Unit,
        [0x52; 32],
        &[],
        Visibility::Private,
    );
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_function_body_check,
    ));
    for (name, result_type, blocks, visibility) in [
        (
            "bool_result",
            TypeExpr::Bool,
            Vec::new(),
            Visibility::Private,
        ),
        (
            "one_block",
            TypeExpr::Unit,
            vec![[0x53; 32]],
            Visibility::Private,
        ),
        ("exported", TypeExpr::Unit, Vec::new(), Visibility::Exported),
    ] {
        let stored = function_stored([0x51; 32], result_type, [0x52; 32], &blocks, visibility);
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "FUNCTION_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_function_body_check,
        96,
        59,
    ));
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0x51; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
