//! Strict `DependencyBinding` (entity kind 18) body decoding construction.
//! Construction provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-dependency-binding-decode.md`.

use super::dependency_binding::dependency_stored;
use super::supported_dispatch::push_preallocated_block as append_block;
use super::*;
use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};

fn dependency_decode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            TypeExpr::Bytes,
        ])),
        error: Box::new(TypeExpr::Bytes),
    }
}

fn block_parameters(
    assembler: &mut Asm,
    namespace: u8,
    block: EntityId,
    value_types: &[TypeExpr],
) -> Vec<EntityId> {
    value_types
        .iter()
        .map(|value_type| {
            assembler.param(namespace, block, ParameterRole::Block, value_type.clone())
        })
        .collect()
}

fn record_state_types(outputs: usize) -> Vec<TypeExpr> {
    let mut types = vec![
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
        TypeExpr::Bool,
    ];
    types.extend((0..outputs).map(|_| TypeExpr::Bytes));
    types
}

fn decoded_record_state_types(outputs: usize) -> Vec<TypeExpr> {
    std::iter::once(u64_type())
        .chain(record_state_types(outputs))
        .collect()
}

fn parameter_values(parameters: &[EntityId]) -> Vec<ValueRef> {
    parameters.iter().copied().map(pav).collect()
}

#[derive(Clone, Copy)]
struct DecodeBlocks {
    function: EntityId,
    decode_function: EntityId,
    forward_error: EntityId,
    length_error: EntityId,
    trailing_error: EntityId,
    resource_error: EntityId,
    missing_error: EntityId,
    unknown_error: EntityId,
    duplicate_error: EntityId,
    order_error: EntityId,
    union_error: EntityId,
    scope_error: EntityId,
    invariant_trap: EntityId,
    width32: EntityId,
    width64: EntityId,
    max_length: EntityId,
    max_fields: EntityId,
    constant0: EntityId,
    constant1: EntityId,
    constant3: EntityId,
    constant18: EntityId,
    constant32: EntityId,
}

/// Calls the shared canonical uvar decoder while carrying the bounded record
/// state. The extracted union payload is its own `Bytes`, so an unterminated
/// uvar at the union boundary cannot consume an outer trailing byte.
fn build_record_uvar_stage(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    width: EntityId,
    outputs: usize,
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let decoded = assembler.id(ns.b);
    let state_types = record_state_types(outputs);
    let parameters = block_parameters(assembler, ns.p, head, &state_types);
    let width_value = assembler.cref(ns.o, head, width, u32_type());
    let call = assembler.op(
        ns.o,
        head,
        Opcode::CallDirect,
        vec![
            pav(parameters[3]),
            pav(parameters[0]),
            op_result(width_value),
            pav(parameters[4]),
        ],
        vec![decode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: control.decode_function,
            type_arguments: Vec::new(),
        }),
    );
    let mut success_arguments = vec![SwitchArgument::CasePayload];
    success_arguments.extend(parameters[1..].iter().copied().map(sav));
    append_block(
        assembler,
        head,
        control.function,
        parameters,
        vec![width_value, call],
        switch(
            op_result(call),
            vec![
                (BuiltinCase::Ok, decoded, success_arguments),
                (
                    BuiltinCase::Err,
                    control.forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let mut decoded_types = vec![TypeExpr::Tuple(vec![u64_type(), u64_type()])];
    decoded_types.extend_from_slice(&state_types[1..]);
    let decoded_parameters = block_parameters(assembler, ns.p, decoded, &decoded_types);
    let value = assembler.op(
        ns.o,
        decoded,
        Opcode::TupleGet,
        vec![pav(decoded_parameters[0])],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let position = assembler.op(
        ns.o,
        decoded,
        Opcode::TupleGet,
        vec![pav(decoded_parameters[0])],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let past_end = assembler.op(
        ns.o,
        decoded,
        Opcode::GreaterThan,
        vec![op_result(position), pav(decoded_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let mut destination_arguments = vec![op_result(value), op_result(position)];
    destination_arguments.extend(decoded_parameters[1..].iter().copied().map(pav));
    append_block(
        assembler,
        decoded,
        control.function,
        decoded_parameters,
        vec![value, position, past_end],
        cond(
            op_result(past_end),
            edge(control.length_error, Vec::new()),
            edge(destination, destination_arguments),
        ),
    );
    head
}

/// Validates one ordered record-field tag. Duplicate detection precedes the
/// order check, matching `decode_record_fields`.
#[allow(clippy::too_many_lines)]
fn build_field_tag_validator(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    outputs: usize,
    expected: u64,
    previous: Option<u64>,
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let mismatch = assembler.id(ns.b);
    let decoded_types = decoded_record_state_types(outputs);
    let parameters = block_parameters(assembler, ns.p, head, &decoded_types);
    let expected_constant = assembler.ku64(ns.k, u128::from(expected));
    let expected_value = assembler.cref(ns.o, head, expected_constant, u64_type());
    let matches = assembler.op(
        ns.o,
        head,
        Opcode::Equal,
        vec![pav(parameters[0]), op_result(expected_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        head,
        control.function,
        parameters.clone(),
        vec![expected_value, matches],
        cond(
            op_result(matches),
            edge(destination, parameter_values(&parameters[1..])),
            edge(mismatch, parameter_values(&parameters)),
        ),
    );

    let known_check = assembler.id(ns.b);
    let mismatch_parameters = block_parameters(assembler, ns.p, mismatch, &decoded_types);
    if let Some(previous) = previous {
        let order_check = assembler.id(ns.b);
        let previous_constant = assembler.ku64(ns.k, u128::from(previous));
        let previous_value = assembler.cref(ns.o, mismatch, previous_constant, u64_type());
        let is_duplicate = assembler.op(
            ns.o,
            mismatch,
            Opcode::Equal,
            vec![pav(mismatch_parameters[0]), op_result(previous_value)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        append_block(
            assembler,
            mismatch,
            control.function,
            mismatch_parameters.clone(),
            vec![previous_value, is_duplicate],
            cond(
                op_result(is_duplicate),
                edge(control.duplicate_error, Vec::new()),
                edge(order_check, parameter_values(&mismatch_parameters)),
            ),
        );
        let order_parameters = block_parameters(assembler, ns.p, order_check, &decoded_types);
        let previous_value = assembler.cref(ns.o, order_check, previous_constant, u64_type());
        let is_earlier = assembler.op(
            ns.o,
            order_check,
            Opcode::LessThan,
            vec![pav(order_parameters[0]), op_result(previous_value)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        append_block(
            assembler,
            order_check,
            control.function,
            order_parameters.clone(),
            vec![previous_value, is_earlier],
            cond(
                op_result(is_earlier),
                edge(control.order_error, Vec::new()),
                edge(known_check, parameter_values(&order_parameters)),
            ),
        );
    } else {
        append_block(
            assembler,
            mismatch,
            control.function,
            mismatch_parameters.clone(),
            Vec::new(),
            branch(edge(known_check, parameter_values(&mismatch_parameters))),
        );
    }

    let known_parameters = block_parameters(assembler, ns.p, known_check, &decoded_types);
    let mut operations = Vec::new();
    let mut known_match = None;
    for known_tag in [1u64, 2, 3] {
        let constant = assembler.ku64(ns.k, u128::from(known_tag));
        let value = assembler.cref(ns.o, known_check, constant, u64_type());
        let equals = assembler.op(
            ns.o,
            known_check,
            Opcode::Equal,
            vec![pav(known_parameters[0]), op_result(value)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        operations.extend([value, equals]);
        known_match = Some(match known_match {
            None => op_result(equals),
            Some(previous_match) => {
                let either = assembler.op(
                    ns.o,
                    known_check,
                    Opcode::BoolOr,
                    vec![previous_match, op_result(equals)],
                    vec![TypeExpr::Bool],
                    Immediate::None,
                );
                operations.push(either);
                op_result(either)
            }
        });
    }
    append_block(
        assembler,
        known_check,
        control.function,
        known_parameters,
        operations,
        cond(
            known_match.expect("DependencyBinding has known field tags"),
            edge(control.order_error, Vec::new()),
            edge(control.unknown_error, Vec::new()),
        ),
    );
    head
}

fn build_record_count_validator(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let minimum = assembler.id(ns.b);
    let maximum = assembler.id(ns.b);
    let decoded_types = decoded_record_state_types(0);

    let parameters = block_parameters(assembler, ns.p, head, &decoded_types);
    let max_fields = assembler.cref(ns.o, head, control.max_fields, u64_type());
    let too_many = assembler.op(
        ns.o,
        head,
        Opcode::GreaterThan,
        vec![pav(parameters[0]), op_result(max_fields)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        head,
        control.function,
        parameters.clone(),
        vec![max_fields, too_many],
        cond(
            op_result(too_many),
            edge(control.resource_error, Vec::new()),
            edge(minimum, parameter_values(&parameters)),
        ),
    );

    let minimum_parameters = block_parameters(assembler, ns.p, minimum, &decoded_types);
    let three = assembler.cref(ns.o, minimum, control.constant3, u64_type());
    let too_few = assembler.op(
        ns.o,
        minimum,
        Opcode::LessThan,
        vec![pav(minimum_parameters[0]), op_result(three)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        minimum,
        control.function,
        minimum_parameters.clone(),
        vec![three, too_few],
        cond(
            op_result(too_few),
            edge(control.missing_error, Vec::new()),
            edge(maximum, parameter_values(&minimum_parameters)),
        ),
    );

    let maximum_parameters = block_parameters(assembler, ns.p, maximum, &decoded_types);
    let three = assembler.cref(ns.o, maximum, control.constant3, u64_type());
    let too_many = assembler.op(
        ns.o,
        maximum,
        Opcode::GreaterThan,
        vec![pav(maximum_parameters[0]), op_result(three)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        maximum,
        control.function,
        maximum_parameters.clone(),
        vec![three, too_many],
        cond(
            op_result(too_many),
            edge(control.unknown_error, Vec::new()),
            edge(destination, parameter_values(&maximum_parameters[1..])),
        ),
    );
    head
}

/// Validates a sized fixed-32 field, then forwards its start/end and record
/// state to a byte-copy loop.
#[allow(clippy::too_many_lines)]
fn build_fixed_32_validator(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    outputs: usize,
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let add = assembler.id(ns.b);
    let bounded = assembler.id(ns.b);
    let minimum = assembler.id(ns.b);
    let maximum = assembler.id(ns.b);
    let decoded_types = decoded_record_state_types(outputs);
    let parameters = block_parameters(assembler, ns.p, head, &decoded_types);
    let max_length = assembler.cref(ns.o, head, control.max_length, u64_type());
    let too_large = assembler.op(
        ns.o,
        head,
        Opcode::GreaterThan,
        vec![pav(parameters[0]), op_result(max_length)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        head,
        control.function,
        parameters.clone(),
        vec![max_length, too_large],
        cond(
            op_result(too_large),
            edge(control.resource_error, Vec::new()),
            edge(add, parameter_values(&parameters)),
        ),
    );

    let add_parameters = block_parameters(assembler, ns.p, add, &decoded_types);
    let field_end = assembler.op(
        ns.o,
        add,
        Opcode::IntAddChecked,
        vec![pav(add_parameters[1]), pav(add_parameters[0])],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let mut bounded_arguments = vec![SwitchArgument::CasePayload];
    bounded_arguments.extend(add_parameters.iter().copied().map(sav));
    append_block(
        assembler,
        add,
        control.function,
        add_parameters,
        vec![field_end],
        switch(
            op_result(field_end),
            vec![
                (BuiltinCase::Ok, bounded, bounded_arguments),
                (BuiltinCase::Err, control.invariant_trap, Vec::new()),
            ],
        ),
    );

    let mut bounded_types = vec![u64_type()];
    bounded_types.extend(decoded_types.clone());
    let bounded_parameters = block_parameters(assembler, ns.p, bounded, &bounded_types);
    let crosses_record = assembler.op(
        ns.o,
        bounded,
        Opcode::GreaterThan,
        vec![pav(bounded_parameters[0]), pav(bounded_parameters[3])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        bounded,
        control.function,
        bounded_parameters.clone(),
        vec![crosses_record],
        cond(
            op_result(crosses_record),
            edge(control.length_error, Vec::new()),
            edge(minimum, parameter_values(&bounded_parameters)),
        ),
    );

    let minimum_parameters = block_parameters(assembler, ns.p, minimum, &bounded_types);
    let exact = assembler.cref(ns.o, minimum, control.constant32, u64_type());
    let short = assembler.op(
        ns.o,
        minimum,
        Opcode::LessThan,
        vec![pav(minimum_parameters[1]), op_result(exact)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        minimum,
        control.function,
        minimum_parameters.clone(),
        vec![exact, short],
        cond(
            op_result(short),
            edge(control.length_error, Vec::new()),
            edge(maximum, parameter_values(&minimum_parameters)),
        ),
    );

    let maximum_parameters = block_parameters(assembler, ns.p, maximum, &bounded_types);
    let exact = assembler.cref(ns.o, maximum, control.constant32, u64_type());
    let long = assembler.op(
        ns.o,
        maximum,
        Opcode::GreaterThan,
        vec![pav(maximum_parameters[1]), op_result(exact)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let mut destination_arguments = vec![pav(maximum_parameters[2]), pav(maximum_parameters[0])];
    destination_arguments.extend(maximum_parameters[3..].iter().copied().map(pav));
    append_block(
        assembler,
        maximum,
        control.function,
        maximum_parameters,
        vec![exact, long],
        cond(
            op_result(long),
            edge(control.trailing_error, Vec::new()),
            edge(destination, destination_arguments),
        ),
    );
    head
}

/// Copies one already-bounded field and appends the resulting `Bytes` value
/// to the threaded semantic outputs.
#[allow(clippy::too_many_lines)]
fn build_field_copy_loop(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    outputs: usize,
    destination: EntityId,
) -> EntityId {
    let setup = assembler.id(ns.b);
    let check = assembler.id(ns.b);
    let get = assembler.id(ns.b);
    let push = assembler.id(ns.b);
    let advance = assembler.id(ns.b);
    let done = assembler.id(ns.b);
    let copy_types = std::iter::once(u64_type())
        .chain(record_state_types(outputs))
        .collect::<Vec<_>>();
    let setup_parameters = block_parameters(assembler, ns.p, setup, &copy_types);
    let empty = assembler.op(
        ns.o,
        setup,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let mut check_arguments = parameter_values(&setup_parameters);
    check_arguments.push(op_result(empty));
    append_block(
        assembler,
        setup,
        control.function,
        setup_parameters,
        vec![empty],
        branch(edge(check, check_arguments)),
    );

    let mut loop_types = copy_types.clone();
    loop_types.push(u8vec_type());
    let check_parameters = block_parameters(assembler, ns.p, check, &loop_types);
    let continues = assembler.op(
        ns.o,
        check,
        Opcode::LessThan,
        vec![pav(check_parameters[0]), pav(check_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check,
        control.function,
        check_parameters.clone(),
        vec![continues],
        cond(
            op_result(continues),
            edge(get, parameter_values(&check_parameters)),
            edge(done, parameter_values(&check_parameters)),
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
    let mut push_arguments = vec![SwitchArgument::CasePayload];
    push_arguments.extend(get_parameters.iter().copied().map(sav));
    append_block(
        assembler,
        get,
        control.function,
        get_parameters,
        vec![byte],
        switch(
            op_result(byte),
            vec![
                (BuiltinCase::None, control.invariant_trap, Vec::new()),
                (BuiltinCase::Some, push, push_arguments),
            ],
        ),
    );

    let mut push_types = vec![u8_type()];
    push_types.extend(loop_types.clone());
    let push_parameters = block_parameters(assembler, ns.p, push, &push_types);
    let accumulator_index = push_parameters.len() - 1;
    let appended = assembler.op(
        ns.o,
        push,
        Opcode::AdapterInvoke,
        vec![
            pav(push_parameters[accumulator_index]),
            pav(push_parameters[0]),
        ],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    let mut advance_arguments = push_parameters[1..accumulator_index]
        .iter()
        .copied()
        .map(sav)
        .collect::<Vec<_>>();
    advance_arguments.push(SwitchArgument::CasePayload);
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
    let one = assembler.cref(ns.o, advance, control.constant1, u64_type());
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
                (BuiltinCase::Err, control.invariant_trap, Vec::new()),
            ],
        ),
    );

    let done_parameters = block_parameters(assembler, ns.p, done, &loop_types);
    let accumulator_index = done_parameters.len() - 1;
    let bytes = assembler.op(
        ns.o,
        done,
        Opcode::AdapterInvoke,
        vec![
            pav(done_parameters[5]),
            pav(done_parameters[accumulator_index]),
        ],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    let mut destination_arguments = vec![sav(done_parameters[1])];
    destination_arguments.extend(
        done_parameters[2..accumulator_index]
            .iter()
            .copied()
            .map(sav),
    );
    destination_arguments.push(SwitchArgument::CasePayload);
    append_block(
        assembler,
        done,
        control.function,
        done_parameters,
        vec![bytes],
        switch(
            op_result(bytes),
            vec![
                (BuiltinCase::Ok, destination, destination_arguments),
                (BuiltinCase::Err, control.resource_error, Vec::new()),
            ],
        ),
    );
    setup
}

/// Extracts the union value into a bounded byte/vector pair before record
/// parsing. Carries whether bytes follow the declared union value so native
/// nested parsing errors retain precedence over the eventual trailing error.
#[allow(clippy::too_many_lines)]
fn build_union_payload_copy(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    destination: EntityId,
) -> EntityId {
    let setup = assembler.id(ns.b);
    let check = assembler.id(ns.b);
    let get = assembler.id(ns.b);
    let push = assembler.id(ns.b);
    let advance = assembler.id(ns.b);
    let done = assembler.id(ns.b);
    let convert = assembler.id(ns.b);
    let types = vec![
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Unit,
        TypeExpr::Bool,
    ];
    let parameters = block_parameters(assembler, ns.p, setup, &types);
    let empty = assembler.op(
        ns.o,
        setup,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let mut arguments = parameter_values(&parameters);
    arguments.push(op_result(empty));
    append_block(
        assembler,
        setup,
        control.function,
        parameters,
        vec![empty],
        branch(edge(check, arguments)),
    );

    let mut loop_types = types.clone();
    loop_types.push(u8vec_type());
    let check_parameters = block_parameters(assembler, ns.p, check, &loop_types);
    let continues = assembler.op(
        ns.o,
        check,
        Opcode::LessThan,
        vec![pav(check_parameters[0]), pav(check_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check,
        control.function,
        check_parameters.clone(),
        vec![continues],
        cond(
            op_result(continues),
            edge(get, parameter_values(&check_parameters)),
            edge(done, parameter_values(&check_parameters)),
        ),
    );

    let get_parameters = block_parameters(assembler, ns.p, get, &loop_types);
    let byte = assembler.op(
        ns.o,
        get,
        Opcode::VectorGet,
        vec![pav(get_parameters[2]), pav(get_parameters[0])],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    let mut push_arguments = vec![SwitchArgument::CasePayload];
    push_arguments.extend(get_parameters.iter().copied().map(sav));
    append_block(
        assembler,
        get,
        control.function,
        get_parameters,
        vec![byte],
        switch(
            op_result(byte),
            vec![
                (BuiltinCase::None, control.invariant_trap, Vec::new()),
                (BuiltinCase::Some, push, push_arguments),
            ],
        ),
    );

    let mut push_types = vec![u8_type()];
    push_types.extend(loop_types.clone());
    let push_parameters = block_parameters(assembler, ns.p, push, &push_types);
    let accumulator_index = push_parameters.len() - 1;
    let appended = assembler.op(
        ns.o,
        push,
        Opcode::AdapterInvoke,
        vec![
            pav(push_parameters[accumulator_index]),
            pav(push_parameters[0]),
        ],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    let mut advance_arguments = push_parameters[1..accumulator_index]
        .iter()
        .copied()
        .map(sav)
        .collect::<Vec<_>>();
    advance_arguments.push(SwitchArgument::CasePayload);
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
    let one = assembler.cref(ns.o, advance, control.constant1, u64_type());
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
                (BuiltinCase::Err, control.invariant_trap, Vec::new()),
            ],
        ),
    );

    let done_parameters = block_parameters(assembler, ns.p, done, &loop_types);
    let accumulator_index = done_parameters.len() - 1;
    let bytes = assembler.op(
        ns.o,
        done,
        Opcode::AdapterInvoke,
        vec![
            pav(done_parameters[3]),
            pav(done_parameters[accumulator_index]),
        ],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    let mut convert_arguments = vec![sav(done_parameters[accumulator_index])];
    convert_arguments.push(SwitchArgument::CasePayload);
    convert_arguments.push(sav(done_parameters[3]));
    convert_arguments.push(sav(done_parameters[4]));
    append_block(
        assembler,
        done,
        control.function,
        done_parameters,
        vec![bytes],
        switch(
            op_result(bytes),
            vec![
                (BuiltinCase::Ok, convert, convert_arguments),
                (BuiltinCase::Err, control.resource_error, Vec::new()),
            ],
        ),
    );

    let convert_types = vec![
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
        TypeExpr::Bool,
    ];
    let convert_parameters = block_parameters(assembler, ns.p, convert, &convert_types);
    let length = assembler.op(
        ns.o,
        convert,
        Opcode::VectorLen,
        vec![pav(convert_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let zero = assembler.cref(ns.o, convert, control.constant0, u64_type());
    append_block(
        assembler,
        convert,
        control.function,
        convert_parameters.clone(),
        vec![length, zero],
        branch(edge(
            destination,
            vec![
                op_result(zero),
                op_result(length),
                pav(convert_parameters[0]),
                pav(convert_parameters[1]),
                pav(convert_parameters[2]),
                pav(convert_parameters[3]),
            ],
        )),
    );
    setup
}

fn build_final_return(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    result_type: &TypeExpr,
) -> EntityId {
    let head = assembler.id(ns.b);
    let outer_check = assembler.id(ns.b);
    let success = assembler.id(ns.b);
    let types = record_state_types(3);
    let parameters = block_parameters(assembler, ns.p, head, &types);
    let unread = assembler.op(
        ns.o,
        head,
        Opcode::LessThan,
        vec![pav(parameters[0]), pav(parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        head,
        control.function,
        parameters.clone(),
        vec![unread],
        cond(
            op_result(unread),
            edge(control.trailing_error, Vec::new()),
            edge(outer_check, parameter_values(&parameters)),
        ),
    );

    let outer_parameters = block_parameters(assembler, ns.p, outer_check, &types);
    append_block(
        assembler,
        outer_check,
        control.function,
        outer_parameters.clone(),
        Vec::new(),
        cond(
            pav(outer_parameters[5]),
            edge(control.trailing_error, Vec::new()),
            edge(success, parameter_values(&outer_parameters[6..])),
        ),
    );

    let success_types = vec![TypeExpr::Bytes, TypeExpr::Bytes, TypeExpr::Bytes];
    let success_parameters = block_parameters(assembler, ns.p, success, &success_types);
    let tuple_type = TypeExpr::Tuple(success_types);
    let tuple = assembler.op(
        ns.o,
        success,
        Opcode::TupleNew,
        parameter_values(&success_parameters),
        vec![tuple_type],
        Immediate::None,
    );
    let ok = assembler.op(
        ns.o,
        success,
        Opcode::ResultOk,
        vec![op_result(tuple)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        success,
        control.function,
        success_parameters,
        vec![tuple, ok],
        ret(op_result(ok)),
    );
    head
}

fn build_union_tag_validator(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let minimum = assembler.id(ns.b);
    let maximum = assembler.id(ns.b);
    let types = vec![
        u64_type(),
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
    ];
    let parameters = block_parameters(assembler, ns.p, head, &types);
    let eighteen = assembler.cref(ns.o, head, control.constant18, u64_type());
    let matches = assembler.op(
        ns.o,
        head,
        Opcode::Equal,
        vec![pav(parameters[0]), op_result(eighteen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        head,
        control.function,
        parameters.clone(),
        vec![eighteen, matches],
        cond(
            op_result(matches),
            edge(destination, parameter_values(&parameters[1..])),
            edge(minimum, parameter_values(&parameters)),
        ),
    );

    let minimum_parameters = block_parameters(assembler, ns.p, minimum, &types);
    let one = assembler.cref(ns.o, minimum, control.constant1, u64_type());
    let below = assembler.op(
        ns.o,
        minimum,
        Opcode::LessThan,
        vec![pav(minimum_parameters[0]), op_result(one)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        minimum,
        control.function,
        minimum_parameters.clone(),
        vec![one, below],
        cond(
            op_result(below),
            edge(control.union_error, Vec::new()),
            edge(maximum, parameter_values(&minimum_parameters)),
        ),
    );

    let maximum_parameters = block_parameters(assembler, ns.p, maximum, &types);
    let eighteen = assembler.cref(ns.o, maximum, control.constant18, u64_type());
    let above = assembler.op(
        ns.o,
        maximum,
        Opcode::GreaterThan,
        vec![pav(maximum_parameters[0]), op_result(eighteen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        maximum,
        control.function,
        maximum_parameters,
        vec![eighteen, above],
        cond(
            op_result(above),
            edge(control.union_error, Vec::new()),
            edge(control.scope_error, Vec::new()),
        ),
    );
    head
}

#[allow(clippy::too_many_lines)]
fn build_initial_uvar_stage(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    width: EntityId,
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let decoded = assembler.id(ns.b);
    let types = vec![
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
    ];
    let parameters = block_parameters(assembler, ns.p, head, &types);
    let width_value = assembler.cref(ns.o, head, width, u32_type());
    let call = assembler.op(
        ns.o,
        head,
        Opcode::CallDirect,
        vec![
            pav(parameters[3]),
            pav(parameters[0]),
            op_result(width_value),
            pav(parameters[4]),
        ],
        vec![decode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: control.decode_function,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        head,
        control.function,
        parameters.clone(),
        vec![width_value, call],
        switch(
            op_result(call),
            vec![
                (
                    BuiltinCase::Ok,
                    decoded,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(parameters[1]),
                        sav(parameters[2]),
                        sav(parameters[3]),
                        sav(parameters[4]),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    control.forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let decoded_types = vec![
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
    ];
    let decoded_parameters = block_parameters(assembler, ns.p, decoded, &decoded_types);
    let value = assembler.op(
        ns.o,
        decoded,
        Opcode::TupleGet,
        vec![pav(decoded_parameters[0])],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let position = assembler.op(
        ns.o,
        decoded,
        Opcode::TupleGet,
        vec![pav(decoded_parameters[0])],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let past_end = assembler.op(
        ns.o,
        decoded,
        Opcode::GreaterThan,
        vec![op_result(position), pav(decoded_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        decoded,
        control.function,
        decoded_parameters.clone(),
        vec![value, position, past_end],
        cond(
            op_result(past_end),
            edge(control.length_error, Vec::new()),
            edge(
                destination,
                vec![
                    op_result(value),
                    op_result(position),
                    pav(decoded_parameters[1]),
                    pav(decoded_parameters[2]),
                    pav(decoded_parameters[3]),
                    pav(decoded_parameters[4]),
                ],
            ),
        ),
    );
    head
}

#[allow(clippy::too_many_lines)]
fn build_union_length_validator(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let add = assembler.id(ns.b);
    let bounded = assembler.id(ns.b);
    let types = vec![
        u64_type(),
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
    ];
    let parameters = block_parameters(assembler, ns.p, head, &types);
    let max_length = assembler.cref(ns.o, head, control.max_length, u64_type());
    let too_large = assembler.op(
        ns.o,
        head,
        Opcode::GreaterThan,
        vec![pav(parameters[0]), op_result(max_length)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        head,
        control.function,
        parameters.clone(),
        vec![max_length, too_large],
        cond(
            op_result(too_large),
            edge(control.resource_error, Vec::new()),
            edge(add, parameter_values(&parameters)),
        ),
    );

    let add_parameters = block_parameters(assembler, ns.p, add, &types);
    let end = assembler.op(
        ns.o,
        add,
        Opcode::IntAddChecked,
        vec![pav(add_parameters[1]), pav(add_parameters[0])],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let mut success_arguments = vec![SwitchArgument::CasePayload];
    success_arguments.extend(add_parameters[1..].iter().copied().map(sav));
    append_block(
        assembler,
        add,
        control.function,
        add_parameters,
        vec![end],
        switch(
            op_result(end),
            vec![
                (BuiltinCase::Ok, bounded, success_arguments),
                (BuiltinCase::Err, control.invariant_trap, Vec::new()),
            ],
        ),
    );

    let bounded_types = vec![
        u64_type(),
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
    ];
    let bounded_parameters = block_parameters(assembler, ns.p, bounded, &bounded_types);
    let crosses_body = assembler.op(
        ns.o,
        bounded,
        Opcode::GreaterThan,
        vec![pav(bounded_parameters[0]), pav(bounded_parameters[2])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let outer_trailing = assembler.op(
        ns.o,
        bounded,
        Opcode::LessThan,
        vec![pav(bounded_parameters[0]), pav(bounded_parameters[2])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        bounded,
        control.function,
        bounded_parameters.clone(),
        vec![crosses_body, outer_trailing],
        cond(
            op_result(crosses_body),
            edge(control.length_error, Vec::new()),
            edge(
                destination,
                vec![
                    pav(bounded_parameters[1]),
                    pav(bounded_parameters[0]),
                    pav(bounded_parameters[3]),
                    pav(bounded_parameters[5]),
                    op_result(outer_trailing),
                ],
            ),
        ),
    );
    head
}

#[allow(clippy::too_many_lines)]
fn build_dependency_binding_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    decode_function: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = dependency_decode_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

    let e_length = assembler.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let e_trailing = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let e_resource = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let e_missing = assembler.kbytes(ns.k, b"SCB_FIELD_MISSING");
    let e_unknown = assembler.kbytes(ns.k, b"SCB_FIELD_UNKNOWN");
    let e_duplicate = assembler.kbytes(ns.k, b"SCB_FIELD_DUPLICATE");
    let e_order = assembler.kbytes(ns.k, b"SCB_FIELD_ORDER");
    let e_union = assembler.kbytes(ns.k, b"SCB_UNION_INVALID");
    let e_scope = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");

    let forward_error = assembler.id(ns.b);
    let forwarded = assembler.param(ns.p, forward_error, ParameterRole::Block, TypeExpr::Bytes);
    let forwarded_result = assembler.op(
        ns.o,
        forward_error,
        Opcode::ResultErr,
        vec![pav(forwarded)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_result],
        ret(op_result(forwarded_result)),
    );

    let control = DecodeBlocks {
        function,
        decode_function,
        forward_error,
        length_error: err_block(assembler, ns, function, result_type.clone(), e_length),
        trailing_error: err_block(assembler, ns, function, result_type.clone(), e_trailing),
        resource_error: err_block(assembler, ns, function, result_type.clone(), e_resource),
        missing_error: err_block(assembler, ns, function, result_type.clone(), e_missing),
        unknown_error: err_block(assembler, ns, function, result_type.clone(), e_unknown),
        duplicate_error: err_block(assembler, ns, function, result_type.clone(), e_duplicate),
        order_error: err_block(assembler, ns, function, result_type.clone(), e_order),
        union_error: err_block(assembler, ns, function, result_type.clone(), e_union),
        scope_error: err_block(assembler, ns, function, result_type.clone(), e_scope),
        invariant_trap: trap_block(assembler, ns, function),
        width32: assembler.ku32(ns.k, 32),
        width64: assembler.ku32(ns.k, 64),
        max_length: assembler.ku64(ns.k, 67_108_864),
        max_fields: assembler.ku64(ns.k, 65_535),
        constant0: assembler.ku64(ns.k, 0),
        constant1: assembler.ku64(ns.k, 1),
        constant3: assembler.ku64(ns.k, 3),
        constant18: assembler.ku64(ns.k, 18),
        constant32: assembler.ku64(ns.k, 32),
    };

    // Build tail first so every helper receives a concrete destination.
    let finish = build_final_return(assembler, ns, &control, &result_type);
    let copy3 = build_field_copy_loop(assembler, ns, &control, 2, finish);
    let length3_value = build_fixed_32_validator(assembler, ns, &control, 2, copy3);
    let length3 =
        build_record_uvar_stage(assembler, ns, &control, control.width64, 2, length3_value);
    let tag3_value = build_field_tag_validator(assembler, ns, &control, 2, 3, Some(2), length3);
    let tag3 = build_record_uvar_stage(assembler, ns, &control, control.width32, 2, tag3_value);

    let copy2 = build_field_copy_loop(assembler, ns, &control, 1, tag3);
    let length2_value = build_fixed_32_validator(assembler, ns, &control, 1, copy2);
    let length2 =
        build_record_uvar_stage(assembler, ns, &control, control.width64, 1, length2_value);
    let tag2_value = build_field_tag_validator(assembler, ns, &control, 1, 2, Some(1), length2);
    let tag2 = build_record_uvar_stage(assembler, ns, &control, control.width32, 1, tag2_value);

    let copy1 = build_field_copy_loop(assembler, ns, &control, 0, tag2);
    let length1_value = build_fixed_32_validator(assembler, ns, &control, 0, copy1);
    let length1 =
        build_record_uvar_stage(assembler, ns, &control, control.width64, 0, length1_value);
    let tag1_value = build_field_tag_validator(assembler, ns, &control, 0, 1, None, length1);
    let tag1 = build_record_uvar_stage(assembler, ns, &control, control.width32, 0, tag1_value);
    let count_value = build_record_count_validator(assembler, ns, &control, tag1);
    let count = build_record_uvar_stage(assembler, ns, &control, control.width64, 0, count_value);

    let union_payload = build_union_payload_copy(assembler, ns, &control, count);
    let union_length_value = build_union_length_validator(assembler, ns, &control, union_payload);
    let union_length =
        build_initial_uvar_stage(assembler, ns, &control, control.width64, union_length_value);
    let union_tag_value = build_union_tag_validator(assembler, ns, &control, union_length);
    let union_tag =
        build_initial_uvar_stage(assembler, ns, &control, control.width32, union_tag_value);

    let entry = assembler.id(ns.b);
    let converted = assembler.id(ns.b);
    let vector = assembler.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(unit), pav(body)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![vector],
        switch(
            op_result(vector),
            vec![
                (
                    BuiltinCase::Ok,
                    converted,
                    vec![SwitchArgument::CasePayload, sav(body), sav(unit)],
                ),
                (BuiltinCase::Err, control.resource_error, Vec::new()),
            ],
        ),
    );
    let converted_types = vec![u8vec_type(), TypeExpr::Bytes, TypeExpr::Unit];
    let converted_parameters = block_parameters(assembler, ns.p, converted, &converted_types);
    let total = assembler.op(
        ns.o,
        converted,
        Opcode::VectorLen,
        vec![pav(converted_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let zero = assembler.cref(ns.o, converted, control.constant0, u64_type());
    append_block(
        assembler,
        converted,
        function,
        converted_parameters.clone(),
        vec![total, zero],
        branch(edge(
            union_tag,
            vec![
                op_result(zero),
                op_result(total),
                pav(converted_parameters[0]),
                pav(converted_parameters[1]),
                pav(converted_parameters[2]),
            ],
        )),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![body, unit],
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

fn dependency_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_ns = Ns {
        k: 55,
        p: 56,
        b: 57,
        o: 58,
    };
    let dependency_ns = Ns {
        k: 59,
        p: 60,
        b: 61,
        o: 62,
    };
    let decode_function = eid(11, 2);
    let function = eid(11, 3);
    let (decode_graph, _) = build_decode(&mut assembler, decode_ns, decode_function);
    let graph =
        build_dependency_binding_decode(&mut assembler, dependency_ns, function, decode_function);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph, decode_graph],
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

fn dependency_decode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    body: &[u8],
) -> sley_vm::ExecutionOutcome {
    execute(package, approved, vec![bytes_input(body), unit_input()])
}

fn assert_dependency_decode_ok(
    outcome: &sley_vm::ExecutionOutcome,
    root: u8,
    package: u8,
    namespace: u8,
) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "DependencyBinding decoder did not return: {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &value.data else {
        panic!(
            "DependencyBinding decoder refused valid input: {:?}",
            value.data
        )
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!(
            "DependencyBinding result is not a tuple: {:?}",
            payload.data
        )
    };
    assert_eq!(fields.len(), 3, "three dependency identities");
    for (field, expected) in fields.iter().zip([root, package, namespace]) {
        let ConstData::Bytes(bytes) = &field.data else {
            panic!("dependency identity is not Bytes: {:?}", field.data)
        };
        assert_eq!(bytes, &vec![expected; 32]);
    }
}

#[test]
fn dependency_binding_decode_matches_native_semantics_for_distinct_inputs() {
    let image = dependency_decode_image();
    let (package, approved) = admit(&image);
    for (root, external, local) in [(1u8, 2u8, 3u8), (4, 5, 6), (7, 8, 9)] {
        let stored = dependency_stored(root, external, local);
        assert_eq!(program_native_code(&stored), "OK");
        let body = ns_body_of(&stored);
        assert_eq!(body.len(), 105, "fixed DependencyBinding body size");
        let outcome = dependency_decode_call(&package, &approved, &body);
        assert_dependency_decode_ok(&outcome, root, external, local);
        eprintln!(
            "DEP_DEC root{root} pkg{external} ns{local} fuel={} instr={} peak={}",
            outcome.fuel_used, outcome.instruction_count, outcome.peak_value_units
        );
    }
}

fn dependency_native_code(body: &[u8]) -> String {
    program_native_code(&ns_wrap_body(0xd1, body))
}

#[test]
fn dependency_binding_decode_matches_native_rejection_precedence() {
    let image = dependency_decode_image();
    let (package, approved) = admit(&image);
    let base = ns_body_of(&dependency_stored(1, 2, 3));
    let mut vectors: Vec<(&str, Vec<u8>)> = Vec::new();

    for (name, offset, value) in [
        ("count_missing", 2usize, 2u8),
        ("count_unknown", 2, 4),
        ("field1_unknown", 3, 2),
        ("field2_duplicate", 37, 1),
        ("field2_order", 37, 0),
        ("field2_unknown", 37, 4),
        ("field3_duplicate", 71, 2),
        ("field3_order", 71, 0),
        ("field3_unknown", 71, 4),
        ("root_short", 4, 31),
        ("root_long", 4, 33),
        ("package_short", 38, 31),
        ("package_long", 38, 33),
        ("local_short", 72, 31),
        ("local_long", 72, 33),
    ] {
        let mut body = base.clone();
        body[offset] = value;
        vectors.push((name, body));
    }

    let mut union_overrun = base.clone();
    union_overrun[1] = 104;
    vectors.push(("union_overrun", union_overrun));
    let mut union_cuts_field = base.clone();
    union_cuts_field[1] = 102;
    vectors.push(("union_cuts_field", union_cuts_field));
    let mut outer_trailing = base.clone();
    outer_trailing.push(0);
    vectors.push(("outer_trailing", outer_trailing));
    let mut inner_trailing = base.clone();
    inner_trailing[1] = 104;
    inner_trailing.push(0);
    vectors.push(("inner_trailing", inner_trailing));
    vectors.push(("bounded_unterminated_count", vec![18, 1, 0x80]));
    let mut nonminimal_count = base.clone();
    nonminimal_count[1] = 104;
    nonminimal_count.splice(2..3, [0x83, 0x00]);
    vectors.push(("nonminimal_count", nonminimal_count));
    let mut excessive_union = vec![18];
    excessive_union.extend_from_slice(&ns_uvar(67_108_865));
    vectors.push(("union_resource_limit", excessive_union));
    let mut excessive_count = base.clone();
    excessive_count[1] = 105;
    excessive_count.splice(2..3, ns_uvar(65_536));
    vectors.push(("count_resource_limit", excessive_count));
    let mut excessive_field = base.clone();
    excessive_field[1] = 106;
    excessive_field.splice(4..5, ns_uvar(67_108_865));
    vectors.push(("field_resource_limit", excessive_field));

    for (name, body) in vectors {
        let native = dependency_native_code(&body);
        assert_ne!(native, "OK", "{name} must be rejected natively");
        assert_refusal(&dependency_decode_call(&package, &approved, &body), &native);
        eprintln!("DEP_DEC_REJ {name} -> {native}");
    }

    for (name, tag, code) in [
        ("known_unimplemented", 17u8, "SSMC_RESERVED_FIELD_PRESENT"),
        ("zero_unknown", 0, "SCB_UNION_INVALID"),
        ("above_closed_union", 19, "SCB_UNION_INVALID"),
    ] {
        let mut body = base.clone();
        body[0] = tag;
        assert_refusal(&dependency_decode_call(&package, &approved, &body), code);
        eprintln!("DEP_DEC_SCOPE {name} -> {code}");
    }
}
