//! Native parity fixtures for a bounded `TestCase` (entity kind 14) codec.

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

fn test_case_stored(
    entity: [u8; 32],
    target: [u8; 32],
    inputs: Vec<ConstValue>,
    environment: sley_ssmc::EffectEnvironment,
    expected: sley_ssmc::ExpectedOutcome,
    resource_limits: sley_ssmc::ResourceLimits,
) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, TestCaseBody};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::TestCase(TestCaseBody {
            target: sley_id::EntityId::from_bytes(target),
            inputs,
            effect_environment: environment,
            expected,
            observations: Vec::new(),
            resource_limits,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds TestCase fixture")
        .stored_bytes()
        .to_vec()
}

fn unit_constant() -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    }
}

fn unit_expected() -> sley_ssmc::ExpectedOutcome {
    sley_ssmc::ExpectedOutcome::Value(unit_constant())
}

fn build_test_case_body_check(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let mut template = vec![14, 80, 6, 1, 32];
    template.extend_from_slice(&[0; 32]);
    template.extend_from_slice(&[
        2, 1, 0, 3, 3, 1, 1, 0, 4, 11, 1, 9, 2, 1, 2, 1, 0, 2, 2, 1, 0, 5, 1, 0, 6, 19, 6, 1, 1, 0,
        2, 1, 0, 3, 1, 0, 4, 1, 0, 5, 1, 0, 6, 1, 0,
    ]);
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(5, 37), (82, 82)],
    )
}

#[test]
fn codec_test_case_empty_unit_zero_limits_profile_round_trips_native_bytes() {
    let stored = test_case_stored(
        [0xe1; 32],
        [0xe2; 32],
        Vec::new(),
        sley_ssmc::EffectEnvironment::Replay(Vec::new()),
        unit_expected(),
        limits(0),
    );
    let body = ns_body_of(&stored);
    let mut expected = vec![14, 80, 6, 1, 32];
    expected.extend_from_slice(&[0xe2; 32]);
    expected.extend_from_slice(&[
        2, 1, 0, 3, 3, 1, 1, 0, 4, 11, 1, 9, 2, 1, 2, 1, 0, 2, 2, 1, 0, 5, 1, 0, 6, 19, 6, 1, 1, 0,
        2, 1, 0, 3, 1, 0, 4, 1, 0, 5, 1, 0, 6, 1, 0,
    ]);
    assert_eq!(body, expected);

    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_test_case_body_check,
    ));
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = super::workspace::decoded_fixed_body(&decoded, "TestCase");
    assert_eq!(entity, vec![0xe1; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "TEST_CASE_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_test_case_body_check,
        119,
        82,
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
        "TEST_CASE_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_test_case_profile_rejects_other_shapes_and_short_entity() {
    let accepted = test_case_stored(
        [0xe1; 32],
        [0xe2; 32],
        Vec::new(),
        sley_ssmc::EffectEnvironment::Replay(Vec::new()),
        unit_expected(),
        limits(0),
    );
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_test_case_body_check,
    ));
    for (name, inputs, environment, expected, resource_limits) in [
        (
            "one_input",
            vec![unit_constant()],
            sley_ssmc::EffectEnvironment::Replay(Vec::new()),
            unit_expected(),
            limits(0),
        ),
        (
            "deterministic_environment",
            Vec::new(),
            sley_ssmc::EffectEnvironment::DeterministicAdapters(Vec::new()),
            unit_expected(),
            limits(0),
        ),
        (
            "failure_expected",
            Vec::new(),
            sley_ssmc::EffectEnvironment::Replay(Vec::new()),
            sley_ssmc::ExpectedOutcome::FailureCode(1),
            limits(0),
        ),
        (
            "nonzero_limits",
            Vec::new(),
            sley_ssmc::EffectEnvironment::Replay(Vec::new()),
            unit_expected(),
            limits(1),
        ),
    ] {
        let stored = test_case_stored(
            [0xe1; 32],
            [0xe2; 32],
            inputs,
            environment,
            expected,
            resource_limits,
        );
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "TEST_CASE_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_test_case_body_check,
        119,
        82,
    ));
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0xe1; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
