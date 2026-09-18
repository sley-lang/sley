//! Strict `DependencyBinding` (entity kind 18) body decoding construction.
//! Construction provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-dependency-binding-decode.md`.

use super::dependency_binding::dependency_stored;
use super::supported_dispatch::push_preallocated_block as append_block;
use super::*;
use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};

pub(super) fn dependency_decode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            TypeExpr::Bytes,
        ])),
        error: Box::new(TypeExpr::Bytes),
    }
}

pub(super) fn package_decode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            u64_type(),
            TypeExpr::Bytes,
            u64_type(),
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

fn record_state_types(output_types: &[TypeExpr]) -> Vec<TypeExpr> {
    let mut types = vec![
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
        TypeExpr::Bool,
    ];
    types.extend_from_slice(output_types);
    types
}

fn decoded_record_state_types(output_types: &[TypeExpr]) -> Vec<TypeExpr> {
    std::iter::once(u64_type())
        .chain(record_state_types(output_types))
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
    constant32: EntityId,
}

#[derive(Clone, Copy)]
struct CompactBlocks {
    function: EntityId,
    resource_error: EntityId,
    invariant_trap: EntityId,
    constant1: EntityId,
}

impl From<&DecodeBlocks> for CompactBlocks {
    fn from(control: &DecodeBlocks) -> Self {
        Self {
            function: control.function,
            resource_error: control.resource_error,
            invariant_trap: control.invariant_trap,
            constant1: control.constant1,
        }
    }
}

/// Calls the shared canonical uvar decoder while carrying the bounded record
/// state. The extracted union payload is its own `Bytes`, so an unterminated
/// uvar at the union boundary cannot consume an outer trailing byte.
fn build_record_uvar_stage(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    width: EntityId,
    output_types: &[TypeExpr],
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let decoded = assembler.id(ns.b);
    let state_types = record_state_types(output_types);
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

/// Handles the canonical one-byte value without a nested call frame. Any
/// other byte, including a continuation byte, falls through to the complete
/// shared uvar decoder so rejection semantics remain unchanged.
fn build_expected_uvar_fast_path(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    state_types: &[TypeExpr],
    expected: u8,
    destination: EntityId,
    fallback: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let compare = assembler.id(ns.b);
    let advance = assembler.id(ns.b);
    let parameters = block_parameters(assembler, ns.p, head, state_types);
    let byte = assembler.op(
        ns.o,
        head,
        Opcode::VectorGet,
        vec![pav(parameters[2]), pav(parameters[0])],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    let mut compare_arguments = vec![SwitchArgument::CasePayload];
    compare_arguments.extend(parameters.iter().copied().map(sav));
    append_block(
        assembler,
        head,
        control.function,
        parameters.clone(),
        vec![byte],
        switch(
            op_result(byte),
            vec![
                (
                    BuiltinCase::None,
                    fallback,
                    switch_parameter_values(&parameters),
                ),
                (BuiltinCase::Some, compare, compare_arguments),
            ],
        ),
    );

    let mut compare_types = vec![u8_type()];
    compare_types.extend_from_slice(state_types);
    let compare_parameters = block_parameters(assembler, ns.p, compare, &compare_types);
    let expected_constant = assembler.ku8(ns.k, u128::from(expected));
    let expected_value = assembler.cref(ns.o, compare, expected_constant, u8_type());
    let matches = assembler.op(
        ns.o,
        compare,
        Opcode::Equal,
        vec![pav(compare_parameters[0]), op_result(expected_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        compare,
        control.function,
        compare_parameters.clone(),
        vec![expected_value, matches],
        cond(
            op_result(matches),
            edge(advance, parameter_values(&compare_parameters[1..])),
            edge(fallback, parameter_values(&compare_parameters[1..])),
        ),
    );

    let advance_parameters = block_parameters(assembler, ns.p, advance, state_types);
    let one = assembler.cref(ns.o, advance, control.constant1, u64_type());
    let next = assembler.op(
        ns.o,
        advance,
        Opcode::IntAddChecked,
        vec![pav(advance_parameters[0]), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let expected_constant = assembler.ku64(ns.k, u128::from(expected));
    let expected_value = assembler.cref(ns.o, advance, expected_constant, u64_type());
    let mut destination_arguments = vec![oav(expected_value), SwitchArgument::CasePayload];
    destination_arguments.extend(advance_parameters[1..].iter().copied().map(sav));
    append_block(
        assembler,
        advance,
        control.function,
        advance_parameters,
        vec![one, next, expected_value],
        switch(
            op_result(next),
            vec![
                (BuiltinCase::Ok, destination, destination_arguments),
                (BuiltinCase::Err, control.invariant_trap, Vec::new()),
            ],
        ),
    );
    head
}

fn switch_parameter_values(parameters: &[EntityId]) -> Vec<SwitchArgument> {
    parameters.iter().copied().map(sav).collect()
}

fn build_expected_record_uvar_stage(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    width: EntityId,
    expected: u8,
    output_types: &[TypeExpr],
    destination: EntityId,
) -> EntityId {
    let state_types = record_state_types(output_types);
    let fallback =
        build_record_uvar_stage(assembler, ns, control, width, output_types, destination);
    build_expected_uvar_fast_path(
        assembler,
        ns,
        control,
        &state_types,
        expected,
        destination,
        fallback,
    )
}

/// Validates one ordered record-field tag. Duplicate detection precedes the
/// order check, matching `decode_record_fields`.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_field_tag_validator(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    output_types: &[TypeExpr],
    expected: u64,
    previous: Option<u64>,
    known_tags: &[u64],
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let mismatch = assembler.id(ns.b);
    let decoded_types = decoded_record_state_types(output_types);
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
    for known_tag in known_tags {
        let constant = assembler.ku64(ns.k, u128::from(*known_tag));
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
            known_match.expect("record has known field tags"),
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
    expected: u64,
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let minimum = assembler.id(ns.b);
    let maximum = assembler.id(ns.b);
    let decoded_types = decoded_record_state_types(&[]);

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
    let expected_constant = assembler.ku64(ns.k, u128::from(expected));
    let three = assembler.cref(ns.o, minimum, expected_constant, u64_type());
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
    let three = assembler.cref(ns.o, maximum, expected_constant, u64_type());
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
    output_types: &[TypeExpr],
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let add = assembler.id(ns.b);
    let bounded = assembler.id(ns.b);
    let minimum = assembler.id(ns.b);
    let maximum = assembler.id(ns.b);
    let decoded_types = decoded_record_state_types(output_types);
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

/// Validates one variable-width field against the record and epoch bounds,
/// then forwards its exact start/end range to the shared byte-copy loop.
fn build_bounded_field_validator(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    output_types: &[TypeExpr],
    destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let add = assembler.id(ns.b);
    let bounded = assembler.id(ns.b);
    let decoded_types = decoded_record_state_types(output_types);
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
    bounded_types.extend(decoded_types);
    let bounded_parameters = block_parameters(assembler, ns.p, bounded, &bounded_types);
    let crosses_record = assembler.op(
        ns.o,
        bounded,
        Opcode::GreaterThan,
        vec![pav(bounded_parameters[0]), pav(bounded_parameters[3])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let mut destination_arguments = vec![pav(bounded_parameters[2]), pav(bounded_parameters[0])];
    destination_arguments.extend(bounded_parameters[3..].iter().copied().map(pav));
    append_block(
        assembler,
        bounded,
        control.function,
        bounded_parameters,
        vec![crosses_record],
        cond(
            op_result(crosses_record),
            edge(control.length_error, Vec::new()),
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
    output_types: &[TypeExpr],
    destination: EntityId,
) -> EntityId {
    let setup = assembler.id(ns.b);
    let check = assembler.id(ns.b);
    let get = assembler.id(ns.b);
    let push = assembler.id(ns.b);
    let advance = assembler.id(ns.b);
    let done = assembler.id(ns.b);
    let copy_types = std::iter::once(u64_type())
        .chain(record_state_types(output_types))
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

/// Final fixed-field copy drops the record `Bytes` before allocating its
/// accumulator. No later uvar call needs that value, and releasing it keeps
/// the full envelope/outer/body composition inside the frozen value budget.
#[allow(clippy::too_many_lines)]
fn build_final_field_copy_loop(
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
    let input_types = std::iter::once(u64_type())
        .chain(record_state_types(&[TypeExpr::Bytes, TypeExpr::Bytes]))
        .collect::<Vec<_>>();
    let setup_parameters = block_parameters(assembler, ns.p, setup, &input_types);
    let empty = assembler.op(
        ns.o,
        setup,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let loop_types = vec![
        u64_type(),
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Unit,
        TypeExpr::Bool,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u8vec_type(),
    ];
    append_block(
        assembler,
        setup,
        control.function,
        setup_parameters.clone(),
        vec![empty],
        branch(edge(
            check,
            vec![
                pav(setup_parameters[0]),
                pav(setup_parameters[1]),
                pav(setup_parameters[2]),
                pav(setup_parameters[3]),
                pav(setup_parameters[5]),
                pav(setup_parameters[6]),
                pav(setup_parameters[7]),
                pav(setup_parameters[8]),
                op_result(empty),
            ],
        )),
    );

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
    let bytes = assembler.op(
        ns.o,
        done,
        Opcode::AdapterInvoke,
        vec![pav(done_parameters[4]), pav(done_parameters[8])],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    append_block(
        assembler,
        done,
        control.function,
        done_parameters.clone(),
        vec![bytes],
        switch(
            op_result(bytes),
            vec![
                (
                    BuiltinCase::Ok,
                    destination,
                    vec![
                        sav(done_parameters[1]),
                        sav(done_parameters[2]),
                        sav(done_parameters[5]),
                        sav(done_parameters[6]),
                        sav(done_parameters[7]),
                        SwitchArgument::CasePayload,
                    ],
                ),
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
    let types = vec![
        u64_type(),
        u64_type(),
        TypeExpr::Bool,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
    ];
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
            pav(outer_parameters[2]),
            edge(control.trailing_error, Vec::new()),
            edge(success, parameter_values(&outer_parameters[3..])),
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
    expected: u64,
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
    let expected_constant = assembler.ku64(ns.k, u128::from(expected));
    let eighteen = assembler.cref(ns.o, head, expected_constant, u64_type());
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
    let closed_max = assembler.ku64(ns.k, 18);
    let eighteen = assembler.cref(ns.o, maximum, closed_max, u64_type());
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

fn build_expected_initial_uvar_stage(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    width: EntityId,
    expected: u8,
    destination: EntityId,
) -> EntityId {
    let state_types = vec![
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
    ];
    let fallback = build_initial_uvar_stage(assembler, ns, control, width, destination);
    build_expected_uvar_fast_path(
        assembler,
        ns,
        control,
        &state_types,
        expected,
        destination,
        fallback,
    )
}

#[allow(clippy::too_many_lines)]
fn build_union_length_validator(
    assembler: &mut Asm,
    ns: Ns,
    control: &DecodeBlocks,
    exact_destination: EntityId,
    trailing_destination: EntityId,
) -> EntityId {
    let head = assembler.id(ns.b);
    let add = assembler.id(ns.b);
    let bounded = assembler.id(ns.b);
    let within_body = assembler.id(ns.b);
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
    append_block(
        assembler,
        bounded,
        control.function,
        bounded_parameters.clone(),
        vec![crosses_body],
        cond(
            op_result(crosses_body),
            edge(control.length_error, Vec::new()),
            edge(within_body, parameter_values(&bounded_parameters)),
        ),
    );

    let within_parameters = block_parameters(assembler, ns.p, within_body, &bounded_types);
    let outer_trailing = assembler.op(
        ns.o,
        within_body,
        Opcode::LessThan,
        vec![pav(within_parameters[0]), pav(within_parameters[2])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        within_body,
        control.function,
        within_parameters.clone(),
        vec![outer_trailing],
        cond(
            op_result(outer_trailing),
            edge(
                trailing_destination,
                vec![
                    pav(within_parameters[1]),
                    pav(within_parameters[0]),
                    pav(within_parameters[3]),
                    pav(within_parameters[5]),
                    op_result(outer_trailing),
                ],
            ),
            edge(
                exact_destination,
                vec![
                    pav(within_parameters[1]),
                    pav(within_parameters[0]),
                    pav(within_parameters[3]),
                    pav(within_parameters[4]),
                    pav(within_parameters[5]),
                    op_result(outer_trailing),
                ],
            ),
        ),
    );
    head
}

/// Copies one fixed canonical range while carrying only the source vector
/// and already-decoded identities. The function-level Unit parameter avoids
/// another live loop value.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_compact_copy_loop(
    assembler: &mut Asm,
    ns: Ns,
    control: CompactBlocks,
    unit: EntityId,
    start: u64,
    end: u64,
    outputs: usize,
    final_copy: bool,
    destination: EntityId,
) -> EntityId {
    let setup = assembler.id(ns.b);
    let check = assembler.id(ns.b);
    let get = assembler.id(ns.b);
    let push = assembler.id(ns.b);
    let advance = assembler.id(ns.b);
    let done = assembler.id(ns.b);
    let mut carry_types = vec![u8vec_type()];
    carry_types.extend((0..outputs).map(|_| TypeExpr::Bytes));
    let setup_parameters = block_parameters(assembler, ns.p, setup, &carry_types);
    let empty = assembler.op(
        ns.o,
        setup,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let start_constant = assembler.ku64(ns.k, u128::from(start));
    let start_value = assembler.cref(ns.o, setup, start_constant, u64_type());
    let end_constant = assembler.ku64(ns.k, u128::from(end));
    let end_value = assembler.cref(ns.o, setup, end_constant, u64_type());
    let mut check_arguments = vec![op_result(start_value), op_result(end_value)];
    check_arguments.extend(parameter_values(&setup_parameters));
    check_arguments.push(op_result(empty));
    append_block(
        assembler,
        setup,
        control.function,
        setup_parameters,
        vec![empty, start_value, end_value],
        branch(edge(check, check_arguments)),
    );

    let mut loop_types = vec![u64_type(), u64_type()];
    loop_types.extend(carry_types.clone());
    loop_types.push(u8vec_type());
    let check_parameters = block_parameters(assembler, ns.p, check, &loop_types);
    let done_arguments = if final_copy {
        parameter_values(&check_parameters[3..])
    } else {
        parameter_values(&check_parameters[2..])
    };
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
            edge(done, done_arguments),
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

    let done_types = if final_copy {
        (0..outputs)
            .map(|_| TypeExpr::Bytes)
            .chain(std::iter::once(u8vec_type()))
            .collect::<Vec<_>>()
    } else {
        carry_types
            .into_iter()
            .chain(std::iter::once(u8vec_type()))
            .collect()
    };
    let done_parameters = block_parameters(assembler, ns.p, done, &done_types);
    let accumulator_index = done_parameters.len() - 1;
    let bytes = assembler.op(
        ns.o,
        done,
        Opcode::AdapterInvoke,
        vec![pav(unit), pav(done_parameters[accumulator_index])],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    let mut destination_arguments = done_parameters[..accumulator_index]
        .iter()
        .copied()
        .map(sav)
        .collect::<Vec<_>>();
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

fn build_canonical_check_chain(
    assembler: &mut Asm,
    ns: Ns,
    control: CompactBlocks,
    checks: &[(u64, u8)],
    fallback_carry_type: Option<TypeExpr>,
    fallback: EntityId,
    success: EntityId,
) -> EntityId {
    let get_blocks = checks
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let compare_blocks = checks
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let carries_fallback_state = fallback_carry_type.is_some();
    let mut state_types = vec![u8vec_type()];
    if let Some(carry_type) = fallback_carry_type {
        state_types.push(carry_type);
        state_types.push(TypeExpr::Unit);
    }
    let mut compare_types = vec![u8_type()];
    compare_types.extend(state_types.clone());
    for (index, ((position, expected), (get_block, compare_block))) in checks
        .iter()
        .zip(
            get_blocks
                .iter()
                .copied()
                .zip(compare_blocks.iter().copied()),
        )
        .enumerate()
    {
        let get_parameters = block_parameters(assembler, ns.p, get_block, &state_types);
        let position_constant = assembler.ku64(ns.k, u128::from(*position));
        let position_value = assembler.cref(ns.o, get_block, position_constant, u64_type());
        let byte = assembler.op(
            ns.o,
            get_block,
            Opcode::VectorGet,
            vec![pav(get_parameters[0]), op_result(position_value)],
            vec![TypeExpr::Option(Box::new(u8_type()))],
            Immediate::None,
        );
        let mut compare_arguments = vec![SwitchArgument::CasePayload];
        compare_arguments.extend(get_parameters.iter().copied().map(sav));
        append_block(
            assembler,
            get_block,
            control.function,
            get_parameters.clone(),
            vec![position_value, byte],
            switch(
                op_result(byte),
                vec![
                    (
                        BuiltinCase::None,
                        fallback,
                        if carries_fallback_state {
                            switch_parameter_values(&get_parameters)
                        } else {
                            Vec::new()
                        },
                    ),
                    (BuiltinCase::Some, compare_block, compare_arguments),
                ],
            ),
        );

        let compare_parameters = block_parameters(assembler, ns.p, compare_block, &compare_types);
        let expected_constant = assembler.ku8(ns.k, u128::from(*expected));
        let expected_value = assembler.cref(ns.o, compare_block, expected_constant, u8_type());
        let matches = assembler.op(
            ns.o,
            compare_block,
            Opcode::Equal,
            vec![pav(compare_parameters[0]), op_result(expected_value)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        let next = get_blocks.get(index + 1).copied();
        let true_edge = match next {
            Some(next_get) => edge(next_get, parameter_values(&compare_parameters[1..])),
            None => edge(success, vec![pav(compare_parameters[1])]),
        };
        append_block(
            assembler,
            compare_block,
            control.function,
            compare_parameters.clone(),
            vec![expected_value, matches],
            cond(
                op_result(matches),
                true_edge,
                edge(
                    fallback,
                    if carries_fallback_state {
                        parameter_values(&compare_parameters[1..])
                    } else {
                        Vec::new()
                    },
                ),
            ),
        );
    }
    get_blocks[0]
}

#[allow(clippy::too_many_lines)]
fn build_program_canonical_check(
    assembler: &mut Asm,
    ns: Ns,
    control: CompactBlocks,
    checks: &[(u64, u8)],
    fallback: EntityId,
    success: EntityId,
) -> EntityId {
    let entry = assembler.id(ns.b);
    let stages = checks
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();

    let source = assembler.param(ns.p, entry, ParameterRole::Block, u8vec_type());
    let first_position = assembler.ku64(ns.k, u128::from(checks[0].0));
    let first_position_value = assembler.cref(ns.o, entry, first_position, u64_type());
    let first = assembler.op(
        ns.o,
        entry,
        Opcode::VectorGet,
        vec![pav(source), op_result(first_position_value)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    append_block(
        assembler,
        entry,
        control.function,
        vec![source],
        vec![first_position_value, first],
        switch(
            op_result(first),
            vec![
                (BuiltinCase::None, control.invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    stages[0],
                    vec![sav(source), SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    for (index, ((_, expected), block)) in checks.iter().zip(stages.iter().copied()).enumerate() {
        let source = assembler.param(ns.p, block, ParameterRole::Block, u8vec_type());
        let accumulated =
            (index > 0).then(|| assembler.param(ns.p, block, ParameterRole::Block, TypeExpr::Bool));
        let actual = assembler.param(ns.p, block, ParameterRole::Block, u8_type());
        let expected_constant = assembler.ku8(ns.k, u128::from(*expected));
        let expected_value = assembler.cref(ns.o, block, expected_constant, u8_type());
        let matches = assembler.op(
            ns.o,
            block,
            Opcode::Equal,
            vec![pav(actual), op_result(expected_value)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        let mut parameters = vec![source];
        parameters.extend(accumulated);
        parameters.push(actual);
        let mut operations = vec![expected_value, matches];
        let combined = accumulated.map_or_else(
            || op_result(matches),
            |prior| {
                let value = assembler.op(
                    ns.o,
                    block,
                    Opcode::BoolAnd,
                    vec![pav(prior), op_result(matches)],
                    vec![TypeExpr::Bool],
                    Immediate::None,
                );
                operations.push(value);
                op_result(value)
            },
        );
        if let Some((next_position, _)) = checks.get(index + 1) {
            let position_constant = assembler.ku64(ns.k, u128::from(*next_position));
            let position_value = assembler.cref(ns.o, block, position_constant, u64_type());
            let next = assembler.op(
                ns.o,
                block,
                Opcode::VectorGet,
                vec![pav(source), op_result(position_value)],
                vec![TypeExpr::Option(Box::new(u8_type()))],
                Immediate::None,
            );
            operations.extend([position_value, next]);
            append_block(
                assembler,
                block,
                control.function,
                parameters,
                operations,
                switch(
                    op_result(next),
                    vec![
                        (BuiltinCase::None, control.invariant_trap, Vec::new()),
                        (
                            BuiltinCase::Some,
                            stages[index + 1],
                            vec![
                                sav(source),
                                SwitchArgument::Value(combined),
                                SwitchArgument::CasePayload,
                            ],
                        ),
                    ],
                ),
            );
        } else {
            append_block(
                assembler,
                block,
                control.function,
                parameters,
                operations,
                cond(
                    combined,
                    edge(success, vec![pav(source)]),
                    edge(fallback, Vec::new()),
                ),
            );
        }
    }
    entry
}

/// Compares a fixed template byte by byte while skipping dynamic ranges.
/// One counted loop replaces a separate graph stage for every framing byte.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_masked_program_canonical_check(
    assembler: &mut Asm,
    ns: Ns,
    control: CompactBlocks,
    unit: EntityId,
    template: &[u8],
    skips: [(u64, u64); 2],
    fallback: EntityId,
    success: EntityId,
) -> EntityId {
    let entry = assembler.id(ns.b);
    let check = assembler.id(ns.b);
    let skip_first = assembler.id(ns.b);
    let skip_second = assembler.id(ns.b);
    let compare = assembler.id(ns.b);
    let next = assembler.id(ns.b);
    let template_constant = assembler.kbytes(ns.k, template);
    let zero_constant = assembler.ku64(ns.k, 0);
    let one_constant = assembler.ku64(ns.k, 1);
    let length_constant = assembler.ku64(
        ns.k,
        u128::try_from(template.len()).expect("template length fits u128"),
    );
    let skip_constants = skips.map(|(start, destination)| {
        (
            assembler.ku64(ns.k, u128::from(start)),
            assembler.ku64(ns.k, u128::from(destination)),
        )
    });
    let option_u8 = TypeExpr::Option(Box::new(u8_type()));

    let source = assembler.param(ns.p, entry, ParameterRole::Block, u8vec_type());
    let template_bytes = assembler.cref(ns.o, entry, template_constant, TypeExpr::Bytes);
    let template_vector = assembler.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(unit), op_result(template_bytes)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    let zero = assembler.cref(ns.o, entry, zero_constant, u64_type());
    append_block(
        assembler,
        entry,
        control.function,
        vec![source],
        vec![template_bytes, template_vector, zero],
        switch(
            op_result(template_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![oav(zero), sav(source), SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, fallback, Vec::new()),
            ],
        ),
    );

    let check_index = assembler.param(ns.p, check, ParameterRole::Block, u64_type());
    let check_source = assembler.param(ns.p, check, ParameterRole::Block, u8vec_type());
    let check_template = assembler.param(ns.p, check, ParameterRole::Block, u8vec_type());
    let length = assembler.cref(ns.o, check, length_constant, u64_type());
    let more = assembler.op(
        ns.o,
        check,
        Opcode::LessThan,
        vec![pav(check_index), op_result(length)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check,
        control.function,
        vec![check_index, check_source, check_template],
        vec![length, more],
        cond(
            op_result(more),
            edge(
                skip_first,
                vec![pav(check_index), pav(check_source), pav(check_template)],
            ),
            edge(success, Vec::new()),
        ),
    );

    let first_index = assembler.param(ns.p, skip_first, ParameterRole::Block, u64_type());
    let first_source = assembler.param(ns.p, skip_first, ParameterRole::Block, u8vec_type());
    let first_template = assembler.param(ns.p, skip_first, ParameterRole::Block, u8vec_type());
    let first_start = assembler.cref(ns.o, skip_first, skip_constants[0].0, u64_type());
    let first_destination = assembler.cref(ns.o, skip_first, skip_constants[0].1, u64_type());
    let at_first_skip = assembler.op(
        ns.o,
        skip_first,
        Opcode::Equal,
        vec![pav(first_index), op_result(first_start)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        skip_first,
        control.function,
        vec![first_index, first_source, first_template],
        vec![first_start, first_destination, at_first_skip],
        cond(
            op_result(at_first_skip),
            edge(
                check,
                vec![
                    op_result(first_destination),
                    pav(first_source),
                    pav(first_template),
                ],
            ),
            edge(
                skip_second,
                vec![pav(first_index), pav(first_source), pav(first_template)],
            ),
        ),
    );

    let second_index = assembler.param(ns.p, skip_second, ParameterRole::Block, u64_type());
    let second_source = assembler.param(ns.p, skip_second, ParameterRole::Block, u8vec_type());
    let second_template = assembler.param(ns.p, skip_second, ParameterRole::Block, u8vec_type());
    let second_start = assembler.cref(ns.o, skip_second, skip_constants[1].0, u64_type());
    let second_destination = assembler.cref(ns.o, skip_second, skip_constants[1].1, u64_type());
    let at_second_skip = assembler.op(
        ns.o,
        skip_second,
        Opcode::Equal,
        vec![pav(second_index), op_result(second_start)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        skip_second,
        control.function,
        vec![second_index, second_source, second_template],
        vec![second_start, second_destination, at_second_skip],
        cond(
            op_result(at_second_skip),
            edge(
                check,
                vec![
                    op_result(second_destination),
                    pav(second_source),
                    pav(second_template),
                ],
            ),
            edge(
                compare,
                vec![pav(second_index), pav(second_source), pav(second_template)],
            ),
        ),
    );

    let compare_index = assembler.param(ns.p, compare, ParameterRole::Block, u64_type());
    let compare_source = assembler.param(ns.p, compare, ParameterRole::Block, u8vec_type());
    let compare_template = assembler.param(ns.p, compare, ParameterRole::Block, u8vec_type());
    let actual = assembler.op(
        ns.o,
        compare,
        Opcode::VectorGet,
        vec![pav(compare_source), pav(compare_index)],
        vec![option_u8.clone()],
        Immediate::None,
    );
    let expected = assembler.op(
        ns.o,
        compare,
        Opcode::VectorGet,
        vec![pav(compare_template), pav(compare_index)],
        vec![option_u8],
        Immediate::None,
    );
    let matches = assembler.op(
        ns.o,
        compare,
        Opcode::Equal,
        vec![op_result(actual), op_result(expected)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        compare,
        control.function,
        vec![compare_index, compare_source, compare_template],
        vec![actual, expected, matches],
        cond(
            op_result(matches),
            edge(
                next,
                vec![
                    pav(compare_index),
                    pav(compare_source),
                    pav(compare_template),
                ],
            ),
            edge(fallback, Vec::new()),
        ),
    );

    let next_index = assembler.param(ns.p, next, ParameterRole::Block, u64_type());
    let next_source = assembler.param(ns.p, next, ParameterRole::Block, u8vec_type());
    let next_template = assembler.param(ns.p, next, ParameterRole::Block, u8vec_type());
    let one = assembler.cref(ns.o, next, one_constant, u64_type());
    let advanced = assembler.op(
        ns.o,
        next,
        Opcode::IntAddChecked,
        vec![pav(next_index), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        next,
        control.function,
        vec![next_index, next_source, next_template],
        vec![one, advanced],
        switch(
            op_result(advanced),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(next_source),
                        sav(next_template),
                    ],
                ),
                (BuiltinCase::Err, fallback, Vec::new()),
            ],
        ),
    );
    entry
}

fn build_canonical_return(
    assembler: &mut Asm,
    ns: Ns,
    control: CompactBlocks,
    result_type: &TypeExpr,
) -> EntityId {
    let block = assembler.id(ns.b);
    let types = vec![TypeExpr::Bytes, TypeExpr::Bytes, TypeExpr::Bytes];
    let parameters = block_parameters(assembler, ns.p, block, &types);
    let tuple_type = TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes, TypeExpr::Bytes]);
    let tuple = assembler.op(
        ns.o,
        block,
        Opcode::TupleNew,
        parameter_values(&parameters),
        vec![tuple_type],
        Immediate::None,
    );
    let ok = assembler.op(
        ns.o,
        block,
        Opcode::ResultOk,
        vec![op_result(tuple)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        block,
        control.function,
        parameters,
        vec![tuple, ok],
        ret(op_result(ok)),
    );
    block
}

#[allow(clippy::too_many_lines)]
pub(super) fn build_dependency_binding_decode_from_vector(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    decode_function: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = dependency_decode_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let input_vector = assembler.param(ns.p, function, ParameterRole::Function, u8vec_type());
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
        constant32: assembler.ku64(ns.k, 32),
    };
    let compact_control = CompactBlocks::from(&control);
    let outputs0 = Vec::<TypeExpr>::new();
    let outputs1 = vec![TypeExpr::Bytes];
    let outputs2 = vec![TypeExpr::Bytes, TypeExpr::Bytes];
    let known_tags = [1, 2, 3];

    let canonical_return = build_canonical_return(assembler, ns, compact_control, &result_type);
    let canonical_copy3 = build_compact_copy_loop(
        assembler,
        ns,
        compact_control,
        unit,
        73,
        105,
        2,
        true,
        canonical_return,
    );
    let canonical_copy2 = build_compact_copy_loop(
        assembler,
        ns,
        compact_control,
        unit,
        39,
        71,
        1,
        false,
        canonical_copy3,
    );
    let canonical_copy1 = build_compact_copy_loop(
        assembler,
        ns,
        compact_control,
        unit,
        5,
        37,
        0,
        false,
        canonical_copy2,
    );

    // Build tail first so every helper receives a concrete destination.
    let finish = build_final_return(assembler, ns, &control, &result_type);
    let copy3 = build_final_field_copy_loop(assembler, ns, &control, finish);
    let length3_value = build_fixed_32_validator(assembler, ns, &control, &outputs2, copy3);
    let length3 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        32,
        &outputs2,
        length3_value,
    );
    let tag3_value = build_field_tag_validator(
        assembler,
        ns,
        &control,
        &outputs2,
        3,
        Some(2),
        &known_tags,
        length3,
    );
    let tag3 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width32,
        3,
        &outputs2,
        tag3_value,
    );

    let copy2 = build_field_copy_loop(assembler, ns, &control, &outputs1, tag3);
    let length2_value = build_fixed_32_validator(assembler, ns, &control, &outputs1, copy2);
    let length2 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        32,
        &outputs1,
        length2_value,
    );
    let tag2_value = build_field_tag_validator(
        assembler,
        ns,
        &control,
        &outputs1,
        2,
        Some(1),
        &known_tags,
        length2,
    );
    let tag2 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width32,
        2,
        &outputs1,
        tag2_value,
    );

    let copy1 = build_field_copy_loop(assembler, ns, &control, &outputs0, tag2);
    let length1_value = build_fixed_32_validator(assembler, ns, &control, &outputs0, copy1);
    let length1 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        32,
        &outputs0,
        length1_value,
    );
    let tag1_value = build_field_tag_validator(
        assembler,
        ns,
        &control,
        &outputs0,
        1,
        None,
        &known_tags,
        length1,
    );
    let tag1 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width32,
        1,
        &outputs0,
        tag1_value,
    );
    let count_value = build_record_count_validator(assembler, ns, &control, 3, tag1);
    let count = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        3,
        &outputs0,
        count_value,
    );

    let union_payload = build_union_payload_copy(assembler, ns, &control, count);
    let union_length_value =
        build_union_length_validator(assembler, ns, &control, count, union_payload);
    let union_length = build_expected_initial_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        103,
        union_length_value,
    );
    let union_tag_value = build_union_tag_validator(assembler, ns, &control, 18, union_length);
    let union_tag = build_expected_initial_uvar_stage(
        assembler,
        ns,
        &control,
        control.width32,
        18,
        union_tag_value,
    );

    let fallback = assembler.id(ns.b);
    let fallback_types = vec![u8vec_type(), TypeExpr::Bytes, TypeExpr::Unit];
    let fallback_parameters = block_parameters(assembler, ns.p, fallback, &fallback_types);
    let fallback_total = assembler.op(
        ns.o,
        fallback,
        Opcode::VectorLen,
        vec![pav(fallback_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let fallback_zero = assembler.cref(ns.o, fallback, control.constant0, u64_type());
    append_block(
        assembler,
        fallback,
        function,
        fallback_parameters.clone(),
        vec![fallback_total, fallback_zero],
        branch(edge(
            union_tag,
            vec![
                op_result(fallback_zero),
                op_result(fallback_total),
                pav(fallback_parameters[0]),
                pav(fallback_parameters[1]),
                pav(fallback_parameters[2]),
            ],
        )),
    );

    let canonical_checks = build_canonical_check_chain(
        assembler,
        ns,
        compact_control,
        &[
            (0, 18),
            (1, 103),
            (2, 3),
            (3, 1),
            (4, 32),
            (37, 2),
            (38, 32),
            (71, 3),
            (72, 32),
        ],
        Some(TypeExpr::Bytes),
        fallback,
        canonical_copy1,
    );
    let entry = assembler.id(ns.b);
    let total = assembler.op(
        ns.o,
        entry,
        Opcode::VectorLen,
        vec![pav(input_vector)],
        vec![u64_type()],
        Immediate::None,
    );
    let expected_length_constant = assembler.ku64(ns.k, 105);
    let expected_length = assembler.cref(ns.o, entry, expected_length_constant, u64_type());
    let canonical_length = assembler.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![op_result(total), op_result(expected_length)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![total, expected_length, canonical_length],
        cond(
            op_result(canonical_length),
            edge(
                canonical_checks,
                vec![pav(input_vector), pav(body), pav(unit)],
            ),
            edge(fallback, vec![pav(input_vector), pav(body), pav(unit)]),
        ),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![body, input_vector, unit],
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

#[allow(clippy::similar_names, clippy::too_many_lines)]
pub(super) fn build_package_decode_from_vector(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    decode_function: EntityId,
    set_decode_function: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = package_decode_result_type();
    let set_result_type = entity_set_decode_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let input_vector = assembler.param(ns.p, function, ParameterRole::Function, u8vec_type());
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
        constant32: assembler.ku64(ns.k, 32),
    };

    let outputs0 = Vec::<TypeExpr>::new();
    let outputs1 = vec![TypeExpr::Bytes];
    let outputs2 = vec![TypeExpr::Bytes, TypeExpr::Bytes];
    let outputs3 = vec![
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u64_type(),
    ];
    let outputs4_raw = vec![
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u64_type(),
        TypeExpr::Bytes,
    ];
    let known_tags = [1, 2, 3, 4];

    let validate_dependencies = assembler.id(ns.b);
    let dependencies_decoded = assembler.id(ns.b);
    let validate_exports = assembler.id(ns.b);
    let exports_decoded = assembler.id(ns.b);
    let record_trailing = assembler.id(ns.b);
    let outer_trailing = assembler.id(ns.b);
    let success = assembler.id(ns.b);

    let success_types = vec![
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u64_type(),
        TypeExpr::Bytes,
        u64_type(),
    ];
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
        function,
        success_parameters,
        vec![tuple, ok],
        ret(op_result(ok)),
    );

    let trailing_types = vec![
        u64_type(),
        u64_type(),
        TypeExpr::Bool,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u64_type(),
        TypeExpr::Bytes,
        u64_type(),
    ];
    let record_parameters = block_parameters(assembler, ns.p, record_trailing, &trailing_types);
    let unread = assembler.op(
        ns.o,
        record_trailing,
        Opcode::LessThan,
        vec![pav(record_parameters[0]), pav(record_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        record_trailing,
        function,
        record_parameters.clone(),
        vec![unread],
        cond(
            op_result(unread),
            edge(control.trailing_error, Vec::new()),
            edge(outer_trailing, parameter_values(&record_parameters[2..])),
        ),
    );

    let outer_types = trailing_types[2..].to_vec();
    let outer_parameters = block_parameters(assembler, ns.p, outer_trailing, &outer_types);
    append_block(
        assembler,
        outer_trailing,
        function,
        outer_parameters.clone(),
        Vec::new(),
        cond(
            pav(outer_parameters[0]),
            edge(control.trailing_error, Vec::new()),
            edge(success, parameter_values(&outer_parameters[1..])),
        ),
    );

    let export_decoded_types = vec![
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes, u64_type()]),
        u64_type(),
        u64_type(),
        TypeExpr::Bool,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u64_type(),
    ];
    let export_parameters =
        block_parameters(assembler, ns.p, exports_decoded, &export_decoded_types);
    let export_members = assembler.op(
        ns.o,
        exports_decoded,
        Opcode::TupleGet,
        vec![pav(export_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let export_count = assembler.op(
        ns.o,
        exports_decoded,
        Opcode::TupleGet,
        vec![pav(export_parameters[0])],
        vec![u64_type()],
        Immediate::Index(2),
    );
    append_block(
        assembler,
        exports_decoded,
        function,
        export_parameters.clone(),
        vec![export_members, export_count],
        branch(edge(
            record_trailing,
            vec![
                pav(export_parameters[1]),
                pav(export_parameters[2]),
                pav(export_parameters[3]),
                pav(export_parameters[4]),
                pav(export_parameters[5]),
                pav(export_parameters[6]),
                pav(export_parameters[7]),
                op_result(export_members),
                op_result(export_count),
            ],
        )),
    );

    let export_state_types = record_state_types(&outputs4_raw);
    let validate_export_parameters =
        block_parameters(assembler, ns.p, validate_exports, &export_state_types);
    let decoded_exports = assembler.op(
        ns.o,
        validate_exports,
        Opcode::CallDirect,
        vec![
            pav(validate_export_parameters[7]),
            pav(validate_export_parameters[10]),
            pav(validate_export_parameters[4]),
        ],
        vec![set_result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: set_decode_function,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        validate_exports,
        function,
        validate_export_parameters.clone(),
        vec![decoded_exports],
        switch(
            op_result(decoded_exports),
            vec![
                (
                    BuiltinCase::Ok,
                    exports_decoded,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(validate_export_parameters[0]),
                        sav(validate_export_parameters[1]),
                        sav(validate_export_parameters[5]),
                        sav(validate_export_parameters[6]),
                        sav(validate_export_parameters[7]),
                        sav(validate_export_parameters[8]),
                        sav(validate_export_parameters[9]),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let copy4 = build_field_copy_loop(assembler, ns, &control, &outputs3, validate_exports);
    let length4_value = build_bounded_field_validator(assembler, ns, &control, &outputs3, copy4);
    let length4 = build_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        &outputs3,
        length4_value,
    );
    let tag4_value = build_field_tag_validator(
        assembler,
        ns,
        &control,
        &outputs3,
        4,
        Some(3),
        &known_tags,
        length4,
    );
    let tag4 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width32,
        4,
        &outputs3,
        tag4_value,
    );

    let dependency_decoded_types = std::iter::once(TypeExpr::Tuple(vec![
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u64_type(),
    ]))
    .chain(record_state_types(&outputs2))
    .collect::<Vec<_>>();
    let dependency_parameters = block_parameters(
        assembler,
        ns.p,
        dependencies_decoded,
        &dependency_decoded_types,
    );
    let dependency_members = assembler.op(
        ns.o,
        dependencies_decoded,
        Opcode::TupleGet,
        vec![pav(dependency_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let dependency_count = assembler.op(
        ns.o,
        dependencies_decoded,
        Opcode::TupleGet,
        vec![pav(dependency_parameters[0])],
        vec![u64_type()],
        Immediate::Index(2),
    );
    let mut tag4_arguments = parameter_values(&dependency_parameters[1..]);
    tag4_arguments.extend([op_result(dependency_members), op_result(dependency_count)]);
    append_block(
        assembler,
        dependencies_decoded,
        function,
        dependency_parameters,
        vec![dependency_members, dependency_count],
        branch(edge(tag4, tag4_arguments)),
    );

    let dependency_raw_types = vec![TypeExpr::Bytes, TypeExpr::Bytes, TypeExpr::Bytes];
    let dependency_state_types = record_state_types(&dependency_raw_types);
    let validate_dependency_parameters = block_parameters(
        assembler,
        ns.p,
        validate_dependencies,
        &dependency_state_types,
    );
    let decoded_dependencies = assembler.op(
        ns.o,
        validate_dependencies,
        Opcode::CallDirect,
        vec![
            pav(validate_dependency_parameters[6]),
            pav(validate_dependency_parameters[8]),
            pav(validate_dependency_parameters[4]),
        ],
        vec![set_result_type],
        Immediate::Function(FunctionRefValue {
            function: set_decode_function,
            type_arguments: Vec::new(),
        }),
    );
    let mut dependency_success_arguments = vec![SwitchArgument::CasePayload];
    dependency_success_arguments
        .extend(validate_dependency_parameters[..8].iter().copied().map(sav));
    append_block(
        assembler,
        validate_dependencies,
        function,
        validate_dependency_parameters,
        vec![decoded_dependencies],
        switch(
            op_result(decoded_dependencies),
            vec![
                (
                    BuiltinCase::Ok,
                    dependencies_decoded,
                    dependency_success_arguments,
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let copy3 = build_field_copy_loop(assembler, ns, &control, &outputs2, validate_dependencies);
    let length3_value = build_bounded_field_validator(assembler, ns, &control, &outputs2, copy3);
    let length3 = build_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        &outputs2,
        length3_value,
    );
    let tag3_value = build_field_tag_validator(
        assembler,
        ns,
        &control,
        &outputs2,
        3,
        Some(2),
        &known_tags,
        length3,
    );
    let tag3 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width32,
        3,
        &outputs2,
        tag3_value,
    );

    let copy2 = build_field_copy_loop(assembler, ns, &control, &outputs1, tag3);
    let length2_value = build_fixed_32_validator(assembler, ns, &control, &outputs1, copy2);
    let length2 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        32,
        &outputs1,
        length2_value,
    );
    let tag2_value = build_field_tag_validator(
        assembler,
        ns,
        &control,
        &outputs1,
        2,
        Some(1),
        &known_tags,
        length2,
    );
    let tag2 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width32,
        2,
        &outputs1,
        tag2_value,
    );

    let copy1 = build_field_copy_loop(assembler, ns, &control, &outputs0, tag2);
    let length1_value = build_fixed_32_validator(assembler, ns, &control, &outputs0, copy1);
    let length1 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        32,
        &outputs0,
        length1_value,
    );
    let tag1_value = build_field_tag_validator(
        assembler,
        ns,
        &control,
        &outputs0,
        1,
        None,
        &known_tags,
        length1,
    );
    let tag1 = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width32,
        1,
        &outputs0,
        tag1_value,
    );
    let count_value = build_record_count_validator(assembler, ns, &control, 4, tag1);
    let count = build_expected_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        4,
        &outputs0,
        count_value,
    );

    let union_payload = build_union_payload_copy(assembler, ns, &control, count);
    let union_length_value =
        build_union_length_validator(assembler, ns, &control, count, union_payload);
    let union_length =
        build_initial_uvar_stage(assembler, ns, &control, control.width64, union_length_value);
    let union_tag_value = build_union_tag_validator(assembler, ns, &control, 2, union_length);
    let union_tag = build_expected_initial_uvar_stage(
        assembler,
        ns,
        &control,
        control.width32,
        2,
        union_tag_value,
    );

    let entry = assembler.id(ns.b);
    let total = assembler.op(
        ns.o,
        entry,
        Opcode::VectorLen,
        vec![pav(input_vector)],
        vec![u64_type()],
        Immediate::None,
    );
    let zero = assembler.cref(ns.o, entry, control.constant0, u64_type());
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![total, zero],
        branch(edge(
            union_tag,
            vec![
                op_result(zero),
                op_result(total),
                pav(input_vector),
                pav(body),
                pav(unit),
            ],
        )),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![body, input_vector, unit],
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

fn build_dependency_binding_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    vector_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = dependency_decode_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
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
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );
    let converted_vector = assembler.param(ns.p, converted, ParameterRole::Block, u8vec_type());
    let decoded = assembler.op(
        ns.o,
        converted,
        Opcode::CallDirect,
        vec![pav(body), pav(converted_vector), pav(unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: vector_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        converted,
        function,
        vec![converted_vector],
        vec![decoded],
        ret(op_result(decoded)),
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

fn build_program_supported_return(
    assembler: &mut Asm,
    ns: Ns,
    control: CompactBlocks,
    result_type: &TypeExpr,
) -> EntityId {
    let block = assembler.id(ns.b);
    let types = vec![
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
    ];
    let parameters = block_parameters(assembler, ns.p, block, &types);
    let kind_constant = assembler.ku64(ns.k, 18);
    let zero_constant = assembler.ku64(ns.k, 0);
    let kind = assembler.cref(ns.o, block, kind_constant, u64_type());
    let zero = assembler.cref(ns.o, block, zero_constant, u64_type());
    let mut values = vec![op_result(kind)];
    values.extend(parameter_values(&parameters));
    values.push(op_result(zero));
    let tuple = assembler.op(
        ns.o,
        block,
        Opcode::TupleNew,
        values,
        vec![super::supported_dispatch::all_supported_program_value_type()],
        Immediate::None,
    );
    let ok = assembler.op(
        ns.o,
        block,
        Opcode::ResultOk,
        vec![op_result(tuple)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        block,
        control.function,
        parameters,
        vec![kind, zero, tuple, ok],
        ret(op_result(ok)),
    );
    block
}

/// Checks one fixed-shape body template while ignoring two dynamic byte
/// ranges. Bounded fixed-body profiles share this graph shape.
#[allow(clippy::too_many_lines)]
pub(super) fn build_fixed_supported_body_check(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    canonical_template: &[u8],
    dynamic_ranges: [(u64, u64); 2],
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let accepted = assembler.id(ns.b);
    let refused = assembler.id(ns.b);
    let true_constant = assembler.kbool(ns.k, true);
    let false_constant = assembler.kbool(ns.k, false);
    let true_value = assembler.cref(ns.o, accepted, true_constant, TypeExpr::Bool);
    append_block(
        assembler,
        accepted,
        function,
        Vec::new(),
        vec![true_value],
        ret(op_result(true_value)),
    );
    let false_value = assembler.cref(ns.o, refused, false_constant, TypeExpr::Bool);
    append_block(
        assembler,
        refused,
        function,
        Vec::new(),
        vec![false_value],
        ret(op_result(false_value)),
    );
    let constant1 = assembler.ku64(ns.k, 1);
    let compact_control = CompactBlocks {
        function,
        resource_error: refused,
        invariant_trap: refused,
        constant1,
    };
    let canonical_checks = build_masked_program_canonical_check(
        assembler,
        ns,
        compact_control,
        unit,
        canonical_template,
        dynamic_ranges,
        refused,
        accepted,
    );
    let entry = assembler.id(ns.b);
    let converted = assembler.id(ns.b);
    let payload_vector = assembler.op(
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
        vec![payload_vector],
        switch(
            op_result(payload_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    converted,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, refused, Vec::new()),
            ],
        ),
    );
    let converted_vector = assembler.param(ns.p, converted, ParameterRole::Block, u8vec_type());
    let payload_length = assembler.op(
        ns.o,
        converted,
        Opcode::VectorLen,
        vec![pav(converted_vector)],
        vec![u64_type()],
        Immediate::None,
    );
    let expected_length_constant = assembler.ku64(
        ns.k,
        u128::try_from(canonical_template.len()).expect("body template length fits u128"),
    );
    let expected_length = assembler.cref(ns.o, converted, expected_length_constant, u64_type());
    let canonical_length = assembler.op(
        ns.o,
        converted,
        Opcode::Equal,
        vec![op_result(payload_length), op_result(expected_length)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        converted,
        function,
        vec![converted_vector],
        vec![payload_length, expected_length, canonical_length],
        cond(
            op_result(canonical_length),
            edge(canonical_checks, vec![pav(converted_vector)]),
            edge(refused, Vec::new()),
        ),
    );
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![body, unit],
        result_type: TypeExpr::Bool,
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

/// Decodes the exact empty-set Workspace body. The root namespace is the only
/// dynamic body range; a terminal no-op range keeps the shared two-range loop.
pub(super) fn build_empty_workspace_supported_body_check(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
) -> FunctionGraph {
    let mut canonical_template = vec![1, 47, 5, 1, 1, 0, 2, 32];
    canonical_template.extend_from_slice(&[0; 32]);
    canonical_template.extend_from_slice(&[3, 1, 0, 4, 1, 0, 5, 1, 0]);
    build_fixed_supported_body_check(
        assembler,
        ns,
        function,
        &canonical_template,
        [(8, 40), (49, 49)],
    )
}

/// Checks the exact empty-set Workspace and Package profiles through one
/// compact counted comparator. Exact lengths select the template; every
/// framing byte is then compared while only identity ranges are skipped.
#[allow(clippy::too_many_lines)]
pub(super) fn build_empty_workspace_package_supported_body_check(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let kind = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let accepted = assembler.id(ns.b);
    let refused = assembler.id(ns.b);
    let converted = assembler.id(ns.b);
    let check_package_length = assembler.id(ns.b);
    let prepare_workspace = assembler.id(ns.b);
    let prepare_package = assembler.id(ns.b);
    let check = assembler.id(ns.b);
    let skip_workspace = assembler.id(ns.b);
    let skip_package_first = assembler.id(ns.b);
    let skip_package_second = assembler.id(ns.b);
    let compare = assembler.id(ns.b);
    let next = assembler.id(ns.b);
    let entry = assembler.id(ns.b);

    let true_constant = assembler.kbool(ns.k, true);
    let false_constant = assembler.kbool(ns.k, false);
    let zero_constant = assembler.ku64(ns.k, 0);
    let one_constant = assembler.ku64(ns.k, 1);
    let workspace_length_constant = assembler.ku64(ns.k, 49);
    let package_length_constant = assembler.ku64(ns.k, 77);
    let workspace_kind_constant = assembler.ku64(ns.k, 1);
    let workspace_start_constant = assembler.ku64(ns.k, 8);
    let workspace_destination_constant = assembler.ku64(ns.k, 40);
    let package_first_start_constant = assembler.ku64(ns.k, 5);
    let package_first_destination_constant = assembler.ku64(ns.k, 37);
    let package_second_start_constant = assembler.ku64(ns.k, 39);
    let package_second_destination_constant = assembler.ku64(ns.k, 71);

    let mut workspace_template = vec![1, 47, 5, 1, 1, 0, 2, 32];
    workspace_template.extend_from_slice(&[0; 32]);
    workspace_template.extend_from_slice(&[3, 1, 0, 4, 1, 0, 5, 1, 0]);
    let workspace_template = assembler.kbytes(ns.k, &workspace_template);
    let mut package_template = vec![2, 75, 4, 1, 32];
    package_template.extend_from_slice(&[0; 32]);
    package_template.extend_from_slice(&[2, 32]);
    package_template.extend_from_slice(&[0; 32]);
    package_template.extend_from_slice(&[3, 1, 0, 4, 1, 0]);
    let package_template = assembler.kbytes(ns.k, &package_template);

    for (block, constant) in [(accepted, true_constant), (refused, false_constant)] {
        let result = assembler.cref(ns.o, block, constant, TypeExpr::Bool);
        append_block(
            assembler,
            block,
            function,
            Vec::new(),
            vec![result],
            ret(op_result(result)),
        );
    }

    let payload_vector = assembler.op(
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
        vec![payload_vector],
        switch(
            op_result(payload_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    converted,
                    vec![SwitchArgument::CasePayload, sav(kind)],
                ),
                (BuiltinCase::Err, refused, Vec::new()),
            ],
        ),
    );

    let source = assembler.param(ns.p, converted, ParameterRole::Block, u8vec_type());
    let declared_kind = assembler.param(ns.p, converted, ParameterRole::Block, u64_type());
    let source_length = assembler.op(
        ns.o,
        converted,
        Opcode::VectorLen,
        vec![pav(source)],
        vec![u64_type()],
        Immediate::None,
    );
    let workspace_length = assembler.cref(ns.o, converted, workspace_length_constant, u64_type());
    let is_workspace = assembler.op(
        ns.o,
        converted,
        Opcode::Equal,
        vec![op_result(source_length), op_result(workspace_length)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let workspace_kind = assembler.cref(ns.o, converted, workspace_kind_constant, u64_type());
    let declared_workspace = assembler.op(
        ns.o,
        converted,
        Opcode::Equal,
        vec![pav(declared_kind), op_result(workspace_kind)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let valid_workspace = assembler.op(
        ns.o,
        converted,
        Opcode::BoolAnd,
        vec![op_result(is_workspace), op_result(declared_workspace)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        converted,
        function,
        vec![source, declared_kind],
        vec![
            source_length,
            workspace_length,
            is_workspace,
            workspace_kind,
            declared_workspace,
            valid_workspace,
        ],
        cond(
            op_result(valid_workspace),
            edge(prepare_workspace, vec![pav(source)]),
            edge(
                check_package_length,
                vec![
                    pav(source),
                    op_result(source_length),
                    op_result(declared_workspace),
                ],
            ),
        ),
    );

    let source = assembler.param(
        ns.p,
        check_package_length,
        ParameterRole::Block,
        u8vec_type(),
    );
    let source_length =
        assembler.param(ns.p, check_package_length, ParameterRole::Block, u64_type());
    let declared_workspace = assembler.param(
        ns.p,
        check_package_length,
        ParameterRole::Block,
        TypeExpr::Bool,
    );
    let package_length = assembler.cref(
        ns.o,
        check_package_length,
        package_length_constant,
        u64_type(),
    );
    let is_package = assembler.op(
        ns.o,
        check_package_length,
        Opcode::Equal,
        vec![pav(source_length), op_result(package_length)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let valid_package = assembler.op(
        ns.o,
        check_package_length,
        Opcode::NotEqual,
        vec![op_result(is_package), pav(declared_workspace)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check_package_length,
        function,
        vec![source, source_length, declared_workspace],
        vec![package_length, is_package, valid_package],
        cond(
            op_result(valid_package),
            edge(prepare_package, vec![pav(source)]),
            edge(refused, Vec::new()),
        ),
    );

    for (block, template) in [
        (prepare_workspace, workspace_template),
        (prepare_package, package_template),
    ] {
        let source = assembler.param(ns.p, block, ParameterRole::Block, u8vec_type());
        let template_bytes = assembler.cref(ns.o, block, template, TypeExpr::Bytes);
        let template_vector = assembler.op(
            ns.o,
            block,
            Opcode::AdapterInvoke,
            vec![pav(unit), op_result(template_bytes)],
            vec![index_result(u8vec_type())],
            Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
        );
        let zero = assembler.cref(ns.o, block, zero_constant, u64_type());
        append_block(
            assembler,
            block,
            function,
            vec![source],
            vec![template_bytes, template_vector, zero],
            switch(
                op_result(template_vector),
                vec![
                    (
                        BuiltinCase::Ok,
                        check,
                        vec![oav(zero), sav(source), SwitchArgument::CasePayload],
                    ),
                    (BuiltinCase::Err, refused, Vec::new()),
                ],
            ),
        );
    }

    let state_types = vec![u64_type(), u8vec_type(), u8vec_type()];
    let state = block_parameters(assembler, ns.p, check, &state_types);
    let length = assembler.op(
        ns.o,
        check,
        Opcode::VectorLen,
        vec![pav(state[2])],
        vec![u64_type()],
        Immediate::None,
    );
    let more = assembler.op(
        ns.o,
        check,
        Opcode::LessThan,
        vec![pav(state[0]), op_result(length)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check,
        function,
        state.clone(),
        vec![length, more],
        cond(
            op_result(more),
            edge(skip_workspace, parameter_values(&state)),
            edge(accepted, Vec::new()),
        ),
    );

    let state = block_parameters(assembler, ns.p, skip_workspace, &state_types);
    let start = assembler.cref(ns.o, skip_workspace, workspace_start_constant, u64_type());
    let at_start = assembler.op(
        ns.o,
        skip_workspace,
        Opcode::Equal,
        vec![pav(state[0]), op_result(start)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let destination = assembler.cref(
        ns.o,
        skip_workspace,
        workspace_destination_constant,
        u64_type(),
    );
    let mut jump = parameter_values(&state);
    jump[0] = op_result(destination);
    append_block(
        assembler,
        skip_workspace,
        function,
        state.clone(),
        vec![start, at_start, destination],
        cond(
            op_result(at_start),
            edge(check, jump),
            edge(skip_package_first, parameter_values(&state)),
        ),
    );

    let state = block_parameters(assembler, ns.p, skip_package_first, &state_types);
    let start = assembler.cref(
        ns.o,
        skip_package_first,
        package_first_start_constant,
        u64_type(),
    );
    let at_start = assembler.op(
        ns.o,
        skip_package_first,
        Opcode::Equal,
        vec![pav(state[0]), op_result(start)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let length = assembler.op(
        ns.o,
        skip_package_first,
        Opcode::VectorLen,
        vec![pav(state[2])],
        vec![u64_type()],
        Immediate::None,
    );
    let package_length = assembler.cref(
        ns.o,
        skip_package_first,
        package_length_constant,
        u64_type(),
    );
    let is_package = assembler.op(
        ns.o,
        skip_package_first,
        Opcode::Equal,
        vec![op_result(length), op_result(package_length)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let skip_first = assembler.op(
        ns.o,
        skip_package_first,
        Opcode::BoolAnd,
        vec![op_result(at_start), op_result(is_package)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let destination = assembler.cref(
        ns.o,
        skip_package_first,
        package_first_destination_constant,
        u64_type(),
    );
    let mut jump = parameter_values(&state);
    jump[0] = op_result(destination);
    append_block(
        assembler,
        skip_package_first,
        function,
        state.clone(),
        vec![
            start,
            at_start,
            length,
            package_length,
            is_package,
            skip_first,
            destination,
        ],
        cond(
            op_result(skip_first),
            edge(check, jump),
            edge(skip_package_second, parameter_values(&state)),
        ),
    );

    let state = block_parameters(assembler, ns.p, skip_package_second, &state_types);
    let start = assembler.cref(
        ns.o,
        skip_package_second,
        package_second_start_constant,
        u64_type(),
    );
    let at_start = assembler.op(
        ns.o,
        skip_package_second,
        Opcode::Equal,
        vec![pav(state[0]), op_result(start)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let destination = assembler.cref(
        ns.o,
        skip_package_second,
        package_second_destination_constant,
        u64_type(),
    );
    let mut jump = parameter_values(&state);
    jump[0] = op_result(destination);
    append_block(
        assembler,
        skip_package_second,
        function,
        state.clone(),
        vec![start, at_start, destination],
        cond(
            op_result(at_start),
            edge(check, jump),
            edge(compare, parameter_values(&state)),
        ),
    );

    let state = block_parameters(assembler, ns.p, compare, &state_types);
    let option_u8 = TypeExpr::Option(Box::new(u8_type()));
    let actual = assembler.op(
        ns.o,
        compare,
        Opcode::VectorGet,
        vec![pav(state[1]), pav(state[0])],
        vec![option_u8.clone()],
        Immediate::None,
    );
    let expected = assembler.op(
        ns.o,
        compare,
        Opcode::VectorGet,
        vec![pav(state[2]), pav(state[0])],
        vec![option_u8],
        Immediate::None,
    );
    let matches = assembler.op(
        ns.o,
        compare,
        Opcode::Equal,
        vec![op_result(actual), op_result(expected)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        compare,
        function,
        state.clone(),
        vec![actual, expected, matches],
        cond(
            op_result(matches),
            edge(next, parameter_values(&state)),
            edge(refused, Vec::new()),
        ),
    );

    let state = block_parameters(assembler, ns.p, next, &state_types);
    let one = assembler.cref(ns.o, next, one_constant, u64_type());
    let advanced = assembler.op(
        ns.o,
        next,
        Opcode::IntAddChecked,
        vec![pav(state[0]), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let mut advanced_state = state.iter().copied().map(sav).collect::<Vec<_>>();
    advanced_state[0] = SwitchArgument::CasePayload;
    append_block(
        assembler,
        next,
        function,
        state,
        vec![one, advanced],
        switch(
            op_result(advanced),
            vec![
                (BuiltinCase::Ok, check, advanced_state),
                (BuiltinCase::Err, refused, Vec::new()),
            ],
        ),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![kind, body, unit],
        result_type: TypeExpr::Bool,
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

/// Decodes the complete outer payload for kind 18. Its canonical fixed-shape
/// path never materializes an intermediate body `Bytes`; any framing mismatch
/// returns the provisional scope refusal while the standalone body decoder
/// retains strict native-parity diagnostics.
pub(super) fn build_dependency_supported_program_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
) -> FunctionGraph {
    let result_type = super::supported_dispatch::supported_program_result_type();
    build_dependency_program_decode_with_result(assembler, ns, function, result_type)
}

#[allow(clippy::too_many_lines)]
fn build_dependency_program_decode_with_result(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    result_type: TypeExpr,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let payload = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let scope_code = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let scope_error = err_block(assembler, ns, function, result_type.clone(), scope_code);
    let constant1 = assembler.ku64(ns.k, 1);
    let compact_control = CompactBlocks {
        function,
        resource_error,
        invariant_trap,
        constant1,
    };

    let canonical_return =
        build_program_supported_return(assembler, ns, compact_control, &result_type);
    let copy_namespace = build_compact_copy_loop(
        assembler,
        ns,
        compact_control,
        unit,
        110,
        142,
        3,
        true,
        canonical_return,
    );
    let copy_package = build_compact_copy_loop(
        assembler,
        ns,
        compact_control,
        unit,
        76,
        108,
        2,
        false,
        copy_namespace,
    );
    let copy_root = build_compact_copy_loop(
        assembler,
        ns,
        compact_control,
        unit,
        42,
        74,
        1,
        false,
        copy_package,
    );
    let extract_values = build_compact_copy_loop(
        assembler,
        ns,
        compact_control,
        unit,
        3,
        35,
        0,
        false,
        copy_root,
    );

    let fallback = assembler.id(ns.b);
    append_block(
        assembler,
        fallback,
        function,
        Vec::new(),
        Vec::new(),
        branch(edge(scope_error, Vec::new())),
    );

    let canonical_checks = build_program_canonical_check(
        assembler,
        ns,
        compact_control,
        &[
            (0, 2),
            (1, 1),
            (2, 32),
            (35, 2),
            (36, 105),
            (37, 18),
            (38, 103),
            (39, 3),
            (40, 1),
            (41, 32),
            (74, 2),
            (75, 32),
            (108, 3),
            (109, 32),
        ],
        fallback,
        extract_values,
    );
    let entry = assembler.id(ns.b);
    let converted = assembler.id(ns.b);
    let payload_vector = assembler.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(unit), pav(payload)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![payload_vector],
        switch(
            op_result(payload_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    converted,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );
    let converted_vector = assembler.param(ns.p, converted, ParameterRole::Block, u8vec_type());
    let payload_length = assembler.op(
        ns.o,
        converted,
        Opcode::VectorLen,
        vec![pav(converted_vector)],
        vec![u64_type()],
        Immediate::None,
    );
    let expected_length_constant = assembler.ku64(ns.k, 142);
    let expected_length = assembler.cref(ns.o, converted, expected_length_constant, u64_type());
    let canonical_length = assembler.op(
        ns.o,
        converted,
        Opcode::Equal,
        vec![op_result(payload_length), op_result(expected_length)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        converted,
        function,
        vec![converted_vector],
        vec![payload_length, expected_length, canonical_length],
        cond(
            op_result(canonical_length),
            edge(canonical_checks, vec![pav(converted_vector)]),
            edge(fallback, Vec::new()),
        ),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![payload, unit],
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
    let dependency_core_ns = Ns {
        k: 63,
        p: 64,
        b: 65,
        o: 66,
    };
    let decode_function = eid(11, 2);
    let function = eid(11, 3);
    let core_function = eid(11, 4);
    let (decode_graph, _) = build_decode(&mut assembler, decode_ns, decode_function);
    let core_graph = build_dependency_binding_decode_from_vector(
        &mut assembler,
        dependency_core_ns,
        core_function,
        decode_function,
    );
    let graph =
        build_dependency_binding_decode(&mut assembler, dependency_ns, function, core_function);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph, core_graph, decode_graph],
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

fn generic_record_map_type() -> TypeExpr {
    TypeExpr::OrderedMap {
        key: Box::new(u64_type()),
        value: Box::new(TypeExpr::Bytes),
    }
}

fn generic_record_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(generic_record_map_type()),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(clippy::too_many_lines)]
fn build_generic_record_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    decode_function: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = generic_record_result_type();
    let map_type = generic_record_map_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

    let e_length = assembler.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let e_trailing = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let e_resource = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let e_duplicate = assembler.kbytes(ns.k, b"SCB_FIELD_DUPLICATE");
    let e_order = assembler.kbytes(ns.k, b"SCB_FIELD_ORDER");

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

    let length_error = err_block(assembler, ns, function, result_type.clone(), e_length);
    let trailing_error = err_block(assembler, ns, function, result_type.clone(), e_trailing);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), e_resource);
    let duplicate_error = err_block(assembler, ns, function, result_type.clone(), e_duplicate);
    let order_error = err_block(assembler, ns, function, result_type.clone(), e_order);
    let control = DecodeBlocks {
        function,
        decode_function,
        forward_error,
        length_error,
        trailing_error,
        resource_error,
        missing_error: resource_error,
        unknown_error: resource_error,
        duplicate_error,
        order_error,
        union_error: resource_error,
        scope_error: resource_error,
        invariant_trap: trap_block(assembler, ns, function),
        width32: assembler.ku32(ns.k, 32),
        width64: assembler.ku32(ns.k, 64),
        max_length: assembler.ku64(ns.k, 67_108_864),
        max_fields: assembler.ku64(ns.k, 65_535),
        constant0: assembler.ku64(ns.k, 0),
        constant1: assembler.ku64(ns.k, 1),
        constant32: assembler.ku64(ns.k, 32),
    };

    let output_types = vec![u64_type(), u64_type(), u64_type(), map_type.clone()];
    let state_types = record_state_types(&output_types);
    let decoded_types = decoded_record_state_types(&output_types);
    let loop_check = assembler.id(ns.b);
    let finish = assembler.id(ns.b);
    let finish_ok = assembler.id(ns.b);
    let tag_validate = assembler.id(ns.b);
    let tag_seen = assembler.id(ns.b);
    let tag_order = assembler.id(ns.b);
    let tag_ready = assembler.id(ns.b);
    let insert = assembler.id(ns.b);
    let count_ready = assembler.id(ns.b);
    let map_ready = assembler.id(ns.b);
    let vector_ready = assembler.id(ns.b);

    let insert_parameters = block_parameters(
        assembler,
        ns.p,
        insert,
        &state_types
            .clone()
            .into_iter()
            .chain(std::iter::once(TypeExpr::Bytes))
            .collect::<Vec<_>>(),
    );
    let inserted = assembler.op(
        ns.o,
        insert,
        Opcode::MapInsert,
        vec![
            pav(insert_parameters[9]),
            pav(insert_parameters[8]),
            pav(insert_parameters[10]),
        ],
        vec![map_type.clone()],
        Immediate::None,
    );
    let one = assembler.cref(ns.o, insert, control.constant1, u64_type());
    let processed = assembler.op(
        ns.o,
        insert,
        Opcode::IntAddChecked,
        vec![pav(insert_parameters[6]), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        insert,
        function,
        insert_parameters.clone(),
        vec![inserted, one, processed],
        switch(
            op_result(processed),
            vec![
                (
                    BuiltinCase::Ok,
                    loop_check,
                    vec![
                        sav(insert_parameters[0]),
                        sav(insert_parameters[1]),
                        sav(insert_parameters[2]),
                        sav(insert_parameters[3]),
                        sav(insert_parameters[4]),
                        sav(insert_parameters[5]),
                        SwitchArgument::CasePayload,
                        sav(insert_parameters[7]),
                        sav(insert_parameters[8]),
                        oav(inserted),
                    ],
                ),
                (BuiltinCase::Err, control.invariant_trap, Vec::new()),
            ],
        ),
    );

    let field_copy = build_field_copy_loop(assembler, ns, &control, &output_types, insert);
    let field_length =
        build_bounded_field_validator(assembler, ns, &control, &output_types, field_copy);
    let length_decode = build_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        &output_types,
        field_length,
    );

    let tag_ready_parameters = block_parameters(assembler, ns.p, tag_ready, &decoded_types);
    let seen_true_constant = assembler.kbool(ns.k, true);
    let seen_true = assembler.cref(ns.o, tag_ready, seen_true_constant, TypeExpr::Bool);
    append_block(
        assembler,
        tag_ready,
        function,
        tag_ready_parameters.clone(),
        vec![seen_true],
        branch(edge(
            length_decode,
            vec![
                pav(tag_ready_parameters[1]),
                pav(tag_ready_parameters[2]),
                pav(tag_ready_parameters[3]),
                pav(tag_ready_parameters[4]),
                pav(tag_ready_parameters[5]),
                op_result(seen_true),
                pav(tag_ready_parameters[7]),
                pav(tag_ready_parameters[8]),
                pav(tag_ready_parameters[0]),
                pav(tag_ready_parameters[10]),
            ],
        )),
    );

    let tag_order_parameters = block_parameters(assembler, ns.p, tag_order, &decoded_types);
    let earlier = assembler.op(
        ns.o,
        tag_order,
        Opcode::LessThan,
        vec![pav(tag_order_parameters[0]), pav(tag_order_parameters[9])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        tag_order,
        function,
        tag_order_parameters.clone(),
        vec![earlier],
        cond(
            op_result(earlier),
            edge(control.order_error, Vec::new()),
            edge(tag_ready, parameter_values(&tag_order_parameters)),
        ),
    );

    let tag_seen_parameters = block_parameters(assembler, ns.p, tag_seen, &decoded_types);
    let duplicate = assembler.op(
        ns.o,
        tag_seen,
        Opcode::Equal,
        vec![pav(tag_seen_parameters[0]), pav(tag_seen_parameters[9])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        tag_seen,
        function,
        tag_seen_parameters.clone(),
        vec![duplicate],
        cond(
            op_result(duplicate),
            edge(control.duplicate_error, Vec::new()),
            edge(tag_order, parameter_values(&tag_seen_parameters)),
        ),
    );

    let tag_validate_parameters = block_parameters(assembler, ns.p, tag_validate, &decoded_types);
    append_block(
        assembler,
        tag_validate,
        function,
        tag_validate_parameters.clone(),
        Vec::new(),
        cond(
            pav(tag_validate_parameters[6]),
            edge(tag_seen, parameter_values(&tag_validate_parameters)),
            edge(tag_ready, parameter_values(&tag_validate_parameters)),
        ),
    );
    let tag_decode = build_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width32,
        &output_types,
        tag_validate,
    );

    let finish_ok_parameters =
        block_parameters(assembler, ns.p, finish_ok, std::slice::from_ref(&map_type));
    let ok = assembler.op(
        ns.o,
        finish_ok,
        Opcode::ResultOk,
        vec![pav(finish_ok_parameters[0])],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        finish_ok,
        function,
        finish_ok_parameters,
        vec![ok],
        ret(op_result(ok)),
    );

    let finish_parameters = block_parameters(assembler, ns.p, finish, &state_types);
    let consumed = assembler.op(
        ns.o,
        finish,
        Opcode::Equal,
        vec![pav(finish_parameters[0]), pav(finish_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        finish,
        function,
        finish_parameters.clone(),
        vec![consumed],
        cond(
            op_result(consumed),
            edge(finish_ok, vec![pav(finish_parameters[9])]),
            edge(control.trailing_error, Vec::new()),
        ),
    );

    let loop_parameters = block_parameters(assembler, ns.p, loop_check, &state_types);
    let complete = assembler.op(
        ns.o,
        loop_check,
        Opcode::Equal,
        vec![pav(loop_parameters[6]), pav(loop_parameters[7])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        loop_check,
        function,
        loop_parameters.clone(),
        vec![complete],
        cond(
            op_result(complete),
            edge(finish, parameter_values(&loop_parameters)),
            edge(tag_decode, parameter_values(&loop_parameters)),
        ),
    );

    let map_ready_types = vec![
        map_type.clone(),
        u64_type(),
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
        TypeExpr::Bool,
    ];
    let map_ready_parameters = block_parameters(assembler, ns.p, map_ready, &map_ready_types);
    let processed_zero = assembler.cref(ns.o, map_ready, control.constant0, u64_type());
    let previous_zero = assembler.cref(ns.o, map_ready, control.constant0, u64_type());
    append_block(
        assembler,
        map_ready,
        function,
        map_ready_parameters.clone(),
        vec![processed_zero, previous_zero],
        branch(edge(
            loop_check,
            vec![
                pav(map_ready_parameters[2]),
                pav(map_ready_parameters[3]),
                pav(map_ready_parameters[4]),
                pav(map_ready_parameters[5]),
                pav(map_ready_parameters[6]),
                pav(map_ready_parameters[7]),
                op_result(processed_zero),
                pav(map_ready_parameters[1]),
                op_result(previous_zero),
                pav(map_ready_parameters[0]),
            ],
        )),
    );

    let count_ready_types = vec![
        u64_type(),
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
        TypeExpr::Bool,
    ];
    let count_ready_parameters = block_parameters(assembler, ns.p, count_ready, &count_ready_types);
    let max_fields = assembler.cref(ns.o, count_ready, control.max_fields, u64_type());
    let too_many = assembler.op(
        ns.o,
        count_ready,
        Opcode::GreaterThan,
        vec![pav(count_ready_parameters[0]), op_result(max_fields)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let create_map = assembler.id(ns.b);
    append_block(
        assembler,
        count_ready,
        function,
        count_ready_parameters.clone(),
        vec![max_fields, too_many],
        cond(
            op_result(too_many),
            edge(control.resource_error, Vec::new()),
            edge(create_map, parameter_values(&count_ready_parameters)),
        ),
    );

    let create_map_parameters = block_parameters(assembler, ns.p, create_map, &count_ready_types);
    let map_new_result = TypeExpr::Result {
        ok: Box::new(map_type.clone()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let empty_map = assembler.op(
        ns.o,
        create_map,
        Opcode::MapNew,
        Vec::new(),
        vec![map_new_result],
        Immediate::None,
    );
    let mut map_ready_arguments = vec![SwitchArgument::CasePayload];
    map_ready_arguments.extend(create_map_parameters.iter().copied().map(sav));
    append_block(
        assembler,
        create_map,
        function,
        create_map_parameters,
        vec![empty_map],
        switch(
            op_result(empty_map),
            vec![
                (BuiltinCase::Ok, map_ready, map_ready_arguments),
                (BuiltinCase::Err, control.invariant_trap, Vec::new()),
            ],
        ),
    );

    let count_decode =
        build_record_uvar_stage(assembler, ns, &control, control.width64, &[], count_ready);
    let vector_ready_types = vec![u8vec_type(), TypeExpr::Bytes, TypeExpr::Unit];
    let vector_ready_parameters =
        block_parameters(assembler, ns.p, vector_ready, &vector_ready_types);
    let total = assembler.op(
        ns.o,
        vector_ready,
        Opcode::VectorLen,
        vec![pav(vector_ready_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let zero = assembler.cref(ns.o, vector_ready, control.constant0, u64_type());
    let unseen_constant = assembler.kbool(ns.k, false);
    let unseen = assembler.cref(ns.o, vector_ready, unseen_constant, TypeExpr::Bool);
    append_block(
        assembler,
        vector_ready,
        function,
        vector_ready_parameters.clone(),
        vec![total, zero, unseen],
        branch(edge(
            count_decode,
            vec![
                op_result(zero),
                op_result(total),
                pav(vector_ready_parameters[0]),
                pav(vector_ready_parameters[1]),
                pav(vector_ready_parameters[2]),
                op_result(unseen),
            ],
        )),
    );

    let entry = assembler.id(ns.b);
    let converted = assembler.op(
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
        vec![converted],
        switch(
            op_result(converted),
            vec![
                (
                    BuiltinCase::Ok,
                    vector_ready,
                    vec![SwitchArgument::CasePayload, sav(body), sav(unit)],
                ),
                (BuiltinCase::Err, control.resource_error, Vec::new()),
            ],
        ),
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

pub(super) fn generic_record_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = eid(12, 3);
    let function = eid(12, 4);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 238,
            p: 239,
            b: 240,
            o: 241,
        },
        decode_function,
    );
    let graph = build_generic_record_decode(
        &mut assembler,
        Ns {
            k: 242,
            p: 243,
            b: 244,
            o: 245,
        },
        function,
        decode_function,
    );
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

fn generic_union_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![u64_type(), TypeExpr::Bytes])),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(clippy::too_many_lines)]
fn build_generic_union_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    decode_function: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = generic_union_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let e_length = assembler.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let e_trailing = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let e_resource = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");

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
    let length_error = err_block(assembler, ns, function, result_type.clone(), e_length);
    let trailing_error = err_block(assembler, ns, function, result_type.clone(), e_trailing);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), e_resource);
    let control = DecodeBlocks {
        function,
        decode_function,
        forward_error,
        length_error,
        trailing_error,
        resource_error,
        missing_error: resource_error,
        unknown_error: resource_error,
        duplicate_error: resource_error,
        order_error: resource_error,
        union_error: resource_error,
        scope_error: resource_error,
        invariant_trap: trap_block(assembler, ns, function),
        width32: assembler.ku32(ns.k, 32),
        width64: assembler.ku32(ns.k, 64),
        max_length: assembler.ku64(ns.k, 67_108_864),
        max_fields: assembler.ku64(ns.k, 65_535),
        constant0: assembler.ku64(ns.k, 0),
        constant1: assembler.ku64(ns.k, 1),
        constant32: assembler.ku64(ns.k, 32),
    };
    let output_types = vec![u64_type()];
    let finish = assembler.id(ns.b);
    let finish_ok = assembler.id(ns.b);
    let tag_ready = assembler.id(ns.b);
    let vector_ready = assembler.id(ns.b);

    let finish_ok_parameters =
        block_parameters(assembler, ns.p, finish_ok, &[u64_type(), TypeExpr::Bytes]);
    let tuple_type = TypeExpr::Tuple(vec![u64_type(), TypeExpr::Bytes]);
    let tuple = assembler.op(
        ns.o,
        finish_ok,
        Opcode::TupleNew,
        parameter_values(&finish_ok_parameters),
        vec![tuple_type],
        Immediate::None,
    );
    let ok = assembler.op(
        ns.o,
        finish_ok,
        Opcode::ResultOk,
        vec![op_result(tuple)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        finish_ok,
        function,
        finish_ok_parameters,
        vec![tuple, ok],
        ret(op_result(ok)),
    );

    let finish_types = record_state_types(&output_types)
        .into_iter()
        .chain(std::iter::once(TypeExpr::Bytes))
        .collect::<Vec<_>>();
    let finish_parameters = block_parameters(assembler, ns.p, finish, &finish_types);
    let consumed = assembler.op(
        ns.o,
        finish,
        Opcode::Equal,
        vec![pav(finish_parameters[0]), pav(finish_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        finish,
        function,
        finish_parameters.clone(),
        vec![consumed],
        cond(
            op_result(consumed),
            edge(
                finish_ok,
                vec![pav(finish_parameters[6]), pav(finish_parameters[7])],
            ),
            edge(control.trailing_error, Vec::new()),
        ),
    );

    let field_copy = build_field_copy_loop(assembler, ns, &control, &output_types, finish);
    let field_length =
        build_bounded_field_validator(assembler, ns, &control, &output_types, field_copy);
    let length_decode = build_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        &output_types,
        field_length,
    );

    let tag_ready_types = decoded_record_state_types(&[]);
    let tag_ready_parameters = block_parameters(assembler, ns.p, tag_ready, &tag_ready_types);
    append_block(
        assembler,
        tag_ready,
        function,
        tag_ready_parameters.clone(),
        Vec::new(),
        branch(edge(
            length_decode,
            vec![
                pav(tag_ready_parameters[1]),
                pav(tag_ready_parameters[2]),
                pav(tag_ready_parameters[3]),
                pav(tag_ready_parameters[4]),
                pav(tag_ready_parameters[5]),
                pav(tag_ready_parameters[6]),
                pav(tag_ready_parameters[0]),
            ],
        )),
    );
    let tag_decode =
        build_record_uvar_stage(assembler, ns, &control, control.width32, &[], tag_ready);

    let vector_ready_types = vec![u8vec_type(), TypeExpr::Bytes, TypeExpr::Unit];
    let vector_ready_parameters =
        block_parameters(assembler, ns.p, vector_ready, &vector_ready_types);
    let total = assembler.op(
        ns.o,
        vector_ready,
        Opcode::VectorLen,
        vec![pav(vector_ready_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let zero = assembler.cref(ns.o, vector_ready, control.constant0, u64_type());
    let unseen_constant = assembler.kbool(ns.k, false);
    let unseen = assembler.cref(ns.o, vector_ready, unseen_constant, TypeExpr::Bool);
    append_block(
        assembler,
        vector_ready,
        function,
        vector_ready_parameters.clone(),
        vec![total, zero, unseen],
        branch(edge(
            tag_decode,
            vec![
                op_result(zero),
                op_result(total),
                pav(vector_ready_parameters[0]),
                pav(vector_ready_parameters[1]),
                pav(vector_ready_parameters[2]),
                op_result(unseen),
            ],
        )),
    );

    let entry = assembler.id(ns.b);
    let converted = assembler.op(
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
        vec![converted],
        switch(
            op_result(converted),
            vec![
                (
                    BuiltinCase::Ok,
                    vector_ready,
                    vec![SwitchArgument::CasePayload, sav(body), sav(unit)],
                ),
                (BuiltinCase::Err, control.resource_error, Vec::new()),
            ],
        ),
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

pub(super) fn generic_union_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = eid(12, 5);
    let function = eid(12, 6);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 246,
            p: 247,
            b: 248,
            o: 249,
        },
        decode_function,
    );
    let graph = build_generic_union_decode(
        &mut assembler,
        Ns {
            k: 250,
            p: 251,
            b: 252,
            o: 253,
        },
        function,
        decode_function,
    );
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

#[allow(clippy::too_many_lines)]
fn build_generic_list_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    decode_function: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = generic_record_result_type();
    let map_type = generic_record_map_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let e_length = assembler.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let e_trailing = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let e_resource = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");

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
    let length_error = err_block(assembler, ns, function, result_type.clone(), e_length);
    let trailing_error = err_block(assembler, ns, function, result_type.clone(), e_trailing);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), e_resource);
    let control = DecodeBlocks {
        function,
        decode_function,
        forward_error,
        length_error,
        trailing_error,
        resource_error,
        missing_error: resource_error,
        unknown_error: resource_error,
        duplicate_error: resource_error,
        order_error: resource_error,
        union_error: resource_error,
        scope_error: resource_error,
        invariant_trap: trap_block(assembler, ns, function),
        width32: assembler.ku32(ns.k, 32),
        width64: assembler.ku32(ns.k, 64),
        max_length: assembler.ku64(ns.k, 67_108_864),
        max_fields: assembler.ku64(ns.k, 65_535),
        constant0: assembler.ku64(ns.k, 0),
        constant1: assembler.ku64(ns.k, 1),
        constant32: assembler.ku64(ns.k, 32),
    };
    let output_types = vec![u64_type(), u64_type(), map_type.clone()];
    let state_types = record_state_types(&output_types);
    let loop_check = assembler.id(ns.b);
    let finish = assembler.id(ns.b);
    let finish_ok = assembler.id(ns.b);
    let insert = assembler.id(ns.b);
    let count_ready = assembler.id(ns.b);
    let map_ready = assembler.id(ns.b);
    let vector_ready = assembler.id(ns.b);

    let finish_ok_parameters =
        block_parameters(assembler, ns.p, finish_ok, std::slice::from_ref(&map_type));
    let ok = assembler.op(
        ns.o,
        finish_ok,
        Opcode::ResultOk,
        vec![pav(finish_ok_parameters[0])],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        finish_ok,
        function,
        finish_ok_parameters,
        vec![ok],
        ret(op_result(ok)),
    );

    let finish_parameters = block_parameters(assembler, ns.p, finish, &state_types);
    let consumed = assembler.op(
        ns.o,
        finish,
        Opcode::Equal,
        vec![pav(finish_parameters[0]), pav(finish_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        finish,
        function,
        finish_parameters.clone(),
        vec![consumed],
        cond(
            op_result(consumed),
            edge(finish_ok, vec![pav(finish_parameters[8])]),
            edge(control.trailing_error, Vec::new()),
        ),
    );

    let insert_types = state_types
        .clone()
        .into_iter()
        .chain(std::iter::once(TypeExpr::Bytes))
        .collect::<Vec<_>>();
    let insert_parameters = block_parameters(assembler, ns.p, insert, &insert_types);
    let inserted = assembler.op(
        ns.o,
        insert,
        Opcode::MapInsert,
        vec![
            pav(insert_parameters[8]),
            pav(insert_parameters[6]),
            pav(insert_parameters[9]),
        ],
        vec![map_type.clone()],
        Immediate::None,
    );
    let one = assembler.cref(ns.o, insert, control.constant1, u64_type());
    let processed = assembler.op(
        ns.o,
        insert,
        Opcode::IntAddChecked,
        vec![pav(insert_parameters[6]), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        insert,
        function,
        insert_parameters.clone(),
        vec![inserted, one, processed],
        switch(
            op_result(processed),
            vec![
                (
                    BuiltinCase::Ok,
                    loop_check,
                    vec![
                        sav(insert_parameters[0]),
                        sav(insert_parameters[1]),
                        sav(insert_parameters[2]),
                        sav(insert_parameters[3]),
                        sav(insert_parameters[4]),
                        sav(insert_parameters[5]),
                        SwitchArgument::CasePayload,
                        sav(insert_parameters[7]),
                        oav(inserted),
                    ],
                ),
                (BuiltinCase::Err, control.invariant_trap, Vec::new()),
            ],
        ),
    );

    let field_copy = build_field_copy_loop(assembler, ns, &control, &output_types, insert);
    let field_length =
        build_bounded_field_validator(assembler, ns, &control, &output_types, field_copy);
    let length_decode = build_record_uvar_stage(
        assembler,
        ns,
        &control,
        control.width64,
        &output_types,
        field_length,
    );

    let loop_parameters = block_parameters(assembler, ns.p, loop_check, &state_types);
    let complete = assembler.op(
        ns.o,
        loop_check,
        Opcode::Equal,
        vec![pav(loop_parameters[6]), pav(loop_parameters[7])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        loop_check,
        function,
        loop_parameters.clone(),
        vec![complete],
        cond(
            op_result(complete),
            edge(finish, parameter_values(&loop_parameters)),
            edge(length_decode, parameter_values(&loop_parameters)),
        ),
    );

    let map_ready_types = vec![
        map_type.clone(),
        u64_type(),
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
        TypeExpr::Bool,
    ];
    let map_ready_parameters = block_parameters(assembler, ns.p, map_ready, &map_ready_types);
    let processed_zero = assembler.cref(ns.o, map_ready, control.constant0, u64_type());
    append_block(
        assembler,
        map_ready,
        function,
        map_ready_parameters.clone(),
        vec![processed_zero],
        branch(edge(
            loop_check,
            vec![
                pav(map_ready_parameters[2]),
                pav(map_ready_parameters[3]),
                pav(map_ready_parameters[4]),
                pav(map_ready_parameters[5]),
                pav(map_ready_parameters[6]),
                pav(map_ready_parameters[7]),
                op_result(processed_zero),
                pav(map_ready_parameters[1]),
                pav(map_ready_parameters[0]),
            ],
        )),
    );

    let count_ready_types = vec![
        u64_type(),
        u64_type(),
        u64_type(),
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Unit,
        TypeExpr::Bool,
    ];
    let count_ready_parameters = block_parameters(assembler, ns.p, count_ready, &count_ready_types);
    let max_items = assembler.cref(ns.o, count_ready, control.max_fields, u64_type());
    let too_many = assembler.op(
        ns.o,
        count_ready,
        Opcode::GreaterThan,
        vec![pav(count_ready_parameters[0]), op_result(max_items)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let create_map = assembler.id(ns.b);
    append_block(
        assembler,
        count_ready,
        function,
        count_ready_parameters.clone(),
        vec![max_items, too_many],
        cond(
            op_result(too_many),
            edge(control.resource_error, Vec::new()),
            edge(create_map, parameter_values(&count_ready_parameters)),
        ),
    );

    let create_map_parameters = block_parameters(assembler, ns.p, create_map, &count_ready_types);
    let map_new_result = TypeExpr::Result {
        ok: Box::new(map_type.clone()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let empty_map = assembler.op(
        ns.o,
        create_map,
        Opcode::MapNew,
        Vec::new(),
        vec![map_new_result],
        Immediate::None,
    );
    let mut map_ready_arguments = vec![SwitchArgument::CasePayload];
    map_ready_arguments.extend(create_map_parameters.iter().copied().map(sav));
    append_block(
        assembler,
        create_map,
        function,
        create_map_parameters,
        vec![empty_map],
        switch(
            op_result(empty_map),
            vec![
                (BuiltinCase::Ok, map_ready, map_ready_arguments),
                (BuiltinCase::Err, control.invariant_trap, Vec::new()),
            ],
        ),
    );

    let count_decode =
        build_record_uvar_stage(assembler, ns, &control, control.width64, &[], count_ready);
    let vector_ready_types = vec![u8vec_type(), TypeExpr::Bytes, TypeExpr::Unit];
    let vector_ready_parameters =
        block_parameters(assembler, ns.p, vector_ready, &vector_ready_types);
    let total = assembler.op(
        ns.o,
        vector_ready,
        Opcode::VectorLen,
        vec![pav(vector_ready_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let zero = assembler.cref(ns.o, vector_ready, control.constant0, u64_type());
    let unused_constant = assembler.kbool(ns.k, false);
    let unused = assembler.cref(ns.o, vector_ready, unused_constant, TypeExpr::Bool);
    append_block(
        assembler,
        vector_ready,
        function,
        vector_ready_parameters.clone(),
        vec![total, zero, unused],
        branch(edge(
            count_decode,
            vec![
                op_result(zero),
                op_result(total),
                pav(vector_ready_parameters[0]),
                pav(vector_ready_parameters[1]),
                pav(vector_ready_parameters[2]),
                op_result(unused),
            ],
        )),
    );

    let entry = assembler.id(ns.b);
    let converted = assembler.op(
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
        vec![converted],
        switch(
            op_result(converted),
            vec![
                (
                    BuiltinCase::Ok,
                    vector_ready,
                    vec![SwitchArgument::CasePayload, sav(body), sav(unit)],
                ),
                (BuiltinCase::Err, control.resource_error, Vec::new()),
            ],
        ),
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

pub(super) fn generic_list_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = eid(12, 7);
    let function = eid(12, 8);
    let decode_ns = Ns {
        k: 254,
        p: 254,
        b: 254,
        o: 254,
    };
    let list_ns = Ns {
        k: 255,
        p: 255,
        b: 255,
        o: 255,
    };
    let (decode_graph, _) = build_decode(&mut assembler, decode_ns, decode_function);
    let graph = build_generic_list_decode(&mut assembler, list_ns, function, decode_function);
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

fn bytes_validation_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Bytes),
        error: Box::new(TypeExpr::Bytes),
    }
}

fn unit_validation_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Unit),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(clippy::too_many_lines)]
fn build_fixed32_decode(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = bytes_validation_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let length_code = assembler.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let length_error = err_block(assembler, ns, function, result_type.clone(), length_code);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
    let success = assembler.id(ns.b);
    let vector_ready = assembler.id(ns.b);

    let ok = assembler.op(
        ns.o,
        success,
        Opcode::ResultOk,
        vec![pav(body)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        success,
        function,
        Vec::new(),
        vec![ok],
        ret(op_result(ok)),
    );

    let vector_parameters = block_parameters(assembler, ns.p, vector_ready, &[u8vec_type()]);
    let length = assembler.op(
        ns.o,
        vector_ready,
        Opcode::VectorLen,
        vec![pav(vector_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let constant32 = assembler.ku64(ns.k, 32);
    let expected = assembler.cref(ns.o, vector_ready, constant32, u64_type());
    let exact = assembler.op(
        ns.o,
        vector_ready,
        Opcode::Equal,
        vec![op_result(length), op_result(expected)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        vector_ready,
        function,
        vector_parameters,
        vec![length, expected, exact],
        cond(
            op_result(exact),
            edge(success, Vec::new()),
            edge(length_error, Vec::new()),
        ),
    );

    let entry = assembler.id(ns.b);
    let converted = assembler.op(
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
        vec![converted],
        switch(
            op_result(converted),
            vec![
                (
                    BuiltinCase::Ok,
                    vector_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
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

#[allow(clippy::too_many_lines)]
fn build_entity_id_collection_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    list_decoder: EntityId,
    fixed32_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let map_type = generic_record_map_type();
    let option_bytes_type = TypeExpr::Option(Box::new(TypeExpr::Bytes));
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let ordered = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bool);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let duplicate_code = assembler.kbytes(ns.k, b"SCB_MAP_DUPLICATE");
    let order_code = assembler.kbytes(ns.k, b"SCB_MAP_ORDER");

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
    let duplicate_error = err_block(assembler, ns, function, result_type.clone(), duplicate_code);
    let order_error = err_block(assembler, ns, function, result_type.clone(), order_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let success = assembler.id(ns.b);
    let advance = assembler.id(ns.b);
    let order_decide = assembler.id(ns.b);
    let compare = assembler.id(ns.b);
    let order_gate = assembler.id(ns.b);
    let fixed_call = assembler.id(ns.b);
    let loop_check = assembler.id(ns.b);
    let list_ready = assembler.id(ns.b);

    let ok = assembler.op(
        ns.o,
        success,
        Opcode::ResultOk,
        vec![pav(unit)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        success,
        function,
        Vec::new(),
        vec![ok],
        ret(op_result(ok)),
    );

    let advance_types = vec![TypeExpr::Bytes, map_type.clone(), u64_type()];
    let advance_parameters = block_parameters(assembler, ns.p, advance, &advance_types);
    let one_constant = assembler.ku64(ns.k, 1);
    let one = assembler.cref(ns.o, advance, one_constant, u64_type());
    let next = assembler.op(
        ns.o,
        advance,
        Opcode::IntAddChecked,
        vec![pav(advance_parameters[2]), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let true_constant = assembler.kbool(ns.k, true);
    let seen = assembler.cref(ns.o, advance, true_constant, TypeExpr::Bool);
    append_block(
        assembler,
        advance,
        function,
        advance_parameters.clone(),
        vec![one, next, seen],
        switch(
            op_result(next),
            vec![
                (
                    BuiltinCase::Ok,
                    loop_check,
                    vec![
                        sav(advance_parameters[1]),
                        SwitchArgument::CasePayload,
                        oav(seen),
                        sav(advance_parameters[0]),
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let order_decide_types = vec![
        TypeExpr::Bool,
        TypeExpr::Bytes,
        map_type.clone(),
        u64_type(),
    ];
    let order_decide_parameters =
        block_parameters(assembler, ns.p, order_decide, &order_decide_types);
    append_block(
        assembler,
        order_decide,
        function,
        order_decide_parameters.clone(),
        Vec::new(),
        cond(
            pav(order_decide_parameters[0]),
            edge(
                advance,
                vec![
                    pav(order_decide_parameters[1]),
                    pav(order_decide_parameters[2]),
                    pav(order_decide_parameters[3]),
                ],
            ),
            edge(order_error, Vec::new()),
        ),
    );

    let compare_types = vec![
        TypeExpr::Bytes,
        map_type.clone(),
        u64_type(),
        TypeExpr::Bytes,
    ];
    let compare_parameters = block_parameters(assembler, ns.p, compare, &compare_types);
    let duplicate = assembler.op(
        ns.o,
        compare,
        Opcode::Equal,
        vec![pav(compare_parameters[3]), pav(compare_parameters[0])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let increasing = assembler.op(
        ns.o,
        compare,
        Opcode::LessThan,
        vec![pav(compare_parameters[3]), pav(compare_parameters[0])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        compare,
        function,
        compare_parameters.clone(),
        vec![duplicate, increasing],
        cond(
            op_result(duplicate),
            edge(duplicate_error, Vec::new()),
            edge(
                order_decide,
                vec![
                    op_result(increasing),
                    pav(compare_parameters[0]),
                    pav(compare_parameters[1]),
                    pav(compare_parameters[2]),
                ],
            ),
        ),
    );

    let order_gate_types = vec![
        TypeExpr::Bytes,
        map_type.clone(),
        u64_type(),
        TypeExpr::Bool,
        TypeExpr::Bytes,
    ];
    let order_gate_parameters = block_parameters(assembler, ns.p, order_gate, &order_gate_types);
    let must_compare = assembler.op(
        ns.o,
        order_gate,
        Opcode::BoolAnd,
        vec![pav(ordered), pav(order_gate_parameters[3])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        order_gate,
        function,
        order_gate_parameters.clone(),
        vec![must_compare],
        cond(
            op_result(must_compare),
            edge(
                compare,
                vec![
                    pav(order_gate_parameters[0]),
                    pav(order_gate_parameters[1]),
                    pav(order_gate_parameters[2]),
                    pav(order_gate_parameters[4]),
                ],
            ),
            edge(
                advance,
                vec![
                    pav(order_gate_parameters[0]),
                    pav(order_gate_parameters[1]),
                    pav(order_gate_parameters[2]),
                ],
            ),
        ),
    );

    let fixed_call_types = vec![
        TypeExpr::Bytes,
        map_type.clone(),
        u64_type(),
        TypeExpr::Bool,
        TypeExpr::Bytes,
    ];
    let fixed_call_parameters = block_parameters(assembler, ns.p, fixed_call, &fixed_call_types);
    let fixed = assembler.op(
        ns.o,
        fixed_call,
        Opcode::CallDirect,
        vec![pav(fixed_call_parameters[0]), pav(unit)],
        vec![bytes_validation_result_type()],
        Immediate::Function(FunctionRefValue {
            function: fixed32_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        fixed_call,
        function,
        fixed_call_parameters.clone(),
        vec![fixed],
        switch(
            op_result(fixed),
            vec![
                (
                    BuiltinCase::Ok,
                    order_gate,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(fixed_call_parameters[1]),
                        sav(fixed_call_parameters[2]),
                        sav(fixed_call_parameters[3]),
                        sav(fixed_call_parameters[4]),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let loop_types = vec![
        map_type.clone(),
        u64_type(),
        TypeExpr::Bool,
        TypeExpr::Bytes,
    ];
    let loop_parameters = block_parameters(assembler, ns.p, loop_check, &loop_types);
    let found = assembler.op(
        ns.o,
        loop_check,
        Opcode::MapGet,
        vec![pav(loop_parameters[0]), pav(loop_parameters[1])],
        vec![option_bytes_type],
        Immediate::None,
    );
    append_block(
        assembler,
        loop_check,
        function,
        loop_parameters.clone(),
        vec![found],
        switch(
            op_result(found),
            vec![
                (BuiltinCase::None, success, Vec::new()),
                (
                    BuiltinCase::Some,
                    fixed_call,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(loop_parameters[0]),
                        sav(loop_parameters[1]),
                        sav(loop_parameters[2]),
                        sav(loop_parameters[3]),
                    ],
                ),
            ],
        ),
    );

    let list_ready_parameters =
        block_parameters(assembler, ns.p, list_ready, std::slice::from_ref(&map_type));
    let zero_constant = assembler.ku64(ns.k, 0);
    let zero = assembler.cref(ns.o, list_ready, zero_constant, u64_type());
    let false_constant = assembler.kbool(ns.k, false);
    let unseen = assembler.cref(ns.o, list_ready, false_constant, TypeExpr::Bool);
    let empty_constant = assembler.kbytes(ns.k, b"");
    let previous = assembler.cref(ns.o, list_ready, empty_constant, TypeExpr::Bytes);
    append_block(
        assembler,
        list_ready,
        function,
        list_ready_parameters.clone(),
        vec![zero, unseen, previous],
        branch(edge(
            loop_check,
            vec![
                pav(list_ready_parameters[0]),
                op_result(zero),
                op_result(unseen),
                op_result(previous),
            ],
        )),
    );

    let entry = assembler.id(ns.b);
    let list = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![generic_record_result_type()],
        Immediate::Function(FunctionRefValue {
            function: list_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![list],
        switch(
            op_result(list),
            vec![
                (
                    BuiltinCase::Ok,
                    list_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![body, ordered, unit],
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

pub(super) fn entity_id_collection_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(210);
    let list_function = assembler.id(210);
    let fixed32_function = assembler.id(210);
    let function = assembler.id(210);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 211,
            p: 211,
            b: 211,
            o: 211,
        },
        decode_function,
    );
    let list_graph = build_generic_list_decode(
        &mut assembler,
        Ns {
            k: 212,
            p: 212,
            b: 212,
            o: 212,
        },
        list_function,
        decode_function,
    );
    let fixed32_graph = build_fixed32_decode(
        &mut assembler,
        Ns {
            k: 213,
            p: 213,
            b: 213,
            o: 213,
        },
        fixed32_function,
    );
    let graph = build_entity_id_collection_decode(
        &mut assembler,
        Ns {
            k: 214,
            p: 214,
            b: 214,
            o: 214,
        },
        function,
        list_function,
        fixed32_function,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph, list_graph, fixed32_graph, decode_graph],
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

fn exact_uvar_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(u64_type()),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(clippy::too_many_lines)]
fn build_exact_uvar_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    decode_function: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = exact_uvar_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let width = assembler.param(ns.p, function, ParameterRole::Function, u32_type());
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let trailing_code = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");

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
    let trailing_error = err_block(assembler, ns, function, result_type.clone(), trailing_code);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
    let success = assembler.id(ns.b);
    let decoded_ready = assembler.id(ns.b);
    let vector_ready = assembler.id(ns.b);

    let success_parameters = block_parameters(assembler, ns.p, success, &[u64_type()]);
    let ok = assembler.op(
        ns.o,
        success,
        Opcode::ResultOk,
        vec![pav(success_parameters[0])],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        success,
        function,
        success_parameters,
        vec![ok],
        ret(op_result(ok)),
    );

    let decoded_parameters = block_parameters(
        assembler,
        ns.p,
        decoded_ready,
        &[TypeExpr::Tuple(vec![u64_type(), u64_type()]), u64_type()],
    );
    let value = assembler.op(
        ns.o,
        decoded_ready,
        Opcode::TupleGet,
        vec![pav(decoded_parameters[0])],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let next = assembler.op(
        ns.o,
        decoded_ready,
        Opcode::TupleGet,
        vec![pav(decoded_parameters[0])],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let exact = assembler.op(
        ns.o,
        decoded_ready,
        Opcode::Equal,
        vec![op_result(next), pav(decoded_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        decoded_ready,
        function,
        decoded_parameters,
        vec![value, next, exact],
        cond(
            op_result(exact),
            edge(success, vec![op_result(value)]),
            edge(trailing_error, Vec::new()),
        ),
    );

    let vector_parameters = block_parameters(assembler, ns.p, vector_ready, &[u8vec_type()]);
    let total = assembler.op(
        ns.o,
        vector_ready,
        Opcode::VectorLen,
        vec![pav(vector_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let zero_constant = assembler.ku64(ns.k, 0);
    let zero = assembler.cref(ns.o, vector_ready, zero_constant, u64_type());
    let decoded = assembler.op(
        ns.o,
        vector_ready,
        Opcode::CallDirect,
        vec![pav(body), op_result(zero), pav(width), pav(unit)],
        vec![decode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: decode_function,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        vector_ready,
        function,
        vector_parameters,
        vec![total, zero, decoded],
        switch(
            op_result(decoded),
            vec![
                (
                    BuiltinCase::Ok,
                    decoded_ready,
                    vec![SwitchArgument::CasePayload, oav(total)],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let entry = assembler.id(ns.b);
    let converted = assembler.op(
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
        vec![converted],
        switch(
            op_result(converted),
            vec![
                (
                    BuiltinCase::Ok,
                    vector_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![body, width, unit],
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
fn build_bounded_uvar_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    exact_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = exact_uvar_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let minimum = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let maximum = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let union_code = assembler.kbytes(ns.k, b"SCB_UNION_INVALID");

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
    let union_error = err_block(assembler, ns, function, result_type.clone(), union_code);
    let success = assembler.id(ns.b);
    let range_ready = assembler.id(ns.b);

    let success_parameters = block_parameters(assembler, ns.p, success, &[u64_type()]);
    let ok = assembler.op(
        ns.o,
        success,
        Opcode::ResultOk,
        vec![pav(success_parameters[0])],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        success,
        function,
        success_parameters,
        vec![ok],
        ret(op_result(ok)),
    );

    let range_parameters = block_parameters(assembler, ns.p, range_ready, &[u64_type()]);
    let below = assembler.op(
        ns.o,
        range_ready,
        Opcode::LessThan,
        vec![pav(range_parameters[0]), pav(minimum)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let above = assembler.op(
        ns.o,
        range_ready,
        Opcode::GreaterThan,
        vec![pav(range_parameters[0]), pav(maximum)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let outside = assembler.op(
        ns.o,
        range_ready,
        Opcode::BoolOr,
        vec![op_result(below), op_result(above)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        range_ready,
        function,
        range_parameters.clone(),
        vec![below, above, outside],
        cond(
            op_result(outside),
            edge(union_error, Vec::new()),
            edge(success, vec![pav(range_parameters[0])]),
        ),
    );

    let entry = assembler.id(ns.b);
    let width32_constant = assembler.ku32(ns.k, 32);
    let width32 = assembler.cref(ns.o, entry, width32_constant, u32_type());
    let decoded = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), op_result(width32), pav(unit)],
        vec![exact_uvar_result_type()],
        Immediate::Function(FunctionRefValue {
            function: exact_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![width32, decoded],
        switch(
            op_result(decoded),
            vec![
                (
                    BuiltinCase::Ok,
                    range_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![body, minimum, maximum, unit],
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

pub(super) fn bounded_uvar_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(215);
    let exact_function = assembler.id(215);
    let function = assembler.id(215);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 216,
            p: 216,
            b: 216,
            o: 216,
        },
        decode_function,
    );
    let exact_graph = build_exact_uvar_decode(
        &mut assembler,
        Ns {
            k: 217,
            p: 217,
            b: 217,
            o: 217,
        },
        exact_function,
        decode_function,
    );
    let graph = build_bounded_uvar_decode(
        &mut assembler,
        Ns {
            k: 218,
            p: 218,
            b: 218,
            o: 218,
        },
        function,
        exact_function,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph, exact_graph, decode_graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: vec![frozen_import(
            BRIDGE_CODE_B2V1,
            TypeExpr::Bytes,
            u8vec_type(),
        )],
        constants: assembler.constants,
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_type_parameter_list_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    list_decoder: EntityId,
    record_decoder: EntityId,
    exact_uvar_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let map_type = generic_record_map_type();
    let option_bytes_type = TypeExpr::Option(Box::new(TypeExpr::Bytes));
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let missing_code = assembler.kbytes(ns.k, b"SCB_FIELD_MISSING");
    let unknown_code = assembler.kbytes(ns.k, b"SCB_FIELD_UNKNOWN");

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
    let missing_error = err_block(assembler, ns, function, result_type.clone(), missing_code);
    let unknown_error = err_block(assembler, ns, function, result_type.clone(), unknown_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let success = assembler.id(ns.b);
    let advance = assembler.id(ns.b);
    let ordinal_call = assembler.id(ns.b);
    let found_check = assembler.id(ns.b);
    let record_ready = assembler.id(ns.b);
    let record_call = assembler.id(ns.b);
    let loop_check = assembler.id(ns.b);
    let initialized = assembler.id(ns.b);
    let list_ready = assembler.id(ns.b);

    let ok = assembler.op(
        ns.o,
        success,
        Opcode::ResultOk,
        vec![pav(unit)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        success,
        function,
        Vec::new(),
        vec![ok],
        ret(op_result(ok)),
    );

    let advance_types = vec![map_type.clone(), map_type.clone(), u64_type()];
    let advance_parameters = block_parameters(assembler, ns.p, advance, &advance_types);
    let one_constant = assembler.ku64(ns.k, 1);
    let one = assembler.cref(ns.o, advance, one_constant, u64_type());
    let next = assembler.op(
        ns.o,
        advance,
        Opcode::IntAddChecked,
        vec![pav(advance_parameters[2]), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        advance,
        function,
        advance_parameters.clone(),
        vec![one, next],
        switch(
            op_result(next),
            vec![
                (
                    BuiltinCase::Ok,
                    loop_check,
                    vec![
                        sav(advance_parameters[0]),
                        sav(advance_parameters[1]),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let ordinal_types = vec![
        TypeExpr::Bytes,
        map_type.clone(),
        map_type.clone(),
        u64_type(),
    ];
    let ordinal_parameters = block_parameters(assembler, ns.p, ordinal_call, &ordinal_types);
    let width_constant = assembler.ku32(ns.k, 32);
    let width = assembler.cref(ns.o, ordinal_call, width_constant, u32_type());
    let ordinal = assembler.op(
        ns.o,
        ordinal_call,
        Opcode::CallDirect,
        vec![pav(ordinal_parameters[0]), op_result(width), pav(unit)],
        vec![exact_uvar_result_type()],
        Immediate::Function(FunctionRefValue {
            function: exact_uvar_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        ordinal_call,
        function,
        ordinal_parameters.clone(),
        vec![width, ordinal],
        switch(
            op_result(ordinal),
            vec![
                (
                    BuiltinCase::Ok,
                    advance,
                    vec![
                        sav(ordinal_parameters[1]),
                        sav(ordinal_parameters[2]),
                        sav(ordinal_parameters[3]),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let record_state_types = vec![
        map_type.clone(),
        map_type.clone(),
        map_type.clone(),
        u64_type(),
    ];
    let found_parameters = block_parameters(assembler, ns.p, found_check, &record_state_types);
    let field_tag_constant = assembler.ku64(ns.k, 1);
    let field_tag = assembler.cref(ns.o, found_check, field_tag_constant, u64_type());
    let found = assembler.op(
        ns.o,
        found_check,
        Opcode::MapGet,
        vec![pav(found_parameters[0]), op_result(field_tag)],
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        found_check,
        function,
        found_parameters.clone(),
        vec![field_tag, found],
        switch(
            op_result(found),
            vec![
                (BuiltinCase::None, missing_error, Vec::new()),
                (
                    BuiltinCase::Some,
                    ordinal_call,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(found_parameters[1]),
                        sav(found_parameters[2]),
                        sav(found_parameters[3]),
                    ],
                ),
            ],
        ),
    );

    let record_parameters = block_parameters(assembler, ns.p, record_ready, &record_state_types);
    let record_tag_constant = assembler.ku64(ns.k, 1);
    let record_tag = assembler.cref(ns.o, record_ready, record_tag_constant, u64_type());
    let remaining = assembler.op(
        ns.o,
        record_ready,
        Opcode::MapRemove,
        vec![pav(record_parameters[0]), op_result(record_tag)],
        vec![map_type.clone()],
        Immediate::None,
    );
    let no_unknown = assembler.op(
        ns.o,
        record_ready,
        Opcode::Equal,
        vec![op_result(remaining), pav(record_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        record_ready,
        function,
        record_parameters.clone(),
        vec![record_tag, remaining, no_unknown],
        cond(
            op_result(no_unknown),
            edge(
                found_check,
                record_parameters.iter().copied().map(pav).collect(),
            ),
            edge(unknown_error, Vec::new()),
        ),
    );

    let record_call_types = vec![
        TypeExpr::Bytes,
        map_type.clone(),
        map_type.clone(),
        u64_type(),
    ];
    let record_call_parameters = block_parameters(assembler, ns.p, record_call, &record_call_types);
    let record = assembler.op(
        ns.o,
        record_call,
        Opcode::CallDirect,
        vec![pav(record_call_parameters[0]), pav(unit)],
        vec![generic_record_result_type()],
        Immediate::Function(FunctionRefValue {
            function: record_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        record_call,
        function,
        record_call_parameters.clone(),
        vec![record],
        switch(
            op_result(record),
            vec![
                (
                    BuiltinCase::Ok,
                    record_ready,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(record_call_parameters[1]),
                        sav(record_call_parameters[2]),
                        sav(record_call_parameters[3]),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let loop_types = vec![map_type.clone(), map_type.clone(), u64_type()];
    let loop_parameters = block_parameters(assembler, ns.p, loop_check, &loop_types);
    let item = assembler.op(
        ns.o,
        loop_check,
        Opcode::MapGet,
        vec![pav(loop_parameters[1]), pav(loop_parameters[2])],
        vec![option_bytes_type],
        Immediate::None,
    );
    append_block(
        assembler,
        loop_check,
        function,
        loop_parameters.clone(),
        vec![item],
        switch(
            op_result(item),
            vec![
                (BuiltinCase::None, success, Vec::new()),
                (
                    BuiltinCase::Some,
                    record_call,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(loop_parameters[0]),
                        sav(loop_parameters[1]),
                        sav(loop_parameters[2]),
                    ],
                ),
            ],
        ),
    );

    let initialized_parameters = block_parameters(
        assembler,
        ns.p,
        initialized,
        &[map_type.clone(), map_type.clone()],
    );
    let zero_constant = assembler.ku64(ns.k, 0);
    let zero = assembler.cref(ns.o, initialized, zero_constant, u64_type());
    append_block(
        assembler,
        initialized,
        function,
        initialized_parameters.clone(),
        vec![zero],
        branch(edge(
            loop_check,
            vec![
                pav(initialized_parameters[0]),
                pav(initialized_parameters[1]),
                op_result(zero),
            ],
        )),
    );

    let list_ready_parameters =
        block_parameters(assembler, ns.p, list_ready, std::slice::from_ref(&map_type));
    let map_new_result = TypeExpr::Result {
        ok: Box::new(map_type.clone()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let empty_map = assembler.op(
        ns.o,
        list_ready,
        Opcode::MapNew,
        Vec::new(),
        vec![map_new_result],
        Immediate::None,
    );
    append_block(
        assembler,
        list_ready,
        function,
        list_ready_parameters.clone(),
        vec![empty_map],
        switch(
            op_result(empty_map),
            vec![
                (
                    BuiltinCase::Ok,
                    initialized,
                    vec![SwitchArgument::CasePayload, sav(list_ready_parameters[0])],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let entry = assembler.id(ns.b);
    let list = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![generic_record_result_type()],
        Immediate::Function(FunctionRefValue {
            function: list_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![list],
        switch(
            op_result(list),
            vec![
                (
                    BuiltinCase::Ok,
                    list_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
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

#[allow(clippy::too_many_lines)]
pub(super) fn type_parameter_list_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(226);
    let list_function = assembler.id(226);
    let record_function = assembler.id(226);
    let exact_uvar_function = assembler.id(226);
    let function = assembler.id(226);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 227,
            p: 227,
            b: 227,
            o: 227,
        },
        decode_function,
    );
    let list_graph = build_generic_list_decode(
        &mut assembler,
        Ns {
            k: 228,
            p: 228,
            b: 228,
            o: 228,
        },
        list_function,
        decode_function,
    );
    let record_graph = build_generic_record_decode(
        &mut assembler,
        Ns {
            k: 229,
            p: 229,
            b: 229,
            o: 229,
        },
        record_function,
        decode_function,
    );
    let exact_uvar_graph = build_exact_uvar_decode(
        &mut assembler,
        Ns {
            k: 230,
            p: 230,
            b: 230,
            o: 230,
        },
        exact_uvar_function,
        decode_function,
    );
    let graph = build_type_parameter_list_decode(
        &mut assembler,
        Ns {
            k: 231,
            p: 231,
            b: 231,
            o: 231,
        },
        function,
        list_function,
        record_function,
        exact_uvar_function,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            record_graph,
            exact_uvar_graph,
            list_graph,
            decode_graph,
        ],
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

fn exact_record_projection_result_type(field_count: usize) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![TypeExpr::Bytes; field_count])),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_exact_record_projection(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    decode_function: EntityId,
    record_decoder: EntityId,
    field_count: usize,
) -> FunctionGraph {
    assert!(field_count > 0, "exact record projection needs fields");
    let block_start = assembler.blocks.len();
    let result_type = exact_record_projection_result_type(field_count);
    let tuple_type = TypeExpr::Tuple(vec![TypeExpr::Bytes; field_count]);
    let map_type = generic_record_map_type();
    let option_bytes_type = TypeExpr::Option(Box::new(TypeExpr::Bytes));
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let missing_code = assembler.kbytes(ns.k, b"SCB_FIELD_MISSING");
    let unknown_code = assembler.kbytes(ns.k, b"SCB_FIELD_UNKNOWN");

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
    let missing_error = err_block(assembler, ns, function, result_type.clone(), missing_code);
    let unknown_error = err_block(assembler, ns, function, result_type.clone(), unknown_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let success = assembler.id(ns.b);
    let get_blocks = (0..field_count)
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let remove_known = assembler.id(ns.b);
    let create_empty = assembler.id(ns.b);
    let exact_count = assembler.id(ns.b);
    let count_ready = assembler.id(ns.b);
    let record_ready = assembler.id(ns.b);

    let success_parameters = block_parameters(
        assembler,
        ns.p,
        success,
        &vec![TypeExpr::Bytes; field_count],
    );
    let tuple = assembler.op(
        ns.o,
        success,
        Opcode::TupleNew,
        success_parameters.iter().copied().map(pav).collect(),
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
        function,
        success_parameters,
        vec![tuple, ok],
        ret(op_result(ok)),
    );

    for (index, block) in get_blocks.iter().copied().enumerate() {
        let mut value_types = vec![map_type.clone()];
        value_types.extend(vec![TypeExpr::Bytes; index]);
        let parameters = block_parameters(assembler, ns.p, block, &value_types);
        let tag_constant = assembler.ku64(ns.k, (index + 1) as u128);
        let tag = assembler.cref(ns.o, block, tag_constant, u64_type());
        let found = assembler.op(
            ns.o,
            block,
            Opcode::MapGet,
            vec![pav(parameters[0]), op_result(tag)],
            vec![option_bytes_type.clone()],
            Immediate::None,
        );
        let destination = if index + 1 == field_count {
            success
        } else {
            get_blocks[index + 1]
        };
        let mut arguments = parameters[1..].iter().copied().map(sav).collect::<Vec<_>>();
        if index + 1 < field_count {
            arguments.insert(0, sav(parameters[0]));
        }
        arguments.push(SwitchArgument::CasePayload);
        append_block(
            assembler,
            block,
            function,
            parameters,
            vec![tag, found],
            switch(
                op_result(found),
                vec![
                    (BuiltinCase::None, missing_error, Vec::new()),
                    (BuiltinCase::Some, destination, arguments),
                ],
            ),
        );
    }

    let remove_parameters = block_parameters(
        assembler,
        ns.p,
        remove_known,
        &[map_type.clone(), map_type.clone()],
    );
    let mut operations = Vec::new();
    let mut remainder = pav(remove_parameters[0]);
    for index in 0..field_count {
        let tag_constant = assembler.ku64(ns.k, (index + 1) as u128);
        let tag = assembler.cref(ns.o, remove_known, tag_constant, u64_type());
        let removed = assembler.op(
            ns.o,
            remove_known,
            Opcode::MapRemove,
            vec![remainder, op_result(tag)],
            vec![map_type.clone()],
            Immediate::None,
        );
        operations.extend([tag, removed]);
        remainder = op_result(removed);
    }
    let no_unknown = assembler.op(
        ns.o,
        remove_known,
        Opcode::Equal,
        vec![remainder, pav(remove_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    operations.push(no_unknown);
    append_block(
        assembler,
        remove_known,
        function,
        remove_parameters.clone(),
        operations,
        cond(
            op_result(no_unknown),
            edge(get_blocks[0], vec![pav(remove_parameters[0])]),
            edge(unknown_error, Vec::new()),
        ),
    );

    let create_parameters = block_parameters(
        assembler,
        ns.p,
        create_empty,
        std::slice::from_ref(&map_type),
    );
    let map_new_result = TypeExpr::Result {
        ok: Box::new(map_type.clone()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let empty_map = assembler.op(
        ns.o,
        create_empty,
        Opcode::MapNew,
        Vec::new(),
        vec![map_new_result],
        Immediate::None,
    );
    append_block(
        assembler,
        create_empty,
        function,
        create_parameters.clone(),
        vec![empty_map],
        switch(
            op_result(empty_map),
            vec![
                (
                    BuiltinCase::Ok,
                    remove_known,
                    vec![sav(create_parameters[0]), SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let exact_parameters = block_parameters(
        assembler,
        ns.p,
        exact_count,
        &[u64_type(), map_type.clone()],
    );
    let expected_constant = assembler.ku64(ns.k, field_count as u128);
    let expected = assembler.cref(ns.o, exact_count, expected_constant, u64_type());
    let matches = assembler.op(
        ns.o,
        exact_count,
        Opcode::Equal,
        vec![pav(exact_parameters[0]), op_result(expected)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        exact_count,
        function,
        exact_parameters.clone(),
        vec![expected, matches],
        cond(
            op_result(matches),
            edge(create_empty, vec![pav(exact_parameters[1])]),
            edge(unknown_error, Vec::new()),
        ),
    );

    let count_parameters = block_parameters(
        assembler,
        ns.p,
        count_ready,
        &[
            TypeExpr::Tuple(vec![u64_type(), u64_type()]),
            map_type.clone(),
        ],
    );
    let count = assembler.op(
        ns.o,
        count_ready,
        Opcode::TupleGet,
        vec![pav(count_parameters[0])],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let expected_constant = assembler.ku64(ns.k, field_count as u128);
    let expected = assembler.cref(ns.o, count_ready, expected_constant, u64_type());
    let too_few = assembler.op(
        ns.o,
        count_ready,
        Opcode::LessThan,
        vec![op_result(count), op_result(expected)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        count_ready,
        function,
        count_parameters.clone(),
        vec![count, expected, too_few],
        cond(
            op_result(too_few),
            edge(missing_error, Vec::new()),
            edge(
                exact_count,
                vec![op_result(count), pav(count_parameters[1])],
            ),
        ),
    );

    let record_parameters = block_parameters(
        assembler,
        ns.p,
        record_ready,
        std::slice::from_ref(&map_type),
    );
    let zero_constant = assembler.ku64(ns.k, 0);
    let zero = assembler.cref(ns.o, record_ready, zero_constant, u64_type());
    let width_constant = assembler.ku32(ns.k, 64);
    let width = assembler.cref(ns.o, record_ready, width_constant, u32_type());
    let decoded_count = assembler.op(
        ns.o,
        record_ready,
        Opcode::CallDirect,
        vec![pav(body), op_result(zero), op_result(width), pav(unit)],
        vec![decode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: decode_function,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        record_ready,
        function,
        record_parameters.clone(),
        vec![zero, width, decoded_count],
        switch(
            op_result(decoded_count),
            vec![
                (
                    BuiltinCase::Ok,
                    count_ready,
                    vec![SwitchArgument::CasePayload, sav(record_parameters[0])],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let entry = assembler.id(ns.b);
    let record = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![generic_record_result_type()],
        Immediate::Function(FunctionRefValue {
            function: record_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![record],
        switch(
            op_result(record),
            vec![
                (
                    BuiltinCase::Ok,
                    record_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
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

fn type_expr_children_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![
            TypeExpr::Option(Box::new(TypeExpr::Bytes)),
            TypeExpr::Option(Box::new(TypeExpr::Bytes)),
            generic_record_map_type(),
        ])),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_type_expr_children_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    union_decoder: EntityId,
    leaf_decoder: EntityId,
    list_decoder: EntityId,
    record2_decoder: EntityId,
    record3_decoder: EntityId,
    fixed32_decoder: EntityId,
    entity_id_collection_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = type_expr_children_result_type();
    let map_type = generic_record_map_type();
    let option_bytes_type = TypeExpr::Option(Box::new(TypeExpr::Bytes));
    let tuple_type = TypeExpr::Tuple(vec![
        option_bytes_type.clone(),
        option_bytes_type.clone(),
        map_type.clone(),
    ]);
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let union_code = assembler.kbytes(ns.k, b"SCB_UNION_INVALID");

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
    let union_error = err_block(assembler, ns, function, result_type.clone(), union_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let success = assembler.id(ns.b);
    let empty_success = assembler.id(ns.b);
    let unary_success = assembler.id(ns.b);
    let binary_success = assembler.id(ns.b);
    let function_effects_ready = assembler.id(ns.b);
    let function_parameters_ready = assembler.id(ns.b);
    let function_fields_ready = assembler.id(ns.b);
    let function_call = assembler.id(ns.b);
    let named_definition_ready = assembler.id(ns.b);
    let named_fields_ready = assembler.id(ns.b);
    let named_call = assembler.id(ns.b);
    let pair_fields_ready = assembler.id(ns.b);
    let pair_call = assembler.id(ns.b);
    let list_call = assembler.id(ns.b);
    let unary_ready = assembler.id(ns.b);
    let leaf_call = assembler.id(ns.b);
    let dispatch = std::array::from_fn::<_, 20, _>(|_| assembler.id(ns.b));
    let union_ready = assembler.id(ns.b);
    let union_call = assembler.id(ns.b);

    let success_parameters = block_parameters(
        assembler,
        ns.p,
        success,
        &[
            option_bytes_type.clone(),
            option_bytes_type.clone(),
            map_type.clone(),
        ],
    );
    let tuple = assembler.op(
        ns.o,
        success,
        Opcode::TupleNew,
        success_parameters.iter().copied().map(pav).collect(),
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
        function,
        success_parameters,
        vec![tuple, ok],
        ret(op_result(ok)),
    );

    let empty_parameters = block_parameters(
        assembler,
        ns.p,
        empty_success,
        std::slice::from_ref(&map_type),
    );
    let first_none = assembler.op(
        ns.o,
        empty_success,
        Opcode::OptionNone,
        Vec::new(),
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    let second_none = assembler.op(
        ns.o,
        empty_success,
        Opcode::OptionNone,
        Vec::new(),
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        empty_success,
        function,
        empty_parameters.clone(),
        vec![first_none, second_none],
        branch(edge(
            success,
            vec![
                op_result(first_none),
                op_result(second_none),
                pav(empty_parameters[0]),
            ],
        )),
    );

    let unary_success_parameters = block_parameters(
        assembler,
        ns.p,
        unary_success,
        &[TypeExpr::Bytes, map_type.clone()],
    );
    let first_some = assembler.op(
        ns.o,
        unary_success,
        Opcode::OptionSome,
        vec![pav(unary_success_parameters[0])],
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    let second_none = assembler.op(
        ns.o,
        unary_success,
        Opcode::OptionNone,
        Vec::new(),
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        unary_success,
        function,
        unary_success_parameters.clone(),
        vec![first_some, second_none],
        branch(edge(
            success,
            vec![
                op_result(first_some),
                op_result(second_none),
                pav(unary_success_parameters[1]),
            ],
        )),
    );

    let binary_success_parameters = block_parameters(
        assembler,
        ns.p,
        binary_success,
        &[TypeExpr::Bytes, TypeExpr::Bytes, map_type.clone()],
    );
    let first_some = assembler.op(
        ns.o,
        binary_success,
        Opcode::OptionSome,
        vec![pav(binary_success_parameters[0])],
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    let second_some = assembler.op(
        ns.o,
        binary_success,
        Opcode::OptionSome,
        vec![pav(binary_success_parameters[1])],
        vec![option_bytes_type],
        Immediate::None,
    );
    append_block(
        assembler,
        binary_success,
        function,
        binary_success_parameters.clone(),
        vec![first_some, second_some],
        branch(edge(
            success,
            vec![
                op_result(first_some),
                op_result(second_some),
                pav(binary_success_parameters[2]),
            ],
        )),
    );

    let function_effects_parameters = block_parameters(
        assembler,
        ns.p,
        function_effects_ready,
        &[TypeExpr::Bytes, map_type.clone()],
    );
    append_block(
        assembler,
        function_effects_ready,
        function,
        function_effects_parameters.clone(),
        Vec::new(),
        branch(edge(
            unary_success,
            vec![
                pav(function_effects_parameters[0]),
                pav(function_effects_parameters[1]),
            ],
        )),
    );

    let function_parameters_parameters = block_parameters(
        assembler,
        ns.p,
        function_parameters_ready,
        &[TypeExpr::Bytes, TypeExpr::Bytes, map_type.clone()],
    );
    let ordered_constant = assembler.kbool(ns.k, true);
    let ordered = assembler.cref(
        ns.o,
        function_parameters_ready,
        ordered_constant,
        TypeExpr::Bool,
    );
    let effects = assembler.op(
        ns.o,
        function_parameters_ready,
        Opcode::CallDirect,
        vec![
            pav(function_parameters_parameters[1]),
            op_result(ordered),
            pav(unit),
        ],
        vec![unit_validation_result_type()],
        Immediate::Function(FunctionRefValue {
            function: entity_id_collection_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        function_parameters_ready,
        function,
        function_parameters_parameters.clone(),
        vec![ordered, effects],
        switch(
            op_result(effects),
            vec![
                (
                    BuiltinCase::Ok,
                    function_effects_ready,
                    vec![
                        sav(function_parameters_parameters[0]),
                        sav(function_parameters_parameters[2]),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let function_fields_parameters = block_parameters(
        assembler,
        ns.p,
        function_fields_ready,
        &[TypeExpr::Tuple(vec![TypeExpr::Bytes; 3]), map_type.clone()],
    );
    let parameters_payload = assembler.op(
        ns.o,
        function_fields_ready,
        Opcode::TupleGet,
        vec![pav(function_fields_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let result_payload = assembler.op(
        ns.o,
        function_fields_ready,
        Opcode::TupleGet,
        vec![pav(function_fields_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let effects_payload = assembler.op(
        ns.o,
        function_fields_ready,
        Opcode::TupleGet,
        vec![pav(function_fields_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(2),
    );
    let parameters = assembler.op(
        ns.o,
        function_fields_ready,
        Opcode::CallDirect,
        vec![op_result(parameters_payload), pav(unit)],
        vec![generic_record_result_type()],
        Immediate::Function(FunctionRefValue {
            function: list_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        function_fields_ready,
        function,
        function_fields_parameters,
        vec![
            parameters_payload,
            result_payload,
            effects_payload,
            parameters,
        ],
        switch(
            op_result(parameters),
            vec![
                (
                    BuiltinCase::Ok,
                    function_parameters_ready,
                    vec![
                        oav(result_payload),
                        oav(effects_payload),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let function_call_parameters = block_parameters(
        assembler,
        ns.p,
        function_call,
        &[TypeExpr::Bytes, map_type.clone()],
    );
    let function_fields = assembler.op(
        ns.o,
        function_call,
        Opcode::CallDirect,
        vec![pav(function_call_parameters[0]), pav(unit)],
        vec![exact_record_projection_result_type(3)],
        Immediate::Function(FunctionRefValue {
            function: record3_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        function_call,
        function,
        function_call_parameters.clone(),
        vec![function_fields],
        switch(
            op_result(function_fields),
            vec![
                (
                    BuiltinCase::Ok,
                    function_fields_ready,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(function_call_parameters[1]),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let named_definition_parameters = block_parameters(
        assembler,
        ns.p,
        named_definition_ready,
        &[TypeExpr::Bytes, map_type.clone()],
    );
    let arguments = assembler.op(
        ns.o,
        named_definition_ready,
        Opcode::CallDirect,
        vec![pav(named_definition_parameters[0]), pav(unit)],
        vec![generic_record_result_type()],
        Immediate::Function(FunctionRefValue {
            function: list_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        named_definition_ready,
        function,
        named_definition_parameters,
        vec![arguments],
        switch(
            op_result(arguments),
            vec![
                (
                    BuiltinCase::Ok,
                    empty_success,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let named_fields_parameters = block_parameters(
        assembler,
        ns.p,
        named_fields_ready,
        &[TypeExpr::Tuple(vec![TypeExpr::Bytes; 2]), map_type.clone()],
    );
    let definition_payload = assembler.op(
        ns.o,
        named_fields_ready,
        Opcode::TupleGet,
        vec![pav(named_fields_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let arguments_payload = assembler.op(
        ns.o,
        named_fields_ready,
        Opcode::TupleGet,
        vec![pav(named_fields_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let definition = assembler.op(
        ns.o,
        named_fields_ready,
        Opcode::CallDirect,
        vec![op_result(definition_payload), pav(unit)],
        vec![bytes_validation_result_type()],
        Immediate::Function(FunctionRefValue {
            function: fixed32_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        named_fields_ready,
        function,
        named_fields_parameters.clone(),
        vec![definition_payload, arguments_payload, definition],
        switch(
            op_result(definition),
            vec![
                (
                    BuiltinCase::Ok,
                    named_definition_ready,
                    vec![oav(arguments_payload), sav(named_fields_parameters[1])],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let named_call_parameters = block_parameters(
        assembler,
        ns.p,
        named_call,
        &[TypeExpr::Bytes, map_type.clone()],
    );
    let named_fields = assembler.op(
        ns.o,
        named_call,
        Opcode::CallDirect,
        vec![pav(named_call_parameters[0]), pav(unit)],
        vec![exact_record_projection_result_type(2)],
        Immediate::Function(FunctionRefValue {
            function: record2_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        named_call,
        function,
        named_call_parameters.clone(),
        vec![named_fields],
        switch(
            op_result(named_fields),
            vec![
                (
                    BuiltinCase::Ok,
                    named_fields_ready,
                    vec![SwitchArgument::CasePayload, sav(named_call_parameters[1])],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let pair_fields_parameters = block_parameters(
        assembler,
        ns.p,
        pair_fields_ready,
        &[TypeExpr::Tuple(vec![TypeExpr::Bytes; 2]), map_type.clone()],
    );
    let first = assembler.op(
        ns.o,
        pair_fields_ready,
        Opcode::TupleGet,
        vec![pav(pair_fields_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let second = assembler.op(
        ns.o,
        pair_fields_ready,
        Opcode::TupleGet,
        vec![pav(pair_fields_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    append_block(
        assembler,
        pair_fields_ready,
        function,
        pair_fields_parameters.clone(),
        vec![first, second],
        branch(edge(
            binary_success,
            vec![
                op_result(first),
                op_result(second),
                pav(pair_fields_parameters[1]),
            ],
        )),
    );

    let pair_call_parameters = block_parameters(
        assembler,
        ns.p,
        pair_call,
        &[TypeExpr::Bytes, map_type.clone()],
    );
    let pair_fields = assembler.op(
        ns.o,
        pair_call,
        Opcode::CallDirect,
        vec![pav(pair_call_parameters[0]), pav(unit)],
        vec![exact_record_projection_result_type(2)],
        Immediate::Function(FunctionRefValue {
            function: record2_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        pair_call,
        function,
        pair_call_parameters.clone(),
        vec![pair_fields],
        switch(
            op_result(pair_fields),
            vec![
                (
                    BuiltinCase::Ok,
                    pair_fields_ready,
                    vec![SwitchArgument::CasePayload, sav(pair_call_parameters[1])],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let list_parameters = block_parameters(
        assembler,
        ns.p,
        list_call,
        &[TypeExpr::Bytes, map_type.clone()],
    );
    let list = assembler.op(
        ns.o,
        list_call,
        Opcode::CallDirect,
        vec![pav(list_parameters[0]), pav(unit)],
        vec![generic_record_result_type()],
        Immediate::Function(FunctionRefValue {
            function: list_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        list_call,
        function,
        list_parameters,
        vec![list],
        switch(
            op_result(list),
            vec![
                (
                    BuiltinCase::Ok,
                    empty_success,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let unary_parameters = block_parameters(
        assembler,
        ns.p,
        unary_ready,
        &[TypeExpr::Bytes, map_type.clone()],
    );
    append_block(
        assembler,
        unary_ready,
        function,
        unary_parameters.clone(),
        Vec::new(),
        branch(edge(
            unary_success,
            vec![pav(unary_parameters[0]), pav(unary_parameters[1])],
        )),
    );

    let leaf_parameters =
        block_parameters(assembler, ns.p, leaf_call, std::slice::from_ref(&map_type));
    let leaf = assembler.op(
        ns.o,
        leaf_call,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![bytes_validation_result_type()],
        Immediate::Function(FunctionRefValue {
            function: leaf_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        leaf_call,
        function,
        leaf_parameters.clone(),
        vec![leaf],
        switch(
            op_result(leaf),
            vec![
                (
                    BuiltinCase::Ok,
                    empty_success,
                    vec![sav(leaf_parameters[0])],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    for (index, block) in dispatch.iter().copied().enumerate() {
        let parameters = block_parameters(
            assembler,
            ns.p,
            block,
            &[u64_type(), TypeExpr::Bytes, map_type.clone()],
        );
        let tag = u64::try_from(index + 1).expect("TypeExpr tag fits u64");
        let expected_constant = assembler.ku64(ns.k, u128::from(tag));
        let expected = assembler.cref(ns.o, block, expected_constant, u64_type());
        let matches = assembler.op(
            ns.o,
            block,
            Opcode::Equal,
            vec![pav(parameters[0]), op_result(expected)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        let true_edge = match tag {
            1..=8 | 16 | 17 | 19 | 20 => edge(leaf_call, vec![pav(parameters[2])]),
            9 => edge(list_call, vec![pav(parameters[1]), pav(parameters[2])]),
            10 => edge(named_call, vec![pav(parameters[1]), pav(parameters[2])]),
            11 | 13 | 18 => edge(unary_ready, vec![pav(parameters[1]), pav(parameters[2])]),
            12 | 14 => edge(pair_call, vec![pav(parameters[1]), pav(parameters[2])]),
            15 => edge(function_call, vec![pav(parameters[1]), pav(parameters[2])]),
            _ => unreachable!(),
        };
        let false_edge = if index + 1 == dispatch.len() {
            edge(union_error, Vec::new())
        } else {
            edge(
                dispatch[index + 1],
                vec![pav(parameters[0]), pav(parameters[1]), pav(parameters[2])],
            )
        };
        append_block(
            assembler,
            block,
            function,
            parameters,
            vec![expected, matches],
            cond(op_result(matches), true_edge, false_edge),
        );
    }

    let union_parameters = block_parameters(
        assembler,
        ns.p,
        union_ready,
        &[
            TypeExpr::Tuple(vec![u64_type(), TypeExpr::Bytes]),
            map_type.clone(),
        ],
    );
    let tag = assembler.op(
        ns.o,
        union_ready,
        Opcode::TupleGet,
        vec![pav(union_parameters[0])],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let payload = assembler.op(
        ns.o,
        union_ready,
        Opcode::TupleGet,
        vec![pav(union_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    append_block(
        assembler,
        union_ready,
        function,
        union_parameters.clone(),
        vec![tag, payload],
        branch(edge(
            dispatch[0],
            vec![op_result(tag), op_result(payload), pav(union_parameters[1])],
        )),
    );

    let union_call_parameters =
        block_parameters(assembler, ns.p, union_call, std::slice::from_ref(&map_type));
    let decoded = assembler.op(
        ns.o,
        union_call,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![generic_union_result_type()],
        Immediate::Function(FunctionRefValue {
            function: union_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        union_call,
        function,
        union_call_parameters.clone(),
        vec![decoded],
        switch(
            op_result(decoded),
            vec![
                (
                    BuiltinCase::Ok,
                    union_ready,
                    vec![SwitchArgument::CasePayload, sav(union_call_parameters[0])],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let entry = assembler.id(ns.b);
    let map_new_result = TypeExpr::Result {
        ok: Box::new(map_type),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let empty_map = assembler.op(
        ns.o,
        entry,
        Opcode::MapNew,
        Vec::new(),
        vec![map_new_result],
        Immediate::None,
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![empty_map],
        switch(
            op_result(empty_map),
            vec![
                (
                    BuiltinCase::Ok,
                    union_call,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
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

#[allow(clippy::similar_names, clippy::too_many_lines)]
pub(super) fn type_expr_children_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(232);
    let record_function = assembler.id(232);
    let union_function = assembler.id(232);
    let list_function = assembler.id(232);
    let fixed32_function = assembler.id(232);
    let entity_id_collection_function = assembler.id(232);
    let exact_uvar_function = assembler.id(232);
    let bounded_uvar_function = assembler.id(232);
    let leaf_function = assembler.id(232);
    let record2_function = assembler.id(232);
    let record3_function = assembler.id(232);
    let function = assembler.id(232);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 233,
            p: 233,
            b: 233,
            o: 233,
        },
        decode_function,
    );
    let record_graph = build_generic_record_decode(
        &mut assembler,
        Ns {
            k: 234,
            p: 234,
            b: 234,
            o: 234,
        },
        record_function,
        decode_function,
    );
    let union_graph = build_generic_union_decode(
        &mut assembler,
        Ns {
            k: 235,
            p: 235,
            b: 235,
            o: 235,
        },
        union_function,
        decode_function,
    );
    let list_graph = build_generic_list_decode(
        &mut assembler,
        Ns {
            k: 236,
            p: 236,
            b: 236,
            o: 236,
        },
        list_function,
        decode_function,
    );
    let fixed32_graph = build_fixed32_decode(
        &mut assembler,
        Ns {
            k: 237,
            p: 237,
            b: 237,
            o: 237,
        },
        fixed32_function,
    );
    let entity_id_collection_graph = build_entity_id_collection_decode(
        &mut assembler,
        Ns {
            k: 238,
            p: 238,
            b: 238,
            o: 238,
        },
        entity_id_collection_function,
        list_function,
        fixed32_function,
    );
    let exact_uvar_graph = build_exact_uvar_decode(
        &mut assembler,
        Ns {
            k: 239,
            p: 239,
            b: 239,
            o: 239,
        },
        exact_uvar_function,
        decode_function,
    );
    let bounded_uvar_graph = build_bounded_uvar_decode(
        &mut assembler,
        Ns {
            k: 240,
            p: 240,
            b: 240,
            o: 240,
        },
        bounded_uvar_function,
        exact_uvar_function,
    );
    let leaf_graph = build_type_expr_leaf_decode(
        &mut assembler,
        Ns {
            k: 241,
            p: 241,
            b: 241,
            o: 241,
        },
        leaf_function,
        union_function,
        fixed32_function,
        exact_uvar_function,
        bounded_uvar_function,
    );
    let record2_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 242,
            p: 242,
            b: 242,
            o: 242,
        },
        record2_function,
        decode_function,
        record_function,
        2,
    );
    let record3_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 243,
            p: 243,
            b: 243,
            o: 243,
        },
        record3_function,
        decode_function,
        record_function,
        3,
    );
    let graph = build_type_expr_children_decode(
        &mut assembler,
        Ns {
            k: 244,
            p: 244,
            b: 244,
            o: 244,
        },
        function,
        union_function,
        leaf_function,
        list_function,
        record2_function,
        record3_function,
        fixed32_function,
        entity_id_collection_function,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            record3_graph,
            record2_graph,
            leaf_graph,
            bounded_uvar_graph,
            exact_uvar_graph,
            entity_id_collection_graph,
            fixed32_graph,
            list_graph,
            union_graph,
            record_graph,
            decode_graph,
        ],
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

fn u64_map_type() -> TypeExpr {
    TypeExpr::OrderedMap {
        key: Box::new(u64_type()),
        value: Box::new(u64_type()),
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_type_expr_append_child(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    resource_error: EntityId,
    invariant_trap: EntityId,
    destination: EntityId,
    carry_types: &[TypeExpr],
) -> EntityId {
    let bytes_map_type = generic_record_map_type();
    let depths_map_type = u64_map_type();
    let mut input_types = vec![
        TypeExpr::Bytes,
        bytes_map_type.clone(),
        depths_map_type.clone(),
        u64_type(),
        u64_type(),
        u64_type(),
    ];
    input_types.extend_from_slice(carry_types);
    let gate = assembler.id(ns.b);
    let depth_ready = assembler.id(ns.b);
    let append = assembler.id(ns.b);

    let append_parameters = block_parameters(assembler, ns.p, append, &input_types);
    let inserted_work = assembler.op(
        ns.o,
        append,
        Opcode::MapInsert,
        vec![
            pav(append_parameters[1]),
            pav(append_parameters[4]),
            pav(append_parameters[0]),
        ],
        vec![bytes_map_type],
        Immediate::None,
    );
    let inserted_depth = assembler.op(
        ns.o,
        append,
        Opcode::MapInsert,
        vec![
            pav(append_parameters[2]),
            pav(append_parameters[4]),
            pav(append_parameters[5]),
        ],
        vec![depths_map_type],
        Immediate::None,
    );
    let one_constant = assembler.ku64(ns.k, 1);
    let one = assembler.cref(ns.o, append, one_constant, u64_type());
    let next_end = assembler.op(
        ns.o,
        append,
        Opcode::IntAddChecked,
        vec![pav(append_parameters[4]), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let mut destination_arguments = vec![
        oav(inserted_work),
        oav(inserted_depth),
        sav(append_parameters[3]),
        SwitchArgument::CasePayload,
        sav(append_parameters[5]),
    ];
    destination_arguments.extend(append_parameters[6..].iter().copied().map(sav));
    append_block(
        assembler,
        append,
        function,
        append_parameters,
        vec![inserted_work, inserted_depth, one, next_end],
        switch(
            op_result(next_end),
            vec![
                (BuiltinCase::Ok, destination, destination_arguments),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let depth_parameters = block_parameters(assembler, ns.p, depth_ready, &input_types);
    let one_constant = assembler.ku64(ns.k, 1);
    let one = assembler.cref(ns.o, depth_ready, one_constant, u64_type());
    let child_depth = assembler.op(
        ns.o,
        depth_ready,
        Opcode::IntAddChecked,
        vec![pav(depth_parameters[5]), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let mut append_arguments = depth_parameters
        .iter()
        .copied()
        .map(sav)
        .collect::<Vec<_>>();
    append_arguments[5] = SwitchArgument::CasePayload;
    append_block(
        assembler,
        depth_ready,
        function,
        depth_parameters,
        vec![one, child_depth],
        switch(
            op_result(child_depth),
            vec![
                (BuiltinCase::Ok, append, append_arguments),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let gate_parameters = block_parameters(assembler, ns.p, gate, &input_types);
    let maximum_constant = assembler.ku64(ns.k, 63);
    let maximum = assembler.cref(ns.o, gate, maximum_constant, u64_type());
    let may_descend = assembler.op(
        ns.o,
        gate,
        Opcode::LessThan,
        vec![pav(gate_parameters[5]), op_result(maximum)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        gate,
        function,
        gate_parameters.clone(),
        vec![maximum, may_descend],
        cond(
            op_result(may_descend),
            edge(
                depth_ready,
                gate_parameters.iter().copied().map(pav).collect(),
            ),
            edge(resource_error, Vec::new()),
        ),
    );
    gate
}

#[allow(clippy::too_many_lines)]
fn build_type_expr_recursive_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    children_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = bytes_validation_result_type();
    let bytes_map_type = generic_record_map_type();
    let depths_map_type = u64_map_type();
    let option_bytes_type = TypeExpr::Option(Box::new(TypeExpr::Bytes));
    let option_u64_type = TypeExpr::Option(Box::new(u64_type()));
    let children_tuple_type = TypeExpr::Tuple(vec![
        option_bytes_type.clone(),
        option_bytes_type.clone(),
        bytes_map_type.clone(),
    ]);
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");

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
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let success = assembler.id(ns.b);
    let advance_node = assembler.id(ns.b);
    let list_advance = assembler.id(ns.b);
    let list_loop = assembler.id(ns.b);
    let list_init = assembler.id(ns.b);
    let direct_two = assembler.id(ns.b);
    let direct_one = assembler.id(ns.b);
    let children_ready = assembler.id(ns.b);
    let children_call = assembler.id(ns.b);
    let depth_lookup = assembler.id(ns.b);
    let work_lookup = assembler.id(ns.b);
    let loop_check = assembler.id(ns.b);
    let initialized = assembler.id(ns.b);
    let depth_map_ready = assembler.id(ns.b);

    let base_types = vec![
        bytes_map_type.clone(),
        depths_map_type.clone(),
        u64_type(),
        u64_type(),
        u64_type(),
    ];
    let append_list = build_type_expr_append_child(
        assembler,
        ns,
        function,
        resource_error,
        invariant_trap,
        list_advance,
        &[bytes_map_type.clone(), u64_type()],
    );
    let append_two = build_type_expr_append_child(
        assembler,
        ns,
        function,
        resource_error,
        invariant_trap,
        list_init,
        std::slice::from_ref(&bytes_map_type),
    );
    let append_one = build_type_expr_append_child(
        assembler,
        ns,
        function,
        resource_error,
        invariant_trap,
        direct_two,
        &[option_bytes_type.clone(), bytes_map_type.clone()],
    );

    let ok = assembler.op(
        ns.o,
        success,
        Opcode::ResultOk,
        vec![pav(body)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        success,
        function,
        Vec::new(),
        vec![ok],
        ret(op_result(ok)),
    );

    let advance_parameters = block_parameters(assembler, ns.p, advance_node, &base_types);
    let one_constant = assembler.ku64(ns.k, 1);
    let one = assembler.cref(ns.o, advance_node, one_constant, u64_type());
    let next = assembler.op(
        ns.o,
        advance_node,
        Opcode::IntAddChecked,
        vec![pav(advance_parameters[2]), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        advance_node,
        function,
        advance_parameters.clone(),
        vec![one, next],
        switch(
            op_result(next),
            vec![
                (
                    BuiltinCase::Ok,
                    loop_check,
                    vec![
                        sav(advance_parameters[0]),
                        sav(advance_parameters[1]),
                        SwitchArgument::CasePayload,
                        sav(advance_parameters[3]),
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let mut list_state_types = base_types.clone();
    list_state_types.extend([bytes_map_type.clone(), u64_type()]);
    let list_advance_parameters =
        block_parameters(assembler, ns.p, list_advance, &list_state_types);
    let one_constant = assembler.ku64(ns.k, 1);
    let one = assembler.cref(ns.o, list_advance, one_constant, u64_type());
    let next = assembler.op(
        ns.o,
        list_advance,
        Opcode::IntAddChecked,
        vec![pav(list_advance_parameters[6]), op_result(one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let mut next_arguments = list_advance_parameters[..6]
        .iter()
        .copied()
        .map(sav)
        .collect::<Vec<_>>();
    next_arguments.push(SwitchArgument::CasePayload);
    append_block(
        assembler,
        list_advance,
        function,
        list_advance_parameters,
        vec![one, next],
        switch(
            op_result(next),
            vec![
                (BuiltinCase::Ok, list_loop, next_arguments),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let list_loop_parameters = block_parameters(assembler, ns.p, list_loop, &list_state_types);
    let child = assembler.op(
        ns.o,
        list_loop,
        Opcode::MapGet,
        vec![pav(list_loop_parameters[5]), pav(list_loop_parameters[6])],
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    let mut append_list_arguments = vec![SwitchArgument::CasePayload];
    append_list_arguments.extend(list_loop_parameters[..5].iter().copied().map(sav));
    append_list_arguments.extend(list_loop_parameters[5..].iter().copied().map(sav));
    append_block(
        assembler,
        list_loop,
        function,
        list_loop_parameters.clone(),
        vec![child],
        switch(
            op_result(child),
            vec![
                (
                    BuiltinCase::None,
                    advance_node,
                    list_loop_parameters[..5].iter().copied().map(sav).collect(),
                ),
                (BuiltinCase::Some, append_list, append_list_arguments),
            ],
        ),
    );

    let mut list_init_types = base_types.clone();
    list_init_types.push(bytes_map_type.clone());
    let list_init_parameters = block_parameters(assembler, ns.p, list_init, &list_init_types);
    let zero_constant = assembler.ku64(ns.k, 0);
    let zero = assembler.cref(ns.o, list_init, zero_constant, u64_type());
    let mut list_arguments = list_init_parameters
        .iter()
        .copied()
        .map(pav)
        .collect::<Vec<_>>();
    list_arguments.push(op_result(zero));
    append_block(
        assembler,
        list_init,
        function,
        list_init_parameters,
        vec![zero],
        branch(edge(list_loop, list_arguments)),
    );

    let mut direct_two_types = base_types.clone();
    direct_two_types.extend([option_bytes_type.clone(), bytes_map_type.clone()]);
    let direct_two_parameters = block_parameters(assembler, ns.p, direct_two, &direct_two_types);
    let mut append_two_arguments = vec![SwitchArgument::CasePayload];
    append_two_arguments.extend(direct_two_parameters[..5].iter().copied().map(sav));
    append_two_arguments.push(sav(direct_two_parameters[6]));
    append_block(
        assembler,
        direct_two,
        function,
        direct_two_parameters.clone(),
        Vec::new(),
        switch(
            pav(direct_two_parameters[5]),
            vec![
                (
                    BuiltinCase::None,
                    list_init,
                    direct_two_parameters[..5]
                        .iter()
                        .copied()
                        .map(sav)
                        .chain(std::iter::once(sav(direct_two_parameters[6])))
                        .collect(),
                ),
                (BuiltinCase::Some, append_two, append_two_arguments),
            ],
        ),
    );

    let mut direct_one_types = base_types.clone();
    direct_one_types.extend([
        option_bytes_type.clone(),
        option_bytes_type.clone(),
        bytes_map_type.clone(),
    ]);
    let direct_one_parameters = block_parameters(assembler, ns.p, direct_one, &direct_one_types);
    let mut append_one_arguments = vec![SwitchArgument::CasePayload];
    append_one_arguments.extend(direct_one_parameters[..5].iter().copied().map(sav));
    append_one_arguments.extend(direct_one_parameters[6..].iter().copied().map(sav));
    append_block(
        assembler,
        direct_one,
        function,
        direct_one_parameters.clone(),
        Vec::new(),
        switch(
            pav(direct_one_parameters[5]),
            vec![
                (
                    BuiltinCase::None,
                    direct_two,
                    direct_one_parameters[..5]
                        .iter()
                        .copied()
                        .map(sav)
                        .chain(direct_one_parameters[6..].iter().copied().map(sav))
                        .collect(),
                ),
                (BuiltinCase::Some, append_one, append_one_arguments),
            ],
        ),
    );

    let mut children_ready_types = base_types.clone();
    children_ready_types.push(children_tuple_type.clone());
    let children_ready_parameters =
        block_parameters(assembler, ns.p, children_ready, &children_ready_types);
    let first = assembler.op(
        ns.o,
        children_ready,
        Opcode::TupleGet,
        vec![pav(children_ready_parameters[5])],
        vec![option_bytes_type.clone()],
        Immediate::Index(0),
    );
    let second = assembler.op(
        ns.o,
        children_ready,
        Opcode::TupleGet,
        vec![pav(children_ready_parameters[5])],
        vec![option_bytes_type.clone()],
        Immediate::Index(1),
    );
    let listed = assembler.op(
        ns.o,
        children_ready,
        Opcode::TupleGet,
        vec![pav(children_ready_parameters[5])],
        vec![bytes_map_type.clone()],
        Immediate::Index(2),
    );
    let mut direct_arguments = children_ready_parameters[..5]
        .iter()
        .copied()
        .map(pav)
        .collect::<Vec<_>>();
    direct_arguments.extend([op_result(first), op_result(second), op_result(listed)]);
    append_block(
        assembler,
        children_ready,
        function,
        children_ready_parameters,
        vec![first, second, listed],
        branch(edge(direct_one, direct_arguments)),
    );

    let mut children_call_types = base_types.clone();
    children_call_types.insert(0, TypeExpr::Bytes);
    let children_call_parameters =
        block_parameters(assembler, ns.p, children_call, &children_call_types);
    let decoded = assembler.op(
        ns.o,
        children_call,
        Opcode::CallDirect,
        vec![pav(children_call_parameters[0]), pav(unit)],
        vec![type_expr_children_result_type()],
        Immediate::Function(FunctionRefValue {
            function: children_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        children_call,
        function,
        children_call_parameters.clone(),
        vec![decoded],
        switch(
            op_result(decoded),
            vec![
                (
                    BuiltinCase::Ok,
                    children_ready,
                    children_call_parameters[1..]
                        .iter()
                        .copied()
                        .map(sav)
                        .chain(std::iter::once(SwitchArgument::CasePayload))
                        .collect(),
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let depth_lookup_types = vec![
        TypeExpr::Bytes,
        bytes_map_type.clone(),
        depths_map_type.clone(),
        u64_type(),
        u64_type(),
    ];
    let depth_lookup_parameters =
        block_parameters(assembler, ns.p, depth_lookup, &depth_lookup_types);
    let depth = assembler.op(
        ns.o,
        depth_lookup,
        Opcode::MapGet,
        vec![
            pav(depth_lookup_parameters[2]),
            pav(depth_lookup_parameters[3]),
        ],
        vec![option_u64_type],
        Immediate::None,
    );
    append_block(
        assembler,
        depth_lookup,
        function,
        depth_lookup_parameters.clone(),
        vec![depth],
        switch(
            op_result(depth),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    children_call,
                    vec![
                        sav(depth_lookup_parameters[0]),
                        sav(depth_lookup_parameters[1]),
                        sav(depth_lookup_parameters[2]),
                        sav(depth_lookup_parameters[3]),
                        sav(depth_lookup_parameters[4]),
                        SwitchArgument::CasePayload,
                    ],
                ),
            ],
        ),
    );

    let loop_types = vec![
        bytes_map_type.clone(),
        depths_map_type.clone(),
        u64_type(),
        u64_type(),
    ];
    let work_lookup_parameters = block_parameters(assembler, ns.p, work_lookup, &loop_types);
    let current = assembler.op(
        ns.o,
        work_lookup,
        Opcode::MapGet,
        vec![
            pav(work_lookup_parameters[0]),
            pav(work_lookup_parameters[2]),
        ],
        vec![option_bytes_type],
        Immediate::None,
    );
    append_block(
        assembler,
        work_lookup,
        function,
        work_lookup_parameters.clone(),
        vec![current],
        switch(
            op_result(current),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    depth_lookup,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(work_lookup_parameters[0]),
                        sav(work_lookup_parameters[1]),
                        sav(work_lookup_parameters[2]),
                        sav(work_lookup_parameters[3]),
                    ],
                ),
            ],
        ),
    );

    let loop_parameters = block_parameters(assembler, ns.p, loop_check, &loop_types);
    let complete = assembler.op(
        ns.o,
        loop_check,
        Opcode::Equal,
        vec![pav(loop_parameters[2]), pav(loop_parameters[3])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        loop_check,
        function,
        loop_parameters.clone(),
        vec![complete],
        cond(
            op_result(complete),
            edge(success, Vec::new()),
            edge(
                work_lookup,
                loop_parameters.iter().copied().map(pav).collect(),
            ),
        ),
    );

    let initialized_parameters = block_parameters(
        assembler,
        ns.p,
        initialized,
        &[bytes_map_type.clone(), depths_map_type.clone()],
    );
    let zero_constant = assembler.ku64(ns.k, 0);
    let zero = assembler.cref(ns.o, initialized, zero_constant, u64_type());
    let work = assembler.op(
        ns.o,
        initialized,
        Opcode::MapInsert,
        vec![pav(initialized_parameters[0]), op_result(zero), pav(body)],
        vec![bytes_map_type.clone()],
        Immediate::None,
    );
    let depths = assembler.op(
        ns.o,
        initialized,
        Opcode::MapInsert,
        vec![
            pav(initialized_parameters[1]),
            op_result(zero),
            op_result(zero),
        ],
        vec![depths_map_type.clone()],
        Immediate::None,
    );
    let one_constant = assembler.ku64(ns.k, 1);
    let one = assembler.cref(ns.o, initialized, one_constant, u64_type());
    append_block(
        assembler,
        initialized,
        function,
        initialized_parameters,
        vec![zero, work, depths, one],
        branch(edge(
            loop_check,
            vec![
                op_result(work),
                op_result(depths),
                op_result(zero),
                op_result(one),
            ],
        )),
    );

    let depth_map_parameters = block_parameters(
        assembler,
        ns.p,
        depth_map_ready,
        std::slice::from_ref(&bytes_map_type),
    );
    let depth_map_new_result = TypeExpr::Result {
        ok: Box::new(depths_map_type),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let depths = assembler.op(
        ns.o,
        depth_map_ready,
        Opcode::MapNew,
        Vec::new(),
        vec![depth_map_new_result],
        Immediate::None,
    );
    append_block(
        assembler,
        depth_map_ready,
        function,
        depth_map_parameters.clone(),
        vec![depths],
        switch(
            op_result(depths),
            vec![
                (
                    BuiltinCase::Ok,
                    initialized,
                    vec![sav(depth_map_parameters[0]), SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let entry = assembler.id(ns.b);
    let work_map_new_result = TypeExpr::Result {
        ok: Box::new(bytes_map_type),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let work = assembler.op(
        ns.o,
        entry,
        Opcode::MapNew,
        Vec::new(),
        vec![work_map_new_result],
        Immediate::None,
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![work],
        switch(
            op_result(work),
            vec![
                (
                    BuiltinCase::Ok,
                    depth_map_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
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

#[allow(clippy::similar_names, clippy::too_many_lines)]
pub(super) fn type_expr_recursive_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(245);
    let record_function = assembler.id(245);
    let union_function = assembler.id(245);
    let list_function = assembler.id(245);
    let fixed32_function = assembler.id(245);
    let entity_id_collection_function = assembler.id(245);
    let exact_uvar_function = assembler.id(245);
    let bounded_uvar_function = assembler.id(245);
    let leaf_function = assembler.id(245);
    let record2_function = assembler.id(245);
    let record3_function = assembler.id(245);
    let children_function = assembler.id(245);
    let function = assembler.id(245);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 246,
            p: 246,
            b: 246,
            o: 246,
        },
        decode_function,
    );
    let record_graph = build_generic_record_decode(
        &mut assembler,
        Ns {
            k: 247,
            p: 247,
            b: 247,
            o: 247,
        },
        record_function,
        decode_function,
    );
    let union_graph = build_generic_union_decode(
        &mut assembler,
        Ns {
            k: 248,
            p: 248,
            b: 248,
            o: 248,
        },
        union_function,
        decode_function,
    );
    let list_graph = build_generic_list_decode(
        &mut assembler,
        Ns {
            k: 249,
            p: 249,
            b: 249,
            o: 249,
        },
        list_function,
        decode_function,
    );
    let fixed32_graph = build_fixed32_decode(
        &mut assembler,
        Ns {
            k: 250,
            p: 250,
            b: 250,
            o: 250,
        },
        fixed32_function,
    );
    let entity_id_collection_graph = build_entity_id_collection_decode(
        &mut assembler,
        Ns {
            k: 251,
            p: 251,
            b: 251,
            o: 251,
        },
        entity_id_collection_function,
        list_function,
        fixed32_function,
    );
    let exact_uvar_graph = build_exact_uvar_decode(
        &mut assembler,
        Ns {
            k: 252,
            p: 252,
            b: 252,
            o: 252,
        },
        exact_uvar_function,
        decode_function,
    );
    let bounded_uvar_graph = build_bounded_uvar_decode(
        &mut assembler,
        Ns {
            k: 253,
            p: 253,
            b: 253,
            o: 253,
        },
        bounded_uvar_function,
        exact_uvar_function,
    );
    let leaf_graph = build_type_expr_leaf_decode(
        &mut assembler,
        Ns {
            k: 254,
            p: 254,
            b: 254,
            o: 254,
        },
        leaf_function,
        union_function,
        fixed32_function,
        exact_uvar_function,
        bounded_uvar_function,
    );
    let record2_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 255,
            p: 255,
            b: 255,
            o: 255,
        },
        record2_function,
        decode_function,
        record_function,
        2,
    );
    let record3_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 200,
            p: 200,
            b: 200,
            o: 200,
        },
        record3_function,
        decode_function,
        record_function,
        3,
    );
    let children_graph = build_type_expr_children_decode(
        &mut assembler,
        Ns {
            k: 201,
            p: 201,
            b: 201,
            o: 201,
        },
        children_function,
        union_function,
        leaf_function,
        list_function,
        record2_function,
        record3_function,
        fixed32_function,
        entity_id_collection_function,
    );
    let graph = build_type_expr_recursive_decode(
        &mut assembler,
        Ns {
            k: 202,
            p: 202,
            b: 202,
            o: 202,
        },
        function,
        children_function,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            children_graph,
            record3_graph,
            record2_graph,
            leaf_graph,
            bounded_uvar_graph,
            exact_uvar_graph,
            entity_id_collection_graph,
            fixed32_graph,
            list_graph,
            union_graph,
            record_graph,
            decode_graph,
        ],
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

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_type_expr_leaf_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    union_decoder: EntityId,
    fixed32_decoder: EntityId,
    exact_uvar_decoder: EntityId,
    bounded_uvar_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = bytes_validation_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let union_code = assembler.kbytes(ns.k, b"SCB_UNION_INVALID");
    let scope_code = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");

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
    let union_error = err_block(assembler, ns, function, result_type.clone(), union_code);
    let scope_error = err_block(assembler, ns, function, result_type.clone(), scope_code);
    let success = assembler.id(ns.b);
    let failure_kind = assembler.id(ns.b);
    let width32 = assembler.id(ns.b);
    let fixed32 = assembler.id(ns.b);
    let width16 = assembler.id(ns.b);
    let empty = assembler.id(ns.b);
    let dispatch = std::array::from_fn::<_, 20, _>(|_| assembler.id(ns.b));
    let union_ready = assembler.id(ns.b);

    let ok = assembler.op(
        ns.o,
        success,
        Opcode::ResultOk,
        vec![pav(body)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        success,
        function,
        Vec::new(),
        vec![ok],
        ret(op_result(ok)),
    );

    let failure_parameters = block_parameters(assembler, ns.p, failure_kind, &[TypeExpr::Bytes]);
    let minimum_constant = assembler.ku64(ns.k, 1);
    let minimum = assembler.cref(ns.o, failure_kind, minimum_constant, u64_type());
    let maximum_constant = assembler.ku64(ns.k, 5);
    let maximum = assembler.cref(ns.o, failure_kind, maximum_constant, u64_type());
    let failure = assembler.op(
        ns.o,
        failure_kind,
        Opcode::CallDirect,
        vec![
            pav(failure_parameters[0]),
            op_result(minimum),
            op_result(maximum),
            pav(unit),
        ],
        vec![exact_uvar_result_type()],
        Immediate::Function(FunctionRefValue {
            function: bounded_uvar_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        failure_kind,
        function,
        failure_parameters,
        vec![minimum, maximum, failure],
        switch(
            op_result(failure),
            vec![
                (BuiltinCase::Ok, success, Vec::new()),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    for (block, width_bits) in [(width16, 16_u128), (width32, 32_u128)] {
        let parameters = block_parameters(assembler, ns.p, block, &[TypeExpr::Bytes]);
        let width_constant = assembler.ku32(ns.k, width_bits);
        let width = assembler.cref(ns.o, block, width_constant, u32_type());
        let decoded = assembler.op(
            ns.o,
            block,
            Opcode::CallDirect,
            vec![pav(parameters[0]), op_result(width), pav(unit)],
            vec![exact_uvar_result_type()],
            Immediate::Function(FunctionRefValue {
                function: exact_uvar_decoder,
                type_arguments: Vec::new(),
            }),
        );
        append_block(
            assembler,
            block,
            function,
            parameters,
            vec![width, decoded],
            switch(
                op_result(decoded),
                vec![
                    (BuiltinCase::Ok, success, Vec::new()),
                    (
                        BuiltinCase::Err,
                        forward_error,
                        vec![SwitchArgument::CasePayload],
                    ),
                ],
            ),
        );
    }

    let fixed_parameters = block_parameters(assembler, ns.p, fixed32, &[TypeExpr::Bytes]);
    let fixed = assembler.op(
        ns.o,
        fixed32,
        Opcode::CallDirect,
        vec![pav(fixed_parameters[0]), pav(unit)],
        vec![bytes_validation_result_type()],
        Immediate::Function(FunctionRefValue {
            function: fixed32_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        fixed32,
        function,
        fixed_parameters,
        vec![fixed],
        switch(
            op_result(fixed),
            vec![
                (BuiltinCase::Ok, success, Vec::new()),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let empty_parameters = block_parameters(assembler, ns.p, empty, &[TypeExpr::Bytes]);
    let empty_constant = assembler.kbytes(ns.k, b"");
    let expected_empty = assembler.cref(ns.o, empty, empty_constant, TypeExpr::Bytes);
    let is_empty = assembler.op(
        ns.o,
        empty,
        Opcode::Equal,
        vec![pav(empty_parameters[0]), op_result(expected_empty)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        empty,
        function,
        empty_parameters,
        vec![expected_empty, is_empty],
        cond(
            op_result(is_empty),
            edge(success, Vec::new()),
            edge(union_error, Vec::new()),
        ),
    );

    for (index, block) in dispatch.iter().copied().enumerate() {
        let parameters = block_parameters(assembler, ns.p, block, &[u64_type(), TypeExpr::Bytes]);
        let tag = u64::try_from(index + 1).expect("TypeExpr tag fits u64");
        let tag_constant = assembler.ku64(ns.k, u128::from(tag));
        let expected = assembler.cref(ns.o, block, tag_constant, u64_type());
        let matches = assembler.op(
            ns.o,
            block,
            Opcode::Equal,
            vec![pav(parameters[0]), op_result(expected)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        let target = match tag {
            1 | 2 | 5 | 6 | 7 | 8 => Some(empty),
            3 | 4 => Some(width16),
            9..=15 | 18 => None,
            16 | 17 => Some(fixed32),
            19 => Some(width32),
            20 => Some(failure_kind),
            _ => unreachable!(),
        };
        let true_edge = target.map_or_else(
            || edge(scope_error, Vec::new()),
            |target| edge(target, vec![pav(parameters[1])]),
        );
        let false_edge = if index + 1 == dispatch.len() {
            edge(union_error, Vec::new())
        } else {
            edge(
                dispatch[index + 1],
                vec![pav(parameters[0]), pav(parameters[1])],
            )
        };
        append_block(
            assembler,
            block,
            function,
            parameters,
            vec![expected, matches],
            cond(op_result(matches), true_edge, false_edge),
        );
    }

    let union_parameters = block_parameters(
        assembler,
        ns.p,
        union_ready,
        &[TypeExpr::Tuple(vec![u64_type(), TypeExpr::Bytes])],
    );
    let tag = assembler.op(
        ns.o,
        union_ready,
        Opcode::TupleGet,
        vec![pav(union_parameters[0])],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let payload = assembler.op(
        ns.o,
        union_ready,
        Opcode::TupleGet,
        vec![pav(union_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    append_block(
        assembler,
        union_ready,
        function,
        union_parameters,
        vec![tag, payload],
        branch(edge(dispatch[0], vec![op_result(tag), op_result(payload)])),
    );

    let entry = assembler.id(ns.b);
    let decoded = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![generic_union_result_type()],
        Immediate::Function(FunctionRefValue {
            function: union_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![decoded],
        switch(
            op_result(decoded),
            vec![
                (
                    BuiltinCase::Ok,
                    union_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
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

#[allow(clippy::too_many_lines)]
pub(super) fn type_expr_leaf_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(219);
    let union_function = assembler.id(219);
    let fixed32_function = assembler.id(219);
    let exact_uvar_function = assembler.id(219);
    let bounded_uvar_function = assembler.id(219);
    let function = assembler.id(219);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 220,
            p: 220,
            b: 220,
            o: 220,
        },
        decode_function,
    );
    let union_graph = build_generic_union_decode(
        &mut assembler,
        Ns {
            k: 221,
            p: 221,
            b: 221,
            o: 221,
        },
        union_function,
        decode_function,
    );
    let fixed32_graph = build_fixed32_decode(
        &mut assembler,
        Ns {
            k: 222,
            p: 222,
            b: 222,
            o: 222,
        },
        fixed32_function,
    );
    let exact_uvar_graph = build_exact_uvar_decode(
        &mut assembler,
        Ns {
            k: 223,
            p: 223,
            b: 223,
            o: 223,
        },
        exact_uvar_function,
        decode_function,
    );
    let bounded_uvar_graph = build_bounded_uvar_decode(
        &mut assembler,
        Ns {
            k: 224,
            p: 224,
            b: 224,
            o: 224,
        },
        bounded_uvar_function,
        exact_uvar_function,
    );
    let graph = build_type_expr_leaf_decode(
        &mut assembler,
        Ns {
            k: 225,
            p: 225,
            b: 225,
            o: 225,
        },
        function,
        union_function,
        fixed32_function,
        exact_uvar_function,
        bounded_uvar_function,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            bounded_uvar_graph,
            exact_uvar_graph,
            fixed32_graph,
            union_graph,
            decode_graph,
        ],
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

fn function_schema_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![TypeExpr::Bytes; 8])),
        error: Box::new(TypeExpr::Bytes),
    }
}

fn parameter_schema_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![TypeExpr::Bytes; 4])),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_parameter_schema_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    union_decoder: EntityId,
    record4_decoder: EntityId,
    fixed32_decoder: EntityId,
    bounded_uvar_decoder: EntityId,
    exact_uvar_decoder: EntityId,
    type_expr_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = parameter_schema_result_type();
    let tuple_type = TypeExpr::Tuple(vec![TypeExpr::Bytes; 4]);
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let union_code = assembler.kbytes(ns.k, b"SCB_UNION_INVALID");

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
    let union_error = err_block(assembler, ns, function, result_type.clone(), union_code);

    let success = assembler.id(ns.b);
    let validate_type = assembler.id(ns.b);
    let validate_ordinal = assembler.id(ns.b);
    let validate_role = assembler.id(ns.b);
    let validate_owner = assembler.id(ns.b);
    let record_ready = assembler.id(ns.b);
    let record_call = assembler.id(ns.b);
    let union_ready = assembler.id(ns.b);

    let success_parameters = block_parameters(assembler, ns.p, success, &vec![TypeExpr::Bytes; 4]);
    let tuple = assembler.op(
        ns.o,
        success,
        Opcode::TupleNew,
        success_parameters.iter().copied().map(pav).collect(),
        vec![tuple_type.clone()],
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
        function,
        success_parameters,
        vec![tuple, ok],
        ret(op_result(ok)),
    );

    let type_parameters =
        block_parameters(assembler, ns.p, validate_type, &vec![TypeExpr::Bytes; 4]);
    let decoded_type = assembler.op(
        ns.o,
        validate_type,
        Opcode::CallDirect,
        vec![pav(type_parameters[3]), pav(unit)],
        vec![bytes_validation_result_type()],
        Immediate::Function(FunctionRefValue {
            function: type_expr_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        validate_type,
        function,
        type_parameters.clone(),
        vec![decoded_type],
        switch(
            op_result(decoded_type),
            vec![
                (
                    BuiltinCase::Ok,
                    success,
                    type_parameters.iter().copied().map(sav).collect(),
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let ordinal_parameters =
        block_parameters(assembler, ns.p, validate_ordinal, &vec![TypeExpr::Bytes; 4]);
    let width32_constant = assembler.ku32(ns.k, 32);
    let width32 = assembler.cref(ns.o, validate_ordinal, width32_constant, u32_type());
    let decoded_ordinal = assembler.op(
        ns.o,
        validate_ordinal,
        Opcode::CallDirect,
        vec![pav(ordinal_parameters[2]), op_result(width32), pav(unit)],
        vec![exact_uvar_result_type()],
        Immediate::Function(FunctionRefValue {
            function: exact_uvar_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        validate_ordinal,
        function,
        ordinal_parameters.clone(),
        vec![width32, decoded_ordinal],
        switch(
            op_result(decoded_ordinal),
            vec![
                (
                    BuiltinCase::Ok,
                    validate_type,
                    ordinal_parameters.iter().copied().map(sav).collect(),
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let role_parameters =
        block_parameters(assembler, ns.p, validate_role, &vec![TypeExpr::Bytes; 4]);
    let role_minimum_constant = assembler.ku64(ns.k, 1);
    let role_maximum_constant = assembler.ku64(ns.k, 2);
    let role_minimum = assembler.cref(ns.o, validate_role, role_minimum_constant, u64_type());
    let role_maximum = assembler.cref(ns.o, validate_role, role_maximum_constant, u64_type());
    let decoded_role = assembler.op(
        ns.o,
        validate_role,
        Opcode::CallDirect,
        vec![
            pav(role_parameters[1]),
            op_result(role_minimum),
            op_result(role_maximum),
            pav(unit),
        ],
        vec![exact_uvar_result_type()],
        Immediate::Function(FunctionRefValue {
            function: bounded_uvar_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        validate_role,
        function,
        role_parameters.clone(),
        vec![role_minimum, role_maximum, decoded_role],
        switch(
            op_result(decoded_role),
            vec![
                (
                    BuiltinCase::Ok,
                    validate_ordinal,
                    role_parameters.iter().copied().map(sav).collect(),
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let owner_parameters =
        block_parameters(assembler, ns.p, validate_owner, &vec![TypeExpr::Bytes; 4]);
    let decoded_owner = assembler.op(
        ns.o,
        validate_owner,
        Opcode::CallDirect,
        vec![pav(owner_parameters[0]), pav(unit)],
        vec![bytes_validation_result_type()],
        Immediate::Function(FunctionRefValue {
            function: fixed32_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        validate_owner,
        function,
        owner_parameters.clone(),
        vec![decoded_owner],
        switch(
            op_result(decoded_owner),
            vec![
                (
                    BuiltinCase::Ok,
                    validate_role,
                    owner_parameters.iter().copied().map(sav).collect(),
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let record_parameters = block_parameters(
        assembler,
        ns.p,
        record_ready,
        std::slice::from_ref(&tuple_type),
    );
    let mut projected_fields = Vec::new();
    for index in 0..4 {
        projected_fields.push(assembler.op(
            ns.o,
            record_ready,
            Opcode::TupleGet,
            vec![pav(record_parameters[0])],
            vec![TypeExpr::Bytes],
            Immediate::Index(index),
        ));
    }
    append_block(
        assembler,
        record_ready,
        function,
        record_parameters,
        projected_fields.clone(),
        branch(edge(
            validate_owner,
            projected_fields.iter().copied().map(op_result).collect(),
        )),
    );

    let record_call_parameters = block_parameters(assembler, ns.p, record_call, &[TypeExpr::Bytes]);
    let decoded_record = assembler.op(
        ns.o,
        record_call,
        Opcode::CallDirect,
        vec![pav(record_call_parameters[0]), pav(unit)],
        vec![exact_record_projection_result_type(4)],
        Immediate::Function(FunctionRefValue {
            function: record4_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        record_call,
        function,
        record_call_parameters,
        vec![decoded_record],
        switch(
            op_result(decoded_record),
            vec![
                (
                    BuiltinCase::Ok,
                    record_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let union_parameters = block_parameters(
        assembler,
        ns.p,
        union_ready,
        &[TypeExpr::Tuple(vec![u64_type(), TypeExpr::Bytes])],
    );
    let tag = assembler.op(
        ns.o,
        union_ready,
        Opcode::TupleGet,
        vec![pav(union_parameters[0])],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let payload = assembler.op(
        ns.o,
        union_ready,
        Opcode::TupleGet,
        vec![pav(union_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let parameter_tag_constant = assembler.ku64(ns.k, 6);
    let expected_tag = assembler.cref(ns.o, union_ready, parameter_tag_constant, u64_type());
    let matches = assembler.op(
        ns.o,
        union_ready,
        Opcode::Equal,
        vec![op_result(tag), op_result(expected_tag)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        union_ready,
        function,
        union_parameters,
        vec![tag, payload, expected_tag, matches],
        cond(
            op_result(matches),
            edge(record_call, vec![op_result(payload)]),
            edge(union_error, Vec::new()),
        ),
    );

    let entry = assembler.id(ns.b);
    let decoded_union = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![generic_union_result_type()],
        Immediate::Function(FunctionRefValue {
            function: union_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![decoded_union],
        switch(
            op_result(decoded_union),
            vec![
                (
                    BuiltinCase::Ok,
                    union_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
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

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_function_schema_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    union_decoder: EntityId,
    record_decoder: EntityId,
    entity_id_collection_decoder: EntityId,
    type_parameter_list_decoder: EntityId,
    fixed32_decoder: EntityId,
    bounded_uvar_decoder: EntityId,
    type_expr_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = function_schema_result_type();
    let tuple_type = TypeExpr::Tuple(vec![TypeExpr::Bytes; 8]);
    let map_type = generic_record_map_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let union_code = assembler.kbytes(ns.k, b"SCB_UNION_INVALID");
    let missing_code = assembler.kbytes(ns.k, b"SCB_FIELD_MISSING");
    let unknown_code = assembler.kbytes(ns.k, b"SCB_FIELD_UNKNOWN");

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
    let union_error = err_block(assembler, ns, function, result_type.clone(), union_code);
    let missing_error = err_block(assembler, ns, function, result_type.clone(), missing_code);
    let unknown_error = err_block(assembler, ns, function, result_type.clone(), unknown_code);
    let invariant_trap = trap_block(assembler, ns, function);

    let success = assembler.id(ns.b);
    let finish = assembler.id(ns.b);
    let scalar_validation = assembler.id(ns.b);
    let type_expr_validation = assembler.id(ns.b);
    let fixed_validation = assembler.id(ns.b);
    let list_validation_blocks = std::array::from_fn::<_, 5, _>(|_| assembler.id(ns.b));
    let get_blocks = std::array::from_fn::<_, 8, _>(|_| assembler.id(ns.b));
    let map_ready = assembler.id(ns.b);
    let record_call = assembler.id(ns.b);
    let union_ready = assembler.id(ns.b);

    let success_parameters = block_parameters(assembler, ns.p, success, &vec![TypeExpr::Bytes; 8]);
    let tuple = assembler.op(
        ns.o,
        success,
        Opcode::TupleNew,
        success_parameters.iter().copied().map(pav).collect(),
        vec![tuple_type.clone()],
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
        function,
        success_parameters,
        vec![tuple, ok],
        ret(op_result(ok)),
    );

    let mut finish_types = vec![map_type.clone(), map_type.clone()];
    finish_types.extend(vec![TypeExpr::Bytes; 8]);
    let finish_parameters = block_parameters(assembler, ns.p, finish, &finish_types);
    let no_unknown_fields = assembler.op(
        ns.o,
        finish,
        Opcode::Equal,
        vec![pav(finish_parameters[0]), pav(finish_parameters[1])],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        finish,
        function,
        finish_parameters.clone(),
        vec![no_unknown_fields],
        cond(
            op_result(no_unknown_fields),
            edge(
                list_validation_blocks[0],
                finish_parameters[2..].iter().copied().map(pav).collect(),
            ),
            edge(unknown_error, Vec::new()),
        ),
    );

    for (index, (block, (field_index, ordered_identities))) in list_validation_blocks
        .iter()
        .copied()
        .zip([
            (0_usize, None),
            (1, Some(false)),
            (3, Some(true)),
            (5, Some(false)),
            (6, Some(true)),
        ])
        .enumerate()
    {
        let parameters = block_parameters(assembler, ns.p, block, &vec![TypeExpr::Bytes; 8]);
        let mut operations = Vec::new();
        let (decode_function, decode_arguments, decode_result) =
            if let Some(ordered_identities) = ordered_identities {
                let ordered_constant = assembler.kbool(ns.k, ordered_identities);
                let ordered_value = assembler.cref(ns.o, block, ordered_constant, TypeExpr::Bool);
                operations.push(ordered_value);
                (
                    entity_id_collection_decoder,
                    vec![
                        pav(parameters[field_index]),
                        op_result(ordered_value),
                        pav(unit),
                    ],
                    unit_validation_result_type(),
                )
            } else {
                (
                    type_parameter_list_decoder,
                    vec![pav(parameters[field_index]), pav(unit)],
                    unit_validation_result_type(),
                )
            };
        let decoded = assembler.op(
            ns.o,
            block,
            Opcode::CallDirect,
            decode_arguments,
            vec![decode_result],
            Immediate::Function(FunctionRefValue {
                function: decode_function,
                type_arguments: Vec::new(),
            }),
        );
        let destination = if index + 1 == list_validation_blocks.len() {
            fixed_validation
        } else {
            list_validation_blocks[index + 1]
        };
        operations.push(decoded);
        append_block(
            assembler,
            block,
            function,
            parameters.clone(),
            operations,
            switch(
                op_result(decoded),
                vec![
                    (
                        BuiltinCase::Ok,
                        destination,
                        parameters.iter().copied().map(sav).collect(),
                    ),
                    (
                        BuiltinCase::Err,
                        forward_error,
                        vec![SwitchArgument::CasePayload],
                    ),
                ],
            ),
        );
    }

    let fixed_validation_parameters =
        block_parameters(assembler, ns.p, fixed_validation, &vec![TypeExpr::Bytes; 8]);
    let fixed = assembler.op(
        ns.o,
        fixed_validation,
        Opcode::CallDirect,
        vec![pav(fixed_validation_parameters[4]), pav(unit)],
        vec![bytes_validation_result_type()],
        Immediate::Function(FunctionRefValue {
            function: fixed32_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        fixed_validation,
        function,
        fixed_validation_parameters.clone(),
        vec![fixed],
        switch(
            op_result(fixed),
            vec![
                (
                    BuiltinCase::Ok,
                    scalar_validation,
                    fixed_validation_parameters
                        .iter()
                        .copied()
                        .map(sav)
                        .collect(),
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let scalar_validation_parameters = block_parameters(
        assembler,
        ns.p,
        scalar_validation,
        &vec![TypeExpr::Bytes; 8],
    );
    let minimum_constant = assembler.ku64(ns.k, 1);
    let minimum = assembler.cref(ns.o, scalar_validation, minimum_constant, u64_type());
    let maximum_constant = assembler.ku64(ns.k, 4);
    let maximum = assembler.cref(ns.o, scalar_validation, maximum_constant, u64_type());
    let visibility = assembler.op(
        ns.o,
        scalar_validation,
        Opcode::CallDirect,
        vec![
            pav(scalar_validation_parameters[7]),
            op_result(minimum),
            op_result(maximum),
            pav(unit),
        ],
        vec![exact_uvar_result_type()],
        Immediate::Function(FunctionRefValue {
            function: bounded_uvar_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        scalar_validation,
        function,
        scalar_validation_parameters.clone(),
        vec![minimum, maximum, visibility],
        switch(
            op_result(visibility),
            vec![
                (
                    BuiltinCase::Ok,
                    type_expr_validation,
                    scalar_validation_parameters
                        .iter()
                        .copied()
                        .map(sav)
                        .collect(),
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let type_expr_validation_parameters = block_parameters(
        assembler,
        ns.p,
        type_expr_validation,
        &vec![TypeExpr::Bytes; 8],
    );
    let result_type_expr = assembler.op(
        ns.o,
        type_expr_validation,
        Opcode::CallDirect,
        vec![pav(type_expr_validation_parameters[2]), pav(unit)],
        vec![bytes_validation_result_type()],
        Immediate::Function(FunctionRefValue {
            function: type_expr_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        type_expr_validation,
        function,
        type_expr_validation_parameters.clone(),
        vec![result_type_expr],
        switch(
            op_result(result_type_expr),
            vec![
                (
                    BuiltinCase::Ok,
                    success,
                    type_expr_validation_parameters
                        .iter()
                        .copied()
                        .map(sav)
                        .collect(),
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    for (index, block) in get_blocks.iter().copied().enumerate() {
        let mut value_types = vec![map_type.clone(), map_type.clone()];
        value_types.extend(vec![TypeExpr::Bytes; index]);
        let parameters = block_parameters(assembler, ns.p, block, &value_types);
        let field_tag = assembler.ku64(ns.k, (index + 1) as u128);
        let field_tag_value = assembler.cref(ns.o, block, field_tag, u64_type());
        let found = assembler.op(
            ns.o,
            block,
            Opcode::MapGet,
            vec![pav(parameters[0]), op_result(field_tag_value)],
            vec![TypeExpr::Option(Box::new(TypeExpr::Bytes))],
            Immediate::None,
        );
        let remaining = assembler.op(
            ns.o,
            block,
            Opcode::MapRemove,
            vec![pav(parameters[0]), op_result(field_tag_value)],
            vec![map_type.clone()],
            Immediate::None,
        );
        let destination = if index == 7 {
            finish
        } else {
            get_blocks[index + 1]
        };
        let mut arguments = vec![oav(remaining), sav(parameters[1])];
        arguments.extend(parameters[2..].iter().copied().map(sav));
        arguments.push(SwitchArgument::CasePayload);
        append_block(
            assembler,
            block,
            function,
            parameters,
            vec![field_tag_value, found, remaining],
            switch(
                op_result(found),
                vec![
                    (BuiltinCase::None, missing_error, Vec::new()),
                    (BuiltinCase::Some, destination, arguments),
                ],
            ),
        );
    }

    let map_ready_parameters =
        block_parameters(assembler, ns.p, map_ready, std::slice::from_ref(&map_type));
    let new_map_result = TypeExpr::Result {
        ok: Box::new(map_type.clone()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let empty_map = assembler.op(
        ns.o,
        map_ready,
        Opcode::MapNew,
        Vec::new(),
        vec![new_map_result],
        Immediate::None,
    );
    append_block(
        assembler,
        map_ready,
        function,
        map_ready_parameters.clone(),
        vec![empty_map],
        switch(
            op_result(empty_map),
            vec![
                (
                    BuiltinCase::Ok,
                    get_blocks[0],
                    vec![sav(map_ready_parameters[0]), SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let record_parameters = block_parameters(assembler, ns.p, record_call, &[TypeExpr::Bytes]);
    let decoded_record = assembler.op(
        ns.o,
        record_call,
        Opcode::CallDirect,
        vec![pav(record_parameters[0]), pav(unit)],
        vec![generic_record_result_type()],
        Immediate::Function(FunctionRefValue {
            function: record_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        record_call,
        function,
        record_parameters,
        vec![decoded_record],
        switch(
            op_result(decoded_record),
            vec![
                (
                    BuiltinCase::Ok,
                    map_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let union_parameters = block_parameters(
        assembler,
        ns.p,
        union_ready,
        &[TypeExpr::Tuple(vec![u64_type(), TypeExpr::Bytes])],
    );
    let tag = assembler.op(
        ns.o,
        union_ready,
        Opcode::TupleGet,
        vec![pav(union_parameters[0])],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let payload = assembler.op(
        ns.o,
        union_ready,
        Opcode::TupleGet,
        vec![pav(union_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let function_tag = assembler.ku64(ns.k, 5);
    let expected_tag = assembler.cref(ns.o, union_ready, function_tag, u64_type());
    let matches = assembler.op(
        ns.o,
        union_ready,
        Opcode::Equal,
        vec![op_result(tag), op_result(expected_tag)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        union_ready,
        function,
        union_parameters,
        vec![tag, payload, expected_tag, matches],
        cond(
            op_result(matches),
            edge(record_call, vec![op_result(payload)]),
            edge(union_error, Vec::new()),
        ),
    );

    let entry = assembler.id(ns.b);
    let decoded_union = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![generic_union_result_type()],
        Immediate::Function(FunctionRefValue {
            function: union_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![decoded_union],
        switch(
            op_result(decoded_union),
            vec![
                (
                    BuiltinCase::Ok,
                    union_ready,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
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

#[allow(clippy::similar_names, clippy::too_many_lines)]
pub(super) fn function_schema_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(200);
    let record_function = assembler.id(200);
    let union_function = assembler.id(200);
    let list_function = assembler.id(200);
    let fixed32_function = assembler.id(200);
    let entity_id_collection_function = assembler.id(200);
    let exact_uvar_function = assembler.id(200);
    let bounded_uvar_function = assembler.id(200);
    let type_parameter_list_function = assembler.id(200);
    let type_expr_leaf_function = assembler.id(200);
    let record2_function = assembler.id(200);
    let record3_function = assembler.id(200);
    let type_expr_children_function = assembler.id(200);
    let type_expr_recursive_function = assembler.id(200);
    let function = assembler.id(200);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 201,
            p: 201,
            b: 201,
            o: 201,
        },
        decode_function,
    );
    let record_graph = build_generic_record_decode(
        &mut assembler,
        Ns {
            k: 202,
            p: 202,
            b: 202,
            o: 202,
        },
        record_function,
        decode_function,
    );
    let union_graph = build_generic_union_decode(
        &mut assembler,
        Ns {
            k: 203,
            p: 203,
            b: 203,
            o: 203,
        },
        union_function,
        decode_function,
    );
    let list_graph = build_generic_list_decode(
        &mut assembler,
        Ns {
            k: 204,
            p: 204,
            b: 204,
            o: 204,
        },
        list_function,
        decode_function,
    );
    let fixed32_graph = build_fixed32_decode(
        &mut assembler,
        Ns {
            k: 205,
            p: 205,
            b: 205,
            o: 205,
        },
        fixed32_function,
    );
    let entity_id_collection_graph = build_entity_id_collection_decode(
        &mut assembler,
        Ns {
            k: 206,
            p: 206,
            b: 206,
            o: 206,
        },
        entity_id_collection_function,
        list_function,
        fixed32_function,
    );
    let exact_uvar_graph = build_exact_uvar_decode(
        &mut assembler,
        Ns {
            k: 207,
            p: 207,
            b: 207,
            o: 207,
        },
        exact_uvar_function,
        decode_function,
    );
    let bounded_uvar_graph = build_bounded_uvar_decode(
        &mut assembler,
        Ns {
            k: 208,
            p: 208,
            b: 208,
            o: 208,
        },
        bounded_uvar_function,
        exact_uvar_function,
    );
    let type_parameter_list_graph = build_type_parameter_list_decode(
        &mut assembler,
        Ns {
            k: 209,
            p: 209,
            b: 209,
            o: 209,
        },
        type_parameter_list_function,
        list_function,
        record_function,
        exact_uvar_function,
    );
    let type_expr_leaf_graph = build_type_expr_leaf_decode(
        &mut assembler,
        Ns {
            k: 210,
            p: 210,
            b: 210,
            o: 210,
        },
        type_expr_leaf_function,
        union_function,
        fixed32_function,
        exact_uvar_function,
        bounded_uvar_function,
    );
    let record2_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 211,
            p: 211,
            b: 211,
            o: 211,
        },
        record2_function,
        decode_function,
        record_function,
        2,
    );
    let record3_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 212,
            p: 212,
            b: 212,
            o: 212,
        },
        record3_function,
        decode_function,
        record_function,
        3,
    );
    let type_expr_children_graph = build_type_expr_children_decode(
        &mut assembler,
        Ns {
            k: 213,
            p: 213,
            b: 213,
            o: 213,
        },
        type_expr_children_function,
        union_function,
        type_expr_leaf_function,
        list_function,
        record2_function,
        record3_function,
        fixed32_function,
        entity_id_collection_function,
    );
    let type_expr_recursive_graph = build_type_expr_recursive_decode(
        &mut assembler,
        Ns {
            k: 214,
            p: 214,
            b: 214,
            o: 214,
        },
        type_expr_recursive_function,
        type_expr_children_function,
    );
    let graph = build_function_schema_decode(
        &mut assembler,
        Ns {
            k: 215,
            p: 215,
            b: 215,
            o: 215,
        },
        function,
        union_function,
        record_function,
        entity_id_collection_function,
        type_parameter_list_function,
        fixed32_function,
        bounded_uvar_function,
        type_expr_recursive_function,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            record_graph,
            union_graph,
            entity_id_collection_graph,
            fixed32_graph,
            bounded_uvar_graph,
            type_parameter_list_graph,
            type_expr_leaf_graph,
            record2_graph,
            record3_graph,
            type_expr_children_graph,
            type_expr_recursive_graph,
            exact_uvar_graph,
            list_graph,
            decode_graph,
        ],
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

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn parameter_schema_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(216);
    let record_function = assembler.id(216);
    let union_function = assembler.id(216);
    let list_function = assembler.id(216);
    let fixed32_function = assembler.id(216);
    let entity_id_collection_function = assembler.id(216);
    let exact_uvar_function = assembler.id(216);
    let bounded_uvar_function = assembler.id(216);
    let type_expr_leaf_function = assembler.id(216);
    let record2_function = assembler.id(216);
    let record3_function = assembler.id(216);
    let type_expr_children_function = assembler.id(216);
    let type_expr_recursive_function = assembler.id(216);
    let record4_function = assembler.id(216);
    let function = assembler.id(216);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 217,
            p: 217,
            b: 217,
            o: 217,
        },
        decode_function,
    );
    let record_graph = build_generic_record_decode(
        &mut assembler,
        Ns {
            k: 218,
            p: 218,
            b: 218,
            o: 218,
        },
        record_function,
        decode_function,
    );
    let union_graph = build_generic_union_decode(
        &mut assembler,
        Ns {
            k: 219,
            p: 219,
            b: 219,
            o: 219,
        },
        union_function,
        decode_function,
    );
    let list_graph = build_generic_list_decode(
        &mut assembler,
        Ns {
            k: 220,
            p: 220,
            b: 220,
            o: 220,
        },
        list_function,
        decode_function,
    );
    let fixed32_graph = build_fixed32_decode(
        &mut assembler,
        Ns {
            k: 221,
            p: 221,
            b: 221,
            o: 221,
        },
        fixed32_function,
    );
    let entity_id_collection_graph = build_entity_id_collection_decode(
        &mut assembler,
        Ns {
            k: 222,
            p: 222,
            b: 222,
            o: 222,
        },
        entity_id_collection_function,
        list_function,
        fixed32_function,
    );
    let exact_uvar_graph = build_exact_uvar_decode(
        &mut assembler,
        Ns {
            k: 223,
            p: 223,
            b: 223,
            o: 223,
        },
        exact_uvar_function,
        decode_function,
    );
    let bounded_uvar_graph = build_bounded_uvar_decode(
        &mut assembler,
        Ns {
            k: 224,
            p: 224,
            b: 224,
            o: 224,
        },
        bounded_uvar_function,
        exact_uvar_function,
    );
    let type_expr_leaf_graph = build_type_expr_leaf_decode(
        &mut assembler,
        Ns {
            k: 225,
            p: 225,
            b: 225,
            o: 225,
        },
        type_expr_leaf_function,
        union_function,
        fixed32_function,
        exact_uvar_function,
        bounded_uvar_function,
    );
    let record2_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 226,
            p: 226,
            b: 226,
            o: 226,
        },
        record2_function,
        decode_function,
        record_function,
        2,
    );
    let record3_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 227,
            p: 227,
            b: 227,
            o: 227,
        },
        record3_function,
        decode_function,
        record_function,
        3,
    );
    let type_expr_children_graph = build_type_expr_children_decode(
        &mut assembler,
        Ns {
            k: 228,
            p: 228,
            b: 228,
            o: 228,
        },
        type_expr_children_function,
        union_function,
        type_expr_leaf_function,
        list_function,
        record2_function,
        record3_function,
        fixed32_function,
        entity_id_collection_function,
    );
    let type_expr_recursive_graph = build_type_expr_recursive_decode(
        &mut assembler,
        Ns {
            k: 229,
            p: 229,
            b: 229,
            o: 229,
        },
        type_expr_recursive_function,
        type_expr_children_function,
    );
    let record4_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 230,
            p: 230,
            b: 230,
            o: 230,
        },
        record4_function,
        decode_function,
        record_function,
        4,
    );
    let graph = build_parameter_schema_decode(
        &mut assembler,
        Ns {
            k: 231,
            p: 231,
            b: 231,
            o: 231,
        },
        function,
        union_function,
        record4_function,
        fixed32_function,
        bounded_uvar_function,
        exact_uvar_function,
        type_expr_recursive_function,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            record4_graph,
            type_expr_recursive_graph,
            type_expr_children_graph,
            record2_graph,
            record3_graph,
            type_expr_leaf_graph,
            bounded_uvar_graph,
            exact_uvar_graph,
            entity_id_collection_graph,
            fixed32_graph,
            list_graph,
            union_graph,
            record_graph,
            decode_graph,
        ],
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

fn assert_entry_cfg_surface(image: &Image) {
    let mut reached = std::collections::BTreeSet::new();
    let mut pending = vec![image.entry.entry_block];
    while let Some(block_id) = pending.pop() {
        if !reached.insert(block_id) {
            continue;
        }
        let block = image
            .blocks
            .iter()
            .find(|block| block.entity_id == block_id)
            .expect("generic decoder block resolves");
        match &block.terminator {
            Terminator::Return(_) | Terminator::Trap(_) => {}
            Terminator::Branch(branch) => pending.push(branch.edge.target),
            Terminator::CondBranch(branch) => {
                pending.extend([branch.if_true.target, branch.if_false.target]);
            }
            Terminator::VariantSwitch(switch) => {
                pending.extend(switch.cases.iter().map(|case| case.edge.target));
            }
        }
    }
    let unreachable = image
        .entry
        .blocks
        .iter()
        .filter(|block| !reached.contains(block))
        .collect::<Vec<_>>();
    assert!(
        unreachable.is_empty(),
        "unreachable blocks: {unreachable:?}"
    );
    let target_arity = |target: EntityId| {
        image
            .blocks
            .iter()
            .find(|block| block.entity_id == target)
            .map_or(0, |block| block.parameters.len())
    };
    let assert_edge = |source: EntityId, target: EntityId, arity: usize| {
        assert_eq!(
            arity,
            target_arity(target),
            "edge arity from {source:?} to {target:?}"
        );
    };
    for block_id in &image.entry.blocks {
        let block = image
            .blocks
            .iter()
            .find(|block| block.entity_id == *block_id)
            .unwrap();
        match &block.terminator {
            Terminator::Return(_) | Terminator::Trap(_) => {}
            Terminator::Branch(branch) => assert_edge(
                block.entity_id,
                branch.edge.target,
                branch.edge.arguments.len(),
            ),
            Terminator::CondBranch(branch) => {
                for edge in [&branch.if_true, &branch.if_false] {
                    assert_edge(block.entity_id, edge.target, edge.arguments.len());
                }
            }
            Terminator::VariantSwitch(switch) => {
                for case in &switch.cases {
                    assert_edge(block.entity_id, case.edge.target, case.edge.arguments.len());
                }
            }
        }
    }
}

#[test]
fn generic_record_decoder_preserves_arbitrary_ordered_fields() {
    let image = generic_record_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit(&image);
    assert_eq!(image.functions.len(), 2);
    assert_eq!(image.parameters.len(), 583);
    assert_eq!(image.blocks.len(), 86);
    assert_eq!(image.operations.len(), 166);
    assert_eq!(image.constants.len(), 43);
    assert_eq!(package.image_bytes.len(), 25_332);
    assert_eq!(
        approved.package_digest,
        [
            0xec, 0x73, 0x4c, 0x39, 0x1f, 0xde, 0xc9, 0x62, 0x59, 0x19, 0xa9, 0xbc, 0x62, 0x44,
            0x5c, 0x4b, 0xe2, 0x04, 0x1e, 0xab, 0xd3, 0x0f, 0x73, 0x8b, 0x75, 0xd9, 0xdf, 0xf0,
            0x81, 0x23, 0xf9, 0xda,
        ]
    );
    eprintln!(
        "GENERIC_RECORD functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    let fields = vec![
        (1_u32, b"alpha".to_vec()),
        (300_u32, vec![0, 1, 2, 3, 4]),
        (65_535_u32, Vec::new()),
    ];
    let encoded = sley_scb1::encode_record(&fields).expect("native record encodes");
    let outcome = execute(
        &package,
        &approved,
        vec![bytes_input(&encoded), unit_input()],
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("generic decoder must return")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("generic decoder must accept canonical fields: {value:?}")
    };
    let ConstData::Map(entries) = decoded.data else {
        panic!("generic decoder must return an ordered map")
    };
    assert_eq!(entries.len(), fields.len());
    for (entry, (tag, payload)) in entries.iter().zip(fields) {
        assert_eq!(entry.key.data, ConstData::UInt(u128::from(tag)));
        assert_eq!(entry.value.data, ConstData::Bytes(payload));
    }
}

fn generic_record_error(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    input: &[u8],
) -> Vec<u8> {
    let outcome = execute(package, approved, vec![bytes_input(input), unit_input()]);
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("generic decoder must return a typed refusal")
    };
    let ConstData::Result(ResultConst::Err(error)) = value.data else {
        panic!("generic decoder must refuse malformed record: {value:?}")
    };
    let ConstData::Bytes(code) = error.data else {
        panic!("generic decoder refusal must be Bytes")
    };
    code
}

#[test]
fn generic_record_decoder_matches_canonical_error_precedence() {
    let image = generic_record_decode_image();
    let (package, approved) = admit(&image);

    let empty = sley_scb1::encode_record(&[]).expect("empty record encodes");
    let outcome = execute(&package, &approved, vec![bytes_input(&empty), unit_input()]);
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("empty record decoder returns")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("empty record must decode")
    };
    assert_eq!(decoded.data, ConstData::Map(Vec::new()));

    let cases: [(&str, Vec<u8>, &[u8]); 6] = [
        ("duplicate", vec![2, 1, 0, 1, 0], b"SCB_FIELD_DUPLICATE"),
        ("order", vec![2, 2, 0, 1, 0], b"SCB_FIELD_ORDER"),
        ("trailing", vec![0, 0], b"SCB_TRAILING_BYTES"),
        ("truncated", vec![1, 1, 2, 0], b"SCB_LENGTH_OVERFLOW"),
        ("nonminimal", vec![0x80, 0], b"SCB_VARINT_NON_MINIMAL"),
        (
            "field_limit",
            sley_scb1::encode_uvar(65_536),
            b"SCB_RESOURCE_LIMIT",
        ),
    ];
    for (name, input, expected) in cases {
        assert_eq!(
            generic_record_error(&package, &approved, &input),
            expected,
            "{name} precedence"
        );
    }
}

#[test]
fn generic_union_decoder_preserves_runtime_tag_and_payload() {
    let image = generic_union_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit(&image);
    assert_eq!(image.functions.len(), 2);
    assert_eq!(image.parameters.len(), 449);
    assert_eq!(image.blocks.len(), 74);
    assert_eq!(image.operations.len(), 146);
    assert_eq!(image.constants.len(), 40);
    assert_eq!(package.image_bytes.len(), 21_210);
    assert_eq!(
        approved.package_digest,
        [
            0x94, 0xdb, 0x89, 0x1d, 0x5b, 0x7f, 0xbe, 0xdc, 0x3a, 0x9b, 0x47, 0x85, 0xbb, 0xe9,
            0x7d, 0xf5, 0xb5, 0xa2, 0x3a, 0x67, 0x9d, 0xe3, 0x61, 0x17, 0xb9, 0xcd, 0x2f, 0x87,
            0x80, 0xf7, 0x2f, 0xd6,
        ]
    );
    eprintln!(
        "GENERIC_UNION functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    let encoded = sley_scb1::encode_union(300, b"runtime-payload").expect("union encodes");
    let outcome = execute(
        &package,
        &approved,
        vec![bytes_input(&encoded), unit_input()],
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("generic union decoder must return")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("generic union decoder must accept canonical bytes: {value:?}")
    };
    assert_eq!(
        decoded.data,
        ConstData::Sequence(vec![u64_input(300), bytes_input(b"runtime-payload"),])
    );
}

#[test]
fn generic_union_decoder_rejects_noncanonical_boundaries() {
    let image = generic_union_decode_image();
    let (package, approved) = admit(&image);
    let mut trailing = sley_scb1::encode_union(1, b"").expect("union encodes");
    trailing.push(0);
    let mut over_limit = sley_scb1::encode_uvar(1);
    over_limit.extend(sley_scb1::encode_uvar(67_108_865));
    let cases: [(&str, Vec<u8>, &[u8]); 5] = [
        ("empty", Vec::new(), b"SCB_LENGTH_OVERFLOW"),
        ("trailing", trailing, b"SCB_TRAILING_BYTES"),
        ("truncated", vec![1, 2, 0], b"SCB_LENGTH_OVERFLOW"),
        (
            "nonminimal_tag",
            vec![0x81, 0, 0],
            b"SCB_VARINT_NON_MINIMAL",
        ),
        ("payload_limit", over_limit, b"SCB_RESOURCE_LIMIT"),
    ];
    for (name, input, expected) in cases {
        assert_eq!(
            generic_record_error(&package, &approved, &input),
            expected,
            "{name} precedence"
        );
    }
}

#[test]
fn generic_list_decoder_preserves_runtime_element_order() {
    let image = generic_list_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit(&image);
    assert_eq!(image.functions.len(), 2);
    assert_eq!(image.parameters.len(), 505);
    assert_eq!(image.blocks.len(), 78);
    assert_eq!(image.operations.len(), 153);
    assert_eq!(image.constants.len(), 40);
    assert_eq!(package.image_bytes.len(), 22_882);
    assert_eq!(
        approved.package_digest,
        [
            0x96, 0x85, 0x54, 0x00, 0x44, 0xfe, 0xc1, 0x73, 0xea, 0x4e, 0x93, 0xcc, 0x7e, 0x33,
            0x24, 0x40, 0xc9, 0x92, 0x18, 0x53, 0x91, 0x1e, 0x1e, 0xa0, 0x4b, 0x15, 0x43, 0xa2,
            0x00, 0xdc, 0x13, 0xfe,
        ]
    );
    eprintln!(
        "GENERIC_LIST functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    let items = vec![b"first".to_vec(), Vec::new(), b"third".to_vec()];
    let encoded = sley_scb1::encode_list(&items).expect("list encodes");
    let outcome = execute(
        &package,
        &approved,
        vec![bytes_input(&encoded), unit_input()],
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("generic list decoder must return")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("generic list decoder must accept canonical bytes: {value:?}")
    };
    let ConstData::Map(entries) = decoded.data else {
        panic!("generic list decoder must return an index map")
    };
    assert_eq!(entries.len(), items.len());
    for (index, (entry, expected)) in entries.iter().zip(items).enumerate() {
        assert_eq!(entry.key.data, ConstData::UInt(index as u128));
        assert_eq!(entry.value.data, ConstData::Bytes(expected));
    }
}

#[test]
fn generic_list_decoder_rejects_noncanonical_boundaries() {
    let image = generic_list_decode_image();
    let (package, approved) = admit(&image);
    let mut trailing = sley_scb1::encode_list(&[]).expect("list encodes");
    trailing.push(0);
    let mut over_limit = sley_scb1::encode_uvar(1);
    over_limit.extend(sley_scb1::encode_uvar(67_108_865));
    let cases: [(&str, Vec<u8>, &[u8]); 6] = [
        ("empty_input", Vec::new(), b"SCB_LENGTH_OVERFLOW"),
        ("trailing", trailing, b"SCB_TRAILING_BYTES"),
        ("truncated", vec![1, 2, 0], b"SCB_LENGTH_OVERFLOW"),
        ("nonminimal_count", vec![0x80, 0], b"SCB_VARINT_NON_MINIMAL"),
        (
            "item_limit",
            sley_scb1::encode_uvar(65_536),
            b"SCB_RESOURCE_LIMIT",
        ),
        ("payload_limit", over_limit, b"SCB_RESOURCE_LIMIT"),
    ];
    for (name, input, expected) in cases {
        assert_eq!(
            generic_record_error(&package, &approved, &input),
            expected,
            "{name} precedence"
        );
    }
}

fn function_schema_body_with_type_parameters_and_result(
    type_parameters: Vec<sley_ssmc::TypeParameterDef>,
    result_type: TypeExpr,
) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, FunctionBody};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0x81; 32]),
        body: EntityBodyValue::Function(FunctionBody {
            type_parameters,
            parameters: vec![EntityId::from_bytes([0x82; 32])],
            result_type,
            effects: EntityIdSet::from_unsorted(vec![EntityId::from_bytes([0x83; 32])])
                .expect("single effect is canonical"),
            entry_block: EntityId::from_bytes([0x84; 32]),
            blocks: vec![
                EntityId::from_bytes([0x84; 32]),
                EntityId::from_bytes([0x85; 32]),
            ],
            contracts: EntityIdSet::from_unsorted(vec![EntityId::from_bytes([0x86; 32])])
                .expect("single contract is canonical"),
            visibility: Visibility::Exported,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema Function fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

fn function_schema_body_with_result(result_type: TypeExpr) -> Vec<u8> {
    function_schema_body_with_type_parameters_and_result(Vec::new(), result_type)
}

fn function_schema_body() -> Vec<u8> {
    function_schema_body_with_type_parameters_and_result(
        vec![sley_ssmc::TypeParameterDef { ordinal: 0 }],
        TypeExpr::Bool,
    )
}

fn type_parameter_list_body(ordinals: &[u32]) -> Vec<u8> {
    let body = function_schema_body_with_type_parameters_and_result(
        ordinals
            .iter()
            .copied()
            .map(|ordinal| sley_ssmc::TypeParameterDef { ordinal })
            .collect(),
        TypeExpr::Bool,
    );
    native_union_record_payloads(&body)[0].clone()
}

fn type_expr_body(value_type: TypeExpr) -> Vec<u8> {
    let body = function_schema_body_with_result(value_type);
    native_union_record_payloads(&body)[2].clone()
}

fn native_union_record_payloads(body: &[u8]) -> Vec<Vec<u8>> {
    let mut union = sley_scb1::ScbValueCursor::new(body).expect("body cursor");
    let (tag, payload) = union.read_union().expect("body union");
    assert_eq!(tag, 5);
    union.check_finished().expect("body union is exact");
    let mut record = sley_scb1::ScbValueCursor::new(payload).expect("record cursor");
    assert_eq!(record.read_record_field_count().expect("field count"), 8);
    let mut fields = Vec::new();
    for expected_tag in 1..=8 {
        assert_eq!(record.read_uvar(32).expect("field tag"), expected_tag);
        fields.push(record.read_sized_payload().expect("field payload").to_vec());
    }
    record.check_finished().expect("record is exact");
    fields
}

#[test]
fn function_schema_decoder_projects_all_runtime_fields() {
    let body = function_schema_body();
    let expected = native_union_record_payloads(&body);
    let image = function_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "FUNCTION_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 15);
    assert_eq!(image.parameters.len(), 1_427);
    assert_eq!(image.blocks.len(), 334);
    assert_eq!(image.operations.len(), 601);
    assert_eq!(image.constants.len(), 191);
    assert_eq!(package.image_bytes.len(), 82_222);
    assert_eq!(
        approved.package_digest,
        [
            0xb5, 0xad, 0x52, 0x70, 0x51, 0x1b, 0x9e, 0xe6, 0xf2, 0xe1, 0x1a, 0x72, 0x47, 0xcb,
            0x24, 0x56, 0x45, 0x53, 0xa9, 0x82, 0xf8, 0x4b, 0x79, 0x88, 0xf3, 0xb2, 0xd8, 0xae,
            0x4b, 0x61, 0x96, 0x3a,
        ]
    );
    let outcome = execute_with_limits(
        &package,
        &approved,
        vec![bytes_input(&body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("Function schema decoder must return")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("Function schema decoder must accept native body: {value:?}")
    };
    let ConstData::Sequence(fields) = decoded.data else {
        panic!("Function schema decoder must return an eight-field tuple")
    };
    assert_eq!(fields.len(), 8);
    for (field, expected) in fields.iter().zip(expected) {
        assert_eq!(field.data, ConstData::Bytes(expected));
    }
    let composite_body =
        function_schema_body_with_result(TypeExpr::Option(Box::new(TypeExpr::Tuple(vec![
            TypeExpr::Bool,
            TypeExpr::Bytes,
        ]))));
    let composite_outcome = execute_with_limits(
        &package,
        &approved,
        vec![bytes_input(&composite_body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(composite_value) = composite_outcome.termination
    else {
        panic!("Function schema decoder must return for recursive result type")
    };
    assert!(matches!(
        composite_value.data,
        ConstData::Result(ResultConst::Ok(_))
    ));
}

fn function_schema_error(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    input: &[u8],
) -> Vec<u8> {
    let outcome = execute_with_limits(
        package,
        approved,
        vec![bytes_input(input), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "Function schema decoder must return a typed refusal: {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Err(error)) = &value.data else {
        panic!("Function schema decoder must refuse malformed input: {value:?}")
    };
    let ConstData::Bytes(code) = &error.data else {
        panic!("Function schema refusal must be Bytes")
    };
    code.clone()
}

#[test]
#[allow(clippy::too_many_lines)]
fn function_schema_decoder_enforces_kind_fields_and_list_boundaries() {
    let body = function_schema_body();
    let fields = native_union_record_payloads(&body);
    let image = function_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let wrong_kind = sley_scb1::encode_union(
        6,
        &sley_scb1::encode_record(
            &fields
                .iter()
                .enumerate()
                .map(|(index, payload)| {
                    (
                        u32::try_from(index + 1).expect("eight Function fields"),
                        payload.clone(),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .expect("record encodes"),
    )
    .expect("union encodes");
    let missing = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(
            &fields[..7]
                .iter()
                .enumerate()
                .map(|(index, payload)| {
                    (
                        u32::try_from(index + 1).expect("seven Function fields"),
                        payload.clone(),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .expect("record encodes"),
    )
    .expect("union encodes");
    let mut unknown_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("eight Function fields"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    unknown_fields.push((9, Vec::new()));
    let unknown = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(&unknown_fields).expect("record encodes"),
    )
    .expect("union encodes");
    let mut malformed_list_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("eight Function fields"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    malformed_list_fields[1].1 = vec![0x80, 0];
    let malformed_list = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(&malformed_list_fields).expect("record encodes"),
    )
    .expect("union encodes");
    let mut missing_type_parameter_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("eight Function fields"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    missing_type_parameter_fields[0].1 = sley_scb1::encode_list(&[
        sley_scb1::encode_record(&[]).expect("empty type-parameter record encodes")
    ])
    .expect("type-parameter list encodes");
    let missing_type_parameter = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(&missing_type_parameter_fields).expect("record encodes"),
    )
    .expect("union encodes");
    let mut unknown_type_parameter_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("eight Function fields"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    unknown_type_parameter_fields[0].1 = sley_scb1::encode_list(&[sley_scb1::encode_record(&[
        (1, sley_scb1::encode_uvar(0)),
        (2, Vec::new()),
    ])
    .expect("unknown type-parameter field encodes")])
    .expect("type-parameter list encodes");
    let unknown_type_parameter = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(&unknown_type_parameter_fields).expect("record encodes"),
    )
    .expect("union encodes");
    let mut short_parameter_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("eight Function fields"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    short_parameter_fields[1].1 =
        sley_scb1::encode_list(&[vec![0; 31]]).expect("short identity list encodes");
    let short_parameter = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(&short_parameter_fields).expect("record encodes"),
    )
    .expect("union encodes");
    let mut unordered_effect_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("eight Function fields"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    unordered_effect_fields[3].1 = sley_scb1::encode_list(&[vec![2; 32], vec![1; 32]])
        .expect("unordered identity set encodes structurally");
    let unordered_effect = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(&unordered_effect_fields).expect("record encodes"),
    )
    .expect("union encodes");
    let mut short_entry_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("eight Function fields"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    short_entry_fields[4].1 = vec![0; 31];
    let short_entry = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(&short_entry_fields).expect("record encodes"),
    )
    .expect("union encodes");
    let mut invalid_visibility_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("eight Function fields"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    invalid_visibility_fields[7].1 = sley_scb1::encode_uvar(5);
    let invalid_visibility = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(&invalid_visibility_fields).expect("record encodes"),
    )
    .expect("union encodes");
    let mut nonminimal_visibility_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("eight Function fields"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    nonminimal_visibility_fields[7].1 = vec![0x81, 0];
    let nonminimal_visibility = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(&nonminimal_visibility_fields).expect("record encodes"),
    )
    .expect("union encodes");
    let mut malformed_result_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("eight Function fields"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    malformed_result_fields[2].1 =
        sley_scb1::encode_union(1, &[0]).expect("malformed Unit TypeExpr encodes structurally");
    let malformed_result = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(&malformed_result_fields).expect("record encodes"),
    )
    .expect("union encodes");
    let mut nested_invalid_result_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("eight Function fields"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    let unknown_child = sley_scb1::encode_union(21, &[]).expect("unknown child TypeExpr encodes");
    nested_invalid_result_fields[2].1 = sley_scb1::encode_union(
        9,
        &sley_scb1::encode_list(&[unknown_child]).expect("Tuple element list encodes"),
    )
    .expect("Tuple TypeExpr encodes structurally");
    let nested_invalid_result = sley_scb1::encode_union(
        5,
        &sley_scb1::encode_record(&nested_invalid_result_fields).expect("record encodes"),
    )
    .expect("union encodes");

    for (name, input, expected) in [
        ("wrong_kind", wrong_kind, b"SCB_UNION_INVALID".as_slice()),
        ("missing", missing, b"SCB_FIELD_MISSING".as_slice()),
        ("unknown", unknown, b"SCB_FIELD_UNKNOWN".as_slice()),
        (
            "malformed_list",
            malformed_list,
            b"SCB_VARINT_NON_MINIMAL".as_slice(),
        ),
        (
            "missing_type_parameter_ordinal",
            missing_type_parameter,
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_type_parameter_field",
            unknown_type_parameter,
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "short_parameter_identity",
            short_parameter,
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "unordered_effect_set",
            unordered_effect,
            b"SCB_MAP_ORDER".as_slice(),
        ),
        (
            "short_entry_identity",
            short_entry,
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "invalid_visibility",
            invalid_visibility,
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "nonminimal_visibility",
            nonminimal_visibility,
            b"SCB_VARINT_NON_MINIMAL".as_slice(),
        ),
        (
            "malformed_result_type",
            malformed_result,
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "nested_invalid_result_type",
            nested_invalid_result,
            b"SCB_UNION_INVALID".as_slice(),
        ),
    ] {
        eprintln!("FUNCTION_SCHEMA_REJ {name}");
        assert_eq!(
            function_schema_error(&package, &approved, &input),
            expected,
            "{name} precedence"
        );
    }
}

fn bool_value_input(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

#[test]
fn entity_id_collection_decoder_accepts_lists_and_canonical_sets() {
    let image = entity_id_collection_decode_image();
    let (package, approved) = admit(&image);
    assert_entry_cfg_surface(&image);
    assert_eq!(image.functions.len(), 4);
    assert_eq!(image.parameters.len(), 538);
    assert_eq!(image.blocks.len(), 96);
    assert_eq!(image.operations.len(), 180);
    assert_eq!(image.constants.len(), 50);
    assert_eq!(package.image_bytes.len(), 26_064);
    assert_eq!(
        approved.package_digest,
        [
            0xcf, 0x1b, 0x06, 0x57, 0x35, 0x3d, 0x4a, 0xde, 0x9f, 0x48, 0x02, 0x17, 0xfd, 0xa3,
            0xae, 0xe3, 0xdd, 0x2c, 0xfd, 0x12, 0x7f, 0x67, 0xb0, 0xf3, 0x30, 0x0a, 0x96, 0x56,
            0x7b, 0x82, 0x33, 0xb5,
        ]
    );
    eprintln!(
        "ENTITY_ID_COLLECTION functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    for (ordered, items) in [
        (false, vec![vec![3; 32], vec![1; 32], vec![3; 32]]),
        (true, vec![vec![1; 32], vec![2; 32], vec![3; 32]]),
    ] {
        let encoded = sley_scb1::encode_list(&items).expect("identity collection encodes");
        let outcome = execute(
            &package,
            &approved,
            vec![
                bytes_input(&encoded),
                bool_value_input(ordered),
                unit_input(),
            ],
        );
        let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
            panic!("identity collection decoder must return")
        };
        let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
            panic!("valid identity collection must decode: {value:?}")
        };
        assert_eq!(decoded.data, ConstData::Unit);
    }
}

#[test]
fn entity_id_collection_decoder_rejects_width_duplicates_and_order() {
    let image = entity_id_collection_decode_image();
    let (package, approved) = admit(&image);
    let cases = [
        (
            "short_identity",
            false,
            vec![vec![1; 31]],
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "duplicate_set_identity",
            true,
            vec![vec![1; 32], vec![1; 32]],
            b"SCB_MAP_DUPLICATE".as_slice(),
        ),
        (
            "descending_set_identity",
            true,
            vec![vec![2; 32], vec![1; 32]],
            b"SCB_MAP_ORDER".as_slice(),
        ),
    ];
    for (name, ordered, items, expected) in cases {
        let encoded = sley_scb1::encode_list(&items).expect("identity collection encodes");
        let outcome = execute(
            &package,
            &approved,
            vec![
                bytes_input(&encoded),
                bool_value_input(ordered),
                unit_input(),
            ],
        );
        let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
            panic!("identity collection decoder must return")
        };
        let ConstData::Result(ResultConst::Err(error)) = value.data else {
            panic!("invalid identity collection must be refused: {value:?}")
        };
        assert_eq!(error.data, ConstData::Bytes(expected.to_vec()), "{name}");
    }
}

#[test]
fn type_parameter_list_decoder_accepts_canonical_records() {
    let image = type_parameter_list_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit(&image);
    eprintln!(
        "TYPE_PARAMETER_LIST functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 5);
    assert_eq!(image.parameters.len(), 803);
    assert_eq!(image.blocks.len(), 134);
    assert_eq!(image.operations.len(), 241);
    assert_eq!(image.constants.len(), 64);
    assert_eq!(package.image_bytes.len(), 36_602);
    assert_eq!(
        approved.package_digest,
        [
            0x61, 0x0c, 0x4a, 0x95, 0x49, 0x2a, 0x5b, 0xd0, 0xcd, 0x7d, 0x0d, 0x98, 0x02, 0xdb,
            0x2d, 0xbf, 0x85, 0x33, 0x95, 0x86, 0x94, 0x90, 0x91, 0x7c, 0x76, 0x48, 0x81, 0x7b,
            0x73, 0xc6, 0xff, 0x1e,
        ]
    );
    for ordinals in [Vec::new(), vec![0], vec![7, u32::MAX]] {
        let encoded = type_parameter_list_body(&ordinals);
        let outcome = execute(
            &package,
            &approved,
            vec![bytes_input(&encoded), unit_input()],
        );
        let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
            panic!("type-parameter list decoder must return")
        };
        let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
            panic!("canonical type-parameter records must decode: {value:?}")
        };
        assert_eq!(decoded.data, ConstData::Unit);
    }
}

#[test]
fn type_parameter_list_decoder_rejects_noncanonical_records() {
    let image = type_parameter_list_decode_image();
    let (package, approved) = admit(&image);
    let missing =
        sley_scb1::encode_list(&[sley_scb1::encode_record(&[]).expect("empty record encodes")])
            .expect("list encodes");
    let unknown_only = sley_scb1::encode_list(&[
        sley_scb1::encode_record(&[(2, Vec::new())]).expect("unknown field record encodes")
    ])
    .expect("list encodes");
    let extra_unknown = sley_scb1::encode_list(&[sley_scb1::encode_record(&[
        (1, sley_scb1::encode_uvar(0)),
        (2, Vec::new()),
    ])
    .expect("extra field record encodes")])
    .expect("list encodes");
    let nonminimal = sley_scb1::encode_list(&[
        sley_scb1::encode_record(&[(1, vec![0x80, 0])]).expect("ordinal record encodes")
    ])
    .expect("list encodes");
    let trailing = sley_scb1::encode_list(&[
        sley_scb1::encode_record(&[(1, vec![0, 0])]).expect("ordinal record encodes")
    ])
    .expect("list encodes");
    let cases = [
        (
            "nonminimal_list_count",
            vec![0x80, 0],
            b"SCB_VARINT_NON_MINIMAL".as_slice(),
        ),
        ("missing_ordinal", missing, b"SCB_FIELD_MISSING".as_slice()),
        (
            "unknown_before_missing",
            unknown_only,
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "extra_unknown",
            extra_unknown,
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "nonminimal_ordinal",
            nonminimal,
            b"SCB_VARINT_NON_MINIMAL".as_slice(),
        ),
        (
            "trailing_ordinal",
            trailing,
            b"SCB_TRAILING_BYTES".as_slice(),
        ),
    ];
    for (name, input, expected) in cases {
        let outcome = execute(&package, &approved, vec![bytes_input(&input), unit_input()]);
        let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
            panic!("type-parameter list decoder must return")
        };
        let ConstData::Result(ResultConst::Err(error)) = value.data else {
            panic!("invalid type-parameter list must be refused: {value:?}")
        };
        assert_eq!(error.data, ConstData::Bytes(expected.to_vec()), "{name}");
    }
}

#[test]
fn bounded_uvar_decoder_accepts_runtime_values_in_range() {
    let image = bounded_uvar_decode_image();
    let (package, approved) = admit(&image);
    assert_entry_cfg_surface(&image);
    assert_eq!(image.functions.len(), 3);
    assert_eq!(image.parameters.len(), 336);
    assert_eq!(image.blocks.len(), 63);
    assert_eq!(image.operations.len(), 133);
    assert_eq!(image.constants.len(), 34);
    assert_eq!(package.image_bytes.len(), 17_820);
    assert_eq!(
        approved.package_digest,
        [
            0x9a, 0x75, 0xac, 0xf1, 0x1c, 0xf9, 0xf1, 0x2e, 0xd4, 0x8f, 0x02, 0x41, 0x4b, 0xd5,
            0x92, 0xe1, 0xb8, 0x95, 0x93, 0x05, 0xb9, 0x5d, 0xbc, 0x13, 0x97, 0x61, 0xee, 0x88,
            0x81, 0x59, 0x72, 0x19,
        ]
    );
    eprintln!(
        "BOUNDED_UVAR functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    for value in 1_u64..=4 {
        let encoded = sley_scb1::encode_uvar(value);
        let outcome = execute(
            &package,
            &approved,
            vec![
                bytes_input(&encoded),
                u64_input(1),
                u64_input(4),
                unit_input(),
            ],
        );
        let sley_vm::ExecutionTermination::Success(result) = outcome.termination else {
            panic!("bounded uvar decoder must return")
        };
        let ConstData::Result(ResultConst::Ok(decoded)) = result.data else {
            panic!("in-range uvar must decode: {result:?}")
        };
        assert_eq!(decoded.data, ConstData::UInt(u128::from(value)));
    }
}

#[test]
fn bounded_uvar_decoder_rejects_noncanonical_and_out_of_range_values() {
    let image = bounded_uvar_decode_image();
    let (package, approved) = admit(&image);
    let cases = [
        ("empty", Vec::new(), b"SCB_LENGTH_OVERFLOW".as_slice()),
        (
            "nonminimal",
            vec![0x81, 0],
            b"SCB_VARINT_NON_MINIMAL".as_slice(),
        ),
        ("trailing", vec![1, 0], b"SCB_TRAILING_BYTES".as_slice()),
        (
            "below_range",
            sley_scb1::encode_uvar(0),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "above_range",
            sley_scb1::encode_uvar(5),
            b"SCB_UNION_INVALID".as_slice(),
        ),
    ];
    for (name, input, expected) in cases {
        let outcome = execute(
            &package,
            &approved,
            vec![
                bytes_input(&input),
                u64_input(1),
                u64_input(4),
                unit_input(),
            ],
        );
        let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
            panic!("bounded uvar decoder must return")
        };
        let ConstData::Result(ResultConst::Err(error)) = value.data else {
            panic!("invalid bounded uvar must be refused: {value:?}")
        };
        assert_eq!(error.data, ConstData::Bytes(expected.to_vec()), "{name}");
    }
}

fn type_expr_children_shape(value_type: TypeExpr) -> (usize, usize) {
    let image = type_expr_children_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    let encoded = type_expr_body(value_type);
    let outcome = execute_with_limits(
        &package,
        &approved,
        vec![bytes_input(&encoded), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("TypeExpr children decoder must return")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("TypeExpr children decoder must accept native bytes: {value:?}")
    };
    let ConstData::Sequence(parts) = decoded.data else {
        panic!("TypeExpr children decoder must return a tuple")
    };
    let direct = parts[..2]
        .iter()
        .filter(|part| matches!(&part.data, ConstData::Option(Some(_))))
        .count();
    let ConstData::Map(entries) = &parts[2].data else {
        panic!("TypeExpr children decoder must return an indexed child map")
    };
    (direct, entries.len())
}

#[test]
fn type_expr_children_decoder_projects_every_composite_family() {
    use sley_ssmc::{FunctionType, NamedType};

    let cases = [
        ("leaf", TypeExpr::Bool, (0, 0)),
        (
            "tuple",
            TypeExpr::Tuple(vec![
                TypeExpr::Bool,
                TypeExpr::Option(Box::new(TypeExpr::Unit)),
            ]),
            (0, 2),
        ),
        (
            "named",
            TypeExpr::Named(NamedType {
                definition: EntityId::from_bytes([0x31; 32]),
                arguments: vec![TypeExpr::Bool, TypeExpr::Bytes],
            }),
            (0, 2),
        ),
        ("vector", TypeExpr::Vector(Box::new(TypeExpr::Bool)), (1, 0)),
        (
            "ordered_map",
            TypeExpr::OrderedMap {
                key: Box::new(TypeExpr::Bool),
                value: Box::new(TypeExpr::Bytes),
            },
            (2, 0),
        ),
        ("option", TypeExpr::Option(Box::new(TypeExpr::Text)), (1, 0)),
        (
            "result",
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Bytes),
                error: Box::new(TypeExpr::Bool),
            },
            (2, 0),
        ),
        (
            "function_ref",
            TypeExpr::FunctionRef(FunctionType {
                parameters: vec![TypeExpr::Bool, TypeExpr::Bytes],
                result: Box::new(TypeExpr::Unit),
                effects: vec![EntityId::from_bytes([0x41; 32])],
            }),
            (1, 2),
        ),
        (
            "local_cell",
            TypeExpr::LocalCell(Box::new(TypeExpr::UInt(
                sley_ssmc::IntegerWidth::from_bits(16),
            ))),
            (1, 0),
        ),
    ];
    for (name, value_type, expected) in cases {
        assert_eq!(type_expr_children_shape(value_type), expected, "{name}");
    }
}

fn type_expr_children_error(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    input: &[u8],
) -> Vec<u8> {
    let outcome = execute_with_limits(
        package,
        approved,
        vec![bytes_input(input), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("TypeExpr children decoder must return")
    };
    let ConstData::Result(ResultConst::Err(error)) = value.data else {
        panic!("TypeExpr children decoder must refuse malformed input: {value:?}")
    };
    let ConstData::Bytes(code) = error.data else {
        panic!("TypeExpr children refusal must be Bytes")
    };
    code
}

#[test]
#[allow(clippy::too_many_lines)]
fn type_expr_children_decoder_preserves_composite_errors() {
    let image = type_expr_children_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "TYPE_EXPR_CHILDREN functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 12);
    assert_eq!(image.parameters.len(), 1_127);
    assert_eq!(image.blocks.len(), 266);
    assert_eq!(image.operations.len(), 479);
    assert_eq!(image.constants.len(), 151);
    assert_eq!(package.image_bytes.len(), 64_094);
    assert_eq!(
        approved.package_digest,
        [
            0x53, 0x5d, 0x77, 0xa5, 0x23, 0x30, 0x7a, 0xd7, 0x21, 0x19, 0xd9, 0x1f, 0x32, 0x33,
            0x2f, 0x11, 0x51, 0x52, 0xca, 0x80, 0xa2, 0x17, 0xb9, 0x63, 0x36, 0x87, 0x41, 0xbd,
            0xdf, 0x69, 0x34, 0x14,
        ]
    );
    let bool_type = type_expr_body(TypeExpr::Bool);
    let tuple_nonminimal =
        sley_scb1::encode_union(9, &[0x80, 0]).expect("tuple TypeExpr encodes structurally");
    let pair_missing = sley_scb1::encode_union(
        12,
        &sley_scb1::encode_record(&[(1, bool_type.clone())]).expect("pair record encodes"),
    )
    .expect("map TypeExpr encodes");
    let pair_unknown = sley_scb1::encode_union(
        14,
        &sley_scb1::encode_record(&[(1, bool_type.clone()), (3, bool_type.clone())])
            .expect("unknown pair record encodes"),
    )
    .expect("result TypeExpr encodes");
    let named_short = sley_scb1::encode_union(
        10,
        &sley_scb1::encode_record(&[
            (1, vec![0; 31]),
            (
                2,
                sley_scb1::encode_list(&[]).expect("arguments list encodes"),
            ),
        ])
        .expect("named record encodes"),
    )
    .expect("named TypeExpr encodes");
    let function_unordered_effects = sley_scb1::encode_union(
        15,
        &sley_scb1::encode_record(&[
            (
                1,
                sley_scb1::encode_list(&[]).expect("parameters list encodes"),
            ),
            (2, bool_type),
            (
                3,
                sley_scb1::encode_list(&[vec![2; 32], vec![1; 32]])
                    .expect("effect list encodes structurally"),
            ),
        ])
        .expect("function record encodes"),
    )
    .expect("function TypeExpr encodes");
    let unknown = sley_scb1::encode_union(21, &[]).expect("unknown TypeExpr encodes");
    for (name, input, expected) in [
        (
            "tuple_nonminimal",
            tuple_nonminimal,
            b"SCB_VARINT_NON_MINIMAL".as_slice(),
        ),
        (
            "pair_missing",
            pair_missing,
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "pair_unknown",
            pair_unknown,
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "named_short_definition",
            named_short,
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "function_unordered_effects",
            function_unordered_effects,
            b"SCB_MAP_ORDER".as_slice(),
        ),
        ("unknown_tag", unknown, b"SCB_UNION_INVALID".as_slice()),
    ] {
        assert_eq!(
            type_expr_children_error(&package, &approved, &input),
            expected,
            "{name}"
        );
    }
}

fn type_expr_recursive_result(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    input: &[u8],
) -> ConstValue {
    let outcome = execute_with_limits(
        package,
        approved,
        vec![bytes_input(input), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("recursive TypeExpr decoder must return")
    };
    value
}

#[test]
fn type_expr_recursive_decoder_accepts_nested_composites() {
    use sley_ssmc::{FunctionType, NamedType};

    let image = type_expr_recursive_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "TYPE_EXPR_RECURSIVE functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 13);
    assert_eq!(image.parameters.len(), 1_267);
    assert_eq!(image.blocks.len(), 293);
    assert_eq!(image.operations.len(), 526);
    assert_eq!(image.constants.len(), 166);
    assert_eq!(package.image_bytes.len(), 71_588);
    assert_eq!(
        approved.package_digest,
        [
            0x4d, 0xb4, 0x85, 0x1e, 0xa7, 0xee, 0xb6, 0xde, 0x33, 0x19, 0xa8, 0x95, 0xaf, 0xd7,
            0x8a, 0x9d, 0xb0, 0x17, 0x65, 0x45, 0xac, 0xee, 0x6f, 0x97, 0x4f, 0xc4, 0xdb, 0x23,
            0x66, 0x97, 0x3b, 0xe5,
        ]
    );
    let nested = TypeExpr::FunctionRef(FunctionType {
        parameters: vec![
            TypeExpr::Tuple(vec![
                TypeExpr::Bool,
                TypeExpr::Vector(Box::new(TypeExpr::UInt(
                    sley_ssmc::IntegerWidth::from_bits(16),
                ))),
            ]),
            TypeExpr::Named(NamedType {
                definition: EntityId::from_bytes([0x51; 32]),
                arguments: vec![TypeExpr::OrderedMap {
                    key: Box::new(TypeExpr::Text),
                    value: Box::new(TypeExpr::Option(Box::new(TypeExpr::Bytes))),
                }],
            }),
        ],
        result: Box::new(TypeExpr::Result {
            ok: Box::new(TypeExpr::LocalCell(Box::new(TypeExpr::Unit))),
            error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Capability)),
        }),
        effects: vec![EntityId::from_bytes([0x61; 32])],
    });
    let encoded = type_expr_body(nested);
    let value = type_expr_recursive_result(&package, &approved, &encoded);
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("nested TypeExpr must validate: {value:?}")
    };
    assert_eq!(decoded.data, ConstData::Bytes(encoded));

    let mut allowed = sley_scb1::encode_union(1, &[]).expect("Unit TypeExpr encodes");
    for _ in 0..63 {
        allowed = sley_scb1::encode_union(13, &allowed).expect("Option TypeExpr encodes");
    }
    let value = type_expr_recursive_result(&package, &approved, &allowed);
    assert!(matches!(value.data, ConstData::Result(ResultConst::Ok(_))));
}

#[test]
fn type_expr_recursive_decoder_rejects_nested_errors_and_depth_overflow() {
    let image = type_expr_recursive_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    let unknown = sley_scb1::encode_union(21, &[]).expect("unknown TypeExpr encodes");
    let nested_unknown = sley_scb1::encode_union(13, &unknown).expect("Option TypeExpr encodes");
    let mut too_deep = sley_scb1::encode_union(1, &[]).expect("Unit TypeExpr encodes");
    for _ in 0..64 {
        too_deep = sley_scb1::encode_union(13, &too_deep).expect("Option TypeExpr encodes");
    }
    for (name, input, expected) in [
        (
            "nested_unknown",
            nested_unknown,
            b"SCB_UNION_INVALID".as_slice(),
        ),
        ("depth_overflow", too_deep, b"SCB_RESOURCE_LIMIT".as_slice()),
    ] {
        let value = type_expr_recursive_result(&package, &approved, &input);
        let ConstData::Result(ResultConst::Err(error)) = value.data else {
            panic!("invalid recursive TypeExpr must be refused: {value:?}")
        };
        assert_eq!(error.data, ConstData::Bytes(expected.to_vec()), "{name}");
    }
}

#[test]
fn type_expr_leaf_decoder_accepts_every_leaf_family() {
    use sley_ssmc::BuiltinFailureKind;

    let image = type_expr_leaf_decode_image();
    let (package, approved) = admit(&image);
    assert_entry_cfg_surface(&image);
    assert_eq!(image.functions.len(), 6);
    assert_eq!(image.parameters.len(), 516);
    assert_eq!(image.blocks.len(), 122);
    assert_eq!(image.operations.len(), 236);
    assert_eq!(image.constants.len(), 75);
    assert_eq!(package.image_bytes.len(), 30_214);
    assert_eq!(
        approved.package_digest,
        [
            0x7b, 0x62, 0x7c, 0xdd, 0x03, 0x4a, 0x92, 0x2b, 0xee, 0x52, 0x72, 0x3e, 0xef, 0xc9,
            0x6a, 0x7c, 0x39, 0x43, 0xf9, 0xef, 0x2b, 0x78, 0xbf, 0x80, 0x16, 0x32, 0xaa, 0xb1,
            0xf1, 0xb4, 0x6c, 0xed,
        ]
    );
    eprintln!(
        "TYPE_EXPR_LEAF functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    let types = [
        TypeExpr::Unit,
        TypeExpr::Bool,
        TypeExpr::SInt(IntegerWidth::from_bits(24)),
        TypeExpr::UInt(IntegerWidth::from_bits(128)),
        TypeExpr::F32,
        TypeExpr::F64,
        TypeExpr::Bytes,
        TypeExpr::Text,
        TypeExpr::AdapterHandle(EntityId::from_bytes([0xa1; 32])),
        TypeExpr::CapabilityToken(EntityId::from_bytes([0xa2; 32])),
        TypeExpr::TypeParameter(3),
        TypeExpr::BuiltinFailure(BuiltinFailureKind::Capability),
    ];
    for value_type in types {
        let body = function_schema_body_with_result(value_type);
        let encoded = native_union_record_payloads(&body)[2].clone();
        let outcome = execute(
            &package,
            &approved,
            vec![bytes_input(&encoded), unit_input()],
        );
        let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
            panic!("TypeExpr leaf decoder must return")
        };
        let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
            panic!("native TypeExpr leaf must decode: {value:?}")
        };
        assert_eq!(decoded.data, ConstData::Bytes(encoded));
    }
}

#[test]
fn type_expr_leaf_decoder_rejects_invalid_leaves_and_scopes_composites() {
    let image = type_expr_leaf_decode_image();
    let (package, approved) = admit(&image);
    let composite_body =
        function_schema_body_with_result(TypeExpr::Option(Box::new(TypeExpr::Unit)));
    let composite = native_union_record_payloads(&composite_body)[2].clone();
    let cases = [
        (
            "unit_payload",
            sley_scb1::encode_union(1, &[0]).expect("union encodes"),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "short_adapter_identity",
            sley_scb1::encode_union(16, &[0; 31]).expect("union encodes"),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "nonminimal_type_parameter",
            sley_scb1::encode_union(19, &[0x81, 0]).expect("union encodes"),
            b"SCB_VARINT_NON_MINIMAL".as_slice(),
        ),
        (
            "unknown_failure_kind",
            sley_scb1::encode_union(20, &sley_scb1::encode_uvar(6)).expect("union encodes"),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "composite_scope",
            composite,
            b"SSMC_RESERVED_FIELD_PRESENT".as_slice(),
        ),
        (
            "unknown_type_tag",
            sley_scb1::encode_union(21, &[]).expect("union encodes"),
            b"SCB_UNION_INVALID".as_slice(),
        ),
    ];
    for (name, input, expected) in cases {
        let outcome = execute(&package, &approved, vec![bytes_input(&input), unit_input()]);
        let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
            panic!("TypeExpr leaf decoder must return")
        };
        let ConstData::Result(ResultConst::Err(error)) = value.data else {
            panic!("invalid TypeExpr leaf must be refused: {value:?}")
        };
        assert_eq!(error.data, ConstData::Bytes(expected.to_vec()), "{name}");
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

fn parameter_schema_body(role: ParameterRole, ordinal: u32, value_type: TypeExpr) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, ParameterBody};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0xe1; 32]),
        body: EntityBodyValue::Parameter(ParameterBody {
            owner: EntityId::from_bytes([0xe2; 32]),
            role,
            ordinal,
            value_type,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema Parameter fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

fn exact_entity_body_fields(body: &[u8], expected_kind: u32, field_count: u32) -> Vec<Vec<u8>> {
    let mut union = sley_scb1::ScbValueCursor::new(body).expect("body cursor");
    let (kind, payload) = union.read_union().expect("body union");
    assert_eq!(kind, expected_kind);
    union.check_finished().expect("body union is exact");
    let mut record = sley_scb1::ScbValueCursor::new(payload).expect("record cursor");
    assert_eq!(
        record.read_record_field_count().expect("field count"),
        u64::from(field_count)
    );
    let mut fields = Vec::new();
    for expected_tag in 1..=field_count {
        assert_eq!(
            record.read_uvar(32).expect("field tag"),
            u64::from(expected_tag)
        );
        fields.push(record.read_sized_payload().expect("field payload").to_vec());
    }
    record.check_finished().expect("record is exact");
    fields
}

fn parameter_schema_with_fields(kind: u32, fields: &[Vec<u8>]) -> Vec<u8> {
    let record_fields = fields
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            (
                u32::try_from(index + 1).expect("Parameter field index fits u32"),
                payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    sley_scb1::encode_union(
        kind,
        &sley_scb1::encode_record(&record_fields).expect("Parameter record encodes"),
    )
    .expect("Parameter body union encodes")
}

fn parameter_schema_error(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    body: &[u8],
) -> Vec<u8> {
    let outcome = execute_with_limits(
        package,
        approved,
        vec![bytes_input(body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("Parameter schema decoder must return a typed refusal")
    };
    let ConstData::Result(ResultConst::Err(error)) = value.data else {
        panic!("Parameter schema decoder must refuse malformed input: {value:?}")
    };
    let ConstData::Bytes(code) = error.data else {
        panic!("Parameter schema refusal must be Bytes")
    };
    code
}

#[test]
fn parameter_schema_decoder_accepts_arbitrary_structural_values() {
    let body = parameter_schema_body(
        ParameterRole::Block,
        u32::MAX,
        TypeExpr::Option(Box::new(TypeExpr::Tuple(vec![
            TypeExpr::Bool,
            TypeExpr::Named(sley_ssmc::NamedType {
                definition: EntityId::from_bytes([0xe3; 32]),
                arguments: vec![TypeExpr::Bytes],
            }),
        ]))),
    );
    let expected = exact_entity_body_fields(&body, 6, 4);
    let image = parameter_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "PARAMETER_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 15);
    assert_eq!(image.parameters.len(), 1_318);
    assert_eq!(image.blocks.len(), 319);
    assert_eq!(image.operations.len(), 582);
    assert_eq!(image.constants.len(), 185);
    assert_eq!(package.image_bytes.len(), 77_884);
    assert_eq!(
        approved.package_digest,
        [
            0x69, 0xba, 0x03, 0x03, 0x58, 0xdb, 0x08, 0x33, 0x22, 0x4d, 0x14, 0x9e, 0x2c, 0xe0,
            0x99, 0x5b, 0x8d, 0xea, 0xcc, 0xcf, 0xb2, 0x43, 0xb8, 0xfa, 0xd8, 0xcf, 0x1a, 0x28,
            0x56, 0x73, 0x1e, 0xd9,
        ]
    );
    let outcome = execute_with_limits(
        &package,
        &approved,
        vec![bytes_input(&body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("Parameter schema decoder must return")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("Parameter schema decoder must accept native body: {value:?}")
    };
    let ConstData::Sequence(fields) = decoded.data else {
        panic!("Parameter schema decoder must return a four-field tuple")
    };
    assert_eq!(fields.len(), 4);
    for (field, expected) in fields.iter().zip(expected) {
        assert_eq!(field.data, ConstData::Bytes(expected));
    }
}

#[test]
fn parameter_schema_decoder_rejects_every_field_boundary() {
    let body = parameter_schema_body(ParameterRole::Function, 7, TypeExpr::Bool);
    let fields = exact_entity_body_fields(&body, 6, 4);
    let image = parameter_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let mut unknown_fields = fields.clone();
    unknown_fields.push(Vec::new());
    let mut short_owner = fields.clone();
    short_owner[0] = vec![0xe2; 31];
    let mut invalid_role = fields.clone();
    invalid_role[1] = sley_scb1::encode_uvar(3);
    let mut wide_ordinal = fields.clone();
    wide_ordinal[2] = sley_scb1::encode_uvar(u64::from(u32::MAX) + 1);
    let mut invalid_type = fields.clone();
    invalid_type[3] = sley_scb1::encode_union(
        13,
        &sley_scb1::encode_union(21, &[]).expect("unknown nested TypeExpr encodes"),
    )
    .expect("Option TypeExpr encodes");

    let cases = [
        (
            "wrong_kind",
            parameter_schema_with_fields(5, &fields),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_field",
            parameter_schema_with_fields(6, &fields[..3]),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_field",
            parameter_schema_with_fields(6, &unknown_fields),
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "short_owner",
            parameter_schema_with_fields(6, &short_owner),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "invalid_role",
            parameter_schema_with_fields(6, &invalid_role),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "wide_ordinal",
            parameter_schema_with_fields(6, &wide_ordinal),
            b"SCB_INTEGER_OVERFLOW".as_slice(),
        ),
        (
            "invalid_type",
            parameter_schema_with_fields(6, &invalid_type),
            b"SCB_UNION_INVALID".as_slice(),
        ),
    ];
    for (name, malformed, expected) in cases {
        assert_eq!(
            parameter_schema_error(&package, &approved, &malformed),
            expected,
            "{name} precedence"
        );
    }
}
