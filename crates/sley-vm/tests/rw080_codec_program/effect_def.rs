//! Native parity fixtures for a bounded `EffectDef` (entity kind 11)
//! program-codec profile.

use super::*;

fn effect_def_stored(
    entity: [u8; 32],
    effect_kind: sley_ssmc::EffectKind,
    scope_type: TypeExpr,
    request_type: TypeExpr,
    response_type: TypeExpr,
    failure_type: TypeExpr,
    visibility: Visibility,
) -> Vec<u8> {
    use sley_mutate::value::{EffectDefBody, EntityBodyValue};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::EffectDef(EffectDefBody {
            effect_kind,
            scope_type,
            request_type,
            response_type,
            failure_type,
            visibility,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds EffectDef fixture")
        .stored_bytes()
        .to_vec()
}

fn build_effect_def_body_check(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let template = vec![
        11, 23, 6, 1, 1, 8, 2, 2, 1, 0, 3, 2, 1, 0, 4, 2, 1, 0, 5, 2, 1, 0, 6, 1, 1,
    ];
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(25, 25), (25, 25)],
    )
}

fn decoded_effect_def(outcome: &sley_vm::ExecutionOutcome) -> (Vec<u8>, Vec<u8>) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("EffectDef decoder must return: {:?}", outcome.termination)
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &value.data else {
        panic!("EffectDef decoder must succeed: {:?}", value.data)
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!("EffectDef decoder returns an entity/body tuple")
    };
    let [entity, body] = fields.as_slice() else {
        panic!("EffectDef decoder returns exactly two fields")
    };
    let ConstData::Bytes(entity) = &entity.data else {
        panic!("EffectDef entity is Bytes")
    };
    let ConstData::Bytes(body) = &body.data else {
        panic!("EffectDef body is Bytes")
    };
    (entity.clone(), body.clone())
}

#[test]
fn codec_effect_def_adapter_unit_private_profile_round_trips_native_bytes() {
    let stored = effect_def_stored(
        [0xc1; 32],
        sley_ssmc::EffectKind::AdapterCall,
        TypeExpr::Unit,
        TypeExpr::Unit,
        TypeExpr::Unit,
        TypeExpr::Unit,
        Visibility::Private,
    );
    let body = ns_body_of(&stored);
    assert_eq!(
        body,
        vec![
            11, 23, 6, 1, 1, 8, 2, 2, 1, 0, 3, 2, 1, 0, 4, 2, 1, 0, 5, 2, 1, 0, 6, 1, 1,
        ]
    );

    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_effect_def_body_check,
    ));
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = decoded_effect_def(&decoded);
    assert_eq!(entity, vec![0xc1; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "EFFECT_DEF_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_effect_def_body_check,
        62,
        25,
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
        "EFFECT_DEF_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_effect_def_profile_rejects_other_shapes_and_short_entity() {
    let accepted = effect_def_stored(
        [0xc1; 32],
        sley_ssmc::EffectKind::AdapterCall,
        TypeExpr::Unit,
        TypeExpr::Unit,
        TypeExpr::Unit,
        TypeExpr::Unit,
        Visibility::Private,
    );
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_effect_def_body_check,
    ));
    for (name, effect_kind, request_type, visibility) in [
        (
            "stdout",
            sley_ssmc::EffectKind::StdoutWrite,
            TypeExpr::Unit,
            Visibility::Private,
        ),
        (
            "bool_request",
            sley_ssmc::EffectKind::AdapterCall,
            TypeExpr::Bool,
            Visibility::Private,
        ),
        (
            "exported",
            sley_ssmc::EffectKind::AdapterCall,
            TypeExpr::Unit,
            Visibility::Exported,
        ),
    ] {
        let stored = effect_def_stored(
            [0xc1; 32],
            effect_kind,
            TypeExpr::Unit,
            request_type,
            TypeExpr::Unit,
            TypeExpr::Unit,
            visibility,
        );
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "EFFECT_DEF_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_effect_def_body_check,
        62,
        25,
    ));
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0xc1; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
