//! Native parity fixtures for a bounded `Block` (entity kind 7) codec.

use super::*;

fn block_stored(
    entity: [u8; 32],
    owner: [u8; 32],
    operations: &[[u8; 32]],
    trap: TrapCode,
    reachability: Reachability,
) -> Vec<u8> {
    use sley_mutate::value::{BlockBody, EntityBodyValue};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::Block(BlockBody {
            function: sley_id::EntityId::from_bytes(owner),
            parameters: Vec::new(),
            operations: operations
                .iter()
                .copied()
                .map(sley_id::EntityId::from_bytes)
                .collect(),
            terminator: Terminator::Trap(TrapTerminator {
                code: trap,
                payload: None,
            }),
            reachability,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds Block fixture")
        .stored_bytes()
        .to_vec()
}

fn build_block_body_check(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let mut template = vec![7, 56, 5, 1, 32];
    template.extend_from_slice(&[0; 32]);
    template.extend_from_slice(&[
        2, 1, 0, 3, 1, 0, 4, 10, 5, 8, 2, 1, 1, 1, 2, 2, 0, 0, 5, 1, 1,
    ]);
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(5, 37), (58, 58)],
    )
}

#[test]
fn codec_block_empty_unreachable_trap_profile_round_trips_native_bytes() {
    let stored = block_stored(
        [0x71; 32],
        [0x72; 32],
        &[],
        TrapCode::Unreachable,
        Reachability::Required,
    );
    let body = ns_body_of(&stored);
    let mut expected = vec![7, 56, 5, 1, 32];
    expected.extend_from_slice(&[0x72; 32]);
    expected.extend_from_slice(&[
        2, 1, 0, 3, 1, 0, 4, 10, 5, 8, 2, 1, 1, 1, 2, 2, 0, 0, 5, 1, 1,
    ]);
    assert_eq!(body, expected);

    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_block_body_check,
    ));
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = super::workspace::decoded_fixed_body(&decoded, "Block");
    assert_eq!(entity, vec![0x71; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "BLOCK_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_block_body_check,
        95,
        58,
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
        "BLOCK_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_block_profile_rejects_other_shapes_and_short_entity() {
    let accepted = block_stored(
        [0x71; 32],
        [0x72; 32],
        &[],
        TrapCode::Unreachable,
        Reachability::Required,
    );
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_block_body_check,
    ));
    for (name, operations, trap, reachability) in [
        (
            "one_operation",
            vec![[0x73; 32]],
            TrapCode::Unreachable,
            Reachability::Required,
        ),
        (
            "resource_trap",
            Vec::new(),
            TrapCode::ResourceExhausted,
            Reachability::Required,
        ),
        (
            "explicitly_unreachable",
            Vec::new(),
            TrapCode::Unreachable,
            Reachability::ExplicitlyUnreachable,
        ),
    ] {
        let stored = block_stored([0x71; 32], [0x72; 32], &operations, trap, reachability);
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "BLOCK_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_block_body_check,
        95,
        58,
    ));
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0x71; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
