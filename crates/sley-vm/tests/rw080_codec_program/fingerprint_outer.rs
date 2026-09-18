//! Optional semantic-fingerprint encoding for the SSMC1 outer object record.
//!
//! This slice composes the existing two-field outer encoder, replaces its
//! record-count byte with three, and appends canonical field 4. The Sley
//! program validates the fingerprint as exactly 32 bytes and copies every
//! runtime byte itself. Labels remain outside this slice because they require
//! the pinned Unicode 16.0 NFC tables.

use super::dependency_binding::{
    EncodeBlocks, block_parameters, build_constant_push_chain, build_exact_32_gate,
};
use super::supported_dispatch::push_preallocated_block as append_block;
use super::*;
use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};

/// Copies `source[start..]` into `accumulator` with a bounded Sley loop.
/// The head accepts `(accumulator, source, carries...)`; the destination
/// receives `(accumulator, carries...)`.
#[allow(clippy::too_many_lines)]
fn build_copy_loop(
    assembler: &mut Asm,
    ns: Ns,
    control: EncodeBlocks,
    invariant_trap: EntityId,
    start: u64,
    carry_types: &[TypeExpr],
    destination: EntityId,
) -> EntityId {
    let setup = assembler.id(ns.b);
    let check = assembler.id(ns.b);
    let get = assembler.id(ns.b);
    let push = assembler.id(ns.b);
    let advance = assembler.id(ns.b);

    let mut setup_types = vec![u8vec_type(), u8vec_type()];
    setup_types.extend_from_slice(carry_types);
    let setup_parameters = block_parameters(assembler, ns.p, setup, &setup_types);
    let start_constant = assembler.ku64(ns.k, u128::from(start));
    let start_value = assembler.cref(ns.o, setup, start_constant, u64_type());
    let mut setup_arguments = vec![
        op_result(start_value),
        pav(setup_parameters[1]),
        pav(setup_parameters[0]),
    ];
    setup_arguments.extend(setup_parameters[2..].iter().copied().map(pav));
    append_block(
        assembler,
        setup,
        control.function,
        setup_parameters,
        vec![start_value],
        branch(edge(check, setup_arguments)),
    );

    let mut loop_types = vec![u64_type(), u8vec_type(), u8vec_type()];
    loop_types.extend_from_slice(carry_types);
    let check_parameters = block_parameters(assembler, ns.p, check, &loop_types);
    let index = check_parameters[0];
    let source = check_parameters[1];
    let accumulator = check_parameters[2];
    let length = assembler.op(
        ns.o,
        check,
        Opcode::VectorLen,
        vec![pav(source)],
        vec![u64_type()],
        Immediate::None,
    );
    let has_byte = assembler.op(
        ns.o,
        check,
        Opcode::LessThan,
        vec![pav(index), op_result(length)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let mut get_arguments = check_parameters
        .iter()
        .copied()
        .map(pav)
        .collect::<Vec<_>>();
    let mut done_arguments = vec![pav(accumulator)];
    done_arguments.extend(check_parameters[3..].iter().copied().map(pav));
    append_block(
        assembler,
        check,
        control.function,
        check_parameters,
        vec![length, has_byte],
        cond(
            op_result(has_byte),
            edge(get, std::mem::take(&mut get_arguments)),
            edge(destination, done_arguments),
        ),
    );

    let get_parameters = block_parameters(assembler, ns.p, get, &loop_types);
    let byte = assembler.op(
        ns.o,
        get,
        Opcode::VectorGet,
        vec![pav(get_parameters[1]), pav(get_parameters[0])],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    let mut some_arguments = vec![SwitchArgument::CasePayload];
    some_arguments.extend(get_parameters.iter().copied().map(sav));
    append_block(
        assembler,
        get,
        control.function,
        get_parameters,
        vec![byte],
        switch(
            op_result(byte),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (BuiltinCase::Some, push, some_arguments),
            ],
        ),
    );

    let mut push_types = vec![u8_type()];
    push_types.extend_from_slice(&loop_types);
    let push_parameters = block_parameters(assembler, ns.p, push, &push_types);
    let appended = assembler.op(
        ns.o,
        push,
        Opcode::AdapterInvoke,
        vec![pav(push_parameters[3]), pav(push_parameters[0])],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    let mut advance_arguments = vec![
        sav(push_parameters[1]),
        sav(push_parameters[2]),
        SwitchArgument::CasePayload,
    ];
    advance_arguments.extend(push_parameters[4..].iter().copied().map(sav));
    append_block(
        assembler,
        push,
        control.function,
        push_parameters,
        vec![appended],
        switch(
            op_result(appended),
            vec![
                (BuiltinCase::Ok, advance, advance_arguments),
                (BuiltinCase::Err, control.resource_error, Vec::new()),
            ],
        ),
    );

    let advance_parameters = block_parameters(assembler, ns.p, advance, &loop_types);
    let one_constant = assembler.ku64(ns.k, 1);
    let one = assembler.cref(ns.o, advance, one_constant, u64_type());
    let next = assembler.op(
        ns.o,
        advance,
        Opcode::IntAddChecked,
        vec![pav(advance_parameters[0]), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let mut next_arguments = vec![SwitchArgument::CasePayload];
    next_arguments.extend(advance_parameters[1..].iter().copied().map(sav));
    append_block(
        assembler,
        advance,
        control.function,
        advance_parameters,
        vec![one, next],
        switch(
            op_result(next),
            vec![
                (BuiltinCase::Ok, check, next_arguments),
                (BuiltinCase::Err, control.resource_error, Vec::new()),
            ],
        ),
    );
    setup
}

#[allow(clippy::too_many_lines)]
fn build_fingerprint_outer_encode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    outer_encoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = encode_result_type();
    let entity = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let fingerprint = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

    let length_code = assembler.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let trailing_code = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let constant_32 = assembler.ku64(ns.k, 32);
    let length_error = err_block(assembler, ns, function, result_type.clone(), length_code);
    let trailing_error = err_block(assembler, ns, function, result_type.clone(), trailing_code);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let control = EncodeBlocks {
        function,
        length_error,
        trailing_error,
        resource_error,
    };

    let entry = assembler.id(ns.b);
    let fingerprint_gate_success = assembler.id(ns.b);
    let convert_fingerprint = assembler.id(ns.b);
    let convert_outer = assembler.id(ns.b);
    let start_output = assembler.id(ns.b);
    let append_metadata = assembler.id(ns.b);
    let convert_output = assembler.id(ns.b);
    let return_output = assembler.id(ns.b);
    let forward_error = assembler.id(ns.b);

    let fingerprint_copy = build_copy_loop(
        assembler,
        ns,
        control,
        invariant_trap,
        0,
        &[TypeExpr::Unit],
        convert_output,
    );
    let metadata_prefix = build_constant_push_chain(
        assembler,
        ns,
        control,
        &[4, 32],
        &[u8vec_type(), TypeExpr::Unit],
        fingerprint_copy,
    );
    let outer_copy = build_copy_loop(
        assembler,
        ns,
        control,
        invariant_trap,
        1,
        &[u8vec_type(), TypeExpr::Unit],
        append_metadata,
    );
    let count_prefix = build_constant_push_chain(
        assembler,
        ns,
        control,
        &[3],
        &[u8vec_type(), u8vec_type(), TypeExpr::Unit],
        outer_copy,
    );

    let gate = build_exact_32_gate(
        assembler,
        ns,
        control,
        &[TypeExpr::Bytes, u8vec_type(), TypeExpr::Unit],
        1,
        constant_32,
        fingerprint_gate_success,
    );

    let encoded_outer = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(entity), pav(body), pav(unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: outer_encoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![encoded_outer],
        switch(
            op_result(encoded_outer),
            vec![
                (
                    BuiltinCase::Ok,
                    convert_fingerprint,
                    vec![SwitchArgument::CasePayload, sav(fingerprint), sav(unit)],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let fingerprint_parameters = block_parameters(
        assembler,
        ns.p,
        convert_fingerprint,
        &[TypeExpr::Bytes, TypeExpr::Bytes, TypeExpr::Unit],
    );
    let converted_fingerprint = assembler.op(
        ns.o,
        convert_fingerprint,
        Opcode::AdapterInvoke,
        vec![
            pav(fingerprint_parameters[2]),
            pav(fingerprint_parameters[1]),
        ],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    append_block(
        assembler,
        convert_fingerprint,
        function,
        fingerprint_parameters.clone(),
        vec![converted_fingerprint],
        switch(
            op_result(converted_fingerprint),
            vec![
                (
                    BuiltinCase::Ok,
                    gate,
                    vec![
                        sav(fingerprint_parameters[0]),
                        SwitchArgument::CasePayload,
                        sav(fingerprint_parameters[2]),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let gate_parameters = block_parameters(
        assembler,
        ns.p,
        fingerprint_gate_success,
        &[TypeExpr::Bytes, u8vec_type(), TypeExpr::Unit],
    );
    append_block(
        assembler,
        fingerprint_gate_success,
        function,
        gate_parameters.clone(),
        Vec::new(),
        branch(edge(
            convert_outer,
            gate_parameters.iter().copied().map(pav).collect(),
        )),
    );

    let convert_parameters = block_parameters(
        assembler,
        ns.p,
        convert_outer,
        &[TypeExpr::Bytes, u8vec_type(), TypeExpr::Unit],
    );
    let converted_outer = assembler.op(
        ns.o,
        convert_outer,
        Opcode::AdapterInvoke,
        vec![pav(convert_parameters[2]), pav(convert_parameters[0])],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    append_block(
        assembler,
        convert_outer,
        function,
        convert_parameters.clone(),
        vec![converted_outer],
        switch(
            op_result(converted_outer),
            vec![
                (
                    BuiltinCase::Ok,
                    start_output,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(convert_parameters[1]),
                        sav(convert_parameters[2]),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let start_parameters = block_parameters(
        assembler,
        ns.p,
        start_output,
        &[u8vec_type(), u8vec_type(), TypeExpr::Unit],
    );
    let empty = assembler.op(
        ns.o,
        start_output,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    append_block(
        assembler,
        start_output,
        function,
        start_parameters.clone(),
        vec![empty],
        branch(edge(
            count_prefix,
            vec![
                op_result(empty),
                pav(start_parameters[0]),
                pav(start_parameters[1]),
                pav(start_parameters[2]),
            ],
        )),
    );

    let metadata_parameters = block_parameters(
        assembler,
        ns.p,
        append_metadata,
        &[u8vec_type(), u8vec_type(), TypeExpr::Unit],
    );
    append_block(
        assembler,
        append_metadata,
        function,
        metadata_parameters.clone(),
        Vec::new(),
        branch(edge(
            metadata_prefix,
            metadata_parameters.iter().copied().map(pav).collect(),
        )),
    );

    let output_parameters = block_parameters(
        assembler,
        ns.p,
        convert_output,
        &[u8vec_type(), TypeExpr::Unit],
    );
    let output = assembler.op(
        ns.o,
        convert_output,
        Opcode::AdapterInvoke,
        vec![pav(output_parameters[1]), pav(output_parameters[0])],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    append_block(
        assembler,
        convert_output,
        function,
        output_parameters,
        vec![output],
        switch(
            op_result(output),
            vec![
                (
                    BuiltinCase::Ok,
                    return_output,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let returned_bytes =
        assembler.param(ns.p, return_output, ParameterRole::Block, TypeExpr::Bytes);
    let returned = assembler.op(
        ns.o,
        return_output,
        Opcode::ResultOk,
        vec![pav(returned_bytes)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        return_output,
        function,
        vec![returned_bytes],
        vec![returned],
        ret(op_result(returned)),
    );

    let forwarded_code =
        assembler.param(ns.p, forward_error, ParameterRole::Block, TypeExpr::Bytes);
    let forwarded = assembler.op(
        ns.o,
        forward_error,
        Opcode::ResultErr,
        vec![pav(forwarded_code)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        forward_error,
        function,
        vec![forwarded_code],
        vec![forwarded],
        ret(op_result(forwarded)),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![entity, body, fingerprint, unit],
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

fn fingerprint_encode_image() -> Image {
    let mut assembler = Asm::new();
    let uvar_ns = Ns {
        k: 3,
        p: 4,
        b: 5,
        o: 6,
    };
    let outer_ns = Ns {
        k: 7,
        p: 8,
        b: 9,
        o: 10,
    };
    let fingerprint_ns = Ns {
        k: 11,
        p: 12,
        b: 13,
        o: 14,
    };
    let uvar_function = eid(15, 1);
    let outer_function = eid(15, 2);
    let fingerprint_function = eid(15, 3);
    let uvar_graph = build_encode(&mut assembler, uvar_ns, uvar_function);
    let outer_graph = build_outer_encode(&mut assembler, outer_ns, outer_function, uvar_function);
    let fingerprint_graph = build_fingerprint_outer_encode(
        &mut assembler,
        fingerprint_ns,
        fingerprint_function,
        outer_function,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fingerprint_graph.clone(),
        functions: vec![fingerprint_graph, outer_graph, uvar_graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
        ],
        constants: assembler.constants,
    }
}

fn call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    entity: &[u8],
    body: &[u8],
    fingerprint: &[u8],
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![
            bytes_input(entity),
            bytes_input(body),
            bytes_input(fingerprint),
            unit_input(),
        ],
    )
}

#[test]
fn fingerprint_outer_encode_matches_native_record_bytes() {
    let (package, approved) = admit(&fingerprint_encode_image());
    let entity = [0x41; 32];
    for (body, fingerprint) in [
        (Vec::new(), vec![0x51; 32]),
        (
            (0..127u8).collect::<Vec<_>>(),
            (0..32u8).collect::<Vec<_>>(),
        ),
        (vec![0xa5; 140], vec![0xfe; 32]),
    ] {
        let expected = sley_scb1::encode_record(&[
            (1, entity.to_vec()),
            (2, body.clone()),
            (4, fingerprint.clone()),
        ])
        .expect("native outer record encodes");
        let outcome = call(&package, &approved, &entity, &body, &fingerprint);
        assert_encode_ok(&outcome, &expected);
        eprintln!(
            "RW080_FINGERPRINT_ENCODE body={} stored={} fuel={} instr={} peak={}",
            body.len(),
            expected.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }
}

#[test]
fn fingerprint_outer_encode_preserves_fixed_width_and_outer_errors() {
    let (package, approved) = admit(&fingerprint_encode_image());
    let entity = [0x61; 32];
    assert_refusal(
        &call(&package, &approved, &entity, b"body", &[0x71; 31]),
        "SCB_LENGTH_OVERFLOW",
    );
    assert_refusal(
        &call(&package, &approved, &entity, b"body", &[0x72; 33]),
        "SCB_TRAILING_BYTES",
    );
    assert_refusal(
        &call(&package, &approved, &[0x61; 31], b"body", &[0x73; 32]),
        "SCB_LENGTH_OVERFLOW",
    );
    assert_refusal(
        &call(&package, &approved, &[0x61; 33], b"body", &[0x74; 32]),
        "SCB_TRAILING_BYTES",
    );
    assert_refusal(
        &call(&package, &approved, &[0x61; 33], b"body", &[0x75; 31]),
        "SCB_TRAILING_BYTES",
    );
    assert_refusal(
        &call(&package, &approved, &[0x61; 31], b"body", &[0x76; 33]),
        "SCB_LENGTH_OVERFLOW",
    );
}
