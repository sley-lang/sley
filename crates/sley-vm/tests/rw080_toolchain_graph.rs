//! RW-080 provisional aggregate toolchain graph construction.
//!
//! This is the smallest admitted closure that binds the codec, checker,
//! lowerer, package-builder, and driver surfaces together. The leaf behavior
//! remains scaffold-only; the graph is a component construction witness, not
//! the canonical state root `S` and not a compiler correctness claim.

use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    Block, ConstData, ConstValue, ConstantDefinition, FunctionGraph, FunctionRefValue, Immediate,
    IntegerWidth, Opcode, Operation, OperationResultRef, Parameter, ParameterRole, Reachability,
    ReturnTerminator, Terminator, TypeExpr, ValueRef, Visibility,
};
use std::fmt::Write;

const DRIVER: u8 = 1;
const CODEC: u8 = 2;
const CHECKER: u8 = 3;
const LOWERER: u8 = 4;
const BUILDER: u8 = 5;

fn id(byte: u8) -> EntityId {
    EntityId::from_bytes([byte; 32])
}

fn epoch() -> SchemaEpochId {
    SchemaEpochId::from_bytes([8; 32])
}

fn fixture_root() -> StateRoot {
    StateRoot::from_bytes([9; 32])
}

fn uint(bits: u16) -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(bits))
}

fn uint_value(bits: u16, value: u128) -> ConstValue {
    ConstValue {
        value_type: uint(bits),
        data: ConstData::UInt(value),
    }
}

fn bytes_value(bytes: &[u8]) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(bytes.to_vec()),
    }
}

fn op_result(operation: u8) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation: id(operation),
        result_index: 0,
    })
}

fn leaf_graph(function: u8, block: u8, result_type: TypeExpr) -> FunctionGraph {
    FunctionGraph {
        entity_id: id(function),
        type_parameters: Vec::new(),
        parameters: if function == BUILDER {
            vec![id(11)]
        } else {
            Vec::new()
        },
        result_type,
        effects: Vec::new(),
        entry_block: id(block),
        blocks: vec![id(block)],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

struct ToolchainImage {
    types: sley_check::TypeEnvironment,
    entry: FunctionGraph,
    functions: Vec<FunctionGraph>,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
}

fn direct_call(
    operation: u8,
    ordinal: u32,
    function: u8,
    operands: Vec<ValueRef>,
    result_type: TypeExpr,
) -> Operation {
    Operation {
        entity_id: id(operation),
        block: id(20),
        ordinal,
        opcode: Opcode::CallDirect,
        operands,
        result_types: vec![result_type],
        immediate: Immediate::Function(FunctionRefValue {
            function: id(function),
            type_arguments: Vec::new(),
        }),
    }
}

fn constant_ref(operation: u8, block: u8, constant: u8, result_type: TypeExpr) -> Operation {
    Operation {
        entity_id: id(operation),
        block: id(block),
        ordinal: 0,
        opcode: Opcode::ConstantRef,
        operands: Vec::new(),
        result_types: vec![result_type],
        immediate: Immediate::Entity(id(constant)),
    }
}

fn toolchain_operations(result_type: TypeExpr) -> Vec<Operation> {
    vec![
        direct_call(
            40,
            0,
            BUILDER,
            vec![ValueRef::Parameter(id(10))],
            TypeExpr::Bytes,
        ),
        direct_call(41, 1, CODEC, Vec::new(), uint(8)),
        direct_call(42, 2, CHECKER, Vec::new(), uint(8)),
        direct_call(43, 3, LOWERER, Vec::new(), uint(32)),
        Operation {
            entity_id: id(44),
            block: id(20),
            ordinal: 4,
            opcode: Opcode::TupleNew,
            operands: vec![op_result(40), op_result(41), op_result(42), op_result(43)],
            result_types: vec![result_type],
            immediate: Immediate::None,
        },
        constant_ref(45, 21, 30, uint(8)),
        constant_ref(46, 22, 31, uint(8)),
        constant_ref(47, 23, 32, uint(32)),
    ]
}

fn returning_block(block: u8, function: u8, operations: Vec<EntityId>, value: ValueRef) -> Block {
    Block {
        entity_id: id(block),
        function: id(function),
        parameters: Vec::new(),
        operations,
        terminator: Terminator::Return(ReturnTerminator { value }),
        reachability: Reachability::Required,
    }
}

fn toolchain_blocks() -> Vec<Block> {
    vec![
        returning_block(20, DRIVER, (40..=44).map(id).collect(), op_result(44)),
        returning_block(21, CODEC, vec![id(45)], op_result(45)),
        returning_block(22, CHECKER, vec![id(46)], op_result(46)),
        returning_block(23, LOWERER, vec![id(47)], op_result(47)),
        returning_block(24, BUILDER, Vec::new(), ValueRef::Parameter(id(11))),
    ]
}

fn aggregate_toolchain_graph() -> ToolchainImage {
    let result_type = TypeExpr::Tuple(vec![TypeExpr::Bytes, uint(8), uint(8), uint(32)]);
    let driver = FunctionGraph {
        entity_id: id(DRIVER),
        type_parameters: Vec::new(),
        parameters: vec![id(10)],
        result_type: result_type.clone(),
        effects: Vec::new(),
        entry_block: id(20),
        blocks: vec![id(20)],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let functions = vec![
        driver.clone(),
        leaf_graph(CODEC, 21, uint(8)),
        leaf_graph(CHECKER, 22, uint(8)),
        leaf_graph(LOWERER, 23, uint(32)),
        leaf_graph(BUILDER, 24, TypeExpr::Bytes),
    ];
    ToolchainImage {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: driver,
        functions,
        parameters: vec![
            Parameter {
                entity_id: id(10),
                owner: id(DRIVER),
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: TypeExpr::Bytes,
            },
            Parameter {
                entity_id: id(11),
                owner: id(BUILDER),
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: TypeExpr::Bytes,
            },
        ],
        blocks: toolchain_blocks(),
        operations: toolchain_operations(result_type),
        constants: vec![
            ConstantDefinition {
                entity_id: id(30),
                value: uint_value(8, 6),
            },
            ConstantDefinition {
                entity_id: id(31),
                value: uint_value(8, 0),
            },
            ConstantDefinition {
                entity_id: id(32),
                value: uint_value(32, 0),
            },
        ],
    }
}

fn generous_limits() -> sley_vm::ExecutionLimits {
    sley_vm::ExecutionLimits {
        max_instructions: 20_000,
        max_fuel: 200_000,
        max_value_units: 2_000_000,
        max_output_units: 100_000,
        cancel_at_fuel: None,
    }
}

fn admitted_toolchain_graph() -> (sley_vm::ExecutionPackage, sley_vm::ApprovedExecutionPackage) {
    use sley_vm::{
        ExecutionPackage, V2Closure, approve_package_v2, bootstrap::BootstrapProfileInput,
        bootstrap::BootstrapProfileVersion,
    };
    let image = aggregate_toolchain_graph();
    let lowered = sley_vm::lower_function(sley_vm::LoweringInput {
        types: &image.types,
        function: &image.entry,
        parameters: &image.parameters,
        blocks: &image.blocks,
        operations: &image.operations,
        schema_epoch: epoch(),
        state_root: fixture_root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &image.constants,
        globals: &[],
        functions: &image.functions,
        contracts: &[],
        adapters: &[],
    })
    .expect("aggregate graph lowers");
    let gate = sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &image.types,
        schema_epoch: epoch(),
        entry: &image.entry,
        presented_image_bytes: &lowered.bytes,
        functions: &image.functions,
        parameters: &image.parameters,
        blocks: &image.blocks,
        operations: &image.operations,
        adapters: &[],
        constants: &image.constants,
        profile_version: BootstrapProfileVersion::V2,
    })
    .expect("aggregate graph admits under V2");
    let package = ExecutionPackage {
        image_bytes: lowered.bytes,
        constants: image.constants.clone(),
        type_definitions: Vec::new(),
        imports: Vec::new(),
        globals: Vec::new(),
        contracts: Vec::new(),
        entry: image.entry.entity_id,
        schema_epoch: epoch(),
        state_root: fixture_root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        admitted_limits: generous_limits(),
        gate_operation_count: gate.operation_count(),
        gate_bridge_uses: gate.bridge_uses(),
        gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
    };
    let closure = V2Closure {
        types: &image.types,
        schema_epoch: epoch(),
        state_root: fixture_root(),
        entry: image.entry.entity_id,
        functions: &image.functions,
        parameters: &image.parameters,
        blocks: &image.blocks,
        operations: &image.operations,
        adapters: &[],
        constants: &image.constants,
        globals: &[],
        contracts: &[],
    };
    let (digests, receipt, report) =
        sley_vm::admit_v2_package(&closure, &package).expect("authority admits aggregate graph");
    let approved = approve_package_v2(&package, &digests, receipt, &report)
        .expect("authority approves aggregate graph");
    (package, approved)
}

fn execute_driver(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    manifest: &[u8],
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![bytes_value(manifest)],
            limits: generous_limits(),
        },
    )
    .expect("aggregate driver executes")
}

fn assert_driver_result(outcome: &sley_vm::ExecutionOutcome, manifest: &[u8]) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "aggregate driver must return, got {:?}",
            outcome.termination
        );
    };
    assert_eq!(
        value.value_type,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, uint(8), uint(8), uint(32)])
    );
    assert_eq!(
        value.data,
        ConstData::Sequence(vec![
            bytes_value(manifest),
            uint_value(8, 6),
            uint_value(8, 0),
            uint_value(32, 0),
        ])
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
        output
    })
}

#[test]
fn aggregate_toolchain_graph_admits_and_runs_the_driver_surface() {
    let (package, approved) = admitted_toolchain_graph();
    for manifest in [b"manifest-a".as_slice(), b"\0manifest-b\xff".as_slice()] {
        let outcome = execute_driver(&package, &approved, manifest);
        assert_driver_result(&outcome, manifest);
        assert!(outcome.fuel_used > 0);
        assert!(outcome.instruction_count > 0);
        println!(
            "rw080_aggregate_driver manifest_bytes={} fuel={} instructions={} peak_value_units={}",
            manifest.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units,
        );
    }
}

#[test]
fn aggregate_toolchain_package_has_stable_component_identity() {
    let (package, _) = admitted_toolchain_graph();
    let envelope = sley_vm::encode_package_envelope_v2(&package).expect("package encodes");
    let decoded = sley_vm::decode_package_envelope_v2(&envelope).expect("package decodes");
    let digests = sley_vm::package_digests_v2(&package).expect("package digests");
    assert_eq!(decoded.entry, id(DRIVER));
    assert_eq!(decoded.schema_epoch, epoch());
    assert_eq!(decoded.state_root, fixture_root());
    assert_eq!(decoded.digests, digests);
    assert_eq!(package.gate_bridge_uses, 0);
    assert_eq!(package.gate_closure_fingerprints.len(), 5);
    assert_eq!(
        digests.package_digest,
        [
            0x25, 0xcf, 0x82, 0xa3, 0xfb, 0xf5, 0xc0, 0x7b, 0x49, 0x26, 0xcb, 0x03, 0xe1, 0xac,
            0xbb, 0x0d, 0x6a, 0x12, 0xe5, 0x2b, 0xb5, 0x3f, 0xd7, 0xd6, 0x7a, 0xbf, 0xa5, 0xaf,
            0x03, 0xd0, 0x5c, 0xac,
        ],
        "the provisional aggregate component identity is pinned"
    );
    println!(
        "rw080_aggregate envelope_bytes={} image_bytes={} operations={} fingerprints={} package_digest={}",
        envelope.len(),
        package.image_bytes.len(),
        package.gate_operation_count,
        package.gate_closure_fingerprints.len(),
        hex(&digests.package_digest),
    );
}
