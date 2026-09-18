//! Canonical construction identities and object materialization for the
//! executable four-leg codec graph.

use super::*;
use sley_id::{CandidateNonce, GenesisNonce, WorkspaceId};
use sley_mutate::{
    EntityObject, EntityObjectRecord, build_entity_object, import_entity_object,
    value::{
        AdapterImportBody, BlockBody, ConstantBody, EntityBodyValue, EntityIdSet, EntryPointBody,
        FunctionBody, OperationBody, ParameterBody,
    },
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

fn canonical_codec_image() -> (Image, BTreeMap<EntityId, EntityId>) {
    let mut image = super::codec_main::codec_main_image();
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

    let (image, _) = canonical_codec_image();
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
