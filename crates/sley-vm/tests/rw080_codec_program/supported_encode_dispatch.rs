//! Supported-kind encode dispatch kept separate from the large retained
//! per-format construction record in the parent integration test.
//! Construction provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-supported-encode-dispatch.md`.
//! Kind-18 extension provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-dependency-binding-compose-encode.md`.

use super::supported_dispatch::{
    all_supported_program_value_type as all_supported_encode_value_type,
    deduplicate_identical_constants as deduplicate_constants,
    dependency_program_value_type as dependency_encode_value_type,
    entrypoint_program_value_type as entrypoint_encode_value_type,
    non_dependency_program_value_type as non_dependency_encode_value_type,
    push_preallocated_block as append_block,
    tagged_entity_set_program_value_type as tagged_entity_set_encode_value_type,
};
use super::*;
use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_RHW1, BRIDGE_CODE_V2B1};

#[allow(clippy::too_many_lines)]
fn build_supported_program_encode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    entrypoint_encoder: EntityId,
    entity_set_encoder: EntityId,
    dependency_encoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = encode_result_type();
    let value_type = non_dependency_encode_value_type();
    let extended_value_type = all_supported_encode_value_type();
    let declared_kind = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let value = assembler.param(
        ns.p,
        function,
        ParameterRole::Function,
        extended_value_type.clone(),
    );
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

    let entry = assembler.id(ns.b);
    let check_namespace = assembler.id(ns.b);
    let select_namespace = assembler.id(ns.b);
    let unwrap_namespace = assembler.id(ns.b);
    let validate_namespace = assembler.id(ns.b);
    let check_entrypoint = assembler.id(ns.b);
    let select_entrypoint = assembler.id(ns.b);
    let unwrap_entrypoint = assembler.id(ns.b);
    let select_dependency = assembler.id(ns.b);
    let check_known = assembler.id(ns.b);
    let call_namespace = assembler.id(ns.b);
    let call_entrypoint = assembler.id(ns.b);
    let call_dependency = assembler.id(ns.b);
    let mismatch = assembler.id(ns.b);
    let unsupported = assembler.id(ns.b);
    let unknown = assembler.id(ns.b);

    let zero = assembler.ku64(ns.k, 0);
    let namespace_kind = assembler.ku64(ns.k, 3);
    let policy_kind = assembler.ku64(ns.k, 17);
    let entrypoint_kind = assembler.ku64(ns.k, 16);
    let dependency_kind = assembler.ku64(ns.k, 18);
    let kind_limit = assembler.ku64(ns.k, 19);
    let scope_code = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");
    let unknown_code = assembler.kbytes(ns.k, b"SSMC_ENTITY_KIND_UNKNOWN");

    let dependency_kind_value = assembler.cref(ns.o, entry, dependency_kind, u64_type());
    let is_dependency = assembler.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(declared_kind), op_result(dependency_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![dependency_kind_value, is_dependency],
        cond(
            op_result(is_dependency),
            edge(select_dependency, vec![pav(value), pav(unit)]),
            edge(
                check_namespace,
                vec![pav(declared_kind), pav(value), pav(unit)],
            ),
        ),
    );

    let namespace_candidate_kind =
        assembler.param(ns.p, check_namespace, ParameterRole::Block, u64_type());
    let namespace_candidate_value = assembler.param(
        ns.p,
        check_namespace,
        ParameterRole::Block,
        extended_value_type.clone(),
    );
    let namespace_candidate_unit =
        assembler.param(ns.p, check_namespace, ParameterRole::Block, TypeExpr::Unit);
    let namespace_kind_value = assembler.cref(ns.o, check_namespace, namespace_kind, u64_type());
    let is_namespace = assembler.op(
        ns.o,
        check_namespace,
        Opcode::Equal,
        vec![
            pav(namespace_candidate_kind),
            op_result(namespace_kind_value),
        ],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let policy_kind_value = assembler.cref(ns.o, check_namespace, policy_kind, u64_type());
    let is_policy = assembler.op(
        ns.o,
        check_namespace,
        Opcode::Equal,
        vec![pav(namespace_candidate_kind), op_result(policy_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let is_entity_set = assembler.op(
        ns.o,
        check_namespace,
        Opcode::BoolOr,
        vec![op_result(is_namespace), op_result(is_policy)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check_namespace,
        function,
        vec![
            namespace_candidate_kind,
            namespace_candidate_value,
            namespace_candidate_unit,
        ],
        vec![
            namespace_kind_value,
            is_namespace,
            policy_kind_value,
            is_policy,
            is_entity_set,
        ],
        cond(
            op_result(is_entity_set),
            edge(
                unwrap_namespace,
                vec![
                    pav(namespace_candidate_value),
                    pav(namespace_candidate_unit),
                    pav(namespace_candidate_kind),
                ],
            ),
            edge(
                check_entrypoint,
                vec![
                    pav(namespace_candidate_kind),
                    pav(namespace_candidate_value),
                    pav(namespace_candidate_unit),
                ],
            ),
        ),
    );

    let wrapped_namespace = assembler.param(
        ns.p,
        unwrap_namespace,
        ParameterRole::Block,
        extended_value_type.clone(),
    );
    let wrapped_namespace_unit =
        assembler.param(ns.p, unwrap_namespace, ParameterRole::Block, TypeExpr::Unit);
    let wrapped_namespace_kind =
        assembler.param(ns.p, unwrap_namespace, ParameterRole::Block, u64_type());
    append_block(
        assembler,
        unwrap_namespace,
        function,
        vec![
            wrapped_namespace,
            wrapped_namespace_unit,
            wrapped_namespace_kind,
        ],
        Vec::new(),
        switch(
            pav(wrapped_namespace),
            vec![
                (
                    BuiltinCase::Ok,
                    select_namespace,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(wrapped_namespace_unit),
                        sav(wrapped_namespace_kind),
                    ],
                ),
                (BuiltinCase::Err, mismatch, Vec::new()),
            ],
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
    let namespace_declared_kind =
        assembler.param(ns.p, select_namespace, ParameterRole::Block, u64_type());
    append_block(
        assembler,
        select_namespace,
        function,
        vec![namespace_value, namespace_unit, namespace_declared_kind],
        Vec::new(),
        switch(
            pav(namespace_value),
            vec![
                (BuiltinCase::Ok, mismatch, Vec::new()),
                (
                    BuiltinCase::Err,
                    validate_namespace,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(namespace_unit),
                        sav(namespace_declared_kind),
                    ],
                ),
            ],
        ),
    );

    let namespace_payload = assembler.param(
        ns.p,
        validate_namespace,
        ParameterRole::Block,
        tagged_entity_set_encode_value_type(),
    );
    let namespace_validate_unit = assembler.param(
        ns.p,
        validate_namespace,
        ParameterRole::Block,
        TypeExpr::Unit,
    );
    let namespace_validate_kind =
        assembler.param(ns.p, validate_namespace, ParameterRole::Block, u64_type());
    let namespace_payload_kind = assembler.op(
        ns.o,
        validate_namespace,
        Opcode::TupleGet,
        vec![pav(namespace_payload)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let namespace_tag_matches = assembler.op(
        ns.o,
        validate_namespace,
        Opcode::Equal,
        vec![
            op_result(namespace_payload_kind),
            pav(namespace_validate_kind),
        ],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        validate_namespace,
        function,
        vec![
            namespace_payload,
            namespace_validate_unit,
            namespace_validate_kind,
        ],
        vec![namespace_payload_kind, namespace_tag_matches],
        cond(
            op_result(namespace_tag_matches),
            edge(
                call_namespace,
                vec![pav(namespace_payload), pav(namespace_validate_unit)],
            ),
            edge(mismatch, Vec::new()),
        ),
    );

    let candidate_kind = assembler.param(ns.p, check_entrypoint, ParameterRole::Block, u64_type());
    let candidate_value = assembler.param(
        ns.p,
        check_entrypoint,
        ParameterRole::Block,
        extended_value_type.clone(),
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
                unwrap_entrypoint,
                vec![pav(candidate_value), pav(candidate_unit)],
            ),
            edge(check_known, vec![pav(candidate_kind)]),
        ),
    );

    let wrapped_entrypoint = assembler.param(
        ns.p,
        unwrap_entrypoint,
        ParameterRole::Block,
        extended_value_type.clone(),
    );
    let wrapped_entrypoint_unit = assembler.param(
        ns.p,
        unwrap_entrypoint,
        ParameterRole::Block,
        TypeExpr::Unit,
    );
    append_block(
        assembler,
        unwrap_entrypoint,
        function,
        vec![wrapped_entrypoint, wrapped_entrypoint_unit],
        Vec::new(),
        switch(
            pav(wrapped_entrypoint),
            vec![
                (
                    BuiltinCase::Ok,
                    select_entrypoint,
                    vec![SwitchArgument::CasePayload, sav(wrapped_entrypoint_unit)],
                ),
                (BuiltinCase::Err, mismatch, Vec::new()),
            ],
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

    let dependency_value = assembler.param(
        ns.p,
        select_dependency,
        ParameterRole::Block,
        extended_value_type,
    );
    let dependency_unit = assembler.param(
        ns.p,
        select_dependency,
        ParameterRole::Block,
        TypeExpr::Unit,
    );
    append_block(
        assembler,
        select_dependency,
        function,
        vec![dependency_value, dependency_unit],
        Vec::new(),
        switch(
            pav(dependency_value),
            vec![
                (BuiltinCase::Ok, mismatch, Vec::new()),
                (
                    BuiltinCase::Err,
                    call_dependency,
                    vec![SwitchArgument::CasePayload, sav(dependency_unit)],
                ),
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
        tagged_entity_set_encode_value_type(),
    );
    let namespace_call_unit =
        assembler.param(ns.p, call_namespace, ParameterRole::Block, TypeExpr::Unit);
    let namespace_kind = assembler.op(
        ns.o,
        call_namespace,
        Opcode::TupleGet,
        vec![pav(namespace_payload)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let namespace_entity = assembler.op(
        ns.o,
        call_namespace,
        Opcode::TupleGet,
        vec![pav(namespace_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let namespace_parent = assembler.op(
        ns.o,
        call_namespace,
        Opcode::TupleGet,
        vec![pav(namespace_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(2),
    );
    let namespace_members = assembler.op(
        ns.o,
        call_namespace,
        Opcode::TupleGet,
        vec![pav(namespace_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(3),
    );
    let namespace_result = assembler.op(
        ns.o,
        call_namespace,
        Opcode::CallDirect,
        vec![
            op_result(namespace_entity),
            op_result(namespace_kind),
            op_result(namespace_parent),
            op_result(namespace_members),
            pav(namespace_call_unit),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: entity_set_encoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        call_namespace,
        function,
        vec![namespace_payload, namespace_call_unit],
        vec![
            namespace_kind,
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

    let dependency_payload = assembler.param(
        ns.p,
        call_dependency,
        ParameterRole::Block,
        dependency_encode_value_type(),
    );
    let dependency_call_unit =
        assembler.param(ns.p, call_dependency, ParameterRole::Block, TypeExpr::Unit);
    let dependency_entity = assembler.op(
        ns.o,
        call_dependency,
        Opcode::TupleGet,
        vec![pav(dependency_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let dependency_root = assembler.op(
        ns.o,
        call_dependency,
        Opcode::TupleGet,
        vec![pav(dependency_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let dependency_package = assembler.op(
        ns.o,
        call_dependency,
        Opcode::TupleGet,
        vec![pav(dependency_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(2),
    );
    let dependency_namespace = assembler.op(
        ns.o,
        call_dependency,
        Opcode::TupleGet,
        vec![pav(dependency_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(3),
    );
    let dependency_result = assembler.op(
        ns.o,
        call_dependency,
        Opcode::CallDirect,
        vec![
            op_result(dependency_entity),
            op_result(dependency_root),
            op_result(dependency_package),
            op_result(dependency_namespace),
            pav(dependency_call_unit),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: dependency_encoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        call_dependency,
        function,
        vec![dependency_payload, dependency_call_unit],
        vec![
            dependency_entity,
            dependency_root,
            dependency_package,
            dependency_namespace,
            dependency_result,
        ],
        ret(op_result(dependency_result)),
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

fn build_dependency_program_encode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    octet_getter: EntityId,
) -> FunctionGraph {
    super::dependency_binding::build_dependency_program_encode_via_get(
        assembler,
        ns,
        function,
        octet_getter,
    )
}

fn build_entity_set_program_encode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    entity_set_encoder: EntityId,
    composer: EntityId,
) -> FunctionGraph {
    build_program_value_encode(
        assembler,
        ns,
        function,
        entity_set_encoder,
        composer,
        &[u64_type(), TypeExpr::Bytes, TypeExpr::Bytes],
    )
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
    let entity_set_ns = Ns {
        k: 31,
        p: 32,
        b: 33,
        o: 34,
    };
    let entity_set_program_ns = Ns {
        k: 35,
        p: 36,
        b: 37,
        o: 38,
    };
    let octet_getter_ns = Ns {
        k: 39,
        p: 40,
        b: 41,
        o: 42,
    };
    let dependency_program_ns = Ns {
        k: 43,
        p: 44,
        b: 45,
        o: 46,
    };
    let composer_ns = Ns {
        k: 47,
        p: 48,
        b: 49,
        o: 50,
    };
    let envelope_ns = Ns {
        k: 51,
        p: 52,
        b: 53,
        o: 54,
    };
    let dispatch_ns = Ns {
        k: 55,
        p: 56,
        b: 57,
        o: 58,
    };
    let entrypoint_body = eid(10, 1);
    let uvar = eid(10, 2);
    let outer = eid(10, 3);
    let build = eid(10, 4);
    let entrypoint_program = eid(10, 5);
    let entity_set_body = eid(10, 6);
    let entity_set_program = eid(10, 7);
    let octet_getter = eid(10, 8);
    let dependency_program = eid(10, 9);
    let composer = eid(10, 10);
    let envelope = eid(10, 11);
    let dispatch = eid(10, 12);

    let entrypoint_body_graph =
        build_entrypoint_encode(&mut assembler, entrypoint_ns, entrypoint_body);
    let uvar_graph = build_encode(&mut assembler, uvar_ns, uvar);
    let outer_graph = build_outer_encode(&mut assembler, outer_ns, outer, uvar);
    let build_graph = build_program_build(&mut assembler, build_ns, build);
    let envelope_graph = build_program_envelope_encode_with_mode(
        &mut assembler,
        envelope_ns,
        envelope,
        build,
        uvar,
        DigestCopyMode::Counted,
    );
    let composer_graph =
        build_program_payload_encode(&mut assembler, composer_ns, composer, outer, envelope);
    let entrypoint_program_graph = build_program_encode(
        &mut assembler,
        entrypoint_program_ns,
        entrypoint_program,
        entrypoint_body,
        composer,
    );
    let entity_set_body_graph = build_entity_set_encode_with_mode(
        &mut assembler,
        entity_set_ns,
        entity_set_body,
        uvar,
        ParentCopyMode::CountedConstant,
        EntitySetBodyKind::Dynamic,
        EntitySetEncodeOutput::Body,
    );
    let entity_set_program_graph = build_entity_set_program_encode(
        &mut assembler,
        entity_set_program_ns,
        entity_set_program,
        entity_set_body,
        composer,
    );
    let octet_getter_graph = super::dependency_binding::build_exact_octet_get(
        &mut assembler,
        octet_getter_ns,
        octet_getter,
    );
    let dependency_program_graph = build_dependency_program_encode(
        &mut assembler,
        dependency_program_ns,
        dependency_program,
        octet_getter,
    );
    let dispatch_graph = build_supported_program_encode(
        &mut assembler,
        dispatch_ns,
        dispatch,
        entrypoint_program,
        entity_set_program,
        dependency_program,
    );

    let mut image = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: dispatch_graph.clone(),
        functions: vec![
            dispatch_graph,
            entrypoint_program_graph,
            entity_set_program_graph,
            dependency_program_graph,
            octet_getter_graph,
            composer_graph,
            envelope_graph,
            entrypoint_body_graph,
            entity_set_body_graph,
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

fn non_dependency_value(arm: ResultConst) -> ConstValue {
    let non_dependency = ConstValue {
        value_type: non_dependency_encode_value_type(),
        data: ConstData::Result(arm),
    };
    ConstValue {
        value_type: all_supported_encode_value_type(),
        data: ConstData::Result(ResultConst::Ok(Box::new(non_dependency))),
    }
}

fn entrypoint_value(entity: u8, function: u8, exposure: u64) -> ConstValue {
    non_dependency_value(ResultConst::Ok(Box::new(tuple_value(
        entrypoint_encode_value_type(),
        vec![
            bytes_input(&[entity; 32]),
            bytes_input(&[function; 32]),
            u64_input(exposure),
        ],
    ))))
}

fn tagged_entity_set_value(kind: u64, entity: &[u8], first: &[u8], second: &[u8]) -> ConstValue {
    non_dependency_value(ResultConst::Err(Box::new(tuple_value(
        tagged_entity_set_encode_value_type(),
        vec![
            u64_input(kind),
            bytes_input(entity),
            bytes_input(first),
            bytes_input(second),
        ],
    ))))
}

fn namespace_value(entity: u8, parent: Option<u8>, members: &[u8]) -> ConstValue {
    let (parent_bytes, member_bytes, _) = ns_semantics(parent, members);
    tagged_entity_set_value(3, &[entity; 32], &parent_bytes, &member_bytes)
}

fn policy_value(entity: &[u8], subject: &[u8], requirements: &[u8]) -> ConstValue {
    tagged_entity_set_value(17, entity, subject, requirements)
}

fn dependency_value(entity: u8, root: &[u8], package: &[u8], namespace: &[u8]) -> ConstValue {
    dependency_value_bytes(&[entity; 32], root, package, namespace)
}

fn dependency_value_bytes(
    entity: &[u8],
    root: &[u8],
    package: &[u8],
    namespace: &[u8],
) -> ConstValue {
    ConstValue {
        value_type: all_supported_encode_value_type(),
        data: ConstData::Result(ResultConst::Err(Box::new(tuple_value(
            dependency_encode_value_type(),
            vec![
                bytes_input(entity),
                bytes_input(root),
                bytes_input(package),
                bytes_input(namespace),
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
fn codec_supported_kind_encode_dispatch_emits_all_four_supported_kinds() {
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

    let namespace = namespace_value(0xc1, Some(0xc2), &[]);
    let namespace_outcome = supported_encode_call(&package, &approved, 3, namespace);
    assert_encode_ok(
        &namespace_outcome,
        &program_ns_stored(0xc1, Some(0xc2), &[]),
    );
    eprintln!(
        "SUPPORTED_ENC kind3 fuel={} instr={} peak={}",
        namespace_outcome.fuel_used,
        namespace_outcome.instruction_count,
        namespace_outcome.peak_value_units
    );

    let policy = policy_value(&[0xe1; 32], &[0xe2; 32], &[]);
    let policy_outcome = supported_encode_call(&package, &approved, 17, policy);
    assert_encode_ok(
        &policy_outcome,
        &super::policy_binding::policy_stored([0xe1; 32], [0xe2; 32], &[]),
    );
    eprintln!(
        "SUPPORTED_ENC kind17 fuel={} instr={} peak={}",
        policy_outcome.fuel_used, policy_outcome.instruction_count, policy_outcome.peak_value_units
    );
    let larger_policy = supported_encode_call(
        &package,
        &approved,
        17,
        policy_value(&[0xe1; 32], &[0xe2; 32], &[0xe3; 32]),
    );
    match &larger_policy.termination {
        sley_vm::ExecutionTermination::ResourceLimit(kind) => assert_eq!(
            *kind,
            sley_vm::ResourceKind::ValueUnits,
            "one-requirement PolicyBinding reaches the established F5 value-unit envelope first",
        ),
        other => {
            panic!("one-requirement PolicyBinding must expose the retained F5 envelope: {other:?}")
        }
    }
    eprintln!(
        "SUPPORTED_ENC_BIND kind17 reqs1 fuel={} instr={} peak={}",
        larger_policy.fuel_used, larger_policy.instruction_count, larger_policy.peak_value_units
    );

    let dependency = dependency_value(0xd1, &[0xd2; 32], &[0xd3; 32], &[0xd4; 32]);
    let dependency_outcome = supported_encode_call(&package, &approved, 18, dependency);
    assert_encode_ok(
        &dependency_outcome,
        &super::dependency_binding::dependency_stored(0xd2, 0xd3, 0xd4),
    );
    eprintln!(
        "SUPPORTED_ENC kind18 fuel={} instr={} peak={}",
        dependency_outcome.fuel_used,
        dependency_outcome.instruction_count,
        dependency_outcome.peak_value_units
    );
}

#[test]
fn codec_supported_kind_dispatch_round_trips_all_four_value_arms() {
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
    assert_eq!(
        decoded_entrypoint.value_type,
        all_supported_encode_value_type()
    );
    assert_encode_ok(
        &supported_encode_call(&encode_package, &encode_approved, 16, decoded_entrypoint),
        &entrypoint,
    );

    let namespace = program_ns_stored(0xc1, Some(0xc2), &[]);
    let decoded_namespace = super::supported_dispatch::supported_decode_ok(
        &super::supported_dispatch::supported_decode_call(
            &decode_package,
            &decode_approved,
            3,
            &namespace,
        ),
    );
    assert_eq!(
        decoded_namespace.value_type,
        all_supported_encode_value_type()
    );
    assert_encode_ok(
        &supported_encode_call(&encode_package, &encode_approved, 3, decoded_namespace),
        &namespace,
    );

    let dependency = super::dependency_binding::dependency_stored(0xd2, 0xd3, 0xd4);
    let decoded_dependency = super::supported_dispatch::supported_decode_ok(
        &super::supported_dispatch::supported_decode_call(
            &decode_package,
            &decode_approved,
            18,
            &dependency,
        ),
    );
    assert_eq!(
        decoded_dependency.value_type,
        all_supported_encode_value_type()
    );
    assert_encode_ok(
        &supported_encode_call(&encode_package, &encode_approved, 18, decoded_dependency),
        &dependency,
    );

    let policy = super::policy_binding::policy_stored([0xe1; 32], [0xe2; 32], &[]);
    let decoded_policy = super::supported_dispatch::supported_decode_ok(
        &super::supported_dispatch::supported_decode_call(
            &decode_package,
            &decode_approved,
            17,
            &policy,
        ),
    );
    assert_eq!(decoded_policy.value_type, all_supported_encode_value_type());
    assert_encode_ok(
        &supported_encode_call(&encode_package, &encode_approved, 17, decoded_policy),
        &policy,
    );
}

#[test]
#[allow(clippy::too_many_lines)]
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
        &supported_encode_call(
            &package,
            &approved,
            3,
            policy_value(&[1; 32], &[2; 32], &[]),
        ),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(&package, &approved, 17, namespace_value(1, None, &[])),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(&package, &approved, 17, entrypoint_value(1, 2, 1)),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(
            &package,
            &approved,
            17,
            dependency_value(1, &[2; 32], &[3; 32], &[4; 32]),
        ),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(&package, &approved, 18, entrypoint_value(1, 2, 1)),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(
            &package,
            &approved,
            16,
            dependency_value(1, &[2; 32], &[3; 32], &[4; 32]),
        ),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(
            &package,
            &approved,
            18,
            dependency_value(1, &[2; 31], &[3; 32], &[4; 32]),
        ),
        "SCB_LENGTH_OVERFLOW",
    );
    for (name, entity, subject, requirements, expected) in [
        (
            "policy_subject_short",
            vec![1; 32],
            vec![2; 31],
            Vec::new(),
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "policy_subject_long",
            vec![1; 32],
            vec![2; 33],
            Vec::new(),
            "SCB_TRAILING_BYTES",
        ),
        (
            "policy_requirement_partial",
            vec![1; 32],
            vec![2; 32],
            vec![3; 33],
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "policy_requirement_duplicate",
            vec![1; 32],
            vec![2; 32],
            [vec![3; 32], vec![3; 32]].concat(),
            "SCB_MAP_DUPLICATE",
        ),
        (
            "policy_requirement_order",
            vec![1; 32],
            vec![2; 32],
            [vec![4; 32], vec![3; 32]].concat(),
            "SCB_MAP_ORDER",
        ),
    ] {
        assert_refusal(
            &supported_encode_call(
                &package,
                &approved,
                17,
                policy_value(&entity, &subject, &requirements),
            ),
            expected,
        );
        eprintln!("SUPPORTED_ENC_REJ {name} -> {expected}");
    }
    for (name, entity, root, external, local, expected) in [
        (
            "package_long",
            vec![1; 32],
            vec![2; 32],
            vec![3; 33],
            vec![4; 32],
            "SCB_TRAILING_BYTES",
        ),
        (
            "local_short",
            vec![1; 32],
            vec![2; 32],
            vec![3; 32],
            vec![4; 31],
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "entity_short",
            vec![1; 31],
            vec![2; 32],
            vec![3; 32],
            vec![4; 32],
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "entity_long",
            vec![1; 33],
            vec![2; 32],
            vec![3; 32],
            vec![4; 32],
            "SCB_TRAILING_BYTES",
        ),
        (
            "root_short_beats_entity_long",
            vec![1; 33],
            vec![2; 31],
            vec![3; 32],
            vec![4; 32],
            "SCB_LENGTH_OVERFLOW",
        ),
    ] {
        assert_refusal(
            &supported_encode_call(
                &package,
                &approved,
                18,
                dependency_value_bytes(&entity, &root, &external, &local),
            ),
            expected,
        );
        eprintln!("SUPPORTED_ENC_REJ {name} -> {expected}");
    }
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
