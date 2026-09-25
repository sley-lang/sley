//! Native parity fixtures for a bounded `Contract` (entity kind 13) codec.

use super::*;

fn limits(value: u64) -> sley_ssmc::ResourceLimits {
    sley_ssmc::ResourceLimits {
        fuel: value,
        memory_bytes: value,
        output_bytes: value,
        effect_count: value,
        call_depth: value,
        wall_timeout_millis: value,
    }
}

fn contract_stored(
    entity: [u8; 32],
    target: [u8; 32],
    kind: sley_ssmc::ContractKind,
    predicate: [u8; 32],
    resource_limits: Option<sley_ssmc::ResourceLimits>,
) -> Vec<u8> {
    use sley_mutate::value::{ContractBody, EntityBodyValue};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::Contract(ContractBody {
            target: sley_id::EntityId::from_bytes(target),
            contract_kind: kind,
            predicate: sley_id::EntityId::from_bytes(predicate),
            bindings: Vec::new(),
            resource_limits,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds Contract fixture")
        .stored_bytes()
        .to_vec()
}

fn build_contract_body_check(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let mut template = vec![13, 75, 4, 1, 32];
    template.extend_from_slice(&[0; 32]);
    template.extend_from_slice(&[2, 1, 1, 3, 32]);
    template.extend_from_slice(&[0; 32]);
    template.extend_from_slice(&[4, 1, 0]);
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(5, 37), (42, 74)],
    )
}

#[test]
fn codec_contract_precondition_empty_profile_round_trips_native_bytes() {
    let stored = contract_stored(
        [0xd1; 32],
        [0xd2; 32],
        sley_ssmc::ContractKind::Precondition,
        [0xd3; 32],
        None,
    );
    let body = ns_body_of(&stored);
    let mut expected = vec![13, 75, 4, 1, 32];
    expected.extend_from_slice(&[0xd2; 32]);
    expected.extend_from_slice(&[2, 1, 1, 3, 32]);
    expected.extend_from_slice(&[0xd3; 32]);
    expected.extend_from_slice(&[4, 1, 0]);
    assert_eq!(body, expected);

    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_contract_body_check,
    ));
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = super::workspace::decoded_fixed_body(&decoded, "Contract");
    assert_eq!(entity, vec![0xd1; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "CONTRACT_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_contract_body_check,
        114,
        77,
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
        "CONTRACT_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_contract_profile_rejects_other_shapes_and_short_entity() {
    let accepted = contract_stored(
        [0xd1; 32],
        [0xd2; 32],
        sley_ssmc::ContractKind::Precondition,
        [0xd3; 32],
        None,
    );
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_contract_body_check,
    ));
    for (name, kind, resource_limits) in [
        (
            "postcondition",
            sley_ssmc::ContractKind::Postcondition,
            None,
        ),
        (
            "resource_limits",
            sley_ssmc::ContractKind::Precondition,
            Some(limits(1)),
        ),
    ] {
        let stored = contract_stored([0xd1; 32], [0xd2; 32], kind, [0xd3; 32], resource_limits);
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "CONTRACT_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_contract_body_check,
        114,
        77,
    ));
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0xd1; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
