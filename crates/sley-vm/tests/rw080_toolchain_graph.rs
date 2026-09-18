//! RW-080 provisional aggregate toolchain graph construction.
//!
//! This is the smallest admitted closure that binds the codec, checker,
//! lowerer, package-builder, and driver surfaces together. The leaf behavior
//! remains scaffold-only; the graph is a component construction witness, not
//! the canonical state root `S` and not a compiler correctness claim.

use sha2::{Digest, Sha256};
use sley_id::{
    CandidateNonce, EntityId, GenesisNonce, ObjectId, PolicyRootId, SchemaEpochId, WorkspaceId,
};
use sley_mutate::{
    EntityObject, EntityObjectRecord, build_entity_object, import_entity_object,
    value::{
        BlockBody, ConstantBody, EntityBodyValue, EntityIdSet, EntryExposure, EntryPointBody,
        FunctionBody, NamespaceBody, OperationBody, PackageBody, ParameterBody, WorkspaceBody,
    },
};
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

const GENESIS_SEED: [u8; 32] = [0x80; 32];
const CANDIDATE_SEED: [u8; 32] = [0x87; 32];
const CONTRACT_ROOT_PREIMAGE: &[u8] = b"SLEY2/RW080/PARTIAL/EMPTY-CONTRACT-ROOT/V1";
const TEST_ROOT_PREIMAGE: &[u8] = b"SLEY2/RW080/PARTIAL/EMPTY-TEST-ROOT/V1";
const ENTITY_TOKENS: [u8; 31] = [
    1, 2, 3, 4, 5, 10, 11, 20, 21, 22, 23, 24, 30, 31, 32, 40, 41, 42, 43, 44, 45, 46, 47, 60, 61,
    62, 63, 64, 65, 66, 67,
];

fn workspace() -> WorkspaceId {
    WorkspaceId::derive(GenesisNonce::from_bytes(GENESIS_SEED))
}

fn candidate_nonce() -> CandidateNonce {
    CandidateNonce::from_bytes(CANDIDATE_SEED)
}

fn seed_position(byte: u8) -> (u32, u64) {
    match byte {
        1..=5 => (5, u64::from(byte - 1)),
        10..=11 => (6, u64::from(byte - 10)),
        20..=24 => (7, u64::from(byte - 20)),
        30..=32 => (9, u64::from(byte - 30)),
        40..=47 => (8, u64::from(byte - 40)),
        60 => (1, 0),
        61 => (2, 0),
        62 => (3, 0),
        63..=67 => (16, u64::from(byte - 63)),
        _ => panic!("unknown aggregate seed-local identity {byte}"),
    }
}

fn id(byte: u8) -> EntityId {
    let (kind, ordinal) = seed_position(byte);
    EntityId::derive(workspace(), candidate_nonce(), kind, ordinal)
}

fn epoch() -> SchemaEpochId {
    sley_state_root::conformance_epoch_id().expect("state-root conformance epoch is frozen")
}

fn contract_root() -> ObjectId {
    ObjectId::derive(CONTRACT_ROOT_PREIMAGE)
}

fn test_root() -> ObjectId {
    ObjectId::derive(TEST_ROOT_PREIMAGE)
}

fn policy_root() -> PolicyRootId {
    PolicyRootId::from_bytes([
        0x3b, 0x8e, 0xab, 0x80, 0xac, 0xdc, 0x87, 0x4b, 0xd3, 0xf3, 0x95, 0x89, 0x81, 0xd0, 0xda,
        0x81, 0xd2, 0xce, 0x23, 0x14, 0x07, 0x3f, 0xc9, 0x77, 0x3d, 0x75, 0xed, 0x90, 0x1f, 0x0d,
        0xc8, 0x9c,
    ])
}

fn seed_owner(token: u8) -> &'static str {
    match token {
        DRIVER | 10 | 20 | 40..=44 | 60..=63 => "driver",
        CODEC | 21 | 30 | 45 | 64 => "codec",
        CHECKER | 22 | 31 | 46 | 65 => "checker",
        LOWERER | 23 | 32 | 47 | 66 => "lowerer",
        BUILDER | 11 | 24 | 67 => "package-builder",
        _ => panic!("unknown aggregate owner token {token}"),
    }
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

fn canonical_object(entity_id: EntityId, body: EntityBodyValue) -> EntityObject {
    build_entity_object(
        epoch(),
        &EntityObjectRecord {
            entity_id,
            body,
            label: None,
            semantic_fingerprint: None,
        },
    )
    .expect("aggregate entity object is canonical")
}

fn canonical_component_metadata() -> Vec<EntityObject> {
    let entry_points = (63..=67).map(id).collect::<Vec<_>>();
    let mut objects = vec![
        canonical_object(
            id(60),
            EntityBodyValue::Workspace(WorkspaceBody {
                packages: EntityIdSet::from_unsorted(vec![id(61)]).unwrap(),
                root_namespace: id(62),
                capability_requirements: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
                contracts: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
                tests: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
            }),
        ),
        canonical_object(
            id(61),
            EntityBodyValue::Package(PackageBody {
                workspace: id(60),
                root_namespace: id(62),
                dependencies: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
                exports: EntityIdSet::from_unsorted(entry_points.clone()).unwrap(),
            }),
        ),
        canonical_object(
            id(62),
            EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: EntityIdSet::from_unsorted(entry_points).unwrap(),
            }),
        ),
    ];
    for (index, function) in [DRIVER, CODEC, CHECKER, LOWERER, BUILDER]
        .into_iter()
        .enumerate()
    {
        objects.push(canonical_object(
            id(63 + u8::try_from(index).expect("five entry points fit u8")),
            EntityBodyValue::EntryPoint(EntryPointBody {
                function: id(function),
                exposure: if function == DRIVER {
                    EntryExposure::Protocol
                } else {
                    EntryExposure::Local
                },
            }),
        ));
    }
    objects
}

fn canonical_component_objects(image: &ToolchainImage) -> Vec<EntityObject> {
    let mut objects = Vec::new();
    objects.extend(image.functions.iter().map(|function| {
        canonical_object(
            function.entity_id,
            EntityBodyValue::Function(FunctionBody {
                type_parameters: function.type_parameters.clone(),
                parameters: function.parameters.clone(),
                result_type: function.result_type.clone(),
                effects: EntityIdSet::from_unsorted(function.effects.clone())
                    .expect("aggregate effects are unique"),
                entry_block: function.entry_block,
                blocks: function.blocks.clone(),
                contracts: EntityIdSet::from_unsorted(function.contracts.clone())
                    .expect("aggregate contracts are unique"),
                visibility: function.visibility,
            }),
        )
    }));
    objects.extend(image.parameters.iter().map(|parameter| {
        canonical_object(
            parameter.entity_id,
            EntityBodyValue::Parameter(ParameterBody {
                owner: parameter.owner,
                role: parameter.role,
                ordinal: parameter.ordinal,
                value_type: parameter.value_type.clone(),
            }),
        )
    }));
    objects.extend(image.blocks.iter().map(|block| {
        canonical_object(
            block.entity_id,
            EntityBodyValue::Block(BlockBody {
                function: block.function,
                parameters: block.parameters.clone(),
                operations: block.operations.clone(),
                terminator: block.terminator.clone(),
                reachability: block.reachability,
            }),
        )
    }));
    objects.extend(image.operations.iter().map(|operation| {
        canonical_object(
            operation.entity_id,
            EntityBodyValue::Operation(OperationBody {
                block: operation.block,
                ordinal: operation.ordinal,
                opcode: operation.opcode.tag(),
                operands: operation.operands.clone(),
                result_types: operation.result_types.clone(),
                immediate: operation.immediate.clone(),
            }),
        )
    }));
    objects.extend(image.constants.iter().map(|constant| {
        canonical_object(
            constant.entity_id,
            EntityBodyValue::Constant(ConstantBody {
                value: constant.value.clone(),
            }),
        )
    }));
    objects.extend(canonical_component_metadata());
    objects.sort_unstable_by_key(|object| object.record().entity_id);
    objects
}

fn canonical_component_root(objects: &[EntityObject]) -> sley_state_root::AcceptedStateRoot {
    let mut builder = sley_state_root::StateRootBuilder::new(
        workspace(),
        contract_root(),
        test_root(),
        policy_root(),
    );
    for object in objects {
        builder = builder.entity_binding(object.record().entity_id, object.object_id());
    }
    for entry in 63..=67 {
        builder = builder.entry_point(id(entry));
    }
    builder
        .build(&sley_state_root::conformance_registry().expect("state-root registry is frozen"))
        .expect("aggregate component root is canonical")
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
    let objects = canonical_component_objects(&image);
    let component_root = canonical_component_root(&objects);
    let lowered = sley_vm::lower_function(sley_vm::LoweringInput {
        types: &image.types,
        function: &image.entry,
        parameters: &image.parameters,
        blocks: &image.blocks,
        operations: &image.operations,
        schema_epoch: epoch(),
        state_root: component_root.root,
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
        state_root: component_root.root,
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        admitted_limits: generous_limits(),
        gate_operation_count: gate.operation_count(),
        gate_bridge_uses: gate.bridge_uses(),
        gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
    };
    let closure = V2Closure {
        types: &image.types,
        schema_epoch: epoch(),
        state_root: component_root.root,
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
fn aggregate_entities_use_contract_derived_identities() {
    for token in ENTITY_TOKENS {
        let (kind, ordinal) = seed_position(token);
        assert_eq!(
            id(token),
            EntityId::derive(workspace(), candidate_nonce(), kind, ordinal)
        );
    }
}

fn component_body(objects: &[EntityObject], token: u8) -> &EntityBodyValue {
    &objects
        .iter()
        .find(|object| object.record().entity_id == id(token))
        .expect("component object exists")
        .record()
        .body
}

fn assert_component_metadata(objects: &[EntityObject]) {
    assert!(matches!(
        component_body(objects, 60),
        EntityBodyValue::Workspace(_)
    ));
    assert!(matches!(
        component_body(objects, 61),
        EntityBodyValue::Package(_)
    ));
    assert!(matches!(
        component_body(objects, 62),
        EntityBodyValue::Namespace(_)
    ));
    for token in 63..=67 {
        assert!(matches!(
            component_body(objects, token),
            EntityBodyValue::EntryPoint(_)
        ));
    }
}

#[test]
fn aggregate_component_objects_and_state_root_round_trip_exactly() {
    let image = aggregate_toolchain_graph();
    let objects = canonical_component_objects(&image);
    let root = canonical_component_root(&objects);
    assert_eq!(objects.len(), ENTITY_TOKENS.len());
    for object in &objects {
        assert_eq!(
            import_entity_object(epoch(), object.stored_bytes()).expect("object reimports"),
            *object
        );
    }
    let registry = sley_state_root::conformance_registry().expect("state-root registry is frozen");
    assert_eq!(
        sley_state_root::import_state_root(&registry, &root.stored_bytes)
            .expect("component root reimports"),
        root
    );
    let expected_bindings = objects
        .iter()
        .map(|object| (object.record().entity_id, object.object_id()))
        .collect::<Vec<_>>();
    assert_eq!(root.record.entity_bindings, expected_bindings);
    let mut expected_entry_points = (63..=67).map(id).collect::<Vec<_>>();
    expected_entry_points.sort_unstable();
    assert_eq!(root.record.entry_points, expected_entry_points);
    assert!(root.record.dependency_roots.is_empty());
    assert!(root.record.interpretation_flags.is_empty());
    assert_eq!(root.record.workspace_id, workspace());
    assert_eq!(root.record.schema_epoch_id, epoch());
    assert_eq!(root.record.contract_root, contract_root());
    assert_eq!(root.record.test_root, test_root());
    assert_eq!(root.record.policy_root, policy_root());
    assert_component_metadata(&objects);
    assert_eq!(
        root.root.into_bytes(),
        [
            0x39, 0xb9, 0x1d, 0xd8, 0x4a, 0x4a, 0x4e, 0xd1, 0x41, 0x4b, 0x03, 0x4b, 0x52, 0x28,
            0x33, 0xd5, 0xf0, 0x6c, 0x12, 0xb3, 0x5d, 0x54, 0xbd, 0x46, 0xec, 0xfc, 0x53, 0x1d,
            0x52, 0xd4, 0xd3, 0x0d,
        ],
        "the partial component root is pinned"
    );
    println!(
        "rw080_component objects={} object_bytes={} root_bytes={} workspace={} epoch={} root={}",
        objects.len(),
        objects
            .iter()
            .map(|object| object.stored_bytes().len())
            .sum::<usize>(),
        root.stored_bytes.len(),
        hex(workspace().as_bytes()),
        hex(epoch().as_bytes()),
        hex(root.root.as_bytes()),
    );
    if std::env::var_os("SLEY_EMIT_RW080_COMPONENT_MANIFEST").is_some() {
        println!(
            "RW080_MANIFEST_META workspace={} genesis_seed={} candidate_seed={} epoch={} contract_root={} contract_preimage={} test_root={} test_preimage={} policy_root={} state_root={} state_root_bytes={}",
            hex(workspace().as_bytes()),
            hex(&GENESIS_SEED),
            hex(&CANDIDATE_SEED),
            hex(epoch().as_bytes()),
            hex(contract_root().as_bytes()),
            hex(CONTRACT_ROOT_PREIMAGE),
            hex(test_root().as_bytes()),
            hex(TEST_ROOT_PREIMAGE),
            hex(policy_root().as_bytes()),
            hex(root.root.as_bytes()),
            hex(&root.stored_bytes),
        );
        for token in ENTITY_TOKENS {
            let (kind, ordinal) = seed_position(token);
            let object = objects
                .iter()
                .find(|object| object.record().entity_id == id(token))
                .expect("every seed identity has one retained object");
            println!(
                "RW080_MANIFEST_OBJECT token={token} owner={} kind={kind} ordinal={ordinal} entity_id={} object_id={} stored_bytes={}",
                seed_owner(token),
                hex(object.record().entity_id.as_bytes()),
                hex(object.object_id().as_bytes()),
                hex(object.stored_bytes()),
            );
        }
    }
}

#[test]
fn aggregate_component_root_changes_with_semantic_input() {
    let base = aggregate_toolchain_graph();
    let base_objects = canonical_component_objects(&base);
    let base_root = canonical_component_root(&base_objects);
    let mut changed = aggregate_toolchain_graph();
    changed.constants[0].value = uint_value(8, 7);
    let changed_objects = canonical_component_objects(&changed);
    let changed_root = canonical_component_root(&changed_objects);
    assert_ne!(changed_root.root, base_root.root);
    let changed_bindings = base_objects
        .iter()
        .zip(&changed_objects)
        .filter(|(left, right)| left.object_id() != right.object_id())
        .collect::<Vec<_>>();
    assert_eq!(changed_bindings.len(), 1);
    assert_eq!(changed_bindings[0].0.record().entity_id, id(30));
}

#[test]
fn retained_component_manifest_is_digest_pinned() {
    let bytes = include_bytes!(
        "../../../machineresearch/sley-2.0/reweave/rw-080-toolchain-component-manifest.json"
    );
    let manifest = std::str::from_utf8(bytes).expect("component manifest is UTF-8 JSON");
    let digest: [u8; 32] = <Sha256 as Digest>::digest(bytes).into();
    assert_eq!(
        digest,
        [
            0xce, 0xa0, 0x72, 0xa2, 0xa2, 0x7e, 0x63, 0x73, 0x4a, 0x33, 0x6e, 0x6e, 0xa6, 0x3b,
            0xf5, 0x7a, 0x20, 0xfa, 0x0f, 0xbf, 0x3c, 0x1c, 0xf8, 0x9e, 0x6b, 0x78, 0xf3, 0xa5,
            0xc4, 0x0d, 0x5c, 0x97,
        ]
    );
    assert!(manifest.contains("\"is_canonical_s\": false"));
    assert!(manifest.contains("\"complete_toolchain_closure\": false"));
    assert!(manifest.contains(&hex(workspace().as_bytes())));
    assert!(manifest.contains(&hex(epoch().as_bytes())));
    let image = aggregate_toolchain_graph();
    let objects = canonical_component_objects(&image);
    let root = canonical_component_root(&objects);
    assert!(manifest.contains(&hex(root.root.as_bytes())));
    let root_sha256: [u8; 32] = <Sha256 as Digest>::digest(&root.stored_bytes).into();
    assert!(manifest.contains(&hex(&root_sha256)));
    for object in &objects {
        assert!(manifest.contains(&hex(object.record().entity_id.as_bytes())));
        assert!(manifest.contains(&hex(object.object_id().as_bytes())));
        let object_sha256: [u8; 32] = <Sha256 as Digest>::digest(object.stored_bytes()).into();
        assert!(manifest.contains(&hex(&object_sha256)));
    }
    let (package, _) = admitted_toolchain_graph();
    let package_digest = sley_vm::package_digests_v2(&package)
        .expect("component package digests")
        .package_digest;
    assert!(manifest.contains(&hex(&package_digest)));
}

#[test]
fn aggregate_toolchain_package_has_stable_component_identity() {
    let (package, _) = admitted_toolchain_graph();
    let envelope = sley_vm::encode_package_envelope_v2(&package).expect("package encodes");
    let decoded = sley_vm::decode_package_envelope_v2(&envelope).expect("package decodes");
    let digests = sley_vm::package_digests_v2(&package).expect("package digests");
    assert_eq!(decoded.entry, id(DRIVER));
    assert_eq!(decoded.schema_epoch, epoch());
    let image = aggregate_toolchain_graph();
    let expected_root = canonical_component_root(&canonical_component_objects(&image));
    assert_eq!(decoded.state_root, expected_root.root);
    assert_eq!(decoded.digests, digests);
    assert_eq!(package.gate_bridge_uses, 0);
    assert_eq!(package.gate_closure_fingerprints.len(), 5);
    assert_eq!(
        digests.package_digest,
        [
            0xee, 0xc3, 0xcc, 0x89, 0x41, 0xa7, 0x59, 0xd8, 0x38, 0x67, 0x9d, 0xe0, 0xea, 0xb9,
            0xbd, 0x10, 0x41, 0xdc, 0x19, 0x37, 0x4c, 0xc1, 0x50, 0x41, 0xc4, 0xe5, 0xb7, 0x98,
            0x80, 0x54, 0x4d, 0xed,
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
