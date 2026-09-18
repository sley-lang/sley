//! Executable four-leg codec composition for the declared bootstrap profile.
//!
//! The child graphs remain the bounded all-kind program profile and the
//! closed frozen-schema registry. This wrapper supplies the canonical selector
//! dispatch and a uniform typed result without delegating semantic work to the
//! host.

use super::supported_dispatch::{deduplicate_identical_constants, push_preallocated_block};
use super::*;

fn codec_value_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        u8_type(),
        u64_type(),
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u64_type(),
    ])
}

fn codec_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(codec_value_type()),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_selector_block(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    block: EntityId,
    selector: EntityId,
    expected: EntityId,
    selected: EntityId,
    fallback: EntityId,
) {
    let expected_value = assembler.cref(ns.o, block, expected, u8_type());
    let matches = assembler.op(
        ns.o,
        block,
        Opcode::Equal,
        vec![pav(selector), op_result(expected_value)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    push_preallocated_block(
        assembler,
        block,
        function,
        Vec::new(),
        vec![expected_value, matches],
        cond(
            op_result(matches),
            edge(selected, Vec::new()),
            edge(fallback, Vec::new()),
        ),
    );
}

// The function is a linear graph assembler: each section emits one selector,
// call, normalization, or typed-return block in the same order as the four-leg
// contract. Splitting it would obscure the explicit block-edge inventory.
#[allow(clippy::too_many_lines)]
fn build_codec_main(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    program_decode: EntityId,
    program_encode: EntityId,
    schema_decode: EntityId,
    schema_encode: EntityId,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = codec_result_type();
    let selector = assembler.param(ns.p, function, ParameterRole::Function, u8_type());
    let kind = assembler.param(ns.p, function, ParameterRole::Function, u64_type());
    let first = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let second = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let third = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let fourth = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let fifth = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);

    let selector_constants = (0_u128..=3)
        .map(|value| assembler.ku8(ns.k, value))
        .collect::<Vec<_>>();
    let zero = assembler.ku64(ns.k, 0);
    let empty = assembler.kbytes(ns.k, b"");
    let version = assembler.kbytes(ns.k, b"VERSION");

    let entry = assembler.id(ns.b);
    let check_encode_program = assembler.id(ns.b);
    let check_decode_schema = assembler.id(ns.b);
    let check_encode_schema = assembler.id(ns.b);
    let call_decode_program = assembler.id(ns.b);
    let call_encode_program = assembler.id(ns.b);
    let call_decode_schema = assembler.id(ns.b);
    let call_encode_schema = assembler.id(ns.b);
    let decoded_program = assembler.id(ns.b);
    let encoded_program = assembler.id(ns.b);
    let decoded_schema = assembler.id(ns.b);
    let encoded_schema = assembler.id(ns.b);
    let return_value = assembler.id(ns.b);
    let forward_error = assembler.id(ns.b);
    let unknown = assembler.id(ns.b);

    build_selector_block(
        assembler,
        ns,
        function,
        entry,
        selector,
        selector_constants[0],
        call_decode_program,
        check_encode_program,
    );
    build_selector_block(
        assembler,
        ns,
        function,
        check_encode_program,
        selector,
        selector_constants[1],
        call_encode_program,
        check_decode_schema,
    );
    build_selector_block(
        assembler,
        ns,
        function,
        check_decode_schema,
        selector,
        selector_constants[2],
        call_decode_schema,
        check_encode_schema,
    );
    build_selector_block(
        assembler,
        ns,
        function,
        check_encode_schema,
        selector,
        selector_constants[3],
        call_encode_schema,
        unknown,
    );

    let program_decode_result = super::supported_dispatch::supported_program_result_type();
    let decoded = assembler.op(
        ns.o,
        call_decode_program,
        Opcode::CallDirect,
        vec![pav(kind), pav(first), pav(unit)],
        vec![program_decode_result],
        Immediate::Function(FunctionRefValue {
            function: program_decode,
            type_arguments: Vec::new(),
        }),
    );
    push_preallocated_block(
        assembler,
        call_decode_program,
        function,
        Vec::new(),
        vec![decoded],
        switch(
            op_result(decoded),
            vec![
                (
                    BuiltinCase::Ok,
                    decoded_program,
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

    let encoded = assembler.op(
        ns.o,
        call_encode_program,
        Opcode::CallDirect,
        vec![
            pav(kind),
            pav(first),
            pav(second),
            pav(third),
            pav(fourth),
            pav(fifth),
            pav(unit),
        ],
        vec![encode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: program_encode,
            type_arguments: Vec::new(),
        }),
    );
    push_preallocated_block(
        assembler,
        call_encode_program,
        function,
        Vec::new(),
        vec![encoded],
        switch(
            op_result(encoded),
            vec![
                (
                    BuiltinCase::Ok,
                    encoded_program,
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

    let schema_decode_result = TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes])),
        error: Box::new(TypeExpr::Bytes),
    };
    let decoded = assembler.op(
        ns.o,
        call_decode_schema,
        Opcode::CallDirect,
        vec![pav(first), pav(unit)],
        vec![schema_decode_result],
        Immediate::Function(FunctionRefValue {
            function: schema_decode,
            type_arguments: Vec::new(),
        }),
    );
    push_preallocated_block(
        assembler,
        call_decode_schema,
        function,
        Vec::new(),
        vec![decoded],
        switch(
            op_result(decoded),
            vec![
                (
                    BuiltinCase::Ok,
                    decoded_schema,
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

    let encoded = assembler.op(
        ns.o,
        call_encode_schema,
        Opcode::CallDirect,
        vec![pav(first), pav(second), pav(unit)],
        vec![encode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: schema_encode,
            type_arguments: Vec::new(),
        }),
    );
    push_preallocated_block(
        assembler,
        call_encode_schema,
        function,
        Vec::new(),
        vec![encoded],
        switch(
            op_result(encoded),
            vec![
                (
                    BuiltinCase::Ok,
                    encoded_schema,
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

    let decoded_value = assembler.param(
        ns.p,
        decoded_program,
        ParameterRole::Block,
        super::supported_dispatch::all_supported_program_value_type(),
    );
    let mut decoded_fields = Vec::new();
    for (index, value_type) in [
        u64_type(),
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u64_type(),
    ]
    .into_iter()
    .enumerate()
    {
        decoded_fields.push(assembler.op(
            ns.o,
            decoded_program,
            Opcode::TupleGet,
            vec![pav(decoded_value)],
            vec![value_type],
            Immediate::Index(u32::try_from(index).expect("six decoded fields fit u32")),
        ));
    }
    let operation = assembler.cref(ns.o, decoded_program, selector_constants[0], u8_type());
    push_preallocated_block(
        assembler,
        decoded_program,
        function,
        vec![decoded_value],
        decoded_fields
            .iter()
            .copied()
            .chain(std::iter::once(operation))
            .collect(),
        branch(edge(
            return_value,
            vec![
                op_result(operation),
                op_result(decoded_fields[0]),
                op_result(decoded_fields[1]),
                op_result(decoded_fields[2]),
                op_result(decoded_fields[3]),
                op_result(decoded_fields[4]),
                op_result(decoded_fields[5]),
            ],
        )),
    );

    let encoded_bytes =
        assembler.param(ns.p, encoded_program, ParameterRole::Block, TypeExpr::Bytes);
    let operation = assembler.cref(ns.o, encoded_program, selector_constants[1], u8_type());
    let empty_one = assembler.cref(ns.o, encoded_program, empty, TypeExpr::Bytes);
    let empty_two = assembler.cref(ns.o, encoded_program, empty, TypeExpr::Bytes);
    let empty_three = assembler.cref(ns.o, encoded_program, empty, TypeExpr::Bytes);
    let zero_value = assembler.cref(ns.o, encoded_program, zero, u64_type());
    push_preallocated_block(
        assembler,
        encoded_program,
        function,
        vec![encoded_bytes],
        vec![operation, empty_one, empty_two, empty_three, zero_value],
        branch(edge(
            return_value,
            vec![
                op_result(operation),
                pav(kind),
                pav(encoded_bytes),
                op_result(empty_one),
                op_result(empty_two),
                op_result(empty_three),
                op_result(zero_value),
            ],
        )),
    );

    let schema_tuple = assembler.param(
        ns.p,
        decoded_schema,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes]),
    );
    let epoch = assembler.op(
        ns.o,
        decoded_schema,
        Opcode::TupleGet,
        vec![pav(schema_tuple)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let record = assembler.op(
        ns.o,
        decoded_schema,
        Opcode::TupleGet,
        vec![pav(schema_tuple)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let operation = assembler.cref(ns.o, decoded_schema, selector_constants[2], u8_type());
    let zero_kind = assembler.cref(ns.o, decoded_schema, zero, u64_type());
    let empty_one = assembler.cref(ns.o, decoded_schema, empty, TypeExpr::Bytes);
    let empty_two = assembler.cref(ns.o, decoded_schema, empty, TypeExpr::Bytes);
    let zero_count = assembler.cref(ns.o, decoded_schema, zero, u64_type());
    push_preallocated_block(
        assembler,
        decoded_schema,
        function,
        vec![schema_tuple],
        vec![
            epoch, record, operation, zero_kind, empty_one, empty_two, zero_count,
        ],
        branch(edge(
            return_value,
            vec![
                op_result(operation),
                op_result(zero_kind),
                op_result(epoch),
                op_result(record),
                op_result(empty_one),
                op_result(empty_two),
                op_result(zero_count),
            ],
        )),
    );

    let schema_bytes = assembler.param(ns.p, encoded_schema, ParameterRole::Block, TypeExpr::Bytes);
    let operation = assembler.cref(ns.o, encoded_schema, selector_constants[3], u8_type());
    let zero_kind = assembler.cref(ns.o, encoded_schema, zero, u64_type());
    let empty_one = assembler.cref(ns.o, encoded_schema, empty, TypeExpr::Bytes);
    let empty_two = assembler.cref(ns.o, encoded_schema, empty, TypeExpr::Bytes);
    let empty_three = assembler.cref(ns.o, encoded_schema, empty, TypeExpr::Bytes);
    let zero_count = assembler.cref(ns.o, encoded_schema, zero, u64_type());
    push_preallocated_block(
        assembler,
        encoded_schema,
        function,
        vec![schema_bytes],
        vec![
            operation,
            zero_kind,
            empty_one,
            empty_two,
            empty_three,
            zero_count,
        ],
        branch(edge(
            return_value,
            vec![
                op_result(operation),
                op_result(zero_kind),
                pav(schema_bytes),
                op_result(empty_one),
                op_result(empty_two),
                op_result(empty_three),
                op_result(zero_count),
            ],
        )),
    );

    let return_types = [
        u8_type(),
        u64_type(),
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u64_type(),
    ];
    let return_parameters = return_types
        .iter()
        .cloned()
        .map(|value_type| assembler.param(ns.p, return_value, ParameterRole::Block, value_type))
        .collect::<Vec<_>>();
    let value = assembler.op(
        ns.o,
        return_value,
        Opcode::TupleNew,
        return_parameters.iter().copied().map(pav).collect(),
        vec![codec_value_type()],
        Immediate::None,
    );
    let ok = assembler.op(
        ns.o,
        return_value,
        Opcode::ResultOk,
        vec![op_result(value)],
        vec![result_type.clone()],
        Immediate::None,
    );
    push_preallocated_block(
        assembler,
        return_value,
        function,
        return_parameters,
        vec![value, ok],
        ret(op_result(ok)),
    );

    let error_code = assembler.param(ns.p, forward_error, ParameterRole::Block, TypeExpr::Bytes);
    let error = assembler.op(
        ns.o,
        forward_error,
        Opcode::ResultErr,
        vec![pav(error_code)],
        vec![result_type.clone()],
        Immediate::None,
    );
    push_preallocated_block(
        assembler,
        forward_error,
        function,
        vec![error_code],
        vec![error],
        ret(op_result(error)),
    );

    let version_code = assembler.cref(ns.o, unknown, version, TypeExpr::Bytes);
    let error = assembler.op(
        ns.o,
        unknown,
        Opcode::ResultErr,
        vec![op_result(version_code)],
        vec![result_type.clone()],
        Immediate::None,
    );
    push_preallocated_block(
        assembler,
        unknown,
        function,
        Vec::new(),
        vec![version_code, error],
        ret(op_result(error)),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![selector, kind, first, second, third, fourth, fifth, unit],
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

fn codec_main_image() -> Image {
    let decode = super::all_kind_digest_dispatch::all_kind_decode_image();
    let encode = super::all_kind_digest_dispatch::all_kind_encode_image();
    let schema_decode = super::schema_codec::schema_decode_image();
    let schema_encode = super::schema_codec::schema_encode_image();
    let mut assembler = Asm::new();
    let graph = build_codec_main(
        &mut assembler,
        Ns {
            k: 234,
            p: 235,
            b: 236,
            o: 237,
        },
        eid(16, 1),
        decode.entry.entity_id,
        encode.entry.entity_id,
        schema_decode.entry.entity_id,
        schema_encode.entry.entity_id,
    );
    let mut image = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: Vec::new(),
        constants: assembler.constants,
    };
    for child in [decode, encode, schema_decode, schema_encode] {
        image.functions.extend(child.functions);
        image.parameters.extend(child.parameters);
        image.blocks.extend(child.blocks);
        image.operations.extend(child.operations);
        image.constants.extend(child.constants);
        for adapter in child.adapters {
            if image
                .adapters
                .iter()
                .all(|existing| existing.entity_id != adapter.entity_id)
            {
                image.adapters.push(adapter);
            }
        }
    }
    deduplicate_identical_constants(&mut image);
    image
}

fn selector_input(selector: u8) -> ConstValue {
    ConstValue {
        value_type: u8_type(),
        data: ConstData::UInt(u128::from(selector)),
    }
}

fn codec_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    selector: u8,
    kind: u64,
    fields: [&[u8]; 5],
) -> sley_vm::ExecutionOutcome {
    execute_with_limits(
        package,
        approved,
        vec![
            selector_input(selector),
            u64_input(kind),
            bytes_input(fields[0]),
            bytes_input(fields[1]),
            bytes_input(fields[2]),
            bytes_input(fields[3]),
            bytes_input(fields[4]),
            unit_input(),
        ],
        codec_profile_limits(),
    )
}

fn codec_ok(outcome: &sley_vm::ExecutionOutcome) -> &[ConstValue] {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("codec_main must terminate with a typed value: {outcome:?}");
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &value.data else {
        panic!("codec_main must return Ok: {value:?}");
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!("codec_main Ok must carry its normalized tuple: {payload:?}");
    };
    fields
}

#[test]
fn codec_main_executes_program_profile_for_all_18_kinds() {
    let (package, approved) = admit_with_limits(&codec_main_image(), codec_profile_limits());
    for (kind, body) in super::all_kind_digest_dispatch::fixed_profile_bodies() {
        let entity = [0x30 + u8::try_from(kind).expect("entity kind fits u8"); 32];
        let stored = super::all_kind_digest_dispatch::stored_from_body(entity, &body);
        let decoded = codec_call(&package, &approved, 0, kind, [&stored, &[], &[], &[], &[]]);
        let decoded_fields = codec_ok(&decoded);
        assert_eq!(decoded_fields[0].data, ConstData::UInt(0));
        assert_eq!(decoded_fields[1].data, ConstData::UInt(u128::from(kind)));
        assert_eq!(decoded_fields[2].data, ConstData::Bytes(entity.to_vec()));

        let (body_input, first, second, third) = if kind == 18 {
            (Vec::new(), vec![0xb1; 32], vec![0xb2; 32], vec![0xb3; 32])
        } else {
            (body.clone(), Vec::new(), Vec::new(), Vec::new())
        };
        let encoded = codec_call(
            &package,
            &approved,
            1,
            kind,
            [&entity, &body_input, &first, &second, &third],
        );
        let encoded_fields = codec_ok(&encoded);
        assert_eq!(encoded_fields[0].data, ConstData::UInt(1));
        assert_eq!(encoded_fields[1].data, ConstData::UInt(u128::from(kind)));
        assert_eq!(encoded_fields[2].data, ConstData::Bytes(stored));
    }
}

#[test]
fn codec_main_executes_schema_legs_and_forwards_typed_refusals() {
    let record = sley_state_root::conformance_epoch_record()
        .canonical_bytes()
        .expect("conformance record is canonical");
    let preimage = sley_schema::bootstrap_preimage(&record).expect("bootstrap preimage");
    let epoch = sley_state_root::conformance_epoch_id().expect("conformance epoch");
    let (package, approved) = admit_with_limits(&codec_main_image(), codec_profile_limits());

    let decoded = codec_call(&package, &approved, 2, 0, [&preimage, &[], &[], &[], &[]]);
    let fields = codec_ok(&decoded);
    assert_eq!(fields[0].data, ConstData::UInt(2));
    assert_eq!(fields[2].data, ConstData::Bytes(epoch.as_bytes().to_vec()));
    assert_eq!(fields[3].data, ConstData::Bytes(record.clone()));

    let encoded = codec_call(
        &package,
        &approved,
        3,
        0,
        [epoch.as_bytes(), &record, &[], &[], &[]],
    );
    let fields = codec_ok(&encoded);
    assert_eq!(fields[0].data, ConstData::UInt(3));
    assert_eq!(fields[2].data, ConstData::Bytes(preimage));

    let malformed = codec_call(
        &package,
        &approved,
        2,
        0,
        [b"not-a-schema", &[], &[], &[], &[]],
    );
    assert_refusal(&malformed, "SCHEMA_RECORD_INVALID");
    let unknown = codec_call(&package, &approved, 4, 0, [&[], &[], &[], &[], &[]]);
    assert_refusal(&unknown, "VERSION");
}
