//! Supported-kind codec dispatch kept separate from the large retained
//! per-format construction record in the parent integration test.
//! Construction provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-supported-dispatch.md`.

use super::*;
use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_RHW1, BRIDGE_CODE_V2B1};

pub(super) fn entrypoint_program_value_type() -> TypeExpr {
    TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes, u64_type()])
}

pub(super) fn namespace_program_value_type() -> TypeExpr {
    TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes, TypeExpr::Bytes])
}

pub(super) fn supported_program_value_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(entrypoint_program_value_type()),
        error: Box::new(namespace_program_value_type()),
    }
}

pub(super) fn dependency_program_value_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
    ])
}

pub(super) fn extended_supported_program_value_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(supported_program_value_type()),
        error: Box::new(dependency_program_value_type()),
    }
}

pub(super) fn tagged_entity_set_program_value_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        u64_type(),
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
    ])
}

pub(super) fn non_dependency_program_value_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(entrypoint_program_value_type()),
        error: Box::new(tagged_entity_set_program_value_type()),
    }
}

pub(super) fn all_supported_program_value_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(non_dependency_program_value_type()),
        error: Box::new(dependency_program_value_type()),
    }
}

fn supported_program_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(all_supported_program_value_type()),
        error: Box::new(TypeExpr::Bytes),
    }
}

pub(super) fn push_preallocated_block(
    assembler: &mut Asm,
    entity_id: EntityId,
    function: EntityId,
    parameters: Vec<EntityId>,
    operations: Vec<EntityId>,
    terminator: Terminator,
) {
    assembler.blocks.push(Block {
        entity_id,
        function,
        parameters,
        operations,
        terminator,
        reachability: Reachability::Required,
    });
}

/// Independent per-format assemblers intentionally carry their own constants.
/// Once those graphs become one image, identical immutable values can share a
/// definition. This keeps the composed codec inside the frozen value budget
/// without changing any control flow or error value.
pub(super) fn deduplicate_identical_constants(image: &mut Image) -> usize {
    let mut retained = Vec::<ConstantDefinition>::new();
    let mut replacements = std::collections::BTreeMap::<EntityId, EntityId>::new();
    for constant in image.constants.drain(..) {
        if let Some(existing) = retained
            .iter()
            .find(|existing| existing.value == constant.value)
        {
            replacements.insert(constant.entity_id, existing.entity_id);
        } else {
            retained.push(constant);
        }
    }
    for operation in &mut image.operations {
        if let Immediate::Entity(entity) = &mut operation.immediate {
            *entity = replacements.get(entity).copied().unwrap_or(*entity);
        }
    }
    let removed = replacements.len();
    image.constants = retained;
    removed
}

/// Validates one stored object, decodes its outer entity record, and dispatches
/// the body to a supported SSMC1 kind. Each selected decoder verifies the
/// actual body tag, so a false declaration fails closed rather than changing
/// meaning.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_supported_program_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    validate_decoder: EntityId,
    outer_decoder: EntityId,
    entrypoint_decoder: EntityId,
    entity_set_decoder: EntityId,
    dependency_program_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = supported_program_result_type();
    let envelope_result_type = encode_result_type();
    let outer_result_type = outer_decode_result_type();
    let entrypoint_result_type = entrypoint_decode_result_type();
    let namespace_result_type = entity_set_decode_result_type();
    let policy_result_type = entity_set_decode_result_type();
    let declared_kind = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let stored = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

    let entry = assembler.id(ns.b);
    let validated = assembler.id(ns.b);
    let call_outer = assembler.id(ns.b);
    let outer_success = assembler.id(ns.b);
    let check_policy = assembler.id(ns.b);
    let check_entrypoint = assembler.id(ns.b);
    let check_known = assembler.id(ns.b);
    let call_namespace = assembler.id(ns.b);
    let call_policy = assembler.id(ns.b);
    let call_entrypoint = assembler.id(ns.b);
    let call_dependency = assembler.id(ns.b);
    let normalize_namespace = assembler.id(ns.b);
    let normalize_policy = assembler.id(ns.b);
    let normalize_entrypoint = assembler.id(ns.b);
    let forward_error = assembler.id(ns.b);
    let unsupported = assembler.id(ns.b);
    let unknown = assembler.id(ns.b);

    let zero = assembler.ku64(ns.k, 0);
    let namespace_kind = assembler.ku64(ns.k, 3);
    let policy_kind = assembler.ku64(ns.k, 17);
    let entrypoint_kind = assembler.ku64(ns.k, 16);
    let dependency_kind = assembler.ku64(ns.k, 18);
    let kind_limit = assembler.ku64(ns.k, 19);
    let unsupported_code = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");
    let unknown_code = assembler.kbytes(ns.k, b"SSMC_ENTITY_KIND_UNKNOWN");

    let validate_result = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(stored), pav(unit)],
        vec![envelope_result_type],
        Immediate::Function(FunctionRefValue {
            function: validate_decoder,
            type_arguments: Vec::new(),
        }),
    );
    push_preallocated_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![validate_result],
        switch(
            op_result(validate_result),
            vec![
                (
                    BuiltinCase::Ok,
                    validated,
                    vec![SwitchArgument::CasePayload, sav(declared_kind), sav(unit)],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let validated_payload = assembler.param(ns.p, validated, ParameterRole::Block, TypeExpr::Bytes);
    let validated_kind = assembler.param(ns.p, validated, ParameterRole::Block, u64_type());
    let validated_unit = assembler.param(ns.p, validated, ParameterRole::Block, TypeExpr::Unit);
    let dependency_kind_value = assembler.cref(ns.o, validated, dependency_kind, u64_type());
    let is_dependency = assembler.op(
        ns.o,
        validated,
        Opcode::Equal,
        vec![pav(validated_kind), op_result(dependency_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    push_preallocated_block(
        assembler,
        validated,
        function,
        vec![validated_payload, validated_kind, validated_unit],
        vec![dependency_kind_value, is_dependency],
        cond(
            op_result(is_dependency),
            edge(
                call_dependency,
                vec![pav(validated_payload), pav(validated_unit)],
            ),
            edge(
                call_outer,
                vec![
                    pav(validated_payload),
                    pav(validated_kind),
                    pav(validated_unit),
                ],
            ),
        ),
    );

    let outer_input = assembler.param(ns.p, call_outer, ParameterRole::Block, TypeExpr::Bytes);
    let outer_declared_kind = assembler.param(ns.p, call_outer, ParameterRole::Block, u64_type());
    let outer_call_unit = assembler.param(ns.p, call_outer, ParameterRole::Block, TypeExpr::Unit);
    let outer_result = assembler.op(
        ns.o,
        call_outer,
        Opcode::CallDirect,
        vec![pav(outer_input), pav(outer_call_unit)],
        vec![outer_result_type],
        Immediate::Function(FunctionRefValue {
            function: outer_decoder,
            type_arguments: Vec::new(),
        }),
    );
    push_preallocated_block(
        assembler,
        call_outer,
        function,
        vec![outer_input, outer_declared_kind, outer_call_unit],
        vec![outer_result],
        switch(
            op_result(outer_result),
            vec![
                (
                    BuiltinCase::Ok,
                    outer_success,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(outer_declared_kind),
                        sav(outer_call_unit),
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

    let outer_payload = assembler.param(
        ns.p,
        outer_success,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes]),
    );
    let outer_kind = assembler.param(ns.p, outer_success, ParameterRole::Block, u64_type());
    let outer_unit = assembler.param(ns.p, outer_success, ParameterRole::Block, TypeExpr::Unit);
    let outer_entity_id = assembler.op(
        ns.o,
        outer_success,
        Opcode::TupleGet,
        vec![pav(outer_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let outer_body = assembler.op(
        ns.o,
        outer_success,
        Opcode::TupleGet,
        vec![pav(outer_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let namespace_kind_value = assembler.cref(ns.o, outer_success, namespace_kind, u64_type());
    let is_namespace = assembler.op(
        ns.o,
        outer_success,
        Opcode::Equal,
        vec![pav(outer_kind), op_result(namespace_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    push_preallocated_block(
        assembler,
        outer_success,
        function,
        vec![outer_payload, outer_kind, outer_unit],
        vec![
            outer_entity_id,
            outer_body,
            namespace_kind_value,
            is_namespace,
        ],
        cond(
            op_result(is_namespace),
            edge(
                call_namespace,
                vec![
                    op_result(outer_body),
                    op_result(outer_entity_id),
                    pav(outer_unit),
                ],
            ),
            edge(
                check_policy,
                vec![
                    pav(outer_kind),
                    op_result(outer_body),
                    op_result(outer_entity_id),
                    pav(outer_unit),
                ],
            ),
        ),
    );

    let policy_candidate_kind =
        assembler.param(ns.p, check_policy, ParameterRole::Block, u64_type());
    let policy_candidate_body =
        assembler.param(ns.p, check_policy, ParameterRole::Block, TypeExpr::Bytes);
    let policy_candidate_entity_id =
        assembler.param(ns.p, check_policy, ParameterRole::Block, TypeExpr::Bytes);
    let policy_candidate_unit =
        assembler.param(ns.p, check_policy, ParameterRole::Block, TypeExpr::Unit);
    let policy_kind_value = assembler.cref(ns.o, check_policy, policy_kind, u64_type());
    let is_policy = assembler.op(
        ns.o,
        check_policy,
        Opcode::Equal,
        vec![pav(policy_candidate_kind), op_result(policy_kind_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    push_preallocated_block(
        assembler,
        check_policy,
        function,
        vec![
            policy_candidate_kind,
            policy_candidate_body,
            policy_candidate_entity_id,
            policy_candidate_unit,
        ],
        vec![policy_kind_value, is_policy],
        cond(
            op_result(is_policy),
            edge(
                call_policy,
                vec![
                    pav(policy_candidate_body),
                    pav(policy_candidate_entity_id),
                    pav(policy_candidate_unit),
                ],
            ),
            edge(
                check_entrypoint,
                vec![
                    pav(policy_candidate_kind),
                    pav(policy_candidate_body),
                    pav(policy_candidate_entity_id),
                    pav(policy_candidate_unit),
                ],
            ),
        ),
    );

    let candidate_kind = assembler.param(ns.p, check_entrypoint, ParameterRole::Block, u64_type());
    let candidate_body = assembler.param(
        ns.p,
        check_entrypoint,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let candidate_entity_id = assembler.param(
        ns.p,
        check_entrypoint,
        ParameterRole::Block,
        TypeExpr::Bytes,
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
    push_preallocated_block(
        assembler,
        check_entrypoint,
        function,
        vec![
            candidate_kind,
            candidate_body,
            candidate_entity_id,
            candidate_unit,
        ],
        vec![entrypoint_kind_value, is_entrypoint],
        cond(
            op_result(is_entrypoint),
            edge(
                call_entrypoint,
                vec![
                    pav(candidate_body),
                    pav(candidate_entity_id),
                    pav(candidate_unit),
                ],
            ),
            edge(check_known, vec![pav(candidate_kind)]),
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
    let known = assembler.op(
        ns.o,
        check_known,
        Opcode::BoolAnd,
        vec![op_result(above_zero), op_result(below_limit)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    push_preallocated_block(
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

    let namespace_body =
        assembler.param(ns.p, call_namespace, ParameterRole::Block, TypeExpr::Bytes);
    let namespace_entity_id =
        assembler.param(ns.p, call_namespace, ParameterRole::Block, TypeExpr::Bytes);
    let namespace_unit =
        assembler.param(ns.p, call_namespace, ParameterRole::Block, TypeExpr::Unit);
    let namespace_decode_kind = assembler.cref(ns.o, call_namespace, namespace_kind, u64_type());
    let namespace_result = assembler.op(
        ns.o,
        call_namespace,
        Opcode::CallDirect,
        vec![
            pav(namespace_body),
            op_result(namespace_decode_kind),
            pav(namespace_unit),
        ],
        vec![namespace_result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: entity_set_decoder,
            type_arguments: Vec::new(),
        }),
    );
    push_preallocated_block(
        assembler,
        call_namespace,
        function,
        vec![namespace_body, namespace_entity_id, namespace_unit],
        vec![namespace_decode_kind, namespace_result],
        switch(
            op_result(namespace_result),
            vec![
                (
                    BuiltinCase::Ok,
                    normalize_namespace,
                    vec![SwitchArgument::CasePayload, sav(namespace_entity_id)],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let policy_body = assembler.param(ns.p, call_policy, ParameterRole::Block, TypeExpr::Bytes);
    let policy_entity_id =
        assembler.param(ns.p, call_policy, ParameterRole::Block, TypeExpr::Bytes);
    let policy_unit = assembler.param(ns.p, call_policy, ParameterRole::Block, TypeExpr::Unit);
    let policy_decode_kind = assembler.cref(ns.o, call_policy, policy_kind, u64_type());
    let policy_result = assembler.op(
        ns.o,
        call_policy,
        Opcode::CallDirect,
        vec![
            pav(policy_body),
            op_result(policy_decode_kind),
            pav(policy_unit),
        ],
        vec![policy_result_type],
        Immediate::Function(FunctionRefValue {
            function: entity_set_decoder,
            type_arguments: Vec::new(),
        }),
    );
    push_preallocated_block(
        assembler,
        call_policy,
        function,
        vec![policy_body, policy_entity_id, policy_unit],
        vec![policy_decode_kind, policy_result],
        switch(
            op_result(policy_result),
            vec![
                (
                    BuiltinCase::Ok,
                    normalize_policy,
                    vec![SwitchArgument::CasePayload, sav(policy_entity_id)],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let entrypoint_body =
        assembler.param(ns.p, call_entrypoint, ParameterRole::Block, TypeExpr::Bytes);
    let entrypoint_entity_id =
        assembler.param(ns.p, call_entrypoint, ParameterRole::Block, TypeExpr::Bytes);
    let entrypoint_unit =
        assembler.param(ns.p, call_entrypoint, ParameterRole::Block, TypeExpr::Unit);
    let entrypoint_result = assembler.op(
        ns.o,
        call_entrypoint,
        Opcode::CallDirect,
        vec![pav(entrypoint_body), pav(entrypoint_unit)],
        vec![entrypoint_result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: entrypoint_decoder,
            type_arguments: Vec::new(),
        }),
    );
    push_preallocated_block(
        assembler,
        call_entrypoint,
        function,
        vec![entrypoint_body, entrypoint_entity_id, entrypoint_unit],
        vec![entrypoint_result],
        switch(
            op_result(entrypoint_result),
            vec![
                (
                    BuiltinCase::Ok,
                    normalize_entrypoint,
                    vec![SwitchArgument::CasePayload, sav(entrypoint_entity_id)],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let dependency_payload =
        assembler.param(ns.p, call_dependency, ParameterRole::Block, TypeExpr::Bytes);
    let dependency_unit =
        assembler.param(ns.p, call_dependency, ParameterRole::Block, TypeExpr::Unit);
    let dependency_result = assembler.op(
        ns.o,
        call_dependency,
        Opcode::CallDirect,
        vec![pav(dependency_payload), pav(dependency_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: dependency_program_decoder,
            type_arguments: Vec::new(),
        }),
    );
    push_preallocated_block(
        assembler,
        call_dependency,
        function,
        vec![dependency_payload, dependency_unit],
        vec![dependency_result],
        ret(op_result(dependency_result)),
    );

    let namespace_payload = assembler.param(
        ns.p,
        normalize_namespace,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes, u64_type()]),
    );
    let namespace_id = assembler.param(
        ns.p,
        normalize_namespace,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let namespace_parent = assembler.op(
        ns.o,
        normalize_namespace,
        Opcode::TupleGet,
        vec![pav(namespace_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let namespace_members = assembler.op(
        ns.o,
        normalize_namespace,
        Opcode::TupleGet,
        vec![pav(namespace_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let namespace_kind_value =
        assembler.cref(ns.o, normalize_namespace, namespace_kind, u64_type());
    let namespace_tuple = assembler.op(
        ns.o,
        normalize_namespace,
        Opcode::TupleNew,
        vec![
            op_result(namespace_kind_value),
            pav(namespace_id),
            op_result(namespace_parent),
            op_result(namespace_members),
        ],
        vec![tagged_entity_set_program_value_type()],
        Immediate::None,
    );
    let namespace_arm = assembler.op(
        ns.o,
        normalize_namespace,
        Opcode::ResultErr,
        vec![op_result(namespace_tuple)],
        vec![non_dependency_program_value_type()],
        Immediate::None,
    );
    let namespace_all = assembler.op(
        ns.o,
        normalize_namespace,
        Opcode::ResultOk,
        vec![op_result(namespace_arm)],
        vec![all_supported_program_value_type()],
        Immediate::None,
    );
    let namespace_ok = assembler.op(
        ns.o,
        normalize_namespace,
        Opcode::ResultOk,
        vec![op_result(namespace_all)],
        vec![result_type.clone()],
        Immediate::None,
    );
    push_preallocated_block(
        assembler,
        normalize_namespace,
        function,
        vec![namespace_payload, namespace_id],
        vec![
            namespace_parent,
            namespace_members,
            namespace_kind_value,
            namespace_tuple,
            namespace_arm,
            namespace_all,
            namespace_ok,
        ],
        ret(op_result(namespace_ok)),
    );

    let policy_payload = assembler.param(
        ns.p,
        normalize_policy,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes, u64_type()]),
    );
    let policy_id = assembler.param(
        ns.p,
        normalize_policy,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let policy_subject = assembler.op(
        ns.o,
        normalize_policy,
        Opcode::TupleGet,
        vec![pav(policy_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let policy_requirements = assembler.op(
        ns.o,
        normalize_policy,
        Opcode::TupleGet,
        vec![pav(policy_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let policy_kind_value = assembler.cref(ns.o, normalize_policy, policy_kind, u64_type());
    let policy_tuple = assembler.op(
        ns.o,
        normalize_policy,
        Opcode::TupleNew,
        vec![
            op_result(policy_kind_value),
            pav(policy_id),
            op_result(policy_subject),
            op_result(policy_requirements),
        ],
        vec![tagged_entity_set_program_value_type()],
        Immediate::None,
    );
    let policy_arm = assembler.op(
        ns.o,
        normalize_policy,
        Opcode::ResultErr,
        vec![op_result(policy_tuple)],
        vec![non_dependency_program_value_type()],
        Immediate::None,
    );
    let policy_all = assembler.op(
        ns.o,
        normalize_policy,
        Opcode::ResultOk,
        vec![op_result(policy_arm)],
        vec![all_supported_program_value_type()],
        Immediate::None,
    );
    let policy_ok = assembler.op(
        ns.o,
        normalize_policy,
        Opcode::ResultOk,
        vec![op_result(policy_all)],
        vec![result_type.clone()],
        Immediate::None,
    );
    push_preallocated_block(
        assembler,
        normalize_policy,
        function,
        vec![policy_payload, policy_id],
        vec![
            policy_subject,
            policy_requirements,
            policy_kind_value,
            policy_tuple,
            policy_arm,
            policy_all,
            policy_ok,
        ],
        ret(op_result(policy_ok)),
    );

    let entrypoint_payload = assembler.param(
        ns.p,
        normalize_entrypoint,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, u64_type()]),
    );
    let entrypoint_id = assembler.param(
        ns.p,
        normalize_entrypoint,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let decoded_function = assembler.op(
        ns.o,
        normalize_entrypoint,
        Opcode::TupleGet,
        vec![pav(entrypoint_payload)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let entrypoint_exposure = assembler.op(
        ns.o,
        normalize_entrypoint,
        Opcode::TupleGet,
        vec![pav(entrypoint_payload)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let entrypoint_tuple = assembler.op(
        ns.o,
        normalize_entrypoint,
        Opcode::TupleNew,
        vec![
            pav(entrypoint_id),
            op_result(decoded_function),
            op_result(entrypoint_exposure),
        ],
        vec![entrypoint_program_value_type()],
        Immediate::None,
    );
    let entrypoint_arm = assembler.op(
        ns.o,
        normalize_entrypoint,
        Opcode::ResultOk,
        vec![op_result(entrypoint_tuple)],
        vec![non_dependency_program_value_type()],
        Immediate::None,
    );
    let entrypoint_all = assembler.op(
        ns.o,
        normalize_entrypoint,
        Opcode::ResultOk,
        vec![op_result(entrypoint_arm)],
        vec![all_supported_program_value_type()],
        Immediate::None,
    );
    let entrypoint_ok = assembler.op(
        ns.o,
        normalize_entrypoint,
        Opcode::ResultOk,
        vec![op_result(entrypoint_all)],
        vec![result_type.clone()],
        Immediate::None,
    );
    push_preallocated_block(
        assembler,
        normalize_entrypoint,
        function,
        vec![entrypoint_payload, entrypoint_id],
        vec![
            decoded_function,
            entrypoint_exposure,
            entrypoint_tuple,
            entrypoint_arm,
            entrypoint_all,
            entrypoint_ok,
        ],
        ret(op_result(entrypoint_ok)),
    );

    let forwarded = assembler.param(ns.p, forward_error, ParameterRole::Block, TypeExpr::Bytes);
    let forwarded_error = assembler.op(
        ns.o,
        forward_error,
        Opcode::ResultErr,
        vec![pav(forwarded)],
        vec![result_type.clone()],
        Immediate::None,
    );
    push_preallocated_block(
        assembler,
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_error],
        ret(op_result(forwarded_error)),
    );

    for (block, code) in [(unsupported, unsupported_code), (unknown, unknown_code)] {
        let code_value = assembler.cref(ns.o, block, code, TypeExpr::Bytes);
        let error = assembler.op(
            ns.o,
            block,
            Opcode::ResultErr,
            vec![op_result(code_value)],
            vec![result_type.clone()],
            Immediate::None,
        );
        push_preallocated_block(
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
        parameters: vec![declared_kind, stored, unit],
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
pub(super) fn supported_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_ns = Ns {
        k: 131,
        p: 132,
        b: 133,
        o: 134,
    };
    let validate_ns = Ns {
        k: 135,
        p: 136,
        b: 137,
        o: 138,
    };
    let outer_ns = Ns {
        k: 139,
        p: 140,
        b: 141,
        o: 142,
    };
    let entrypoint_ns = Ns {
        k: 143,
        p: 144,
        b: 145,
        o: 146,
    };
    let namespace_ns = Ns {
        k: 147,
        p: 148,
        b: 149,
        o: 150,
    };
    let dependency_program_ns = Ns {
        k: 155,
        p: 156,
        b: 157,
        o: 158,
    };
    let dispatch_ns = Ns {
        k: 151,
        p: 152,
        b: 153,
        o: 154,
    };
    let decode_function = eid(9, 45);
    let validate_function = eid(9, 46);
    let outer_function = eid(9, 47);
    let entrypoint_function = eid(9, 48);
    let namespace_function = eid(9, 49);
    let function = eid(9, 50);
    let dependency_program_function = eid(9, 51);
    let (decode_graph, _) = build_decode(&mut assembler, decode_ns, decode_function);
    let validate_graph = build_program_validate(
        &mut assembler,
        validate_ns,
        validate_function,
        decode_function,
    );
    let outer_graph = build_outer_decode(&mut assembler, outer_ns, outer_function, decode_function);
    let entrypoint_graph = build_entrypoint_decode(
        &mut assembler,
        entrypoint_ns,
        entrypoint_function,
        decode_function,
    );
    let namespace_graph = build_combined_entity_set_decode(
        &mut assembler,
        namespace_ns,
        namespace_function,
        decode_function,
    );
    let dependency_program_graph =
        super::dependency_binding_decode::build_dependency_supported_program_decode(
            &mut assembler,
            dependency_program_ns,
            dependency_program_function,
            &all_supported_program_value_type(),
            supported_program_result_type(),
        );
    let graph = build_supported_program_decode(
        &mut assembler,
        dispatch_ns,
        function,
        validate_function,
        outer_function,
        entrypoint_function,
        namespace_function,
        dependency_program_function,
    );
    let mut image = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            validate_graph,
            outer_graph,
            entrypoint_graph,
            namespace_graph,
            dependency_program_graph,
            decode_graph,
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
        deduplicate_identical_constants(&mut image) > 0,
        "composed format graphs share canonical constants"
    );
    image
}

pub(super) fn supported_decode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    kind: u64,
    stored: &[u8],
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![u64_input(kind), bytes_input(stored), unit_input()],
    )
}

pub(super) fn supported_decode_ok(outcome: &sley_vm::ExecutionOutcome) -> ConstValue {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "supported decoder must return a value, got {:?}; peak {}, fuel {}, instructions {}",
            outcome.termination,
            outcome.peak_value_units,
            outcome.fuel_used,
            outcome.instruction_count,
        )
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &value.data else {
        panic!("supported decoder must return Ok, got {:?}", value.data)
    };
    payload.as_ref().clone()
}

#[test]
fn codec_supported_kind_dispatch_decodes_all_four_supported_kinds() {
    let image = supported_decode_image();
    let (package, approved) = admit(&image);

    let entrypoint = program_stored(0xa1, 0xb2, sley_mutate::value::EntryExposure::Protocol);
    let entrypoint_outcome = supported_decode_call(&package, &approved, 16, &entrypoint);
    let entrypoint_value = supported_decode_ok(&entrypoint_outcome);
    eprintln!(
        "SUPPORTED_DEC kind16 stored{}B fuel={} instr={} peak={}",
        entrypoint.len(),
        entrypoint_outcome.fuel_used,
        entrypoint_outcome.instruction_count,
        entrypoint_outcome.peak_value_units
    );
    let ConstData::Result(ResultConst::Ok(non_dependency_entrypoint)) = entrypoint_value.data
    else {
        panic!("entrypoint must use the all-supported Ok arm")
    };
    let ConstData::Result(ResultConst::Ok(entrypoint_fields)) = non_dependency_entrypoint.data
    else {
        panic!("entrypoint must use the non-dependency Ok arm")
    };
    let ConstData::Sequence(fields) = entrypoint_fields.data else {
        panic!("entrypoint arm must carry a tuple")
    };
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].data, ConstData::Bytes(vec![0xa1; 32]));
    assert_eq!(fields[1].data, ConstData::Bytes(vec![0xb2; 32]));
    assert_eq!(fields[2].data, ConstData::UInt(2));

    // The existing Namespace program records the admitted resource envelope:
    // a parent-only object remains below the protected one-million-unit cap.
    let namespace = program_ns_stored(0xc1, Some(0xc2), &[]);
    let (parent, members, _) = ns_semantics(Some(0xc2), &[]);
    let namespace_outcome = supported_decode_call(&package, &approved, 3, &namespace);
    let namespace_value = supported_decode_ok(&namespace_outcome);
    eprintln!(
        "SUPPORTED_DEC kind3 stored{}B fuel={} instr={} peak={}",
        namespace.len(),
        namespace_outcome.fuel_used,
        namespace_outcome.instruction_count,
        namespace_outcome.peak_value_units
    );
    let ConstData::Result(ResultConst::Ok(non_dependency_namespace)) = namespace_value.data else {
        panic!("namespace must use the all-supported Ok arm")
    };
    let ConstData::Result(ResultConst::Err(namespace_fields)) = non_dependency_namespace.data
    else {
        panic!("namespace must use the tagged entity-set arm")
    };
    let ConstData::Sequence(fields) = namespace_fields.data else {
        panic!("namespace arm must carry a tuple")
    };
    assert_eq!(fields.len(), 4);
    assert_eq!(fields[0].data, ConstData::UInt(3));
    assert_eq!(fields[1].data, ConstData::Bytes(vec![0xc1; 32]));
    assert_eq!(fields[2].data, ConstData::Bytes(parent));
    assert_eq!(fields[3].data, ConstData::Bytes(members));

    let dependency = super::dependency_binding::dependency_stored(0xd2, 0xd3, 0xd4);
    let dependency_outcome = supported_decode_call(&package, &approved, 18, &dependency);
    let dependency_value = supported_decode_ok(&dependency_outcome);
    eprintln!(
        "SUPPORTED_DEC kind18 stored{}B fuel={} instr={} peak={}",
        dependency.len(),
        dependency_outcome.fuel_used,
        dependency_outcome.instruction_count,
        dependency_outcome.peak_value_units
    );
    let ConstData::Result(ResultConst::Err(dependency_fields)) = dependency_value.data else {
        panic!("DependencyBinding must retain the all-supported Err arm")
    };
    let ConstData::Sequence(fields) = dependency_fields.data else {
        panic!("DependencyBinding arm must carry a tuple")
    };
    assert_eq!(fields.len(), 4);
    assert_eq!(fields[0].data, ConstData::Bytes(vec![0xd1; 32]));
    assert_eq!(fields[1].data, ConstData::Bytes(vec![0xd2; 32]));
    assert_eq!(fields[2].data, ConstData::Bytes(vec![0xd3; 32]));
    assert_eq!(fields[3].data, ConstData::Bytes(vec![0xd4; 32]));

    let policy = super::policy_binding::policy_stored([0xe1; 32], [0xe2; 32], &[[0xe3; 32]]);
    let policy_outcome = supported_decode_call(&package, &approved, 17, &policy);
    let policy_value = supported_decode_ok(&policy_outcome);
    eprintln!(
        "SUPPORTED_DEC kind17 stored{}B fuel={} instr={} peak={}",
        policy.len(),
        policy_outcome.fuel_used,
        policy_outcome.instruction_count,
        policy_outcome.peak_value_units
    );
    let ConstData::Result(ResultConst::Ok(non_dependency_policy)) = policy_value.data else {
        panic!("PolicyBinding must use the all-supported Ok arm")
    };
    let ConstData::Result(ResultConst::Err(policy_fields)) = non_dependency_policy.data else {
        panic!("PolicyBinding must use the tagged entity-set arm")
    };
    let ConstData::Sequence(fields) = policy_fields.data else {
        panic!("PolicyBinding arm must carry a tuple")
    };
    assert_eq!(fields.len(), 4);
    assert_eq!(fields[0].data, ConstData::UInt(17));
    assert_eq!(fields[1].data, ConstData::Bytes(vec![0xe1; 32]));
    assert_eq!(fields[2].data, ConstData::Bytes(vec![0xe2; 32]));
    assert_eq!(fields[3].data, ConstData::Bytes(vec![0xe3; 32]));
}

#[test]
fn codec_supported_kind_dispatch_fails_closed_on_mismatch_and_unknown_kind() {
    let image = supported_decode_image();
    let (package, approved) = admit(&image);
    let entrypoint = program_stored(0xa1, 0xb2, sley_mutate::value::EntryExposure::Local);

    assert_refusal(
        &supported_decode_call(&package, &approved, 3, &entrypoint),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_decode_call(&package, &approved, 18, &entrypoint),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_decode_call(&package, &approved, 17, &entrypoint),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_decode_call(&package, &approved, 1, &entrypoint),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_decode_call(&package, &approved, 0, &entrypoint),
        "SSMC_ENTITY_KIND_UNKNOWN",
    );
    assert_refusal(
        &supported_decode_call(&package, &approved, 19, &entrypoint),
        "SSMC_ENTITY_KIND_UNKNOWN",
    );

    let mut corrupted = entrypoint;
    *corrupted.last_mut().unwrap() ^= 1;
    assert_refusal(
        &supported_decode_call(&package, &approved, 16, &corrupted),
        "SCB_DIGEST_MISMATCH",
    );
    assert_refusal(
        &supported_decode_call(&package, &approved, 19, &corrupted),
        "SCB_DIGEST_MISMATCH",
    );

    let dependency = super::dependency_binding::dependency_stored(0xd2, 0xd3, 0xd4);
    let mut corrupted_dependency = dependency.clone();
    *corrupted_dependency.last_mut().unwrap() ^= 1;
    assert_refusal(
        &supported_decode_call(&package, &approved, 18, &corrupted_dependency),
        "SCB_DIGEST_MISMATCH",
    );

    let policy = super::policy_binding::policy_stored([0xe1; 32], [0xe2; 32], &[]);
    assert_refusal(
        &supported_decode_call(&package, &approved, 16, &policy),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    assert_refusal(
        &supported_decode_call(&package, &approved, 18, &policy),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
    let mut corrupted_policy = policy;
    *corrupted_policy.last_mut().unwrap() ^= 1;
    assert_refusal(
        &supported_decode_call(&package, &approved, 17, &corrupted_policy),
        "SCB_DIGEST_MISMATCH",
    );

    let mut noncanonical_dependency_body = ns_body_of(&dependency);
    noncanonical_dependency_body[2] = 4;
    let noncanonical_dependency = ns_wrap_body(0xd1, &noncanonical_dependency_body);
    assert_refusal(
        &supported_decode_call(&package, &approved, 18, &noncanonical_dependency),
        "SSMC_RESERVED_FIELD_PRESENT",
    );
}
