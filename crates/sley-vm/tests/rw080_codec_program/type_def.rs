//! Native parity fixtures for a bounded `TypeDef` (entity kind 4) codec.

use super::*;

fn type_def_stored(
    entity: [u8; 32],
    form: sley_ssmc::TypeDefForm,
    invariants: &[[u8; 32]],
    visibility: Visibility,
) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, TypeDefBody};

    let invariants = EntityIdSet::from_unsorted(
        invariants
            .iter()
            .copied()
            .map(sley_id::EntityId::from_bytes)
            .collect(),
    )
    .expect("fixture invariants are unique");
    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::TypeDef(TypeDefBody {
            type_parameters: Vec::new(),
            form,
            invariants,
            visibility,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds TypeDef fixture")
        .stored_bytes()
        .to_vec()
}

fn build_type_def_body_check(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let template = vec![4, 15, 4, 1, 1, 0, 2, 3, 1, 1, 0, 3, 1, 0, 4, 1, 1];
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(17, 17), (17, 17)],
    )
}

#[test]
fn codec_type_def_empty_record_private_profile_round_trips_native_bytes() {
    let stored = type_def_stored(
        [0x41; 32],
        sley_ssmc::TypeDefForm::Record(Vec::new()),
        &[],
        Visibility::Private,
    );
    let body = ns_body_of(&stored);
    assert_eq!(
        body,
        vec![4, 15, 4, 1, 1, 0, 2, 3, 1, 1, 0, 3, 1, 0, 4, 1, 1]
    );

    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_type_def_body_check,
    ));
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = super::workspace::decoded_fixed_body(&decoded, "TypeDef");
    assert_eq!(entity, vec![0x41; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "TYPE_DEF_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_type_def_body_check,
        54,
        17,
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
        "TYPE_DEF_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_type_def_profile_rejects_other_shapes_and_short_entity() {
    let accepted = type_def_stored(
        [0x41; 32],
        sley_ssmc::TypeDefForm::Record(Vec::new()),
        &[],
        Visibility::Private,
    );
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&super::workspace::fixed_body_decode_image(
        build_type_def_body_check,
    ));
    for (name, form, invariants, visibility) in [
        (
            "variant",
            sley_ssmc::TypeDefForm::Variant(Vec::new()),
            Vec::new(),
            Visibility::Private,
        ),
        (
            "one_invariant",
            sley_ssmc::TypeDefForm::Record(Vec::new()),
            vec![[0x42; 32]],
            Visibility::Private,
        ),
        (
            "exported",
            sley_ssmc::TypeDefForm::Record(Vec::new()),
            Vec::new(),
            Visibility::Exported,
        ),
    ] {
        let stored = type_def_stored([0x41; 32], form, &invariants, visibility);
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "TYPE_DEF_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&super::workspace::fixed_body_encode_image(
        build_type_def_body_check,
        54,
        17,
    ));
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0x41; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
