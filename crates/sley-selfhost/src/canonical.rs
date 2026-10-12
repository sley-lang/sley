//! Canonical program state `S` (C0 side): SSMC1 entity objects and the
//! state-root record for one seed program, plus closure pruning.
//!
//! The seed assembler's output becomes canonical state here exactly once.
//! After that the entity objects and the root are the source of truth; the
//! toolchain rebuilds itself from them.

use std::collections::{BTreeSet, HashMap};

use sley_id::{
    CandidateNonce, EntityId, GenesisNonce, ObjectId, PolicyRootId, SchemaEpochId, WorkspaceId,
};
use sley_mutate::value::{
    AdapterImportBody, BlockBody, ConstantBody, EntityBodyValue, EntityIdSet, EntryPointBody,
    FunctionBody, NamespaceBody, OperationBody, PackageBody, ParameterBody, TypeDefBody,
    WorkspaceBody,
};
use sley_mutate::{EntityObject, EntityObjectRecord, build_entity_object};
use sley_ssmc::{EntryExposure, Immediate, Opcode, TypeExpr};
use sley_state_root::{AcceptedStateRoot, StateRootBuilder};

use crate::seed::Program;

/// One canonical program state.
#[derive(Clone, Debug)]
pub struct CanonicalS {
    /// Schema epoch of every object and the root.
    pub epoch: SchemaEpochId,
    /// Entity objects in ascending entity-id order (root-binding order).
    pub objects: Vec<EntityObject>,
    /// The accepted state root.
    pub root: AcceptedStateRoot,
    /// The `EntryPoint` entity.
    pub entry_point: EntityId,
    /// The entry function.
    pub entry_function: EntityId,
}

/// The schema epoch canonical state is built under.
///
/// # Panics
///
/// Panics if the frozen conformance epoch is unavailable.
#[must_use]
pub fn epoch() -> SchemaEpochId {
    sley_state_root::conformance_epoch_id().expect("frozen conformance epoch")
}

fn hash32(domain: &str, seed: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sley-selfhost.canonical.v1\0");
    hasher.update(domain.as_bytes());
    hasher.update(&[0]);
    hasher.update(seed.as_bytes());
    *hasher.finalize().as_bytes()
}

/// Narrows a program to the call closure of `entry`, keeping only the
/// constants, type definitions, and imports that closure references.
///
/// # Panics
///
/// Panics when `entry` is not a program function.
#[must_use]
pub fn prune_to_closure(program: &Program, entry: EntityId) -> Program {
    let functions: HashMap<EntityId, usize> = program
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| (f.entity_id, i))
        .collect();
    assert!(functions.contains_key(&entry), "entry is a program function");
    let block_owner: HashMap<EntityId, EntityId> = program
        .blocks
        .iter()
        .map(|b| (b.entity_id, b.function))
        .collect();
    let mut ops_of: HashMap<EntityId, Vec<usize>> = HashMap::new();
    for (i, op) in program.operations.iter().enumerate() {
        ops_of.entry(block_owner[&op.block]).or_default().push(i);
    }
    let mut reached = BTreeSet::new();
    let mut work = vec![entry];
    while let Some(function) = work.pop() {
        if !reached.insert(function) {
            continue;
        }
        for &i in ops_of.get(&function).into_iter().flatten() {
            let op = &program.operations[i];
            if op.opcode == Opcode::CallDirect
                && let Immediate::Function(reference) = &op.immediate
            {
                work.push(reference.function);
            }
        }
    }
    let blocks: BTreeSet<EntityId> = program
        .blocks
        .iter()
        .filter(|b| reached.contains(&b.function))
        .map(|b| b.entity_id)
        .collect();
    let operations: Vec<_> = program
        .operations
        .iter()
        .filter(|op| blocks.contains(&op.block))
        .cloned()
        .collect();
    let mut named = BTreeSet::new();
    let mut referenced = BTreeSet::new();
    let note_type = |ty: &TypeExpr, named: &mut BTreeSet<EntityId>| collect_named(ty, named);
    for op in &operations {
        for ty in &op.result_types {
            note_type(ty, &mut named);
        }
        match &op.immediate {
            Immediate::Entity(id) => {
                referenced.insert(*id);
            }
            Immediate::Variant(variant) => {
                named.insert(variant.definition);
            }
            _ => {}
        }
    }
    let parameters: Vec<_> = program
        .parameters
        .iter()
        .filter(|p| reached.contains(&p.owner) || blocks.contains(&p.owner))
        .cloned()
        .collect();
    for parameter in &parameters {
        note_type(&parameter.value_type, &mut named);
    }
    let functions_out: Vec<_> = program
        .functions
        .iter()
        .filter(|f| reached.contains(&f.entity_id))
        .cloned()
        .collect();
    for function in &functions_out {
        note_type(&function.result_type, &mut named);
    }
    let constants: Vec<_> = program
        .constants
        .iter()
        .filter(|c| referenced.contains(&c.entity_id))
        .cloned()
        .collect();
    for constant in &constants {
        note_type(&constant.value.value_type, &mut named);
    }
    let imports: Vec<_> = program
        .imports
        .iter()
        .filter(|i| referenced.contains(&i.entity_id))
        .cloned()
        .collect();
    for import in &imports {
        note_type(&import.request_type, &mut named);
        note_type(&import.response_type, &mut named);
    }
    // Close named types over their members.
    let definitions: HashMap<EntityId, &sley_ssmc::TypeDefinition> = program
        .type_definitions
        .iter()
        .map(|d| (d.entity_id, d))
        .collect();
    let mut pending: Vec<EntityId> = named.iter().copied().collect();
    while let Some(id) = pending.pop() {
        if let Some(definition) = definitions.get(&id) {
            let mut inner = BTreeSet::new();
            match &definition.form {
                sley_ssmc::TypeDefForm::Record(fields) => {
                    for field in fields {
                        collect_named(&field.value_type, &mut inner);
                    }
                }
                sley_ssmc::TypeDefForm::Variant(cases) => {
                    for case in cases {
                        if let Some(payload) = &case.payload_type {
                            collect_named(payload, &mut inner);
                        }
                    }
                }
            }
            for found in inner {
                if named.insert(found) {
                    pending.push(found);
                }
            }
        }
    }
    Program {
        functions: functions_out,
        parameters,
        blocks: program
            .blocks
            .iter()
            .filter(|b| blocks.contains(&b.entity_id))
            .cloned()
            .collect(),
        operations,
        constants,
        type_definitions: program
            .type_definitions
            .iter()
            .filter(|d| named.contains(&d.entity_id))
            .cloned()
            .collect(),
        imports,
    }
}

fn collect_named(ty: &TypeExpr, out: &mut BTreeSet<EntityId>) {
    match ty {
        TypeExpr::Named(named) => {
            out.insert(named.definition);
            for argument in &named.arguments {
                collect_named(argument, out);
            }
        }
        TypeExpr::Tuple(items) => items.iter().for_each(|item| collect_named(item, out)),
        TypeExpr::Vector(inner) | TypeExpr::Option(inner) | TypeExpr::LocalCell(inner) => {
            collect_named(inner, out);
        }
        TypeExpr::OrderedMap { key, value } | TypeExpr::Result { ok: key, error: value } => {
            collect_named(key, out);
            collect_named(value, out);
        }
        TypeExpr::FunctionRef(function) => {
            for parameter in &function.parameters {
                collect_named(parameter, out);
            }
            collect_named(&function.result, out);
        }
        _ => {}
    }
}

fn set(ids: impl IntoIterator<Item = EntityId>) -> EntityIdSet {
    EntityIdSet::from_unsorted(ids.into_iter().collect()).expect("unique identities")
}

/// Builds canonical state for `program` with `entry_function` exposed
/// through one `EntryPoint`. `seed` derives the workspace, package,
/// namespace, and entry-point identities.
///
/// # Panics
///
/// Panics when an object or the root fails to build.
#[must_use]
pub fn build(program: &Program, entry_function: EntityId, seed: &str) -> CanonicalS {
    let epoch = epoch();
    let workspace_id = WorkspaceId::derive(GenesisNonce::from_bytes(hash32("genesis", seed)));
    let nonce = CandidateNonce::from_bytes(hash32("candidate", seed));
    let workspace = EntityId::derive(workspace_id, nonce, 1, 1);
    let package = EntityId::derive(workspace_id, nonce, 2, 1);
    let namespace = EntityId::derive(workspace_id, nonce, 3, 1);
    let entry_point = EntityId::derive(workspace_id, nonce, 16, 1);
    let mut records: Vec<(EntityId, EntityBodyValue)> = vec![
        (
            workspace,
            EntityBodyValue::Workspace(WorkspaceBody {
                packages: set([package]),
                root_namespace: namespace,
                capability_requirements: set([]),
                contracts: set([]),
                tests: set([]),
            }),
        ),
        (
            package,
            EntityBodyValue::Package(PackageBody {
                workspace,
                root_namespace: namespace,
                dependencies: set([]),
                exports: set([entry_point]),
            }),
        ),
        (
            namespace,
            EntityBodyValue::Namespace(NamespaceBody {
                parent: None,
                members: set([entry_point]),
            }),
        ),
        (
            entry_point,
            EntityBodyValue::EntryPoint(EntryPointBody {
                function: entry_function,
                exposure: EntryExposure::Local,
            }),
        ),
    ];
    for function in &program.functions {
        records.push((
            function.entity_id,
            EntityBodyValue::Function(FunctionBody {
                type_parameters: function.type_parameters.clone(),
                parameters: function.parameters.clone(),
                result_type: function.result_type.clone(),
                effects: set(function.effects.iter().copied()),
                entry_block: function.entry_block,
                blocks: function.blocks.clone(),
                contracts: set(function.contracts.iter().copied()),
                visibility: function.visibility,
            }),
        ));
    }
    for parameter in &program.parameters {
        records.push((
            parameter.entity_id,
            EntityBodyValue::Parameter(ParameterBody {
                owner: parameter.owner,
                role: parameter.role,
                ordinal: parameter.ordinal,
                value_type: parameter.value_type.clone(),
            }),
        ));
    }
    for block in &program.blocks {
        records.push((
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
    for operation in &program.operations {
        records.push((
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
    for constant in &program.constants {
        records.push((
            constant.entity_id,
            EntityBodyValue::Constant(ConstantBody {
                value: constant.value.clone(),
            }),
        ));
    }
    for definition in &program.type_definitions {
        records.push((
            definition.entity_id,
            EntityBodyValue::TypeDef(TypeDefBody {
                type_parameters: definition.type_parameters.clone(),
                form: definition.form.clone(),
                invariants: set(definition.invariants.iter().copied()),
                visibility: definition.visibility,
            }),
        ));
    }
    for import in &program.imports {
        records.push((
            import.entity_id,
            EntityBodyValue::AdapterImport(AdapterImportBody {
                adapter_id: import.adapter_id,
                abi_version: import.abi_version,
                request_type: import.request_type.clone(),
                response_type: import.response_type.clone(),
                failure_type: import.failure_type.clone(),
                effects: set(import.effects.iter().copied()),
            }),
        ));
    }
    records.sort_by_key(|(id, _)| *id);
    let objects: Vec<EntityObject> = records
        .into_iter()
        .map(|(entity_id, body)| {
            build_entity_object(
                epoch,
                &EntityObjectRecord {
                    entity_id,
                    body,
                    label: None,
                    semantic_fingerprint: None,
                },
            )
            .expect("canonical entity object builds")
        })
        .collect();
    let mut builder = StateRootBuilder::new(
        workspace_id,
        ObjectId::from_bytes(hash32("contract-root", seed)),
        ObjectId::from_bytes(hash32("test-root", seed)),
        PolicyRootId::from_bytes(hash32("policy-root", seed)),
    )
    .entry_point(entry_point);
    for object in &objects {
        builder = builder.entity_binding(object.record().entity_id, object.object_id());
    }
    let registry = sley_state_root::conformance_registry().expect("frozen registry");
    let root = builder.build(&registry).expect("state root builds");
    CanonicalS {
        epoch,
        objects,
        root,
        entry_point,
        entry_function,
    }
}
