//! Bounded composition of the seven executable RW-100 checker slices.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

const CHILD_COUNT: usize = 7;
const INVALID_SELECTOR: u32 = u32::MAX;

fn composed_plan_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        u8_type(),
        u64_type(),
        u32_type(),
        u32_type(),
        u64_type(),
    ])
}

fn composed_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(composed_plan_type()),
        error: Box::new(u32_type()),
    }
}

fn composed_id(child: u8, kind: u8, index: u16) -> EntityId {
    let mut bytes = [0_u8; 32];
    bytes[0] = 0xce;
    bytes[1] = child;
    bytes[2] = kind;
    bytes[3..5].copy_from_slice(&index.to_be_bytes());
    EntityId::from_bytes(bytes)
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
            sley_ssmc::ResultConst::Ok(value) | sley_ssmc::ResultConst::Err(value) => {
                remap_constant(value, ids);
            }
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
    child: u8,
    kind: u8,
    values: impl IntoIterator<Item = EntityId>,
) {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_unstable();
    for (index, entity) in values.into_iter().enumerate() {
        let index = u16::try_from(index).expect("bounded checker category fits u16");
        assert!(
            ids.insert(entity, composed_id(child, kind, index))
                .is_none(),
            "checker entity belongs to one category"
        );
    }
}

fn rebase_mapping(image: &CheckerScaffold, child: u8) -> BTreeMap<EntityId, EntityId> {
    let mut ids = BTreeMap::new();
    insert_category(
        &mut ids,
        child,
        1,
        image.functions.iter().map(|value| value.entity_id),
    );
    insert_category(
        &mut ids,
        child,
        2,
        image.parameters.iter().map(|value| value.entity_id),
    );
    insert_category(
        &mut ids,
        child,
        3,
        image.blocks.iter().map(|value| value.entity_id),
    );
    insert_category(
        &mut ids,
        child,
        4,
        image.operations.iter().map(|value| value.entity_id),
    );
    insert_category(
        &mut ids,
        child,
        5,
        image.constants.iter().map(|value| value.entity_id),
    );
    ids
}

fn rebase_checker(mut image: CheckerScaffold, child: u8) -> CheckerScaffold {
    let ids = rebase_mapping(&image, child);
    let remap_graph = |graph: &mut FunctionGraph| {
        graph.entity_id = mapped(&ids, graph.entity_id);
        graph
            .parameters
            .iter_mut()
            .for_each(|entity| *entity = mapped(&ids, *entity));
        remap_type(&mut graph.result_type, &ids);
        graph
            .effects
            .iter_mut()
            .for_each(|entity| *entity = mapped(&ids, *entity));
        graph.entry_block = mapped(&ids, graph.entry_block);
        graph
            .blocks
            .iter_mut()
            .for_each(|entity| *entity = mapped(&ids, *entity));
        graph
            .contracts
            .iter_mut()
            .for_each(|entity| *entity = mapped(&ids, *entity));
    };
    remap_graph(&mut image.entry);
    image.functions.iter_mut().for_each(remap_graph);
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
            .for_each(|operand| remap_value(operand, &ids));
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
    for constant in &mut image.constants {
        constant.entity_id = mapped(&ids, constant.entity_id);
        remap_constant(&mut constant.value, &ids);
    }
    image
}

fn child_programs() -> Vec<CheckerScaffold> {
    [
        single_bool_cfg_checker(),
        option_switch_cfg_checker(),
        ordered_operation_inventory_checker(),
        unary_type_chain_checker(),
        single_effect_closure_checker(),
        effect_set_inventory_checker(),
        two_function_effect_closure_checker(),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, child)| {
        rebase_checker(
            child,
            u8::try_from(index + 1).expect("seven checker children fit u8"),
        )
    })
    .collect()
}

fn result_payload_type(result: &TypeExpr) -> TypeExpr {
    let TypeExpr::Result { ok, error } = result else {
        panic!("checker child must return Result")
    };
    assert_eq!(**error, u32_type());
    (**ok).clone()
}

fn emit_constant_ref(
    assembler: &mut InventoryCheckAssembler,
    block: EntityId,
    constant: EntityId,
    value_type: TypeExpr,
) -> ValueRef {
    inventory_operation_value(assembler.constant_ref(block, constant, value_type))
}

#[derive(Clone, Copy)]
struct NormalizeConstants {
    selector: EntityId,
    zero_u32: EntityId,
    zero_u64: EntityId,
}

fn emit_normalized_success(
    assembler: &mut InventoryCheckAssembler,
    function: EntityId,
    block: EntityId,
    payload: EntityId,
    constants: NormalizeConstants,
    child_result: &TypeExpr,
) {
    let payload_type = result_payload_type(child_result);
    let selector = assembler.constant_ref(block, constants.selector, u8_type());
    let get = |assembler: &mut InventoryCheckAssembler, index, value_type| {
        inventory_operation_value(assembler.operation(
            block,
            Opcode::TupleGet,
            vec![ValueRef::Parameter(payload)],
            value_type,
            Immediate::Index(index),
        ))
    };
    let mut fields = vec![inventory_operation_value(selector)];
    if payload_type == check_plan_type() {
        fields.extend([
            emit_constant_ref(assembler, block, constants.zero_u64, u64_type()),
            get(assembler, 0, u32_type()),
            get(assembler, 1, u32_type()),
            get(assembler, 2, u64_type()),
        ]);
    } else if payload_type == u64_type() {
        fields.extend([
            ValueRef::Parameter(payload),
            emit_constant_ref(assembler, block, constants.zero_u32, u32_type()),
            emit_constant_ref(assembler, block, constants.zero_u32, u32_type()),
            emit_constant_ref(assembler, block, constants.zero_u64, u64_type()),
        ]);
    } else {
        assert_eq!(payload_type, effect_summary_type());
        fields.extend([
            get(assembler, 0, u64_type()),
            get(assembler, 1, u32_type()),
            get(assembler, 2, u32_type()),
            get(assembler, 3, u64_type()),
        ]);
    }
    let tuple = assembler.operation(
        block,
        Opcode::TupleNew,
        fields,
        composed_plan_type(),
        Immediate::None,
    );
    let accepted = assembler.operation(
        block,
        Opcode::ResultOk,
        vec![inventory_operation_value(tuple)],
        composed_result_type(),
        Immediate::None,
    );
    let operations = assembler
        .operations
        .iter()
        .filter(|operation| operation.block == block)
        .map(|operation| operation.entity_id)
        .collect();
    assembler.push_block(
        block,
        function,
        vec![payload],
        operations,
        Terminator::Return(ReturnTerminator {
            value: inventory_operation_value(accepted),
        }),
    );
}

fn append_wrapper_parameters(
    assembler: &mut InventoryCheckAssembler,
    function: EntityId,
    selector: EntityId,
    children: &[CheckerScaffold],
) -> (Vec<EntityId>, Vec<Vec<EntityId>>) {
    let mut ordinal = 1_u32;
    let mut child_arguments = Vec::new();
    let mut graph_parameters = vec![selector];
    for child in children {
        let parameter_index = child
            .parameters
            .iter()
            .map(|parameter| (parameter.entity_id, parameter))
            .collect::<BTreeMap<_, _>>();
        let mut arguments = Vec::new();
        for parameter in &child.entry.parameters {
            let value_type = parameter_index[parameter].value_type.clone();
            let wrapper_parameter =
                assembler.parameter(function, ParameterRole::Function, ordinal, value_type);
            ordinal = ordinal.checked_add(1).expect("bounded wrapper arity");
            graph_parameters.push(wrapper_parameter);
            arguments.push(wrapper_parameter);
        }
        child_arguments.push(arguments);
    }
    (graph_parameters, child_arguments)
}

struct WrapperLayout {
    selector_constants: Vec<EntityId>,
    zero_u32: EntityId,
    zero_u64: EntityId,
    invalid_selector: EntityId,
    dispatch_blocks: Vec<EntityId>,
    call_blocks: Vec<EntityId>,
    success_blocks: Vec<EntityId>,
    error_blocks: Vec<EntityId>,
    unknown_block: EntityId,
}

fn allocate_block_set(assembler: &mut InventoryCheckAssembler) -> Vec<EntityId> {
    (0..CHILD_COUNT).map(|_| assembler.block_id()).collect()
}

fn allocate_wrapper_layout(assembler: &mut InventoryCheckAssembler) -> WrapperLayout {
    let selector_constants = (0..CHILD_COUNT)
        .map(|selector| {
            assembler.constant(u8_value(
                u128::try_from(selector).expect("seven selectors fit u128"),
            ))
        })
        .collect();
    let zero_u32 = assembler.constant(u32_value(0));
    let zero_u64 = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(0),
    });
    let invalid_selector = assembler.constant(u32_value(u128::from(INVALID_SELECTOR)));
    WrapperLayout {
        selector_constants,
        zero_u32,
        zero_u64,
        invalid_selector,
        dispatch_blocks: allocate_block_set(assembler),
        call_blocks: allocate_block_set(assembler),
        success_blocks: allocate_block_set(assembler),
        error_blocks: allocate_block_set(assembler),
        unknown_block: assembler.block_id(),
    }
}

fn emit_selector_dispatch(
    assembler: &mut InventoryCheckAssembler,
    function: EntityId,
    selector: EntityId,
    index: usize,
    layout: &WrapperLayout,
) {
    let block = layout.dispatch_blocks[index];
    let expected = assembler.constant_ref(block, layout.selector_constants[index], u8_type());
    let matches = assembler.operation(
        block,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(selector),
            inventory_operation_value(expected),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    let fallback = layout
        .dispatch_blocks
        .get(index + 1)
        .copied()
        .unwrap_or(layout.unknown_block);
    assembler.push_block(
        block,
        function,
        Vec::new(),
        vec![expected, matches],
        inventory_cond(
            inventory_operation_value(matches),
            layout.call_blocks[index],
            Vec::new(),
            fallback,
            Vec::new(),
        ),
    );
}

fn emit_child_call(
    assembler: &mut InventoryCheckAssembler,
    function: EntityId,
    child: &CheckerScaffold,
    arguments: &[EntityId],
    index: usize,
    layout: &WrapperLayout,
) {
    let call = assembler.operation(
        layout.call_blocks[index],
        Opcode::CallDirect,
        arguments.iter().copied().map(ValueRef::Parameter).collect(),
        child.entry.result_type.clone(),
        Immediate::Function(FunctionRefValue {
            function: child.entry.entity_id,
            type_arguments: Vec::new(),
        }),
    );
    let success_payload = assembler.parameter(
        layout.success_blocks[index],
        ParameterRole::Block,
        0,
        result_payload_type(&child.entry.result_type),
    );
    let error_payload = assembler.parameter(
        layout.error_blocks[index],
        ParameterRole::Block,
        0,
        u32_type(),
    );
    assembler.push_block(
        layout.call_blocks[index],
        function,
        Vec::new(),
        vec![call],
        inventory_switch(
            inventory_operation_value(call),
            vec![
                (
                    BuiltinCase::Ok,
                    layout.success_blocks[index],
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    layout.error_blocks[index],
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    emit_normalized_success(
        assembler,
        function,
        layout.success_blocks[index],
        success_payload,
        NormalizeConstants {
            selector: layout.selector_constants[index],
            zero_u32: layout.zero_u32,
            zero_u64: layout.zero_u64,
        },
        &child.entry.result_type,
    );
    let rejected = assembler.operation(
        layout.error_blocks[index],
        Opcode::ResultErr,
        vec![ValueRef::Parameter(error_payload)],
        composed_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        layout.error_blocks[index],
        function,
        vec![error_payload],
        vec![rejected],
        Terminator::Return(ReturnTerminator {
            value: inventory_operation_value(rejected),
        }),
    );
}

fn emit_unknown_selector(
    assembler: &mut InventoryCheckAssembler,
    function: EntityId,
    layout: &WrapperLayout,
) {
    let invalid = assembler.constant_ref(layout.unknown_block, layout.invalid_selector, u32_type());
    let rejected = assembler.operation(
        layout.unknown_block,
        Opcode::ResultErr,
        vec![inventory_operation_value(invalid)],
        composed_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        layout.unknown_block,
        function,
        Vec::new(),
        vec![invalid, rejected],
        Terminator::Return(ReturnTerminator {
            value: inventory_operation_value(rejected),
        }),
    );
}

fn bounded_checker_main(children: &[CheckerScaffold]) -> CheckerScaffold {
    assert_eq!(children.len(), CHILD_COUNT);
    let function = checker_inventory_id(5, 100);
    let mut assembler = InventoryCheckAssembler::new();
    let selector = assembler.parameter(function, ParameterRole::Function, 0, u8_type());
    let (graph_parameters, child_arguments) =
        append_wrapper_parameters(&mut assembler, function, selector, children);
    let layout = allocate_wrapper_layout(&mut assembler);

    for index in 0..CHILD_COUNT {
        emit_selector_dispatch(&mut assembler, function, selector, index, &layout);
        emit_child_call(
            &mut assembler,
            function,
            &children[index],
            &child_arguments[index],
            index,
            &layout,
        );
    }
    emit_unknown_selector(&mut assembler, function, &layout);

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: graph_parameters,
        result_type: composed_result_type(),
        effects: Vec::new(),
        entry_block: layout.dispatch_blocks[0],
        blocks: assembler
            .blocks
            .iter()
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    CheckerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
    }
}

pub(super) fn bounded_checker_program() -> CheckerScaffold {
    let children = child_programs();
    let wrapper = rebase_checker(bounded_checker_main(&children), 0);
    let mut functions = wrapper.functions;
    let entry = wrapper.entry;
    let mut parameters = wrapper.parameters;
    let mut blocks = wrapper.blocks;
    let mut operations = wrapper.operations;
    let mut constants = wrapper.constants;
    for child in children {
        functions.extend(child.functions);
        parameters.extend(child.parameters);
        blocks.extend(child.blocks);
        operations.extend(child.operations);
        constants.extend(child.constants);
    }
    functions.sort_unstable_by_key(|value| value.entity_id);
    parameters.sort_unstable_by_key(|value| value.entity_id);
    blocks.sort_unstable_by_key(|value| value.entity_id);
    operations.sort_unstable_by_key(|value| value.entity_id);
    constants.sort_unstable_by_key(|value| value.entity_id);
    let all_ids = functions
        .iter()
        .map(|value| value.entity_id)
        .chain(parameters.iter().map(|value| value.entity_id))
        .chain(blocks.iter().map(|value| value.entity_id))
        .chain(operations.iter().map(|value| value.entity_id))
        .chain(constants.iter().map(|value| value.entity_id))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        all_ids.len(),
        functions.len() + parameters.len() + blocks.len() + operations.len() + constants.len()
    );
    CheckerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry,
        functions,
        parameters,
        blocks,
        operations,
        constants,
    }
}

fn bool_value(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

fn u64_value(value: u64) -> ConstValue {
    ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::from(value)),
    }
}

fn valid_child_inputs() -> Vec<Vec<ConstValue>> {
    let option_tag = u64::from(TypeExpr::Option(Box::new(TypeExpr::Bool)).tag());
    let vector_tag = u64::from(TypeExpr::Vector(Box::new(TypeExpr::Bool)).tag());
    let cell_tag = u64::from(TypeExpr::LocalCell(Box::new(TypeExpr::Bool)).tag());
    let uint_tag = u64::from(u32_type().tag());
    vec![
        vec![
            u32_value(2),
            bool_value(true),
            u32_value(1),
            u32_value(0),
            bool_value(true),
            bool_value(true),
            u32_value(0),
            bool_value(true),
        ],
        vec![
            bool_value(true),
            u32_value(u128::from(option_tag)),
            u32_value(2),
            u32_value(u128::from(BuiltinCase::None.tag())),
            u32_value(u128::from(BuiltinCase::Some.tag())),
            bool_value(false),
            bool_value(true),
        ],
        vec![
            cfg_inventory_value(&[(0, 1, 0), (1, 2, 0), (2, 2, 1)]),
            u64_value(2),
        ],
        vec![
            u64vec_value(&[vector_tag, option_tag, cell_tag]),
            u64_value(uint_tag),
            u64_value(32),
            u64_value(0),
        ],
        vec![
            bool_value(true),
            u64_value(20),
            bool_value(true),
            u64_value(20),
            bool_value(true),
            u64_value(20),
        ],
        vec![
            u64vec_value(&[10, 20, 30]),
            u64vec_value(&[10, 30]),
            u64vec_value(&[10, 30]),
        ],
        vec![
            bool_value(true),
            bool_value(true),
            bool_value(true),
            bool_value(true),
            bool_value(true),
        ],
    ]
}

fn composed_inputs(selector: u8, children: &[Vec<ConstValue>]) -> Vec<ConstValue> {
    let mut inputs = vec![u8_value(u128::from(selector))];
    inputs.extend(children.iter().flatten().cloned());
    inputs
}

fn execute_inputs(scaffold: &CheckerScaffold, inputs: Vec<ConstValue>) -> ConstValue {
    let (package, approved) = admit_checker_program(scaffold);
    let outcome = sley_vm::execute_approved_package_v2(
        &package,
        &approved,
        sley_vm::ExecutionRequest {
            inputs,
            limits: generous_limits(),
        },
    )
    .expect("v2 executes bounded checker program");
    let sley_vm::ExecutionTermination::Success(value) = outcome.termination else {
        panic!("bounded checker must return a typed result")
    };
    value
}

fn normalized_child_result(selector: u8, child: ConstValue) -> ConstValue {
    let ConstData::Result(result) = child.data else {
        panic!("checker child returns Result")
    };
    let data = match result {
        sley_ssmc::ResultConst::Err(error) => ConstData::Result(sley_ssmc::ResultConst::Err(error)),
        sley_ssmc::ResultConst::Ok(payload) => {
            let mut fields = vec![u8_value(u128::from(selector))];
            if payload.value_type == check_plan_type() {
                let ConstData::Sequence(mut report) = payload.data else {
                    panic!("CFG report is a tuple")
                };
                fields.push(u64_value(0));
                fields.append(&mut report);
            } else if payload.value_type == u64_type() {
                fields.extend([*payload, u32_value(0), u32_value(0), u64_value(0)]);
            } else {
                assert_eq!(payload.value_type, effect_summary_type());
                let ConstData::Sequence(report) = payload.data else {
                    panic!("effect report is a tuple")
                };
                fields.extend(report);
            }
            ConstData::Result(sley_ssmc::ResultConst::Ok(Box::new(ConstValue {
                value_type: composed_plan_type(),
                data: ConstData::Sequence(fields),
            })))
        }
    };
    ConstValue {
        value_type: composed_result_type(),
        data,
    }
}

fn assert_selected_child_parity(selector: u8, inputs: &[Vec<ConstValue>]) {
    let children = child_programs();
    let direct = execute_inputs(
        &children[usize::from(selector)],
        inputs[usize::from(selector)].clone(),
    );
    let composed = execute_inputs(
        &bounded_checker_program(),
        composed_inputs(selector, inputs),
    );
    assert_eq!(composed, normalized_child_result(selector, direct));
}

#[test]
fn bounded_checker_main_dispatches_and_normalizes_all_seven_slices() {
    let inputs = valid_child_inputs();
    for selector in 0..u8::try_from(CHILD_COUNT).expect("seven selectors fit u8") {
        assert_selected_child_parity(selector, &inputs);
    }
    let program = bounded_checker_program();
    assert_eq!(program.functions.len(), 8);
    assert_eq!(program.parameters.len(), 209);
    assert_eq!(program.blocks.len(), 181);
    assert_eq!(program.operations.len(), 346);
    assert_eq!(program.constants.len(), 85);
    eprintln!(
        "RW100_BOUNDED_CHECKER functions={} parameters={} blocks={} operations={} constants={}",
        program.functions.len(),
        program.parameters.len(),
        program.blocks.len(),
        program.operations.len(),
        program.constants.len(),
    );
}

#[test]
fn bounded_checker_main_forwards_frozen_errors_and_refuses_unknown_selector() {
    let mut cfg_error = valid_child_inputs();
    cfg_error[0][0] = u32_value(1);
    assert_selected_child_parity(0, &cfg_error);

    let mut type_error = valid_child_inputs();
    type_error[3][2] = u64_value(24);
    assert_selected_child_parity(3, &type_error);

    let mut effect_error = valid_child_inputs();
    effect_error[5][0] = u64vec_value(&[20, 10]);
    assert_selected_child_parity(5, &effect_error);

    let unknown = execute_inputs(
        &bounded_checker_program(),
        composed_inputs(7, &valid_child_inputs()),
    );
    assert_eq!(
        unknown,
        ConstValue {
            value_type: composed_result_type(),
            data: ConstData::Result(sley_ssmc::ResultConst::Err(Box::new(u32_value(
                u128::from(INVALID_SELECTOR,)
            )))),
        }
    );
}
