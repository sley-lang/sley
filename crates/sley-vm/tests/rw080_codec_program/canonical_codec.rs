//! Canonical construction identities and object materialization for the
//! executable four-leg codec graph.

use super::*;
use sley_id::{CandidateNonce, GenesisNonce, PolicyRootId, WorkspaceId};
use sley_mutate::{
    EntityObject, EntityObjectRecord, build_entity_object, import_entity_object,
    value::{
        AdapterImportBody, BlockBody, ConstantBody, ContractBody, EntityBodyValue, EntityIdSet,
        EntryPointBody, FunctionBody, NamespaceBody, OperationBody, PackageBody, ParameterBody,
        TestCaseBody, WorkspaceBody,
    },
};
use sley_ssmc::{
    ContractDefinition, ContractKind, EffectEnvironment, ExpectedOutcome, ResourceLimits,
    TestCaseDefinition,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

const GENESIS_SEED: [u8; 32] = [0x80; 32];
const CANDIDATE_SEED: [u8; 32] = [0x87; 32];
const FUNCTION_BASE: u64 = 1_000;
const PARAMETER_BASE: u64 = 1_000;
const BLOCK_BASE: u64 = 1_000;
const OPERATION_BASE: u64 = 1_000;
const CONSTANT_BASE: u64 = 1_000;
const ENTRY_POINT_ORDINAL: u64 = 5;
const WORKSPACE_ORDINAL: u64 = 1;
const PACKAGE_ORDINAL: u64 = 1;
const NAMESPACE_ORDINAL: u64 = 1;
const CONTRACT_ORDINAL: u64 = 1;
const TEST_ORDINAL: u64 = 1;
const WITNESS_FUNCTION_BASE: u64 = 10_000;
const WITNESS_BLOCK_BASE: u64 = 10_000;
const WITNESS_OPERATION_BASE: u64 = 10_000;
const WITNESS_CONSTANT_BASE: u64 = 10_000;

fn construction_workspace() -> WorkspaceId {
    WorkspaceId::derive(GenesisNonce::from_bytes(GENESIS_SEED))
}

fn construction_candidate() -> CandidateNonce {
    CandidateNonce::from_bytes(CANDIDATE_SEED)
}

fn derived_id(kind: u32, ordinal: u64) -> EntityId {
    EntityId::derive(
        construction_workspace(),
        construction_candidate(),
        kind,
        ordinal,
    )
}

fn source_epoch() -> SchemaEpochId {
    sley_state_root::conformance_epoch_id().expect("source schema epoch is frozen")
}

fn policy_root() -> PolicyRootId {
    PolicyRootId::from_bytes([
        0x3b, 0x8e, 0xab, 0x80, 0xac, 0xdc, 0x87, 0x4b, 0xd3, 0xf3, 0x95, 0x89, 0x81, 0xd0, 0xda,
        0x81, 0xd2, 0xce, 0x23, 0x14, 0x07, 0x3f, 0xc9, 0x77, 0x3d, 0x75, 0xed, 0x90, 0x1f, 0x0d,
        0xc8, 0x9c,
    ])
}

fn mapped(ids: &BTreeMap<EntityId, EntityId>, entity: EntityId) -> EntityId {
    ids.get(&entity).copied().unwrap_or(entity)
}

fn remap_type(value: &mut TypeExpr, ids: &BTreeMap<EntityId, EntityId>) {
    match value {
        TypeExpr::Tuple(items) => items.iter_mut().for_each(|item| remap_type(item, ids)),
        TypeExpr::Named(named) => {
            named.definition = mapped(ids, named.definition);
            named
                .arguments
                .iter_mut()
                .for_each(|argument| remap_type(argument, ids));
        }
        TypeExpr::Vector(item) | TypeExpr::Option(item) | TypeExpr::LocalCell(item) => {
            remap_type(item, ids);
        }
        TypeExpr::OrderedMap { key, value } => {
            remap_type(key, ids);
            remap_type(value, ids);
        }
        TypeExpr::Result { ok, error } => {
            remap_type(ok, ids);
            remap_type(error, ids);
        }
        TypeExpr::FunctionRef(reference) => {
            reference
                .parameters
                .iter_mut()
                .for_each(|parameter| remap_type(parameter, ids));
            remap_type(&mut reference.result, ids);
            reference
                .effects
                .iter_mut()
                .for_each(|effect| *effect = mapped(ids, *effect));
        }
        TypeExpr::AdapterHandle(entity) | TypeExpr::CapabilityToken(entity) => {
            *entity = mapped(ids, *entity);
        }
        TypeExpr::Unit
        | TypeExpr::Bool
        | TypeExpr::SInt(_)
        | TypeExpr::UInt(_)
        | TypeExpr::F32
        | TypeExpr::F64
        | TypeExpr::Bytes
        | TypeExpr::Text
        | TypeExpr::TypeParameter(_)
        | TypeExpr::BuiltinFailure(_) => {}
    }
}

fn remap_constant(value: &mut ConstValue, ids: &BTreeMap<EntityId, EntityId>) {
    remap_type(&mut value.value_type, ids);
    match &mut value.data {
        ConstData::Sequence(items) => items.iter_mut().for_each(|item| remap_constant(item, ids)),
        ConstData::Record(record) => {
            record.definition = mapped(ids, record.definition);
            record
                .fields
                .iter_mut()
                .for_each(|field| remap_constant(&mut field.value, ids));
        }
        ConstData::Variant(variant) => {
            variant.definition = mapped(ids, variant.definition);
            if let Some(payload) = &mut variant.payload {
                remap_constant(payload, ids);
            }
        }
        ConstData::Map(entries) => entries.iter_mut().for_each(|entry| {
            remap_constant(&mut entry.key, ids);
            remap_constant(&mut entry.value, ids);
        }),
        ConstData::Option(value) => {
            if let Some(value) = value {
                remap_constant(value, ids);
            }
        }
        ConstData::Result(result) => match result {
            ResultConst::Ok(value) | ResultConst::Err(value) => remap_constant(value, ids),
        },
        ConstData::FunctionRef(reference) => {
            reference.function = mapped(ids, reference.function);
            reference
                .type_arguments
                .iter_mut()
                .for_each(|argument| remap_type(argument, ids));
        }
        ConstData::Unit
        | ConstData::Bool(_)
        | ConstData::SInt(_)
        | ConstData::UInt(_)
        | ConstData::F32Bits(_)
        | ConstData::F64Bits(_)
        | ConstData::Bytes(_)
        | ConstData::Text(_)
        | ConstData::BuiltinFailure(_) => {}
    }
}

fn remap_value(value: &mut ValueRef, ids: &BTreeMap<EntityId, EntityId>) {
    match value {
        ValueRef::Parameter(entity) => *entity = mapped(ids, *entity),
        ValueRef::OperationResult(result) => {
            result.operation = mapped(ids, result.operation);
        }
    }
}

fn remap_edge(edge: &mut TargetEdge, ids: &BTreeMap<EntityId, EntityId>) {
    edge.target = mapped(ids, edge.target);
    edge.arguments
        .iter_mut()
        .for_each(|argument| remap_value(argument, ids));
}

fn remap_terminator(terminator: &mut Terminator, ids: &BTreeMap<EntityId, EntityId>) {
    match terminator {
        Terminator::Return(value) => remap_value(&mut value.value, ids),
        Terminator::Branch(value) => remap_edge(&mut value.edge, ids),
        Terminator::CondBranch(value) => {
            remap_value(&mut value.condition, ids);
            remap_edge(&mut value.if_true, ids);
            remap_edge(&mut value.if_false, ids);
        }
        Terminator::VariantSwitch(value) => {
            remap_value(&mut value.value, ids);
            for case in &mut value.cases {
                case.edge.target = mapped(ids, case.edge.target);
                for argument in &mut case.edge.arguments {
                    if let SwitchArgument::Value(value) = argument {
                        remap_value(value, ids);
                    }
                }
            }
        }
        Terminator::Trap(value) => {
            if let Some(payload) = &mut value.payload {
                remap_value(payload, ids);
            }
        }
    }
}

fn insert_category(
    ids: &mut BTreeMap<EntityId, EntityId>,
    kind: u32,
    base: u64,
    values: impl IntoIterator<Item = EntityId>,
) {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_unstable();
    for (index, entity) in values.into_iter().enumerate() {
        let ordinal = base + u64::try_from(index).expect("codec entity count fits u64");
        assert!(
            ids.insert(entity, derived_id(kind, ordinal)).is_none(),
            "each language entity has one construction identity"
        );
    }
}

fn construction_mapping(image: &Image) -> BTreeMap<EntityId, EntityId> {
    let mut ids = BTreeMap::new();
    insert_category(
        &mut ids,
        5,
        FUNCTION_BASE,
        image.functions.iter().map(|value| value.entity_id),
    );
    insert_category(
        &mut ids,
        6,
        PARAMETER_BASE,
        image.parameters.iter().map(|value| value.entity_id),
    );
    insert_category(
        &mut ids,
        7,
        BLOCK_BASE,
        image.blocks.iter().map(|value| value.entity_id),
    );
    insert_category(
        &mut ids,
        8,
        OPERATION_BASE,
        image.operations.iter().map(|value| value.entity_id),
    );
    insert_category(
        &mut ids,
        9,
        CONSTANT_BASE,
        image.constants.iter().map(|value| value.entity_id),
    );
    ids
}

fn remap_graph(graph: &mut FunctionGraph, ids: &BTreeMap<EntityId, EntityId>) {
    graph.entity_id = mapped(ids, graph.entity_id);
    graph
        .parameters
        .iter_mut()
        .for_each(|entity| *entity = mapped(ids, *entity));
    remap_type(&mut graph.result_type, ids);
    graph
        .effects
        .iter_mut()
        .for_each(|entity| *entity = mapped(ids, *entity));
    graph.entry_block = mapped(ids, graph.entry_block);
    graph
        .blocks
        .iter_mut()
        .for_each(|entity| *entity = mapped(ids, *entity));
    graph
        .contracts
        .iter_mut()
        .for_each(|entity| *entity = mapped(ids, *entity));
}

/// The canonical codec component: the arbitrary four-leg composition under
/// the construction identities (codec re-mint of 2026-09-18).
pub(super) fn canonical_codec_image() -> (Image, BTreeMap<EntityId, EntityId>) {
    canonical_image_of(super::codec_main::arbitrary_codec_main_image())
}

/// The bounded generation that preceded the re-mint, retained as history
/// under the same construction identities.
pub(super) fn bounded_canonical_codec_image() -> (Image, BTreeMap<EntityId, EntityId>) {
    canonical_image_of(super::codec_main::codec_main_image())
}

/// Rewrites one composed codec image into derived construction identities.
fn canonical_image_of(mut image: Image) -> (Image, BTreeMap<EntityId, EntityId>) {
    let ids = construction_mapping(&image);
    remap_graph(&mut image.entry, &ids);
    image
        .functions
        .iter_mut()
        .for_each(|graph| remap_graph(graph, &ids));
    for parameter in &mut image.parameters {
        parameter.entity_id = mapped(&ids, parameter.entity_id);
        parameter.owner = mapped(&ids, parameter.owner);
        remap_type(&mut parameter.value_type, &ids);
    }
    for block in &mut image.blocks {
        block.entity_id = mapped(&ids, block.entity_id);
        block.function = mapped(&ids, block.function);
        block
            .parameters
            .iter_mut()
            .for_each(|entity| *entity = mapped(&ids, *entity));
        block
            .operations
            .iter_mut()
            .for_each(|entity| *entity = mapped(&ids, *entity));
        remap_terminator(&mut block.terminator, &ids);
    }
    for operation in &mut image.operations {
        operation.entity_id = mapped(&ids, operation.entity_id);
        operation.block = mapped(&ids, operation.block);
        operation
            .operands
            .iter_mut()
            .for_each(|value| remap_value(value, &ids));
        operation
            .result_types
            .iter_mut()
            .for_each(|value| remap_type(value, &ids));
        match &mut operation.immediate {
            Immediate::Entity(entity) => *entity = mapped(&ids, *entity),
            Immediate::Variant(value) => value.definition = mapped(&ids, value.definition),
            Immediate::Function(value) => {
                value.function = mapped(&ids, value.function);
                value
                    .type_arguments
                    .iter_mut()
                    .for_each(|argument| remap_type(argument, &ids));
            }
            Immediate::None
            | Immediate::Index(_)
            | Immediate::Field(_)
            | Immediate::Observation(_) => {}
        }
    }
    for constant in &mut image.constants {
        constant.entity_id = mapped(&ids, constant.entity_id);
        remap_constant(&mut constant.value, &ids);
    }
    for adapter in &mut image.adapters {
        remap_type(&mut adapter.request_type, &ids);
        remap_type(&mut adapter.response_type, &ids);
        remap_type(&mut adapter.failure_type, &ids);
        adapter
            .effects
            .iter_mut()
            .for_each(|entity| *entity = mapped(&ids, *entity));
    }
    assert_eq!(
        image
            .functions
            .iter()
            .find(|graph| graph.entity_id == image.entry.entity_id),
        Some(&image.entry)
    );
    (image, ids)
}

fn canonical_object(entity_id: EntityId, body: EntityBodyValue) -> EntityObject {
    build_entity_object(
        source_epoch(),
        &EntityObjectRecord {
            entity_id,
            body,
            label: None,
            semantic_fingerprint: None,
        },
    )
    .expect("canonical codec entity object builds")
}

fn canonical_codec_objects(image: &Image) -> Vec<EntityObject> {
    let mut objects = Vec::new();
    objects.extend(image.functions.iter().map(|function| {
        canonical_object(
            function.entity_id,
            EntityBodyValue::Function(FunctionBody {
                type_parameters: function.type_parameters.clone(),
                parameters: function.parameters.clone(),
                result_type: function.result_type.clone(),
                effects: EntityIdSet::from_unsorted(function.effects.clone())
                    .expect("codec effects are unique"),
                entry_block: function.entry_block,
                blocks: function.blocks.clone(),
                contracts: EntityIdSet::from_unsorted(function.contracts.clone())
                    .expect("codec contracts are unique"),
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
    objects.extend(image.adapters.iter().map(|adapter| {
        canonical_object(
            adapter.entity_id,
            EntityBodyValue::AdapterImport(AdapterImportBody {
                adapter_id: adapter.adapter_id,
                abi_version: adapter.abi_version,
                request_type: adapter.request_type.clone(),
                response_type: adapter.response_type.clone(),
                failure_type: adapter.failure_type.clone(),
                effects: EntityIdSet::from_unsorted(adapter.effects.clone())
                    .expect("codec adapter effects are unique"),
            }),
        )
    }));
    objects.push(canonical_object(
        derived_id(16, ENTRY_POINT_ORDINAL),
        EntityBodyValue::EntryPoint(EntryPointBody {
            function: image.entry.entity_id,
            exposure: sley_mutate::value::EntryExposure::Local,
        }),
    ));
    objects.sort_unstable_by_key(|object| object.record().entity_id);
    objects
}

fn u8_input(value: u8) -> ConstValue {
    ConstValue {
        value_type: u8_type(),
        data: ConstData::UInt(u128::from(value)),
    }
}

fn codec_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![
            u8_type(),
            u64_type(),
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            u64_type(),
        ])),
        error: Box::new(TypeExpr::Bytes),
    }
}

fn codec_schema_fixture() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let record = sley_state_root::conformance_epoch_record()
        .canonical_bytes()
        .expect("frozen conformance record is canonical");
    let preimage = sley_schema::bootstrap_preimage(&record)
        .expect("frozen conformance record has a canonical preimage");
    (source_epoch().as_bytes().to_vec(), record, preimage)
}

pub(super) fn codec_schema_decode_inputs() -> Vec<ConstValue> {
    let (_, _, preimage) = codec_schema_fixture();
    vec![
        u8_input(2),
        u64_input(0),
        bytes_input(&preimage),
        bytes_input(&[]),
        bytes_input(&[]),
        bytes_input(&[]),
        bytes_input(&[]),
        unit_input(),
    ]
}

fn schema_decode_inputs() -> Vec<ConstValue> {
    let (_, _, preimage) = codec_schema_fixture();
    vec![bytes_input(&preimage), unit_input()]
}

pub(super) fn codec_schema_decode_expected() -> ConstValue {
    let (epoch, record, _) = codec_schema_fixture();
    let tuple_type = match codec_result_type() {
        TypeExpr::Result { ok, .. } => *ok,
        _ => unreachable!("codec result is a Result"),
    };
    ConstValue {
        value_type: codec_result_type(),
        data: ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
            value_type: tuple_type,
            data: ConstData::Sequence(vec![
                u8_input(2),
                u64_input(0),
                bytes_input(&epoch),
                bytes_input(&record),
                bytes_input(&[]),
                bytes_input(&[]),
                u64_input(0),
            ]),
        }))),
    }
}

fn schema_decode_expected() -> ConstValue {
    let (epoch, record, _) = codec_schema_fixture();
    let tuple_type = TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes]);
    ConstValue {
        value_type: TypeExpr::Result {
            ok: Box::new(tuple_type.clone()),
            error: Box::new(TypeExpr::Bytes),
        },
        data: ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
            value_type: tuple_type,
            data: ConstData::Sequence(vec![bytes_input(&epoch), bytes_input(&record)]),
        }))),
    }
}

struct CodecWitnesses {
    functions: Vec<FunctionGraph>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
    contracts: Vec<ContractDefinition>,
    tests: Vec<TestCaseDefinition>,
}

fn codec_witnesses(schema_decode: EntityId) -> CodecWitnesses {
    let predicate = derived_id(5, WITNESS_FUNCTION_BASE);
    let target = derived_id(5, WITNESS_FUNCTION_BASE + 1);
    let predicate_block = derived_id(7, WITNESS_BLOCK_BASE);
    let target_block = derived_id(7, WITNESS_BLOCK_BASE + 1);
    let predicate_operation = derived_id(8, WITNESS_OPERATION_BASE);
    let target_operation = derived_id(8, WITNESS_OPERATION_BASE + 1);
    let true_constant = derived_id(9, WITNESS_CONSTANT_BASE);
    let contract = derived_id(13, CONTRACT_ORDINAL);
    let test = derived_id(14, TEST_ORDINAL);
    let operation = |entity_id, block| Operation {
        entity_id,
        block,
        ordinal: 0,
        opcode: Opcode::ConstantRef,
        operands: Vec::new(),
        result_types: vec![TypeExpr::Bool],
        immediate: Immediate::Entity(true_constant),
    };
    let block = |entity_id, function, operation| Block {
        entity_id,
        function,
        parameters: Vec::new(),
        operations: vec![operation],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation,
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    };
    let function = |entity_id, entry_block, contracts| FunctionGraph {
        entity_id,
        type_parameters: Vec::new(),
        parameters: Vec::new(),
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block,
        blocks: vec![entry_block],
        contracts,
        visibility: Visibility::Private,
    };
    CodecWitnesses {
        functions: vec![
            function(predicate, predicate_block, Vec::new()),
            function(target, target_block, vec![contract]),
        ],
        blocks: vec![
            block(predicate_block, predicate, predicate_operation),
            block(target_block, target, target_operation),
        ],
        operations: vec![
            operation(predicate_operation, predicate_block),
            operation(target_operation, target_block),
        ],
        constants: vec![ConstantDefinition {
            entity_id: true_constant,
            value: ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(true),
            },
        }],
        contracts: vec![ContractDefinition {
            entity_id: contract,
            target,
            contract_kind: ContractKind::Precondition,
            predicate,
            bindings: Vec::new(),
            resource_limits: None,
        }],
        tests: vec![TestCaseDefinition {
            entity_id: test,
            target: schema_decode,
            inputs: schema_decode_inputs(),
            effect_environment: EffectEnvironment::Replay(Vec::new()),
            expected: ExpectedOutcome::Value(schema_decode_expected()),
            observations: Vec::new(),
            resource_limits: ResourceLimits {
                fuel: 10_000_000,
                memory_bytes: 100_000_000,
                output_bytes: 10_000_000,
                effect_count: 0,
                call_depth: 64,
                wall_timeout_millis: 1_000,
            },
        }],
    }
}

fn schema_decode_component_image(image: &Image, target: EntityId) -> Image {
    let entry = image
        .functions
        .iter()
        .find(|function| function.entity_id == target)
        .expect("canonical schema decode function is retained")
        .clone();
    let block_ids = entry.blocks.iter().copied().collect::<BTreeSet<_>>();
    let blocks = image
        .blocks
        .iter()
        .filter(|block| block_ids.contains(&block.entity_id))
        .cloned()
        .collect::<Vec<_>>();
    let operations = image
        .operations
        .iter()
        .filter(|operation| block_ids.contains(&operation.block))
        .cloned()
        .collect::<Vec<_>>();
    let constant_ids = operations
        .iter()
        .filter(|operation| operation.opcode == Opcode::ConstantRef)
        .filter_map(|operation| match operation.immediate {
            Immediate::Entity(entity) => Some(entity),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    Image {
        types: image.types.clone(),
        entry: entry.clone(),
        functions: vec![entry],
        parameters: image
            .parameters
            .iter()
            .filter(|parameter| parameter.owner == target || block_ids.contains(&parameter.owner))
            .cloned()
            .collect(),
        blocks,
        operations,
        adapters: Vec::new(),
        constants: image
            .constants
            .iter()
            .filter(|constant| constant_ids.contains(&constant.entity_id))
            .cloned()
            .collect(),
    }
}

fn canonical_witness_objects(witnesses: &CodecWitnesses) -> Vec<EntityObject> {
    let mut objects = Vec::new();
    objects.extend(witnesses.functions.iter().map(|function| {
        canonical_object(
            function.entity_id,
            EntityBodyValue::Function(FunctionBody {
                type_parameters: function.type_parameters.clone(),
                parameters: function.parameters.clone(),
                result_type: function.result_type.clone(),
                effects: EntityIdSet::from_unsorted(function.effects.clone()).unwrap(),
                entry_block: function.entry_block,
                blocks: function.blocks.clone(),
                contracts: EntityIdSet::from_unsorted(function.contracts.clone()).unwrap(),
                visibility: function.visibility,
            }),
        )
    }));
    objects.extend(witnesses.blocks.iter().map(|block| {
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
    objects.extend(witnesses.operations.iter().map(|operation| {
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
    objects.extend(witnesses.constants.iter().map(|constant| {
        canonical_object(
            constant.entity_id,
            EntityBodyValue::Constant(ConstantBody {
                value: constant.value.clone(),
            }),
        )
    }));
    objects.extend(witnesses.contracts.iter().map(|contract| {
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
    }));
    objects.extend(witnesses.tests.iter().map(|test| {
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

fn canonical_codec_component_objects(
    image: &Image,
    witnesses: &CodecWitnesses,
) -> Vec<EntityObject> {
    let workspace = derived_id(1, WORKSPACE_ORDINAL);
    let package = derived_id(2, PACKAGE_ORDINAL);
    let namespace = derived_id(3, NAMESPACE_ORDINAL);
    let entry_point = derived_id(16, ENTRY_POINT_ORDINAL);
    let contract = derived_id(13, CONTRACT_ORDINAL);
    let test = derived_id(14, TEST_ORDINAL);
    let mut objects = canonical_codec_objects(image);
    objects.extend(canonical_witness_objects(witnesses));
    objects.extend([
        canonical_object(
            workspace,
            EntityBodyValue::Workspace(WorkspaceBody {
                packages: EntityIdSet::from_unsorted(vec![package]).unwrap(),
                root_namespace: namespace,
                capability_requirements: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
                contracts: EntityIdSet::from_unsorted(vec![contract]).unwrap(),
                tests: EntityIdSet::from_unsorted(vec![test]).unwrap(),
            }),
        ),
        canonical_object(
            package,
            EntityBodyValue::Package(PackageBody {
                workspace,
                root_namespace: namespace,
                dependencies: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
                exports: EntityIdSet::from_unsorted(vec![entry_point]).unwrap(),
            }),
        ),
        canonical_object(
            namespace,
            EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: EntityIdSet::from_unsorted(vec![
                    entry_point,
                    contract,
                    test,
                    witnesses.functions[0].entity_id,
                    witnesses.functions[1].entity_id,
                ])
                .unwrap(),
            }),
        ),
    ]);
    objects.sort_unstable_by_key(|object| object.record().entity_id);
    objects
}

fn canonical_codec_component_root(objects: &[EntityObject]) -> sley_state_root::AcceptedStateRoot {
    let object_id = |entity_id| {
        objects
            .iter()
            .find(|object| object.record().entity_id == entity_id)
            .expect("codec component root anchor is retained")
            .object_id()
    };
    let contract = derived_id(13, CONTRACT_ORDINAL);
    let test = derived_id(14, TEST_ORDINAL);
    let entry_point = derived_id(16, ENTRY_POINT_ORDINAL);
    let mut builder = sley_state_root::StateRootBuilder::new(
        construction_workspace(),
        object_id(contract),
        object_id(test),
        policy_root(),
    );
    for object in objects {
        builder = builder.entity_binding(object.record().entity_id, object.object_id());
    }
    builder
        .entry_point(entry_point)
        .build(&sley_state_root::conformance_registry().expect("state-root registry is frozen"))
        .expect("codec component root is canonical")
}

struct OwnedUnit {
    function: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}

fn validate_retained_schema_test(
    schema_image: &Image,
    witnesses: &CodecWitnesses,
    schema_decode: EntityId,
) {
    let mut functions = schema_image.functions.clone();
    functions.extend(witnesses.functions.clone());
    functions.sort_unstable_by_key(|function| function.entity_id);
    let mut all_blocks = schema_image.blocks.clone();
    all_blocks.extend(witnesses.blocks.clone());
    let mut all_operations = schema_image.operations.clone();
    all_operations.extend(witnesses.operations.clone());
    let owned_units = functions
        .into_iter()
        .map(|function| {
            let block_ids = function.blocks.iter().copied().collect::<BTreeSet<_>>();
            OwnedUnit {
                parameters: schema_image
                    .parameters
                    .iter()
                    .filter(|parameter| {
                        parameter.owner == function.entity_id
                            || block_ids.contains(&parameter.owner)
                    })
                    .cloned()
                    .collect(),
                blocks: all_blocks
                    .iter()
                    .filter(|block| block.function == function.entity_id)
                    .cloned()
                    .collect(),
                operations: all_operations
                    .iter()
                    .filter(|operation| block_ids.contains(&operation.block))
                    .cloned()
                    .collect(),
                function,
            }
        })
        .collect::<Vec<_>>();
    let units = owned_units
        .iter()
        .map(|unit| sley_check::effects::FunctionUnit {
            function: &unit.function,
            parameters: &unit.parameters,
            blocks: &unit.blocks,
            operations: &unit.operations,
        })
        .collect::<Vec<_>>();
    let mut constants = schema_image.constants.clone();
    constants.extend(witnesses.constants.clone());
    constants.sort_unstable_by_key(|constant| constant.entity_id);
    let report = sley_check::contracts::validate_contract_test_program(
        &schema_image.types,
        &units,
        &[],
        &[],
        &[],
        &[],
        &constants,
        &[],
        &witnesses.contracts,
        &witnesses.tests,
        &[schema_decode],
        &[witnesses.tests[0].entity_id],
    )
    .expect("codec contract and retained schema test validate");
    assert_eq!(report.contracts, vec![witnesses.contracts[0].entity_id]);
    assert_eq!(report.tests, vec![witnesses.tests[0].entity_id]);
    assert_eq!(report.selected_tests, vec![witnesses.tests[0].entity_id]);
    assert_eq!(
        report.selection_finality,
        sley_check::contracts::TestPlanFinality::PolicyIncomplete
    );
}

fn assert_codec_component_integrity(
    objects: &[EntityObject],
    root: &sley_state_root::AcceptedStateRoot,
) -> [u8; 32] {
    assert_eq!(objects.len(), 6_233);
    let stored_digest = assert_component_reimports(objects, root);
    assert_eq!(
        hex(root.root.as_bytes()),
        "87afab53ad6634ae0e169cbe767e641c292a363e7ba435808ce9be64ee36e555"
    );
    assert_eq!(root.stored_bytes.len(), 411_675);
    assert_eq!(
        hex(&stored_digest),
        "ea8ed0afe3a82d47dc91058196665be766b16733fbde4ea1fd94edb505292a2f"
    );
    stored_digest
}

/// The structural half of the integrity check, shared with the arbitrary
/// derivation: bindings, workspace, epoch, entry point, policy root, and
/// byte-exact reimport of the root and every object.
fn assert_component_reimports(
    objects: &[EntityObject],
    root: &sley_state_root::AcceptedStateRoot,
) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    assert_eq!(root.record.entity_bindings.len(), objects.len());
    assert_eq!(root.record.workspace_id, construction_workspace());
    assert_eq!(root.record.schema_epoch_id, source_epoch());
    assert_eq!(
        root.record.entry_points,
        vec![derived_id(16, ENTRY_POINT_ORDINAL)]
    );
    assert_eq!(root.record.policy_root, policy_root());
    assert_eq!(
        sley_state_root::import_state_root(
            &sley_state_root::conformance_registry().expect("state-root registry is frozen"),
            &root.stored_bytes,
        )
        .expect("codec component root reimports"),
        *root
    );
    for object in objects {
        assert_eq!(
            import_entity_object(source_epoch(), object.stored_bytes())
                .expect("codec component object reimports"),
            *object
        );
    }
    Sha256::digest(&root.stored_bytes).into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
        output
    })
}

#[test]
fn canonical_codec_graph_uses_derived_language_identities_and_still_executes() {
    let (image, ids) = canonical_codec_image();
    let language_ids = image
        .functions
        .iter()
        .map(|value| value.entity_id)
        .chain(image.parameters.iter().map(|value| value.entity_id))
        .chain(image.blocks.iter().map(|value| value.entity_id))
        .chain(image.operations.iter().map(|value| value.entity_id))
        .chain(image.constants.iter().map(|value| value.entity_id))
        .collect::<BTreeSet<_>>();
    assert_eq!(language_ids.len(), ids.len());
    assert_eq!(language_ids, ids.values().copied().collect());
    assert!(
        image
            .adapters
            .iter()
            .all(|adapter| !language_ids.contains(&adapter.entity_id))
    );

    let (package, approved) = admit_with_limits(&image, codec_profile_limits());
    let (_, body) = super::all_kind_digest_dispatch::fixed_profile_bodies()
        .into_iter()
        .find(|(kind, _)| *kind == 1)
        .expect("Workspace profile exists");
    let entity = [0x44; 32];
    let stored = super::all_kind_digest_dispatch::stored_from_body(entity, &body);
    let outcome = execute_with_limits(
        &package,
        &approved,
        vec![
            u8_input(0),
            u64_input(1),
            bytes_input(&stored),
            bytes_input(&[]),
            bytes_input(&[]),
            bytes_input(&[]),
            bytes_input(&[]),
            unit_input(),
        ],
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("derived codec image must execute")
    };
    assert!(matches!(value.data, ConstData::Result(ResultConst::Ok(_))));
}

#[test]
fn canonical_codec_objects_round_trip_and_bind_the_complete_graph() {
    use sha2::{Digest, Sha256};

    let image = super::integration_codec_program();
    let objects = canonical_codec_objects(&image);
    let expected = image.functions.len()
        + image.parameters.len()
        + image.blocks.len()
        + image.operations.len()
        + image.constants.len()
        + image.adapters.len()
        + 1;
    assert_eq!(objects.len(), expected);
    for object in &objects {
        assert_eq!(
            import_entity_object(source_epoch(), object.stored_bytes())
                .expect("codec object reimports"),
            *object
        );
    }
    let all_ids = objects
        .iter()
        .map(|object| object.record().entity_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(all_ids.len(), objects.len());
    for function in &image.functions {
        assert!(all_ids.contains(&function.entity_id));
    }
    for adapter in &image.adapters {
        assert!(all_ids.contains(&adapter.entity_id));
    }
    assert!(all_ids.contains(&derived_id(16, ENTRY_POINT_ORDINAL)));

    let mut hasher = Sha256::new();
    for object in &objects {
        hasher.update(object.stored_bytes());
    }
    let digest: [u8; 32] = hasher.finalize().into();
    eprintln!(
        "RW090_CANONICAL_CODEC objects={} stored_bytes={} bundle_sha256={}",
        objects.len(),
        objects
            .iter()
            .map(|object| object.stored_bytes().len())
            .sum::<usize>(),
        hex(&digest),
    );
}

#[test]
fn canonical_codec_object_identity_changes_with_semantic_input() {
    let (image, _) = canonical_codec_image();
    let baseline = canonical_codec_objects(&image);
    let mut changed = image;
    let constant = changed
        .constants
        .iter_mut()
        .find(|constant| matches!(constant.value.data, ConstData::Bool(_)))
        .expect("codec carries a Boolean constant");
    let ConstData::Bool(value) = &mut constant.value.data else {
        unreachable!()
    };
    *value = !*value;
    let changed = canonical_codec_objects(&changed);
    let differences = baseline
        .iter()
        .zip(&changed)
        .filter(|(left, right)| left.object_id() != right.object_id())
        .collect::<Vec<_>>();
    assert_eq!(differences.len(), 1);
    assert!(matches!(
        differences[0].0.record().body,
        EntityBodyValue::Constant(_)
    ));
}

#[test]
fn bounded_codec_component_retains_validated_contract_test_and_executes_from_its_root() {
    let (image, ids) = bounded_canonical_codec_image();
    let source_schema_decode = super::schema_codec::schema_decode_image().entry.entity_id;
    let schema_decode = ids[&source_schema_decode];
    let schema_image = schema_decode_component_image(&image, schema_decode);
    let witnesses = codec_witnesses(schema_decode);
    let objects = canonical_codec_component_objects(&image, &witnesses);
    let root = canonical_codec_component_root(&objects);
    let stored_digest = assert_codec_component_integrity(&objects, &root);
    validate_retained_schema_test(&schema_image, &witnesses, schema_decode);

    let (package, approved) = admit_with_bindings(
        &schema_image,
        codec_profile_limits(),
        source_epoch(),
        root.root,
    );
    let outcome = execute_with_limits(
        &package,
        &approved,
        witnesses.tests[0].inputs.clone(),
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(actual) = outcome.termination else {
        panic!("retained codec test must execute successfully")
    };
    let ExpectedOutcome::Value(expected) = &witnesses.tests[0].expected else {
        unreachable!("codec witness has an exact value expectation")
    };
    assert_eq!(&actual, expected);

    assert_eq!(
        super::integration_execute_codec(&image, root.root),
        super::integration_codec_expected()
    );

    eprintln!(
        "RW090_BOUNDED_CODEC_COMPONENT objects={} root={} root_bytes={} root_sha256={}",
        objects.len(),
        hex(root.root.as_bytes()),
        root.stored_bytes.len(),
        hex(&stored_digest),
    );
}

/// The canonical codec component (arbitrary four-leg composition) binds
/// every object under an accepted root and executes the retained schema
/// contract and test from it.
#[test]
fn canonical_codec_component_retains_validated_contract_test_and_executes_from_its_root() {
    let (image, ids) = canonical_codec_image();
    let source_schema_decode = super::schema_codec::schema_decode_image().entry.entity_id;
    let schema_decode = ids[&source_schema_decode];
    let schema_image = schema_decode_component_image(&image, schema_decode);
    let witnesses = codec_witnesses(schema_decode);
    let objects = canonical_codec_component_objects(&image, &witnesses);
    let root = canonical_codec_component_root(&objects);
    let stored_digest = assert_component_reimports(&objects, &root);
    assert_eq!(objects.len(), 13_214);
    assert_eq!(
        hex(root.root.as_bytes()),
        "8c933ccab89b6e150e1070ad534b498dad736bd0ae689a5d30fc600084b2d78a"
    );
    assert_eq!(root.stored_bytes.len(), 872_421);
    assert_eq!(
        hex(&stored_digest),
        "5a0d017c85ea2846bdaece500b9963ca5f761a50ed7d73bb291ec2a970a1e289"
    );
    validate_retained_schema_test(&schema_image, &witnesses, schema_decode);

    let (package, approved) = admit_with_bindings(
        &schema_image,
        codec_profile_limits(),
        source_epoch(),
        root.root,
    );
    let outcome = execute_with_limits(
        &package,
        &approved,
        witnesses.tests[0].inputs.clone(),
        codec_profile_limits(),
    );
    let sley_vm::ExecutionTermination::Success(actual) = outcome.termination else {
        panic!("retained codec test must execute successfully")
    };
    let ExpectedOutcome::Value(expected) = &witnesses.tests[0].expected else {
        unreachable!("codec witness has an exact value expectation")
    };
    assert_eq!(&actual, expected);
    assert_eq!(
        super::integration_execute_codec(&image, root.root),
        super::integration_codec_expected()
    );

    eprintln!(
        "RW090_CODEC_COMPONENT objects={} stored_object_bytes={} root={} root_bytes={} root_sha256={}",
        objects.len(),
        objects
            .iter()
            .map(|object| object.stored_bytes().len())
            .sum::<usize>(),
        hex(root.root.as_bytes()),
        root.stored_bytes.len(),
        hex(&stored_digest),
    );
}
