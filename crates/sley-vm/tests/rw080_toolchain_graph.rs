//! RW-080 provisional aggregate toolchain graph construction.
//!
//! This is the smallest admitted closure that binds the codec, checker,
//! lowerer, package-builder, and driver surfaces together. The leaf behavior
//! remains scaffold-only; the graph is a component construction witness, not
//! the canonical state root `S` and not a compiler correctness claim.

use sha2::{Digest, Sha256};
use sley_id::{CandidateNonce, EntityId, GenesisNonce, PolicyRootId, SchemaEpochId, WorkspaceId};
use sley_mutate::{
    EntityObject, EntityObjectRecord, build_entity_object, import_entity_object,
    value::{
        BlockBody, ConstantBody, ContractBody, EntityBodyValue, EntityIdSet, EntryExposure,
        EntryPointBody, FunctionBody, NamespaceBody, OperationBody, PackageBody, ParameterBody,
        TestCaseBody, TypeDefBody, WorkspaceBody,
    },
};
use sley_ssmc::{
    Block, ConstData, ConstValue, ConstantDefinition, ContractDefinition, ContractKind,
    EffectEnvironment, ExpectedOutcome, FieldConst, FunctionGraph, FunctionRefValue, Immediate,
    IntegerWidth, MemberId, NamedType, Opcode, Operation, OperationResultRef, Parameter,
    ParameterRole, Reachability, RecordConst, RecordField, ResourceLimits, ResultConst,
    ReturnTerminator, Terminator, TestCaseDefinition, TypeDefForm, TypeDefinition, TypeExpr,
    ValueRef, VariantCase, VariantConst, Visibility,
};
use std::fmt::Write;

#[path = "rw080_toolchain_graph/rw120_driver.rs"]
mod rw120_driver;

const DRIVER: u8 = 1;
const CODEC: u8 = 2;
const CHECKER: u8 = 3;
const LOWERER: u8 = 4;
const BUILDER: u8 = 5;
const PREDICATE: u8 = 6;
const CONTRACT_TARGET: u8 = 7;

const GENESIS_SEED: [u8; 32] = [0x80; 32];
const CANDIDATE_SEED: [u8; 32] = [0x87; 32];
const BOOTSTRAP_PROFILE_V2: [u8; 32] = [
    0xfb, 0x2d, 0x8c, 0xc8, 0x7e, 0xe7, 0xde, 0x68, 0xcd, 0xe8, 0x19, 0x7a, 0x77, 0x00, 0x3a, 0x41,
    0x7a, 0x00, 0x62, 0xac, 0xb6, 0xed, 0x08, 0x7d, 0x85, 0xf8, 0x99, 0xda, 0x1a, 0x84, 0x74, 0x59,
];
const HOST_ABI_V2: [u8; 32] = [
    0xbc, 0x56, 0x46, 0x53, 0x30, 0x2a, 0x73, 0xeb, 0x5f, 0x99, 0x84, 0x27, 0x25, 0x0a, 0x2b, 0xb7,
    0xcd, 0x87, 0xf5, 0x68, 0x5e, 0xf1, 0x26, 0x19, 0xbd, 0x4a, 0xe1, 0xf1, 0xb2, 0xaf, 0x70, 0xd5,
];
const EXEC_PACKAGE_V2: [u8; 32] = [
    0xf4, 0x95, 0x8c, 0x5e, 0x3d, 0x57, 0x76, 0x21, 0x73, 0xb8, 0x81, 0x28, 0x8b, 0x00, 0x8a, 0xf1,
    0x7d, 0x45, 0xb5, 0xf0, 0x7a, 0x43, 0x1f, 0xcc, 0x44, 0x2d, 0x9e, 0xec, 0x57, 0x70, 0xda, 0x94,
];
const RAW_BLAKE3_V1: [u8; 32] = [
    0x78, 0x52, 0x05, 0xfb, 0x49, 0x49, 0x02, 0x37, 0xcb, 0xec, 0x7f, 0xfe, 0x2f, 0xc4, 0xc2, 0xb0,
    0xf9, 0x80, 0x14, 0xb9, 0xaa, 0xe7, 0x6c, 0xc5, 0x47, 0x95, 0x92, 0x1e, 0x5d, 0x96, 0x9f, 0x72,
];
const ENTITY_TOKENS: [u8; 45] = [
    1, 2, 3, 4, 5, 6, 7, 10, 11, 20, 21, 22, 23, 24, 25, 26, 30, 31, 32, 33, 40, 41, 42, 43, 44,
    45, 46, 47, 48, 49, 50, 51, 60, 61, 62, 63, 64, 65, 66, 67, 70, 71, 72, 80, 81,
];

fn workspace() -> WorkspaceId {
    WorkspaceId::derive(GenesisNonce::from_bytes(GENESIS_SEED))
}

fn candidate_nonce() -> CandidateNonce {
    CandidateNonce::from_bytes(CANDIDATE_SEED)
}

fn seed_position(byte: u8) -> (u32, u64) {
    match byte {
        1..=7 => (5, u64::from(byte - 1)),
        10..=11 => (6, u64::from(byte - 10)),
        20..=26 => (7, u64::from(byte - 20)),
        30..=33 => (9, u64::from(byte - 30)),
        40..=51 => (8, u64::from(byte - 40)),
        60 => (1, 0),
        61 => (2, 0),
        62 => (3, 0),
        63..=67 => (16, u64::from(byte - 63)),
        70..=72 => (4, u64::from(byte - 70)),
        80 => (13, 0),
        81 => (14, 0),
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

fn policy_root() -> PolicyRootId {
    PolicyRootId::from_bytes([
        0x3b, 0x8e, 0xab, 0x80, 0xac, 0xdc, 0x87, 0x4b, 0xd3, 0xf3, 0x95, 0x89, 0x81, 0xd0, 0xda,
        0x81, 0xd2, 0xce, 0x23, 0x14, 0x07, 0x3f, 0xc9, 0x77, 0x3d, 0x75, 0xed, 0x90, 0x1f, 0x0d,
        0xc8, 0x9c,
    ])
}

fn seed_owner(token: u8) -> &'static str {
    match token {
        DRIVER
        | PREDICATE
        | CONTRACT_TARGET
        | 10
        | 20
        | 25..=26
        | 33
        | 40..=44
        | 48..=51
        | 60..=63
        | 70..=72
        | 80..=81 => "driver",
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

fn bytes_vector(values: impl IntoIterator<Item = Vec<u8>>) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Vector(Box::new(TypeExpr::Bytes)),
        data: ConstData::Sequence(
            values
                .into_iter()
                .map(|value| bytes_value(&value))
                .collect(),
        ),
    }
}

fn build_manifest_value(
    object_closure: &[u8],
    state_root: &[u8; 32],
    package_digest: &[u8; 32],
) -> ConstValue {
    let entry_points = (63..=67)
        .map(|token| id(token).as_bytes().to_vec())
        .collect::<Vec<_>>();
    ConstValue {
        value_type: build_manifest_type(),
        data: ConstData::Record(RecordConst {
            definition: id(70),
            fields: vec![
                FieldConst {
                    member_id: member(0xA0),
                    value: bytes_value(object_closure),
                },
                FieldConst {
                    member_id: member(0xA1),
                    value: bytes_vector(entry_points),
                },
                FieldConst {
                    member_id: member(0xA2),
                    value: bytes_vector(vec![package_digest.to_vec()]),
                },
                FieldConst {
                    member_id: member(0xA3),
                    value: bytes_value(&BOOTSTRAP_PROFILE_V2),
                },
                FieldConst {
                    member_id: member(0xA4),
                    value: bytes_value(&HOST_ABI_V2),
                },
                FieldConst {
                    member_id: member(0xA5),
                    value: bytes_value(&EXEC_PACKAGE_V2),
                },
                FieldConst {
                    member_id: member(0xA6),
                    value: bytes_value(&RAW_BLAKE3_V1),
                },
                FieldConst {
                    member_id: member(0xA7),
                    value: bytes_value(epoch().as_bytes()),
                },
                FieldConst {
                    member_id: member(0xA8),
                    value: bytes_value(state_root),
                },
            ],
        }),
    }
}

fn construction_test_manifest_value() -> ConstValue {
    let state_root: [u8; 32] =
        <Sha256 as Digest>::digest(b"SLEY2/RW080/CONTRACT-TEST/EXPECTED-STATE/V1").into();
    let package_digest: [u8; 32] =
        <Sha256 as Digest>::digest(b"SLEY2/RW080/CONTRACT-TEST/EXPECTED-PACKAGE/V1").into();
    build_manifest_value(
        b"SLEY2/RW080/CONTRACT-TEST/OBJECT-CLOSURE/V1",
        &state_root,
        &package_digest,
    )
}

fn incomplete_build_result() -> ConstValue {
    ConstValue {
        value_type: build_result_type(),
        data: ConstData::Result(ResultConst::Err(Box::new(ConstValue {
            value_type: build_error_type(),
            data: ConstData::Variant(VariantConst {
                definition: id(72),
                member_id: member(0xCF),
                payload: None,
            }),
        }))),
    }
}

fn member(byte: u8) -> MemberId {
    MemberId::from_bytes([byte; 32])
}

fn named(token: u8) -> TypeExpr {
    TypeExpr::Named(NamedType {
        definition: id(token),
        arguments: Vec::new(),
    })
}

fn build_manifest_type() -> TypeExpr {
    named(70)
}

fn built_toolchain_type() -> TypeExpr {
    named(71)
}

fn build_error_type() -> TypeExpr {
    named(72)
}

fn build_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(built_toolchain_type()),
        error: Box::new(build_error_type()),
    }
}

fn record_field(member_id: u8, value_type: TypeExpr) -> RecordField {
    RecordField {
        member_id: member(member_id),
        value_type,
        visibility: Visibility::Private,
    }
}

fn toolchain_type_definitions() -> Vec<TypeDefinition> {
    let bytes_vector = || TypeExpr::Vector(Box::new(TypeExpr::Bytes));
    let mut definitions = vec![
        TypeDefinition {
            entity_id: id(70),
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(vec![
                record_field(0xA0, TypeExpr::Bytes),
                record_field(0xA1, bytes_vector()),
                record_field(0xA2, bytes_vector()),
                record_field(0xA3, TypeExpr::Bytes),
                record_field(0xA4, TypeExpr::Bytes),
                record_field(0xA5, TypeExpr::Bytes),
                record_field(0xA6, TypeExpr::Bytes),
                record_field(0xA7, TypeExpr::Bytes),
                record_field(0xA8, TypeExpr::Bytes),
            ]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
        TypeDefinition {
            entity_id: id(71),
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(vec![
                record_field(0xB0, bytes_vector()),
                record_field(0xB1, bytes_vector()),
                record_field(0xB2, TypeExpr::Bool),
            ]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
        TypeDefinition {
            entity_id: id(72),
            type_parameters: Vec::new(),
            form: TypeDefForm::Variant(vec![
                VariantCase {
                    member_id: member(0xC0),
                    payload_type: Some(uint(8)),
                },
                VariantCase {
                    member_id: member(0xC1),
                    payload_type: Some(uint(8)),
                },
                VariantCase {
                    member_id: member(0xC2),
                    payload_type: Some(uint(32)),
                },
                VariantCase {
                    member_id: member(0xC3),
                    payload_type: Some(TypeExpr::Bytes),
                },
                VariantCase {
                    member_id: member(0xC4),
                    payload_type: Some(TypeExpr::Bytes),
                },
                VariantCase {
                    member_id: member(0xCF),
                    payload_type: None,
                },
            ]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
    ];
    definitions.sort_unstable_by_key(|definition| definition.entity_id);
    definitions
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
    type_definitions: Vec<TypeDefinition>,
    entry: FunctionGraph,
    functions: Vec<FunctionGraph>,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
    contracts: Vec<ContractDefinition>,
    tests: Vec<TestCaseDefinition>,
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

fn toolchain_operations() -> Vec<Operation> {
    vec![
        Operation {
            entity_id: id(40),
            block: id(20),
            ordinal: 0,
            opcode: Opcode::RecordGet,
            operands: vec![ValueRef::Parameter(id(10))],
            result_types: vec![TypeExpr::Bytes],
            immediate: Immediate::Field(member(0xA0)),
        },
        direct_call(41, 1, BUILDER, vec![op_result(40)], TypeExpr::Bytes),
        direct_call(42, 2, CODEC, Vec::new(), uint(8)),
        direct_call(43, 3, CHECKER, Vec::new(), uint(8)),
        direct_call(44, 4, LOWERER, Vec::new(), uint(32)),
        Operation {
            entity_id: id(48),
            block: id(20),
            ordinal: 5,
            opcode: Opcode::VariantNew,
            operands: Vec::new(),
            result_types: vec![build_error_type()],
            immediate: Immediate::Variant(sley_ssmc::VariantImmediate {
                definition: id(72),
                member_id: member(0xCF),
            }),
        },
        Operation {
            entity_id: id(49),
            block: id(20),
            ordinal: 6,
            opcode: Opcode::ResultErr,
            operands: vec![op_result(48)],
            result_types: vec![build_result_type()],
            immediate: Immediate::None,
        },
        constant_ref(45, 21, 30, uint(8)),
        constant_ref(46, 22, 31, uint(8)),
        constant_ref(47, 23, 32, uint(32)),
        constant_ref(50, 25, 33, TypeExpr::Bool),
        constant_ref(51, 26, 33, TypeExpr::Bool),
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
        returning_block(
            20,
            DRIVER,
            [40, 41, 42, 43, 44, 48, 49].map(id).to_vec(),
            op_result(49),
        ),
        returning_block(21, CODEC, vec![id(45)], op_result(45)),
        returning_block(22, CHECKER, vec![id(46)], op_result(46)),
        returning_block(23, LOWERER, vec![id(47)], op_result(47)),
        returning_block(24, BUILDER, Vec::new(), ValueRef::Parameter(id(11))),
        returning_block(25, PREDICATE, vec![id(50)], op_result(50)),
        returning_block(26, CONTRACT_TARGET, vec![id(51)], op_result(51)),
    ]
}

fn aggregate_toolchain_graph() -> ToolchainImage {
    let type_definitions = toolchain_type_definitions();
    let result_type = build_result_type();
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
    let mut contract_target = leaf_graph(CONTRACT_TARGET, 26, TypeExpr::Bool);
    contract_target.contracts = vec![id(80)];
    let functions = vec![
        driver.clone(),
        leaf_graph(CODEC, 21, uint(8)),
        leaf_graph(CHECKER, 22, uint(8)),
        leaf_graph(LOWERER, 23, uint(32)),
        leaf_graph(BUILDER, 24, TypeExpr::Bytes),
        leaf_graph(PREDICATE, 25, TypeExpr::Bool),
        contract_target,
    ];
    let mut constants = vec![
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
        ConstantDefinition {
            entity_id: id(33),
            value: ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(true),
            },
        },
    ];
    constants.sort_unstable_by_key(|constant| constant.entity_id);
    ToolchainImage {
        types: sley_check::TypeEnvironment::new(type_definitions.clone()).unwrap(),
        type_definitions,
        entry: driver,
        functions,
        parameters: vec![
            Parameter {
                entity_id: id(10),
                owner: id(DRIVER),
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: build_manifest_type(),
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
        operations: toolchain_operations(),
        constants,
        contracts: vec![ContractDefinition {
            entity_id: id(80),
            target: id(CONTRACT_TARGET),
            contract_kind: ContractKind::Precondition,
            predicate: id(PREDICATE),
            bindings: Vec::new(),
            resource_limits: None,
        }],
        tests: vec![TestCaseDefinition {
            entity_id: id(81),
            target: id(DRIVER),
            inputs: vec![construction_test_manifest_value()],
            effect_environment: EffectEnvironment::Replay(Vec::new()),
            expected: ExpectedOutcome::Value(incomplete_build_result()),
            observations: Vec::new(),
            resource_limits: ResourceLimits {
                fuel: 200_000,
                memory_bytes: 2_000_000,
                output_bytes: 100_000,
                effect_count: 0,
                call_depth: 32,
                wall_timeout_millis: 1_000,
            },
        }],
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
    let namespace_members = entry_points
        .iter()
        .copied()
        .chain((70..=72).map(id))
        .chain([id(PREDICATE), id(CONTRACT_TARGET), id(80), id(81)])
        .collect::<Vec<_>>();
    let mut objects = vec![
        canonical_object(
            id(60),
            EntityBodyValue::Workspace(WorkspaceBody {
                packages: EntityIdSet::from_unsorted(vec![id(61)]).unwrap(),
                root_namespace: id(62),
                capability_requirements: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
                contracts: EntityIdSet::from_unsorted(vec![id(80)]).unwrap(),
                tests: EntityIdSet::from_unsorted(vec![id(81)]).unwrap(),
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
                members: EntityIdSet::from_unsorted(namespace_members).unwrap(),
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

fn canonical_contract_test_objects(image: &ToolchainImage) -> Vec<EntityObject> {
    let mut objects = image
        .contracts
        .iter()
        .map(|contract| {
            canonical_object(
                contract.entity_id,
                EntityBodyValue::Contract(ContractBody {
                    target: contract.target,
                    contract_kind: contract.contract_kind,
                    predicate: contract.predicate,
                    bindings: contract.bindings.clone(),
                    resource_limits: contract.resource_limits,
                }),
            )
        })
        .collect::<Vec<_>>();
    objects.extend(image.tests.iter().map(|test| {
        canonical_object(
            test.entity_id,
            EntityBodyValue::TestCase(TestCaseBody {
                target: test.target,
                inputs: test.inputs.clone(),
                effect_environment: test.effect_environment.clone(),
                expected: test.expected.clone(),
                observations: test.observations.clone(),
                resource_limits: test.resource_limits,
            }),
        )
    }));
    objects
}

fn canonical_component_objects(image: &ToolchainImage) -> Vec<EntityObject> {
    let mut objects = Vec::new();
    objects.extend(image.type_definitions.iter().map(|definition| {
        canonical_object(
            definition.entity_id,
            EntityBodyValue::TypeDef(TypeDefBody {
                type_parameters: definition.type_parameters.clone(),
                form: definition.form.clone(),
                invariants: EntityIdSet::from_unsorted(definition.invariants.clone())
                    .expect("aggregate invariants are unique"),
                visibility: definition.visibility,
            }),
        )
    }));
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
    objects.extend(canonical_contract_test_objects(image));
    objects.extend(canonical_component_metadata());
    objects.sort_unstable_by_key(|object| object.record().entity_id);
    objects
}

fn canonical_component_root(objects: &[EntityObject]) -> sley_state_root::AcceptedStateRoot {
    let anchor = |token| {
        objects
            .iter()
            .find(|object| object.record().entity_id == id(token))
            .expect("semantic root anchor is retained")
            .object_id()
    };
    let mut builder =
        sley_state_root::StateRootBuilder::new(workspace(), anchor(80), anchor(81), policy_root());
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
    let executable_functions = image
        .functions
        .iter()
        .filter(|function| {
            !matches!(function.entity_id, value if value == id(PREDICATE) || value == id(CONTRACT_TARGET))
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
        state_root: component_root.root,
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &image.constants,
        globals: &[],
        functions: &executable_functions,
        contracts: &[],
        adapters: &[],
    })
    .expect("aggregate graph lowers");
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
    .expect("aggregate graph admits under V2");
    let package = ExecutionPackage {
        image_bytes: lowered.bytes,
        constants: image.constants.clone(),
        type_definitions: image.type_definitions.clone(),
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
        sley_vm::admit_v2_package(&closure, &package).expect("authority admits aggregate graph");
    let approved = approve_package_v2(&package, &digests, receipt, &report)
        .expect("authority approves aggregate graph");
    (package, approved)
}

fn execute_driver(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    object_closure: &[u8],
) -> sley_vm::ExecutionOutcome {
    let manifest = build_manifest_value(
        object_closure,
        package.state_root.as_bytes(),
        &approved.package_digest,
    );
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![manifest],
            limits: generous_limits(),
        },
    )
    .expect("aggregate driver executes")
}

fn assert_driver_result(outcome: &sley_vm::ExecutionOutcome) {
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "aggregate driver must return, got {:?}",
            outcome.termination
        );
    };
    assert_eq!(*value, incomplete_build_result());
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
        assert_driver_result(&outcome);
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
fn aggregate_driver_uses_typed_manifest_result_and_error_contract() {
    let image = aggregate_toolchain_graph();
    assert_eq!(image.entry.parameters, vec![id(10)]);
    assert_eq!(image.parameters[0].value_type, build_manifest_type());
    assert_eq!(
        image.entry.result_type,
        TypeExpr::Result {
            ok: Box::new(built_toolchain_type()),
            error: Box::new(build_error_type()),
        }
    );
    assert_eq!(image.type_definitions, toolchain_type_definitions());
    let operation = |token| {
        image
            .operations
            .iter()
            .find(|operation| operation.entity_id == id(token))
            .expect("driver operation exists")
    };
    assert_eq!(operation(40).opcode, Opcode::RecordGet);
    assert_eq!(operation(40).immediate, Immediate::Field(member(0xA0)));
    assert_eq!(operation(41).opcode, Opcode::CallDirect);
    assert_eq!(operation(41).operands, vec![op_result(40)]);
    assert_eq!(operation(48).opcode, Opcode::VariantNew);
    assert_eq!(operation(49).opcode, Opcode::ResultErr);
    let identity = [0xD1; 32];
    image
        .types
        .check_constant(&build_manifest_value(
            b"typed-closure",
            &identity,
            &identity,
        ))
        .expect("typed build manifest is a canonical value");
    image
        .types
        .check_constant(&incomplete_build_result())
        .expect("typed incomplete BuildError is a canonical result");
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
    for token in 70..=72 {
        assert!(matches!(
            component_body(objects, token),
            EntityBodyValue::TypeDef(_)
        ));
    }
    assert!(matches!(
        component_body(objects, 80),
        EntityBodyValue::Contract(_)
    ));
    assert!(matches!(
        component_body(objects, 81),
        EntityBodyValue::TestCase(_)
    ));
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
    let contract_anchor = objects
        .iter()
        .find(|object| object.record().entity_id == id(80))
        .expect("contract anchor object exists");
    let test_anchor = objects
        .iter()
        .find(|object| object.record().entity_id == id(81))
        .expect("test anchor object exists");
    assert_eq!(root.record.contract_root, contract_anchor.object_id());
    assert_eq!(root.record.test_root, test_anchor.object_id());
    assert_eq!(root.record.policy_root, policy_root());
    assert_component_metadata(&objects);
    assert_eq!(
        root.root.into_bytes(),
        [
            0x76, 0xa9, 0xdd, 0xd6, 0x0e, 0xba, 0x89, 0x9a, 0xf6, 0x4c, 0xd8, 0x8f, 0x26, 0xe4,
            0xcf, 0x24, 0xe5, 0x36, 0xc8, 0xd0, 0x01, 0xe0, 0xc0, 0xa3, 0x0f, 0xa4, 0x67, 0x8f,
            0xc0, 0xb2, 0x17, 0x2a,
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
            "RW080_MANIFEST_META workspace={} genesis_seed={} candidate_seed={} epoch={} contract_root={} contract_token=80 test_root={} test_token=81 policy_root={} state_root={} state_root_bytes={}",
            hex(workspace().as_bytes()),
            hex(&GENESIS_SEED),
            hex(&CANDIDATE_SEED),
            hex(epoch().as_bytes()),
            hex(contract_anchor.object_id().as_bytes()),
            hex(test_anchor.object_id().as_bytes()),
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
            0xe0, 0xaf, 0x16, 0x83, 0x32, 0x00, 0x65, 0xb9, 0x3f, 0x67, 0xc2, 0x35, 0x33, 0x7d,
            0x14, 0xe3, 0xa6, 0xd8, 0x82, 0xc7, 0x20, 0x1e, 0x93, 0x69, 0x7e, 0x5d, 0x87, 0x0c,
            0x83, 0x27, 0xea, 0xb6,
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
    let image = aggregate_toolchain_graph();
    assert_eq!(decoded.entry, id(DRIVER));
    assert_eq!(decoded.schema_epoch, epoch());
    assert_eq!(
        sley_vm::decode_layouts_section(&decoded.layouts_bytes).expect("layouts decode"),
        image.type_definitions
    );
    let expected_root = canonical_component_root(&canonical_component_objects(&image));
    assert_eq!(decoded.state_root, expected_root.root);
    assert_eq!(decoded.digests, digests);
    assert_eq!(package.gate_bridge_uses, 0);
    assert_eq!(package.gate_closure_fingerprints.len(), 5);
    assert_eq!(
        digests.package_digest,
        [
            0xa9, 0x3c, 0xb6, 0xf6, 0x73, 0x5a, 0x81, 0x66, 0x62, 0x05, 0xc3, 0xa4, 0x20, 0x50,
            0x9a, 0x68, 0xee, 0x17, 0xef, 0x36, 0x59, 0xee, 0xde, 0xae, 0x21, 0x65, 0x5a, 0xe1,
            0x24, 0xec, 0xef, 0x93,
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

#[test]
fn aggregate_contract_and_test_roots_are_semantic_retained_objects() {
    struct OwnedUnit {
        function: FunctionGraph,
        parameters: Vec<Parameter>,
        blocks: Vec<Block>,
        operations: Vec<Operation>,
    }

    let image = aggregate_toolchain_graph();
    assert_eq!(image.contracts.len(), 1);
    assert_eq!(image.tests.len(), 1);
    let mut owned_units = image
        .functions
        .iter()
        .map(|function| {
            let blocks = image
                .blocks
                .iter()
                .filter(|block| block.function == function.entity_id)
                .cloned()
                .collect::<Vec<_>>();
            let block_ids = blocks
                .iter()
                .map(|block| block.entity_id)
                .collect::<Vec<_>>();
            OwnedUnit {
                function: function.clone(),
                parameters: image
                    .parameters
                    .iter()
                    .filter(|parameter| parameter.owner == function.entity_id)
                    .cloned()
                    .collect(),
                blocks,
                operations: image
                    .operations
                    .iter()
                    .filter(|operation| block_ids.contains(&operation.block))
                    .cloned()
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    owned_units.sort_unstable_by_key(|unit| unit.function.entity_id);
    let units = owned_units
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
        &units,
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
    .expect("retained contract and test pass the canonical semantic validator");
    assert_eq!(report.contracts, vec![id(80)]);
    assert_eq!(report.tests, vec![id(81)]);
    assert_eq!(report.selected_tests, vec![id(81)]);
    assert_eq!(
        report.selection_finality,
        sley_check::contracts::TestPlanFinality::PolicyIncomplete
    );

    let objects = canonical_component_objects(&image);
    let root = canonical_component_root(&objects);
    let contract_object = objects
        .iter()
        .find(|object| object.record().entity_id == id(80))
        .expect("contract root object is retained");
    let test_object = objects
        .iter()
        .find(|object| object.record().entity_id == id(81))
        .expect("test root object is retained");
    assert_eq!(root.record.contract_root, contract_object.object_id());
    assert_eq!(root.record.test_root, test_object.object_id());

    let (package, approved) = admitted_toolchain_graph();
    let outcome = sley_vm::execute_approved_package_v2(
        &package,
        &approved,
        sley_vm::ExecutionRequest {
            inputs: image.tests[0].inputs.clone(),
            limits: generous_limits(),
        },
    )
    .expect("the retained test case executes");
    assert_driver_result(&outcome);
}
