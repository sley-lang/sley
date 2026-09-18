//! `DependencyBinding` (entity kind 18) body encoding construction.
//! Construction provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-dependency-binding-encode.md`.

use super::supported_dispatch::push_preallocated_block as append_block;
use super::*;
use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};

#[derive(Clone, Copy)]
struct EncodeBlocks {
    function: EntityId,
    length_error: EntityId,
    trailing_error: EntityId,
    resource_error: EntityId,
    invariant_trap: EntityId,
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

/// Builds one exact-32-byte gate while preserving the caller's value tuple.
/// Short inputs use the native fixed-width underflow code; long inputs use the
/// nested trailing code. The returned block accepts `value_types` in order.
fn build_exact_32_gate(
    assembler: &mut Asm,
    ns: Ns,
    control: EncodeBlocks,
    value_types: &[TypeExpr],
    checked_index: usize,
    constant_32: EntityId,
    success: EntityId,
) -> EntityId {
    let check = assembler.id(ns.b);
    let check_greater = assembler.id(ns.b);
    let parameters = block_parameters(assembler, ns.p, check, value_types);
    let length = assembler.op(
        ns.o,
        check,
        Opcode::VectorLen,
        vec![pav(parameters[checked_index])],
        vec![u64_type()],
        Immediate::None,
    );
    let bound = assembler.cref(ns.o, check, constant_32, u64_type());
    let is_short = assembler.op(
        ns.o,
        check,
        Opcode::LessThan,
        vec![op_result(length), op_result(bound)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let is_long = assembler.op(
        ns.o,
        check,
        Opcode::GreaterThan,
        vec![op_result(length), op_result(bound)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let mut greater_arguments = parameters.iter().copied().map(pav).collect::<Vec<_>>();
    greater_arguments.push(op_result(is_long));
    append_block(
        assembler,
        check,
        control.function,
        parameters,
        vec![length, bound, is_short, is_long],
        cond(
            op_result(is_short),
            edge(control.length_error, Vec::new()),
            edge(check_greater, greater_arguments),
        ),
    );

    let mut greater_types = value_types.to_vec();
    greater_types.push(TypeExpr::Bool);
    let greater_parameters = block_parameters(assembler, ns.p, check_greater, &greater_types);
    let flag = *greater_parameters.last().expect("greater gate flag");
    let success_arguments = greater_parameters[..value_types.len()]
        .iter()
        .copied()
        .map(pav)
        .collect();
    append_block(
        assembler,
        check_greater,
        control.function,
        greater_parameters,
        Vec::new(),
        cond(
            pav(flag),
            edge(control.trailing_error, Vec::new()),
            edge(success, success_arguments),
        ),
    );
    check
}

/// Builds a backedge-free chain that appends fixed bytes to a `UInt8` vector.
/// The head accepts `(accumulator, carries...)`; the destination receives the
/// same shape. Adapter refusal forwards to the supplied resource block.
fn build_constant_push_chain(
    assembler: &mut Asm,
    ns: Ns,
    control: EncodeBlocks,
    bytes: &[u8],
    carry_types: &[TypeExpr],
    destination: EntityId,
) -> EntityId {
    assert!(!bytes.is_empty(), "constant chain is nonempty");
    let blocks = (0..bytes.len())
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let mut parameter_types = vec![u8vec_type()];
    parameter_types.extend_from_slice(carry_types);
    for (index, (block, byte)) in blocks.iter().copied().zip(bytes).enumerate() {
        let parameters = block_parameters(assembler, ns.p, block, &parameter_types);
        let accumulator = parameters[0];
        let byte_constant = assembler.ku8(ns.k, u128::from(*byte));
        let byte_value = assembler.cref(ns.o, block, byte_constant, u8_type());
        let pushed = assembler.op(
            ns.o,
            block,
            Opcode::AdapterInvoke,
            vec![pav(accumulator), op_result(byte_value)],
            vec![index_result(u8vec_type())],
            Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
        );
        let target = blocks.get(index + 1).copied().unwrap_or(destination);
        let mut arguments = vec![SwitchArgument::CasePayload];
        arguments.extend(parameters[1..].iter().copied().map(sav));
        append_block(
            assembler,
            block,
            control.function,
            parameters,
            vec![byte_value, pushed],
            switch(
                op_result(pushed),
                vec![
                    (BuiltinCase::Ok, target, arguments),
                    (BuiltinCase::Err, control.resource_error, Vec::new()),
                ],
            ),
        );
    }
    blocks[0]
}

/// Builds an unrolled 32-byte source-vector copy. The head accepts
/// `(accumulator, source, carries...)`; the destination receives
/// `(accumulator, carries...)`. Every indexed read is construction-bounded.
fn build_copy_32_chain(
    assembler: &mut Asm,
    ns: Ns,
    control: EncodeBlocks,
    carry_types: &[TypeExpr],
    destination: EntityId,
) -> EntityId {
    let gets = (0..32).map(|_| assembler.id(ns.b)).collect::<Vec<_>>();
    let push_blocks = (0..32).map(|_| assembler.id(ns.b)).collect::<Vec<_>>();
    let mut get_types = vec![u8vec_type(), u8vec_type()];
    get_types.extend_from_slice(carry_types);
    let mut push_types = vec![u8_type(), u8vec_type(), u8vec_type()];
    push_types.extend_from_slice(carry_types);

    for index in 0..32usize {
        let get_block = gets[index];
        let push_block = push_blocks[index];
        let get_parameters = block_parameters(assembler, ns.p, get_block, &get_types);
        let accumulator = get_parameters[0];
        let source = get_parameters[1];
        let index_constant = assembler.ku64(
            ns.k,
            u128::try_from(index).expect("fixed copy index fits u128"),
        );
        let index_value = assembler.cref(ns.o, get_block, index_constant, u64_type());
        let byte = assembler.op(
            ns.o,
            get_block,
            Opcode::VectorGet,
            vec![pav(source), op_result(index_value)],
            vec![TypeExpr::Option(Box::new(u8_type()))],
            Immediate::None,
        );
        let mut some_arguments = vec![SwitchArgument::CasePayload, sav(accumulator), sav(source)];
        some_arguments.extend(get_parameters[2..].iter().copied().map(sav));
        append_block(
            assembler,
            get_block,
            control.function,
            get_parameters,
            vec![index_value, byte],
            switch(
                op_result(byte),
                vec![
                    (BuiltinCase::None, control.invariant_trap, Vec::new()),
                    (BuiltinCase::Some, push_block, some_arguments),
                ],
            ),
        );

        let push_parameters = block_parameters(assembler, ns.p, push_block, &push_types);
        let appended = assembler.op(
            ns.o,
            push_block,
            Opcode::AdapterInvoke,
            vec![pav(push_parameters[1]), pav(push_parameters[0])],
            vec![index_result(u8vec_type())],
            Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
        );
        let target = if index == 31 {
            destination
        } else {
            gets[index + 1]
        };
        let mut next_arguments = vec![SwitchArgument::CasePayload];
        if index != 31 {
            next_arguments.push(sav(push_parameters[2]));
        }
        next_arguments.extend(push_parameters[3..].iter().copied().map(sav));
        append_block(
            assembler,
            push_block,
            control.function,
            push_parameters,
            vec![appended],
            switch(
                op_result(appended),
                vec![
                    (BuiltinCase::Ok, target, next_arguments),
                    (BuiltinCase::Err, control.resource_error, Vec::new()),
                ],
            ),
        );
    }
    gets[0]
}

#[allow(clippy::too_many_lines)]
fn build_dependency_binding_encode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = encode_result_type();
    let dependency_root = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let external_package =
        assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let local_namespace = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
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
        invariant_trap,
    };

    let output_done = assembler.id(ns.b);
    let output_return = assembler.id(ns.b);
    let output_accumulator = assembler.param(ns.p, output_done, ParameterRole::Block, u8vec_type());
    let output_unit = assembler.param(ns.p, output_done, ParameterRole::Block, TypeExpr::Unit);
    let converted = assembler.op(
        ns.o,
        output_done,
        Opcode::AdapterInvoke,
        vec![pav(output_unit), pav(output_accumulator)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    append_block(
        assembler,
        output_done,
        function,
        vec![output_accumulator, output_unit],
        vec![converted],
        switch(
            op_result(converted),
            vec![
                (
                    BuiltinCase::Ok,
                    output_return,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );
    let output_bytes = assembler.param(ns.p, output_return, ParameterRole::Block, TypeExpr::Bytes);
    let output_ok = assembler.op(
        ns.o,
        output_return,
        Opcode::ResultOk,
        vec![pav(output_bytes)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        output_return,
        function,
        vec![output_bytes],
        vec![output_ok],
        ret(op_result(output_ok)),
    );

    // Build the canonical body tail-first so every generated chain has a
    // concrete continuation: union(18, record(3, fixed32, fixed32, fixed32)).
    let copy_local = build_copy_32_chain(assembler, ns, control, &[TypeExpr::Unit], output_done);
    let prefix_local = build_constant_push_chain(
        assembler,
        ns,
        control,
        &[3, 32],
        &[u8vec_type(), TypeExpr::Unit],
        copy_local,
    );
    let copy_package = build_copy_32_chain(
        assembler,
        ns,
        control,
        &[u8vec_type(), TypeExpr::Unit],
        prefix_local,
    );
    let prefix_package = build_constant_push_chain(
        assembler,
        ns,
        control,
        &[2, 32],
        &[u8vec_type(), u8vec_type(), TypeExpr::Unit],
        copy_package,
    );
    let copy_root = build_copy_32_chain(
        assembler,
        ns,
        control,
        &[u8vec_type(), u8vec_type(), TypeExpr::Unit],
        prefix_package,
    );
    let prefix_root = build_constant_push_chain(
        assembler,
        ns,
        control,
        &[18, 103, 3, 1, 32],
        &[u8vec_type(), u8vec_type(), u8vec_type(), TypeExpr::Unit],
        copy_root,
    );

    let entry = assembler.id(ns.b);
    let convert_package = assembler.id(ns.b);
    let convert_local = assembler.id(ns.b);
    let final_values = [u8vec_type(), u8vec_type(), u8vec_type(), TypeExpr::Unit];
    let output_start = assembler.id(ns.b);
    let output_parameters = block_parameters(assembler, ns.p, output_start, &final_values);
    let empty = assembler.op(
        ns.o,
        output_start,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    append_block(
        assembler,
        output_start,
        function,
        output_parameters.clone(),
        vec![empty],
        branch(edge(
            prefix_root,
            std::iter::once(op_result(empty))
                .chain(output_parameters.iter().copied().map(pav))
                .collect(),
        )),
    );

    let local_length = build_exact_32_gate(
        assembler,
        ns,
        control,
        &final_values,
        2,
        constant_32,
        output_start,
    );
    let package_values = [u8vec_type(), u8vec_type(), TypeExpr::Bytes, TypeExpr::Unit];
    let package_length = build_exact_32_gate(
        assembler,
        ns,
        control,
        &package_values,
        1,
        constant_32,
        convert_local,
    );
    let root_values = [
        u8vec_type(),
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Unit,
    ];
    let root_length = build_exact_32_gate(
        assembler,
        ns,
        control,
        &root_values,
        0,
        constant_32,
        convert_package,
    );

    let root_vector = assembler.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(unit), pav(dependency_root)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![root_vector],
        switch(
            op_result(root_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    root_length,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(external_package),
                        sav(local_namespace),
                        sav(unit),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let package_root = assembler.param(ns.p, convert_package, ParameterRole::Block, u8vec_type());
    let package_bytes =
        assembler.param(ns.p, convert_package, ParameterRole::Block, TypeExpr::Bytes);
    let package_local =
        assembler.param(ns.p, convert_package, ParameterRole::Block, TypeExpr::Bytes);
    let package_unit = assembler.param(ns.p, convert_package, ParameterRole::Block, TypeExpr::Unit);
    let package_vector = assembler.op(
        ns.o,
        convert_package,
        Opcode::AdapterInvoke,
        vec![pav(package_unit), pav(package_bytes)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    append_block(
        assembler,
        convert_package,
        function,
        vec![package_root, package_bytes, package_local, package_unit],
        vec![package_vector],
        switch(
            op_result(package_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    package_length,
                    vec![
                        sav(package_root),
                        SwitchArgument::CasePayload,
                        sav(package_local),
                        sav(package_unit),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let local_root = assembler.param(ns.p, convert_local, ParameterRole::Block, u8vec_type());
    let local_package = assembler.param(ns.p, convert_local, ParameterRole::Block, u8vec_type());
    let local_bytes = assembler.param(ns.p, convert_local, ParameterRole::Block, TypeExpr::Bytes);
    let local_unit = assembler.param(ns.p, convert_local, ParameterRole::Block, TypeExpr::Unit);
    let local_vector = assembler.op(
        ns.o,
        convert_local,
        Opcode::AdapterInvoke,
        vec![pav(local_unit), pav(local_bytes)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    append_block(
        assembler,
        convert_local,
        function,
        vec![local_root, local_package, local_bytes, local_unit],
        vec![local_vector],
        switch(
            op_result(local_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    local_length,
                    vec![
                        sav(local_root),
                        sav(local_package),
                        SwitchArgument::CasePayload,
                        sav(local_unit),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![dependency_root, external_package, local_namespace, unit],
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

fn dependency_encode_image() -> Image {
    let mut assembler = Asm::new();
    let ns = Ns {
        k: 51,
        p: 52,
        b: 53,
        o: 54,
    };
    let function = eid(11, 1);
    let graph = build_dependency_binding_encode(&mut assembler, ns, function);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
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

pub(super) fn dependency_stored(root: u8, package: u8, namespace: u8) -> Vec<u8> {
    use sley_mutate::value::{DependencyBindingBody, EntityBodyValue};
    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes([0xd1; 32]),
        body: EntityBodyValue::DependencyBinding(DependencyBindingBody {
            dependency_root: sley_id::StateRoot::from_bytes([root; 32]),
            external_package: sley_id::EntityId::from_bytes([package; 32]),
            local_namespace: sley_id::EntityId::from_bytes([namespace; 32]),
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds DependencyBinding fixture")
        .stored_bytes()
        .to_vec()
}

fn dependency_encode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    root: &[u8],
    external_package: &[u8],
    local_namespace: &[u8],
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![
            bytes_input(root),
            bytes_input(external_package),
            bytes_input(local_namespace),
            unit_input(),
        ],
    )
}

#[test]
fn dependency_binding_encode_matches_native_body_for_distinct_inputs() {
    let image = dependency_encode_image();
    let (package, approved) = admit(&image);
    for (root, external, local) in [(1u8, 2u8, 3u8), (4, 5, 6), (7, 8, 9)] {
        let expected = ns_body_of(&dependency_stored(root, external, local));
        assert_eq!(expected.len(), 105, "fixed DependencyBinding body size");
        let outcome = dependency_encode_call(
            &package,
            &approved,
            &[root; 32],
            &[external; 32],
            &[local; 32],
        );
        assert_encode_ok(&outcome, &expected);
        eprintln!(
            "DEP_ENC root{root} pkg{external} ns{local} fuel={} instr={} peak={}",
            outcome.fuel_used, outcome.instruction_count, outcome.peak_value_units
        );
    }
}

#[test]
fn dependency_binding_encode_rejects_each_non_fixed_identity_in_field_order() {
    let image = dependency_encode_image();
    let (package, approved) = admit(&image);
    let exact = vec![1u8; 32];
    for (name, root, external, local, code) in [
        (
            "root_short",
            vec![1; 31],
            exact.clone(),
            exact.clone(),
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "root_long",
            vec![1; 33],
            exact.clone(),
            exact.clone(),
            "SCB_TRAILING_BYTES",
        ),
        (
            "package_short",
            exact.clone(),
            vec![1; 31],
            exact.clone(),
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "package_long",
            exact.clone(),
            vec![1; 33],
            exact.clone(),
            "SCB_TRAILING_BYTES",
        ),
        (
            "local_short",
            exact.clone(),
            exact.clone(),
            vec![1; 31],
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "local_long",
            exact.clone(),
            exact.clone(),
            vec![1; 33],
            "SCB_TRAILING_BYTES",
        ),
        (
            "root_short_beats_later_faults",
            vec![1; 31],
            vec![1; 33],
            vec![1; 31],
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "package_long_beats_local_short",
            exact.clone(),
            vec![1; 33],
            vec![1; 31],
            "SCB_TRAILING_BYTES",
        ),
    ] {
        assert_refusal(
            &dependency_encode_call(&package, &approved, &root, &external, &local),
            code,
        );
        eprintln!("DEP_ENC_REJ {name} -> {code}");
    }
}
