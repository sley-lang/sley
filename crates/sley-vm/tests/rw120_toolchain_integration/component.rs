//! Canonical union of the retained codec, checker, lowerer, and package builder.

use super::{checker, codec, lower};
use sley_id::{CandidateNonce, EntityId, GenesisNonce, PolicyRootId, WorkspaceId};
use sley_mutate::{
    EntityObject, EntityObjectRecord, build_entity_object,
    value::{
        AdapterImportBody, BlockBody, ConstantBody, ContractBody, EntityBodyValue, EntityIdSet,
        EntryExposure, EntryPointBody, FunctionBody, NamespaceBody, OperationBody, PackageBody,
        ParameterBody, TestCaseBody, WorkspaceBody,
    },
};
use sley_ssmc::{
    AdapterImport, Block, ConstData, ConstValue, ConstantDefinition, ContractDefinition,
    ContractKind, EffectEnvironment, ExpectedOutcome, FunctionGraph, FunctionRefValue, Immediate,
    Opcode, Operation, OperationResultRef, Parameter, ParameterRole, Reachability, ResourceLimits,
    ReturnTerminator, Terminator, TestCaseDefinition, TypeExpr, ValueRef, Visibility,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

#[derive(Clone)]
pub(super) struct MergedProgram {
    pub(super) entry_points: Vec<EntityId>,
    pub(super) functions: Vec<FunctionGraph>,
    pub(super) parameters: Vec<Parameter>,
    pub(super) blocks: Vec<Block>,
    pub(super) operations: Vec<Operation>,
    pub(super) constants: Vec<ConstantDefinition>,
    pub(super) adapters: Vec<AdapterImport>,
}

pub(super) struct DriverFixture {
    pub(super) program: MergedProgram,
    pub(super) entry: EntityId,
    pub(super) inputs: Vec<ConstValue>,
    pub(super) expected: ConstValue,
}

fn merge_values<T: Clone + Debug + Eq>(
    groups: impl IntoIterator<Item = Vec<T>>,
    identity: impl Fn(&T) -> EntityId,
) -> Vec<T> {
    let mut values = BTreeMap::new();
    for value in groups.into_iter().flatten() {
        let entity = identity(&value);
        if let Some(existing) = values.insert(entity, value.clone()) {
            assert_eq!(existing, value, "shared integration entity is exact");
        }
    }
    values.into_values().collect()
}

pub(super) fn merged_program() -> MergedProgram {
    merged_program_with(codec::integration_codec_program())
}

/// The same union with the arbitrary codec substituted for the retained one:
/// the canonical `S` a codec re-mint would produce.
pub(super) fn arbitrary_merged_program() -> MergedProgram {
    merged_program_with(codec::integration_arbitrary_codec_program())
}

fn merged_program_with(codec: codec::Image) -> MergedProgram {
    let checker = checker::integration_checker_program();
    let (lowerer, builder) = lower::integration_lowerer_programs();
    let entry_points = vec![
        codec.entry.entity_id,
        checker.entry.entity_id,
        lowerer.entry.entity_id,
        builder.entry.entity_id,
    ];
    MergedProgram {
        functions: merge_values(
            vec![
                codec.functions,
                checker.functions,
                lowerer.functions,
                builder.functions,
            ],
            |value| value.entity_id,
        ),
        parameters: merge_values(
            vec![
                codec.parameters,
                checker.parameters,
                lowerer.parameters,
                builder.parameters,
            ],
            |value| value.entity_id,
        ),
        blocks: merge_values(
            vec![codec.blocks, checker.blocks, lowerer.blocks, builder.blocks],
            |value| value.entity_id,
        ),
        operations: merge_values(
            vec![
                codec.operations,
                checker.operations,
                lowerer.operations,
                builder.operations,
            ],
            |value| value.entity_id,
        ),
        constants: merge_values(
            vec![
                codec.constants,
                checker.constants,
                lowerer.constants,
                builder.constants,
            ],
            |value| value.entity_id,
        ),
        adapters: merge_values(
            vec![codec.adapters, lowerer.adapters, builder.adapters],
            |value| value.entity_id,
        ),
        entry_points,
    }
}

pub(super) fn checker_program() -> MergedProgram {
    let checker = checker::integration_checker_program();
    MergedProgram {
        entry_points: vec![checker.entry.entity_id],
        functions: checker.functions,
        parameters: checker.parameters,
        blocks: checker.blocks,
        operations: checker.operations,
        constants: checker.constants,
        adapters: Vec::new(),
    }
}

const DRIVER_BASE: u64 = 90_000;

fn operation_result(operation: EntityId) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation,
        result_index: 0,
    })
}

struct DriverCalls {
    parameters: Vec<Parameter>,
    calls: Vec<Operation>,
    inputs: Vec<ConstValue>,
    expected_values: Vec<ConstValue>,
}

fn driver_calls(
    program: &MergedProgram,
    entry: EntityId,
    block: EntityId,
    tests: [(Vec<ConstValue>, ConstValue); 4],
) -> DriverCalls {
    let child_entries = program.entry_points.clone();
    let parameter_by_id = program
        .parameters
        .iter()
        .map(|parameter| (parameter.entity_id, parameter))
        .collect::<BTreeMap<_, _>>();
    let function_by_id = program
        .functions
        .iter()
        .map(|function| (function.entity_id, function))
        .collect::<BTreeMap<_, _>>();
    let mut inputs = Vec::new();
    let mut expected_values = Vec::new();
    let mut parameters = Vec::new();
    let mut calls = Vec::new();
    for (call_index, ((child_entry, (child_inputs, child_expected)), child_function)) in
        child_entries
            .iter()
            .copied()
            .zip(tests)
            .zip(child_entries.iter().map(|child| function_by_id[child]))
            .enumerate()
    {
        assert_eq!(child_function.entity_id, child_entry);
        assert_eq!(child_function.parameters.len(), child_inputs.len());
        let mut operands = Vec::new();
        for (child_parameter, input) in child_function.parameters.iter().zip(&child_inputs) {
            assert_eq!(
                parameter_by_id[child_parameter].value_type,
                input.value_type
            );
            let ordinal = u32::try_from(parameters.len()).unwrap();
            let entity_id = derived_id(6, DRIVER_BASE + u64::from(ordinal));
            parameters.push(Parameter {
                entity_id,
                owner: entry,
                role: ParameterRole::Function,
                ordinal,
                value_type: input.value_type.clone(),
            });
            operands.push(ValueRef::Parameter(entity_id));
        }
        assert_eq!(child_function.result_type, child_expected.value_type);
        let operation = Operation {
            entity_id: derived_id(8, DRIVER_BASE + u64::try_from(call_index).unwrap()),
            block,
            ordinal: u32::try_from(call_index).unwrap(),
            opcode: Opcode::CallDirect,
            operands,
            result_types: vec![child_function.result_type.clone()],
            immediate: Immediate::Function(FunctionRefValue {
                function: child_entry,
                type_arguments: Vec::new(),
            }),
        };
        inputs.extend(child_inputs);
        expected_values.push(child_expected);
        calls.push(operation);
    }
    DriverCalls {
        parameters,
        calls,
        inputs,
        expected_values,
    }
}

pub(super) fn driver_fixture() -> DriverFixture {
    driver_fixture_over(merged_program())
}

pub(super) fn arbitrary_driver_fixture() -> DriverFixture {
    driver_fixture_over(arbitrary_merged_program())
}

fn driver_fixture_over(mut program: MergedProgram) -> DriverFixture {
    let entry = derived_id(5, DRIVER_BASE);
    let block_id = derived_id(7, DRIVER_BASE);
    assert!(
        program
            .functions
            .iter()
            .all(|value| value.entity_id != entry)
    );
    assert!(
        program
            .blocks
            .iter()
            .all(|value| value.entity_id != block_id)
    );
    let tests = [
        codec::integration_codec_test(),
        checker::integration_checker_test(),
        lower::integration_lowerer_test(),
        lower::integration_builder_test(),
    ];
    let DriverCalls {
        parameters,
        mut calls,
        inputs,
        expected_values,
    } = driver_calls(&program, entry, block_id, tests);

    let result_type = TypeExpr::Tuple(
        calls
            .iter()
            .map(|call| call.result_types[0].clone())
            .collect(),
    );
    let tuple = Operation {
        entity_id: derived_id(8, DRIVER_BASE + u64::try_from(calls.len()).unwrap()),
        block: block_id,
        ordinal: u32::try_from(calls.len()).unwrap(),
        opcode: Opcode::TupleNew,
        operands: calls
            .iter()
            .map(|call| operation_result(call.entity_id))
            .collect(),
        result_types: vec![result_type.clone()],
        immediate: Immediate::None,
    };
    let mut block_operations = calls
        .iter()
        .map(|operation| operation.entity_id)
        .collect::<Vec<_>>();
    block_operations.push(tuple.entity_id);
    let function_parameters = parameters
        .iter()
        .map(|parameter| parameter.entity_id)
        .collect::<Vec<_>>();
    let function = FunctionGraph {
        entity_id: entry,
        type_parameters: Vec::new(),
        parameters: function_parameters,
        result_type: result_type.clone(),
        effects: Vec::new(),
        entry_block: block_id,
        blocks: vec![block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let block = Block {
        entity_id: block_id,
        function: entry,
        parameters: Vec::new(),
        operations: block_operations,
        terminator: Terminator::Return(ReturnTerminator {
            value: operation_result(tuple.entity_id),
        }),
        reachability: Reachability::Required,
    };
    calls.push(tuple);
    program.entry_points.push(entry);
    program.functions.push(function);
    program.parameters.extend(parameters);
    program.blocks.push(block);
    program.operations.extend(calls);
    program
        .functions
        .sort_unstable_by_key(|value| value.entity_id);
    program
        .parameters
        .sort_unstable_by_key(|value| value.entity_id);
    program.blocks.sort_unstable_by_key(|value| value.entity_id);
    program
        .operations
        .sort_unstable_by_key(|value| value.entity_id);

    DriverFixture {
        program,
        entry,
        inputs,
        expected: ConstValue {
            value_type: result_type,
            data: ConstData::Sequence(expected_values),
        },
    }
}

const GENESIS_SEED: [u8; 32] = [0x80; 32];
const CANDIDATE_SEED: [u8; 32] = [0x87; 32];
const WITNESS_BASE: u64 = 80_000;
const WORKSPACE_ORDINAL: u64 = 5;
const PACKAGE_ORDINAL: u64 = 5;
const NAMESPACE_ORDINAL: u64 = 5;
const ENTRY_BASE: u64 = 20;
const CONTRACT_ORDINAL: u64 = 5;
const TEST_ORDINAL: u64 = 5;

fn workspace() -> WorkspaceId {
    WorkspaceId::derive(GenesisNonce::from_bytes(GENESIS_SEED))
}

fn candidate() -> CandidateNonce {
    CandidateNonce::from_bytes(CANDIDATE_SEED)
}

fn derived_id(kind: u32, ordinal: u64) -> EntityId {
    EntityId::derive(workspace(), candidate(), kind, ordinal)
}

fn epoch() -> sley_id::SchemaEpochId {
    sley_state_root::conformance_epoch_id().expect("state-root epoch is frozen")
}

fn policy_root() -> PolicyRootId {
    PolicyRootId::from_bytes([
        0x3b, 0x8e, 0xab, 0x80, 0xac, 0xdc, 0x87, 0x4b, 0xd3, 0xf3, 0x95, 0x89, 0x81, 0xd0, 0xda,
        0x81, 0xd2, 0xce, 0x23, 0x14, 0x07, 0x3f, 0xc9, 0x77, 0x3d, 0x75, 0xed, 0x90, 0x1f, 0x0d,
        0xc8, 0x9c,
    ])
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
    .expect("integrated component object builds")
}

fn program_objects(program: &MergedProgram) -> Vec<EntityObject> {
    let mut objects = Vec::new();
    objects.extend(program.functions.iter().map(|function| {
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
    objects.extend(program.parameters.iter().map(|parameter| {
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
    objects.extend(program.blocks.iter().map(|block| {
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
    objects.extend(program.operations.iter().map(|operation| {
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
    objects.extend(program.constants.iter().map(|constant| {
        canonical_object(
            constant.entity_id,
            EntityBodyValue::Constant(ConstantBody {
                value: constant.value.clone(),
            }),
        )
    }));
    objects.extend(program.adapters.iter().map(|adapter| {
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

struct Witnesses {
    functions: Vec<FunctionGraph>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
    contract: ContractDefinition,
    test: TestCaseDefinition,
}

fn bool_value(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

fn witnesses(checker_entry: EntityId) -> Witnesses {
    let predicate = derived_id(5, WITNESS_BASE);
    let target = derived_id(5, WITNESS_BASE + 1);
    let predicate_block = derived_id(7, WITNESS_BASE);
    let target_block = derived_id(7, WITNESS_BASE + 1);
    let predicate_operation = derived_id(8, WITNESS_BASE);
    let target_operation = derived_id(8, WITNESS_BASE + 1);
    let true_constant = derived_id(9, WITNESS_BASE);
    let contract = derived_id(13, CONTRACT_ORDINAL);
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
    let (inputs, expected) = checker::integration_checker_test();
    Witnesses {
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
            value: bool_value(true),
        }],
        contract: ContractDefinition {
            entity_id: contract,
            target,
            contract_kind: ContractKind::Precondition,
            predicate,
            bindings: Vec::new(),
            resource_limits: None,
        },
        test: TestCaseDefinition {
            entity_id: derived_id(14, TEST_ORDINAL),
            target: checker_entry,
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

fn witness_objects(witnesses: &Witnesses) -> Vec<EntityObject> {
    let mut objects = Vec::new();
    for function in &witnesses.functions {
        objects.push(canonical_object(
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
        ));
    }
    for block in &witnesses.blocks {
        objects.push(canonical_object(
            block.entity_id,
            EntityBodyValue::Block(BlockBody {
                function: block.function,
                parameters: block.parameters.clone(),
                operations: block.operations.clone(),
                terminator: block.terminator.clone(),
                reachability: block.reachability,
            }),
        ));
    }
    for operation in &witnesses.operations {
        objects.push(canonical_object(
            operation.entity_id,
            EntityBodyValue::Operation(OperationBody {
                block: operation.block,
                ordinal: operation.ordinal,
                opcode: operation.opcode.tag(),
                operands: operation.operands.clone(),
                result_types: operation.result_types.clone(),
                immediate: operation.immediate.clone(),
            }),
        ));
    }
    objects.push(canonical_object(
        witnesses.constants[0].entity_id,
        EntityBodyValue::Constant(ConstantBody {
            value: witnesses.constants[0].value.clone(),
        }),
    ));
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

fn metadata_objects(program: &MergedProgram, witnesses: &Witnesses) -> Vec<EntityObject> {
    let workspace_id = derived_id(1, WORKSPACE_ORDINAL);
    let package = derived_id(2, PACKAGE_ORDINAL);
    let namespace = derived_id(3, NAMESPACE_ORDINAL);
    let entry_points = program
        .entry_points
        .iter()
        .enumerate()
        .map(|(index, _)| derived_id(16, ENTRY_BASE + u64::try_from(index).unwrap()))
        .collect::<Vec<_>>();
    let mut objects = program
        .entry_points
        .iter()
        .zip(&entry_points)
        .map(|(function, entry)| {
            canonical_object(
                *entry,
                EntityBodyValue::EntryPoint(EntryPointBody {
                    function: *function,
                    exposure: EntryExposure::Local,
                }),
            )
        })
        .collect::<Vec<_>>();
    objects.extend([
        canonical_object(
            workspace_id,
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
                workspace: workspace_id,
                root_namespace: namespace,
                dependencies: EntityIdSet::from_unsorted(Vec::new()).unwrap(),
                exports: EntityIdSet::from_unsorted(entry_points.clone()).unwrap(),
            }),
        ),
        canonical_object(
            namespace,
            EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: EntityIdSet::from_unsorted(
                    entry_points
                        .into_iter()
                        .chain(witnesses.functions.iter().map(|value| value.entity_id))
                        .chain([witnesses.contract.entity_id, witnesses.test.entity_id])
                        .collect(),
                )
                .unwrap(),
            }),
        ),
    ]);
    objects
}

pub(super) struct ComponentEvidence {
    pub(super) objects: Vec<EntityObject>,
    pub(super) root: sley_state_root::AcceptedStateRoot,
    pub(super) test: TestCaseDefinition,
}

pub(super) fn component_evidence(program: &MergedProgram) -> ComponentEvidence {
    let checker_entry = program.entry_points[1];
    let witnesses = witnesses(checker_entry);
    let mut by_entity = BTreeMap::new();
    for object in program_objects(program)
        .into_iter()
        .chain(witness_objects(&witnesses))
        .chain(metadata_objects(program, &witnesses))
    {
        assert!(
            by_entity
                .insert(object.record().entity_id, object)
                .is_none(),
            "integrated component identities are disjoint"
        );
    }
    let objects = by_entity.into_values().collect::<Vec<_>>();
    let object_id = |entity_id| {
        objects
            .iter()
            .find(|object| object.record().entity_id == entity_id)
            .expect("integrated root anchor exists")
            .object_id()
    };
    let mut builder = sley_state_root::StateRootBuilder::new(
        workspace(),
        object_id(witnesses.contract.entity_id),
        object_id(witnesses.test.entity_id),
        policy_root(),
    );
    for object in &objects {
        builder = builder.entity_binding(object.record().entity_id, object.object_id());
    }
    for index in 0..program.entry_points.len() {
        builder = builder.entry_point(derived_id(16, ENTRY_BASE + u64::try_from(index).unwrap()));
    }
    let root = builder
        .build(&sley_state_root::conformance_registry().expect("state-root registry is frozen"))
        .expect("integrated component root builds");
    ComponentEvidence {
        objects,
        root,
        test: witnesses.test,
    }
}

pub(super) struct DriverExecutionEvidence {
    pub(super) value: ConstValue,
    pub(super) instruction_count: u64,
    pub(super) fuel_used: u64,
    pub(super) peak_value_units: u64,
    pub(super) package_digest: [u8; 32],
    pub(super) image_bytes: usize,
    pub(super) gate_operation_count: u32,
    pub(super) gate_bridge_uses: u32,
}

pub(super) struct DriverReference {
    pub(super) lowered: sley_vm::LoweredFunction,
    pub(super) package: sley_vm::ExecutionPackage,
}

fn driver_limits() -> sley_vm::ExecutionLimits {
    sley_vm::ExecutionLimits {
        max_instructions: 1_000_000,
        max_fuel: 100_000_000,
        max_value_units: 1_000_000_000,
        max_output_units: 100_000_000,
        cancel_at_fuel: None,
    }
}

pub(super) fn reconstruction_limits() -> sley_vm::ExecutionLimits {
    sley_vm::ExecutionLimits {
        max_instructions: 100_000_000,
        max_fuel: 10_000_000_000,
        max_value_units: 100_000_000_000_000,
        max_output_units: 10_000_000_000,
        cancel_at_fuel: None,
    }
}

pub(super) fn reference_driver_package_with_limits(
    program: &MergedProgram,
    entry: EntityId,
    state_root: sley_id::StateRoot,
    limits: sley_vm::ExecutionLimits,
) -> DriverReference {
    use sley_vm::bootstrap::{BootstrapProfileInput, BootstrapProfileVersion};

    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    let entry_function = program
        .functions
        .iter()
        .find(|function| function.entity_id == entry)
        .expect("integrated driver entry exists");
    let lowered = sley_vm::lower_function(sley_vm::LoweringInput {
        types: &types,
        function: entry_function,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        schema_epoch: epoch(),
        state_root,
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &program.constants,
        globals: &[],
        functions: &program.functions,
        contracts: &[],
        adapters: &program.adapters,
    })
    .expect("integrated driver lowers with its complete reachable closure");
    let gate = sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &types,
        schema_epoch: epoch(),
        entry: entry_function,
        presented_image_bytes: &lowered.bytes,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.adapters,
        constants: &program.constants,
        profile_version: BootstrapProfileVersion::V2,
    })
    .expect("integrated driver admits under the frozen successor profile");
    let package = sley_vm::ExecutionPackage {
        image_bytes: lowered.bytes.clone(),
        constants: program.constants.clone(),
        type_definitions: Vec::new(),
        imports: program.adapters.clone(),
        globals: Vec::new(),
        contracts: Vec::new(),
        entry,
        schema_epoch: epoch(),
        state_root,
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        admitted_limits: limits,
        gate_operation_count: gate.operation_count(),
        gate_bridge_uses: gate.bridge_uses(),
        gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
    };
    DriverReference { lowered, package }
}

pub(super) fn execute_driver(
    program: &MergedProgram,
    entry: EntityId,
    state_root: sley_id::StateRoot,
    inputs: Vec<ConstValue>,
) -> DriverExecutionEvidence {
    execute_driver_with_limits(program, entry, state_root, inputs, driver_limits())
}

pub(super) fn execute_driver_with_limits(
    program: &MergedProgram,
    entry: EntityId,
    state_root: sley_id::StateRoot,
    inputs: Vec<ConstValue>,
    limits: sley_vm::ExecutionLimits,
) -> DriverExecutionEvidence {
    use sley_vm::{V2Closure, approve_package_v2};

    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    let package = reference_driver_package_with_limits(program, entry, state_root, limits).package;
    let closure = V2Closure {
        types: &types,
        schema_epoch: epoch(),
        state_root,
        entry,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.adapters,
        constants: &program.constants,
        globals: &[],
        contracts: &[],
    };
    let (digests, receipt, report) =
        sley_vm::admit_v2_package(&closure, &package).expect("integrated driver admits");
    let approved = approve_package_v2(&package, &digests, receipt, &report)
        .expect("integrated driver approval binds the admitted closure");
    let outcome = sley_vm::execute_approved_package_v2(
        &package,
        &approved,
        sley_vm::ExecutionRequest { inputs, limits },
    )
    .expect("integrated driver executes");
    let instruction_count = outcome.instruction_count;
    let fuel_used = outcome.fuel_used;
    let peak_value_units = outcome.peak_value_units;
    let value = match outcome.termination {
        sley_vm::ExecutionTermination::Success(value) => value,
        termination => panic!(
            "integrated driver returns its four typed results, got {termination:?}; instructions={instruction_count} fuel={fuel_used} peak_value_units={peak_value_units}"
        ),
    };
    DriverExecutionEvidence {
        value,
        instruction_count,
        fuel_used,
        peak_value_units,
        package_digest: approved.package_digest,
        image_bytes: package.image_bytes.len(),
        gate_operation_count: package.gate_operation_count,
        gate_bridge_uses: package.gate_bridge_uses,
    }
}

struct OwnedUnit {
    function: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}

pub(super) fn validate_component_test() {
    let checker = checker::integration_checker_program();
    let witnesses = witnesses(checker.entry.entity_id);
    let mut functions = checker.functions.clone();
    functions.extend(witnesses.functions.clone());
    functions.sort_unstable_by_key(|value| value.entity_id);
    let parameters = checker.parameters.clone();
    let mut blocks = checker.blocks.clone();
    blocks.extend(witnesses.blocks.clone());
    let mut operations = checker.operations.clone();
    operations.extend(witnesses.operations.clone());
    let owned = functions
        .into_iter()
        .map(|function| {
            let block_ids = function.blocks.iter().copied().collect::<BTreeSet<_>>();
            OwnedUnit {
                parameters: parameters
                    .iter()
                    .filter(|value| {
                        value.owner == function.entity_id || block_ids.contains(&value.owner)
                    })
                    .cloned()
                    .collect(),
                blocks: blocks
                    .iter()
                    .filter(|value| value.function == function.entity_id)
                    .cloned()
                    .collect(),
                operations: operations
                    .iter()
                    .filter(|value| block_ids.contains(&value.block))
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
    let mut constants = checker.constants;
    constants.extend(witnesses.constants);
    constants.sort_unstable_by_key(|value| value.entity_id);
    let report = sley_check::contracts::validate_contract_test_program(
        &checker.types,
        &units,
        &[],
        &[],
        &[],
        &[],
        &constants,
        &[],
        std::slice::from_ref(&witnesses.contract),
        std::slice::from_ref(&witnesses.test),
        &[checker.entry.entity_id],
        &[witnesses.test.entity_id],
    )
    .expect("integrated checker test validates");
    assert_eq!(report.selected_tests, vec![witnesses.test.entity_id]);
}
