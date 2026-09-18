//! Compact aggregate coverage for one native canonical profile of every
//! SSMC1 entity kind. The richer per-kind constructions remain the semantic
//! probes; the decode image pins their representative body bytes by raw
//! BLAKE3 so all 18 kinds can share one bounded dispatch graph. The paired
//! encode image selects the matching fixed-length witness by declared kind
//! without paying the decode checker's live-value cost twice.

use super::supported_dispatch::push_preallocated_block as append_block;
use super::*;
use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_RHW1, BRIDGE_CODE_V2B1};

pub(super) fn fixed_profile_bodies() -> Vec<(u64, Vec<u8>)> {
    let workspace = ns_body_of(&super::workspace::workspace_stored(
        [0x11; 32],
        [0x12; 32],
        &[],
        &[],
        &[],
        &[],
    ));
    let package = ns_body_of(&super::package::package_stored(
        [0x21; 32],
        [0x22; 32],
        [0x23; 32],
        &[],
        &[],
    ));
    let namespace = ns_body_of(&super::program_ns_stored(0x31, None, &[]));

    let type_def = vec![4, 15, 4, 1, 1, 0, 2, 3, 1, 1, 0, 3, 1, 0, 4, 1, 1];

    let mut function = vec![5, 57, 8, 1, 1, 0, 2, 1, 0, 3, 2, 1, 0, 4, 1, 0, 5, 32];
    function.extend_from_slice(&[0x52; 32]);
    function.extend_from_slice(&[6, 1, 0, 7, 1, 0, 8, 1, 1]);

    let mut parameter = vec![6, 45, 4, 1, 32];
    parameter.extend_from_slice(&[0x62; 32]);
    parameter.extend_from_slice(&[2, 1, 1, 3, 1, 0, 4, 2, 1, 0]);

    let mut block = vec![7, 56, 5, 1, 32];
    block.extend_from_slice(&[0x72; 32]);
    block.extend_from_slice(&[
        2, 1, 0, 3, 1, 0, 4, 10, 5, 8, 2, 1, 1, 1, 2, 2, 0, 0, 5, 1, 1,
    ]);

    let mut operation = vec![8, 51, 6, 1, 32];
    operation.extend_from_slice(&[0x82; 32]);
    operation.extend_from_slice(&[2, 1, 0, 3, 1, 1, 4, 1, 0, 5, 1, 0, 6, 2, 1, 0]);

    let constant = vec![9, 12, 1, 1, 9, 2, 1, 2, 1, 0, 2, 2, 1, 0];

    let mut global_value = vec![10, 42, 3, 1, 2, 1, 0, 2, 32];
    global_value.extend_from_slice(&[0xa2; 32]);
    global_value.extend_from_slice(&[3, 1, 1]);

    let effect_def = vec![
        11, 23, 6, 1, 1, 8, 2, 2, 1, 0, 3, 2, 1, 0, 4, 2, 1, 0, 5, 2, 1, 0, 6, 1, 1,
    ];

    let mut capability_requirement = vec![12, 41, 3, 1, 32];
    capability_requirement.extend_from_slice(&[0xc2; 32]);
    capability_requirement.extend_from_slice(&[2, 1, 0, 3, 1, 0]);

    let mut contract = vec![13, 75, 4, 1, 32];
    contract.extend_from_slice(&[0xd2; 32]);
    contract.extend_from_slice(&[2, 1, 1, 3, 32]);
    contract.extend_from_slice(&[0xd3; 32]);
    contract.extend_from_slice(&[4, 1, 0]);

    let mut test_case = vec![14, 80, 6, 1, 32];
    test_case.extend_from_slice(&[0xe2; 32]);
    test_case.extend_from_slice(&[
        2, 1, 0, 3, 3, 1, 1, 0, 4, 11, 1, 9, 2, 1, 2, 1, 0, 2, 2, 1, 0, 5, 1, 0, 6, 19, 6, 1, 1, 0,
        2, 1, 0, 3, 1, 0, 4, 1, 0, 5, 1, 0, 6, 1, 0,
    ]);

    let mut adapter_import = vec![15, 53, 6, 1, 32];
    adapter_import.extend_from_slice(&[0xf2; 32]);
    adapter_import.extend_from_slice(&[2, 1, 1, 3, 2, 1, 0, 4, 2, 1, 0, 5, 2, 1, 0, 6, 1, 0]);

    let entry_point = ns_body_of(&super::program_stored(
        0x91,
        0x92,
        sley_mutate::value::EntryExposure::Local,
    ));
    let policy_binding = ns_body_of(&super::policy_binding::policy_stored(
        [0xa1; 32],
        [0xa2; 32],
        &[],
    ));
    let dependency_binding = ns_body_of(&super::dependency_binding::dependency_stored(
        0xb1, 0xb2, 0xb3,
    ));

    vec![
        // The 105-byte DependencyBinding profile is the runtime peak. Keep
        // its selector first so it does not retain a chain of discriminator
        // comparisons before entering the exact digest check.
        (18, dependency_binding),
        (1, workspace),
        (2, package),
        (3, namespace),
        (4, type_def),
        (5, function),
        (6, parameter),
        (7, block),
        (8, operation),
        (9, constant),
        (10, global_value),
        (11, effect_def),
        (12, capability_requirement),
        (13, contract),
        (14, test_case),
        (15, adapter_import),
        (16, entry_point),
        (17, policy_binding),
    ]
}

pub(super) fn stored_from_body(entity: [u8; 32], body: &[u8]) -> Vec<u8> {
    let payload = sley_scb1::encode_record(&[(1, entity.to_vec()), (2, body.to_vec())])
        .expect("aggregate profile outer record is canonical");
    let mut preimage = b"SLEYSCB1".to_vec();
    preimage.extend_from_slice(&sley_scb1::encode_uvar(1));
    preimage.extend_from_slice(&sley_scb1::encode_uvar(200));
    preimage.extend_from_slice(&[9; 32]);
    preimage.extend_from_slice(&sley_scb1::encode_uvar(
        u64::try_from(payload.len()).expect("aggregate payload length fits u64"),
    ));
    preimage.extend_from_slice(&payload);
    let digest = sley_id::ObjectId::derive(&preimage);
    preimage.extend_from_slice(digest.as_bytes());
    preimage
}

#[allow(clippy::too_many_lines)]
fn build_digest_profile_check(assembler: &mut Asm, ns: Ns, function: EntityId) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let kind = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let accepted = assembler.id(ns.b);
    let refused = assembler.id(ns.b);
    let profiles = fixed_profile_bodies();
    let selectors = profiles
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let hash_blocks = profiles
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let comparisons = profiles
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let entry = assembler.id(ns.b);

    for (block, verdict) in [(accepted, true), (refused, false)] {
        let constant = assembler.kbool(ns.k, verdict);
        let value = assembler.cref(ns.o, block, constant, TypeExpr::Bool);
        append_block(
            assembler,
            block,
            function,
            Vec::new(),
            vec![value],
            ret(op_result(value)),
        );
    }

    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        Vec::new(),
        branch(edge(selectors[0], Vec::new())),
    );

    for (index, ((_, profile), block)) in profiles.iter().zip(&selectors).enumerate() {
        let expected_digest = sley_vm::raw_blake3_256(profile)
            .expect("fixed aggregate body is below the raw-hash resource limit");
        let expected_kind = assembler.ku64(ns.k, u128::from(profiles[index].0));
        let expected = assembler.cref(ns.o, *block, expected_kind, u64_type());
        let matches = assembler.op(
            ns.o,
            *block,
            Opcode::Equal,
            vec![pav(kind), op_result(expected)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        let fallback = selectors.get(index + 1).copied().unwrap_or(refused);
        append_block(
            assembler,
            *block,
            function,
            Vec::new(),
            vec![expected, matches],
            cond(
                op_result(matches),
                edge(hash_blocks[index], Vec::new()),
                edge(fallback, Vec::new()),
            ),
        );

        let digest = assembler.op(
            ns.o,
            hash_blocks[index],
            Opcode::AdapterInvoke,
            vec![pav(unit), pav(body)],
            vec![index_result(TypeExpr::Bytes)],
            Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_RHW1))),
        );
        append_block(
            assembler,
            hash_blocks[index],
            function,
            Vec::new(),
            vec![digest],
            switch(
                op_result(digest),
                vec![
                    (
                        BuiltinCase::Ok,
                        comparisons[index],
                        vec![SwitchArgument::CasePayload],
                    ),
                    (BuiltinCase::Err, refused, Vec::new()),
                ],
            ),
        );

        let comparison = comparisons[index];
        let candidate = assembler.param(ns.p, comparison, ParameterRole::Block, TypeExpr::Bytes);
        let expected_constant = assembler.kbytes(ns.k, &expected_digest);
        let expected = assembler.cref(ns.o, comparison, expected_constant, TypeExpr::Bytes);
        let matches = assembler.op(
            ns.o,
            comparison,
            Opcode::Equal,
            vec![pav(candidate), op_result(expected)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        append_block(
            assembler,
            comparison,
            function,
            vec![candidate],
            vec![expected, matches],
            cond(
                op_result(matches),
                edge(accepted, Vec::new()),
                edge(refused, Vec::new()),
            ),
        );
    }

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

#[allow(clippy::too_many_lines)]
fn build_fixed_profile_program_encode_dispatch(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    witnesses: &[(u64, EntityId)],
    dependency_encoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = encode_result_type();
    let kind = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let entity = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let body = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let first = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let second = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let third = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let selectors = witnesses
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let encoders = witnesses
        .iter()
        .map(|_| assembler.id(ns.b))
        .collect::<Vec<_>>();
    let dependency = assembler.id(ns.b);
    let check_dependency = assembler.id(ns.b);
    let unknown = assembler.id(ns.b);
    let entry = assembler.id(ns.b);
    let unknown_code = assembler.kbytes(ns.k, b"SSMC_ENTITY_KIND_UNKNOWN");
    let dependency_constant = assembler.ku64(ns.k, 18);

    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        Vec::new(),
        branch(edge(check_dependency, Vec::new())),
    );

    let dependency_kind = assembler.cref(ns.o, check_dependency, dependency_constant, u64_type());
    let is_dependency = assembler.op(
        ns.o,
        check_dependency,
        Opcode::Equal,
        vec![pav(kind), op_result(dependency_kind)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check_dependency,
        function,
        Vec::new(),
        vec![dependency_kind, is_dependency],
        cond(
            op_result(is_dependency),
            edge(dependency, Vec::new()),
            edge(selectors[0], Vec::new()),
        ),
    );

    let dependency_result = assembler.op(
        ns.o,
        dependency,
        Opcode::CallDirect,
        vec![pav(entity), pav(first), pav(second), pav(third), pav(unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: dependency_encoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        dependency,
        function,
        Vec::new(),
        vec![dependency_result],
        ret(op_result(dependency_result)),
    );

    for (index, ((expected_kind, witness), selector)) in
        witnesses.iter().zip(&selectors).enumerate()
    {
        let expected_constant = assembler.ku64(ns.k, u128::from(*expected_kind));
        let expected = assembler.cref(ns.o, *selector, expected_constant, u64_type());
        let matches = assembler.op(
            ns.o,
            *selector,
            Opcode::Equal,
            vec![pav(kind), op_result(expected)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        let fallback = selectors.get(index + 1).copied().unwrap_or(unknown);
        append_block(
            assembler,
            *selector,
            function,
            Vec::new(),
            vec![expected, matches],
            cond(
                op_result(matches),
                edge(encoders[index], Vec::new()),
                edge(fallback, Vec::new()),
            ),
        );

        let encoded = assembler.op(
            ns.o,
            encoders[index],
            Opcode::CallDirect,
            vec![pav(entity), pav(body), pav(unit)],
            vec![result_type.clone()],
            Immediate::Function(FunctionRefValue {
                function: *witness,
                type_arguments: Vec::new(),
            }),
        );
        append_block(
            assembler,
            encoders[index],
            function,
            Vec::new(),
            vec![encoded],
            ret(op_result(encoded)),
        );
    }

    let code = assembler.cref(ns.o, unknown, unknown_code, TypeExpr::Bytes);
    let error = assembler.op(
        ns.o,
        unknown,
        Opcode::ResultErr,
        vec![op_result(code)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        unknown,
        function,
        Vec::new(),
        vec![code, error],
        ret(op_result(error)),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![kind, entity, body, first, second, third, unit],
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
/// How the dispatcher judges a decoded body for kinds 1 through 17.
#[derive(Clone, Copy)]
enum BodyCheck {
    /// `(kind, body, unit) -> Bool`: the bounded digest-pinned profile; a
    /// mismatch is `SSMC_RESERVED_FIELD_PRESENT`.
    Digest(EntityId),
    /// `(kind, body, unit) -> Result<Unit, Bytes>`: the arbitrary schema
    /// decoders; a refusal forwards the decoder's own code.
    Schema(EntityId),
}

#[allow(clippy::too_many_lines)]
fn build_all_kind_program_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    validate_decoder: EntityId,
    outer_decoder: EntityId,
    body_check: BodyCheck,
    dependency_decoder: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = super::supported_dispatch::supported_program_result_type();
    let declared_kind = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let stored = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let entry = assembler.id(ns.b);
    let envelope_ok = assembler.id(ns.b);
    let check_known = assembler.id(ns.b);
    let call_outer = assembler.id(ns.b);
    let outer_ok = assembler.id(ns.b);
    let return_generic = assembler.id(ns.b);
    let call_dependency = assembler.id(ns.b);
    let forward_error = assembler.id(ns.b);
    let scope_error = assembler.id(ns.b);
    let unknown_error = assembler.id(ns.b);

    let zero_constant = assembler.ku64(ns.k, 0);
    let dependency_constant = assembler.ku64(ns.k, 18);
    let empty_constant = assembler.kbytes(ns.k, b"");
    let scope_code = assembler.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");
    let unknown_code = assembler.kbytes(ns.k, b"SSMC_ENTITY_KIND_UNKNOWN");

    let envelope = assembler.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(stored), pav(unit)],
        vec![encode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: validate_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        entry,
        function,
        Vec::new(),
        vec![envelope],
        switch(
            op_result(envelope),
            vec![
                (
                    BuiltinCase::Ok,
                    envelope_ok,
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

    let payload = assembler.param(ns.p, envelope_ok, ParameterRole::Block, TypeExpr::Bytes);
    let kind = assembler.param(ns.p, envelope_ok, ParameterRole::Block, u64_type());
    let envelope_unit = assembler.param(ns.p, envelope_ok, ParameterRole::Block, TypeExpr::Unit);
    let dependency_kind = assembler.cref(ns.o, envelope_ok, dependency_constant, u64_type());
    let is_dependency = assembler.op(
        ns.o,
        envelope_ok,
        Opcode::Equal,
        vec![pav(kind), op_result(dependency_kind)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        envelope_ok,
        function,
        vec![payload, kind, envelope_unit],
        vec![dependency_kind, is_dependency],
        cond(
            op_result(is_dependency),
            edge(call_dependency, vec![pav(payload), pav(envelope_unit)]),
            edge(
                check_known,
                vec![pav(payload), pav(kind), pav(envelope_unit)],
            ),
        ),
    );

    let known_payload = assembler.param(ns.p, check_known, ParameterRole::Block, TypeExpr::Bytes);
    let known_kind = assembler.param(ns.p, check_known, ParameterRole::Block, u64_type());
    let known_unit = assembler.param(ns.p, check_known, ParameterRole::Block, TypeExpr::Unit);
    let zero = assembler.cref(ns.o, check_known, zero_constant, u64_type());
    let dependency_kind = assembler.cref(ns.o, check_known, dependency_constant, u64_type());
    let above_zero = assembler.op(
        ns.o,
        check_known,
        Opcode::LessThan,
        vec![op_result(zero), pav(known_kind)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let below_dependency = assembler.op(
        ns.o,
        check_known,
        Opcode::LessThan,
        vec![pav(known_kind), op_result(dependency_kind)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let known = assembler.op(
        ns.o,
        check_known,
        Opcode::BoolAnd,
        vec![op_result(above_zero), op_result(below_dependency)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    append_block(
        assembler,
        check_known,
        function,
        vec![known_payload, known_kind, known_unit],
        vec![zero, dependency_kind, above_zero, below_dependency, known],
        cond(
            op_result(known),
            edge(
                call_outer,
                vec![pav(known_payload), pav(known_kind), pav(known_unit)],
            ),
            edge(unknown_error, Vec::new()),
        ),
    );

    let outer_payload = assembler.param(ns.p, call_outer, ParameterRole::Block, TypeExpr::Bytes);
    let outer_kind = assembler.param(ns.p, call_outer, ParameterRole::Block, u64_type());
    let outer_unit = assembler.param(ns.p, call_outer, ParameterRole::Block, TypeExpr::Unit);
    let outer = assembler.op(
        ns.o,
        call_outer,
        Opcode::CallDirect,
        vec![pav(outer_payload), pav(outer_unit)],
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
        vec![outer_payload, outer_kind, outer_unit],
        vec![outer],
        switch(
            op_result(outer),
            vec![
                (
                    BuiltinCase::Ok,
                    outer_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(outer_kind),
                        sav(outer_unit),
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

    let outer_tuple = assembler.param(
        ns.p,
        outer_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes]),
    );
    let checked_kind = assembler.param(ns.p, outer_ok, ParameterRole::Block, u64_type());
    let checked_unit = assembler.param(ns.p, outer_ok, ParameterRole::Block, TypeExpr::Unit);
    let entity = assembler.op(
        ns.o,
        outer_ok,
        Opcode::TupleGet,
        vec![pav(outer_tuple)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let body = assembler.op(
        ns.o,
        outer_ok,
        Opcode::TupleGet,
        vec![pav(outer_tuple)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    match body_check {
        BodyCheck::Digest(body_checker) => {
            let valid = assembler.op(
                ns.o,
                outer_ok,
                Opcode::CallDirect,
                vec![pav(checked_kind), op_result(body), pav(checked_unit)],
                vec![TypeExpr::Bool],
                Immediate::Function(FunctionRefValue {
                    function: body_checker,
                    type_arguments: Vec::new(),
                }),
            );
            append_block(
                assembler,
                outer_ok,
                function,
                vec![outer_tuple, checked_kind, checked_unit],
                vec![entity, body, valid],
                cond(
                    op_result(valid),
                    edge(
                        return_generic,
                        vec![pav(checked_kind), op_result(entity), op_result(body)],
                    ),
                    edge(scope_error, Vec::new()),
                ),
            );
        }
        BodyCheck::Schema(body_checker) => {
            let valid = assembler.op(
                ns.o,
                outer_ok,
                Opcode::CallDirect,
                vec![pav(checked_kind), op_result(body), pav(checked_unit)],
                vec![TypeExpr::Result {
                    ok: Box::new(TypeExpr::Unit),
                    error: Box::new(TypeExpr::Bytes),
                }],
                Immediate::Function(FunctionRefValue {
                    function: body_checker,
                    type_arguments: Vec::new(),
                }),
            );
            append_block(
                assembler,
                outer_ok,
                function,
                vec![outer_tuple, checked_kind, checked_unit],
                vec![entity, body, valid],
                switch(
                    op_result(valid),
                    vec![
                        (
                            BuiltinCase::Ok,
                            return_generic,
                            vec![sav(checked_kind), oav(entity), oav(body)],
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
    }

    let result_kind = assembler.param(ns.p, return_generic, ParameterRole::Block, u64_type());
    let result_entity =
        assembler.param(ns.p, return_generic, ParameterRole::Block, TypeExpr::Bytes);
    let result_body = assembler.param(ns.p, return_generic, ParameterRole::Block, TypeExpr::Bytes);
    let empty = assembler.cref(ns.o, return_generic, empty_constant, TypeExpr::Bytes);
    let zero = assembler.cref(ns.o, return_generic, zero_constant, u64_type());
    let value = assembler.op(
        ns.o,
        return_generic,
        Opcode::TupleNew,
        vec![
            pav(result_kind),
            pav(result_entity),
            pav(result_body),
            op_result(empty),
            op_result(empty),
            op_result(zero),
        ],
        vec![super::supported_dispatch::all_supported_program_value_type()],
        Immediate::None,
    );
    let ok = assembler.op(
        ns.o,
        return_generic,
        Opcode::ResultOk,
        vec![op_result(value)],
        vec![result_type.clone()],
        Immediate::None,
    );
    append_block(
        assembler,
        return_generic,
        function,
        vec![result_kind, result_entity, result_body],
        vec![empty, zero, value, ok],
        ret(op_result(ok)),
    );

    let dependency_payload =
        assembler.param(ns.p, call_dependency, ParameterRole::Block, TypeExpr::Bytes);
    let dependency_unit =
        assembler.param(ns.p, call_dependency, ParameterRole::Block, TypeExpr::Unit);
    let dependency = assembler.op(
        ns.o,
        call_dependency,
        Opcode::CallDirect,
        vec![pav(dependency_payload), pav(dependency_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: dependency_decoder,
            type_arguments: Vec::new(),
        }),
    );
    append_block(
        assembler,
        call_dependency,
        function,
        vec![dependency_payload, dependency_unit],
        vec![dependency],
        ret(op_result(dependency)),
    );

    let forwarded = assembler.param(ns.p, forward_error, ParameterRole::Block, TypeExpr::Bytes);
    let error = assembler.op(
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
        vec![error],
        ret(op_result(error)),
    );

    // The scope refusal exists only under the digest profile; keep the
    // digest image's block order unchanged so its retained evidence holds.
    let mut typed_errors = Vec::new();
    if matches!(body_check, BodyCheck::Digest(_)) {
        typed_errors.push((scope_error, scope_code));
    }
    typed_errors.push((unknown_error, unknown_code));
    for (block, code) in typed_errors {
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

pub(super) fn all_kind_decode_image() -> Image {
    let mut assembler = Asm::new();
    let root = eid(14, 1);
    let validate = eid(14, 2);
    let outer = eid(14, 3);
    let checker = eid(14, 4);
    let uvar = eid(14, 5);
    let dependency = eid(14, 6);
    let (uvar_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 130,
            p: 131,
            b: 132,
            o: 133,
        },
        uvar,
    );
    let validate_graph = build_program_validate(
        &mut assembler,
        Ns {
            k: 134,
            p: 135,
            b: 136,
            o: 137,
        },
        validate,
        uvar,
    );
    let outer_graph = build_outer_decode(
        &mut assembler,
        Ns {
            k: 138,
            p: 139,
            b: 140,
            o: 141,
        },
        outer,
        uvar,
    );
    let checker_graph = build_digest_profile_check(
        &mut assembler,
        Ns {
            k: 142,
            p: 143,
            b: 144,
            o: 145,
        },
        checker,
    );
    let dependency_graph =
        super::dependency_binding_decode::build_dependency_supported_program_decode(
            &mut assembler,
            Ns {
                k: 150,
                p: 151,
                b: 152,
                o: 153,
            },
            dependency,
        );
    let root_graph = build_all_kind_program_decode(
        &mut assembler,
        Ns {
            k: 146,
            p: 147,
            b: 148,
            o: 149,
        },
        root,
        validate,
        outer,
        BodyCheck::Digest(checker),
        dependency,
    );
    let mut image = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: root_graph.clone(),
        functions: vec![
            root_graph,
            dependency_graph,
            checker_graph,
            validate_graph,
            outer_graph,
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
    super::supported_dispatch::deduplicate_identical_constants(&mut image);
    image
}

/// The all-kind decoder with every body judged by its arbitrary schema
/// decoder instead of a pinned digest. Namespaces `130..=153` and the
/// identity namespaces `14`/`15` stay reserved for this image's own
/// functions; the schema closure fills the rest.
pub(super) fn arbitrary_all_kind_decode_image() -> Image {
    let mut assembler = Asm::new();
    let root = eid(14, 1);
    let validate = eid(14, 2);
    let outer = eid(14, 3);
    let checker = eid(14, 4);
    let uvar = eid(14, 5);
    let dependency = eid(14, 6);
    let (uvar_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 130,
            p: 131,
            b: 132,
            o: 133,
        },
        uvar,
    );
    let validate_graph = build_program_validate(
        &mut assembler,
        Ns {
            k: 134,
            p: 135,
            b: 136,
            o: 137,
        },
        validate,
        uvar,
    );
    let outer_graph = build_outer_decode(
        &mut assembler,
        Ns {
            k: 138,
            p: 139,
            b: 140,
            o: 141,
        },
        outer,
        uvar,
    );
    let dependency_graph =
        super::dependency_binding_decode::build_dependency_supported_program_decode(
            &mut assembler,
            Ns {
                k: 150,
                p: 151,
                b: 152,
                o: 153,
            },
            dependency,
        );
    let root_graph = build_all_kind_program_decode(
        &mut assembler,
        Ns {
            k: 146,
            p: 147,
            b: 148,
            o: 149,
        },
        root,
        validate,
        outer,
        BodyCheck::Schema(checker),
        dependency,
    );
    let schema_graphs = super::dependency_binding_decode::build_arbitrary_schema_body_check(
        &mut assembler,
        checker,
        vec![14..=15, 130..=153],
        16,
    );
    let mut functions = vec![
        root_graph.clone(),
        dependency_graph,
        validate_graph,
        outer_graph,
        uvar_graph,
    ];
    functions.extend(schema_graphs);
    let mut image = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: root_graph,
        functions,
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
    super::supported_dispatch::deduplicate_identical_constants(&mut image);
    image
}

#[allow(clippy::too_many_lines)]
pub(super) fn all_kind_encode_image() -> Image {
    let mut assembler = Asm::new();
    let root = eid(15, 1);
    let exact = eid(15, 2);
    let concat = eid(15, 3);
    let octet_getter = eid(15, 4);
    let dependency = eid(15, 5);
    let exact_graph = super::package::build_exact_identity_validate(
        &mut assembler,
        Ns {
            k: 0,
            p: 1,
            b: 2,
            o: 3,
        },
        exact,
    );
    let concat_graph = super::package::build_concat_bytes(
        &mut assembler,
        Ns {
            k: 4,
            p: 5,
            b: 6,
            o: 7,
        },
        concat,
    );
    let octet_getter_graph = super::dependency_binding::build_exact_octet_get(
        &mut assembler,
        Ns {
            k: 8,
            p: 9,
            b: 10,
            o: 11,
        },
        octet_getter,
    );
    let dependency_graph = super::dependency_binding::build_dependency_program_encode_via_get(
        &mut assembler,
        Ns {
            k: 17,
            p: 18,
            b: 19,
            o: 20,
        },
        dependency,
        octet_getter,
    );
    let mut witness_graphs = Vec::new();
    let mut witnesses = Vec::new();
    for (kind, body) in fixed_profile_bodies() {
        if kind == 18 {
            continue;
        }
        let kind_u8 = u8::try_from(kind).expect("entity kind fits u8");
        let witness = eid(15, u16::from(kind_u8) + 10);
        let namespace = 21_u8
            .checked_add(
                kind_u8
                    .checked_sub(1)
                    .expect("entity kinds start at one")
                    .checked_mul(4)
                    .expect("witness namespace fits u8"),
            )
            .expect("witness namespace fits u8");
        let body_length = u64::try_from(body.len()).expect("fixed profile length fits u64");
        let graph = super::package::build_single_fixed_body_witness_program_encode(
            &mut assembler,
            Ns {
                k: namespace,
                p: namespace + 1,
                b: namespace + 2,
                o: namespace + 3,
            },
            witness,
            exact,
            concat,
            body_length + 37,
            body_length,
        );
        witness_graphs.push(graph);
        witnesses.push((kind, witness));
    }
    let root_graph = build_fixed_profile_program_encode_dispatch(
        &mut assembler,
        Ns {
            k: 92,
            p: 93,
            b: 94,
            o: 95,
        },
        root,
        &witnesses,
        dependency,
    );
    let mut functions = vec![root_graph.clone()];
    functions.extend([dependency_graph, octet_getter_graph]);
    functions.extend(witness_graphs);
    functions.extend([exact_graph, concat_graph]);
    let mut image = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: root_graph.clone(),
        functions,
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
    super::supported_dispatch::deduplicate_identical_constants(&mut image);
    image
}

#[test]
fn codec_digest_dispatch_round_trips_one_native_profile_for_all_18_kinds() {
    let (decode_package, decode_approved) = admit(&all_kind_decode_image());
    let (encode_package, encode_approved) = admit(&all_kind_encode_image());
    for (kind, body) in fixed_profile_bodies() {
        let entity_byte = 0x20_u8 + u8::try_from(kind).expect("entity kind fits u8");
        let entity = [entity_byte; 32];
        let stored = stored_from_body(entity, &body);
        sley_mutate::import_entity_object(program_epoch9(), &stored)
            .expect("aggregate profile is accepted by the native codec");

        let decoded = execute(
            &decode_package,
            &decode_approved,
            vec![u64_input(kind), bytes_input(&stored), unit_input()],
        );
        let decoded_value = super::supported_dispatch::supported_decode_ok(&decoded);
        let ConstData::Sequence(fields) = decoded_value.data else {
            panic!("aggregate decoder returns the supported six-field tuple")
        };
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

        let (encoded_body, first, second, third) = if kind == 18 {
            (Vec::new(), vec![0xb1; 32], vec![0xb2; 32], vec![0xb3; 32])
        } else {
            (body.clone(), Vec::new(), Vec::new(), Vec::new())
        };
        let encoded = execute(
            &encode_package,
            &encode_approved,
            vec![
                u64_input(kind),
                bytes_input(&entity),
                bytes_input(&encoded_body),
                bytes_input(&first),
                bytes_input(&second),
                bytes_input(&third),
                unit_input(),
            ],
        );
        assert_encode_ok(&encoded, &stored);
        eprintln!(
            "ALL_KIND kind{kind} stored{}B dec={}/{}/{} enc={}/{}/{}",
            stored.len(),
            decoded.fuel_used,
            decoded.instruction_count,
            decoded.peak_value_units,
            encoded.fuel_used,
            encoded.instruction_count,
            encoded.peak_value_units,
        );
    }
}

#[test]
fn codec_digest_dispatch_refuses_unpinned_decode_and_unknown_encode_kind() {
    let (_, mut body) = fixed_profile_bodies()
        .into_iter()
        .find(|(kind, _)| *kind == 1)
        .expect("Workspace profile exists");
    body.push(0);
    let entity = [0xcc; 32];
    let stored = stored_from_body(entity, &body);

    let (decode_package, decode_approved) = admit(&all_kind_decode_image());
    let decoded = execute(
        &decode_package,
        &decode_approved,
        vec![u64_input(1), bytes_input(&stored), unit_input()],
    );
    assert_refusal(&decoded, "SSMC_RESERVED_FIELD_PRESENT");

    let (encode_package, encode_approved) = admit(&all_kind_encode_image());
    let encoded = execute(
        &encode_package,
        &encode_approved,
        vec![
            u64_input(0),
            bytes_input(&entity),
            bytes_input(&body),
            bytes_input(&[]),
            bytes_input(&[]),
            bytes_input(&[]),
            unit_input(),
        ],
    );
    assert_refusal(&encoded, "SSMC_ENTITY_KIND_UNKNOWN");
}
