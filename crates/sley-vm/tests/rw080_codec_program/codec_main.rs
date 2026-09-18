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

pub(super) fn codec_main_image() -> Image {
    let decode = super::all_kind_digest_dispatch::all_kind_decode_image();
    let encode = super::all_kind_digest_dispatch::all_kind_encode_image();
    let decode_entry = decode.entry.entity_id;
    let encode_entry = encode.entry.entity_id;
    compose_codec_main(vec![decode, encode], decode_entry, encode_entry)
}

/// The four-leg composition with selectors 0 and 1 routed to the arbitrary
/// program legs (`arbitrary_program_legs_image`). The bounded
/// `codec_main_image` and the canonical codec component derived from it are
/// unchanged; this is the candidate for the next component derivation.
pub(super) fn arbitrary_codec_main_image() -> Image {
    let legs = super::all_kind_digest_dispatch::arbitrary_program_legs_image();
    compose_codec_main(vec![legs.image], legs.decode_entry, legs.encode_entry)
}

fn compose_codec_main(
    program_children: Vec<Image>,
    decode_entry: EntityId,
    encode_entry: EntityId,
) -> Image {
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
        decode_entry,
        encode_entry,
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
    let mut children = program_children;
    children.push(schema_decode);
    children.push(schema_encode);
    for child in children {
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

#[test]
#[allow(clippy::too_many_lines)]
fn arbitrary_codec_main_executes_all_four_legs_over_arbitrary_bodies() {
    let image = arbitrary_codec_main_image();
    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    eprintln!(
        "ARBITRARY_CODEC_MAIN functions={} parameters={} blocks={} operations={} constants={} adapters={} image_bytes={} package_digest={:?}",
        image.functions.len(),
        image.parameters.len(),
        image.blocks.len(),
        image.operations.len(),
        image.constants.len(),
        image.adapters.len(),
        package.image_bytes.len(),
        approved.package_digest,
    );
    assert_eq!(image.functions.len(), 132);
    assert_eq!(image.parameters.len(), 7_791);
    assert_eq!(image.blocks.len(), 2_131);
    assert_eq!(image.operations.len(), 4_531);
    assert_eq!(image.constants.len(), 138);
    assert_eq!(image.adapters.len(), 4);
    assert_eq!(package.image_bytes.len(), 568_674);
    assert_eq!(
        approved.package_digest,
        [
            0x44, 0x77, 0x0d, 0x83, 0x31, 0x9b, 0x52, 0xd4, 0x08, 0x48, 0x32, 0x19, 0xf5, 0xe2,
            0xce, 0x9d, 0xd3, 0x27, 0x3a, 0x16, 0x34, 0xec, 0x43, 0xfd, 0xa8, 0xec, 0x49, 0x6b,
            0x2e, 0x7b, 0xdb, 0xa6,
        ]
    );

    // Selector 0 accepts every representative object the bounded profile
    // accepts, plus every rich body the bounded profile refuses; selector 1
    // still re-emits the representative objects byte for byte.
    let representative = super::all_kind_digest_dispatch::fixed_profile_bodies();
    for (kind, body) in &representative {
        let entity = [0x30 + u8::try_from(*kind).expect("entity kind fits u8"); 32];
        let stored = super::all_kind_digest_dispatch::stored_from_body(entity, body);
        let decoded = codec_call(&package, &approved, 0, *kind, [&stored, &[], &[], &[], &[]]);
        let fields = codec_ok(&decoded);
        assert_eq!(fields[0].data, ConstData::UInt(0));
        assert_eq!(fields[1].data, ConstData::UInt(u128::from(*kind)));
        assert_eq!(fields[2].data, ConstData::Bytes(entity.to_vec()));
        let (body_input, first, second, third) = if *kind == 18 {
            (Vec::new(), vec![0xb1; 32], vec![0xb2; 32], vec![0xb3; 32])
        } else {
            (body.clone(), Vec::new(), Vec::new(), Vec::new())
        };
        let encoded = codec_call(
            &package,
            &approved,
            1,
            *kind,
            [&entity, &body_input, &first, &second, &third],
        );
        let fields = codec_ok(&encoded);
        assert_eq!(fields[2].data, ConstData::Bytes(stored));
    }
    let bounded = admit_with_limits(&codec_main_image(), codec_profile_limits());
    let mut peak = (0_u64, 0_u64, 0_u64);
    for (kind, body) in super::dependency_binding_decode::rich_schema_bodies() {
        let entity = [0x40 + u8::try_from(kind).expect("entity kind fits u8"); 32];
        let stored = super::all_kind_digest_dispatch::stored_from_body(entity, &body);
        let decoded = codec_call(&package, &approved, 0, kind, [&stored, &[], &[], &[], &[]]);
        let fields = codec_ok(&decoded);
        assert_eq!(fields[1].data, ConstData::UInt(u128::from(kind)));
        assert_eq!(fields[2].data, ConstData::Bytes(entity.to_vec()));
        assert_eq!(fields[3].data, ConstData::Bytes(body.clone()));
        // Selector 1 re-emits the same arbitrary object byte for byte; the
        // bounded composition refuses the object on decode.
        let encoded = codec_call(
            &package,
            &approved,
            1,
            kind,
            [&entity, &body, &[], &[], &[]],
        );
        assert_eq!(codec_ok(&encoded)[2].data, ConstData::Bytes(stored.clone()));
        peak = (
            peak.0.max(decoded.fuel_used.max(encoded.fuel_used)),
            peak.1
                .max(decoded.instruction_count.max(encoded.instruction_count)),
            peak.2
                .max(decoded.peak_value_units.max(encoded.peak_value_units)),
        );
        let refused = codec_call(
            &bounded.0,
            &bounded.1,
            0,
            kind,
            [&stored, &[], &[], &[], &[]],
        );
        assert_refusal(&refused, "SSMC_RESERVED_FIELD_PRESENT");
    }
    eprintln!(
        "ARBITRARY_CODEC_MAIN peak fuel={} instructions={} value_units={}",
        peak.0, peak.1, peak.2
    );
    assert_eq!(peak, (660_371, 75_558, 45_356_640));

    // The schema legs and the typed refusal paths are unchanged.
    let record = sley_state_root::conformance_epoch_record()
        .canonical_bytes()
        .expect("conformance record is canonical");
    let preimage = sley_schema::bootstrap_preimage(&record).expect("bootstrap preimage");
    let epoch = sley_state_root::conformance_epoch_id().expect("conformance epoch");
    let decoded = codec_call(&package, &approved, 2, 0, [&preimage, &[], &[], &[], &[]]);
    let fields = codec_ok(&decoded);
    assert_eq!(fields[2].data, ConstData::Bytes(epoch.as_bytes().to_vec()));
    assert_eq!(fields[3].data, ConstData::Bytes(record.clone()));
    let encoded = codec_call(
        &package,
        &approved,
        3,
        0,
        [epoch.as_bytes(), &record, &[], &[], &[]],
    );
    assert_eq!(codec_ok(&encoded)[2].data, ConstData::Bytes(preimage));
    let unknown = codec_call(&package, &approved, 4, 0, [&[], &[], &[], &[], &[]]);
    assert_refusal(&unknown, "VERSION");
    let (_, workspace_body) = representative
        .iter()
        .find(|(kind, _)| *kind == 1)
        .expect("Workspace profile exists");
    let mut malformed = workspace_body.clone();
    malformed.push(0);
    let stored = super::all_kind_digest_dispatch::stored_from_body([0xcc; 32], &malformed);
    let refused = codec_call(&package, &approved, 0, 1, [&stored, &[], &[], &[], &[]]);
    assert_refusal(&refused, "SCB_TRAILING_BYTES");
    let refused = codec_call(
        &package,
        &approved,
        1,
        1,
        [&[0xcc; 32], &malformed, &[], &[], &[]],
    );
    assert_refusal(&refused, "SCB_TRAILING_BYTES");
    let refused = codec_call(
        &package,
        &approved,
        1,
        1,
        [&[0xcc; 31], workspace_body, &[], &[], &[]],
    );
    assert_refusal(&refused, "SCB_LENGTH_OVERFLOW");
    // The encode leg refuses an unknown entity kind before composing
    // anything: kind 0 and kind 19 are outside the closed 1..=18 vocabulary
    // on both legs.
    for unknown_kind in [0_u64, 19] {
        let refused = codec_call(
            &package,
            &approved,
            1,
            unknown_kind,
            [&[0xcc; 32], workspace_body, &[], &[], &[]],
        );
        assert_refusal(&refused, "SSMC_ENTITY_KIND_UNKNOWN");
        let refused = codec_call(
            &package,
            &approved,
            0,
            unknown_kind,
            [
                &super::all_kind_digest_dispatch::stored_from_body([0xcc; 32], workspace_body),
                &[],
                &[],
                &[],
                &[],
            ],
        );
        assert_refusal(&refused, "SSMC_ENTITY_KIND_UNKNOWN");
    }
    // Kind 18 on the canonical decode leg answers with the strict decoder's
    // native codes rather than a template mismatch.
    let (_, dependency_body) = representative
        .iter()
        .find(|(kind, _)| *kind == 18)
        .expect("DependencyBinding profile exists");
    let mut trailing = dependency_body.clone();
    trailing.push(0);
    let stored = super::all_kind_digest_dispatch::stored_from_body([0xcc; 32], &trailing);
    let refused = codec_call(&package, &approved, 0, 18, [&stored, &[], &[], &[], &[]]);
    assert_refusal(&refused, "SCB_TRAILING_BYTES");
}

#[test]
fn arbitrary_codec_main_children_use_disjoint_identity_namespaces() {
    let children = [
        (
            "decode",
            super::all_kind_digest_dispatch::arbitrary_program_legs_image().image,
        ),
        ("schema_decode", super::schema_codec::schema_decode_image()),
        ("schema_encode", super::schema_codec::schema_encode_image()),
    ];
    let mut owners: std::collections::BTreeMap<u8, std::collections::BTreeSet<&str>> =
        std::collections::BTreeMap::new();
    for (name, image) in &children {
        let ids = image
            .functions
            .iter()
            .map(|graph| graph.entity_id)
            .chain(image.parameters.iter().map(|p| p.entity_id))
            .chain(image.blocks.iter().map(|b| b.entity_id))
            .chain(image.operations.iter().map(|o| o.entity_id))
            .chain(image.constants.iter().map(|c| c.entity_id));
        for id in ids {
            owners.entry(id.as_bytes()[0]).or_default().insert(name);
        }
    }
    // The two schema legs deliberately share identity namespace 12 with
    // distinct ordinals; the arbitrary decoder must share nothing.
    let shared = owners
        .iter()
        .filter(|(_, names)| names.len() > 1 && names.contains("decode"))
        .map(|(namespace, names)| format!("{namespace}: {names:?}"))
        .collect::<Vec<_>>();
    assert!(shared.is_empty(), "shared identity namespaces: {shared:?}");
}
