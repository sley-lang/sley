#![allow(unsafe_code)]
#![no_main]

//! S20-700 scoped persistent surface: the S20-320 full context capsule
//! builder (`sley_query::build_context_capsule`) over root-backed queries
//! answered by the S20-310 full engine for a structured root decoded from
//! fuzz bytes.
//!
//! The byte grammar is the root-query engine target's: a complete-root
//! judgment root, then a typed query, limits, continuation flag, and
//! cursor. Every answered query must yield a capsule whose facts, status,
//! and record obey the contract; a continuation walk of capsules must
//! carry one exact total and never present a page as complete.

use core::slice;

use sley_id::{
    EntityId, ObjectId, PolicyRootId, SchemaEpochId, SemanticFingerprint, StateRoot, WorkspaceId,
};
use sley_query::{
    CapsuleCompleteness, CompleteRootFacts, ContextCapsuleErrorCode, Cursor, ImpactEntity,
    ImpactKind, ModeledEntityKind, QueryLimits, RootQuery, RootQueryInput,
    build_complete_root_snapshot, build_context_capsule, build_root_query_request,
    execute_root_query, judge_complete_root,
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
const MAX_PAGES: usize = 16;
/// Impact kinds in frozen tag order 1 through 12.
const IMPACT_KINDS: [ImpactKind; 12] = [
    ImpactKind::Ownership,
    ImpactKind::TypeReference,
    ImpactKind::ValueReference,
    ImpactKind::ControlFlow,
    ImpactKind::Call,
    ImpactKind::Effect,
    ImpactKind::Capability,
    ImpactKind::Contract,
    ImpactKind::Initializer,
    ImpactKind::TestTarget,
    ImpactKind::Adapter,
    ImpactKind::DefinitionMember,
];

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

    fn ids(&mut self) -> Vec<EntityId> {
        let count = usize::from(self.byte() % 5);
        let mut ids: Vec<EntityId> = (0..count).map(|_| self.id()).collect();
        ids.sort_unstable();
        ids.dedup();
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
fn decode(reader: &mut Reader<'_>) -> Owned {
    let mut owned = Owned::default();
    let count = usize::from(reader.byte() % 25);
    for _ in 0..count {
        let kind = reader.byte() % 18 + 1;
        let entity_id = reader.id();
        match kind {
            1 => owned.workspaces.push(WorkspaceDefinition {
                entity_id,
                root_namespace: reader.id(),
                packages: reader.ids(),
                capability_requirements: reader.ids(),
                contracts: reader.ids(),
                tests: reader.ids(),
            }),
            2 => owned.packages.push(PackageDefinition {
                entity_id,
                workspace: reader.id(),
                root_namespace: reader.id(),
                dependencies: reader.ids(),
                exports: reader.ids(),
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
                    members: reader.ids(),
                });
            }
            4 => owned.type_definitions.push(TypeDefinition {
                entity_id,
                type_parameters: Vec::new(),
                form: TypeDefForm::Record(Vec::new()),
                invariants: reader.ids(),
                visibility: Visibility::Private,
            }),
            5 => owned.functions.push(FunctionGraph {
                entity_id,
                type_parameters: Vec::new(),
                entry_block: reader.id(),
                parameters: reader.list(),
                result_type: TypeExpr::Bool,
                effects: reader.ids(),
                blocks: reader.list(),
                contracts: reader.ids(),
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
                constraint_contracts: reader.ids(),
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
                effects: reader.ids(),
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
                requirements: reader.ids(),
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
    entities.sort_by_key(|entity| entity.entity_id());
    entities.dedup_by_key(|entity| entity.entity_id());
    entities
}

fn pick(entities: &[ImpactEntity<'_>], byte: u8) -> EntityId {
    if entities.is_empty() || byte == 0xFF {
        EntityId::from_bytes([byte; 32])
    } else {
        entities[usize::from(byte) % entities.len()].entity_id()
    }
}

fn kinds_from_mask(mask: u8, extra: u8) -> Vec<ImpactKind> {
    let bits = u16::from(mask) | (u16::from(extra & 0x0F) << 8);
    IMPACT_KINDS
        .iter()
        .enumerate()
        .filter(|(index, _)| bits & (1 << index) != 0)
        .map(|(_, kind)| *kind)
        .collect()
}

fn decode_query(reader: &mut Reader<'_>, entities: &[ImpactEntity<'_>]) -> RootQuery {
    let class = reader.byte() % 19 + 1;
    let entity = pick(entities, reader.byte());
    let kind_byte = reader.byte();
    let mask = reader.byte();
    let extra = reader.byte();
    let seed_count = usize::from(reader.byte() % 4);
    let mut seeds: Vec<EntityId> = (0..seed_count)
        .map(|_| pick(entities, reader.byte()))
        .collect();
    if extra & 0x10 == 0 {
        seeds.sort_unstable();
        seeds.dedup();
    }
    let kind = ModeledEntityKind::from_ssmc_tag(u32::from(kind_byte % 18) + 1)
        .unwrap_or(ModeledEntityKind::Function);
    match class {
        1 => RootQuery::GetRootSummary,
        2 => RootQuery::GetEntity { entity },
        3 => RootQuery::GetSemanticFingerprint { entity },
        4 => RootQuery::ListEntitiesByKind { kind },
        5 => RootQuery::ListWorkspacePackages,
        6 => RootQuery::ListPackageExports { package: entity },
        7 => RootQuery::ListPackageDependencies { package: entity },
        8 => RootQuery::ListNamespaceMembers { namespace: entity },
        9 => RootQuery::ListOwningNamespaces { entity },
        10 => RootQuery::ListEntryPoints,
        11 => RootQuery::ListDependencyRoots,
        12 => RootQuery::ListDirectDependencies {
            entity,
            kinds: kinds_from_mask(mask, extra),
        },
        13 => RootQuery::ListDirectDependents {
            entity,
            kinds: kinds_from_mask(mask, extra),
        },
        14 => RootQuery::ReverseImpactClosure { seeds },
        15 => RootQuery::ForwardDependencyClosure { seeds },
        16 => RootQuery::ListContractsFor { target: entity },
        17 => RootQuery::ListTestsFor { target: entity },
        18 => RootQuery::ListDeclaredEffects { entity },
        _ => RootQuery::ListCapabilityRequirementsFor { subject: entity },
    }
}

fn decode_limits(reader: &mut Reader<'_>) -> QueryLimits {
    let full = QueryLimits::profile_maximum();
    let entities = reader.byte();
    let edges = reader.byte();
    let depth = reader.byte();
    let bytes = reader.byte();
    let work = reader.byte();
    QueryLimits {
        max_returned_entities: if entities == 0 {
            full.max_returned_entities
        } else {
            u64::from(entities % 8)
        },
        max_returned_edges: if edges == 0 {
            full.max_returned_edges
        } else {
            u64::from(edges % 8)
        },
        max_depth: if depth == 0 {
            full.max_depth
        } else {
            u32::from(depth % 4)
        },
        max_response_bytes: if bytes == 0 {
            full.max_response_bytes
        } else {
            u64::from(bytes) * 16
        },
        max_work: if work == 0 {
            full.max_work
        } else {
            u64::from(work) * 8
        },
    }
}

fn decode_cursor(reader: &mut Reader<'_>, entities: &[ImpactEntity<'_>]) -> Option<Cursor> {
    match reader.byte() % 4 {
        0 => None,
        1 => Some(Cursor::Entity(pick(entities, reader.byte()))),
        2 => Some(Cursor::Root(StateRoot::from_bytes([reader.byte(); 32]))),
        _ => {
            let dependent = pick(entities, reader.byte());
            let dependency = pick(entities, reader.byte());
            let kind = IMPACT_KINDS[usize::from(reader.byte() % 12)];
            Some(Cursor::Edge(sley_query::ImpactEdge {
                dependent,
                dependency,
                kind,
            }))
        }
    }
}

#[allow(clippy::too_many_lines)]
fn fuzz_one(input: &[u8]) {
    let mut reader = Reader {
        bytes: input,
        offset: 0,
    };
    let owned = decode(&mut reader);
    let entities = borrow(&owned);
    let bound: Vec<EntityId> = entities.iter().map(|entity| entity.entity_id()).collect();
    let entry_points: Vec<EntityId> = owned
        .entry_points
        .iter()
        .map(|entry| entry.entity_id)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let dependency_roots: Vec<StateRoot> = owned
        .dependency_bindings
        .iter()
        .map(|binding| binding.dependency_root)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let facts = CompleteRootFacts {
        bound_entities: &bound,
        entry_points: &entry_points,
        dependency_roots: &dependency_roots,
    };
    let query = decode_query(&mut reader, &entities);
    let limits = decode_limits(&mut reader);
    let allow = reader.byte() % 2 == 1;
    let after = decode_cursor(&mut reader, &entities);

    if judge_complete_root(&entities, facts).is_err() {
        return;
    }
    let epoch = SchemaEpochId::from_bytes([0x11; 32]);
    let bindings: Vec<(EntityId, ObjectId)> = bound
        .iter()
        .map(|entity| (*entity, ObjectId::from_bytes([entity.as_bytes()[0] | 0x80; 32])))
        .collect();
    // The engine binds every caller-supplied fact to the claimed root, so
    // the target claims the honestly recomputed digest and keeps reaching
    // the engine instead of dying at binding on every input.
    let root = match sley_state_root::recompute_root(&sley_state_root::StateRootRecord {
        workspace_id: WorkspaceId::from_bytes([0x22; 32]),
        schema_epoch_id: epoch,
        entity_bindings: bindings.clone(),
        entry_points: entry_points.clone(),
        dependency_roots: dependency_roots.clone(),
        contract_root: ObjectId::from_bytes([0xC0; 32]),
        test_root: ObjectId::from_bytes([0xD0; 32]),
        policy_root: PolicyRootId::from_bytes([0xE0; 32]),
        interpretation_flags: Vec::new(),
    }) {
        Ok(root) => root,
        Err(_) => return,
    };
    let Ok(snapshot) = build_complete_root_snapshot(epoch, root, &entities, facts) else {
        return;
    };
    let fingerprints: Vec<(EntityId, SemanticFingerprint)> = entities
        .iter()
        .filter(|entity| {
            matches!(
                entity.kind(),
                ModeledEntityKind::TypeDef | ModeledEntityKind::Function
            )
        })
        .map(|entity| {
            (
                entity.entity_id(),
                SemanticFingerprint::from_bytes([entity.entity_id().as_bytes()[0] ^ 0x5A; 32]),
            )
        })
        .collect();
    let input = RootQueryInput {
        snapshot: &snapshot,
        entities: &entities,
        facts,
        bindings: &bindings,
        fingerprints: &fingerprints,
        root,
        workspace_id: WorkspaceId::from_bytes([0x22; 32]),
        schema_epoch: epoch,
        contract_root: ObjectId::from_bytes([0xC0; 32]),
        test_root: ObjectId::from_bytes([0xD0; 32]),
        policy_root: PolicyRootId::from_bytes([0xE0; 32]),
        interpretation_flags: &[],
    };
    let Ok(request) = build_root_query_request(&input, query.clone(), limits, allow, after) else {
        return;
    };
    let Ok(response) = execute_root_query(&input, &request) else {
        return;
    };
    let capsule = match build_context_capsule(&request, &response) {
        Ok(capsule) => capsule,
        Err(error) => {
            assert!(ContextCapsuleErrorCode::ALL.contains(&error.code()));
            // Only a resource ceiling may reject an engine-produced pair.
            assert_eq!(error.code(), ContextCapsuleErrorCode::ResourceLimit);
            return;
        }
    };
    assert_eq!(&capsule.record()[..8], b"SLEYCCP1");
    assert_eq!(capsule.query_id(), response.query_id());
    assert_eq!(capsule.class_tag(), query.tag());
    assert_eq!(capsule.kinds().len(), capsule.entities().len());
    assert!(capsule.entities().windows(2).all(|pair| pair[0] < pair[1]));
    assert!(capsule.roots().windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(capsule.omitted(), capsule.total_count() - capsule.returned());
    assert_eq!(capsule.is_truncated(), capsule.next_after().is_some());
    if capsule.completeness() == CapsuleCompleteness::Complete {
        assert!(!capsule.is_truncated() && after.is_none() && capsule.omitted() == 0);
    } else {
        assert!(capsule.is_truncated() || after.is_some());
    }
    for relationship in capsule.relationships() {
        assert!((relationship.dependent_index as usize) < capsule.entities().len());
        assert!((relationship.dependency_index as usize) < capsule.entities().len());
    }
    let table_indexes = capsule
        .objects()
        .iter()
        .map(|(index, _)| *index)
        .chain(capsule.fingerprints().iter().map(|(index, _)| *index));
    for index in table_indexes {
        assert!((index as usize) < capsule.entities().len());
    }
    let again = build_context_capsule(&request, &response).expect("repeatable capsule");
    assert_eq!(again, capsule, "capsule drifted between builds");
    // Walk the continuation as capsules: one exact total, never complete.
    let mut pages = 1;
    let mut cursor = capsule.next_after();
    while let Some(next) = cursor {
        if pages >= MAX_PAGES {
            break;
        }
        let page_request = build_root_query_request(&input, query.clone(), limits, allow, Some(next))
            .expect("a returned cursor must build a request");
        let page_response = execute_root_query(&input, &page_request).expect("a returned cursor must page");
        let page = build_context_capsule(&page_request, &page_response).expect("a page must capsule");
        assert_eq!(page.total_count(), capsule.total_count());
        assert_eq!(page.completeness(), CapsuleCompleteness::Page);
        assert_eq!(page.after(), Some(next));
        cursor = page.next_after();
        pages += 1;
    }
}
