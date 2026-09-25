//! Native parity fixtures for a bounded `GlobalValue` (entity kind 10)
//! program-codec profile.

use super::*;

fn global_value_stored(
    entity: [u8; 32],
    initializer: [u8; 32],
    value_type: TypeExpr,
    visibility: Visibility,
) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, GlobalValueBody};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::GlobalValue(GlobalValueBody {
            value_type,
            initializer: sley_id::EntityId::from_bytes(initializer),
            visibility,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds GlobalValue fixture")
        .stored_bytes()
        .to_vec()
}

fn build_global_value_body_check(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let mut template = vec![10, 42, 3, 1, 2, 1, 0, 2, 32];
    template.extend_from_slice(&[0; 32]);
    template.extend_from_slice(&[3, 1, 1]);
    super::dependency_binding_decode::build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &template,
        [(9, 41), (44, 44)],
    )
}

fn global_value_decode_image() -> Image {
    let mut assembler = Asm::new();
    let root_ns = Ns {
        k: 234,
        p: 235,
        b: 236,
        o: 237,
    };
    let validate_ns = Ns {
        k: 238,
        p: 239,
        b: 240,
        o: 241,
    };
    let outer_ns = Ns {
        k: 242,
        p: 243,
        b: 244,
        o: 245,
    };
    let checker_ns = Ns {
        k: 246,
        p: 247,
        b: 248,
        o: 249,
    };
    let uvar_ns = Ns {
        k: 250,
        p: 251,
        b: 252,
        o: 253,
    };
    let root = eid(13, 1);
    let validate = eid(13, 2);
    let outer = eid(13, 3);
    let checker = eid(13, 4);
    let uvar = eid(13, 5);

    let (uvar_graph, _) = build_decode(&mut assembler, uvar_ns, uvar);
    let validate_graph = build_program_validate(&mut assembler, validate_ns, validate, uvar);
    let outer_graph = build_outer_decode(&mut assembler, outer_ns, outer, uvar);
    let checker_graph = build_global_value_body_check(&mut assembler, checker_ns, checker);
    let root_graph = super::workspace::build_fixed_body_program_decode(
        &mut assembler,
        root_ns,
        root,
        validate,
        outer,
        checker,
    );
    let mut image = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: root_graph.clone(),
        functions: vec![
            root_graph,
            validate_graph,
            outer_graph,
            checker_graph,
            uvar_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: vec![
            frozen_import(
                sley_vm::host_abi::BRIDGE_CODE_B2V1,
                TypeExpr::Bytes,
                u8vec_type(),
            ),
            frozen_import(sley_vm::host_abi::BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(
                sley_vm::host_abi::BRIDGE_CODE_V2B1,
                u8vec_type(),
                TypeExpr::Bytes,
            ),
            frozen_import(
                sley_vm::host_abi::BRIDGE_CODE_RHW1,
                TypeExpr::Bytes,
                TypeExpr::Bytes,
            ),
        ],
        constants: assembler.constants,
    };
    assert!(
        super::supported_dispatch::deduplicate_identical_constants(&mut image) > 0,
        "GlobalValue decode closure shares immutable constants"
    );
    image
}

fn global_value_encode_image() -> Image {
    let mut assembler = Asm::new();
    let root_ns = Ns {
        k: 234,
        p: 235,
        b: 236,
        o: 237,
    };
    let checker_ns = Ns {
        k: 238,
        p: 239,
        b: 240,
        o: 241,
    };
    let witness_ns = Ns {
        k: 242,
        p: 243,
        b: 244,
        o: 245,
    };
    let exact_ns = Ns {
        k: 246,
        p: 247,
        b: 248,
        o: 249,
    };
    let concat_ns = Ns {
        k: 250,
        p: 251,
        b: 252,
        o: 253,
    };
    let root = eid(14, 1);
    let checker = eid(14, 2);
    let witness = eid(14, 3);
    let exact = eid(14, 4);
    let concat = eid(14, 5);

    let checker_graph = build_global_value_body_check(&mut assembler, checker_ns, checker);
    let concat_graph = super::package::build_concat_bytes(&mut assembler, concat_ns, concat);
    let exact_graph =
        super::package::build_exact_identity_validate(&mut assembler, exact_ns, exact);
    let witness_graph = super::package::build_single_fixed_body_witness_program_encode(
        &mut assembler,
        witness_ns,
        witness,
        exact,
        concat,
        81,
        44,
    );
    let root_graph = super::workspace::build_fixed_body_program_encode(
        &mut assembler,
        root_ns,
        root,
        checker,
        witness,
    );
    let mut image = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: root_graph.clone(),
        functions: vec![
            root_graph,
            witness_graph,
            exact_graph,
            concat_graph,
            checker_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: vec![
            frozen_import(
                sley_vm::host_abi::BRIDGE_CODE_B2V1,
                TypeExpr::Bytes,
                u8vec_type(),
            ),
            frozen_import(sley_vm::host_abi::BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(
                sley_vm::host_abi::BRIDGE_CODE_V2B1,
                u8vec_type(),
                TypeExpr::Bytes,
            ),
            frozen_import(
                sley_vm::host_abi::BRIDGE_CODE_RHW1,
                TypeExpr::Bytes,
                TypeExpr::Bytes,
            ),
        ],
        constants: assembler.constants,
    };
    assert!(
        super::supported_dispatch::deduplicate_identical_constants(&mut image) > 0,
        "GlobalValue encode closure shares immutable constants"
    );
    image
}

fn decoded_global_value(outcome: &sley_vm::ExecutionOutcome) -> (Vec<u8>, Vec<u8>) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("GlobalValue decoder must return: {:?}", outcome.termination)
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &value.data else {
        panic!("GlobalValue decoder must succeed: {:?}", value.data)
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!("GlobalValue decoder returns an entity/body tuple")
    };
    let [entity, body] = fields.as_slice() else {
        panic!("GlobalValue decoder returns exactly two fields")
    };
    let ConstData::Bytes(entity) = &entity.data else {
        panic!("GlobalValue entity is Bytes")
    };
    let ConstData::Bytes(body) = &body.data else {
        panic!("GlobalValue body is Bytes")
    };
    (entity.clone(), body.clone())
}

#[test]
fn codec_global_value_unit_private_profile_round_trips_native_bytes() {
    let stored = global_value_stored([0xa1; 32], [0xa2; 32], TypeExpr::Unit, Visibility::Private);
    let body = ns_body_of(&stored);
    let mut expected_body = vec![10, 42, 3, 1, 2, 1, 0, 2, 32];
    expected_body.extend_from_slice(&[0xa2; 32]);
    expected_body.extend_from_slice(&[3, 1, 1]);
    assert_eq!(body, expected_body);

    let (decode_package, decode_approved) = admit(&global_value_decode_image());
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, decoded_body) = decoded_global_value(&decoded);
    assert_eq!(entity, vec![0xa1; 32]);
    assert_eq!(decoded_body, body);
    eprintln!(
        "GLOBAL_VALUE_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&global_value_encode_image());
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
        "GLOBAL_VALUE_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_global_value_profile_rejects_other_shapes_and_short_entity() {
    let accepted = global_value_stored([0xa1; 32], [0xa2; 32], TypeExpr::Unit, Visibility::Private);
    let body = ns_body_of(&accepted);
    let (decode_package, decode_approved) = admit(&global_value_decode_image());
    for (name, value_type, visibility) in [
        ("bool_type", TypeExpr::Bool, Visibility::Private),
        ("exported", TypeExpr::Unit, Visibility::Exported),
    ] {
        let stored = global_value_stored([0xa1; 32], [0xa2; 32], value_type, visibility);
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "GLOBAL_VALUE_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let (encode_package, encode_approved) = admit(&global_value_encode_image());
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0xa1; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
