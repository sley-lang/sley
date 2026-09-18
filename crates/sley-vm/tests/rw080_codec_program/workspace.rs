//! Native parity fixtures for the bounded `Workspace` (entity kind 1)
//! supported-dispatch profile.
//! Construction provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-workspace-program.md`.

use super::*;

pub(super) fn decoded_fixed_body(
    outcome: &sley_vm::ExecutionOutcome,
    kind: &str,
) -> (Vec<u8>, Vec<u8>) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("{kind} decoder must return: {:?}", outcome.termination)
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &value.data else {
        panic!("{kind} decoder must succeed: {:?}", value.data)
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!("{kind} decoder returns an entity/body tuple")
    };
    let [entity, body] = fields.as_slice() else {
        panic!("{kind} decoder returns exactly two fields")
    };
    let ConstData::Bytes(entity) = &entity.data else {
        panic!("{kind} entity is Bytes")
    };
    let ConstData::Bytes(body) = &body.data else {
        panic!("{kind} body is Bytes")
    };
    (entity.clone(), body.clone())
}

fn workspace_decode_result_type() -> TypeExpr {
    outer_decode_result_type()
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn build_fixed_body_program_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    validate_decoder: EntityId,
    outer_decoder: EntityId,
    body_checker: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = workspace_decode_result_type();
    let stored = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let entry = assembler.id(ns.b);
    let envelope_ok = assembler.id(ns.b);
    let outer_ok = assembler.id(ns.b);
    let return_ok = assembler.id(ns.b);
    let forward_error = assembler.id(ns.b);
    let scope_error = assembler.id(ns.b);
    let scope_code = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");

    let envelope = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(stored), pav(unit)],
        vec![encode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: validate_decoder,
            type_arguments: Vec::new(),
        }),
    );
    super::supported_dispatch::push_preallocated_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![envelope],
        switch(
            op_result(envelope),
            vec![
                (
                    BuiltinCase::Ok,
                    envelope_ok,
                    vec![SwitchArgument::CasePayload, sav(unit)],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let payload = assembler.param(ns.p, envelope_ok, ParameterRole::Block, TypeExpr::Bytes);
    let envelope_unit = assembler.param(ns.p, envelope_ok, ParameterRole::Block, TypeExpr::Unit);
    let outer = assembler.op(
        ns.o,
        envelope_ok,
        Opcode::CallDirect,
        vec![pav(payload), pav(envelope_unit)],
        vec![outer_decode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: outer_decoder,
            type_arguments: Vec::new(),
        }),
    );
    super::supported_dispatch::push_preallocated_block(
        assembler,
        envelope_ok,
        function,
        vec![payload, envelope_unit],
        vec![outer],
        switch(
            op_result(outer),
            vec![
                (
                    BuiltinCase::Ok,
                    outer_ok,
                    vec![SwitchArgument::CasePayload, sav(envelope_unit)],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let outer_tuple = assembler.param(
        ns.p,
        outer_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes]),
    );
    let outer_unit = assembler.param(ns.p, outer_ok, ParameterRole::Block, TypeExpr::Unit);
    let body = assembler.op(
        ns.o,
        outer_ok,
        Opcode::TupleGet,
        vec![pav(outer_tuple)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let valid = assembler.op(
        ns.o,
        outer_ok,
        Opcode::CallDirect,
        vec![op_result(body), pav(outer_unit)],
        vec![TypeExpr::Bool],
        Immediate::Function(FunctionRefValue {
            function: body_checker,
            type_arguments: Vec::new(),
        }),
    );
    super::supported_dispatch::push_preallocated_block(
        assembler,
        outer_ok,
        function,
        vec![outer_tuple, outer_unit],
        vec![body, valid],
        cond(
            op_result(valid),
            edge(return_ok, vec![pav(outer_tuple)]),
            edge(scope_error, Vec::new()),
        ),
    );

    let valid_outer = assembler.param(
        ns.p,
        return_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes]),
    );
    let ok = assembler.op(
        ns.o,
        return_ok,
        Opcode::ResultOk,
        vec![pav(valid_outer)],
        vec![result_type.clone()],
        Immediate::None,
    );
    super::supported_dispatch::push_preallocated_block(
        assembler,
        return_ok,
        function,
        vec![valid_outer],
        vec![ok],
        ret(op_result(ok)),
    );

    let forwarded = assembler.param(ns.p, forward_error, ParameterRole::Block, TypeExpr::Bytes);
    let error = assembler.op(
        ns.o,
        forward_error,
        Opcode::ResultErr,
        vec![pav(forwarded)],
        vec![result_type.clone()],
        Immediate::None,
    );
    super::supported_dispatch::push_preallocated_block(
        assembler,
        forward_error,
        function,
        vec![forwarded],
        vec![error],
        ret(op_result(error)),
    );

    let scope = assembler.cref(ns.o, scope_error, scope_code, TypeExpr::Bytes);
    let error = assembler.op(
        ns.o,
        scope_error,
        Opcode::ResultErr,
        vec![op_result(scope)],
        vec![result_type.clone()],
        Immediate::None,
    );
    super::supported_dispatch::push_preallocated_block(
        assembler,
        scope_error,
        function,
        Vec::new(),
        vec![scope, error],
        ret(op_result(error)),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![stored, unit],
        result_type,
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler.blocks[block_start..]
            .iter()
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

pub(super) fn build_fixed_body_program_encode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    body_checker: EntityId,
    witness_encoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = encode_result_type();
    let entity = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let entry = assembler.id(ns.b);
    let encode = assembler.id(ns.b);
    let refused = assembler.id(ns.b);
    let scope_code = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");

    let valid = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![TypeExpr::Bool],
        Immediate::Function(FunctionRefValue {
            function: body_checker,
            type_arguments: Vec::new(),
        }),
    );
    super::supported_dispatch::push_preallocated_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![valid],
        cond(
            op_result(valid),
            edge(encode, Vec::new()),
            edge(refused, Vec::new()),
        ),
    );

    let encoded = assembler.op(
        ns.o,
        encode,
        Opcode::CallDirect,
        vec![pav(entity), pav(body), pav(unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: witness_encoder,
            type_arguments: Vec::new(),
        }),
    );
    super::supported_dispatch::push_preallocated_block(
        assembler,
        encode,
        function,
        Vec::new(),
        vec![encoded],
        ret(op_result(encoded)),
    );

    let scope = assembler.cref(ns.o, refused, scope_code, TypeExpr::Bytes);
    let error = assembler.op(
        ns.o,
        refused,
        Opcode::ResultErr,
        vec![op_result(scope)],
        vec![result_type.clone()],
        Immediate::None,
    );
    super::supported_dispatch::push_preallocated_block(
        assembler,
        refused,
        function,
        Vec::new(),
        vec![scope, error],
        ret(op_result(error)),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![entity, body, unit],
        result_type,
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler.blocks[block_start..]
            .iter()
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

pub(super) fn workspace_stored(
    entity: [u8; 32],
    root_namespace: [u8; 32],
    packages: &[[u8; 32]],
    capability_requirements: &[[u8; 32]],
    contracts: &[[u8; 32]],
    tests: &[[u8; 32]],
) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, WorkspaceBody};

    let set = |members: &[[u8; 32]]| {
        EntityIdSet::from_unsorted(
            members
                .iter()
                .copied()
                .map(sley_id::EntityId::from_bytes)
                .collect(),
        )
        .expect("fixture identities are unique")
    };
    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::Workspace(WorkspaceBody {
            packages: set(packages),
            root_namespace: sley_id::EntityId::from_bytes(root_namespace),
            capability_requirements: set(capability_requirements),
            contracts: set(contracts),
            tests: set(tests),
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds Workspace fixture")
        .stored_bytes()
        .to_vec()
}

fn workspace_stored_from_body(entity: [u8; 32], body: &[u8]) -> Vec<u8> {
    let payload = sley_scb1::encode_record(&[(1, entity.to_vec()), (2, body.to_vec())])
        .expect("workspace outer record");
    let mut preimage = b"SLEYSCB1".to_vec();
    preimage.extend_from_slice(&sley_scb1::encode_uvar(1));
    preimage.extend_from_slice(&sley_scb1::encode_uvar(200));
    preimage.extend_from_slice(&[9; 32]);
    preimage.extend_from_slice(&sley_scb1::encode_uvar(
        u64::try_from(payload.len()).expect("fixture payload length fits u64"),
    ));
    preimage.extend_from_slice(&payload);
    let digest = sley_id::ObjectId::derive(&preimage);
    preimage.extend_from_slice(digest.as_bytes());
    preimage
}

pub(super) fn fixed_body_decode_image(
    build_checker: fn(&mut Asm, Ns, EntityId) -> FunctionGraph,
) -> Image {
    let mut assembler = Asm::new();
    let root_ns = Ns {
        k: 214,
        p: 215,
        b: 216,
        o: 217,
    };
    let validate_ns = Ns {
        k: 218,
        p: 219,
        b: 220,
        o: 221,
    };
    let outer_ns = Ns {
        k: 222,
        p: 223,
        b: 224,
        o: 225,
    };
    let checker_ns = Ns {
        k: 226,
        p: 227,
        b: 228,
        o: 229,
    };
    let uvar_ns = Ns {
        k: 230,
        p: 231,
        b: 232,
        o: 233,
    };
    let root = eid(11, 1);
    let validate = eid(11, 2);
    let outer = eid(11, 3);
    let checker = eid(11, 4);
    let uvar = eid(11, 5);

    let (uvar_graph, _) = build_decode(&mut assembler, uvar_ns, uvar);
    let validate_graph = build_program_validate(&mut assembler, validate_ns, validate, uvar);
    let outer_graph = build_outer_decode(&mut assembler, outer_ns, outer, uvar);
    let checker_graph = build_checker(&mut assembler, checker_ns, checker);
    let root_graph =
        build_fixed_body_program_decode(&mut assembler, root_ns, root, validate, outer, checker);
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
        "fixed-body decode closure shares immutable constants"
    );
    image
}

fn workspace_decode_image() -> Image {
    fixed_body_decode_image(
        super::dependency_binding_decode::build_empty_workspace_supported_body_check,
    )
}

pub(super) fn fixed_body_encode_image(
    build_checker: fn(&mut Asm, Ns, EntityId) -> FunctionGraph,
    payload_length: u64,
    body_length: u64,
) -> Image {
    let mut assembler = Asm::new();
    let root_ns = Ns {
        k: 214,
        p: 215,
        b: 216,
        o: 217,
    };
    let checker_ns = Ns {
        k: 218,
        p: 219,
        b: 220,
        o: 221,
    };
    let witness_ns = Ns {
        k: 222,
        p: 223,
        b: 224,
        o: 225,
    };
    let exact_ns = Ns {
        k: 226,
        p: 227,
        b: 228,
        o: 229,
    };
    let concat_ns = Ns {
        k: 230,
        p: 231,
        b: 232,
        o: 233,
    };
    let root = eid(12, 1);
    let checker = eid(12, 2);
    let witness = eid(12, 3);
    let exact = eid(12, 4);
    let concat = eid(12, 5);

    let checker_graph = build_checker(&mut assembler, checker_ns, checker);
    let concat_graph = super::package::build_concat_bytes(&mut assembler, concat_ns, concat);
    let exact_graph =
        super::package::build_exact_identity_validate(&mut assembler, exact_ns, exact);
    let witness_graph = super::package::build_single_fixed_body_witness_program_encode(
        &mut assembler,
        witness_ns,
        witness,
        exact,
        concat,
        payload_length,
        body_length,
    );
    let root_graph =
        build_fixed_body_program_encode(&mut assembler, root_ns, root, checker, witness);
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
        "fixed-body encode closure shares immutable constants"
    );
    image
}

fn workspace_encode_image() -> Image {
    fixed_body_encode_image(
        super::dependency_binding_decode::build_empty_workspace_supported_body_check,
        86,
        49,
    )
}

fn decoded_workspace(outcome: &sley_vm::ExecutionOutcome) -> (Vec<u8>, Vec<u8>) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("workspace decoder must return: {:?}", outcome.termination)
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &value.data else {
        panic!("workspace decoder must succeed: {:?}", value.data)
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!("workspace decoder returns an entity/body tuple")
    };
    let [entity, body] = fields.as_slice() else {
        panic!("workspace decoder returns exactly two fields")
    };
    let ConstData::Bytes(entity) = &entity.data else {
        panic!("workspace entity is Bytes")
    };
    let ConstData::Bytes(body) = &body.data else {
        panic!("workspace body is Bytes")
    };
    (entity.clone(), body.clone())
}

#[test]
fn codec_workspace_empty_profile_decodes_and_reencodes_native_bytes() {
    let stored = workspace_stored([0xa1; 32], [0xa2; 32], &[], &[], &[], &[]);
    let (decode_package, decode_approved) = admit(&workspace_decode_image());
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&stored), unit_input()],
    );
    let (entity, body) = decoded_workspace(&decoded);
    assert_eq!(entity, vec![0xa1; 32]);
    assert_eq!(body, ns_body_of(&stored));
    eprintln!(
        "WORKSPACE_DEC stored{}B fuel={} instr={} peak={}",
        stored.len(),
        decoded.fuel_used,
        decoded.instruction_count,
        decoded.peak_value_units
    );

    let (encode_package, encode_approved) = admit(&workspace_encode_image());
    let encoded = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&entity), bytes_input(&body), unit_input()],
    );
    assert_encode_ok(&encoded, &stored);
    eprintln!(
        "WORKSPACE_ENC fuel={} instr={} peak={}",
        encoded.fuel_used, encoded.instruction_count, encoded.peak_value_units
    );
}

#[test]
fn codec_workspace_empty_profile_rejects_nonempty_sets_and_wrong_entity_width() {
    let (decode_package, decode_approved) = admit(&workspace_decode_image());
    for (name, packages, capabilities, contracts, tests) in [
        (
            "packages",
            vec![[1; 32]],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        (
            "capabilities",
            Vec::new(),
            vec![[2; 32]],
            Vec::new(),
            Vec::new(),
        ),
        (
            "contracts",
            Vec::new(),
            Vec::new(),
            vec![[3; 32]],
            Vec::new(),
        ),
        ("tests", Vec::new(), Vec::new(), Vec::new(), vec![[4; 32]]),
    ] {
        let stored = workspace_stored(
            [0xa1; 32],
            [0xa2; 32],
            &packages,
            &capabilities,
            &contracts,
            &tests,
        );
        let outcome = execute(
            &decode_package,
            &decode_approved,
            vec![bytes_input(&stored), unit_input()],
        );
        assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
        eprintln!(
            "WORKSPACE_SCOPE {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }

    let stored = workspace_stored([0xa1; 32], [0xa2; 32], &[], &[], &[], &[]);
    let body = ns_body_of(&stored);
    let mut malformed_body = body.clone();
    malformed_body[0] = 2;
    let malformed_stored = workspace_stored_from_body([0xa1; 32], &malformed_body);
    let malformed_decode = execute(
        &decode_package,
        &decode_approved,
        vec![bytes_input(&malformed_stored), unit_input()],
    );
    assert_refusal(&malformed_decode, "SSMC_RESERVED_FIELD_PRESENT");

    let (encode_package, encode_approved) = admit(&workspace_encode_image());
    let malformed_encode = execute(
        &encode_package,
        &encode_approved,
        vec![
            bytes_input(&[0xa1; 32]),
            bytes_input(&malformed_body),
            unit_input(),
        ],
    );
    assert_refusal(&malformed_encode, "SSMC_RESERVED_FIELD_PRESENT");
    let short = execute(
        &encode_package,
        &encode_approved,
        vec![bytes_input(&[0xa1; 31]), bytes_input(&body), unit_input()],
    );
    assert_refusal(&short, "SCB_LENGTH_OVERFLOW");
}
