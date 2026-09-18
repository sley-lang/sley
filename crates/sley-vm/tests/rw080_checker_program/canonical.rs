//! Canonical objects and component root for the bounded checker composition.

use super::*;
use sha2::{Digest, Sha256};
use sley_id::{CandidateNonce, GenesisNonce, PolicyRootId, WorkspaceId};
use sley_mutate::{
    EntityObject, EntityObjectRecord, build_entity_object, import_entity_object,
    value::{
        BlockBody, ConstantBody, ContractBody, EntityBodyValue, EntityIdSet, EntryPointBody,
        FunctionBody, NamespaceBody, OperationBody, PackageBody, ParameterBody, TestCaseBody,
        WorkspaceBody,
    },
};
use sley_ssmc::{
    ContractDefinition, ContractKind, EffectEnvironment, ExpectedOutcome, ResourceLimits,
    ResultConst, TestCaseDefinition,
};
use std::collections::BTreeSet;
use std::fmt::Write;

const GENESIS_SEED: [u8; 32] = [0x80; 32];
const CANDIDATE_SEED: [u8; 32] = [0x87; 32];
const WORKSPACE_ORDINAL: u64 = 2;
const PACKAGE_ORDINAL: u64 = 2;
const NAMESPACE_ORDINAL: u64 = 2;
const ENTRY_POINT_ORDINAL: u64 = 6;
const CONTRACT_ORDINAL: u64 = 2;
const TEST_ORDINAL: u64 = 2;
const WITNESS_BASE: u64 = 30_000;

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
    .expect("canonical checker entity object builds")
}

fn canonical_program_objects(program: &CheckerScaffold) -> Vec<EntityObject> {
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
    objects.push(canonical_object(
        derived_id(16, ENTRY_POINT_ORDINAL),
        EntityBodyValue::EntryPoint(EntryPointBody {
            function: program.entry.entity_id,
            exposure: sley_mutate::value::EntryExposure::Local,
        }),
    ));
    objects
}

fn checker_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![
            u8_type(),
            u64_type(),
            u32_type(),
            u32_type(),
            u64_type(),
        ])),
        error: Box::new(u32_type()),
    }
}

fn exact_checker_result() -> ConstValue {
    let tuple_type = TypeExpr::Tuple(vec![
        u8_type(),
        u64_type(),
        u32_type(),
        u32_type(),
        u64_type(),
    ]);
    ConstValue {
        value_type: checker_result_type(),
        data: ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
            value_type: tuple_type,
            data: ConstData::Sequence(vec![
                u8_value(3),
                ConstValue {
                    value_type: u64_type(),
                    data: ConstData::UInt(4),
                },
                u32_value(0),
                u32_value(0),
                ConstValue {
                    value_type: u64_type(),
                    data: ConstData::UInt(0),
                },
            ]),
        }))),
    }
}

struct CheckerWitnesses {
    functions: Vec<FunctionGraph>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
    contracts: Vec<ContractDefinition>,
    tests: Vec<TestCaseDefinition>,
}

fn checker_witnesses(program: &CheckerScaffold) -> CheckerWitnesses {
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
    CheckerWitnesses {
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
            entity_id: derived_id(14, TEST_ORDINAL),
            target: program.entry.entity_id,
            inputs: super::composed::valid_bounded_inputs(3),
            effect_environment: EffectEnvironment::Replay(Vec::new()),
            expected: ExpectedOutcome::Value(exact_checker_result()),
            observations: Vec::new(),
            resource_limits: ResourceLimits {
                fuel: 100_000,
                memory_bytes: 1_000_000,
                output_bytes: 100_000,
                effect_count: 0,
                call_depth: 64,
                wall_timeout_millis: 1_000,
            },
        }],
    }
}

fn canonical_witness_objects(witnesses: &CheckerWitnesses) -> Vec<EntityObject> {
    let program = CheckerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: witnesses.functions[0].clone(),
        functions: witnesses.functions.clone(),
        parameters: Vec::new(),
        blocks: witnesses.blocks.clone(),
        operations: witnesses.operations.clone(),
        constants: witnesses.constants.clone(),
    };
    let mut objects = canonical_program_objects(&program);
    objects
        .pop()
        .expect("temporary witness entry point is last");
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

fn checker_component_objects(
    program: &CheckerScaffold,
    witnesses: &CheckerWitnesses,
) -> Vec<EntityObject> {
    let workspace = derived_id(1, WORKSPACE_ORDINAL);
    let package = derived_id(2, PACKAGE_ORDINAL);
    let namespace = derived_id(3, NAMESPACE_ORDINAL);
    let entry_point = derived_id(16, ENTRY_POINT_ORDINAL);
    let contract = derived_id(13, CONTRACT_ORDINAL);
    let test = derived_id(14, TEST_ORDINAL);
    let mut objects = canonical_program_objects(program);
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

fn checker_component_root(objects: &[EntityObject]) -> sley_state_root::AcceptedStateRoot {
    let object_id = |entity_id| {
        objects
            .iter()
            .find(|object| object.record().entity_id == entity_id)
            .expect("checker component root anchor is retained")
            .object_id()
    };
    let contract = derived_id(13, CONTRACT_ORDINAL);
    let test = derived_id(14, TEST_ORDINAL);
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
        .entry_point(derived_id(16, ENTRY_POINT_ORDINAL))
        .build(&sley_state_root::conformance_registry().expect("state-root registry is frozen"))
        .expect("checker component root is canonical")
}

struct OwnedUnit {
    function: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}

fn validate_checker_test(program: &CheckerScaffold, witnesses: &CheckerWitnesses) {
    let mut functions = program.functions.clone();
    functions.extend(witnesses.functions.clone());
    functions.sort_unstable_by_key(|function| function.entity_id);
    let mut blocks = program.blocks.clone();
    blocks.extend(witnesses.blocks.clone());
    let mut operations = program.operations.clone();
    operations.extend(witnesses.operations.clone());
    let owned = functions
        .into_iter()
        .map(|function| {
            let block_ids = function.blocks.iter().copied().collect::<BTreeSet<_>>();
            OwnedUnit {
                parameters: program
                    .parameters
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
    let mut constants = program.constants.clone();
    constants.extend(witnesses.constants.clone());
    constants.sort_unstable_by_key(|constant| constant.entity_id);
    let report = sley_check::contracts::validate_contract_test_program(
        &program.types,
        &units,
        &[],
        &[],
        &[],
        &[],
        &constants,
        &[],
        &witnesses.contracts,
        &witnesses.tests,
        &[program.entry.entity_id],
        &[witnesses.tests[0].entity_id],
    )
    .expect("retained checker contract and test validate");
    assert_eq!(report.selected_tests, vec![witnesses.tests[0].entity_id]);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
        output
    })
}

#[test]
fn canonical_checker_component_round_trips_validates_and_executes() {
    let program = super::composed::canonical_checker_program();
    let witnesses = checker_witnesses(&program);
    let objects = checker_component_objects(&program, &witnesses);
    let root = checker_component_root(&objects);
    assert_eq!(objects.len(), 842);
    assert_eq!(root.record.entity_bindings.len(), objects.len());
    assert_eq!(root.record.workspace_id, construction_workspace());
    assert_eq!(root.record.schema_epoch_id, source_epoch());
    assert_eq!(root.record.policy_root, policy_root());
    assert_eq!(
        sley_state_root::import_state_root(
            &sley_state_root::conformance_registry().expect("state-root registry is frozen"),
            &root.stored_bytes,
        )
        .expect("checker component root reimports"),
        root
    );
    for object in &objects {
        assert_eq!(
            import_entity_object(source_epoch(), object.stored_bytes())
                .expect("checker object reimports"),
            *object
        );
    }
    validate_checker_test(&program, &witnesses);

    let (package, approved) =
        admit_checker_program_with_bindings(&program, source_epoch(), root.root);
    let outcome = sley_vm::execute_approved_package_v2(
        &package,
        &approved,
        sley_vm::ExecutionRequest {
            inputs: witnesses.tests[0].inputs.clone(),
            limits: generous_limits(),
        },
    )
    .expect("v2 executes retained checker test");
    let sley_vm::ExecutionTermination::Success(actual) = outcome.termination else {
        panic!("retained checker test returns a typed value")
    };
    let ExpectedOutcome::Value(expected) = &witnesses.tests[0].expected else {
        unreachable!("checker witness has an exact value expectation")
    };
    assert_eq!(&actual, expected);

    let mut bundle_hasher = Sha256::new();
    for object in &objects {
        bundle_hasher.update(object.stored_bytes());
    }
    let bundle_digest: [u8; 32] = bundle_hasher.finalize().into();
    let root_digest: [u8; 32] = Sha256::digest(&root.stored_bytes).into();
    assert_eq!(
        objects
            .iter()
            .map(|object| object.stored_bytes().len())
            .sum::<usize>(),
        206_250
    );
    assert_eq!(
        hex(&bundle_digest),
        "a3a5085970bd9fc7b52f7166e8b7e55a452efa085f9b264167665912c2ed32ef"
    );
    assert_eq!(
        hex(root.root.as_bytes()),
        "6466a198aeeb3790fb26cc274e377f1a4a58029f0ac852997781b4a95fbe124f"
    );
    assert_eq!(root.stored_bytes.len(), 55_869);
    assert_eq!(
        hex(&root_digest),
        "0217400d3e9fa6b37aaaae2d1277aab12abaeb2c90ba17b08aa8ff4298f32d05"
    );
    eprintln!(
        "RW100_CHECKER_COMPONENT objects={} object_bytes={} bundle_sha256={} root={} root_bytes={} root_sha256={}",
        objects.len(),
        objects
            .iter()
            .map(|object| object.stored_bytes().len())
            .sum::<usize>(),
        hex(&bundle_digest),
        hex(root.root.as_bytes()),
        root.stored_bytes.len(),
        hex(&root_digest),
    );
}
