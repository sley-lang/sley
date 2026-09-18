//! Canonical component root for the complete bounded lowerer and package builder.

use super::*;
use sha2::{Digest, Sha256};
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
    EffectEnvironment, ExpectedOutcome, ResourceLimits, ResultConst, TestCaseDefinition,
};
use std::fmt::Write;

const GENESIS_SEED: [u8; 32] = [0x80; 32];
const CANDIDATE_SEED: [u8; 32] = [0x87; 32];
const LOWER_BASE: u64 = 40_000;
const BUILDER_BASE: u64 = 50_000;
const WORKSPACE_ORDINAL: u64 = 3;
const PACKAGE_ORDINAL: u64 = 3;
const NAMESPACE_ORDINAL: u64 = 3;
const LOWER_ENTRY_ORDINAL: u64 = 7;
const BUILDER_ENTRY_ORDINAL: u64 = 8;
const CONTRACT_ORDINAL: u64 = 3;
const TEST_ORDINAL: u64 = 3;
const WITNESS_BASE: u64 = 60_000;

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

fn canonical_mapping(scaffold: &LowerScaffold, base: u64) -> BTreeMap<EntityId, EntityId> {
    let mut ids = BTreeMap::new();
    let categories: [(u32, Vec<EntityId>); 5] = [
        (
            5_u32,
            scaffold
                .functions
                .iter()
                .map(|value| value.entity_id)
                .collect(),
        ),
        (
            6,
            scaffold
                .parameters
                .iter()
                .map(|value| value.entity_id)
                .collect(),
        ),
        (
            7,
            scaffold
                .blocks
                .iter()
                .map(|value| value.entity_id)
                .collect(),
        ),
        (
            8,
            scaffold
                .operations
                .iter()
                .map(|value| value.entity_id)
                .collect(),
        ),
        (
            9,
            scaffold
                .constants
                .iter()
                .map(|value| value.entity_id)
                .collect(),
        ),
    ];
    for (kind, mut entities) in categories {
        entities.sort_unstable();
        for (index, entity) in entities.into_iter().enumerate() {
            let ordinal = base + u64::try_from(index).expect("lowerer category fits u64");
            assert!(
                ids.insert(entity, derived_id(kind, ordinal)).is_none(),
                "lowerer entity belongs to one category"
            );
        }
    }
    ids
}

fn canonicalize(mut scaffold: LowerScaffold, base: u64) -> LowerScaffold {
    let ids = canonical_mapping(&scaffold, base);
    remap_graph(&mut scaffold.entry, &ids);
    scaffold
        .functions
        .iter_mut()
        .for_each(|graph| remap_graph(graph, &ids));
    for parameter in &mut scaffold.parameters {
        parameter.entity_id = mapped(&ids, parameter.entity_id);
        parameter.owner = mapped(&ids, parameter.owner);
        remap_type(&mut parameter.value_type, &ids);
    }
    for block in &mut scaffold.blocks {
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
        rebase_terminator(&mut block.terminator, &ids);
    }
    for operation in &mut scaffold.operations {
        operation.entity_id = mapped(&ids, operation.entity_id);
        operation.block = mapped(&ids, operation.block);
        operation
            .operands
            .iter_mut()
            .for_each(|operand| rebase_value_ref(operand, &ids));
        operation
            .result_types
            .iter_mut()
            .for_each(|result| remap_type(result, &ids));
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
    for constant in &mut scaffold.constants {
        constant.entity_id = mapped(&ids, constant.entity_id);
        remap_constant(&mut constant.value, &ids);
    }
    for adapter in &mut scaffold.adapters {
        remap_type(&mut adapter.request_type, &ids);
        remap_type(&mut adapter.response_type, &ids);
        remap_type(&mut adapter.failure_type, &ids);
        adapter
            .effects
            .iter_mut()
            .for_each(|effect| *effect = mapped(&ids, *effect));
    }
    scaffold
        .functions
        .sort_unstable_by_key(|value| value.entity_id);
    scaffold
        .parameters
        .sort_unstable_by_key(|value| value.entity_id);
    scaffold
        .blocks
        .sort_unstable_by_key(|value| value.entity_id);
    scaffold
        .operations
        .sort_unstable_by_key(|value| value.entity_id);
    scaffold
        .constants
        .sort_unstable_by_key(|value| value.entity_id);
    scaffold
        .adapters
        .sort_unstable_by_key(|value| value.entity_id);
    scaffold
}

pub(super) fn canonical_lowerer_programs() -> (LowerScaffold, LowerScaffold) {
    (
        canonicalize(complete_function_image_encoder(), LOWER_BASE),
        canonicalize(package_builder(), BUILDER_BASE),
    )
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
    .expect("canonical lowerer entity object builds")
}

fn scaffold_objects(scaffold: &LowerScaffold) -> Vec<EntityObject> {
    let mut objects = Vec::new();
    objects.extend(scaffold.functions.iter().map(|function| {
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
    objects.extend(scaffold.parameters.iter().map(|parameter| {
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
    objects.extend(scaffold.blocks.iter().map(|block| {
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
    objects.extend(scaffold.operations.iter().map(|operation| {
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
    objects.extend(scaffold.constants.iter().map(|constant| {
        canonical_object(
            constant.entity_id,
            EntityBodyValue::Constant(ConstantBody {
                value: constant.value.clone(),
            }),
        )
    }));
    objects.extend(scaffold.adapters.iter().map(|adapter| {
        canonical_object(
            adapter.entity_id,
            EntityBodyValue::AdapterImport(AdapterImportBody {
                adapter_id: adapter.adapter_id,
                abi_version: adapter.abi_version,
                request_type: adapter.request_type.clone(),
                response_type: adapter.response_type.clone(),
                failure_type: adapter.failure_type.clone(),
                effects: EntityIdSet::from_unsorted(adapter.effects.clone()).unwrap(),
            }),
        )
    }));
    objects
}

pub(super) fn lower_fixture() -> (Vec<ConstValue>, ConstValue) {
    let native = native_direct_call_lowered();
    let root = complete_function_fact(&native.bytecode);
    let callees = native
        .callees
        .iter()
        .map(complete_function_fact)
        .collect::<Vec<_>>();
    let inputs = vec![
        bytes_value(&root.identity),
        u32vec_value(&root.parameter_registers),
        bytesvec_value(&root.register_types),
        bytes_value(&root.result_type),
        u32_value(u128::from(root.entry_slot)),
        u32_value(u128::from(root.block_count)),
        complete_block_facts_value(&root.blocks),
        complete_function_facts_value(&callees),
    ];
    let expected = ConstValue {
        value_type: bytes_lower_result_type(),
        data: ConstData::Result(ResultConst::Ok(Box::new(bytes_value(&native.bytes)))),
    };
    (inputs, expected)
}

struct LowerWitnesses {
    functions: Vec<FunctionGraph>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
    contract: ContractDefinition,
    test: TestCaseDefinition,
}

fn lower_witnesses(lower: &LowerScaffold) -> LowerWitnesses {
    let predicate = derived_id(5, WITNESS_BASE);
    let target = derived_id(5, WITNESS_BASE + 1);
    let predicate_block = derived_id(7, WITNESS_BASE);
    let target_block = derived_id(7, WITNESS_BASE + 1);
    let predicate_operation = derived_id(8, WITNESS_BASE);
    let target_operation = derived_id(8, WITNESS_BASE + 1);
    let true_constant = derived_id(9, WITNESS_BASE);
    let contract_id = derived_id(13, CONTRACT_ORDINAL);
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
    let (inputs, expected) = lower_fixture();
    LowerWitnesses {
        functions: vec![
            function(predicate, predicate_block, Vec::new()),
            function(target, target_block, vec![contract_id]),
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
            value: bool_value(true),
        }],
        contract: ContractDefinition {
            entity_id: contract_id,
            target,
            contract_kind: ContractKind::Precondition,
            predicate,
            bindings: Vec::new(),
            resource_limits: None,
        },
        test: TestCaseDefinition {
            entity_id: derived_id(14, TEST_ORDINAL),
            target: lower.entry.entity_id,
            inputs,
            effect_environment: EffectEnvironment::Replay(Vec::new()),
            expected: ExpectedOutcome::Value(expected),
            observations: Vec::new(),
            resource_limits: ResourceLimits {
                fuel: 1_000_000,
                memory_bytes: 100_000_000,
                output_bytes: 100_000,
                effect_count: 0,
                call_depth: 128,
                wall_timeout_millis: 2_000,
            },
        },
    }
}

fn witness_objects(witnesses: &LowerWitnesses) -> Vec<EntityObject> {
    let scaffold = LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: witnesses.functions[0].clone(),
        functions: witnesses.functions.clone(),
        parameters: Vec::new(),
        blocks: witnesses.blocks.clone(),
        operations: witnesses.operations.clone(),
        constants: witnesses.constants.clone(),
        adapters: Vec::new(),
    };
    let mut objects = scaffold_objects(&scaffold);
    objects.extend([
        canonical_object(
            witnesses.contract.entity_id,
            EntityBodyValue::Contract(ContractBody {
                target: witnesses.contract.target,
                contract_kind: witnesses.contract.contract_kind,
                predicate: witnesses.contract.predicate,
                bindings: witnesses.contract.bindings.clone(),
                resource_limits: witnesses.contract.resource_limits,
            }),
        ),
        canonical_object(
            witnesses.test.entity_id,
            EntityBodyValue::TestCase(TestCaseBody {
                target: witnesses.test.target,
                inputs: witnesses.test.inputs.clone(),
                effect_environment: witnesses.test.effect_environment.clone(),
                expected: witnesses.test.expected.clone(),
                observations: witnesses.test.observations.clone(),
                resource_limits: witnesses.test.resource_limits,
            }),
        ),
    ]);
    objects
}

fn component_objects(
    lower: &LowerScaffold,
    builder: &LowerScaffold,
    witnesses: &LowerWitnesses,
) -> Vec<EntityObject> {
    let lower_entry = derived_id(16, LOWER_ENTRY_ORDINAL);
    let builder_entry = derived_id(16, BUILDER_ENTRY_ORDINAL);
    let workspace = derived_id(1, WORKSPACE_ORDINAL);
    let package = derived_id(2, PACKAGE_ORDINAL);
    let namespace = derived_id(3, NAMESPACE_ORDINAL);
    let mut by_entity = BTreeMap::new();
    for object in scaffold_objects(lower)
        .into_iter()
        .chain(scaffold_objects(builder))
        .chain(witness_objects(witnesses))
    {
        if let Some(existing) = by_entity.insert(object.record().entity_id, object.clone()) {
            assert_eq!(
                existing, object,
                "shared adapter objects are byte-identical"
            );
        }
    }
    for object in [
        canonical_object(
            lower_entry,
            EntityBodyValue::EntryPoint(EntryPointBody {
                function: lower.entry.entity_id,
                exposure: sley_mutate::value::EntryExposure::Local,
            }),
        ),
        canonical_object(
            builder_entry,
            EntityBodyValue::EntryPoint(EntryPointBody {
                function: builder.entry.entity_id,
                exposure: sley_mutate::value::EntryExposure::Local,
            }),
        ),
        canonical_object(
            workspace,
            EntityBodyValue::Workspace(WorkspaceBody {
                packages: EntityIdSet::from_unsorted(vec![package]).unwrap(),
                root_namespace: namespace,
                capability_requirements: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
                contracts: EntityIdSet::from_unsorted(vec![witnesses.contract.entity_id]).unwrap(),
                tests: EntityIdSet::from_unsorted(vec![witnesses.test.entity_id]).unwrap(),
            }),
        ),
        canonical_object(
            package,
            EntityBodyValue::Package(PackageBody {
                workspace,
                root_namespace: namespace,
                dependencies: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
                exports: EntityIdSet::from_unsorted(vec![lower_entry, builder_entry]).unwrap(),
            }),
        ),
        canonical_object(
            namespace,
            EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: EntityIdSet::from_unsorted(vec![
                    lower_entry,
                    builder_entry,
                    witnesses.contract.entity_id,
                    witnesses.test.entity_id,
                    witnesses.functions[0].entity_id,
                    witnesses.functions[1].entity_id,
                ])
                .unwrap(),
            }),
        ),
    ] {
        assert!(
            by_entity
                .insert(object.record().entity_id, object)
                .is_none()
        );
    }
    by_entity.into_values().collect()
}

fn component_root(objects: &[EntityObject]) -> sley_state_root::AcceptedStateRoot {
    let object_id = |entity_id| {
        objects
            .iter()
            .find(|object| object.record().entity_id == entity_id)
            .expect("lowerer component root anchor is retained")
            .object_id()
    };
    let mut builder = sley_state_root::StateRootBuilder::new(
        construction_workspace(),
        object_id(derived_id(13, CONTRACT_ORDINAL)),
        object_id(derived_id(14, TEST_ORDINAL)),
        policy_root(),
    );
    for object in objects {
        builder = builder.entity_binding(object.record().entity_id, object.object_id());
    }
    builder
        .entry_point(derived_id(16, LOWER_ENTRY_ORDINAL))
        .entry_point(derived_id(16, BUILDER_ENTRY_ORDINAL))
        .build(&sley_state_root::conformance_registry().expect("state-root registry is frozen"))
        .expect("lowerer component root is canonical")
}

fn merged_adapters(lower: &LowerScaffold, builder: &LowerScaffold) -> Vec<AdapterImport> {
    let mut by_id = BTreeMap::new();
    for adapter in lower.adapters.iter().chain(&builder.adapters) {
        if let Some(existing) = by_id.insert(adapter.entity_id, adapter.clone()) {
            assert_eq!(existing, *adapter);
        }
    }
    by_id.into_values().collect()
}

struct OwnedUnit {
    function: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}

fn validate_retained_test(
    lower: &LowerScaffold,
    builder: &LowerScaffold,
    witnesses: &LowerWitnesses,
) {
    let mut functions = lower.functions.clone();
    functions.extend(builder.functions.clone());
    functions.extend(witnesses.functions.clone());
    functions.sort_unstable_by_key(|function| function.entity_id);
    let mut parameters = lower.parameters.clone();
    parameters.extend(builder.parameters.clone());
    let mut blocks = lower.blocks.clone();
    blocks.extend(builder.blocks.clone());
    blocks.extend(witnesses.blocks.clone());
    let mut operations = lower.operations.clone();
    operations.extend(builder.operations.clone());
    operations.extend(witnesses.operations.clone());
    let owned = functions
        .into_iter()
        .map(|function| {
            let block_ids = function.blocks.iter().copied().collect::<BTreeSet<_>>();
            OwnedUnit {
                parameters: parameters
                    .iter()
                    .filter(|parameter| {
                        parameter.owner == function.entity_id
                            || block_ids.contains(&parameter.owner)
                    })
                    .cloned()
                    .collect(),
                blocks: blocks
                    .iter()
                    .filter(|block| block.function == function.entity_id)
                    .cloned()
                    .collect(),
                operations: operations
                    .iter()
                    .filter(|operation| block_ids.contains(&operation.block))
                    .cloned()
                    .collect(),
                function,
            }
        })
        .collect::<Vec<_>>();
    let units = owned
        .iter()
        .map(|unit| sley_check::effects::FunctionUnit {
            function: &unit.function,
            parameters: &unit.parameters,
            blocks: &unit.blocks,
            operations: &unit.operations,
        })
        .collect::<Vec<_>>();
    let mut constants = lower.constants.clone();
    constants.extend(builder.constants.clone());
    constants.extend(witnesses.constants.clone());
    constants.sort_unstable_by_key(|constant| constant.entity_id);
    let report = sley_check::contracts::validate_contract_test_program(
        &lower.types,
        &units,
        &[],
        &[],
        &merged_adapters(lower, builder),
        &[],
        &constants,
        &[],
        std::slice::from_ref(&witnesses.contract),
        std::slice::from_ref(&witnesses.test),
        &[lower.entry.entity_id],
        &[witnesses.test.entity_id],
    )
    .expect("retained lowerer contract and test validate");
    assert_eq!(report.selected_tests, vec![witnesses.test.entity_id]);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
        output
    })
}

#[test]
fn canonical_lowerer_builder_component_round_trips_validates_and_executes() {
    let (lower, builder) = super::integration_lowerer_programs();
    let witnesses = lower_witnesses(&lower);
    let objects = component_objects(&lower, &builder, &witnesses);
    let root = component_root(&objects);
    assert_eq!(objects.len(), 4_738);
    assert_eq!(root.record.entity_bindings.len(), objects.len());
    assert_eq!(root.record.workspace_id, construction_workspace());
    assert_eq!(root.record.schema_epoch_id, source_epoch());
    assert_eq!(root.record.policy_root, policy_root());
    assert_eq!(
        sley_state_root::import_state_root(
            &sley_state_root::conformance_registry().expect("state-root registry is frozen"),
            &root.stored_bytes,
        )
        .expect("lowerer component root reimports"),
        root
    );
    for object in &objects {
        assert_eq!(
            import_entity_object(source_epoch(), object.stored_bytes())
                .expect("lowerer component object reimports"),
            *object
        );
    }
    validate_retained_test(&lower, &builder, &witnesses);

    let (actual, integration_expected) = super::integration_execute_lowerer(&lower, root.root);
    let ExpectedOutcome::Value(expected) = &witnesses.test.expected else {
        unreachable!("lowerer witness has an exact value expectation")
    };
    assert_eq!(&actual, expected);
    assert_eq!(actual, integration_expected);
    let (actual_package, expected_package) =
        super::integration_execute_builder(&builder, root.root);
    assert_eq!(actual_package, expected_package);

    let mut bundle_hasher = Sha256::new();
    let object_bytes = objects
        .iter()
        .map(|object| object.stored_bytes().len())
        .sum::<usize>();
    for object in &objects {
        bundle_hasher.update(object.stored_bytes());
    }
    let bundle_digest: [u8; 32] = bundle_hasher.finalize().into();
    let root_digest: [u8; 32] = Sha256::digest(&root.stored_bytes).into();
    assert_eq!(object_bytes, 1_177_725);
    assert_eq!(
        bundle_digest,
        [
            0x8e, 0x4d, 0xfd, 0xd3, 0x41, 0xd8, 0x13, 0xdb, 0x46, 0x8f, 0xc0, 0xbb, 0x09, 0xd6,
            0xb2, 0x28, 0xca, 0x07, 0x13, 0x7d, 0xd9, 0xe3, 0x4e, 0x7e, 0x52, 0x4d, 0x6f, 0x18,
            0x32, 0x19, 0xcc, 0xc4,
        ]
    );
    assert_eq!(
        root.root.as_bytes(),
        &[
            0xb7, 0x43, 0x29, 0x6c, 0x43, 0xcf, 0x35, 0x0b, 0x15, 0x5c, 0x4a, 0xec, 0x44, 0x5b,
            0xf8, 0x61, 0xed, 0x6d, 0x81, 0xa0, 0x9e, 0x23, 0x08, 0x57, 0x64, 0x03, 0x4d, 0xf7,
            0xc1, 0xaa, 0x95, 0x2f,
        ]
    );
    assert_eq!(root.stored_bytes.len(), 313_038);
    assert_eq!(
        root_digest,
        [
            0xdd, 0xcd, 0x54, 0xf6, 0x0a, 0x4f, 0xb9, 0xf9, 0x6d, 0xc2, 0xd6, 0x17, 0x3d, 0x6d,
            0xf2, 0x3d, 0xc2, 0x3e, 0x18, 0x62, 0x17, 0xbf, 0x51, 0xec, 0x14, 0x03, 0x22, 0x72,
            0x9a, 0x51, 0xf7, 0x41,
        ]
    );
    eprintln!(
        "RW110_LOWER_COMPONENT lower=({},{},{},{},{}) builder=({},{},{},{},{}) objects={} object_bytes={} bundle_sha256={} root={} root_bytes={} root_sha256={}",
        lower.functions.len(),
        lower.parameters.len(),
        lower.blocks.len(),
        lower.operations.len(),
        lower.constants.len(),
        builder.functions.len(),
        builder.parameters.len(),
        builder.blocks.len(),
        builder.operations.len(),
        builder.constants.len(),
        objects.len(),
        object_bytes,
        hex(&bundle_digest),
        hex(root.root.as_bytes()),
        root.stored_bytes.len(),
        hex(&root_digest),
    );
}
