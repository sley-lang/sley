//! Bounded schema bootstrap codec for the frozen conformance epoch.
//!
//! The epoch registry is closed at this construction step, so the canonical
//! record, its derived ID, and its exact `SLEYEP01` preimage are finite pinned
//! data. The admitted Sley functions own every comparison, branch, refusal,
//! tuple construction, and emitted byte value. Native schema code appears only
//! in the parity tests that derive the fixture and expected decisions.

use super::*;

const RECORD_INVALID: &[u8] = b"SCHEMA_RECORD_INVALID";
const EPOCH_MISMATCH: &[u8] = b"SCHEMA_EPOCH_MISMATCH";

struct SchemaFixture {
    epoch: Vec<u8>,
    record: Vec<u8>,
    preimage: Vec<u8>,
}

fn schema_fixture() -> SchemaFixture {
    let record = sley_state_root::conformance_epoch_record()
        .canonical_bytes()
        .expect("frozen conformance record is canonical");
    let preimage = sley_schema::bootstrap_preimage(&record)
        .expect("native schema crate emits the frozen bootstrap preimage");
    let (imported_epoch, imported_record) = sley_schema::import_bootstrap_preimage(&preimage)
        .expect("native schema crate imports its own frozen preimage");
    assert_eq!(
        imported_record
            .canonical_bytes()
            .expect("imported record remains canonical"),
        record
    );
    let expected_epoch =
        sley_state_root::conformance_epoch_id().expect("frozen conformance epoch has a derived ID");
    assert_eq!(imported_epoch, expected_epoch);
    SchemaFixture {
        epoch: expected_epoch.as_bytes().to_vec(),
        record,
        preimage,
    }
}

fn schema_decode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes])),
        error: Box::new(TypeExpr::Bytes),
    }
}

fn schema_encode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Bytes),
        error: Box::new(TypeExpr::Bytes),
    }
}

fn build_schema_decode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    fixture: &SchemaFixture,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = schema_decode_result_type();
    let input = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let expected_preimage = assembler.kbytes(ns.k, &fixture.preimage);
    let expected_epoch = assembler.kbytes(ns.k, &fixture.epoch);
    let expected_record = assembler.kbytes(ns.k, &fixture.record);
    let invalid_code = assembler.kbytes(ns.k, RECORD_INVALID);

    let entry = assembler.id(ns.b);
    let accepted = assembler.id(ns.b);
    let rejected = err_block(assembler, ns, function, result_type.clone(), invalid_code);

    let expected = assembler.cref(ns.o, entry, expected_preimage, TypeExpr::Bytes);
    let matches = assembler.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(input), op_result(expected)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    assembler.blocks.push(Block {
        entity_id: entry,
        function,
        parameters: Vec::new(),
        operations: vec![expected, matches],
        terminator: cond(
            op_result(matches),
            edge(accepted, Vec::new()),
            edge(rejected, Vec::new()),
        ),
        reachability: Reachability::Required,
    });

    let epoch = assembler.cref(ns.o, accepted, expected_epoch, TypeExpr::Bytes);
    let record = assembler.cref(ns.o, accepted, expected_record, TypeExpr::Bytes);
    let decoded = assembler.op(
        ns.o,
        accepted,
        Opcode::TupleNew,
        vec![op_result(epoch), op_result(record)],
        vec![TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes])],
        Immediate::None,
    );
    let ok = assembler.op(
        ns.o,
        accepted,
        Opcode::ResultOk,
        vec![op_result(decoded)],
        vec![result_type.clone()],
        Immediate::None,
    );
    assembler.blocks.push(Block {
        entity_id: accepted,
        function,
        parameters: Vec::new(),
        operations: vec![epoch, record, decoded, ok],
        terminator: ret(op_result(ok)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![input, unit],
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

fn build_schema_encode(
    assembler: &mut Asm,
    ns: Ns,
    function: EntityId,
    fixture: &SchemaFixture,
) -> FunctionGraph {
    let block_start = assembler.blocks.len();
    let result_type = schema_encode_result_type();
    let epoch = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let record = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Bytes);
    let unit = assembler.param(ns.p, function, ParameterRole::Function, TypeExpr::Unit);
    let expected_epoch = assembler.kbytes(ns.k, &fixture.epoch);
    let expected_record = assembler.kbytes(ns.k, &fixture.record);
    let expected_preimage = assembler.kbytes(ns.k, &fixture.preimage);
    let epoch_code = assembler.kbytes(ns.k, EPOCH_MISMATCH);
    let record_code = assembler.kbytes(ns.k, RECORD_INVALID);

    let entry = assembler.id(ns.b);
    let record_check = assembler.id(ns.b);
    let accepted = assembler.id(ns.b);
    let epoch_rejected = err_block(assembler, ns, function, result_type.clone(), epoch_code);
    let record_rejected = err_block(assembler, ns, function, result_type.clone(), record_code);

    let frozen_epoch = assembler.cref(ns.o, entry, expected_epoch, TypeExpr::Bytes);
    let epoch_matches = assembler.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(epoch), op_result(frozen_epoch)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    assembler.blocks.push(Block {
        entity_id: entry,
        function,
        parameters: Vec::new(),
        operations: vec![frozen_epoch, epoch_matches],
        terminator: cond(
            op_result(epoch_matches),
            edge(record_check, Vec::new()),
            edge(epoch_rejected, Vec::new()),
        ),
        reachability: Reachability::Required,
    });

    let frozen_record = assembler.cref(ns.o, record_check, expected_record, TypeExpr::Bytes);
    let record_matches = assembler.op(
        ns.o,
        record_check,
        Opcode::Equal,
        vec![pav(record), op_result(frozen_record)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    assembler.blocks.push(Block {
        entity_id: record_check,
        function,
        parameters: Vec::new(),
        operations: vec![frozen_record, record_matches],
        terminator: cond(
            op_result(record_matches),
            edge(accepted, Vec::new()),
            edge(record_rejected, Vec::new()),
        ),
        reachability: Reachability::Required,
    });

    let preimage = assembler.cref(ns.o, accepted, expected_preimage, TypeExpr::Bytes);
    let ok = assembler.op(
        ns.o,
        accepted,
        Opcode::ResultOk,
        vec![op_result(preimage)],
        vec![result_type.clone()],
        Immediate::None,
    );
    assembler.blocks.push(Block {
        entity_id: accepted,
        function,
        parameters: Vec::new(),
        operations: vec![preimage, ok],
        terminator: ret(op_result(ok)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![epoch, record, unit],
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

fn schema_decode_image() -> Image {
    let fixture = schema_fixture();
    let mut assembler = Asm::new();
    let ns = Ns {
        k: 226,
        p: 227,
        b: 228,
        o: 229,
    };
    let function = eid(12, 1);
    let graph = build_schema_decode(&mut assembler, ns, function, &fixture);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: Vec::new(),
        constants: assembler.constants,
    }
}

fn schema_encode_image() -> Image {
    let fixture = schema_fixture();
    let mut assembler = Asm::new();
    let ns = Ns {
        k: 230,
        p: 231,
        b: 232,
        o: 233,
    };
    let function = eid(12, 2);
    let graph = build_schema_encode(&mut assembler, ns, function, &fixture);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: Vec::new(),
        constants: assembler.constants,
    }
}

fn assert_schema_decode_ok(outcome: &sley_vm::ExecutionOutcome, fixture: &SchemaFixture) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("schema decode must terminate successfully: {outcome:?}");
    };
    let ConstData::Result(ResultConst::Ok(decoded)) = &value.data else {
        panic!("schema decode must return Ok: {value:?}");
    };
    let ConstData::Sequence(fields) = &decoded.data else {
        panic!("schema decode Ok must contain a tuple: {decoded:?}");
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].data, ConstData::Bytes(fixture.epoch.clone()));
    assert_eq!(fields[1].data, ConstData::Bytes(fixture.record.clone()));
}

#[test]
fn schema_decode_matches_native_registered_epoch_and_refusals() {
    let fixture = schema_fixture();
    let (package, approved) = admit(&schema_decode_image());
    let accepted = execute(
        &package,
        &approved,
        vec![bytes_input(&fixture.preimage), unit_input()],
    );
    assert_schema_decode_ok(&accepted, &fixture);

    let mut bad_magic = fixture.preimage.clone();
    bad_magic[0] ^= 1;
    let mut truncated = fixture.preimage.clone();
    truncated.pop();
    let mut trailing = fixture.preimage.clone();
    trailing.push(0);
    let mut nonminimal_version = Vec::from(&b"SLEYEP01"[..]);
    nonminimal_version.extend_from_slice(&[0x81, 0x00]);
    nonminimal_version.extend_from_slice(&fixture.preimage[9..]);
    for input in [bad_magic, truncated, trailing, nonminimal_version] {
        assert_eq!(
            sley_schema::import_bootstrap_preimage(&input)
                .expect_err("native schema import must reject malformed preimage")
                .code(),
            sley_schema::SchemaErrorCode::RecordInvalid
        );
        let refused = execute(&package, &approved, vec![bytes_input(&input), unit_input()]);
        assert_refusal(&refused, "SCHEMA_RECORD_INVALID");
    }
}

#[test]
fn schema_encode_matches_native_and_preserves_failure_precedence() {
    let fixture = schema_fixture();
    let (package, approved) = admit(&schema_encode_image());
    let accepted = execute(
        &package,
        &approved,
        vec![
            bytes_input(&fixture.epoch),
            bytes_input(&fixture.record),
            unit_input(),
        ],
    );
    assert_encode_ok(&accepted, &fixture.preimage);
    assert_eq!(
        fixture.preimage,
        sley_schema::bootstrap_preimage(&fixture.record)
            .expect("native schema codec emits canonical preimage")
    );

    let wrong_epoch = vec![0; fixture.epoch.len()];
    let wrong_record = vec![0; fixture.record.len()];
    let cases = [
        (&wrong_epoch, &wrong_record, "SCHEMA_EPOCH_MISMATCH"),
        (&wrong_epoch, &fixture.record, "SCHEMA_EPOCH_MISMATCH"),
        (&fixture.epoch, &wrong_record, "SCHEMA_RECORD_INVALID"),
    ];
    for (epoch_bytes, record_bytes, expected) in cases {
        let refused = execute(
            &package,
            &approved,
            vec![
                bytes_input(epoch_bytes),
                bytes_input(record_bytes),
                unit_input(),
            ],
        );
        assert_refusal(&refused, expected);
    }
}
