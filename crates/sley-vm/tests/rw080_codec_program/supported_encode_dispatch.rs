//! Supported-kind encode dispatch kept separate from the large retained
//! per-format construction record in the parent integration test.
//! Construction provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-supported-encode-dispatch.md`.

use super::supported_dispatch::{
    deduplicate_identical_constants as deduplicate_constants,
    entrypoint_program_value_type as entrypoint_encode_value_type,
    namespace_program_value_type as namespace_encode_value_type,
    push_preallocated_block as append_block,
    supported_program_value_type as supported_encode_value_type,
};
use super::*;
use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_RHW1, BRIDGE_CODE_V2B1};

#[allow(clippy::too_many_lines)]
fn build_supported_program_encode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    entrypoint_encoder: EntityId,
    namespace_encoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = encode_result_type();
    let value_type = supported_encode_value_type();
    let declared_kind = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let value = assembler.param(ns.p, function, ParameterRole::Function, value_type.clone());
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

    let entry = assembler.id(ns.b);
    let select_namespace = assembler.id(ns.b);
    let check_entrypoint = assembler.id(ns.b);
    let select_entrypoint = assembler.id(ns.b);
    let check_known = assembler.id(ns.b);
    let call_namespace = assembler.id(ns.b);
    let call_entrypoint = assembler.id(ns.b);
    let mismatch = assembler.id(ns.b);
    let unsupported = assembler.id(ns.b);
    let unknown = assembler.id(ns.b);

    let zero = assembler.ku64(ns.k, 0);
    let namespace_kind = assembler.ku64(ns.k, 3);
    let entrypoint_kind = assembler.ku64(ns.k, 16);
    let kind_limit = assembler.ku64(ns.k, 19);
    let scope_code = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");
    let unknown_code = assembler.kbytes(ns.k, b"SSMC_ENTITY_KIND_UNKNOWN");

    let namespace_kind_value = assembler.cref(ns.o, entry, namespace_kind, u64_type());
    let is_namespace = assembler.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(declared_kind), op_result(namespace_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![namespace_kind_value, is_namespace],
        cond(
            op_result(is_namespace),
            edge(select_namespace, vec![pav(value), pav(unit)]),
            edge(
                check_entrypoint,
                vec![pav(declared_kind), pav(value), pav(unit)],
            ),
        ),
    );

    let namespace_value = assembler.param(
        ns.p,
        select_namespace,
        ParameterRole::Block,
        value_type.clone(),
    );
    let namespace_unit =
        assembler.param(ns.p, select_namespace, ParameterRole::Block, TypeExpr::Unit);
    append_block(
        assembler,
        select_namespace,
        function,
        vec![namespace_value, namespace_unit],
        Vec::new(),
        switch(
            pav(namespace_value),
            vec![
                (BuiltinCase::Ok, mismatch, Vec::new()),
                (
                    BuiltinCase::Err,
                    call_namespace,
                    vec![SwitchArgument::CasePayload, sav(namespace_unit)],
                ),
            ],
        ),
    );

    let candidate_kind = assembler.param(ns.p, check_entrypoint, ParameterRole::Block, u64_type());
    let candidate_value = assembler.param(
        ns.p,
        check_entrypoint,
        ParameterRole::Block,
        value_type.clone(),
    );
    let candidate_unit =
        assembler.param(ns.p, check_entrypoint, ParameterRole::Block, TypeExpr::Unit);
    let entrypoint_kind_value = assembler.cref(ns.o, check_entrypoint, entrypoint_kind, u64_type());
    let is_entrypoint = assembler.op(
        ns.o,
        check_entrypoint,
        Opcode::Equal,
        vec![pav(candidate_kind), op_result(entrypoint_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check_entrypoint,
        function,
        vec![candidate_kind, candidate_value, candidate_unit],
        vec![entrypoint_kind_value, is_entrypoint],
        cond(
            op_result(is_entrypoint),
            edge(
                select_entrypoint,
                vec![pav(candidate_value), pav(candidate_unit)],
            ),
            edge(check_known, vec![pav(candidate_kind)]),
        ),
    );

    let entrypoint_value =
        assembler.param(ns.p, select_entrypoint, ParameterRole::Block, value_type);
    let entrypoint_unit = assembler.param(
        ns.p,
        select_entrypoint,
        ParameterRole::Block,
        TypeExpr::Unit,
    );
    append_block(
        assembler,
        select_entrypoint,
        function,
        vec![entrypoint_value, entrypoint_unit],
        Vec::new(),
        switch(
            pav(entrypoint_value),
            vec![
                (
                    BuiltinCase::Ok,
                    call_entrypoint,
                    vec![SwitchArgument::CasePayload, sav(entrypoint_unit)],
                ),
                (BuiltinCase::Err, mismatch, Vec::new()),
            ],
        ),
    );

    let remaining_kind = assembler.param(ns.p, check_known, ParameterRole::Block, u64_type());
    let zero_value = assembler.cref(ns.o, check_known, zero, u64_type());
    let limit_value = assembler.cref(ns.o, check_known, kind_limit, u64_type());
    let above_zero = assembler.op(
        ns.o,
        check_known,
        Opcode::LessThan,
        vec![op_result(zero_value), pav(remaining_kind)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let below_limit = assembler.op(
        ns.o,
        check_known,
        Opcode::LessThan,
        vec![pav(remaining_kind), op_result(limit_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let is_known = assembler.op(
        ns.o,
        check_known,
        Opcode::BoolAnd,
        vec![op_result(above_zero), op_result(below_limit)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check_known,
        function,
        vec![remaining_kind],
        vec![zero_value, limit_value, above_zero, below_limit, is_known],
        cond(
            op_result(is_known),
            edge(unsupported, Vec::new()),
            edge(unknown, Vec::new()),
        ),
    );

    let namespace_payload = assembler.param(
        ns.p,
        call_namespace,
        ParameterRole::Block,
        namespace_encode_value_type(),
    );
    let namespace_call_unit =
        assembler.param(ns.p, call_namespace, ParameterRole::Block, TypeExpr::Unit);
    let namespace_entity = assembler.op(
        ns.o,
        call_namespace,
        Opcode::TupleGet,
        vec![pav(namespace_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let namespace_parent = assembler.op(
        ns.o,
        call_namespace,
        Opcode::TupleGet,
        vec![pav(namespace_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let namespace_members = assembler.op(
        ns.o,
        call_namespace,
        Opcode::TupleGet,
        vec![pav(namespace_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(2),
    );
    let namespace_result = assembler.op(
        ns.o,
        call_namespace,
        Opcode::CallDirect,
        vec![
            op_result(namespace_entity),
            op_result(namespace_parent),
            op_result(namespace_members),
            pav(namespace_call_unit),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: namespace_encoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        call_namespace,
        function,
        vec![namespace_payload, namespace_call_unit],
        vec![
            namespace_entity,
            namespace_parent,
            namespace_members,
            namespace_result,
        ],
        ret(op_result(namespace_result)),
    );

    let entrypoint_payload = assembler.param(
        ns.p,
        call_entrypoint,
        ParameterRole::Block,
        entrypoint_encode_value_type(),
    );
    let entrypoint_call_unit =
        assembler.param(ns.p, call_entrypoint, ParameterRole::Block, TypeExpr::Unit);
    let entrypoint_entity = assembler.op(
        ns.o,
        call_entrypoint,
        Opcode::TupleGet,
        vec![pav(entrypoint_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let entrypoint_function = assembler.op(
        ns.o,
        call_entrypoint,
        Opcode::TupleGet,
        vec![pav(entrypoint_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let entrypoint_exposure = assembler.op(
        ns.o,
        call_entrypoint,
        Opcode::TupleGet,
        vec![pav(entrypoint_payload)],
        vec![u64_type()],
        Immediate::Index(2),
    );
    let entrypoint_result = assembler.op(
        ns.o,
        call_entrypoint,
        Opcode::CallDirect,
        vec![
            op_result(entrypoint_entity),
            op_result(entrypoint_function),
            op_result(entrypoint_exposure),
            pav(entrypoint_call_unit),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: entrypoint_encoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        call_entrypoint,
        function,
        vec![entrypoint_payload, entrypoint_call_unit],
        vec![
            entrypoint_entity,
            entrypoint_function,
            entrypoint_exposure,
            entrypoint_result,
        ],
        ret(op_result(entrypoint_result)),
    );

    for (block, code) in [
        (mismatch, scope_code),
        (unsupported, scope_code),
        (unknown, unknown_code),
    ] {
        let code_value = assembler.cref(ns.o, block, code, TypeExpr::Bytes);
        let error = assembler.op(
            ns.o,
            block,
            Opcode::ResultErr,
            vec![op_result(code_value)],
            vec![result_type.clone()],
            Immediate::None,
        );
        append_block(
            assembler,
            block,
            function,
            Vec::new(),
            vec![code_value, error],
            ret(op_result(error)),
        );
    }

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![declared_kind, value, unit],
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
fn supported_encode_image() -> Image {
    let mut assembler = Asm::new();
    let entrypoint_ns = Ns {
        k: 11,
        p: 12,
        b: 13,
        o: 14,
    };
    let uvar_ns = Ns {
        k: 15,
        p: 16,
        b: 17,
        o: 18,
    };
    let outer_ns = Ns {
        k: 19,
        p: 20,
        b: 21,
        o: 22,
    };
    let build_ns = Ns {
        k: 23,
        p: 24,
        b: 25,
        o: 26,
    };
    let entrypoint_program_ns = Ns {
        k: 27,
        p: 28,
        b: 29,
        o: 30,
    };
    let namespace_ns = Ns {
        k: 31,
        p: 32,
        b: 33,
        o: 34,
    };
    let namespace_program_ns = Ns {
        k: 35,
        p: 36,
        b: 37,
        o: 38,
    };
    let dispatch_ns = Ns {
        k: 39,
        p: 40,
        b: 41,
        o: 42,
    };

    let entrypoint_body = eid(10, 1);
    let uvar = eid(10, 2);
    let outer = eid(10, 3);
    let build = eid(10, 4);
    let entrypoint_program = eid(10, 5);
    let namespace_body = eid(10, 6);
    let namespace_program = eid(10, 7);
    let dispatch = eid(10, 8);

    let entrypoint_body_graph =
        build_entrypoint_encode(&mut assembler, entrypoint_ns, entrypoint_body);
    let uvar_graph = build_encode(&mut assembler, uvar_ns, uvar);
    let outer_graph = build_outer_encode(&mut assembler, outer_ns, outer, uvar);
    let build_graph = build_program_build(&mut assembler, build_ns, build);
    let entrypoint_program_graph = build_program_encode(
        &mut assembler,
        entrypoint_program_ns,
        entrypoint_program,
        entrypoint_body,
        outer,
        build,
        uvar,
    );
    let namespace_body_graph =
        build_namespace_encode(&mut assembler, namespace_ns, namespace_body, uvar);
    let namespace_program_graph = build_program_ns_encode(
        &mut assembler,
        namespace_program_ns,
        namespace_program,
        namespace_body,
        outer,
        build,
        uvar,
    );
    let dispatch_graph = build_supported_program_encode(
        &mut assembler,
        dispatch_ns,
        dispatch,
        entrypoint_program,
        namespace_program,
    );

    let mut image = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: dispatch_graph.clone(),
        functions: vec![
            dispatch_graph,
            entrypoint_program_graph,
            namespace_program_graph,
            entrypoint_body_graph,
            namespace_body_graph,
            outer_graph,
            build_graph,
            uvar_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
            frozen_import(BRIDGE_CODE_RHW1, TypeExpr::Bytes, TypeExpr::Bytes),
        ],
        constants: assembler.constants,
    };
    assert!(
        deduplicate_constants(&mut image) > 0,
        "composed format graphs share canonical constants"
    );
    image
}

fn tuple_value(value_type: TypeExpr, items: Vec<ConstValue>) -> ConstValue {
    ConstValue {
        value_type,
        data: ConstData::Sequence(items),
    }
}

fn entrypoint_value(entity: u8, function: u8, exposure: u64) -> ConstValue {
    ConstValue {
        value_type: supported_encode_value_type(),
        data: ConstData::Result(ResultConst::Ok(Box::new(tuple_value(
            entrypoint_encode_value_type(),
            vec![
                bytes_input(&[entity; 32]),
                bytes_input(&[function; 32]),
                u64_input(exposure),
            ],
        )))),
    }
}

fn namespace_value(entity: u8, parent: Option<u8>, members: &[u8]) -> ConstValue {
    let (parent_bytes, member_bytes, _) = ns_semantics(parent, members);
    ConstValue {
        value_type: supported_encode_value_type(),
        data: ConstData::Result(ResultConst::Err(Box::new(tuple_value(
            namespace_encode_value_type(),
            vec![
                bytes_input(&[entity; 32]),
                bytes_input(&parent_bytes),
                bytes_input(&member_bytes),
            ],
        )))),
    }
}

fn supported_encode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    kind: u64,
    value: ConstValue,
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![u64_input(kind), value, unit_input()],
    )
}

#[test]
fn codec_supported_kind_encode_dispatch_emits_entrypoint_and_namespace() {
    let image = supported_encode_image();
    let (package, approved) = admit(&image);

    let entrypoint = entrypoint_value(0xa1, 0xb2, 2);
    let entrypoint_outcome = supported_encode_call(&package, &approved, 16, entrypoint);
    assert_encode_ok(
        &entrypoint_outcome,
        &program_stored(0xa1, 0xb2, sley_mutate::value::EntryExposure::Protocol),
    );
    eprintln!(
        "SUPPORTED_ENC kind16 fuel={} instr={} peak={}",
        entrypoint_outcome.fuel_used,
        entrypoint_outcome.instruction_count,
        entrypoint_outcome.peak_value_units
    );

    let namespace = namespace_value(0xc1, None, &[]);
    let namespace_outcome = supported_encode_call(&package, &approved, 3, namespace);
    assert_encode_ok(&namespace_outcome, &program_ns_stored(0xc1, None, &[]));
    eprintln!(
        "SUPPORTED_ENC kind3 fuel={} instr={} peak={}",
        namespace_outcome.fuel_used,
        namespace_outcome.instruction_count,
        namespace_outcome.peak_value_units
    );
}

#[test]
fn codec_supported_kind_dispatch_round_trips_both_value_arms() {
    let decode_image = super::supported_dispatch::supported_decode_image();
    let (decode_package, decode_approved) = admit(&decode_image);
    let encode_image = supported_encode_image();
    let (encode_package, encode_approved) = admit(&encode_image);

    let entrypoint = program_stored(0xa1, 0xb2, sley_mutate::value::EntryExposure::Local);
    let decoded_entrypoint = super::supported_dispatch::supported_decode_ok(
        &super::supported_dispatch::supported_decode_call(
            &decode_package,
            &decode_approved,
            16,
            &entrypoint,
        ),
    );
    assert_eq!(decoded_entrypoint.value_type, supported_encode_value_type());
    assert_encode_ok(
        &supported_encode_call(&encode_package, &encode_approved, 16, decoded_entrypoint),
        &entrypoint,
    );

    let namespace = program_ns_stored(0xc1, None, &[]);
    let decoded_namespace = super::supported_dispatch::supported_decode_ok(
        &super::supported_dispatch::supported_decode_call(
            &decode_package,
            &decode_approved,
            3,
            &namespace,
        ),
    );
    assert_eq!(decoded_namespace.value_type, supported_encode_value_type());
    assert_encode_ok(
        &supported_encode_call(&encode_package, &encode_approved, 3, decoded_namespace),
        &namespace,
    );
}

#[test]
fn codec_supported_kind_encode_dispatch_rejects_mismatched_and_unknown_kinds() {
    let image = supported_encode_image();
    let (package, approved) = admit(&image);

    assert_refusal(
        &supported_encode_call(&package, &approved, 3, entrypoint_value(1, 2, 1)),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(&package, &approved, 16, namespace_value(1, None, &[])),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(&package, &approved, 1, entrypoint_value(1, 2, 1)),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(&package, &approved, 0, entrypoint_value(1, 2, 1)),
        "SSMC_ENTITY_KIND_UNKNOWN",
    );
    assert_refusal(
        &supported_encode_call(&package, &approved, 19, entrypoint_value(1, 2, 1)),
        "SSMC_ENTITY_KIND_UNKNOWN",
    );
}
