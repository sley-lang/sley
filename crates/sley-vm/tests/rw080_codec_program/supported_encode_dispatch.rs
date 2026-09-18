//! Supported-kind encode dispatch kept separate from the large retained
//! per-format construction record in the parent integration test.
//! Construction provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-supported-encode-dispatch.md`.
//! Kind-18 extension provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-dependency-binding-compose-encode.md`.
//! Kind-2 extension provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-package-supported-encode.md`.

use super::supported_dispatch::{
    all_supported_program_value_type as all_supported_encode_value_type,
    deduplicate_identical_constants as deduplicate_constants,
    push_preallocated_block as append_block,
};
use super::*;
use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_RHW1, BRIDGE_CODE_V2B1};

fn block_parameters(
    assembler: &mut Asm,
    namespace: u8,
    block: EntityId,
    types: &[TypeExpr],
) -> Vec<EntityId> {
    types
        .iter()
        .cloned()
        .map(|value_type| assembler.param(namespace, block, ParameterRole::Block, value_type))
        .collect()
}

fn parameter_values(parameters: &[EntityId]) -> Vec<ValueRef> {
    parameters.iter().copied().map(pav).collect()
}

#[allow(clippy::too_many_lines)]
fn build_supported_program_encode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    entrypoint_encoder: EntityId,
    tagged_encoder: EntityId,
    dependency_encoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = encode_result_type();
    let value_type = all_supported_encode_value_type();
    let declared_kind = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let value = assembler.param(ns.p, function, ParameterRole::Function, value_type.clone());
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

    let entry = assembler.id(ns.b);
    let match_kind = assembler.id(ns.b);
    let unpack = assembler.id(ns.b);
    let check_dependency = assembler.id(ns.b);
    let validate_dependency = assembler.id(ns.b);
    let call_dependency = assembler.id(ns.b);
    let check_entrypoint = assembler.id(ns.b);
    let validate_entrypoint = assembler.id(ns.b);
    let call_entrypoint = assembler.id(ns.b);
    let check_tagged = assembler.id(ns.b);
    let validate_tagged = assembler.id(ns.b);
    let call_tagged = assembler.id(ns.b);
    let check_known = assembler.id(ns.b);
    let mismatch = assembler.id(ns.b);
    let unsupported = assembler.id(ns.b);
    let unknown = assembler.id(ns.b);

    let zero = assembler.ku64(ns.k, 0);
    let empty_bytes = assembler.kbytes(ns.k, b"");
    let workspace_kind = assembler.ku64(ns.k, 1);
    let package_kind = assembler.ku64(ns.k, 2);
    let namespace_kind = assembler.ku64(ns.k, 3);
    let entrypoint_kind = assembler.ku64(ns.k, 16);
    let policy_kind = assembler.ku64(ns.k, 17);
    let dependency_kind = assembler.ku64(ns.k, 18);
    let kind_limit = assembler.ku64(ns.k, 19);
    let scope_code = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");
    let unknown_code = assembler.kbytes(ns.k, b"SSMC_ENTITY_KIND_UNKNOWN");

    let zero_value = assembler.cref(ns.o, entry, zero, u64_type());
    let limit_value = assembler.cref(ns.o, entry, kind_limit, u64_type());
    let above_zero = assembler.op(
        ns.o,
        entry,
        Opcode::LessThan,
        vec![op_result(zero_value), pav(declared_kind)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let below_limit = assembler.op(
        ns.o,
        entry,
        Opcode::LessThan,
        vec![pav(declared_kind), op_result(limit_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let known = assembler.op(
        ns.o,
        entry,
        Opcode::BoolAnd,
        vec![op_result(above_zero), op_result(below_limit)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![zero_value, limit_value, above_zero, below_limit, known],
        cond(
            op_result(known),
            edge(match_kind, vec![pav(value), pav(unit)]),
            edge(unknown, Vec::new()),
        ),
    );

    let candidate = assembler.param(
        ns.p,
        match_kind,
        ParameterRole::Block,
        all_supported_encode_value_type(),
    );
    let match_unit = assembler.param(ns.p, match_kind, ParameterRole::Block, TypeExpr::Unit);
    let tuple_kind = assembler.op(
        ns.o,
        match_kind,
        Opcode::TupleGet,
        vec![pav(candidate)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let kind_matches = assembler.op(
        ns.o,
        match_kind,
        Opcode::Equal,
        vec![pav(declared_kind), op_result(tuple_kind)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        match_kind,
        function,
        vec![candidate, match_unit],
        vec![tuple_kind, kind_matches],
        cond(
            op_result(kind_matches),
            edge(
                unpack,
                vec![pav(candidate), pav(match_unit), op_result(tuple_kind)],
            ),
            edge(mismatch, Vec::new()),
        ),
    );

    let packed = assembler.param(ns.p, unpack, ParameterRole::Block, value_type);
    let unpack_unit = assembler.param(ns.p, unpack, ParameterRole::Block, TypeExpr::Unit);
    let kind = assembler.param(ns.p, unpack, ParameterRole::Block, u64_type());
    let entity = assembler.op(
        ns.o,
        unpack,
        Opcode::TupleGet,
        vec![pav(packed)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let first = assembler.op(
        ns.o,
        unpack,
        Opcode::TupleGet,
        vec![pav(packed)],
        vec![TypeExpr::Bytes],
        Immediate::Index(2),
    );
    let second = assembler.op(
        ns.o,
        unpack,
        Opcode::TupleGet,
        vec![pav(packed)],
        vec![TypeExpr::Bytes],
        Immediate::Index(3),
    );
    let third = assembler.op(
        ns.o,
        unpack,
        Opcode::TupleGet,
        vec![pav(packed)],
        vec![TypeExpr::Bytes],
        Immediate::Index(4),
    );
    let scalar = assembler.op(
        ns.o,
        unpack,
        Opcode::TupleGet,
        vec![pav(packed)],
        vec![u64_type()],
        Immediate::Index(5),
    );
    append_block(
        assembler,
        unpack,
        function,
        vec![packed, unpack_unit, kind],
        vec![entity, first, second, third, scalar],
        branch(edge(
            check_dependency,
            vec![
                pav(kind),
                op_result(entity),
                op_result(first),
                op_result(second),
                op_result(third),
                op_result(scalar),
                pav(unpack_unit),
            ],
        )),
    );

    let field_types = vec![
        u64_type(),
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u64_type(),
        TypeExpr::Unit,
    ];
    let dependency_fields = block_parameters(assembler, ns.p, check_dependency, &field_types);
    let dependency_kind_value = assembler.cref(ns.o, check_dependency, dependency_kind, u64_type());
    let is_dependency = assembler.op(
        ns.o,
        check_dependency,
        Opcode::Equal,
        vec![pav(dependency_fields[0]), op_result(dependency_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check_dependency,
        function,
        dependency_fields.clone(),
        vec![dependency_kind_value, is_dependency],
        cond(
            op_result(is_dependency),
            edge(validate_dependency, parameter_values(&dependency_fields)),
            edge(check_entrypoint, parameter_values(&dependency_fields)),
        ),
    );

    let dependency_fields = block_parameters(assembler, ns.p, validate_dependency, &field_types);
    let zero_value = assembler.cref(ns.o, validate_dependency, zero, u64_type());
    let dependency_canonical = assembler.op(
        ns.o,
        validate_dependency,
        Opcode::Equal,
        vec![pav(dependency_fields[5]), op_result(zero_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        validate_dependency,
        function,
        dependency_fields.clone(),
        vec![zero_value, dependency_canonical],
        cond(
            op_result(dependency_canonical),
            edge(call_dependency, parameter_values(&dependency_fields)),
            edge(mismatch, Vec::new()),
        ),
    );

    let dependency_fields = block_parameters(assembler, ns.p, call_dependency, &field_types);
    let dependency_result = assembler.op(
        ns.o,
        call_dependency,
        Opcode::CallDirect,
        vec![
            pav(dependency_fields[1]),
            pav(dependency_fields[2]),
            pav(dependency_fields[3]),
            pav(dependency_fields[4]),
            pav(dependency_fields[6]),
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
        dependency_fields,
        vec![dependency_result],
        ret(op_result(dependency_result)),
    );

    let entrypoint_fields = block_parameters(assembler, ns.p, check_entrypoint, &field_types);
    let entrypoint_kind_value = assembler.cref(ns.o, check_entrypoint, entrypoint_kind, u64_type());
    let is_entrypoint = assembler.op(
        ns.o,
        check_entrypoint,
        Opcode::Equal,
        vec![pav(entrypoint_fields[0]), op_result(entrypoint_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check_entrypoint,
        function,
        entrypoint_fields.clone(),
        vec![entrypoint_kind_value, is_entrypoint],
        cond(
            op_result(is_entrypoint),
            edge(validate_entrypoint, parameter_values(&entrypoint_fields)),
            edge(check_tagged, parameter_values(&entrypoint_fields)),
        ),
    );

    let entrypoint_fields = block_parameters(assembler, ns.p, validate_entrypoint, &field_types);
    let empty = assembler.cref(ns.o, validate_entrypoint, empty_bytes, TypeExpr::Bytes);
    let second_empty = assembler.op(
        ns.o,
        validate_entrypoint,
        Opcode::Equal,
        vec![pav(entrypoint_fields[3]), op_result(empty)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let third_empty = assembler.op(
        ns.o,
        validate_entrypoint,
        Opcode::Equal,
        vec![pav(entrypoint_fields[4]), op_result(empty)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let entrypoint_canonical = assembler.op(
        ns.o,
        validate_entrypoint,
        Opcode::BoolAnd,
        vec![op_result(second_empty), op_result(third_empty)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        validate_entrypoint,
        function,
        entrypoint_fields.clone(),
        vec![empty, second_empty, third_empty, entrypoint_canonical],
        cond(
            op_result(entrypoint_canonical),
            edge(call_entrypoint, parameter_values(&entrypoint_fields)),
            edge(mismatch, Vec::new()),
        ),
    );

    let entrypoint_fields = block_parameters(assembler, ns.p, call_entrypoint, &field_types);
    let entrypoint_result = assembler.op(
        ns.o,
        call_entrypoint,
        Opcode::CallDirect,
        vec![
            pav(entrypoint_fields[1]),
            pav(entrypoint_fields[2]),
            pav(entrypoint_fields[5]),
            pav(entrypoint_fields[6]),
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
        entrypoint_fields,
        vec![entrypoint_result],
        ret(op_result(entrypoint_result)),
    );

    let tagged_fields = block_parameters(assembler, ns.p, check_tagged, &field_types);
    let workspace_kind_value = assembler.cref(ns.o, check_tagged, workspace_kind, u64_type());
    let is_workspace = assembler.op(
        ns.o,
        check_tagged,
        Opcode::Equal,
        vec![pav(tagged_fields[0]), op_result(workspace_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let package_kind_value = assembler.cref(ns.o, check_tagged, package_kind, u64_type());
    let is_package = assembler.op(
        ns.o,
        check_tagged,
        Opcode::Equal,
        vec![pav(tagged_fields[0]), op_result(package_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let namespace_kind_value = assembler.cref(ns.o, check_tagged, namespace_kind, u64_type());
    let is_namespace = assembler.op(
        ns.o,
        check_tagged,
        Opcode::Equal,
        vec![pav(tagged_fields[0]), op_result(namespace_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let policy_kind_value = assembler.cref(ns.o, check_tagged, policy_kind, u64_type());
    let is_policy = assembler.op(
        ns.o,
        check_tagged,
        Opcode::Equal,
        vec![pav(tagged_fields[0]), op_result(policy_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let package_or_namespace = assembler.op(
        ns.o,
        check_tagged,
        Opcode::BoolOr,
        vec![op_result(is_package), op_result(is_namespace)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let fixed_or_namespace = assembler.op(
        ns.o,
        check_tagged,
        Opcode::BoolOr,
        vec![op_result(is_workspace), op_result(package_or_namespace)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let is_tagged = assembler.op(
        ns.o,
        check_tagged,
        Opcode::BoolOr,
        vec![op_result(fixed_or_namespace), op_result(is_policy)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check_tagged,
        function,
        tagged_fields.clone(),
        vec![
            workspace_kind_value,
            is_workspace,
            package_kind_value,
            is_package,
            namespace_kind_value,
            is_namespace,
            policy_kind_value,
            is_policy,
            package_or_namespace,
            fixed_or_namespace,
            is_tagged,
        ],
        cond(
            op_result(is_tagged),
            edge(validate_tagged, parameter_values(&tagged_fields)),
            edge(check_known, vec![pav(tagged_fields[0])]),
        ),
    );

    let tagged_fields = block_parameters(assembler, ns.p, validate_tagged, &field_types);
    let empty = assembler.cref(ns.o, validate_tagged, empty_bytes, TypeExpr::Bytes);
    let zero_value = assembler.cref(ns.o, validate_tagged, zero, u64_type());
    let third_empty = assembler.op(
        ns.o,
        validate_tagged,
        Opcode::Equal,
        vec![pav(tagged_fields[4]), op_result(empty)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let scalar_zero = assembler.op(
        ns.o,
        validate_tagged,
        Opcode::Equal,
        vec![pav(tagged_fields[5]), op_result(zero_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let tagged_canonical = assembler.op(
        ns.o,
        validate_tagged,
        Opcode::BoolAnd,
        vec![op_result(third_empty), op_result(scalar_zero)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        validate_tagged,
        function,
        tagged_fields.clone(),
        vec![
            empty,
            zero_value,
            third_empty,
            scalar_zero,
            tagged_canonical,
        ],
        cond(
            op_result(tagged_canonical),
            edge(call_tagged, parameter_values(&tagged_fields)),
            edge(mismatch, Vec::new()),
        ),
    );

    let tagged_fields = block_parameters(assembler, ns.p, call_tagged, &field_types);
    let tagged_result = assembler.op(
        ns.o,
        call_tagged,
        Opcode::CallDirect,
        vec![
            pav(tagged_fields[1]),
            pav(tagged_fields[0]),
            pav(tagged_fields[2]),
            pav(tagged_fields[3]),
            pav(tagged_fields[6]),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: tagged_encoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        call_tagged,
        function,
        tagged_fields,
        vec![tagged_result],
        ret(op_result(tagged_result)),
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
    let known = assembler.op(
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
        vec![zero_value, limit_value, above_zero, below_limit, known],
        cond(
            op_result(known),
            edge(unsupported, Vec::new()),
            edge(unknown, Vec::new()),
        ),
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

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_tagged_program_encode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    entity_set_encoder: EntityId,
    package_checker: EntityId,
    package_encoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = encode_result_type();
    let entity = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let kind = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let first = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let second = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let entry = assembler.id(ns.b);
    let call_entity_set = assembler.id(ns.b);
    let check_witness = assembler.id(ns.b);
    let check_package = assembler.id(ns.b);
    let call_package = assembler.id(ns.b);
    let mismatch = assembler.id(ns.b);
    let workspace_kind = assembler.ku64(ns.k, 1);
    let package_kind = assembler.ku64(ns.k, 2);
    let empty_bytes = assembler.kbytes(ns.k, b"");
    let scope_code = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");

    let workspace_kind_value = assembler.cref(ns.o, entry, workspace_kind, u64_type());
    let is_workspace = assembler.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(kind), op_result(workspace_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let package_kind_value = assembler.cref(ns.o, entry, package_kind, u64_type());
    let is_package = assembler.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(kind), op_result(package_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let is_fixed_body = assembler.op(
        ns.o,
        entry,
        Opcode::BoolOr,
        vec![op_result(is_workspace), op_result(is_package)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![
            workspace_kind_value,
            is_workspace,
            package_kind_value,
            is_package,
            is_fixed_body,
        ],
        cond(
            op_result(is_fixed_body),
            edge(check_witness, Vec::new()),
            edge(call_entity_set, Vec::new()),
        ),
    );

    let entity_set_result = assembler.op(
        ns.o,
        call_entity_set,
        Opcode::CallDirect,
        vec![pav(entity), pav(kind), pav(first), pav(second), pav(unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: entity_set_encoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        call_entity_set,
        function,
        Vec::new(),
        vec![entity_set_result],
        ret(op_result(entity_set_result)),
    );

    let empty = assembler.cref(ns.o, check_witness, empty_bytes, TypeExpr::Bytes);
    let witness_matches = assembler.op(
        ns.o,
        check_witness,
        Opcode::Equal,
        vec![pav(second), op_result(empty)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check_witness,
        function,
        Vec::new(),
        vec![empty, witness_matches],
        cond(
            op_result(witness_matches),
            edge(check_package, Vec::new()),
            edge(mismatch, Vec::new()),
        ),
    );

    let package_valid = assembler.op(
        ns.o,
        check_package,
        Opcode::CallDirect,
        vec![pav(kind), pav(first), pav(unit)],
        vec![TypeExpr::Bool],
        Immediate::Function(FunctionRefValue {
            function: package_checker,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        check_package,
        function,
        Vec::new(),
        vec![package_valid],
        cond(
            op_result(package_valid),
            edge(call_package, Vec::new()),
            edge(mismatch, Vec::new()),
        ),
    );

    let package_result = assembler.op(
        ns.o,
        call_package,
        Opcode::CallDirect,
        vec![pav(entity), pav(kind), pav(first), pav(unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: package_encoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        call_package,
        function,
        Vec::new(),
        vec![package_result],
        ret(op_result(package_result)),
    );

    let scope = assembler.cref(ns.o, mismatch, scope_code, TypeExpr::Bytes);
    let error = assembler.op(
        ns.o,
        mismatch,
        Opcode::ResultErr,
        vec![op_result(scope)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        mismatch,
        function,
        Vec::new(),
        vec![scope, error],
        ret(op_result(error)),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![entity, kind, first, second, unit],
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
    let package_check_ns = Ns {
        k: 59,
        p: 60,
        b: 61,
        o: 62,
    };
    let tagged_program_ns = Ns {
        k: 63,
        p: 64,
        b: 65,
        o: 66,
    };
    let concat_ns = Ns {
        k: 67,
        p: 68,
        b: 69,
        o: 70,
    };
    let exact_ns = Ns {
        k: 71,
        p: 72,
        b: 73,
        o: 74,
    };
    let package_witness_ns = Ns {
        k: 75,
        p: 76,
        b: 77,
        o: 78,
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
    let package_check = eid(10, 13);
    let tagged_program = eid(10, 14);
    let concat = eid(10, 15);
    let exact = eid(10, 16);
    let package_witness = eid(10, 17);

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
    let package_check_graph =
        super::dependency_binding_decode::build_empty_workspace_package_supported_body_check(
            &mut assembler,
            package_check_ns,
            package_check,
        );
    let concat_graph = super::package::build_concat_bytes(&mut assembler, concat_ns, concat);
    let exact_graph =
        super::package::build_exact_identity_validate(&mut assembler, exact_ns, exact);
    let package_witness_graph = super::package::build_workspace_package_witness_program_encode(
        &mut assembler,
        package_witness_ns,
        package_witness,
        exact,
        concat,
    );
    let tagged_program_graph = build_tagged_program_encode(
        &mut assembler,
        tagged_program_ns,
        tagged_program,
        entity_set_program,
        package_check,
        package_witness,
    );
    let dispatch_graph = build_supported_program_encode(
        &mut assembler,
        dispatch_ns,
        dispatch,
        entrypoint_program,
        tagged_program,
        dependency_program,
    );

    let mut image = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: dispatch_graph.clone(),
        functions: vec![
            dispatch_graph,
            entrypoint_program_graph,
            tagged_program_graph,
            package_witness_graph,
            exact_graph,
            concat_graph,
            entity_set_program_graph,
            dependency_program_graph,
            package_check_graph,
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

fn supported_value(
    kind: u64,
    entity: &[u8],
    first: &[u8],
    second: &[u8],
    third: &[u8],
    scalar: u64,
) -> ConstValue {
    tuple_value(
        all_supported_encode_value_type(),
        vec![
            u64_input(kind),
            bytes_input(entity),
            bytes_input(first),
            bytes_input(second),
            bytes_input(third),
            u64_input(scalar),
        ],
    )
}

fn entrypoint_value(entity: u8, function: u8, exposure: u64) -> ConstValue {
    supported_value(16, &[entity; 32], &[function; 32], b"", b"", exposure)
}

fn tagged_entity_set_value(kind: u64, entity: &[u8], first: &[u8], second: &[u8]) -> ConstValue {
    supported_value(kind, entity, first, second, b"", 0)
}

fn namespace_value(entity: u8, parent: Option<u8>, members: &[u8]) -> ConstValue {
    let (parent_bytes, member_bytes, _) = ns_semantics(parent, members);
    tagged_entity_set_value(3, &[entity; 32], &parent_bytes, &member_bytes)
}

fn policy_value(entity: &[u8], subject: &[u8], requirements: &[u8]) -> ConstValue {
    tagged_entity_set_value(17, entity, subject, requirements)
}

fn package_value(entity: u8, workspace: u8, root_namespace: u8) -> ConstValue {
    let stored = super::package::package_stored(
        [entity; 32],
        [workspace; 32],
        [root_namespace; 32],
        &[],
        &[],
    );
    let body = ns_body_of(&stored);
    tagged_entity_set_value(2, &[entity; 32], &body, b"")
}

fn workspace_value(entity: u8, root_namespace: u8) -> ConstValue {
    let stored =
        super::workspace::workspace_stored([entity; 32], [root_namespace; 32], &[], &[], &[], &[]);
    let body = ns_body_of(&stored);
    tagged_entity_set_value(1, &[entity; 32], &body, b"")
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
    supported_value(18, entity, root, package, namespace, 0)
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
fn codec_supported_kind_encode_dispatch_emits_all_six_supported_kinds() {
    let image = supported_encode_image();
    let (package, approved) = admit(&image);

    let workspace = workspace_value(0x91, 0x92);
    let workspace_outcome = supported_encode_call(&package, &approved, 1, workspace);
    assert_encode_ok(
        &workspace_outcome,
        &super::workspace::workspace_stored([0x91; 32], [0x92; 32], &[], &[], &[], &[]),
    );
    eprintln!(
        "SUPPORTED_ENC kind1 fuel={} instr={} peak={}",
        workspace_outcome.fuel_used,
        workspace_outcome.instruction_count,
        workspace_outcome.peak_value_units
    );

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

    let package_value = package_value(0xf1, 0xf2, 0xf3);
    let package_outcome = supported_encode_call(&package, &approved, 2, package_value);
    eprintln!(
        "SUPPORTED_ENC kind2 fuel={} instr={} peak={}",
        package_outcome.fuel_used,
        package_outcome.instruction_count,
        package_outcome.peak_value_units
    );
    assert_encode_ok(
        &package_outcome,
        &super::package::package_stored([0xf1; 32], [0xf2; 32], [0xf3; 32], &[], &[]),
    );
}

#[test]
fn codec_supported_kind_dispatch_round_trips_all_six_value_arms() {
    let decode_image = super::supported_dispatch::supported_decode_image();
    let (decode_package, decode_approved) = admit(&decode_image);
    let encode_image = supported_encode_image();
    let (encode_package, encode_approved) = admit(&encode_image);

    let workspace = super::workspace::workspace_stored([0x91; 32], [0x92; 32], &[], &[], &[], &[]);
    let decoded_workspace = super::supported_dispatch::supported_decode_ok(
        &super::supported_dispatch::supported_decode_call(
            &decode_package,
            &decode_approved,
            1,
            &workspace,
        ),
    );
    assert_eq!(
        decoded_workspace.value_type,
        all_supported_encode_value_type()
    );
    assert_encode_ok(
        &supported_encode_call(&encode_package, &encode_approved, 1, decoded_workspace),
        &workspace,
    );

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

    let package = super::package::package_stored([0xf1; 32], [0xf2; 32], [0xf3; 32], &[], &[]);
    let package_value = super::supported_dispatch::supported_decode_ok(
        &super::supported_dispatch::supported_decode_call(
            &decode_package,
            &decode_approved,
            2,
            &package,
        ),
    );
    assert_eq!(package_value.value_type, all_supported_encode_value_type());
    assert_encode_ok(
        &supported_encode_call(&encode_package, &encode_approved, 2, package_value),
        &package,
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn codec_supported_kind_encode_dispatch_rejects_mismatched_and_unknown_kinds() {
    let image = supported_encode_image();
    let (package, approved) = admit(&image);

    for (kind, value) in [
        (
            1,
            supported_value(1, &[1; 32], &[2; 49], b"reserved", b"", 0),
        ),
        (
            16,
            supported_value(16, &[1; 32], &[2; 32], b"reserved", b"", 1),
        ),
        (3, supported_value(3, &[1; 32], b"", b"", b"reserved", 0)),
        (17, supported_value(17, &[1; 32], &[2; 32], b"", b"", 1)),
        (
            18,
            supported_value(18, &[1; 32], &[2; 32], &[3; 32], &[4; 32], 1),
        ),
    ] {
        assert_refusal(
            &supported_encode_call(&package, &approved, kind, value),
            "SSMC_RESERVED_FIELD_PRESENT",
        );
    }

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
    assert_refusal(
        &supported_encode_call(&package, &approved, 2, namespace_value(1, None, &[])),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(&package, &approved, 3, package_value(1, 2, 3)),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    let package_stored = super::package::package_stored([1; 32], [2; 32], [3; 32], &[], &[]);
    let package_body = ns_body_of(&package_stored);
    let workspace_stored = super::workspace::workspace_stored([1; 32], [2; 32], &[], &[], &[], &[]);
    let workspace_body = ns_body_of(&workspace_stored);
    assert_refusal(
        &supported_encode_call(
            &package,
            &approved,
            2,
            tagged_entity_set_value(2, &[1; 32], &workspace_body, b""),
        ),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(
            &package,
            &approved,
            1,
            tagged_entity_set_value(1, &[1; 32], &package_body, b""),
        ),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(
            &package,
            &approved,
            2,
            tagged_entity_set_value(2, &[1; 32], &package_body, b"nonempty"),
        ),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    let mut malformed_package_body = package_body;
    malformed_package_body[73] = 1;
    assert_refusal(
        &supported_encode_call(
            &package,
            &approved,
            2,
            tagged_entity_set_value(2, &[1; 32], &malformed_package_body, b""),
        ),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_encode_call(
            &package,
            &approved,
            2,
            tagged_entity_set_value(2, &[1; 31], &ns_body_of(&package_stored), b""),
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
