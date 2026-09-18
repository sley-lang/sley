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

pub(super) fn build_dependency_binding_decode(
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
    let trailing_code = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let length_error = err_block(assembler, ns, function, result_type.clone(), length_code);
    let trailing_error = err_block(assembler, ns, function, result_type.clone(), trailing_code);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
    let success = assembler.id(ns.b);
    let vector_ready = assembler.id(ns.b);
    let not_exact = assembler.id(ns.b);

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

    // The native cursor takes 32 bytes then requires the payload finished:
    // a shorter payload is the array read overflowing its input, a longer
    // one leaves trailing bytes (rw-080-contract.md section 1.1 parity).
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
            edge(not_exact, vec![op_result(length)]),
        ),
    );
    let inexact_length = assembler.param(ns.p, not_exact, ParameterRole::Block, u64_type());
    let expected_again = assembler.cref(ns.o, not_exact, constant32, u64_type());
    let short = assembler.op(
        ns.o,
        not_exact,
        Opcode::LessThan,
        vec![pav(inexact_length), op_result(expected_again)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        not_exact,
        function,
        vec![inexact_length],
        vec![expected_again, short],
        cond(
            op_result(short),
            edge(length_error, Vec::new()),
            edge(trailing_error, Vec::new()),
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

/// Child projection: the node's first and second direct children, its
/// listed children, and the SCB1 depth offsets the native codec charges for
/// each slot (child depth = node depth + offset + 1) plus the deepest
/// mandatory container beneath the node (node depth + container offset must
/// stay below the 64-level bound even when the node has no children).
fn type_expr_children_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(children_tuple_type()),
        error: Box::new(TypeExpr::Bytes),
    }
}

fn children_tuple_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        TypeExpr::Option(Box::new(TypeExpr::Bytes)),
        TypeExpr::Option(Box::new(TypeExpr::Bytes)),
        generic_record_map_type(),
        u64_type(),
        u64_type(),
        u64_type(),
        u64_type(),
    ])
}

/// Depth offsets one projector success path reports: `(first, second, listed,
/// container)`; each child slot offset is the number of native containers
/// between the node and that child minus one (a direct child is 0), the
/// container offset names the deepest mandatory container beneath the node.
#[derive(Clone, Copy)]
struct DepthOffsets {
    first: u64,
    second: u64,
    listed: u64,
    container: u64,
}

impl DepthOffsets {
    const DIRECT: Self = Self {
        first: 0,
        second: 0,
        listed: 0,
        container: 0,
    };

    fn constants(self, assembler: &mut Asm, ns: Ns, block: EntityId) -> [EntityId; 4] {
        [self.first, self.second, self.listed, self.container].map(|value| {
            let constant = assembler.ku64(ns.k, u128::from(value));
            assembler.cref(ns.o, block, constant, u64_type())
        })
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
    let tuple_type = children_tuple_type();
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
            u64_type(),
            u64_type(),
            u64_type(),
            u64_type(),
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

    let offset_types = [u64_type(), u64_type(), u64_type(), u64_type()];
    let mut empty_types = vec![map_type.clone()];
    empty_types.extend(offset_types.iter().cloned());
    let empty_parameters = block_parameters(assembler, ns.p, empty_success, &empty_types);
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
                pav(empty_parameters[1]),
                pav(empty_parameters[2]),
                pav(empty_parameters[3]),
                pav(empty_parameters[4]),
            ],
        )),
    );

    let mut unary_types = vec![TypeExpr::Bytes, map_type.clone()];
    unary_types.extend(offset_types.iter().cloned());
    let unary_success_parameters = block_parameters(assembler, ns.p, unary_success, &unary_types);
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
                pav(unary_success_parameters[2]),
                pav(unary_success_parameters[3]),
                pav(unary_success_parameters[4]),
                pav(unary_success_parameters[5]),
            ],
        )),
    );

    let mut binary_types = vec![TypeExpr::Bytes, TypeExpr::Bytes, map_type.clone()];
    binary_types.extend(offset_types.iter().cloned());
    let binary_success_parameters =
        block_parameters(assembler, ns.p, binary_success, &binary_types);
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
                pav(binary_success_parameters[3]),
                pav(binary_success_parameters[4]),
                pav(binary_success_parameters[5]),
                pav(binary_success_parameters[6]),
            ],
        )),
    );

    let function_effects_parameters = block_parameters(
        assembler,
        ns.p,
        function_effects_ready,
        &[TypeExpr::Bytes, map_type.clone()],
    );
    // FunctionRef: the FunctionType record sits at d+1; `result` is a
    // field at d+2 (offset 1), `parameters` is a list at d+2 whose elements
    // are at d+3 (offset 2), and the mandatory `effects` set is at d+2.
    let function_offsets = DepthOffsets {
        first: 1,
        second: 0,
        listed: 2,
        container: 2,
    }
    .constants(assembler, ns, function_effects_ready);
    append_block(
        assembler,
        function_effects_ready,
        function,
        function_effects_parameters.clone(),
        function_offsets.to_vec(),
        branch(edge(
            unary_success,
            vec![
                pav(function_effects_parameters[0]),
                pav(function_effects_parameters[1]),
                op_result(function_offsets[0]),
                op_result(function_offsets[1]),
                op_result(function_offsets[2]),
                op_result(function_offsets[3]),
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
    // Named: the NamedType record sits at d+1, its `arguments` list at d+2
    // and each argument at d+3 (offset 2); the list is mandatory.
    let named_offsets = DepthOffsets {
        first: 0,
        second: 0,
        listed: 2,
        container: 2,
    }
    .constants(assembler, ns, named_definition_ready);
    let mut named_operations = vec![arguments];
    named_operations.extend(named_offsets);
    append_block(
        assembler,
        named_definition_ready,
        function,
        named_definition_parameters,
        named_operations,
        switch(
            op_result(arguments),
            vec![
                (
                    BuiltinCase::Ok,
                    empty_success,
                    vec![
                        SwitchArgument::CasePayload,
                        oav(named_offsets[0]),
                        oav(named_offsets[1]),
                        oav(named_offsets[2]),
                        oav(named_offsets[3]),
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
    // OrderedMap and Result: a two-field record sits at d+1 and both
    // children are its fields at d+2 (offset 1); the record is mandatory.
    let pair_offsets = DepthOffsets {
        first: 1,
        second: 1,
        listed: 0,
        container: 1,
    }
    .constants(assembler, ns, pair_fields_ready);
    let mut pair_operations = vec![first, second];
    pair_operations.extend(pair_offsets);
    append_block(
        assembler,
        pair_fields_ready,
        function,
        pair_fields_parameters.clone(),
        pair_operations,
        branch(edge(
            binary_success,
            vec![
                op_result(first),
                op_result(second),
                pav(pair_fields_parameters[1]),
                op_result(pair_offsets[0]),
                op_result(pair_offsets[1]),
                op_result(pair_offsets[2]),
                op_result(pair_offsets[3]),
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
    // Tuple: the element list sits at d+1 and each element at d+2.
    let tuple_offsets = DepthOffsets {
        first: 0,
        second: 0,
        listed: 1,
        container: 1,
    }
    .constants(assembler, ns, list_call);
    let mut list_operations = vec![list];
    list_operations.extend(tuple_offsets);
    append_block(
        assembler,
        list_call,
        function,
        list_parameters,
        list_operations,
        switch(
            op_result(list),
            vec![
                (
                    BuiltinCase::Ok,
                    empty_success,
                    vec![
                        SwitchArgument::CasePayload,
                        oav(tuple_offsets[0]),
                        oav(tuple_offsets[1]),
                        oav(tuple_offsets[2]),
                        oav(tuple_offsets[3]),
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

    let unary_parameters = block_parameters(
        assembler,
        ns.p,
        unary_ready,
        &[TypeExpr::Bytes, map_type.clone()],
    );
    // Vector, Option and LocalCell: the single child is the union payload
    // itself, one level down.
    let unary_offsets = DepthOffsets::DIRECT.constants(assembler, ns, unary_ready);
    append_block(
        assembler,
        unary_ready,
        function,
        unary_parameters.clone(),
        unary_offsets.to_vec(),
        branch(edge(
            unary_success,
            vec![
                pav(unary_parameters[0]),
                pav(unary_parameters[1]),
                op_result(unary_offsets[0]),
                op_result(unary_offsets[1]),
                op_result(unary_offsets[2]),
                op_result(unary_offsets[3]),
            ],
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
    // Leaves: a width or identity payload is a leaf value at d+1, which the
    // native codec admits up to the bound itself; no container lies below.
    let leaf_offsets = DepthOffsets::DIRECT.constants(assembler, ns, leaf_call);
    let mut leaf_operations = vec![leaf];
    leaf_operations.extend(leaf_offsets);
    append_block(
        assembler,
        leaf_call,
        function,
        leaf_parameters.clone(),
        leaf_operations,
        switch(
            op_result(leaf),
            vec![
                (
                    BuiltinCase::Ok,
                    empty_success,
                    vec![
                        sav(leaf_parameters[0]),
                        oav(leaf_offsets[0]),
                        oav(leaf_offsets[1]),
                        oav(leaf_offsets[2]),
                        oav(leaf_offsets[3]),
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
/// Worklist driver core over a child projector. `start_depth` is the SCB1
/// nesting depth of the root node in its enclosing object (native
/// `sley_mutate` charges the body union 0, the kind record 1, a direct field
/// 2, a list element 3, and so on); every child is one level deeper and a
/// node at depth 64 is `SCB_RESOURCE_LIMIT`, exactly the native container
/// bound. Callers reach the core through `build_type_expr_depth_entry`.
fn build_type_expr_recursive_core(
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
    let children_tuple_type = children_tuple_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let start_depth = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
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
    let container_gate = assembler.id(ns.b);
    let add_first = assembler.id(ns.b);
    let add_second = assembler.id(ns.b);
    let add_listed = assembler.id(ns.b);
    let direct_two_depth = assembler.id(ns.b);
    let list_init_depth = assembler.id(ns.b);
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
    // Each chain runs at its own effective depth (node depth plus the
    // slot's native offset), so the second and listed chains carry their
    // effective depths until a retarget block installs them.
    let append_two = build_type_expr_append_child(
        assembler,
        ns,
        function,
        resource_error,
        invariant_trap,
        list_init_depth,
        &[bytes_map_type.clone(), u64_type()],
    );
    let append_one = build_type_expr_append_child(
        assembler,
        ns,
        function,
        resource_error,
        invariant_trap,
        direct_two_depth,
        &[
            option_bytes_type.clone(),
            bytes_map_type.clone(),
            u64_type(),
            u64_type(),
        ],
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

    // list_init_depth: install the listed chain's effective depth.
    let mut list_depth_types = base_types.clone();
    list_depth_types.extend([bytes_map_type.clone(), u64_type()]);
    let list_depth_parameters =
        block_parameters(assembler, ns.p, list_init_depth, &list_depth_types);
    append_block(
        assembler,
        list_init_depth,
        function,
        list_depth_parameters.clone(),
        Vec::new(),
        branch(edge(
            list_init,
            vec![
                pav(list_depth_parameters[0]),
                pav(list_depth_parameters[1]),
                pav(list_depth_parameters[2]),
                pav(list_depth_parameters[3]),
                pav(list_depth_parameters[6]),
                pav(list_depth_parameters[5]),
            ],
        )),
    );

    let mut direct_two_types = base_types.clone();
    direct_two_types.extend([
        option_bytes_type.clone(),
        bytes_map_type.clone(),
        u64_type(),
    ]);
    let direct_two_parameters = block_parameters(assembler, ns.p, direct_two, &direct_two_types);
    let mut append_two_arguments = vec![SwitchArgument::CasePayload];
    append_two_arguments.extend(direct_two_parameters[..5].iter().copied().map(sav));
    append_two_arguments.push(sav(direct_two_parameters[6]));
    append_two_arguments.push(sav(direct_two_parameters[7]));
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
                    list_init_depth,
                    direct_two_parameters[..5]
                        .iter()
                        .copied()
                        .map(sav)
                        .chain([sav(direct_two_parameters[6]), sav(direct_two_parameters[7])])
                        .collect(),
                ),
                (BuiltinCase::Some, append_two, append_two_arguments),
            ],
        ),
    );

    // direct_two_depth: install the second chain's effective depth.
    let mut two_depth_types = base_types.clone();
    two_depth_types.extend([
        option_bytes_type.clone(),
        bytes_map_type.clone(),
        u64_type(),
        u64_type(),
    ]);
    let two_depth_parameters =
        block_parameters(assembler, ns.p, direct_two_depth, &two_depth_types);
    append_block(
        assembler,
        direct_two_depth,
        function,
        two_depth_parameters.clone(),
        Vec::new(),
        branch(edge(
            direct_two,
            vec![
                pav(two_depth_parameters[0]),
                pav(two_depth_parameters[1]),
                pav(two_depth_parameters[2]),
                pav(two_depth_parameters[3]),
                pav(two_depth_parameters[7]),
                pav(two_depth_parameters[5]),
                pav(two_depth_parameters[6]),
                pav(two_depth_parameters[8]),
            ],
        )),
    );

    let mut direct_one_types = base_types.clone();
    direct_one_types.extend([
        option_bytes_type.clone(),
        option_bytes_type.clone(),
        bytes_map_type.clone(),
        u64_type(),
        u64_type(),
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
                    direct_two_depth,
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

    // Effective depths: node depth plus each slot's native offset, computed
    // with checked adds (an overflow is impossible below the bound and traps).
    let child_types = [
        option_bytes_type.clone(),
        option_bytes_type.clone(),
        bytes_map_type.clone(),
    ];
    let mut add_listed_types = base_types.clone();
    add_listed_types.extend(child_types.iter().cloned());
    add_listed_types.extend([u64_type(), u64_type(), u64_type()]);
    let add_listed_parameters = block_parameters(assembler, ns.p, add_listed, &add_listed_types);
    let listed_depth = assembler.op(
        ns.o,
        add_listed,
        Opcode::IntAddChecked,
        vec![pav(add_listed_parameters[4]), pav(add_listed_parameters[8])],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        add_listed,
        function,
        add_listed_parameters.clone(),
        vec![listed_depth],
        switch(
            op_result(listed_depth),
            vec![
                (
                    BuiltinCase::Ok,
                    direct_one,
                    vec![
                        sav(add_listed_parameters[0]),
                        sav(add_listed_parameters[1]),
                        sav(add_listed_parameters[2]),
                        sav(add_listed_parameters[3]),
                        sav(add_listed_parameters[9]),
                        sav(add_listed_parameters[5]),
                        sav(add_listed_parameters[6]),
                        sav(add_listed_parameters[7]),
                        sav(add_listed_parameters[10]),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let mut add_second_types = base_types.clone();
    add_second_types.extend(child_types.iter().cloned());
    add_second_types.extend([u64_type(), u64_type(), u64_type()]);
    let add_second_parameters = block_parameters(assembler, ns.p, add_second, &add_second_types);
    let second_depth = assembler.op(
        ns.o,
        add_second,
        Opcode::IntAddChecked,
        vec![pav(add_second_parameters[4]), pav(add_second_parameters[8])],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        add_second,
        function,
        add_second_parameters.clone(),
        vec![second_depth],
        switch(
            op_result(second_depth),
            vec![
                (
                    BuiltinCase::Ok,
                    add_listed,
                    vec![
                        sav(add_second_parameters[0]),
                        sav(add_second_parameters[1]),
                        sav(add_second_parameters[2]),
                        sav(add_second_parameters[3]),
                        sav(add_second_parameters[4]),
                        sav(add_second_parameters[5]),
                        sav(add_second_parameters[6]),
                        sav(add_second_parameters[7]),
                        sav(add_second_parameters[9]),
                        sav(add_second_parameters[10]),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let mut add_first_types = base_types.clone();
    add_first_types.extend(child_types.iter().cloned());
    add_first_types.extend([u64_type(), u64_type(), u64_type()]);
    let add_first_parameters = block_parameters(assembler, ns.p, add_first, &add_first_types);
    let first_depth = assembler.op(
        ns.o,
        add_first,
        Opcode::IntAddChecked,
        vec![pav(add_first_parameters[4]), pav(add_first_parameters[8])],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        add_first,
        function,
        add_first_parameters.clone(),
        vec![first_depth],
        switch(
            op_result(first_depth),
            vec![
                (
                    BuiltinCase::Ok,
                    add_second,
                    vec![
                        sav(add_first_parameters[0]),
                        sav(add_first_parameters[1]),
                        sav(add_first_parameters[2]),
                        sav(add_first_parameters[3]),
                        sav(add_first_parameters[4]),
                        sav(add_first_parameters[5]),
                        sav(add_first_parameters[6]),
                        sav(add_first_parameters[7]),
                        sav(add_first_parameters[9]),
                        sav(add_first_parameters[10]),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    // container_gate: the deepest mandatory container beneath the node must
    // itself sit below the bound (native `check_container_depth`), even when
    // the node carries no children.
    let mut gate_types = base_types.clone();
    gate_types.extend(child_types.iter().cloned());
    gate_types.extend([u64_type(), u64_type(), u64_type(), u64_type()]);
    let gate_parameters = block_parameters(assembler, ns.p, container_gate, &gate_types);
    let bound_constant = assembler.ku64(ns.k, 64);
    let bound = assembler.cref(ns.o, container_gate, bound_constant, u64_type());
    let container_fits = assembler.op(
        ns.o,
        container_gate,
        Opcode::LessThan,
        vec![pav(gate_parameters[11]), op_result(bound)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        container_gate,
        function,
        gate_parameters.clone(),
        vec![bound, container_fits],
        cond(
            op_result(container_fits),
            edge(
                add_first,
                gate_parameters[..11].iter().copied().map(pav).collect(),
            ),
            edge(resource_error, Vec::new()),
        ),
    );

    let mut children_ready_types = base_types.clone();
    children_ready_types.push(children_tuple_type.clone());
    let children_ready_parameters =
        block_parameters(assembler, ns.p, children_ready, &children_ready_types);
    let tuple_field = |assembler: &mut Asm, index: u32, field_type: TypeExpr| {
        assembler.op(
            ns.o,
            children_ready,
            Opcode::TupleGet,
            vec![pav(children_ready_parameters[5])],
            vec![field_type],
            Immediate::Index(index),
        )
    };
    let first = tuple_field(assembler, 0, option_bytes_type.clone());
    let second = tuple_field(assembler, 1, option_bytes_type.clone());
    let listed = tuple_field(assembler, 2, bytes_map_type.clone());
    let first_offset = tuple_field(assembler, 3, u64_type());
    let second_offset = tuple_field(assembler, 4, u64_type());
    let listed_offset = tuple_field(assembler, 5, u64_type());
    let container_offset = tuple_field(assembler, 6, u64_type());
    let container_depth = assembler.op(
        ns.o,
        children_ready,
        Opcode::IntAddChecked,
        vec![
            pav(children_ready_parameters[4]),
            op_result(container_offset),
        ],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let mut gate_arguments = children_ready_parameters[..5]
        .iter()
        .copied()
        .map(sav)
        .collect::<Vec<_>>();
    gate_arguments.extend([
        oav(first),
        oav(second),
        oav(listed),
        oav(first_offset),
        oav(second_offset),
        oav(listed_offset),
        SwitchArgument::CasePayload,
    ]);
    append_block(
        assembler,
        children_ready,
        function,
        children_ready_parameters,
        vec![
            first,
            second,
            listed,
            first_offset,
            second_offset,
            listed_offset,
            container_offset,
            container_depth,
        ],
        switch(
            op_result(container_depth),
            vec![
                (BuiltinCase::Ok, container_gate, gate_arguments),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
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
            pav(start_depth),
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
        parameters: vec![body, start_depth, unit],
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

/// `(body, unit)` entry that runs the recursive core from a fixed nesting
/// depth, so one core serves every site a `TypeExpr` (or a `ConstValue`)
/// appears at while each site charges the depth the native codec charges.
fn build_type_expr_depth_entry(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    core: EntityId,
    depth: u64,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = bytes_validation_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let depth_constant = assembler.ku64(ns.k, u128::from(depth));
    let entry = assembler.id(ns.b);
    let depth_value = assembler.cref(ns.o, entry, depth_constant, u64_type());
    let decoded = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), op_result(depth_value), pav(unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: core,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![depth_value, decoded],
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

/// Recursive decoder reached through `function` at `start_depth`: the core
/// takes a fresh identity in the block namespace and is returned second.
fn build_type_expr_recursive_decode_at(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    children_decoder: EntityId,
    start_depth: u64,
) -> (EntityId, Vec<FunctionGraph>) {
    let core = assembler.id(ns.b);
    let core_graph = build_type_expr_recursive_core(assembler, ns, core, children_decoder);
    let entry_graph = build_type_expr_depth_entry(assembler, ns, function, core, start_depth);
    (core, vec![entry_graph, core_graph])
}

/// Standalone recursive decoder: the root node is at depth 0.
fn build_type_expr_recursive_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    children_decoder: EntityId,
) -> Vec<FunctionGraph> {
    build_type_expr_recursive_decode_at(assembler, ns, function, children_decoder, 0).1
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
    let graphs = build_type_expr_recursive_decode(
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
    let mut functions = graphs.clone();
    functions.extend([
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
    ]);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graphs[0].clone(),
        functions,
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
    let type_expr_recursive_graphs = build_type_expr_recursive_decode_at(
        &mut assembler,
        Ns {
            k: 214,
            p: 214,
            b: 214,
            o: 214,
        },
        type_expr_recursive_function,
        type_expr_children_function,
        2,
    )
    .1;
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
            type_expr_recursive_graphs[0].clone(),
            type_expr_recursive_graphs[1].clone(),
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
    let type_expr_recursive_graphs = build_type_expr_recursive_decode_at(
        &mut assembler,
        Ns {
            k: 229,
            p: 229,
            b: 229,
            o: 229,
        },
        type_expr_recursive_function,
        type_expr_children_function,
        2,
    )
    .1;
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
            type_expr_recursive_graphs[0].clone(),
            type_expr_recursive_graphs[1].clone(),
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

#[derive(Clone, Copy)]
enum SimpleFieldValidator {
    Fixed32,
    ExactUvar(u32),
    BoundedUvar { minimum: u64, maximum: u64 },
    TypeExpr,
    EntityIds { ordered: bool },
    Unit(EntityId),
    Bytes(EntityId),
}

#[derive(Clone, Copy)]
struct SimpleSchemaDecoders {
    union: EntityId,
    exact_record: EntityId,
    fixed32: EntityId,
    exact_uvar: EntityId,
    bounded_uvar: EntityId,
    type_expr: EntityId,
    entity_ids: EntityId,
}

#[allow(clippy::too_many_lines)]
fn build_unit_list_validate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    list_decoder: EntityId,
    element_decoder: EntityId,
    element_result_type: TypeExpr,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let map_type = generic_record_map_type();
    let option_bytes_type = TypeExpr::Option(Box::new(TypeExpr::Bytes));
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

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
    let invariant_trap = trap_block(assembler, ns, function);
    let success = assembler.id(ns.b);
    let advance = assembler.id(ns.b);
    let element_call = assembler.id(ns.b);
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

    let state_types = vec![map_type.clone(), u64_type()];
    let advance_parameters = block_parameters(assembler, ns.p, advance, &state_types);
    let one_constant = assembler.ku64(ns.k, 1);
    let one = assembler.cref(ns.o, advance, one_constant, u64_type());
    let next = assembler.op(
        ns.o,
        advance,
        Opcode::IntAddChecked,
        vec![pav(advance_parameters[1]), op_result(one)],
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
                    vec![sav(advance_parameters[0]), SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let element_types = vec![TypeExpr::Bytes, map_type.clone(), u64_type()];
    let element_parameters = block_parameters(assembler, ns.p, element_call, &element_types);
    let validated = assembler.op(
        ns.o,
        element_call,
        Opcode::CallDirect,
        vec![pav(element_parameters[0]), pav(unit)],
        vec![element_result_type],
        Immediate::Function(FunctionRefValue {
            function: element_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        element_call,
        function,
        element_parameters.clone(),
        vec![validated],
        switch(
            op_result(validated),
            vec![
                (
                    BuiltinCase::Ok,
                    advance,
                    vec![sav(element_parameters[1]), sav(element_parameters[2])],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let loop_parameters = block_parameters(assembler, ns.p, loop_check, &state_types);
    let item = assembler.op(
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
        vec![item],
        switch(
            op_result(item),
            vec![
                (BuiltinCase::None, success, Vec::new()),
                (
                    BuiltinCase::Some,
                    element_call,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(loop_parameters[0]),
                        sav(loop_parameters[1]),
                    ],
                ),
            ],
        ),
    );

    let list_parameters =
        block_parameters(assembler, ns.p, list_ready, std::slice::from_ref(&map_type));
    let zero_constant = assembler.ku64(ns.k, 0);
    let zero = assembler.cref(ns.o, list_ready, zero_constant, u64_type());
    append_block(
        assembler,
        list_ready,
        function,
        list_parameters.clone(),
        vec![zero],
        branch(edge(
            loop_check,
            vec![pav(list_parameters[0]), op_result(zero)],
        )),
    );

    let entry = assembler.id(ns.b);
    let decoded_list = assembler.op(
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
        vec![decoded_list],
        switch(
            op_result(decoded_list),
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
fn build_projected_record_validate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    validators: &[SimpleFieldValidator],
    decoders: SimpleSchemaDecoders,
) -> FunctionGraph {
    assert!(!validators.is_empty(), "projected record needs fields");
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let tuple_type = TypeExpr::Tuple(vec![TypeExpr::Bytes; validators.len()]);
    let field_types = vec![TypeExpr::Bytes; validators.len()];
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

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
    let success = assembler.id(ns.b);
    let validation_blocks = validators
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let record_ready = assembler.id(ns.b);

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

    for (index, (block, validator)) in validation_blocks
        .iter()
        .copied()
        .zip(validators.iter().copied())
        .enumerate()
    {
        let parameters = block_parameters(assembler, ns.p, block, &field_types);
        let mut operations = Vec::new();
        let (decode_function, decode_arguments, decode_result_type) = match validator {
            SimpleFieldValidator::Fixed32 => (
                decoders.fixed32,
                vec![pav(parameters[index]), pav(unit)],
                bytes_validation_result_type(),
            ),
            SimpleFieldValidator::ExactUvar(width) => {
                let width_constant = assembler.ku32(ns.k, u128::from(width));
                let width_value = assembler.cref(ns.o, block, width_constant, u32_type());
                operations.push(width_value);
                (
                    decoders.exact_uvar,
                    vec![pav(parameters[index]), op_result(width_value), pav(unit)],
                    exact_uvar_result_type(),
                )
            }
            SimpleFieldValidator::BoundedUvar { minimum, maximum } => {
                let minimum_constant = assembler.ku64(ns.k, u128::from(minimum));
                let maximum_constant = assembler.ku64(ns.k, u128::from(maximum));
                let minimum_value = assembler.cref(ns.o, block, minimum_constant, u64_type());
                let maximum_value = assembler.cref(ns.o, block, maximum_constant, u64_type());
                operations.extend([minimum_value, maximum_value]);
                (
                    decoders.bounded_uvar,
                    vec![
                        pav(parameters[index]),
                        op_result(minimum_value),
                        op_result(maximum_value),
                        pav(unit),
                    ],
                    exact_uvar_result_type(),
                )
            }
            SimpleFieldValidator::TypeExpr => (
                decoders.type_expr,
                vec![pav(parameters[index]), pav(unit)],
                bytes_validation_result_type(),
            ),
            SimpleFieldValidator::EntityIds { ordered } => {
                let ordered_constant = assembler.kbool(ns.k, ordered);
                let ordered_value = assembler.cref(ns.o, block, ordered_constant, TypeExpr::Bool);
                operations.push(ordered_value);
                (
                    decoders.entity_ids,
                    vec![pav(parameters[index]), op_result(ordered_value), pav(unit)],
                    unit_validation_result_type(),
                )
            }
            SimpleFieldValidator::Unit(decoder) => (
                decoder,
                vec![pav(parameters[index]), pav(unit)],
                unit_validation_result_type(),
            ),
            SimpleFieldValidator::Bytes(decoder) => (
                decoder,
                vec![pav(parameters[index]), pav(unit)],
                bytes_validation_result_type(),
            ),
        };
        let decoded = assembler.op(
            ns.o,
            block,
            Opcode::CallDirect,
            decode_arguments,
            vec![decode_result_type],
            Immediate::Function(FunctionRefValue {
                function: decode_function,
                type_arguments: Vec::new(),
            }),
        );
        operations.push(decoded);
        let next_validation = validation_blocks.get(index + 1).copied();
        let destination = next_validation.unwrap_or(success);
        let success_arguments = next_validation
            .map_or_else(Vec::new, |_| parameters.iter().copied().map(sav).collect());
        append_block(
            assembler,
            block,
            function,
            parameters.clone(),
            operations,
            switch(
                op_result(decoded),
                vec![
                    (BuiltinCase::Ok, destination, success_arguments),
                    (
                        BuiltinCase::Err,
                        forward_error,
                        vec![SwitchArgument::CasePayload],
                    ),
                ],
            ),
        );
    }

    let record_parameters = block_parameters(
        assembler,
        ns.p,
        record_ready,
        std::slice::from_ref(&tuple_type),
    );
    let projected_fields = (0..validators.len())
        .map(|index| {
            assembler.op(
                ns.o,
                record_ready,
                Opcode::TupleGet,
                vec![pav(record_parameters[0])],
                vec![TypeExpr::Bytes],
                Immediate::Index(u32::try_from(index).expect("record field index fits u32")),
            )
        })
        .collect::<Vec<_>>();
    append_block(
        assembler,
        record_ready,
        function,
        record_parameters,
        projected_fields.clone(),
        branch(edge(
            validation_blocks[0],
            projected_fields.iter().copied().map(op_result).collect(),
        )),
    );

    let entry = assembler.id(ns.b);
    let decoded_record = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![exact_record_projection_result_type(validators.len())],
        Immediate::Function(FunctionRefValue {
            function: decoders.exact_record,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
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
fn build_contract_source_validate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    union_decoder: EntityId,
    fixed32_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let union_type = TypeExpr::Tuple(vec![u64_type(), TypeExpr::Bytes]);
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
    let fixed_call = assembler.id(ns.b);
    let empty_check = assembler.id(ns.b);
    let tag_checks = std::array::from_fn::<_, 4, _>(|_| assembler.id(ns.b));
    let union_ready = assembler.id(ns.b);

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

    let fixed_parameters = block_parameters(assembler, ns.p, fixed_call, &[TypeExpr::Bytes]);
    let fixed = assembler.op(
        ns.o,
        fixed_call,
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
        fixed_call,
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

    let empty_parameters = block_parameters(assembler, ns.p, empty_check, &[TypeExpr::Bytes]);
    let empty_constant = assembler.kbytes(ns.k, b"");
    let empty = assembler.cref(ns.o, empty_check, empty_constant, TypeExpr::Bytes);
    let is_empty = assembler.op(
        ns.o,
        empty_check,
        Opcode::Equal,
        vec![pav(empty_parameters[0]), op_result(empty)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        empty_check,
        function,
        empty_parameters,
        vec![empty, is_empty],
        cond(
            op_result(is_empty),
            edge(success, Vec::new()),
            edge(union_error, Vec::new()),
        ),
    );

    for (index, block) in tag_checks.iter().copied().enumerate() {
        let parameters = block_parameters(assembler, ns.p, block, &[u64_type(), TypeExpr::Bytes]);
        let expected_constant = assembler.ku64(ns.k, (index + 1) as u128);
        let expected = assembler.cref(ns.o, block, expected_constant, u64_type());
        let tag_matches = assembler.op(
            ns.o,
            block,
            Opcode::Equal,
            vec![pav(parameters[0]), op_result(expected)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        let matched = if matches!(index, 0 | 3) {
            fixed_call
        } else {
            empty_check
        };
        let fallback = tag_checks.get(index + 1).copied().unwrap_or(union_error);
        let fallback_arguments = if index + 1 < tag_checks.len() {
            vec![pav(parameters[0]), pav(parameters[1])]
        } else {
            Vec::new()
        };
        append_block(
            assembler,
            block,
            function,
            parameters.clone(),
            vec![expected, tag_matches],
            cond(
                op_result(tag_matches),
                edge(matched, vec![pav(parameters[1])]),
                edge(fallback, fallback_arguments),
            ),
        );
    }

    let union_parameters = block_parameters(
        assembler,
        ns.p,
        union_ready,
        std::slice::from_ref(&union_type),
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
        branch(edge(
            tag_checks[0],
            vec![op_result(tag), op_result(payload)],
        )),
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
fn build_option_unit_validate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    union_decoder: EntityId,
    some_decoder: EntityId,
    some_result_type: TypeExpr,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let union_type = TypeExpr::Tuple(vec![u64_type(), TypeExpr::Bytes]);
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
    let some_call = assembler.id(ns.b);
    let none_check = assembler.id(ns.b);
    let union_ready = assembler.id(ns.b);

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

    let some_parameters = block_parameters(assembler, ns.p, some_call, &[TypeExpr::Bytes]);
    let some = assembler.op(
        ns.o,
        some_call,
        Opcode::CallDirect,
        vec![pav(some_parameters[0]), pav(unit)],
        vec![some_result_type],
        Immediate::Function(FunctionRefValue {
            function: some_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        some_call,
        function,
        some_parameters,
        vec![some],
        switch(
            op_result(some),
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

    let none_parameters =
        block_parameters(assembler, ns.p, none_check, &[u64_type(), TypeExpr::Bytes]);
    let zero_constant = assembler.ku64(ns.k, 0);
    let one_constant = assembler.ku64(ns.k, 1);
    let zero = assembler.cref(ns.o, none_check, zero_constant, u64_type());
    let one = assembler.cref(ns.o, none_check, one_constant, u64_type());
    let empty_constant = assembler.kbytes(ns.k, b"");
    let empty = assembler.cref(ns.o, none_check, empty_constant, TypeExpr::Bytes);
    let is_none = assembler.op(
        ns.o,
        none_check,
        Opcode::Equal,
        vec![pav(none_parameters[0]), op_result(zero)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let empty_payload = assembler.op(
        ns.o,
        none_check,
        Opcode::Equal,
        vec![pav(none_parameters[1]), op_result(empty)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let valid_none = assembler.op(
        ns.o,
        none_check,
        Opcode::BoolAnd,
        vec![op_result(is_none), op_result(empty_payload)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let is_some = assembler.op(
        ns.o,
        none_check,
        Opcode::Equal,
        vec![pav(none_parameters[0]), op_result(one)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let dispatch_some = assembler.id(ns.b);
    append_block(
        assembler,
        none_check,
        function,
        none_parameters.clone(),
        vec![
            zero,
            one,
            empty,
            is_none,
            empty_payload,
            valid_none,
            is_some,
        ],
        cond(
            op_result(valid_none),
            edge(success, Vec::new()),
            edge(
                dispatch_some,
                vec![op_result(is_some), pav(none_parameters[1])],
            ),
        ),
    );

    let dispatch_parameters = block_parameters(
        assembler,
        ns.p,
        dispatch_some,
        &[TypeExpr::Bool, TypeExpr::Bytes],
    );
    append_block(
        assembler,
        dispatch_some,
        function,
        dispatch_parameters.clone(),
        Vec::new(),
        cond(
            pav(dispatch_parameters[0]),
            edge(some_call, vec![pav(dispatch_parameters[1])]),
            edge(union_error, Vec::new()),
        ),
    );

    let union_parameters = block_parameters(
        assembler,
        ns.p,
        union_ready,
        std::slice::from_ref(&union_type),
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
        branch(edge(none_check, vec![op_result(tag), op_result(payload)])),
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
fn build_closed_union_validate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    union_decoder: EntityId,
    arms: &[(EntityId, TypeExpr)],
) -> FunctionGraph {
    assert!(!arms.is_empty(), "closed union needs arms");
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let union_type = TypeExpr::Tuple(vec![u64_type(), TypeExpr::Bytes]);
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
    let arm_calls = arms.iter().map(|_| assembler.id(ns.b)).collect::<Vec<_>>();
    let tag_checks = arms.iter().map(|_| assembler.id(ns.b)).collect::<Vec<_>>();
    let union_ready = assembler.id(ns.b);

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

    for (block, (arm, arm_result_type)) in arm_calls.iter().copied().zip(arms.iter().cloned()) {
        let parameters = block_parameters(assembler, ns.p, block, &[TypeExpr::Bytes]);
        let validated = assembler.op(
            ns.o,
            block,
            Opcode::CallDirect,
            vec![pav(parameters[0]), pav(unit)],
            vec![arm_result_type],
            Immediate::Function(FunctionRefValue {
                function: arm,
                type_arguments: Vec::new(),
            }),
        );
        append_block(
            assembler,
            block,
            function,
            parameters,
            vec![validated],
            switch(
                op_result(validated),
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

    for (index, block) in tag_checks.iter().copied().enumerate() {
        let parameters = block_parameters(assembler, ns.p, block, &[u64_type(), TypeExpr::Bytes]);
        let expected_constant = assembler.ku64(ns.k, (index + 1) as u128);
        let expected = assembler.cref(ns.o, block, expected_constant, u64_type());
        let tag_matches = assembler.op(
            ns.o,
            block,
            Opcode::Equal,
            vec![pav(parameters[0]), op_result(expected)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        let fallback = tag_checks.get(index + 1).copied().unwrap_or(union_error);
        let fallback_arguments = if index + 1 < tag_checks.len() {
            vec![pav(parameters[0]), pav(parameters[1])]
        } else {
            Vec::new()
        };
        append_block(
            assembler,
            block,
            function,
            parameters.clone(),
            vec![expected, tag_matches],
            cond(
                op_result(tag_matches),
                edge(arm_calls[index], vec![pav(parameters[1])]),
                edge(fallback, fallback_arguments),
            ),
        );
    }

    let union_parameters = block_parameters(
        assembler,
        ns.p,
        union_ready,
        std::slice::from_ref(&union_type),
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
        branch(edge(
            tag_checks[0],
            vec![op_result(tag), op_result(payload)],
        )),
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
fn build_optional_empty_unit_validate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    some_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

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
    let success = assembler.id(ns.b);
    let some_call = assembler.id(ns.b);

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

    let some_parameters = block_parameters(assembler, ns.p, some_call, &[TypeExpr::Bytes]);
    let some = assembler.op(
        ns.o,
        some_call,
        Opcode::CallDirect,
        vec![pav(some_parameters[0]), pav(unit)],
        vec![unit_validation_result_type()],
        Immediate::Function(FunctionRefValue {
            function: some_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        some_call,
        function,
        some_parameters,
        vec![some],
        switch(
            op_result(some),
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

    let entry = assembler.id(ns.b);
    let empty_constant = assembler.kbytes(ns.k, b"");
    let empty = assembler.cref(ns.o, entry, empty_constant, TypeExpr::Bytes);
    let is_empty = assembler.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(body), op_result(empty)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![empty, is_empty],
        cond(
            op_result(is_empty),
            edge(success, Vec::new()),
            edge(some_call, vec![pav(body)]),
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
fn build_optional_last_record_projection(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    record4_decoder: EntityId,
    record5_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = exact_record_projection_result_type(5);
    let tuple4_type = TypeExpr::Tuple(vec![TypeExpr::Bytes; 4]);
    let tuple5_type = TypeExpr::Tuple(vec![TypeExpr::Bytes; 5]);
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

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
    let five_success = assembler.id(ns.b);
    let four_success = assembler.id(ns.b);
    let call_four = assembler.id(ns.b);
    let five_error = assembler.id(ns.b);

    let five_parameters = block_parameters(
        assembler,
        ns.p,
        five_success,
        std::slice::from_ref(&tuple5_type),
    );
    let five_ok = assembler.op(
        ns.o,
        five_success,
        Opcode::ResultOk,
        vec![pav(five_parameters[0])],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        five_success,
        function,
        five_parameters,
        vec![five_ok],
        ret(op_result(five_ok)),
    );

    let four_parameters = block_parameters(
        assembler,
        ns.p,
        four_success,
        std::slice::from_ref(&tuple4_type),
    );
    let mut operations = Vec::new();
    let fields = (0..4)
        .map(|index| {
            let field = assembler.op(
                ns.o,
                four_success,
                Opcode::TupleGet,
                vec![pav(four_parameters[0])],
                vec![TypeExpr::Bytes],
                Immediate::Index(index),
            );
            operations.push(field);
            op_result(field)
        })
        .collect::<Vec<_>>();
    let empty_constant = assembler.kbytes(ns.k, b"");
    let empty = assembler.cref(ns.o, four_success, empty_constant, TypeExpr::Bytes);
    operations.push(empty);
    let mut tuple_fields = fields;
    tuple_fields.push(op_result(empty));
    let tuple = assembler.op(
        ns.o,
        four_success,
        Opcode::TupleNew,
        tuple_fields,
        vec![tuple5_type.clone()],
        Immediate::None,
    );
    let four_ok = assembler.op(
        ns.o,
        four_success,
        Opcode::ResultOk,
        vec![op_result(tuple)],
        vec![result_type.clone()],
        Immediate::None,
    );
    operations.extend([tuple, four_ok]);
    append_block(
        assembler,
        four_success,
        function,
        four_parameters,
        operations,
        ret(op_result(four_ok)),
    );

    let four_call = assembler.op(
        ns.o,
        call_four,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![exact_record_projection_result_type(4)],
        Immediate::Function(FunctionRefValue {
            function: record4_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        call_four,
        function,
        Vec::new(),
        vec![four_call],
        switch(
            op_result(four_call),
            vec![
                (
                    BuiltinCase::Ok,
                    four_success,
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

    let error_parameters = block_parameters(assembler, ns.p, five_error, &[TypeExpr::Bytes]);
    let missing_constant = assembler.kbytes(ns.k, b"SCB_FIELD_MISSING");
    let missing = assembler.cref(ns.o, five_error, missing_constant, TypeExpr::Bytes);
    let is_missing = assembler.op(
        ns.o,
        five_error,
        Opcode::Equal,
        vec![pav(error_parameters[0]), op_result(missing)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        five_error,
        function,
        error_parameters.clone(),
        vec![missing, is_missing],
        cond(
            op_result(is_missing),
            edge(call_four, Vec::new()),
            edge(forward_error, vec![pav(error_parameters[0])]),
        ),
    );

    let entry = assembler.id(ns.b);
    let five_call = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: record5_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![five_call],
        switch(
            op_result(five_call),
            vec![
                (
                    BuiltinCase::Ok,
                    five_success,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    five_error,
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

fn simple_schema_result_type(field_count: usize) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![TypeExpr::Bytes; field_count])),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_simple_entity_schema_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    expected_kind: u64,
    validators: &[SimpleFieldValidator],
    decoders: SimpleSchemaDecoders,
) -> FunctionGraph {
    assert!(!validators.is_empty(), "entity schema needs fields");
    let block_start = assembler.blocks.len();
    let result_type = simple_schema_result_type(validators.len());
    let tuple_type = TypeExpr::Tuple(vec![TypeExpr::Bytes; validators.len()]);
    let field_types = vec![TypeExpr::Bytes; validators.len()];
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
    let validation_blocks = validators
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let record_ready = assembler.id(ns.b);
    let record_call = assembler.id(ns.b);
    let union_ready = assembler.id(ns.b);

    let success_parameters = block_parameters(assembler, ns.p, success, &field_types);
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

    for (index, (block, validator)) in validation_blocks
        .iter()
        .copied()
        .zip(validators.iter().copied())
        .enumerate()
    {
        let parameters = block_parameters(assembler, ns.p, block, &field_types);
        let mut operations = Vec::new();
        let (decode_function, decode_arguments, decode_result_type) = match validator {
            SimpleFieldValidator::Fixed32 => (
                decoders.fixed32,
                vec![pav(parameters[index]), pav(unit)],
                bytes_validation_result_type(),
            ),
            SimpleFieldValidator::ExactUvar(width) => {
                let width_constant = assembler.ku32(ns.k, u128::from(width));
                let width_value = assembler.cref(ns.o, block, width_constant, u32_type());
                operations.push(width_value);
                (
                    decoders.exact_uvar,
                    vec![pav(parameters[index]), op_result(width_value), pav(unit)],
                    exact_uvar_result_type(),
                )
            }
            SimpleFieldValidator::BoundedUvar { minimum, maximum } => {
                let minimum_constant = assembler.ku64(ns.k, u128::from(minimum));
                let maximum_constant = assembler.ku64(ns.k, u128::from(maximum));
                let minimum_value = assembler.cref(ns.o, block, minimum_constant, u64_type());
                let maximum_value = assembler.cref(ns.o, block, maximum_constant, u64_type());
                operations.extend([minimum_value, maximum_value]);
                (
                    decoders.bounded_uvar,
                    vec![
                        pav(parameters[index]),
                        op_result(minimum_value),
                        op_result(maximum_value),
                        pav(unit),
                    ],
                    exact_uvar_result_type(),
                )
            }
            SimpleFieldValidator::TypeExpr => (
                decoders.type_expr,
                vec![pav(parameters[index]), pav(unit)],
                bytes_validation_result_type(),
            ),
            SimpleFieldValidator::EntityIds { ordered } => {
                let ordered_constant = assembler.kbool(ns.k, ordered);
                let ordered_value = assembler.cref(ns.o, block, ordered_constant, TypeExpr::Bool);
                operations.push(ordered_value);
                (
                    decoders.entity_ids,
                    vec![pav(parameters[index]), op_result(ordered_value), pav(unit)],
                    unit_validation_result_type(),
                )
            }
            SimpleFieldValidator::Unit(decoder) => (
                decoder,
                vec![pav(parameters[index]), pav(unit)],
                unit_validation_result_type(),
            ),
            SimpleFieldValidator::Bytes(decoder) => (
                decoder,
                vec![pav(parameters[index]), pav(unit)],
                bytes_validation_result_type(),
            ),
        };
        let decoded = assembler.op(
            ns.o,
            block,
            Opcode::CallDirect,
            decode_arguments,
            vec![decode_result_type],
            Immediate::Function(FunctionRefValue {
                function: decode_function,
                type_arguments: Vec::new(),
            }),
        );
        operations.push(decoded);
        let destination = validation_blocks.get(index + 1).copied().unwrap_or(success);
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

    let record_parameters = block_parameters(
        assembler,
        ns.p,
        record_ready,
        std::slice::from_ref(&tuple_type),
    );
    let projected_fields = (0..validators.len())
        .map(|index| {
            assembler.op(
                ns.o,
                record_ready,
                Opcode::TupleGet,
                vec![pav(record_parameters[0])],
                vec![TypeExpr::Bytes],
                Immediate::Index(u32::try_from(index).expect("schema field index fits u32")),
            )
        })
        .collect::<Vec<_>>();
    append_block(
        assembler,
        record_ready,
        function,
        record_parameters,
        projected_fields.clone(),
        branch(edge(
            validation_blocks[0],
            projected_fields.iter().copied().map(op_result).collect(),
        )),
    );

    let record_call_parameters = block_parameters(assembler, ns.p, record_call, &[TypeExpr::Bytes]);
    let decoded_record = assembler.op(
        ns.o,
        record_call,
        Opcode::CallDirect,
        vec![pav(record_call_parameters[0]), pav(unit)],
        vec![exact_record_projection_result_type(validators.len())],
        Immediate::Function(FunctionRefValue {
            function: decoders.exact_record,
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
    let kind_constant = assembler.ku64(ns.k, u128::from(expected_kind));
    let expected_tag = assembler.cref(ns.o, union_ready, kind_constant, u64_type());
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
            function: decoders.union,
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
fn simple_entity_schema_decode_image(
    expected_kind: u64,
    validators: &[SimpleFieldValidator],
) -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(232);
    let record_function = assembler.id(232);
    let union_function = assembler.id(232);
    let list_function = assembler.id(232);
    let fixed32_function = assembler.id(232);
    let entity_id_collection_function = assembler.id(232);
    let exact_uvar_function = assembler.id(232);
    let bounded_uvar_function = assembler.id(232);
    let type_expr_leaf_function = assembler.id(232);
    let record2_function = assembler.id(232);
    let record3_function = assembler.id(232);
    let type_expr_children_function = assembler.id(232);
    let type_expr_recursive_function = assembler.id(232);
    let extra_record_function = (!matches!(validators.len(), 2 | 3)).then(|| assembler.id(232));
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
    let type_expr_leaf_graph = build_type_expr_leaf_decode(
        &mut assembler,
        Ns {
            k: 241,
            p: 241,
            b: 241,
            o: 241,
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
    let type_expr_children_graph = build_type_expr_children_decode(
        &mut assembler,
        Ns {
            k: 244,
            p: 244,
            b: 244,
            o: 244,
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
    let type_expr_recursive_graphs = build_type_expr_recursive_decode_at(
        &mut assembler,
        Ns {
            k: 245,
            p: 245,
            b: 245,
            o: 245,
        },
        type_expr_recursive_function,
        type_expr_children_function,
        2,
    )
    .1;
    let (exact_record_function, extra_record_graph) = match validators.len() {
        2 => (record2_function, None),
        3 => (record3_function, None),
        field_count => {
            let schema_record_function =
                extra_record_function.expect("separate schema record allocated");
            (
                schema_record_function,
                Some(build_exact_record_projection(
                    &mut assembler,
                    Ns {
                        k: 246,
                        p: 246,
                        b: 246,
                        o: 246,
                    },
                    schema_record_function,
                    decode_function,
                    record_function,
                    field_count,
                )),
            )
        }
    };
    let graph = build_simple_entity_schema_decode(
        &mut assembler,
        Ns {
            k: 247,
            p: 247,
            b: 247,
            o: 247,
        },
        function,
        expected_kind,
        validators,
        SimpleSchemaDecoders {
            union: union_function,
            exact_record: exact_record_function,
            fixed32: fixed32_function,
            exact_uvar: exact_uvar_function,
            bounded_uvar: bounded_uvar_function,
            type_expr: type_expr_recursive_function,
            entity_ids: entity_id_collection_function,
        },
    );
    let mut functions = vec![graph.clone()];
    functions.extend(extra_record_graph);
    functions.extend(type_expr_recursive_graphs);
    functions.extend([
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
    ]);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph,
        functions,
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

fn global_value_schema_decode_image() -> Image {
    simple_entity_schema_decode_image(
        10,
        &[
            SimpleFieldValidator::TypeExpr,
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 4,
            },
        ],
    )
}

fn adapter_import_schema_decode_image() -> Image {
    simple_entity_schema_decode_image(
        15,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::ExactUvar(32),
            SimpleFieldValidator::TypeExpr,
            SimpleFieldValidator::TypeExpr,
            SimpleFieldValidator::TypeExpr,
            SimpleFieldValidator::EntityIds { ordered: true },
        ],
    )
}

fn effect_def_schema_decode_image() -> Image {
    simple_entity_schema_decode_image(
        11,
        &[
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 8,
            },
            SimpleFieldValidator::TypeExpr,
            SimpleFieldValidator::TypeExpr,
            SimpleFieldValidator::TypeExpr,
            SimpleFieldValidator::TypeExpr,
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 4,
            },
        ],
    )
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn contract_schema_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(160);
    let record_function = assembler.id(160);
    let union_function = assembler.id(160);
    let list_function = assembler.id(160);
    let fixed32_function = assembler.id(160);
    let exact_uvar_function = assembler.id(160);
    let bounded_uvar_function = assembler.id(160);
    let record2_function = assembler.id(160);
    let record4_function = assembler.id(160);
    let record5_function = assembler.id(160);
    let record6_function = assembler.id(160);
    let contract_source_function = assembler.id(160);
    let contract_binding_function = assembler.id(160);
    let contract_bindings_function = assembler.id(160);
    let resource_limits_function = assembler.id(160);
    let contract_projection_function = assembler.id(160);
    let optional_resource_limits_function = assembler.id(160);
    let function = assembler.id(160);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 161,
            p: 161,
            b: 161,
            o: 161,
        },
        decode_function,
    );
    let record_graph = build_generic_record_decode(
        &mut assembler,
        Ns {
            k: 162,
            p: 162,
            b: 162,
            o: 162,
        },
        record_function,
        decode_function,
    );
    let union_graph = build_generic_union_decode(
        &mut assembler,
        Ns {
            k: 163,
            p: 163,
            b: 163,
            o: 163,
        },
        union_function,
        decode_function,
    );
    let list_graph = build_generic_list_decode(
        &mut assembler,
        Ns {
            k: 164,
            p: 164,
            b: 164,
            o: 164,
        },
        list_function,
        decode_function,
    );
    let fixed32_graph = build_fixed32_decode(
        &mut assembler,
        Ns {
            k: 165,
            p: 165,
            b: 165,
            o: 165,
        },
        fixed32_function,
    );
    let exact_uvar_graph = build_exact_uvar_decode(
        &mut assembler,
        Ns {
            k: 166,
            p: 166,
            b: 166,
            o: 166,
        },
        exact_uvar_function,
        decode_function,
    );
    let bounded_uvar_graph = build_bounded_uvar_decode(
        &mut assembler,
        Ns {
            k: 167,
            p: 167,
            b: 167,
            o: 167,
        },
        bounded_uvar_function,
        exact_uvar_function,
    );
    let record2_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 168,
            p: 168,
            b: 168,
            o: 168,
        },
        record2_function,
        decode_function,
        record_function,
        2,
    );
    let record5_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 169,
            p: 169,
            b: 169,
            o: 169,
        },
        record5_function,
        decode_function,
        record_function,
        5,
    );
    let record4_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 176,
            p: 176,
            b: 176,
            o: 176,
        },
        record4_function,
        decode_function,
        record_function,
        4,
    );
    let record6_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 170,
            p: 170,
            b: 170,
            o: 170,
        },
        record6_function,
        decode_function,
        record_function,
        6,
    );
    let contract_source_graph = build_contract_source_validate(
        &mut assembler,
        Ns {
            k: 171,
            p: 171,
            b: 171,
            o: 171,
        },
        contract_source_function,
        union_function,
        fixed32_function,
    );
    let contract_binding_graph = build_projected_record_validate(
        &mut assembler,
        Ns {
            k: 172,
            p: 172,
            b: 172,
            o: 172,
        },
        contract_binding_function,
        &[
            SimpleFieldValidator::ExactUvar(32),
            SimpleFieldValidator::Unit(contract_source_function),
        ],
        SimpleSchemaDecoders {
            union: union_function,
            exact_record: record2_function,
            fixed32: fixed32_function,
            exact_uvar: exact_uvar_function,
            bounded_uvar: bounded_uvar_function,
            type_expr: fixed32_function,
            entity_ids: fixed32_function,
        },
    );
    let contract_bindings_graph = build_unit_list_validate(
        &mut assembler,
        Ns {
            k: 173,
            p: 173,
            b: 173,
            o: 173,
        },
        contract_bindings_function,
        list_function,
        contract_binding_function,
        unit_validation_result_type(),
    );
    let resource_limits_graph = build_projected_record_validate(
        &mut assembler,
        Ns {
            k: 174,
            p: 174,
            b: 174,
            o: 174,
        },
        resource_limits_function,
        &[SimpleFieldValidator::ExactUvar(64); 6],
        SimpleSchemaDecoders {
            union: union_function,
            exact_record: record6_function,
            fixed32: fixed32_function,
            exact_uvar: exact_uvar_function,
            bounded_uvar: bounded_uvar_function,
            type_expr: fixed32_function,
            entity_ids: fixed32_function,
        },
    );
    let contract_projection_graph = build_optional_last_record_projection(
        &mut assembler,
        Ns {
            k: 177,
            p: 177,
            b: 177,
            o: 177,
        },
        contract_projection_function,
        record4_function,
        record5_function,
    );
    let optional_resource_limits_graph = build_optional_empty_unit_validate(
        &mut assembler,
        Ns {
            k: 178,
            p: 178,
            b: 178,
            o: 178,
        },
        optional_resource_limits_function,
        resource_limits_function,
    );
    let graph = build_simple_entity_schema_decode(
        &mut assembler,
        Ns {
            k: 179,
            p: 179,
            b: 179,
            o: 179,
        },
        function,
        13,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 7,
            },
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(contract_bindings_function),
            SimpleFieldValidator::Unit(optional_resource_limits_function),
        ],
        SimpleSchemaDecoders {
            union: union_function,
            exact_record: contract_projection_function,
            fixed32: fixed32_function,
            exact_uvar: exact_uvar_function,
            bounded_uvar: bounded_uvar_function,
            type_expr: fixed32_function,
            entity_ids: fixed32_function,
        },
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            optional_resource_limits_graph,
            contract_projection_graph,
            resource_limits_graph,
            contract_bindings_graph,
            contract_binding_graph,
            contract_source_graph,
            record6_graph,
            record5_graph,
            record4_graph,
            record2_graph,
            bounded_uvar_graph,
            exact_uvar_graph,
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

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn type_def_schema_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(130);
    let record_function = assembler.id(130);
    let union_function = assembler.id(130);
    let list_function = assembler.id(130);
    let fixed32_function = assembler.id(130);
    let entity_id_collection_function = assembler.id(130);
    let exact_uvar_function = assembler.id(130);
    let bounded_uvar_function = assembler.id(130);
    let type_expr_leaf_function = assembler.id(130);
    let record2_function = assembler.id(130);
    let record3_function = assembler.id(130);
    let record4_function = assembler.id(130);
    let type_expr_children_function = assembler.id(130);
    let type_expr_recursive_function = assembler.id(130);
    let type_parameter_list_function = assembler.id(130);
    let record_field_function = assembler.id(130);
    let record_fields_function = assembler.id(130);
    let optional_payload_type_function = assembler.id(130);
    let variant_case_function = assembler.id(130);
    let variant_cases_function = assembler.id(130);
    let type_def_form_function = assembler.id(130);
    let function = assembler.id(130);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 131,
            p: 131,
            b: 131,
            o: 131,
        },
        decode_function,
    );
    let record_graph = build_generic_record_decode(
        &mut assembler,
        Ns {
            k: 132,
            p: 132,
            b: 132,
            o: 132,
        },
        record_function,
        decode_function,
    );
    let union_graph = build_generic_union_decode(
        &mut assembler,
        Ns {
            k: 133,
            p: 133,
            b: 133,
            o: 133,
        },
        union_function,
        decode_function,
    );
    let list_graph = build_generic_list_decode(
        &mut assembler,
        Ns {
            k: 134,
            p: 134,
            b: 134,
            o: 134,
        },
        list_function,
        decode_function,
    );
    let fixed32_graph = build_fixed32_decode(
        &mut assembler,
        Ns {
            k: 135,
            p: 135,
            b: 135,
            o: 135,
        },
        fixed32_function,
    );
    let entity_id_collection_graph = build_entity_id_collection_decode(
        &mut assembler,
        Ns {
            k: 136,
            p: 136,
            b: 136,
            o: 136,
        },
        entity_id_collection_function,
        list_function,
        fixed32_function,
    );
    let exact_uvar_graph = build_exact_uvar_decode(
        &mut assembler,
        Ns {
            k: 137,
            p: 137,
            b: 137,
            o: 137,
        },
        exact_uvar_function,
        decode_function,
    );
    let bounded_uvar_graph = build_bounded_uvar_decode(
        &mut assembler,
        Ns {
            k: 138,
            p: 138,
            b: 138,
            o: 138,
        },
        bounded_uvar_function,
        exact_uvar_function,
    );
    let type_expr_leaf_graph = build_type_expr_leaf_decode(
        &mut assembler,
        Ns {
            k: 139,
            p: 139,
            b: 139,
            o: 139,
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
            k: 140,
            p: 140,
            b: 140,
            o: 140,
        },
        record2_function,
        decode_function,
        record_function,
        2,
    );
    let record3_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 141,
            p: 141,
            b: 141,
            o: 141,
        },
        record3_function,
        decode_function,
        record_function,
        3,
    );
    let record4_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 142,
            p: 142,
            b: 142,
            o: 142,
        },
        record4_function,
        decode_function,
        record_function,
        4,
    );
    let type_expr_children_graph = build_type_expr_children_decode(
        &mut assembler,
        Ns {
            k: 143,
            p: 143,
            b: 143,
            o: 143,
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
    let type_expr_recursive_graphs = build_type_expr_recursive_decode_at(
        &mut assembler,
        Ns {
            k: 144,
            p: 144,
            b: 144,
            o: 144,
        },
        type_expr_recursive_function,
        type_expr_children_function,
        5,
    )
    .1;
    let type_parameter_list_graph = build_type_parameter_list_decode(
        &mut assembler,
        Ns {
            k: 145,
            p: 145,
            b: 145,
            o: 145,
        },
        type_parameter_list_function,
        list_function,
        record_function,
        exact_uvar_function,
    );
    let nested_decoders = SimpleSchemaDecoders {
        union: union_function,
        exact_record: record3_function,
        fixed32: fixed32_function,
        exact_uvar: exact_uvar_function,
        bounded_uvar: bounded_uvar_function,
        type_expr: type_expr_recursive_function,
        entity_ids: entity_id_collection_function,
    };
    let record_field_graph = build_projected_record_validate(
        &mut assembler,
        Ns {
            k: 146,
            p: 146,
            b: 146,
            o: 146,
        },
        record_field_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::TypeExpr,
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 4,
            },
        ],
        nested_decoders,
    );
    let record_fields_graph = build_unit_list_validate(
        &mut assembler,
        Ns {
            k: 147,
            p: 147,
            b: 147,
            o: 147,
        },
        record_fields_function,
        list_function,
        record_field_function,
        unit_validation_result_type(),
    );
    let optional_payload_type_graph = build_option_unit_validate(
        &mut assembler,
        Ns {
            k: 148,
            p: 148,
            b: 148,
            o: 148,
        },
        optional_payload_type_function,
        union_function,
        type_expr_recursive_function,
        bytes_validation_result_type(),
    );
    let variant_case_graph = build_projected_record_validate(
        &mut assembler,
        Ns {
            k: 149,
            p: 149,
            b: 149,
            o: 149,
        },
        variant_case_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(optional_payload_type_function),
        ],
        SimpleSchemaDecoders {
            exact_record: record2_function,
            ..nested_decoders
        },
    );
    let variant_cases_graph = build_unit_list_validate(
        &mut assembler,
        Ns {
            k: 150,
            p: 150,
            b: 150,
            o: 150,
        },
        variant_cases_function,
        list_function,
        variant_case_function,
        unit_validation_result_type(),
    );
    let type_def_form_graph = build_closed_union_validate(
        &mut assembler,
        Ns {
            k: 151,
            p: 151,
            b: 151,
            o: 151,
        },
        type_def_form_function,
        union_function,
        &[
            (record_fields_function, unit_validation_result_type()),
            (variant_cases_function, unit_validation_result_type()),
        ],
    );
    let graph = build_simple_entity_schema_decode(
        &mut assembler,
        Ns {
            k: 152,
            p: 152,
            b: 152,
            o: 152,
        },
        function,
        4,
        &[
            SimpleFieldValidator::Unit(type_parameter_list_function),
            SimpleFieldValidator::Unit(type_def_form_function),
            SimpleFieldValidator::EntityIds { ordered: true },
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 4,
            },
        ],
        SimpleSchemaDecoders {
            exact_record: record4_function,
            ..nested_decoders
        },
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            type_def_form_graph,
            variant_cases_graph,
            variant_case_graph,
            optional_payload_type_graph,
            record_fields_graph,
            record_field_graph,
            type_parameter_list_graph,
            type_expr_recursive_graphs[0].clone(),
            type_expr_recursive_graphs[1].clone(),
            type_expr_children_graph,
            record4_graph,
            record3_graph,
            record2_graph,
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
    assert_eq!(image.functions.len(), 16);
    assert_eq!(image.parameters.len(), 1_520);
    assert_eq!(image.blocks.len(), 343);
    assert_eq!(image.operations.len(), 641);
    assert_eq!(image.constants.len(), 218);
    assert_eq!(package.image_bytes.len(), 87_376);
    assert_eq!(
        approved.package_digest,
        [
            0xbe, 0xc7, 0x3d, 0x3c, 0xb2, 0x9f, 0xc1, 0x7c, 0xe4, 0xf0, 0x37, 0x8c, 0x7c, 0x0d,
            0x79, 0xee, 0x6c, 0x68, 0xcd, 0x96, 0x26, 0x97, 0x58, 0xff, 0x0d, 0xb5, 0x8c, 0x42,
            0x09, 0x84, 0xbd, 0x64,
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
    assert_eq!(image.parameters.len(), 539);
    assert_eq!(image.blocks.len(), 98);
    assert_eq!(image.operations.len(), 184);
    assert_eq!(image.constants.len(), 51);
    assert_eq!(package.image_bytes.len(), 26_388);
    assert_eq!(
        approved.package_digest,
        [
            0xf2, 0xe1, 0xa0, 0xb1, 0xe2, 0x80, 0x7a, 0x5b, 0xce, 0x41, 0x80, 0x89, 0x72, 0x01,
            0x31, 0x4d, 0x54, 0xef, 0x51, 0x35, 0x68, 0xf4, 0x4a, 0x28, 0x8c, 0x8f, 0x03, 0x42,
            0x17, 0x3c, 0xbf, 0x52,
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
    assert_eq!(image.parameters.len(), 1_144);
    assert_eq!(image.blocks.len(), 268);
    assert_eq!(image.operations.len(), 507);
    assert_eq!(image.constants.len(), 176);
    assert_eq!(package.image_bytes.len(), 66_586);
    assert_eq!(
        approved.package_digest,
        [
            0x5f, 0x7e, 0xf0, 0x43, 0x5d, 0x61, 0xb0, 0x3f, 0x9a, 0xec, 0xbb, 0xe4, 0x4e, 0x84,
            0x3f, 0x6b, 0xf7, 0x03, 0xe5, 0x69, 0x7f, 0x02, 0x84, 0x7d, 0x0c, 0x71, 0xd3, 0xfc,
            0x05, 0x84, 0x1c, 0xa5,
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
    assert_eq!(image.functions.len(), 14);
    assert_eq!(image.parameters.len(), 1_360);
    assert_eq!(image.blocks.len(), 302);
    assert_eq!(image.operations.len(), 566);
    assert_eq!(image.constants.len(), 193);
    assert_eq!(package.image_bytes.len(), 76_742);
    assert_eq!(
        approved.package_digest,
        [
            0xdc, 0xb7, 0x69, 0x74, 0xfd, 0xa2, 0x29, 0xf3, 0xf1, 0x20, 0x85, 0xe5, 0x1d, 0x38,
            0xe1, 0x12, 0xbf, 0x47, 0x40, 0xa1, 0x81, 0xa1, 0x41, 0x0d, 0xb7, 0x9b, 0xac, 0xa8,
            0x2b, 0x54, 0x57, 0x21,
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
    assert_eq!(image.parameters.len(), 517);
    assert_eq!(image.blocks.len(), 124);
    assert_eq!(image.operations.len(), 240);
    assert_eq!(image.constants.len(), 76);
    assert_eq!(package.image_bytes.len(), 30_538);
    assert_eq!(
        approved.package_digest,
        [
            0xb6, 0xf2, 0xd8, 0x0e, 0x65, 0x7a, 0x15, 0x48, 0x9e, 0xdc, 0x97, 0xd6, 0xfd, 0xa1,
            0xb8, 0x8a, 0x11, 0x00, 0x7d, 0x48, 0xfe, 0x01, 0x90, 0x05, 0xea, 0x24, 0x43, 0xa3,
            0x6d, 0x6e, 0x0c, 0xc7,
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
    simple_schema_error(package, approved, body, "Parameter")
}

/// Native refusal parity for one malformed entity body: the body is wrapped
/// in a canonical stored object and handed to `sley_mutate::import_entity_object`,
/// whose code must equal the code the Sley decoder returned. A pinned
/// expectation that the native codec does not share is a decoder deviation,
/// not a fixture detail (rw-080-contract.md section 1.1: identical rejection
/// codes).
fn assert_native_body_parity(malformed_body: &[u8], expected: &[u8], name: &str) {
    if name == "wrong_kind" || name == "wrong_entity_kind" {
        // A declared-kind mismatch is a dispatch precondition: the caller
        // names the kind and the body's union tag must agree. The native
        // codec has no declared kind (it reads the tag), so it decodes the
        // body as the tag's kind and reports that kind's first field
        // failure instead; the case has no native counterpart.
        return;
    }
    let stored = super::all_kind_digest_dispatch::stored_from_body([0xce; 32], malformed_body);
    let native = match sley_mutate::import_entity_object(program_epoch9(), &stored) {
        Ok(_) => "OK".to_owned(),
        Err(error) => error.code().to_string(),
    };
    assert_eq!(
        native.as_bytes(),
        expected,
        "{name}: native refusal code differs from the Sley decoder's"
    );
}

fn simple_schema_error(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    body: &[u8],
    schema_name: &str,
) -> Vec<u8> {
    let outcome = execute_with_limits(
        package,
        approved,
        vec![bytes_input(body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("{schema_name} schema decoder must return a typed refusal")
    };
    let ConstData::Result(ResultConst::Err(error)) = value.data else {
        panic!("{schema_name} schema decoder must refuse malformed input: {value:?}")
    };
    let ConstData::Bytes(code) = error.data else {
        panic!("{schema_name} schema refusal must be Bytes")
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
    assert_eq!(image.functions.len(), 16);
    assert_eq!(image.parameters.len(), 1_411);
    assert_eq!(image.blocks.len(), 328);
    assert_eq!(image.operations.len(), 622);
    assert_eq!(image.constants.len(), 212);
    assert_eq!(package.image_bytes.len(), 83_038);
    assert_eq!(
        approved.package_digest,
        [
            0x8f, 0xbb, 0x0b, 0x75, 0x42, 0x55, 0x80, 0xed, 0x87, 0x52, 0x9d, 0x14, 0xe8, 0xc8,
            0xe4, 0x2e, 0xd7, 0x8e, 0x0f, 0x03, 0x18, 0x6e, 0xc7, 0xb2, 0x21, 0x07, 0x59, 0x1e,
            0x67, 0xb4, 0xce, 0x1d,
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
        assert_native_body_parity(&malformed, expected, name);
    }
}

fn global_value_schema_body(value_type: TypeExpr, visibility: Visibility) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, GlobalValueBody};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0xd1; 32]),
        body: EntityBodyValue::GlobalValue(GlobalValueBody {
            value_type,
            initializer: EntityId::from_bytes([0xd2; 32]),
            visibility,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema GlobalValue fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

#[test]
fn global_value_schema_decoder_accepts_arbitrary_structural_values() {
    let body = global_value_schema_body(
        TypeExpr::Result {
            ok: Box::new(TypeExpr::Vector(Box::new(TypeExpr::Bytes))),
            error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Capability)),
        },
        Visibility::Exported,
    );
    let expected = exact_entity_body_fields(&body, 10, 3);
    let image = global_value_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "GLOBAL_VALUE_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 15);
    assert_eq!(image.parameters.len(), 1_378);
    assert_eq!(image.blocks.len(), 312);
    assert_eq!(image.operations.len(), 585);
    assert_eq!(image.constants.len(), 197);
    assert_eq!(package.image_bytes.len(), 79_008);
    assert_eq!(
        approved.package_digest,
        [
            0xbd, 0x84, 0xf9, 0x73, 0x10, 0x2d, 0xed, 0x92, 0x85, 0xbe, 0x9f, 0x25, 0xec, 0x50,
            0x89, 0x49, 0x10, 0xb2, 0xad, 0x14, 0xd6, 0x5c, 0x23, 0x9d, 0xa5, 0xb1, 0x13, 0xda,
            0xac, 0x2e, 0x7c, 0xe1,
        ]
    );
    let outcome = execute_with_limits(
        &package,
        &approved,
        vec![bytes_input(&body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("GlobalValue schema decoder must return")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("GlobalValue schema decoder must accept native body: {value:?}")
    };
    let ConstData::Sequence(fields) = decoded.data else {
        panic!("GlobalValue schema decoder must return a three-field tuple")
    };
    assert_eq!(fields.len(), 3);
    for (field, expected) in fields.iter().zip(expected) {
        assert_eq!(field.data, ConstData::Bytes(expected));
    }
}

#[test]
fn global_value_schema_decoder_rejects_every_field_boundary() {
    let body = global_value_schema_body(TypeExpr::Bool, Visibility::Workspace);
    let fields = exact_entity_body_fields(&body, 10, 3);
    let image = global_value_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let mut unknown_fields = fields.clone();
    unknown_fields.push(Vec::new());
    let mut invalid_type = fields.clone();
    invalid_type[0] = sley_scb1::encode_union(21, &[]).expect("unknown TypeExpr encodes");
    let mut short_initializer = fields.clone();
    short_initializer[1] = vec![0xd2; 31];
    let mut invalid_visibility = fields.clone();
    invalid_visibility[2] = sley_scb1::encode_uvar(5);

    let cases = [
        (
            "wrong_kind",
            parameter_schema_with_fields(9, &fields),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_field",
            parameter_schema_with_fields(10, &fields[..2]),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_field",
            parameter_schema_with_fields(10, &unknown_fields),
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "invalid_type",
            parameter_schema_with_fields(10, &invalid_type),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "short_initializer",
            parameter_schema_with_fields(10, &short_initializer),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "invalid_visibility",
            parameter_schema_with_fields(10, &invalid_visibility),
            b"SCB_UNION_INVALID".as_slice(),
        ),
    ];
    for (name, malformed, expected) in cases {
        assert_eq!(
            simple_schema_error(&package, &approved, &malformed, "GlobalValue"),
            expected,
            "{name} precedence"
        );
        assert_native_body_parity(&malformed, expected, name);
    }
}

fn adapter_import_schema_body() -> Vec<u8> {
    use sley_mutate::value::{AdapterImportBody, EntityBodyValue, EntityIdSet};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0xc1; 32]),
        body: EntityBodyValue::AdapterImport(AdapterImportBody {
            adapter_id: [0xc2; 32],
            abi_version: u32::MAX,
            request_type: TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Text]),
            response_type: TypeExpr::Vector(Box::new(TypeExpr::Bool)),
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::ContractViolation),
            effects: EntityIdSet::from_unsorted(vec![
                EntityId::from_bytes([0xc3; 32]),
                EntityId::from_bytes([0xc4; 32]),
            ])
            .expect("adapter effects are canonical"),
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema AdapterImport fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

#[test]
fn adapter_import_schema_decoder_accepts_arbitrary_structural_values() {
    let body = adapter_import_schema_body();
    let expected = exact_entity_body_fields(&body, 15, 6);
    let image = adapter_import_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "ADAPTER_IMPORT_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 16);
    assert_eq!(image.parameters.len(), 1_446);
    assert_eq!(image.blocks.len(), 332);
    assert_eq!(image.operations.len(), 633);
    assert_eq!(image.constants.len(), 215);
    assert_eq!(package.image_bytes.len(), 84_704);
    assert_eq!(
        approved.package_digest,
        [
            0x5f, 0xdf, 0xce, 0x72, 0xe1, 0x16, 0x85, 0xac, 0x28, 0xba, 0x2b, 0x42, 0x7e, 0x9f,
            0x44, 0x19, 0x11, 0x10, 0xb7, 0x0f, 0x67, 0x59, 0x1a, 0x6f, 0xbd, 0x4d, 0x37, 0x30,
            0xdc, 0x03, 0x58, 0x29,
        ]
    );
    let outcome = execute_with_limits(
        &package,
        &approved,
        vec![bytes_input(&body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("AdapterImport schema decoder must return")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("AdapterImport schema decoder must accept native body: {value:?}")
    };
    let ConstData::Sequence(fields) = decoded.data else {
        panic!("AdapterImport schema decoder must return a six-field tuple")
    };
    assert_eq!(fields.len(), 6);
    for (field, expected) in fields.iter().zip(expected) {
        assert_eq!(field.data, ConstData::Bytes(expected));
    }
}

#[test]
fn adapter_import_schema_decoder_rejects_every_field_boundary() {
    let body = adapter_import_schema_body();
    let fields = exact_entity_body_fields(&body, 15, 6);
    let image = adapter_import_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let mut unknown_fields = fields.clone();
    unknown_fields.push(Vec::new());
    let mut short_adapter = fields.clone();
    short_adapter[0] = vec![0xc2; 31];
    let mut wide_abi = fields.clone();
    wide_abi[1] = sley_scb1::encode_uvar(u64::from(u32::MAX) + 1);
    let mut invalid_request = fields.clone();
    invalid_request[2] = sley_scb1::encode_union(21, &[]).expect("unknown TypeExpr encodes");
    let mut unordered_effects = fields.clone();
    unordered_effects[5] = sley_scb1::encode_list(&[vec![0xc4; 32], vec![0xc3; 32]])
        .expect("unordered effect list encodes structurally");

    let cases = [
        (
            "wrong_kind",
            parameter_schema_with_fields(14, &fields),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_field",
            parameter_schema_with_fields(15, &fields[..5]),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_field",
            parameter_schema_with_fields(15, &unknown_fields),
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "short_adapter",
            parameter_schema_with_fields(15, &short_adapter),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "wide_abi",
            parameter_schema_with_fields(15, &wide_abi),
            b"SCB_INTEGER_OVERFLOW".as_slice(),
        ),
        (
            "invalid_request",
            parameter_schema_with_fields(15, &invalid_request),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "unordered_effects",
            parameter_schema_with_fields(15, &unordered_effects),
            b"SCB_MAP_ORDER".as_slice(),
        ),
    ];
    for (name, malformed, expected) in cases {
        assert_eq!(
            simple_schema_error(&package, &approved, &malformed, "AdapterImport"),
            expected,
            "{name} precedence"
        );
        assert_native_body_parity(&malformed, expected, name);
    }
}

fn effect_def_schema_body() -> Vec<u8> {
    use sley_mutate::value::{EffectDefBody, EntityBodyValue};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0xb1; 32]),
        body: EntityBodyValue::EffectDef(EffectDefBody {
            effect_kind: sley_ssmc::EffectKind::RandomRead,
            scope_type: TypeExpr::Named(sley_ssmc::NamedType {
                definition: EntityId::from_bytes([0xb2; 32]),
                arguments: vec![TypeExpr::Text],
            }),
            request_type: TypeExpr::Tuple(vec![TypeExpr::UInt(IntegerWidth::from_bits(64))]),
            response_type: TypeExpr::Vector(Box::new(TypeExpr::Bytes)),
            failure_type: TypeExpr::Result {
                ok: Box::new(TypeExpr::Unit),
                error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Capability)),
            },
            visibility: Visibility::Workspace,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema EffectDef fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

#[test]
fn effect_def_schema_decoder_accepts_arbitrary_structural_values() {
    let body = effect_def_schema_body();
    let expected = exact_entity_body_fields(&body, 11, 6);
    let image = effect_def_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "EFFECT_DEF_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 16);
    assert_eq!(image.parameters.len(), 1_446);
    assert_eq!(image.blocks.len(), 332);
    assert_eq!(image.operations.len(), 635);
    assert_eq!(image.constants.len(), 217);
    assert_eq!(package.image_bytes.len(), 84_856);
    assert_eq!(
        approved.package_digest,
        [
            0x7e, 0x19, 0x10, 0xfd, 0x14, 0xda, 0x64, 0x5d, 0x46, 0xbe, 0x7e, 0x2a, 0x31, 0x4b,
            0x66, 0x6a, 0x7f, 0xa6, 0x1b, 0x2e, 0x1e, 0x3e, 0x1e, 0x3b, 0xd4, 0x66, 0xbb, 0x6d,
            0x71, 0x02, 0x75, 0x05,
        ]
    );
    let outcome = execute_with_limits(
        &package,
        &approved,
        vec![bytes_input(&body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("EffectDef schema decoder must return")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("EffectDef schema decoder must accept native body: {value:?}")
    };
    let ConstData::Sequence(fields) = decoded.data else {
        panic!("EffectDef schema decoder must return a six-field tuple")
    };
    assert_eq!(fields.len(), 6);
    for (field, expected) in fields.iter().zip(expected) {
        assert_eq!(field.data, ConstData::Bytes(expected));
    }
}

#[test]
fn effect_def_schema_decoder_rejects_every_field_boundary() {
    let body = effect_def_schema_body();
    let fields = exact_entity_body_fields(&body, 11, 6);
    let image = effect_def_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let mut unknown_fields = fields.clone();
    unknown_fields.push(Vec::new());
    let mut invalid_kind = fields.clone();
    invalid_kind[0] = sley_scb1::encode_uvar(9);
    let mut invalid_scope = fields.clone();
    invalid_scope[1] = sley_scb1::encode_union(21, &[]).expect("unknown TypeExpr encodes");
    let mut invalid_visibility = fields.clone();
    invalid_visibility[5] = sley_scb1::encode_uvar(0);

    let cases = [
        (
            "wrong_entity_kind",
            parameter_schema_with_fields(10, &fields),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_field",
            parameter_schema_with_fields(11, &fields[..5]),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_field",
            parameter_schema_with_fields(11, &unknown_fields),
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "invalid_effect_kind",
            parameter_schema_with_fields(11, &invalid_kind),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "invalid_scope",
            parameter_schema_with_fields(11, &invalid_scope),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "invalid_visibility",
            parameter_schema_with_fields(11, &invalid_visibility),
            b"SCB_UNION_INVALID".as_slice(),
        ),
    ];
    for (name, malformed, expected) in cases {
        assert_eq!(
            simple_schema_error(&package, &approved, &malformed, "EffectDef"),
            expected,
            "{name} precedence"
        );
        assert_native_body_parity(&malformed, expected, name);
    }
}

fn contract_schema_body(include_resource_limits: bool) -> Vec<u8> {
    use sley_mutate::value::{ContractBody, EntityBodyValue};
    use sley_ssmc::{ContractBinding, ContractKind, ContractSource, ResourceLimits};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0xa1; 32]),
        body: EntityBodyValue::Contract(ContractBody {
            target: EntityId::from_bytes([0xa2; 32]),
            contract_kind: ContractKind::ResourceCeiling,
            predicate: EntityId::from_bytes([0xa3; 32]),
            bindings: vec![
                ContractBinding {
                    predicate_parameter: 0,
                    source: ContractSource::Parameter(EntityId::from_bytes([0xa4; 32])),
                },
                ContractBinding {
                    predicate_parameter: 1,
                    source: ContractSource::Result,
                },
                ContractBinding {
                    predicate_parameter: u32::MAX,
                    source: ContractSource::Global(EntityId::from_bytes([0xa5; 32])),
                },
            ],
            resource_limits: include_resource_limits.then_some(ResourceLimits {
                fuel: 99,
                memory_bytes: 1,
                output_bytes: 2,
                effect_count: 3,
                call_depth: 4,
                wall_timeout_millis: 5,
            }),
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema Contract fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

#[test]
fn contract_schema_decoder_accepts_arbitrary_structural_values() {
    let body = contract_schema_body(true);
    let expected = exact_entity_body_fields(&body, 13, 5);
    let image = contract_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "CONTRACT_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 18);
    assert_eq!(image.parameters.len(), 1_143);
    assert_eq!(image.blocks.len(), 273);
    assert_eq!(image.operations.len(), 514);
    assert_eq!(image.constants.len(), 154);
    assert_eq!(package.image_bytes.len(), 68_412);
    assert_eq!(
        approved.package_digest,
        [
            0x9e, 0xdc, 0xfd, 0x61, 0x3a, 0xa2, 0x58, 0x2f, 0x52, 0x1e, 0x2f, 0xc5, 0x2d, 0x52,
            0x4f, 0x1c, 0x34, 0x4b, 0x99, 0x9d, 0xc1, 0x65, 0x1a, 0xac, 0x87, 0x5f, 0xdb, 0x64,
            0x41, 0xb1, 0xf7, 0xe3,
        ]
    );
    let outcome = execute_with_limits(
        &package,
        &approved,
        vec![bytes_input(&body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("Contract schema decoder must return")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("Contract schema decoder must accept native body: {value:?}")
    };
    let ConstData::Sequence(fields) = decoded.data else {
        panic!("Contract schema decoder must return a five-field tuple")
    };
    assert_eq!(fields.len(), 5);
    for (field, expected) in fields.iter().zip(expected) {
        assert_eq!(field.data, ConstData::Bytes(expected));
    }

    let no_limits = contract_schema_body(false);
    assert_eq!(exact_entity_body_fields(&no_limits, 13, 4).len(), 4);
    let no_limits_outcome = execute_with_limits(
        &package,
        &approved,
        vec![bytes_input(&no_limits), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(no_limits_value) = no_limits_outcome.termination
    else {
        panic!("Contract schema decoder must return without optional limits")
    };
    let ConstData::Result(ResultConst::Ok(no_limits_decoded)) = no_limits_value.data else {
        panic!("Contract schema decoder must accept absent optional limits")
    };
    let ConstData::Sequence(no_limits_fields) = no_limits_decoded.data else {
        panic!("Contract schema decoder normalizes five fields")
    };
    assert_eq!(no_limits_fields.len(), 5);
    assert_eq!(no_limits_fields[4].data, ConstData::Bytes(Vec::new()));
}

#[test]
fn contract_schema_decoder_rejects_every_nested_boundary() {
    let body = contract_schema_body(true);
    let fields = exact_entity_body_fields(&body, 13, 5);
    let image = contract_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let mut unknown_fields = fields.clone();
    unknown_fields.push(Vec::new());
    let mut short_target = fields.clone();
    short_target[0] = vec![0xa2; 31];
    let mut invalid_kind = fields.clone();
    invalid_kind[1] = sley_scb1::encode_uvar(8);
    let invalid_source = sley_scb1::encode_union(5, &[]).expect("unknown source union encodes");
    let invalid_binding =
        sley_scb1::encode_record(&[(1, sley_scb1::encode_uvar(0)), (2, invalid_source)])
            .expect("invalid binding encodes structurally");
    let mut invalid_bindings = fields.clone();
    invalid_bindings[3] = sley_scb1::encode_list(&[invalid_binding]).expect("binding list encodes");
    let wide_binding = sley_scb1::encode_record(&[
        (1, sley_scb1::encode_uvar(u64::from(u32::MAX) + 1)),
        (
            2,
            sley_scb1::encode_union(2, &[]).expect("Result source encodes"),
        ),
    ])
    .expect("wide binding encodes structurally");
    let mut wide_binding_fields = fields.clone();
    wide_binding_fields[3] = sley_scb1::encode_list(&[wide_binding]).expect("binding list encodes");
    let mut invalid_limits = fields.clone();
    invalid_limits[4] = sley_scb1::encode_record(&[
        (1, sley_scb1::encode_uvar(1)),
        (2, sley_scb1::encode_uvar(2)),
        (3, sley_scb1::encode_uvar(3)),
        (4, sley_scb1::encode_uvar(4)),
        (5, sley_scb1::encode_uvar(5)),
    ])
    .expect("short resource record encodes");

    let cases = [
        (
            "wrong_entity_kind",
            parameter_schema_with_fields(12, &fields),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_required",
            parameter_schema_with_fields(13, &fields[..3]),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_field",
            parameter_schema_with_fields(13, &unknown_fields),
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "short_target",
            parameter_schema_with_fields(13, &short_target),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "invalid_contract_kind",
            parameter_schema_with_fields(13, &invalid_kind),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "invalid_binding_source",
            parameter_schema_with_fields(13, &invalid_bindings),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "wide_binding_ordinal",
            parameter_schema_with_fields(13, &wide_binding_fields),
            b"SCB_INTEGER_OVERFLOW".as_slice(),
        ),
        (
            "missing_resource_field",
            parameter_schema_with_fields(13, &invalid_limits),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
    ];
    for (name, malformed, expected) in cases {
        assert_eq!(
            simple_schema_error(&package, &approved, &malformed, "Contract"),
            expected,
            "{name} precedence"
        );
        assert_native_body_parity(&malformed, expected, name);
    }
}

fn type_def_schema_body(form: sley_ssmc::TypeDefForm) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, TypeDefBody};
    use sley_ssmc::TypeParameterDef;

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0xd1; 32]),
        body: EntityBodyValue::TypeDef(TypeDefBody {
            type_parameters: vec![
                TypeParameterDef { ordinal: 0 },
                TypeParameterDef { ordinal: u32::MAX },
            ],
            form,
            invariants: EntityIdSet::from_unsorted(vec![
                EntityId::from_bytes([0xd2; 32]),
                EntityId::from_bytes([0xd3; 32]),
            ])
            .expect("type-definition invariants are canonical"),
            visibility: Visibility::Exported,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema TypeDef fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

fn type_def_record_form() -> sley_ssmc::TypeDefForm {
    use sley_ssmc::{MemberId, RecordField, TypeDefForm};

    TypeDefForm::Record(vec![
        RecordField {
            member_id: MemberId::from_bytes([0xd4; 32]),
            value_type: TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Text]),
            visibility: Visibility::Private,
        },
        RecordField {
            member_id: MemberId::from_bytes([0xd5; 32]),
            value_type: TypeExpr::Vector(Box::new(TypeExpr::Bool)),
            visibility: Visibility::Exported,
        },
    ])
}

fn type_def_variant_form() -> sley_ssmc::TypeDefForm {
    use sley_ssmc::{MemberId, TypeDefForm, VariantCase};

    TypeDefForm::Variant(vec![
        VariantCase {
            member_id: MemberId::from_bytes([0xd6; 32]),
            payload_type: Some(TypeExpr::Option(Box::new(TypeExpr::Bytes))),
        },
        VariantCase {
            member_id: MemberId::from_bytes([0xd7; 32]),
            payload_type: None,
        },
    ])
}

fn assert_type_def_schema_projection(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    body: &[u8],
) {
    let expected = exact_entity_body_fields(body, 4, 4);
    let outcome = execute_with_limits(
        package,
        approved,
        vec![bytes_input(body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("TypeDef schema decoder must return")
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("TypeDef schema decoder must accept native body: {value:?}")
    };
    let ConstData::Sequence(fields) = decoded.data else {
        panic!("TypeDef schema decoder must return a four-field tuple")
    };
    assert_eq!(fields.len(), 4);
    for (field, expected) in fields.iter().zip(expected) {
        assert_eq!(field.data, ConstData::Bytes(expected));
    }
}

#[test]
fn type_def_schema_decoder_accepts_both_forms() {
    let image = type_def_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "TYPE_DEF_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 23);
    assert_eq!(image.parameters.len(), 1_501);
    assert_eq!(image.blocks.len(), 388);
    assert_eq!(image.operations.len(), 704);
    assert_eq!(image.constants.len(), 232);
    assert_eq!(package.image_bytes.len(), 93_662);
    assert_eq!(
        approved.package_digest,
        [
            0x8e, 0x39, 0x01, 0xd3, 0x73, 0xbb, 0x38, 0x3a, 0xeb, 0x48, 0x4f, 0x49, 0x8a, 0xc2,
            0x72, 0x69, 0xb9, 0xfd, 0x49, 0x4f, 0xb4, 0x86, 0xb5, 0xce, 0xdc, 0x0a, 0x5d, 0x8e,
            0x30, 0xac, 0xd9, 0xdb,
        ]
    );

    assert_type_def_schema_projection(
        &package,
        &approved,
        &type_def_schema_body(type_def_record_form()),
    );
    assert_type_def_schema_projection(
        &package,
        &approved,
        &type_def_schema_body(type_def_variant_form()),
    );
    assert_type_def_schema_projection(
        &package,
        &approved,
        &type_def_schema_body(sley_ssmc::TypeDefForm::Record(Vec::new())),
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn type_def_schema_decoder_rejects_every_nested_boundary() {
    let body = type_def_schema_body(type_def_record_form());
    let fields = exact_entity_body_fields(&body, 4, 4);
    let image = type_def_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let type_expr = sley_scb1::encode_union(1, &[]).expect("Unit TypeExpr encodes");
    let record_field = |member: Vec<u8>, value_type: Vec<u8>, visibility: u64| {
        sley_scb1::encode_record(&[
            (1, member),
            (2, value_type),
            (3, sley_scb1::encode_uvar(visibility)),
        ])
        .expect("record field encodes structurally")
    };
    let record_form = |field: Vec<u8>| {
        sley_scb1::encode_union(
            1,
            &sley_scb1::encode_list(&[field]).expect("record field list encodes"),
        )
        .expect("record form encodes")
    };
    let variant_form = |payload: Vec<u8>| {
        let case = sley_scb1::encode_record(&[(1, vec![0xd6; 32]), (2, payload)])
            .expect("variant case encodes structurally");
        sley_scb1::encode_union(
            2,
            &sley_scb1::encode_list(&[case]).expect("variant case list encodes"),
        )
        .expect("variant form encodes")
    };

    let mut unknown_fields = fields.clone();
    unknown_fields.push(Vec::new());
    let mut wide_type_parameter = fields.clone();
    wide_type_parameter[0] = sley_scb1::encode_list(&[sley_scb1::encode_record(&[(
        1,
        sley_scb1::encode_uvar(u64::from(u32::MAX) + 1),
    )])
    .expect("wide type parameter encodes structurally")])
    .expect("type-parameter list encodes");
    let mut unknown_form = fields.clone();
    unknown_form[1] = sley_scb1::encode_union(3, &[]).expect("unknown form union encodes");
    let mut short_member = fields.clone();
    short_member[1] = record_form(record_field(vec![0xd4; 31], type_expr.clone(), 1));
    let mut malformed_field_type = fields.clone();
    malformed_field_type[1] = record_form(record_field(
        vec![0xd4; 32],
        sley_scb1::encode_union(21, &[]).expect("unknown TypeExpr encodes"),
        1,
    ));
    let mut invalid_field_visibility = fields.clone();
    invalid_field_visibility[1] = record_form(record_field(vec![0xd4; 32], type_expr.clone(), 5));
    let mut missing_case_payload = fields.clone();
    missing_case_payload[1] = sley_scb1::encode_union(
        2,
        &sley_scb1::encode_list(&[
            sley_scb1::encode_record(&[(1, vec![0xd6; 32])]).expect("short case encodes")
        ])
        .expect("variant case list encodes"),
    )
    .expect("variant form encodes");
    let mut invalid_payload_option = fields.clone();
    invalid_payload_option[1] =
        variant_form(sley_scb1::encode_union(2, &[]).expect("bad option tag encodes"));
    let mut nonempty_none_payload = fields.clone();
    nonempty_none_payload[1] =
        variant_form(sley_scb1::encode_union(0, &type_expr).expect("nonempty None encodes"));
    let mut malformed_payload_type = fields.clone();
    malformed_payload_type[1] = variant_form(
        sley_scb1::encode_union(
            1,
            &sley_scb1::encode_union(21, &[]).expect("unknown TypeExpr encodes"),
        )
        .expect("Some payload encodes"),
    );
    let mut unordered_invariants = fields.clone();
    unordered_invariants[2] = sley_scb1::encode_list(&[vec![0xd3; 32], vec![0xd2; 32]])
        .expect("unordered invariant list encodes structurally");
    let mut invalid_visibility = fields.clone();
    invalid_visibility[3] = sley_scb1::encode_uvar(0);

    let cases = [
        (
            "wrong_entity_kind",
            parameter_schema_with_fields(5, &fields),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_required",
            parameter_schema_with_fields(4, &fields[..3]),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_field",
            parameter_schema_with_fields(4, &unknown_fields),
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "wide_type_parameter",
            parameter_schema_with_fields(4, &wide_type_parameter),
            b"SCB_INTEGER_OVERFLOW".as_slice(),
        ),
        (
            "unknown_form",
            parameter_schema_with_fields(4, &unknown_form),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "short_member",
            parameter_schema_with_fields(4, &short_member),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "malformed_field_type",
            parameter_schema_with_fields(4, &malformed_field_type),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "invalid_field_visibility",
            parameter_schema_with_fields(4, &invalid_field_visibility),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_case_payload",
            parameter_schema_with_fields(4, &missing_case_payload),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "invalid_payload_option",
            parameter_schema_with_fields(4, &invalid_payload_option),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "nonempty_none_payload",
            parameter_schema_with_fields(4, &nonempty_none_payload),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "malformed_payload_type",
            parameter_schema_with_fields(4, &malformed_payload_type),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "unordered_invariants",
            parameter_schema_with_fields(4, &unordered_invariants),
            b"SCB_MAP_ORDER".as_slice(),
        ),
        (
            "invalid_visibility",
            parameter_schema_with_fields(4, &invalid_visibility),
            b"SCB_UNION_INVALID".as_slice(),
        ),
    ];
    for (name, malformed, expected) in cases {
        assert_eq!(
            simple_schema_error(&package, &approved, &malformed, "TypeDef"),
            expected,
            "{name} precedence"
        );
        assert_native_body_parity(&malformed, expected, name);
    }
}

fn option_u8_type() -> TypeExpr {
    TypeExpr::Option(Box::new(u8_type()))
}

fn u8_const(assembler: &mut Asm, ns: Ns, block: EntityId, value: u8) -> EntityId {
    let constant = assembler.ku8(ns.k, u128::from(value));
    assembler.cref(ns.o, block, constant, u8_type())
}

fn u64_const(assembler: &mut Asm, ns: Ns, block: EntityId, value: u64) -> EntityId {
    let constant = assembler.ku64(ns.k, u128::from(value));
    assembler.cref(ns.o, block, constant, u64_type())
}

fn bool_op(
    assembler: &mut Asm,
    ns: Ns,
    block: EntityId,
    opcode: Opcode,
    operands: Vec<ValueRef>,
) -> EntityId {
    assembler.op(
        ns.o,
        block,
        opcode,
        operands,
        vec![TypeExpr::Bool],
        Immediate::None,
    )
}

/// `Return(ResultErr(payload))` block forwarding one callee error payload.
fn forward_error_block(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    result_type: &TypeExpr,
) -> EntityId {
    let block = assembler.id(ns.b);
    let forwarded = assembler.param(ns.p, block, ParameterRole::Block, TypeExpr::Bytes);
    let result = assembler.op(
        ns.o,
        block,
        Opcode::ResultErr,
        vec![pav(forwarded)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        block,
        function,
        vec![forwarded],
        vec![result],
        ret(op_result(result)),
    );
    block
}

/// `Return(ResultOk(unit))` block for unit-result validators.
fn unit_success_block(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    unit: EntityId,
    result_type: &TypeExpr,
) -> EntityId {
    let block = assembler.id(ns.b);
    let ok = assembler.op(
        ns.o,
        block,
        Opcode::ResultOk,
        vec![pav(unit)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        block,
        function,
        Vec::new(),
        vec![ok],
        ret(op_result(ok)),
    );
    block
}

/// Entry block converting the body to a byte vector through the frozen
/// `B2V1` bridge and continuing at `vector_ready(vector)`.
fn vector_entry_block(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    body: EntityId,
    unit: EntityId,
    resource_error: EntityId,
    vector_ready: EntityId,
) -> EntityId {
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
    entry
}

fn unit_validator_graph(
    assembler: &Asm,
    function: EntityId,
    body: EntityId,
    unit: EntityId,
    entry: EntityId,
    block_start: usize,
) -> FunctionGraph {
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![body, unit],
        result_type: unit_validation_result_type(),
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

/// Validates one strict SCB1 boolean byte: `0` or `1`, nothing else.
#[allow(clippy::too_many_lines)]
fn build_bool_validate(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let length_code = assembler.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let bool_code = assembler.kbytes(ns.k, b"SCB_BOOL_INVALID");
    let trailing_code = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let length_error = err_block(assembler, ns, function, result_type.clone(), length_code);
    let bool_error = err_block(assembler, ns, function, result_type.clone(), bool_code);
    let trailing_error = err_block(assembler, ns, function, result_type.clone(), trailing_code);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
    let success = unit_success_block(assembler, ns, function, unit, &result_type);
    let trailing_check = assembler.id(ns.b);
    let value_check = assembler.id(ns.b);
    let vector_ready = assembler.id(ns.b);

    let trailing_parameters = block_parameters(assembler, ns.p, trailing_check, &[u8vec_type()]);
    let length = assembler.op(
        ns.o,
        trailing_check,
        Opcode::VectorLen,
        vec![pav(trailing_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let one = u64_const(assembler, ns, trailing_check, 1);
    let more = bool_op(
        assembler,
        ns,
        trailing_check,
        Opcode::GreaterThan,
        vec![op_result(length), op_result(one)],
    );
    append_block(
        assembler,
        trailing_check,
        function,
        trailing_parameters,
        vec![length, one, more],
        cond(
            op_result(more),
            edge(trailing_error, Vec::new()),
            edge(success, Vec::new()),
        ),
    );

    let value_parameters =
        block_parameters(assembler, ns.p, value_check, &[u8_type(), u8vec_type()]);
    let one_byte = u8_const(assembler, ns, value_check, 1);
    let invalid = bool_op(
        assembler,
        ns,
        value_check,
        Opcode::GreaterThan,
        vec![pav(value_parameters[0]), op_result(one_byte)],
    );
    append_block(
        assembler,
        value_check,
        function,
        value_parameters.clone(),
        vec![one_byte, invalid],
        cond(
            op_result(invalid),
            edge(bool_error, Vec::new()),
            edge(trailing_check, vec![pav(value_parameters[1])]),
        ),
    );

    let vector_parameters = block_parameters(assembler, ns.p, vector_ready, &[u8vec_type()]);
    let zero = u64_const(assembler, ns, vector_ready, 0);
    let first = assembler.op(
        ns.o,
        vector_ready,
        Opcode::VectorGet,
        vec![pav(vector_parameters[0]), op_result(zero)],
        vec![option_u8_type()],
        Immediate::None,
    );
    append_block(
        assembler,
        vector_ready,
        function,
        vector_parameters.clone(),
        vec![zero, first],
        switch(
            op_result(first),
            vec![
                (BuiltinCase::None, length_error, Vec::new()),
                (
                    BuiltinCase::Some,
                    value_check,
                    vec![SwitchArgument::CasePayload, sav(vector_parameters[0])],
                ),
            ],
        ),
    );

    let entry = vector_entry_block(
        assembler,
        ns,
        function,
        body,
        unit,
        resource_error,
        vector_ready,
    );
    unit_validator_graph(assembler, function, body, unit, entry, block_start)
}

/// Validates one canonical 128-bit unsigned varint occupying the whole body,
/// with the native strict-decoder precedence: overflow before minimality
/// before trailing bytes.
#[allow(clippy::too_many_lines)]
fn build_uvar128_validate(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let length_code = assembler.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let overflow_code = assembler.kbytes(ns.k, b"SCB_INTEGER_OVERFLOW");
    let minimal_code = assembler.kbytes(ns.k, b"SCB_VARINT_NON_MINIMAL");
    let trailing_code = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let length_error = err_block(assembler, ns, function, result_type.clone(), length_code);
    let overflow_error = err_block(assembler, ns, function, result_type.clone(), overflow_code);
    let minimal_error = err_block(assembler, ns, function, result_type.clone(), minimal_code);
    let trailing_error = err_block(assembler, ns, function, result_type.clone(), trailing_code);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let success = unit_success_block(assembler, ns, function, unit, &result_type);
    let fetch = assembler.id(ns.b);
    let classify = assembler.id(ns.b);
    let continuation = assembler.id(ns.b);
    let continuation_check = assembler.id(ns.b);
    let advance = assembler.id(ns.b);
    let last = assembler.id(ns.b);
    let last_minimal = assembler.id(ns.b);
    let last_consumed = assembler.id(ns.b);
    let trailing_compare = assembler.id(ns.b);
    let vector_ready = assembler.id(ns.b);
    let index_types = vec![u64_type(), u8vec_type()];
    let byte_types = vec![u8_type(), u64_type(), u8vec_type()];

    let fetch_parameters = block_parameters(assembler, ns.p, fetch, &index_types);
    let item = assembler.op(
        ns.o,
        fetch,
        Opcode::VectorGet,
        vec![pav(fetch_parameters[1]), pav(fetch_parameters[0])],
        vec![option_u8_type()],
        Immediate::None,
    );
    append_block(
        assembler,
        fetch,
        function,
        fetch_parameters.clone(),
        vec![item],
        switch(
            op_result(item),
            vec![
                (BuiltinCase::None, length_error, Vec::new()),
                (
                    BuiltinCase::Some,
                    classify,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(fetch_parameters[0]),
                        sav(fetch_parameters[1]),
                    ],
                ),
            ],
        ),
    );

    let classify_parameters = block_parameters(assembler, ns.p, classify, &byte_types);
    let high_bit = u8_const(assembler, ns, classify, 0x80);
    let continues = bool_op(
        assembler,
        ns,
        classify,
        Opcode::GreaterEqual,
        vec![pav(classify_parameters[0]), op_result(high_bit)],
    );
    append_block(
        assembler,
        classify,
        function,
        classify_parameters.clone(),
        vec![high_bit, continues],
        cond(
            op_result(continues),
            edge(continuation, parameter_values(&classify_parameters)),
            edge(last, parameter_values(&classify_parameters)),
        ),
    );

    let continuation_parameters = block_parameters(assembler, ns.p, continuation, &byte_types);
    let continuation_bit = u8_const(assembler, ns, continuation, 0x80);
    let payload = assembler.op(
        ns.o,
        continuation,
        Opcode::IntSubChecked,
        vec![pav(continuation_parameters[0]), op_result(continuation_bit)],
        vec![arith_result(u8_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        continuation,
        function,
        continuation_parameters.clone(),
        vec![continuation_bit, payload],
        switch(
            op_result(payload),
            vec![
                (
                    BuiltinCase::Ok,
                    continuation_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(continuation_parameters[1]),
                        sav(continuation_parameters[2]),
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let check_parameters = block_parameters(assembler, ns.p, continuation_check, &byte_types);
    let nineteen = u64_const(assembler, ns, continuation_check, 19);
    let eighteen = u64_const(assembler, ns, continuation_check, 18);
    let three = u8_const(assembler, ns, continuation_check, 3);
    let too_long = bool_op(
        assembler,
        ns,
        continuation_check,
        Opcode::GreaterEqual,
        vec![pav(check_parameters[1]), op_result(nineteen)],
    );
    let at_top = bool_op(
        assembler,
        ns,
        continuation_check,
        Opcode::Equal,
        vec![pav(check_parameters[1]), op_result(eighteen)],
    );
    let too_big = bool_op(
        assembler,
        ns,
        continuation_check,
        Opcode::GreaterThan,
        vec![pav(check_parameters[0]), op_result(three)],
    );
    let top_overflow = bool_op(
        assembler,
        ns,
        continuation_check,
        Opcode::BoolAnd,
        vec![op_result(at_top), op_result(too_big)],
    );
    let overflow = bool_op(
        assembler,
        ns,
        continuation_check,
        Opcode::BoolOr,
        vec![op_result(too_long), op_result(top_overflow)],
    );
    append_block(
        assembler,
        continuation_check,
        function,
        check_parameters.clone(),
        vec![
            nineteen,
            eighteen,
            three,
            too_long,
            at_top,
            too_big,
            top_overflow,
            overflow,
        ],
        cond(
            op_result(overflow),
            edge(overflow_error, Vec::new()),
            edge(
                advance,
                vec![pav(check_parameters[1]), pav(check_parameters[2])],
            ),
        ),
    );

    let advance_parameters = block_parameters(assembler, ns.p, advance, &index_types);
    let step = u64_const(assembler, ns, advance, 1);
    let next = assembler.op(
        ns.o,
        advance,
        Opcode::IntAddChecked,
        vec![pav(advance_parameters[0]), op_result(step)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        advance,
        function,
        advance_parameters.clone(),
        vec![step, next],
        switch(
            op_result(next),
            vec![
                (
                    BuiltinCase::Ok,
                    fetch,
                    vec![SwitchArgument::CasePayload, sav(advance_parameters[1])],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let last_parameters = block_parameters(assembler, ns.p, last, &byte_types);
    let last_nineteen = u64_const(assembler, ns, last, 19);
    let last_eighteen = u64_const(assembler, ns, last, 18);
    let last_three = u8_const(assembler, ns, last, 3);
    let zero_byte = u8_const(assembler, ns, last, 0);
    let last_too_long = bool_op(
        assembler,
        ns,
        last,
        Opcode::GreaterEqual,
        vec![pav(last_parameters[1]), op_result(last_nineteen)],
    );
    let nonzero = bool_op(
        assembler,
        ns,
        last,
        Opcode::NotEqual,
        vec![pav(last_parameters[0]), op_result(zero_byte)],
    );
    let long_overflow = bool_op(
        assembler,
        ns,
        last,
        Opcode::BoolAnd,
        vec![op_result(last_too_long), op_result(nonzero)],
    );
    let last_at_top = bool_op(
        assembler,
        ns,
        last,
        Opcode::Equal,
        vec![pav(last_parameters[1]), op_result(last_eighteen)],
    );
    let last_too_big = bool_op(
        assembler,
        ns,
        last,
        Opcode::GreaterThan,
        vec![pav(last_parameters[0]), op_result(last_three)],
    );
    let last_top_overflow = bool_op(
        assembler,
        ns,
        last,
        Opcode::BoolAnd,
        vec![op_result(last_at_top), op_result(last_too_big)],
    );
    let last_overflow = bool_op(
        assembler,
        ns,
        last,
        Opcode::BoolOr,
        vec![op_result(long_overflow), op_result(last_top_overflow)],
    );
    append_block(
        assembler,
        last,
        function,
        last_parameters.clone(),
        vec![
            last_nineteen,
            last_eighteen,
            last_three,
            zero_byte,
            last_too_long,
            nonzero,
            long_overflow,
            last_at_top,
            last_too_big,
            last_top_overflow,
            last_overflow,
        ],
        cond(
            op_result(last_overflow),
            edge(overflow_error, Vec::new()),
            edge(last_minimal, parameter_values(&last_parameters)),
        ),
    );

    let minimal_parameters = block_parameters(assembler, ns.p, last_minimal, &byte_types);
    let zero_index = u64_const(assembler, ns, last_minimal, 0);
    let minimal_zero_byte = u8_const(assembler, ns, last_minimal, 0);
    let not_first = bool_op(
        assembler,
        ns,
        last_minimal,
        Opcode::GreaterThan,
        vec![pav(minimal_parameters[1]), op_result(zero_index)],
    );
    let zero_payload = bool_op(
        assembler,
        ns,
        last_minimal,
        Opcode::Equal,
        vec![pav(minimal_parameters[0]), op_result(minimal_zero_byte)],
    );
    let non_minimal = bool_op(
        assembler,
        ns,
        last_minimal,
        Opcode::BoolAnd,
        vec![op_result(not_first), op_result(zero_payload)],
    );
    append_block(
        assembler,
        last_minimal,
        function,
        minimal_parameters.clone(),
        vec![
            zero_index,
            minimal_zero_byte,
            not_first,
            zero_payload,
            non_minimal,
        ],
        cond(
            op_result(non_minimal),
            edge(minimal_error, Vec::new()),
            edge(
                last_consumed,
                vec![pav(minimal_parameters[1]), pav(minimal_parameters[2])],
            ),
        ),
    );

    let consumed_parameters = block_parameters(assembler, ns.p, last_consumed, &index_types);
    let consumed_step = u64_const(assembler, ns, last_consumed, 1);
    let consumed = assembler.op(
        ns.o,
        last_consumed,
        Opcode::IntAddChecked,
        vec![pav(consumed_parameters[0]), op_result(consumed_step)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        last_consumed,
        function,
        consumed_parameters.clone(),
        vec![consumed_step, consumed],
        switch(
            op_result(consumed),
            vec![
                (
                    BuiltinCase::Ok,
                    trailing_compare,
                    vec![SwitchArgument::CasePayload, sav(consumed_parameters[1])],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let trailing_parameters = block_parameters(assembler, ns.p, trailing_compare, &index_types);
    let total = assembler.op(
        ns.o,
        trailing_compare,
        Opcode::VectorLen,
        vec![pav(trailing_parameters[1])],
        vec![u64_type()],
        Immediate::None,
    );
    let more = bool_op(
        assembler,
        ns,
        trailing_compare,
        Opcode::LessThan,
        vec![pav(trailing_parameters[0]), op_result(total)],
    );
    append_block(
        assembler,
        trailing_compare,
        function,
        trailing_parameters,
        vec![total, more],
        cond(
            op_result(more),
            edge(trailing_error, Vec::new()),
            edge(success, Vec::new()),
        ),
    );

    let vector_parameters = block_parameters(assembler, ns.p, vector_ready, &[u8vec_type()]);
    let start = u64_const(assembler, ns, vector_ready, 0);
    append_block(
        assembler,
        vector_ready,
        function,
        vector_parameters.clone(),
        vec![start],
        branch(edge(
            fetch,
            vec![op_result(start), pav(vector_parameters[0])],
        )),
    );

    let entry = vector_entry_block(
        assembler,
        ns,
        function,
        body,
        unit,
        resource_error,
        vector_ready,
    );
    unit_validator_graph(assembler, function, body, unit, entry, block_start)
}

/// Validates exactly `width` big-endian bytes holding a canonical IEEE bit
/// pattern: negative zero and every NaN other than the canonical quiet NaN
/// are refused, matching `validate_f32_bits` / `validate_f64_bits`.
#[allow(clippy::too_many_lines)]
fn build_float_bits_validate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    width: usize,
) -> FunctionGraph {
    assert!(matches!(width, 4 | 8), "float widths are 4 or 8 bytes");
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let length_code = assembler.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let float_code = assembler.kbytes(ns.k, b"SCB_FLOAT_NON_CANONICAL");
    let trailing_code = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let length_error = err_block(assembler, ns, function, result_type.clone(), length_code);
    let float_error = err_block(assembler, ns, function, result_type.clone(), float_code);
    let trailing_error = err_block(assembler, ns, function, result_type.clone(), trailing_code);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let success = unit_success_block(assembler, ns, function, unit, &result_type);
    let trailing_check = assembler.id(ns.b);
    let analyze = assembler.id(ns.b);
    let fetches = (0..width).map(|_| assembler.id(ns.b)).collect::<Vec<_>>();
    let vector_ready = assembler.id(ns.b);
    let width_u64 = u64::try_from(width).expect("float width fits u64");

    let trailing_parameters = block_parameters(assembler, ns.p, trailing_check, &[u8vec_type()]);
    let length = assembler.op(
        ns.o,
        trailing_check,
        Opcode::VectorLen,
        vec![pav(trailing_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let expected_length = u64_const(assembler, ns, trailing_check, width_u64);
    let more = bool_op(
        assembler,
        ns,
        trailing_check,
        Opcode::GreaterThan,
        vec![op_result(length), op_result(expected_length)],
    );
    append_block(
        assembler,
        trailing_check,
        function,
        trailing_parameters,
        vec![length, expected_length, more],
        cond(
            op_result(more),
            edge(trailing_error, Vec::new()),
            edge(success, Vec::new()),
        ),
    );

    let mut analyze_types = vec![u8vec_type()];
    analyze_types.extend(std::iter::repeat_n(u8_type(), width));
    let analyze_parameters = block_parameters(assembler, ns.p, analyze, &analyze_types);
    let bytes = &analyze_parameters[1..];
    let mut operations = Vec::new();
    let byte_is = |assembler: &mut Asm, index: usize, value: u8, operations: &mut Vec<EntityId>| {
        let constant = u8_const(assembler, ns, analyze, value);
        let equal = bool_op(
            assembler,
            ns,
            analyze,
            Opcode::Equal,
            vec![pav(bytes[index]), op_result(constant)],
        );
        operations.extend([constant, equal]);
        equal
    };
    let (exponent_floor, canonical_nan_second) = if width == 4 {
        (0x80_u8, 0xc0_u8)
    } else {
        (0xf0_u8, 0xf8_u8)
    };
    let sign_only = byte_is(assembler, 0, 0x80, &mut operations);
    let positive_max_exponent = byte_is(assembler, 0, 0x7f, &mut operations);
    let negative_max_exponent = byte_is(assembler, 0, 0xff, &mut operations);
    let second_zero = byte_is(assembler, 1, 0, &mut operations);
    let second_exponent_only = byte_is(assembler, 1, exponent_floor, &mut operations);
    let second_canonical_nan = byte_is(assembler, 1, canonical_nan_second, &mut operations);
    let rest_zero_checks = (2..width)
        .map(|index| byte_is(assembler, index, 0, &mut operations))
        .collect::<Vec<_>>();
    let exponent_floor_value = u8_const(assembler, ns, analyze, exponent_floor);
    let second_high = bool_op(
        assembler,
        ns,
        analyze,
        Opcode::GreaterEqual,
        vec![pav(bytes[1]), op_result(exponent_floor_value)],
    );
    operations.extend([exponent_floor_value, second_high]);
    let fold = |assembler: &mut Asm,
                opcode: Opcode,
                values: &[EntityId],
                operations: &mut Vec<EntityId>| {
        let mut accumulator = values[0];
        for value in &values[1..] {
            accumulator = bool_op(
                assembler,
                ns,
                analyze,
                opcode,
                vec![op_result(accumulator), op_result(*value)],
            );
            operations.push(accumulator);
        }
        accumulator
    };
    let rest_zero = fold(
        assembler,
        Opcode::BoolAnd,
        &rest_zero_checks,
        &mut operations,
    );
    let negative_zero = fold(
        assembler,
        Opcode::BoolAnd,
        &[sign_only, second_zero, rest_zero],
        &mut operations,
    );
    let max_exponent_first = fold(
        assembler,
        Opcode::BoolOr,
        &[positive_max_exponent, negative_max_exponent],
        &mut operations,
    );
    let max_exponent = fold(
        assembler,
        Opcode::BoolAnd,
        &[max_exponent_first, second_high],
        &mut operations,
    );
    let second_mantissa_zero_and_rest = fold(
        assembler,
        Opcode::BoolAnd,
        &[second_exponent_only, rest_zero],
        &mut operations,
    );
    let mantissa_nonzero = bool_op(
        assembler,
        ns,
        analyze,
        Opcode::BoolNot,
        vec![op_result(second_mantissa_zero_and_rest)],
    );
    operations.push(mantissa_nonzero);
    let canonical_nan = fold(
        assembler,
        Opcode::BoolAnd,
        &[positive_max_exponent, second_canonical_nan, rest_zero],
        &mut operations,
    );
    let not_canonical_nan = bool_op(
        assembler,
        ns,
        analyze,
        Opcode::BoolNot,
        vec![op_result(canonical_nan)],
    );
    operations.push(not_canonical_nan);
    let bad_nan = fold(
        assembler,
        Opcode::BoolAnd,
        &[max_exponent, mantissa_nonzero, not_canonical_nan],
        &mut operations,
    );
    let non_canonical = fold(
        assembler,
        Opcode::BoolOr,
        &[negative_zero, bad_nan],
        &mut operations,
    );
    append_block(
        assembler,
        analyze,
        function,
        analyze_parameters.clone(),
        operations,
        cond(
            op_result(non_canonical),
            edge(float_error, Vec::new()),
            edge(trailing_check, vec![pav(analyze_parameters[0])]),
        ),
    );

    for (index, block) in fetches.iter().copied().enumerate() {
        let mut types = vec![u8vec_type()];
        types.extend(std::iter::repeat_n(u8_type(), index));
        let parameters = block_parameters(assembler, ns.p, block, &types);
        let position = u64_const(
            assembler,
            ns,
            block,
            u64::try_from(index).expect("byte index fits u64"),
        );
        let item = assembler.op(
            ns.o,
            block,
            Opcode::VectorGet,
            vec![pav(parameters[0]), op_result(position)],
            vec![option_u8_type()],
            Immediate::None,
        );
        let destination = fetches.get(index + 1).copied().unwrap_or(analyze);
        let mut arguments = parameters.iter().copied().map(sav).collect::<Vec<_>>();
        arguments.push(SwitchArgument::CasePayload);
        append_block(
            assembler,
            block,
            function,
            parameters,
            vec![position, item],
            switch(
                op_result(item),
                vec![
                    (BuiltinCase::None, invariant_trap, Vec::new()),
                    (BuiltinCase::Some, destination, arguments),
                ],
            ),
        );
    }

    let vector_parameters = block_parameters(assembler, ns.p, vector_ready, &[u8vec_type()]);
    let total = assembler.op(
        ns.o,
        vector_ready,
        Opcode::VectorLen,
        vec![pav(vector_parameters[0])],
        vec![u64_type()],
        Immediate::None,
    );
    let required = u64_const(assembler, ns, vector_ready, width_u64);
    let short = bool_op(
        assembler,
        ns,
        vector_ready,
        Opcode::LessThan,
        vec![op_result(total), op_result(required)],
    );
    append_block(
        assembler,
        vector_ready,
        function,
        vector_parameters.clone(),
        vec![total, required, short],
        cond(
            op_result(short),
            edge(length_error, Vec::new()),
            edge(fetches[0], vec![pav(vector_parameters[0])]),
        ),
    );

    let entry = vector_entry_block(
        assembler,
        ns,
        function,
        body,
        unit,
        resource_error,
        vector_ready,
    );
    unit_validator_graph(assembler, function, body, unit, entry, block_start)
}

/// One UTF-8 lead-byte tier: the comparison and bound selecting it, and the
/// continuation shape `(count, first_low, first_high)` it demands; `None`
/// marks a refused lead range.
type LeadByteTier = (Opcode, u8, Option<(u64, u8, u8)>);

/// Validates one length-prefixed byte payload occupying the whole body
/// (`read_bytes`), optionally requiring the payload to be valid UTF-8
/// (`read_text`). Precedence follows the native reader: length prefix
/// errors, then the 16 MiB payload cap, then a short payload, then UTF-8,
/// then trailing bytes.
#[allow(clippy::too_many_lines)]
fn build_sized_payload_validate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    decode_function: EntityId,
    utf8: bool,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let length_code = assembler.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let trailing_code = assembler.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let resource_code = assembler.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let forward_error = forward_error_block(assembler, ns, function, &result_type);
    let length_error = err_block(assembler, ns, function, result_type.clone(), length_code);
    let trailing_error = err_block(assembler, ns, function, result_type.clone(), trailing_code);
    let resource_error = err_block(assembler, ns, function, result_type.clone(), resource_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let success = unit_success_block(assembler, ns, function, unit, &result_type);
    let trailing_check = assembler.id(ns.b);
    let range_check = assembler.id(ns.b);
    let bounds = assembler.id(ns.b);
    let length_ready = assembler.id(ns.b);
    let vector_ready = assembler.id(ns.b);
    let scan_types = vec![u64_type(), u64_type(), u8vec_type()];
    let lead_types = vec![u8_type(), u64_type(), u64_type(), u8vec_type()];
    let expect_types = vec![
        u64_type(),
        u64_type(),
        u8vec_type(),
        u64_type(),
        u8_type(),
        u8_type(),
    ];

    let trailing_parameters =
        block_parameters(assembler, ns.p, trailing_check, &[u64_type(), u8vec_type()]);
    let total = assembler.op(
        ns.o,
        trailing_check,
        Opcode::VectorLen,
        vec![pav(trailing_parameters[1])],
        vec![u64_type()],
        Immediate::None,
    );
    let more = bool_op(
        assembler,
        ns,
        trailing_check,
        Opcode::LessThan,
        vec![pav(trailing_parameters[0]), op_result(total)],
    );
    append_block(
        assembler,
        trailing_check,
        function,
        trailing_parameters,
        vec![total, more],
        cond(
            op_result(more),
            edge(trailing_error, Vec::new()),
            edge(success, Vec::new()),
        ),
    );

    let payload_start = if utf8 {
        let utf8_code = assembler.kbytes(ns.k, b"SCB_UTF8_INVALID");
        let utf8_error = err_block(assembler, ns, function, result_type.clone(), utf8_code);
        let scan = assembler.id(ns.b);
        let fetch = assembler.id(ns.b);
        let classify = assembler.id(ns.b);
        let ascii_check = assembler.id(ns.b);
        let tiers = std::array::from_fn::<_, 8, _>(|_| assembler.id(ns.b));
        let expect = assembler.id(ns.b);
        let expect_fetch = assembler.id(ns.b);
        let expect_get = assembler.id(ns.b);
        let expect_check = assembler.id(ns.b);
        let expect_advance = assembler.id(ns.b);
        let expect_decrement = assembler.id(ns.b);

        let scan_parameters = block_parameters(assembler, ns.p, scan, &scan_types);
        let done = bool_op(
            assembler,
            ns,
            scan,
            Opcode::Equal,
            vec![pav(scan_parameters[0]), pav(scan_parameters[1])],
        );
        append_block(
            assembler,
            scan,
            function,
            scan_parameters.clone(),
            vec![done],
            cond(
                op_result(done),
                edge(
                    trailing_check,
                    vec![pav(scan_parameters[1]), pav(scan_parameters[2])],
                ),
                edge(fetch, parameter_values(&scan_parameters)),
            ),
        );

        let fetch_parameters = block_parameters(assembler, ns.p, fetch, &scan_types);
        let item = assembler.op(
            ns.o,
            fetch,
            Opcode::VectorGet,
            vec![pav(fetch_parameters[2]), pav(fetch_parameters[0])],
            vec![option_u8_type()],
            Immediate::None,
        );
        let mut fetch_arguments = vec![SwitchArgument::CasePayload];
        fetch_arguments.extend(fetch_parameters.iter().copied().map(sav));
        append_block(
            assembler,
            fetch,
            function,
            fetch_parameters,
            vec![item],
            switch(
                op_result(item),
                vec![
                    (BuiltinCase::None, invariant_trap, Vec::new()),
                    (BuiltinCase::Some, classify, fetch_arguments),
                ],
            ),
        );

        let classify_parameters = block_parameters(assembler, ns.p, classify, &lead_types);
        let step = u64_const(assembler, ns, classify, 1);
        let next = assembler.op(
            ns.o,
            classify,
            Opcode::IntAddChecked,
            vec![pav(classify_parameters[1]), op_result(step)],
            vec![arith_result(u64_type())],
            Immediate::None,
        );
        append_block(
            assembler,
            classify,
            function,
            classify_parameters.clone(),
            vec![step, next],
            switch(
                op_result(next),
                vec![
                    (
                        BuiltinCase::Ok,
                        ascii_check,
                        vec![
                            sav(classify_parameters[0]),
                            SwitchArgument::CasePayload,
                            sav(classify_parameters[2]),
                            sav(classify_parameters[3]),
                        ],
                    ),
                    (BuiltinCase::Err, invariant_trap, Vec::new()),
                ],
            ),
        );

        let ascii_parameters = block_parameters(assembler, ns.p, ascii_check, &lead_types);
        let ascii_limit = u8_const(assembler, ns, ascii_check, 0x80);
        let ascii = bool_op(
            assembler,
            ns,
            ascii_check,
            Opcode::LessThan,
            vec![pav(ascii_parameters[0]), op_result(ascii_limit)],
        );
        append_block(
            assembler,
            ascii_check,
            function,
            ascii_parameters.clone(),
            vec![ascii_limit, ascii],
            cond(
                op_result(ascii),
                edge(
                    scan,
                    vec![
                        pav(ascii_parameters[1]),
                        pav(ascii_parameters[2]),
                        pav(ascii_parameters[3]),
                    ],
                ),
                edge(tiers[0], parameter_values(&ascii_parameters)),
            ),
        );

        // Lead-byte tiers in ascending order: (comparison, bound, matched
        // continuation shape). `None` sends the match to the UTF-8 refusal.
        let tier_rules: [LeadByteTier; 8] = [
            (Opcode::LessThan, 0xc2, None),
            (Opcode::LessEqual, 0xdf, Some((1, 0x80, 0xbf))),
            (Opcode::Equal, 0xe0, Some((2, 0xa0, 0xbf))),
            (Opcode::Equal, 0xed, Some((2, 0x80, 0x9f))),
            (Opcode::LessEqual, 0xef, Some((2, 0x80, 0xbf))),
            (Opcode::Equal, 0xf0, Some((3, 0x90, 0xbf))),
            (Opcode::LessEqual, 0xf3, Some((3, 0x80, 0xbf))),
            (Opcode::Equal, 0xf4, Some((3, 0x80, 0x8f))),
        ];
        for (index, (block, (opcode, bound, shape))) in
            tiers.iter().copied().zip(tier_rules).enumerate()
        {
            let parameters = block_parameters(assembler, ns.p, block, &lead_types);
            let bound_value = u8_const(assembler, ns, block, bound);
            let matched = bool_op(
                assembler,
                ns,
                block,
                opcode,
                vec![pav(parameters[0]), op_result(bound_value)],
            );
            let mut operations = vec![bound_value, matched];
            let matched_edge = match shape {
                None => edge(utf8_error, Vec::new()),
                Some((remaining, low, high)) => {
                    let remaining_value = u64_const(assembler, ns, block, remaining);
                    let low_value = u8_const(assembler, ns, block, low);
                    let high_value = u8_const(assembler, ns, block, high);
                    operations.extend([remaining_value, low_value, high_value]);
                    edge(
                        expect,
                        vec![
                            pav(parameters[1]),
                            pav(parameters[2]),
                            pav(parameters[3]),
                            op_result(remaining_value),
                            op_result(low_value),
                            op_result(high_value),
                        ],
                    )
                }
            };
            let fallback_edge = match tiers.get(index + 1) {
                Some(next_tier) => edge(*next_tier, parameter_values(&parameters)),
                None => edge(utf8_error, Vec::new()),
            };
            append_block(
                assembler,
                block,
                function,
                parameters,
                operations,
                cond(op_result(matched), matched_edge, fallback_edge),
            );
        }

        let expect_parameters = block_parameters(assembler, ns.p, expect, &expect_types);
        let none_left = u64_const(assembler, ns, expect, 0);
        let complete = bool_op(
            assembler,
            ns,
            expect,
            Opcode::Equal,
            vec![pav(expect_parameters[3]), op_result(none_left)],
        );
        append_block(
            assembler,
            expect,
            function,
            expect_parameters.clone(),
            vec![none_left, complete],
            cond(
                op_result(complete),
                edge(
                    scan,
                    vec![
                        pav(expect_parameters[0]),
                        pav(expect_parameters[1]),
                        pav(expect_parameters[2]),
                    ],
                ),
                edge(expect_fetch, parameter_values(&expect_parameters)),
            ),
        );

        let fetch_check_parameters = block_parameters(assembler, ns.p, expect_fetch, &expect_types);
        let at_end = bool_op(
            assembler,
            ns,
            expect_fetch,
            Opcode::Equal,
            vec![
                pav(fetch_check_parameters[0]),
                pav(fetch_check_parameters[1]),
            ],
        );
        append_block(
            assembler,
            expect_fetch,
            function,
            fetch_check_parameters.clone(),
            vec![at_end],
            cond(
                op_result(at_end),
                edge(utf8_error, Vec::new()),
                edge(expect_get, parameter_values(&fetch_check_parameters)),
            ),
        );

        let get_parameters = block_parameters(assembler, ns.p, expect_get, &expect_types);
        let continuation = assembler.op(
            ns.o,
            expect_get,
            Opcode::VectorGet,
            vec![pav(get_parameters[2]), pav(get_parameters[0])],
            vec![option_u8_type()],
            Immediate::None,
        );
        let mut get_arguments = vec![SwitchArgument::CasePayload];
        get_arguments.extend(get_parameters.iter().copied().map(sav));
        append_block(
            assembler,
            expect_get,
            function,
            get_parameters,
            vec![continuation],
            switch(
                op_result(continuation),
                vec![
                    (BuiltinCase::None, invariant_trap, Vec::new()),
                    (BuiltinCase::Some, expect_check, get_arguments),
                ],
            ),
        );

        let mut check_types = vec![u8_type()];
        check_types.extend(expect_types.iter().cloned());
        let check_parameters = block_parameters(assembler, ns.p, expect_check, &check_types);
        let above_low = bool_op(
            assembler,
            ns,
            expect_check,
            Opcode::GreaterEqual,
            vec![pav(check_parameters[0]), pav(check_parameters[5])],
        );
        let below_high = bool_op(
            assembler,
            ns,
            expect_check,
            Opcode::LessEqual,
            vec![pav(check_parameters[0]), pav(check_parameters[6])],
        );
        let in_range = bool_op(
            assembler,
            ns,
            expect_check,
            Opcode::BoolAnd,
            vec![op_result(above_low), op_result(below_high)],
        );
        append_block(
            assembler,
            expect_check,
            function,
            check_parameters.clone(),
            vec![above_low, below_high, in_range],
            cond(
                op_result(in_range),
                edge(
                    expect_advance,
                    vec![
                        pav(check_parameters[1]),
                        pav(check_parameters[2]),
                        pav(check_parameters[3]),
                        pav(check_parameters[4]),
                    ],
                ),
                edge(utf8_error, Vec::new()),
            ),
        );

        let advance_types = vec![u64_type(), u64_type(), u8vec_type(), u64_type()];
        let advance_parameters = block_parameters(assembler, ns.p, expect_advance, &advance_types);
        let advance_step = u64_const(assembler, ns, expect_advance, 1);
        let advanced = assembler.op(
            ns.o,
            expect_advance,
            Opcode::IntAddChecked,
            vec![pav(advance_parameters[0]), op_result(advance_step)],
            vec![arith_result(u64_type())],
            Immediate::None,
        );
        append_block(
            assembler,
            expect_advance,
            function,
            advance_parameters.clone(),
            vec![advance_step, advanced],
            switch(
                op_result(advanced),
                vec![
                    (
                        BuiltinCase::Ok,
                        expect_decrement,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(advance_parameters[1]),
                            sav(advance_parameters[2]),
                            sav(advance_parameters[3]),
                        ],
                    ),
                    (BuiltinCase::Err, invariant_trap, Vec::new()),
                ],
            ),
        );

        let decrement_parameters =
            block_parameters(assembler, ns.p, expect_decrement, &advance_types);
        let decrement_step = u64_const(assembler, ns, expect_decrement, 1);
        let remaining = assembler.op(
            ns.o,
            expect_decrement,
            Opcode::IntSubChecked,
            vec![pav(decrement_parameters[3]), op_result(decrement_step)],
            vec![arith_result(u64_type())],
            Immediate::None,
        );
        let continuation_low = u8_const(assembler, ns, expect_decrement, 0x80);
        let continuation_high = u8_const(assembler, ns, expect_decrement, 0xbf);
        append_block(
            assembler,
            expect_decrement,
            function,
            decrement_parameters.clone(),
            vec![
                decrement_step,
                remaining,
                continuation_low,
                continuation_high,
            ],
            switch(
                op_result(remaining),
                vec![
                    (
                        BuiltinCase::Ok,
                        expect,
                        vec![
                            sav(decrement_parameters[0]),
                            sav(decrement_parameters[1]),
                            sav(decrement_parameters[2]),
                            SwitchArgument::CasePayload,
                            oav(continuation_low),
                            oav(continuation_high),
                        ],
                    ),
                    (BuiltinCase::Err, invariant_trap, Vec::new()),
                ],
            ),
        );
        Some(scan)
    } else {
        None
    };

    let range_parameters = block_parameters(assembler, ns.p, range_check, &scan_types);
    let range_total = assembler.op(
        ns.o,
        range_check,
        Opcode::VectorLen,
        vec![pav(range_parameters[2])],
        vec![u64_type()],
        Immediate::None,
    );
    let overruns = bool_op(
        assembler,
        ns,
        range_check,
        Opcode::GreaterThan,
        vec![pav(range_parameters[1]), op_result(range_total)],
    );
    let accepted_edge = match payload_start {
        Some(scan) => edge(scan, parameter_values(&range_parameters)),
        None => edge(
            trailing_check,
            vec![pav(range_parameters[1]), pav(range_parameters[2])],
        ),
    };
    append_block(
        assembler,
        range_check,
        function,
        range_parameters.clone(),
        vec![range_total, overruns],
        cond(
            op_result(overruns),
            edge(length_error, Vec::new()),
            accepted_edge,
        ),
    );

    let bounds_parameters = block_parameters(assembler, ns.p, bounds, &scan_types);
    let end = assembler.op(
        ns.o,
        bounds,
        Opcode::IntAddChecked,
        vec![pav(bounds_parameters[1]), pav(bounds_parameters[0])],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        bounds,
        function,
        bounds_parameters.clone(),
        vec![end],
        switch(
            op_result(end),
            vec![
                (
                    BuiltinCase::Ok,
                    range_check,
                    vec![
                        sav(bounds_parameters[1]),
                        SwitchArgument::CasePayload,
                        sav(bounds_parameters[2]),
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let length_parameters = block_parameters(
        assembler,
        ns.p,
        length_ready,
        &[TypeExpr::Tuple(vec![u64_type(), u64_type()]), u8vec_type()],
    );
    let payload_length = assembler.op(
        ns.o,
        length_ready,
        Opcode::TupleGet,
        vec![pav(length_parameters[0])],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let payload_offset = assembler.op(
        ns.o,
        length_ready,
        Opcode::TupleGet,
        vec![pav(length_parameters[0])],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let payload_cap = u64_const(assembler, ns, length_ready, 16_777_216);
    let too_large = bool_op(
        assembler,
        ns,
        length_ready,
        Opcode::GreaterThan,
        vec![op_result(payload_length), op_result(payload_cap)],
    );
    append_block(
        assembler,
        length_ready,
        function,
        length_parameters.clone(),
        vec![payload_length, payload_offset, payload_cap, too_large],
        cond(
            op_result(too_large),
            edge(resource_error, Vec::new()),
            edge(
                bounds,
                vec![
                    op_result(payload_length),
                    op_result(payload_offset),
                    pav(length_parameters[1]),
                ],
            ),
        ),
    );

    let vector_parameters = block_parameters(assembler, ns.p, vector_ready, &[u8vec_type()]);
    let start = u64_const(assembler, ns, vector_ready, 0);
    let width_constant = assembler.ku32(ns.k, 64);
    let width = assembler.cref(ns.o, vector_ready, width_constant, u32_type());
    let decoded = assembler.op(
        ns.o,
        vector_ready,
        Opcode::CallDirect,
        vec![pav(body), op_result(start), op_result(width), pav(unit)],
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
        vector_parameters.clone(),
        vec![start, width, decoded],
        switch(
            op_result(decoded),
            vec![
                (
                    BuiltinCase::Ok,
                    length_ready,
                    vec![SwitchArgument::CasePayload, sav(vector_parameters[0])],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let entry = vector_entry_block(
        assembler,
        ns,
        function,
        body,
        unit,
        resource_error,
        vector_ready,
    );
    unit_validator_graph(assembler, function, body, unit, entry, block_start)
}

/// Admits one unit-result leaf validator as a standalone image so each
/// primitive can be exercised directly.
fn leaf_validator_image(
    uses_decode: bool,
    build: impl FnOnce(&mut Asm, Ns, EntityId, EntityId) -> FunctionGraph,
) -> Image {
    let mut assembler = Asm::new();
    let decode_function = assembler.id(100);
    let function = assembler.id(100);
    // The bootstrap gate admits exactly the reached closure, so the uvar
    // decoder rides only when the validator calls it.
    let decode_graph = uses_decode.then(|| {
        build_decode(
            &mut assembler,
            Ns {
                k: 101,
                p: 101,
                b: 101,
                o: 101,
            },
            decode_function,
        )
        .0
    });
    let graph = build(
        &mut assembler,
        Ns {
            k: 102,
            p: 102,
            b: 102,
            o: 102,
        },
        function,
        decode_function,
    );
    let mut functions = vec![graph.clone()];
    functions.extend(decode_graph);
    // Likewise only the referenced bridge row may ride the admission.
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph,
        functions,
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

fn leaf_validator_outcome(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    body: &[u8],
) -> Result<(), Vec<u8>> {
    let outcome = execute_with_limits(
        package,
        approved,
        vec![bytes_input(body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!(
            "leaf validator must return a typed result: {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(result) = value.data else {
        panic!("leaf validator must return a Result: {value:?}")
    };
    match result {
        ResultConst::Ok(_) => Ok(()),
        ResultConst::Err(error) => {
            let ConstData::Bytes(code) = error.data else {
                panic!("leaf refusal must be Bytes")
            };
            Err(code)
        }
    }
}

/// Runs one native strict read over the whole body, the way every nested
/// `decode_nested_exact` does: cursor, read, then `check_finished`.
fn native_leaf_verdict(
    body: &[u8],
    read: impl FnOnce(&mut sley_scb1::ScbValueCursor<'_>) -> Result<(), sley_scb1::ScbError>,
) -> Result<(), Vec<u8>> {
    let verdict = (|| {
        let mut cursor = sley_scb1::ScbValueCursor::new(body)?;
        read(&mut cursor)?;
        cursor.check_finished()
    })();
    verdict.map_err(|error| error.code().as_str().as_bytes().to_vec())
}

/// Every case is checked twice: against the expected code the case names,
/// and against the native reader's verdict on the same bytes, so the Sley
/// validator is pinned to native precedence rather than to the author's
/// reading of it.
/// One leaf case: name, body, and the expected refusal code (`None` accepts).
type LeafCase<'a> = (&'a str, Vec<u8>, Option<&'a [u8]>);

fn assert_leaf_cases(
    name: &str,
    image: &Image,
    native: impl Fn(&mut sley_scb1::ScbValueCursor<'_>) -> Result<(), sley_scb1::ScbError>,
    cases: &[LeafCase<'_>],
) {
    let (package, approved) = admit_with_limits(image, codec_profile_limits());
    for (case, body, expected) in cases {
        let outcome = leaf_validator_outcome(&package, &approved, body);
        match expected {
            None => assert_eq!(outcome, Ok(()), "{name} {case} must be accepted"),
            Some(code) => assert_eq!(
                outcome.as_ref().map_err(Vec::as_slice),
                Err(*code),
                "{name} {case} refusal"
            ),
        }
        assert_eq!(
            outcome,
            native_leaf_verdict(body, &native),
            "{name} {case} native parity"
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn const_leaf_validators_match_native_primitive_rules() {
    let bool_image = leaf_validator_image(false, |assembler, ns, function, _| {
        build_bool_validate(assembler, ns, function)
    });
    assert_leaf_cases(
        "bool",
        &bool_image,
        |cursor| cursor.read_bool().map(drop),
        &[
            ("false", sley_scb1::encode_bool(false), None),
            ("true", sley_scb1::encode_bool(true), None),
            ("empty", Vec::new(), Some(b"SCB_LENGTH_OVERFLOW")),
            ("two", vec![2], Some(b"SCB_BOOL_INVALID")),
            ("trailing", vec![1, 0], Some(b"SCB_TRAILING_BYTES")),
            (
                "invalid_then_trailing",
                vec![9, 0],
                Some(b"SCB_BOOL_INVALID"),
            ),
        ],
    );

    let uvar_image = leaf_validator_image(false, |assembler, ns, function, _| {
        build_uvar128_validate(assembler, ns, function)
    });
    let mut twenty_zero_payload = vec![0x80; 19];
    twenty_zero_payload.push(0x00);
    let mut twenty_nonzero_payload = vec![0x80; 19];
    twenty_nonzero_payload.push(0x01);
    let mut twenty_continuations = vec![0x80; 20];
    twenty_continuations.push(0x00);
    let mut top_overflow = vec![0x80; 18];
    top_overflow.push(0x04);
    let mut top_overflow_continued = vec![0x80; 18];
    top_overflow_continued.push(0x84);
    top_overflow_continued.push(0x00);
    assert_leaf_cases(
        "uvar128",
        &uvar_image,
        |cursor| cursor.read_uvar128(128).map(drop),
        &[
            ("zero", sley_scb1::encode_uvar128(0), None),
            ("small", sley_scb1::encode_uvar128(127), None),
            ("two_bytes", sley_scb1::encode_uvar128(128), None),
            (
                "u64_max",
                sley_scb1::encode_uvar128(u128::from(u64::MAX)),
                None,
            ),
            ("u128_max", sley_scb1::encode_uvar128(u128::MAX), None),
            ("sint_min", sley_scb1::encode_sint128(i128::MIN), None),
            ("empty", Vec::new(), Some(b"SCB_LENGTH_OVERFLOW")),
            ("truncated", vec![0x80], Some(b"SCB_LENGTH_OVERFLOW")),
            (
                "non_minimal",
                vec![0x80, 0x00],
                Some(b"SCB_VARINT_NON_MINIMAL"),
            ),
            ("trailing", vec![0x01, 0x00], Some(b"SCB_TRAILING_BYTES")),
            ("top_overflow", top_overflow, Some(b"SCB_INTEGER_OVERFLOW")),
            (
                "top_overflow_continued",
                top_overflow_continued,
                Some(b"SCB_INTEGER_OVERFLOW"),
            ),
            (
                "twenty_zero_payload",
                twenty_zero_payload,
                Some(b"SCB_VARINT_NON_MINIMAL"),
            ),
            (
                "twenty_nonzero_payload",
                twenty_nonzero_payload,
                Some(b"SCB_INTEGER_OVERFLOW"),
            ),
            (
                "twenty_continuations",
                twenty_continuations,
                Some(b"SCB_INTEGER_OVERFLOW"),
            ),
        ],
    );

    let f32_image = leaf_validator_image(false, |assembler, ns, function, _| {
        build_float_bits_validate(assembler, ns, function, 4)
    });
    assert_leaf_cases(
        "f32",
        &f32_image,
        |cursor| cursor.read_f32_bits().map(drop),
        &[
            ("zero", sley_scb1::encode_f32_bits(0).unwrap(), None),
            (
                "one",
                sley_scb1::encode_f32_bits(1.0f32.to_bits()).unwrap(),
                None,
            ),
            (
                "negative_one",
                sley_scb1::encode_f32_bits((-1.0f32).to_bits()).unwrap(),
                None,
            ),
            (
                "infinity",
                sley_scb1::encode_f32_bits(f32::INFINITY.to_bits()).unwrap(),
                None,
            ),
            (
                "negative_infinity",
                sley_scb1::encode_f32_bits(f32::NEG_INFINITY.to_bits()).unwrap(),
                None,
            ),
            (
                "canonical_nan",
                sley_scb1::encode_f32_bits(0x7fc0_0000).unwrap(),
                None,
            ),
            (
                "max_finite",
                sley_scb1::encode_f32_bits(f32::MAX.to_bits()).unwrap(),
                None,
            ),
            (
                "negative_zero",
                0x8000_0000u32.to_be_bytes().to_vec(),
                Some(b"SCB_FLOAT_NON_CANONICAL"),
            ),
            (
                "signalling_nan",
                0x7f80_0001u32.to_be_bytes().to_vec(),
                Some(b"SCB_FLOAT_NON_CANONICAL"),
            ),
            (
                "negative_nan",
                0xffc0_0000u32.to_be_bytes().to_vec(),
                Some(b"SCB_FLOAT_NON_CANONICAL"),
            ),
            (
                "payload_nan",
                0x7fc0_0001u32.to_be_bytes().to_vec(),
                Some(b"SCB_FLOAT_NON_CANONICAL"),
            ),
            (
                "short",
                vec![0x3f, 0x80, 0x00],
                Some(b"SCB_LENGTH_OVERFLOW"),
            ),
            (
                "trailing",
                vec![0x3f, 0x80, 0x00, 0x00, 0x00],
                Some(b"SCB_TRAILING_BYTES"),
            ),
            (
                "non_canonical_then_trailing",
                vec![0x80, 0x00, 0x00, 0x00, 0x00],
                Some(b"SCB_FLOAT_NON_CANONICAL"),
            ),
        ],
    );

    let f64_image = leaf_validator_image(false, |assembler, ns, function, _| {
        build_float_bits_validate(assembler, ns, function, 8)
    });
    assert_leaf_cases(
        "f64",
        &f64_image,
        |cursor| cursor.read_f64_bits().map(drop),
        &[
            ("zero", sley_scb1::encode_f64_bits(0).unwrap(), None),
            (
                "one",
                sley_scb1::encode_f64_bits(1.0f64.to_bits()).unwrap(),
                None,
            ),
            (
                "negative_infinity",
                sley_scb1::encode_f64_bits(f64::NEG_INFINITY.to_bits()).unwrap(),
                None,
            ),
            (
                "canonical_nan",
                sley_scb1::encode_f64_bits(0x7ff8_0000_0000_0000).unwrap(),
                None,
            ),
            (
                "negative_zero",
                0x8000_0000_0000_0000u64.to_be_bytes().to_vec(),
                Some(b"SCB_FLOAT_NON_CANONICAL"),
            ),
            (
                "signalling_nan",
                0x7ff0_0000_0000_0001u64.to_be_bytes().to_vec(),
                Some(b"SCB_FLOAT_NON_CANONICAL"),
            ),
            (
                "negative_nan",
                0xfff8_0000_0000_0000u64.to_be_bytes().to_vec(),
                Some(b"SCB_FLOAT_NON_CANONICAL"),
            ),
            (
                "high_mantissa_nan",
                0x7ff8_0000_0000_0001u64.to_be_bytes().to_vec(),
                Some(b"SCB_FLOAT_NON_CANONICAL"),
            ),
            ("short", vec![0; 7], Some(b"SCB_LENGTH_OVERFLOW")),
            ("trailing", vec![0; 9], Some(b"SCB_TRAILING_BYTES")),
        ],
    );

    let bytes_image = leaf_validator_image(true, |assembler, ns, function, decode| {
        build_sized_payload_validate(assembler, ns, function, decode, false)
    });
    let mut oversized = sley_scb1::encode_uvar(16_777_217);
    oversized.push(0);
    let mut over_cap_exact = sley_scb1::encode_uvar(16_777_216);
    over_cap_exact.push(0);
    assert_leaf_cases(
        "bytes",
        &bytes_image,
        |cursor| cursor.read_bytes().map(drop),
        &[
            ("empty", sley_scb1::encode_bytes(&[]).unwrap(), None),
            (
                "payload",
                sley_scb1::encode_bytes(&[0xff, 0x00, 0x80]).unwrap(),
                None,
            ),
            (
                "invalid_utf8_allowed",
                sley_scb1::encode_bytes(&[0xc0, 0x80]).unwrap(),
                None,
            ),
            ("missing_prefix", Vec::new(), Some(b"SCB_LENGTH_OVERFLOW")),
            (
                "short_payload",
                vec![0x03, 0x01, 0x02],
                Some(b"SCB_LENGTH_OVERFLOW"),
            ),
            (
                "trailing",
                vec![0x01, 0x01, 0x02],
                Some(b"SCB_TRAILING_BYTES"),
            ),
            (
                "non_minimal_prefix",
                vec![0x80, 0x00],
                Some(b"SCB_VARINT_NON_MINIMAL"),
            ),
            ("oversized", oversized, Some(b"SCB_RESOURCE_LIMIT")),
            ("cap_short", over_cap_exact, Some(b"SCB_LENGTH_OVERFLOW")),
        ],
    );

    let text_image = leaf_validator_image(true, |assembler, ns, function, decode| {
        build_sized_payload_validate(assembler, ns, function, decode, true)
    });
    let text = |value: &str| sley_scb1::encode_text(value).unwrap();
    let raw = |bytes: &[u8]| sley_scb1::encode_bytes(bytes).unwrap();
    assert_leaf_cases(
        "text",
        &text_image,
        |cursor| cursor.read_text().map(drop),
        &[
            ("empty", text(""), None),
            ("ascii", text("sley"), None),
            ("two_byte", text("é"), None),
            ("three_byte", text("€"), None),
            ("four_byte", text("😀"), None),
            ("e0_floor", raw(&[0xe0, 0xa0, 0x80]), None),
            ("ed_ceiling", raw(&[0xed, 0x9f, 0xbf]), None),
            ("f0_floor", raw(&[0xf0, 0x90, 0x80, 0x80]), None),
            ("f4_ceiling", raw(&[0xf4, 0x8f, 0xbf, 0xbf]), None),
            ("mixed", text("a\u{7ff}\u{ffff}\u{10ffff}z"), None),
            (
                "stray_continuation",
                raw(&[0x80]),
                Some(b"SCB_UTF8_INVALID"),
            ),
            (
                "overlong_two",
                raw(&[0xc0, 0x80]),
                Some(b"SCB_UTF8_INVALID"),
            ),
            ("overlong_c1", raw(&[0xc1, 0xbf]), Some(b"SCB_UTF8_INVALID")),
            (
                "overlong_three",
                raw(&[0xe0, 0x9f, 0xbf]),
                Some(b"SCB_UTF8_INVALID"),
            ),
            (
                "surrogate",
                raw(&[0xed, 0xa0, 0x80]),
                Some(b"SCB_UTF8_INVALID"),
            ),
            (
                "overlong_four",
                raw(&[0xf0, 0x8f, 0xbf, 0xbf]),
                Some(b"SCB_UTF8_INVALID"),
            ),
            (
                "above_max",
                raw(&[0xf4, 0x90, 0x80, 0x80]),
                Some(b"SCB_UTF8_INVALID"),
            ),
            (
                "f5_lead",
                raw(&[0xf5, 0x80, 0x80, 0x80]),
                Some(b"SCB_UTF8_INVALID"),
            ),
            ("ff_lead", raw(&[0xff]), Some(b"SCB_UTF8_INVALID")),
            (
                "truncated_sequence",
                raw(&[0xe2, 0x82]),
                Some(b"SCB_UTF8_INVALID"),
            ),
            (
                "bad_continuation",
                raw(&[0xe2, 0x28, 0xa1]),
                Some(b"SCB_UTF8_INVALID"),
            ),
            (
                "short_payload",
                vec![0x02, 0x61],
                Some(b"SCB_LENGTH_OVERFLOW"),
            ),
            (
                "invalid_then_trailing",
                vec![0x01, 0x80, 0x61],
                Some(b"SCB_UTF8_INVALID"),
            ),
            (
                "trailing",
                vec![0x01, 0x61, 0x61],
                Some(b"SCB_TRAILING_BYTES"),
            ),
        ],
    );
}

fn option_bytes_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Option(Box::new(TypeExpr::Bytes))),
        error: Box::new(TypeExpr::Bytes),
    }
}

/// Accepts exactly the empty payload (`ConstData::Unit`, tag 1) and refuses
/// anything else as an invalid union, matching `1 if payload.is_empty()`.
fn build_empty_payload_validate(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let union_code = assembler.kbytes(ns.k, b"SCB_UNION_INVALID");
    let union_error = err_block(assembler, ns, function, result_type.clone(), union_code);
    let success = unit_success_block(assembler, ns, function, unit, &result_type);
    let entry = assembler.id(ns.b);
    let empty_constant = assembler.kbytes(ns.k, b"");
    let empty = assembler.cref(ns.o, entry, empty_constant, TypeExpr::Bytes);
    let is_empty = bool_op(
        assembler,
        ns,
        entry,
        Opcode::Equal,
        vec![pav(body), op_result(empty)],
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![empty, is_empty],
        cond(
            op_result(is_empty),
            edge(success, Vec::new()),
            edge(union_error, Vec::new()),
        ),
    );
    unit_validator_graph(assembler, function, body, unit, entry, block_start)
}

/// Projects the payload of a small closed union as an optional child:
/// tags `1..=last_payload_tag` yield `Some(payload)`; when `none_tag` is
/// set, that tag with an empty payload yields `None`; everything else is
/// `SCB_UNION_INVALID`. Covers `Option<Box<ConstValue>>` (`none_tag = 0`,
/// last tag 1) and `ResultConst` (no none tag, last tag 2).
#[allow(clippy::too_many_lines)]
fn build_tagged_child_projection(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    union_decoder: EntityId,
    none_tag: Option<u64>,
    last_payload_tag: u64,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = option_bytes_result_type();
    let option_bytes_type = TypeExpr::Option(Box::new(TypeExpr::Bytes));
    let union_type = TypeExpr::Tuple(vec![u64_type(), TypeExpr::Bytes]);
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let union_code = assembler.kbytes(ns.k, b"SCB_UNION_INVALID");
    let forward_error = forward_error_block(assembler, ns, function, &result_type);
    let union_error = err_block(assembler, ns, function, result_type.clone(), union_code);
    let some_success = assembler.id(ns.b);
    let payload_check = assembler.id(ns.b);
    let union_ready = assembler.id(ns.b);

    let some_parameters = block_parameters(assembler, ns.p, some_success, &[TypeExpr::Bytes]);
    let some = assembler.op(
        ns.o,
        some_success,
        Opcode::OptionSome,
        vec![pav(some_parameters[0])],
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    let some_ok = assembler.op(
        ns.o,
        some_success,
        Opcode::ResultOk,
        vec![op_result(some)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        some_success,
        function,
        some_parameters,
        vec![some, some_ok],
        ret(op_result(some_ok)),
    );

    let none_check = none_tag.map(|tag| {
        let none_success = assembler.id(ns.b);
        let none = assembler.op(
            ns.o,
            none_success,
            Opcode::OptionNone,
            Vec::new(),
            vec![option_bytes_type.clone()],
            Immediate::None,
        );
        let none_ok = assembler.op(
            ns.o,
            none_success,
            Opcode::ResultOk,
            vec![op_result(none)],
            vec![result_type.clone()],
            Immediate::None,
        );
        append_block(
            assembler,
            none_success,
            function,
            Vec::new(),
            vec![none, none_ok],
            ret(op_result(none_ok)),
        );

        let none_check = assembler.id(ns.b);
        let parameters =
            block_parameters(assembler, ns.p, none_check, &[u64_type(), TypeExpr::Bytes]);
        let expected = u64_const(assembler, ns, none_check, tag);
        let empty_constant = assembler.kbytes(ns.k, b"");
        let empty = assembler.cref(ns.o, none_check, empty_constant, TypeExpr::Bytes);
        let tag_matches = bool_op(
            assembler,
            ns,
            none_check,
            Opcode::Equal,
            vec![pav(parameters[0]), op_result(expected)],
        );
        let payload_empty = bool_op(
            assembler,
            ns,
            none_check,
            Opcode::Equal,
            vec![pav(parameters[1]), op_result(empty)],
        );
        let is_none = bool_op(
            assembler,
            ns,
            none_check,
            Opcode::BoolAnd,
            vec![op_result(tag_matches), op_result(payload_empty)],
        );
        append_block(
            assembler,
            none_check,
            function,
            parameters.clone(),
            vec![expected, empty, tag_matches, payload_empty, is_none],
            cond(
                op_result(is_none),
                edge(none_success, Vec::new()),
                edge(payload_check, parameter_values(&parameters)),
            ),
        );
        none_check
    });

    let payload_parameters = block_parameters(
        assembler,
        ns.p,
        payload_check,
        &[u64_type(), TypeExpr::Bytes],
    );
    let first = u64_const(assembler, ns, payload_check, 1);
    let last = u64_const(assembler, ns, payload_check, last_payload_tag);
    let at_least_first = bool_op(
        assembler,
        ns,
        payload_check,
        Opcode::GreaterEqual,
        vec![pav(payload_parameters[0]), op_result(first)],
    );
    let at_most_last = bool_op(
        assembler,
        ns,
        payload_check,
        Opcode::LessEqual,
        vec![pav(payload_parameters[0]), op_result(last)],
    );
    let in_range = bool_op(
        assembler,
        ns,
        payload_check,
        Opcode::BoolAnd,
        vec![op_result(at_least_first), op_result(at_most_last)],
    );
    append_block(
        assembler,
        payload_check,
        function,
        payload_parameters.clone(),
        vec![first, last, at_least_first, at_most_last, in_range],
        cond(
            op_result(in_range),
            edge(some_success, vec![pav(payload_parameters[1])]),
            edge(union_error, Vec::new()),
        ),
    );

    let union_parameters = block_parameters(
        assembler,
        ns.p,
        union_ready,
        std::slice::from_ref(&union_type),
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
        branch(edge(
            none_check.unwrap_or(payload_check),
            vec![op_result(tag), op_result(payload)],
        )),
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

/// How one two-field entry list contributes children.
#[derive(Clone, Copy)]
enum EntryListMode {
    /// `Vec<FieldConst>`: field 1 is an exact member identity, field 2 is
    /// the child value.
    RecordFields,
    /// `Vec<MapEntryConst>`: both fields are children, and the raw key
    /// bytes must be strictly increasing (`SCB_MAP_DUPLICATE` /
    /// `SCB_MAP_ORDER`).
    MapEntries,
}

/// Walks a canonical list of two-field records and returns the projected
/// children as an indexed map in list order.
#[allow(clippy::too_many_lines)]
fn build_entry_list_children(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    list_decoder: EntityId,
    record2_decoder: EntityId,
    fixed32_decoder: EntityId,
    mode: EntryListMode,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = generic_record_result_type();
    let map_type = generic_record_map_type();
    let option_bytes_type = TypeExpr::Option(Box::new(TypeExpr::Bytes));
    let pair_type = TypeExpr::Tuple(vec![TypeExpr::Bytes; 2]);
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let forward_error = forward_error_block(assembler, ns, function, &result_type);
    let invariant_trap = trap_block(assembler, ns, function);
    let success = assembler.id(ns.b);
    let advance = assembler.id(ns.b);
    let insert = assembler.id(ns.b);
    let pair_ready = assembler.id(ns.b);
    let element_call = assembler.id(ns.b);
    let loop_check = assembler.id(ns.b);
    let list_ready = assembler.id(ns.b);
    // (list, index, out, out_index, previous key)
    let state_types = vec![
        map_type.clone(),
        u64_type(),
        map_type.clone(),
        u64_type(),
        TypeExpr::Bytes,
    ];

    let success_parameters =
        block_parameters(assembler, ns.p, success, std::slice::from_ref(&map_type));
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

    let advance_parameters = block_parameters(assembler, ns.p, advance, &state_types);
    let step = u64_const(assembler, ns, advance, 1);
    let next = assembler.op(
        ns.o,
        advance,
        Opcode::IntAddChecked,
        vec![pav(advance_parameters[1]), op_result(step)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    append_block(
        assembler,
        advance,
        function,
        advance_parameters.clone(),
        vec![step, next],
        switch(
            op_result(next),
            vec![
                (
                    BuiltinCase::Ok,
                    loop_check,
                    vec![
                        sav(advance_parameters[0]),
                        SwitchArgument::CasePayload,
                        sav(advance_parameters[2]),
                        sav(advance_parameters[3]),
                        sav(advance_parameters[4]),
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    // insert(first, second, list, index, out, out_index): appends the
    // children this mode projects and carries the key forward.
    let mut insert_types = vec![TypeExpr::Bytes, TypeExpr::Bytes];
    insert_types.extend_from_slice(&state_types[..4]);
    let insert_parameters = block_parameters(assembler, ns.p, insert, &insert_types);
    let mut insert_operations = Vec::new();
    let insert_step = u64_const(assembler, ns, insert, 1);
    insert_operations.push(insert_step);
    match mode {
        EntryListMode::RecordFields => {
            let inserted = assembler.op(
                ns.o,
                insert,
                Opcode::MapInsert,
                vec![
                    pav(insert_parameters[4]),
                    pav(insert_parameters[5]),
                    pav(insert_parameters[1]),
                ],
                vec![map_type.clone()],
                Immediate::None,
            );
            let advanced = assembler.op(
                ns.o,
                insert,
                Opcode::IntAddChecked,
                vec![pav(insert_parameters[5]), op_result(insert_step)],
                vec![arith_result(u64_type())],
                Immediate::None,
            );
            insert_operations.extend([inserted, advanced]);
            append_block(
                assembler,
                insert,
                function,
                insert_parameters.clone(),
                insert_operations,
                switch(
                    op_result(advanced),
                    vec![
                        (
                            BuiltinCase::Ok,
                            advance,
                            vec![
                                sav(insert_parameters[2]),
                                sav(insert_parameters[3]),
                                oav(inserted),
                                SwitchArgument::CasePayload,
                                sav(insert_parameters[0]),
                            ],
                        ),
                        (BuiltinCase::Err, invariant_trap, Vec::new()),
                    ],
                ),
            );
        }
        EntryListMode::MapEntries => {
            let first_inserted = assembler.op(
                ns.o,
                insert,
                Opcode::MapInsert,
                vec![
                    pav(insert_parameters[4]),
                    pav(insert_parameters[5]),
                    pav(insert_parameters[0]),
                ],
                vec![map_type.clone()],
                Immediate::None,
            );
            let second_index = assembler.op(
                ns.o,
                insert,
                Opcode::IntAddChecked,
                vec![pav(insert_parameters[5]), op_result(insert_step)],
                vec![arith_result(u64_type())],
                Immediate::None,
            );
            insert_operations.extend([first_inserted, second_index]);
            // The second insert needs the unwrapped index, so it continues
            // in a follow-on block reached through the checked add.
            let second_insert = assembler.id(ns.b);
            let second_types = vec![
                TypeExpr::Bytes,
                TypeExpr::Bytes,
                map_type.clone(),
                u64_type(),
                map_type.clone(),
                u64_type(),
            ];
            let second_parameters = block_parameters(assembler, ns.p, second_insert, &second_types);
            let second_step = u64_const(assembler, ns, second_insert, 1);
            let second_inserted = assembler.op(
                ns.o,
                second_insert,
                Opcode::MapInsert,
                vec![
                    pav(second_parameters[4]),
                    pav(second_parameters[5]),
                    pav(second_parameters[1]),
                ],
                vec![map_type.clone()],
                Immediate::None,
            );
            let next_out = assembler.op(
                ns.o,
                second_insert,
                Opcode::IntAddChecked,
                vec![pav(second_parameters[5]), op_result(second_step)],
                vec![arith_result(u64_type())],
                Immediate::None,
            );
            append_block(
                assembler,
                second_insert,
                function,
                second_parameters.clone(),
                vec![second_step, second_inserted, next_out],
                switch(
                    op_result(next_out),
                    vec![
                        (
                            BuiltinCase::Ok,
                            advance,
                            vec![
                                sav(second_parameters[2]),
                                sav(second_parameters[3]),
                                oav(second_inserted),
                                SwitchArgument::CasePayload,
                                sav(second_parameters[0]),
                            ],
                        ),
                        (BuiltinCase::Err, invariant_trap, Vec::new()),
                    ],
                ),
            );
            append_block(
                assembler,
                insert,
                function,
                insert_parameters.clone(),
                insert_operations,
                switch(
                    op_result(second_index),
                    vec![
                        (
                            BuiltinCase::Ok,
                            second_insert,
                            vec![
                                sav(insert_parameters[0]),
                                sav(insert_parameters[1]),
                                sav(insert_parameters[2]),
                                sav(insert_parameters[3]),
                                oav(first_inserted),
                                SwitchArgument::CasePayload,
                            ],
                        ),
                        (BuiltinCase::Err, invariant_trap, Vec::new()),
                    ],
                ),
            );
        }
    }

    // pair_ready(pair, list, index, out, out_index, previous): validates
    // the mode's first field, then inserts.
    let mut pair_types = vec![pair_type.clone()];
    pair_types.extend_from_slice(&state_types);
    let pair_parameters = block_parameters(assembler, ns.p, pair_ready, &pair_types);
    let first = assembler.op(
        ns.o,
        pair_ready,
        Opcode::TupleGet,
        vec![pav(pair_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let second = assembler.op(
        ns.o,
        pair_ready,
        Opcode::TupleGet,
        vec![pav(pair_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let insert_edge_arguments = |first: ValueRef, second: ValueRef| {
        vec![
            first,
            second,
            pav(pair_parameters[1]),
            pav(pair_parameters[2]),
            pav(pair_parameters[3]),
            pav(pair_parameters[4]),
        ]
    };
    match mode {
        EntryListMode::RecordFields => {
            let member_valid = assembler.op(
                ns.o,
                pair_ready,
                Opcode::CallDirect,
                vec![op_result(first), pav(unit)],
                vec![bytes_validation_result_type()],
                Immediate::Function(FunctionRefValue {
                    function: fixed32_decoder,
                    type_arguments: Vec::new(),
                }),
            );
            let mut ok_arguments = vec![oav(first), oav(second)];
            ok_arguments.extend(pair_parameters[1..5].iter().copied().map(sav));
            append_block(
                assembler,
                pair_ready,
                function,
                pair_parameters.clone(),
                vec![first, second, member_valid],
                switch(
                    op_result(member_valid),
                    vec![
                        (BuiltinCase::Ok, insert, ok_arguments),
                        (
                            BuiltinCase::Err,
                            forward_error,
                            vec![SwitchArgument::CasePayload],
                        ),
                    ],
                ),
            );
        }
        EntryListMode::MapEntries => {
            let duplicate_code = assembler.kbytes(ns.k, b"SCB_MAP_DUPLICATE");
            let order_code = assembler.kbytes(ns.k, b"SCB_MAP_ORDER");
            let duplicate_error =
                err_block(assembler, ns, function, result_type.clone(), duplicate_code);
            let order_error = err_block(assembler, ns, function, result_type.clone(), order_code);
            let order_check = assembler.id(ns.b);
            let zero = u64_const(assembler, ns, pair_ready, 0);
            let is_first = bool_op(
                assembler,
                ns,
                pair_ready,
                Opcode::Equal,
                vec![pav(pair_parameters[2]), op_result(zero)],
            );
            append_block(
                assembler,
                pair_ready,
                function,
                pair_parameters.clone(),
                vec![first, second, zero, is_first],
                cond(
                    op_result(is_first),
                    edge(
                        insert,
                        insert_edge_arguments(op_result(first), op_result(second)),
                    ),
                    edge(
                        order_check,
                        vec![
                            op_result(first),
                            op_result(second),
                            pav(pair_parameters[1]),
                            pav(pair_parameters[2]),
                            pav(pair_parameters[3]),
                            pav(pair_parameters[4]),
                            pav(pair_parameters[5]),
                        ],
                    ),
                ),
            );

            // order_check(key, value, list, index, out, out_index, previous)
            let mut order_types = vec![TypeExpr::Bytes, TypeExpr::Bytes];
            order_types.extend_from_slice(&state_types);
            let order_parameters = block_parameters(assembler, ns.p, order_check, &order_types);
            let duplicate = bool_op(
                assembler,
                ns,
                order_check,
                Opcode::Equal,
                vec![pav(order_parameters[6]), pav(order_parameters[0])],
            );
            let increasing = bool_op(
                assembler,
                ns,
                order_check,
                Opcode::LessThan,
                vec![pav(order_parameters[6]), pav(order_parameters[0])],
            );
            let order_gate = assembler.id(ns.b);
            append_block(
                assembler,
                order_check,
                function,
                order_parameters.clone(),
                vec![duplicate, increasing],
                cond(
                    op_result(duplicate),
                    edge(duplicate_error, Vec::new()),
                    edge(
                        order_gate,
                        vec![
                            op_result(increasing),
                            pav(order_parameters[0]),
                            pav(order_parameters[1]),
                            pav(order_parameters[2]),
                            pav(order_parameters[3]),
                            pav(order_parameters[4]),
                            pav(order_parameters[5]),
                        ],
                    ),
                ),
            );
            let mut gate_types = vec![TypeExpr::Bool, TypeExpr::Bytes, TypeExpr::Bytes];
            gate_types.extend_from_slice(&state_types[..4]);
            let gate_parameters = block_parameters(assembler, ns.p, order_gate, &gate_types);
            append_block(
                assembler,
                order_gate,
                function,
                gate_parameters.clone(),
                Vec::new(),
                cond(
                    pav(gate_parameters[0]),
                    edge(insert, parameter_values(&gate_parameters[1..])),
                    edge(order_error, Vec::new()),
                ),
            );
        }
    }

    let element_types = {
        let mut types = vec![TypeExpr::Bytes];
        types.extend_from_slice(&state_types);
        types
    };
    let element_parameters = block_parameters(assembler, ns.p, element_call, &element_types);
    let projected = assembler.op(
        ns.o,
        element_call,
        Opcode::CallDirect,
        vec![pav(element_parameters[0]), pav(unit)],
        vec![exact_record_projection_result_type(2)],
        Immediate::Function(FunctionRefValue {
            function: record2_decoder,
            type_arguments: Vec::new(),
        }),
    );
    let mut projected_arguments = vec![SwitchArgument::CasePayload];
    projected_arguments.extend(element_parameters[1..].iter().copied().map(sav));
    append_block(
        assembler,
        element_call,
        function,
        element_parameters,
        vec![projected],
        switch(
            op_result(projected),
            vec![
                (BuiltinCase::Ok, pair_ready, projected_arguments),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let loop_parameters = block_parameters(assembler, ns.p, loop_check, &state_types);
    let item = assembler.op(
        ns.o,
        loop_check,
        Opcode::MapGet,
        vec![pav(loop_parameters[0]), pav(loop_parameters[1])],
        vec![option_bytes_type],
        Immediate::None,
    );
    let mut item_arguments = vec![SwitchArgument::CasePayload];
    item_arguments.extend(loop_parameters.iter().copied().map(sav));
    append_block(
        assembler,
        loop_check,
        function,
        loop_parameters.clone(),
        vec![item],
        switch(
            op_result(item),
            vec![
                (BuiltinCase::None, success, vec![sav(loop_parameters[2])]),
                (BuiltinCase::Some, element_call, item_arguments),
            ],
        ),
    );

    let list_parameters =
        block_parameters(assembler, ns.p, list_ready, std::slice::from_ref(&map_type));
    let map_new_result = TypeExpr::Result {
        ok: Box::new(map_type.clone()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let out = assembler.op(
        ns.o,
        list_ready,
        Opcode::MapNew,
        Vec::new(),
        vec![map_new_result],
        Immediate::None,
    );
    let start = u64_const(assembler, ns, list_ready, 0);
    let out_start = u64_const(assembler, ns, list_ready, 0);
    let no_key_constant = assembler.kbytes(ns.k, b"");
    let no_key = assembler.cref(ns.o, list_ready, no_key_constant, TypeExpr::Bytes);
    append_block(
        assembler,
        list_ready,
        function,
        list_parameters.clone(),
        vec![out, start, out_start, no_key],
        switch(
            op_result(out),
            vec![
                (
                    BuiltinCase::Ok,
                    loop_check,
                    vec![
                        sav(list_parameters[0]),
                        oav(start),
                        SwitchArgument::CasePayload,
                        oav(out_start),
                        oav(no_key),
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let entry = assembler.id(ns.b);
    let decoded_list = assembler.op(
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
        vec![decoded_list],
        switch(
            op_result(decoded_list),
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

/// What the final field of a prefixed record projects.
#[derive(Clone, Copy)]
enum LastFieldChildren {
    /// An indexed child map (`RecordConst.fields`).
    Map(EntityId),
    /// An optional single child (`VariantConst.payload`).
    Optional(EntityId),
}

/// Projects a record whose leading fields are exact 32-byte identities and
/// whose last field yields the node's children.
#[allow(clippy::too_many_lines)]
fn build_prefixed_record_child(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    exact_record_decoder: EntityId,
    fixed32_decoder: EntityId,
    identity_count: usize,
    last: LastFieldChildren,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let field_count = identity_count + 1;
    let (last_decoder, result_type) = match last {
        LastFieldChildren::Map(decoder) => (decoder, generic_record_result_type()),
        LastFieldChildren::Optional(decoder) => (decoder, option_bytes_result_type()),
    };
    let tuple_type = TypeExpr::Tuple(vec![TypeExpr::Bytes; field_count]);
    let field_types = vec![TypeExpr::Bytes; field_count];
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let forward_error = forward_error_block(assembler, ns, function, &result_type);
    let identity_checks = (0..identity_count)
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let last_call = assembler.id(ns.b);
    let record_ready = assembler.id(ns.b);

    let last_parameters = block_parameters(assembler, ns.p, last_call, &field_types);
    let children = assembler.op(
        ns.o,
        last_call,
        Opcode::CallDirect,
        vec![pav(last_parameters[identity_count]), pav(unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: last_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        last_call,
        function,
        last_parameters,
        vec![children],
        ret(op_result(children)),
    );

    for (index, block) in identity_checks.iter().copied().enumerate() {
        let parameters = block_parameters(assembler, ns.p, block, &field_types);
        let validated = assembler.op(
            ns.o,
            block,
            Opcode::CallDirect,
            vec![pav(parameters[index]), pav(unit)],
            vec![bytes_validation_result_type()],
            Immediate::Function(FunctionRefValue {
                function: fixed32_decoder,
                type_arguments: Vec::new(),
            }),
        );
        let destination = identity_checks.get(index + 1).copied().unwrap_or(last_call);
        append_block(
            assembler,
            block,
            function,
            parameters.clone(),
            vec![validated],
            switch(
                op_result(validated),
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

    let record_parameters = block_parameters(
        assembler,
        ns.p,
        record_ready,
        std::slice::from_ref(&tuple_type),
    );
    let projected_fields = (0..field_count)
        .map(|index| {
            assembler.op(
                ns.o,
                record_ready,
                Opcode::TupleGet,
                vec![pav(record_parameters[0])],
                vec![TypeExpr::Bytes],
                Immediate::Index(u32::try_from(index).expect("record field index fits u32")),
            )
        })
        .collect::<Vec<_>>();
    append_block(
        assembler,
        record_ready,
        function,
        record_parameters,
        projected_fields.clone(),
        branch(edge(
            identity_checks.first().copied().unwrap_or(last_call),
            projected_fields.iter().copied().map(op_result).collect(),
        )),
    );

    let entry = assembler.id(ns.b);
    let decoded_record = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![exact_record_projection_result_type(field_count)],
        Immediate::Function(FunctionRefValue {
            function: exact_record_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
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

/// One `ConstData` union arm as the child projector sees it.
#[derive(Clone, Copy)]
enum ConstDataArm {
    /// A unit-result payload validator (leaf families).
    Unit(EntityId),
    /// A decoder returning an indexed child map.
    List(EntityId),
    /// A decoder returning an optional single child.
    Optional(EntityId),
}

/// Shallow `ConstValue` child projector: validates the node's two-field
/// record, its recursive `value_type`, and its `data` union, and returns the
/// node's direct and list children in the worklist driver's shape.
#[allow(clippy::too_many_lines)]
fn build_const_value_children_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    record2_decoder: EntityId,
    type_expr_decoder: EntityId,
    union_decoder: EntityId,
    arms: &[ConstDataArm; 16],
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = type_expr_children_result_type();
    let map_type = generic_record_map_type();
    let option_bytes_type = TypeExpr::Option(Box::new(TypeExpr::Bytes));
    let tuple_type = children_tuple_type();
    let pair_type = TypeExpr::Tuple(vec![TypeExpr::Bytes; 2]);
    let union_type = TypeExpr::Tuple(vec![u64_type(), TypeExpr::Bytes]);
    let map_new_result = TypeExpr::Result {
        ok: Box::new(map_type.clone()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let union_code = assembler.kbytes(ns.k, b"SCB_UNION_INVALID");
    let forward_error = forward_error_block(assembler, ns, function, &result_type);
    let union_error = err_block(assembler, ns, function, result_type.clone(), union_code);
    let invariant_trap = trap_block(assembler, ns, function);
    let success = assembler.id(ns.b);
    let finish_unit = assembler.id(ns.b);
    let finish_list = assembler.id(ns.b);
    let finish_optional = assembler.id(ns.b);
    let arm_calls = arms.iter().map(|_| assembler.id(ns.b)).collect::<Vec<_>>();
    let tag_checks = arms.iter().map(|_| assembler.id(ns.b)).collect::<Vec<_>>();
    let union_ready = assembler.id(ns.b);
    let union_call = assembler.id(ns.b);
    let type_check = assembler.id(ns.b);

    let success_parameters = block_parameters(
        assembler,
        ns.p,
        success,
        &[
            option_bytes_type.clone(),
            option_bytes_type.clone(),
            map_type.clone(),
            u64_type(),
            u64_type(),
            u64_type(),
            u64_type(),
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

    let offset_types = [u64_type(), u64_type(), u64_type(), u64_type()];
    let unit_parameters = block_parameters(assembler, ns.p, finish_unit, &offset_types);
    let unit_first = assembler.op(
        ns.o,
        finish_unit,
        Opcode::OptionNone,
        Vec::new(),
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    let unit_second = assembler.op(
        ns.o,
        finish_unit,
        Opcode::OptionNone,
        Vec::new(),
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    let unit_map = assembler.op(
        ns.o,
        finish_unit,
        Opcode::MapNew,
        Vec::new(),
        vec![map_new_result.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        finish_unit,
        function,
        unit_parameters.clone(),
        vec![unit_first, unit_second, unit_map],
        switch(
            op_result(unit_map),
            vec![
                (
                    BuiltinCase::Ok,
                    success,
                    vec![
                        oav(unit_first),
                        oav(unit_second),
                        SwitchArgument::CasePayload,
                        sav(unit_parameters[0]),
                        sav(unit_parameters[1]),
                        sav(unit_parameters[2]),
                        sav(unit_parameters[3]),
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let mut list_types = vec![map_type.clone()];
    list_types.extend(offset_types.iter().cloned());
    let list_parameters = block_parameters(assembler, ns.p, finish_list, &list_types);
    let list_first = assembler.op(
        ns.o,
        finish_list,
        Opcode::OptionNone,
        Vec::new(),
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    let list_second = assembler.op(
        ns.o,
        finish_list,
        Opcode::OptionNone,
        Vec::new(),
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        finish_list,
        function,
        list_parameters.clone(),
        vec![list_first, list_second],
        branch(edge(
            success,
            vec![
                op_result(list_first),
                op_result(list_second),
                pav(list_parameters[0]),
                pav(list_parameters[1]),
                pav(list_parameters[2]),
                pav(list_parameters[3]),
                pav(list_parameters[4]),
            ],
        )),
    );

    let mut optional_types = vec![option_bytes_type.clone()];
    optional_types.extend(offset_types.iter().cloned());
    let optional_parameters = block_parameters(assembler, ns.p, finish_optional, &optional_types);
    let optional_second = assembler.op(
        ns.o,
        finish_optional,
        Opcode::OptionNone,
        Vec::new(),
        vec![option_bytes_type.clone()],
        Immediate::None,
    );
    let optional_map = assembler.op(
        ns.o,
        finish_optional,
        Opcode::MapNew,
        Vec::new(),
        vec![map_new_result],
        Immediate::None,
    );
    append_block(
        assembler,
        finish_optional,
        function,
        optional_parameters.clone(),
        vec![optional_second, optional_map],
        switch(
            op_result(optional_map),
            vec![
                (
                    BuiltinCase::Ok,
                    success,
                    vec![
                        sav(optional_parameters[0]),
                        oav(optional_second),
                        SwitchArgument::CasePayload,
                        sav(optional_parameters[1]),
                        sav(optional_parameters[2]),
                        sav(optional_parameters[3]),
                        sav(optional_parameters[4]),
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    for (index, (block, arm)) in arm_calls
        .iter()
        .copied()
        .zip(arms.iter().copied())
        .enumerate()
    {
        let parameters = block_parameters(assembler, ns.p, block, &[TypeExpr::Bytes]);
        let (decoder, call_result_type, finish, mut ok_arguments) = match arm {
            ConstDataArm::Unit(decoder) => (
                decoder,
                unit_validation_result_type(),
                finish_unit,
                Vec::new(),
            ),
            ConstDataArm::List(decoder) => (
                decoder,
                generic_record_result_type(),
                finish_list,
                vec![SwitchArgument::CasePayload],
            ),
            ConstDataArm::Optional(decoder) => (
                decoder,
                option_bytes_result_type(),
                finish_optional,
                vec![SwitchArgument::CasePayload],
            ),
        };
        // Native depth beneath a `ConstValue` node at d: the `data` union is
        // at d+1 and the family payload at d+2. Children offsets count the
        // containers between the node and the child minus one; the container
        // offset is the deepest mandatory container (crates/sley-mutate/src/
        // codec.rs `ConstData`, `RecordConst`, `VariantConst`,
        // `decode_map_entries`, `ResultConst`).
        let tag = index + 1;
        let offsets = match tag {
            // Unit and the seven scalar leaves: union at d+1, leaf at d+2.
            1..=8 => DepthOffsets {
                first: 0,
                second: 0,
                listed: 0,
                container: 1,
            },
            // Sequence: Vec at d+2, elements at d+3.
            9 => DepthOffsets {
                first: 0,
                second: 0,
                listed: 2,
                container: 2,
            },
            // Record: RecordConst at d+2, fields Vec at d+3, field record at
            // d+4, value at d+5.
            10 => DepthOffsets {
                first: 0,
                second: 0,
                listed: 4,
                container: 3,
            },
            // Variant: VariantConst at d+2, payload option at d+3, value d+4.
            11 => DepthOffsets {
                first: 3,
                second: 0,
                listed: 0,
                container: 3,
            },
            // Map: entries at d+2, entry record at d+3, key and value at d+4.
            12 => DepthOffsets {
                first: 0,
                second: 0,
                listed: 3,
                container: 2,
            },
            // Option and Result: an inner union at d+2, the value at d+3.
            13 | 14 => DepthOffsets {
                first: 2,
                second: 0,
                listed: 0,
                container: 2,
            },
            // FunctionRef: record at d+2 with its type-argument list at d+3;
            // BuiltinFailure: record at d+2.
            15 => DepthOffsets {
                first: 0,
                second: 0,
                listed: 0,
                container: 3,
            },
            16 => DepthOffsets {
                first: 0,
                second: 0,
                listed: 0,
                container: 2,
            },
            _ => unreachable!("ConstData has sixteen families"),
        }
        .constants(assembler, ns, block);
        ok_arguments.extend(offsets.iter().copied().map(oav));
        let called = assembler.op(
            ns.o,
            block,
            Opcode::CallDirect,
            vec![pav(parameters[0]), pav(unit)],
            vec![call_result_type],
            Immediate::Function(FunctionRefValue {
                function: decoder,
                type_arguments: Vec::new(),
            }),
        );
        let mut operations = offsets.to_vec();
        operations.push(called);
        append_block(
            assembler,
            block,
            function,
            parameters,
            operations,
            switch(
                op_result(called),
                vec![
                    (BuiltinCase::Ok, finish, ok_arguments),
                    (
                        BuiltinCase::Err,
                        forward_error,
                        vec![SwitchArgument::CasePayload],
                    ),
                ],
            ),
        );
    }

    for (index, block) in tag_checks.iter().copied().enumerate() {
        let parameters = block_parameters(assembler, ns.p, block, &[u64_type(), TypeExpr::Bytes]);
        let expected = u64_const(
            assembler,
            ns,
            block,
            u64::try_from(index + 1).expect("union tag fits u64"),
        );
        let tag_matches = bool_op(
            assembler,
            ns,
            block,
            Opcode::Equal,
            vec![pav(parameters[0]), op_result(expected)],
        );
        let fallback = tag_checks.get(index + 1).copied().unwrap_or(union_error);
        let fallback_arguments = if index + 1 < tag_checks.len() {
            parameter_values(&parameters)
        } else {
            Vec::new()
        };
        append_block(
            assembler,
            block,
            function,
            parameters.clone(),
            vec![expected, tag_matches],
            cond(
                op_result(tag_matches),
                edge(arm_calls[index], vec![pav(parameters[1])]),
                edge(fallback, fallback_arguments),
            ),
        );
    }

    let union_parameters = block_parameters(
        assembler,
        ns.p,
        union_ready,
        std::slice::from_ref(&union_type),
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
        branch(edge(
            tag_checks[0],
            vec![op_result(tag), op_result(payload)],
        )),
    );

    let union_call_parameters = block_parameters(assembler, ns.p, union_call, &[TypeExpr::Bytes]);
    let decoded_union = assembler.op(
        ns.o,
        union_call,
        Opcode::CallDirect,
        vec![pav(union_call_parameters[0]), pav(unit)],
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
        union_call_parameters,
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

    let type_parameters = block_parameters(
        assembler,
        ns.p,
        type_check,
        std::slice::from_ref(&pair_type),
    );
    let value_type = assembler.op(
        ns.o,
        type_check,
        Opcode::TupleGet,
        vec![pav(type_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let data = assembler.op(
        ns.o,
        type_check,
        Opcode::TupleGet,
        vec![pav(type_parameters[0])],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let type_valid = assembler.op(
        ns.o,
        type_check,
        Opcode::CallDirect,
        vec![op_result(value_type), pav(unit)],
        vec![bytes_validation_result_type()],
        Immediate::Function(FunctionRefValue {
            function: type_expr_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        type_check,
        function,
        type_parameters,
        vec![value_type, data, type_valid],
        switch(
            op_result(type_valid),
            vec![
                (BuiltinCase::Ok, union_call, vec![oav(data)]),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let entry = assembler.id(ns.b);
    let decoded_record = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), pav(unit)],
        vec![exact_record_projection_result_type(2)],
        Immediate::Function(FunctionRefValue {
            function: record2_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![decoded_record],
        switch(
            op_result(decoded_record),
            vec![
                (
                    BuiltinCase::Ok,
                    type_check,
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

/// Function identities a `ConstValue`-bearing schema composes with.
#[derive(Clone, Copy)]
struct ConstValueClosure {
    const_value: EntityId,
    list: EntityId,
    union: EntityId,
    record: EntityId,
    decode: EntityId,
    fixed32: EntityId,
    exact_uvar: EntityId,
    bounded_uvar: EntityId,
    type_expr: EntityId,
    entity_ids: EntityId,
    record2: EntityId,
    record3: EntityId,
}

impl ConstValueClosure {
    fn decoders(self, exact_record: EntityId) -> SimpleSchemaDecoders {
        SimpleSchemaDecoders {
            union: self.union,
            exact_record,
            fixed32: self.fixed32,
            exact_uvar: self.exact_uvar,
            bounded_uvar: self.bounded_uvar,
            type_expr: self.type_expr,
            entity_ids: self.entity_ids,
        }
    }
}

/// Builds the complete recursive `ConstValue` closure (namespaces
/// `110..=141`) and returns its graphs, driver first.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn build_const_value_closure(assembler: &mut Asm) -> (ConstValueClosure, Vec<FunctionGraph>) {
    let decode_function = assembler.id(110);
    let record_function = assembler.id(110);
    let union_function = assembler.id(110);
    let list_function = assembler.id(110);
    let fixed32_function = assembler.id(110);
    let entity_id_collection_function = assembler.id(110);
    let exact_uvar_function = assembler.id(110);
    let bounded_uvar_function = assembler.id(110);
    let type_expr_leaf_function = assembler.id(110);
    let record2_function = assembler.id(110);
    let record3_function = assembler.id(110);
    let type_expr_children_function = assembler.id(110);
    let type_expr_recursive_function = assembler.id(110);
    let empty_payload_function = assembler.id(110);
    let bool_function = assembler.id(110);
    let uvar128_function = assembler.id(110);
    let f32_function = assembler.id(110);
    let f64_function = assembler.id(110);
    let bytes_function = assembler.id(110);
    let text_function = assembler.id(110);
    let record_fields_function = assembler.id(110);
    let record_const_function = assembler.id(110);
    let option_child_function = assembler.id(110);
    let variant_const_function = assembler.id(110);
    let map_entries_function = assembler.id(110);
    let result_child_function = assembler.id(110);
    let type_arguments_function = assembler.id(110);
    let function_ref_function = assembler.id(110);
    let builtin_failure_function = assembler.id(110);
    let const_children_function = assembler.id(110);
    let const_value_function = assembler.id(110);
    let (decode_graph, _) = build_decode(
        assembler,
        Ns {
            k: 111,
            p: 111,
            b: 111,
            o: 111,
        },
        decode_function,
    );
    let record_graph = build_generic_record_decode(
        assembler,
        Ns {
            k: 112,
            p: 112,
            b: 112,
            o: 112,
        },
        record_function,
        decode_function,
    );
    let union_graph = build_generic_union_decode(
        assembler,
        Ns {
            k: 113,
            p: 113,
            b: 113,
            o: 113,
        },
        union_function,
        decode_function,
    );
    let list_graph = build_generic_list_decode(
        assembler,
        Ns {
            k: 114,
            p: 114,
            b: 114,
            o: 114,
        },
        list_function,
        decode_function,
    );
    let fixed32_graph = build_fixed32_decode(
        assembler,
        Ns {
            k: 115,
            p: 115,
            b: 115,
            o: 115,
        },
        fixed32_function,
    );
    let entity_id_collection_graph = build_entity_id_collection_decode(
        assembler,
        Ns {
            k: 116,
            p: 116,
            b: 116,
            o: 116,
        },
        entity_id_collection_function,
        list_function,
        fixed32_function,
    );
    let exact_uvar_graph = build_exact_uvar_decode(
        assembler,
        Ns {
            k: 117,
            p: 117,
            b: 117,
            o: 117,
        },
        exact_uvar_function,
        decode_function,
    );
    let bounded_uvar_graph = build_bounded_uvar_decode(
        assembler,
        Ns {
            k: 118,
            p: 118,
            b: 118,
            o: 118,
        },
        bounded_uvar_function,
        exact_uvar_function,
    );
    let type_expr_leaf_graph = build_type_expr_leaf_decode(
        assembler,
        Ns {
            k: 119,
            p: 119,
            b: 119,
            o: 119,
        },
        type_expr_leaf_function,
        union_function,
        fixed32_function,
        exact_uvar_function,
        bounded_uvar_function,
    );
    let record2_graph = build_exact_record_projection(
        assembler,
        Ns {
            k: 120,
            p: 120,
            b: 120,
            o: 120,
        },
        record2_function,
        decode_function,
        record_function,
        2,
    );
    let record3_graph = build_exact_record_projection(
        assembler,
        Ns {
            k: 121,
            p: 121,
            b: 121,
            o: 121,
        },
        record3_function,
        decode_function,
        record_function,
        3,
    );
    let type_expr_children_graph = build_type_expr_children_decode(
        assembler,
        Ns {
            k: 122,
            p: 122,
            b: 122,
            o: 122,
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
    // The standalone constant closure roots its `ConstValue` at depth 0, so
    // the node's `value_type` field sits at depth 1.
    let type_expr_recursive_graphs = build_type_expr_recursive_decode_at(
        assembler,
        Ns {
            k: 123,
            p: 123,
            b: 123,
            o: 123,
        },
        type_expr_recursive_function,
        type_expr_children_function,
        1,
    )
    .1;
    let empty_payload_graph = build_empty_payload_validate(
        assembler,
        Ns {
            k: 124,
            p: 124,
            b: 124,
            o: 124,
        },
        empty_payload_function,
    );
    let bool_graph = build_bool_validate(
        assembler,
        Ns {
            k: 125,
            p: 125,
            b: 125,
            o: 125,
        },
        bool_function,
    );
    let uvar128_graph = build_uvar128_validate(
        assembler,
        Ns {
            k: 126,
            p: 126,
            b: 126,
            o: 126,
        },
        uvar128_function,
    );
    let f32_graph = build_float_bits_validate(
        assembler,
        Ns {
            k: 127,
            p: 127,
            b: 127,
            o: 127,
        },
        f32_function,
        4,
    );
    let f64_graph = build_float_bits_validate(
        assembler,
        Ns {
            k: 128,
            p: 128,
            b: 128,
            o: 128,
        },
        f64_function,
        8,
    );
    let bytes_graph = build_sized_payload_validate(
        assembler,
        Ns {
            k: 129,
            p: 129,
            b: 129,
            o: 129,
        },
        bytes_function,
        decode_function,
        false,
    );
    let text_graph = build_sized_payload_validate(
        assembler,
        Ns {
            k: 130,
            p: 130,
            b: 130,
            o: 130,
        },
        text_function,
        decode_function,
        true,
    );
    let record_fields_graph = build_entry_list_children(
        assembler,
        Ns {
            k: 131,
            p: 131,
            b: 131,
            o: 131,
        },
        record_fields_function,
        list_function,
        record2_function,
        fixed32_function,
        EntryListMode::RecordFields,
    );
    let record_const_graph = build_prefixed_record_child(
        assembler,
        Ns {
            k: 132,
            p: 132,
            b: 132,
            o: 132,
        },
        record_const_function,
        record2_function,
        fixed32_function,
        1,
        LastFieldChildren::Map(record_fields_function),
    );
    let option_child_graph = build_tagged_child_projection(
        assembler,
        Ns {
            k: 133,
            p: 133,
            b: 133,
            o: 133,
        },
        option_child_function,
        union_function,
        Some(0),
        1,
    );
    let variant_const_graph = build_prefixed_record_child(
        assembler,
        Ns {
            k: 134,
            p: 134,
            b: 134,
            o: 134,
        },
        variant_const_function,
        record3_function,
        fixed32_function,
        2,
        LastFieldChildren::Optional(option_child_function),
    );
    let map_entries_graph = build_entry_list_children(
        assembler,
        Ns {
            k: 135,
            p: 135,
            b: 135,
            o: 135,
        },
        map_entries_function,
        list_function,
        record2_function,
        fixed32_function,
        EntryListMode::MapEntries,
    );
    let result_child_graph = build_tagged_child_projection(
        assembler,
        Ns {
            k: 136,
            p: 136,
            b: 136,
            o: 136,
        },
        result_child_function,
        union_function,
        None,
        2,
    );
    let type_arguments_graph = build_unit_list_validate(
        assembler,
        Ns {
            k: 137,
            p: 137,
            b: 137,
            o: 137,
        },
        type_arguments_function,
        list_function,
        type_expr_recursive_function,
        bytes_validation_result_type(),
    );
    let leaf_decoders = SimpleSchemaDecoders {
        union: union_function,
        exact_record: record2_function,
        fixed32: fixed32_function,
        exact_uvar: exact_uvar_function,
        bounded_uvar: bounded_uvar_function,
        type_expr: type_expr_recursive_function,
        entity_ids: entity_id_collection_function,
    };
    let function_ref_graph = build_projected_record_validate(
        assembler,
        Ns {
            k: 138,
            p: 138,
            b: 138,
            o: 138,
        },
        function_ref_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(type_arguments_function),
        ],
        leaf_decoders,
    );
    let builtin_failure_graph = build_projected_record_validate(
        assembler,
        Ns {
            k: 139,
            p: 139,
            b: 139,
            o: 139,
        },
        builtin_failure_function,
        &[
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 5,
            },
            SimpleFieldValidator::ExactUvar(16),
        ],
        leaf_decoders,
    );
    let const_children_graph = build_const_value_children_decode(
        assembler,
        Ns {
            k: 140,
            p: 140,
            b: 140,
            o: 140,
        },
        const_children_function,
        record2_function,
        type_expr_recursive_function,
        union_function,
        &[
            ConstDataArm::Unit(empty_payload_function),
            ConstDataArm::Unit(bool_function),
            ConstDataArm::Unit(uvar128_function),
            ConstDataArm::Unit(uvar128_function),
            ConstDataArm::Unit(f32_function),
            ConstDataArm::Unit(f64_function),
            ConstDataArm::Unit(bytes_function),
            ConstDataArm::Unit(text_function),
            ConstDataArm::List(list_function),
            ConstDataArm::List(record_const_function),
            ConstDataArm::Optional(variant_const_function),
            ConstDataArm::List(map_entries_function),
            ConstDataArm::Optional(option_child_function),
            ConstDataArm::Optional(result_child_function),
            ConstDataArm::Unit(function_ref_function),
            ConstDataArm::Unit(builtin_failure_function),
        ],
    );
    let const_value_graphs = build_type_expr_recursive_decode(
        assembler,
        Ns {
            k: 141,
            p: 141,
            b: 141,
            o: 141,
        },
        const_value_function,
        const_children_function,
    );
    let closure = ConstValueClosure {
        const_value: const_value_function,
        list: list_function,
        union: union_function,
        record: record_function,
        decode: decode_function,
        fixed32: fixed32_function,
        exact_uvar: exact_uvar_function,
        bounded_uvar: bounded_uvar_function,
        type_expr: type_expr_recursive_function,
        entity_ids: entity_id_collection_function,
        record2: record2_function,
        record3: record3_function,
    };
    (
        closure,
        vec![
            const_value_graphs[0].clone(),
            const_value_graphs[1].clone(),
            const_children_graph,
            builtin_failure_graph,
            function_ref_graph,
            type_arguments_graph,
            result_child_graph,
            map_entries_graph,
            variant_const_graph,
            option_child_graph,
            record_const_graph,
            record_fields_graph,
            text_graph,
            bytes_graph,
            f64_graph,
            f32_graph,
            uvar128_graph,
            bool_graph,
            empty_payload_graph,
            type_expr_recursive_graphs[0].clone(),
            type_expr_recursive_graphs[1].clone(),
            type_expr_children_graph,
            record3_graph,
            record2_graph,
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
    )
}

fn codec_bridge_imports() -> Vec<AdapterImport> {
    vec![
        frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
        frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
        frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
    ]
}

/// Bootstrap `ConstValue` decoder: the worklist driver over the constant
/// child projector, with every leaf and composite arm reachable.
fn const_value_decode_image() -> Image {
    let mut assembler = Asm::new();
    let (_, functions) = build_const_value_closure(&mut assembler);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: functions[0].clone(),
        functions,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: codec_bridge_imports(),
        constants: assembler.constants,
    }
}

/// An entity schema whose fields include recursive constants: the
/// `ConstValue` closure plus the schema's own exact record projection and
/// entry function (namespaces `142..=144`).
fn const_bearing_schema_image(
    expected_kind: u64,
    validators: impl FnOnce(
        &mut Asm,
        ConstValueClosure,
    ) -> (Vec<SimpleFieldValidator>, Vec<FunctionGraph>),
) -> Image {
    let mut assembler = Asm::new();
    let (closure, mut functions) = build_const_value_closure(&mut assembler);
    let (validators, extra) = validators(&mut assembler, closure);
    let schema_record_function = assembler.id(110);
    let function = assembler.id(110);
    let schema_record_graph = build_exact_record_projection(
        &mut assembler,
        Ns {
            k: 143,
            p: 143,
            b: 143,
            o: 143,
        },
        schema_record_function,
        closure.decode,
        closure.record,
        validators.len(),
    );
    let graph = build_simple_entity_schema_decode(
        &mut assembler,
        Ns {
            k: 144,
            p: 144,
            b: 144,
            o: 144,
        },
        function,
        expected_kind,
        &validators,
        closure.decoders(schema_record_function),
    );
    let mut all = vec![graph.clone(), schema_record_graph];
    all.extend(extra);
    all.append(&mut functions);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph,
        functions: all,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: codec_bridge_imports(),
        constants: assembler.constants,
    }
}

fn constant_schema_decode_image() -> Image {
    const_bearing_schema_image(9, |_, closure| {
        (
            vec![SimpleFieldValidator::Bytes(closure.const_value)],
            Vec::new(),
        )
    })
}

fn capability_requirement_schema_decode_image() -> Image {
    const_bearing_schema_image(12, |assembler, closure| {
        let scopes_function = assembler.id(110);
        let scopes_graph = build_unit_list_validate(
            assembler,
            Ns {
                k: 142,
                p: 142,
                b: 142,
                o: 142,
            },
            scopes_function,
            closure.list,
            closure.const_value,
            bytes_validation_result_type(),
        );
        (
            vec![
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::Unit(scopes_function),
                SimpleFieldValidator::EntityIds { ordered: true },
            ],
            vec![scopes_graph],
        )
    })
}

fn const_value_verdict(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    body: &[u8],
) -> Result<Vec<u8>, Vec<u8>> {
    let outcome = execute_with_limits(
        package,
        approved,
        vec![bytes_input(body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!(
            "ConstValue decoder must return a typed result: {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(result) = value.data else {
        panic!("ConstValue decoder must return a Result: {value:?}")
    };
    match result {
        ResultConst::Ok(accepted) => {
            let ConstData::Bytes(bytes) = accepted.data else {
                panic!("ConstValue acceptance must be Bytes")
            };
            Ok(bytes)
        }
        ResultConst::Err(error) => {
            let ConstData::Bytes(code) = error.data else {
                panic!("ConstValue refusal must be Bytes")
            };
            Err(code)
        }
    }
}

fn native_const_value_verdict(body: &[u8]) -> Result<Vec<u8>, Vec<u8>> {
    sley_mutate::decode_const_value(body)
        .map(|_| body.to_vec())
        .map_err(|error| error.code().as_str().as_bytes().to_vec())
}

fn const_of(value_type: TypeExpr, data: ConstData) -> ConstValue {
    ConstValue { value_type, data }
}

/// One constant exercising every `ConstData` family, nested several levels.
#[allow(clippy::too_many_lines)]
fn every_family_const_value() -> ConstValue {
    use sley_ssmc::{
        BuiltinFailureValue, FieldConst, FunctionRefValue, MapEntryConst, MemberId, RecordConst,
        VariantConst,
    };

    let unit = const_of(TypeExpr::Unit, ConstData::Unit);
    let flag = const_of(TypeExpr::Bool, ConstData::Bool(true));
    let signed = const_of(
        TypeExpr::SInt(IntegerWidth::from_bits(128)),
        ConstData::SInt(i128::MIN),
    );
    let unsigned = const_of(
        TypeExpr::UInt(IntegerWidth::from_bits(128)),
        ConstData::UInt(u128::MAX),
    );
    let single = const_of(TypeExpr::F32, ConstData::F32Bits(1.5f32.to_bits()));
    let double = const_of(TypeExpr::F64, ConstData::F64Bits((-2.25f64).to_bits()));
    let bytes = const_of(TypeExpr::Bytes, ConstData::Bytes(vec![0x00, 0xff, 0x80]));
    let text = const_of(TypeExpr::Text, ConstData::Text("sley é 😀".to_owned()));
    let record = const_of(
        TypeExpr::Named(sley_ssmc::NamedType {
            definition: EntityId::from_bytes([0xe1; 32]),
            arguments: Vec::new(),
        }),
        ConstData::Record(RecordConst {
            definition: EntityId::from_bytes([0xe1; 32]),
            fields: vec![
                FieldConst {
                    member_id: MemberId::from_bytes([0xe2; 32]),
                    value: flag.clone(),
                },
                FieldConst {
                    member_id: MemberId::from_bytes([0xe3; 32]),
                    value: const_of(
                        TypeExpr::Option(Box::new(TypeExpr::Text)),
                        ConstData::Option(Some(Box::new(text.clone()))),
                    ),
                },
            ],
        }),
    );
    let variant_with_payload = const_of(
        TypeExpr::Unit,
        ConstData::Variant(VariantConst {
            definition: EntityId::from_bytes([0xe4; 32]),
            member_id: MemberId::from_bytes([0xe5; 32]),
            payload: Some(Box::new(unsigned.clone())),
        }),
    );
    let variant_without_payload = const_of(
        TypeExpr::Unit,
        ConstData::Variant(VariantConst {
            definition: EntityId::from_bytes([0xe4; 32]),
            member_id: MemberId::from_bytes([0xe6; 32]),
            payload: None,
        }),
    );
    let map = const_of(
        TypeExpr::OrderedMap {
            key: Box::new(TypeExpr::UInt(IntegerWidth::from_bits(8))),
            value: Box::new(TypeExpr::Text),
        },
        ConstData::Map(vec![
            MapEntryConst {
                key: const_of(
                    TypeExpr::UInt(IntegerWidth::from_bits(8)),
                    ConstData::UInt(1),
                ),
                value: text.clone(),
            },
            MapEntryConst {
                key: const_of(
                    TypeExpr::UInt(IntegerWidth::from_bits(8)),
                    ConstData::UInt(2),
                ),
                value: bytes.clone(),
            },
        ]),
    );
    let none = const_of(
        TypeExpr::Option(Box::new(TypeExpr::Unit)),
        ConstData::Option(None),
    );
    let some = const_of(
        TypeExpr::Option(Box::new(TypeExpr::Bool)),
        ConstData::Option(Some(Box::new(signed.clone()))),
    );
    let ok = const_of(
        TypeExpr::Result {
            ok: Box::new(TypeExpr::Unit),
            error: Box::new(TypeExpr::Bytes),
        },
        ConstData::Result(ResultConst::Ok(Box::new(unit.clone()))),
    );
    let err = const_of(
        TypeExpr::Result {
            ok: Box::new(TypeExpr::Unit),
            error: Box::new(TypeExpr::Bytes),
        },
        ConstData::Result(ResultConst::Err(Box::new(map.clone()))),
    );
    let function_ref = const_of(
        TypeExpr::Unit,
        ConstData::FunctionRef(FunctionRefValue {
            function: EntityId::from_bytes([0xe7; 32]),
            type_arguments: vec![TypeExpr::Bool, TypeExpr::Vector(Box::new(TypeExpr::Bytes))],
        }),
    );
    let failure = const_of(
        TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        ConstData::BuiltinFailure(BuiltinFailureValue {
            kind: BuiltinFailureKind::Index,
            code: u16::MAX,
        }),
    );
    const_of(
        TypeExpr::Vector(Box::new(TypeExpr::Unit)),
        ConstData::Sequence(vec![
            unit,
            flag,
            signed,
            unsigned,
            single,
            double,
            bytes,
            text,
            record,
            variant_with_payload,
            variant_without_payload,
            map,
            none,
            some,
            ok,
            err,
            function_ref,
            failure,
        ]),
    )
}

#[test]
fn const_value_decoder_accepts_every_family_recursively() {
    let image = const_value_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "CONST_VALUE functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 33);
    assert_eq!(image.parameters.len(), 2_010);
    assert_eq!(image.blocks.len(), 554);
    assert_eq!(image.operations.len(), 1_172);
    assert_eq!(image.constants.len(), 441);
    assert_eq!(package.image_bytes.len(), 141_198);
    assert_eq!(
        approved.package_digest,
        [
            0x34, 0xdc, 0xca, 0xf8, 0x2e, 0xce, 0xfe, 0xb6, 0xe7, 0xf6, 0xa9, 0xbd, 0x67, 0x70,
            0x14, 0x11, 0x68, 0x45, 0x4f, 0x80, 0x74, 0x67, 0xdc, 0x51, 0xfb, 0x58, 0x64, 0x67,
            0x2e, 0x07, 0xf3, 0x39,
        ]
    );

    let value = every_family_const_value();
    let body = sley_mutate::encode_const_value(&value).expect("native encodes fixture");
    assert_eq!(native_const_value_verdict(&body), Ok(body.clone()));
    let outcome = execute_with_limits(
        &package,
        &approved,
        vec![bytes_input(&body), unit_input()],
        codec_profile_limits(),
    );
    eprintln!(
        "CONST_VALUE_FIXTURE bytes={} instructions={}",
        body.len(),
        outcome.instruction_count
    );
    assert_eq!(body.len(), 936);
    assert_eq!(outcome.instruction_count, 85_018);
    assert_eq!(const_value_verdict(&package, &approved, &body), Ok(body));

    let mut chain = const_of(TypeExpr::Unit, ConstData::Unit);
    for _ in 0..20 {
        chain = const_of(
            TypeExpr::Option(Box::new(TypeExpr::Unit)),
            ConstData::Option(Some(Box::new(chain))),
        );
    }
    let deep = sley_mutate::encode_const_value(&chain).expect("native encodes deep fixture");
    assert_eq!(native_const_value_verdict(&deep), Ok(deep.clone()));
    assert_eq!(const_value_verdict(&package, &approved, &deep), Ok(deep));
}

#[test]
#[allow(clippy::too_many_lines)]
fn const_value_decoder_rejects_every_family_boundary() {
    let image = const_value_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    let encode = |value: &ConstValue| sley_mutate::encode_const_value(value).unwrap();
    let node = |value_type: Vec<u8>, data: Vec<u8>| {
        sley_scb1::encode_record(&[(1, value_type), (2, data)]).unwrap()
    };
    let unit_type = sley_scb1::encode_union(1, &[]).unwrap();
    let data = |tag: u32, payload: &[u8]| sley_scb1::encode_union(tag, payload).unwrap();
    let unit_node = node(unit_type.clone(), data(1, &[]));
    let bool_node = |byte: u8| node(unit_type.clone(), data(2, &[byte]));
    let text = |bytes: &[u8]| sley_scb1::encode_bytes(bytes).unwrap();
    let list = |elements: &[Vec<u8>]| sley_scb1::encode_list(elements).unwrap();
    let id32 = |fill: u8| vec![fill; 32];
    let entry = |key: &[u8], value: &[u8]| {
        sley_scb1::encode_record(&[(1, key.to_vec()), (2, value.to_vec())]).unwrap()
    };
    let key = |value: u8| {
        node(
            unit_type.clone(),
            data(4, &sley_scb1::encode_uvar128(value.into())),
        )
    };

    let cases: Vec<(&str, Vec<u8>, &[u8])> = vec![
        (
            "missing_data_field",
            sley_scb1::encode_record(&[(1, unit_type.clone())]).unwrap(),
            b"SCB_FIELD_MISSING",
        ),
        (
            "unknown_node_field",
            sley_scb1::encode_record(&[(1, unit_type.clone()), (2, data(1, &[])), (3, Vec::new())])
                .unwrap(),
            b"SCB_FIELD_UNKNOWN",
        ),
        (
            "malformed_value_type",
            node(sley_scb1::encode_union(21, &[]).unwrap(), data(1, &[])),
            b"SCB_UNION_INVALID",
        ),
        (
            "unknown_data_tag",
            node(unit_type.clone(), data(17, &[])),
            b"SCB_UNION_INVALID",
        ),
        (
            "zero_data_tag",
            node(unit_type.clone(), data(0, &[])),
            b"SCB_UNION_INVALID",
        ),
        (
            "nonempty_unit",
            node(unit_type.clone(), data(1, &[0])),
            b"SCB_UNION_INVALID",
        ),
        ("bad_bool", bool_node(2), b"SCB_BOOL_INVALID"),
        (
            "non_minimal_sint",
            node(unit_type.clone(), data(3, &[0x80, 0x00])),
            b"SCB_VARINT_NON_MINIMAL",
        ),
        (
            "uint_overflow",
            node(unit_type.clone(), data(4, &[0x80; 20])),
            b"SCB_INTEGER_OVERFLOW",
        ),
        (
            "negative_zero_f32",
            node(unit_type.clone(), data(5, &0x8000_0000u32.to_be_bytes())),
            b"SCB_FLOAT_NON_CANONICAL",
        ),
        (
            "short_f64",
            node(unit_type.clone(), data(6, &[0; 7])),
            b"SCB_LENGTH_OVERFLOW",
        ),
        (
            "short_bytes",
            node(unit_type.clone(), data(7, &[0x02, 0x01])),
            b"SCB_LENGTH_OVERFLOW",
        ),
        (
            "invalid_text",
            node(unit_type.clone(), data(8, &text(&[0xc0, 0x80]))),
            b"SCB_UTF8_INVALID",
        ),
        (
            "nested_bad_bool_in_sequence",
            node(
                unit_type.clone(),
                data(9, &list(&[unit_node.clone(), bool_node(7)])),
            ),
            b"SCB_BOOL_INVALID",
        ),
        (
            "short_record_definition",
            node(
                unit_type.clone(),
                data(
                    10,
                    &sley_scb1::encode_record(&[(1, vec![0xe1; 31]), (2, list(&[]))]).unwrap(),
                ),
            ),
            b"SCB_LENGTH_OVERFLOW",
        ),
        (
            "record_field_short_member",
            node(
                unit_type.clone(),
                data(
                    10,
                    &sley_scb1::encode_record(&[
                        (1, id32(0xe1)),
                        (2, list(&[entry(&[0xe2; 31], &unit_node)])),
                    ])
                    .unwrap(),
                ),
            ),
            b"SCB_LENGTH_OVERFLOW",
        ),
        (
            "record_field_bad_value",
            node(
                unit_type.clone(),
                data(
                    10,
                    &sley_scb1::encode_record(&[
                        (1, id32(0xe1)),
                        (2, list(&[entry(&id32(0xe2), &bool_node(9))])),
                    ])
                    .unwrap(),
                ),
            ),
            b"SCB_BOOL_INVALID",
        ),
        (
            "variant_missing_payload_field",
            node(
                unit_type.clone(),
                data(
                    11,
                    &sley_scb1::encode_record(&[(1, id32(0xe4)), (2, id32(0xe5))]).unwrap(),
                ),
            ),
            b"SCB_FIELD_MISSING",
        ),
        (
            "variant_bad_option_tag",
            node(
                unit_type.clone(),
                data(
                    11,
                    &sley_scb1::encode_record(&[
                        (1, id32(0xe4)),
                        (2, id32(0xe5)),
                        (3, data(2, &[])),
                    ])
                    .unwrap(),
                ),
            ),
            b"SCB_UNION_INVALID",
        ),
        (
            "variant_bad_payload",
            node(
                unit_type.clone(),
                data(
                    11,
                    &sley_scb1::encode_record(&[
                        (1, id32(0xe4)),
                        (2, id32(0xe5)),
                        (3, data(1, &bool_node(4))),
                    ])
                    .unwrap(),
                ),
            ),
            b"SCB_BOOL_INVALID",
        ),
        (
            "map_duplicate_key",
            node(
                unit_type.clone(),
                data(
                    12,
                    &list(&[entry(&key(1), &unit_node), entry(&key(1), &unit_node)]),
                ),
            ),
            b"SCB_MAP_DUPLICATE",
        ),
        (
            "map_unordered_keys",
            node(
                unit_type.clone(),
                data(
                    12,
                    &list(&[entry(&key(2), &unit_node), entry(&key(1), &unit_node)]),
                ),
            ),
            b"SCB_MAP_ORDER",
        ),
        (
            "map_bad_value",
            node(
                unit_type.clone(),
                data(12, &list(&[entry(&key(1), &bool_node(5))])),
            ),
            b"SCB_BOOL_INVALID",
        ),
        (
            "map_entry_unknown_field",
            node(
                unit_type.clone(),
                data(
                    12,
                    &list(&[sley_scb1::encode_record(&[
                        (1, key(1)),
                        (2, unit_node.clone()),
                        (3, Vec::new()),
                    ])
                    .unwrap()]),
                ),
            ),
            b"SCB_FIELD_UNKNOWN",
        ),
        (
            "nonempty_none",
            node(unit_type.clone(), data(13, &data(0, &[0]))),
            b"SCB_UNION_INVALID",
        ),
        (
            "option_bad_tag",
            node(unit_type.clone(), data(13, &data(2, &unit_node))),
            b"SCB_UNION_INVALID",
        ),
        (
            "option_bad_child",
            node(unit_type.clone(), data(13, &data(1, &bool_node(3)))),
            b"SCB_BOOL_INVALID",
        ),
        (
            "result_zero_tag",
            node(unit_type.clone(), data(14, &data(0, &[]))),
            b"SCB_UNION_INVALID",
        ),
        (
            "result_third_tag",
            node(unit_type.clone(), data(14, &data(3, &unit_node))),
            b"SCB_UNION_INVALID",
        ),
        (
            "result_bad_err_child",
            node(unit_type.clone(), data(14, &data(2, &bool_node(6)))),
            b"SCB_BOOL_INVALID",
        ),
        (
            "function_ref_bad_type_argument",
            node(
                unit_type.clone(),
                data(
                    15,
                    &sley_scb1::encode_record(&[
                        (1, id32(0xe7)),
                        (2, list(&[sley_scb1::encode_union(21, &[]).unwrap()])),
                    ])
                    .unwrap(),
                ),
            ),
            b"SCB_UNION_INVALID",
        ),
        (
            "function_ref_short_identity",
            node(
                unit_type.clone(),
                data(
                    15,
                    &sley_scb1::encode_record(&[(1, vec![0xe7; 31]), (2, list(&[]))]).unwrap(),
                ),
            ),
            b"SCB_LENGTH_OVERFLOW",
        ),
        (
            "failure_kind_zero",
            node(
                unit_type.clone(),
                data(
                    16,
                    &sley_scb1::encode_record(&[
                        (1, sley_scb1::encode_uvar(0)),
                        (2, sley_scb1::encode_uvar(1)),
                    ])
                    .unwrap(),
                ),
            ),
            b"SCB_UNION_INVALID",
        ),
        (
            "failure_kind_six",
            node(
                unit_type.clone(),
                data(
                    16,
                    &sley_scb1::encode_record(&[
                        (1, sley_scb1::encode_uvar(6)),
                        (2, sley_scb1::encode_uvar(1)),
                    ])
                    .unwrap(),
                ),
            ),
            b"SCB_UNION_INVALID",
        ),
        (
            "failure_code_wide",
            node(
                unit_type.clone(),
                data(
                    16,
                    &sley_scb1::encode_record(&[
                        (1, sley_scb1::encode_uvar(1)),
                        (2, sley_scb1::encode_uvar(u64::from(u16::MAX) + 1)),
                    ])
                    .unwrap(),
                ),
            ),
            b"SCB_INTEGER_OVERFLOW",
        ),
        (
            "deep_sequence_bad_leaf",
            {
                let mut inner = bool_node(8);
                for _ in 0..6 {
                    inner = node(
                        unit_type.clone(),
                        data(9, &list(&[unit_node.clone(), inner])),
                    );
                }
                inner
            },
            b"SCB_BOOL_INVALID",
        ),
        (
            "trailing_after_node",
            {
                let mut bytes = encode(&const_of(TypeExpr::Unit, ConstData::Unit));
                bytes.push(0);
                bytes
            },
            b"SCB_TRAILING_BYTES",
        ),
    ];
    for (name, body, expected) in cases {
        let verdict = const_value_verdict(&package, &approved, &body);
        assert_eq!(
            verdict.as_ref().map_err(Vec::as_slice),
            Err(expected),
            "{name} refusal"
        );
        assert_eq!(
            verdict,
            native_const_value_verdict(&body),
            "{name} native parity"
        );
    }
}

fn constant_schema_body(value: ConstValue) -> Vec<u8> {
    use sley_mutate::value::{ConstantBody, EntityBodyValue};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0xf1; 32]),
        body: EntityBodyValue::Constant(ConstantBody { value }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema Constant fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

fn capability_requirement_schema_body(scopes: Vec<ConstValue>) -> Vec<u8> {
    use sley_mutate::value::{CapabilityRequirementBody, EntityBodyValue, EntityIdSet};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0xf2; 32]),
        body: EntityBodyValue::CapabilityRequirement(CapabilityRequirementBody {
            effect: EntityId::from_bytes([0xf3; 32]),
            allowed_scopes: scopes,
            constraint_contracts: EntityIdSet::from_unsorted(vec![
                EntityId::from_bytes([0xf5; 32]),
                EntityId::from_bytes([0xf4; 32]),
            ])
            .expect("constraint contracts are canonical"),
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema CapabilityRequirement fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

/// A compact constant touching a record, a map, an option, and text.
fn scope_const_value(seed: u8) -> ConstValue {
    use sley_ssmc::{FieldConst, MapEntryConst, MemberId, RecordConst};

    let byte_type = TypeExpr::UInt(IntegerWidth::from_bits(8));
    const_of(
        TypeExpr::Named(sley_ssmc::NamedType {
            definition: EntityId::from_bytes([seed; 32]),
            arguments: Vec::new(),
        }),
        ConstData::Record(RecordConst {
            definition: EntityId::from_bytes([seed; 32]),
            fields: vec![
                FieldConst {
                    member_id: MemberId::from_bytes([seed.wrapping_add(1); 32]),
                    value: const_of(
                        TypeExpr::OrderedMap {
                            key: Box::new(byte_type.clone()),
                            value: Box::new(TypeExpr::Text),
                        },
                        ConstData::Map(vec![MapEntryConst {
                            key: const_of(byte_type.clone(), ConstData::UInt(u128::from(seed))),
                            value: const_of(TypeExpr::Text, ConstData::Text("scope".to_owned())),
                        }]),
                    ),
                },
                FieldConst {
                    member_id: MemberId::from_bytes([seed.wrapping_add(2); 32]),
                    value: const_of(
                        TypeExpr::Option(Box::new(TypeExpr::Bool)),
                        ConstData::Option(Some(Box::new(const_of(
                            TypeExpr::Bool,
                            ConstData::Bool(seed.is_multiple_of(2)),
                        )))),
                    ),
                },
            ],
        }),
    )
}

fn assert_schema_projection(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    body: &[u8],
    kind: u32,
    field_count: u32,
) {
    let expected = exact_entity_body_fields(body, kind, field_count);
    let outcome = execute_with_limits(
        package,
        approved,
        vec![bytes_input(body), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!(
            "kind {kind} schema decoder must return: {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = value.data else {
        panic!("kind {kind} schema decoder must accept native body: {value:?}")
    };
    let ConstData::Sequence(fields) = decoded.data else {
        panic!("kind {kind} schema decoder must return a field tuple")
    };
    assert_eq!(fields.len(), expected.len());
    for (field, expected) in fields.iter().zip(expected) {
        assert_eq!(field.data, ConstData::Bytes(expected));
    }
}

#[test]
fn constant_schema_decoder_accepts_recursive_values() {
    let image = constant_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "CONSTANT_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 35);
    assert_eq!(image.parameters.len(), 2_031);
    assert_eq!(image.blocks.len(), 574);
    assert_eq!(image.operations.len(), 1_207);
    assert_eq!(image.constants.len(), 451);
    assert_eq!(package.image_bytes.len(), 145_012);
    assert_eq!(
        approved.package_digest,
        [
            0x13, 0x09, 0x58, 0x2a, 0xbe, 0x0c, 0x96, 0xd3, 0xd2, 0xb6, 0x3a, 0x61, 0x05, 0xe2,
            0xa0, 0x26, 0x91, 0x45, 0x28, 0xb2, 0xaf, 0x47, 0x8b, 0x33, 0xd4, 0xad, 0x58, 0xc6,
            0x3f, 0x7f, 0xcf, 0x9b,
        ]
    );

    assert_schema_projection(
        &package,
        &approved,
        &constant_schema_body(scope_const_value(0x10)),
        9,
        1,
    );
    assert_schema_projection(
        &package,
        &approved,
        &constant_schema_body(const_of(TypeExpr::Unit, ConstData::Unit)),
        9,
        1,
    );
}

#[test]
fn constant_schema_decoder_rejects_kind_fields_and_nested_values() {
    let body = constant_schema_body(scope_const_value(0x10));
    let fields = exact_entity_body_fields(&body, 9, 1);
    let image = constant_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let mut unknown_fields = fields.clone();
    unknown_fields.push(Vec::new());
    let bad_leaf = constant_schema_body(const_of(TypeExpr::Bool, ConstData::Bool(true)));
    let mut bad_leaf_fields = exact_entity_body_fields(&bad_leaf, 9, 1);
    // Flip the encoded boolean byte (the last byte of the node) to 2.
    *bad_leaf_fields[0].last_mut().unwrap() = 2;
    let mut unordered_map = fields.clone();
    unordered_map[0] = {
        use sley_ssmc::MapEntryConst;
        let byte_type = TypeExpr::UInt(IntegerWidth::from_bits(8));
        let entry = |key: u128| MapEntryConst {
            key: const_of(byte_type.clone(), ConstData::UInt(key)),
            value: const_of(TypeExpr::Unit, ConstData::Unit),
        };
        // Native encoding refuses unordered maps, so assemble the list by
        // hand from two canonical entries in the wrong order.
        let encoded_entry = |entry: &MapEntryConst| {
            sley_scb1::encode_record(&[
                (1, sley_mutate::encode_const_value(&entry.key).unwrap()),
                (2, sley_mutate::encode_const_value(&entry.value).unwrap()),
            ])
            .unwrap()
        };
        let entries =
            sley_scb1::encode_list(&[encoded_entry(&entry(2)), encoded_entry(&entry(1))]).unwrap();
        sley_scb1::encode_record(&[
            (1, sley_scb1::encode_union(1, &[]).unwrap()),
            (2, sley_scb1::encode_union(12, &entries).unwrap()),
        ])
        .unwrap()
    };

    let cases = [
        (
            "wrong_entity_kind",
            parameter_schema_with_fields(10, &fields),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_value",
            parameter_schema_with_fields(9, &[]),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_field",
            parameter_schema_with_fields(9, &unknown_fields),
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "invalid_nested_bool",
            parameter_schema_with_fields(9, &bad_leaf_fields),
            b"SCB_BOOL_INVALID".as_slice(),
        ),
        (
            "unordered_map_keys",
            parameter_schema_with_fields(9, &unordered_map),
            b"SCB_MAP_ORDER".as_slice(),
        ),
    ];
    for (name, malformed, expected) in cases {
        assert_eq!(
            simple_schema_error(&package, &approved, &malformed, "Constant"),
            expected,
            "{name} precedence"
        );
        assert_native_body_parity(&malformed, expected, name);
    }
}

#[test]
fn capability_requirement_schema_decoder_accepts_scope_lists() {
    let image = capability_requirement_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "CAPABILITY_REQUIREMENT_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 36);
    assert_eq!(image.parameters.len(), 2_059);
    assert_eq!(image.blocks.len(), 586);
    assert_eq!(image.operations.len(), 1_228);
    assert_eq!(image.constants.len(), 458);
    assert_eq!(package.image_bytes.len(), 147_792);
    assert_eq!(
        approved.package_digest,
        [
            0xe4, 0xa5, 0xa7, 0xa2, 0x42, 0x18, 0x62, 0xed, 0xb6, 0x1c, 0xd1, 0xeb, 0xaa, 0xe7,
            0xf1, 0x64, 0x31, 0xb1, 0xad, 0x72, 0x23, 0xe5, 0xb5, 0xa0, 0x60, 0x98, 0x12, 0x8f,
            0xc7, 0x3b, 0x16, 0xb6,
        ]
    );

    assert_schema_projection(
        &package,
        &approved,
        &capability_requirement_schema_body(vec![scope_const_value(0x20), scope_const_value(0x31)]),
        12,
        3,
    );
    assert_schema_projection(
        &package,
        &approved,
        &capability_requirement_schema_body(Vec::new()),
        12,
        3,
    );
}

#[test]
fn capability_requirement_schema_decoder_rejects_every_field_boundary() {
    let body = capability_requirement_schema_body(vec![scope_const_value(0x20)]);
    let fields = exact_entity_body_fields(&body, 12, 3);
    let image = capability_requirement_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let mut unknown_fields = fields.clone();
    unknown_fields.push(Vec::new());
    let mut short_effect = fields.clone();
    short_effect[0] = vec![0xf3; 31];
    let mut bad_scope = fields.clone();
    bad_scope[1] = sley_scb1::encode_list(&[sley_scb1::encode_record(&[
        (1, sley_scb1::encode_union(1, &[]).unwrap()),
        (2, sley_scb1::encode_union(17, &[]).unwrap()),
    ])
    .unwrap()])
    .unwrap();
    let mut nonminimal_scope_list = fields.clone();
    nonminimal_scope_list[1] = vec![0x80, 0x00];
    let mut unordered_contracts = fields.clone();
    unordered_contracts[2] = sley_scb1::encode_list(&[vec![0xf5; 32], vec![0xf4; 32]]).unwrap();
    let mut duplicate_contracts = fields.clone();
    duplicate_contracts[2] = sley_scb1::encode_list(&[vec![0xf4; 32], vec![0xf4; 32]]).unwrap();

    let cases = [
        (
            "wrong_entity_kind",
            parameter_schema_with_fields(13, &fields),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_required",
            parameter_schema_with_fields(12, &fields[..2]),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_field",
            parameter_schema_with_fields(12, &unknown_fields),
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "short_effect",
            parameter_schema_with_fields(12, &short_effect),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "unknown_scope_data_tag",
            parameter_schema_with_fields(12, &bad_scope),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "nonminimal_scope_count",
            parameter_schema_with_fields(12, &nonminimal_scope_list),
            b"SCB_VARINT_NON_MINIMAL".as_slice(),
        ),
        (
            "unordered_contracts",
            parameter_schema_with_fields(12, &unordered_contracts),
            b"SCB_MAP_ORDER".as_slice(),
        ),
        (
            "duplicate_contracts",
            parameter_schema_with_fields(12, &duplicate_contracts),
            b"SCB_MAP_DUPLICATE".as_slice(),
        ),
    ];
    for (name, malformed, expected) in cases {
        assert_eq!(
            simple_schema_error(&package, &approved, &malformed, "CapabilityRequirement"),
            expected,
            "{name} precedence"
        );
        assert_native_body_parity(&malformed, expected, name);
    }
}

/// Wraps the bounded-varint decoder as a two-argument unit validator so a
/// closed enum can stand alone as a union arm.
fn build_bounded_enum_validate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    bounded_uvar_decoder: EntityId,
    minimum: u64,
    maximum: u64,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let forward_error = forward_error_block(assembler, ns, function, &result_type);
    let success = unit_success_block(assembler, ns, function, unit, &result_type);
    let entry = assembler.id(ns.b);
    let minimum_value = u64_const(assembler, ns, entry, minimum);
    let maximum_value = u64_const(assembler, ns, entry, maximum);
    let decoded = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![
            pav(body),
            op_result(minimum_value),
            op_result(maximum_value),
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
        entry,
        function,
        Vec::new(),
        vec![minimum_value, maximum_value, decoded],
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
    unit_validator_graph(assembler, function, body, unit, entry, block_start)
}

/// Shared validators for the program-structure schemas (Block, Operation):
/// value references, target edges, and the primitive decoders they use.
#[derive(Clone, Copy)]
struct ProgramStructureClosure {
    decode: EntityId,
    record: EntityId,
    union: EntityId,
    list: EntityId,
    fixed32: EntityId,
    entity_ids: EntityId,
    exact_uvar: EntityId,
    bounded_uvar: EntityId,
    record1: EntityId,
    record2: EntityId,
    record3: EntityId,
    value_ref: EntityId,
    value_refs: EntityId,
    target_edge: EntityId,
    empty_payload: EntityId,
}

impl ProgramStructureClosure {
    fn decoders(self, exact_record: EntityId) -> SimpleSchemaDecoders {
        SimpleSchemaDecoders {
            union: self.union,
            exact_record,
            fixed32: self.fixed32,
            exact_uvar: self.exact_uvar,
            bounded_uvar: self.bounded_uvar,
            type_expr: self.fixed32,
            entity_ids: self.entity_ids,
        }
    }
}

fn ns_of(number: u8) -> Ns {
    Ns {
        k: number,
        p: number,
        b: number,
        o: number,
    }
}

/// Builds the program-structure closure (identity namespace 64, block
/// namespaces `65..=79`) and returns its graphs.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn build_program_structure_closure(
    assembler: &mut Asm,
) -> (ProgramStructureClosure, Vec<FunctionGraph>) {
    let decode_function = assembler.id(64);
    let record_function = assembler.id(64);
    let union_function = assembler.id(64);
    let list_function = assembler.id(64);
    let fixed32_function = assembler.id(64);
    let entity_ids_function = assembler.id(64);
    let exact_uvar_function = assembler.id(64);
    let bounded_uvar_function = assembler.id(64);
    let record1_function = assembler.id(64);
    let record2_function = assembler.id(64);
    let record3_function = assembler.id(64);
    let operation_result_function = assembler.id(64);
    let value_ref_function = assembler.id(64);
    let value_refs_function = assembler.id(64);
    let target_edge_function = assembler.id(64);
    let empty_payload_function = assembler.id(64);
    let (decode_graph, _) = build_decode(assembler, ns_of(65), decode_function);
    let record_graph =
        build_generic_record_decode(assembler, ns_of(66), record_function, decode_function);
    let union_graph =
        build_generic_union_decode(assembler, ns_of(67), union_function, decode_function);
    let list_graph =
        build_generic_list_decode(assembler, ns_of(68), list_function, decode_function);
    let fixed32_graph = build_fixed32_decode(assembler, ns_of(69), fixed32_function);
    let entity_ids_graph = build_entity_id_collection_decode(
        assembler,
        ns_of(70),
        entity_ids_function,
        list_function,
        fixed32_function,
    );
    let exact_uvar_graph =
        build_exact_uvar_decode(assembler, ns_of(71), exact_uvar_function, decode_function);
    let bounded_uvar_graph = build_bounded_uvar_decode(
        assembler,
        ns_of(72),
        bounded_uvar_function,
        exact_uvar_function,
    );
    let record1_graph = build_exact_record_projection(
        assembler,
        ns_of(73),
        record1_function,
        decode_function,
        record_function,
        1,
    );
    let record2_graph = build_exact_record_projection(
        assembler,
        ns_of(74),
        record2_function,
        decode_function,
        record_function,
        2,
    );
    let record3_graph = build_exact_record_projection(
        assembler,
        ns_of(75),
        record3_function,
        decode_function,
        record_function,
        3,
    );
    let closure = ProgramStructureClosure {
        decode: decode_function,
        record: record_function,
        union: union_function,
        list: list_function,
        fixed32: fixed32_function,
        entity_ids: entity_ids_function,
        exact_uvar: exact_uvar_function,
        bounded_uvar: bounded_uvar_function,
        record1: record1_function,
        record2: record2_function,
        record3: record3_function,
        value_ref: value_ref_function,
        value_refs: value_refs_function,
        target_edge: target_edge_function,
        empty_payload: empty_payload_function,
    };
    let operation_result_graph = build_projected_record_validate(
        assembler,
        ns_of(76),
        operation_result_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::ExactUvar(32),
        ],
        closure.decoders(record2_function),
    );
    let value_ref_graph = build_closed_union_validate(
        assembler,
        ns_of(77),
        value_ref_function,
        union_function,
        &[
            (fixed32_function, bytes_validation_result_type()),
            (operation_result_function, unit_validation_result_type()),
        ],
    );
    let value_refs_graph = build_unit_list_validate(
        assembler,
        ns_of(78),
        value_refs_function,
        list_function,
        value_ref_function,
        unit_validation_result_type(),
    );
    let target_edge_graph = build_projected_record_validate(
        assembler,
        ns_of(79),
        target_edge_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(value_refs_function),
        ],
        closure.decoders(record2_function),
    );
    let empty_payload_graph =
        build_empty_payload_validate(assembler, ns_of(80), empty_payload_function);
    (
        closure,
        vec![
            empty_payload_graph,
            target_edge_graph,
            value_refs_graph,
            value_ref_graph,
            operation_result_graph,
            record3_graph,
            record2_graph,
            record1_graph,
            bounded_uvar_graph,
            exact_uvar_graph,
            entity_ids_graph,
            fixed32_graph,
            list_graph,
            union_graph,
            record_graph,
            decode_graph,
        ],
    )
}

/// One program-structure entity schema over the shared closure; the
/// schema's own validators use block namespaces from `81` upward.
fn program_structure_schema_image(
    expected_kind: u64,
    build: impl FnOnce(
        &mut Asm,
        ProgramStructureClosure,
    ) -> (Vec<SimpleFieldValidator>, Vec<FunctionGraph>),
) -> Image {
    let mut assembler = Asm::new();
    let (closure, mut functions) = build_program_structure_closure(&mut assembler);
    let (validators, extra) = build(&mut assembler, closure);
    let schema_record_function = assembler.id(64);
    let function = assembler.id(64);
    let schema_record_graph = build_exact_record_projection(
        &mut assembler,
        ns_of(98),
        schema_record_function,
        closure.decode,
        closure.record,
        validators.len(),
    );
    let graph = build_simple_entity_schema_decode(
        &mut assembler,
        ns_of(99),
        function,
        expected_kind,
        &validators,
        closure.decoders(schema_record_function),
    );
    let mut all = vec![graph.clone(), schema_record_graph];
    all.extend(extra);
    all.append(&mut functions);
    let mut image = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph,
        functions: all,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: codec_bridge_imports(),
        constants: assembler.constants,
    };
    retain_reached_functions(&mut image);
    image
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn block_schema_decode_image() -> Image {
    program_structure_schema_image(7, |assembler, closure| {
        let return_function = assembler.id(64);
        let branch_function = assembler.id(64);
        let cond_branch_function = assembler.id(64);
        let builtin_case_function = assembler.id(64);
        let case_key_function = assembler.id(64);
        let switch_argument_function = assembler.id(64);
        let switch_arguments_function = assembler.id(64);
        let switch_edge_function = assembler.id(64);
        let switch_case_function = assembler.id(64);
        let switch_cases_function = assembler.id(64);
        let variant_switch_function = assembler.id(64);
        let optional_value_ref_function = assembler.id(64);
        let trap_function = assembler.id(64);
        let terminator_function = assembler.id(64);
        let return_graph = build_projected_record_validate(
            assembler,
            ns_of(81),
            return_function,
            &[SimpleFieldValidator::Unit(closure.value_ref)],
            closure.decoders(closure.record1),
        );
        let branch_graph = build_projected_record_validate(
            assembler,
            ns_of(82),
            branch_function,
            &[SimpleFieldValidator::Unit(closure.target_edge)],
            closure.decoders(closure.record1),
        );
        let cond_branch_graph = build_projected_record_validate(
            assembler,
            ns_of(83),
            cond_branch_function,
            &[
                SimpleFieldValidator::Unit(closure.value_ref),
                SimpleFieldValidator::Unit(closure.target_edge),
                SimpleFieldValidator::Unit(closure.target_edge),
            ],
            closure.decoders(closure.record3),
        );
        let builtin_case_graph = build_bounded_enum_validate(
            assembler,
            ns_of(84),
            builtin_case_function,
            closure.bounded_uvar,
            1,
            4,
        );
        let case_key_graph = build_closed_union_validate(
            assembler,
            ns_of(85),
            case_key_function,
            closure.union,
            &[
                (closure.fixed32, bytes_validation_result_type()),
                (builtin_case_function, unit_validation_result_type()),
            ],
        );
        let switch_argument_graph = build_closed_union_validate(
            assembler,
            ns_of(86),
            switch_argument_function,
            closure.union,
            &[
                (closure.value_ref, unit_validation_result_type()),
                (closure.empty_payload, unit_validation_result_type()),
            ],
        );
        let switch_arguments_graph = build_unit_list_validate(
            assembler,
            ns_of(87),
            switch_arguments_function,
            closure.list,
            switch_argument_function,
            unit_validation_result_type(),
        );
        let switch_edge_graph = build_projected_record_validate(
            assembler,
            ns_of(88),
            switch_edge_function,
            &[
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::Unit(switch_arguments_function),
            ],
            closure.decoders(closure.record2),
        );
        let switch_case_graph = build_projected_record_validate(
            assembler,
            ns_of(89),
            switch_case_function,
            &[
                SimpleFieldValidator::Unit(case_key_function),
                SimpleFieldValidator::Unit(switch_edge_function),
            ],
            closure.decoders(closure.record2),
        );
        let switch_cases_graph = build_unit_list_validate(
            assembler,
            ns_of(90),
            switch_cases_function,
            closure.list,
            switch_case_function,
            unit_validation_result_type(),
        );
        let variant_switch_graph = build_projected_record_validate(
            assembler,
            ns_of(91),
            variant_switch_function,
            &[
                SimpleFieldValidator::Unit(closure.value_ref),
                SimpleFieldValidator::Unit(switch_cases_function),
            ],
            closure.decoders(closure.record2),
        );
        let optional_value_ref_graph = build_option_unit_validate(
            assembler,
            ns_of(92),
            optional_value_ref_function,
            closure.union,
            closure.value_ref,
            unit_validation_result_type(),
        );
        let trap_graph = build_projected_record_validate(
            assembler,
            ns_of(93),
            trap_function,
            &[
                SimpleFieldValidator::BoundedUvar {
                    minimum: 1,
                    maximum: 4,
                },
                SimpleFieldValidator::Unit(optional_value_ref_function),
            ],
            closure.decoders(closure.record2),
        );
        let terminator_graph = build_closed_union_validate(
            assembler,
            ns_of(94),
            terminator_function,
            closure.union,
            &[
                (return_function, unit_validation_result_type()),
                (branch_function, unit_validation_result_type()),
                (cond_branch_function, unit_validation_result_type()),
                (variant_switch_function, unit_validation_result_type()),
                (trap_function, unit_validation_result_type()),
            ],
        );
        (
            vec![
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::EntityIds { ordered: false },
                SimpleFieldValidator::EntityIds { ordered: false },
                SimpleFieldValidator::Unit(terminator_function),
                SimpleFieldValidator::BoundedUvar {
                    minimum: 1,
                    maximum: 2,
                },
            ],
            vec![
                terminator_graph,
                trap_graph,
                optional_value_ref_graph,
                variant_switch_graph,
                switch_cases_graph,
                switch_case_graph,
                switch_edge_graph,
                switch_arguments_graph,
                switch_argument_graph,
                case_key_graph,
                builtin_case_graph,
                cond_branch_graph,
                branch_graph,
                return_graph,
            ],
        )
    })
}

/// Wraps the exact-width varint decoder as a two-argument unit validator so
/// a bare `u32`/`u64` can stand alone as a union arm.
fn build_exact_width_validate(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    exact_uvar_decoder: EntityId,
    width: u32,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let forward_error = forward_error_block(assembler, ns, function, &result_type);
    let success = unit_success_block(assembler, ns, function, unit, &result_type);
    let entry = assembler.id(ns.b);
    let width_constant = assembler.ku32(ns.k, u128::from(width));
    let width_value = assembler.cref(ns.o, entry, width_constant, u32_type());
    let decoded = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(body), op_result(width_value), pav(unit)],
        vec![exact_uvar_result_type()],
        Immediate::Function(FunctionRefValue {
            function: exact_uvar_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![width_value, decoded],
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
    unit_validator_graph(assembler, function, body, unit, entry, block_start)
}

#[allow(clippy::too_many_lines)]
fn operation_schema_decode_image() -> Image {
    program_structure_schema_image(8, |assembler, closure| {
        let type_expr_leaf_function = assembler.id(64);
        let type_expr_children_function = assembler.id(64);
        let type_expr_function = assembler.id(64);
        let result_types_function = assembler.id(64);
        let index_function = assembler.id(64);
        let variant_immediate_function = assembler.id(64);
        let function_ref_function = assembler.id(64);
        let immediate_function = assembler.id(64);
        let type_expr_leaf_graph = build_type_expr_leaf_decode(
            assembler,
            ns_of(81),
            type_expr_leaf_function,
            closure.union,
            closure.fixed32,
            closure.exact_uvar,
            closure.bounded_uvar,
        );
        let type_expr_children_graph = build_type_expr_children_decode(
            assembler,
            ns_of(82),
            type_expr_children_function,
            closure.union,
            type_expr_leaf_function,
            closure.list,
            closure.record2,
            closure.record3,
            closure.fixed32,
            closure.entity_ids,
        );
        // Operation.result_types elements sit at depth 3; the FunctionRef
        // immediate's type arguments at depth 5 (record 1, immediate union
        // 2, FunctionRefValue record 3, list 4, element 5).
        let (type_expr_core, type_expr_graphs) = build_type_expr_recursive_decode_at(
            assembler,
            ns_of(83),
            type_expr_function,
            type_expr_children_function,
            3,
        );
        let type_arguments_type_expr_function = assembler.id(64);
        let type_arguments_type_expr_graph = build_type_expr_depth_entry(
            assembler,
            ns_of(83),
            type_arguments_type_expr_function,
            type_expr_core,
            5,
        );
        let result_types_graph = build_unit_list_validate(
            assembler,
            ns_of(84),
            result_types_function,
            closure.list,
            type_expr_function,
            bytes_validation_result_type(),
        );
        let index_graph = build_exact_width_validate(
            assembler,
            ns_of(85),
            index_function,
            closure.exact_uvar,
            32,
        );
        let variant_immediate_graph = build_projected_record_validate(
            assembler,
            ns_of(86),
            variant_immediate_function,
            &[SimpleFieldValidator::Fixed32, SimpleFieldValidator::Fixed32],
            closure.decoders(closure.record2),
        );
        let type_arguments_function = assembler.id(64);
        let type_arguments_graph = build_unit_list_validate(
            assembler,
            ns_of(96),
            type_arguments_function,
            closure.list,
            type_arguments_type_expr_function,
            bytes_validation_result_type(),
        );
        let function_ref_graph = build_projected_record_validate(
            assembler,
            ns_of(87),
            function_ref_function,
            &[
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::Unit(type_arguments_function),
            ],
            closure.decoders(closure.record2),
        );
        let immediate_graph = build_closed_union_validate(
            assembler,
            ns_of(88),
            immediate_function,
            closure.union,
            &[
                (closure.empty_payload, unit_validation_result_type()),
                (closure.fixed32, bytes_validation_result_type()),
                (index_function, unit_validation_result_type()),
                (closure.fixed32, bytes_validation_result_type()),
                (variant_immediate_function, unit_validation_result_type()),
                (closure.fixed32, bytes_validation_result_type()),
                (function_ref_function, unit_validation_result_type()),
            ],
        );
        (
            vec![
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::ExactUvar(32),
                SimpleFieldValidator::ExactUvar(32),
                SimpleFieldValidator::Unit(closure.value_refs),
                SimpleFieldValidator::Unit(result_types_function),
                SimpleFieldValidator::Unit(immediate_function),
            ],
            vec![
                immediate_graph,
                function_ref_graph,
                type_arguments_graph,
                type_arguments_type_expr_graph,
                variant_immediate_graph,
                index_graph,
                result_types_graph,
                type_expr_graphs[0].clone(),
                type_expr_graphs[1].clone(),
                type_expr_children_graph,
                type_expr_leaf_graph,
            ],
        )
    })
}

fn block_schema_body(terminator: sley_ssmc::Terminator) -> Vec<u8> {
    use sley_mutate::value::{BlockBody, EntityBodyValue};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0x71; 32]),
        body: EntityBodyValue::Block(BlockBody {
            function: EntityId::from_bytes([0x72; 32]),
            parameters: vec![
                EntityId::from_bytes([0x74; 32]),
                EntityId::from_bytes([0x73; 32]),
            ],
            operations: vec![EntityId::from_bytes([0x75; 32])],
            terminator,
            reachability: Reachability::ExplicitlyUnreachable,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema Block fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

fn operation_schema_body(immediate: Immediate) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, OperationBody};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0x81; 32]),
        body: EntityBodyValue::Operation(OperationBody {
            block: EntityId::from_bytes([0x82; 32]),
            ordinal: u32::MAX,
            opcode: Opcode::CallDirect.tag(),
            operands: vec![
                ValueRef::Parameter(EntityId::from_bytes([0x83; 32])),
                ValueRef::OperationResult(OperationResultRef {
                    operation: EntityId::from_bytes([0x84; 32]),
                    result_index: u32::MAX,
                }),
            ],
            result_types: vec![
                TypeExpr::Tuple(vec![TypeExpr::Bool, TypeExpr::Bytes]),
                TypeExpr::Option(Box::new(TypeExpr::Unit)),
            ],
            immediate,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema Operation fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

fn every_block_terminator() -> Vec<sley_ssmc::Terminator> {
    use sley_ssmc::{
        BranchTerminator, CaseKey, CondBranchTerminator, MemberId, ReturnTerminator, SwitchCase,
        SwitchEdge, TargetEdge, Terminator, TrapCode, TrapTerminator, VariantSwitchTerminator,
    };

    let parameter = ValueRef::Parameter(EntityId::from_bytes([0x73; 32]));
    let result = ValueRef::OperationResult(OperationResultRef {
        operation: EntityId::from_bytes([0x75; 32]),
        result_index: 1,
    });
    let edge = |fill: u8, arguments: Vec<ValueRef>| TargetEdge {
        target: EntityId::from_bytes([fill; 32]),
        arguments,
    };
    vec![
        Terminator::Return(ReturnTerminator { value: result }),
        Terminator::Branch(BranchTerminator {
            edge: edge(0x76, vec![parameter, result]),
        }),
        Terminator::CondBranch(CondBranchTerminator {
            condition: parameter,
            if_true: edge(0x76, Vec::new()),
            if_false: edge(0x77, vec![result]),
        }),
        Terminator::VariantSwitch(VariantSwitchTerminator {
            value: result,
            cases: vec![
                SwitchCase {
                    case_key: CaseKey::Builtin(BuiltinCase::Err),
                    edge: SwitchEdge {
                        target: EntityId::from_bytes([0x78; 32]),
                        arguments: vec![
                            SwitchArgument::CasePayload,
                            SwitchArgument::Value(parameter),
                        ],
                    },
                },
                SwitchCase {
                    case_key: CaseKey::Member(MemberId::from_bytes([0x79; 32])),
                    edge: SwitchEdge {
                        target: EntityId::from_bytes([0x7a; 32]),
                        arguments: Vec::new(),
                    },
                },
            ],
        }),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::AdapterContractViolation,
            payload: Some(parameter),
        }),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::Unreachable,
            payload: None,
        }),
    ]
}

fn every_operation_immediate() -> Vec<Immediate> {
    use sley_ssmc::{MemberId, VariantImmediate};

    vec![
        Immediate::None,
        Immediate::Entity(EntityId::from_bytes([0x85; 32])),
        Immediate::Index(u32::MAX),
        Immediate::Field(MemberId::from_bytes([0x86; 32])),
        Immediate::Variant(VariantImmediate {
            definition: EntityId::from_bytes([0x87; 32]),
            member_id: MemberId::from_bytes([0x88; 32]),
        }),
        Immediate::Observation([0x89; 32]),
        Immediate::Function(FunctionRefValue {
            function: EntityId::from_bytes([0x8a; 32]),
            type_arguments: vec![TypeExpr::Vector(Box::new(TypeExpr::Text))],
        }),
    ]
}

#[test]
fn block_schema_decoder_accepts_every_terminator() {
    let image = block_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "BLOCK_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 32);
    assert_eq!(image.parameters.len(), 1_227);
    assert_eq!(image.blocks.len(), 368);
    assert_eq!(image.operations.len(), 611);
    assert_eq!(image.constants.len(), 160);
    assert_eq!(package.image_bytes.len(), 81_622);
    assert_eq!(
        approved.package_digest,
        [
            0x1e, 0x26, 0xa1, 0x7a, 0x94, 0x4e, 0xd5, 0x46, 0xfb, 0x9b, 0xb5, 0x6b, 0x19, 0xa2,
            0x8d, 0xaa, 0x09, 0x21, 0x56, 0x7a, 0xf1, 0xa4, 0x63, 0x13, 0x64, 0x67, 0x4d, 0x84,
            0xfd, 0xe5, 0x93, 0x67,
        ]
    );
    for terminator in every_block_terminator() {
        assert_schema_projection(&package, &approved, &block_schema_body(terminator), 7, 5);
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn block_schema_decoder_rejects_every_terminator_boundary() {
    use sley_ssmc::{ReturnTerminator, Terminator};

    let body = block_schema_body(Terminator::Return(ReturnTerminator {
        value: ValueRef::Parameter(EntityId::from_bytes([0x73; 32])),
    }));
    let fields = exact_entity_body_fields(&body, 7, 5);
    let image = block_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let union = |tag: u32, payload: &[u8]| sley_scb1::encode_union(tag, payload).unwrap();
    let record = |fields: &[(u32, Vec<u8>)]| sley_scb1::encode_record(fields).unwrap();
    let list = |elements: &[Vec<u8>]| sley_scb1::encode_list(elements).unwrap();
    let parameter_ref = union(1, &[0x73; 32]);
    let edge = |arguments: Vec<u8>| record(&[(1, vec![0x76; 32]), (2, arguments)]);

    let mut unknown_fields = fields.clone();
    unknown_fields.push(Vec::new());
    let mut short_parameter = fields.clone();
    short_parameter[1] = list(&[vec![0x74; 31]]);
    let mut unknown_terminator = fields.clone();
    unknown_terminator[3] = union(6, &[]);
    let mut return_unknown_ref = fields.clone();
    return_unknown_ref[3] = union(1, &record(&[(1, union(3, &[]))]));
    let mut return_wide_result_index = fields.clone();
    return_wide_result_index[3] = union(
        1,
        &record(&[(
            1,
            union(
                2,
                &record(&[
                    (1, vec![0x75; 32]),
                    (2, sley_scb1::encode_uvar(u64::from(u32::MAX) + 1)),
                ]),
            ),
        )]),
    );
    let mut branch_missing_arguments = fields.clone();
    branch_missing_arguments[3] = union(2, &record(&[(1, record(&[(1, vec![0x76; 32])]))]));
    let mut cond_bad_argument = fields.clone();
    cond_bad_argument[3] = union(
        3,
        &record(&[
            (1, parameter_ref.clone()),
            (2, edge(list(&[]))),
            (3, edge(list(&[union(2, &record(&[(1, vec![0x75; 32])]))]))),
        ]),
    );
    let mut switch_bad_builtin_case = fields.clone();
    switch_bad_builtin_case[3] = union(
        4,
        &record(&[
            (1, parameter_ref.clone()),
            (
                2,
                list(&[record(&[
                    (1, union(2, &sley_scb1::encode_uvar(5))),
                    (2, edge(list(&[]))),
                ])]),
            ),
        ]),
    );
    let mut switch_bad_argument_tag = fields.clone();
    switch_bad_argument_tag[3] = union(
        4,
        &record(&[
            (1, parameter_ref.clone()),
            (
                2,
                list(&[record(&[
                    (1, union(1, &[0x79; 32])),
                    (2, edge(list(&[union(3, &[])]))),
                ])]),
            ),
        ]),
    );
    let mut switch_nonempty_case_payload = fields.clone();
    switch_nonempty_case_payload[3] = union(
        4,
        &record(&[
            (1, parameter_ref.clone()),
            (
                2,
                list(&[record(&[
                    (1, union(1, &[0x79; 32])),
                    (2, edge(list(&[union(2, &[0])]))),
                ])]),
            ),
        ]),
    );
    let mut trap_bad_code = fields.clone();
    trap_bad_code[3] = union(
        5,
        &record(&[(1, sley_scb1::encode_uvar(5)), (2, union(0, &[]))]),
    );
    let mut trap_bad_payload_option = fields.clone();
    trap_bad_payload_option[3] = union(
        5,
        &record(&[(1, sley_scb1::encode_uvar(1)), (2, union(2, &[]))]),
    );
    let mut invalid_reachability = fields.clone();
    invalid_reachability[4] = sley_scb1::encode_uvar(3);

    let cases = [
        (
            "wrong_entity_kind",
            parameter_schema_with_fields(8, &fields),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_required",
            parameter_schema_with_fields(7, &fields[..4]),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_field",
            parameter_schema_with_fields(7, &unknown_fields),
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "short_parameter",
            parameter_schema_with_fields(7, &short_parameter),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "unknown_terminator",
            parameter_schema_with_fields(7, &unknown_terminator),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "return_unknown_ref",
            parameter_schema_with_fields(7, &return_unknown_ref),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "return_wide_result_index",
            parameter_schema_with_fields(7, &return_wide_result_index),
            b"SCB_INTEGER_OVERFLOW".as_slice(),
        ),
        (
            "branch_missing_arguments",
            parameter_schema_with_fields(7, &branch_missing_arguments),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "cond_bad_argument",
            parameter_schema_with_fields(7, &cond_bad_argument),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "switch_bad_builtin_case",
            parameter_schema_with_fields(7, &switch_bad_builtin_case),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "switch_bad_argument_tag",
            parameter_schema_with_fields(7, &switch_bad_argument_tag),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "switch_nonempty_case_payload",
            parameter_schema_with_fields(7, &switch_nonempty_case_payload),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "trap_bad_code",
            parameter_schema_with_fields(7, &trap_bad_code),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "trap_bad_payload_option",
            parameter_schema_with_fields(7, &trap_bad_payload_option),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "invalid_reachability",
            parameter_schema_with_fields(7, &invalid_reachability),
            b"SCB_UNION_INVALID".as_slice(),
        ),
    ];
    for (name, malformed, expected) in cases {
        assert_eq!(
            simple_schema_error(&package, &approved, &malformed, "Block"),
            expected,
            "{name} precedence"
        );
        assert_native_body_parity(&malformed, expected, name);
    }
}

#[test]
fn operation_schema_decoder_accepts_every_immediate() {
    let image = operation_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "OPERATION_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 27);
    assert_eq!(image.parameters.len(), 1_545);
    assert_eq!(image.blocks.len(), 409);
    assert_eq!(image.operations.len(), 731);
    assert_eq!(image.constants.len(), 232);
    assert_eq!(package.image_bytes.len(), 97_682);
    assert_eq!(
        approved.package_digest,
        [
            0x07, 0x7f, 0xf3, 0xd4, 0x2d, 0xdb, 0xa3, 0x96, 0x8a, 0xdb, 0xf8, 0xab, 0x53, 0x0b,
            0xbf, 0xb5, 0x0d, 0x55, 0xb1, 0x3f, 0xd9, 0x85, 0xd9, 0x26, 0x69, 0x42, 0xbb, 0x46,
            0xb0, 0xf0, 0x98, 0x76,
        ]
    );
    for immediate in every_operation_immediate() {
        assert_schema_projection(&package, &approved, &operation_schema_body(immediate), 8, 6);
    }
}

#[test]
fn operation_schema_decoder_rejects_every_immediate_boundary() {
    let body = operation_schema_body(Immediate::None);
    let fields = exact_entity_body_fields(&body, 8, 6);
    let image = operation_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let union = |tag: u32, payload: &[u8]| sley_scb1::encode_union(tag, payload).unwrap();
    let record = |fields: &[(u32, Vec<u8>)]| sley_scb1::encode_record(fields).unwrap();
    let list = |elements: &[Vec<u8>]| sley_scb1::encode_list(elements).unwrap();

    let mut unknown_fields = fields.clone();
    unknown_fields.push(Vec::new());
    let mut wide_ordinal = fields.clone();
    wide_ordinal[1] = sley_scb1::encode_uvar(u64::from(u32::MAX) + 1);
    let mut nonminimal_opcode = fields.clone();
    nonminimal_opcode[2] = vec![0x80, 0x00];
    let mut bad_operand = fields.clone();
    bad_operand[3] = list(&[union(1, &[0x83; 31])]);
    let mut bad_result_type = fields.clone();
    bad_result_type[4] = list(&[union(21, &[])]);
    let mut unknown_immediate = fields.clone();
    unknown_immediate[5] = union(8, &[]);
    let mut nonempty_none_immediate = fields.clone();
    nonempty_none_immediate[5] = union(1, &[0]);
    let mut wide_index = fields.clone();
    wide_index[5] = union(3, &sley_scb1::encode_uvar(u64::from(u32::MAX) + 1));
    let mut short_variant_member = fields.clone();
    short_variant_member[5] = union(5, &record(&[(1, vec![0x87; 32]), (2, vec![0x88; 31])]));
    let mut function_bad_type_argument = fields.clone();
    function_bad_type_argument[5] = union(
        7,
        &record(&[(1, vec![0x8a; 32]), (2, list(&[union(21, &[])]))]),
    );

    let cases = [
        (
            "wrong_entity_kind",
            parameter_schema_with_fields(7, &fields),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_required",
            parameter_schema_with_fields(8, &fields[..5]),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_field",
            parameter_schema_with_fields(8, &unknown_fields),
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "wide_ordinal",
            parameter_schema_with_fields(8, &wide_ordinal),
            b"SCB_INTEGER_OVERFLOW".as_slice(),
        ),
        (
            "nonminimal_opcode",
            parameter_schema_with_fields(8, &nonminimal_opcode),
            b"SCB_VARINT_NON_MINIMAL".as_slice(),
        ),
        (
            "bad_operand",
            parameter_schema_with_fields(8, &bad_operand),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "bad_result_type",
            parameter_schema_with_fields(8, &bad_result_type),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "unknown_immediate",
            parameter_schema_with_fields(8, &unknown_immediate),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "nonempty_none_immediate",
            parameter_schema_with_fields(8, &nonempty_none_immediate),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "wide_index",
            parameter_schema_with_fields(8, &wide_index),
            b"SCB_INTEGER_OVERFLOW".as_slice(),
        ),
        (
            "short_variant_member",
            parameter_schema_with_fields(8, &short_variant_member),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "function_bad_type_argument",
            parameter_schema_with_fields(8, &function_bad_type_argument),
            b"SCB_UNION_INVALID".as_slice(),
        ),
    ];
    for (name, malformed, expected) in cases {
        assert_eq!(
            simple_schema_error(&package, &approved, &malformed, "Operation"),
            expected,
            "{name} precedence"
        );
        assert_native_body_parity(&malformed, expected, name);
    }
}

/// Keeps only the functions the entry reaches through direct calls, so a
/// shared closure can carry helpers that one schema uses and another does
/// not (the bootstrap gate admits exactly the reached closure).
fn retain_reached_functions(image: &mut Image) {
    let calls: std::collections::BTreeMap<EntityId, Vec<EntityId>> = image
        .functions
        .iter()
        .map(|graph| {
            let callees = graph
                .blocks
                .iter()
                .flat_map(|block_id| {
                    image
                        .blocks
                        .iter()
                        .find(|block| block.entity_id == *block_id)
                        .into_iter()
                        .flat_map(|block| block.operations.iter().copied())
                })
                .filter_map(|operation_id| {
                    image
                        .operations
                        .iter()
                        .find(|operation| operation.entity_id == operation_id)
                })
                .filter_map(|operation| match &operation.immediate {
                    Immediate::Function(reference) => Some(reference.function),
                    _ => None,
                })
                .collect();
            (graph.entity_id, callees)
        })
        .collect();
    let mut reached = std::collections::BTreeSet::new();
    let mut pending = vec![image.entry.entity_id];
    while let Some(function) = pending.pop() {
        if reached.insert(function) {
            pending.extend(calls.get(&function).into_iter().flatten().copied());
        }
    }
    image
        .functions
        .retain(|graph| reached.contains(&graph.entity_id));
    // Drop the inventories the unreached functions owned, so the recorded
    // surface counts describe exactly the admitted closure.
    image
        .blocks
        .retain(|block| reached.contains(&block.function));
    let live_blocks: std::collections::BTreeSet<EntityId> =
        image.blocks.iter().map(|block| block.entity_id).collect();
    image
        .operations
        .retain(|operation| live_blocks.contains(&operation.block));
    image.parameters.retain(|parameter| {
        reached.contains(&parameter.owner) || live_blocks.contains(&parameter.owner)
    });
    let referenced_constants: std::collections::BTreeSet<EntityId> = image
        .operations
        .iter()
        .filter_map(|operation| match (operation.opcode, &operation.immediate) {
            (Opcode::ConstantRef, Immediate::Entity(constant)) => Some(*constant),
            _ => None,
        })
        .collect();
    image
        .constants
        .retain(|constant| referenced_constants.contains(&constant.entity_id));
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn test_case_schema_decode_image() -> Image {
    const_bearing_schema_image(14, |assembler, closure| {
        let const_list_function = assembler.id(110);
        let result_const_function = assembler.id(110);
        let replay_binding_function = assembler.id(110);
        let replay_bindings_function = assembler.id(110);
        let adapter_config_function = assembler.id(110);
        let adapter_configs_function = assembler.id(110);
        let effect_environment_function = assembler.id(110);
        let failure_code_function = assembler.id(110);
        let expected_outcome_function = assembler.id(110);
        let observation_function = assembler.id(110);
        let observations_function = assembler.id(110);
        let record6_function = assembler.id(110);
        let resource_limits_function = assembler.id(110);
        let const_list_graph = build_unit_list_validate(
            assembler,
            ns_of(145),
            const_list_function,
            closure.list,
            closure.const_value,
            bytes_validation_result_type(),
        );
        let result_const_graph = build_closed_union_validate(
            assembler,
            ns_of(146),
            result_const_function,
            closure.union,
            &[
                (closure.const_value, bytes_validation_result_type()),
                (closure.const_value, bytes_validation_result_type()),
            ],
        );
        let replay_binding_graph = build_projected_record_validate(
            assembler,
            ns_of(147),
            replay_binding_function,
            &[
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::Unit(const_list_function),
                SimpleFieldValidator::Unit(result_const_function),
            ],
            closure.decoders(closure.record3),
        );
        let replay_bindings_graph = build_unit_list_validate(
            assembler,
            ns_of(148),
            replay_bindings_function,
            closure.list,
            replay_binding_function,
            unit_validation_result_type(),
        );
        let adapter_config_graph = build_projected_record_validate(
            assembler,
            ns_of(149),
            adapter_config_function,
            &[
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::Bytes(closure.const_value),
            ],
            closure.decoders(closure.record2),
        );
        let adapter_configs_graph = build_unit_list_validate(
            assembler,
            ns_of(150),
            adapter_configs_function,
            closure.list,
            adapter_config_function,
            unit_validation_result_type(),
        );
        let effect_environment_graph = build_closed_union_validate(
            assembler,
            ns_of(151),
            effect_environment_function,
            closure.union,
            &[
                (replay_bindings_function, unit_validation_result_type()),
                (adapter_configs_function, unit_validation_result_type()),
            ],
        );
        let failure_code_graph = build_exact_width_validate(
            assembler,
            ns_of(152),
            failure_code_function,
            closure.exact_uvar,
            32,
        );
        let expected_outcome_graph = build_closed_union_validate(
            assembler,
            ns_of(153),
            expected_outcome_function,
            closure.union,
            &[
                (closure.const_value, bytes_validation_result_type()),
                (failure_code_function, unit_validation_result_type()),
            ],
        );
        let observation_graph = build_projected_record_validate(
            assembler,
            ns_of(154),
            observation_function,
            &[
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::Bytes(closure.const_value),
            ],
            closure.decoders(closure.record2),
        );
        let observations_graph = build_unit_list_validate(
            assembler,
            ns_of(155),
            observations_function,
            closure.list,
            observation_function,
            unit_validation_result_type(),
        );
        let record6_graph = build_exact_record_projection(
            assembler,
            ns_of(156),
            record6_function,
            closure.decode,
            closure.record,
            6,
        );
        let resource_limits_graph = build_projected_record_validate(
            assembler,
            ns_of(157),
            resource_limits_function,
            &[SimpleFieldValidator::ExactUvar(64); 6],
            closure.decoders(record6_function),
        );
        (
            vec![
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::Unit(const_list_function),
                SimpleFieldValidator::Unit(effect_environment_function),
                SimpleFieldValidator::Unit(expected_outcome_function),
                SimpleFieldValidator::Unit(observations_function),
                SimpleFieldValidator::Unit(resource_limits_function),
            ],
            vec![
                resource_limits_graph,
                record6_graph,
                observations_graph,
                observation_graph,
                expected_outcome_graph,
                failure_code_graph,
                effect_environment_graph,
                adapter_configs_graph,
                adapter_config_graph,
                replay_bindings_graph,
                replay_binding_graph,
                result_const_graph,
                const_list_graph,
            ],
        )
    })
}

fn test_case_schema_body(
    effect_environment: sley_ssmc::EffectEnvironment,
    expected: sley_ssmc::ExpectedOutcome,
) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, TestCaseBody};
    use sley_ssmc::{ExpectedObservation, ResourceLimits};

    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes([0x91; 32]),
        body: EntityBodyValue::TestCase(TestCaseBody {
            target: EntityId::from_bytes([0x92; 32]),
            inputs: vec![
                const_of(TypeExpr::Bool, ConstData::Bool(false)),
                scope_const_value(0x40),
            ],
            effect_environment,
            expected,
            observations: vec![ExpectedObservation {
                observation_id: [0x93; 32],
                value: const_of(TypeExpr::Text, ConstData::Text("seen".to_owned())),
            }],
            resource_limits: ResourceLimits {
                fuel: u64::MAX,
                memory_bytes: 1,
                output_bytes: 2,
                effect_count: 3,
                call_depth: 4,
                wall_timeout_millis: 5,
            },
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds schema TestCase fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

fn replay_environment() -> sley_ssmc::EffectEnvironment {
    use sley_ssmc::{EffectEnvironment, ReplayBinding};

    EffectEnvironment::Replay(vec![
        ReplayBinding {
            adapter_import: EntityId::from_bytes([0x94; 32]),
            request: vec![const_of(TypeExpr::Unit, ConstData::Unit)],
            response: ResultConst::Ok(Box::new(const_of(TypeExpr::Bool, ConstData::Bool(true)))),
        },
        ReplayBinding {
            adapter_import: EntityId::from_bytes([0x95; 32]),
            request: Vec::new(),
            response: ResultConst::Err(Box::new(const_of(
                TypeExpr::Bytes,
                ConstData::Bytes(vec![1, 2]),
            ))),
        },
    ])
}

fn adapter_environment() -> sley_ssmc::EffectEnvironment {
    use sley_ssmc::{AdapterConfig, EffectEnvironment};

    EffectEnvironment::DeterministicAdapters(vec![AdapterConfig {
        adapter_import: EntityId::from_bytes([0x96; 32]),
        configuration: scope_const_value(0x50),
    }])
}

#[test]
fn test_case_schema_decoder_accepts_both_environments_and_outcomes() {
    use sley_ssmc::ExpectedOutcome;

    let image = test_case_schema_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "TEST_CASE_SCHEMA functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 48);
    assert_eq!(image.parameters.len(), 2_280);
    assert_eq!(image.blocks.len(), 692);
    assert_eq!(image.operations.len(), 1_398);
    assert_eq!(image.constants.len(), 503);
    assert_eq!(package.image_bytes.len(), 169_632);
    assert_eq!(
        approved.package_digest,
        [
            0xfe, 0x7d, 0x8f, 0xe7, 0x2e, 0xa9, 0xad, 0x84, 0xfe, 0xd3, 0x3a, 0x32, 0xd0, 0x56,
            0x04, 0x14, 0x2b, 0x97, 0x67, 0x88, 0x2b, 0x9a, 0x1f, 0x90, 0x5a, 0x93, 0x4e, 0xc3,
            0x02, 0x57, 0x6a, 0x1a,
        ]
    );

    assert_schema_projection(
        &package,
        &approved,
        &test_case_schema_body(
            replay_environment(),
            ExpectedOutcome::Value(scope_const_value(0x60)),
        ),
        14,
        6,
    );
    assert_schema_projection(
        &package,
        &approved,
        &test_case_schema_body(
            adapter_environment(),
            ExpectedOutcome::FailureCode(u32::MAX),
        ),
        14,
        6,
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn test_case_schema_decoder_rejects_every_field_boundary() {
    use sley_ssmc::ExpectedOutcome;

    let body = test_case_schema_body(replay_environment(), ExpectedOutcome::FailureCode(7));
    let fields = exact_entity_body_fields(&body, 14, 6);
    let image = test_case_schema_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());

    let union = |tag: u32, payload: &[u8]| sley_scb1::encode_union(tag, payload).unwrap();
    let record = |fields: &[(u32, Vec<u8>)]| sley_scb1::encode_record(fields).unwrap();
    let list = |elements: &[Vec<u8>]| sley_scb1::encode_list(elements).unwrap();
    let unit_const =
        sley_mutate::encode_const_value(&const_of(TypeExpr::Unit, ConstData::Unit)).unwrap();
    let bad_const = record(&[(1, union(1, &[])), (2, union(2, &[9]))]);

    let mut unknown_fields = fields.clone();
    unknown_fields.push(Vec::new());
    let mut short_target = fields.clone();
    short_target[0] = vec![0x92; 31];
    let mut bad_input = fields.clone();
    bad_input[1] = list(&[unit_const.clone(), bad_const.clone()]);
    let mut unknown_environment = fields.clone();
    unknown_environment[2] = union(3, &[]);
    let mut replay_bad_response_tag = fields.clone();
    replay_bad_response_tag[2] = union(
        1,
        &list(&[record(&[
            (1, vec![0x94; 32]),
            (2, list(&[])),
            (3, union(3, &unit_const)),
        ])]),
    );
    let mut replay_bad_request = fields.clone();
    replay_bad_request[2] = union(
        1,
        &list(&[record(&[
            (1, vec![0x94; 32]),
            (2, list(std::slice::from_ref(&bad_const))),
            (3, union(1, &unit_const)),
        ])]),
    );
    let mut adapter_missing_configuration = fields.clone();
    adapter_missing_configuration[2] = union(2, &list(&[record(&[(1, vec![0x96; 32])])]));
    let mut unknown_outcome = fields.clone();
    unknown_outcome[3] = union(3, &[]);
    let mut wide_failure_code = fields.clone();
    wide_failure_code[3] = union(2, &sley_scb1::encode_uvar(u64::from(u32::MAX) + 1));
    let mut bad_outcome_value = fields.clone();
    bad_outcome_value[3] = union(1, &bad_const);
    let mut short_observation_id = fields.clone();
    short_observation_id[4] = list(&[record(&[(1, vec![0x93; 31]), (2, unit_const.clone())])]);
    let mut short_limits = fields.clone();
    short_limits[5] = record(&[
        (1, sley_scb1::encode_uvar(1)),
        (2, sley_scb1::encode_uvar(2)),
        (3, sley_scb1::encode_uvar(3)),
        (4, sley_scb1::encode_uvar(4)),
        (5, sley_scb1::encode_uvar(5)),
    ]);

    let cases = [
        (
            "wrong_entity_kind",
            parameter_schema_with_fields(13, &fields),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "missing_required",
            parameter_schema_with_fields(14, &fields[..5]),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_field",
            parameter_schema_with_fields(14, &unknown_fields),
            b"SCB_FIELD_UNKNOWN".as_slice(),
        ),
        (
            "short_target",
            parameter_schema_with_fields(14, &short_target),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "bad_input",
            parameter_schema_with_fields(14, &bad_input),
            b"SCB_BOOL_INVALID".as_slice(),
        ),
        (
            "unknown_environment",
            parameter_schema_with_fields(14, &unknown_environment),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "replay_bad_response_tag",
            parameter_schema_with_fields(14, &replay_bad_response_tag),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "replay_bad_request",
            parameter_schema_with_fields(14, &replay_bad_request),
            b"SCB_BOOL_INVALID".as_slice(),
        ),
        (
            "adapter_missing_configuration",
            parameter_schema_with_fields(14, &adapter_missing_configuration),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
        (
            "unknown_outcome",
            parameter_schema_with_fields(14, &unknown_outcome),
            b"SCB_UNION_INVALID".as_slice(),
        ),
        (
            "wide_failure_code",
            parameter_schema_with_fields(14, &wide_failure_code),
            b"SCB_INTEGER_OVERFLOW".as_slice(),
        ),
        (
            "bad_outcome_value",
            parameter_schema_with_fields(14, &bad_outcome_value),
            b"SCB_BOOL_INVALID".as_slice(),
        ),
        (
            "short_observation_id",
            parameter_schema_with_fields(14, &short_observation_id),
            b"SCB_LENGTH_OVERFLOW".as_slice(),
        ),
        (
            "short_limits",
            parameter_schema_with_fields(14, &short_limits),
            b"SCB_FIELD_MISSING".as_slice(),
        ),
    ];
    for (name, malformed, expected) in cases {
        assert_eq!(
            simple_schema_error(&package, &approved, &malformed, "TestCase"),
            expected,
            "{name} precedence"
        );
        assert_native_body_parity(&malformed, expected, name);
    }
}

// ---------------------------------------------------------------------------
// Arbitrary-schema composition: every entity kind's decoder built once, over
// one shared primitive closure, under one sequential namespace allocator, so
// the all-kind dispatcher can validate arbitrary canonical bodies.
// ---------------------------------------------------------------------------

/// Hands out block namespaces in order, skipping the ranges the host image
/// already uses for its own identities.
struct NsAlloc {
    next: u8,
    reserved: Vec<std::ops::RangeInclusive<u8>>,
}

impl NsAlloc {
    fn new(start: u8, reserved: Vec<std::ops::RangeInclusive<u8>>) -> Self {
        Self {
            next: start,
            reserved,
        }
    }

    fn next(&mut self) -> Ns {
        loop {
            let number = self.next;
            self.next = number
                .checked_add(1)
                .expect("composed image exhausted its 256 namespaces");
            if !self.reserved.iter().any(|range| range.contains(&number)) {
                return ns_of(number);
            }
        }
    }
}

/// Shared primitive decoders every schema recipe composes with.
#[derive(Clone, Copy)]
struct Primitives {
    ids: u8,
    decode: EntityId,
    record: EntityId,
    union: EntityId,
    list: EntityId,
    fixed32: EntityId,
    entity_ids: EntityId,
    exact_uvar: EntityId,
    bounded_uvar: EntityId,
    records: [EntityId; 6],
    /// Recursive `TypeExpr` core; reached only through the depth entries.
    type_expr_core: EntityId,
    /// `TypeExpr` as a direct kind-record field (native depth 2).
    type_expr: EntityId,
    /// `TypeExpr` as an element of a kind-record list field (depth 3).
    type_expr_listed: EntityId,
    /// `TypeExpr` inside a `TypeDef` record field or a `FunctionRef` immediate's
    /// type-argument list (depth 5).
    type_expr_form: EntityId,
    /// `TypeExpr` inside a `TypeDef` variant case's optional payload (depth 6).
    type_expr_variant: EntityId,
    type_parameter_list: EntityId,
    empty_payload: EntityId,
}

impl Primitives {
    /// A `(body, unit)` entry running the shared `TypeExpr` core from
    /// `depth`; sites whose depth is not one of the named fields build one.
    fn type_expr_at(
        self,
        assembler: &mut Asm,
        alloc: &mut NsAlloc,
        depth: u64,
        graphs: &mut Vec<FunctionGraph>,
    ) -> EntityId {
        let function = assembler.id(self.ids);
        graphs.push(build_type_expr_depth_entry(
            assembler,
            alloc.next(),
            function,
            self.type_expr_core,
            depth,
        ));
        function
    }

    fn id(self, assembler: &mut Asm) -> EntityId {
        assembler.id(self.ids)
    }

    fn record(self, field_count: usize) -> EntityId {
        self.records[field_count - 1]
    }

    fn decoders(self, exact_record: EntityId) -> SimpleSchemaDecoders {
        SimpleSchemaDecoders {
            union: self.union,
            exact_record,
            fixed32: self.fixed32,
            exact_uvar: self.exact_uvar,
            bounded_uvar: self.bounded_uvar,
            type_expr: self.type_expr,
            entity_ids: self.entity_ids,
        }
    }
}

#[allow(clippy::too_many_lines)]
fn build_primitives(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    ids: u8,
) -> (Primitives, Vec<FunctionGraph>) {
    let decode = assembler.id(ids);
    let record = assembler.id(ids);
    let union = assembler.id(ids);
    let list = assembler.id(ids);
    let fixed32 = assembler.id(ids);
    let entity_ids = assembler.id(ids);
    let exact_uvar = assembler.id(ids);
    let bounded_uvar = assembler.id(ids);
    let records = std::array::from_fn::<_, 6, _>(|_| assembler.id(ids));
    let type_expr_leaf = assembler.id(ids);
    let type_expr_children = assembler.id(ids);
    let type_expr = assembler.id(ids);
    let type_expr_listed = assembler.id(ids);
    let type_expr_form = assembler.id(ids);
    let type_expr_variant = assembler.id(ids);
    let type_parameter_list = assembler.id(ids);
    let empty_payload = assembler.id(ids);
    let mut graphs = vec![build_decode(assembler, alloc.next(), decode).0];
    graphs.push(build_generic_record_decode(
        assembler,
        alloc.next(),
        record,
        decode,
    ));
    graphs.push(build_generic_union_decode(
        assembler,
        alloc.next(),
        union,
        decode,
    ));
    graphs.push(build_generic_list_decode(
        assembler,
        alloc.next(),
        list,
        decode,
    ));
    graphs.push(build_fixed32_decode(assembler, alloc.next(), fixed32));
    graphs.push(build_entity_id_collection_decode(
        assembler,
        alloc.next(),
        entity_ids,
        list,
        fixed32,
    ));
    graphs.push(build_exact_uvar_decode(
        assembler,
        alloc.next(),
        exact_uvar,
        decode,
    ));
    graphs.push(build_bounded_uvar_decode(
        assembler,
        alloc.next(),
        bounded_uvar,
        exact_uvar,
    ));
    for (index, function) in records.iter().copied().enumerate() {
        graphs.push(build_exact_record_projection(
            assembler,
            alloc.next(),
            function,
            decode,
            record,
            index + 1,
        ));
    }
    graphs.push(build_type_expr_leaf_decode(
        assembler,
        alloc.next(),
        type_expr_leaf,
        union,
        fixed32,
        exact_uvar,
        bounded_uvar,
    ));
    graphs.push(build_type_expr_children_decode(
        assembler,
        alloc.next(),
        type_expr_children,
        union,
        type_expr_leaf,
        list,
        records[1],
        records[2],
        fixed32,
        entity_ids,
    ));
    let (type_expr_core, core_graphs) = build_type_expr_recursive_decode_at(
        assembler,
        alloc.next(),
        type_expr,
        type_expr_children,
        2,
    );
    graphs.extend(core_graphs);
    for (entry, depth) in [
        (type_expr_listed, 3),
        (type_expr_form, 5),
        (type_expr_variant, 6),
    ] {
        graphs.push(build_type_expr_depth_entry(
            assembler,
            alloc.next(),
            entry,
            type_expr_core,
            depth,
        ));
    }
    graphs.push(build_type_parameter_list_decode(
        assembler,
        alloc.next(),
        type_parameter_list,
        list,
        record,
        exact_uvar,
    ));
    graphs.push(build_empty_payload_validate(
        assembler,
        alloc.next(),
        empty_payload,
    ));
    (
        Primitives {
            ids,
            decode,
            record,
            union,
            list,
            fixed32,
            entity_ids,
            exact_uvar,
            bounded_uvar,
            records,
            type_expr_core,
            type_expr,
            type_expr_listed,
            type_expr_form,
            type_expr_variant,
            type_parameter_list,
            empty_payload,
        },
        graphs,
    )
}

/// One entity kind's arbitrary schema decoder: its entry and result type.
#[derive(Clone, Copy)]
struct SchemaEntry {
    kind: u64,
    function: EntityId,
    field_count: usize,
}

impl SchemaEntry {
    fn result_type(self) -> TypeExpr {
        simple_schema_result_type(self.field_count)
    }
}

/// A simple-record schema over the primitives (one exact record projection
/// plus the entity entry).
fn simple_schema_recipe(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    prims: &Primitives,
    kind: u64,
    validators: &[SimpleFieldValidator],
    graphs: &mut Vec<FunctionGraph>,
) -> SchemaEntry {
    let function = prims.id(assembler);
    graphs.push(build_simple_entity_schema_decode(
        assembler,
        alloc.next(),
        function,
        kind,
        validators,
        prims.decoders(prims.record(validators.len())),
    ));
    SchemaEntry {
        kind,
        function,
        field_count: validators.len(),
    }
}

/// The recursive `ConstValue` decoder over the primitives.
#[allow(clippy::similar_names, clippy::too_many_lines)]
/// `ConstValue` decoder entries by the native depth of the constant's root
/// node: a Constant body's `value` field (2), a list element or an
/// `ExpectedOutcome` payload (3), an `ExpectedObservation` value (4), an
/// `AdapterConfig` configuration (5), and a `ReplayBinding` request element
/// or response value (6). Nested nodes are charged from the root by the
/// child projector's native offsets.
#[derive(Clone, Copy)]
struct ConstEntries {
    depth2: EntityId,
    depth3: EntityId,
    depth4: EntityId,
    depth5: EntityId,
    depth6: EntityId,
}

#[allow(clippy::too_many_lines)]
fn const_value_recipe(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    prims: &Primitives,
    graphs: &mut Vec<FunctionGraph>,
) -> ConstEntries {
    // The shared child projector validates every node's `value_type` and a
    // FunctionRef's type arguments at the offsets of the primary root (a
    // Constant body's value at depth 2): value_type at 3, type arguments at
    // 6. Roots at other depths and nested nodes therefore see the TypeExpr
    // bound charged from depth 2; the deviation is recorded in
    // rw-090-codec-component-manifest.json `known_deviations`.
    let value_type_type_expr = prims.type_expr_at(assembler, alloc, 3, graphs);
    let type_arguments_type_expr = prims.type_expr_at(assembler, alloc, 6, graphs);
    let bool_function = prims.id(assembler);
    let uvar128_function = prims.id(assembler);
    let f32_function = prims.id(assembler);
    let f64_function = prims.id(assembler);
    let bytes_function = prims.id(assembler);
    let text_function = prims.id(assembler);
    let record_fields_function = prims.id(assembler);
    let record_const_function = prims.id(assembler);
    let option_child_function = prims.id(assembler);
    let variant_const_function = prims.id(assembler);
    let map_entries_function = prims.id(assembler);
    let result_child_function = prims.id(assembler);
    let type_arguments_function = prims.id(assembler);
    let function_ref_function = prims.id(assembler);
    let builtin_failure_function = prims.id(assembler);
    let const_children_function = prims.id(assembler);
    let const_value_function = prims.id(assembler);
    graphs.push(build_bool_validate(assembler, alloc.next(), bool_function));
    graphs.push(build_uvar128_validate(
        assembler,
        alloc.next(),
        uvar128_function,
    ));
    graphs.push(build_float_bits_validate(
        assembler,
        alloc.next(),
        f32_function,
        4,
    ));
    graphs.push(build_float_bits_validate(
        assembler,
        alloc.next(),
        f64_function,
        8,
    ));
    graphs.push(build_sized_payload_validate(
        assembler,
        alloc.next(),
        bytes_function,
        prims.decode,
        false,
    ));
    graphs.push(build_sized_payload_validate(
        assembler,
        alloc.next(),
        text_function,
        prims.decode,
        true,
    ));
    graphs.push(build_entry_list_children(
        assembler,
        alloc.next(),
        record_fields_function,
        prims.list,
        prims.record(2),
        prims.fixed32,
        EntryListMode::RecordFields,
    ));
    graphs.push(build_prefixed_record_child(
        assembler,
        alloc.next(),
        record_const_function,
        prims.record(2),
        prims.fixed32,
        1,
        LastFieldChildren::Map(record_fields_function),
    ));
    graphs.push(build_tagged_child_projection(
        assembler,
        alloc.next(),
        option_child_function,
        prims.union,
        Some(0),
        1,
    ));
    graphs.push(build_prefixed_record_child(
        assembler,
        alloc.next(),
        variant_const_function,
        prims.record(3),
        prims.fixed32,
        2,
        LastFieldChildren::Optional(option_child_function),
    ));
    graphs.push(build_entry_list_children(
        assembler,
        alloc.next(),
        map_entries_function,
        prims.list,
        prims.record(2),
        prims.fixed32,
        EntryListMode::MapEntries,
    ));
    graphs.push(build_tagged_child_projection(
        assembler,
        alloc.next(),
        result_child_function,
        prims.union,
        None,
        2,
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        type_arguments_function,
        prims.list,
        type_arguments_type_expr,
        bytes_validation_result_type(),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        function_ref_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(type_arguments_function),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        builtin_failure_function,
        &[
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 5,
            },
            SimpleFieldValidator::ExactUvar(16),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_const_value_children_decode(
        assembler,
        alloc.next(),
        const_children_function,
        prims.record(2),
        value_type_type_expr,
        prims.union,
        &[
            ConstDataArm::Unit(prims.empty_payload),
            ConstDataArm::Unit(bool_function),
            ConstDataArm::Unit(uvar128_function),
            ConstDataArm::Unit(uvar128_function),
            ConstDataArm::Unit(f32_function),
            ConstDataArm::Unit(f64_function),
            ConstDataArm::Unit(bytes_function),
            ConstDataArm::Unit(text_function),
            ConstDataArm::List(prims.list),
            ConstDataArm::List(record_const_function),
            ConstDataArm::Optional(variant_const_function),
            ConstDataArm::List(map_entries_function),
            ConstDataArm::Optional(option_child_function),
            ConstDataArm::Optional(result_child_function),
            ConstDataArm::Unit(function_ref_function),
            ConstDataArm::Unit(builtin_failure_function),
        ],
    ));
    let (const_core, core_graphs) = build_type_expr_recursive_decode_at(
        assembler,
        alloc.next(),
        const_value_function,
        const_children_function,
        2,
    );
    graphs.extend(core_graphs);
    let mut entry = |depth: u64, graphs: &mut Vec<FunctionGraph>| {
        let function = prims.id(assembler);
        graphs.push(build_type_expr_depth_entry(
            assembler,
            alloc.next(),
            function,
            const_core,
            depth,
        ));
        function
    };
    ConstEntries {
        depth2: const_value_function,
        depth3: entry(3, graphs),
        depth4: entry(4, graphs),
        depth5: entry(5, graphs),
        depth6: entry(6, graphs),
    }
}

#[allow(clippy::similar_names)]
fn type_def_recipe(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    prims: &Primitives,
    graphs: &mut Vec<FunctionGraph>,
) -> SchemaEntry {
    let record_field_function = prims.id(assembler);
    let record_fields_function = prims.id(assembler);
    let optional_payload_function = prims.id(assembler);
    let variant_case_function = prims.id(assembler);
    let variant_cases_function = prims.id(assembler);
    let form_function = prims.id(assembler);
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        record_field_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::TypeExpr,
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 4,
            },
        ],
        SimpleSchemaDecoders {
            type_expr: prims.type_expr_form,
            ..prims.decoders(prims.record(3))
        },
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        record_fields_function,
        prims.list,
        record_field_function,
        unit_validation_result_type(),
    ));
    graphs.push(build_option_unit_validate(
        assembler,
        alloc.next(),
        optional_payload_function,
        prims.union,
        prims.type_expr_variant,
        bytes_validation_result_type(),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        variant_case_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(optional_payload_function),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        variant_cases_function,
        prims.list,
        variant_case_function,
        unit_validation_result_type(),
    ));
    graphs.push(build_closed_union_validate(
        assembler,
        alloc.next(),
        form_function,
        prims.union,
        &[
            (record_fields_function, unit_validation_result_type()),
            (variant_cases_function, unit_validation_result_type()),
        ],
    ));
    simple_schema_recipe(
        assembler,
        alloc,
        prims,
        4,
        &[
            SimpleFieldValidator::Unit(prims.type_parameter_list),
            SimpleFieldValidator::Unit(form_function),
            SimpleFieldValidator::EntityIds { ordered: true },
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 4,
            },
        ],
        graphs,
    )
}

fn function_recipe(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    prims: &Primitives,
    graphs: &mut Vec<FunctionGraph>,
) -> SchemaEntry {
    let function = prims.id(assembler);
    graphs.push(build_function_schema_decode(
        assembler,
        alloc.next(),
        function,
        prims.union,
        prims.record,
        prims.entity_ids,
        prims.type_parameter_list,
        prims.fixed32,
        prims.bounded_uvar,
        prims.type_expr,
    ));
    SchemaEntry {
        kind: 5,
        function,
        field_count: 8,
    }
}

fn parameter_recipe(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    prims: &Primitives,
    graphs: &mut Vec<FunctionGraph>,
) -> SchemaEntry {
    let function = prims.id(assembler);
    graphs.push(build_parameter_schema_decode(
        assembler,
        alloc.next(),
        function,
        prims.union,
        prims.record(4),
        prims.fixed32,
        prims.bounded_uvar,
        prims.exact_uvar,
        prims.type_expr,
    ));
    SchemaEntry {
        kind: 6,
        function,
        field_count: 4,
    }
}

/// Value references and target edges shared by Block and Operation.
#[derive(Clone, Copy)]
struct ProgramStructure {
    value_ref: EntityId,
    value_refs: EntityId,
    target_edge: EntityId,
}

fn program_structure_recipe(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    prims: &Primitives,
    graphs: &mut Vec<FunctionGraph>,
) -> ProgramStructure {
    let operation_result_function = prims.id(assembler);
    let value_ref = prims.id(assembler);
    let value_refs = prims.id(assembler);
    let target_edge = prims.id(assembler);
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        operation_result_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::ExactUvar(32),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_closed_union_validate(
        assembler,
        alloc.next(),
        value_ref,
        prims.union,
        &[
            (prims.fixed32, bytes_validation_result_type()),
            (operation_result_function, unit_validation_result_type()),
        ],
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        value_refs,
        prims.list,
        value_ref,
        unit_validation_result_type(),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        target_edge,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(value_refs),
        ],
        prims.decoders(prims.record(2)),
    ));
    ProgramStructure {
        value_ref,
        value_refs,
        target_edge,
    }
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn block_recipe(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    prims: &Primitives,
    structure: ProgramStructure,
    graphs: &mut Vec<FunctionGraph>,
) -> SchemaEntry {
    let return_function = prims.id(assembler);
    let branch_function = prims.id(assembler);
    let cond_branch_function = prims.id(assembler);
    let builtin_case_function = prims.id(assembler);
    let case_key_function = prims.id(assembler);
    let switch_argument_function = prims.id(assembler);
    let switch_arguments_function = prims.id(assembler);
    let switch_edge_function = prims.id(assembler);
    let switch_case_function = prims.id(assembler);
    let switch_cases_function = prims.id(assembler);
    let variant_switch_function = prims.id(assembler);
    let optional_value_ref_function = prims.id(assembler);
    let trap_function = prims.id(assembler);
    let terminator_function = prims.id(assembler);
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        return_function,
        &[SimpleFieldValidator::Unit(structure.value_ref)],
        prims.decoders(prims.record(1)),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        branch_function,
        &[SimpleFieldValidator::Unit(structure.target_edge)],
        prims.decoders(prims.record(1)),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        cond_branch_function,
        &[
            SimpleFieldValidator::Unit(structure.value_ref),
            SimpleFieldValidator::Unit(structure.target_edge),
            SimpleFieldValidator::Unit(structure.target_edge),
        ],
        prims.decoders(prims.record(3)),
    ));
    graphs.push(build_bounded_enum_validate(
        assembler,
        alloc.next(),
        builtin_case_function,
        prims.bounded_uvar,
        1,
        4,
    ));
    graphs.push(build_closed_union_validate(
        assembler,
        alloc.next(),
        case_key_function,
        prims.union,
        &[
            (prims.fixed32, bytes_validation_result_type()),
            (builtin_case_function, unit_validation_result_type()),
        ],
    ));
    graphs.push(build_closed_union_validate(
        assembler,
        alloc.next(),
        switch_argument_function,
        prims.union,
        &[
            (structure.value_ref, unit_validation_result_type()),
            (prims.empty_payload, unit_validation_result_type()),
        ],
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        switch_arguments_function,
        prims.list,
        switch_argument_function,
        unit_validation_result_type(),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        switch_edge_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(switch_arguments_function),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        switch_case_function,
        &[
            SimpleFieldValidator::Unit(case_key_function),
            SimpleFieldValidator::Unit(switch_edge_function),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        switch_cases_function,
        prims.list,
        switch_case_function,
        unit_validation_result_type(),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        variant_switch_function,
        &[
            SimpleFieldValidator::Unit(structure.value_ref),
            SimpleFieldValidator::Unit(switch_cases_function),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_option_unit_validate(
        assembler,
        alloc.next(),
        optional_value_ref_function,
        prims.union,
        structure.value_ref,
        unit_validation_result_type(),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        trap_function,
        &[
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 4,
            },
            SimpleFieldValidator::Unit(optional_value_ref_function),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_closed_union_validate(
        assembler,
        alloc.next(),
        terminator_function,
        prims.union,
        &[
            (return_function, unit_validation_result_type()),
            (branch_function, unit_validation_result_type()),
            (cond_branch_function, unit_validation_result_type()),
            (variant_switch_function, unit_validation_result_type()),
            (trap_function, unit_validation_result_type()),
        ],
    ));
    simple_schema_recipe(
        assembler,
        alloc,
        prims,
        7,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::EntityIds { ordered: false },
            SimpleFieldValidator::EntityIds { ordered: false },
            SimpleFieldValidator::Unit(terminator_function),
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 2,
            },
        ],
        graphs,
    )
}

#[allow(clippy::too_many_lines)]
fn operation_recipe(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    prims: &Primitives,
    structure: ProgramStructure,
    graphs: &mut Vec<FunctionGraph>,
) -> SchemaEntry {
    let result_types_function = prims.id(assembler);
    let index_function = prims.id(assembler);
    let variant_immediate_function = prims.id(assembler);
    let function_ref_function = prims.id(assembler);
    let immediate_function = prims.id(assembler);
    let type_arguments_function = prims.id(assembler);
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        result_types_function,
        prims.list,
        prims.type_expr_listed,
        bytes_validation_result_type(),
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        type_arguments_function,
        prims.list,
        prims.type_expr_form,
        bytes_validation_result_type(),
    ));
    graphs.push(build_exact_width_validate(
        assembler,
        alloc.next(),
        index_function,
        prims.exact_uvar,
        32,
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        variant_immediate_function,
        &[SimpleFieldValidator::Fixed32, SimpleFieldValidator::Fixed32],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        function_ref_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(type_arguments_function),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_closed_union_validate(
        assembler,
        alloc.next(),
        immediate_function,
        prims.union,
        &[
            (prims.empty_payload, unit_validation_result_type()),
            (prims.fixed32, bytes_validation_result_type()),
            (index_function, unit_validation_result_type()),
            (prims.fixed32, bytes_validation_result_type()),
            (variant_immediate_function, unit_validation_result_type()),
            (prims.fixed32, bytes_validation_result_type()),
            (function_ref_function, unit_validation_result_type()),
        ],
    ));
    simple_schema_recipe(
        assembler,
        alloc,
        prims,
        8,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::ExactUvar(32),
            SimpleFieldValidator::ExactUvar(32),
            SimpleFieldValidator::Unit(structure.value_refs),
            SimpleFieldValidator::Unit(result_types_function),
            SimpleFieldValidator::Unit(immediate_function),
        ],
        graphs,
    )
}

fn capability_requirement_recipe(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    prims: &Primitives,
    const_value: ConstEntries,
    graphs: &mut Vec<FunctionGraph>,
) -> SchemaEntry {
    let scopes_function = prims.id(assembler);
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        scopes_function,
        prims.list,
        const_value.depth3,
        bytes_validation_result_type(),
    ));
    simple_schema_recipe(
        assembler,
        alloc,
        prims,
        12,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(scopes_function),
            SimpleFieldValidator::EntityIds { ordered: true },
        ],
        graphs,
    )
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn contract_recipe(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    prims: &Primitives,
    graphs: &mut Vec<FunctionGraph>,
) -> SchemaEntry {
    let source_function = prims.id(assembler);
    let binding_function = prims.id(assembler);
    let bindings_function = prims.id(assembler);
    let resource_limits_function = prims.id(assembler);
    let projection_function = prims.id(assembler);
    let optional_limits_function = prims.id(assembler);
    let function = prims.id(assembler);
    graphs.push(build_contract_source_validate(
        assembler,
        alloc.next(),
        source_function,
        prims.union,
        prims.fixed32,
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        binding_function,
        &[
            SimpleFieldValidator::ExactUvar(32),
            SimpleFieldValidator::Unit(source_function),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        bindings_function,
        prims.list,
        binding_function,
        unit_validation_result_type(),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        resource_limits_function,
        &[SimpleFieldValidator::ExactUvar(64); 6],
        prims.decoders(prims.record(6)),
    ));
    graphs.push(build_optional_last_record_projection(
        assembler,
        alloc.next(),
        projection_function,
        prims.record(4),
        prims.record(5),
    ));
    graphs.push(build_optional_empty_unit_validate(
        assembler,
        alloc.next(),
        optional_limits_function,
        resource_limits_function,
    ));
    graphs.push(build_simple_entity_schema_decode(
        assembler,
        alloc.next(),
        function,
        13,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::BoundedUvar {
                minimum: 1,
                maximum: 7,
            },
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(bindings_function),
            SimpleFieldValidator::Unit(optional_limits_function),
        ],
        prims.decoders(projection_function),
    ));
    SchemaEntry {
        kind: 13,
        function,
        field_count: 5,
    }
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn test_case_recipe(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    prims: &Primitives,
    const_value: ConstEntries,
    graphs: &mut Vec<FunctionGraph>,
) -> SchemaEntry {
    // TestCase.inputs elements sit at depth 3; a ReplayBinding's request
    // elements and response value at depth 6; an AdapterConfig's
    // configuration at 5; an ExpectedOutcome value at 3; an
    // ExpectedObservation value at 4.
    let inputs_function = prims.id(assembler);
    let const_list_function = prims.id(assembler);
    let result_const_function = prims.id(assembler);
    let replay_binding_function = prims.id(assembler);
    let replay_bindings_function = prims.id(assembler);
    let adapter_config_function = prims.id(assembler);
    let adapter_configs_function = prims.id(assembler);
    let effect_environment_function = prims.id(assembler);
    let failure_code_function = prims.id(assembler);
    let expected_outcome_function = prims.id(assembler);
    let observation_function = prims.id(assembler);
    let observations_function = prims.id(assembler);
    let resource_limits_function = prims.id(assembler);
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        inputs_function,
        prims.list,
        const_value.depth3,
        bytes_validation_result_type(),
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        const_list_function,
        prims.list,
        const_value.depth6,
        bytes_validation_result_type(),
    ));
    graphs.push(build_closed_union_validate(
        assembler,
        alloc.next(),
        result_const_function,
        prims.union,
        &[
            (const_value.depth6, bytes_validation_result_type()),
            (const_value.depth6, bytes_validation_result_type()),
        ],
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        replay_binding_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(const_list_function),
            SimpleFieldValidator::Unit(result_const_function),
        ],
        prims.decoders(prims.record(3)),
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        replay_bindings_function,
        prims.list,
        replay_binding_function,
        unit_validation_result_type(),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        adapter_config_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Bytes(const_value.depth5),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        adapter_configs_function,
        prims.list,
        adapter_config_function,
        unit_validation_result_type(),
    ));
    graphs.push(build_closed_union_validate(
        assembler,
        alloc.next(),
        effect_environment_function,
        prims.union,
        &[
            (replay_bindings_function, unit_validation_result_type()),
            (adapter_configs_function, unit_validation_result_type()),
        ],
    ));
    graphs.push(build_exact_width_validate(
        assembler,
        alloc.next(),
        failure_code_function,
        prims.exact_uvar,
        32,
    ));
    graphs.push(build_closed_union_validate(
        assembler,
        alloc.next(),
        expected_outcome_function,
        prims.union,
        &[
            (const_value.depth3, bytes_validation_result_type()),
            (failure_code_function, unit_validation_result_type()),
        ],
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        observation_function,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Bytes(const_value.depth4),
        ],
        prims.decoders(prims.record(2)),
    ));
    graphs.push(build_unit_list_validate(
        assembler,
        alloc.next(),
        observations_function,
        prims.list,
        observation_function,
        unit_validation_result_type(),
    ));
    graphs.push(build_projected_record_validate(
        assembler,
        alloc.next(),
        resource_limits_function,
        &[SimpleFieldValidator::ExactUvar(64); 6],
        prims.decoders(prims.record(6)),
    ));
    simple_schema_recipe(
        assembler,
        alloc,
        prims,
        14,
        &[
            SimpleFieldValidator::Fixed32,
            SimpleFieldValidator::Unit(inputs_function),
            SimpleFieldValidator::Unit(effect_environment_function),
            SimpleFieldValidator::Unit(expected_outcome_function),
            SimpleFieldValidator::Unit(observations_function),
            SimpleFieldValidator::Unit(resource_limits_function),
        ],
        graphs,
    )
}

/// Every entity kind's arbitrary schema decoder, built over one shared
/// primitive closure. Kinds are returned in ascending order; kind 18 is
/// omitted because the dispatcher routes it to the retained
/// `DependencyBinding` decoder.
#[allow(clippy::too_many_lines)]
fn all_schema_recipes(
    assembler: &mut Asm,
    alloc: &mut NsAlloc,
    ids: u8,
) -> (Vec<SchemaEntry>, Vec<FunctionGraph>) {
    let (prims, mut graphs) = build_primitives(assembler, alloc, ids);
    let const_value = const_value_recipe(assembler, alloc, &prims, &mut graphs);
    let structure = program_structure_recipe(assembler, alloc, &prims, &mut graphs);
    let optional_parent_function = prims.id(assembler);
    graphs.push(build_option_unit_validate(
        assembler,
        alloc.next(),
        optional_parent_function,
        prims.union,
        prims.fixed32,
        bytes_validation_result_type(),
    ));
    let visibility = SimpleFieldValidator::BoundedUvar {
        minimum: 1,
        maximum: 4,
    };
    let mut entries = vec![
        simple_schema_recipe(
            assembler,
            alloc,
            &prims,
            1,
            &[
                SimpleFieldValidator::EntityIds { ordered: true },
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::EntityIds { ordered: true },
                SimpleFieldValidator::EntityIds { ordered: true },
                SimpleFieldValidator::EntityIds { ordered: true },
            ],
            &mut graphs,
        ),
        simple_schema_recipe(
            assembler,
            alloc,
            &prims,
            2,
            &[
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::EntityIds { ordered: true },
                SimpleFieldValidator::EntityIds { ordered: true },
            ],
            &mut graphs,
        ),
        simple_schema_recipe(
            assembler,
            alloc,
            &prims,
            3,
            &[
                SimpleFieldValidator::Unit(optional_parent_function),
                SimpleFieldValidator::EntityIds { ordered: true },
            ],
            &mut graphs,
        ),
        type_def_recipe(assembler, alloc, &prims, &mut graphs),
        function_recipe(assembler, alloc, &prims, &mut graphs),
        parameter_recipe(assembler, alloc, &prims, &mut graphs),
        block_recipe(assembler, alloc, &prims, structure, &mut graphs),
        operation_recipe(assembler, alloc, &prims, structure, &mut graphs),
        simple_schema_recipe(
            assembler,
            alloc,
            &prims,
            9,
            &[SimpleFieldValidator::Bytes(const_value.depth2)],
            &mut graphs,
        ),
        simple_schema_recipe(
            assembler,
            alloc,
            &prims,
            10,
            &[
                SimpleFieldValidator::TypeExpr,
                SimpleFieldValidator::Fixed32,
                visibility,
            ],
            &mut graphs,
        ),
        simple_schema_recipe(
            assembler,
            alloc,
            &prims,
            11,
            &[
                SimpleFieldValidator::BoundedUvar {
                    minimum: 1,
                    maximum: 8,
                },
                SimpleFieldValidator::TypeExpr,
                SimpleFieldValidator::TypeExpr,
                SimpleFieldValidator::TypeExpr,
                SimpleFieldValidator::TypeExpr,
                visibility,
            ],
            &mut graphs,
        ),
        capability_requirement_recipe(assembler, alloc, &prims, const_value, &mut graphs),
        contract_recipe(assembler, alloc, &prims, &mut graphs),
        test_case_recipe(assembler, alloc, &prims, const_value, &mut graphs),
        simple_schema_recipe(
            assembler,
            alloc,
            &prims,
            15,
            &[
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::ExactUvar(32),
                SimpleFieldValidator::TypeExpr,
                SimpleFieldValidator::TypeExpr,
                SimpleFieldValidator::TypeExpr,
                SimpleFieldValidator::EntityIds { ordered: true },
            ],
            &mut graphs,
        ),
        simple_schema_recipe(
            assembler,
            alloc,
            &prims,
            16,
            &[
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::BoundedUvar {
                    minimum: 1,
                    maximum: 2,
                },
            ],
            &mut graphs,
        ),
        simple_schema_recipe(
            assembler,
            alloc,
            &prims,
            17,
            &[
                SimpleFieldValidator::Fixed32,
                SimpleFieldValidator::EntityIds { ordered: true },
            ],
            &mut graphs,
        ),
    ];
    entries.sort_by_key(|entry| entry.kind);
    (entries, graphs)
}

/// `(kind, body, unit) -> Result<Unit, Bytes>`: routes a declared kind to its
/// arbitrary schema decoder and forwards that decoder's refusal unchanged;
/// a kind with no decoder is `SSMC_ENTITY_KIND_UNKNOWN`.
#[allow(clippy::too_many_lines)]
fn build_schema_body_check(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    entries: &[SchemaEntry],
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = unit_validation_result_type();
    let kind = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let unknown_code = assembler.kbytes(ns.k, b"SSMC_ENTITY_KIND_UNKNOWN");
    let forward_error = forward_error_block(assembler, ns, function, &result_type);
    let unknown_error = err_block(assembler, ns, function, result_type.clone(), unknown_code);
    let success = unit_success_block(assembler, ns, function, unit, &result_type);
    let calls = entries
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let checks = entries
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();

    for (block, entry) in calls.iter().copied().zip(entries.iter().copied()) {
        let decoded = assembler.op(
            ns.o,
            block,
            Opcode::CallDirect,
            vec![pav(body), pav(unit)],
            vec![entry.result_type()],
            Immediate::Function(FunctionRefValue {
                function: entry.function,
                type_arguments: Vec::new(),
            }),
        );
        append_block(
            assembler,
            block,
            function,
            Vec::new(),
            vec![decoded],
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

    for (index, (block, entry)) in checks
        .iter()
        .copied()
        .zip(entries.iter().copied())
        .enumerate()
    {
        let expected = u64_const(assembler, ns, block, entry.kind);
        let matches = bool_op(
            assembler,
            ns,
            block,
            Opcode::Equal,
            vec![pav(kind), op_result(expected)],
        );
        let fallback = checks.get(index + 1).copied().unwrap_or(unknown_error);
        append_block(
            assembler,
            block,
            function,
            Vec::new(),
            vec![expected, matches],
            cond(
                op_result(matches),
                edge(calls[index], Vec::new()),
                edge(fallback, Vec::new()),
            ),
        );
    }

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![kind, body, unit],
        result_type,
        effects: Vec::new(),
        entry_block: checks[0],
        blocks: assembler.blocks[block_start..]
            .iter()
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

/// Builds the schema body checker and every decoder it reaches into an
/// existing assembler, avoiding the host image's reserved namespaces.
pub(super) fn build_arbitrary_schema_body_check(
    assembler: &mut Asm,
    checker: EntityId,
    reserved: Vec<std::ops::RangeInclusive<u8>>,
    ids: u8,
) -> Vec<FunctionGraph> {
    let mut alloc = NsAlloc::new(1, reserved);
    let (entries, mut graphs) = all_schema_recipes(assembler, &mut alloc, ids);
    graphs.push(build_schema_body_check(
        assembler,
        alloc.next(),
        checker,
        &entries,
    ));
    graphs
}

fn entity_body_bytes(entity: [u8; 32], body: sley_mutate::value::EntityBodyValue) -> Vec<u8> {
    let record = sley_mutate::EntityObjectRecord {
        entity_id: EntityId::from_bytes(entity),
        body,
        label: None,
        semantic_fingerprint: None,
    };
    let stored = sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds arbitrary body fixture")
        .stored_bytes()
        .to_vec();
    ns_body_of(&stored)
}

/// Rich canonical bodies for the kinds whose earlier dispatch profiles were
/// pinned to empty collections or fixed choices.
fn rich_identity_bodies() -> Vec<(u64, Vec<u8>)> {
    use sley_mutate::value::{
        EntityBodyValue, EntityIdSet, EntryPointBody, NamespaceBody, PackageBody,
        PolicyBindingBody, WorkspaceBody,
    };

    let set = |fills: &[u8]| {
        EntityIdSet::from_unsorted(
            fills
                .iter()
                .map(|fill| EntityId::from_bytes([*fill; 32]))
                .collect(),
        )
        .expect("fixture identity sets are canonical")
    };
    vec![
        (
            1,
            entity_body_bytes(
                [0xa1; 32],
                EntityBodyValue::Workspace(WorkspaceBody {
                    packages: set(&[0x13, 0x11]),
                    root_namespace: EntityId::from_bytes([0x12; 32]),
                    capability_requirements: set(&[0x14]),
                    contracts: set(&[0x16, 0x15]),
                    tests: set(&[0x17]),
                }),
            ),
        ),
        (
            2,
            entity_body_bytes(
                [0xa2; 32],
                EntityBodyValue::Package(PackageBody {
                    workspace: EntityId::from_bytes([0x21; 32]),
                    root_namespace: EntityId::from_bytes([0x22; 32]),
                    dependencies: set(&[0x24, 0x23]),
                    exports: set(&[0x25]),
                }),
            ),
        ),
        (
            3,
            entity_body_bytes(
                [0xa3; 32],
                EntityBodyValue::Namespace(NamespaceBody {
                    parent: Some(EntityId::from_bytes([0x31; 32])),
                    members: set(&[0x33, 0x32]),
                }),
            ),
        ),
        (
            16,
            entity_body_bytes(
                [0xa6; 32],
                EntityBodyValue::EntryPoint(EntryPointBody {
                    function: EntityId::from_bytes([0x61; 32]),
                    exposure: sley_ssmc::EntryExposure::Protocol,
                }),
            ),
        ),
        (
            17,
            entity_body_bytes(
                [0xa7; 32],
                EntityBodyValue::PolicyBinding(PolicyBindingBody {
                    subject: EntityId::from_bytes([0x71; 32]),
                    requirements: set(&[0x73, 0x72]),
                }),
            ),
        ),
    ]
}

/// Every rich per-kind fixture built today, keyed by entity kind.
pub(super) fn rich_schema_bodies() -> Vec<(u64, Vec<u8>)> {
    use sley_ssmc::{ExpectedOutcome, ReturnTerminator, Terminator};

    let mut bodies = rich_identity_bodies();
    bodies.push((4, type_def_schema_body(type_def_record_form())));
    bodies.push((4, type_def_schema_body(type_def_variant_form())));
    bodies.push((5, function_schema_body()));
    bodies.push((
        6,
        parameter_schema_body(
            ParameterRole::Block,
            u32::MAX,
            TypeExpr::Option(Box::new(TypeExpr::Tuple(vec![
                TypeExpr::Bool,
                TypeExpr::Bytes,
            ]))),
        ),
    ));
    for terminator in every_block_terminator() {
        bodies.push((7, block_schema_body(terminator)));
    }
    bodies.push((
        7,
        block_schema_body(Terminator::Return(ReturnTerminator {
            value: ValueRef::Parameter(EntityId::from_bytes([0x73; 32])),
        })),
    ));
    for immediate in every_operation_immediate() {
        bodies.push((8, operation_schema_body(immediate)));
    }
    bodies.push((9, constant_schema_body(scope_const_value(0x10))));
    bodies.push((
        10,
        global_value_schema_body(
            TypeExpr::Vector(Box::new(TypeExpr::Text)),
            Visibility::Workspace,
        ),
    ));
    bodies.push((11, effect_def_schema_body()));
    bodies.push((
        12,
        capability_requirement_schema_body(vec![scope_const_value(0x20), scope_const_value(0x31)]),
    ));
    bodies.push((13, contract_schema_body(true)));
    bodies.push((13, contract_schema_body(false)));
    bodies.push((
        14,
        test_case_schema_body(
            replay_environment(),
            ExpectedOutcome::Value(scope_const_value(0x60)),
        ),
    ));
    bodies.push((
        14,
        test_case_schema_body(
            adapter_environment(),
            ExpectedOutcome::FailureCode(u32::MAX),
        ),
    ));
    bodies.push((15, adapter_import_schema_body()));
    bodies
}

fn arbitrary_dispatch_decode(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    kind: u64,
    stored: &[u8],
) -> (Result<Vec<ConstValue>, Vec<u8>>, sley_vm::ExecutionOutcome) {
    let outcome = execute_with_limits(
        package,
        approved,
        vec![u64_input(kind), bytes_input(stored), unit_input()],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "arbitrary dispatch must return a typed result for kind {kind}: {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(result) = &value.data else {
        panic!("arbitrary dispatch must return a Result: {value:?}")
    };
    let verdict = match result {
        ResultConst::Ok(accepted) => {
            let ConstData::Sequence(fields) = &accepted.data else {
                panic!("arbitrary dispatch must return the six-field tuple")
            };
            Ok(fields.clone())
        }
        ResultConst::Err(error) => {
            let ConstData::Bytes(code) = &error.data else {
                panic!("arbitrary dispatch refusal must be Bytes")
            };
            Err(code.clone())
        }
    };
    (verdict, outcome)
}

#[test]
#[allow(clippy::too_many_lines)]
fn arbitrary_dispatch_accepts_representative_and_rich_bodies_for_all_kinds() {
    let image = super::all_kind_digest_dispatch::arbitrary_all_kind_decode_image();
    assert_entry_cfg_surface(&image);
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "ARBITRARY_ALL_KIND functions={} parameters={} blocks={} operations={} constants={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 122);
    assert_eq!(image.parameters.len(), 6_803);
    assert_eq!(image.blocks.len(), 1_860);
    assert_eq!(image.operations.len(), 3_380);
    assert_eq!(image.constants.len(), 129);
    assert_eq!(package.image_bytes.len(), 447_594);
    assert_eq!(
        approved.package_digest,
        [
            0x6b, 0x98, 0xdc, 0xbb, 0xa4, 0xbb, 0x2a, 0xee, 0xf4, 0xab, 0x6f, 0x8e, 0x1b, 0x19,
            0x03, 0x90, 0xb7, 0xc0, 0xd6, 0x7d, 0x01, 0x2f, 0xb0, 0xba, 0xc0, 0x4e, 0x33, 0x77,
            0xaa, 0xa6, 0x1f, 0x38,
        ]
    );

    let mut bodies = super::all_kind_digest_dispatch::fixed_profile_bodies();
    bodies.extend(rich_schema_bodies());
    let mut kinds_seen = std::collections::BTreeSet::new();
    let mut peak = (0_u64, 0_u64, 0_u64);
    for (kind, body) in bodies {
        kinds_seen.insert(kind);
        let entity = [0x20_u8 + u8::try_from(kind).expect("entity kind fits u8"); 32];
        let stored = super::all_kind_digest_dispatch::stored_from_body(entity, &body);
        sley_mutate::import_entity_object(program_epoch9(), &stored)
            .expect("every fixture is accepted by the native codec");
        let (verdict, outcome) = arbitrary_dispatch_decode(&package, &approved, kind, &stored);
        let fields = verdict.unwrap_or_else(|code| {
            panic!(
                "kind {kind} body must be accepted, refused {}",
                String::from_utf8_lossy(&code)
            )
        });
        assert_eq!(fields.len(), 6);
        assert_eq!(fields[0].data, ConstData::UInt(u128::from(kind)));
        assert_eq!(fields[1].data, ConstData::Bytes(entity.to_vec()));
        if kind == 18 {
            assert_eq!(fields[2].data, ConstData::Bytes(vec![0xb1; 32]));
            assert_eq!(fields[3].data, ConstData::Bytes(vec![0xb2; 32]));
            assert_eq!(fields[4].data, ConstData::Bytes(vec![0xb3; 32]));
        } else {
            assert_eq!(fields[2].data, ConstData::Bytes(body.clone()));
            assert_eq!(fields[3].data, ConstData::Bytes(Vec::new()));
            assert_eq!(fields[4].data, ConstData::Bytes(Vec::new()));
        }
        assert_eq!(fields[5].data, ConstData::UInt(0));
        eprintln!(
            "ARBITRARY_ALL_KIND kind{kind} stored{}B fuel={} instructions={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units,
        );
        peak = (
            peak.0.max(outcome.fuel_used),
            peak.1.max(outcome.instruction_count),
            peak.2.max(outcome.peak_value_units),
        );
    }
    assert_eq!(kinds_seen.len(), 18, "every entity kind is exercised");
    eprintln!(
        "ARBITRARY_ALL_KIND peak fuel={} instructions={} value_units={}",
        peak.0, peak.1, peak.2
    );
    assert_eq!(peak, (660_293, 73_591, 35_704_544));
}

#[test]
#[allow(clippy::too_many_lines)]
fn arbitrary_dispatch_refuses_identity_kind_boundaries_with_native_codes() {
    // Kinds 1, 2, 3, 5, 16 and 17 carry only identities, sets and enums;
    // their refusal codes on the canonical entry are pinned here against the
    // native codec on the same stored bytes, and kind 18 is exercised on the
    // canonical entry (not the standalone decoder image) so the strict route
    // is the one that answers.
    let image = super::all_kind_digest_dispatch::arbitrary_all_kind_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    let entity = [0xcd; 32];
    let stored = |body: &[u8]| super::all_kind_digest_dispatch::stored_from_body(entity, body);
    let rich = rich_identity_bodies();
    let body_of = |kind: u64| {
        rich.iter()
            .find(|(candidate, _)| *candidate == kind)
            .map(|(_, body)| body.clone())
            .expect("rich identity fixture exists")
    };
    let field_counts = [(1_u32, 5_u32), (2, 4), (3, 2), (16, 2), (17, 2)];
    let mut cases: Vec<(String, u64, Vec<u8>, &[u8])> = Vec::new();
    for (kind, count) in field_counts {
        let fields = exact_entity_body_fields(&body_of(u64::from(kind)), kind, count);
        let mut unknown = fields.clone();
        unknown.push(Vec::new());
        let mut trailing = fields.clone();
        trailing[0].push(0);
        cases.push((
            format!("kind{kind}_missing_last_field"),
            u64::from(kind),
            parameter_schema_with_fields(kind, &fields[..fields.len() - 1]),
            b"SCB_FIELD_MISSING",
        ));
        cases.push((
            format!("kind{kind}_unknown_field"),
            u64::from(kind),
            parameter_schema_with_fields(kind, &unknown),
            b"SCB_FIELD_UNKNOWN",
        ));
        cases.push((
            format!("kind{kind}_trailing_first_field"),
            u64::from(kind),
            parameter_schema_with_fields(kind, &trailing),
            // A surplus byte after a set's last item, after a fixed identity,
            // or after an option's payload is a trailing byte natively.
            b"SCB_TRAILING_BYTES",
        ));
    }
    // Ordered identity sets: swapping two members breaks the canonical order.
    let workspace_fields = exact_entity_body_fields(&body_of(1), 1, 5);
    let mut unordered = workspace_fields.clone();
    unordered[0] = sley_scb1::encode_list(&[vec![0x13; 32], vec![0x11; 32]]).expect("list encodes");
    cases.push((
        "kind1_unordered_packages".to_owned(),
        1,
        parameter_schema_with_fields(1, &unordered),
        b"SCB_MAP_ORDER",
    ));
    let policy_fields = exact_entity_body_fields(&body_of(17), 17, 2);
    let mut duplicate = policy_fields.clone();
    duplicate[1] = sley_scb1::encode_list(&[vec![0x72; 32], vec![0x72; 32]]).expect("list encodes");
    cases.push((
        "kind17_duplicate_requirement".to_owned(),
        17,
        parameter_schema_with_fields(17, &duplicate),
        b"SCB_MAP_DUPLICATE",
    ));
    let entry_fields = exact_entity_body_fields(&body_of(16), 16, 2);
    let mut exposure = entry_fields.clone();
    exposure[1] = sley_scb1::encode_uvar(3);
    cases.push((
        "kind16_exposure_out_of_range".to_owned(),
        16,
        parameter_schema_with_fields(16, &exposure),
        b"SCB_UNION_INVALID",
    ));
    // Function: a short entry-block identity and a wide parameter list width.
    let function_fields = exact_entity_body_fields(&function_schema_body(), 5, 8);
    let mut short_entry = function_fields.clone();
    short_entry[4] = vec![0x55; 31];
    cases.push((
        "kind5_short_entry_block".to_owned(),
        5,
        parameter_schema_with_fields(5, &short_entry),
        b"SCB_LENGTH_OVERFLOW",
    ));
    let mut function_unknown = function_fields.clone();
    function_unknown.push(Vec::new());
    cases.push((
        "kind5_unknown_field".to_owned(),
        5,
        parameter_schema_with_fields(5, &function_unknown),
        b"SCB_FIELD_UNKNOWN",
    ));
    // Kind 18 on the canonical entry: the strict body decoder answers.
    let dependency_body = super::all_kind_digest_dispatch::fixed_profile_bodies()
        .into_iter()
        .find(|(kind, _)| *kind == 18)
        .map(|(_, body)| body)
        .expect("kind 18 fixture");
    let dependency_fields = exact_entity_body_fields(&dependency_body, 18, 3);
    let mut dependency_trailing = dependency_body.clone();
    dependency_trailing.push(0);
    cases.push((
        "kind18_trailing_body_byte".to_owned(),
        18,
        dependency_trailing,
        b"SCB_TRAILING_BYTES",
    ));
    let mut short_root = dependency_fields.clone();
    short_root[0] = vec![0xb1; 31];
    cases.push((
        "kind18_short_root".to_owned(),
        18,
        parameter_schema_with_fields(18, &short_root),
        b"SCB_LENGTH_OVERFLOW",
    ));
    cases.push((
        "kind18_missing_namespace".to_owned(),
        18,
        parameter_schema_with_fields(18, &dependency_fields[..2]),
        b"SCB_FIELD_MISSING",
    ));
    let mut dependency_unknown = dependency_fields.clone();
    dependency_unknown.push(Vec::new());
    cases.push((
        "kind18_unknown_field".to_owned(),
        18,
        parameter_schema_with_fields(18, &dependency_unknown),
        b"SCB_FIELD_UNKNOWN",
    ));

    for (name, kind, body, expected) in cases {
        let stored_bytes = stored(&body);
        let (verdict, _) = arbitrary_dispatch_decode(&package, &approved, kind, &stored_bytes);
        let native = match sley_mutate::import_entity_object(program_epoch9(), &stored_bytes) {
            Ok(_) => "OK".to_owned(),
            Err(error) => error.code().to_string(),
        };
        assert!(
            native != "OK",
            "{name}: the native codec must refuse the mutation"
        );
        // The native code is the oracle; the pinned code documents the
        // expected family and must agree with both.
        assert_eq!(
            verdict.as_ref().map_err(Vec::as_slice),
            Err(native.as_bytes()),
            "{name}: Sley refusal must equal the native code"
        );
        assert_eq!(native.as_bytes(), expected, "{name}: pinned expectation");
        eprintln!("ARBITRARY_REFUSAL_PARITY {name} kind{kind} code={native}");
    }
}

/// Native verdict on one stored object, as the parity oracle's code string.
fn native_stored_verdict(stored: &[u8]) -> String {
    match sley_mutate::import_entity_object(program_epoch9(), stored) {
        Ok(_) => "OK".to_owned(),
        Err(error) => error.code().to_string(),
    }
}

/// Sley verdict on one stored object through the canonical arbitrary entry.
fn arbitrary_stored_verdict(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    kind: u64,
    stored: &[u8],
) -> String {
    match arbitrary_dispatch_decode(package, approved, kind, stored).0 {
        Ok(_) => "OK".to_owned(),
        Err(code) => String::from_utf8(code).expect("refusal codes are ASCII"),
    }
}

fn option_type_chain(levels: usize) -> Vec<u8> {
    let mut node = sley_scb1::encode_union(1, &[]).expect("Unit TypeExpr encodes");
    for _ in 0..levels {
        node = sley_scb1::encode_union(13, &node).expect("Option TypeExpr encodes");
    }
    node
}

fn tuple_type_chain(levels: usize) -> Vec<u8> {
    let mut node = sley_scb1::encode_union(1, &[]).expect("Unit TypeExpr encodes");
    for _ in 0..levels {
        let list = sley_scb1::encode_list(&[node]).expect("Tuple element list encodes");
        node = sley_scb1::encode_union(9, &list).expect("Tuple TypeExpr encodes");
    }
    node
}

/// A `ConstValue` chain of `levels` Sequence nodes around a Unit leaf whose
/// innermost node carries `leaf_type` as its declared type.
fn sequence_const_chain(levels: usize, leaf_type: &[u8]) -> Vec<u8> {
    let unit_type = sley_scb1::encode_union(1, &[]).expect("Unit TypeExpr encodes");
    let mut node = sley_scb1::encode_record(&[
        (1, leaf_type.to_vec()),
        (
            2,
            sley_scb1::encode_union(1, &[]).expect("Unit ConstData encodes"),
        ),
    ])
    .expect("ConstValue record encodes");
    for _ in 0..levels {
        let list = sley_scb1::encode_list(&[node]).expect("Sequence list encodes");
        let data = sley_scb1::encode_union(9, &list).expect("Sequence ConstData encodes");
        node = sley_scb1::encode_record(&[(1, unit_type.clone()), (2, data)])
            .expect("ConstValue record encodes");
    }
    node
}

/// Entity-level nesting boundary parity (RW-090 Nabu P1): every site a
/// `TypeExpr` or `ConstValue` appears at inside an entity body is charged the
/// depth the native codec charges, so the Sley verdict equals the native
/// verdict on both sides of the 64-level bound. The exact native boundary of
/// each site is pinned as well, so a silent shift in either codec is visible.
#[test]
#[allow(clippy::too_many_lines)]
fn arbitrary_dispatch_matches_native_nesting_boundaries_at_every_site() {
    type Site = (&'static str, u64, Box<dyn Fn(usize) -> Vec<u8>>, usize);
    let image = super::all_kind_digest_dispatch::arbitrary_all_kind_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    let entity = [0xce; 32];
    let stored = |body: &[u8]| super::all_kind_digest_dispatch::stored_from_body(entity, body);
    let member = vec![0xd9; 32];

    let parameter_fields = exact_entity_body_fields(
        &parameter_schema_body(ParameterRole::Function, 7, TypeExpr::Bool),
        6,
        4,
    );
    let operation_fields = exact_entity_body_fields(&operation_schema_body(Immediate::None), 8, 6);
    let type_def_fields =
        exact_entity_body_fields(&type_def_schema_body(type_def_record_form()), 4, 4);
    let constant_fields = exact_entity_body_fields(
        &constant_schema_body(const_of(TypeExpr::Unit, ConstData::Unit)),
        9,
        1,
    );

    // (site, kind, body builder over the chain length, first refused chain
    // length; the chain length counts the wrapping nodes around the Unit leaf,
    // which is itself a union node one level deeper)
    let sites: Vec<Site> = vec![
        (
            "parameter.value_type option chain (field at depth 2)",
            6,
            Box::new({
                let fields = parameter_fields.clone();
                move |k| {
                    let mut fields = fields.clone();
                    fields[3] = option_type_chain(k);
                    parameter_schema_with_fields(6, &fields)
                }
            }),
            62,
        ),
        (
            "parameter.value_type tuple chain (two levels per node)",
            6,
            Box::new({
                let fields = parameter_fields.clone();
                move |k| {
                    let mut fields = fields.clone();
                    fields[3] = tuple_type_chain(k);
                    parameter_schema_with_fields(6, &fields)
                }
            }),
            31,
        ),
        (
            "operation.result_types element (list element at depth 3)",
            8,
            Box::new({
                let fields = operation_fields.clone();
                move |k| {
                    let mut fields = fields.clone();
                    fields[4] = sley_scb1::encode_list(&[option_type_chain(k)])
                        .expect("result type list encodes");
                    parameter_schema_with_fields(8, &fields)
                }
            }),
            61,
        ),
        (
            "operation FunctionRef immediate type argument (depth 5)",
            8,
            Box::new({
                let fields = operation_fields.clone();
                move |k| {
                    let mut fields = fields.clone();
                    let function_ref = sley_scb1::encode_record(&[
                        (1, vec![0x8a; 32]),
                        (
                            2,
                            sley_scb1::encode_list(&[option_type_chain(k)])
                                .expect("type argument list encodes"),
                        ),
                    ])
                    .expect("FunctionRefValue encodes");
                    fields[5] =
                        sley_scb1::encode_union(7, &function_ref).expect("immediate encodes");
                    parameter_schema_with_fields(8, &fields)
                }
            }),
            59,
        ),
        (
            "type_def record field value_type (depth 5)",
            4,
            Box::new({
                let fields = type_def_fields.clone();
                let member = member.clone();
                move |k| {
                    let mut fields = fields.clone();
                    let field = sley_scb1::encode_record(&[
                        (1, member.clone()),
                        (2, option_type_chain(k)),
                        (3, sley_scb1::encode_uvar(1)),
                    ])
                    .expect("RecordField encodes");
                    let list = sley_scb1::encode_list(&[field]).expect("field list encodes");
                    fields[1] = sley_scb1::encode_union(1, &list).expect("Record form encodes");
                    parameter_schema_with_fields(4, &fields)
                }
            }),
            59,
        ),
        (
            "type_def variant case optional payload (depth 6)",
            4,
            Box::new({
                let fields = type_def_fields.clone();
                let member = member.clone();
                move |k| {
                    let mut fields = fields.clone();
                    let payload =
                        sley_scb1::encode_union(1, &option_type_chain(k)).expect("Some encodes");
                    let case = sley_scb1::encode_record(&[(1, member.clone()), (2, payload)])
                        .expect("VariantCase encodes");
                    let list = sley_scb1::encode_list(&[case]).expect("case list encodes");
                    fields[1] = sley_scb1::encode_union(2, &list).expect("Variant form encodes");
                    parameter_schema_with_fields(4, &fields)
                }
            }),
            58,
        ),
        (
            "constant.value sequence chain (three native levels per node)",
            9,
            Box::new({
                let fields = constant_fields.clone();
                move |k| {
                    let mut fields = fields.clone();
                    let unit_type = sley_scb1::encode_union(1, &[]).expect("Unit encodes");
                    fields[0] = sequence_const_chain(k, &unit_type);
                    parameter_schema_with_fields(9, &fields)
                }
            }),
            21,
        ),
    ];

    for (name, kind, build, first_refused) in &sites {
        let mut observed_first_refused = None;
        for k in first_refused - 4..=first_refused + 2 {
            let stored_bytes = stored(&build(k));
            let native = native_stored_verdict(&stored_bytes);
            let sley = arbitrary_stored_verdict(&package, &approved, *kind, &stored_bytes);
            assert_eq!(sley, native, "{name}: Sley parity at {k} levels");
            eprintln!("NESTING_BOUNDARY {name} levels={k} verdict={native}");
            if native != "OK" && observed_first_refused.is_none() {
                observed_first_refused = Some(k);
            }
        }
        assert_eq!(
            observed_first_refused,
            Some(*first_refused),
            "{name}: native boundary"
        );
    }

    // Known deviation (rw-090-codec-component-manifest.json known_deviations):
    // a TypeExpr inside a NESTED ConstValue is charged from the constant's
    // root (depth 3) rather than the node's own depth, so a type chain that
    // the native codec refuses beneath a deep constant node is still accepted
    // here. Pinned so the gap is visible until the const projector threads
    // the node depth into the TypeExpr core.
    {
        let mut fields = constant_fields.clone();
        // Constant root at 2; the node under one Sequence level sits at 5, so
        // its `value_type` is at 6 and an Option chain of 59 levels reaches
        // depth 64 natively, while the root-charged decoder allows up to 61.
        fields[0] = sequence_const_chain(1, &option_type_chain(59));
        let stored_bytes = stored(&parameter_schema_with_fields(9, &fields));
        assert_eq!(native_stored_verdict(&stored_bytes), "SCB_RESOURCE_LIMIT");
        assert_eq!(
            arbitrary_stored_verdict(&package, &approved, 9, &stored_bytes),
            "OK",
            "known deviation: nested-const TypeExpr depth is charged from the root"
        );
        fields[0] = sequence_const_chain(1, &option_type_chain(61));
        let stored_bytes = stored(&parameter_schema_with_fields(9, &fields));
        assert_eq!(native_stored_verdict(&stored_bytes), "SCB_RESOURCE_LIMIT");
        assert_eq!(
            arbitrary_stored_verdict(&package, &approved, 9, &stored_bytes),
            "SCB_RESOURCE_LIMIT",
            "the root-charged bound still applies"
        );
    }
}

/// Resource bound of the canonical entry under `codec_profile_limits`
/// (RW-090 Vulcan P3): a valid body the native codec accepts but whose
/// validation exceeds the profile's budget terminates the VM with a
/// deterministic resource limit, not with a typed refusal. Identity-heavy
/// bodies bind on the value-unit envelope first (a 1,750-byte Workspace
/// here, at 33,486 instructions); instruction-heavy bodies bind on the
/// 100,000-instruction ceiling (roughly 90 instructions per body byte). Both
/// bounds are disclosed in rw-090-codec-component-manifest.json
/// (`resource_bound`).
#[test]
fn arbitrary_dispatch_over_budget_body_terminates_with_a_resource_limit() {
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, WorkspaceBody};

    let image = super::all_kind_digest_dispatch::arbitrary_all_kind_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    let workspace = |members: u8| {
        let packages = EntityIdSet::from_unsorted(
            (1..=members)
                .map(|fill| EntityId::from_bytes([fill; 32]))
                .collect(),
        )
        .expect("ascending identities form a canonical set");
        entity_body_bytes(
            [0xa1; 32],
            EntityBodyValue::Workspace(WorkspaceBody {
                packages,
                root_namespace: EntityId::from_bytes([0xf1; 32]),
                capability_requirements: EntityIdSet::from_unsorted(vec![]).unwrap(),
                contracts: EntityIdSet::from_unsorted(vec![]).unwrap(),
                tests: EntityIdSet::from_unsorted(vec![]).unwrap(),
            }),
        )
    };
    let stored = |body: &[u8]| super::all_kind_digest_dispatch::stored_from_body([0xa1; 32], body);
    // 24 packages: 958 bytes stored, inside the budget; 48 packages: 1,750
    // bytes, over it. Both are accepted by the native codec.
    let inside = stored(&workspace(24));
    let over = stored(&workspace(48));
    for body in [&inside, &over] {
        sley_mutate::import_entity_object(program_epoch9(), body)
            .expect("the native codec accepts both workspaces");
    }
    let (verdict, outcome) = arbitrary_dispatch_decode(&package, &approved, 1, &inside);
    assert!(
        verdict.is_ok(),
        "the 958-byte workspace validates inside the budget"
    );
    eprintln!(
        "RESOURCE_BOUND inside stored={}B instructions={} fuel={}",
        inside.len(),
        outcome.instruction_count,
        outcome.fuel_used
    );
    let outcome = execute_with_limits(
        &package,
        &approved,
        vec![u64_input(1), bytes_input(&over), unit_input()],
        codec_profile_limits(),
    );
    eprintln!(
        "RESOURCE_BOUND over stored={}B termination={:?} instructions={}",
        over.len(),
        outcome.termination,
        outcome.instruction_count
    );
    assert_eq!(over.len(), 1_750);
    assert!(
        matches!(
            outcome.termination,
            sley_vm::ExecutionTermination::ResourceLimit(sley_vm::ResourceKind::ValueUnits)
        ),
        "an over-budget identity-heavy body is the VM's value-unit limit: {:?}",
        outcome.termination
    );
    assert_eq!(outcome.instruction_count, 33_486);
}

#[test]
fn arbitrary_dispatch_forwards_schema_refusals_and_refuses_unknown_kinds() {
    let image = super::all_kind_digest_dispatch::arbitrary_all_kind_decode_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    let entity = [0xcc; 32];
    let stored = |body: &[u8]| super::all_kind_digest_dispatch::stored_from_body(entity, body);

    let workspace_fields = exact_entity_body_fields(&rich_identity_bodies()[0].1, 1, 5);
    let mut unordered_packages = workspace_fields.clone();
    unordered_packages[0] =
        sley_scb1::encode_list(&[vec![0x13; 32], vec![0x11; 32]]).expect("list encodes");
    let type_def_fields =
        exact_entity_body_fields(&type_def_schema_body(type_def_record_form()), 4, 4);
    let mut unknown_form = type_def_fields.clone();
    unknown_form[1] = sley_scb1::encode_union(3, &[]).expect("union encodes");
    let constant_fields = exact_entity_body_fields(
        &constant_schema_body(const_of(TypeExpr::Bool, ConstData::Bool(true))),
        9,
        1,
    );
    let mut bad_leaf = constant_fields.clone();
    *bad_leaf[0].last_mut().unwrap() = 2;

    let cases: Vec<(&str, u64, Vec<u8>, &[u8])> = vec![
        (
            "unordered_workspace_packages",
            1,
            parameter_schema_with_fields(1, &unordered_packages),
            b"SCB_MAP_ORDER",
        ),
        (
            "unknown_type_def_form",
            4,
            parameter_schema_with_fields(4, &unknown_form),
            b"SCB_UNION_INVALID",
        ),
        (
            "invalid_constant_leaf",
            9,
            parameter_schema_with_fields(9, &bad_leaf),
            b"SCB_BOOL_INVALID",
        ),
        (
            "declared_kind_mismatch",
            2,
            parameter_schema_with_fields(1, &workspace_fields),
            b"SCB_UNION_INVALID",
        ),
        (
            "missing_workspace_field",
            1,
            parameter_schema_with_fields(1, &workspace_fields[..4]),
            b"SCB_FIELD_MISSING",
        ),
        (
            "unknown_kind_zero",
            0,
            parameter_schema_with_fields(1, &workspace_fields),
            b"SSMC_ENTITY_KIND_UNKNOWN",
        ),
        (
            "unknown_kind_nineteen",
            19,
            parameter_schema_with_fields(1, &workspace_fields),
            b"SSMC_ENTITY_KIND_UNKNOWN",
        ),
    ];
    for (name, kind, body, expected) in cases {
        let (verdict, _) = arbitrary_dispatch_decode(&package, &approved, kind, &stored(&body));
        assert_eq!(
            verdict.as_ref().map_err(Vec::as_slice),
            Err(expected),
            "{name} refusal"
        );
    }
}
