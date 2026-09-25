//! Native parity fixtures for a bounded `AdapterImport` (entity kind 15)
//! program-codec profile.

use super::*;

fn adapter_import_stored(
    entity: [u8; 32],
    adapter_id: [u8; 32],
    abi_version: u32,
    request_type: TypeExpr,
    response_type: TypeExpr,
    failure_type: TypeExpr,
    effects: &[[u8; 32]],
) -> Vec<u8> {
    use sley_mutate::value::{AdapterImportBody, EntityBodyValue, EntityIdSet};

    let effects = EntityIdSet::from_unsorted(
        effects
            .iter()
            .copied()
            .map(sley_id::EntityId::from_bytes)
            .collect(),
    )
    .expect("fixture effects are unique");
    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::AdapterImport(AdapterImportBody {
            adapter_id,
            abi_version,
            request_type,
            response_type,
            failure_type,
            effects,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds AdapterImport fixture")
        .stored_bytes()
        .to_vec()
}

fn build_adapter_import_body_check(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
) -> FunctionGraph {
    let mut template = vec![15, 53, 6, 1, 32];
    template.extend_from_slice(&[0; 32]);
    template.extend_from_slice(&[2, 1, 1, 3, 2, 1, 0, 4, 2, 1, 0, 5, 2, 1, 0, 6, 1, 0]);
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(5, 37), (55, 55)],
    )
}

fn decoded_adapter_import(outcome: &sley_vm::ExecutionOutcome) -> (Vec<u8>, Vec<u8>) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "AdapterImport decoder must return: {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &value.data else {
        panic!("AdapterImport decoder must succeed: {:?}", value.data)
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!("AdapterImport decoder returns an entity/body tuple")
    };
    let [entity, body] = fields.as_slice() else {
        panic!("AdapterImport decoder returns exactly two fields")
    };
    let ConstData::Bytes(entity) = &entity.data else {
        panic!("AdapterImport entity is Bytes")
    };
    let ConstData::Bytes(body) = &body.data else {
        panic!("AdapterImport body is Bytes")
    };
    (entity.clone(), body.clone())
}

#[test]
fn codec_adapter_import_unit_empty_profile_round_trips_native_bytes() {
    let stored = adapter_import_stored(
        [0xe1; 32],
        [0xe2; 32],
        1,
        TypeExpr::Unit,
        TypeExpr::Unit,
        TypeExpr::Unit,
        &[],
    );
    let body = ns_body_of(&stored);
    let mut expected_body = vec![15, 53, 6, 1, 32];
    expected_body.extend_from_slice(&[0xe2; 32]);
    expected_body.extend_from_slice(&[2, 1, 1, 3, 2, 1, 0, 4, 2, 1, 0, 5, 2, 1, 0, 6, 1, 0]);
    assert_eq!(body, expected_body);

    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_adapter_import_body_check,
    ));
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = decoded_adapter_import(&decoded);
    assert_eq!(entity, vec![0xe1; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "ADAPTER_IMPORT_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_adapter_import_body_check,
        92,
        55,
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
        "ADAPTER_IMPORT_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_adapter_import_profile_rejects_other_shapes_and_short_entity() {
    let accepted = adapter_import_stored(
        [0xe1; 32],
        [0xe2; 32],
        1,
        TypeExpr::Unit,
        TypeExpr::Unit,
        TypeExpr::Unit,
        &[],
    );
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_adapter_import_body_check,
    ));
    for (name, abi_version, request_type, effects) in [
        ("abi_two", 2, TypeExpr::Unit, Vec::new()),
        ("bool_request", 1, TypeExpr::Bool, Vec::new()),
        ("one_effect", 1, TypeExpr::Unit, vec![[0xe3; 32]]),
    ] {
        let stored = adapter_import_stored(
            [0xe1; 32],
            [0xe2; 32],
            abi_version,
            request_type,
            TypeExpr::Unit,
            TypeExpr::Unit,
            &effects,
        );
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "ADAPTER_IMPORT_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_adapter_import_body_check,
        92,
        55,
    ));
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0xe1; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
