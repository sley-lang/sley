//! Successor driver that hands the lowerer's bytes directly to the package builder.

use super::{checker, codec, component, lower};
use sley_id::{CandidateNonce, EntityId, GenesisNonce, WorkspaceId};
use sley_ssmc::{
    Block, BuiltinCase, CaseKey, ConstData, ConstValue, FunctionGraph, FunctionRefValue, Immediate,
    IntegerWidth, Opcode, Operation, OperationResultRef, Parameter, ParameterRole, Reachability,
    ReturnTerminator, SwitchArgument, SwitchCase, SwitchEdge, Terminator, TypeExpr, ValueRef,
    VariantSwitchTerminator, Visibility,
};
use std::collections::BTreeMap;

const GENESIS_SEED: [u8; 32] = [0x80; 32];
const CANDIDATE_SEED: [u8; 32] = [0x87; 32];
const HANDOFF_BASE: u64 = 100_000;

pub(super) struct HandoffFixture {
    pub(super) program: component::MergedProgram,
    pub(super) entry: EntityId,
    pub(super) inputs: Vec<ConstValue>,
    pub(super) expected: ConstValue,
}

struct ParameterizedInputs {
    parameters: Vec<Parameter>,
    operands: Vec<Vec<ValueRef>>,
    inputs: Vec<ConstValue>,
}

fn workspace() -> WorkspaceId {
    WorkspaceId::derive(GenesisNonce::from_bytes(GENESIS_SEED))
}

fn candidate() -> CandidateNonce {
    CandidateNonce::from_bytes(CANDIDATE_SEED)
}

fn derived_id(kind: u32, ordinal: u64) -> EntityId {
    EntityId::derive(workspace(), candidate(), kind, ordinal)
}

fn operation_result(operation: EntityId) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation,
        result_index: 0,
    })
}

fn parameterized_inputs(entry: EntityId, groups: Vec<Vec<ConstValue>>) -> ParameterizedInputs {
    let mut parameters = Vec::new();
    let mut operands = Vec::new();
    let mut inputs = Vec::new();
    for group in groups {
        let mut group_operands = Vec::new();
        for input in group {
            let ordinal = u32::try_from(parameters.len()).unwrap();
            let entity_id = derived_id(6, HANDOFF_BASE + u64::from(ordinal));
            parameters.push(Parameter {
                entity_id,
                owner: entry,
                role: ParameterRole::Function,
                ordinal,
                value_type: input.value_type.clone(),
            });
            group_operands.push(ValueRef::Parameter(entity_id));
            inputs.push(input);
        }
        operands.push(group_operands);
    }
    ParameterizedInputs {
        parameters,
        operands,
        inputs,
    }
}

fn direct_call(
    index: u64,
    block: EntityId,
    ordinal: u32,
    function: &FunctionGraph,
    operands: Vec<ValueRef>,
) -> Operation {
    Operation {
        entity_id: derived_id(8, HANDOFF_BASE + index),
        block,
        ordinal,
        opcode: Opcode::CallDirect,
        operands,
        result_types: vec![function.result_type.clone()],
        immediate: Immediate::Function(FunctionRefValue {
            function: function.entity_id,
            type_arguments: Vec::new(),
        }),
    }
}

fn result_operation(
    index: u64,
    block: EntityId,
    ordinal: u32,
    opcode: Opcode,
    operand: EntityId,
    result_type: TypeExpr,
) -> Operation {
    Operation {
        entity_id: derived_id(8, HANDOFF_BASE + index),
        block,
        ordinal,
        opcode,
        operands: vec![ValueRef::Parameter(operand)],
        result_types: vec![result_type],
        immediate: Immediate::None,
    }
}

fn tuple_operation(
    index: u64,
    block: EntityId,
    ordinal: u32,
    values: &[EntityId],
    result_type: TypeExpr,
) -> Operation {
    Operation {
        entity_id: derived_id(8, HANDOFF_BASE + index),
        block,
        ordinal,
        opcode: Opcode::TupleNew,
        operands: values.iter().copied().map(operation_result).collect(),
        result_types: vec![result_type],
        immediate: Immediate::None,
    }
}

#[derive(Clone, Copy)]
struct HandoffLayout {
    entry_block: EntityId,
    success_block: EntityId,
    error_block: EntityId,
    success_payload: EntityId,
    error_payload: EntityId,
}

struct HandoffOperations {
    entry: [Operation; 3],
    success: [Operation; 3],
    error: [Operation; 3],
}

fn handoff_layout(parameters: &mut Vec<Parameter>) -> HandoffLayout {
    let entry_block = derived_id(7, HANDOFF_BASE);
    let success_block = derived_id(7, HANDOFF_BASE + 1);
    let error_block = derived_id(7, HANDOFF_BASE + 2);
    let next_parameter = u64::try_from(parameters.len()).unwrap();
    let success_payload = derived_id(6, HANDOFF_BASE + next_parameter);
    let error_payload = derived_id(6, HANDOFF_BASE + next_parameter + 1);
    parameters.extend([
        Parameter {
            entity_id: success_payload,
            owner: success_block,
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: TypeExpr::Bytes,
        },
        Parameter {
            entity_id: error_payload,
            owner: error_block,
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: TypeExpr::UInt(IntegerWidth::from_bits(32)),
        },
    ]);
    HandoffLayout {
        entry_block,
        success_block,
        error_block,
        success_payload,
        error_payload,
    }
}

fn entry_operations(
    functions: &[&FunctionGraph],
    operands: &[Vec<ValueRef>],
    layout: HandoffLayout,
) -> [Operation; 3] {
    [
        direct_call(0, layout.entry_block, 0, functions[0], operands[0].clone()),
        direct_call(1, layout.entry_block, 1, functions[1], operands[1].clone()),
        direct_call(2, layout.entry_block, 2, functions[2], operands[2].clone()),
    ]
}

fn success_operations(
    functions: &[&FunctionGraph],
    operands: &[Vec<ValueRef>],
    entry: &[Operation; 3],
    layout: HandoffLayout,
    result_type: &TypeExpr,
) -> [Operation; 3] {
    let lower_ok = result_operation(
        3,
        layout.success_block,
        0,
        Opcode::ResultOk,
        layout.success_payload,
        functions[2].result_type.clone(),
    );
    let mut builder_operands = vec![ValueRef::Parameter(layout.success_payload)];
    builder_operands.extend(operands[3].clone());
    let builder_call = direct_call(4, layout.success_block, 1, functions[3], builder_operands);
    let success_tuple = tuple_operation(
        5,
        layout.success_block,
        2,
        &[
            entry[0].entity_id,
            entry[1].entity_id,
            lower_ok.entity_id,
            builder_call.entity_id,
        ],
        result_type.clone(),
    );
    [lower_ok, builder_call, success_tuple]
}

fn error_operations(
    functions: &[&FunctionGraph],
    entry: &[Operation; 3],
    layout: HandoffLayout,
    result_type: &TypeExpr,
) -> [Operation; 3] {
    let lower_error = result_operation(
        6,
        layout.error_block,
        0,
        Opcode::ResultErr,
        layout.error_payload,
        functions[2].result_type.clone(),
    );
    let builder_error = result_operation(
        7,
        layout.error_block,
        1,
        Opcode::ResultErr,
        layout.error_payload,
        functions[3].result_type.clone(),
    );
    let error_tuple = tuple_operation(
        8,
        layout.error_block,
        2,
        &[
            entry[0].entity_id,
            entry[1].entity_id,
            lower_error.entity_id,
            builder_error.entity_id,
        ],
        result_type.clone(),
    );
    [lower_error, builder_error, error_tuple]
}

fn handoff_blocks(
    entry: EntityId,
    layout: HandoffLayout,
    operations: &HandoffOperations,
) -> [Block; 3] {
    let [codec_call, checker_call, lower_call] = &operations.entry;
    let [lower_ok, builder_call, success_tuple] = &operations.success;
    let [lower_error, builder_error, error_tuple] = &operations.error;
    [
        Block {
            entity_id: layout.entry_block,
            function: entry,
            parameters: Vec::new(),
            operations: vec![
                codec_call.entity_id,
                checker_call.entity_id,
                lower_call.entity_id,
            ],
            terminator: Terminator::VariantSwitch(VariantSwitchTerminator {
                value: operation_result(lower_call.entity_id),
                cases: vec![
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Ok),
                        edge: SwitchEdge {
                            target: layout.success_block,
                            arguments: vec![SwitchArgument::CasePayload],
                        },
                    },
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Err),
                        edge: SwitchEdge {
                            target: layout.error_block,
                            arguments: vec![SwitchArgument::CasePayload],
                        },
                    },
                ],
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: layout.success_block,
            function: entry,
            parameters: vec![layout.success_payload],
            operations: vec![
                lower_ok.entity_id,
                builder_call.entity_id,
                success_tuple.entity_id,
            ],
            terminator: Terminator::Return(ReturnTerminator {
                value: operation_result(success_tuple.entity_id),
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: layout.error_block,
            function: entry,
            parameters: vec![layout.error_payload],
            operations: vec![
                lower_error.entity_id,
                builder_error.entity_id,
                error_tuple.entity_id,
            ],
            terminator: Terminator::Return(ReturnTerminator {
                value: operation_result(error_tuple.entity_id),
            }),
            reachability: Reachability::Required,
        },
    ]
}

fn append_handoff_graph(
    program: &mut component::MergedProgram,
    entry: EntityId,
    mut parameters: Vec<Parameter>,
    operands: &[Vec<ValueRef>],
    result_type: TypeExpr,
) {
    let function_by_id = program
        .functions
        .iter()
        .map(|function| (function.entity_id, function))
        .collect::<BTreeMap<_, _>>();
    let functions = program
        .entry_points
        .iter()
        .map(|id| function_by_id[id])
        .collect::<Vec<_>>();
    let layout = handoff_layout(&mut parameters);
    let entry_operations = entry_operations(&functions, operands, layout);
    let success = success_operations(
        &functions,
        operands,
        &entry_operations,
        layout,
        &result_type,
    );
    let error = error_operations(&functions, &entry_operations, layout, &result_type);
    let operations = HandoffOperations {
        entry: entry_operations,
        success,
        error,
    };
    let function_parameters = parameters
        .iter()
        .filter(|parameter| parameter.owner == entry)
        .map(|parameter| parameter.entity_id)
        .collect::<Vec<_>>();
    program.functions.push(FunctionGraph {
        entity_id: entry,
        type_parameters: Vec::new(),
        parameters: function_parameters,
        result_type,
        effects: Vec::new(),
        entry_block: layout.entry_block,
        blocks: vec![layout.entry_block, layout.success_block, layout.error_block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    });
    program
        .blocks
        .extend(handoff_blocks(entry, layout, &operations));
    program.operations.extend(
        operations
            .entry
            .into_iter()
            .chain(operations.success)
            .chain(operations.error),
    );
    program.parameters.extend(parameters);
    program.entry_points.push(entry);
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
}

pub(super) fn fixture() -> HandoffFixture {
    fixture_over(component::merged_program())
}

/// The handoff driver over the bounded-codec union that preceded the
/// 2026-09-18 codec re-mint, retained as history.
pub(super) fn bounded_fixture() -> HandoffFixture {
    fixture_over(component::bounded_merged_program())
}

fn fixture_over(mut program: component::MergedProgram) -> HandoffFixture {
    let entry = derived_id(5, HANDOFF_BASE);
    let (codec_inputs, expected_codec) = codec::integration_codec_test();
    let (checker_inputs, expected_checker) = checker::integration_checker_test();
    let handoff = lower::integration_lower_builder_handoff_test();
    let groups = vec![
        codec_inputs,
        checker_inputs,
        handoff.lower_inputs,
        handoff.builder_tail_inputs,
    ];
    let ParameterizedInputs {
        parameters,
        operands,
        inputs,
    } = parameterized_inputs(entry, groups);
    let expected_values = vec![
        expected_codec,
        expected_checker,
        handoff.expected_lower,
        handoff.expected_builder,
    ];
    let result_type = TypeExpr::Tuple(
        expected_values
            .iter()
            .map(|value| value.value_type.clone())
            .collect(),
    );
    append_handoff_graph(
        &mut program,
        entry,
        parameters,
        &operands,
        result_type.clone(),
    );
    HandoffFixture {
        program,
        entry,
        inputs,
        expected: ConstValue {
            value_type: result_type,
            data: ConstData::Sequence(expected_values),
        },
    }
}
