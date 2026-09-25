//! Native parity fixtures for a bounded `CapabilityRequirement` (entity kind
//! 12) program-codec profile.

use super::*;

fn capability_requirement_stored(
    entity: [u8; 32],
    effect: [u8; 32],
    allowed_scopes: Vec<ConstValue>,
    constraint_contracts: &[[u8; 32]],
) -> Vec<u8> {
    use sley_mutate::value::{CapabilityRequirementBody, EntityBodyValue, EntityIdSet};

    let constraint_contracts = EntityIdSet::from_unsorted(
        constraint_contracts
            .iter()
            .copied()
            .map(sley_id::EntityId::from_bytes)
            .collect(),
    )
    .expect("fixture contracts are unique");
    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::CapabilityRequirement(CapabilityRequirementBody {
            effect: sley_id::EntityId::from_bytes(effect),
            allowed_scopes,
            constraint_contracts,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds CapabilityRequirement fixture")
        .stored_bytes()
        .to_vec()
}

fn unit_constant() -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    }
}

fn build_capability_requirement_body_check(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
) -> FunctionGraph {
    let mut template = vec![12, 41, 3, 1, 32];
    template.extend_from_slice(&[0; 32]);
    template.extend_from_slice(&[2, 1, 0, 3, 1, 0]);
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(5, 37), (43, 43)],
    )
}

fn decoded_capability_requirement(outcome: &sley_vm::ExecutionOutcome) -> (Vec<u8>, Vec<u8>) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "CapabilityRequirement decoder must return: {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &value.data else {
        panic!(
            "CapabilityRequirement decoder must succeed: {:?}",
            value.data
        )
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!("CapabilityRequirement decoder returns an entity/body tuple")
    };
    let [entity, body] = fields.as_slice() else {
        panic!("CapabilityRequirement decoder returns exactly two fields")
    };
    let ConstData::Bytes(entity) = &entity.data else {
        panic!("CapabilityRequirement entity is Bytes")
    };
    let ConstData::Bytes(body) = &body.data else {
        panic!("CapabilityRequirement body is Bytes")
    };
    (entity.clone(), body.clone())
}

#[test]
fn codec_capability_requirement_empty_profile_round_trips_native_bytes() {
    let stored = capability_requirement_stored([0xf1; 32], [0xf2; 32], Vec::new(), &[]);
    let body = ns_body_of(&stored);
    let mut expected_body = vec![12, 41, 3, 1, 32];
    expected_body.extend_from_slice(&[0xf2; 32]);
    expected_body.extend_from_slice(&[2, 1, 0, 3, 1, 0]);
    assert_eq!(body, expected_body);

    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_capability_requirement_body_check,
    ));
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = decoded_capability_requirement(&decoded);
    assert_eq!(entity, vec![0xf1; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "CAPABILITY_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_capability_requirement_body_check,
        80,
        43,
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
        "CAPABILITY_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_capability_requirement_profile_rejects_nonempty_sets_and_short_entity() {
    let accepted = capability_requirement_stored([0xf1; 32], [0xf2; 32], Vec::new(), &[]);
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_capability_requirement_body_check,
    ));
    for (name, scopes, contracts) in [
        ("one_scope", vec![unit_constant()], Vec::new()),
        ("one_contract", Vec::new(), vec![[0xf3; 32]]),
    ] {
        let stored = capability_requirement_stored([0xf1; 32], [0xf2; 32], scopes, &contracts);
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "CAPABILITY_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_capability_requirement_body_check,
        80,
        43,
    ));
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0xf1; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
