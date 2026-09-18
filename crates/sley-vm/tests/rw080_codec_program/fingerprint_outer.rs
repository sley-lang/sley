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

fn fingerprint_decode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            TypeExpr::Bytes,
        ])),
        error: Box::new(TypeExpr::Bytes),
    }
}

/// Calls the admitted uvar decoder and forwards its typed refusal unchanged.
/// The head accepts `(input, position, unit, carries...)`; the destination
/// receives `(value, next_position, input, unit, carries...)`.
#[allow(clippy::too_many_arguments)]
fn build_uvar_step(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    decoder: EntityId,
    width: EntityId,
    carry_types: &[TypeExpr],
    destination: EntityId,
    forward_error: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let success = assembler.id(ns.b);
    let mut head_types = vec![TypeExpr::Bytes, u64_type(), TypeExpr::Unit];
    head_types.extend_from_slice(carry_types);
    let head_parameters = block_parameters(assembler, ns.p, head, &head_types);
    let width_value = assembler.cref(ns.o, head, width, u32_type());
    let decoded_value = assembler.op(
        ns.o,
        head,
        Opcode::CallDirect,
        vec![
            pav(head_parameters[0]),
            pav(head_parameters[1]),
            op_result(width_value),
            pav(head_parameters[2]),
        ],
        vec![decode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: decoder,
            type_arguments: Vec::new(),
        }),
    );
    let mut success_arguments = vec![SwitchArgument::CasePayload];
    success_arguments.extend(head_parameters[0..1].iter().copied().map(sav));
    success_arguments.extend(head_parameters[2..].iter().copied().map(sav));
    append_block(
        assembler,
        head,
        function,
        head_parameters,
        vec![width_value, decoded_value],
        switch(
            op_result(decoded_value),
            vec![
                (BuiltinCase::Ok, success, success_arguments),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let tuple_type = TypeExpr::Tuple(vec![u64_type(), u64_type()]);
    let mut success_types = vec![tuple_type, TypeExpr::Bytes, TypeExpr::Unit];
    success_types.extend_from_slice(carry_types);
    let success_parameters = block_parameters(assembler, ns.p, success, &success_types);
    let value = assembler.op(
        ns.o,
        success,
        Opcode::TupleGet,
        vec![pav(success_parameters[0])],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let next_position = assembler.op(
        ns.o,
        success,
        Opcode::TupleGet,
        vec![pav(success_parameters[0])],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let mut destination_arguments = vec![
        op_result(value),
        op_result(next_position),
        pav(success_parameters[1]),
        pav(success_parameters[2]),
    ];
    destination_arguments.extend(success_parameters[3..].iter().copied().map(pav));
    append_block(
        assembler,
        success,
        function,
        success_parameters,
        vec![value, next_position],
        branch(edge(destination, destination_arguments)),
    );
    head
}

/// Enforces the SCB sized-payload resource and input bounds. The head accepts
/// `(length, data_position, input, unit, vector, input_length, carries...)`;
/// the destination additionally receives `end_position` after `data_position`.
#[allow(clippy::too_many_arguments)]
fn build_payload_end_gate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    maximum: EntityId,
    carry_types: &[TypeExpr],
    destination: EntityId,
    length_error: EntityId,
    resource_error: EntityId,
    invariant_trap: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let add = assembler.id(ns.b);
    let bounds = assembler.id(ns.b);
    let mut parameter_types = vec![
        u64_type(),
        u64_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
        u8vec_type(),
        u64_type(),
    ];
    parameter_types.extend_from_slice(carry_types);
    let head_parameters = block_parameters(assembler, ns.p, head, &parameter_types);
    let maximum_value = assembler.cref(ns.o, head, maximum, u64_type());
    let too_large = assembler.op(
        ns.o,
        head,
        Opcode::GreaterThan,
        vec![pav(head_parameters[0]), op_result(maximum_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        head,
        function,
        head_parameters.clone(),
        vec![maximum_value, too_large],
        cond(
            op_result(too_large),
            edge(resource_error, Vec::new()),
            edge(add, head_parameters.iter().copied().map(pav).collect()),
        ),
    );

    let add_parameters = block_parameters(assembler, ns.p, add, &parameter_types);
    let end = assembler.op(
        ns.o,
        add,
        Opcode::IntAddChecked,
        vec![pav(add_parameters[1]), pav(add_parameters[0])],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let mut bounds_arguments = vec![SwitchArgument::CasePayload];
    bounds_arguments.extend(add_parameters.iter().copied().map(sav));
    append_block(
        assembler,
        add,
        function,
        add_parameters,
        vec![end],
        switch(
            op_result(end),
            vec![
                (BuiltinCase::Ok, bounds, bounds_arguments),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let mut bounds_types = vec![u64_type()];
    bounds_types.extend_from_slice(&parameter_types);
    let bounds_parameters = block_parameters(assembler, ns.p, bounds, &bounds_types);
    let beyond_input = assembler.op(
        ns.o,
        bounds,
        Opcode::GreaterThan,
        vec![pav(bounds_parameters[0]), pav(bounds_parameters[6])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let mut destination_arguments = vec![
        pav(bounds_parameters[1]),
        pav(bounds_parameters[2]),
        pav(bounds_parameters[0]),
    ];
    destination_arguments.extend(bounds_parameters[3..].iter().copied().map(pav));
    append_block(
        assembler,
        bounds,
        function,
        bounds_parameters,
        vec![beyond_input],
        cond(
            op_result(beyond_input),
            edge(length_error, Vec::new()),
            edge(destination, destination_arguments),
        ),
    );
    head
}

/// Requires a previously bounded sized payload to contain exactly 32 bytes.
/// All values are forwarded unchanged to `destination` on success.
#[allow(clippy::too_many_arguments)]
fn build_exact_32_length_gate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    constant_32: EntityId,
    carry_types: &[TypeExpr],
    destination: EntityId,
    length_error: EntityId,
    trailing_error: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let long = assembler.id(ns.b);
    let mut parameter_types = vec![
        u64_type(),
        u64_type(),
        u64_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
        u8vec_type(),
        u64_type(),
    ];
    parameter_types.extend_from_slice(carry_types);
    let parameters = block_parameters(assembler, ns.p, head, &parameter_types);
    let expected = assembler.cref(ns.o, head, constant_32, u64_type());
    let short = assembler.op(
        ns.o,
        head,
        Opcode::LessThan,
        vec![pav(parameters[0]), op_result(expected)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        head,
        function,
        parameters.clone(),
        vec![expected, short],
        cond(
            op_result(short),
            edge(length_error, Vec::new()),
            edge(long, parameters.iter().copied().map(pav).collect()),
        ),
    );

    let long_parameters = block_parameters(assembler, ns.p, long, &parameter_types);
    let expected = assembler.cref(ns.o, long, constant_32, u64_type());
    let is_long = assembler.op(
        ns.o,
        long,
        Opcode::GreaterThan,
        vec![pav(long_parameters[0]), op_result(expected)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        long,
        function,
        long_parameters.clone(),
        vec![expected, is_long],
        cond(
            op_result(is_long),
            edge(trailing_error, Vec::new()),
            edge(
                destination,
                long_parameters.iter().copied().map(pav).collect(),
            ),
        ),
    );
    head
}

/// Copies `source[start..end]` into an accumulator. The head accepts
/// `(start, end, accumulator, source, carries...)`; the destination receives
/// `(accumulator, source, carries...)`.
#[allow(clippy::too_many_lines)]
fn build_copy_range_loop(
    assembler: &mut Asm,
    ns: Ns,
    control: EncodeBlocks,
    invariant_trap: EntityId,
    carry_types: &[TypeExpr],
    destination: EntityId,
) -> EntityId {
    let check = assembler.id(ns.b);
    let get = assembler.id(ns.b);
    let push = assembler.id(ns.b);
    let advance = assembler.id(ns.b);
    let mut loop_types = vec![u64_type(), u64_type(), u8vec_type(), u8vec_type()];
    loop_types.extend_from_slice(carry_types);

    let check_parameters = block_parameters(assembler, ns.p, check, &loop_types);
    let has_byte = assembler.op(
        ns.o,
        check,
        Opcode::LessThan,
        vec![pav(check_parameters[0]), pav(check_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let mut done_arguments = vec![pav(check_parameters[2]), pav(check_parameters[3])];
    done_arguments.extend(check_parameters[4..].iter().copied().map(pav));
    append_block(
        assembler,
        check,
        control.function,
        check_parameters.clone(),
        vec![has_byte],
        cond(
            op_result(has_byte),
            edge(get, check_parameters.iter().copied().map(pav).collect()),
            edge(destination, done_arguments),
        ),
    );

    let get_parameters = block_parameters(assembler, ns.p, get, &loop_types);
    let byte = assembler.op(
        ns.o,
        get,
        Opcode::VectorGet,
        vec![pav(get_parameters[3]), pav(get_parameters[0])],
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
        sav(push_parameters[4]),
    ];
    advance_arguments.extend(push_parameters[5..].iter().copied().map(sav));
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
    check
}

#[allow(clippy::too_many_lines)]
fn build_fingerprint_outer_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    uvar_decoder: EntityId,
    outer_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = fingerprint_decode_result_type();
    let input = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

    let zero = assembler.ku64(ns.k, 0);
    let one = assembler.ku64(ns.k, 1);
    let two = assembler.ku64(ns.k, 2);
    let three = assembler.ku64(ns.k, 3);
    let four = assembler.ku64(ns.k, 4);
    let thirty_two = assembler.ku64(ns.k, 32);
    let maximum_fields = assembler.ku64(ns.k, 65_535);
    let maximum_payload = assembler.ku64(ns.k, 67_108_864);
    let width_32 = assembler.ku32(ns.k, 32);
    let width_64 = assembler.ku32(ns.k, 64);
    let missing_code = assembler.kbytes(ns.k, b"SCB_FIELD_MISSING");
    let unknown_code = assembler.kbytes(ns.k, b"SCB_FIELD_UNKNOWN");
    let duplicate_code = assembler.kbytes(ns.k, b"SCB_FIELD_DUPLICATE");
    let order_code = assembler.kbytes(ns.k, b"SCB_FIELD_ORDER");
    let length_code = assembler.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let trailing_code = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let scope_code = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");

    let missing_error = err_block(assembler, ns, function, result_type.clone(), missing_code);
    let unknown_error = err_block(assembler, ns, function, result_type.clone(), unknown_code);
    let duplicate_error = err_block(assembler, ns, function, result_type.clone(), duplicate_code);
    let order_error = err_block(assembler, ns, function, result_type.clone(), order_code);
    let length_error = err_block(assembler, ns, function, result_type.clone(), length_code);
    let trailing_error = err_block(assembler, ns, function, result_type.clone(), trailing_code);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
    let scope_error = err_block(assembler, ns, function, result_type.clone(), scope_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let control = EncodeBlocks {
        function,
        length_error,
        trailing_error,
        resource_error,
    };

    let entry = assembler.id(ns.b);
    let input_ready = assembler.id(ns.b);
    let count_max = assembler.id(ns.b);
    let count_low = assembler.id(ns.b);
    let count_high = assembler.id(ns.b);
    let count_scope = assembler.id(ns.b);
    let field1_begin = assembler.id(ns.b);
    let field1_match = assembler.id(ns.b);
    let field1_done = assembler.id(ns.b);
    let field2_begin = assembler.id(ns.b);
    let field2_order = assembler.id(ns.b);
    let field2_length = assembler.id(ns.b);
    let field2_match = assembler.id(ns.b);
    let field2_match_scope = assembler.id(ns.b);
    let field2_done = assembler.id(ns.b);
    let field3_begin = assembler.id(ns.b);
    let field3_order = assembler.id(ns.b);
    let field3_length = assembler.id(ns.b);
    let field3_match = assembler.id(ns.b);
    let field3_match_scope = assembler.id(ns.b);
    let field3_done = assembler.id(ns.b);
    let copy_start = assembler.id(ns.b);
    let outer_copy_start = assembler.id(ns.b);
    let outer_done = assembler.id(ns.b);
    let fingerprint_start = assembler.id(ns.b);
    let fingerprint_done = assembler.id(ns.b);
    let call_outer = assembler.id(ns.b);
    let return_output = assembler.id(ns.b);
    let forward_error = assembler.id(ns.b);

    let field3_exact = build_exact_32_length_gate(
        assembler,
        ns,
        function,
        thirty_two,
        &[u64_type(), u64_type(), u64_type()],
        field3_done,
        length_error,
        trailing_error,
    );
    let field3_payload = build_payload_end_gate(
        assembler,
        ns,
        function,
        maximum_payload,
        &[u64_type(), u64_type(), u64_type()],
        field3_match,
        length_error,
        resource_error,
        invariant_trap,
    );
    let field3_len = build_uvar_step(
        assembler,
        ns,
        function,
        uvar_decoder,
        width_64,
        &[u8vec_type(), u64_type(), u64_type(), u64_type(), u64_type()],
        field3_payload,
        forward_error,
    );
    let field3_tag = build_uvar_step(
        assembler,
        ns,
        function,
        uvar_decoder,
        width_32,
        &[u8vec_type(), u64_type(), u64_type(), u64_type()],
        field3_begin,
        forward_error,
    );
    let field2_payload = build_payload_end_gate(
        assembler,
        ns,
        function,
        maximum_payload,
        &[u64_type(), u64_type()],
        field2_match,
        length_error,
        resource_error,
        invariant_trap,
    );
    let field2_len = build_uvar_step(
        assembler,
        ns,
        function,
        uvar_decoder,
        width_64,
        &[u8vec_type(), u64_type(), u64_type(), u64_type()],
        field2_payload,
        forward_error,
    );
    let field2_tag = build_uvar_step(
        assembler,
        ns,
        function,
        uvar_decoder,
        width_32,
        &[u8vec_type(), u64_type(), u64_type()],
        field2_begin,
        forward_error,
    );
    let field1_exact = build_exact_32_length_gate(
        assembler,
        ns,
        function,
        thirty_two,
        &[u64_type(), u64_type()],
        field1_done,
        length_error,
        trailing_error,
    );
    let field1_payload = build_payload_end_gate(
        assembler,
        ns,
        function,
        maximum_payload,
        &[u64_type(), u64_type()],
        field1_match,
        length_error,
        resource_error,
        invariant_trap,
    );
    let field1_len = build_uvar_step(
        assembler,
        ns,
        function,
        uvar_decoder,
        width_64,
        &[u8vec_type(), u64_type(), u64_type(), u64_type()],
        field1_payload,
        forward_error,
    );
    let field1_tag = build_uvar_step(
        assembler,
        ns,
        function,
        uvar_decoder,
        width_32,
        &[u8vec_type(), u64_type(), u64_type()],
        field1_begin,
        forward_error,
    );
    let count_step = build_uvar_step(
        assembler,
        ns,
        function,
        uvar_decoder,
        width_64,
        &[u8vec_type(), u64_type()],
        count_max,
        forward_error,
    );

    let fingerprint_copy = build_copy_range_loop(
        assembler,
        ns,
        control,
        invariant_trap,
        &[TypeExpr::Bytes, TypeExpr::Unit],
        fingerprint_done,
    );
    let outer_copy = build_copy_range_loop(
        assembler,
        ns,
        control,
        invariant_trap,
        &[u64_type(), u64_type(), TypeExpr::Unit],
        outer_done,
    );
    let outer_prefix = build_constant_push_chain(
        assembler,
        ns,
        control,
        &[2],
        &[
            u64_type(),
            u64_type(),
            u64_type(),
            u64_type(),
            u8vec_type(),
            TypeExpr::Unit,
        ],
        outer_copy_start,
    );

    let converted_input = assembler.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(unit), pav(input)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![converted_input],
        switch(
            op_result(converted_input),
            vec![
                (
                    BuiltinCase::Ok,
                    input_ready,
                    vec![SwitchArgument::CasePayload, sav(input), sav(unit)],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let ready_parameters = block_parameters(
        assembler,
        ns.p,
        input_ready,
        &[u8vec_type(), TypeExpr::Bytes, TypeExpr::Unit],
    );
    let input_length = assembler.op(
        ns.o,
        input_ready,
        Opcode::VectorLen,
        vec![pav(ready_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let start = assembler.cref(ns.o, input_ready, zero, u64_type());
    append_block(
        assembler,
        input_ready,
        function,
        ready_parameters.clone(),
        vec![input_length, start],
        branch(edge(
            count_step,
            vec![
                pav(ready_parameters[1]),
                op_result(start),
                pav(ready_parameters[2]),
                pav(ready_parameters[0]),
                op_result(input_length),
            ],
        )),
    );

    let count_parameters = block_parameters(
        assembler,
        ns.p,
        count_max,
        &[
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
        ],
    );
    let max_fields = assembler.cref(ns.o, count_max, maximum_fields, u64_type());
    let too_many = assembler.op(
        ns.o,
        count_max,
        Opcode::GreaterThan,
        vec![pav(count_parameters[0]), op_result(max_fields)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        count_max,
        function,
        count_parameters.clone(),
        vec![max_fields, too_many],
        cond(
            op_result(too_many),
            edge(resource_error, Vec::new()),
            edge(
                count_low,
                count_parameters.iter().copied().map(pav).collect(),
            ),
        ),
    );

    let low_parameters = block_parameters(
        assembler,
        ns.p,
        count_low,
        &[
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
        ],
    );
    let required_count = assembler.cref(ns.o, count_low, three, u64_type());
    let too_few = assembler.op(
        ns.o,
        count_low,
        Opcode::LessThan,
        vec![pav(low_parameters[0]), op_result(required_count)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        count_low,
        function,
        low_parameters.clone(),
        vec![required_count, too_few],
        cond(
            op_result(too_few),
            edge(missing_error, Vec::new()),
            edge(
                count_high,
                low_parameters.iter().copied().map(pav).collect(),
            ),
        ),
    );

    let high_parameters = block_parameters(
        assembler,
        ns.p,
        count_high,
        &[
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
        ],
    );
    let maximum_count = assembler.cref(ns.o, count_high, four, u64_type());
    let above_schema = assembler.op(
        ns.o,
        count_high,
        Opcode::GreaterThan,
        vec![pav(high_parameters[0]), op_result(maximum_count)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        count_high,
        function,
        high_parameters.clone(),
        vec![maximum_count, above_schema],
        cond(
            op_result(above_schema),
            edge(unknown_error, Vec::new()),
            edge(
                count_scope,
                high_parameters.iter().copied().map(pav).collect(),
            ),
        ),
    );

    let scope_parameters = block_parameters(
        assembler,
        ns.p,
        count_scope,
        &[
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
        ],
    );
    let label_and_fingerprint = assembler.cref(ns.o, count_scope, four, u64_type());
    let has_label = assembler.op(
        ns.o,
        count_scope,
        Opcode::Equal,
        vec![pav(scope_parameters[0]), op_result(label_and_fingerprint)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        count_scope,
        function,
        scope_parameters.clone(),
        vec![label_and_fingerprint, has_label],
        cond(
            op_result(has_label),
            edge(scope_error, Vec::new()),
            edge(
                field1_tag,
                vec![
                    pav(scope_parameters[2]),
                    pav(scope_parameters[1]),
                    pav(scope_parameters[3]),
                    pav(scope_parameters[4]),
                    pav(scope_parameters[5]),
                    pav(scope_parameters[1]),
                ],
            ),
        ),
    );

    let field1_parameters = block_parameters(
        assembler,
        ns.p,
        field1_begin,
        &[
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
        ],
    );
    append_block(
        assembler,
        field1_begin,
        function,
        field1_parameters.clone(),
        Vec::new(),
        branch(edge(
            field1_len,
            vec![
                pav(field1_parameters[2]),
                pav(field1_parameters[1]),
                pav(field1_parameters[3]),
                pav(field1_parameters[4]),
                pav(field1_parameters[5]),
                pav(field1_parameters[6]),
                pav(field1_parameters[0]),
            ],
        )),
    );

    let field1_match_parameters = block_parameters(
        assembler,
        ns.p,
        field1_match,
        &[
            u64_type(),
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
        ],
    );
    let tag_one = assembler.cref(ns.o, field1_match, one, u64_type());
    let is_field1 = assembler.op(
        ns.o,
        field1_match,
        Opcode::Equal,
        vec![pav(field1_match_parameters[8]), op_result(tag_one)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        field1_match,
        function,
        field1_match_parameters.clone(),
        vec![tag_one, is_field1],
        cond(
            op_result(is_field1),
            edge(
                field1_exact,
                field1_match_parameters.iter().copied().map(pav).collect(),
            ),
            edge(unknown_error, Vec::new()),
        ),
    );

    let field1_done_parameters = block_parameters(
        assembler,
        ns.p,
        field1_done,
        &[
            u64_type(),
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
        ],
    );
    append_block(
        assembler,
        field1_done,
        function,
        field1_done_parameters.clone(),
        Vec::new(),
        branch(edge(
            field2_tag,
            vec![
                pav(field1_done_parameters[3]),
                pav(field1_done_parameters[2]),
                pav(field1_done_parameters[4]),
                pav(field1_done_parameters[5]),
                pav(field1_done_parameters[6]),
                pav(field1_done_parameters[7]),
            ],
        )),
    );

    let field2_parameters = block_parameters(
        assembler,
        ns.p,
        field2_begin,
        &[
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
        ],
    );
    let previous_one = assembler.cref(ns.o, field2_begin, one, u64_type());
    let is_duplicate = assembler.op(
        ns.o,
        field2_begin,
        Opcode::Equal,
        vec![pav(field2_parameters[0]), op_result(previous_one)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        field2_begin,
        function,
        field2_parameters.clone(),
        vec![previous_one, is_duplicate],
        cond(
            op_result(is_duplicate),
            edge(duplicate_error, Vec::new()),
            edge(
                field2_order,
                field2_parameters.iter().copied().map(pav).collect(),
            ),
        ),
    );

    let field2_order_parameters = block_parameters(
        assembler,
        ns.p,
        field2_order,
        &[
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
        ],
    );
    let previous_one = assembler.cref(ns.o, field2_order, one, u64_type());
    let is_ordered_before = assembler.op(
        ns.o,
        field2_order,
        Opcode::LessThan,
        vec![pav(field2_order_parameters[0]), op_result(previous_one)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        field2_order,
        function,
        field2_order_parameters.clone(),
        vec![previous_one, is_ordered_before],
        cond(
            op_result(is_ordered_before),
            edge(order_error, Vec::new()),
            edge(
                field2_length,
                field2_order_parameters.iter().copied().map(pav).collect(),
            ),
        ),
    );

    let field2_length_parameters = block_parameters(
        assembler,
        ns.p,
        field2_length,
        &[
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
        ],
    );
    append_block(
        assembler,
        field2_length,
        function,
        field2_length_parameters.clone(),
        Vec::new(),
        branch(edge(
            field2_len,
            vec![
                pav(field2_length_parameters[2]),
                pav(field2_length_parameters[1]),
                pav(field2_length_parameters[3]),
                pav(field2_length_parameters[4]),
                pav(field2_length_parameters[5]),
                pav(field2_length_parameters[6]),
                pav(field2_length_parameters[0]),
            ],
        )),
    );

    let field2_match_parameters = block_parameters(
        assembler,
        ns.p,
        field2_match,
        &[
            u64_type(),
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
        ],
    );
    let tag_two = assembler.cref(ns.o, field2_match, two, u64_type());
    let is_field2 = assembler.op(
        ns.o,
        field2_match,
        Opcode::Equal,
        vec![pav(field2_match_parameters[8]), op_result(tag_two)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        field2_match,
        function,
        field2_match_parameters.clone(),
        vec![tag_two, is_field2],
        cond(
            op_result(is_field2),
            edge(
                field2_done,
                field2_match_parameters.iter().copied().map(pav).collect(),
            ),
            edge(
                field2_match_scope,
                field2_match_parameters.iter().copied().map(pav).collect(),
            ),
        ),
    );

    let field2_scope_parameters = block_parameters(
        assembler,
        ns.p,
        field2_match_scope,
        &[
            u64_type(),
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
        ],
    );
    let tag_four = assembler.cref(ns.o, field2_match_scope, four, u64_type());
    let in_reserved_range = assembler.op(
        ns.o,
        field2_match_scope,
        Opcode::LessEqual,
        vec![pav(field2_scope_parameters[8]), op_result(tag_four)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        field2_match_scope,
        function,
        field2_scope_parameters,
        vec![tag_four, in_reserved_range],
        cond(
            op_result(in_reserved_range),
            edge(scope_error, Vec::new()),
            edge(unknown_error, Vec::new()),
        ),
    );

    let field2_done_parameters = block_parameters(
        assembler,
        ns.p,
        field2_done,
        &[
            u64_type(),
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
        ],
    );
    append_block(
        assembler,
        field2_done,
        function,
        field2_done_parameters.clone(),
        Vec::new(),
        branch(edge(
            field3_tag,
            vec![
                pav(field2_done_parameters[3]),
                pav(field2_done_parameters[2]),
                pav(field2_done_parameters[4]),
                pav(field2_done_parameters[5]),
                pav(field2_done_parameters[6]),
                pav(field2_done_parameters[7]),
                pav(field2_done_parameters[2]),
            ],
        )),
    );

    let field3_parameters = block_parameters(
        assembler,
        ns.p,
        field3_begin,
        &[
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
        ],
    );
    let previous_two = assembler.cref(ns.o, field3_begin, two, u64_type());
    let is_duplicate = assembler.op(
        ns.o,
        field3_begin,
        Opcode::Equal,
        vec![pav(field3_parameters[0]), op_result(previous_two)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        field3_begin,
        function,
        field3_parameters.clone(),
        vec![previous_two, is_duplicate],
        cond(
            op_result(is_duplicate),
            edge(duplicate_error, Vec::new()),
            edge(
                field3_order,
                field3_parameters.iter().copied().map(pav).collect(),
            ),
        ),
    );

    let field3_order_parameters = block_parameters(
        assembler,
        ns.p,
        field3_order,
        &[
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
        ],
    );
    let previous_two = assembler.cref(ns.o, field3_order, two, u64_type());
    let is_ordered_before = assembler.op(
        ns.o,
        field3_order,
        Opcode::LessThan,
        vec![pav(field3_order_parameters[0]), op_result(previous_two)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        field3_order,
        function,
        field3_order_parameters.clone(),
        vec![previous_two, is_ordered_before],
        cond(
            op_result(is_ordered_before),
            edge(order_error, Vec::new()),
            edge(
                field3_length,
                field3_order_parameters.iter().copied().map(pav).collect(),
            ),
        ),
    );

    let field3_length_parameters = block_parameters(
        assembler,
        ns.p,
        field3_length,
        &[
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
        ],
    );
    append_block(
        assembler,
        field3_length,
        function,
        field3_length_parameters.clone(),
        Vec::new(),
        branch(edge(
            field3_len,
            vec![
                pav(field3_length_parameters[2]),
                pav(field3_length_parameters[1]),
                pav(field3_length_parameters[3]),
                pav(field3_length_parameters[4]),
                pav(field3_length_parameters[5]),
                pav(field3_length_parameters[6]),
                pav(field3_length_parameters[7]),
                pav(field3_length_parameters[0]),
            ],
        )),
    );

    let field3_match_parameters = block_parameters(
        assembler,
        ns.p,
        field3_match,
        &[
            u64_type(),
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
            u64_type(),
        ],
    );
    let tag_four = assembler.cref(ns.o, field3_match, four, u64_type());
    let is_fingerprint = assembler.op(
        ns.o,
        field3_match,
        Opcode::Equal,
        vec![pav(field3_match_parameters[9]), op_result(tag_four)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        field3_match,
        function,
        field3_match_parameters.clone(),
        vec![tag_four, is_fingerprint],
        cond(
            op_result(is_fingerprint),
            edge(
                field3_exact,
                field3_match_parameters.iter().copied().map(pav).collect(),
            ),
            edge(
                field3_match_scope,
                field3_match_parameters.iter().copied().map(pav).collect(),
            ),
        ),
    );

    let field3_scope_parameters = block_parameters(
        assembler,
        ns.p,
        field3_match_scope,
        &[
            u64_type(),
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
            u64_type(),
        ],
    );
    let label_tag = assembler.cref(ns.o, field3_match_scope, three, u64_type());
    let is_label = assembler.op(
        ns.o,
        field3_match_scope,
        Opcode::Equal,
        vec![pav(field3_scope_parameters[9]), op_result(label_tag)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        field3_match_scope,
        function,
        field3_scope_parameters,
        vec![label_tag, is_label],
        cond(
            op_result(is_label),
            edge(scope_error, Vec::new()),
            edge(unknown_error, Vec::new()),
        ),
    );

    let field3_done_parameters = block_parameters(
        assembler,
        ns.p,
        field3_done,
        &[
            u64_type(),
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
            u64_type(),
        ],
    );
    let finished = assembler.op(
        ns.o,
        field3_done,
        Opcode::Equal,
        vec![
            pav(field3_done_parameters[2]),
            pav(field3_done_parameters[6]),
        ],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        field3_done,
        function,
        field3_done_parameters.clone(),
        vec![finished],
        cond(
            op_result(finished),
            edge(
                copy_start,
                vec![
                    pav(field3_done_parameters[7]),
                    pav(field3_done_parameters[8]),
                    pav(field3_done_parameters[1]),
                    pav(field3_done_parameters[2]),
                    pav(field3_done_parameters[5]),
                    pav(field3_done_parameters[4]),
                ],
            ),
            edge(trailing_error, Vec::new()),
        ),
    );

    let copy_parameters = block_parameters(
        assembler,
        ns.p,
        copy_start,
        &[
            u64_type(),
            u64_type(),
            u64_type(),
            u64_type(),
            u8vec_type(),
            TypeExpr::Unit,
        ],
    );
    let outer_accumulator = assembler.op(
        ns.o,
        copy_start,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    append_block(
        assembler,
        copy_start,
        function,
        copy_parameters.clone(),
        vec![outer_accumulator],
        branch(edge(
            outer_prefix,
            vec![
                op_result(outer_accumulator),
                pav(copy_parameters[0]),
                pav(copy_parameters[1]),
                pav(copy_parameters[2]),
                pav(copy_parameters[3]),
                pav(copy_parameters[4]),
                pav(copy_parameters[5]),
            ],
        )),
    );

    let outer_start_parameters = block_parameters(
        assembler,
        ns.p,
        outer_copy_start,
        &[
            u8vec_type(),
            u64_type(),
            u64_type(),
            u64_type(),
            u64_type(),
            u8vec_type(),
            TypeExpr::Unit,
        ],
    );
    append_block(
        assembler,
        outer_copy_start,
        function,
        outer_start_parameters.clone(),
        Vec::new(),
        branch(edge(
            outer_copy,
            vec![
                pav(outer_start_parameters[1]),
                pav(outer_start_parameters[2]),
                pav(outer_start_parameters[0]),
                pav(outer_start_parameters[5]),
                pav(outer_start_parameters[3]),
                pav(outer_start_parameters[4]),
                pav(outer_start_parameters[6]),
            ],
        )),
    );

    let outer_done_parameters = block_parameters(
        assembler,
        ns.p,
        outer_done,
        &[
            u8vec_type(),
            u8vec_type(),
            u64_type(),
            u64_type(),
            TypeExpr::Unit,
        ],
    );
    let outer_bytes = assembler.op(
        ns.o,
        outer_done,
        Opcode::AdapterInvoke,
        vec![pav(outer_done_parameters[4]), pav(outer_done_parameters[0])],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    append_block(
        assembler,
        outer_done,
        function,
        outer_done_parameters.clone(),
        vec![outer_bytes],
        switch(
            op_result(outer_bytes),
            vec![
                (
                    BuiltinCase::Ok,
                    fingerprint_start,
                    vec![
                        sav(outer_done_parameters[1]),
                        sav(outer_done_parameters[2]),
                        sav(outer_done_parameters[3]),
                        SwitchArgument::CasePayload,
                        sav(outer_done_parameters[4]),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let fingerprint_start_parameters = block_parameters(
        assembler,
        ns.p,
        fingerprint_start,
        &[
            u8vec_type(),
            u64_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Unit,
        ],
    );
    let fingerprint_accumulator = assembler.op(
        ns.o,
        fingerprint_start,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    append_block(
        assembler,
        fingerprint_start,
        function,
        fingerprint_start_parameters.clone(),
        vec![fingerprint_accumulator],
        branch(edge(
            fingerprint_copy,
            vec![
                pav(fingerprint_start_parameters[1]),
                pav(fingerprint_start_parameters[2]),
                op_result(fingerprint_accumulator),
                pav(fingerprint_start_parameters[0]),
                pav(fingerprint_start_parameters[3]),
                pav(fingerprint_start_parameters[4]),
            ],
        )),
    );

    let fingerprint_done_parameters = block_parameters(
        assembler,
        ns.p,
        fingerprint_done,
        &[u8vec_type(), u8vec_type(), TypeExpr::Bytes, TypeExpr::Unit],
    );
    let fingerprint_bytes = assembler.op(
        ns.o,
        fingerprint_done,
        Opcode::AdapterInvoke,
        vec![
            pav(fingerprint_done_parameters[3]),
            pav(fingerprint_done_parameters[0]),
        ],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    append_block(
        assembler,
        fingerprint_done,
        function,
        fingerprint_done_parameters.clone(),
        vec![fingerprint_bytes],
        switch(
            op_result(fingerprint_bytes),
            vec![
                (
                    BuiltinCase::Ok,
                    call_outer,
                    vec![
                        sav(fingerprint_done_parameters[2]),
                        SwitchArgument::CasePayload,
                        sav(fingerprint_done_parameters[3]),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let call_parameters = block_parameters(
        assembler,
        ns.p,
        call_outer,
        &[TypeExpr::Bytes, TypeExpr::Bytes, TypeExpr::Unit],
    );
    let decoded_outer = assembler.op(
        ns.o,
        call_outer,
        Opcode::CallDirect,
        vec![pav(call_parameters[0]), pav(call_parameters[2])],
        vec![outer_decode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: outer_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        call_outer,
        function,
        call_parameters.clone(),
        vec![decoded_outer],
        switch(
            op_result(decoded_outer),
            vec![
                (
                    BuiltinCase::Ok,
                    return_output,
                    vec![SwitchArgument::CasePayload, sav(call_parameters[1])],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let return_parameters = block_parameters(
        assembler,
        ns.p,
        return_output,
        &[
            TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes]),
            TypeExpr::Bytes,
        ],
    );
    let entity = assembler.op(
        ns.o,
        return_output,
        Opcode::TupleGet,
        vec![pav(return_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let body = assembler.op(
        ns.o,
        return_output,
        Opcode::TupleGet,
        vec![pav(return_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let output_tuple = assembler.op(
        ns.o,
        return_output,
        Opcode::TupleNew,
        vec![
            op_result(entity),
            op_result(body),
            pav(return_parameters[1]),
        ],
        vec![TypeExpr::Tuple(vec![
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            TypeExpr::Bytes,
        ])],
        Immediate::None,
    );
    let output = assembler.op(
        ns.o,
        return_output,
        Opcode::ResultOk,
        vec![op_result(output_tuple)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        return_output,
        function,
        return_parameters,
        vec![entity, body, output_tuple, output],
        ret(op_result(output)),
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
        parameters: vec![input, unit],
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

fn fingerprint_decode_image() -> Image {
    let mut assembler = Asm::new();
    let uvar_ns = Ns {
        k: 31,
        p: 32,
        b: 33,
        o: 34,
    };
    let outer_ns = Ns {
        k: 35,
        p: 36,
        b: 37,
        o: 38,
    };
    let fingerprint_ns = Ns {
        k: 39,
        p: 40,
        b: 41,
        o: 42,
    };
    let uvar_function = eid(16, 1);
    let outer_function = eid(16, 2);
    let fingerprint_function = eid(16, 3);
    let (uvar_graph, _) = build_decode(&mut assembler, uvar_ns, uvar_function);
    let outer_graph = build_outer_decode(&mut assembler, outer_ns, outer_function, uvar_function);
    let fingerprint_graph = build_fingerprint_outer_decode(
        &mut assembler,
        fingerprint_ns,
        fingerprint_function,
        uvar_function,
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

fn fingerprint_decode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    input: &[u8],
) -> sley_vm::ExecutionOutcome {
    execute(package, approved, vec![bytes_input(input), unit_input()])
}

fn assert_fingerprint_decode_ok(
    outcome: &sley_vm::ExecutionOutcome,
    expected_entity: &[u8],
    expected_body: &[u8],
    expected_fingerprint: &[u8],
) {
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(found) => match &found.data {
            ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                ConstData::Sequence(items) => {
                    assert_eq!(items.len(), 3, "fingerprint decode returns three fields");
                    match (&items[0].data, &items[1].data, &items[2].data) {
                        (
                            ConstData::Bytes(entity),
                            ConstData::Bytes(body),
                            ConstData::Bytes(fingerprint),
                        ) => {
                            assert_eq!(entity, expected_entity, "entity bytes match");
                            assert_eq!(body, expected_body, "body bytes match");
                            assert_eq!(
                                fingerprint, expected_fingerprint,
                                "fingerprint bytes match"
                            );
                        }
                        other => panic!("fingerprint tuple must carry Bytes, got {other:?}"),
                    }
                }
                other => panic!("fingerprint decode must return a tuple, got {other:?}"),
            },
            other => panic!("fingerprint decode must succeed, got {other:?}"),
        },
        other => panic!("fingerprint decode must succeed, got {other:?}"),
    }
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

#[test]
fn fingerprint_outer_decode_returns_native_fields() {
    let (package, approved) = admit(&fingerprint_decode_image());
    let entity = [0x81; 32];
    for (body, fingerprint) in [
        (Vec::new(), vec![0x91; 32]),
        ((0..127u8).collect::<Vec<_>>(), (0..32u8).collect()),
    ] {
        let payload = sley_scb1::encode_record(&[
            (1, entity.to_vec()),
            (2, body.clone()),
            (4, fingerprint.clone()),
        ])
        .expect("native fingerprint record encodes");
        let outcome = fingerprint_decode_call(&package, &approved, &payload);
        assert_fingerprint_decode_ok(&outcome, &entity, &body, &fingerprint);
        eprintln!(
            "RW080_FINGERPRINT_DECODE body={} stored={} fuel={} instr={} peak={}",
            body.len(),
            payload.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }
}

#[test]
fn fingerprint_outer_decode_rejects_missing_and_malformed_fingerprint() {
    let (package, approved) = admit(&fingerprint_decode_image());
    let entity = [0xa1; 32];
    let without = sley_scb1::encode_record(&[(1, entity.to_vec()), (2, b"body".to_vec())]).unwrap();
    assert_refusal(
        &fingerprint_decode_call(&package, &approved, &without),
        "SCB_FIELD_MISSING",
    );
    for (fingerprint, expected) in [
        (vec![0xb1; 31], "SCB_LENGTH_OVERFLOW"),
        (vec![0xb2; 33], "SCB_TRAILING_BYTES"),
    ] {
        let payload = sley_scb1::encode_record(&[
            (1, entity.to_vec()),
            (2, b"body".to_vec()),
            (4, fingerprint),
        ])
        .unwrap();
        assert_refusal(
            &fingerprint_decode_call(&package, &approved, &payload),
            expected,
        );
    }
    for (bad_entity, expected) in [
        (vec![0xa2; 31], "SCB_LENGTH_OVERFLOW"),
        (vec![0xa3; 33], "SCB_TRAILING_BYTES"),
    ] {
        let payload = sley_scb1::encode_record(&[
            (1, bad_entity),
            (2, b"body".to_vec()),
            (4, vec![0xb3; 32]),
        ])
        .unwrap();
        assert_refusal(
            &fingerprint_decode_call(&package, &approved, &payload),
            expected,
        );
    }
}

#[test]
fn fingerprint_outer_decode_preserves_structural_errors() {
    let (package, approved) = admit(&fingerprint_decode_image());
    let entity = [0xa1; 32];
    let field1 = {
        let mut bytes = vec![1, 32];
        bytes.extend_from_slice(&entity);
        bytes
    };
    let field2 = {
        let mut bytes = vec![2, 4];
        bytes.extend_from_slice(b"body");
        bytes
    };
    let valid = sley_scb1::encode_record(&[
        (1, entity.to_vec()),
        (2, b"body".to_vec()),
        (4, vec![0xc1; 32]),
    ])
    .unwrap();
    let mut trailing = valid.clone();
    trailing.push(0);
    let mut duplicate_field2 = vec![3];
    duplicate_field2.extend_from_slice(&field1);
    duplicate_field2.push(1);
    let mut ordered_before_field2 = vec![3];
    ordered_before_field2.extend_from_slice(&field1);
    ordered_before_field2.push(0);
    let mut duplicate_field3 = vec![3];
    duplicate_field3.extend_from_slice(&field1);
    duplicate_field3.extend_from_slice(&field2);
    duplicate_field3.push(2);
    let mut ordered_before_field3 = vec![3];
    ordered_before_field3.extend_from_slice(&field1);
    ordered_before_field3.extend_from_slice(&field2);
    ordered_before_field3.push(1);
    let mut label_instead = vec![3];
    label_instead.extend_from_slice(&field1);
    label_instead.extend_from_slice(&field2);
    label_instead.extend_from_slice(&[3, 0]);
    let mut unknown_field3 = vec![3];
    unknown_field3.extend_from_slice(&field1);
    unknown_field3.extend_from_slice(&field2);
    unknown_field3.extend_from_slice(&[5, 0]);
    let mut nonminimal_fingerprint_length = vec![3];
    nonminimal_fingerprint_length.extend_from_slice(&field1);
    nonminimal_fingerprint_length.extend_from_slice(&field2);
    nonminimal_fingerprint_length.extend_from_slice(&[4, 0xa0, 0]);
    let mut oversized_fingerprint = vec![3];
    oversized_fingerprint.extend_from_slice(&field1);
    oversized_fingerprint.extend_from_slice(&field2);
    oversized_fingerprint.push(4);
    oversized_fingerprint.extend_from_slice(&sley_scb1::encode_uvar(67_108_865));

    for (name, payload, expected) in [
        ("empty", Vec::new(), "SCB_LENGTH_OVERFLOW"),
        ("count_nonminimal", vec![0x83, 0], "SCB_VARINT_NON_MINIMAL"),
        ("count4_scope", vec![4], "SSMC_RESERVED_FIELD_PRESENT"),
        ("count5", vec![5], "SCB_FIELD_UNKNOWN"),
        ("field2_duplicate", duplicate_field2, "SCB_FIELD_DUPLICATE"),
        ("field2_order", ordered_before_field2, "SCB_FIELD_ORDER"),
        ("field3_duplicate", duplicate_field3, "SCB_FIELD_DUPLICATE"),
        ("field3_order", ordered_before_field3, "SCB_FIELD_ORDER"),
        ("label_scope", label_instead, "SSMC_RESERVED_FIELD_PRESENT"),
        ("field3_unknown", unknown_field3, "SCB_FIELD_UNKNOWN"),
        (
            "fingerprint_length_nonminimal",
            nonminimal_fingerprint_length,
            "SCB_VARINT_NON_MINIMAL",
        ),
        (
            "fingerprint_length_resource",
            oversized_fingerprint,
            "SCB_RESOURCE_LIMIT",
        ),
        ("trailing", trailing, "SCB_TRAILING_BYTES"),
    ] {
        assert_refusal(
            &fingerprint_decode_call(&package, &approved, &payload),
            expected,
        );
        eprintln!("RW080_FINGERPRINT_DECODE_REJECT {name} -> {expected}");
    }
}

#[test]
fn fingerprint_outer_decode_mutations_agree_with_native_outer_layer() {
    let (package, approved) = admit(&fingerprint_decode_image());
    let entity = [1; 32];
    let body = hex_decode("03080201020000020100");
    let base =
        sley_scb1::encode_record(&[(1, entity.to_vec()), (2, body), (4, vec![0xd1; 32])]).unwrap();
    let expected_epoch = SchemaEpochId::from_bytes([9; 32]);
    let mut state = 0xF17E_0042u64;
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    };
    let mut agree = 0u32;
    let mut scope = 0u32;
    for _ in 0..100 {
        let mut payload = base.clone();
        let suffix_start = payload.len() - 34;
        match next() % 4 {
            0 => {
                let index = usize::try_from(next()).expect("mutation index fits usize")
                    % (payload.len() - suffix_start)
                    + suffix_start;
                payload[index] ^= 1u8 << u32::try_from(next() % 8).expect("bit index fits u32");
            }
            1 => {
                payload.pop();
            }
            2 => payload.push(u8::try_from(next() % 256).expect("mutation byte fits u8")),
            _ => {
                payload.insert(
                    suffix_start,
                    u8::try_from(next() % 256).expect("mutation byte fits u8"),
                );
            }
        }

        let mut preimage = b"SLEYSCB1".to_vec();
        preimage.extend_from_slice(&sley_scb1::encode_uvar(1));
        preimage.extend_from_slice(&sley_scb1::encode_uvar(200));
        preimage.extend_from_slice(expected_epoch.as_bytes());
        preimage.extend_from_slice(&sley_scb1::encode_uvar(
            u64::try_from(payload.len()).expect("payload length fits u64"),
        ));
        preimage.extend_from_slice(&payload);
        let digest = sley_id::ObjectId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(digest.as_bytes());
        let native = sley_mutate::import_entity_object(expected_epoch, &stored);
        let outcome = fingerprint_decode_call(&package, &approved, &payload);
        let sley_code = match &outcome.termination {
            sley_vm::ExecutionTermination::Success(found) => match &found.data {
                ConstData::Result(ResultConst::Ok(_)) => None,
                ConstData::Result(ResultConst::Err(error)) => match &error.data {
                    ConstData::Bytes(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
                    other => panic!("fingerprint refusal must carry Bytes, got {other:?}"),
                },
                other => panic!("fingerprint decode must return Result, got {other:?}"),
            },
            other => panic!("fingerprint decode must terminate successfully, got {other:?}"),
        };
        match (native, sley_code) {
            (Ok(_), None) => agree += 1,
            (Ok(_), Some(code)) => {
                assert_eq!(code, "SSMC_RESERVED_FIELD_PRESENT");
                scope += 1;
            }
            (Err(native_error), None) => panic!(
                "Sley accepted native outer refusal {} for {}",
                native_error.code(),
                hex_encode(&payload)
            ),
            (Err(native_error), Some(code)) => {
                if code == "SSMC_RESERVED_FIELD_PRESENT" {
                    scope += 1;
                } else {
                    assert_eq!(
                        code,
                        native_error.code().to_string(),
                        "outer-layer mutation {}",
                        hex_encode(&payload)
                    );
                    agree += 1;
                }
            }
        }
    }
    eprintln!("RW080_FINGERPRINT_DECODE_MUT agree={agree} scope={scope} /100");
    assert_eq!(agree + scope, 100);
}
