//! Bounded RW-120 build-driver construction.

use super::*;
use sley_ssmc::{CondBranchTerminator, TargetEdge};

const SUCCESS_BLOCK_ORDINAL: u64 = 100;
const ERROR_BLOCK_ORDINAL: u64 = 101;
const DRIVER_OPERATION_BASE: u64 = 100;
const DRIVER_PARAMETER_BASE: u64 = 100;
const DRIVER_CONSTANT_BASE: u64 = 100;

fn driver_id(kind: u32, ordinal: u64) -> EntityId {
    EntityId::derive(workspace(), candidate_nonce(), kind, ordinal)
}

fn operation_result(entity_id: EntityId) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation: entity_id,
        result_index: 0,
    })
}

fn driver_operation(
    index: u64,
    block: EntityId,
    ordinal: u32,
    opcode: Opcode,
    operands: Vec<ValueRef>,
    result_type: TypeExpr,
    immediate: Immediate,
) -> Operation {
    Operation {
        entity_id: driver_id(8, DRIVER_OPERATION_BASE + index),
        block,
        ordinal,
        opcode,
        operands,
        result_types: vec![result_type],
        immediate,
    }
}

fn driver_constant(index: u64, value: ConstValue) -> ConstantDefinition {
    ConstantDefinition {
        entity_id: driver_id(9, DRIVER_CONSTANT_BASE + index),
        value,
    }
}

fn block_parameter(index: u64, owner: EntityId, ordinal: u32, value_type: TypeExpr) -> Parameter {
    Parameter {
        entity_id: driver_id(6, DRIVER_PARAMETER_BASE + index),
        owner,
        role: ParameterRole::Block,
        ordinal,
        value_type,
    }
}

fn bytes_vector_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(TypeExpr::Bytes))
}

fn false_value() -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(false),
    }
}

fn bounded_built_result(object_closure: &[u8], package_digest: &[u8; 32]) -> ConstValue {
    ConstValue {
        value_type: build_result_type(),
        data: ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
            value_type: built_toolchain_type(),
            data: ConstData::Record(RecordConst {
                definition: id(71),
                fields: vec![
                    FieldConst {
                        member_id: member(0xB0),
                        value: bytes_vector(vec![object_closure.to_vec()]),
                    },
                    FieldConst {
                        member_id: member(0xB1),
                        value: bytes_vector(vec![package_digest.to_vec()]),
                    },
                    FieldConst {
                        member_id: member(0xB2),
                        value: false_value(),
                    },
                ],
            }),
        }))),
    }
}

fn dependency_error(bad_pin: &[u8]) -> ConstValue {
    ConstValue {
        value_type: build_result_type(),
        data: ConstData::Result(ResultConst::Err(Box::new(ConstValue {
            value_type: build_error_type(),
            data: ConstData::Variant(VariantConst {
                definition: id(72),
                member_id: member(0xC3),
                payload: Some(Box::new(bytes_value(bad_pin))),
            }),
        }))),
    }
}

fn bounded_construction_test_result() -> ConstValue {
    let digest: [u8; 32] =
        <Sha256 as Digest>::digest(b"SLEY2/RW080/CONTRACT-TEST/EXPECTED-PACKAGE/V1").into();
    bounded_built_result(b"SLEY2/RW080/CONTRACT-TEST/OBJECT-CLOSURE/V1", &digest)
}

fn validation_operations(
    entry: EntityId,
    constants: &[EntityId; 6],
) -> (Vec<Operation>, Vec<EntityId>) {
    let fields = [0xA0, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7];
    let field_types = [
        TypeExpr::Bytes,
        bytes_vector_type(),
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
    ];
    let mut operations = fields
        .into_iter()
        .zip(field_types)
        .enumerate()
        .map(|(index, (field, result_type))| {
            driver_operation(
                u64::try_from(index).unwrap(),
                entry,
                u32::try_from(index).unwrap(),
                Opcode::RecordGet,
                vec![ValueRef::Parameter(id(10))],
                result_type,
                Immediate::Field(member(field)),
            )
        })
        .collect::<Vec<_>>();
    let projected = operations
        .iter()
        .map(|operation| operation.entity_id)
        .collect::<Vec<_>>();

    let mut equalities = Vec::new();
    for index in 0..5_u64 {
        let constant_ref = driver_operation(
            7 + index * 2,
            entry,
            7 + u32::try_from(index * 2).unwrap(),
            Opcode::ConstantRef,
            Vec::new(),
            TypeExpr::Bytes,
            Immediate::Entity(constants[usize::try_from(index).unwrap()]),
        );
        let equal = driver_operation(
            8 + index * 2,
            entry,
            8 + u32::try_from(index * 2).unwrap(),
            Opcode::Equal,
            vec![
                operation_result(projected[usize::try_from(index + 2).unwrap()]),
                operation_result(constant_ref.entity_id),
            ],
            TypeExpr::Bool,
            Immediate::None,
        );
        equalities.push(equal.entity_id);
        operations.extend([constant_ref, equal]);
    }
    let mut condition = equalities[0];
    for (offset, equality) in equalities.into_iter().skip(1).enumerate() {
        let and = driver_operation(
            17 + u64::try_from(offset).unwrap(),
            entry,
            17 + u32::try_from(offset).unwrap(),
            Opcode::BoolAnd,
            vec![operation_result(condition), operation_result(equality)],
            TypeExpr::Bool,
            Immediate::None,
        );
        condition = and.entity_id;
        operations.push(and);
    }
    let identities = operations
        .iter()
        .map(|operation| operation.entity_id)
        .collect::<Vec<_>>();
    (operations, identities)
}

fn direct_leaf_call(
    index: u64,
    block: EntityId,
    ordinal: u32,
    function: u8,
    operands: Vec<ValueRef>,
    result_type: TypeExpr,
) -> Operation {
    driver_operation(
        index,
        block,
        ordinal,
        Opcode::CallDirect,
        operands,
        result_type,
        Immediate::Function(FunctionRefValue {
            function: id(function),
            type_arguments: Vec::new(),
        }),
    )
}

fn successful_build_operations(
    success: EntityId,
    success_closure: EntityId,
    success_digests: EntityId,
    false_constant: EntityId,
) -> (Vec<Operation>, Vec<EntityId>) {
    let success_offset = 30_u64;
    let built = direct_leaf_call(
        success_offset,
        success,
        0,
        BUILDER,
        vec![ValueRef::Parameter(success_closure)],
        TypeExpr::Bytes,
    );
    let codec = direct_leaf_call(success_offset + 1, success, 1, CODEC, Vec::new(), uint(8));
    let checker = direct_leaf_call(success_offset + 2, success, 2, CHECKER, Vec::new(), uint(8));
    let lowerer = direct_leaf_call(
        success_offset + 3,
        success,
        3,
        LOWERER,
        Vec::new(),
        uint(32),
    );
    let artifact_vector = driver_operation(
        success_offset + 4,
        success,
        4,
        Opcode::VectorNew,
        vec![operation_result(built.entity_id)],
        bytes_vector_type(),
        Immediate::None,
    );
    let fixed_point = driver_operation(
        success_offset + 5,
        success,
        5,
        Opcode::ConstantRef,
        Vec::new(),
        TypeExpr::Bool,
        Immediate::Entity(false_constant),
    );
    let record = driver_operation(
        success_offset + 6,
        success,
        6,
        Opcode::RecordNew,
        vec![
            operation_result(artifact_vector.entity_id),
            ValueRef::Parameter(success_digests),
            operation_result(fixed_point.entity_id),
        ],
        built_toolchain_type(),
        Immediate::Entity(id(71)),
    );
    let ok = driver_operation(
        success_offset + 7,
        success,
        7,
        Opcode::ResultOk,
        vec![operation_result(record.entity_id)],
        build_result_type(),
        Immediate::None,
    );
    let operations = vec![
        built,
        codec,
        checker,
        lowerer,
        artifact_vector,
        fixed_point,
        record,
        ok,
    ];
    let identities = operations
        .iter()
        .map(|operation| operation.entity_id)
        .collect();
    (operations, identities)
}

fn failed_build_operations(
    error: EntityId,
    error_pin: EntityId,
) -> (Vec<Operation>, Vec<EntityId>) {
    let failure = driver_operation(
        40,
        error,
        0,
        Opcode::VariantNew,
        vec![ValueRef::Parameter(error_pin)],
        build_error_type(),
        Immediate::Variant(sley_ssmc::VariantImmediate {
            definition: id(72),
            member_id: member(0xC3),
        }),
    );
    let err = driver_operation(
        41,
        error,
        1,
        Opcode::ResultErr,
        vec![operation_result(failure.entity_id)],
        build_result_type(),
        Immediate::None,
    );
    let operations = vec![failure, err];
    let identities = operations
        .iter()
        .map(|operation| operation.entity_id)
        .collect();
    (operations, identities)
}

struct DriverOperations {
    all: Vec<Operation>,
    entry: Vec<EntityId>,
    success: Vec<EntityId>,
    error: Vec<EntityId>,
}

fn build_driver_operations(
    entry: EntityId,
    success: EntityId,
    error: EntityId,
    success_closure: EntityId,
    success_digests: EntityId,
    error_pin: EntityId,
    constants: &[EntityId; 6],
) -> DriverOperations {
    let (mut all, entry) = validation_operations(entry, constants);
    let (success_operations, success) =
        successful_build_operations(success, success_closure, success_digests, constants[5]);
    let (error_operations, error) = failed_build_operations(error, error_pin);
    all.extend(success_operations);
    all.extend(error_operations);
    DriverOperations {
        all,
        entry,
        success,
        error,
    }
}

fn driver_blocks(
    entry: EntityId,
    success: EntityId,
    error: EntityId,
    success_closure: EntityId,
    success_digests: EntityId,
    error_pin: EntityId,
    operations: &DriverOperations,
) -> [Block; 3] {
    let result = |index| operation_result(driver_id(8, DRIVER_OPERATION_BASE + index));
    [
        Block {
            entity_id: entry,
            function: id(DRIVER),
            parameters: Vec::new(),
            operations: operations.entry.clone(),
            terminator: Terminator::CondBranch(CondBranchTerminator {
                condition: result(20),
                if_true: TargetEdge {
                    target: success,
                    arguments: vec![result(0), result(1)],
                },
                if_false: TargetEdge {
                    target: error,
                    arguments: vec![result(2)],
                },
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: success,
            function: id(DRIVER),
            parameters: vec![success_closure, success_digests],
            operations: operations.success.clone(),
            terminator: Terminator::Return(ReturnTerminator { value: result(37) }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: error,
            function: id(DRIVER),
            parameters: vec![error_pin],
            operations: operations.error.clone(),
            terminator: Terminator::Return(ReturnTerminator { value: result(41) }),
            reachability: Reachability::Required,
        },
    ]
}

fn bounded_driver_graph() -> ToolchainImage {
    let mut image = aggregate_toolchain_graph();
    let entry = id(20);
    let success = driver_id(7, SUCCESS_BLOCK_ORDINAL);
    let error = driver_id(7, ERROR_BLOCK_ORDINAL);
    let success_closure = driver_id(6, DRIVER_PARAMETER_BASE);
    let success_digests = driver_id(6, DRIVER_PARAMETER_BASE + 1);
    let error_pin = driver_id(6, DRIVER_PARAMETER_BASE + 2);
    let new_constants = [
        driver_constant(0, bytes_value(&BOOTSTRAP_PROFILE_V2)),
        driver_constant(1, bytes_value(&HOST_ABI_V2)),
        driver_constant(2, bytes_value(&EXEC_PACKAGE_V2)),
        driver_constant(3, bytes_value(&RAW_BLAKE3_V1)),
        driver_constant(4, bytes_value(epoch().as_bytes())),
        driver_constant(5, false_value()),
    ];
    let constant_ids = new_constants.clone().map(|constant| constant.entity_id);
    let operations = build_driver_operations(
        entry,
        success,
        error,
        success_closure,
        success_digests,
        error_pin,
        &constant_ids,
    );

    image
        .operations
        .retain(|operation| operation.block != entry);
    image.operations.extend(operations.all.clone());
    image
        .operations
        .sort_unstable_by_key(|operation| operation.entity_id);
    image.blocks.retain(|block| block.entity_id != entry);
    image.blocks.extend(driver_blocks(
        entry,
        success,
        error,
        success_closure,
        success_digests,
        error_pin,
        &operations,
    ));
    image.blocks.sort_unstable_by_key(|block| block.entity_id);
    image.parameters.extend([
        block_parameter(0, success, 0, TypeExpr::Bytes),
        block_parameter(1, success, 1, bytes_vector_type()),
        block_parameter(2, error, 0, TypeExpr::Bytes),
    ]);
    image
        .parameters
        .sort_unstable_by_key(|parameter| parameter.entity_id);
    image.constants.extend(new_constants);
    image
        .constants
        .sort_unstable_by_key(|constant| constant.entity_id);
    let driver = image
        .functions
        .iter_mut()
        .find(|function| function.entity_id == id(DRIVER))
        .expect("aggregate driver is retained");
    driver.blocks = vec![entry, success, error];
    driver.blocks.sort_unstable();
    image.entry = driver.clone();
    image.tests[0].expected = ExpectedOutcome::Value(bounded_construction_test_result());
    image.types = sley_check::TypeEnvironment::new(image.type_definitions.clone()).unwrap();
    image
}

struct OwnedUnit {
    function: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}

fn validate_bounded_driver_test(image: &ToolchainImage) {
    let mut units = image
        .functions
        .iter()
        .map(|function| {
            let block_ids = function
                .blocks
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            OwnedUnit {
                function: function.clone(),
                parameters: image
                    .parameters
                    .iter()
                    .filter(|parameter| {
                        parameter.owner == function.entity_id
                            || block_ids.contains(&parameter.owner)
                    })
                    .cloned()
                    .collect(),
                blocks: image
                    .blocks
                    .iter()
                    .filter(|block| block.function == function.entity_id)
                    .cloned()
                    .collect(),
                operations: image
                    .operations
                    .iter()
                    .filter(|operation| block_ids.contains(&operation.block))
                    .cloned()
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    units.sort_unstable_by_key(|unit| unit.function.entity_id);
    let borrowed = units
        .iter()
        .map(|unit| sley_check::effects::FunctionUnit {
            function: &unit.function,
            parameters: &unit.parameters,
            blocks: &unit.blocks,
            operations: &unit.operations,
        })
        .collect::<Vec<_>>();
    let report = sley_check::contracts::validate_contract_test_program(
        &image.types,
        &borrowed,
        &[],
        &[],
        &[],
        &image.type_definitions,
        &image.constants,
        &[],
        &image.contracts,
        &image.tests,
        &[id(DRIVER)],
        &[id(81)],
    )
    .expect("bounded driver contract and test validate");
    assert_eq!(report.selected_tests, vec![id(81)]);
}

fn admit_bounded_driver(
    image: &ToolchainImage,
) -> (sley_vm::ExecutionPackage, sley_vm::ApprovedExecutionPackage) {
    use sley_vm::{
        ExecutionPackage, V2Closure, approve_package_v2, bootstrap::BootstrapProfileInput,
        bootstrap::BootstrapProfileVersion,
    };
    let objects = canonical_component_objects(image);
    let root = canonical_component_root(&objects);
    let executable_functions = image
        .functions
        .iter()
        .filter(|function| {
            function.entity_id != id(PREDICATE) && function.entity_id != id(CONTRACT_TARGET)
        })
        .cloned()
        .collect::<Vec<_>>();
    let lowered = sley_vm::lower_function(sley_vm::LoweringInput {
        types: &image.types,
        function: &image.entry,
        parameters: &image.parameters,
        blocks: &image.blocks,
        operations: &image.operations,
        schema_epoch: epoch(),
        state_root: root.root,
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &image.constants,
        globals: &[],
        functions: &executable_functions,
        contracts: &[],
        adapters: &[],
    })
    .expect("bounded build driver lowers");
    let gate = sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &image.types,
        schema_epoch: epoch(),
        entry: &image.entry,
        presented_image_bytes: &lowered.bytes,
        functions: &executable_functions,
        parameters: &image.parameters,
        blocks: &image.blocks,
        operations: &image.operations,
        adapters: &[],
        constants: &image.constants,
        profile_version: BootstrapProfileVersion::V2,
    })
    .expect("bounded build driver admits under V2");
    let package = ExecutionPackage {
        image_bytes: lowered.bytes,
        constants: image.constants.clone(),
        type_definitions: image.type_definitions.clone(),
        imports: Vec::new(),
        globals: Vec::new(),
        contracts: Vec::new(),
        entry: image.entry.entity_id,
        schema_epoch: epoch(),
        state_root: root.root,
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        admitted_limits: generous_limits(),
        gate_operation_count: gate.operation_count(),
        gate_bridge_uses: gate.bridge_uses(),
        gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
    };
    let closure = V2Closure {
        types: &image.types,
        schema_epoch: epoch(),
        state_root: root.root,
        entry: image.entry.entity_id,
        functions: &executable_functions,
        parameters: &image.parameters,
        blocks: &image.blocks,
        operations: &image.operations,
        adapters: &[],
        constants: &image.constants,
        globals: &[],
        contracts: &[],
    };
    let (digests, receipt, report) =
        sley_vm::admit_v2_package(&closure, &package).expect("bounded driver authority admission");
    let approved = approve_package_v2(&package, &digests, receipt, &report)
        .expect("bounded driver authority approval");
    (package, approved)
}

fn run_driver(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    object_closure: &[u8],
    field: u8,
    replacement: &[u8],
) -> sley_vm::ExecutionOutcome {
    let mut manifest = build_manifest_value(
        object_closure,
        package.state_root.as_bytes(),
        &approved.package_digest,
    );
    let ConstData::Record(record) = &mut manifest.data else {
        unreachable!("build manifest is a record")
    };
    record
        .fields
        .iter_mut()
        .find(|value| value.member_id == member(field))
        .expect("selected dependency-pin field is present")
        .value = bytes_value(replacement);
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![manifest],
            limits: generous_limits(),
        },
    )
    .expect("bounded build driver executes")
}

fn success_value(outcome: &sley_vm::ExecutionOutcome) -> &ConstValue {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("bounded driver must return a typed result")
    };
    value
}

#[test]
fn bounded_driver_assembles_a_typed_non_fixed_point_result() {
    let image = bounded_driver_graph();
    let (package, approved) = admit_bounded_driver(&image);
    for closure in [b"closure-a".as_slice(), b"\0closure-b\xff".as_slice()] {
        let outcome = run_driver(&package, &approved, closure, 0xA3, &BOOTSTRAP_PROFILE_V2);
        assert_eq!(
            success_value(&outcome),
            &bounded_built_result(closure, &approved.package_digest)
        );
    }
    assert_ne!(
        success_value(&run_driver(
            &package,
            &approved,
            b"closure-a",
            0xA3,
            &BOOTSTRAP_PROFILE_V2,
        )),
        success_value(&run_driver(
            &package,
            &approved,
            b"closure-b",
            0xA3,
            &BOOTSTRAP_PROFILE_V2,
        ))
    );
}

#[test]
fn bounded_driver_refuses_each_changed_pin_with_typed_dependency_error() {
    let image = bounded_driver_graph();
    let (package, approved) = admit_bounded_driver(&image);
    for field in 0xA3..=0xA7 {
        let bad_pin = [field; 32];
        let outcome = run_driver(&package, &approved, b"closure", field, &bad_pin);
        let error_context = if field == 0xA3 {
            bad_pin.as_slice()
        } else {
            BOOTSTRAP_PROFILE_V2.as_slice()
        };
        assert_eq!(success_value(&outcome), &dependency_error(error_context));
    }
}

#[test]
fn bounded_driver_retains_and_executes_its_exact_test_case() {
    let image = bounded_driver_graph();
    validate_bounded_driver_test(&image);
    let objects = canonical_component_objects(&image);
    let root = canonical_component_root(&objects);
    let (package, approved) = admit_bounded_driver(&image);
    let outcome = sley_vm::execute_approved_package_v2(
        &package,
        &approved,
        sley_vm::ExecutionRequest {
            inputs: image.tests[0].inputs.clone(),
            limits: generous_limits(),
        },
    )
    .expect("retained bounded driver test executes");
    let ExpectedOutcome::Value(expected) = &image.tests[0].expected else {
        unreachable!("bounded driver test has an exact value expectation")
    };
    assert_eq!(success_value(&outcome), expected);
    let mut object_hasher = Sha256::new();
    for object in &objects {
        object_hasher.update(object.stored_bytes());
    }
    let object_bytes = objects
        .iter()
        .map(|object| object.stored_bytes().len())
        .sum::<usize>();
    let object_digest: [u8; 32] = object_hasher.finalize().into();
    let root_digest: [u8; 32] = <Sha256 as Digest>::digest(&root.stored_bytes).into();
    assert_eq!(objects.len(), 80);
    assert_eq!(object_bytes, 20_789);
    assert_eq!(
        hex(&object_digest),
        "530a6df328131c4f6cc6179af84335a1c61fbcb2b93a980ef0ea2d41d70bd3f3"
    );
    assert_eq!(
        hex(root.root.as_bytes()),
        "e4ad2b891c0ccf72c64aceda5af2cb7bac55f2201a268776006083bcae61b4d9"
    );
    assert_eq!(root.stored_bytes.len(), 5_707);
    assert_eq!(
        hex(&root_digest),
        "4c5dffd8215b09bc2af5d3319f36172e75637484a36c1be7ff985aacf77d427b"
    );
    assert_eq!(
        hex(&approved.package_digest),
        "49cf29999395d4950a09c484aea9afbd94d73ccb5e6823f7dcbb5a0056b5ddfe"
    );
    assert_eq!(package.image_bytes.len(), 3_138);
    assert_eq!(package.gate_operation_count, 34);
    eprintln!(
        "RW120_BOUNDED_DRIVER objects={} object_bytes={} object_sha256={} root={} root_bytes={} root_sha256={} package_digest={} image_bytes={} operations={}",
        objects.len(),
        object_bytes,
        hex(&object_digest),
        hex(root.root.as_bytes()),
        root.stored_bytes.len(),
        hex(&root_digest),
        hex(&approved.package_digest),
        package.image_bytes.len(),
        package.gate_operation_count,
    );
}
