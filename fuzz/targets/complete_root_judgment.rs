#![allow(unsafe_code)]
#![no_main]

//! S20-700 adjacent persistent surface: the S20-250 full complete-root
//! closure judgment (`sley_query::judge_complete_root`) over a structured
//! eighteen-kind request decoded from fuzz bytes.
//!
//! Byte grammar (every byte read past the end reads as zero):
//! `count = b % 25`, then per entity `kind = b % 18 + 1`, `id = b`, then the
//! kind's identity and set fields (a set is `n = b % 5` followed by `n` id
//! bytes; a list keeps its order, a set is sorted and deduplicated unless
//! flag bit 0 is set), then a flags byte: bit 0 raw sets, bit 1 keep entity
//! order, bit 2 entry points from bytes, bit 3 dependency roots from bytes,
//! bit 4 bound entities from bytes.

use core::slice;

use sley_id::{EntityId, StateRoot};
use sley_query::{
    CompleteRootFacts, ImpactEntity, ImpactErrorCode, ImpactIndex, judge_complete_root,
};
use sley_ssmc::{
    AdapterImport, Block, CapabilityRequirement, ConstData, ConstValue, ConstantDefinition,
    ContractDefinition, ContractKind, DependencyBindingDefinition, EffectDefinition,
    EffectEnvironment, EffectKind, EntryExposure, EntryPointDefinition, ExpectedOutcome,
    FunctionGraph, GlobalValueDefinition, Immediate, NamespaceDefinition, Opcode, Operation,
    PackageDefinition, Parameter, ParameterRole, PolicyBindingDefinition, Reachability,
    ResourceLimits, ReturnTerminator, Terminator, TestCaseDefinition, TypeDefForm,
    TypeDefinition, TypeExpr, ValueRef, Visibility, WorkspaceDefinition,
};

const MAX_FUZZ_INPUT_BYTES: usize = 4_096;

#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn LLVMFuzzerTestOneInput(data: *const u8, len: usize) -> i32 {
    if len == 0 || len > MAX_FUZZ_INPUT_BYTES {
        return 0;
    }
    let input = unsafe { slice::from_raw_parts(data, len) };
    fuzz_one(input);
    0
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> u8 {
        let value = self.bytes.get(self.offset).copied().unwrap_or(0);
        self.offset = self.offset.saturating_add(1);
        value
    }

    fn id(&mut self) -> EntityId {
        EntityId::from_bytes([self.byte(); 32])
    }

    fn ids(&mut self, raw: bool) -> Vec<EntityId> {
        let count = usize::from(self.byte() % 5);
        let mut ids: Vec<EntityId> = (0..count).map(|_| self.id()).collect();
        if !raw {
            ids.sort_unstable();
            ids.dedup();
        }
        ids
    }

    fn list(&mut self) -> Vec<EntityId> {
        let count = usize::from(self.byte() % 5);
        (0..count).map(|_| self.id()).collect()
    }
}

#[derive(Default)]
struct Owned {
    workspaces: Vec<WorkspaceDefinition>,
    packages: Vec<PackageDefinition>,
    namespaces: Vec<NamespaceDefinition>,
    type_definitions: Vec<TypeDefinition>,
    functions: Vec<FunctionGraph>,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
    globals: Vec<GlobalValueDefinition>,
    effects: Vec<EffectDefinition>,
    requirements: Vec<CapabilityRequirement>,
    contracts: Vec<ContractDefinition>,
    tests: Vec<TestCaseDefinition>,
    adapters: Vec<AdapterImport>,
    entry_points: Vec<EntryPointDefinition>,
    policy_bindings: Vec<PolicyBindingDefinition>,
    dependency_bindings: Vec<DependencyBindingDefinition>,
}

#[allow(clippy::too_many_lines)]
fn decode(reader: &mut Reader<'_>, raw: bool) -> Owned {
    let mut owned = Owned::default();
    let count = usize::from(reader.byte() % 25);
    for _ in 0..count {
        let kind = reader.byte() % 18 + 1;
        let entity_id = reader.id();
        match kind {
            1 => owned.workspaces.push(WorkspaceDefinition {
                entity_id,
                root_namespace: reader.id(),
                packages: reader.ids(raw),
                capability_requirements: reader.ids(raw),
                contracts: reader.ids(raw),
                tests: reader.ids(raw),
            }),
            2 => owned.packages.push(PackageDefinition {
                entity_id,
                workspace: reader.id(),
                root_namespace: reader.id(),
                dependencies: reader.ids(raw),
                exports: reader.ids(raw),
            }),
            3 => {
                let parent = if reader.byte() % 2 == 1 {
                    Some(reader.id())
                } else {
                    None
                };
                owned.namespaces.push(NamespaceDefinition {
                    entity_id,
                    parent,
                    members: reader.ids(raw),
                });
            }
            4 => owned.type_definitions.push(TypeDefinition {
                entity_id,
                type_parameters: Vec::new(),
                form: TypeDefForm::Record(Vec::new()),
                invariants: reader.ids(raw),
                visibility: Visibility::Private,
            }),
            5 => owned.functions.push(FunctionGraph {
                entity_id,
                type_parameters: Vec::new(),
                entry_block: reader.id(),
                parameters: reader.list(),
                result_type: TypeExpr::Bool,
                effects: reader.ids(raw),
                blocks: reader.list(),
                contracts: reader.ids(raw),
                visibility: Visibility::Private,
            }),
            6 => owned.parameters.push(Parameter {
                entity_id,
                owner: reader.id(),
                role: if reader.byte() % 2 == 0 {
                    ParameterRole::Function
                } else {
                    ParameterRole::Block
                },
                ordinal: 0,
                value_type: TypeExpr::Bool,
            }),
            7 => {
                let function = reader.id();
                let returned = reader.id();
                owned.blocks.push(Block {
                    entity_id,
                    function,
                    parameters: reader.list(),
                    operations: reader.list(),
                    terminator: Terminator::Return(ReturnTerminator {
                        value: ValueRef::Parameter(returned),
                    }),
                    reachability: Reachability::Required,
                });
            }
            8 => owned.operations.push(Operation {
                entity_id,
                block: reader.id(),
                ordinal: 0,
                opcode: Opcode::ConstantRef,
                operands: Vec::new(),
                result_types: vec![TypeExpr::Bool],
                immediate: Immediate::Entity(reader.id()),
            }),
            9 => owned.constants.push(ConstantDefinition {
                entity_id,
                value: ConstValue {
                    value_type: TypeExpr::Bool,
                    data: ConstData::Bool(true),
                },
            }),
            10 => owned.globals.push(GlobalValueDefinition {
                entity_id,
                value_type: TypeExpr::Bool,
                initializer: reader.id(),
                visibility: Visibility::Private,
            }),
            11 => owned.effects.push(EffectDefinition {
                entity_id,
                effect_kind: EffectKind::StdoutWrite,
                scope_type: TypeExpr::Unit,
                request_type: TypeExpr::Unit,
                response_type: TypeExpr::Unit,
                failure_type: TypeExpr::Unit,
                visibility: Visibility::Private,
            }),
            12 => owned.requirements.push(CapabilityRequirement {
                entity_id,
                effect: reader.id(),
                allowed_scopes: Vec::new(),
                constraint_contracts: reader.ids(raw),
            }),
            13 => owned.contracts.push(ContractDefinition {
                entity_id,
                target: reader.id(),
                contract_kind: ContractKind::Invariant,
                predicate: reader.id(),
                bindings: Vec::new(),
                resource_limits: None,
            }),
            14 => owned.tests.push(TestCaseDefinition {
                entity_id,
                target: reader.id(),
                inputs: Vec::new(),
                effect_environment: EffectEnvironment::Replay(Vec::new()),
                expected: ExpectedOutcome::FailureCode(1),
                observations: Vec::new(),
                resource_limits: ResourceLimits {
                    fuel: 1,
                    memory_bytes: 1,
                    output_bytes: 1,
                    effect_count: 1,
                    call_depth: 1,
                    wall_timeout_millis: 1,
                },
            }),
            15 => owned.adapters.push(AdapterImport {
                entity_id,
                adapter_id: [0; 32],
                abi_version: 1,
                request_type: TypeExpr::Unit,
                response_type: TypeExpr::Unit,
                failure_type: TypeExpr::Unit,
                effects: reader.ids(raw),
            }),
            16 => owned.entry_points.push(EntryPointDefinition {
                entity_id,
                function: reader.id(),
                exposure: if reader.byte() % 2 == 0 {
                    EntryExposure::Local
                } else {
                    EntryExposure::Protocol
                },
            }),
            17 => owned.policy_bindings.push(PolicyBindingDefinition {
                entity_id,
                subject: reader.id(),
                requirements: reader.ids(raw),
            }),
            _ => owned.dependency_bindings.push(DependencyBindingDefinition {
                entity_id,
                dependency_root: StateRoot::from_bytes([reader.byte(); 32]),
                external_package: reader.id(),
                local_namespace: reader.id(),
            }),
        }
    }
    owned
}

fn borrow(owned: &Owned) -> Vec<ImpactEntity<'_>> {
    let mut entities = Vec::new();
    entities.extend(owned.workspaces.iter().map(ImpactEntity::Workspace));
    entities.extend(owned.packages.iter().map(ImpactEntity::Package));
    entities.extend(owned.namespaces.iter().map(ImpactEntity::Namespace));
    entities.extend(owned.type_definitions.iter().map(ImpactEntity::TypeDef));
    entities.extend(owned.functions.iter().map(ImpactEntity::Function));
    entities.extend(owned.parameters.iter().map(ImpactEntity::Parameter));
    entities.extend(owned.blocks.iter().map(ImpactEntity::Block));
    entities.extend(owned.operations.iter().map(ImpactEntity::Operation));
    entities.extend(owned.constants.iter().map(ImpactEntity::Constant));
    entities.extend(owned.globals.iter().map(ImpactEntity::GlobalValue));
    entities.extend(owned.effects.iter().map(ImpactEntity::EffectDef));
    entities.extend(
        owned
            .requirements
            .iter()
            .map(ImpactEntity::CapabilityRequirement),
    );
    entities.extend(owned.contracts.iter().map(ImpactEntity::Contract));
    entities.extend(owned.tests.iter().map(ImpactEntity::TestCase));
    entities.extend(owned.adapters.iter().map(ImpactEntity::AdapterImport));
    entities.extend(owned.entry_points.iter().map(ImpactEntity::EntryPoint));
    entities.extend(owned.policy_bindings.iter().map(ImpactEntity::PolicyBinding));
    entities.extend(
        owned
            .dependency_bindings
            .iter()
            .map(ImpactEntity::DependencyBinding),
    );
    entities
}

fn fuzz_one(input: &[u8]) {
    let mut reader = Reader {
        bytes: input,
        offset: 0,
    };
    // Flags are read from the last byte so the entity grammar starts at zero.
    let flags = input[input.len() - 1];
    let raw_sets = flags & 1 != 0;
    let keep_order = flags & 2 != 0;
    let owned = decode(&mut reader, raw_sets);
    let mut entities = borrow(&owned);
    if !keep_order {
        entities.sort_by_key(|entity| entity.entity_id());
        entities.dedup_by_key(|entity| entity.entity_id());
    }
    let bound: Vec<EntityId> = if flags & 16 != 0 {
        reader.ids(raw_sets)
    } else {
        entities.iter().map(|entity| entity.entity_id()).collect()
    };
    let entry_points: Vec<EntityId> = if flags & 4 != 0 {
        reader.ids(raw_sets)
    } else {
        owned
            .entry_points
            .iter()
            .map(|entry| entry.entity_id)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    let dependency_roots: Vec<StateRoot> = if flags & 8 != 0 {
        let count = usize::from(reader.byte() % 3);
        (0..count)
            .map(|_| StateRoot::from_bytes([reader.byte(); 32]))
            .collect()
    } else {
        owned
            .dependency_bindings
            .iter()
            .map(|binding| binding.dependency_root)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    let facts = CompleteRootFacts {
        bound_entities: &bound,
        entry_points: &entry_points,
        dependency_roots: &dependency_roots,
    };
    match judge_complete_root(&entities, facts) {
        Ok(judged) => {
            let again = judge_complete_root(&entities, facts)
                .expect("a passing complete-root judgment must be repeatable");
            assert_eq!(judged, again, "complete-root judgment drifted between runs");
            let index = ImpactIndex::build(&entities)
                .expect("a judged request must build its plain impact index");
            assert_eq!(judged.index(), &index, "judged index differs from the plain index");
            assert_eq!(judged.bound_entities(), entities.len());
            assert!(
                entities
                    .iter()
                    .any(|entity| entity.entity_id() == judged.workspace()
                        && matches!(entity, ImpactEntity::Workspace(_))),
                "judged workspace is not a workspace entity of the request"
            );
            for edge in index.direct_edges() {
                assert!(
                    bound.binary_search(&edge.dependency).is_ok(),
                    "an edge escaped the bound inventory"
                );
            }
        }
        Err(error) => {
            assert!(
                ImpactErrorCode::ALL.contains(&error.code()),
                "unknown complete-root failure code"
            );
        }
    }
}
