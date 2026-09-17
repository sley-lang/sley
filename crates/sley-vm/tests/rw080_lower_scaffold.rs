//! RW-080 lowerer construction (§1.3): scaffold plus single-op algorithm.
//!
//! PROVISIONAL C0 SEED SCAFFOLD — explicitly not accepted runtime
//! authority. Seed-authored (this file is the seed-assembler record)
//! under the operator development override; it exercises the v2
//! admit/approve/execute path for the third RW-080 toolchain module
//! The original marker scaffold remains intact. Per contract §1.3, it is
//! the entry plus the error vocabulary with traps on real inputs:
//! marker 0 returns the trivial-accept value (the one value-returning
//! success exit), markers 1..=7 trap (`TrapCode::Unreachable` with the
//! lowering-leg index as payload, proving dispatch reached the leg),
//! and any other marker returns a typed `LoweringError`-family value.
//! The witness bytes are unread by design (proven: distinct witness
//! bytes behave identically), so the scaffold lowers nothing, checks
//! nothing, and assembles no package. The `build_package` entry is
//! deliberately absent: `BuildError` has no frozen native vocabulary
//! (zero hits repo-wide), so a builder scaffold would invent codes.
//! The later `single_bool_lowerer` is the first real bounded algorithm:
//! inputs supplied after admission select BoolNot/BoolAnd/BoolOr and their
//! checked arity; Sley derives the dense operand and result registers and
//! returns frozen lowering errors. Tests compare the derived model with the
//! native reference lowerer. It does not yet encode SLEYBC02 bytes.
//! The ordered-inventory slice advances that algorithm with a real CFG
//! backedge over runtime rows. Sley validates every row and advances the
//! dense-register frontier internally, emits the ordered typed instruction
//! model, and preserves frozen late-row failures. Its scalar-family extension
//! adds all six equality/ordering opcodes and all eight checked-integer
//! opcodes, the unary/binary floating family, value constructors, local-cell
//! operations, and value hashing to the same traversal.
//! A second runtime-vector path walks arbitrary operand lists for FMA, tuple/
//! vector construction and access, and ordered-map construction/access/update
//! operations.
//! The terminator slices lower return, branch, conditional branch, trap, and
//! built-in variant-switch models. They walk every edge or case argument and
//! every runtime switch case in Sley.
//! Construction provenance:
//! machineresearch/sley-2.0/reweave/rw-080-lower-scaffold.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-single-op.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-inventory.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-scalar-inventory.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-checked-integers.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-floating.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-values-cells.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-variadic.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-immediate-free.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-map-construction.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-bootstrap-immediates.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-exact-immediates.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-instruction-map.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-immediate-inventory.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-mixed-inventory.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-simple-block.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-complete-block.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-function-blocks.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-function-metadata.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-fixed-width-bytes.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-byte-chunks.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-function-header.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-instruction-bytes.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-instruction-map-bytes.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-simple-terminator-bytes.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-switch-terminator-bytes.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-complete-terminator-bytes.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-complete-block-bytes.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-block-map-bytes.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-function-body-bytes.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-root-image.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-callee-table.md and
//! machineresearch/sley-2.0/reweave/rw-080-package-empty-sections.md and
//! machineresearch/sley-2.0/reweave/rw-080-package-envelope-compose.md and
//! machineresearch/sley-2.0/reweave/rw-080-package-inventory-rows.md and
//! machineresearch/sley-2.0/reweave/rw-080-package-builder.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-terminators.md.

use std::collections::{BTreeMap, BTreeSet};

use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, BranchTerminator, BuiltinCase, BuiltinFailureKind, CaseKey,
    CondBranchTerminator, ConstData, ConstValue, ConstantDefinition, ContractDefinition,
    ContractKind, FunctionGraph, FunctionRefValue, GlobalValueDefinition, Immediate, IntegerWidth,
    MemberId, NamedType, Opcode, Operation, OperationResultRef, Parameter, ParameterRole,
    Reachability, RecordField, ReturnTerminator, SwitchArgument, SwitchCase, SwitchEdge,
    TargetEdge, Terminator, TrapCode, TrapTerminator, TypeDefForm, TypeDefinition, TypeExpr,
    ValueRef, VariantCase, VariantImmediate, VariantSwitchTerminator, Visibility,
};

fn id(byte: u8) -> EntityId {
    EntityId::from_bytes([byte; 32])
}

fn epoch() -> SchemaEpochId {
    SchemaEpochId::from_bytes([8; 32])
}

fn root() -> StateRoot {
    StateRoot::from_bytes([9; 32])
}

fn u32_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(32))
}

fn u64_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(64))
}

fn u32vec_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(u32_type()))
}

fn u64vec_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(u64_type()))
}

fn u8_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(8))
}

fn u8vec_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(u8_type()))
}

fn bytesvec_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(TypeExpr::Bytes))
}

fn single_lowered_type() -> TypeExpr {
    TypeExpr::Tuple(vec![u32_type(), u32vec_type(), u32vec_type()])
}

fn single_lower_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(single_lowered_type()),
        error: Box::new(u32_type()),
    }
}

fn bool_inventory_row_type() -> TypeExpr {
    TypeExpr::Tuple(vec![u32_type(), u32_type(), u32_type(), u32_type()])
}

fn bool_inventory_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(bool_inventory_row_type()))
}

fn inventory_summary_type() -> TypeExpr {
    TypeExpr::Tuple(vec![inventory_model_type(), u64_type(), u32_type()])
}

fn inventory_model_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(single_lowered_type()))
}

fn inventory_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(inventory_summary_type()),
        error: Box::new(u32_type()),
    }
}

fn variadic_summary_type() -> TypeExpr {
    TypeExpr::Tuple(vec![single_lowered_type(), u32_type()])
}

fn variadic_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(variadic_summary_type()),
        error: Box::new(u32_type()),
    }
}

fn immediate_instruction_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        u32_type(),
        u32vec_type(),
        u32vec_type(),
        u32_type(),
        u64_type(),
        u64_type(),
        TypeExpr::Bytes,
    ])
}

fn immediate_summary_type() -> TypeExpr {
    TypeExpr::Tuple(vec![immediate_instruction_type(), u32_type()])
}

fn immediate_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(immediate_summary_type()),
        error: Box::new(u32_type()),
    }
}

fn immediate_inventory_row_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        u32_type(),
        u32vec_type(),
        u32_type(),
        u64_type(),
        u64_type(),
        TypeExpr::Bytes,
    ])
}

fn immediate_inventory_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(immediate_inventory_row_type()))
}

fn immediate_inventory_model_type() -> TypeExpr {
    TypeExpr::OrderedMap {
        key: Box::new(u64_type()),
        value: Box::new(immediate_instruction_type()),
    }
}

fn empty_immediate_inventory_model_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(immediate_inventory_model_type()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    }
}

fn immediate_inventory_summary_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        immediate_inventory_model_type(),
        u64_type(),
        u32_type(),
    ])
}

fn immediate_inventory_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(immediate_inventory_summary_type()),
        error: Box::new(u32_type()),
    }
}

fn arithmetic_result_type(inner: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(inner),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    }
}

fn index_result_type(inner: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(inner),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
    }
}

fn unit_lower_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Unit),
        error: Box::new(u32_type()),
    }
}

fn optional_u32_type() -> TypeExpr {
    TypeExpr::Option(Box::new(u32_type()))
}

fn terminator_model_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        u32_type(),
        u32_type(),
        u32_type(),
        u32vec_type(),
        u32_type(),
        u32vec_type(),
        optional_u32_type(),
    ])
}

fn terminator_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(terminator_model_type()),
        error: Box::new(u32_type()),
    }
}

fn simple_block_model_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        u32_type(),
        u32vec_type(),
        immediate_inventory_model_type(),
        terminator_model_type(),
        u32_type(),
    ])
}

fn simple_block_summary_type() -> TypeExpr {
    TypeExpr::Tuple(vec![simple_block_model_type(), u32_type()])
}

fn simple_block_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(simple_block_summary_type()),
        error: Box::new(u32_type()),
    }
}

fn complete_terminator_model_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        u32_type(),
        terminator_model_type(),
        builtin_switch_model_type(),
    ])
}

fn complete_block_model_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        u32_type(),
        u32vec_type(),
        immediate_inventory_model_type(),
        complete_terminator_model_type(),
        u32_type(),
    ])
}

fn complete_block_summary_type() -> TypeExpr {
    TypeExpr::Tuple(vec![complete_block_model_type(), u32_type()])
}

fn complete_block_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(complete_block_summary_type()),
        error: Box::new(u32_type()),
    }
}

fn complete_block_fact_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        u32_type(),
        u32vec_type(),
        immediate_inventory_type(),
        u32_type(),
        u32_type(),
        u32_type(),
        u32vec_type(),
        u32_type(),
        u32vec_type(),
        optional_u32_type(),
        builtin_switch_case_facts_type(),
        u32_type(),
    ])
}

fn complete_block_facts_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(complete_block_fact_type()))
}

fn complete_block_map_type() -> TypeExpr {
    TypeExpr::OrderedMap {
        key: Box::new(u32_type()),
        value: Box::new(complete_block_model_type()),
    }
}

fn empty_complete_block_map_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(complete_block_map_type()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    }
}

fn complete_function_model_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        TypeExpr::Bytes,
        u32vec_type(),
        bytesvec_type(),
        TypeExpr::Bytes,
        u32_type(),
        complete_block_map_type(),
        u32_type(),
    ])
}

fn complete_function_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(complete_function_model_type()),
        error: Box::new(u32_type()),
    }
}

fn complete_function_fact_type() -> TypeExpr {
    TypeExpr::Tuple(vec![
        TypeExpr::Bytes,
        u32vec_type(),
        bytesvec_type(),
        TypeExpr::Bytes,
        u32_type(),
        u32_type(),
        complete_block_facts_type(),
    ])
}

fn complete_function_facts_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(complete_function_fact_type()))
}

fn byte_vector_lower_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(u8vec_type()),
        error: Box::new(u32_type()),
    }
}

fn bytes_lower_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Bytes),
        error: Box::new(u32_type()),
    }
}

fn package_sections_type() -> TypeExpr {
    TypeExpr::Tuple(vec![TypeExpr::Bytes; 4])
}

fn package_sections_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(package_sections_type()),
        error: Box::new(u32_type()),
    }
}

fn dense_register_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(u32_type()),
        error: Box::new(u32_type()),
    }
}

fn switch_argument_fact_type() -> TypeExpr {
    TypeExpr::Tuple(vec![u32_type(), u32_type()])
}

fn switch_argument_facts_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(switch_argument_fact_type()))
}

fn builtin_switch_case_fact_type() -> TypeExpr {
    TypeExpr::Tuple(vec![u32_type(), u32_type(), switch_argument_facts_type()])
}

fn builtin_switch_case_facts_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(builtin_switch_case_fact_type()))
}

fn builtin_switch_model_type() -> TypeExpr {
    TypeExpr::Tuple(vec![u32_type(), builtin_switch_case_facts_type()])
}

fn builtin_switch_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(builtin_switch_model_type()),
        error: Box::new(u32_type()),
    }
}

fn u32_value(n: u128) -> ConstValue {
    ConstValue {
        value_type: u32_type(),
        data: ConstData::UInt(n),
    }
}

fn bool_value(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

fn bool_inventory_value(rows: &[(u32, u32, u32, u32)]) -> ConstValue {
    ConstValue {
        value_type: bool_inventory_type(),
        data: ConstData::Sequence(
            rows.iter()
                .map(|(opcode, arity, operand_zero, operand_one)| ConstValue {
                    value_type: bool_inventory_row_type(),
                    data: ConstData::Sequence(vec![
                        u32_value(u128::from(*opcode)),
                        u32_value(u128::from(*arity)),
                        u32_value(u128::from(*operand_zero)),
                        u32_value(u128::from(*operand_one)),
                    ]),
                })
                .collect(),
        ),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImmediateInventoryFact {
    opcode: u32,
    operands: Vec<u32>,
    immediate_tag: u32,
    primary: u64,
    secondary: u64,
    immediate_bytes: Vec<u8>,
}

fn immediate_inventory_value(rows: &[ImmediateInventoryFact]) -> ConstValue {
    ConstValue {
        value_type: immediate_inventory_type(),
        data: ConstData::Sequence(
            rows.iter()
                .map(|row| ConstValue {
                    value_type: immediate_inventory_row_type(),
                    data: ConstData::Sequence(vec![
                        u32_value(u128::from(row.opcode)),
                        u32vec_value(&row.operands),
                        u32_value(u128::from(row.immediate_tag)),
                        u64_value(u128::from(row.primary)),
                        u64_value(u128::from(row.secondary)),
                        bytes_value(&row.immediate_bytes),
                    ]),
                })
                .collect(),
        ),
    }
}

fn u32vec_value(values: &[u32]) -> ConstValue {
    ConstValue {
        value_type: u32vec_type(),
        data: ConstData::Sequence(
            values
                .iter()
                .map(|value| u32_value(u128::from(*value)))
                .collect(),
        ),
    }
}

fn u8vec_value(values: &[u8]) -> ConstValue {
    ConstValue {
        value_type: u8vec_type(),
        data: ConstData::Sequence(
            values
                .iter()
                .map(|value| ConstValue {
                    value_type: u8_type(),
                    data: ConstData::UInt(u128::from(*value)),
                })
                .collect(),
        ),
    }
}

fn bytesvec_value(values: &[Vec<u8>]) -> ConstValue {
    ConstValue {
        value_type: bytesvec_type(),
        data: ConstData::Sequence(values.iter().map(|value| bytes_value(value)).collect()),
    }
}

fn optional_u32_value(value: Option<u32>) -> ConstValue {
    ConstValue {
        value_type: optional_u32_type(),
        data: ConstData::Option(value.map(|value| Box::new(u32_value(u128::from(value))))),
    }
}

type BuiltinSwitchFact = (u32, u32, Vec<(u32, u32)>);

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompleteBlockFact {
    slot: u32,
    parameter_registers: Vec<u32>,
    instructions: Vec<ImmediateInventoryFact>,
    terminator: sley_vm::BytecodeTerminator,
    reachability: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompleteFunctionFact {
    identity: Vec<u8>,
    parameter_registers: Vec<u32>,
    register_types: Vec<Vec<u8>>,
    result_type: Vec<u8>,
    entry_slot: u32,
    block_count: u32,
    blocks: Vec<CompleteBlockFact>,
}

fn complete_function_fact(function: &sley_vm::BytecodeFunction) -> CompleteFunctionFact {
    CompleteFunctionFact {
        identity: function.function.as_bytes().to_vec(),
        parameter_registers: function.parameter_registers.clone(),
        register_types: function.register_types.iter().map(encoded_type).collect(),
        result_type: encoded_type(&function.result_type),
        entry_slot: function.entry_block,
        block_count: u32::try_from(function.blocks.len()).expect("fixture block count fits u32"),
        blocks: complete_block_facts(function),
    }
}

fn builtin_switch_facts_value(cases: &[BuiltinSwitchFact]) -> ConstValue {
    ConstValue {
        value_type: builtin_switch_case_facts_type(),
        data: ConstData::Sequence(
            cases
                .iter()
                .map(|(key, target, arguments)| ConstValue {
                    value_type: builtin_switch_case_fact_type(),
                    data: ConstData::Sequence(vec![
                        u32_value(u128::from(*key)),
                        u32_value(u128::from(*target)),
                        ConstValue {
                            value_type: switch_argument_facts_type(),
                            data: ConstData::Sequence(
                                arguments
                                    .iter()
                                    .map(|(tag, value)| ConstValue {
                                        value_type: switch_argument_fact_type(),
                                        data: ConstData::Sequence(vec![
                                            u32_value(u128::from(*tag)),
                                            u32_value(u128::from(*value)),
                                        ]),
                                    })
                                    .collect(),
                            ),
                        },
                    ]),
                })
                .collect(),
        ),
    }
}

fn complete_block_facts_value(blocks: &[CompleteBlockFact]) -> ConstValue {
    ConstValue {
        value_type: complete_block_facts_type(),
        data: ConstData::Sequence(
            blocks
                .iter()
                .map(|block| {
                    let (
                        kind,
                        primary,
                        target_zero,
                        arguments_zero,
                        target_one,
                        arguments_one,
                        payload,
                        cases,
                    ) = if let sley_vm::BytecodeTerminator::VariantSwitch { .. } = &block.terminator
                    {
                        let (selector, cases) = builtin_switch_facts(&block.terminator);
                        (4, selector, 0, Vec::new(), 0, Vec::new(), None, cases)
                    } else {
                        let (
                            kind,
                            primary,
                            target_zero,
                            arguments_zero,
                            target_one,
                            arguments_one,
                            payload,
                        ) = simple_terminator_fact(&block.terminator);
                        (
                            kind,
                            primary,
                            target_zero,
                            arguments_zero,
                            target_one,
                            arguments_one,
                            payload,
                            Vec::new(),
                        )
                    };
                    ConstValue {
                        value_type: complete_block_fact_type(),
                        data: ConstData::Sequence(vec![
                            u32_value(u128::from(block.slot)),
                            u32vec_value(&block.parameter_registers),
                            immediate_inventory_value(&block.instructions),
                            u32_value(u128::from(kind)),
                            u32_value(u128::from(primary)),
                            u32_value(u128::from(target_zero)),
                            u32vec_value(&arguments_zero),
                            u32_value(u128::from(target_one)),
                            u32vec_value(&arguments_one),
                            optional_u32_value(payload),
                            builtin_switch_facts_value(&cases),
                            u32_value(u128::from(block.reachability)),
                        ]),
                    }
                })
                .collect(),
        ),
    }
}

fn complete_function_facts_value(functions: &[CompleteFunctionFact]) -> ConstValue {
    ConstValue {
        value_type: complete_function_facts_type(),
        data: ConstData::Sequence(
            functions
                .iter()
                .map(|function| ConstValue {
                    value_type: complete_function_fact_type(),
                    data: ConstData::Sequence(vec![
                        bytes_value(&function.identity),
                        u32vec_value(&function.parameter_registers),
                        bytesvec_value(&function.register_types),
                        bytes_value(&function.result_type),
                        u32_value(u128::from(function.entry_slot)),
                        u32_value(u128::from(function.block_count)),
                        complete_block_facts_value(&function.blocks),
                    ]),
                })
                .collect(),
        ),
    }
}

fn complete_block_map_value(blocks: &[sley_vm::BytecodeBlock]) -> ConstValue {
    ConstValue {
        value_type: complete_block_map_type(),
        data: ConstData::Map(
            blocks
                .iter()
                .map(|block| sley_ssmc::MapEntryConst {
                    key: u32_value(u128::from(block.slot)),
                    value: complete_block_model_value(block),
                })
                .collect(),
        ),
    }
}

fn bytes_value(bytes: &[u8]) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(bytes.to_vec()),
    }
}

fn encoded_type(value: &TypeExpr) -> Vec<u8> {
    fn entity(output: &mut Vec<u8>, value: EntityId) {
        output.extend_from_slice(&1_u32.to_be_bytes());
        output.extend_from_slice(value.as_bytes());
    }
    fn walk(output: &mut Vec<u8>, value: &TypeExpr) {
        output.extend_from_slice(&value.tag().to_be_bytes());
        match value {
            TypeExpr::Unit
            | TypeExpr::Bool
            | TypeExpr::F32
            | TypeExpr::F64
            | TypeExpr::Bytes
            | TypeExpr::Text => {}
            TypeExpr::SInt(width) | TypeExpr::UInt(width) => {
                output.extend_from_slice(&width.bits().to_be_bytes());
            }
            TypeExpr::Tuple(values) => {
                output.extend_from_slice(
                    &u64::try_from(values.len())
                        .expect("type tuple length fits u64")
                        .to_be_bytes(),
                );
                for value in values {
                    walk(output, value);
                }
            }
            TypeExpr::Named(value) => {
                entity(output, value.definition);
                output.extend_from_slice(
                    &u64::try_from(value.arguments.len())
                        .expect("type argument length fits u64")
                        .to_be_bytes(),
                );
                for argument in &value.arguments {
                    walk(output, argument);
                }
            }
            TypeExpr::Vector(value) | TypeExpr::Option(value) | TypeExpr::LocalCell(value) => {
                walk(output, value);
            }
            TypeExpr::OrderedMap { key, value } => {
                walk(output, key);
                walk(output, value);
            }
            TypeExpr::Result { ok, error } => {
                walk(output, ok);
                walk(output, error);
            }
            TypeExpr::FunctionRef(value) => {
                output.extend_from_slice(
                    &u64::try_from(value.parameters.len())
                        .expect("function type parameter length fits u64")
                        .to_be_bytes(),
                );
                for parameter in &value.parameters {
                    walk(output, parameter);
                }
                walk(output, &value.result);
                output.extend_from_slice(
                    &u64::try_from(value.effects.len())
                        .expect("function type effect length fits u64")
                        .to_be_bytes(),
                );
                for effect in &value.effects {
                    entity(output, *effect);
                }
            }
            TypeExpr::AdapterHandle(value) | TypeExpr::CapabilityToken(value) => {
                entity(output, *value);
            }
            TypeExpr::TypeParameter(value) => output.extend_from_slice(&value.to_be_bytes()),
            TypeExpr::BuiltinFailure(value) => {
                output.extend_from_slice(&value.tag().to_be_bytes());
            }
        }
    }
    let mut output = Vec::new();
    walk(&mut output, value);
    output
}

fn encoded_immediate(value: &Immediate) -> Vec<u8> {
    fn entity(output: &mut Vec<u8>, value: EntityId) {
        output.extend_from_slice(&1_u32.to_be_bytes());
        output.extend_from_slice(value.as_bytes());
    }
    let mut output = value.tag().to_be_bytes().to_vec();
    match value {
        Immediate::None => {}
        Immediate::Entity(value) => entity(&mut output, *value),
        Immediate::Index(value) => output.extend_from_slice(&value.to_be_bytes()),
        Immediate::Field(value) => output.extend_from_slice(value.as_bytes()),
        Immediate::Variant(value) => {
            entity(&mut output, value.definition);
            output.extend_from_slice(value.member_id.as_bytes());
        }
        Immediate::Observation(value) => output.extend_from_slice(value),
        Immediate::Function(value) => {
            entity(&mut output, value.function);
            output.extend_from_slice(
                &u64::try_from(value.type_arguments.len())
                    .expect("immediate type-argument length fits u64")
                    .to_be_bytes(),
            );
            for argument in &value.type_arguments {
                output.extend_from_slice(&encoded_type(argument));
            }
        }
    }
    output
}

fn encoded_function_header(value: &sley_vm::BytecodeFunction) -> Vec<u8> {
    let mut output = value.function.as_bytes().to_vec();
    output.extend_from_slice(
        &u64::try_from(value.parameter_registers.len())
            .expect("parameter register length fits u64")
            .to_be_bytes(),
    );
    for register in &value.parameter_registers {
        output.extend_from_slice(&register.to_be_bytes());
    }
    output.extend_from_slice(
        &u64::try_from(value.register_types.len())
            .expect("register type length fits u64")
            .to_be_bytes(),
    );
    for value_type in &value.register_types {
        output.extend_from_slice(&encoded_type(value_type));
    }
    output.extend_from_slice(&encoded_type(&value.result_type));
    output.extend_from_slice(&value.entry_block.to_be_bytes());
    output.extend_from_slice(
        &u64::try_from(value.blocks.len())
            .expect("block length fits u64")
            .to_be_bytes(),
    );
    output
}

fn encoded_instruction(value: &sley_vm::Instruction) -> Vec<u8> {
    let mut output = value.opcode.to_be_bytes().to_vec();
    output.extend_from_slice(
        &u64::try_from(value.operands.len())
            .expect("operand length fits u64")
            .to_be_bytes(),
    );
    for register in &value.operands {
        output.extend_from_slice(&register.to_be_bytes());
    }
    output.extend_from_slice(
        &u64::try_from(value.results.len())
            .expect("result length fits u64")
            .to_be_bytes(),
    );
    for register in &value.results {
        output.extend_from_slice(&register.to_be_bytes());
    }
    output.extend_from_slice(&encoded_immediate(&value.immediate));
    output
}

fn append_encoded_registers(output: &mut Vec<u8>, values: &[u32]) {
    output.extend_from_slice(
        &u64::try_from(values.len())
            .expect("register length fits u64")
            .to_be_bytes(),
    );
    for value in values {
        output.extend_from_slice(&value.to_be_bytes());
    }
}

fn encoded_terminator(value: &sley_vm::BytecodeTerminator) -> Vec<u8> {
    let mut output = Vec::new();
    match value {
        sley_vm::BytecodeTerminator::Return(register) => {
            output.extend_from_slice(&1_u32.to_be_bytes());
            output.extend_from_slice(&register.to_be_bytes());
        }
        sley_vm::BytecodeTerminator::Branch(edge) => {
            output.extend_from_slice(&2_u32.to_be_bytes());
            output.extend_from_slice(&edge.target.to_be_bytes());
            append_encoded_registers(&mut output, &edge.arguments);
        }
        sley_vm::BytecodeTerminator::CondBranch {
            condition,
            if_true,
            if_false,
        } => {
            output.extend_from_slice(&3_u32.to_be_bytes());
            output.extend_from_slice(&condition.to_be_bytes());
            for edge in [if_true, if_false] {
                output.extend_from_slice(&edge.target.to_be_bytes());
                append_encoded_registers(&mut output, &edge.arguments);
            }
        }
        sley_vm::BytecodeTerminator::VariantSwitch { value, cases } => {
            output.extend_from_slice(&4_u32.to_be_bytes());
            output.extend_from_slice(&value.to_be_bytes());
            output.extend_from_slice(
                &u64::try_from(cases.len())
                    .expect("switch case length fits u64")
                    .to_be_bytes(),
            );
            for case in cases {
                match case.case_key {
                    CaseKey::Member(member) => {
                        output.extend_from_slice(&1_u32.to_be_bytes());
                        output.extend_from_slice(member.as_bytes());
                    }
                    CaseKey::Builtin(builtin) => {
                        output.extend_from_slice(&2_u32.to_be_bytes());
                        output.extend_from_slice(&builtin.tag().to_be_bytes());
                    }
                }
                output.extend_from_slice(&case.edge.target.to_be_bytes());
                output.extend_from_slice(
                    &u64::try_from(case.edge.arguments.len())
                        .expect("switch argument length fits u64")
                        .to_be_bytes(),
                );
                for argument in &case.edge.arguments {
                    match argument {
                        sley_vm::BytecodeSwitchArgument::Value(register) => {
                            output.extend_from_slice(&1_u32.to_be_bytes());
                            output.extend_from_slice(&register.to_be_bytes());
                        }
                        sley_vm::BytecodeSwitchArgument::CasePayload => {
                            output.extend_from_slice(&2_u32.to_be_bytes());
                        }
                    }
                }
            }
        }
        sley_vm::BytecodeTerminator::Trap { code, payload } => {
            output.extend_from_slice(&5_u32.to_be_bytes());
            output.extend_from_slice(&code.to_be_bytes());
            match payload {
                None => output.extend_from_slice(&1_u32.to_be_bytes()),
                Some(register) => {
                    output.extend_from_slice(&2_u32.to_be_bytes());
                    output.extend_from_slice(&register.to_be_bytes());
                }
            }
        }
    }
    output
}

fn encoded_block(value: &sley_vm::BytecodeBlock) -> Vec<u8> {
    let mut output = value.slot.to_be_bytes().to_vec();
    append_encoded_registers(&mut output, &value.parameter_registers);
    output.extend_from_slice(
        &u64::try_from(value.instructions.len())
            .expect("instruction length fits u64")
            .to_be_bytes(),
    );
    for instruction in &value.instructions {
        output.extend_from_slice(&encoded_instruction(instruction));
    }
    output.extend_from_slice(&encoded_terminator(&value.terminator));
    output.extend_from_slice(&value.reachability.to_be_bytes());
    output
}

fn encoded_function_body(value: &sley_vm::BytecodeFunction) -> Vec<u8> {
    let mut output = encoded_function_header(value);
    for block in &value.blocks {
        output.extend_from_slice(&encoded_block(block));
    }
    output
}

// Scaffold vocabulary (§1.3), documented in the manifest.
// LOWER_OK_EMPTY=0 is scaffold-local: the trivial closure is accepted
// with an empty lowered model. It is NOT a frozen code; the real
// LoweredModel arrives with the RW-110 corpus. PROFILE_UNSUPPORTED is
// the frozen `LowerErrorCode::ProfileUnsupported` numeric (26000,
// S20-260): it doubles as the single returnable error code, the same
// role VERSION=6 plays in the §1.1 codec scaffold and TYPE=1 in the
// §1.2 checker scaffold. Leg indexes double as trap payloads, so
// reaching a leg is observable: leg k traps carrying k, and leg k owns
// the k-th frozen lowering failure in `LowerErrorCode` order.
const LOWER_OK_EMPTY: u128 = 0;
const PROFILE_UNSUPPORTED: u128 = 26_000;

// Frozen `LowerErrorCode` numerics in leg order (S20-260; asserted
// against the native enum in `lower_scaffold_vocabulary_matches_frozen_native`).
const LEG_COUNT: u128 = 7;

// Fixture-namespace entity identities (recorded in the manifest).
// Within this image the function, parameter, constant, block, and
// operation identities are pairwise disjoint, so references resolve
// to the intended table entries. These bytes are fixture-local and
// freeze no production ABI; numeric reuse across independent
// scaffold images is not an execution collision.
const FUNCTION: u8 = 216;
const MARKER_PARAM: u8 = 214;
const WITNESS_PARAM: u8 = 215;
const ENTRY_BLOCK: u8 = 230;
const ACCEPT_BLOCK: u8 = 245;
const UNKNOWN_BLOCK: u8 = 246;

struct LowerScaffold {
    types: sley_check::TypeEnvironment,
    entry: FunctionGraph,
    functions: Vec<FunctionGraph>,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
    adapters: Vec<AdapterImport>,
}

fn rebase_value_ref(value: &mut ValueRef, ids: &BTreeMap<EntityId, EntityId>) {
    match value {
        ValueRef::Parameter(entity) => {
            *entity = ids.get(entity).copied().unwrap_or(*entity);
        }
        ValueRef::OperationResult(result) => {
            result.operation = ids
                .get(&result.operation)
                .copied()
                .unwrap_or(result.operation);
        }
    }
}

fn rebase_target_edge(edge: &mut TargetEdge, ids: &BTreeMap<EntityId, EntityId>) {
    edge.target = ids.get(&edge.target).copied().unwrap_or(edge.target);
    for argument in &mut edge.arguments {
        rebase_value_ref(argument, ids);
    }
}

fn rebase_terminator(terminator: &mut Terminator, ids: &BTreeMap<EntityId, EntityId>) {
    match terminator {
        Terminator::Return(value) => rebase_value_ref(&mut value.value, ids),
        Terminator::Branch(value) => rebase_target_edge(&mut value.edge, ids),
        Terminator::CondBranch(value) => {
            rebase_value_ref(&mut value.condition, ids);
            rebase_target_edge(&mut value.if_true, ids);
            rebase_target_edge(&mut value.if_false, ids);
        }
        Terminator::VariantSwitch(value) => {
            rebase_value_ref(&mut value.value, ids);
            for case in &mut value.cases {
                case.edge.target = ids
                    .get(&case.edge.target)
                    .copied()
                    .unwrap_or(case.edge.target);
                for argument in &mut case.edge.arguments {
                    if let SwitchArgument::Value(value) = argument {
                        rebase_value_ref(value, ids);
                    }
                }
            }
        }
        Terminator::Trap(value) => {
            if let Some(payload) = &mut value.payload {
                rebase_value_ref(payload, ids);
            }
        }
    }
}

/// Moves one independently assembled fixture's parameter/block/operation/
/// constant identities into a disjoint byte namespace while preserving its
/// stable function and external bridge identities.
fn rebase_scaffold_artifacts(mut scaffold: LowerScaffold, namespace_delta: u8) -> LowerScaffold {
    let mut ids = BTreeMap::new();
    for entity in scaffold
        .parameters
        .iter()
        .map(|value| value.entity_id)
        .chain(scaffold.blocks.iter().map(|value| value.entity_id))
        .chain(scaffold.operations.iter().map(|value| value.entity_id))
        .chain(scaffold.constants.iter().map(|value| value.entity_id))
    {
        let mut bytes = *entity.as_bytes();
        bytes[0] = bytes[0]
            .checked_add(namespace_delta)
            .expect("fixture namespace byte does not overflow");
        ids.insert(entity, EntityId::from_bytes(bytes));
    }
    let map_id = |entity: EntityId| ids.get(&entity).copied().unwrap_or(entity);
    let rebase_graph = |graph: &mut FunctionGraph| {
        graph.parameters.iter_mut().for_each(|id| *id = map_id(*id));
        graph.entry_block = map_id(graph.entry_block);
        graph.blocks.iter_mut().for_each(|id| *id = map_id(*id));
        graph.contracts.iter_mut().for_each(|id| *id = map_id(*id));
    };
    rebase_graph(&mut scaffold.entry);
    for graph in &mut scaffold.functions {
        rebase_graph(graph);
    }
    for parameter in &mut scaffold.parameters {
        parameter.entity_id = map_id(parameter.entity_id);
        parameter.owner = map_id(parameter.owner);
    }
    for block in &mut scaffold.blocks {
        block.entity_id = map_id(block.entity_id);
        block.function = map_id(block.function);
        block.parameters.iter_mut().for_each(|id| *id = map_id(*id));
        block.operations.iter_mut().for_each(|id| *id = map_id(*id));
        rebase_terminator(&mut block.terminator, &ids);
    }
    for operation in &mut scaffold.operations {
        operation.entity_id = map_id(operation.entity_id);
        operation.block = map_id(operation.block);
        for operand in &mut operation.operands {
            rebase_value_ref(operand, &ids);
        }
        match &mut operation.immediate {
            Immediate::Entity(entity) => *entity = map_id(*entity),
            Immediate::Variant(value) => value.definition = map_id(value.definition),
            Immediate::Function(value) => value.function = map_id(value.function),
            Immediate::None
            | Immediate::Index(_)
            | Immediate::Field(_)
            | Immediate::Observation(_) => {}
        }
    }
    for constant in &mut scaffold.constants {
        constant.entity_id = map_id(constant.entity_id);
    }
    scaffold
}

fn rebase_scaffold_functions(
    mut scaffold: LowerScaffold,
    functions: &[(EntityId, EntityId)],
) -> LowerScaffold {
    let ids = functions.iter().copied().collect::<BTreeMap<_, _>>();
    let map_id = |entity: EntityId| ids.get(&entity).copied().unwrap_or(entity);
    scaffold.entry.entity_id = map_id(scaffold.entry.entity_id);
    for graph in &mut scaffold.functions {
        graph.entity_id = map_id(graph.entity_id);
    }
    for parameter in &mut scaffold.parameters {
        parameter.owner = map_id(parameter.owner);
    }
    for block in &mut scaffold.blocks {
        block.function = map_id(block.function);
    }
    for operation in &mut scaffold.operations {
        if let Immediate::Function(reference) = &mut operation.immediate {
            reference.function = map_id(reference.function);
        }
    }
    scaffold
}

fn rebase_scaffold_function_namespace(scaffold: LowerScaffold, namespace: u8) -> LowerScaffold {
    let mappings = scaffold
        .functions
        .iter()
        .map(|graph| {
            let mut bytes = *graph.entity_id.as_bytes();
            bytes[0] = namespace;
            (graph.entity_id, EntityId::from_bytes(bytes))
        })
        .collect::<Vec<_>>();
    rebase_scaffold_functions(scaffold, &mappings)
}

fn remove_scaffold_function(mut scaffold: LowerScaffold, function: EntityId) -> LowerScaffold {
    let owned_blocks = scaffold
        .blocks
        .iter()
        .filter(|block| block.function == function)
        .map(|block| block.entity_id)
        .collect::<BTreeSet<_>>();
    scaffold
        .functions
        .retain(|graph| graph.entity_id != function);
    scaffold.parameters.retain(|parameter| {
        parameter.owner != function && !owned_blocks.contains(&parameter.owner)
    });
    scaffold
        .operations
        .retain(|operation| !owned_blocks.contains(&operation.block));
    scaffold.blocks.retain(|block| block.function != function);
    scaffold
}

struct ScaffoldBuilder {
    function: EntityId,
    marker_param: EntityId,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    next_op: u8,
}

impl ScaffoldBuilder {
    fn new(function: EntityId, marker_param: EntityId) -> Self {
        Self {
            function,
            marker_param,
            blocks: Vec::new(),
            operations: Vec::new(),
            next_op: 100,
        }
    }

    fn take_op(&mut self) -> EntityId {
        let op = id(self.next_op);
        self.next_op += 1;
        op
    }

    fn const_ref(&mut self, block: EntityId, ordinal: u32, target: EntityId) -> EntityId {
        let op = self.take_op();
        self.operations.push(Operation {
            entity_id: op,
            block,
            ordinal,
            opcode: Opcode::ConstantRef,
            operands: Vec::new(),
            result_types: vec![u32_type()],
            immediate: Immediate::Entity(target),
        });
        op
    }

    // Chain block: compare the marker against one constant, branch to
    // the target on equality else to the next chain block.
    fn chain_block(&mut self, block_id: u8, marker_const: EntityId, target: u8, next: u8) {
        let block = id(block_id);
        let const_op = self.const_ref(block, 0, marker_const);
        let eq_op = self.take_op();
        self.operations.push(Operation {
            entity_id: eq_op,
            block,
            ordinal: 1,
            opcode: Opcode::Equal,
            operands: vec![
                ValueRef::Parameter(self.marker_param),
                ValueRef::OperationResult(OperationResultRef {
                    operation: const_op,
                    result_index: 0,
                }),
            ],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        });
        self.blocks.push(Block {
            entity_id: block,
            function: self.function,
            parameters: Vec::new(),
            operations: vec![const_op, eq_op],
            terminator: Terminator::CondBranch(CondBranchTerminator {
                condition: ValueRef::OperationResult(OperationResultRef {
                    operation: eq_op,
                    result_index: 0,
                }),
                if_true: TargetEdge {
                    target: id(target),
                    arguments: Vec::new(),
                },
                if_false: TargetEdge {
                    target: id(next),
                    arguments: Vec::new(),
                },
            }),
            reachability: Reachability::Required,
        });
    }

    // Lowering leg: unimplemented trap carrying the leg index.
    fn leg_block(&mut self, leg: u8, payload_const: EntityId) {
        let block = id(238 + leg - 1);
        let payload_op = self.const_ref(block, 0, payload_const);
        self.blocks.push(Block {
            entity_id: block,
            function: self.function,
            parameters: Vec::new(),
            operations: vec![payload_op],
            terminator: Terminator::Trap(TrapTerminator {
                code: TrapCode::Unreachable,
                payload: Some(ValueRef::OperationResult(OperationResultRef {
                    operation: payload_op,
                    result_index: 0,
                })),
            }),
            reachability: Reachability::Required,
        });
    }

    // Value exit: return the cited constant (trivial accept or the
    // vocabulary representative).
    fn value_block(&mut self, block_id: u8, value_const: EntityId) {
        let block = id(block_id);
        let value_op = self.const_ref(block, 0, value_const);
        self.blocks.push(Block {
            entity_id: block,
            function: self.function,
            parameters: Vec::new(),
            operations: vec![value_op],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: value_op,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        });
    }

    fn finish(self) -> (Vec<Block>, Vec<Operation>) {
        (self.blocks, self.operations)
    }
}

fn lower_scaffold() -> LowerScaffold {
    let function = id(FUNCTION);
    let marker_param = id(MARKER_PARAM);
    let witness_param = id(WITNESS_PARAM);
    // Marker constants K0..K7; K1..K7 double as the per-leg trap
    // payloads, so reaching a leg is observable: leg k traps carrying k.
    let const_id = |k: u8| id(200 + k);
    let accept_const = id(208);
    let profile_const = id(209);
    let mut constants: Vec<ConstantDefinition> = (0..8)
        .map(|k| ConstantDefinition {
            entity_id: const_id(k),
            value: u32_value(u128::from(k)),
        })
        .collect();
    constants.push(ConstantDefinition {
        entity_id: accept_const,
        value: u32_value(LOWER_OK_EMPTY),
    });
    constants.push(ConstantDefinition {
        entity_id: profile_const,
        value: u32_value(PROFILE_UNSUPPORTED),
    });

    // Operation identities run on their own sequential namespace (100+);
    // block identities stay in the 230s/240s; constants in the 200s;
    // params at 214/215; the function at 216. Pairwise disjoint
    // within this image by construction (asserted by
    // `lower_scaffold_fixture_identities_are_within_image_disjoint`).
    let mut builder = ScaffoldBuilder::new(function, marker_param);
    // Entry tests marker 0 (trivial accept); the chain then tests 1..=7
    // in order; the final else covers everything unknown, so chain 7
    // both dispatches leg 7 and returns PROFILE_UNSUPPORTED for
    // anything else.
    builder.chain_block(ENTRY_BLOCK, const_id(0), ACCEPT_BLOCK, 231);
    for leg in 1..8_u8 {
        let next = if leg == 7 { UNKNOWN_BLOCK } else { 231 + leg };
        builder.chain_block(231 + leg - 1, const_id(leg), 238 + leg - 1, next);
    }
    // Lowering legs: unimplemented traps carrying the leg index.
    for leg in 1..8_u8 {
        builder.leg_block(leg, const_id(leg));
    }
    // Trivial accept: the admitted trivial closure with an empty model.
    builder.value_block(ACCEPT_BLOCK, accept_const);
    // Unknown marker: typed PROFILE_UNSUPPORTED error value (the
    // vocabulary return path; the only value-returning error exit).
    builder.value_block(UNKNOWN_BLOCK, profile_const);
    let (blocks, operations) = builder.finish();

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![marker_param, witness_param],
        result_type: u32_type(),
        effects: Vec::new(),
        entry_block: id(ENTRY_BLOCK),
        blocks: blocks.iter().map(|block| block.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: vec![
            Parameter {
                entity_id: marker_param,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: u32_type(),
            },
            Parameter {
                entity_id: witness_param,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 1,
                value_type: TypeExpr::Bytes,
            },
        ],
        blocks,
        operations,
        constants,
        adapters: Vec::new(),
    }
}

/// Real bounded §1.3 algorithm: lower one checked Boolean operation to
/// `(opcode, operand_registers, result_registers)`. Operands and the next
/// dense result register are runtime inputs and every operand must refer to a
/// prior register. Inputs arrive after admission, so the image cannot contain
/// the answer being compared with the native reference lowerer.
#[allow(clippy::too_many_lines)]
fn single_bool_lowerer() -> LowerScaffold {
    const FUNCTION_ID: u8 = 1;
    const OPCODE_PARAM: u8 = 10;
    const PARAM_COUNT_PARAM: u8 = 11;
    const OPERAND_ZERO_PARAM: u8 = 12;
    const OPERAND_ONE_PARAM: u8 = 13;
    const NEXT_REGISTER_PARAM: u8 = 14;
    const ENTRY: u8 = 20;
    const AND_CHECK: u8 = 21;
    const OR_CHECK: u8 = 22;
    const UNARY_COUNT: u8 = 23;
    const BINARY_COUNT: u8 = 24;
    const UNARY_EMIT: u8 = 25;
    const BINARY_EMIT: u8 = 26;
    const OPCODE_ERROR: u8 = 27;
    const SIGNATURE_ERROR: u8 = 28;
    const UNARY_REFERENCE: u8 = 29;
    const BINARY_REFERENCE_ZERO: u8 = 30;
    const BINARY_REFERENCE_ONE: u8 = 31;
    const LOCAL_REFERENCE_ERROR: u8 = 32;
    const K_NOT: u8 = 60;
    const K_AND: u8 = 61;
    const K_OR: u8 = 62;
    const K_ONE: u8 = 63;
    const K_TWO: u8 = 64;
    const K_OPCODE_ERROR: u8 = 66;
    const K_SIGNATURE_ERROR: u8 = 67;
    const K_LOCAL_REFERENCE_ERROR: u8 = 68;

    let function = id(FUNCTION_ID);
    let opcode_param = id(OPCODE_PARAM);
    let parameter_count_param = id(PARAM_COUNT_PARAM);
    let operand_zero_param = id(OPERAND_ZERO_PARAM);
    let operand_one_param = id(OPERAND_ONE_PARAM);
    let next_register_param = id(NEXT_REGISTER_PARAM);
    let constants = vec![
        ConstantDefinition {
            entity_id: id(K_NOT),
            value: u32_value(u128::from(Opcode::BoolNot.tag())),
        },
        ConstantDefinition {
            entity_id: id(K_AND),
            value: u32_value(u128::from(Opcode::BoolAnd.tag())),
        },
        ConstantDefinition {
            entity_id: id(K_OR),
            value: u32_value(u128::from(Opcode::BoolOr.tag())),
        },
        ConstantDefinition {
            entity_id: id(K_ONE),
            value: u32_value(1),
        },
        ConstantDefinition {
            entity_id: id(K_TWO),
            value: u32_value(2),
        },
        ConstantDefinition {
            entity_id: id(K_OPCODE_ERROR),
            value: u32_value(u128::from(
                sley_vm::LowerErrorCode::OpcodeUnsupported.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_SIGNATURE_ERROR),
            value: u32_value(u128::from(
                sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_LOCAL_REFERENCE_ERROR),
            value: u32_value(u128::from(
                sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
            )),
        },
    ];

    let mut builder = ScaffoldBuilder::new(function, opcode_param);
    builder.chain_block(ENTRY, id(K_NOT), UNARY_COUNT, AND_CHECK);
    builder.chain_block(AND_CHECK, id(K_AND), BINARY_COUNT, OR_CHECK);
    builder.chain_block(OR_CHECK, id(K_OR), BINARY_COUNT, OPCODE_ERROR);

    let mut count_block = |block_id: u8, expected: u8, success: u8| {
        let block = id(block_id);
        let expected_op = builder.const_ref(block, 0, id(expected));
        let equal_op = builder.take_op();
        builder.operations.push(Operation {
            entity_id: equal_op,
            block,
            ordinal: 1,
            opcode: Opcode::Equal,
            operands: vec![
                ValueRef::Parameter(parameter_count_param),
                ValueRef::OperationResult(OperationResultRef {
                    operation: expected_op,
                    result_index: 0,
                }),
            ],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        });
        builder.blocks.push(Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: vec![expected_op, equal_op],
            terminator: Terminator::CondBranch(CondBranchTerminator {
                condition: ValueRef::OperationResult(OperationResultRef {
                    operation: equal_op,
                    result_index: 0,
                }),
                if_true: TargetEdge {
                    target: id(success),
                    arguments: Vec::new(),
                },
                if_false: TargetEdge {
                    target: id(SIGNATURE_ERROR),
                    arguments: Vec::new(),
                },
            }),
            reachability: Reachability::Required,
        });
    };
    count_block(UNARY_COUNT, K_ONE, UNARY_REFERENCE);
    count_block(BINARY_COUNT, K_TWO, BINARY_REFERENCE_ZERO);

    let mut reference_block = |block_id: u8, operand: EntityId, success: u8| {
        let block = id(block_id);
        let valid = builder.take_op();
        builder.operations.push(Operation {
            entity_id: valid,
            block,
            ordinal: 0,
            opcode: Opcode::LessThan,
            operands: vec![
                ValueRef::Parameter(operand),
                ValueRef::Parameter(next_register_param),
            ],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        });
        builder.blocks.push(Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: vec![valid],
            terminator: Terminator::CondBranch(CondBranchTerminator {
                condition: ValueRef::OperationResult(OperationResultRef {
                    operation: valid,
                    result_index: 0,
                }),
                if_true: TargetEdge {
                    target: id(success),
                    arguments: Vec::new(),
                },
                if_false: TargetEdge {
                    target: id(LOCAL_REFERENCE_ERROR),
                    arguments: Vec::new(),
                },
            }),
            reachability: Reachability::Required,
        });
    };
    reference_block(UNARY_REFERENCE, operand_zero_param, UNARY_EMIT);
    reference_block(
        BINARY_REFERENCE_ZERO,
        operand_zero_param,
        BINARY_REFERENCE_ONE,
    );
    reference_block(BINARY_REFERENCE_ONE, operand_one_param, BINARY_EMIT);

    let mut emit_block = |block_id: u8, operand_parameters: &[EntityId]| {
        let block = id(block_id);
        let mut operation_ids = Vec::new();
        let operands = builder.take_op();
        builder.operations.push(Operation {
            entity_id: operands,
            block,
            ordinal: 0,
            opcode: Opcode::VectorNew,
            operands: operand_parameters
                .iter()
                .copied()
                .map(ValueRef::Parameter)
                .collect(),
            result_types: vec![u32vec_type()],
            immediate: Immediate::None,
        });
        operation_ids.push(operands);
        let results = builder.take_op();
        builder.operations.push(Operation {
            entity_id: results,
            block,
            ordinal: 1,
            opcode: Opcode::VectorNew,
            operands: vec![ValueRef::Parameter(next_register_param)],
            result_types: vec![u32vec_type()],
            immediate: Immediate::None,
        });
        operation_ids.push(results);
        let tuple = builder.take_op();
        builder.operations.push(Operation {
            entity_id: tuple,
            block,
            ordinal: 2,
            opcode: Opcode::TupleNew,
            operands: vec![
                ValueRef::Parameter(opcode_param),
                ValueRef::OperationResult(OperationResultRef {
                    operation: operands,
                    result_index: 0,
                }),
                ValueRef::OperationResult(OperationResultRef {
                    operation: results,
                    result_index: 0,
                }),
            ],
            result_types: vec![single_lowered_type()],
            immediate: Immediate::None,
        });
        operation_ids.push(tuple);
        let success = builder.take_op();
        builder.operations.push(Operation {
            entity_id: success,
            block,
            ordinal: 3,
            opcode: Opcode::ResultOk,
            operands: vec![ValueRef::OperationResult(OperationResultRef {
                operation: tuple,
                result_index: 0,
            })],
            result_types: vec![single_lower_result_type()],
            immediate: Immediate::None,
        });
        operation_ids.push(success);
        builder.blocks.push(Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: operation_ids,
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: success,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        });
    };
    emit_block(UNARY_EMIT, &[operand_zero_param]);
    emit_block(BINARY_EMIT, &[operand_zero_param, operand_one_param]);

    let mut error_block = |block_id: u8, error_constant: u8| {
        let block = id(block_id);
        let code = builder.const_ref(block, 0, id(error_constant));
        let failure = builder.take_op();
        builder.operations.push(Operation {
            entity_id: failure,
            block,
            ordinal: 1,
            opcode: Opcode::ResultErr,
            operands: vec![ValueRef::OperationResult(OperationResultRef {
                operation: code,
                result_index: 0,
            })],
            result_types: vec![single_lower_result_type()],
            immediate: Immediate::None,
        });
        builder.blocks.push(Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: vec![code, failure],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: failure,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        });
    };
    error_block(OPCODE_ERROR, K_OPCODE_ERROR);
    error_block(SIGNATURE_ERROR, K_SIGNATURE_ERROR);
    error_block(LOCAL_REFERENCE_ERROR, K_LOCAL_REFERENCE_ERROR);
    let (blocks, operations) = builder.finish();
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            opcode_param,
            parameter_count_param,
            operand_zero_param,
            operand_one_param,
            next_register_param,
        ],
        result_type: single_lower_result_type(),
        effects: Vec::new(),
        entry_block: id(ENTRY),
        blocks: blocks.iter().map(|block| block.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: vec![
            Parameter {
                entity_id: opcode_param,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: u32_type(),
            },
            Parameter {
                entity_id: operand_zero_param,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 2,
                value_type: u32_type(),
            },
            Parameter {
                entity_id: operand_one_param,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 3,
                value_type: u32_type(),
            },
            Parameter {
                entity_id: next_register_param,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 4,
                value_type: u32_type(),
            },
            Parameter {
                entity_id: parameter_count_param,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 1,
                value_type: u32_type(),
            },
        ],
        blocks,
        operations,
        constants,
        adapters: Vec::new(),
    }
}

fn inventory_id(namespace: u8, index: u16) -> EntityId {
    let mut bytes = [0_u8; 32];
    bytes[0] = namespace;
    bytes[1..3].copy_from_slice(&index.to_be_bytes());
    EntityId::from_bytes(bytes)
}

fn u64_value(value: u128) -> ConstValue {
    ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(value),
    }
}

struct InventoryAssembler {
    next_block: u16,
    next_parameter: u16,
    next_operation: u16,
    next_constant: u16,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
}

impl InventoryAssembler {
    fn new() -> Self {
        Self {
            next_block: 1,
            next_parameter: 1,
            next_operation: 1,
            next_constant: 1,
            parameters: Vec::new(),
            blocks: Vec::new(),
            operations: Vec::new(),
            constants: Vec::new(),
        }
    }

    fn block_id(&mut self) -> EntityId {
        let result = inventory_id(1, self.next_block);
        self.next_block += 1;
        result
    }

    fn parameter(
        &mut self,
        owner: EntityId,
        role: ParameterRole,
        ordinal: u32,
        value_type: TypeExpr,
    ) -> EntityId {
        let result = inventory_id(2, self.next_parameter);
        self.next_parameter += 1;
        self.parameters.push(Parameter {
            entity_id: result,
            owner,
            role,
            ordinal,
            value_type,
        });
        result
    }

    fn operation(
        &mut self,
        block: EntityId,
        opcode: Opcode,
        operands: Vec<ValueRef>,
        result_type: TypeExpr,
        immediate: Immediate,
    ) -> EntityId {
        let result = inventory_id(3, self.next_operation);
        self.next_operation += 1;
        let ordinal = u32::try_from(
            self.operations
                .iter()
                .filter(|operation| operation.block == block)
                .count(),
        )
        .expect("fixture operation count fits u32");
        self.operations.push(Operation {
            entity_id: result,
            block,
            ordinal,
            opcode,
            operands,
            result_types: vec![result_type],
            immediate,
        });
        result
    }

    fn constant(&mut self, value: ConstValue) -> EntityId {
        let result = inventory_id(4, self.next_constant);
        self.next_constant += 1;
        self.constants.push(ConstantDefinition {
            entity_id: result,
            value,
        });
        result
    }

    fn constant_ref(
        &mut self,
        block: EntityId,
        constant: EntityId,
        value_type: TypeExpr,
    ) -> EntityId {
        self.operation(
            block,
            Opcode::ConstantRef,
            Vec::new(),
            value_type,
            Immediate::Entity(constant),
        )
    }

    fn push_block(
        &mut self,
        entity_id: EntityId,
        function: EntityId,
        parameters: Vec<EntityId>,
        operations: Vec<EntityId>,
        terminator: Terminator,
    ) {
        self.blocks.push(Block {
            entity_id,
            function,
            parameters,
            operations,
            terminator,
            reachability: Reachability::Required,
        });
    }
}

#[derive(Clone, Copy)]
struct InventoryLoopParameters {
    index: EntityId,
    next_register: EntityId,
    inventory: EntityId,
    length: EntityId,
    model: EntityId,
}

#[derive(Clone, Copy)]
struct InventoryRowParameters {
    loop_parameters: InventoryLoopParameters,
    opcode: EntityId,
    arity: EntityId,
    operand_zero: EntityId,
    operand_one: EntityId,
}

#[derive(Clone, Copy)]
struct ImmediateInventoryLoopParameters {
    index: EntityId,
    next_register: EntityId,
    inventory: EntityId,
    length: EntityId,
    model: EntityId,
}

fn immediate_inventory_loop_parameters(
    assembler: &mut InventoryAssembler,
    block: EntityId,
) -> ImmediateInventoryLoopParameters {
    ImmediateInventoryLoopParameters {
        index: assembler.parameter(block, ParameterRole::Block, 0, u64_type()),
        next_register: assembler.parameter(block, ParameterRole::Block, 1, u32_type()),
        inventory: assembler.parameter(block, ParameterRole::Block, 2, immediate_inventory_type()),
        length: assembler.parameter(block, ParameterRole::Block, 3, u64_type()),
        model: assembler.parameter(
            block,
            ParameterRole::Block,
            4,
            immediate_inventory_model_type(),
        ),
    }
}

fn immediate_inventory_loop_ids(parameters: ImmediateInventoryLoopParameters) -> Vec<EntityId> {
    vec![
        parameters.index,
        parameters.next_register,
        parameters.inventory,
        parameters.length,
        parameters.model,
    ]
}

fn immediate_inventory_loop_values(parameters: ImmediateInventoryLoopParameters) -> Vec<ValueRef> {
    immediate_inventory_loop_ids(parameters)
        .into_iter()
        .map(ValueRef::Parameter)
        .collect()
}

fn inventory_loop_parameters(
    assembler: &mut InventoryAssembler,
    block: EntityId,
) -> InventoryLoopParameters {
    InventoryLoopParameters {
        index: assembler.parameter(block, ParameterRole::Block, 0, u64_type()),
        next_register: assembler.parameter(block, ParameterRole::Block, 1, u32_type()),
        inventory: assembler.parameter(block, ParameterRole::Block, 2, bool_inventory_type()),
        length: assembler.parameter(block, ParameterRole::Block, 3, u64_type()),
        model: assembler.parameter(block, ParameterRole::Block, 4, inventory_model_type()),
    }
}

fn inventory_row_parameters(
    assembler: &mut InventoryAssembler,
    block: EntityId,
) -> InventoryRowParameters {
    let loop_parameters = inventory_loop_parameters(assembler, block);
    InventoryRowParameters {
        loop_parameters,
        opcode: assembler.parameter(block, ParameterRole::Block, 5, u32_type()),
        arity: assembler.parameter(block, ParameterRole::Block, 6, u32_type()),
        operand_zero: assembler.parameter(block, ParameterRole::Block, 7, u32_type()),
        operand_one: assembler.parameter(block, ParameterRole::Block, 8, u32_type()),
    }
}

fn inventory_loop_ids(parameters: InventoryLoopParameters) -> Vec<EntityId> {
    vec![
        parameters.index,
        parameters.next_register,
        parameters.inventory,
        parameters.length,
        parameters.model,
    ]
}

fn inventory_loop_values(parameters: InventoryLoopParameters) -> Vec<ValueRef> {
    inventory_loop_ids(parameters)
        .into_iter()
        .map(ValueRef::Parameter)
        .collect()
}

fn inventory_row_ids(parameters: &InventoryRowParameters) -> Vec<EntityId> {
    let mut result = inventory_loop_ids(parameters.loop_parameters);
    result.extend([
        parameters.opcode,
        parameters.arity,
        parameters.operand_zero,
        parameters.operand_one,
    ]);
    result
}

fn inventory_row_values(parameters: &InventoryRowParameters) -> Vec<ValueRef> {
    inventory_row_ids(parameters)
        .into_iter()
        .map(ValueRef::Parameter)
        .collect()
}

fn operation_value(operation: EntityId) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation,
        result_index: 0,
    })
}

fn inventory_branch(target: EntityId, arguments: Vec<ValueRef>) -> Terminator {
    Terminator::Branch(BranchTerminator {
        edge: TargetEdge { target, arguments },
    })
}

fn inventory_cond(
    condition: ValueRef,
    if_true: EntityId,
    true_arguments: Vec<ValueRef>,
    if_false: EntityId,
    false_arguments: Vec<ValueRef>,
) -> Terminator {
    Terminator::CondBranch(CondBranchTerminator {
        condition,
        if_true: TargetEdge {
            target: if_true,
            arguments: true_arguments,
        },
        if_false: TargetEdge {
            target: if_false,
            arguments: false_arguments,
        },
    })
}

fn inventory_switch(
    value: ValueRef,
    cases: Vec<(BuiltinCase, EntityId, Vec<SwitchArgument>)>,
) -> Terminator {
    Terminator::VariantSwitch(VariantSwitchTerminator {
        value,
        cases: cases
            .into_iter()
            .map(|(case, target, arguments)| SwitchCase {
                case_key: CaseKey::Builtin(case),
                edge: SwitchEdge { target, arguments },
            })
            .collect(),
    })
}

fn inventory_switch_values(values: Vec<ValueRef>) -> Vec<SwitchArgument> {
    values.into_iter().map(SwitchArgument::Value).collect()
}

/// Real bounded §1.3 scalar-operation inventory algorithm. The inventory is
/// supplied at execution time, walked by a CFG backedge, and validated row by
/// row before the dense register frontier advances.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn ordered_scalar_inventory_lowerer() -> LowerScaffold {
    let function = inventory_id(5, 1);
    let mut assembler = InventoryAssembler::new();
    let inventory_parameter =
        assembler.parameter(function, ParameterRole::Function, 0, bool_inventory_type());
    let first_register_parameter =
        assembler.parameter(function, ParameterRole::Function, 1, u32_type());

    let entry = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let unpack = assembler.block_id();
    let opcode_not = assembler.block_id();
    let opcode_and = assembler.block_id();
    let opcode_or = assembler.block_id();
    let opcode_equal = assembler.block_id();
    let opcode_not_equal = assembler.block_id();
    let opcode_less = assembler.block_id();
    let opcode_less_equal = assembler.block_id();
    let opcode_greater = assembler.block_id();
    let opcode_greater_equal = assembler.block_id();
    let opcode_int_add = assembler.block_id();
    let opcode_int_sub = assembler.block_id();
    let opcode_int_mul = assembler.block_id();
    let opcode_int_div = assembler.block_id();
    let opcode_int_rem = assembler.block_id();
    let opcode_int_neg = assembler.block_id();
    let opcode_int_shl = assembler.block_id();
    let opcode_int_shr = assembler.block_id();
    let opcode_float_add = assembler.block_id();
    let opcode_float_sub = assembler.block_id();
    let opcode_float_mul = assembler.block_id();
    let opcode_float_div = assembler.block_id();
    let opcode_float_neg = assembler.block_id();
    let opcode_option_some = assembler.block_id();
    let opcode_option_none = assembler.block_id();
    let opcode_result_ok = assembler.block_id();
    let opcode_result_err = assembler.block_id();
    let opcode_cell_new = assembler.block_id();
    let opcode_cell_get = assembler.block_id();
    let opcode_cell_set = assembler.block_id();
    let opcode_value_hash = assembler.block_id();
    let zero_count = assembler.block_id();
    let unary_count = assembler.block_id();
    let binary_count = assembler.block_id();
    let emit_zero = assembler.block_id();
    let unary_reference = assembler.block_id();
    let binary_reference_zero = assembler.block_id();
    let binary_reference_one = assembler.block_id();
    let emit_unary = assembler.block_id();
    let emit_binary = assembler.block_id();
    let advance_index = assembler.block_id();
    let advance_register = assembler.block_id();
    let done = assembler.block_id();
    let opcode_error = assembler.block_id();
    let signature_error = assembler.block_id();
    let local_reference_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();

    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let zero_u32 = assembler.constant(u32_value(0));
    let one_u32 = assembler.constant(u32_value(1));
    let two_u32 = assembler.constant(u32_value(2));
    let not_tag = assembler.constant(u32_value(u128::from(Opcode::BoolNot.tag())));
    let and_tag = assembler.constant(u32_value(u128::from(Opcode::BoolAnd.tag())));
    let or_tag = assembler.constant(u32_value(u128::from(Opcode::BoolOr.tag())));
    let equal_tag = assembler.constant(u32_value(u128::from(Opcode::Equal.tag())));
    let not_equal_tag = assembler.constant(u32_value(u128::from(Opcode::NotEqual.tag())));
    let less_tag = assembler.constant(u32_value(u128::from(Opcode::LessThan.tag())));
    let less_equal_tag = assembler.constant(u32_value(u128::from(Opcode::LessEqual.tag())));
    let greater_tag = assembler.constant(u32_value(u128::from(Opcode::GreaterThan.tag())));
    let greater_equal_tag = assembler.constant(u32_value(u128::from(Opcode::GreaterEqual.tag())));
    let int_add_tag = assembler.constant(u32_value(u128::from(Opcode::IntAddChecked.tag())));
    let int_sub_tag = assembler.constant(u32_value(u128::from(Opcode::IntSubChecked.tag())));
    let int_mul_tag = assembler.constant(u32_value(u128::from(Opcode::IntMulChecked.tag())));
    let int_div_tag = assembler.constant(u32_value(u128::from(Opcode::IntDivChecked.tag())));
    let int_rem_tag = assembler.constant(u32_value(u128::from(Opcode::IntRemChecked.tag())));
    let int_neg_tag = assembler.constant(u32_value(u128::from(Opcode::IntNegChecked.tag())));
    let int_shl_tag = assembler.constant(u32_value(u128::from(Opcode::IntShlChecked.tag())));
    let int_shr_tag = assembler.constant(u32_value(u128::from(Opcode::IntShrChecked.tag())));
    let float_add_tag = assembler.constant(u32_value(u128::from(Opcode::FloatAdd.tag())));
    let float_sub_tag = assembler.constant(u32_value(u128::from(Opcode::FloatSub.tag())));
    let float_mul_tag = assembler.constant(u32_value(u128::from(Opcode::FloatMul.tag())));
    let float_div_tag = assembler.constant(u32_value(u128::from(Opcode::FloatDiv.tag())));
    let float_neg_tag = assembler.constant(u32_value(u128::from(Opcode::FloatNeg.tag())));
    let option_some_tag = assembler.constant(u32_value(u128::from(Opcode::OptionSome.tag())));
    let option_none_tag = assembler.constant(u32_value(u128::from(Opcode::OptionNone.tag())));
    let result_ok_tag = assembler.constant(u32_value(u128::from(Opcode::ResultOk.tag())));
    let result_err_tag = assembler.constant(u32_value(u128::from(Opcode::ResultErr.tag())));
    let cell_new_tag = assembler.constant(u32_value(u128::from(Opcode::CellNew.tag())));
    let cell_get_tag = assembler.constant(u32_value(u128::from(Opcode::CellGet.tag())));
    let cell_set_tag = assembler.constant(u32_value(u128::from(Opcode::CellSet.tag())));
    let value_hash_tag = assembler.constant(u32_value(u128::from(Opcode::ValueHash.tag())));
    let opcode_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::OpcodeUnsupported.numeric(),
    )));
    let signature_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
    )));
    let local_reference_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    )));
    let resource_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let entry_zero = assembler.constant_ref(entry, zero_u64, u64_type());
    let inventory_length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(inventory_parameter)],
        u64_type(),
        Immediate::None,
    );
    let empty_model = assembler.operation(
        entry,
        Opcode::VectorNew,
        Vec::new(),
        inventory_model_type(),
        Immediate::None,
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![entry_zero, inventory_length, empty_model],
        inventory_branch(
            check,
            vec![
                operation_value(entry_zero),
                ValueRef::Parameter(first_register_parameter),
                ValueRef::Parameter(inventory_parameter),
                operation_value(inventory_length),
                operation_value(empty_model),
            ],
        ),
    );

    let check_parameters = inventory_loop_parameters(&mut assembler, check);
    let has_row = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_parameters.index),
            ValueRef::Parameter(check_parameters.length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        inventory_loop_ids(check_parameters),
        vec![has_row],
        inventory_cond(
            operation_value(has_row),
            get,
            inventory_loop_values(check_parameters),
            done,
            vec![
                ValueRef::Parameter(check_parameters.model),
                ValueRef::Parameter(check_parameters.index),
                ValueRef::Parameter(check_parameters.next_register),
            ],
        ),
    );

    let get_parameters = inventory_loop_parameters(&mut assembler, get);
    let row = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(get_parameters.inventory),
            ValueRef::Parameter(get_parameters.index),
        ],
        TypeExpr::Option(Box::new(bool_inventory_row_type())),
        Immediate::None,
    );
    let mut unpack_arguments = vec![SwitchArgument::CasePayload];
    unpack_arguments.extend(inventory_switch_values(inventory_loop_values(
        get_parameters,
    )));
    assembler.push_block(
        get,
        function,
        inventory_loop_ids(get_parameters),
        vec![row],
        inventory_switch(
            operation_value(row),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (BuiltinCase::Some, unpack, unpack_arguments),
            ],
        ),
    );

    let unpack_row =
        assembler.parameter(unpack, ParameterRole::Block, 0, bool_inventory_row_type());
    let unpack_loop = InventoryLoopParameters {
        index: assembler.parameter(unpack, ParameterRole::Block, 1, u64_type()),
        next_register: assembler.parameter(unpack, ParameterRole::Block, 2, u32_type()),
        inventory: assembler.parameter(unpack, ParameterRole::Block, 3, bool_inventory_type()),
        length: assembler.parameter(unpack, ParameterRole::Block, 4, u64_type()),
        model: assembler.parameter(unpack, ParameterRole::Block, 5, inventory_model_type()),
    };
    let mut fields = Vec::new();
    for index in 0..4 {
        fields.push(assembler.operation(
            unpack,
            Opcode::TupleGet,
            vec![ValueRef::Parameter(unpack_row)],
            u32_type(),
            Immediate::Index(index),
        ));
    }
    assembler.push_block(
        unpack,
        function,
        {
            let mut ids = vec![unpack_row];
            ids.extend(inventory_loop_ids(unpack_loop));
            ids
        },
        fields.clone(),
        inventory_branch(
            opcode_not,
            vec![
                ValueRef::Parameter(unpack_loop.index),
                ValueRef::Parameter(unpack_loop.next_register),
                ValueRef::Parameter(unpack_loop.inventory),
                ValueRef::Parameter(unpack_loop.length),
                ValueRef::Parameter(unpack_loop.model),
                operation_value(fields[0]),
                operation_value(fields[1]),
                operation_value(fields[2]),
                operation_value(fields[3]),
            ],
        ),
    );

    let opcode_block = |assembler: &mut InventoryAssembler,
                        block: EntityId,
                        tag: EntityId,
                        matched: EntityId,
                        unmatched: EntityId| {
        let parameters = inventory_row_parameters(assembler, block);
        let tag_value = assembler.constant_ref(block, tag, u32_type());
        let equal = assembler.operation(
            block,
            Opcode::Equal,
            vec![
                ValueRef::Parameter(parameters.opcode),
                operation_value(tag_value),
            ],
            TypeExpr::Bool,
            Immediate::None,
        );
        let unmatched_arguments = if unmatched == opcode_error {
            Vec::new()
        } else {
            inventory_row_values(&parameters)
        };
        assembler.push_block(
            block,
            function,
            inventory_row_ids(&parameters),
            vec![tag_value, equal],
            inventory_cond(
                operation_value(equal),
                matched,
                inventory_row_values(&parameters),
                unmatched,
                unmatched_arguments,
            ),
        );
    };
    opcode_block(&mut assembler, opcode_not, not_tag, unary_count, opcode_and);
    opcode_block(&mut assembler, opcode_and, and_tag, binary_count, opcode_or);
    opcode_block(
        &mut assembler,
        opcode_or,
        or_tag,
        binary_count,
        opcode_equal,
    );
    opcode_block(
        &mut assembler,
        opcode_equal,
        equal_tag,
        binary_count,
        opcode_not_equal,
    );
    opcode_block(
        &mut assembler,
        opcode_not_equal,
        not_equal_tag,
        binary_count,
        opcode_less,
    );
    opcode_block(
        &mut assembler,
        opcode_less,
        less_tag,
        binary_count,
        opcode_less_equal,
    );
    opcode_block(
        &mut assembler,
        opcode_less_equal,
        less_equal_tag,
        binary_count,
        opcode_greater,
    );
    opcode_block(
        &mut assembler,
        opcode_greater,
        greater_tag,
        binary_count,
        opcode_greater_equal,
    );
    opcode_block(
        &mut assembler,
        opcode_greater_equal,
        greater_equal_tag,
        binary_count,
        opcode_int_add,
    );
    opcode_block(
        &mut assembler,
        opcode_int_add,
        int_add_tag,
        binary_count,
        opcode_int_sub,
    );
    opcode_block(
        &mut assembler,
        opcode_int_sub,
        int_sub_tag,
        binary_count,
        opcode_int_mul,
    );
    opcode_block(
        &mut assembler,
        opcode_int_mul,
        int_mul_tag,
        binary_count,
        opcode_int_div,
    );
    opcode_block(
        &mut assembler,
        opcode_int_div,
        int_div_tag,
        binary_count,
        opcode_int_rem,
    );
    opcode_block(
        &mut assembler,
        opcode_int_rem,
        int_rem_tag,
        binary_count,
        opcode_int_neg,
    );
    opcode_block(
        &mut assembler,
        opcode_int_neg,
        int_neg_tag,
        unary_count,
        opcode_int_shl,
    );
    opcode_block(
        &mut assembler,
        opcode_int_shl,
        int_shl_tag,
        binary_count,
        opcode_int_shr,
    );
    opcode_block(
        &mut assembler,
        opcode_int_shr,
        int_shr_tag,
        binary_count,
        opcode_float_add,
    );
    opcode_block(
        &mut assembler,
        opcode_float_add,
        float_add_tag,
        binary_count,
        opcode_float_sub,
    );
    opcode_block(
        &mut assembler,
        opcode_float_sub,
        float_sub_tag,
        binary_count,
        opcode_float_mul,
    );
    opcode_block(
        &mut assembler,
        opcode_float_mul,
        float_mul_tag,
        binary_count,
        opcode_float_div,
    );
    opcode_block(
        &mut assembler,
        opcode_float_div,
        float_div_tag,
        binary_count,
        opcode_float_neg,
    );
    opcode_block(
        &mut assembler,
        opcode_float_neg,
        float_neg_tag,
        unary_count,
        opcode_option_some,
    );
    opcode_block(
        &mut assembler,
        opcode_option_some,
        option_some_tag,
        unary_count,
        opcode_option_none,
    );
    opcode_block(
        &mut assembler,
        opcode_option_none,
        option_none_tag,
        zero_count,
        opcode_result_ok,
    );
    opcode_block(
        &mut assembler,
        opcode_result_ok,
        result_ok_tag,
        unary_count,
        opcode_result_err,
    );
    opcode_block(
        &mut assembler,
        opcode_result_err,
        result_err_tag,
        unary_count,
        opcode_cell_new,
    );
    opcode_block(
        &mut assembler,
        opcode_cell_new,
        cell_new_tag,
        unary_count,
        opcode_cell_get,
    );
    opcode_block(
        &mut assembler,
        opcode_cell_get,
        cell_get_tag,
        unary_count,
        opcode_cell_set,
    );
    opcode_block(
        &mut assembler,
        opcode_cell_set,
        cell_set_tag,
        binary_count,
        opcode_value_hash,
    );
    opcode_block(
        &mut assembler,
        opcode_value_hash,
        value_hash_tag,
        unary_count,
        opcode_error,
    );

    let count_block = |assembler: &mut InventoryAssembler,
                       block: EntityId,
                       expected: EntityId,
                       success: EntityId| {
        let parameters = inventory_row_parameters(assembler, block);
        let expected_value = assembler.constant_ref(block, expected, u32_type());
        let equal = assembler.operation(
            block,
            Opcode::Equal,
            vec![
                ValueRef::Parameter(parameters.arity),
                operation_value(expected_value),
            ],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            inventory_row_ids(&parameters),
            vec![expected_value, equal],
            inventory_cond(
                operation_value(equal),
                success,
                inventory_row_values(&parameters),
                signature_error,
                Vec::new(),
            ),
        );
    };
    count_block(&mut assembler, zero_count, zero_u32, emit_zero);
    count_block(&mut assembler, unary_count, one_u32, unary_reference);
    count_block(&mut assembler, binary_count, two_u32, binary_reference_zero);

    let reference_block = |assembler: &mut InventoryAssembler,
                           block: EntityId,
                           operand_one: bool,
                           success: EntityId| {
        let parameters = inventory_row_parameters(assembler, block);
        let operand = if operand_one {
            parameters.operand_one
        } else {
            parameters.operand_zero
        };
        let valid = assembler.operation(
            block,
            Opcode::LessThan,
            vec![
                ValueRef::Parameter(operand),
                ValueRef::Parameter(parameters.loop_parameters.next_register),
            ],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            inventory_row_ids(&parameters),
            vec![valid],
            inventory_cond(
                operation_value(valid),
                success,
                inventory_row_values(&parameters),
                local_reference_error,
                Vec::new(),
            ),
        );
    };
    reference_block(&mut assembler, unary_reference, false, emit_unary);
    reference_block(
        &mut assembler,
        binary_reference_zero,
        false,
        binary_reference_one,
    );
    reference_block(&mut assembler, binary_reference_one, true, emit_binary);

    let emit_block = |assembler: &mut InventoryAssembler, block: EntityId, arity: u32| {
        let parameters = inventory_row_parameters(assembler, block);
        let mut operand_values = Vec::new();
        if arity >= 1 {
            operand_values.push(ValueRef::Parameter(parameters.operand_zero));
        }
        if arity >= 2 {
            operand_values.push(ValueRef::Parameter(parameters.operand_one));
        }
        let operands = assembler.operation(
            block,
            Opcode::VectorNew,
            operand_values,
            u32vec_type(),
            Immediate::None,
        );
        let results = assembler.operation(
            block,
            Opcode::VectorNew,
            vec![ValueRef::Parameter(
                parameters.loop_parameters.next_register,
            )],
            u32vec_type(),
            Immediate::None,
        );
        let instruction = assembler.operation(
            block,
            Opcode::TupleNew,
            vec![
                ValueRef::Parameter(parameters.opcode),
                operation_value(operands),
                operation_value(results),
            ],
            single_lowered_type(),
            Immediate::None,
        );
        let push = assembler.operation(
            block,
            Opcode::AdapterInvoke,
            vec![
                ValueRef::Parameter(parameters.loop_parameters.model),
                operation_value(instruction),
            ],
            index_result_type(inventory_model_type()),
            Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
                sley_vm::host_abi::BRIDGE_CODE_PSH1,
            ))),
        );
        assembler.push_block(
            block,
            function,
            inventory_row_ids(&parameters),
            vec![operands, results, instruction, push],
            inventory_switch(
                operation_value(push),
                vec![
                    (
                        BuiltinCase::Ok,
                        advance_index,
                        vec![
                            SwitchArgument::Value(ValueRef::Parameter(
                                parameters.loop_parameters.index,
                            )),
                            SwitchArgument::Value(ValueRef::Parameter(
                                parameters.loop_parameters.next_register,
                            )),
                            SwitchArgument::Value(ValueRef::Parameter(
                                parameters.loop_parameters.inventory,
                            )),
                            SwitchArgument::Value(ValueRef::Parameter(
                                parameters.loop_parameters.length,
                            )),
                            SwitchArgument::CasePayload,
                        ],
                    ),
                    (BuiltinCase::Err, resource_error, Vec::new()),
                ],
            ),
        );
    };
    emit_block(&mut assembler, emit_zero, 0);
    emit_block(&mut assembler, emit_unary, 1);
    emit_block(&mut assembler, emit_binary, 2);

    let advance_index_parameters = inventory_loop_parameters(&mut assembler, advance_index);
    let index_one = assembler.constant_ref(advance_index, one_u64, u64_type());
    let next_index = assembler.operation(
        advance_index,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_index_parameters.index),
            operation_value(index_one),
        ],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance_index,
        function,
        inventory_loop_ids(advance_index_parameters),
        vec![index_one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    advance_register,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(
                            advance_index_parameters.next_register,
                        )),
                        SwitchArgument::Value(ValueRef::Parameter(
                            advance_index_parameters.inventory,
                        )),
                        SwitchArgument::Value(ValueRef::Parameter(advance_index_parameters.length)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_index_parameters.model)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let advance_register_parameters = inventory_loop_parameters(&mut assembler, advance_register);
    let register_one = assembler.constant_ref(advance_register, one_u32, u32_type());
    let next_register = assembler.operation(
        advance_register,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_register_parameters.next_register),
            operation_value(register_one),
        ],
        arithmetic_result_type(u32_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance_register,
        function,
        inventory_loop_ids(advance_register_parameters),
        vec![register_one, next_register],
        inventory_switch(
            operation_value(next_register),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(
                            advance_register_parameters.index,
                        )),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(
                            advance_register_parameters.inventory,
                        )),
                        SwitchArgument::Value(ValueRef::Parameter(
                            advance_register_parameters.length,
                        )),
                        SwitchArgument::Value(ValueRef::Parameter(
                            advance_register_parameters.model,
                        )),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let done_model = assembler.parameter(done, ParameterRole::Block, 0, inventory_model_type());
    let done_index = assembler.parameter(done, ParameterRole::Block, 1, u64_type());
    let done_register = assembler.parameter(done, ParameterRole::Block, 2, u32_type());
    let summary = assembler.operation(
        done,
        Opcode::TupleNew,
        vec![
            ValueRef::Parameter(done_model),
            ValueRef::Parameter(done_index),
            ValueRef::Parameter(done_register),
        ],
        inventory_summary_type(),
        Immediate::None,
    );
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![operation_value(summary)],
        inventory_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![done_model, done_index, done_register],
        vec![summary, success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    for (block, code) in [
        (opcode_error, opcode_error_code),
        (signature_error, signature_error_code),
        (local_reference_error, local_reference_error_code),
        (resource_error, resource_error_code),
    ] {
        let code_value = assembler.constant_ref(block, code, u32_type());
        let failure = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![operation_value(code_value)],
            inventory_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![code_value, failure],
            Terminator::Return(ReturnTerminator {
                value: operation_value(failure),
            }),
        );
    }
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![inventory_parameter, first_register_parameter],
        result_type: inventory_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![AdapterImport {
            entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
                sley_vm::host_abi::BRIDGE_CODE_PSH1,
            )),
            adapter_id: sley_vm::host_abi::bridge_identity(sley_vm::host_abi::BRIDGE_CODE_PSH1),
            abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
            request_type: single_lowered_type(),
            response_type: inventory_model_type(),
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
            effects: Vec::new(),
        }],
    }
}

#[allow(clippy::too_many_lines)]
fn build_register_vector_validator(
    assembler: &mut InventoryAssembler,
    function: EntityId,
) -> FunctionGraph {
    let entry = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let compare = assembler.block_id();
    let advance = assembler.block_id();
    let done = assembler.block_id();
    let local_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();

    let values = assembler.parameter(function, ParameterRole::Function, 0, u32vec_type());
    let register_count = assembler.parameter(function, ParameterRole::Function, 1, u32_type());
    let zero = assembler.constant(u64_value(0));
    let one = assembler.constant(u64_value(1));
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let local_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    )));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let entry_zero = assembler.constant_ref(entry, zero, u64_type());
    let length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(values)],
        u64_type(),
        Immediate::None,
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![entry_zero, length],
        inventory_branch(
            check,
            vec![
                operation_value(entry_zero),
                ValueRef::Parameter(values),
                operation_value(length),
                ValueRef::Parameter(register_count),
            ],
        ),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let check_values = assembler.parameter(check, ParameterRole::Block, 1, u32vec_type());
    let check_length = assembler.parameter(check, ParameterRole::Block, 2, u64_type());
    let check_count = assembler.parameter(check, ParameterRole::Block, 3, u32_type());
    let has_value = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_index),
            ValueRef::Parameter(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    let loop_values = vec![
        ValueRef::Parameter(check_index),
        ValueRef::Parameter(check_values),
        ValueRef::Parameter(check_length),
        ValueRef::Parameter(check_count),
    ];
    assembler.push_block(
        check,
        function,
        vec![check_index, check_values, check_length, check_count],
        vec![has_value],
        inventory_cond(
            operation_value(has_value),
            get,
            loop_values,
            done,
            Vec::new(),
        ),
    );

    let get_index = assembler.parameter(get, ParameterRole::Block, 0, u64_type());
    let get_values = assembler.parameter(get, ParameterRole::Block, 1, u32vec_type());
    let get_length = assembler.parameter(get, ParameterRole::Block, 2, u64_type());
    let get_count = assembler.parameter(get, ParameterRole::Block, 3, u32_type());
    let found = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(get_values),
            ValueRef::Parameter(get_index),
        ],
        optional_u32_type(),
        Immediate::None,
    );
    assembler.push_block(
        get,
        function,
        vec![get_index, get_values, get_length, get_count],
        vec![found],
        inventory_switch(
            operation_value(found),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    compare,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                        SwitchArgument::Value(ValueRef::Parameter(get_values)),
                        SwitchArgument::Value(ValueRef::Parameter(get_length)),
                        SwitchArgument::Value(ValueRef::Parameter(get_count)),
                    ],
                ),
            ],
        ),
    );

    let compare_value = assembler.parameter(compare, ParameterRole::Block, 0, u32_type());
    let compare_index = assembler.parameter(compare, ParameterRole::Block, 1, u64_type());
    let compare_values = assembler.parameter(compare, ParameterRole::Block, 2, u32vec_type());
    let compare_length = assembler.parameter(compare, ParameterRole::Block, 3, u64_type());
    let compare_count = assembler.parameter(compare, ParameterRole::Block, 4, u32_type());
    let valid = assembler.operation(
        compare,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(compare_value),
            ValueRef::Parameter(compare_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        compare,
        function,
        vec![
            compare_value,
            compare_index,
            compare_values,
            compare_length,
            compare_count,
        ],
        vec![valid],
        inventory_cond(
            operation_value(valid),
            advance,
            vec![
                ValueRef::Parameter(compare_index),
                ValueRef::Parameter(compare_values),
                ValueRef::Parameter(compare_length),
                ValueRef::Parameter(compare_count),
            ],
            local_error,
            Vec::new(),
        ),
    );

    let advance_index = assembler.parameter(advance, ParameterRole::Block, 0, u64_type());
    let advance_values = assembler.parameter(advance, ParameterRole::Block, 1, u32vec_type());
    let advance_length = assembler.parameter(advance, ParameterRole::Block, 2, u64_type());
    let advance_count = assembler.parameter(advance, ParameterRole::Block, 3, u32_type());
    let advance_one = assembler.constant_ref(advance, one, u64_type());
    let next_index = assembler.operation(
        advance,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_index),
            operation_value(advance_one),
        ],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance,
        function,
        vec![advance_index, advance_values, advance_length, advance_count],
        vec![advance_one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_values)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_length)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_count)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let unit_value = assembler.constant_ref(done, unit, TypeExpr::Unit);
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![operation_value(unit_value)],
        unit_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        Vec::new(),
        vec![unit_value, success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    for (block, code) in [(local_error, local_code), (resource_error, resource_code)] {
        let value = assembler.constant_ref(block, code, u32_type());
        let failure = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![operation_value(value)],
            unit_lower_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![value, failure],
            Terminator::Return(ReturnTerminator {
                value: operation_value(failure),
            }),
        );
    }
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![values, register_count],
        result_type: unit_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

/// Lowers one runtime immediate-free bootstrap operation whose complete
/// operand vector is supplied after checking. A Sley helper walks every
/// register before the main function derives the dense result frontier.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn immediate_free_operation_lowerer() -> LowerScaffold {
    let function = inventory_id(5, 6);
    let validator = inventory_id(5, 7);
    let mut assembler = InventoryAssembler::new();
    let validator_graph = build_register_vector_validator(&mut assembler, validator);

    let opcode = assembler.parameter(function, ParameterRole::Function, 0, u32_type());
    let operands = assembler.parameter(function, ParameterRole::Function, 1, u32vec_type());
    let next_register = assembler.parameter(function, ParameterRole::Function, 2, u32_type());

    let entry = assembler.block_id();
    let opcode_bool_and = assembler.block_id();
    let opcode_bool_or = assembler.block_id();
    let opcode_equal = assembler.block_id();
    let opcode_not_equal = assembler.block_id();
    let opcode_less = assembler.block_id();
    let opcode_less_equal = assembler.block_id();
    let opcode_greater = assembler.block_id();
    let opcode_greater_equal = assembler.block_id();
    let opcode_int_add = assembler.block_id();
    let opcode_int_sub = assembler.block_id();
    let opcode_int_mul = assembler.block_id();
    let opcode_int_div = assembler.block_id();
    let opcode_int_rem = assembler.block_id();
    let opcode_int_neg = assembler.block_id();
    let opcode_int_shl = assembler.block_id();
    let opcode_int_shr = assembler.block_id();
    let opcode_float_add = assembler.block_id();
    let opcode_float_sub = assembler.block_id();
    let opcode_float_mul = assembler.block_id();
    let opcode_float_div = assembler.block_id();
    let opcode_float_neg = assembler.block_id();
    let opcode_option_some = assembler.block_id();
    let opcode_option_none = assembler.block_id();
    let opcode_result_ok = assembler.block_id();
    let opcode_result_err = assembler.block_id();
    let opcode_cell_new = assembler.block_id();
    let opcode_cell_get = assembler.block_id();
    let opcode_cell_set = assembler.block_id();
    let opcode_value_hash = assembler.block_id();
    let opcode_float_fma = assembler.block_id();
    let opcode_tuple_new = assembler.block_id();
    let opcode_vector_new = assembler.block_id();
    let opcode_vector_len = assembler.block_id();
    let opcode_vector_get = assembler.block_id();
    let opcode_vector_set = assembler.block_id();
    let opcode_map_new = assembler.block_id();
    let opcode_map_get = assembler.block_id();
    let opcode_map_contains = assembler.block_id();
    let opcode_map_insert = assembler.block_id();
    let opcode_map_remove = assembler.block_id();
    let count_zero = assembler.block_id();
    let count_one = assembler.block_id();
    let count_two = assembler.block_id();
    let count_three = assembler.block_id();
    let map_new_count = assembler.block_id();
    let map_new_remainder = assembler.block_id();
    let validate = assembler.block_id();
    let emit = assembler.block_id();
    let success = assembler.block_id();
    let forward_error = assembler.block_id();
    let opcode_error = assembler.block_id();
    let signature_error = assembler.block_id();
    let resource_error = assembler.block_id();

    let scalar_tags = [
        Opcode::BoolNot,
        Opcode::BoolAnd,
        Opcode::BoolOr,
        Opcode::Equal,
        Opcode::NotEqual,
        Opcode::LessThan,
        Opcode::LessEqual,
        Opcode::GreaterThan,
        Opcode::GreaterEqual,
        Opcode::IntAddChecked,
        Opcode::IntSubChecked,
        Opcode::IntMulChecked,
        Opcode::IntDivChecked,
        Opcode::IntRemChecked,
        Opcode::IntNegChecked,
        Opcode::IntShlChecked,
        Opcode::IntShrChecked,
        Opcode::FloatAdd,
        Opcode::FloatSub,
        Opcode::FloatMul,
        Opcode::FloatDiv,
        Opcode::FloatNeg,
        Opcode::OptionSome,
        Opcode::OptionNone,
        Opcode::ResultOk,
        Opcode::ResultErr,
        Opcode::CellNew,
        Opcode::CellGet,
        Opcode::CellSet,
        Opcode::ValueHash,
    ]
    .map(|value| assembler.constant(u32_value(u128::from(value.tag()))));
    let variadic_tags = [
        Opcode::FloatFma,
        Opcode::TupleNew,
        Opcode::VectorNew,
        Opcode::VectorLen,
        Opcode::VectorGet,
        Opcode::VectorSet,
        Opcode::MapNew,
        Opcode::MapGet,
        Opcode::MapContains,
        Opcode::MapInsert,
        Opcode::MapRemove,
    ]
    .map(|value| assembler.constant(u32_value(u128::from(value.tag()))));
    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let two_u64 = assembler.constant(u64_value(2));
    let three_u64 = assembler.constant(u64_value(3));
    let one_u32 = assembler.constant(u32_value(1));
    let opcode_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::OpcodeUnsupported.numeric(),
    )));
    let signature_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
    )));
    let resource_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let dispatch = |assembler: &mut InventoryAssembler,
                    block: EntityId,
                    tag: EntityId,
                    matched: EntityId,
                    unmatched: EntityId| {
        let tag_value = assembler.constant_ref(block, tag, u32_type());
        let matches = assembler.operation(
            block,
            Opcode::Equal,
            vec![ValueRef::Parameter(opcode), operation_value(tag_value)],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![tag_value, matches],
            inventory_cond(
                operation_value(matches),
                matched,
                Vec::new(),
                unmatched,
                Vec::new(),
            ),
        );
    };
    dispatch(
        &mut assembler,
        entry,
        scalar_tags[0],
        count_one,
        opcode_bool_and,
    );
    dispatch(
        &mut assembler,
        opcode_bool_and,
        scalar_tags[1],
        count_two,
        opcode_bool_or,
    );
    dispatch(
        &mut assembler,
        opcode_bool_or,
        scalar_tags[2],
        count_two,
        opcode_equal,
    );
    dispatch(
        &mut assembler,
        opcode_equal,
        scalar_tags[3],
        count_two,
        opcode_not_equal,
    );
    dispatch(
        &mut assembler,
        opcode_not_equal,
        scalar_tags[4],
        count_two,
        opcode_less,
    );
    dispatch(
        &mut assembler,
        opcode_less,
        scalar_tags[5],
        count_two,
        opcode_less_equal,
    );
    dispatch(
        &mut assembler,
        opcode_less_equal,
        scalar_tags[6],
        count_two,
        opcode_greater,
    );
    dispatch(
        &mut assembler,
        opcode_greater,
        scalar_tags[7],
        count_two,
        opcode_greater_equal,
    );
    dispatch(
        &mut assembler,
        opcode_greater_equal,
        scalar_tags[8],
        count_two,
        opcode_int_add,
    );
    dispatch(
        &mut assembler,
        opcode_int_add,
        scalar_tags[9],
        count_two,
        opcode_int_sub,
    );
    dispatch(
        &mut assembler,
        opcode_int_sub,
        scalar_tags[10],
        count_two,
        opcode_int_mul,
    );
    dispatch(
        &mut assembler,
        opcode_int_mul,
        scalar_tags[11],
        count_two,
        opcode_int_div,
    );
    dispatch(
        &mut assembler,
        opcode_int_div,
        scalar_tags[12],
        count_two,
        opcode_int_rem,
    );
    dispatch(
        &mut assembler,
        opcode_int_rem,
        scalar_tags[13],
        count_two,
        opcode_int_neg,
    );
    dispatch(
        &mut assembler,
        opcode_int_neg,
        scalar_tags[14],
        count_one,
        opcode_int_shl,
    );
    dispatch(
        &mut assembler,
        opcode_int_shl,
        scalar_tags[15],
        count_two,
        opcode_int_shr,
    );
    dispatch(
        &mut assembler,
        opcode_int_shr,
        scalar_tags[16],
        count_two,
        opcode_float_add,
    );
    dispatch(
        &mut assembler,
        opcode_float_add,
        scalar_tags[17],
        count_two,
        opcode_float_sub,
    );
    dispatch(
        &mut assembler,
        opcode_float_sub,
        scalar_tags[18],
        count_two,
        opcode_float_mul,
    );
    dispatch(
        &mut assembler,
        opcode_float_mul,
        scalar_tags[19],
        count_two,
        opcode_float_div,
    );
    dispatch(
        &mut assembler,
        opcode_float_div,
        scalar_tags[20],
        count_two,
        opcode_float_neg,
    );
    dispatch(
        &mut assembler,
        opcode_float_neg,
        scalar_tags[21],
        count_one,
        opcode_option_some,
    );
    dispatch(
        &mut assembler,
        opcode_option_some,
        scalar_tags[22],
        count_one,
        opcode_option_none,
    );
    dispatch(
        &mut assembler,
        opcode_option_none,
        scalar_tags[23],
        count_zero,
        opcode_result_ok,
    );
    dispatch(
        &mut assembler,
        opcode_result_ok,
        scalar_tags[24],
        count_one,
        opcode_result_err,
    );
    dispatch(
        &mut assembler,
        opcode_result_err,
        scalar_tags[25],
        count_one,
        opcode_cell_new,
    );
    dispatch(
        &mut assembler,
        opcode_cell_new,
        scalar_tags[26],
        count_one,
        opcode_cell_get,
    );
    dispatch(
        &mut assembler,
        opcode_cell_get,
        scalar_tags[27],
        count_one,
        opcode_cell_set,
    );
    dispatch(
        &mut assembler,
        opcode_cell_set,
        scalar_tags[28],
        count_two,
        opcode_value_hash,
    );
    dispatch(
        &mut assembler,
        opcode_value_hash,
        scalar_tags[29],
        count_one,
        opcode_float_fma,
    );
    dispatch(
        &mut assembler,
        opcode_float_fma,
        variadic_tags[0],
        count_three,
        opcode_tuple_new,
    );
    dispatch(
        &mut assembler,
        opcode_tuple_new,
        variadic_tags[1],
        validate,
        opcode_vector_new,
    );
    dispatch(
        &mut assembler,
        opcode_vector_new,
        variadic_tags[2],
        validate,
        opcode_vector_len,
    );
    dispatch(
        &mut assembler,
        opcode_vector_len,
        variadic_tags[3],
        count_one,
        opcode_vector_get,
    );
    dispatch(
        &mut assembler,
        opcode_vector_get,
        variadic_tags[4],
        count_two,
        opcode_vector_set,
    );
    dispatch(
        &mut assembler,
        opcode_vector_set,
        variadic_tags[5],
        count_three,
        opcode_map_new,
    );
    dispatch(
        &mut assembler,
        opcode_map_new,
        variadic_tags[6],
        map_new_count,
        opcode_map_get,
    );
    dispatch(
        &mut assembler,
        opcode_map_get,
        variadic_tags[7],
        count_two,
        opcode_map_contains,
    );
    dispatch(
        &mut assembler,
        opcode_map_contains,
        variadic_tags[8],
        count_two,
        opcode_map_insert,
    );
    dispatch(
        &mut assembler,
        opcode_map_insert,
        variadic_tags[9],
        count_three,
        opcode_map_remove,
    );
    dispatch(
        &mut assembler,
        opcode_map_remove,
        variadic_tags[10],
        count_two,
        opcode_error,
    );

    let count_check = |assembler: &mut InventoryAssembler, block: EntityId, expected: EntityId| {
        let length = assembler.operation(
            block,
            Opcode::VectorLen,
            vec![ValueRef::Parameter(operands)],
            u64_type(),
            Immediate::None,
        );
        let expected_value = assembler.constant_ref(block, expected, u64_type());
        let matches = assembler.operation(
            block,
            Opcode::Equal,
            vec![operation_value(length), operation_value(expected_value)],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![length, expected_value, matches],
            inventory_cond(
                operation_value(matches),
                validate,
                Vec::new(),
                signature_error,
                Vec::new(),
            ),
        );
    };
    count_check(&mut assembler, count_zero, zero_u64);
    count_check(&mut assembler, count_one, one_u64);
    count_check(&mut assembler, count_two, two_u64);
    count_check(&mut assembler, count_three, three_u64);

    let map_length = assembler.operation(
        map_new_count,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(operands)],
        u64_type(),
        Immediate::None,
    );
    let map_two = assembler.constant_ref(map_new_count, two_u64, u64_type());
    let remainder = assembler.operation(
        map_new_count,
        Opcode::IntRemChecked,
        vec![operation_value(map_length), operation_value(map_two)],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        map_new_count,
        function,
        Vec::new(),
        vec![map_length, map_two, remainder],
        inventory_switch(
            operation_value(remainder),
            vec![
                (
                    BuiltinCase::Ok,
                    map_new_remainder,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );
    let found_remainder =
        assembler.parameter(map_new_remainder, ParameterRole::Block, 0, u64_type());
    let map_zero = assembler.constant_ref(map_new_remainder, zero_u64, u64_type());
    let even = assembler.operation(
        map_new_remainder,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(found_remainder),
            operation_value(map_zero),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        map_new_remainder,
        function,
        vec![found_remainder],
        vec![map_zero, even],
        inventory_cond(
            operation_value(even),
            validate,
            Vec::new(),
            signature_error,
            Vec::new(),
        ),
    );

    let validation = assembler.operation(
        validate,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(operands),
            ValueRef::Parameter(next_register),
        ],
        unit_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: validator,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        validate,
        function,
        Vec::new(),
        vec![validation],
        inventory_switch(
            operation_value(validation),
            vec![
                (BuiltinCase::Ok, emit, Vec::new()),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let results = assembler.operation(
        emit,
        Opcode::VectorNew,
        vec![ValueRef::Parameter(next_register)],
        u32vec_type(),
        Immediate::None,
    );
    let instruction = assembler.operation(
        emit,
        Opcode::TupleNew,
        vec![
            ValueRef::Parameter(opcode),
            ValueRef::Parameter(operands),
            operation_value(results),
        ],
        single_lowered_type(),
        Immediate::None,
    );
    let one = assembler.constant_ref(emit, one_u32, u32_type());
    let advanced = assembler.operation(
        emit,
        Opcode::IntAddChecked,
        vec![ValueRef::Parameter(next_register), operation_value(one)],
        arithmetic_result_type(u32_type()),
        Immediate::None,
    );
    assembler.push_block(
        emit,
        function,
        Vec::new(),
        vec![results, instruction, one, advanced],
        inventory_switch(
            operation_value(advanced),
            vec![
                (
                    BuiltinCase::Ok,
                    success,
                    vec![
                        SwitchArgument::Value(operation_value(instruction)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let success_instruction =
        assembler.parameter(success, ParameterRole::Block, 0, single_lowered_type());
    let success_frontier = assembler.parameter(success, ParameterRole::Block, 1, u32_type());
    let summary = assembler.operation(
        success,
        Opcode::TupleNew,
        vec![
            ValueRef::Parameter(success_instruction),
            ValueRef::Parameter(success_frontier),
        ],
        variadic_summary_type(),
        Immediate::None,
    );
    let accepted = assembler.operation(
        success,
        Opcode::ResultOk,
        vec![operation_value(summary)],
        variadic_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success,
        function,
        vec![success_instruction, success_frontier],
        vec![summary, accepted],
        Terminator::Return(ReturnTerminator {
            value: operation_value(accepted),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_error = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        variadic_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_error],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_error),
        }),
    );
    for (block, code) in [
        (opcode_error, opcode_error_code),
        (signature_error, signature_error_code),
        (resource_error, resource_error_code),
    ] {
        let value = assembler.constant_ref(block, code, u32_type());
        let rejected = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![operation_value(value)],
            variadic_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![value, rejected],
            Terminator::Return(ReturnTerminator {
                value: operation_value(rejected),
            }),
        );
    }

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![opcode, operands, next_register],
        result_type: variadic_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph, validator_graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: Vec::new(),
    }
}

/// Lowers the complete frozen bootstrap immediate-bearing family into a
/// instruction model. Compact fields support Sley-side validation while the
/// exact canonical immediate bytes retain every identity/member/type payload.
#[allow(clippy::too_many_lines)]
fn bootstrap_immediate_lowerer() -> LowerScaffold {
    let function = inventory_id(5, 8);
    let validator = inventory_id(5, 9);
    let mut assembler = InventoryAssembler::new();
    let validator_graph = build_register_vector_validator(&mut assembler, validator);

    let opcode = assembler.parameter(function, ParameterRole::Function, 0, u32_type());
    let operands = assembler.parameter(function, ParameterRole::Function, 1, u32vec_type());
    let immediate_tag = assembler.parameter(function, ParameterRole::Function, 2, u32_type());
    let primary = assembler.parameter(function, ParameterRole::Function, 3, u64_type());
    let secondary = assembler.parameter(function, ParameterRole::Function, 4, u64_type());
    let immediate_bytes =
        assembler.parameter(function, ParameterRole::Function, 5, TypeExpr::Bytes);
    let next_register = assembler.parameter(function, ParameterRole::Function, 6, u32_type());

    let entry = assembler.block_id();
    let opcode_tuple_get = assembler.block_id();
    let opcode_record_new = assembler.block_id();
    let opcode_record_get = assembler.block_id();
    let opcode_variant_new = assembler.block_id();
    let opcode_variant_get = assembler.block_id();
    let opcode_call_direct = assembler.block_id();
    let opcode_adapter_invoke = assembler.block_id();
    let tag_constant = assembler.block_id();
    let tag_tuple = assembler.block_id();
    let tag_record_new = assembler.block_id();
    let tag_record_get = assembler.block_id();
    let tag_variant_new = assembler.block_id();
    let tag_variant_get = assembler.block_id();
    let tag_call = assembler.block_id();
    let tag_adapter = assembler.block_id();
    let count_zero = assembler.block_id();
    let count_one = assembler.block_id();
    let count_two = assembler.block_id();
    let variant_count = assembler.block_id();
    let validate = assembler.block_id();
    let emit = assembler.block_id();
    let success = assembler.block_id();
    let forward_error = assembler.block_id();
    let opcode_error = assembler.block_id();
    let immediate_error = assembler.block_id();
    let signature_error = assembler.block_id();
    let resource_error = assembler.block_id();

    let opcode_tags = [
        Opcode::ConstantRef,
        Opcode::TupleGet,
        Opcode::RecordNew,
        Opcode::RecordGet,
        Opcode::VariantNew,
        Opcode::VariantGet,
        Opcode::CallDirect,
        Opcode::AdapterInvoke,
    ]
    .map(|value| assembler.constant(u32_value(u128::from(value.tag()))));
    let immediate_tags = [
        Immediate::Entity(id(0)).tag(),
        Immediate::Index(0).tag(),
        Immediate::Field(MemberId::from_bytes([0; 32])).tag(),
        Immediate::Variant(VariantImmediate {
            definition: id(0),
            member_id: MemberId::from_bytes([0; 32]),
        })
        .tag(),
        Immediate::Function(FunctionRefValue {
            function: id(0),
            type_arguments: Vec::new(),
        })
        .tag(),
    ]
    .map(|tag| assembler.constant(u32_value(u128::from(tag))));
    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let two_u64 = assembler.constant(u64_value(2));
    let one_u32 = assembler.constant(u32_value(1));
    let opcode_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::OpcodeUnsupported.numeric(),
    )));
    let immediate_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ImmediateMismatch.numeric(),
    )));
    let signature_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
    )));
    let resource_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let dispatch = |assembler: &mut InventoryAssembler,
                    block: EntityId,
                    expected: EntityId,
                    matched: EntityId,
                    unmatched: EntityId| {
        let expected_value = assembler.constant_ref(block, expected, u32_type());
        let matches = assembler.operation(
            block,
            Opcode::Equal,
            vec![ValueRef::Parameter(opcode), operation_value(expected_value)],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![expected_value, matches],
            inventory_cond(
                operation_value(matches),
                matched,
                Vec::new(),
                unmatched,
                Vec::new(),
            ),
        );
    };
    dispatch(
        &mut assembler,
        entry,
        opcode_tags[0],
        tag_constant,
        opcode_tuple_get,
    );
    dispatch(
        &mut assembler,
        opcode_tuple_get,
        opcode_tags[1],
        tag_tuple,
        opcode_record_new,
    );
    dispatch(
        &mut assembler,
        opcode_record_new,
        opcode_tags[2],
        tag_record_new,
        opcode_record_get,
    );
    dispatch(
        &mut assembler,
        opcode_record_get,
        opcode_tags[3],
        tag_record_get,
        opcode_variant_new,
    );
    dispatch(
        &mut assembler,
        opcode_variant_new,
        opcode_tags[4],
        tag_variant_new,
        opcode_variant_get,
    );
    dispatch(
        &mut assembler,
        opcode_variant_get,
        opcode_tags[5],
        tag_variant_get,
        opcode_call_direct,
    );
    dispatch(
        &mut assembler,
        opcode_call_direct,
        opcode_tags[6],
        tag_call,
        opcode_adapter_invoke,
    );
    dispatch(
        &mut assembler,
        opcode_adapter_invoke,
        opcode_tags[7],
        tag_adapter,
        opcode_error,
    );

    let tag_check = |assembler: &mut InventoryAssembler,
                     block: EntityId,
                     expected: EntityId,
                     target: EntityId| {
        let expected_value = assembler.constant_ref(block, expected, u32_type());
        let matches = assembler.operation(
            block,
            Opcode::Equal,
            vec![
                ValueRef::Parameter(immediate_tag),
                operation_value(expected_value),
            ],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![expected_value, matches],
            inventory_cond(
                operation_value(matches),
                target,
                Vec::new(),
                immediate_error,
                Vec::new(),
            ),
        );
    };
    tag_check(&mut assembler, tag_constant, immediate_tags[0], count_zero);
    tag_check(&mut assembler, tag_tuple, immediate_tags[1], count_one);
    tag_check(&mut assembler, tag_record_new, immediate_tags[0], validate);
    tag_check(&mut assembler, tag_record_get, immediate_tags[2], count_one);
    tag_check(
        &mut assembler,
        tag_variant_new,
        immediate_tags[3],
        variant_count,
    );
    tag_check(
        &mut assembler,
        tag_variant_get,
        immediate_tags[3],
        count_one,
    );
    tag_check(&mut assembler, tag_call, immediate_tags[4], validate);
    tag_check(&mut assembler, tag_adapter, immediate_tags[0], count_two);

    let count_check = |assembler: &mut InventoryAssembler, block: EntityId, expected: EntityId| {
        let length = assembler.operation(
            block,
            Opcode::VectorLen,
            vec![ValueRef::Parameter(operands)],
            u64_type(),
            Immediate::None,
        );
        let expected_value = assembler.constant_ref(block, expected, u64_type());
        let matches = assembler.operation(
            block,
            Opcode::Equal,
            vec![operation_value(length), operation_value(expected_value)],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![length, expected_value, matches],
            inventory_cond(
                operation_value(matches),
                validate,
                Vec::new(),
                signature_error,
                Vec::new(),
            ),
        );
    };
    count_check(&mut assembler, count_zero, zero_u64);
    count_check(&mut assembler, count_one, one_u64);
    count_check(&mut assembler, count_two, two_u64);

    let variant_length = assembler.operation(
        variant_count,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(operands)],
        u64_type(),
        Immediate::None,
    );
    let variant_limit = assembler.constant_ref(variant_count, two_u64, u64_type());
    let variant_valid = assembler.operation(
        variant_count,
        Opcode::LessThan,
        vec![
            operation_value(variant_length),
            operation_value(variant_limit),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        variant_count,
        function,
        Vec::new(),
        vec![variant_length, variant_limit, variant_valid],
        inventory_cond(
            operation_value(variant_valid),
            validate,
            Vec::new(),
            signature_error,
            Vec::new(),
        ),
    );

    let validation = assembler.operation(
        validate,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(operands),
            ValueRef::Parameter(next_register),
        ],
        unit_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: validator,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        validate,
        function,
        Vec::new(),
        vec![validation],
        inventory_switch(
            operation_value(validation),
            vec![
                (BuiltinCase::Ok, emit, Vec::new()),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let results = assembler.operation(
        emit,
        Opcode::VectorNew,
        vec![ValueRef::Parameter(next_register)],
        u32vec_type(),
        Immediate::None,
    );
    let instruction = assembler.operation(
        emit,
        Opcode::TupleNew,
        vec![
            ValueRef::Parameter(opcode),
            ValueRef::Parameter(operands),
            operation_value(results),
            ValueRef::Parameter(immediate_tag),
            ValueRef::Parameter(primary),
            ValueRef::Parameter(secondary),
            ValueRef::Parameter(immediate_bytes),
        ],
        immediate_instruction_type(),
        Immediate::None,
    );
    let one = assembler.constant_ref(emit, one_u32, u32_type());
    let advanced = assembler.operation(
        emit,
        Opcode::IntAddChecked,
        vec![ValueRef::Parameter(next_register), operation_value(one)],
        arithmetic_result_type(u32_type()),
        Immediate::None,
    );
    assembler.push_block(
        emit,
        function,
        Vec::new(),
        vec![results, instruction, one, advanced],
        inventory_switch(
            operation_value(advanced),
            vec![
                (
                    BuiltinCase::Ok,
                    success,
                    vec![
                        SwitchArgument::Value(operation_value(instruction)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let success_instruction = assembler.parameter(
        success,
        ParameterRole::Block,
        0,
        immediate_instruction_type(),
    );
    let success_frontier = assembler.parameter(success, ParameterRole::Block, 1, u32_type());
    let summary = assembler.operation(
        success,
        Opcode::TupleNew,
        vec![
            ValueRef::Parameter(success_instruction),
            ValueRef::Parameter(success_frontier),
        ],
        immediate_summary_type(),
        Immediate::None,
    );
    let accepted = assembler.operation(
        success,
        Opcode::ResultOk,
        vec![operation_value(summary)],
        immediate_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success,
        function,
        vec![success_instruction, success_frontier],
        vec![summary, accepted],
        Terminator::Return(ReturnTerminator {
            value: operation_value(accepted),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_error = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        immediate_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_error],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_error),
        }),
    );
    for (block, code) in [
        (opcode_error, opcode_error_code),
        (immediate_error, immediate_error_code),
        (signature_error, signature_error_code),
        (resource_error, resource_error_code),
    ] {
        let value = assembler.constant_ref(block, code, u32_type());
        let rejected = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![operation_value(value)],
            immediate_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![value, rejected],
            Terminator::Return(ReturnTerminator {
                value: operation_value(rejected),
            }),
        );
    }

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            opcode,
            operands,
            immediate_tag,
            primary,
            secondary,
            immediate_bytes,
            next_register,
        ],
        result_type: immediate_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph, validator_graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: Vec::new(),
    }
}

/// Walks an execution-time inventory of immediate-bearing operations. The
/// Sley caller owns ordering, first-failure propagation, instruction
/// accumulation, and the dense register frontier; the Sley callee owns the
/// per-operation opcode/immediate/arity/reference judgment.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn ordered_immediate_inventory_lowerer() -> LowerScaffold {
    let base = bootstrap_immediate_lowerer();
    let lower_operation = base.entry.entity_id;
    let mut functions = base.functions;
    let mut assembler = InventoryAssembler {
        next_block: u16::try_from(base.blocks.len() + 1).expect("fixture block count fits u16"),
        next_parameter: u16::try_from(base.parameters.len() + 1)
            .expect("fixture parameter count fits u16"),
        next_operation: u16::try_from(base.operations.len() + 1)
            .expect("fixture operation count fits u16"),
        next_constant: u16::try_from(base.constants.len() + 1)
            .expect("fixture constant count fits u16"),
        parameters: base.parameters,
        blocks: base.blocks,
        operations: base.operations,
        constants: base.constants,
    };
    let function = inventory_id(5, 10);
    let inventory = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        immediate_inventory_type(),
    );
    let first_register = assembler.parameter(function, ParameterRole::Function, 1, u32_type());

    let entry = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let unpack = assembler.block_id();
    let call = assembler.block_id();
    let accept = assembler.block_id();
    let advance = assembler.block_id();
    let done = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();

    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let zero = assembler.constant_ref(entry, zero_u64, u64_type());
    let length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(inventory)],
        u64_type(),
        Immediate::None,
    );
    let model = assembler.operation(
        entry,
        Opcode::MapNew,
        Vec::new(),
        empty_immediate_inventory_model_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![zero, length, model],
        inventory_switch(
            operation_value(model),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::Value(operation_value(zero)),
                        SwitchArgument::Value(ValueRef::Parameter(first_register)),
                        SwitchArgument::Value(ValueRef::Parameter(inventory)),
                        SwitchArgument::Value(operation_value(length)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let check_parameters = immediate_inventory_loop_parameters(&mut assembler, check);
    let has_row = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_parameters.index),
            ValueRef::Parameter(check_parameters.length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        immediate_inventory_loop_ids(check_parameters),
        vec![has_row],
        inventory_cond(
            operation_value(has_row),
            get,
            immediate_inventory_loop_values(check_parameters),
            done,
            vec![
                ValueRef::Parameter(check_parameters.model),
                ValueRef::Parameter(check_parameters.index),
                ValueRef::Parameter(check_parameters.next_register),
            ],
        ),
    );

    let get_parameters = immediate_inventory_loop_parameters(&mut assembler, get);
    let row = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(get_parameters.inventory),
            ValueRef::Parameter(get_parameters.index),
        ],
        TypeExpr::Option(Box::new(immediate_inventory_row_type())),
        Immediate::None,
    );
    let mut unpack_arguments = vec![SwitchArgument::CasePayload];
    unpack_arguments.extend(inventory_switch_values(immediate_inventory_loop_values(
        get_parameters,
    )));
    assembler.push_block(
        get,
        function,
        immediate_inventory_loop_ids(get_parameters),
        vec![row],
        inventory_switch(
            operation_value(row),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (BuiltinCase::Some, unpack, unpack_arguments),
            ],
        ),
    );

    let unpack_row = assembler.parameter(
        unpack,
        ParameterRole::Block,
        0,
        immediate_inventory_row_type(),
    );
    let unpack_loop = ImmediateInventoryLoopParameters {
        index: assembler.parameter(unpack, ParameterRole::Block, 1, u64_type()),
        next_register: assembler.parameter(unpack, ParameterRole::Block, 2, u32_type()),
        inventory: assembler.parameter(unpack, ParameterRole::Block, 3, immediate_inventory_type()),
        length: assembler.parameter(unpack, ParameterRole::Block, 4, u64_type()),
        model: assembler.parameter(
            unpack,
            ParameterRole::Block,
            5,
            immediate_inventory_model_type(),
        ),
    };
    let field_types = [
        u32_type(),
        u32vec_type(),
        u32_type(),
        u64_type(),
        u64_type(),
        TypeExpr::Bytes,
    ];
    let fields = field_types
        .into_iter()
        .enumerate()
        .map(|(index, value_type)| {
            assembler.operation(
                unpack,
                Opcode::TupleGet,
                vec![ValueRef::Parameter(unpack_row)],
                value_type,
                Immediate::Index(u32::try_from(index).expect("row field index fits u32")),
            )
        })
        .collect::<Vec<_>>();
    let mut unpack_ids = vec![unpack_row];
    unpack_ids.extend(immediate_inventory_loop_ids(unpack_loop));
    let mut call_arguments = immediate_inventory_loop_values(unpack_loop);
    call_arguments.extend(fields.iter().copied().map(operation_value));
    assembler.push_block(
        unpack,
        function,
        unpack_ids,
        fields.clone(),
        inventory_branch(call, call_arguments),
    );

    let call_loop = immediate_inventory_loop_parameters(&mut assembler, call);
    let call_opcode = assembler.parameter(call, ParameterRole::Block, 5, u32_type());
    let call_operands = assembler.parameter(call, ParameterRole::Block, 6, u32vec_type());
    let call_tag = assembler.parameter(call, ParameterRole::Block, 7, u32_type());
    let call_primary = assembler.parameter(call, ParameterRole::Block, 8, u64_type());
    let call_secondary = assembler.parameter(call, ParameterRole::Block, 9, u64_type());
    let call_immediate_bytes = assembler.parameter(call, ParameterRole::Block, 10, TypeExpr::Bytes);
    let lowered = assembler.operation(
        call,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(call_opcode),
            ValueRef::Parameter(call_operands),
            ValueRef::Parameter(call_tag),
            ValueRef::Parameter(call_primary),
            ValueRef::Parameter(call_secondary),
            ValueRef::Parameter(call_immediate_bytes),
            ValueRef::Parameter(call_loop.next_register),
        ],
        immediate_result_type(),
        Immediate::Function(FunctionRefValue {
            function: lower_operation,
            type_arguments: Vec::new(),
        }),
    );
    let mut call_ids = immediate_inventory_loop_ids(call_loop);
    call_ids.extend([
        call_opcode,
        call_operands,
        call_tag,
        call_primary,
        call_secondary,
        call_immediate_bytes,
    ]);
    let accept_arguments = vec![
        SwitchArgument::CasePayload,
        SwitchArgument::Value(ValueRef::Parameter(call_loop.index)),
        SwitchArgument::Value(ValueRef::Parameter(call_loop.inventory)),
        SwitchArgument::Value(ValueRef::Parameter(call_loop.length)),
        SwitchArgument::Value(ValueRef::Parameter(call_loop.model)),
    ];
    assembler.push_block(
        call,
        function,
        call_ids,
        vec![lowered],
        inventory_switch(
            operation_value(lowered),
            vec![
                (BuiltinCase::Ok, accept, accept_arguments),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let accepted = assembler.parameter(accept, ParameterRole::Block, 0, immediate_summary_type());
    let accept_index = assembler.parameter(accept, ParameterRole::Block, 1, u64_type());
    let accept_inventory =
        assembler.parameter(accept, ParameterRole::Block, 2, immediate_inventory_type());
    let accept_length = assembler.parameter(accept, ParameterRole::Block, 3, u64_type());
    let accept_model = assembler.parameter(
        accept,
        ParameterRole::Block,
        4,
        immediate_inventory_model_type(),
    );
    let accepted_instruction = assembler.operation(
        accept,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(accepted)],
        immediate_instruction_type(),
        Immediate::Index(0),
    );
    let accepted_frontier = assembler.operation(
        accept,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(accepted)],
        u32_type(),
        Immediate::Index(1),
    );
    let inserted = assembler.operation(
        accept,
        Opcode::MapInsert,
        vec![
            ValueRef::Parameter(accept_model),
            ValueRef::Parameter(accept_index),
            operation_value(accepted_instruction),
        ],
        immediate_inventory_model_type(),
        Immediate::None,
    );
    assembler.push_block(
        accept,
        function,
        vec![
            accepted,
            accept_index,
            accept_inventory,
            accept_length,
            accept_model,
        ],
        vec![accepted_instruction, accepted_frontier, inserted],
        inventory_branch(
            advance,
            vec![
                ValueRef::Parameter(accept_index),
                operation_value(accepted_frontier),
                ValueRef::Parameter(accept_inventory),
                ValueRef::Parameter(accept_length),
                operation_value(inserted),
            ],
        ),
    );

    let advance_parameters = immediate_inventory_loop_parameters(&mut assembler, advance);
    let one = assembler.constant_ref(advance, one_u64, u64_type());
    let next_index = assembler.operation(
        advance,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_parameters.index),
            operation_value(one),
        ],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance,
        function,
        immediate_inventory_loop_ids(advance_parameters),
        vec![one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(
                            advance_parameters.next_register,
                        )),
                        SwitchArgument::Value(ValueRef::Parameter(advance_parameters.inventory)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_parameters.length)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_parameters.model)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let done_model = assembler.parameter(
        done,
        ParameterRole::Block,
        0,
        immediate_inventory_model_type(),
    );
    let done_count = assembler.parameter(done, ParameterRole::Block, 1, u64_type());
    let done_frontier = assembler.parameter(done, ParameterRole::Block, 2, u32_type());
    let summary = assembler.operation(
        done,
        Opcode::TupleNew,
        vec![
            ValueRef::Parameter(done_model),
            ValueRef::Parameter(done_count),
            ValueRef::Parameter(done_frontier),
        ],
        immediate_inventory_summary_type(),
        Immediate::None,
    );
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![operation_value(summary)],
        immediate_inventory_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![done_model, done_count, done_frontier],
        vec![summary, success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        immediate_inventory_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        immediate_inventory_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![inventory, first_register],
        result_type: immediate_inventory_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    functions.insert(0, graph.clone());
    LowerScaffold {
        types: base.types,
        entry: graph,
        functions,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: base.adapters,
    }
}

/// Composes the complete immediate-free target family with the bootstrap
/// immediate-bearing family behind the ordered inventory entry. The
/// dispatcher recognizes the opcode family before checking the immediate so
/// native opcode/immediate/signature failure precedence is retained.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn mixed_operation_inventory_lowerer() -> LowerScaffold {
    let mut base = ordered_immediate_inventory_lowerer();
    let immediate_free = rebase_scaffold_artifacts(immediate_free_operation_lowerer(), 16);
    let immediate_free_entry = immediate_free.entry.entity_id;
    base.functions.extend(immediate_free.functions);
    base.parameters.extend(immediate_free.parameters);
    base.blocks.extend(immediate_free.blocks);
    base.operations.extend(immediate_free.operations);
    base.constants.extend(immediate_free.constants);
    assert!(immediate_free.adapters.is_empty());

    let function = inventory_id(5, 11);
    let immediate_entry = inventory_id(5, 8);
    let mut retargeted = 0;
    let inventory_entry_blocks = base
        .blocks
        .iter()
        .filter(|block| block.function == base.entry.entity_id)
        .map(|block| block.entity_id)
        .collect::<Vec<_>>();
    for operation in &mut base.operations {
        if inventory_entry_blocks.contains(&operation.block)
            && operation.opcode == Opcode::CallDirect
            && matches!(
                &operation.immediate,
                Immediate::Function(reference) if reference.function == immediate_entry
            )
        {
            operation.immediate = Immediate::Function(FunctionRefValue {
                function,
                type_arguments: Vec::new(),
            });
            retargeted += 1;
        }
    }
    assert_eq!(retargeted, 1, "one inventory call is retargeted");

    let mut assembler = InventoryAssembler {
        next_block: u16::try_from(base.blocks.len() + 1).expect("fixture block count fits u16"),
        next_parameter: u16::try_from(base.parameters.len() + 1)
            .expect("fixture parameter count fits u16"),
        next_operation: u16::try_from(base.operations.len() + 1)
            .expect("fixture operation count fits u16"),
        next_constant: u16::try_from(base.constants.len() + 1)
            .expect("fixture constant count fits u16"),
        parameters: base.parameters,
        blocks: base.blocks,
        operations: base.operations,
        constants: base.constants,
    };
    let opcode = assembler.parameter(function, ParameterRole::Function, 0, u32_type());
    let operands = assembler.parameter(function, ParameterRole::Function, 1, u32vec_type());
    let immediate_tag = assembler.parameter(function, ParameterRole::Function, 2, u32_type());
    let primary = assembler.parameter(function, ParameterRole::Function, 3, u64_type());
    let secondary = assembler.parameter(function, ParameterRole::Function, 4, u64_type());
    let immediate_bytes =
        assembler.parameter(function, ParameterRole::Function, 5, TypeExpr::Bytes);
    let next_register = assembler.parameter(function, ParameterRole::Function, 6, u32_type());

    let immediate_free_opcodes = [
        Opcode::BoolNot,
        Opcode::BoolAnd,
        Opcode::BoolOr,
        Opcode::Equal,
        Opcode::NotEqual,
        Opcode::LessThan,
        Opcode::LessEqual,
        Opcode::GreaterThan,
        Opcode::GreaterEqual,
        Opcode::IntAddChecked,
        Opcode::IntSubChecked,
        Opcode::IntMulChecked,
        Opcode::IntDivChecked,
        Opcode::IntRemChecked,
        Opcode::IntNegChecked,
        Opcode::IntShlChecked,
        Opcode::IntShrChecked,
        Opcode::FloatAdd,
        Opcode::FloatSub,
        Opcode::FloatMul,
        Opcode::FloatDiv,
        Opcode::FloatNeg,
        Opcode::FloatFma,
        Opcode::OptionSome,
        Opcode::OptionNone,
        Opcode::ResultOk,
        Opcode::ResultErr,
        Opcode::TupleNew,
        Opcode::VectorNew,
        Opcode::VectorLen,
        Opcode::VectorGet,
        Opcode::VectorSet,
        Opcode::MapNew,
        Opcode::MapGet,
        Opcode::MapContains,
        Opcode::MapInsert,
        Opcode::MapRemove,
        Opcode::CellNew,
        Opcode::CellGet,
        Opcode::CellSet,
        Opcode::ValueHash,
    ];
    let dispatch_blocks = immediate_free_opcodes
        .iter()
        .map(|_| assembler.block_id())
        .collect::<Vec<_>>();
    let tag_check = assembler.block_id();
    let call_immediate_free = assembler.block_id();
    let wrap_immediate_free = assembler.block_id();
    let call_immediate = assembler.block_id();
    let forward_error = assembler.block_id();
    let immediate_error = assembler.block_id();
    let opcode_tags =
        immediate_free_opcodes.map(|value| assembler.constant(u32_value(u128::from(value.tag()))));
    let none_tag = assembler.constant(u32_value(u128::from(Immediate::None.tag())));
    let none_bytes = assembler.constant(bytes_value(&encoded_immediate(&Immediate::None)));
    let zero_u64 = assembler.constant(u64_value(0));
    let immediate_error_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ImmediateMismatch.numeric(),
    )));

    for (index, block) in dispatch_blocks.iter().copied().enumerate() {
        let tag = assembler.constant_ref(block, opcode_tags[index], u32_type());
        let matches = assembler.operation(
            block,
            Opcode::Equal,
            vec![ValueRef::Parameter(opcode), operation_value(tag)],
            TypeExpr::Bool,
            Immediate::None,
        );
        let unmatched = dispatch_blocks
            .get(index + 1)
            .copied()
            .unwrap_or(call_immediate);
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![tag, matches],
            inventory_cond(
                operation_value(matches),
                tag_check,
                Vec::new(),
                unmatched,
                Vec::new(),
            ),
        );
    }

    let expected_none = assembler.constant_ref(tag_check, none_tag, u32_type());
    let tag_matches = assembler.operation(
        tag_check,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(immediate_tag),
            operation_value(expected_none),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    let zero_primary = assembler.constant_ref(tag_check, zero_u64, u64_type());
    let primary_matches = assembler.operation(
        tag_check,
        Opcode::Equal,
        vec![ValueRef::Parameter(primary), operation_value(zero_primary)],
        TypeExpr::Bool,
        Immediate::None,
    );
    let zero_secondary = assembler.constant_ref(tag_check, zero_u64, u64_type());
    let secondary_matches = assembler.operation(
        tag_check,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(secondary),
            operation_value(zero_secondary),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    let tag_and_primary = assembler.operation(
        tag_check,
        Opcode::BoolAnd,
        vec![
            operation_value(tag_matches),
            operation_value(primary_matches),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    let immediate_is_none = assembler.operation(
        tag_check,
        Opcode::BoolAnd,
        vec![
            operation_value(tag_and_primary),
            operation_value(secondary_matches),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    let expected_none_bytes = assembler.constant_ref(tag_check, none_bytes, TypeExpr::Bytes);
    let bytes_match = assembler.operation(
        tag_check,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(immediate_bytes),
            operation_value(expected_none_bytes),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    let complete_none = assembler.operation(
        tag_check,
        Opcode::BoolAnd,
        vec![
            operation_value(immediate_is_none),
            operation_value(bytes_match),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        tag_check,
        function,
        Vec::new(),
        vec![
            expected_none,
            tag_matches,
            zero_primary,
            primary_matches,
            zero_secondary,
            secondary_matches,
            tag_and_primary,
            immediate_is_none,
            expected_none_bytes,
            bytes_match,
            complete_none,
        ],
        inventory_cond(
            operation_value(complete_none),
            call_immediate_free,
            Vec::new(),
            immediate_error,
            Vec::new(),
        ),
    );

    let lowered_free = assembler.operation(
        call_immediate_free,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(opcode),
            ValueRef::Parameter(operands),
            ValueRef::Parameter(next_register),
        ],
        variadic_result_type(),
        Immediate::Function(FunctionRefValue {
            function: immediate_free_entry,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        call_immediate_free,
        function,
        Vec::new(),
        vec![lowered_free],
        inventory_switch(
            operation_value(lowered_free),
            vec![
                (
                    BuiltinCase::Ok,
                    wrap_immediate_free,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let free_summary = assembler.parameter(
        wrap_immediate_free,
        ParameterRole::Block,
        0,
        variadic_summary_type(),
    );
    let compact_instruction = assembler.operation(
        wrap_immediate_free,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(free_summary)],
        single_lowered_type(),
        Immediate::Index(0),
    );
    let free_frontier = assembler.operation(
        wrap_immediate_free,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(free_summary)],
        u32_type(),
        Immediate::Index(1),
    );
    let compact_opcode = assembler.operation(
        wrap_immediate_free,
        Opcode::TupleGet,
        vec![operation_value(compact_instruction)],
        u32_type(),
        Immediate::Index(0),
    );
    let compact_operands = assembler.operation(
        wrap_immediate_free,
        Opcode::TupleGet,
        vec![operation_value(compact_instruction)],
        u32vec_type(),
        Immediate::Index(1),
    );
    let compact_results = assembler.operation(
        wrap_immediate_free,
        Opcode::TupleGet,
        vec![operation_value(compact_instruction)],
        u32vec_type(),
        Immediate::Index(2),
    );
    let full_instruction = assembler.operation(
        wrap_immediate_free,
        Opcode::TupleNew,
        vec![
            operation_value(compact_opcode),
            operation_value(compact_operands),
            operation_value(compact_results),
            ValueRef::Parameter(immediate_tag),
            ValueRef::Parameter(primary),
            ValueRef::Parameter(secondary),
            ValueRef::Parameter(immediate_bytes),
        ],
        immediate_instruction_type(),
        Immediate::None,
    );
    let full_summary = assembler.operation(
        wrap_immediate_free,
        Opcode::TupleNew,
        vec![
            operation_value(full_instruction),
            operation_value(free_frontier),
        ],
        immediate_summary_type(),
        Immediate::None,
    );
    let free_success = assembler.operation(
        wrap_immediate_free,
        Opcode::ResultOk,
        vec![operation_value(full_summary)],
        immediate_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        wrap_immediate_free,
        function,
        vec![free_summary],
        vec![
            compact_instruction,
            free_frontier,
            compact_opcode,
            compact_operands,
            compact_results,
            full_instruction,
            full_summary,
            free_success,
        ],
        Terminator::Return(ReturnTerminator {
            value: operation_value(free_success),
        }),
    );

    let lowered_immediate = assembler.operation(
        call_immediate,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(opcode),
            ValueRef::Parameter(operands),
            ValueRef::Parameter(immediate_tag),
            ValueRef::Parameter(primary),
            ValueRef::Parameter(secondary),
            ValueRef::Parameter(immediate_bytes),
            ValueRef::Parameter(next_register),
        ],
        immediate_result_type(),
        Immediate::Function(FunctionRefValue {
            function: immediate_entry,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        call_immediate,
        function,
        Vec::new(),
        vec![lowered_immediate],
        Terminator::Return(ReturnTerminator {
            value: operation_value(lowered_immediate),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        immediate_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let immediate_code = assembler.constant_ref(immediate_error, immediate_error_code, u32_type());
    let immediate_failure = assembler.operation(
        immediate_error,
        Opcode::ResultErr,
        vec![operation_value(immediate_code)],
        immediate_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        immediate_error,
        function,
        Vec::new(),
        vec![immediate_code, immediate_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(immediate_failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            opcode,
            operands,
            immediate_tag,
            primary,
            secondary,
            immediate_bytes,
            next_register,
        ],
        result_type: immediate_result_type(),
        effects: Vec::new(),
        entry_block: dispatch_blocks[0],
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let mut functions = base.functions;
    functions.push(graph);
    LowerScaffold {
        types: base.types,
        entry: base.entry,
        functions,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: base.adapters,
    }
}

/// Builds one simple bytecode-block model from an ordered mixed-operation
/// inventory and a return/branch/conditional/trap terminator fact set.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn simple_block_lowerer() -> LowerScaffold {
    let mut base = mixed_operation_inventory_lowerer();
    let operation_lowerer = base.entry.entity_id;
    let terminator = rebase_scaffold_artifacts(simple_terminator_lowerer(), 32);
    let terminator_lowerer = terminator.entry.entity_id;
    base.functions.extend(terminator.functions);
    base.parameters.extend(terminator.parameters);
    base.blocks.extend(terminator.blocks);
    base.operations.extend(terminator.operations);
    base.constants.extend(terminator.constants);
    assert!(terminator.adapters.is_empty());

    let mut assembler = InventoryAssembler {
        next_block: u16::try_from(base.blocks.len() + 1).expect("fixture block count fits u16"),
        next_parameter: u16::try_from(base.parameters.len() + 1)
            .expect("fixture parameter count fits u16"),
        next_operation: u16::try_from(base.operations.len() + 1)
            .expect("fixture operation count fits u16"),
        next_constant: u16::try_from(base.constants.len() + 1)
            .expect("fixture constant count fits u16"),
        parameters: base.parameters,
        blocks: base.blocks,
        operations: base.operations,
        constants: base.constants,
    };
    let function = inventory_id(5, 12);
    let inventory = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        immediate_inventory_type(),
    );
    let first_register = assembler.parameter(function, ParameterRole::Function, 1, u32_type());
    let slot = assembler.parameter(function, ParameterRole::Function, 2, u32_type());
    let parameter_registers =
        assembler.parameter(function, ParameterRole::Function, 3, u32vec_type());
    let kind = assembler.parameter(function, ParameterRole::Function, 4, u32_type());
    let primary = assembler.parameter(function, ParameterRole::Function, 5, u32_type());
    let target_zero = assembler.parameter(function, ParameterRole::Function, 6, u32_type());
    let arguments_zero = assembler.parameter(function, ParameterRole::Function, 7, u32vec_type());
    let target_one = assembler.parameter(function, ParameterRole::Function, 8, u32_type());
    let arguments_one = assembler.parameter(function, ParameterRole::Function, 9, u32vec_type());
    let payload = assembler.parameter(function, ParameterRole::Function, 10, optional_u32_type());
    let block_count = assembler.parameter(function, ParameterRole::Function, 11, u32_type());
    let reachability = assembler.parameter(function, ParameterRole::Function, 12, u32_type());

    let entry = assembler.block_id();
    let lower_terminator = assembler.block_id();
    let emit = assembler.block_id();
    let forward_error = assembler.block_id();

    let lowered_operations = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(inventory),
            ValueRef::Parameter(first_register),
        ],
        immediate_inventory_result_type(),
        Immediate::Function(FunctionRefValue {
            function: operation_lowerer,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![lowered_operations],
        inventory_switch(
            operation_value(lowered_operations),
            vec![
                (
                    BuiltinCase::Ok,
                    lower_terminator,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let operation_summary = assembler.parameter(
        lower_terminator,
        ParameterRole::Block,
        0,
        immediate_inventory_summary_type(),
    );
    let instruction_model = assembler.operation(
        lower_terminator,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(operation_summary)],
        immediate_inventory_model_type(),
        Immediate::Index(0),
    );
    let register_frontier = assembler.operation(
        lower_terminator,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(operation_summary)],
        u32_type(),
        Immediate::Index(2),
    );
    let lowered_terminator = assembler.operation(
        lower_terminator,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(kind),
            ValueRef::Parameter(primary),
            ValueRef::Parameter(target_zero),
            ValueRef::Parameter(arguments_zero),
            ValueRef::Parameter(target_one),
            ValueRef::Parameter(arguments_one),
            ValueRef::Parameter(payload),
            operation_value(register_frontier),
            ValueRef::Parameter(block_count),
        ],
        terminator_result_type(),
        Immediate::Function(FunctionRefValue {
            function: terminator_lowerer,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        lower_terminator,
        function,
        vec![operation_summary],
        vec![instruction_model, register_frontier, lowered_terminator],
        inventory_switch(
            operation_value(lowered_terminator),
            vec![
                (
                    BuiltinCase::Ok,
                    emit,
                    vec![
                        SwitchArgument::Value(operation_value(instruction_model)),
                        SwitchArgument::Value(operation_value(register_frontier)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let emit_instructions = assembler.parameter(
        emit,
        ParameterRole::Block,
        0,
        immediate_inventory_model_type(),
    );
    let emit_frontier = assembler.parameter(emit, ParameterRole::Block, 1, u32_type());
    let emit_terminator =
        assembler.parameter(emit, ParameterRole::Block, 2, terminator_model_type());
    let block = assembler.operation(
        emit,
        Opcode::TupleNew,
        vec![
            ValueRef::Parameter(slot),
            ValueRef::Parameter(parameter_registers),
            ValueRef::Parameter(emit_instructions),
            ValueRef::Parameter(emit_terminator),
            ValueRef::Parameter(reachability),
        ],
        simple_block_model_type(),
        Immediate::None,
    );
    let summary = assembler.operation(
        emit,
        Opcode::TupleNew,
        vec![operation_value(block), ValueRef::Parameter(emit_frontier)],
        simple_block_summary_type(),
        Immediate::None,
    );
    let success = assembler.operation(
        emit,
        Opcode::ResultOk,
        vec![operation_value(summary)],
        simple_block_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        emit,
        function,
        vec![emit_instructions, emit_frontier, emit_terminator],
        vec![block, summary, success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        simple_block_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            inventory,
            first_register,
            slot,
            parameter_registers,
            kind,
            primary,
            target_zero,
            arguments_zero,
            target_one,
            arguments_one,
            payload,
            block_count,
            reachability,
        ],
        result_type: simple_block_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let mut functions = base.functions;
    functions.push(graph.clone());
    LowerScaffold {
        types: base.types,
        entry: graph,
        functions,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: base.adapters,
    }
}

/// Builds one block model with any frozen terminator kind. Simple controls
/// and built-in variant switches are normalized into one closed tuple after
/// their dedicated Sley validators accept.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn complete_block_lowerer() -> LowerScaffold {
    let replaced_block_entry = inventory_id(5, 12);
    let mut base = remove_scaffold_function(simple_block_lowerer(), replaced_block_entry);
    let operation_lowerer = inventory_id(5, 10);
    let simple_terminator = inventory_id(5, 3);
    let old_switch = inventory_id(5, 5);
    let old_switch_validator = inventory_id(5, 6);
    let switch_terminator = inventory_id(5, 13);
    let switch_validator = inventory_id(5, 14);
    let variant = rebase_scaffold_functions(
        rebase_scaffold_artifacts(builtin_variant_switch_lowerer(), 48),
        &[
            (old_switch, switch_terminator),
            (old_switch_validator, switch_validator),
        ],
    );
    base.functions.extend(variant.functions);
    base.parameters.extend(variant.parameters);
    base.blocks.extend(variant.blocks);
    base.operations.extend(variant.operations);
    base.constants.extend(variant.constants);
    assert!(variant.adapters.is_empty());

    let mut assembler = InventoryAssembler {
        next_block: u16::try_from(base.blocks.len() + 1).expect("fixture block count fits u16"),
        next_parameter: u16::try_from(base.parameters.len() + 1)
            .expect("fixture parameter count fits u16"),
        next_operation: u16::try_from(base.operations.len() + 1)
            .expect("fixture operation count fits u16"),
        next_constant: u16::try_from(base.constants.len() + 1)
            .expect("fixture constant count fits u16"),
        parameters: base.parameters,
        blocks: base.blocks,
        operations: base.operations,
        constants: base.constants,
    };
    let function = inventory_id(5, 15);
    let inventory = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        immediate_inventory_type(),
    );
    let first_register = assembler.parameter(function, ParameterRole::Function, 1, u32_type());
    let slot = assembler.parameter(function, ParameterRole::Function, 2, u32_type());
    let parameter_registers =
        assembler.parameter(function, ParameterRole::Function, 3, u32vec_type());
    let kind = assembler.parameter(function, ParameterRole::Function, 4, u32_type());
    let primary = assembler.parameter(function, ParameterRole::Function, 5, u32_type());
    let target_zero = assembler.parameter(function, ParameterRole::Function, 6, u32_type());
    let arguments_zero = assembler.parameter(function, ParameterRole::Function, 7, u32vec_type());
    let target_one = assembler.parameter(function, ParameterRole::Function, 8, u32_type());
    let arguments_one = assembler.parameter(function, ParameterRole::Function, 9, u32vec_type());
    let payload = assembler.parameter(function, ParameterRole::Function, 10, optional_u32_type());
    let block_count = assembler.parameter(function, ParameterRole::Function, 11, u32_type());
    let reachability = assembler.parameter(function, ParameterRole::Function, 12, u32_type());
    let switch_cases = assembler.parameter(
        function,
        ParameterRole::Function,
        13,
        builtin_switch_case_facts_type(),
    );

    let entry = assembler.block_id();
    let dispatch = assembler.block_id();
    let call_simple = assembler.block_id();
    let normalize_simple = assembler.block_id();
    let call_switch = assembler.block_id();
    let normalize_switch = assembler.block_id();
    let emit = assembler.block_id();
    let forward_error = assembler.block_id();
    let switch_kind = assembler.constant(u32_value(4));
    let zero_u32 = assembler.constant(u32_value(0));

    let lowered_operations = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(inventory),
            ValueRef::Parameter(first_register),
        ],
        immediate_inventory_result_type(),
        Immediate::Function(FunctionRefValue {
            function: operation_lowerer,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![lowered_operations],
        inventory_switch(
            operation_value(lowered_operations),
            vec![
                (BuiltinCase::Ok, dispatch, vec![SwitchArgument::CasePayload]),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let operation_summary = assembler.parameter(
        dispatch,
        ParameterRole::Block,
        0,
        immediate_inventory_summary_type(),
    );
    let instruction_model = assembler.operation(
        dispatch,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(operation_summary)],
        immediate_inventory_model_type(),
        Immediate::Index(0),
    );
    let register_frontier = assembler.operation(
        dispatch,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(operation_summary)],
        u32_type(),
        Immediate::Index(2),
    );
    let switch_tag = assembler.constant_ref(dispatch, switch_kind, u32_type());
    let is_switch = assembler.operation(
        dispatch,
        Opcode::Equal,
        vec![ValueRef::Parameter(kind), operation_value(switch_tag)],
        TypeExpr::Bool,
        Immediate::None,
    );
    let lowered_arguments = vec![
        operation_value(instruction_model),
        operation_value(register_frontier),
    ];
    assembler.push_block(
        dispatch,
        function,
        vec![operation_summary],
        vec![instruction_model, register_frontier, switch_tag, is_switch],
        inventory_cond(
            operation_value(is_switch),
            call_switch,
            lowered_arguments.clone(),
            call_simple,
            lowered_arguments,
        ),
    );

    let simple_instructions = assembler.parameter(
        call_simple,
        ParameterRole::Block,
        0,
        immediate_inventory_model_type(),
    );
    let simple_frontier = assembler.parameter(call_simple, ParameterRole::Block, 1, u32_type());
    let lowered_simple = assembler.operation(
        call_simple,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(kind),
            ValueRef::Parameter(primary),
            ValueRef::Parameter(target_zero),
            ValueRef::Parameter(arguments_zero),
            ValueRef::Parameter(target_one),
            ValueRef::Parameter(arguments_one),
            ValueRef::Parameter(payload),
            ValueRef::Parameter(simple_frontier),
            ValueRef::Parameter(block_count),
        ],
        terminator_result_type(),
        Immediate::Function(FunctionRefValue {
            function: simple_terminator,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        call_simple,
        function,
        vec![simple_instructions, simple_frontier],
        vec![lowered_simple],
        inventory_switch(
            operation_value(lowered_simple),
            vec![
                (
                    BuiltinCase::Ok,
                    normalize_simple,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(simple_instructions)),
                        SwitchArgument::Value(ValueRef::Parameter(simple_frontier)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let normalized_simple_instructions = assembler.parameter(
        normalize_simple,
        ParameterRole::Block,
        0,
        immediate_inventory_model_type(),
    );
    let normalized_simple_frontier =
        assembler.parameter(normalize_simple, ParameterRole::Block, 1, u32_type());
    let simple_model = assembler.parameter(
        normalize_simple,
        ParameterRole::Block,
        2,
        terminator_model_type(),
    );
    let empty_cases = assembler.operation(
        normalize_simple,
        Opcode::VectorNew,
        Vec::new(),
        builtin_switch_case_facts_type(),
        Immediate::None,
    );
    let zero_selector = assembler.constant_ref(normalize_simple, zero_u32, u32_type());
    let empty_switch = assembler.operation(
        normalize_simple,
        Opcode::TupleNew,
        vec![operation_value(zero_selector), operation_value(empty_cases)],
        builtin_switch_model_type(),
        Immediate::None,
    );
    let complete_simple = assembler.operation(
        normalize_simple,
        Opcode::TupleNew,
        vec![
            ValueRef::Parameter(kind),
            ValueRef::Parameter(simple_model),
            operation_value(empty_switch),
        ],
        complete_terminator_model_type(),
        Immediate::None,
    );
    assembler.push_block(
        normalize_simple,
        function,
        vec![
            normalized_simple_instructions,
            normalized_simple_frontier,
            simple_model,
        ],
        vec![empty_cases, zero_selector, empty_switch, complete_simple],
        inventory_branch(
            emit,
            vec![
                ValueRef::Parameter(normalized_simple_instructions),
                ValueRef::Parameter(normalized_simple_frontier),
                operation_value(complete_simple),
            ],
        ),
    );

    let switch_instructions = assembler.parameter(
        call_switch,
        ParameterRole::Block,
        0,
        immediate_inventory_model_type(),
    );
    let switch_frontier = assembler.parameter(call_switch, ParameterRole::Block, 1, u32_type());
    let lowered_switch = assembler.operation(
        call_switch,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(primary),
            ValueRef::Parameter(switch_cases),
            ValueRef::Parameter(switch_frontier),
            ValueRef::Parameter(block_count),
        ],
        builtin_switch_result_type(),
        Immediate::Function(FunctionRefValue {
            function: switch_terminator,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        call_switch,
        function,
        vec![switch_instructions, switch_frontier],
        vec![lowered_switch],
        inventory_switch(
            operation_value(lowered_switch),
            vec![
                (
                    BuiltinCase::Ok,
                    normalize_switch,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(switch_instructions)),
                        SwitchArgument::Value(ValueRef::Parameter(switch_frontier)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let normalized_switch_instructions = assembler.parameter(
        normalize_switch,
        ParameterRole::Block,
        0,
        immediate_inventory_model_type(),
    );
    let normalized_switch_frontier =
        assembler.parameter(normalize_switch, ParameterRole::Block, 1, u32_type());
    let switch_model = assembler.parameter(
        normalize_switch,
        ParameterRole::Block,
        2,
        builtin_switch_model_type(),
    );
    let zero = assembler.constant_ref(normalize_switch, zero_u32, u32_type());
    let empty_registers = assembler.operation(
        normalize_switch,
        Opcode::VectorNew,
        Vec::new(),
        u32vec_type(),
        Immediate::None,
    );
    let no_payload = assembler.operation(
        normalize_switch,
        Opcode::OptionNone,
        Vec::new(),
        optional_u32_type(),
        Immediate::None,
    );
    let empty_simple = assembler.operation(
        normalize_switch,
        Opcode::TupleNew,
        vec![
            operation_value(zero),
            operation_value(zero),
            operation_value(zero),
            operation_value(empty_registers),
            operation_value(zero),
            operation_value(empty_registers),
            operation_value(no_payload),
        ],
        terminator_model_type(),
        Immediate::None,
    );
    let complete_switch = assembler.operation(
        normalize_switch,
        Opcode::TupleNew,
        vec![
            ValueRef::Parameter(kind),
            operation_value(empty_simple),
            ValueRef::Parameter(switch_model),
        ],
        complete_terminator_model_type(),
        Immediate::None,
    );
    assembler.push_block(
        normalize_switch,
        function,
        vec![
            normalized_switch_instructions,
            normalized_switch_frontier,
            switch_model,
        ],
        vec![
            zero,
            empty_registers,
            no_payload,
            empty_simple,
            complete_switch,
        ],
        inventory_branch(
            emit,
            vec![
                ValueRef::Parameter(normalized_switch_instructions),
                ValueRef::Parameter(normalized_switch_frontier),
                operation_value(complete_switch),
            ],
        ),
    );

    let emit_instructions = assembler.parameter(
        emit,
        ParameterRole::Block,
        0,
        immediate_inventory_model_type(),
    );
    let emit_frontier = assembler.parameter(emit, ParameterRole::Block, 1, u32_type());
    let emit_terminator = assembler.parameter(
        emit,
        ParameterRole::Block,
        2,
        complete_terminator_model_type(),
    );
    let block = assembler.operation(
        emit,
        Opcode::TupleNew,
        vec![
            ValueRef::Parameter(slot),
            ValueRef::Parameter(parameter_registers),
            ValueRef::Parameter(emit_instructions),
            ValueRef::Parameter(emit_terminator),
            ValueRef::Parameter(reachability),
        ],
        complete_block_model_type(),
        Immediate::None,
    );
    let summary = assembler.operation(
        emit,
        Opcode::TupleNew,
        vec![operation_value(block), ValueRef::Parameter(emit_frontier)],
        complete_block_summary_type(),
        Immediate::None,
    );
    let success = assembler.operation(
        emit,
        Opcode::ResultOk,
        vec![operation_value(summary)],
        complete_block_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        emit,
        function,
        vec![emit_instructions, emit_frontier, emit_terminator],
        vec![block, summary, success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        complete_block_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            inventory,
            first_register,
            slot,
            parameter_registers,
            kind,
            primary,
            target_zero,
            arguments_zero,
            target_one,
            arguments_one,
            payload,
            block_count,
            reachability,
            switch_cases,
        ],
        result_type: complete_block_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let mut functions = base.functions;
    functions.push(graph.clone());
    LowerScaffold {
        types: base.types,
        entry: graph,
        functions,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: base.adapters,
    }
}

#[allow(clippy::too_many_lines)]
fn build_dense_register_validator(
    assembler: &mut InventoryAssembler,
    function: EntityId,
) -> FunctionGraph {
    let values = assembler.parameter(function, ParameterRole::Function, 0, u32vec_type());
    let first_register = assembler.parameter(function, ParameterRole::Function, 1, u32_type());

    let entry = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let compare = assembler.block_id();
    let advance_index = assembler.block_id();
    let advance_register = assembler.block_id();
    let done = assembler.block_id();
    let local_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();

    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let one_u32 = assembler.constant(u32_value(1));
    let local_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    )));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let zero = assembler.constant_ref(entry, zero_u64, u64_type());
    let length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(values)],
        u64_type(),
        Immediate::None,
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![zero, length],
        inventory_branch(
            check,
            vec![
                operation_value(zero),
                ValueRef::Parameter(first_register),
                operation_value(length),
            ],
        ),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let check_register = assembler.parameter(check, ParameterRole::Block, 1, u32_type());
    let check_length = assembler.parameter(check, ParameterRole::Block, 2, u64_type());
    let has_value = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_index),
            ValueRef::Parameter(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    let check_state = vec![
        ValueRef::Parameter(check_index),
        ValueRef::Parameter(check_register),
        ValueRef::Parameter(check_length),
    ];
    assembler.push_block(
        check,
        function,
        vec![check_index, check_register, check_length],
        vec![has_value],
        inventory_cond(
            operation_value(has_value),
            get,
            check_state,
            done,
            vec![ValueRef::Parameter(check_register)],
        ),
    );

    let get_index = assembler.parameter(get, ParameterRole::Block, 0, u64_type());
    let get_register = assembler.parameter(get, ParameterRole::Block, 1, u32_type());
    let get_length = assembler.parameter(get, ParameterRole::Block, 2, u64_type());
    let found = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![ValueRef::Parameter(values), ValueRef::Parameter(get_index)],
        optional_u32_type(),
        Immediate::None,
    );
    assembler.push_block(
        get,
        function,
        vec![get_index, get_register, get_length],
        vec![found],
        inventory_switch(
            operation_value(found),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    compare,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                        SwitchArgument::Value(ValueRef::Parameter(get_register)),
                        SwitchArgument::Value(ValueRef::Parameter(get_length)),
                    ],
                ),
            ],
        ),
    );

    let compare_value = assembler.parameter(compare, ParameterRole::Block, 0, u32_type());
    let compare_index = assembler.parameter(compare, ParameterRole::Block, 1, u64_type());
    let compare_register = assembler.parameter(compare, ParameterRole::Block, 2, u32_type());
    let compare_length = assembler.parameter(compare, ParameterRole::Block, 3, u64_type());
    let matches = assembler.operation(
        compare,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(compare_value),
            ValueRef::Parameter(compare_register),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        compare,
        function,
        vec![
            compare_value,
            compare_index,
            compare_register,
            compare_length,
        ],
        vec![matches],
        inventory_cond(
            operation_value(matches),
            advance_index,
            vec![
                ValueRef::Parameter(compare_index),
                ValueRef::Parameter(compare_register),
                ValueRef::Parameter(compare_length),
            ],
            local_error,
            Vec::new(),
        ),
    );

    let advance_index_value =
        assembler.parameter(advance_index, ParameterRole::Block, 0, u64_type());
    let advance_index_register =
        assembler.parameter(advance_index, ParameterRole::Block, 1, u32_type());
    let advance_index_length =
        assembler.parameter(advance_index, ParameterRole::Block, 2, u64_type());
    let one_index = assembler.constant_ref(advance_index, one_u64, u64_type());
    let next_index = assembler.operation(
        advance_index,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_index_value),
            operation_value(one_index),
        ],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance_index,
        function,
        vec![
            advance_index_value,
            advance_index_register,
            advance_index_length,
        ],
        vec![one_index, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    advance_register,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_index_register)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_index_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let advance_register_index =
        assembler.parameter(advance_register, ParameterRole::Block, 0, u64_type());
    let advance_register_value =
        assembler.parameter(advance_register, ParameterRole::Block, 1, u32_type());
    let advance_register_length =
        assembler.parameter(advance_register, ParameterRole::Block, 2, u64_type());
    let one_register = assembler.constant_ref(advance_register, one_u32, u32_type());
    let next_register = assembler.operation(
        advance_register,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_register_value),
            operation_value(one_register),
        ],
        arithmetic_result_type(u32_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance_register,
        function,
        vec![
            advance_register_index,
            advance_register_value,
            advance_register_length,
        ],
        vec![one_register, next_register],
        inventory_switch(
            operation_value(next_register),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(advance_register_index)),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_register_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let done_register = assembler.parameter(done, ParameterRole::Block, 0, u32_type());
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(done_register)],
        dense_register_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![done_register],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    for (block, code) in [(local_error, local_code), (resource_error, resource_code)] {
        let value = assembler.constant_ref(block, code, u32_type());
        let failure = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![operation_value(value)],
            dense_register_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![value, failure],
            Terminator::Return(ReturnTerminator {
                value: operation_value(failure),
            }),
        );
    }
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![values, first_register],
        result_type: dense_register_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

#[allow(clippy::too_many_lines)]
fn build_bytes_vector_count_validator(
    assembler: &mut InventoryAssembler,
    function: EntityId,
) -> FunctionGraph {
    let values = assembler.parameter(function, ParameterRole::Function, 0, bytesvec_type());
    let expected = assembler.parameter(function, ParameterRole::Function, 1, u32_type());
    let entry = assembler.block_id();
    let check = assembler.block_id();
    let advance_index = assembler.block_id();
    let advance_count = assembler.block_id();
    let done = assembler.block_id();
    let success = assembler.block_id();
    let local_error = assembler.block_id();
    let resource_error = assembler.block_id();

    let zero_u64 = assembler.constant(u64_value(0));
    let zero_u32 = assembler.constant(u32_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let one_u32 = assembler.constant(u32_value(1));
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let local_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    )));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let index = assembler.constant_ref(entry, zero_u64, u64_type());
    let count = assembler.constant_ref(entry, zero_u32, u32_type());
    let length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(values)],
        u64_type(),
        Immediate::None,
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![index, count, length],
        inventory_branch(
            check,
            vec![
                operation_value(index),
                operation_value(count),
                operation_value(length),
            ],
        ),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let check_count = assembler.parameter(check, ParameterRole::Block, 1, u32_type());
    let check_length = assembler.parameter(check, ParameterRole::Block, 2, u64_type());
    let has_value = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_index),
            ValueRef::Parameter(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        vec![check_index, check_count, check_length],
        vec![has_value],
        inventory_cond(
            operation_value(has_value),
            advance_index,
            vec![
                ValueRef::Parameter(check_index),
                ValueRef::Parameter(check_count),
                ValueRef::Parameter(check_length),
            ],
            done,
            vec![ValueRef::Parameter(check_count)],
        ),
    );

    let advance_index_value =
        assembler.parameter(advance_index, ParameterRole::Block, 0, u64_type());
    let advance_index_count =
        assembler.parameter(advance_index, ParameterRole::Block, 1, u32_type());
    let advance_index_length =
        assembler.parameter(advance_index, ParameterRole::Block, 2, u64_type());
    let index_one = assembler.constant_ref(advance_index, one_u64, u64_type());
    let next_index = assembler.operation(
        advance_index,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_index_value),
            operation_value(index_one),
        ],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance_index,
        function,
        vec![
            advance_index_value,
            advance_index_count,
            advance_index_length,
        ],
        vec![index_one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    advance_count,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_index_count)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_index_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let advance_count_index =
        assembler.parameter(advance_count, ParameterRole::Block, 0, u64_type());
    let advance_count_value =
        assembler.parameter(advance_count, ParameterRole::Block, 1, u32_type());
    let advance_count_length =
        assembler.parameter(advance_count, ParameterRole::Block, 2, u64_type());
    let count_one = assembler.constant_ref(advance_count, one_u32, u32_type());
    let next_count = assembler.operation(
        advance_count,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_count_value),
            operation_value(count_one),
        ],
        arithmetic_result_type(u32_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance_count,
        function,
        vec![
            advance_count_index,
            advance_count_value,
            advance_count_length,
        ],
        vec![count_one, next_count],
        inventory_switch(
            operation_value(next_count),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(advance_count_index)),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_count_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let done_count = assembler.parameter(done, ParameterRole::Block, 0, u32_type());
    let matches = assembler.operation(
        done,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(done_count),
            ValueRef::Parameter(expected),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![done_count],
        vec![matches],
        inventory_cond(
            operation_value(matches),
            success,
            Vec::new(),
            local_error,
            Vec::new(),
        ),
    );

    let unit_value = assembler.constant_ref(success, unit, TypeExpr::Unit);
    let accepted = assembler.operation(
        success,
        Opcode::ResultOk,
        vec![operation_value(unit_value)],
        unit_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success,
        function,
        Vec::new(),
        vec![unit_value, accepted],
        Terminator::Return(ReturnTerminator {
            value: operation_value(accepted),
        }),
    );
    for (block, code) in [(local_error, local_code), (resource_error, resource_code)] {
        let value = assembler.constant_ref(block, code, u32_type());
        let failure = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![operation_value(value)],
            unit_lower_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![value, failure],
            Terminator::Return(ReturnTerminator {
                value: operation_value(failure),
            }),
        );
    }
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![values, expected],
        result_type: unit_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

/// Walks semantic blocks in order, validates the native dense-register
/// allocation schedule, and accumulates complete lowered block models.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn complete_function_lowerer() -> LowerScaffold {
    let base = complete_block_lowerer();
    let lower_block = base.entry.entity_id;
    let dense_validator = inventory_id(5, 16);
    let function = inventory_id(5, 17);
    let type_count_validator = inventory_id(5, 18);
    let mut assembler = InventoryAssembler {
        next_block: u16::try_from(base.blocks.len() + 1).expect("fixture block count fits u16"),
        next_parameter: u16::try_from(base.parameters.len() + 1)
            .expect("fixture parameter count fits u16"),
        next_operation: u16::try_from(base.operations.len() + 1)
            .expect("fixture operation count fits u16"),
        next_constant: u16::try_from(base.constants.len() + 1)
            .expect("fixture constant count fits u16"),
        parameters: base.parameters,
        blocks: base.blocks,
        operations: base.operations,
        constants: base.constants,
    };
    let dense_graph = build_dense_register_validator(&mut assembler, dense_validator);
    let type_count_graph = build_bytes_vector_count_validator(&mut assembler, type_count_validator);

    let function_identity =
        assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bytes);
    let function_parameters =
        assembler.parameter(function, ParameterRole::Function, 1, u32vec_type());
    let register_types = assembler.parameter(function, ParameterRole::Function, 2, bytesvec_type());
    let result_type = assembler.parameter(function, ParameterRole::Function, 3, TypeExpr::Bytes);
    let entry_slot = assembler.parameter(function, ParameterRole::Function, 4, u32_type());
    let block_count = assembler.parameter(function, ParameterRole::Function, 5, u32_type());
    let block_facts = assembler.parameter(
        function,
        ParameterRole::Function,
        6,
        complete_block_facts_type(),
    );

    let entry = assembler.block_id();
    let identity_check = assembler.block_id();
    let validate_function_parameters_block = assembler.block_id();
    let initialize = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let unpack = assembler.block_id();
    let validate_parameters = assembler.block_id();
    let call_block = assembler.block_id();
    let accept_block = assembler.block_id();
    let advance_index = assembler.block_id();
    let advance_slot = assembler.block_id();
    let verify_count = assembler.block_id();
    let verify_entry = assembler.block_id();
    let validate_types = assembler.block_id();
    let emit = assembler.block_id();
    let forward_error = assembler.block_id();
    let local_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();

    let zero_u32 = assembler.constant(u32_value(0));
    let zero_u64 = assembler.constant(u64_value(0));
    let one_u32 = assembler.constant(u32_value(1));
    let one_u64 = assembler.constant(u64_value(1));
    let identity_length = assembler.constant(u64_value(32));
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let local_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    )));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let identity_scope = assembler.constant_ref(entry, unit, TypeExpr::Unit);
    let identity_octets = assembler.operation(
        entry,
        Opcode::AdapterInvoke,
        vec![
            operation_value(identity_scope),
            ValueRef::Parameter(function_identity),
        ],
        index_result_type(u8vec_type()),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![identity_scope, identity_octets],
        inventory_switch(
            operation_value(identity_octets),
            vec![
                (
                    BuiltinCase::Ok,
                    identity_check,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let checked_identity =
        assembler.parameter(identity_check, ParameterRole::Block, 0, u8vec_type());
    let found_identity_length = assembler.operation(
        identity_check,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(checked_identity)],
        u64_type(),
        Immediate::None,
    );
    let expected_identity_length =
        assembler.constant_ref(identity_check, identity_length, u64_type());
    let identity_valid = assembler.operation(
        identity_check,
        Opcode::Equal,
        vec![
            operation_value(found_identity_length),
            operation_value(expected_identity_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        identity_check,
        function,
        vec![checked_identity],
        vec![
            found_identity_length,
            expected_identity_length,
            identity_valid,
        ],
        inventory_cond(
            operation_value(identity_valid),
            validate_function_parameters_block,
            Vec::new(),
            local_error,
            Vec::new(),
        ),
    );

    let first = assembler.constant_ref(validate_function_parameters_block, zero_u32, u32_type());
    let validate_function_parameters = assembler.operation(
        validate_function_parameters_block,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(function_parameters),
            operation_value(first),
        ],
        dense_register_result_type(),
        Immediate::Function(FunctionRefValue {
            function: dense_validator,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        validate_function_parameters_block,
        function,
        Vec::new(),
        vec![first, validate_function_parameters],
        inventory_switch(
            operation_value(validate_function_parameters),
            vec![
                (
                    BuiltinCase::Ok,
                    initialize,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let initial_frontier = assembler.parameter(initialize, ParameterRole::Block, 0, u32_type());
    let initial_index = assembler.constant_ref(initialize, zero_u64, u64_type());
    let initial_slot = assembler.constant_ref(initialize, zero_u32, u32_type());
    let length = assembler.operation(
        initialize,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(block_facts)],
        u64_type(),
        Immediate::None,
    );
    let empty_map = assembler.operation(
        initialize,
        Opcode::MapNew,
        Vec::new(),
        empty_complete_block_map_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        initialize,
        function,
        vec![initial_frontier],
        vec![initial_index, initial_slot, length, empty_map],
        inventory_switch(
            operation_value(empty_map),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::Value(operation_value(initial_index)),
                        SwitchArgument::Value(operation_value(initial_slot)),
                        SwitchArgument::Value(ValueRef::Parameter(initial_frontier)),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(operation_value(length)),
                    ],
                ),
                (BuiltinCase::Err, invariant_trap, Vec::new()),
            ],
        ),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let check_slot = assembler.parameter(check, ParameterRole::Block, 1, u32_type());
    let check_frontier = assembler.parameter(check, ParameterRole::Block, 2, u32_type());
    let check_map = assembler.parameter(check, ParameterRole::Block, 3, complete_block_map_type());
    let check_length = assembler.parameter(check, ParameterRole::Block, 4, u64_type());
    let has_block = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_index),
            ValueRef::Parameter(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    let check_state = vec![
        ValueRef::Parameter(check_index),
        ValueRef::Parameter(check_slot),
        ValueRef::Parameter(check_frontier),
        ValueRef::Parameter(check_map),
        ValueRef::Parameter(check_length),
    ];
    assembler.push_block(
        check,
        function,
        vec![
            check_index,
            check_slot,
            check_frontier,
            check_map,
            check_length,
        ],
        vec![has_block],
        inventory_cond(
            operation_value(has_block),
            get,
            check_state,
            verify_count,
            vec![
                ValueRef::Parameter(check_slot),
                ValueRef::Parameter(check_map),
                ValueRef::Parameter(check_frontier),
            ],
        ),
    );

    let get_index = assembler.parameter(get, ParameterRole::Block, 0, u64_type());
    let get_slot = assembler.parameter(get, ParameterRole::Block, 1, u32_type());
    let get_frontier = assembler.parameter(get, ParameterRole::Block, 2, u32_type());
    let get_map = assembler.parameter(get, ParameterRole::Block, 3, complete_block_map_type());
    let get_length = assembler.parameter(get, ParameterRole::Block, 4, u64_type());
    let found = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(block_facts),
            ValueRef::Parameter(get_index),
        ],
        TypeExpr::Option(Box::new(complete_block_fact_type())),
        Immediate::None,
    );
    assembler.push_block(
        get,
        function,
        vec![get_index, get_slot, get_frontier, get_map, get_length],
        vec![found],
        inventory_switch(
            operation_value(found),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                        SwitchArgument::Value(ValueRef::Parameter(get_slot)),
                        SwitchArgument::Value(ValueRef::Parameter(get_frontier)),
                        SwitchArgument::Value(ValueRef::Parameter(get_map)),
                        SwitchArgument::Value(ValueRef::Parameter(get_length)),
                    ],
                ),
            ],
        ),
    );

    let unpack_fact =
        assembler.parameter(unpack, ParameterRole::Block, 0, complete_block_fact_type());
    let unpack_index = assembler.parameter(unpack, ParameterRole::Block, 1, u64_type());
    let unpack_slot = assembler.parameter(unpack, ParameterRole::Block, 2, u32_type());
    let unpack_frontier = assembler.parameter(unpack, ParameterRole::Block, 3, u32_type());
    let unpack_map =
        assembler.parameter(unpack, ParameterRole::Block, 4, complete_block_map_type());
    let unpack_length = assembler.parameter(unpack, ParameterRole::Block, 5, u64_type());
    let fact_slot = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(unpack_fact)],
        u32_type(),
        Immediate::Index(0),
    );
    let fact_parameters = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(unpack_fact)],
        u32vec_type(),
        Immediate::Index(1),
    );
    let slot_matches = assembler.operation(
        unpack,
        Opcode::Equal,
        vec![operation_value(fact_slot), ValueRef::Parameter(unpack_slot)],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        unpack,
        function,
        vec![
            unpack_fact,
            unpack_index,
            unpack_slot,
            unpack_frontier,
            unpack_map,
            unpack_length,
        ],
        vec![fact_slot, fact_parameters, slot_matches],
        inventory_cond(
            operation_value(slot_matches),
            validate_parameters,
            vec![
                ValueRef::Parameter(unpack_fact),
                operation_value(fact_parameters),
                ValueRef::Parameter(unpack_index),
                ValueRef::Parameter(unpack_slot),
                ValueRef::Parameter(unpack_frontier),
                ValueRef::Parameter(unpack_map),
                ValueRef::Parameter(unpack_length),
            ],
            local_error,
            Vec::new(),
        ),
    );

    let validate_fact = assembler.parameter(
        validate_parameters,
        ParameterRole::Block,
        0,
        complete_block_fact_type(),
    );
    let validate_registers =
        assembler.parameter(validate_parameters, ParameterRole::Block, 1, u32vec_type());
    let validate_index =
        assembler.parameter(validate_parameters, ParameterRole::Block, 2, u64_type());
    let validate_slot =
        assembler.parameter(validate_parameters, ParameterRole::Block, 3, u32_type());
    let validate_frontier =
        assembler.parameter(validate_parameters, ParameterRole::Block, 4, u32_type());
    let validate_map = assembler.parameter(
        validate_parameters,
        ParameterRole::Block,
        5,
        complete_block_map_type(),
    );
    let validate_length =
        assembler.parameter(validate_parameters, ParameterRole::Block, 6, u64_type());
    let validated = assembler.operation(
        validate_parameters,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(validate_registers),
            ValueRef::Parameter(validate_frontier),
        ],
        dense_register_result_type(),
        Immediate::Function(FunctionRefValue {
            function: dense_validator,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        validate_parameters,
        function,
        vec![
            validate_fact,
            validate_registers,
            validate_index,
            validate_slot,
            validate_frontier,
            validate_map,
            validate_length,
        ],
        vec![validated],
        inventory_switch(
            operation_value(validated),
            vec![
                (
                    BuiltinCase::Ok,
                    call_block,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(validate_fact)),
                        SwitchArgument::Value(ValueRef::Parameter(validate_index)),
                        SwitchArgument::Value(ValueRef::Parameter(validate_slot)),
                        SwitchArgument::Value(ValueRef::Parameter(validate_map)),
                        SwitchArgument::Value(ValueRef::Parameter(validate_length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let call_frontier = assembler.parameter(call_block, ParameterRole::Block, 0, u32_type());
    let call_fact = assembler.parameter(
        call_block,
        ParameterRole::Block,
        1,
        complete_block_fact_type(),
    );
    let call_index = assembler.parameter(call_block, ParameterRole::Block, 2, u64_type());
    let call_slot = assembler.parameter(call_block, ParameterRole::Block, 3, u32_type());
    let call_map = assembler.parameter(
        call_block,
        ParameterRole::Block,
        4,
        complete_block_map_type(),
    );
    let call_length = assembler.parameter(call_block, ParameterRole::Block, 5, u64_type());
    let fact_types = [
        u32_type(),
        u32vec_type(),
        immediate_inventory_type(),
        u32_type(),
        u32_type(),
        u32_type(),
        u32vec_type(),
        u32_type(),
        u32vec_type(),
        optional_u32_type(),
        builtin_switch_case_facts_type(),
        u32_type(),
    ];
    let fields = fact_types
        .into_iter()
        .enumerate()
        .map(|(index, value_type)| {
            assembler.operation(
                call_block,
                Opcode::TupleGet,
                vec![ValueRef::Parameter(call_fact)],
                value_type,
                Immediate::Index(u32::try_from(index).expect("fact field index fits u32")),
            )
        })
        .collect::<Vec<_>>();
    let lowered = assembler.operation(
        call_block,
        Opcode::CallDirect,
        vec![
            operation_value(fields[2]),
            ValueRef::Parameter(call_frontier),
            operation_value(fields[0]),
            operation_value(fields[1]),
            operation_value(fields[3]),
            operation_value(fields[4]),
            operation_value(fields[5]),
            operation_value(fields[6]),
            operation_value(fields[7]),
            operation_value(fields[8]),
            operation_value(fields[9]),
            ValueRef::Parameter(block_count),
            operation_value(fields[11]),
            operation_value(fields[10]),
        ],
        complete_block_result_type(),
        Immediate::Function(FunctionRefValue {
            function: lower_block,
            type_arguments: Vec::new(),
        }),
    );
    let mut call_operations = fields;
    call_operations.push(lowered);
    assembler.push_block(
        call_block,
        function,
        vec![
            call_frontier,
            call_fact,
            call_index,
            call_slot,
            call_map,
            call_length,
        ],
        call_operations,
        inventory_switch(
            operation_value(lowered),
            vec![
                (
                    BuiltinCase::Ok,
                    accept_block,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(call_index)),
                        SwitchArgument::Value(ValueRef::Parameter(call_slot)),
                        SwitchArgument::Value(ValueRef::Parameter(call_map)),
                        SwitchArgument::Value(ValueRef::Parameter(call_length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let accepted = assembler.parameter(
        accept_block,
        ParameterRole::Block,
        0,
        complete_block_summary_type(),
    );
    let accept_index = assembler.parameter(accept_block, ParameterRole::Block, 1, u64_type());
    let accept_slot = assembler.parameter(accept_block, ParameterRole::Block, 2, u32_type());
    let accept_map = assembler.parameter(
        accept_block,
        ParameterRole::Block,
        3,
        complete_block_map_type(),
    );
    let accept_length = assembler.parameter(accept_block, ParameterRole::Block, 4, u64_type());
    let accepted_block = assembler.operation(
        accept_block,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(accepted)],
        complete_block_model_type(),
        Immediate::Index(0),
    );
    let accepted_frontier = assembler.operation(
        accept_block,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(accepted)],
        u32_type(),
        Immediate::Index(1),
    );
    let inserted = assembler.operation(
        accept_block,
        Opcode::MapInsert,
        vec![
            ValueRef::Parameter(accept_map),
            ValueRef::Parameter(accept_slot),
            operation_value(accepted_block),
        ],
        complete_block_map_type(),
        Immediate::None,
    );
    assembler.push_block(
        accept_block,
        function,
        vec![
            accepted,
            accept_index,
            accept_slot,
            accept_map,
            accept_length,
        ],
        vec![accepted_block, accepted_frontier, inserted],
        inventory_branch(
            advance_index,
            vec![
                ValueRef::Parameter(accept_index),
                ValueRef::Parameter(accept_slot),
                operation_value(accepted_frontier),
                operation_value(inserted),
                ValueRef::Parameter(accept_length),
            ],
        ),
    );

    let advance_index_value =
        assembler.parameter(advance_index, ParameterRole::Block, 0, u64_type());
    let advance_index_slot =
        assembler.parameter(advance_index, ParameterRole::Block, 1, u32_type());
    let advance_index_frontier =
        assembler.parameter(advance_index, ParameterRole::Block, 2, u32_type());
    let advance_index_map = assembler.parameter(
        advance_index,
        ParameterRole::Block,
        3,
        complete_block_map_type(),
    );
    let advance_index_length =
        assembler.parameter(advance_index, ParameterRole::Block, 4, u64_type());
    let index_one = assembler.constant_ref(advance_index, one_u64, u64_type());
    let next_index = assembler.operation(
        advance_index,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_index_value),
            operation_value(index_one),
        ],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance_index,
        function,
        vec![
            advance_index_value,
            advance_index_slot,
            advance_index_frontier,
            advance_index_map,
            advance_index_length,
        ],
        vec![index_one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    advance_slot,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_index_slot)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_index_frontier)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_index_map)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_index_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let advance_slot_index = assembler.parameter(advance_slot, ParameterRole::Block, 0, u64_type());
    let advance_slot_value = assembler.parameter(advance_slot, ParameterRole::Block, 1, u32_type());
    let advance_slot_frontier =
        assembler.parameter(advance_slot, ParameterRole::Block, 2, u32_type());
    let advance_slot_map = assembler.parameter(
        advance_slot,
        ParameterRole::Block,
        3,
        complete_block_map_type(),
    );
    let advance_slot_length =
        assembler.parameter(advance_slot, ParameterRole::Block, 4, u64_type());
    let slot_one = assembler.constant_ref(advance_slot, one_u32, u32_type());
    let next_slot = assembler.operation(
        advance_slot,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_slot_value),
            operation_value(slot_one),
        ],
        arithmetic_result_type(u32_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance_slot,
        function,
        vec![
            advance_slot_index,
            advance_slot_value,
            advance_slot_frontier,
            advance_slot_map,
            advance_slot_length,
        ],
        vec![slot_one, next_slot],
        inventory_switch(
            operation_value(next_slot),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(advance_slot_index)),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_slot_frontier)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_slot_map)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_slot_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let count_slot = assembler.parameter(verify_count, ParameterRole::Block, 0, u32_type());
    let count_map = assembler.parameter(
        verify_count,
        ParameterRole::Block,
        1,
        complete_block_map_type(),
    );
    let count_frontier = assembler.parameter(verify_count, ParameterRole::Block, 2, u32_type());
    let count_matches = assembler.operation(
        verify_count,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(count_slot),
            ValueRef::Parameter(block_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        verify_count,
        function,
        vec![count_slot, count_map, count_frontier],
        vec![count_matches],
        inventory_cond(
            operation_value(count_matches),
            verify_entry,
            vec![
                ValueRef::Parameter(count_map),
                ValueRef::Parameter(count_frontier),
            ],
            local_error,
            Vec::new(),
        ),
    );

    let entry_map = assembler.parameter(
        verify_entry,
        ParameterRole::Block,
        0,
        complete_block_map_type(),
    );
    let entry_frontier = assembler.parameter(verify_entry, ParameterRole::Block, 1, u32_type());
    let entry_valid = assembler.operation(
        verify_entry,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(entry_slot),
            ValueRef::Parameter(block_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        verify_entry,
        function,
        vec![entry_map, entry_frontier],
        vec![entry_valid],
        inventory_cond(
            operation_value(entry_valid),
            validate_types,
            vec![
                ValueRef::Parameter(entry_map),
                ValueRef::Parameter(entry_frontier),
            ],
            local_error,
            Vec::new(),
        ),
    );

    let type_map = assembler.parameter(
        validate_types,
        ParameterRole::Block,
        0,
        complete_block_map_type(),
    );
    let type_frontier = assembler.parameter(validate_types, ParameterRole::Block, 1, u32_type());
    let types_valid = assembler.operation(
        validate_types,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(register_types),
            ValueRef::Parameter(type_frontier),
        ],
        unit_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: type_count_validator,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        validate_types,
        function,
        vec![type_map, type_frontier],
        vec![types_valid],
        inventory_switch(
            operation_value(types_valid),
            vec![
                (
                    BuiltinCase::Ok,
                    emit,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(type_map)),
                        SwitchArgument::Value(ValueRef::Parameter(type_frontier)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let emit_map = assembler.parameter(emit, ParameterRole::Block, 0, complete_block_map_type());
    let emit_frontier = assembler.parameter(emit, ParameterRole::Block, 1, u32_type());
    let model = assembler.operation(
        emit,
        Opcode::TupleNew,
        vec![
            ValueRef::Parameter(function_identity),
            ValueRef::Parameter(function_parameters),
            ValueRef::Parameter(register_types),
            ValueRef::Parameter(result_type),
            ValueRef::Parameter(entry_slot),
            ValueRef::Parameter(emit_map),
            ValueRef::Parameter(emit_frontier),
        ],
        complete_function_model_type(),
        Immediate::None,
    );
    let success = assembler.operation(
        emit,
        Opcode::ResultOk,
        vec![operation_value(model)],
        complete_function_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        emit,
        function,
        vec![emit_map, emit_frontier],
        vec![model, success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        complete_function_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    for (block, code) in [(local_error, local_code), (resource_error, resource_code)] {
        let value = assembler.constant_ref(block, code, u32_type());
        let failure = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![operation_value(value)],
            complete_function_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![value, failure],
            Terminator::Return(ReturnTerminator {
                value: operation_value(failure),
            }),
        );
    }
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            function_identity,
            function_parameters,
            register_types,
            result_type,
            entry_slot,
            block_count,
            block_facts,
        ],
        result_type: complete_function_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let mut functions = base.functions;
    functions.extend([dense_graph, type_count_graph, graph.clone()]);
    let mut adapters = base.adapters;
    adapters.push(AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        )),
        adapter_id: sley_vm::host_abi::bridge_identity(sley_vm::host_abi::BRIDGE_CODE_B2V1),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type: TypeExpr::Bytes,
        response_type: u8vec_type(),
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    });
    LowerScaffold {
        types: base.types,
        entry: graph,
        functions,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters,
    }
}

/// Builds the bootstrap-safe integer-to-octet narrowing ladder used by the
/// canonical fixed-width encoder. The caller proves that `value < 256`; the
/// ladder reconstructs the `UInt8` value one bit at a time because the frozen
/// bootstrap profile deliberately has no integer-cast opcode.
#[allow(clippy::too_many_lines)]
fn build_unsigned_octet_narrower(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    value_type: &TypeExpr,
    constant_value: fn(u128) -> ConstValue,
) -> FunctionGraph {
    let value = assembler.parameter(function, ParameterRole::Function, 0, value_type.clone());
    let entry = assembler.block_id();
    let narrow_blocks = (0..8).map(|_| assembler.block_id()).collect::<Vec<_>>();
    let subtract_blocks = (0..8).map(|_| assembler.block_id()).collect::<Vec<_>>();
    let add_blocks = (0..8).map(|_| assembler.block_id()).collect::<Vec<_>>();
    let done = assembler.block_id();
    let invariant_trap = assembler.block_id();
    let zero_u8 = assembler.constant(ConstValue {
        value_type: u8_type(),
        data: ConstData::UInt(0),
    });
    let powers = [128_u128, 64, 32, 16, 8, 4, 2, 1]
        .into_iter()
        .map(|power| assembler.constant(constant_value(power)))
        .collect::<Vec<_>>();
    let bits = [128_u128, 64, 32, 16, 8, 4, 2, 1]
        .into_iter()
        .map(|bit| {
            assembler.constant(ConstValue {
                value_type: u8_type(),
                data: ConstData::UInt(bit),
            })
        })
        .collect::<Vec<_>>();

    let zero = assembler.constant_ref(entry, zero_u8, u8_type());
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![zero],
        inventory_branch(
            narrow_blocks[0],
            vec![ValueRef::Parameter(value), operation_value(zero)],
        ),
    );

    for index in 0..8 {
        let narrow = narrow_blocks[index];
        let current = assembler.parameter(narrow, ParameterRole::Block, 0, value_type.clone());
        let octet = assembler.parameter(narrow, ParameterRole::Block, 1, u8_type());
        let power = assembler.constant_ref(narrow, powers[index], value_type.clone());
        let contains_bit = assembler.operation(
            narrow,
            Opcode::GreaterEqual,
            vec![ValueRef::Parameter(current), operation_value(power)],
            TypeExpr::Bool,
            Immediate::None,
        );
        let false_target = if index == 7 {
            done
        } else {
            narrow_blocks[index + 1]
        };
        let false_arguments = if index == 7 {
            vec![ValueRef::Parameter(octet)]
        } else {
            vec![ValueRef::Parameter(current), ValueRef::Parameter(octet)]
        };
        assembler.push_block(
            narrow,
            function,
            vec![current, octet],
            vec![power, contains_bit],
            inventory_cond(
                operation_value(contains_bit),
                subtract_blocks[index],
                vec![ValueRef::Parameter(current), ValueRef::Parameter(octet)],
                false_target,
                false_arguments,
            ),
        );

        let subtract = subtract_blocks[index];
        let subtract_current =
            assembler.parameter(subtract, ParameterRole::Block, 0, value_type.clone());
        let subtract_octet = assembler.parameter(subtract, ParameterRole::Block, 1, u8_type());
        let subtract_power = assembler.constant_ref(subtract, powers[index], value_type.clone());
        let reduced = assembler.operation(
            subtract,
            Opcode::IntSubChecked,
            vec![
                ValueRef::Parameter(subtract_current),
                operation_value(subtract_power),
            ],
            arithmetic_result_type(value_type.clone()),
            Immediate::None,
        );
        assembler.push_block(
            subtract,
            function,
            vec![subtract_current, subtract_octet],
            vec![subtract_power, reduced],
            inventory_switch(
                operation_value(reduced),
                vec![
                    (
                        BuiltinCase::Ok,
                        add_blocks[index],
                        vec![
                            SwitchArgument::CasePayload,
                            SwitchArgument::Value(ValueRef::Parameter(subtract_octet)),
                        ],
                    ),
                    (BuiltinCase::Err, invariant_trap, Vec::new()),
                ],
            ),
        );

        let add = add_blocks[index];
        let add_current = assembler.parameter(add, ParameterRole::Block, 0, value_type.clone());
        let add_octet = assembler.parameter(add, ParameterRole::Block, 1, u8_type());
        let add_bit = assembler.constant_ref(add, bits[index], u8_type());
        let increased = assembler.operation(
            add,
            Opcode::IntAddChecked,
            vec![ValueRef::Parameter(add_octet), operation_value(add_bit)],
            arithmetic_result_type(u8_type()),
            Immediate::None,
        );
        let next_target = if index == 7 {
            done
        } else {
            narrow_blocks[index + 1]
        };
        let mut next_arguments = Vec::new();
        if index != 7 {
            next_arguments.push(SwitchArgument::Value(ValueRef::Parameter(add_current)));
        }
        next_arguments.push(SwitchArgument::CasePayload);
        assembler.push_block(
            add,
            function,
            vec![add_current, add_octet],
            vec![add_bit, increased],
            inventory_switch(
                operation_value(increased),
                vec![
                    (BuiltinCase::Ok, next_target, next_arguments),
                    (BuiltinCase::Err, invariant_trap, Vec::new()),
                ],
            ),
        );
    }

    let narrowed = assembler.parameter(done, ParameterRole::Block, 0, u8_type());
    assembler.push_block(
        done,
        function,
        vec![narrowed],
        Vec::new(),
        Terminator::Return(ReturnTerminator {
            value: ValueRef::Parameter(narrowed),
        }),
    );
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![value],
        result_type: u8_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

/// Appends one fixed-width unsigned value in canonical big-endian order.
/// Division and remainder stay in the source width; the private narrowing
/// function converts each proven 0..=255 quotient into an octet.
#[allow(clippy::too_many_lines)]
fn build_fixed_width_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    narrow_function: EntityId,
    value_type: &TypeExpr,
    constant_value: fn(u128) -> ConstValue,
    divisors: &[u128],
) -> FunctionGraph {
    let value = assembler.parameter(function, ParameterRole::Function, 0, value_type.clone());
    let accumulator = assembler.parameter(function, ParameterRole::Function, 1, u8vec_type());
    let entry = assembler.block_id();
    let step_blocks = divisors
        .iter()
        .map(|_| assembler.block_id())
        .collect::<Vec<_>>();
    let quotient_blocks = divisors
        .iter()
        .map(|_| assembler.block_id())
        .collect::<Vec<_>>();
    let append_blocks = divisors
        .iter()
        .map(|_| assembler.block_id())
        .collect::<Vec<_>>();
    let done = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();
    let divisor_constants = divisors
        .iter()
        .map(|divisor| assembler.constant(constant_value(*divisor)))
        .collect::<Vec<_>>();
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    assembler.push_block(
        entry,
        function,
        Vec::new(),
        Vec::new(),
        inventory_branch(
            step_blocks[0],
            vec![ValueRef::Parameter(value), ValueRef::Parameter(accumulator)],
        ),
    );

    for index in 0..divisors.len() {
        let step = step_blocks[index];
        let remaining = assembler.parameter(step, ParameterRole::Block, 0, value_type.clone());
        let bytes = assembler.parameter(step, ParameterRole::Block, 1, u8vec_type());
        let divisor = assembler.constant_ref(step, divisor_constants[index], value_type.clone());
        let quotient = assembler.operation(
            step,
            Opcode::IntDivChecked,
            vec![ValueRef::Parameter(remaining), operation_value(divisor)],
            arithmetic_result_type(value_type.clone()),
            Immediate::None,
        );
        assembler.push_block(
            step,
            function,
            vec![remaining, bytes],
            vec![divisor, quotient],
            inventory_switch(
                operation_value(quotient),
                vec![
                    (
                        BuiltinCase::Ok,
                        quotient_blocks[index],
                        vec![
                            SwitchArgument::CasePayload,
                            SwitchArgument::Value(ValueRef::Parameter(remaining)),
                            SwitchArgument::Value(ValueRef::Parameter(bytes)),
                        ],
                    ),
                    (BuiltinCase::Err, invariant_trap, Vec::new()),
                ],
            ),
        );

        let quotient_block = quotient_blocks[index];
        let quotient_value =
            assembler.parameter(quotient_block, ParameterRole::Block, 0, value_type.clone());
        let quotient_source =
            assembler.parameter(quotient_block, ParameterRole::Block, 1, value_type.clone());
        let quotient_bytes =
            assembler.parameter(quotient_block, ParameterRole::Block, 2, u8vec_type());
        let remainder_divisor =
            assembler.constant_ref(quotient_block, divisor_constants[index], value_type.clone());
        let remainder = assembler.operation(
            quotient_block,
            Opcode::IntRemChecked,
            vec![
                ValueRef::Parameter(quotient_source),
                operation_value(remainder_divisor),
            ],
            arithmetic_result_type(value_type.clone()),
            Immediate::None,
        );
        assembler.push_block(
            quotient_block,
            function,
            vec![quotient_value, quotient_source, quotient_bytes],
            vec![remainder_divisor, remainder],
            inventory_switch(
                operation_value(remainder),
                vec![
                    (
                        BuiltinCase::Ok,
                        append_blocks[index],
                        vec![
                            SwitchArgument::Value(ValueRef::Parameter(quotient_value)),
                            SwitchArgument::CasePayload,
                            SwitchArgument::Value(ValueRef::Parameter(quotient_bytes)),
                        ],
                    ),
                    (BuiltinCase::Err, invariant_trap, Vec::new()),
                ],
            ),
        );

        let append = append_blocks[index];
        let append_quotient =
            assembler.parameter(append, ParameterRole::Block, 0, value_type.clone());
        let append_remainder =
            assembler.parameter(append, ParameterRole::Block, 1, value_type.clone());
        let append_bytes = assembler.parameter(append, ParameterRole::Block, 2, u8vec_type());
        let octet = assembler.operation(
            append,
            Opcode::CallDirect,
            vec![ValueRef::Parameter(append_quotient)],
            u8_type(),
            Immediate::Function(FunctionRefValue {
                function: narrow_function,
                type_arguments: Vec::new(),
            }),
        );
        let pushed = assembler.operation(
            append,
            Opcode::AdapterInvoke,
            vec![ValueRef::Parameter(append_bytes), operation_value(octet)],
            index_result_type(u8vec_type()),
            Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
                sley_vm::host_abi::BRIDGE_CODE_PSH1,
            ))),
        );
        let (next_target, next_arguments) = if index + 1 == divisors.len() {
            (done, vec![SwitchArgument::CasePayload])
        } else {
            (
                step_blocks[index + 1],
                vec![
                    SwitchArgument::Value(ValueRef::Parameter(append_remainder)),
                    SwitchArgument::CasePayload,
                ],
            )
        };
        assembler.push_block(
            append,
            function,
            vec![append_quotient, append_remainder, append_bytes],
            vec![octet, pushed],
            inventory_switch(
                operation_value(pushed),
                vec![
                    (BuiltinCase::Ok, next_target, next_arguments),
                    (BuiltinCase::Err, resource_error, Vec::new()),
                ],
            ),
        );
    }

    let completed = assembler.parameter(done, ParameterRole::Block, 0, u8vec_type());
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(completed)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![completed],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(failure),
        }),
    );
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![value, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

/// Exact fixed-width byte primitive for SLEYBC02 assembly. It preserves an
/// arbitrary prefix, appends one `UInt32` and one `UInt64` in canonical big-endian
/// order, and converts the Sley-owned octet vector back to Bytes.
#[allow(clippy::too_many_lines)]
fn fixed_width_byte_encoder() -> LowerScaffold {
    let narrow_u32 = inventory_id(5, 19);
    let append_u32 = inventory_id(5, 20);
    let narrow_u64 = inventory_id(5, 21);
    let append_u64 = inventory_id(5, 22);
    let function = inventory_id(5, 23);
    let mut assembler = InventoryAssembler::new();
    let narrow_u32_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u32, &u32_type(), u32_value);
    let append_u32_graph = build_fixed_width_appender(
        &mut assembler,
        append_u32,
        narrow_u32,
        &u32_type(),
        u32_value,
        &[1_u128 << 24, 1_u128 << 16, 1_u128 << 8, 1],
    );
    let narrow_u64_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u64, &u64_type(), u64_value);
    let append_u64_graph = build_fixed_width_appender(
        &mut assembler,
        append_u64,
        narrow_u64,
        &u64_type(),
        u64_value,
        &[
            1_u128 << 56,
            1_u128 << 48,
            1_u128 << 40,
            1_u128 << 32,
            1_u128 << 24,
            1_u128 << 16,
            1_u128 << 8,
            1,
        ],
    );

    let prefix = assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bytes);
    let value_u32 = assembler.parameter(function, ParameterRole::Function, 1, u32_type());
    let value_u64 = assembler.parameter(function, ParameterRole::Function, 2, u64_type());
    let entry = assembler.block_id();
    let append_u32_block = assembler.block_id();
    let append_u64_block = assembler.block_id();
    let convert_block = assembler.block_id();
    let success_block = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let input_scope = assembler.constant_ref(entry, unit, TypeExpr::Unit);
    let input_octets = assembler.operation(
        entry,
        Opcode::AdapterInvoke,
        vec![operation_value(input_scope), ValueRef::Parameter(prefix)],
        index_result_type(u8vec_type()),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![input_scope, input_octets],
        inventory_switch(
            operation_value(input_octets),
            vec![
                (
                    BuiltinCase::Ok,
                    append_u32_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let initial = assembler.parameter(append_u32_block, ParameterRole::Block, 0, u8vec_type());
    let encoded_u32 = assembler.operation(
        append_u32_block,
        Opcode::CallDirect,
        vec![ValueRef::Parameter(value_u32), ValueRef::Parameter(initial)],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_u32_block,
        function,
        vec![initial],
        vec![encoded_u32],
        inventory_switch(
            operation_value(encoded_u32),
            vec![
                (
                    BuiltinCase::Ok,
                    append_u64_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let after_u32 = assembler.parameter(append_u64_block, ParameterRole::Block, 0, u8vec_type());
    let encoded_u64 = assembler.operation(
        append_u64_block,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(value_u64),
            ValueRef::Parameter(after_u32),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u64,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_u64_block,
        function,
        vec![after_u32],
        vec![encoded_u64],
        inventory_switch(
            operation_value(encoded_u64),
            vec![
                (
                    BuiltinCase::Ok,
                    convert_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let octets = assembler.parameter(convert_block, ParameterRole::Block, 0, u8vec_type());
    let output_scope = assembler.constant_ref(convert_block, unit, TypeExpr::Unit);
    let output_bytes = assembler.operation(
        convert_block,
        Opcode::AdapterInvoke,
        vec![operation_value(output_scope), ValueRef::Parameter(octets)],
        index_result_type(TypeExpr::Bytes),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    assembler.push_block(
        convert_block,
        function,
        vec![octets],
        vec![output_scope, output_bytes],
        inventory_switch(
            operation_value(output_bytes),
            vec![
                (
                    BuiltinCase::Ok,
                    success_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let output = assembler.parameter(success_block, ParameterRole::Block, 0, TypeExpr::Bytes);
    let success = assembler.operation(
        success_block,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(output)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success_block,
        function,
        vec![output],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );

    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![prefix, value_u32, value_u64],
        result_type: bytes_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let adapter = |code, request_type, response_type| AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(code)),
        adapter_id: sley_vm::host_abi::bridge_identity(code),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type,
        response_type,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            append_u32_graph,
            narrow_u32_graph,
            append_u64_graph,
            narrow_u64_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![
            adapter(
                sley_vm::host_abi::BRIDGE_CODE_B2V1,
                TypeExpr::Bytes,
                u8vec_type(),
            ),
            adapter(sley_vm::host_abi::BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            adapter(
                sley_vm::host_abi::BRIDGE_CODE_V2B1,
                u8vec_type(),
                TypeExpr::Bytes,
            ),
        ],
    }
}

/// Appends an exact `Bytes` chunk to a Sley-owned octet vector. B2V1 exposes
/// the chunk for a checked indexed walk; PSH1 performs the only mutation.
#[allow(clippy::too_many_lines)]
fn build_byte_chunk_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
) -> FunctionGraph {
    let accumulator = assembler.parameter(function, ParameterRole::Function, 0, u8vec_type());
    let chunk = assembler.parameter(function, ParameterRole::Function, 1, TypeExpr::Bytes);
    let entry = assembler.block_id();
    let initialize = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let append = assembler.block_id();
    let advance = assembler.block_id();
    let done = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();
    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let scope = assembler.constant_ref(entry, unit, TypeExpr::Unit);
    let chunk_octets = assembler.operation(
        entry,
        Opcode::AdapterInvoke,
        vec![operation_value(scope), ValueRef::Parameter(chunk)],
        index_result_type(u8vec_type()),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![scope, chunk_octets],
        inventory_switch(
            operation_value(chunk_octets),
            vec![
                (
                    BuiltinCase::Ok,
                    initialize,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let values = assembler.parameter(initialize, ParameterRole::Block, 0, u8vec_type());
    let zero = assembler.constant_ref(initialize, zero_u64, u64_type());
    let length = assembler.operation(
        initialize,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(values)],
        u64_type(),
        Immediate::None,
    );
    assembler.push_block(
        initialize,
        function,
        vec![values],
        vec![zero, length],
        inventory_branch(
            check,
            vec![
                operation_value(zero),
                ValueRef::Parameter(accumulator),
                ValueRef::Parameter(values),
                operation_value(length),
            ],
        ),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let check_accumulator = assembler.parameter(check, ParameterRole::Block, 1, u8vec_type());
    let check_values = assembler.parameter(check, ParameterRole::Block, 2, u8vec_type());
    let check_length = assembler.parameter(check, ParameterRole::Block, 3, u64_type());
    let has_value = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_index),
            ValueRef::Parameter(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        vec![check_index, check_accumulator, check_values, check_length],
        vec![has_value],
        inventory_cond(
            operation_value(has_value),
            get,
            vec![
                ValueRef::Parameter(check_index),
                ValueRef::Parameter(check_accumulator),
                ValueRef::Parameter(check_values),
                ValueRef::Parameter(check_length),
            ],
            done,
            vec![ValueRef::Parameter(check_accumulator)],
        ),
    );

    let get_index = assembler.parameter(get, ParameterRole::Block, 0, u64_type());
    let get_accumulator = assembler.parameter(get, ParameterRole::Block, 1, u8vec_type());
    let get_values = assembler.parameter(get, ParameterRole::Block, 2, u8vec_type());
    let get_length = assembler.parameter(get, ParameterRole::Block, 3, u64_type());
    let octet = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(get_values),
            ValueRef::Parameter(get_index),
        ],
        TypeExpr::Option(Box::new(u8_type())),
        Immediate::None,
    );
    assembler.push_block(
        get,
        function,
        vec![get_index, get_accumulator, get_values, get_length],
        vec![octet],
        inventory_switch(
            operation_value(octet),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    append,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                        SwitchArgument::Value(ValueRef::Parameter(get_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(get_values)),
                        SwitchArgument::Value(ValueRef::Parameter(get_length)),
                    ],
                ),
            ],
        ),
    );

    let append_octet = assembler.parameter(append, ParameterRole::Block, 0, u8_type());
    let append_index = assembler.parameter(append, ParameterRole::Block, 1, u64_type());
    let append_accumulator = assembler.parameter(append, ParameterRole::Block, 2, u8vec_type());
    let append_values = assembler.parameter(append, ParameterRole::Block, 3, u8vec_type());
    let append_length = assembler.parameter(append, ParameterRole::Block, 4, u64_type());
    let pushed = assembler.operation(
        append,
        Opcode::AdapterInvoke,
        vec![
            ValueRef::Parameter(append_accumulator),
            ValueRef::Parameter(append_octet),
        ],
        index_result_type(u8vec_type()),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    assembler.push_block(
        append,
        function,
        vec![
            append_octet,
            append_index,
            append_accumulator,
            append_values,
            append_length,
        ],
        vec![pushed],
        inventory_switch(
            operation_value(pushed),
            vec![
                (
                    BuiltinCase::Ok,
                    advance,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(append_index)),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(append_values)),
                        SwitchArgument::Value(ValueRef::Parameter(append_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let advance_index = assembler.parameter(advance, ParameterRole::Block, 0, u64_type());
    let advance_accumulator = assembler.parameter(advance, ParameterRole::Block, 1, u8vec_type());
    let advance_values = assembler.parameter(advance, ParameterRole::Block, 2, u8vec_type());
    let advance_length = assembler.parameter(advance, ParameterRole::Block, 3, u64_type());
    let one = assembler.constant_ref(advance, one_u64, u64_type());
    let next_index = assembler.operation(
        advance,
        Opcode::IntAddChecked,
        vec![ValueRef::Parameter(advance_index), operation_value(one)],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance,
        function,
        vec![
            advance_index,
            advance_accumulator,
            advance_values,
            advance_length,
        ],
        vec![one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_values)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let completed = assembler.parameter(done, ParameterRole::Block, 0, u8vec_type());
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(completed)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![completed],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(failure),
        }),
    );
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![accumulator, chunk],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

/// Focused closure proving exact byte-chunk concatenation through the frozen
/// bridge inventory before the helper is composed into function-body encoding.
#[allow(clippy::too_many_lines)]
fn byte_chunk_encoder() -> LowerScaffold {
    let append_chunk = inventory_id(5, 24);
    let function = inventory_id(5, 25);
    let mut assembler = InventoryAssembler::new();
    let append_graph = build_byte_chunk_appender(&mut assembler, append_chunk);
    let prefix = assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bytes);
    let chunk = assembler.parameter(function, ParameterRole::Function, 1, TypeExpr::Bytes);
    let entry = assembler.block_id();
    let append = assembler.block_id();
    let convert = assembler.block_id();
    let success_block = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let input_scope = assembler.constant_ref(entry, unit, TypeExpr::Unit);
    let prefix_octets = assembler.operation(
        entry,
        Opcode::AdapterInvoke,
        vec![operation_value(input_scope), ValueRef::Parameter(prefix)],
        index_result_type(u8vec_type()),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![input_scope, prefix_octets],
        inventory_switch(
            operation_value(prefix_octets),
            vec![
                (BuiltinCase::Ok, append, vec![SwitchArgument::CasePayload]),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let initial = assembler.parameter(append, ParameterRole::Block, 0, u8vec_type());
    let appended = assembler.operation(
        append,
        Opcode::CallDirect,
        vec![ValueRef::Parameter(initial), ValueRef::Parameter(chunk)],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_chunk,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append,
        function,
        vec![initial],
        vec![appended],
        inventory_switch(
            operation_value(appended),
            vec![
                (BuiltinCase::Ok, convert, vec![SwitchArgument::CasePayload]),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let octets = assembler.parameter(convert, ParameterRole::Block, 0, u8vec_type());
    let output_scope = assembler.constant_ref(convert, unit, TypeExpr::Unit);
    let output = assembler.operation(
        convert,
        Opcode::AdapterInvoke,
        vec![operation_value(output_scope), ValueRef::Parameter(octets)],
        index_result_type(TypeExpr::Bytes),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    assembler.push_block(
        convert,
        function,
        vec![octets],
        vec![output_scope, output],
        inventory_switch(
            operation_value(output),
            vec![
                (
                    BuiltinCase::Ok,
                    success_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let encoded = assembler.parameter(success_block, ParameterRole::Block, 0, TypeExpr::Bytes);
    let success = assembler.operation(
        success_block,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(encoded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success_block,
        function,
        vec![encoded],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );

    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![prefix, chunk],
        result_type: bytes_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let adapter = |code, request_type, response_type| AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(code)),
        adapter_id: sley_vm::host_abi::bridge_identity(code),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type,
        response_type,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph, append_graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![
            adapter(
                sley_vm::host_abi::BRIDGE_CODE_B2V1,
                TypeExpr::Bytes,
                u8vec_type(),
            ),
            adapter(sley_vm::host_abi::BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            adapter(
                sley_vm::host_abi::BRIDGE_CODE_V2B1,
                u8vec_type(),
                TypeExpr::Bytes,
            ),
        ],
    }
}

/// Prefixes one exact byte section with its canonical `UInt64` byte length
/// and appends the section to a Sley-owned octet accumulator.
#[allow(clippy::too_many_lines)]
fn build_framed_byte_section_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_u64: EntityId,
    append_chunk: EntityId,
) -> FunctionGraph {
    let section = assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bytes);
    let accumulator = assembler.parameter(function, ParameterRole::Function, 1, u8vec_type());
    let entry = assembler.block_id();
    let prefix_length = assembler.block_id();
    let append_section = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let scope = assembler.constant_ref(entry, unit, TypeExpr::Unit);
    let octets = assembler.operation(
        entry,
        Opcode::AdapterInvoke,
        vec![operation_value(scope), ValueRef::Parameter(section)],
        index_result_type(u8vec_type()),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![scope, octets],
        inventory_switch(
            operation_value(octets),
            vec![
                (
                    BuiltinCase::Ok,
                    prefix_length,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let section_octets = assembler.parameter(prefix_length, ParameterRole::Block, 0, u8vec_type());
    let length = assembler.operation(
        prefix_length,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(section_octets)],
        u64_type(),
        Immediate::None,
    );
    let prefixed = assembler.operation(
        prefix_length,
        Opcode::CallDirect,
        vec![operation_value(length), ValueRef::Parameter(accumulator)],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u64,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        prefix_length,
        function,
        vec![section_octets],
        vec![length, prefixed],
        inventory_switch(
            operation_value(prefixed),
            vec![
                (
                    BuiltinCase::Ok,
                    append_section,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let section_accumulator =
        assembler.parameter(append_section, ParameterRole::Block, 0, u8vec_type());
    let appended = assembler.operation(
        append_section,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(section_accumulator),
            ValueRef::Parameter(section),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_chunk,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_section,
        function,
        vec![section_accumulator],
        vec![appended],
        Terminator::Return(ReturnTerminator {
            value: operation_value(appended),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![section, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

/// Appends every item of a concrete vector through a private item encoder.
/// `prefix_length` emits the canonical `UInt64` element count first when the
/// owning format is counted. The `item_first` flag reflects the established
/// helper signatures: numeric appenders take `(value, acc)`, while exact byte
/// chunks take `(acc, value)`.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_vector_byte_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_u64: EntityId,
    append_item: EntityId,
    vector_type: &TypeExpr,
    item_type: &TypeExpr,
    item_first: bool,
    prefix_length: bool,
) -> FunctionGraph {
    let values = assembler.parameter(function, ParameterRole::Function, 0, vector_type.clone());
    let accumulator = assembler.parameter(function, ParameterRole::Function, 1, u8vec_type());
    let entry = assembler.block_id();
    let initialize = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let append = assembler.block_id();
    let advance = assembler.block_id();
    let done = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();
    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(values)],
        u64_type(),
        Immediate::None,
    );
    if prefix_length {
        let length_prefix = assembler.operation(
            entry,
            Opcode::CallDirect,
            vec![operation_value(length), ValueRef::Parameter(accumulator)],
            byte_vector_lower_result_type(),
            Immediate::Function(FunctionRefValue {
                function: append_u64,
                type_arguments: Vec::new(),
            }),
        );
        assembler.push_block(
            entry,
            function,
            Vec::new(),
            vec![length, length_prefix],
            inventory_switch(
                operation_value(length_prefix),
                vec![
                    (
                        BuiltinCase::Ok,
                        initialize,
                        vec![
                            SwitchArgument::CasePayload,
                            SwitchArgument::Value(operation_value(length)),
                        ],
                    ),
                    (
                        BuiltinCase::Err,
                        forward_error,
                        vec![SwitchArgument::CasePayload],
                    ),
                ],
            ),
        );
    } else {
        assembler.push_block(
            entry,
            function,
            Vec::new(),
            vec![length],
            inventory_branch(
                initialize,
                vec![ValueRef::Parameter(accumulator), operation_value(length)],
            ),
        );
    }

    let initial_accumulator =
        assembler.parameter(initialize, ParameterRole::Block, 0, u8vec_type());
    let initial_length = assembler.parameter(initialize, ParameterRole::Block, 1, u64_type());
    let zero = assembler.constant_ref(initialize, zero_u64, u64_type());
    assembler.push_block(
        initialize,
        function,
        vec![initial_accumulator, initial_length],
        vec![zero],
        inventory_branch(
            check,
            vec![
                operation_value(zero),
                ValueRef::Parameter(initial_accumulator),
                ValueRef::Parameter(initial_length),
            ],
        ),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let check_accumulator = assembler.parameter(check, ParameterRole::Block, 1, u8vec_type());
    let check_length = assembler.parameter(check, ParameterRole::Block, 2, u64_type());
    let has_value = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_index),
            ValueRef::Parameter(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        vec![check_index, check_accumulator, check_length],
        vec![has_value],
        inventory_cond(
            operation_value(has_value),
            get,
            vec![
                ValueRef::Parameter(check_index),
                ValueRef::Parameter(check_accumulator),
                ValueRef::Parameter(check_length),
            ],
            done,
            vec![ValueRef::Parameter(check_accumulator)],
        ),
    );

    let get_index = assembler.parameter(get, ParameterRole::Block, 0, u64_type());
    let get_accumulator = assembler.parameter(get, ParameterRole::Block, 1, u8vec_type());
    let get_length = assembler.parameter(get, ParameterRole::Block, 2, u64_type());
    let item = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![ValueRef::Parameter(values), ValueRef::Parameter(get_index)],
        TypeExpr::Option(Box::new(item_type.clone())),
        Immediate::None,
    );
    assembler.push_block(
        get,
        function,
        vec![get_index, get_accumulator, get_length],
        vec![item],
        inventory_switch(
            operation_value(item),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    append,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                        SwitchArgument::Value(ValueRef::Parameter(get_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(get_length)),
                    ],
                ),
            ],
        ),
    );

    let append_item_value = assembler.parameter(append, ParameterRole::Block, 0, item_type.clone());
    let append_index = assembler.parameter(append, ParameterRole::Block, 1, u64_type());
    let append_accumulator = assembler.parameter(append, ParameterRole::Block, 2, u8vec_type());
    let append_length = assembler.parameter(append, ParameterRole::Block, 3, u64_type());
    let item_operands = if item_first {
        vec![
            ValueRef::Parameter(append_item_value),
            ValueRef::Parameter(append_accumulator),
        ]
    } else {
        vec![
            ValueRef::Parameter(append_accumulator),
            ValueRef::Parameter(append_item_value),
        ]
    };
    let appended = assembler.operation(
        append,
        Opcode::CallDirect,
        item_operands,
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_item,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append,
        function,
        vec![
            append_item_value,
            append_index,
            append_accumulator,
            append_length,
        ],
        vec![appended],
        inventory_switch(
            operation_value(appended),
            vec![
                (
                    BuiltinCase::Ok,
                    advance,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(append_index)),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(append_length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let advance_index = assembler.parameter(advance, ParameterRole::Block, 0, u64_type());
    let advance_accumulator = assembler.parameter(advance, ParameterRole::Block, 1, u8vec_type());
    let advance_length = assembler.parameter(advance, ParameterRole::Block, 2, u64_type());
    let one = assembler.constant_ref(advance, one_u64, u64_type());
    let next_index = assembler.operation(
        advance,
        Opcode::IntAddChecked,
        vec![ValueRef::Parameter(advance_index), operation_value(one)],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance,
        function,
        vec![advance_index, advance_accumulator, advance_length],
        vec![one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let completed = assembler.parameter(done, ParameterRole::Block, 0, u8vec_type());
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(completed)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![completed],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![values, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

/// Emits one complete extended-profile instruction from the lossless lowered
/// model: opcode, operand registers, result registers, and exact immediate.
#[allow(clippy::too_many_lines)]
fn build_instruction_byte_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_u32: EntityId,
    append_u32_vector: EntityId,
    append_chunk: EntityId,
) -> FunctionGraph {
    let instruction = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        immediate_instruction_type(),
    );
    let accumulator = assembler.parameter(function, ParameterRole::Function, 1, u8vec_type());
    let entry = assembler.block_id();
    let append_operands = assembler.block_id();
    let append_results = assembler.block_id();
    let append_immediate = assembler.block_id();
    let forward_error = assembler.block_id();

    let opcode = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(instruction)],
        u32_type(),
        Immediate::Index(0),
    );
    let operands = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(instruction)],
        u32vec_type(),
        Immediate::Index(1),
    );
    let results = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(instruction)],
        u32vec_type(),
        Immediate::Index(2),
    );
    let immediate_bytes = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(instruction)],
        TypeExpr::Bytes,
        Immediate::Index(6),
    );
    let encoded_opcode = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![operation_value(opcode), ValueRef::Parameter(accumulator)],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![opcode, operands, results, immediate_bytes, encoded_opcode],
        inventory_switch(
            operation_value(encoded_opcode),
            vec![
                (
                    BuiltinCase::Ok,
                    append_operands,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(operation_value(operands)),
                        SwitchArgument::Value(operation_value(results)),
                        SwitchArgument::Value(operation_value(immediate_bytes)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let operand_accumulator =
        assembler.parameter(append_operands, ParameterRole::Block, 0, u8vec_type());
    let operand_values =
        assembler.parameter(append_operands, ParameterRole::Block, 1, u32vec_type());
    let operand_results =
        assembler.parameter(append_operands, ParameterRole::Block, 2, u32vec_type());
    let operand_immediate =
        assembler.parameter(append_operands, ParameterRole::Block, 3, TypeExpr::Bytes);
    let encoded_operands = assembler.operation(
        append_operands,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(operand_values),
            ValueRef::Parameter(operand_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32_vector,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_operands,
        function,
        vec![
            operand_accumulator,
            operand_values,
            operand_results,
            operand_immediate,
        ],
        vec![encoded_operands],
        inventory_switch(
            operation_value(encoded_operands),
            vec![
                (
                    BuiltinCase::Ok,
                    append_results,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(operand_results)),
                        SwitchArgument::Value(ValueRef::Parameter(operand_immediate)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let result_accumulator =
        assembler.parameter(append_results, ParameterRole::Block, 0, u8vec_type());
    let result_values = assembler.parameter(append_results, ParameterRole::Block, 1, u32vec_type());
    let result_immediate =
        assembler.parameter(append_results, ParameterRole::Block, 2, TypeExpr::Bytes);
    let encoded_results = assembler.operation(
        append_results,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(result_values),
            ValueRef::Parameter(result_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32_vector,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_results,
        function,
        vec![result_accumulator, result_values, result_immediate],
        vec![encoded_results],
        inventory_switch(
            operation_value(encoded_results),
            vec![
                (
                    BuiltinCase::Ok,
                    append_immediate,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(result_immediate)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let immediate_accumulator =
        assembler.parameter(append_immediate, ParameterRole::Block, 0, u8vec_type());
    let immediate_value =
        assembler.parameter(append_immediate, ParameterRole::Block, 1, TypeExpr::Bytes);
    let encoded_immediate = assembler.operation(
        append_immediate,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(immediate_accumulator),
            ValueRef::Parameter(immediate_value),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_chunk,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_immediate,
        function,
        vec![immediate_accumulator, immediate_value],
        vec![encoded_immediate],
        Terminator::Return(ReturnTerminator {
            value: operation_value(encoded_immediate),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(failure),
        }),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![instruction, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn instruction_byte_encoder() -> LowerScaffold {
    let narrow_u32 = inventory_id(5, 19);
    let append_u32 = inventory_id(5, 20);
    let narrow_u64 = inventory_id(5, 21);
    let append_u64 = inventory_id(5, 22);
    let append_chunk = inventory_id(5, 24);
    let append_u32_vector = inventory_id(5, 26);
    let function = inventory_id(5, 29);
    let mut assembler = InventoryAssembler::new();
    let narrow_u32_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u32, &u32_type(), u32_value);
    let append_u32_graph = build_fixed_width_appender(
        &mut assembler,
        append_u32,
        narrow_u32,
        &u32_type(),
        u32_value,
        &[1_u128 << 24, 1_u128 << 16, 1_u128 << 8, 1],
    );
    let narrow_u64_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u64, &u64_type(), u64_value);
    let append_u64_graph = build_fixed_width_appender(
        &mut assembler,
        append_u64,
        narrow_u64,
        &u64_type(),
        u64_value,
        &[
            1_u128 << 56,
            1_u128 << 48,
            1_u128 << 40,
            1_u128 << 32,
            1_u128 << 24,
            1_u128 << 16,
            1_u128 << 8,
            1,
        ],
    );
    let append_chunk_graph = build_byte_chunk_appender(&mut assembler, append_chunk);
    let append_u32_vector_graph = build_vector_byte_appender(
        &mut assembler,
        append_u32_vector,
        append_u64,
        append_u32,
        &u32vec_type(),
        &u32_type(),
        true,
        true,
    );
    let graph = build_instruction_byte_appender(
        &mut assembler,
        function,
        append_u32,
        append_u32_vector,
        append_chunk,
    );
    let adapter = |code, request_type, response_type| AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(code)),
        adapter_id: sley_vm::host_abi::bridge_identity(code),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type,
        response_type,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            narrow_u32_graph,
            append_u32_graph,
            narrow_u64_graph,
            append_u64_graph,
            append_chunk_graph,
            append_u32_vector_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![
            adapter(
                sley_vm::host_abi::BRIDGE_CODE_B2V1,
                TypeExpr::Bytes,
                u8vec_type(),
            ),
            adapter(sley_vm::host_abi::BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
        ],
    }
}

/// Emits the canonical instruction count and walks the dense ordinal keys of
/// the Sley-owned instruction map. The count comes from the already validated
/// semantic inventory because the frozen map profile has no length opcode.
#[allow(clippy::too_many_lines)]
fn build_instruction_map_byte_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_u64: EntityId,
    append_instruction: EntityId,
) -> FunctionGraph {
    let instructions = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        immediate_inventory_model_type(),
    );
    let count = assembler.parameter(function, ParameterRole::Function, 1, u64_type());
    let accumulator = assembler.parameter(function, ParameterRole::Function, 2, u8vec_type());
    let entry = assembler.block_id();
    let initialize = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let append = assembler.block_id();
    let advance = assembler.block_id();
    let done = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();
    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let count_prefix = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![ValueRef::Parameter(count), ValueRef::Parameter(accumulator)],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u64,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![count_prefix],
        inventory_switch(
            operation_value(count_prefix),
            vec![
                (
                    BuiltinCase::Ok,
                    initialize,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let initial_accumulator =
        assembler.parameter(initialize, ParameterRole::Block, 0, u8vec_type());
    let zero = assembler.constant_ref(initialize, zero_u64, u64_type());
    assembler.push_block(
        initialize,
        function,
        vec![initial_accumulator],
        vec![zero],
        inventory_branch(
            check,
            vec![
                operation_value(zero),
                ValueRef::Parameter(initial_accumulator),
            ],
        ),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let check_accumulator = assembler.parameter(check, ParameterRole::Block, 1, u8vec_type());
    let has_instruction = assembler.operation(
        check,
        Opcode::LessThan,
        vec![ValueRef::Parameter(check_index), ValueRef::Parameter(count)],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        vec![check_index, check_accumulator],
        vec![has_instruction],
        inventory_cond(
            operation_value(has_instruction),
            get,
            vec![
                ValueRef::Parameter(check_index),
                ValueRef::Parameter(check_accumulator),
            ],
            done,
            vec![ValueRef::Parameter(check_accumulator)],
        ),
    );

    let get_index = assembler.parameter(get, ParameterRole::Block, 0, u64_type());
    let get_accumulator = assembler.parameter(get, ParameterRole::Block, 1, u8vec_type());
    let found = assembler.operation(
        get,
        Opcode::MapGet,
        vec![
            ValueRef::Parameter(instructions),
            ValueRef::Parameter(get_index),
        ],
        TypeExpr::Option(Box::new(immediate_instruction_type())),
        Immediate::None,
    );
    assembler.push_block(
        get,
        function,
        vec![get_index, get_accumulator],
        vec![found],
        inventory_switch(
            operation_value(found),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    append,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                        SwitchArgument::Value(ValueRef::Parameter(get_accumulator)),
                    ],
                ),
            ],
        ),
    );

    let append_value = assembler.parameter(
        append,
        ParameterRole::Block,
        0,
        immediate_instruction_type(),
    );
    let append_index = assembler.parameter(append, ParameterRole::Block, 1, u64_type());
    let append_accumulator = assembler.parameter(append, ParameterRole::Block, 2, u8vec_type());
    let encoded = assembler.operation(
        append,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(append_value),
            ValueRef::Parameter(append_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_instruction,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append,
        function,
        vec![append_value, append_index, append_accumulator],
        vec![encoded],
        inventory_switch(
            operation_value(encoded),
            vec![
                (
                    BuiltinCase::Ok,
                    advance,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(append_index)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let advance_index = assembler.parameter(advance, ParameterRole::Block, 0, u64_type());
    let advance_accumulator = assembler.parameter(advance, ParameterRole::Block, 1, u8vec_type());
    let one = assembler.constant_ref(advance, one_u64, u64_type());
    let next_index = assembler.operation(
        advance,
        Opcode::IntAddChecked,
        vec![ValueRef::Parameter(advance_index), operation_value(one)],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance,
        function,
        vec![advance_index, advance_accumulator],
        vec![one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_accumulator)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let completed = assembler.parameter(done, ParameterRole::Block, 0, u8vec_type());
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(completed)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![completed],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![instructions, count, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn instruction_map_byte_encoder() -> LowerScaffold {
    let narrow_u32 = inventory_id(5, 19);
    let append_u32 = inventory_id(5, 20);
    let narrow_u64 = inventory_id(5, 21);
    let append_u64 = inventory_id(5, 22);
    let append_chunk = inventory_id(5, 24);
    let append_u32_vector = inventory_id(5, 26);
    let append_instruction = inventory_id(5, 29);
    let function = inventory_id(5, 30);
    let mut assembler = InventoryAssembler::new();
    let narrow_u32_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u32, &u32_type(), u32_value);
    let append_u32_graph = build_fixed_width_appender(
        &mut assembler,
        append_u32,
        narrow_u32,
        &u32_type(),
        u32_value,
        &[1_u128 << 24, 1_u128 << 16, 1_u128 << 8, 1],
    );
    let narrow_u64_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u64, &u64_type(), u64_value);
    let append_u64_graph = build_fixed_width_appender(
        &mut assembler,
        append_u64,
        narrow_u64,
        &u64_type(),
        u64_value,
        &[
            1_u128 << 56,
            1_u128 << 48,
            1_u128 << 40,
            1_u128 << 32,
            1_u128 << 24,
            1_u128 << 16,
            1_u128 << 8,
            1,
        ],
    );
    let append_chunk_graph = build_byte_chunk_appender(&mut assembler, append_chunk);
    let append_u32_vector_graph = build_vector_byte_appender(
        &mut assembler,
        append_u32_vector,
        append_u64,
        append_u32,
        &u32vec_type(),
        &u32_type(),
        true,
        true,
    );
    let append_instruction_graph = build_instruction_byte_appender(
        &mut assembler,
        append_instruction,
        append_u32,
        append_u32_vector,
        append_chunk,
    );
    let graph = build_instruction_map_byte_appender(
        &mut assembler,
        function,
        append_u64,
        append_instruction,
    );
    let adapter = |code, request_type, response_type| AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(code)),
        adapter_id: sley_vm::host_abi::bridge_identity(code),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type,
        response_type,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            narrow_u32_graph,
            append_u32_graph,
            narrow_u64_graph,
            append_u64_graph,
            append_chunk_graph,
            append_u32_vector_graph,
            append_instruction_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![
            adapter(
                sley_vm::host_abi::BRIDGE_CODE_B2V1,
                TypeExpr::Bytes,
                u8vec_type(),
            ),
            adapter(sley_vm::host_abi::BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
        ],
    }
}

fn build_target_edge_byte_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_u32: EntityId,
    append_u32_vector: EntityId,
) -> FunctionGraph {
    let target = assembler.parameter(function, ParameterRole::Function, 0, u32_type());
    let arguments = assembler.parameter(function, ParameterRole::Function, 1, u32vec_type());
    let accumulator = assembler.parameter(function, ParameterRole::Function, 2, u8vec_type());
    let entry = assembler.block_id();
    let append_arguments = assembler.block_id();
    let forward_error = assembler.block_id();
    let encoded_target = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(target),
            ValueRef::Parameter(accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![encoded_target],
        inventory_switch(
            operation_value(encoded_target),
            vec![
                (
                    BuiltinCase::Ok,
                    append_arguments,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let edge_accumulator =
        assembler.parameter(append_arguments, ParameterRole::Block, 0, u8vec_type());
    let encoded_arguments = assembler.operation(
        append_arguments,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(arguments),
            ValueRef::Parameter(edge_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32_vector,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_arguments,
        function,
        vec![edge_accumulator],
        vec![encoded_arguments],
        Terminator::Return(ReturnTerminator {
            value: operation_value(encoded_arguments),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(failure),
        }),
    );
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![target, arguments, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

/// Emits the four non-switch terminator families from their validated simple
/// model. Kind dispatch is private in the eventual complete terminator encoder.
#[allow(clippy::too_many_lines)]
fn build_simple_terminator_byte_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_u32: EntityId,
    append_edge: EntityId,
) -> FunctionGraph {
    let terminator = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        terminator_model_type(),
    );
    let accumulator = assembler.parameter(function, ParameterRole::Function, 1, u8vec_type());
    let entry = assembler.block_id();
    let dispatch_return = assembler.block_id();
    let dispatch_branch = assembler.block_id();
    let dispatch_cond = assembler.block_id();
    let encode_return = assembler.block_id();
    let encode_branch = assembler.block_id();
    let encode_cond_condition = assembler.block_id();
    let encode_cond_true = assembler.block_id();
    let encode_cond_false = assembler.block_id();
    let encode_trap_code = assembler.block_id();
    let dispatch_trap_payload = assembler.block_id();
    let encode_trap_none = assembler.block_id();
    let encode_trap_some_tag = assembler.block_id();
    let encode_trap_some_value = assembler.block_id();
    let forward_error = assembler.block_id();
    let one = assembler.constant(u32_value(1));
    let two = assembler.constant(u32_value(2));
    let three = assembler.constant(u32_value(3));

    let fields = [
        u32_type(),
        u32_type(),
        u32_type(),
        u32vec_type(),
        u32_type(),
        u32vec_type(),
        optional_u32_type(),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, value_type)| {
        assembler.operation(
            entry,
            Opcode::TupleGet,
            vec![ValueRef::Parameter(terminator)],
            value_type,
            Immediate::Index(u32::try_from(index).expect("terminator field index fits u32")),
        )
    })
    .collect::<Vec<_>>();
    let encoded_kind = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![operation_value(fields[0]), ValueRef::Parameter(accumulator)],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        fields
            .iter()
            .copied()
            .chain(std::iter::once(encoded_kind))
            .collect(),
        inventory_switch(
            operation_value(encoded_kind),
            vec![
                (
                    BuiltinCase::Ok,
                    dispatch_return,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(operation_value(fields[0])),
                        SwitchArgument::Value(operation_value(fields[1])),
                        SwitchArgument::Value(operation_value(fields[2])),
                        SwitchArgument::Value(operation_value(fields[3])),
                        SwitchArgument::Value(operation_value(fields[4])),
                        SwitchArgument::Value(operation_value(fields[5])),
                        SwitchArgument::Value(operation_value(fields[6])),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let make_dispatch_parameters =
        |assembler: &mut InventoryAssembler, block: EntityId| -> Vec<EntityId> {
            [
                u8vec_type(),
                u32_type(),
                u32_type(),
                u32_type(),
                u32vec_type(),
                u32_type(),
                u32vec_type(),
                optional_u32_type(),
            ]
            .into_iter()
            .enumerate()
            .map(|(ordinal, value_type)| {
                assembler.parameter(
                    block,
                    ParameterRole::Block,
                    u32::try_from(ordinal).expect("dispatch ordinal fits u32"),
                    value_type,
                )
            })
            .collect::<Vec<_>>()
        };
    let dispatch_values = |parameters: &[EntityId]| {
        parameters
            .iter()
            .copied()
            .map(ValueRef::Parameter)
            .collect::<Vec<_>>()
    };

    let return_parameters = make_dispatch_parameters(assembler, dispatch_return);
    let one_value = assembler.constant_ref(dispatch_return, one, u32_type());
    let is_return = assembler.operation(
        dispatch_return,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(return_parameters[1]),
            operation_value(one_value),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        dispatch_return,
        function,
        return_parameters.clone(),
        vec![one_value, is_return],
        inventory_cond(
            operation_value(is_return),
            encode_return,
            vec![
                ValueRef::Parameter(return_parameters[0]),
                ValueRef::Parameter(return_parameters[2]),
            ],
            dispatch_branch,
            dispatch_values(&return_parameters),
        ),
    );

    let branch_parameters = make_dispatch_parameters(assembler, dispatch_branch);
    let two_value = assembler.constant_ref(dispatch_branch, two, u32_type());
    let is_branch = assembler.operation(
        dispatch_branch,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(branch_parameters[1]),
            operation_value(two_value),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        dispatch_branch,
        function,
        branch_parameters.clone(),
        vec![two_value, is_branch],
        inventory_cond(
            operation_value(is_branch),
            encode_branch,
            vec![
                ValueRef::Parameter(branch_parameters[0]),
                ValueRef::Parameter(branch_parameters[3]),
                ValueRef::Parameter(branch_parameters[4]),
            ],
            dispatch_cond,
            dispatch_values(&branch_parameters),
        ),
    );

    let cond_parameters = make_dispatch_parameters(assembler, dispatch_cond);
    let three_value = assembler.constant_ref(dispatch_cond, three, u32_type());
    let is_cond = assembler.operation(
        dispatch_cond,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(cond_parameters[1]),
            operation_value(three_value),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        dispatch_cond,
        function,
        cond_parameters.clone(),
        vec![three_value, is_cond],
        inventory_cond(
            operation_value(is_cond),
            encode_cond_condition,
            vec![
                ValueRef::Parameter(cond_parameters[0]),
                ValueRef::Parameter(cond_parameters[2]),
                ValueRef::Parameter(cond_parameters[3]),
                ValueRef::Parameter(cond_parameters[4]),
                ValueRef::Parameter(cond_parameters[5]),
                ValueRef::Parameter(cond_parameters[6]),
            ],
            encode_trap_code,
            vec![
                ValueRef::Parameter(cond_parameters[0]),
                ValueRef::Parameter(cond_parameters[2]),
                ValueRef::Parameter(cond_parameters[7]),
            ],
        ),
    );

    let return_accumulator =
        assembler.parameter(encode_return, ParameterRole::Block, 0, u8vec_type());
    let return_register = assembler.parameter(encode_return, ParameterRole::Block, 1, u32_type());
    let returned = assembler.operation(
        encode_return,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(return_register),
            ValueRef::Parameter(return_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_return,
        function,
        vec![return_accumulator, return_register],
        vec![returned],
        Terminator::Return(ReturnTerminator {
            value: operation_value(returned),
        }),
    );

    let branch_accumulator =
        assembler.parameter(encode_branch, ParameterRole::Block, 0, u8vec_type());
    let branch_target = assembler.parameter(encode_branch, ParameterRole::Block, 1, u32_type());
    let branch_arguments =
        assembler.parameter(encode_branch, ParameterRole::Block, 2, u32vec_type());
    let branched = assembler.operation(
        encode_branch,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(branch_target),
            ValueRef::Parameter(branch_arguments),
            ValueRef::Parameter(branch_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_edge,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_branch,
        function,
        vec![branch_accumulator, branch_target, branch_arguments],
        vec![branched],
        Terminator::Return(ReturnTerminator {
            value: operation_value(branched),
        }),
    );

    let cond_accumulator =
        assembler.parameter(encode_cond_condition, ParameterRole::Block, 0, u8vec_type());
    let cond_value =
        assembler.parameter(encode_cond_condition, ParameterRole::Block, 1, u32_type());
    let cond_true_target =
        assembler.parameter(encode_cond_condition, ParameterRole::Block, 2, u32_type());
    let cond_true_arguments = assembler.parameter(
        encode_cond_condition,
        ParameterRole::Block,
        3,
        u32vec_type(),
    );
    let cond_false_target =
        assembler.parameter(encode_cond_condition, ParameterRole::Block, 4, u32_type());
    let cond_false_arguments = assembler.parameter(
        encode_cond_condition,
        ParameterRole::Block,
        5,
        u32vec_type(),
    );
    let encoded_condition = assembler.operation(
        encode_cond_condition,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(cond_value),
            ValueRef::Parameter(cond_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_cond_condition,
        function,
        vec![
            cond_accumulator,
            cond_value,
            cond_true_target,
            cond_true_arguments,
            cond_false_target,
            cond_false_arguments,
        ],
        vec![encoded_condition],
        inventory_switch(
            operation_value(encoded_condition),
            vec![
                (
                    BuiltinCase::Ok,
                    encode_cond_true,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(cond_true_target)),
                        SwitchArgument::Value(ValueRef::Parameter(cond_true_arguments)),
                        SwitchArgument::Value(ValueRef::Parameter(cond_false_target)),
                        SwitchArgument::Value(ValueRef::Parameter(cond_false_arguments)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let true_accumulator =
        assembler.parameter(encode_cond_true, ParameterRole::Block, 0, u8vec_type());
    let true_target = assembler.parameter(encode_cond_true, ParameterRole::Block, 1, u32_type());
    let true_arguments =
        assembler.parameter(encode_cond_true, ParameterRole::Block, 2, u32vec_type());
    let false_target = assembler.parameter(encode_cond_true, ParameterRole::Block, 3, u32_type());
    let false_arguments =
        assembler.parameter(encode_cond_true, ParameterRole::Block, 4, u32vec_type());
    let encoded_true = assembler.operation(
        encode_cond_true,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(true_target),
            ValueRef::Parameter(true_arguments),
            ValueRef::Parameter(true_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_edge,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_cond_true,
        function,
        vec![
            true_accumulator,
            true_target,
            true_arguments,
            false_target,
            false_arguments,
        ],
        vec![encoded_true],
        inventory_switch(
            operation_value(encoded_true),
            vec![
                (
                    BuiltinCase::Ok,
                    encode_cond_false,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(false_target)),
                        SwitchArgument::Value(ValueRef::Parameter(false_arguments)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let false_accumulator =
        assembler.parameter(encode_cond_false, ParameterRole::Block, 0, u8vec_type());
    let final_target = assembler.parameter(encode_cond_false, ParameterRole::Block, 1, u32_type());
    let final_arguments =
        assembler.parameter(encode_cond_false, ParameterRole::Block, 2, u32vec_type());
    let encoded_false = assembler.operation(
        encode_cond_false,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(final_target),
            ValueRef::Parameter(final_arguments),
            ValueRef::Parameter(false_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_edge,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_cond_false,
        function,
        vec![false_accumulator, final_target, final_arguments],
        vec![encoded_false],
        Terminator::Return(ReturnTerminator {
            value: operation_value(encoded_false),
        }),
    );

    let trap_accumulator =
        assembler.parameter(encode_trap_code, ParameterRole::Block, 0, u8vec_type());
    let trap_code = assembler.parameter(encode_trap_code, ParameterRole::Block, 1, u32_type());
    let trap_payload = assembler.parameter(
        encode_trap_code,
        ParameterRole::Block,
        2,
        optional_u32_type(),
    );
    let encoded_code = assembler.operation(
        encode_trap_code,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(trap_code),
            ValueRef::Parameter(trap_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_trap_code,
        function,
        vec![trap_accumulator, trap_code, trap_payload],
        vec![encoded_code],
        inventory_switch(
            operation_value(encoded_code),
            vec![
                (
                    BuiltinCase::Ok,
                    dispatch_trap_payload,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(trap_payload)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let payload_accumulator =
        assembler.parameter(dispatch_trap_payload, ParameterRole::Block, 0, u8vec_type());
    let payload_value = assembler.parameter(
        dispatch_trap_payload,
        ParameterRole::Block,
        1,
        optional_u32_type(),
    );
    assembler.push_block(
        dispatch_trap_payload,
        function,
        vec![payload_accumulator, payload_value],
        Vec::new(),
        inventory_switch(
            ValueRef::Parameter(payload_value),
            vec![
                (
                    BuiltinCase::None,
                    encode_trap_none,
                    vec![SwitchArgument::Value(ValueRef::Parameter(
                        payload_accumulator,
                    ))],
                ),
                (
                    BuiltinCase::Some,
                    encode_trap_some_tag,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(payload_accumulator)),
                        SwitchArgument::CasePayload,
                    ],
                ),
            ],
        ),
    );
    let none_accumulator =
        assembler.parameter(encode_trap_none, ParameterRole::Block, 0, u8vec_type());
    let none_tag = assembler.constant_ref(encode_trap_none, one, u32_type());
    let encoded_none = assembler.operation(
        encode_trap_none,
        Opcode::CallDirect,
        vec![
            operation_value(none_tag),
            ValueRef::Parameter(none_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_trap_none,
        function,
        vec![none_accumulator],
        vec![none_tag, encoded_none],
        Terminator::Return(ReturnTerminator {
            value: operation_value(encoded_none),
        }),
    );
    let some_accumulator =
        assembler.parameter(encode_trap_some_tag, ParameterRole::Block, 0, u8vec_type());
    let some_payload =
        assembler.parameter(encode_trap_some_tag, ParameterRole::Block, 1, u32_type());
    let some_tag = assembler.constant_ref(encode_trap_some_tag, two, u32_type());
    let encoded_some_tag = assembler.operation(
        encode_trap_some_tag,
        Opcode::CallDirect,
        vec![
            operation_value(some_tag),
            ValueRef::Parameter(some_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_trap_some_tag,
        function,
        vec![some_accumulator, some_payload],
        vec![some_tag, encoded_some_tag],
        inventory_switch(
            operation_value(encoded_some_tag),
            vec![
                (
                    BuiltinCase::Ok,
                    encode_trap_some_value,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(some_payload)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let value_accumulator = assembler.parameter(
        encode_trap_some_value,
        ParameterRole::Block,
        0,
        u8vec_type(),
    );
    let value_payload =
        assembler.parameter(encode_trap_some_value, ParameterRole::Block, 1, u32_type());
    let encoded_payload = assembler.operation(
        encode_trap_some_value,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(value_payload),
            ValueRef::Parameter(value_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_trap_some_value,
        function,
        vec![value_accumulator, value_payload],
        vec![encoded_payload],
        Terminator::Return(ReturnTerminator {
            value: operation_value(encoded_payload),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(failure),
        }),
    );
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![terminator, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn simple_terminator_byte_encoder() -> LowerScaffold {
    let narrow_u32 = inventory_id(5, 19);
    let append_u32 = inventory_id(5, 20);
    let narrow_u64 = inventory_id(5, 21);
    let append_u64 = inventory_id(5, 22);
    let append_u32_vector = inventory_id(5, 26);
    let append_edge = inventory_id(5, 31);
    let function = inventory_id(5, 32);
    let mut assembler = InventoryAssembler::new();
    let narrow_u32_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u32, &u32_type(), u32_value);
    let append_u32_graph = build_fixed_width_appender(
        &mut assembler,
        append_u32,
        narrow_u32,
        &u32_type(),
        u32_value,
        &[1_u128 << 24, 1_u128 << 16, 1_u128 << 8, 1],
    );
    let narrow_u64_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u64, &u64_type(), u64_value);
    let append_u64_graph = build_fixed_width_appender(
        &mut assembler,
        append_u64,
        narrow_u64,
        &u64_type(),
        u64_value,
        &[
            1_u128 << 56,
            1_u128 << 48,
            1_u128 << 40,
            1_u128 << 32,
            1_u128 << 24,
            1_u128 << 16,
            1_u128 << 8,
            1,
        ],
    );
    let append_u32_vector_graph = build_vector_byte_appender(
        &mut assembler,
        append_u32_vector,
        append_u64,
        append_u32,
        &u32vec_type(),
        &u32_type(),
        true,
        true,
    );
    let append_edge_graph =
        build_target_edge_byte_appender(&mut assembler, append_edge, append_u32, append_u32_vector);
    let graph =
        build_simple_terminator_byte_appender(&mut assembler, function, append_u32, append_edge);
    let adapter = AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        )),
        adapter_id: sley_vm::host_abi::bridge_identity(sley_vm::host_abi::BRIDGE_CODE_PSH1),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type: u8_type(),
        response_type: u8vec_type(),
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            narrow_u32_graph,
            append_u32_graph,
            narrow_u64_graph,
            append_u64_graph,
            append_u32_vector_graph,
            append_edge_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![adapter],
    }
}

#[allow(clippy::too_many_lines)]
fn build_switch_arguments_byte_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_u32: EntityId,
    append_u64: EntityId,
) -> FunctionGraph {
    let arguments = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        switch_argument_facts_type(),
    );
    let accumulator = assembler.parameter(function, ParameterRole::Function, 1, u8vec_type());
    let entry = assembler.block_id();
    let initialize = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let unpack = assembler.block_id();
    let dispatch_value = assembler.block_id();
    let append_value = assembler.block_id();
    let advance = assembler.block_id();
    let done = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();
    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let one_u32 = assembler.constant(u32_value(1));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(arguments)],
        u64_type(),
        Immediate::None,
    );
    let encoded_length = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![operation_value(length), ValueRef::Parameter(accumulator)],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u64,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![length, encoded_length],
        inventory_switch(
            operation_value(encoded_length),
            vec![
                (
                    BuiltinCase::Ok,
                    initialize,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(operation_value(length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let initial_accumulator =
        assembler.parameter(initialize, ParameterRole::Block, 0, u8vec_type());
    let initial_length = assembler.parameter(initialize, ParameterRole::Block, 1, u64_type());
    let zero = assembler.constant_ref(initialize, zero_u64, u64_type());
    assembler.push_block(
        initialize,
        function,
        vec![initial_accumulator, initial_length],
        vec![zero],
        inventory_branch(
            check,
            vec![
                operation_value(zero),
                ValueRef::Parameter(initial_accumulator),
                ValueRef::Parameter(initial_length),
            ],
        ),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let check_accumulator = assembler.parameter(check, ParameterRole::Block, 1, u8vec_type());
    let check_length = assembler.parameter(check, ParameterRole::Block, 2, u64_type());
    let has_argument = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_index),
            ValueRef::Parameter(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        vec![check_index, check_accumulator, check_length],
        vec![has_argument],
        inventory_cond(
            operation_value(has_argument),
            get,
            vec![
                ValueRef::Parameter(check_index),
                ValueRef::Parameter(check_accumulator),
                ValueRef::Parameter(check_length),
            ],
            done,
            vec![ValueRef::Parameter(check_accumulator)],
        ),
    );

    let get_index = assembler.parameter(get, ParameterRole::Block, 0, u64_type());
    let get_accumulator = assembler.parameter(get, ParameterRole::Block, 1, u8vec_type());
    let get_length = assembler.parameter(get, ParameterRole::Block, 2, u64_type());
    let found = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(arguments),
            ValueRef::Parameter(get_index),
        ],
        TypeExpr::Option(Box::new(switch_argument_fact_type())),
        Immediate::None,
    );
    assembler.push_block(
        get,
        function,
        vec![get_index, get_accumulator, get_length],
        vec![found],
        inventory_switch(
            operation_value(found),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                        SwitchArgument::Value(ValueRef::Parameter(get_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(get_length)),
                    ],
                ),
            ],
        ),
    );

    let argument =
        assembler.parameter(unpack, ParameterRole::Block, 0, switch_argument_fact_type());
    let unpack_index = assembler.parameter(unpack, ParameterRole::Block, 1, u64_type());
    let unpack_accumulator = assembler.parameter(unpack, ParameterRole::Block, 2, u8vec_type());
    let unpack_length = assembler.parameter(unpack, ParameterRole::Block, 3, u64_type());
    let tag = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(argument)],
        u32_type(),
        Immediate::Index(0),
    );
    let value = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(argument)],
        u32_type(),
        Immediate::Index(1),
    );
    let encoded_tag = assembler.operation(
        unpack,
        Opcode::CallDirect,
        vec![
            operation_value(tag),
            ValueRef::Parameter(unpack_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        unpack,
        function,
        vec![argument, unpack_index, unpack_accumulator, unpack_length],
        vec![tag, value, encoded_tag],
        inventory_switch(
            operation_value(encoded_tag),
            vec![
                (
                    BuiltinCase::Ok,
                    dispatch_value,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(operation_value(tag)),
                        SwitchArgument::Value(operation_value(value)),
                        SwitchArgument::Value(ValueRef::Parameter(unpack_index)),
                        SwitchArgument::Value(ValueRef::Parameter(unpack_length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let dispatch_accumulator =
        assembler.parameter(dispatch_value, ParameterRole::Block, 0, u8vec_type());
    let dispatch_tag = assembler.parameter(dispatch_value, ParameterRole::Block, 1, u32_type());
    let dispatch_register =
        assembler.parameter(dispatch_value, ParameterRole::Block, 2, u32_type());
    let dispatch_index = assembler.parameter(dispatch_value, ParameterRole::Block, 3, u64_type());
    let dispatch_length = assembler.parameter(dispatch_value, ParameterRole::Block, 4, u64_type());
    let value_tag = assembler.constant_ref(dispatch_value, one_u32, u32_type());
    let has_register = assembler.operation(
        dispatch_value,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(dispatch_tag),
            operation_value(value_tag),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        dispatch_value,
        function,
        vec![
            dispatch_accumulator,
            dispatch_tag,
            dispatch_register,
            dispatch_index,
            dispatch_length,
        ],
        vec![value_tag, has_register],
        inventory_cond(
            operation_value(has_register),
            append_value,
            vec![
                ValueRef::Parameter(dispatch_accumulator),
                ValueRef::Parameter(dispatch_register),
                ValueRef::Parameter(dispatch_index),
                ValueRef::Parameter(dispatch_length),
            ],
            advance,
            vec![
                ValueRef::Parameter(dispatch_index),
                ValueRef::Parameter(dispatch_accumulator),
                ValueRef::Parameter(dispatch_length),
            ],
        ),
    );

    let value_accumulator =
        assembler.parameter(append_value, ParameterRole::Block, 0, u8vec_type());
    let value_register = assembler.parameter(append_value, ParameterRole::Block, 1, u32_type());
    let value_index = assembler.parameter(append_value, ParameterRole::Block, 2, u64_type());
    let value_length = assembler.parameter(append_value, ParameterRole::Block, 3, u64_type());
    let encoded_value = assembler.operation(
        append_value,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(value_register),
            ValueRef::Parameter(value_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_value,
        function,
        vec![value_accumulator, value_register, value_index, value_length],
        vec![encoded_value],
        inventory_switch(
            operation_value(encoded_value),
            vec![
                (
                    BuiltinCase::Ok,
                    advance,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(value_index)),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(value_length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let advance_index = assembler.parameter(advance, ParameterRole::Block, 0, u64_type());
    let advance_accumulator = assembler.parameter(advance, ParameterRole::Block, 1, u8vec_type());
    let advance_length = assembler.parameter(advance, ParameterRole::Block, 2, u64_type());
    let one = assembler.constant_ref(advance, one_u64, u64_type());
    let next_index = assembler.operation(
        advance,
        Opcode::IntAddChecked,
        vec![ValueRef::Parameter(advance_index), operation_value(one)],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance,
        function,
        vec![advance_index, advance_accumulator, advance_length],
        vec![one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let completed = assembler.parameter(done, ParameterRole::Block, 0, u8vec_type());
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(completed)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![completed],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![arguments, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

#[allow(clippy::too_many_lines)]
fn build_switch_cases_byte_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_u32: EntityId,
    append_u64: EntityId,
    append_arguments: EntityId,
) -> FunctionGraph {
    let cases = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        builtin_switch_case_facts_type(),
    );
    let accumulator = assembler.parameter(function, ParameterRole::Function, 1, u8vec_type());
    let entry = assembler.block_id();
    let initialize = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let unpack = assembler.block_id();
    let append_key = assembler.block_id();
    let append_target = assembler.block_id();
    let append_case_arguments = assembler.block_id();
    let advance = assembler.block_id();
    let done = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();
    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let builtin_key_tag = assembler.constant(u32_value(2));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(cases)],
        u64_type(),
        Immediate::None,
    );
    let encoded_length = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![operation_value(length), ValueRef::Parameter(accumulator)],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u64,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![length, encoded_length],
        inventory_switch(
            operation_value(encoded_length),
            vec![
                (
                    BuiltinCase::Ok,
                    initialize,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(operation_value(length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let initial_accumulator =
        assembler.parameter(initialize, ParameterRole::Block, 0, u8vec_type());
    let initial_length = assembler.parameter(initialize, ParameterRole::Block, 1, u64_type());
    let zero = assembler.constant_ref(initialize, zero_u64, u64_type());
    assembler.push_block(
        initialize,
        function,
        vec![initial_accumulator, initial_length],
        vec![zero],
        inventory_branch(
            check,
            vec![
                operation_value(zero),
                ValueRef::Parameter(initial_accumulator),
                ValueRef::Parameter(initial_length),
            ],
        ),
    );
    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let check_accumulator = assembler.parameter(check, ParameterRole::Block, 1, u8vec_type());
    let check_length = assembler.parameter(check, ParameterRole::Block, 2, u64_type());
    let has_case = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_index),
            ValueRef::Parameter(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        vec![check_index, check_accumulator, check_length],
        vec![has_case],
        inventory_cond(
            operation_value(has_case),
            get,
            vec![
                ValueRef::Parameter(check_index),
                ValueRef::Parameter(check_accumulator),
                ValueRef::Parameter(check_length),
            ],
            done,
            vec![ValueRef::Parameter(check_accumulator)],
        ),
    );
    let get_index = assembler.parameter(get, ParameterRole::Block, 0, u64_type());
    let get_accumulator = assembler.parameter(get, ParameterRole::Block, 1, u8vec_type());
    let get_length = assembler.parameter(get, ParameterRole::Block, 2, u64_type());
    let found = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![ValueRef::Parameter(cases), ValueRef::Parameter(get_index)],
        TypeExpr::Option(Box::new(builtin_switch_case_fact_type())),
        Immediate::None,
    );
    assembler.push_block(
        get,
        function,
        vec![get_index, get_accumulator, get_length],
        vec![found],
        inventory_switch(
            operation_value(found),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                        SwitchArgument::Value(ValueRef::Parameter(get_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(get_length)),
                    ],
                ),
            ],
        ),
    );
    let case = assembler.parameter(
        unpack,
        ParameterRole::Block,
        0,
        builtin_switch_case_fact_type(),
    );
    let unpack_index = assembler.parameter(unpack, ParameterRole::Block, 1, u64_type());
    let unpack_accumulator = assembler.parameter(unpack, ParameterRole::Block, 2, u8vec_type());
    let unpack_length = assembler.parameter(unpack, ParameterRole::Block, 3, u64_type());
    let key = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(case)],
        u32_type(),
        Immediate::Index(0),
    );
    let target = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(case)],
        u32_type(),
        Immediate::Index(1),
    );
    let arguments = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(case)],
        switch_argument_facts_type(),
        Immediate::Index(2),
    );
    let key_tag = assembler.constant_ref(unpack, builtin_key_tag, u32_type());
    let encoded_key_tag = assembler.operation(
        unpack,
        Opcode::CallDirect,
        vec![
            operation_value(key_tag),
            ValueRef::Parameter(unpack_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        unpack,
        function,
        vec![case, unpack_index, unpack_accumulator, unpack_length],
        vec![key, target, arguments, key_tag, encoded_key_tag],
        inventory_switch(
            operation_value(encoded_key_tag),
            vec![
                (
                    BuiltinCase::Ok,
                    append_key,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(operation_value(key)),
                        SwitchArgument::Value(operation_value(target)),
                        SwitchArgument::Value(operation_value(arguments)),
                        SwitchArgument::Value(ValueRef::Parameter(unpack_index)),
                        SwitchArgument::Value(ValueRef::Parameter(unpack_length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let key_accumulator = assembler.parameter(append_key, ParameterRole::Block, 0, u8vec_type());
    let key_value = assembler.parameter(append_key, ParameterRole::Block, 1, u32_type());
    let key_target = assembler.parameter(append_key, ParameterRole::Block, 2, u32_type());
    let key_arguments = assembler.parameter(
        append_key,
        ParameterRole::Block,
        3,
        switch_argument_facts_type(),
    );
    let key_index = assembler.parameter(append_key, ParameterRole::Block, 4, u64_type());
    let key_length = assembler.parameter(append_key, ParameterRole::Block, 5, u64_type());
    let encoded_key = assembler.operation(
        append_key,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(key_value),
            ValueRef::Parameter(key_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_key,
        function,
        vec![
            key_accumulator,
            key_value,
            key_target,
            key_arguments,
            key_index,
            key_length,
        ],
        vec![encoded_key],
        inventory_switch(
            operation_value(encoded_key),
            vec![
                (
                    BuiltinCase::Ok,
                    append_target,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(key_target)),
                        SwitchArgument::Value(ValueRef::Parameter(key_arguments)),
                        SwitchArgument::Value(ValueRef::Parameter(key_index)),
                        SwitchArgument::Value(ValueRef::Parameter(key_length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let target_accumulator =
        assembler.parameter(append_target, ParameterRole::Block, 0, u8vec_type());
    let target_value = assembler.parameter(append_target, ParameterRole::Block, 1, u32_type());
    let target_arguments = assembler.parameter(
        append_target,
        ParameterRole::Block,
        2,
        switch_argument_facts_type(),
    );
    let target_index = assembler.parameter(append_target, ParameterRole::Block, 3, u64_type());
    let target_length = assembler.parameter(append_target, ParameterRole::Block, 4, u64_type());
    let encoded_target = assembler.operation(
        append_target,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(target_value),
            ValueRef::Parameter(target_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_target,
        function,
        vec![
            target_accumulator,
            target_value,
            target_arguments,
            target_index,
            target_length,
        ],
        vec![encoded_target],
        inventory_switch(
            operation_value(encoded_target),
            vec![
                (
                    BuiltinCase::Ok,
                    append_case_arguments,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(target_arguments)),
                        SwitchArgument::Value(ValueRef::Parameter(target_index)),
                        SwitchArgument::Value(ValueRef::Parameter(target_length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let arguments_accumulator =
        assembler.parameter(append_case_arguments, ParameterRole::Block, 0, u8vec_type());
    let arguments_value = assembler.parameter(
        append_case_arguments,
        ParameterRole::Block,
        1,
        switch_argument_facts_type(),
    );
    let arguments_index =
        assembler.parameter(append_case_arguments, ParameterRole::Block, 2, u64_type());
    let arguments_length =
        assembler.parameter(append_case_arguments, ParameterRole::Block, 3, u64_type());
    let encoded_arguments = assembler.operation(
        append_case_arguments,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(arguments_value),
            ValueRef::Parameter(arguments_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_arguments,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_case_arguments,
        function,
        vec![
            arguments_accumulator,
            arguments_value,
            arguments_index,
            arguments_length,
        ],
        vec![encoded_arguments],
        inventory_switch(
            operation_value(encoded_arguments),
            vec![
                (
                    BuiltinCase::Ok,
                    advance,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(arguments_index)),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(arguments_length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let advance_index = assembler.parameter(advance, ParameterRole::Block, 0, u64_type());
    let advance_accumulator = assembler.parameter(advance, ParameterRole::Block, 1, u8vec_type());
    let advance_length = assembler.parameter(advance, ParameterRole::Block, 2, u64_type());
    let one = assembler.constant_ref(advance, one_u64, u64_type());
    let next_index = assembler.operation(
        advance,
        Opcode::IntAddChecked,
        vec![ValueRef::Parameter(advance_index), operation_value(one)],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance,
        function,
        vec![advance_index, advance_accumulator, advance_length],
        vec![one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );
    let completed = assembler.parameter(done, ParameterRole::Block, 0, u8vec_type());
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(completed)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![completed],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![cases, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

#[allow(clippy::too_many_lines)]
fn build_builtin_switch_terminator_byte_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_u32: EntityId,
    append_cases: EntityId,
) -> FunctionGraph {
    let model = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        builtin_switch_model_type(),
    );
    let accumulator = assembler.parameter(function, ParameterRole::Function, 1, u8vec_type());
    let entry = assembler.block_id();
    let append_selector = assembler.block_id();
    let append_cases_block = assembler.block_id();
    let forward_error = assembler.block_id();
    let kind = assembler.constant(u32_value(4));
    let selector = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(model)],
        u32_type(),
        Immediate::Index(0),
    );
    let cases = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(model)],
        builtin_switch_case_facts_type(),
        Immediate::Index(1),
    );
    let kind_value = assembler.constant_ref(entry, kind, u32_type());
    let encoded_kind = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![
            operation_value(kind_value),
            ValueRef::Parameter(accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![selector, cases, kind_value, encoded_kind],
        inventory_switch(
            operation_value(encoded_kind),
            vec![
                (
                    BuiltinCase::Ok,
                    append_selector,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(operation_value(selector)),
                        SwitchArgument::Value(operation_value(cases)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let selector_accumulator =
        assembler.parameter(append_selector, ParameterRole::Block, 0, u8vec_type());
    let selector_value = assembler.parameter(append_selector, ParameterRole::Block, 1, u32_type());
    let selector_cases = assembler.parameter(
        append_selector,
        ParameterRole::Block,
        2,
        builtin_switch_case_facts_type(),
    );
    let encoded_selector = assembler.operation(
        append_selector,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(selector_value),
            ValueRef::Parameter(selector_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_selector,
        function,
        vec![selector_accumulator, selector_value, selector_cases],
        vec![encoded_selector],
        inventory_switch(
            operation_value(encoded_selector),
            vec![
                (
                    BuiltinCase::Ok,
                    append_cases_block,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(selector_cases)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let cases_accumulator =
        assembler.parameter(append_cases_block, ParameterRole::Block, 0, u8vec_type());
    let cases_value = assembler.parameter(
        append_cases_block,
        ParameterRole::Block,
        1,
        builtin_switch_case_facts_type(),
    );
    let encoded_cases = assembler.operation(
        append_cases_block,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(cases_value),
            ValueRef::Parameter(cases_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_cases,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_cases_block,
        function,
        vec![cases_accumulator, cases_value],
        vec![encoded_cases],
        Terminator::Return(ReturnTerminator {
            value: operation_value(encoded_cases),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(failure),
        }),
    );
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![model, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn builtin_switch_terminator_byte_encoder() -> LowerScaffold {
    let narrow_u32 = inventory_id(5, 19);
    let append_u32 = inventory_id(5, 20);
    let narrow_u64 = inventory_id(5, 21);
    let append_u64 = inventory_id(5, 22);
    let append_arguments = inventory_id(5, 33);
    let append_cases = inventory_id(5, 34);
    let function = inventory_id(5, 35);
    let mut assembler = InventoryAssembler::new();
    let narrow_u32_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u32, &u32_type(), u32_value);
    let append_u32_graph = build_fixed_width_appender(
        &mut assembler,
        append_u32,
        narrow_u32,
        &u32_type(),
        u32_value,
        &[1_u128 << 24, 1_u128 << 16, 1_u128 << 8, 1],
    );
    let narrow_u64_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u64, &u64_type(), u64_value);
    let append_u64_graph = build_fixed_width_appender(
        &mut assembler,
        append_u64,
        narrow_u64,
        &u64_type(),
        u64_value,
        &[
            1_u128 << 56,
            1_u128 << 48,
            1_u128 << 40,
            1_u128 << 32,
            1_u128 << 24,
            1_u128 << 16,
            1_u128 << 8,
            1,
        ],
    );
    let append_arguments_graph = build_switch_arguments_byte_appender(
        &mut assembler,
        append_arguments,
        append_u32,
        append_u64,
    );
    let append_cases_graph = build_switch_cases_byte_appender(
        &mut assembler,
        append_cases,
        append_u32,
        append_u64,
        append_arguments,
    );
    let graph = build_builtin_switch_terminator_byte_appender(
        &mut assembler,
        function,
        append_u32,
        append_cases,
    );
    let adapter = AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        )),
        adapter_id: sley_vm::host_abi::bridge_identity(sley_vm::host_abi::BRIDGE_CODE_PSH1),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type: u8_type(),
        response_type: u8vec_type(),
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            narrow_u32_graph,
            append_u32_graph,
            narrow_u64_graph,
            append_u64_graph,
            append_arguments_graph,
            append_cases_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![adapter],
    }
}

fn build_complete_terminator_byte_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_simple: EntityId,
    append_switch: EntityId,
) -> FunctionGraph {
    let terminator = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        complete_terminator_model_type(),
    );
    let accumulator = assembler.parameter(function, ParameterRole::Function, 1, u8vec_type());
    let entry = assembler.block_id();
    let encode_simple = assembler.block_id();
    let encode_switch = assembler.block_id();
    let switch_kind = assembler.constant(u32_value(4));
    let kind = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(terminator)],
        u32_type(),
        Immediate::Index(0),
    );
    let simple = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(terminator)],
        terminator_model_type(),
        Immediate::Index(1),
    );
    let switch = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(terminator)],
        builtin_switch_model_type(),
        Immediate::Index(2),
    );
    let switch_tag = assembler.constant_ref(entry, switch_kind, u32_type());
    let is_switch = assembler.operation(
        entry,
        Opcode::Equal,
        vec![operation_value(kind), operation_value(switch_tag)],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![kind, simple, switch, switch_tag, is_switch],
        inventory_cond(
            operation_value(is_switch),
            encode_switch,
            vec![operation_value(switch), ValueRef::Parameter(accumulator)],
            encode_simple,
            vec![operation_value(simple), ValueRef::Parameter(accumulator)],
        ),
    );
    for (block, model_type, callee) in [
        (encode_simple, terminator_model_type(), append_simple),
        (encode_switch, builtin_switch_model_type(), append_switch),
    ] {
        let model = assembler.parameter(block, ParameterRole::Block, 0, model_type);
        let bytes = assembler.parameter(block, ParameterRole::Block, 1, u8vec_type());
        let encoded = assembler.operation(
            block,
            Opcode::CallDirect,
            vec![ValueRef::Parameter(model), ValueRef::Parameter(bytes)],
            byte_vector_lower_result_type(),
            Immediate::Function(FunctionRefValue {
                function: callee,
                type_arguments: Vec::new(),
            }),
        );
        assembler.push_block(
            block,
            function,
            vec![model, bytes],
            vec![encoded],
            Terminator::Return(ReturnTerminator {
                value: operation_value(encoded),
            }),
        );
    }
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![terminator, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

#[allow(clippy::too_many_lines)]
fn complete_terminator_byte_encoder() -> LowerScaffold {
    let narrow_u32 = inventory_id(5, 19);
    let append_u32 = inventory_id(5, 20);
    let narrow_u64 = inventory_id(5, 21);
    let append_u64 = inventory_id(5, 22);
    let append_u32_vector = inventory_id(5, 26);
    let append_edge = inventory_id(5, 31);
    let append_simple = inventory_id(5, 32);
    let append_arguments = inventory_id(5, 33);
    let append_cases = inventory_id(5, 34);
    let append_switch = inventory_id(5, 35);
    let function = inventory_id(5, 36);
    let mut assembler = InventoryAssembler::new();
    let narrow_u32_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u32, &u32_type(), u32_value);
    let append_u32_graph = build_fixed_width_appender(
        &mut assembler,
        append_u32,
        narrow_u32,
        &u32_type(),
        u32_value,
        &[1_u128 << 24, 1_u128 << 16, 1_u128 << 8, 1],
    );
    let narrow_u64_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u64, &u64_type(), u64_value);
    let append_u64_graph = build_fixed_width_appender(
        &mut assembler,
        append_u64,
        narrow_u64,
        &u64_type(),
        u64_value,
        &[
            1_u128 << 56,
            1_u128 << 48,
            1_u128 << 40,
            1_u128 << 32,
            1_u128 << 24,
            1_u128 << 16,
            1_u128 << 8,
            1,
        ],
    );
    let append_u32_vector_graph = build_vector_byte_appender(
        &mut assembler,
        append_u32_vector,
        append_u64,
        append_u32,
        &u32vec_type(),
        &u32_type(),
        true,
        true,
    );
    let append_edge_graph =
        build_target_edge_byte_appender(&mut assembler, append_edge, append_u32, append_u32_vector);
    let append_simple_graph = build_simple_terminator_byte_appender(
        &mut assembler,
        append_simple,
        append_u32,
        append_edge,
    );
    let append_arguments_graph = build_switch_arguments_byte_appender(
        &mut assembler,
        append_arguments,
        append_u32,
        append_u64,
    );
    let append_cases_graph = build_switch_cases_byte_appender(
        &mut assembler,
        append_cases,
        append_u32,
        append_u64,
        append_arguments,
    );
    let append_switch_graph = build_builtin_switch_terminator_byte_appender(
        &mut assembler,
        append_switch,
        append_u32,
        append_cases,
    );
    let graph = build_complete_terminator_byte_appender(
        &mut assembler,
        function,
        append_simple,
        append_switch,
    );
    let adapter = AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        )),
        adapter_id: sley_vm::host_abi::bridge_identity(sley_vm::host_abi::BRIDGE_CODE_PSH1),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type: u8_type(),
        response_type: u8vec_type(),
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            narrow_u32_graph,
            append_u32_graph,
            narrow_u64_graph,
            append_u64_graph,
            append_u32_vector_graph,
            append_edge_graph,
            append_simple_graph,
            append_arguments_graph,
            append_cases_graph,
            append_switch_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![adapter],
    }
}

#[allow(clippy::too_many_lines)]
fn build_complete_block_byte_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_u32: EntityId,
    append_u32_vector: EntityId,
    append_instruction_map: EntityId,
    append_terminator: EntityId,
) -> FunctionGraph {
    let block = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        complete_block_model_type(),
    );
    let instruction_count = assembler.parameter(function, ParameterRole::Function, 1, u64_type());
    let accumulator = assembler.parameter(function, ParameterRole::Function, 2, u8vec_type());
    let entry = assembler.block_id();
    let append_parameters = assembler.block_id();
    let append_instructions = assembler.block_id();
    let append_terminator_block = assembler.block_id();
    let append_reachability = assembler.block_id();
    let forward_error = assembler.block_id();

    let slot = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(block)],
        u32_type(),
        Immediate::Index(0),
    );
    let parameters = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(block)],
        u32vec_type(),
        Immediate::Index(1),
    );
    let instructions = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(block)],
        immediate_inventory_model_type(),
        Immediate::Index(2),
    );
    let terminator = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(block)],
        complete_terminator_model_type(),
        Immediate::Index(3),
    );
    let reachability = assembler.operation(
        entry,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(block)],
        u32_type(),
        Immediate::Index(4),
    );
    let encoded_slot = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![operation_value(slot), ValueRef::Parameter(accumulator)],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![
            slot,
            parameters,
            instructions,
            terminator,
            reachability,
            encoded_slot,
        ],
        inventory_switch(
            operation_value(encoded_slot),
            vec![
                (
                    BuiltinCase::Ok,
                    append_parameters,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(operation_value(parameters)),
                        SwitchArgument::Value(operation_value(instructions)),
                        SwitchArgument::Value(operation_value(terminator)),
                        SwitchArgument::Value(operation_value(reachability)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let parameter_accumulator =
        assembler.parameter(append_parameters, ParameterRole::Block, 0, u8vec_type());
    let parameter_values =
        assembler.parameter(append_parameters, ParameterRole::Block, 1, u32vec_type());
    let parameter_instructions = assembler.parameter(
        append_parameters,
        ParameterRole::Block,
        2,
        immediate_inventory_model_type(),
    );
    let parameter_terminator = assembler.parameter(
        append_parameters,
        ParameterRole::Block,
        3,
        complete_terminator_model_type(),
    );
    let parameter_reachability =
        assembler.parameter(append_parameters, ParameterRole::Block, 4, u32_type());
    let encoded_parameters = assembler.operation(
        append_parameters,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(parameter_values),
            ValueRef::Parameter(parameter_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32_vector,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_parameters,
        function,
        vec![
            parameter_accumulator,
            parameter_values,
            parameter_instructions,
            parameter_terminator,
            parameter_reachability,
        ],
        vec![encoded_parameters],
        inventory_switch(
            operation_value(encoded_parameters),
            vec![
                (
                    BuiltinCase::Ok,
                    append_instructions,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(parameter_instructions)),
                        SwitchArgument::Value(ValueRef::Parameter(parameter_terminator)),
                        SwitchArgument::Value(ValueRef::Parameter(parameter_reachability)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let instruction_accumulator =
        assembler.parameter(append_instructions, ParameterRole::Block, 0, u8vec_type());
    let instruction_values = assembler.parameter(
        append_instructions,
        ParameterRole::Block,
        1,
        immediate_inventory_model_type(),
    );
    let instruction_terminator = assembler.parameter(
        append_instructions,
        ParameterRole::Block,
        2,
        complete_terminator_model_type(),
    );
    let instruction_reachability =
        assembler.parameter(append_instructions, ParameterRole::Block, 3, u32_type());
    let encoded_instructions = assembler.operation(
        append_instructions,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(instruction_values),
            ValueRef::Parameter(instruction_count),
            ValueRef::Parameter(instruction_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_instruction_map,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_instructions,
        function,
        vec![
            instruction_accumulator,
            instruction_values,
            instruction_terminator,
            instruction_reachability,
        ],
        vec![encoded_instructions],
        inventory_switch(
            operation_value(encoded_instructions),
            vec![
                (
                    BuiltinCase::Ok,
                    append_terminator_block,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(instruction_terminator)),
                        SwitchArgument::Value(ValueRef::Parameter(instruction_reachability)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let terminator_accumulator = assembler.parameter(
        append_terminator_block,
        ParameterRole::Block,
        0,
        u8vec_type(),
    );
    let terminator_value = assembler.parameter(
        append_terminator_block,
        ParameterRole::Block,
        1,
        complete_terminator_model_type(),
    );
    let terminator_reachability =
        assembler.parameter(append_terminator_block, ParameterRole::Block, 2, u32_type());
    let encoded_terminator = assembler.operation(
        append_terminator_block,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(terminator_value),
            ValueRef::Parameter(terminator_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_terminator,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_terminator_block,
        function,
        vec![
            terminator_accumulator,
            terminator_value,
            terminator_reachability,
        ],
        vec![encoded_terminator],
        inventory_switch(
            operation_value(encoded_terminator),
            vec![
                (
                    BuiltinCase::Ok,
                    append_reachability,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(terminator_reachability)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let reachability_accumulator =
        assembler.parameter(append_reachability, ParameterRole::Block, 0, u8vec_type());
    let reachability_value =
        assembler.parameter(append_reachability, ParameterRole::Block, 1, u32_type());
    let encoded_reachability = assembler.operation(
        append_reachability,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(reachability_value),
            ValueRef::Parameter(reachability_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_reachability,
        function,
        vec![reachability_accumulator, reachability_value],
        vec![encoded_reachability],
        Terminator::Return(ReturnTerminator {
            value: operation_value(encoded_reachability),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(failure),
        }),
    );
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![block, instruction_count, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

#[allow(clippy::too_many_lines)]
fn complete_block_byte_encoder() -> LowerScaffold {
    let narrow_u32 = inventory_id(5, 19);
    let append_u32 = inventory_id(5, 20);
    let narrow_u64 = inventory_id(5, 21);
    let append_u64 = inventory_id(5, 22);
    let append_chunk = inventory_id(5, 24);
    let append_u32_vector = inventory_id(5, 26);
    let append_instruction = inventory_id(5, 29);
    let append_instruction_map = inventory_id(5, 30);
    let append_edge = inventory_id(5, 31);
    let append_simple = inventory_id(5, 32);
    let append_arguments = inventory_id(5, 33);
    let append_cases = inventory_id(5, 34);
    let append_switch = inventory_id(5, 35);
    let append_terminator = inventory_id(5, 36);
    let function = inventory_id(5, 37);
    let mut assembler = InventoryAssembler::new();
    let narrow_u32_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u32, &u32_type(), u32_value);
    let append_u32_graph = build_fixed_width_appender(
        &mut assembler,
        append_u32,
        narrow_u32,
        &u32_type(),
        u32_value,
        &[1_u128 << 24, 1_u128 << 16, 1_u128 << 8, 1],
    );
    let narrow_u64_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u64, &u64_type(), u64_value);
    let append_u64_graph = build_fixed_width_appender(
        &mut assembler,
        append_u64,
        narrow_u64,
        &u64_type(),
        u64_value,
        &[
            1_u128 << 56,
            1_u128 << 48,
            1_u128 << 40,
            1_u128 << 32,
            1_u128 << 24,
            1_u128 << 16,
            1_u128 << 8,
            1,
        ],
    );
    let append_chunk_graph = build_byte_chunk_appender(&mut assembler, append_chunk);
    let append_u32_vector_graph = build_vector_byte_appender(
        &mut assembler,
        append_u32_vector,
        append_u64,
        append_u32,
        &u32vec_type(),
        &u32_type(),
        true,
        true,
    );
    let append_instruction_graph = build_instruction_byte_appender(
        &mut assembler,
        append_instruction,
        append_u32,
        append_u32_vector,
        append_chunk,
    );
    let append_instruction_map_graph = build_instruction_map_byte_appender(
        &mut assembler,
        append_instruction_map,
        append_u64,
        append_instruction,
    );
    let append_edge_graph =
        build_target_edge_byte_appender(&mut assembler, append_edge, append_u32, append_u32_vector);
    let append_simple_graph = build_simple_terminator_byte_appender(
        &mut assembler,
        append_simple,
        append_u32,
        append_edge,
    );
    let append_arguments_graph = build_switch_arguments_byte_appender(
        &mut assembler,
        append_arguments,
        append_u32,
        append_u64,
    );
    let append_cases_graph = build_switch_cases_byte_appender(
        &mut assembler,
        append_cases,
        append_u32,
        append_u64,
        append_arguments,
    );
    let append_switch_graph = build_builtin_switch_terminator_byte_appender(
        &mut assembler,
        append_switch,
        append_u32,
        append_cases,
    );
    let append_terminator_graph = build_complete_terminator_byte_appender(
        &mut assembler,
        append_terminator,
        append_simple,
        append_switch,
    );
    let graph = build_complete_block_byte_appender(
        &mut assembler,
        function,
        append_u32,
        append_u32_vector,
        append_instruction_map,
        append_terminator,
    );
    let adapter = |code, request_type, response_type| AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(code)),
        adapter_id: sley_vm::host_abi::bridge_identity(code),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type,
        response_type,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            narrow_u32_graph,
            append_u32_graph,
            narrow_u64_graph,
            append_u64_graph,
            append_chunk_graph,
            append_u32_vector_graph,
            append_instruction_graph,
            append_instruction_map_graph,
            append_edge_graph,
            append_simple_graph,
            append_arguments_graph,
            append_cases_graph,
            append_switch_graph,
            append_terminator_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![
            adapter(
                sley_vm::host_abi::BRIDGE_CODE_B2V1,
                TypeExpr::Bytes,
                u8vec_type(),
            ),
            adapter(sley_vm::host_abi::BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
        ],
    }
}

#[allow(clippy::too_many_lines)]
fn build_complete_block_map_byte_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    append_block: EntityId,
) -> FunctionGraph {
    let blocks = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        complete_block_map_type(),
    );
    let facts = assembler.parameter(
        function,
        ParameterRole::Function,
        1,
        complete_block_facts_type(),
    );
    let accumulator = assembler.parameter(function, ParameterRole::Function, 2, u8vec_type());
    let entry = assembler.block_id();
    let check = assembler.block_id();
    let get_fact = assembler.block_id();
    let get_block = assembler.block_id();
    let append = assembler.block_id();
    let advance_index = assembler.block_id();
    let advance_slot = assembler.block_id();
    let done = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();
    let zero_u64 = assembler.constant(u64_value(0));
    let zero_u32 = assembler.constant(u32_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let one_u32 = assembler.constant(u32_value(1));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let zero_index = assembler.constant_ref(entry, zero_u64, u64_type());
    let zero_slot = assembler.constant_ref(entry, zero_u32, u32_type());
    let length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(facts)],
        u64_type(),
        Immediate::None,
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![zero_index, zero_slot, length],
        inventory_branch(
            check,
            vec![
                operation_value(zero_index),
                operation_value(zero_slot),
                ValueRef::Parameter(accumulator),
                operation_value(length),
            ],
        ),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let check_slot = assembler.parameter(check, ParameterRole::Block, 1, u32_type());
    let check_accumulator = assembler.parameter(check, ParameterRole::Block, 2, u8vec_type());
    let check_length = assembler.parameter(check, ParameterRole::Block, 3, u64_type());
    let has_block = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_index),
            ValueRef::Parameter(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        vec![check_index, check_slot, check_accumulator, check_length],
        vec![has_block],
        inventory_cond(
            operation_value(has_block),
            get_fact,
            vec![
                ValueRef::Parameter(check_index),
                ValueRef::Parameter(check_slot),
                ValueRef::Parameter(check_accumulator),
                ValueRef::Parameter(check_length),
            ],
            done,
            vec![ValueRef::Parameter(check_accumulator)],
        ),
    );

    let fact_index = assembler.parameter(get_fact, ParameterRole::Block, 0, u64_type());
    let fact_slot = assembler.parameter(get_fact, ParameterRole::Block, 1, u32_type());
    let fact_accumulator = assembler.parameter(get_fact, ParameterRole::Block, 2, u8vec_type());
    let fact_length = assembler.parameter(get_fact, ParameterRole::Block, 3, u64_type());
    let fact = assembler.operation(
        get_fact,
        Opcode::VectorGet,
        vec![ValueRef::Parameter(facts), ValueRef::Parameter(fact_index)],
        TypeExpr::Option(Box::new(complete_block_fact_type())),
        Immediate::None,
    );
    assembler.push_block(
        get_fact,
        function,
        vec![fact_index, fact_slot, fact_accumulator, fact_length],
        vec![fact],
        inventory_switch(
            operation_value(fact),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    get_block,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(fact_index)),
                        SwitchArgument::Value(ValueRef::Parameter(fact_slot)),
                        SwitchArgument::Value(ValueRef::Parameter(fact_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(fact_length)),
                    ],
                ),
            ],
        ),
    );

    let block_fact = assembler.parameter(
        get_block,
        ParameterRole::Block,
        0,
        complete_block_fact_type(),
    );
    let block_index = assembler.parameter(get_block, ParameterRole::Block, 1, u64_type());
    let block_slot = assembler.parameter(get_block, ParameterRole::Block, 2, u32_type());
    let block_accumulator = assembler.parameter(get_block, ParameterRole::Block, 3, u8vec_type());
    let block_length = assembler.parameter(get_block, ParameterRole::Block, 4, u64_type());
    let instruction_facts = assembler.operation(
        get_block,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(block_fact)],
        immediate_inventory_type(),
        Immediate::Index(2),
    );
    let instruction_count = assembler.operation(
        get_block,
        Opcode::VectorLen,
        vec![operation_value(instruction_facts)],
        u64_type(),
        Immediate::None,
    );
    let block_model = assembler.operation(
        get_block,
        Opcode::MapGet,
        vec![ValueRef::Parameter(blocks), ValueRef::Parameter(block_slot)],
        TypeExpr::Option(Box::new(complete_block_model_type())),
        Immediate::None,
    );
    assembler.push_block(
        get_block,
        function,
        vec![
            block_fact,
            block_index,
            block_slot,
            block_accumulator,
            block_length,
        ],
        vec![instruction_facts, instruction_count, block_model],
        inventory_switch(
            operation_value(block_model),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    append,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(operation_value(instruction_count)),
                        SwitchArgument::Value(ValueRef::Parameter(block_index)),
                        SwitchArgument::Value(ValueRef::Parameter(block_slot)),
                        SwitchArgument::Value(ValueRef::Parameter(block_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(block_length)),
                    ],
                ),
            ],
        ),
    );

    let append_model =
        assembler.parameter(append, ParameterRole::Block, 0, complete_block_model_type());
    let append_instruction_count = assembler.parameter(append, ParameterRole::Block, 1, u64_type());
    let append_index = assembler.parameter(append, ParameterRole::Block, 2, u64_type());
    let append_slot = assembler.parameter(append, ParameterRole::Block, 3, u32_type());
    let append_accumulator = assembler.parameter(append, ParameterRole::Block, 4, u8vec_type());
    let append_length = assembler.parameter(append, ParameterRole::Block, 5, u64_type());
    let encoded = assembler.operation(
        append,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(append_model),
            ValueRef::Parameter(append_instruction_count),
            ValueRef::Parameter(append_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_block,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append,
        function,
        vec![
            append_model,
            append_instruction_count,
            append_index,
            append_slot,
            append_accumulator,
            append_length,
        ],
        vec![encoded],
        inventory_switch(
            operation_value(encoded),
            vec![
                (
                    BuiltinCase::Ok,
                    advance_index,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(append_index)),
                        SwitchArgument::Value(ValueRef::Parameter(append_slot)),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(append_length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let index_value = assembler.parameter(advance_index, ParameterRole::Block, 0, u64_type());
    let index_slot = assembler.parameter(advance_index, ParameterRole::Block, 1, u32_type());
    let index_accumulator =
        assembler.parameter(advance_index, ParameterRole::Block, 2, u8vec_type());
    let index_length = assembler.parameter(advance_index, ParameterRole::Block, 3, u64_type());
    let index_one = assembler.constant_ref(advance_index, one_u64, u64_type());
    let next_index = assembler.operation(
        advance_index,
        Opcode::IntAddChecked,
        vec![ValueRef::Parameter(index_value), operation_value(index_one)],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance_index,
        function,
        vec![index_value, index_slot, index_accumulator, index_length],
        vec![index_one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    advance_slot,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(index_slot)),
                        SwitchArgument::Value(ValueRef::Parameter(index_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(index_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let slot_index = assembler.parameter(advance_slot, ParameterRole::Block, 0, u64_type());
    let slot_value = assembler.parameter(advance_slot, ParameterRole::Block, 1, u32_type());
    let slot_accumulator = assembler.parameter(advance_slot, ParameterRole::Block, 2, u8vec_type());
    let slot_length = assembler.parameter(advance_slot, ParameterRole::Block, 3, u64_type());
    let slot_one = assembler.constant_ref(advance_slot, one_u32, u32_type());
    let next_slot = assembler.operation(
        advance_slot,
        Opcode::IntAddChecked,
        vec![ValueRef::Parameter(slot_value), operation_value(slot_one)],
        arithmetic_result_type(u32_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance_slot,
        function,
        vec![slot_index, slot_value, slot_accumulator, slot_length],
        vec![slot_one, next_slot],
        inventory_switch(
            operation_value(next_slot),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(slot_index)),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(slot_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(slot_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let completed = assembler.parameter(done, ParameterRole::Block, 0, u8vec_type());
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(completed)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![completed],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![blocks, facts, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn complete_block_map_byte_encoder() -> LowerScaffold {
    let base = complete_block_byte_encoder();
    let append_block = base.entry.entity_id;
    let function = inventory_id(5, 38);
    let mut assembler = InventoryAssembler {
        next_block: u16::try_from(base.blocks.len() + 1).expect("fixture block count fits u16"),
        next_parameter: u16::try_from(base.parameters.len() + 1)
            .expect("fixture parameter count fits u16"),
        next_operation: u16::try_from(base.operations.len() + 1)
            .expect("fixture operation count fits u16"),
        next_constant: u16::try_from(base.constants.len() + 1)
            .expect("fixture constant count fits u16"),
        parameters: base.parameters,
        blocks: base.blocks,
        operations: base.operations,
        constants: base.constants,
    };
    let graph = build_complete_block_map_byte_appender(&mut assembler, function, append_block);
    let mut functions = base.functions;
    functions.insert(0, graph.clone());
    LowerScaffold {
        types: base.types,
        entry: graph,
        functions,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: base.adapters,
    }
}

/// Composes complete semantic lowering with exact SLEYBC02 function-body
/// metadata emission. The returned bytes end after the canonical block count;
/// ordered block bodies are the next serializer layer.
#[allow(clippy::too_many_lines)]
fn complete_function_header_encoder() -> LowerScaffold {
    let base = complete_function_lowerer();
    let lower_function = base.entry.entity_id;
    let narrow_u32 = inventory_id(5, 19);
    let append_u32 = inventory_id(5, 20);
    let narrow_u64 = inventory_id(5, 21);
    let append_u64 = inventory_id(5, 22);
    let append_chunk = inventory_id(5, 24);
    let append_u32_vector = inventory_id(5, 26);
    let append_bytes_vector = inventory_id(5, 27);
    let function = inventory_id(5, 28);
    let mut assembler = InventoryAssembler {
        next_block: u16::try_from(base.blocks.len() + 1).expect("fixture block count fits u16"),
        next_parameter: u16::try_from(base.parameters.len() + 1)
            .expect("fixture parameter count fits u16"),
        next_operation: u16::try_from(base.operations.len() + 1)
            .expect("fixture operation count fits u16"),
        next_constant: u16::try_from(base.constants.len() + 1)
            .expect("fixture constant count fits u16"),
        parameters: base.parameters,
        blocks: base.blocks,
        operations: base.operations,
        constants: base.constants,
    };
    let narrow_u32_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u32, &u32_type(), u32_value);
    let append_u32_graph = build_fixed_width_appender(
        &mut assembler,
        append_u32,
        narrow_u32,
        &u32_type(),
        u32_value,
        &[1_u128 << 24, 1_u128 << 16, 1_u128 << 8, 1],
    );
    let narrow_u64_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u64, &u64_type(), u64_value);
    let append_u64_graph = build_fixed_width_appender(
        &mut assembler,
        append_u64,
        narrow_u64,
        &u64_type(),
        u64_value,
        &[
            1_u128 << 56,
            1_u128 << 48,
            1_u128 << 40,
            1_u128 << 32,
            1_u128 << 24,
            1_u128 << 16,
            1_u128 << 8,
            1,
        ],
    );
    let append_chunk_graph = build_byte_chunk_appender(&mut assembler, append_chunk);
    let append_u32_vector_graph = build_vector_byte_appender(
        &mut assembler,
        append_u32_vector,
        append_u64,
        append_u32,
        &u32vec_type(),
        &u32_type(),
        true,
        true,
    );
    let append_bytes_vector_graph = build_vector_byte_appender(
        &mut assembler,
        append_bytes_vector,
        append_u64,
        append_chunk,
        &bytesvec_type(),
        &TypeExpr::Bytes,
        false,
        true,
    );

    let function_identity =
        assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bytes);
    let function_parameters =
        assembler.parameter(function, ParameterRole::Function, 1, u32vec_type());
    let register_types = assembler.parameter(function, ParameterRole::Function, 2, bytesvec_type());
    let result_type = assembler.parameter(function, ParameterRole::Function, 3, TypeExpr::Bytes);
    let entry_slot = assembler.parameter(function, ParameterRole::Function, 4, u32_type());
    let block_count = assembler.parameter(function, ParameterRole::Function, 5, u32_type());
    let block_facts = assembler.parameter(
        function,
        ParameterRole::Function,
        6,
        complete_block_facts_type(),
    );

    let entry = assembler.block_id();
    let unpack = assembler.block_id();
    let append_parameters = assembler.block_id();
    let append_register_types = assembler.block_id();
    let append_result = assembler.block_id();
    let append_entry = assembler.block_id();
    let append_block_count = assembler.block_id();
    let convert = assembler.block_id();
    let success_block = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let lowered = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(function_identity),
            ValueRef::Parameter(function_parameters),
            ValueRef::Parameter(register_types),
            ValueRef::Parameter(result_type),
            ValueRef::Parameter(entry_slot),
            ValueRef::Parameter(block_count),
            ValueRef::Parameter(block_facts),
        ],
        complete_function_result_type(),
        Immediate::Function(FunctionRefValue {
            function: lower_function,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![lowered],
        inventory_switch(
            operation_value(lowered),
            vec![
                (BuiltinCase::Ok, unpack, vec![SwitchArgument::CasePayload]),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let model = assembler.parameter(
        unpack,
        ParameterRole::Block,
        0,
        complete_function_model_type(),
    );
    let identity = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(model)],
        TypeExpr::Bytes,
        Immediate::Index(0),
    );
    let parameters = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(model)],
        u32vec_type(),
        Immediate::Index(1),
    );
    let types = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(model)],
        bytesvec_type(),
        Immediate::Index(2),
    );
    let result = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(model)],
        TypeExpr::Bytes,
        Immediate::Index(3),
    );
    let model_entry = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(model)],
        u32_type(),
        Immediate::Index(4),
    );
    let found_block_count = assembler.operation(
        unpack,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(block_facts)],
        u64_type(),
        Immediate::None,
    );
    let empty = assembler.operation(
        unpack,
        Opcode::VectorNew,
        Vec::new(),
        u8vec_type(),
        Immediate::None,
    );
    let encoded_identity = assembler.operation(
        unpack,
        Opcode::CallDirect,
        vec![operation_value(empty), operation_value(identity)],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_chunk,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        unpack,
        function,
        vec![model],
        vec![
            identity,
            parameters,
            types,
            result,
            model_entry,
            found_block_count,
            empty,
            encoded_identity,
        ],
        inventory_switch(
            operation_value(encoded_identity),
            vec![
                (
                    BuiltinCase::Ok,
                    append_parameters,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(operation_value(parameters)),
                        SwitchArgument::Value(operation_value(types)),
                        SwitchArgument::Value(operation_value(result)),
                        SwitchArgument::Value(operation_value(model_entry)),
                        SwitchArgument::Value(operation_value(found_block_count)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let parameter_accumulator =
        assembler.parameter(append_parameters, ParameterRole::Block, 0, u8vec_type());
    let parameter_values =
        assembler.parameter(append_parameters, ParameterRole::Block, 1, u32vec_type());
    let parameter_types =
        assembler.parameter(append_parameters, ParameterRole::Block, 2, bytesvec_type());
    let parameter_result =
        assembler.parameter(append_parameters, ParameterRole::Block, 3, TypeExpr::Bytes);
    let parameter_entry =
        assembler.parameter(append_parameters, ParameterRole::Block, 4, u32_type());
    let parameter_block_count =
        assembler.parameter(append_parameters, ParameterRole::Block, 5, u64_type());
    let encoded_parameters = assembler.operation(
        append_parameters,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(parameter_values),
            ValueRef::Parameter(parameter_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32_vector,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_parameters,
        function,
        vec![
            parameter_accumulator,
            parameter_values,
            parameter_types,
            parameter_result,
            parameter_entry,
            parameter_block_count,
        ],
        vec![encoded_parameters],
        inventory_switch(
            operation_value(encoded_parameters),
            vec![
                (
                    BuiltinCase::Ok,
                    append_register_types,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(parameter_types)),
                        SwitchArgument::Value(ValueRef::Parameter(parameter_result)),
                        SwitchArgument::Value(ValueRef::Parameter(parameter_entry)),
                        SwitchArgument::Value(ValueRef::Parameter(parameter_block_count)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let types_accumulator =
        assembler.parameter(append_register_types, ParameterRole::Block, 0, u8vec_type());
    let type_values = assembler.parameter(
        append_register_types,
        ParameterRole::Block,
        1,
        bytesvec_type(),
    );
    let types_result = assembler.parameter(
        append_register_types,
        ParameterRole::Block,
        2,
        TypeExpr::Bytes,
    );
    let types_entry =
        assembler.parameter(append_register_types, ParameterRole::Block, 3, u32_type());
    let types_block_count =
        assembler.parameter(append_register_types, ParameterRole::Block, 4, u64_type());
    let encoded_types = assembler.operation(
        append_register_types,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(type_values),
            ValueRef::Parameter(types_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_bytes_vector,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_register_types,
        function,
        vec![
            types_accumulator,
            type_values,
            types_result,
            types_entry,
            types_block_count,
        ],
        vec![encoded_types],
        inventory_switch(
            operation_value(encoded_types),
            vec![
                (
                    BuiltinCase::Ok,
                    append_result,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(types_result)),
                        SwitchArgument::Value(ValueRef::Parameter(types_entry)),
                        SwitchArgument::Value(ValueRef::Parameter(types_block_count)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let result_accumulator =
        assembler.parameter(append_result, ParameterRole::Block, 0, u8vec_type());
    let result_bytes = assembler.parameter(append_result, ParameterRole::Block, 1, TypeExpr::Bytes);
    let result_entry = assembler.parameter(append_result, ParameterRole::Block, 2, u32_type());
    let result_block_count =
        assembler.parameter(append_result, ParameterRole::Block, 3, u64_type());
    let encoded_result = assembler.operation(
        append_result,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(result_accumulator),
            ValueRef::Parameter(result_bytes),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_chunk,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_result,
        function,
        vec![
            result_accumulator,
            result_bytes,
            result_entry,
            result_block_count,
        ],
        vec![encoded_result],
        inventory_switch(
            operation_value(encoded_result),
            vec![
                (
                    BuiltinCase::Ok,
                    append_entry,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(result_entry)),
                        SwitchArgument::Value(ValueRef::Parameter(result_block_count)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let entry_accumulator =
        assembler.parameter(append_entry, ParameterRole::Block, 0, u8vec_type());
    let entry_value = assembler.parameter(append_entry, ParameterRole::Block, 1, u32_type());
    let entry_block_count = assembler.parameter(append_entry, ParameterRole::Block, 2, u64_type());
    let encoded_entry = assembler.operation(
        append_entry,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(entry_value),
            ValueRef::Parameter(entry_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_entry,
        function,
        vec![entry_accumulator, entry_value, entry_block_count],
        vec![encoded_entry],
        inventory_switch(
            operation_value(encoded_entry),
            vec![
                (
                    BuiltinCase::Ok,
                    append_block_count,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(entry_block_count)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let count_accumulator =
        assembler.parameter(append_block_count, ParameterRole::Block, 0, u8vec_type());
    let count_value = assembler.parameter(append_block_count, ParameterRole::Block, 1, u64_type());
    let encoded_count = assembler.operation(
        append_block_count,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(count_value),
            ValueRef::Parameter(count_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u64,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_block_count,
        function,
        vec![count_accumulator, count_value],
        vec![encoded_count],
        inventory_switch(
            operation_value(encoded_count),
            vec![
                (BuiltinCase::Ok, convert, vec![SwitchArgument::CasePayload]),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let completed = assembler.parameter(convert, ParameterRole::Block, 0, u8vec_type());
    let scope = assembler.constant_ref(convert, unit, TypeExpr::Unit);
    let output = assembler.operation(
        convert,
        Opcode::AdapterInvoke,
        vec![operation_value(scope), ValueRef::Parameter(completed)],
        index_result_type(TypeExpr::Bytes),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    assembler.push_block(
        convert,
        function,
        vec![completed],
        vec![scope, output],
        inventory_switch(
            operation_value(output),
            vec![
                (
                    BuiltinCase::Ok,
                    success_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let encoded = assembler.parameter(success_block, ParameterRole::Block, 0, TypeExpr::Bytes);
    let success = assembler.operation(
        success_block,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(encoded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success_block,
        function,
        vec![encoded],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            function_identity,
            function_parameters,
            register_types,
            result_type,
            entry_slot,
            block_count,
            block_facts,
        ],
        result_type: bytes_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let mut functions = base.functions;
    functions.extend([
        narrow_u32_graph,
        append_u32_graph,
        narrow_u64_graph,
        append_u64_graph,
        append_chunk_graph,
        append_u32_vector_graph,
        append_bytes_vector_graph,
    ]);
    functions.insert(0, graph.clone());
    let mut adapters = base.adapters;
    for (code, request_type, response_type) in [
        (sley_vm::host_abi::BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
        (
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
            u8vec_type(),
            TypeExpr::Bytes,
        ),
    ] {
        adapters.push(AdapterImport {
            entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(code)),
            adapter_id: sley_vm::host_abi::bridge_identity(code),
            abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
            request_type,
            response_type,
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
            effects: Vec::new(),
        });
    }
    LowerScaffold {
        types: base.types,
        entry: graph,
        functions,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters,
    }
}

/// Joins the exact header encoder and ordered block serializer into one
/// complete SLEYBC02 function-body byte result.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn complete_function_body_encoder() -> LowerScaffold {
    let base = complete_function_header_encoder();
    let encode_header = base.entry.entity_id;
    let lower_function = inventory_id(5, 17);
    let append_u32 = inventory_id(5, 20);
    let append_u64 = inventory_id(5, 22);
    let append_chunk = inventory_id(5, 24);
    let append_u32_vector = inventory_id(5, 26);
    let append_instruction = inventory_id(5, 29);
    let append_instruction_map = inventory_id(5, 30);
    let append_edge = inventory_id(5, 31);
    let append_simple = inventory_id(5, 32);
    let append_arguments = inventory_id(5, 33);
    let append_cases = inventory_id(5, 34);
    let append_switch = inventory_id(5, 35);
    let append_terminator = inventory_id(5, 36);
    let append_block = inventory_id(5, 37);
    let append_blocks = inventory_id(5, 38);
    let function = inventory_id(5, 39);
    let mut assembler = InventoryAssembler {
        next_block: u16::try_from(base.blocks.len() + 1).expect("fixture block count fits u16"),
        next_parameter: u16::try_from(base.parameters.len() + 1)
            .expect("fixture parameter count fits u16"),
        next_operation: u16::try_from(base.operations.len() + 1)
            .expect("fixture operation count fits u16"),
        next_constant: u16::try_from(base.constants.len() + 1)
            .expect("fixture constant count fits u16"),
        parameters: base.parameters,
        blocks: base.blocks,
        operations: base.operations,
        constants: base.constants,
    };
    let append_instruction_graph = build_instruction_byte_appender(
        &mut assembler,
        append_instruction,
        append_u32,
        append_u32_vector,
        append_chunk,
    );
    let append_instruction_map_graph = build_instruction_map_byte_appender(
        &mut assembler,
        append_instruction_map,
        append_u64,
        append_instruction,
    );
    let append_edge_graph =
        build_target_edge_byte_appender(&mut assembler, append_edge, append_u32, append_u32_vector);
    let append_simple_graph = build_simple_terminator_byte_appender(
        &mut assembler,
        append_simple,
        append_u32,
        append_edge,
    );
    let append_arguments_graph = build_switch_arguments_byte_appender(
        &mut assembler,
        append_arguments,
        append_u32,
        append_u64,
    );
    let append_cases_graph = build_switch_cases_byte_appender(
        &mut assembler,
        append_cases,
        append_u32,
        append_u64,
        append_arguments,
    );
    let append_switch_graph = build_builtin_switch_terminator_byte_appender(
        &mut assembler,
        append_switch,
        append_u32,
        append_cases,
    );
    let append_terminator_graph = build_complete_terminator_byte_appender(
        &mut assembler,
        append_terminator,
        append_simple,
        append_switch,
    );
    let append_block_graph = build_complete_block_byte_appender(
        &mut assembler,
        append_block,
        append_u32,
        append_u32_vector,
        append_instruction_map,
        append_terminator,
    );
    let append_blocks_graph =
        build_complete_block_map_byte_appender(&mut assembler, append_blocks, append_block);

    let function_identity =
        assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bytes);
    let function_parameters =
        assembler.parameter(function, ParameterRole::Function, 1, u32vec_type());
    let register_types = assembler.parameter(function, ParameterRole::Function, 2, bytesvec_type());
    let result_type = assembler.parameter(function, ParameterRole::Function, 3, TypeExpr::Bytes);
    let entry_slot = assembler.parameter(function, ParameterRole::Function, 4, u32_type());
    let block_count = assembler.parameter(function, ParameterRole::Function, 5, u32_type());
    let block_facts = assembler.parameter(
        function,
        ParameterRole::Function,
        6,
        complete_block_facts_type(),
    );
    let entry = assembler.block_id();
    let encode_header_block = assembler.block_id();
    let open_header = assembler.block_id();
    let append_block_records = assembler.block_id();
    let convert = assembler.block_id();
    let success_block = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let lowered = assembler.operation(
        entry,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(function_identity),
            ValueRef::Parameter(function_parameters),
            ValueRef::Parameter(register_types),
            ValueRef::Parameter(result_type),
            ValueRef::Parameter(entry_slot),
            ValueRef::Parameter(block_count),
            ValueRef::Parameter(block_facts),
        ],
        complete_function_result_type(),
        Immediate::Function(FunctionRefValue {
            function: lower_function,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![lowered],
        inventory_switch(
            operation_value(lowered),
            vec![
                (
                    BuiltinCase::Ok,
                    encode_header_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let model = assembler.parameter(
        encode_header_block,
        ParameterRole::Block,
        0,
        complete_function_model_type(),
    );
    let header = assembler.operation(
        encode_header_block,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(function_identity),
            ValueRef::Parameter(function_parameters),
            ValueRef::Parameter(register_types),
            ValueRef::Parameter(result_type),
            ValueRef::Parameter(entry_slot),
            ValueRef::Parameter(block_count),
            ValueRef::Parameter(block_facts),
        ],
        bytes_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: encode_header,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_header_block,
        function,
        vec![model],
        vec![header],
        inventory_switch(
            operation_value(header),
            vec![
                (
                    BuiltinCase::Ok,
                    open_header,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(model)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let header_model = assembler.parameter(
        open_header,
        ParameterRole::Block,
        0,
        complete_function_model_type(),
    );
    let header_bytes = assembler.parameter(open_header, ParameterRole::Block, 1, TypeExpr::Bytes);
    let blocks = assembler.operation(
        open_header,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(header_model)],
        complete_block_map_type(),
        Immediate::Index(5),
    );
    let scope = assembler.constant_ref(open_header, unit, TypeExpr::Unit);
    let header_octets = assembler.operation(
        open_header,
        Opcode::AdapterInvoke,
        vec![operation_value(scope), ValueRef::Parameter(header_bytes)],
        index_result_type(u8vec_type()),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    assembler.push_block(
        open_header,
        function,
        vec![header_model, header_bytes],
        vec![blocks, scope, header_octets],
        inventory_switch(
            operation_value(header_octets),
            vec![
                (
                    BuiltinCase::Ok,
                    append_block_records,
                    vec![
                        SwitchArgument::Value(operation_value(blocks)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let lowered_blocks = assembler.parameter(
        append_block_records,
        ParameterRole::Block,
        0,
        complete_block_map_type(),
    );
    let body_prefix =
        assembler.parameter(append_block_records, ParameterRole::Block, 1, u8vec_type());
    let body = assembler.operation(
        append_block_records,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(lowered_blocks),
            ValueRef::Parameter(block_facts),
            ValueRef::Parameter(body_prefix),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_blocks,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_block_records,
        function,
        vec![lowered_blocks, body_prefix],
        vec![body],
        inventory_switch(
            operation_value(body),
            vec![
                (BuiltinCase::Ok, convert, vec![SwitchArgument::CasePayload]),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let body_octets = assembler.parameter(convert, ParameterRole::Block, 0, u8vec_type());
    let output_scope = assembler.constant_ref(convert, unit, TypeExpr::Unit);
    let output = assembler.operation(
        convert,
        Opcode::AdapterInvoke,
        vec![
            operation_value(output_scope),
            ValueRef::Parameter(body_octets),
        ],
        index_result_type(TypeExpr::Bytes),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    assembler.push_block(
        convert,
        function,
        vec![body_octets],
        vec![output_scope, output],
        inventory_switch(
            operation_value(output),
            vec![
                (
                    BuiltinCase::Ok,
                    success_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let encoded = assembler.parameter(success_block, ParameterRole::Block, 0, TypeExpr::Bytes);
    let success = assembler.operation(
        success_block,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(encoded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success_block,
        function,
        vec![encoded],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            function_identity,
            function_parameters,
            register_types,
            result_type,
            entry_slot,
            block_count,
            block_facts,
        ],
        result_type: bytes_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let mut functions = base.functions;
    functions.extend([
        append_instruction_graph,
        append_instruction_map_graph,
        append_edge_graph,
        append_simple_graph,
        append_arguments_graph,
        append_cases_graph,
        append_switch_graph,
        append_terminator_graph,
        append_block_graph,
        append_blocks_graph,
    ]);
    functions.insert(0, graph.clone());
    LowerScaffold {
        types: base.types,
        entry: graph,
        functions,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: base.adapters,
    }
}

/// Wraps one complete root body in the canonical SLEYBC02 envelope with an
/// empty transitive-callee table.
#[allow(clippy::too_many_lines)]
fn root_function_image_encoder() -> LowerScaffold {
    let base = complete_function_body_encoder();
    let encode_body = base.entry.entity_id;
    let append_u32 = inventory_id(5, 20);
    let append_u64 = inventory_id(5, 22);
    let append_chunk = inventory_id(5, 24);
    let function = inventory_id(5, 40);
    let mut assembler = InventoryAssembler {
        next_block: u16::try_from(base.blocks.len() + 1).expect("fixture block count fits u16"),
        next_parameter: u16::try_from(base.parameters.len() + 1)
            .expect("fixture parameter count fits u16"),
        next_operation: u16::try_from(base.operations.len() + 1)
            .expect("fixture operation count fits u16"),
        next_constant: u16::try_from(base.constants.len() + 1)
            .expect("fixture constant count fits u16"),
        parameters: base.parameters,
        blocks: base.blocks,
        operations: base.operations,
        constants: base.constants,
    };
    let function_identity =
        assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bytes);
    let function_parameters =
        assembler.parameter(function, ParameterRole::Function, 1, u32vec_type());
    let register_types = assembler.parameter(function, ParameterRole::Function, 2, bytesvec_type());
    let result_type = assembler.parameter(function, ParameterRole::Function, 3, TypeExpr::Bytes);
    let entry_slot = assembler.parameter(function, ParameterRole::Function, 4, u32_type());
    let block_count = assembler.parameter(function, ParameterRole::Function, 5, u32_type());
    let block_facts = assembler.parameter(
        function,
        ParameterRole::Function,
        6,
        complete_block_facts_type(),
    );
    let entry = assembler.block_id();
    let append_version = assembler.block_id();
    let encode_body_block = assembler.block_id();
    let append_body = assembler.block_id();
    let append_callee_count = assembler.block_id();
    let convert = assembler.block_id();
    let success_block = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let magic = assembler.constant(bytes_value(b"SLEYBC02"));
    let version = assembler.constant(u32_value(1));
    let zero_callees = assembler.constant(u64_value(0));
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let magic_bytes = assembler.constant_ref(entry, magic, TypeExpr::Bytes);
    let input_scope = assembler.constant_ref(entry, unit, TypeExpr::Unit);
    let magic_octets = assembler.operation(
        entry,
        Opcode::AdapterInvoke,
        vec![operation_value(input_scope), operation_value(magic_bytes)],
        index_result_type(u8vec_type()),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![magic_bytes, input_scope, magic_octets],
        inventory_switch(
            operation_value(magic_octets),
            vec![
                (
                    BuiltinCase::Ok,
                    append_version,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );
    let magic_accumulator =
        assembler.parameter(append_version, ParameterRole::Block, 0, u8vec_type());
    let version_value = assembler.constant_ref(append_version, version, u32_type());
    let versioned = assembler.operation(
        append_version,
        Opcode::CallDirect,
        vec![
            operation_value(version_value),
            ValueRef::Parameter(magic_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_version,
        function,
        vec![magic_accumulator],
        vec![version_value, versioned],
        inventory_switch(
            operation_value(versioned),
            vec![
                (
                    BuiltinCase::Ok,
                    encode_body_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let envelope = assembler.parameter(encode_body_block, ParameterRole::Block, 0, u8vec_type());
    let body = assembler.operation(
        encode_body_block,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(function_identity),
            ValueRef::Parameter(function_parameters),
            ValueRef::Parameter(register_types),
            ValueRef::Parameter(result_type),
            ValueRef::Parameter(entry_slot),
            ValueRef::Parameter(block_count),
            ValueRef::Parameter(block_facts),
        ],
        bytes_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: encode_body,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_body_block,
        function,
        vec![envelope],
        vec![body],
        inventory_switch(
            operation_value(body),
            vec![
                (
                    BuiltinCase::Ok,
                    append_body,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(envelope)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let body_accumulator = assembler.parameter(append_body, ParameterRole::Block, 0, u8vec_type());
    let body_bytes = assembler.parameter(append_body, ParameterRole::Block, 1, TypeExpr::Bytes);
    let image_body = assembler.operation(
        append_body,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(body_accumulator),
            ValueRef::Parameter(body_bytes),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_chunk,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_body,
        function,
        vec![body_accumulator, body_bytes],
        vec![image_body],
        inventory_switch(
            operation_value(image_body),
            vec![
                (
                    BuiltinCase::Ok,
                    append_callee_count,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let image_accumulator =
        assembler.parameter(append_callee_count, ParameterRole::Block, 0, u8vec_type());
    let callee_count = assembler.constant_ref(append_callee_count, zero_callees, u64_type());
    let completed = assembler.operation(
        append_callee_count,
        Opcode::CallDirect,
        vec![
            operation_value(callee_count),
            ValueRef::Parameter(image_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u64,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_callee_count,
        function,
        vec![image_accumulator],
        vec![callee_count, completed],
        inventory_switch(
            operation_value(completed),
            vec![
                (BuiltinCase::Ok, convert, vec![SwitchArgument::CasePayload]),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let octets = assembler.parameter(convert, ParameterRole::Block, 0, u8vec_type());
    let output_scope = assembler.constant_ref(convert, unit, TypeExpr::Unit);
    let output = assembler.operation(
        convert,
        Opcode::AdapterInvoke,
        vec![operation_value(output_scope), ValueRef::Parameter(octets)],
        index_result_type(TypeExpr::Bytes),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    assembler.push_block(
        convert,
        function,
        vec![octets],
        vec![output_scope, output],
        inventory_switch(
            operation_value(output),
            vec![
                (
                    BuiltinCase::Ok,
                    success_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );
    let encoded = assembler.parameter(success_block, ParameterRole::Block, 0, TypeExpr::Bytes);
    let success = assembler.operation(
        success_block,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(encoded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success_block,
        function,
        vec![encoded],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            function_identity,
            function_parameters,
            register_types,
            result_type,
            entry_slot,
            block_count,
            block_facts,
        ],
        result_type: bytes_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let mut functions = base.functions;
    functions.insert(0, graph.clone());
    LowerScaffold {
        types: base.types,
        entry: graph,
        functions,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: base.adapters,
    }
}

/// Appends an ordered vector of complete function facts by running every
/// element through the same semantic function-body lowerer used for the root.
#[allow(clippy::too_many_lines)]
fn build_function_fact_vector_appender(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    encode_body: EntityId,
    append_chunk: EntityId,
) -> FunctionGraph {
    let functions = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        complete_function_facts_type(),
    );
    let accumulator = assembler.parameter(function, ParameterRole::Function, 1, u8vec_type());
    let entry = assembler.block_id();
    let check = assembler.block_id();
    let get_fact = assembler.block_id();
    let encode = assembler.block_id();
    let append = assembler.block_id();
    let advance = assembler.block_id();
    let done = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();
    let zero = assembler.constant(u64_value(0));
    let one = assembler.constant(u64_value(1));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let zero_index = assembler.constant_ref(entry, zero, u64_type());
    let length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(functions)],
        u64_type(),
        Immediate::None,
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![zero_index, length],
        inventory_branch(
            check,
            vec![
                operation_value(zero_index),
                ValueRef::Parameter(accumulator),
                operation_value(length),
            ],
        ),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let check_accumulator = assembler.parameter(check, ParameterRole::Block, 1, u8vec_type());
    let check_length = assembler.parameter(check, ParameterRole::Block, 2, u64_type());
    let has_function = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_index),
            ValueRef::Parameter(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        vec![check_index, check_accumulator, check_length],
        vec![has_function],
        inventory_cond(
            operation_value(has_function),
            get_fact,
            vec![
                ValueRef::Parameter(check_index),
                ValueRef::Parameter(check_accumulator),
                ValueRef::Parameter(check_length),
            ],
            done,
            vec![ValueRef::Parameter(check_accumulator)],
        ),
    );

    let fact_index = assembler.parameter(get_fact, ParameterRole::Block, 0, u64_type());
    let fact_accumulator = assembler.parameter(get_fact, ParameterRole::Block, 1, u8vec_type());
    let fact_length = assembler.parameter(get_fact, ParameterRole::Block, 2, u64_type());
    let fact = assembler.operation(
        get_fact,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(functions),
            ValueRef::Parameter(fact_index),
        ],
        TypeExpr::Option(Box::new(complete_function_fact_type())),
        Immediate::None,
    );
    assembler.push_block(
        get_fact,
        function,
        vec![fact_index, fact_accumulator, fact_length],
        vec![fact],
        inventory_switch(
            operation_value(fact),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    encode,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(fact_index)),
                        SwitchArgument::Value(ValueRef::Parameter(fact_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(fact_length)),
                    ],
                ),
            ],
        ),
    );

    let function_fact = assembler.parameter(
        encode,
        ParameterRole::Block,
        0,
        complete_function_fact_type(),
    );
    let encode_index = assembler.parameter(encode, ParameterRole::Block, 1, u64_type());
    let encode_accumulator = assembler.parameter(encode, ParameterRole::Block, 2, u8vec_type());
    let encode_length = assembler.parameter(encode, ParameterRole::Block, 3, u64_type());
    let identity = assembler.operation(
        encode,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(function_fact)],
        TypeExpr::Bytes,
        Immediate::Index(0),
    );
    let parameters = assembler.operation(
        encode,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(function_fact)],
        u32vec_type(),
        Immediate::Index(1),
    );
    let register_types = assembler.operation(
        encode,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(function_fact)],
        bytesvec_type(),
        Immediate::Index(2),
    );
    let result_type = assembler.operation(
        encode,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(function_fact)],
        TypeExpr::Bytes,
        Immediate::Index(3),
    );
    let entry_slot = assembler.operation(
        encode,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(function_fact)],
        u32_type(),
        Immediate::Index(4),
    );
    let block_count = assembler.operation(
        encode,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(function_fact)],
        u32_type(),
        Immediate::Index(5),
    );
    let block_facts = assembler.operation(
        encode,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(function_fact)],
        complete_block_facts_type(),
        Immediate::Index(6),
    );
    let body = assembler.operation(
        encode,
        Opcode::CallDirect,
        vec![
            operation_value(identity),
            operation_value(parameters),
            operation_value(register_types),
            operation_value(result_type),
            operation_value(entry_slot),
            operation_value(block_count),
            operation_value(block_facts),
        ],
        bytes_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: encode_body,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode,
        function,
        vec![
            function_fact,
            encode_index,
            encode_accumulator,
            encode_length,
        ],
        vec![
            identity,
            parameters,
            register_types,
            result_type,
            entry_slot,
            block_count,
            block_facts,
            body,
        ],
        inventory_switch(
            operation_value(body),
            vec![
                (
                    BuiltinCase::Ok,
                    append,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(encode_index)),
                        SwitchArgument::Value(ValueRef::Parameter(encode_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(encode_length)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let append_index = assembler.parameter(append, ParameterRole::Block, 0, u64_type());
    let append_accumulator = assembler.parameter(append, ParameterRole::Block, 1, u8vec_type());
    let append_length = assembler.parameter(append, ParameterRole::Block, 2, u64_type());
    let append_body = assembler.parameter(append, ParameterRole::Block, 3, TypeExpr::Bytes);
    let appended = assembler.operation(
        append,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(append_accumulator),
            ValueRef::Parameter(append_body),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_chunk,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append,
        function,
        vec![append_index, append_accumulator, append_length, append_body],
        vec![appended],
        inventory_switch(
            operation_value(appended),
            vec![
                (
                    BuiltinCase::Ok,
                    advance,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(append_index)),
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(append_length)),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let advance_index = assembler.parameter(advance, ParameterRole::Block, 0, u64_type());
    let advance_accumulator = assembler.parameter(advance, ParameterRole::Block, 1, u8vec_type());
    let advance_length = assembler.parameter(advance, ParameterRole::Block, 2, u64_type());
    let one_value = assembler.constant_ref(advance, one, u64_type());
    let next_index = assembler.operation(
        advance,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_index),
            operation_value(one_value),
        ],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance,
        function,
        vec![advance_index, advance_accumulator, advance_length],
        vec![one_value, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(advance_accumulator)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_length)),
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let completed = assembler.parameter(done, ParameterRole::Block, 0, u8vec_type());
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(completed)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        vec![completed],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        byte_vector_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );
    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![functions, accumulator],
        result_type: byte_vector_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

/// Emits a complete extended-profile image from root facts plus an ordered
/// vector of transitive-callee facts.
#[allow(clippy::too_many_lines)]
fn complete_function_image_encoder() -> LowerScaffold {
    let base = complete_function_body_encoder();
    let encode_body = base.entry.entity_id;
    let append_u32 = inventory_id(5, 20);
    let append_u64 = inventory_id(5, 22);
    let append_chunk = inventory_id(5, 24);
    let append_callees = inventory_id(5, 41);
    let function = inventory_id(5, 42);
    let mut assembler = InventoryAssembler {
        next_block: u16::try_from(base.blocks.len() + 1).expect("fixture block count fits u16"),
        next_parameter: u16::try_from(base.parameters.len() + 1)
            .expect("fixture parameter count fits u16"),
        next_operation: u16::try_from(base.operations.len() + 1)
            .expect("fixture operation count fits u16"),
        next_constant: u16::try_from(base.constants.len() + 1)
            .expect("fixture constant count fits u16"),
        parameters: base.parameters,
        blocks: base.blocks,
        operations: base.operations,
        constants: base.constants,
    };
    let append_callees_graph = build_function_fact_vector_appender(
        &mut assembler,
        append_callees,
        encode_body,
        append_chunk,
    );
    let function_identity =
        assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bytes);
    let function_parameters =
        assembler.parameter(function, ParameterRole::Function, 1, u32vec_type());
    let register_types = assembler.parameter(function, ParameterRole::Function, 2, bytesvec_type());
    let result_type = assembler.parameter(function, ParameterRole::Function, 3, TypeExpr::Bytes);
    let entry_slot = assembler.parameter(function, ParameterRole::Function, 4, u32_type());
    let block_count = assembler.parameter(function, ParameterRole::Function, 5, u32_type());
    let block_facts = assembler.parameter(
        function,
        ParameterRole::Function,
        6,
        complete_block_facts_type(),
    );
    let callee_facts = assembler.parameter(
        function,
        ParameterRole::Function,
        7,
        complete_function_facts_type(),
    );
    let entry = assembler.block_id();
    let append_version = assembler.block_id();
    let encode_root = assembler.block_id();
    let append_root = assembler.block_id();
    let append_callee_count = assembler.block_id();
    let append_callee_bodies = assembler.block_id();
    let convert = assembler.block_id();
    let success_block = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let magic = assembler.constant(bytes_value(b"SLEYBC02"));
    let version = assembler.constant(u32_value(1));
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let magic_bytes = assembler.constant_ref(entry, magic, TypeExpr::Bytes);
    let input_scope = assembler.constant_ref(entry, unit, TypeExpr::Unit);
    let magic_octets = assembler.operation(
        entry,
        Opcode::AdapterInvoke,
        vec![operation_value(input_scope), operation_value(magic_bytes)],
        index_result_type(u8vec_type()),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![magic_bytes, input_scope, magic_octets],
        inventory_switch(
            operation_value(magic_octets),
            vec![
                (
                    BuiltinCase::Ok,
                    append_version,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let magic_accumulator =
        assembler.parameter(append_version, ParameterRole::Block, 0, u8vec_type());
    let version_value = assembler.constant_ref(append_version, version, u32_type());
    let versioned = assembler.operation(
        append_version,
        Opcode::CallDirect,
        vec![
            operation_value(version_value),
            ValueRef::Parameter(magic_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u32,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_version,
        function,
        vec![magic_accumulator],
        vec![version_value, versioned],
        inventory_switch(
            operation_value(versioned),
            vec![
                (
                    BuiltinCase::Ok,
                    encode_root,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let envelope = assembler.parameter(encode_root, ParameterRole::Block, 0, u8vec_type());
    let root_body = assembler.operation(
        encode_root,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(function_identity),
            ValueRef::Parameter(function_parameters),
            ValueRef::Parameter(register_types),
            ValueRef::Parameter(result_type),
            ValueRef::Parameter(entry_slot),
            ValueRef::Parameter(block_count),
            ValueRef::Parameter(block_facts),
        ],
        bytes_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: encode_body,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        encode_root,
        function,
        vec![envelope],
        vec![root_body],
        inventory_switch(
            operation_value(root_body),
            vec![
                (
                    BuiltinCase::Ok,
                    append_root,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(envelope)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let root_accumulator = assembler.parameter(append_root, ParameterRole::Block, 0, u8vec_type());
    let root_bytes = assembler.parameter(append_root, ParameterRole::Block, 1, TypeExpr::Bytes);
    let image_root = assembler.operation(
        append_root,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(root_accumulator),
            ValueRef::Parameter(root_bytes),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_chunk,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_root,
        function,
        vec![root_accumulator, root_bytes],
        vec![image_root],
        inventory_switch(
            operation_value(image_root),
            vec![
                (
                    BuiltinCase::Ok,
                    append_callee_count,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let count_accumulator =
        assembler.parameter(append_callee_count, ParameterRole::Block, 0, u8vec_type());
    let callee_count = assembler.operation(
        append_callee_count,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(callee_facts)],
        u64_type(),
        Immediate::None,
    );
    let counted = assembler.operation(
        append_callee_count,
        Opcode::CallDirect,
        vec![
            operation_value(callee_count),
            ValueRef::Parameter(count_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_u64,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_callee_count,
        function,
        vec![count_accumulator],
        vec![callee_count, counted],
        inventory_switch(
            operation_value(counted),
            vec![
                (
                    BuiltinCase::Ok,
                    append_callee_bodies,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let callee_accumulator =
        assembler.parameter(append_callee_bodies, ParameterRole::Block, 0, u8vec_type());
    let completed = assembler.operation(
        append_callee_bodies,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(callee_facts),
            ValueRef::Parameter(callee_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_callees,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_callee_bodies,
        function,
        vec![callee_accumulator],
        vec![completed],
        inventory_switch(
            operation_value(completed),
            vec![
                (BuiltinCase::Ok, convert, vec![SwitchArgument::CasePayload]),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let octets = assembler.parameter(convert, ParameterRole::Block, 0, u8vec_type());
    let output_scope = assembler.constant_ref(convert, unit, TypeExpr::Unit);
    let output = assembler.operation(
        convert,
        Opcode::AdapterInvoke,
        vec![operation_value(output_scope), ValueRef::Parameter(octets)],
        index_result_type(TypeExpr::Bytes),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    assembler.push_block(
        convert,
        function,
        vec![octets],
        vec![output_scope, output],
        inventory_switch(
            operation_value(output),
            vec![
                (
                    BuiltinCase::Ok,
                    success_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );
    let encoded = assembler.parameter(success_block, ParameterRole::Block, 0, TypeExpr::Bytes);
    let success = assembler.operation(
        success_block,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(encoded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success_block,
        function,
        vec![encoded],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            function_identity,
            function_parameters,
            register_types,
            result_type,
            entry_slot,
            block_count,
            block_facts,
            callee_facts,
        ],
        result_type: bytes_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let mut functions = base.functions;
    functions.extend([append_callees_graph]);
    functions.insert(0, graph.clone());
    LowerScaffold {
        types: base.types,
        entry: graph,
        functions,
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: base.adapters,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_package_section_append_stage(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    block: EntityId,
    parameters: Vec<EntityId>,
    mut operations: Vec<EntityId>,
    accumulator: ValueRef,
    item: ValueRef,
    append_function: EntityId,
    item_first: bool,
    next: EntityId,
    forward_error: EntityId,
) {
    let operands = if item_first {
        vec![item, accumulator]
    } else {
        vec![accumulator, item]
    };
    let appended = assembler.operation(
        block,
        Opcode::CallDirect,
        operands,
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_function,
            type_arguments: Vec::new(),
        }),
    );
    operations.push(appended);
    assembler.push_block(
        block,
        function,
        parameters,
        operations,
        inventory_switch(
            operation_value(appended),
            vec![
                (BuiltinCase::Ok, next, vec![SwitchArgument::CasePayload]),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
}

/// Emits the three empty counted inventory sections plus the complete
/// dependency section from canonical checked global/contract row bytes. Every
/// identity, gate claim, fingerprint, and execution limit is a typed runtime
/// input; only the frozen profile fields and the three returned empty section
/// placeholders are constants in this Sley program.
#[allow(clippy::too_many_lines)]
fn package_dependency_sections_encoder() -> LowerScaffold {
    let narrow_u32 = inventory_id(5, 19);
    let append_u32 = inventory_id(5, 20);
    let narrow_u64 = inventory_id(5, 21);
    let append_u64 = inventory_id(5, 22);
    let append_chunk = inventory_id(5, 24);
    let append_u32_sequence = inventory_id(5, 43);
    let append_u64_sequence = inventory_id(5, 44);
    let append_fingerprints = inventory_id(5, 45);
    let function = inventory_id(5, 46);
    let mut assembler = InventoryAssembler::new();
    let narrow_u32_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u32, &u32_type(), u32_value);
    let append_u32_graph = build_fixed_width_appender(
        &mut assembler,
        append_u32,
        narrow_u32,
        &u32_type(),
        u32_value,
        &[1_u128 << 24, 1_u128 << 16, 1_u128 << 8, 1],
    );
    let narrow_u64_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u64, &u64_type(), u64_value);
    let append_u64_graph = build_fixed_width_appender(
        &mut assembler,
        append_u64,
        narrow_u64,
        &u64_type(),
        u64_value,
        &[
            1_u128 << 56,
            1_u128 << 48,
            1_u128 << 40,
            1_u128 << 32,
            1_u128 << 24,
            1_u128 << 16,
            1_u128 << 8,
            1,
        ],
    );
    let append_chunk_graph = build_byte_chunk_appender(&mut assembler, append_chunk);
    let append_u32_sequence_graph = build_vector_byte_appender(
        &mut assembler,
        append_u32_sequence,
        append_u64,
        append_u32,
        &u32vec_type(),
        &u32_type(),
        true,
        false,
    );
    let append_u64_sequence_graph = build_vector_byte_appender(
        &mut assembler,
        append_u64_sequence,
        append_u64,
        append_u64,
        &u64vec_type(),
        &u64_type(),
        true,
        false,
    );
    let append_fingerprints_graph = build_vector_byte_appender(
        &mut assembler,
        append_fingerprints,
        append_u64,
        append_chunk,
        &bytesvec_type(),
        &TypeExpr::Bytes,
        false,
        true,
    );

    let entry_identity = assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bytes);
    let schema_epoch = assembler.parameter(function, ParameterRole::Function, 1, TypeExpr::Bytes);
    let state_root = assembler.parameter(function, ParameterRole::Function, 2, TypeExpr::Bytes);
    let gate_operations = assembler.parameter(function, ParameterRole::Function, 3, u32_type());
    let gate_bridges = assembler.parameter(function, ParameterRole::Function, 4, u32_type());
    let fingerprints = assembler.parameter(function, ParameterRole::Function, 5, bytesvec_type());
    let max_instructions = assembler.parameter(function, ParameterRole::Function, 6, u64_type());
    let max_fuel = assembler.parameter(function, ParameterRole::Function, 7, u64_type());
    let max_value_units = assembler.parameter(function, ParameterRole::Function, 8, u64_type());
    let max_output_units = assembler.parameter(function, ParameterRole::Function, 9, u64_type());
    let global_rows = assembler.parameter(function, ParameterRole::Function, 10, bytesvec_type());
    let contract_rows = assembler.parameter(function, ParameterRole::Function, 11, bytesvec_type());
    let cancel_present = assembler.parameter(function, ParameterRole::Function, 12, TypeExpr::Bool);
    let cancel_at_fuel = assembler.parameter(function, ParameterRole::Function, 13, u64_type());

    let entry = assembler.block_id();
    let append_epoch = assembler.block_id();
    let append_root = assembler.block_id();
    let append_profile_u32 = assembler.block_id();
    let append_profile_u64 = assembler.block_id();
    let append_gate = assembler.block_id();
    let append_fingerprint_rows = assembler.block_id();
    let append_limits = assembler.block_id();
    let append_cancel = assembler.block_id();
    let append_cancel_none = assembler.block_id();
    let append_cancel_some_tag = assembler.block_id();
    let append_cancel_value = assembler.block_id();
    let append_globals = assembler.block_id();
    let append_contracts = assembler.block_id();
    let convert = assembler.block_id();
    let success_block = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();

    let zero_u32 = assembler.constant(u32_value(0));
    let one_u32 = assembler.constant(u32_value(1));
    let two_u32 = assembler.constant(u32_value(2));
    let zero_u64 = assembler.constant(u64_value(0));
    let empty_octets = assembler.constant(u8vec_value(&[]));
    let empty_counted_section = assembler.constant(bytes_value(&0_u64.to_be_bytes()));
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let initial = assembler.constant_ref(entry, empty_octets, u8vec_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        entry,
        Vec::new(),
        vec![initial],
        operation_value(initial),
        ValueRef::Parameter(entry_identity),
        append_chunk,
        false,
        append_epoch,
        forward_error,
    );

    let epoch_accumulator =
        assembler.parameter(append_epoch, ParameterRole::Block, 0, u8vec_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_epoch,
        vec![epoch_accumulator],
        Vec::new(),
        ValueRef::Parameter(epoch_accumulator),
        ValueRef::Parameter(schema_epoch),
        append_chunk,
        false,
        append_root,
        forward_error,
    );

    let root_accumulator = assembler.parameter(append_root, ParameterRole::Block, 0, u8vec_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_root,
        vec![root_accumulator],
        Vec::new(),
        ValueRef::Parameter(root_accumulator),
        ValueRef::Parameter(state_root),
        append_chunk,
        false,
        append_profile_u32,
        forward_error,
    );

    let profile_u32_accumulator =
        assembler.parameter(append_profile_u32, ParameterRole::Block, 0, u8vec_type());
    let profile_one = assembler.constant_ref(append_profile_u32, one_u32, u32_type());
    let profile_zero = assembler.constant_ref(append_profile_u32, zero_u32, u32_type());
    let profile_two = assembler.constant_ref(append_profile_u32, two_u32, u32_type());
    let profile_words = assembler.operation(
        append_profile_u32,
        Opcode::VectorNew,
        vec![
            operation_value(profile_one),
            operation_value(profile_zero),
            operation_value(profile_zero),
            operation_value(profile_two),
            operation_value(profile_two),
            operation_value(profile_zero),
            operation_value(profile_zero),
        ],
        u32vec_type(),
        Immediate::None,
    );
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_profile_u32,
        vec![profile_u32_accumulator],
        vec![profile_one, profile_zero, profile_two, profile_words],
        ValueRef::Parameter(profile_u32_accumulator),
        operation_value(profile_words),
        append_u32_sequence,
        true,
        append_profile_u64,
        forward_error,
    );

    let profile_u64_accumulator =
        assembler.parameter(append_profile_u64, ParameterRole::Block, 0, u8vec_type());
    let profile_zero_u64 = assembler.constant_ref(append_profile_u64, zero_u64, u64_type());
    let profile_counts = assembler.operation(
        append_profile_u64,
        Opcode::VectorNew,
        vec![operation_value(profile_zero_u64); 3],
        u64vec_type(),
        Immediate::None,
    );
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_profile_u64,
        vec![profile_u64_accumulator],
        vec![profile_zero_u64, profile_counts],
        ValueRef::Parameter(profile_u64_accumulator),
        operation_value(profile_counts),
        append_u64_sequence,
        true,
        append_gate,
        forward_error,
    );

    let gate_accumulator = assembler.parameter(append_gate, ParameterRole::Block, 0, u8vec_type());
    let gate_words = assembler.operation(
        append_gate,
        Opcode::VectorNew,
        vec![
            ValueRef::Parameter(gate_operations),
            ValueRef::Parameter(gate_bridges),
        ],
        u32vec_type(),
        Immediate::None,
    );
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_gate,
        vec![gate_accumulator],
        vec![gate_words],
        ValueRef::Parameter(gate_accumulator),
        operation_value(gate_words),
        append_u32_sequence,
        true,
        append_fingerprint_rows,
        forward_error,
    );

    let fingerprint_accumulator = assembler.parameter(
        append_fingerprint_rows,
        ParameterRole::Block,
        0,
        u8vec_type(),
    );
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_fingerprint_rows,
        vec![fingerprint_accumulator],
        Vec::new(),
        ValueRef::Parameter(fingerprint_accumulator),
        ValueRef::Parameter(fingerprints),
        append_fingerprints,
        true,
        append_limits,
        forward_error,
    );

    let limits_accumulator =
        assembler.parameter(append_limits, ParameterRole::Block, 0, u8vec_type());
    let limit_words = assembler.operation(
        append_limits,
        Opcode::VectorNew,
        vec![
            ValueRef::Parameter(max_instructions),
            ValueRef::Parameter(max_fuel),
            ValueRef::Parameter(max_value_units),
            ValueRef::Parameter(max_output_units),
        ],
        u64vec_type(),
        Immediate::None,
    );
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_limits,
        vec![limits_accumulator],
        vec![limit_words],
        ValueRef::Parameter(limits_accumulator),
        operation_value(limit_words),
        append_u64_sequence,
        true,
        append_cancel,
        forward_error,
    );

    let cancel_accumulator =
        assembler.parameter(append_cancel, ParameterRole::Block, 0, u8vec_type());
    assembler.push_block(
        append_cancel,
        function,
        vec![cancel_accumulator],
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(cancel_present),
            append_cancel_some_tag,
            vec![ValueRef::Parameter(cancel_accumulator)],
            append_cancel_none,
            vec![ValueRef::Parameter(cancel_accumulator)],
        ),
    );

    let cancel_none_accumulator =
        assembler.parameter(append_cancel_none, ParameterRole::Block, 0, u8vec_type());
    let cancel_none = assembler.constant_ref(append_cancel_none, one_u32, u32_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_cancel_none,
        vec![cancel_none_accumulator],
        vec![cancel_none],
        ValueRef::Parameter(cancel_none_accumulator),
        operation_value(cancel_none),
        append_u32,
        true,
        append_globals,
        forward_error,
    );

    let cancel_some_accumulator = assembler.parameter(
        append_cancel_some_tag,
        ParameterRole::Block,
        0,
        u8vec_type(),
    );
    let cancel_some = assembler.constant_ref(append_cancel_some_tag, two_u32, u32_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_cancel_some_tag,
        vec![cancel_some_accumulator],
        vec![cancel_some],
        ValueRef::Parameter(cancel_some_accumulator),
        operation_value(cancel_some),
        append_u32,
        true,
        append_cancel_value,
        forward_error,
    );

    let cancel_value_accumulator =
        assembler.parameter(append_cancel_value, ParameterRole::Block, 0, u8vec_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_cancel_value,
        vec![cancel_value_accumulator],
        Vec::new(),
        ValueRef::Parameter(cancel_value_accumulator),
        ValueRef::Parameter(cancel_at_fuel),
        append_u64,
        true,
        append_globals,
        forward_error,
    );

    let global_accumulator =
        assembler.parameter(append_globals, ParameterRole::Block, 0, u8vec_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_globals,
        vec![global_accumulator],
        Vec::new(),
        ValueRef::Parameter(global_accumulator),
        ValueRef::Parameter(global_rows),
        append_fingerprints,
        true,
        append_contracts,
        forward_error,
    );

    let contract_accumulator =
        assembler.parameter(append_contracts, ParameterRole::Block, 0, u8vec_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_contracts,
        vec![contract_accumulator],
        Vec::new(),
        ValueRef::Parameter(contract_accumulator),
        ValueRef::Parameter(contract_rows),
        append_fingerprints,
        true,
        convert,
        forward_error,
    );

    let dependency_octets = assembler.parameter(convert, ParameterRole::Block, 0, u8vec_type());
    let output_scope = assembler.constant_ref(convert, unit, TypeExpr::Unit);
    let dependency_bytes = assembler.operation(
        convert,
        Opcode::AdapterInvoke,
        vec![
            operation_value(output_scope),
            ValueRef::Parameter(dependency_octets),
        ],
        index_result_type(TypeExpr::Bytes),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    assembler.push_block(
        convert,
        function,
        vec![dependency_octets],
        vec![output_scope, dependency_bytes],
        inventory_switch(
            operation_value(dependency_bytes),
            vec![
                (
                    BuiltinCase::Ok,
                    success_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let dependency = assembler.parameter(success_block, ParameterRole::Block, 0, TypeExpr::Bytes);
    let empty = assembler.constant_ref(success_block, empty_counted_section, TypeExpr::Bytes);
    let sections = assembler.operation(
        success_block,
        Opcode::TupleNew,
        vec![
            operation_value(empty),
            operation_value(empty),
            operation_value(empty),
            ValueRef::Parameter(dependency),
        ],
        package_sections_type(),
        Immediate::None,
    );
    let success = assembler.operation(
        success_block,
        Opcode::ResultOk,
        vec![operation_value(sections)],
        package_sections_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success_block,
        function,
        vec![dependency],
        vec![empty, sections, success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        package_sections_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        package_sections_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            entry_identity,
            schema_epoch,
            state_root,
            gate_operations,
            gate_bridges,
            fingerprints,
            max_instructions,
            max_fuel,
            max_value_units,
            max_output_units,
            global_rows,
            contract_rows,
            cancel_present,
            cancel_at_fuel,
        ],
        result_type: package_sections_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let bridge_adapter = |code, request_type, response_type| AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(code)),
        adapter_id: sley_vm::host_abi::bridge_identity(code),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type,
        response_type,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            append_u32_sequence_graph,
            append_u64_sequence_graph,
            append_fingerprints_graph,
            append_chunk_graph,
            append_u32_graph,
            narrow_u32_graph,
            append_u64_graph,
            narrow_u64_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![
            bridge_adapter(
                sley_vm::host_abi::BRIDGE_CODE_B2V1,
                TypeExpr::Bytes,
                u8vec_type(),
            ),
            bridge_adapter(sley_vm::host_abi::BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            bridge_adapter(
                sley_vm::host_abi::BRIDGE_CODE_V2B1,
                u8vec_type(),
                TypeExpr::Bytes,
            ),
        ],
    }
}

/// Emits one canonical counted package-inventory section from canonical row
/// bytes. Constant rows already contain their entity identity and value-length
/// frame, while layout and import rows receive a Sley-owned `u64` row frame.
/// The checker/codec owns row meaning; this program owns the section count,
/// optional row framing, ordering, and byte composition.
#[allow(clippy::too_many_lines)]
fn package_inventory_section_encoder() -> LowerScaffold {
    let narrow_u64 = inventory_id(5, 21);
    let append_u64 = inventory_id(5, 22);
    let append_chunk = inventory_id(5, 24);
    let append_counted_chunks = inventory_id(5, 50);
    let append_framed_section = inventory_id(5, 51);
    let append_counted_framed = inventory_id(5, 52);
    let function = inventory_id(5, 53);
    let mut assembler = InventoryAssembler::new();
    let narrow_u64_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u64, &u64_type(), u64_value);
    let append_u64_graph = build_fixed_width_appender(
        &mut assembler,
        append_u64,
        narrow_u64,
        &u64_type(),
        u64_value,
        &[
            1_u128 << 56,
            1_u128 << 48,
            1_u128 << 40,
            1_u128 << 32,
            1_u128 << 24,
            1_u128 << 16,
            1_u128 << 8,
            1,
        ],
    );
    let append_chunk_graph = build_byte_chunk_appender(&mut assembler, append_chunk);
    let append_counted_chunks_graph = build_vector_byte_appender(
        &mut assembler,
        append_counted_chunks,
        append_u64,
        append_chunk,
        &bytesvec_type(),
        &TypeExpr::Bytes,
        false,
        true,
    );
    let append_framed_section_graph = build_framed_byte_section_appender(
        &mut assembler,
        append_framed_section,
        append_u64,
        append_chunk,
    );
    let append_counted_framed_graph = build_vector_byte_appender(
        &mut assembler,
        append_counted_framed,
        append_u64,
        append_framed_section,
        &bytesvec_type(),
        &TypeExpr::Bytes,
        true,
        true,
    );

    let rows = assembler.parameter(function, ParameterRole::Function, 0, bytesvec_type());
    let frame_rows = assembler.parameter(function, ParameterRole::Function, 1, TypeExpr::Bool);
    let entry = assembler.block_id();
    let append_opaque = assembler.block_id();
    let append_framed = assembler.block_id();
    let convert = assembler.block_id();
    let success_block = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let empty_octets = assembler.constant(u8vec_value(&[]));
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let empty = assembler.constant_ref(entry, empty_octets, u8vec_type());
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![empty],
        inventory_cond(
            ValueRef::Parameter(frame_rows),
            append_framed,
            vec![operation_value(empty)],
            append_opaque,
            vec![operation_value(empty)],
        ),
    );

    let opaque_accumulator =
        assembler.parameter(append_opaque, ParameterRole::Block, 0, u8vec_type());
    let opaque = assembler.operation(
        append_opaque,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(rows),
            ValueRef::Parameter(opaque_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_counted_chunks,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_opaque,
        function,
        vec![opaque_accumulator],
        vec![opaque],
        inventory_switch(
            operation_value(opaque),
            vec![
                (BuiltinCase::Ok, convert, vec![SwitchArgument::CasePayload]),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let framed_accumulator =
        assembler.parameter(append_framed, ParameterRole::Block, 0, u8vec_type());
    let framed = assembler.operation(
        append_framed,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(rows),
            ValueRef::Parameter(framed_accumulator),
        ],
        byte_vector_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: append_counted_framed,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        append_framed,
        function,
        vec![framed_accumulator],
        vec![framed],
        inventory_switch(
            operation_value(framed),
            vec![
                (BuiltinCase::Ok, convert, vec![SwitchArgument::CasePayload]),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let section_octets = assembler.parameter(convert, ParameterRole::Block, 0, u8vec_type());
    let output_scope = assembler.constant_ref(convert, unit, TypeExpr::Unit);
    let section_bytes = assembler.operation(
        convert,
        Opcode::AdapterInvoke,
        vec![
            operation_value(output_scope),
            ValueRef::Parameter(section_octets),
        ],
        index_result_type(TypeExpr::Bytes),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    assembler.push_block(
        convert,
        function,
        vec![section_octets],
        vec![output_scope, section_bytes],
        inventory_switch(
            operation_value(section_bytes),
            vec![
                (
                    BuiltinCase::Ok,
                    success_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let section = assembler.parameter(success_block, ParameterRole::Block, 0, TypeExpr::Bytes);
    let success = assembler.operation(
        success_block,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(section)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success_block,
        function,
        vec![section],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![rows, frame_rows],
        result_type: bytes_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let bridge_adapter = |code, request_type, response_type| AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(code)),
        adapter_id: sley_vm::host_abi::bridge_identity(code),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type,
        response_type,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            append_counted_chunks_graph,
            append_counted_framed_graph,
            append_framed_section_graph,
            append_chunk_graph,
            append_u64_graph,
            narrow_u64_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![
            bridge_adapter(
                sley_vm::host_abi::BRIDGE_CODE_B2V1,
                TypeExpr::Bytes,
                u8vec_type(),
            ),
            bridge_adapter(sley_vm::host_abi::BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            bridge_adapter(
                sley_vm::host_abi::BRIDGE_CODE_V2B1,
                u8vec_type(),
                TypeExpr::Bytes,
            ),
        ],
    }
}

/// Composes a canonical `EXEC_PACKAGE_V2` envelope from exact section bytes and
/// their already-computed SHA-256 digests. Hash production is deliberately a
/// separate remaining slice; this function owns the fixed header and all five
/// `u64` section frames.
#[allow(clippy::too_many_lines)]
fn package_envelope_composer() -> LowerScaffold {
    let narrow_u32 = inventory_id(5, 19);
    let append_u32 = inventory_id(5, 20);
    let narrow_u64 = inventory_id(5, 21);
    let append_u64 = inventory_id(5, 22);
    let append_chunk = inventory_id(5, 24);
    let append_u32_sequence = inventory_id(5, 43);
    let append_digest_sequence = inventory_id(5, 47);
    let append_framed_section = inventory_id(5, 48);
    let function = inventory_id(5, 49);
    let mut assembler = InventoryAssembler::new();
    let narrow_u32_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u32, &u32_type(), u32_value);
    let append_u32_graph = build_fixed_width_appender(
        &mut assembler,
        append_u32,
        narrow_u32,
        &u32_type(),
        u32_value,
        &[1_u128 << 24, 1_u128 << 16, 1_u128 << 8, 1],
    );
    let narrow_u64_graph =
        build_unsigned_octet_narrower(&mut assembler, narrow_u64, &u64_type(), u64_value);
    let append_u64_graph = build_fixed_width_appender(
        &mut assembler,
        append_u64,
        narrow_u64,
        &u64_type(),
        u64_value,
        &[
            1_u128 << 56,
            1_u128 << 48,
            1_u128 << 40,
            1_u128 << 32,
            1_u128 << 24,
            1_u128 << 16,
            1_u128 << 8,
            1,
        ],
    );
    let append_chunk_graph = build_byte_chunk_appender(&mut assembler, append_chunk);
    let append_u32_sequence_graph = build_vector_byte_appender(
        &mut assembler,
        append_u32_sequence,
        append_u64,
        append_u32,
        &u32vec_type(),
        &u32_type(),
        true,
        false,
    );
    let append_digest_sequence_graph = build_vector_byte_appender(
        &mut assembler,
        append_digest_sequence,
        append_u64,
        append_chunk,
        &bytesvec_type(),
        &TypeExpr::Bytes,
        false,
        false,
    );
    let append_framed_section_graph = build_framed_byte_section_appender(
        &mut assembler,
        append_framed_section,
        append_u64,
        append_chunk,
    );

    let image = assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bytes);
    let constants = assembler.parameter(function, ParameterRole::Function, 1, TypeExpr::Bytes);
    let layouts = assembler.parameter(function, ParameterRole::Function, 2, TypeExpr::Bytes);
    let imports = assembler.parameter(function, ParameterRole::Function, 3, TypeExpr::Bytes);
    let dependency = assembler.parameter(function, ParameterRole::Function, 4, TypeExpr::Bytes);
    let image_digest = assembler.parameter(function, ParameterRole::Function, 5, TypeExpr::Bytes);
    let constants_digest =
        assembler.parameter(function, ParameterRole::Function, 6, TypeExpr::Bytes);
    let layouts_digest = assembler.parameter(function, ParameterRole::Function, 7, TypeExpr::Bytes);
    let imports_digest = assembler.parameter(function, ParameterRole::Function, 8, TypeExpr::Bytes);
    let dependency_digest =
        assembler.parameter(function, ParameterRole::Function, 9, TypeExpr::Bytes);
    let entry_identity =
        assembler.parameter(function, ParameterRole::Function, 10, TypeExpr::Bytes);
    let schema_epoch = assembler.parameter(function, ParameterRole::Function, 11, TypeExpr::Bytes);
    let state_root = assembler.parameter(function, ParameterRole::Function, 12, TypeExpr::Bytes);

    let start = assembler.block_id();
    let append_version = assembler.block_id();
    let append_profile = assembler.block_id();
    let append_abi = assembler.block_id();
    let append_vm = assembler.block_id();
    let append_digests = assembler.block_id();
    let append_entry = assembler.block_id();
    let append_epoch = assembler.block_id();
    let append_root = assembler.block_id();
    let append_image = assembler.block_id();
    let append_constants = assembler.block_id();
    let append_layouts = assembler.block_id();
    let append_imports = assembler.block_id();
    let append_dependency = assembler.block_id();
    let convert = assembler.block_id();
    let success_block = assembler.block_id();
    let forward_error = assembler.block_id();
    let resource_error = assembler.block_id();

    let empty_octets = assembler.constant(u8vec_value(&[]));
    let magic = assembler.constant(bytes_value(sley_vm::exec_package::EXEC_PACKAGE_MAGIC));
    let profile_digest = assembler.constant(bytes_value(&sley_vm::BOOTSTRAP_PROFILE_2_DIGEST));
    let zero_u32 = assembler.constant(u32_value(0));
    let one_u32 = assembler.constant(u32_value(1));
    let two_u32 = assembler.constant(u32_value(2));
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let initial = assembler.constant_ref(start, empty_octets, u8vec_type());
    let magic_bytes = assembler.constant_ref(start, magic, TypeExpr::Bytes);
    push_package_section_append_stage(
        &mut assembler,
        function,
        start,
        Vec::new(),
        vec![initial, magic_bytes],
        operation_value(initial),
        operation_value(magic_bytes),
        append_chunk,
        false,
        append_version,
        forward_error,
    );

    let version_accumulator =
        assembler.parameter(append_version, ParameterRole::Block, 0, u8vec_type());
    let version = assembler.constant_ref(append_version, two_u32, u32_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_version,
        vec![version_accumulator],
        vec![version],
        ValueRef::Parameter(version_accumulator),
        operation_value(version),
        append_u32,
        true,
        append_profile,
        forward_error,
    );

    let profile_accumulator =
        assembler.parameter(append_profile, ParameterRole::Block, 0, u8vec_type());
    let profile = assembler.constant_ref(append_profile, profile_digest, TypeExpr::Bytes);
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_profile,
        vec![profile_accumulator],
        vec![profile],
        ValueRef::Parameter(profile_accumulator),
        operation_value(profile),
        append_chunk,
        false,
        append_abi,
        forward_error,
    );

    let abi_accumulator = assembler.parameter(append_abi, ParameterRole::Block, 0, u8vec_type());
    let abi = assembler.constant_ref(append_abi, two_u32, u32_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_abi,
        vec![abi_accumulator],
        vec![abi],
        ValueRef::Parameter(abi_accumulator),
        operation_value(abi),
        append_u32,
        true,
        append_vm,
        forward_error,
    );

    let vm_accumulator = assembler.parameter(append_vm, ParameterRole::Block, 0, u8vec_type());
    let vm_one = assembler.constant_ref(append_vm, one_u32, u32_type());
    let vm_zero = assembler.constant_ref(append_vm, zero_u32, u32_type());
    let vm_words = assembler.operation(
        append_vm,
        Opcode::VectorNew,
        vec![
            operation_value(vm_one),
            operation_value(vm_zero),
            operation_value(vm_zero),
        ],
        u32vec_type(),
        Immediate::None,
    );
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_vm,
        vec![vm_accumulator],
        vec![vm_one, vm_zero, vm_words],
        ValueRef::Parameter(vm_accumulator),
        operation_value(vm_words),
        append_u32_sequence,
        true,
        append_digests,
        forward_error,
    );

    let digest_accumulator =
        assembler.parameter(append_digests, ParameterRole::Block, 0, u8vec_type());
    let digest_words = assembler.operation(
        append_digests,
        Opcode::VectorNew,
        vec![
            ValueRef::Parameter(image_digest),
            ValueRef::Parameter(constants_digest),
            ValueRef::Parameter(layouts_digest),
            ValueRef::Parameter(imports_digest),
            ValueRef::Parameter(dependency_digest),
        ],
        bytesvec_type(),
        Immediate::None,
    );
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_digests,
        vec![digest_accumulator],
        vec![digest_words],
        ValueRef::Parameter(digest_accumulator),
        operation_value(digest_words),
        append_digest_sequence,
        true,
        append_entry,
        forward_error,
    );

    let entry_accumulator =
        assembler.parameter(append_entry, ParameterRole::Block, 0, u8vec_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_entry,
        vec![entry_accumulator],
        Vec::new(),
        ValueRef::Parameter(entry_accumulator),
        ValueRef::Parameter(entry_identity),
        append_chunk,
        false,
        append_epoch,
        forward_error,
    );

    let epoch_accumulator =
        assembler.parameter(append_epoch, ParameterRole::Block, 0, u8vec_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_epoch,
        vec![epoch_accumulator],
        Vec::new(),
        ValueRef::Parameter(epoch_accumulator),
        ValueRef::Parameter(schema_epoch),
        append_chunk,
        false,
        append_root,
        forward_error,
    );

    let root_accumulator = assembler.parameter(append_root, ParameterRole::Block, 0, u8vec_type());
    push_package_section_append_stage(
        &mut assembler,
        function,
        append_root,
        vec![root_accumulator],
        Vec::new(),
        ValueRef::Parameter(root_accumulator),
        ValueRef::Parameter(state_root),
        append_chunk,
        false,
        append_image,
        forward_error,
    );

    for (block, next, section) in [
        (append_image, append_constants, image),
        (append_constants, append_layouts, constants),
        (append_layouts, append_imports, layouts),
        (append_imports, append_dependency, imports),
        (append_dependency, convert, dependency),
    ] {
        let accumulator = assembler.parameter(block, ParameterRole::Block, 0, u8vec_type());
        push_package_section_append_stage(
            &mut assembler,
            function,
            block,
            vec![accumulator],
            Vec::new(),
            ValueRef::Parameter(accumulator),
            ValueRef::Parameter(section),
            append_framed_section,
            true,
            next,
            forward_error,
        );
    }

    let envelope_octets = assembler.parameter(convert, ParameterRole::Block, 0, u8vec_type());
    let output_scope = assembler.constant_ref(convert, unit, TypeExpr::Unit);
    let envelope = assembler.operation(
        convert,
        Opcode::AdapterInvoke,
        vec![
            operation_value(output_scope),
            ValueRef::Parameter(envelope_octets),
        ],
        index_result_type(TypeExpr::Bytes),
        Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    assembler.push_block(
        convert,
        function,
        vec![envelope_octets],
        vec![output_scope, envelope],
        inventory_switch(
            operation_value(envelope),
            vec![
                (
                    BuiltinCase::Ok,
                    success_block,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let encoded = assembler.parameter(success_block, ParameterRole::Block, 0, TypeExpr::Bytes);
    let success = assembler.operation(
        success_block,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(encoded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success_block,
        function,
        vec![encoded],
        vec![success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    let resource = assembler.constant_ref(resource_error, resource_code, u32_type());
    let resource_failure = assembler.operation(
        resource_error,
        Opcode::ResultErr,
        vec![operation_value(resource)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        resource_error,
        function,
        Vec::new(),
        vec![resource, resource_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(resource_failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            image,
            constants,
            layouts,
            imports,
            dependency,
            image_digest,
            constants_digest,
            layouts_digest,
            imports_digest,
            dependency_digest,
            entry_identity,
            schema_epoch,
            state_root,
        ],
        result_type: bytes_lower_result_type(),
        effects: Vec::new(),
        entry_block: start,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let bridge_adapter = |code, request_type, response_type| AdapterImport {
        entity_id: EntityId::from_bytes(sley_vm::host_abi::bridge_identity(code)),
        adapter_id: sley_vm::host_abi::bridge_identity(code),
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type,
        response_type,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![
            graph,
            append_framed_section_graph,
            append_digest_sequence_graph,
            append_u32_sequence_graph,
            append_chunk_graph,
            append_u32_graph,
            narrow_u32_graph,
            append_u64_graph,
            narrow_u64_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: vec![
            bridge_adapter(
                sley_vm::host_abi::BRIDGE_CODE_B2V1,
                TypeExpr::Bytes,
                u8vec_type(),
            ),
            bridge_adapter(sley_vm::host_abi::BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            bridge_adapter(
                sley_vm::host_abi::BRIDGE_CODE_V2B1,
                u8vec_type(),
                TypeExpr::Bytes,
            ),
        ],
    }
}

/// Canonical package-builder entry over the admitted section and envelope
/// closures. Canonical checked row bodies and host-mechanic SHA-256 results
/// are explicit inputs; Sley owns counts, frames, dependency bytes, and the
/// final `EXEC_PACKAGE_V2` composition in one invocation.
#[allow(clippy::too_many_lines)]
fn package_builder() -> LowerScaffold {
    let inventory = rebase_scaffold_function_namespace(
        rebase_scaffold_artifacts(package_inventory_section_encoder(), 32),
        6,
    );
    let dependency = rebase_scaffold_function_namespace(
        rebase_scaffold_artifacts(package_dependency_sections_encoder(), 64),
        7,
    );
    let composer = rebase_scaffold_function_namespace(
        rebase_scaffold_artifacts(package_envelope_composer(), 96),
        8,
    );
    let inventory_entry = inventory.entry.entity_id;
    let dependency_entry = dependency.entry.entity_id;
    let composer_entry = composer.entry.entity_id;
    let adapters = inventory.adapters.clone();
    assert_eq!(dependency.adapters, adapters);
    assert_eq!(composer.adapters, adapters);

    let function = inventory_id(9, 1);
    let mut assembler = InventoryAssembler::new();
    let image = assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bytes);
    let constant_rows = assembler.parameter(function, ParameterRole::Function, 1, bytesvec_type());
    let layout_rows = assembler.parameter(function, ParameterRole::Function, 2, bytesvec_type());
    let import_rows = assembler.parameter(function, ParameterRole::Function, 3, bytesvec_type());
    let entry_identity = assembler.parameter(function, ParameterRole::Function, 4, TypeExpr::Bytes);
    let schema_epoch = assembler.parameter(function, ParameterRole::Function, 5, TypeExpr::Bytes);
    let state_root = assembler.parameter(function, ParameterRole::Function, 6, TypeExpr::Bytes);
    let gate_operations = assembler.parameter(function, ParameterRole::Function, 7, u32_type());
    let gate_bridges = assembler.parameter(function, ParameterRole::Function, 8, u32_type());
    let fingerprints = assembler.parameter(function, ParameterRole::Function, 9, bytesvec_type());
    let max_instructions = assembler.parameter(function, ParameterRole::Function, 10, u64_type());
    let max_fuel = assembler.parameter(function, ParameterRole::Function, 11, u64_type());
    let max_value_units = assembler.parameter(function, ParameterRole::Function, 12, u64_type());
    let max_output_units = assembler.parameter(function, ParameterRole::Function, 13, u64_type());
    let global_rows = assembler.parameter(function, ParameterRole::Function, 14, bytesvec_type());
    let contract_rows = assembler.parameter(function, ParameterRole::Function, 15, bytesvec_type());
    let cancel_present = assembler.parameter(function, ParameterRole::Function, 16, TypeExpr::Bool);
    let cancel_at_fuel = assembler.parameter(function, ParameterRole::Function, 17, u64_type());
    let image_digest = assembler.parameter(function, ParameterRole::Function, 18, TypeExpr::Bytes);
    let constants_digest =
        assembler.parameter(function, ParameterRole::Function, 19, TypeExpr::Bytes);
    let layouts_digest =
        assembler.parameter(function, ParameterRole::Function, 20, TypeExpr::Bytes);
    let imports_digest =
        assembler.parameter(function, ParameterRole::Function, 21, TypeExpr::Bytes);
    let dependency_digest =
        assembler.parameter(function, ParameterRole::Function, 22, TypeExpr::Bytes);

    let build_constants = assembler.block_id();
    let build_layouts = assembler.block_id();
    let build_imports = assembler.block_id();
    let build_dependency = assembler.block_id();
    let compose = assembler.block_id();
    let forward_error = assembler.block_id();
    let false_constant = assembler.constant(bool_value(false));
    let true_constant = assembler.constant(bool_value(true));

    let no_frame = assembler.constant_ref(build_constants, false_constant, TypeExpr::Bool);
    let constants_result = assembler.operation(
        build_constants,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(constant_rows),
            operation_value(no_frame),
        ],
        bytes_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: inventory_entry,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        build_constants,
        function,
        Vec::new(),
        vec![no_frame, constants_result],
        inventory_switch(
            operation_value(constants_result),
            vec![
                (
                    BuiltinCase::Ok,
                    build_layouts,
                    vec![SwitchArgument::CasePayload],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let constants_section =
        assembler.parameter(build_layouts, ParameterRole::Block, 0, TypeExpr::Bytes);
    let frame_layouts = assembler.constant_ref(build_layouts, true_constant, TypeExpr::Bool);
    let layouts_result = assembler.operation(
        build_layouts,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(layout_rows),
            operation_value(frame_layouts),
        ],
        bytes_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: inventory_entry,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        build_layouts,
        function,
        vec![constants_section],
        vec![frame_layouts, layouts_result],
        inventory_switch(
            operation_value(layouts_result),
            vec![
                (
                    BuiltinCase::Ok,
                    build_imports,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(constants_section)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let carried_constants =
        assembler.parameter(build_imports, ParameterRole::Block, 0, TypeExpr::Bytes);
    let layouts_section =
        assembler.parameter(build_imports, ParameterRole::Block, 1, TypeExpr::Bytes);
    let frame_imports = assembler.constant_ref(build_imports, true_constant, TypeExpr::Bool);
    let imports_result = assembler.operation(
        build_imports,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(import_rows),
            operation_value(frame_imports),
        ],
        bytes_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: inventory_entry,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        build_imports,
        function,
        vec![carried_constants, layouts_section],
        vec![frame_imports, imports_result],
        inventory_switch(
            operation_value(imports_result),
            vec![
                (
                    BuiltinCase::Ok,
                    build_dependency,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(carried_constants)),
                        SwitchArgument::Value(ValueRef::Parameter(layouts_section)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let dependency_constants =
        assembler.parameter(build_dependency, ParameterRole::Block, 0, TypeExpr::Bytes);
    let dependency_layouts =
        assembler.parameter(build_dependency, ParameterRole::Block, 1, TypeExpr::Bytes);
    let imports_section =
        assembler.parameter(build_dependency, ParameterRole::Block, 2, TypeExpr::Bytes);
    let dependency_result = assembler.operation(
        build_dependency,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(entry_identity),
            ValueRef::Parameter(schema_epoch),
            ValueRef::Parameter(state_root),
            ValueRef::Parameter(gate_operations),
            ValueRef::Parameter(gate_bridges),
            ValueRef::Parameter(fingerprints),
            ValueRef::Parameter(max_instructions),
            ValueRef::Parameter(max_fuel),
            ValueRef::Parameter(max_value_units),
            ValueRef::Parameter(max_output_units),
            ValueRef::Parameter(global_rows),
            ValueRef::Parameter(contract_rows),
            ValueRef::Parameter(cancel_present),
            ValueRef::Parameter(cancel_at_fuel),
        ],
        package_sections_result_type(),
        Immediate::Function(FunctionRefValue {
            function: dependency_entry,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        build_dependency,
        function,
        vec![dependency_constants, dependency_layouts, imports_section],
        vec![dependency_result],
        inventory_switch(
            operation_value(dependency_result),
            vec![
                (
                    BuiltinCase::Ok,
                    compose,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(dependency_constants)),
                        SwitchArgument::Value(ValueRef::Parameter(dependency_layouts)),
                        SwitchArgument::Value(ValueRef::Parameter(imports_section)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let compose_constants = assembler.parameter(compose, ParameterRole::Block, 0, TypeExpr::Bytes);
    let compose_layouts = assembler.parameter(compose, ParameterRole::Block, 1, TypeExpr::Bytes);
    let compose_imports = assembler.parameter(compose, ParameterRole::Block, 2, TypeExpr::Bytes);
    let dependency_sections =
        assembler.parameter(compose, ParameterRole::Block, 3, package_sections_type());
    let dependency_section = assembler.operation(
        compose,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(dependency_sections)],
        TypeExpr::Bytes,
        Immediate::Index(3),
    );
    let envelope = assembler.operation(
        compose,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(image),
            ValueRef::Parameter(compose_constants),
            ValueRef::Parameter(compose_layouts),
            ValueRef::Parameter(compose_imports),
            operation_value(dependency_section),
            ValueRef::Parameter(image_digest),
            ValueRef::Parameter(constants_digest),
            ValueRef::Parameter(layouts_digest),
            ValueRef::Parameter(imports_digest),
            ValueRef::Parameter(dependency_digest),
            ValueRef::Parameter(entry_identity),
            ValueRef::Parameter(schema_epoch),
            ValueRef::Parameter(state_root),
        ],
        bytes_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: composer_entry,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        compose,
        function,
        vec![
            compose_constants,
            compose_layouts,
            compose_imports,
            dependency_sections,
        ],
        vec![dependency_section, envelope],
        Terminator::Return(ReturnTerminator {
            value: operation_value(envelope),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        bytes_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(failure),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            image,
            constant_rows,
            layout_rows,
            import_rows,
            entry_identity,
            schema_epoch,
            state_root,
            gate_operations,
            gate_bridges,
            fingerprints,
            max_instructions,
            max_fuel,
            max_value_units,
            max_output_units,
            global_rows,
            contract_rows,
            cancel_present,
            cancel_at_fuel,
            image_digest,
            constants_digest,
            layouts_digest,
            imports_digest,
            dependency_digest,
        ],
        result_type: bytes_lower_result_type(),
        effects: Vec::new(),
        entry_block: build_constants,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };

    let mut functions = vec![graph.clone()];
    functions.extend(inventory.functions);
    functions.extend(dependency.functions);
    functions.extend(composer.functions);
    let mut parameters = assembler.parameters;
    parameters.extend(inventory.parameters);
    parameters.extend(dependency.parameters);
    parameters.extend(composer.parameters);
    let mut blocks = assembler.blocks;
    blocks.extend(inventory.blocks);
    blocks.extend(dependency.blocks);
    blocks.extend(composer.blocks);
    let mut operations = assembler.operations;
    operations.extend(inventory.operations);
    operations.extend(dependency.operations);
    operations.extend(composer.operations);
    let mut constants = assembler.constants;
    constants.extend(inventory.constants);
    constants.extend(dependency.constants);
    constants.extend(composer.constants);
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph,
        functions,
        parameters,
        blocks,
        operations,
        constants,
        adapters,
    }
}

fn append_terminator_success_block(
    assembler: &mut InventoryAssembler,
    function: EntityId,
    block: EntityId,
    parameters: Vec<EntityId>,
    operations: Vec<EntityId>,
    fields: &[ValueRef; 7],
) {
    let model = assembler.operation(
        block,
        Opcode::TupleNew,
        fields.to_vec(),
        terminator_model_type(),
        Immediate::None,
    );
    let success = assembler.operation(
        block,
        Opcode::ResultOk,
        vec![operation_value(model)],
        terminator_result_type(),
        Immediate::None,
    );
    let mut block_operations = operations;
    block_operations.extend([model, success]);
    assembler.push_block(
        block,
        function,
        parameters,
        block_operations,
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
}

/// Lowers return, branch, conditional-branch, and trap terminators from
/// runtime dense-register/block facts. Edge argument vectors are traversed by
/// the Sley helper above, so a late invalid argument cannot be skipped.
#[allow(clippy::too_many_lines)]
fn simple_terminator_lowerer() -> LowerScaffold {
    let function = inventory_id(5, 3);
    let validator = inventory_id(5, 4);
    let mut assembler = InventoryAssembler::new();
    let validator_graph = build_register_vector_validator(&mut assembler, validator);

    let kind = assembler.parameter(function, ParameterRole::Function, 0, u32_type());
    let primary = assembler.parameter(function, ParameterRole::Function, 1, u32_type());
    let target_zero = assembler.parameter(function, ParameterRole::Function, 2, u32_type());
    let arguments_zero = assembler.parameter(function, ParameterRole::Function, 3, u32vec_type());
    let target_one = assembler.parameter(function, ParameterRole::Function, 4, u32_type());
    let arguments_one = assembler.parameter(function, ParameterRole::Function, 5, u32vec_type());
    let payload = assembler.parameter(function, ParameterRole::Function, 6, optional_u32_type());
    let register_count = assembler.parameter(function, ParameterRole::Function, 7, u32_type());
    let block_count = assembler.parameter(function, ParameterRole::Function, 8, u32_type());

    let entry = assembler.block_id();
    let kind_branch = assembler.block_id();
    let kind_cond = assembler.block_id();
    let kind_trap = assembler.block_id();
    let return_check = assembler.block_id();
    let branch_target_check = assembler.block_id();
    let branch_arguments_call = assembler.block_id();
    let cond_register_check = assembler.block_id();
    let cond_target_zero_check = assembler.block_id();
    let cond_target_one_check = assembler.block_id();
    let cond_arguments_zero_call = assembler.block_id();
    let cond_arguments_one_call = assembler.block_id();
    let trap_code_low_check = assembler.block_id();
    let trap_code_high_check = assembler.block_id();
    let trap_payload_switch = assembler.block_id();
    let trap_payload_check = assembler.block_id();
    let emit_return = assembler.block_id();
    let emit_branch = assembler.block_id();
    let emit_cond = assembler.block_id();
    let emit_trap_none = assembler.block_id();
    let emit_trap_some = assembler.block_id();
    let forward_error = assembler.block_id();
    let unsupported_error = assembler.block_id();
    let local_error = assembler.block_id();

    let return_tag = assembler.constant(u32_value(1));
    let branch_tag = assembler.constant(u32_value(2));
    let cond_tag = assembler.constant(u32_value(3));
    let trap_tag = assembler.constant(u32_value(5));
    let zero = assembler.constant(u32_value(0));
    let unsupported_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::OpcodeUnsupported.numeric(),
    )));
    let local_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    )));

    let dispatch_block = |assembler: &mut InventoryAssembler,
                          block: EntityId,
                          tag: EntityId,
                          matched: EntityId,
                          unmatched: EntityId| {
        let tag_value = assembler.constant_ref(block, tag, u32_type());
        let equal = assembler.operation(
            block,
            Opcode::Equal,
            vec![ValueRef::Parameter(kind), operation_value(tag_value)],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![tag_value, equal],
            inventory_cond(
                operation_value(equal),
                matched,
                Vec::new(),
                unmatched,
                Vec::new(),
            ),
        );
    };
    dispatch_block(&mut assembler, entry, return_tag, return_check, kind_branch);
    dispatch_block(
        &mut assembler,
        kind_branch,
        branch_tag,
        branch_target_check,
        kind_cond,
    );
    dispatch_block(
        &mut assembler,
        kind_cond,
        cond_tag,
        cond_register_check,
        kind_trap,
    );
    dispatch_block(
        &mut assembler,
        kind_trap,
        trap_tag,
        trap_code_low_check,
        unsupported_error,
    );

    let return_valid = assembler.operation(
        return_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(primary),
            ValueRef::Parameter(register_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        return_check,
        function,
        Vec::new(),
        vec![return_valid],
        inventory_cond(
            operation_value(return_valid),
            emit_return,
            Vec::new(),
            local_error,
            Vec::new(),
        ),
    );

    let branch_target_valid = assembler.operation(
        branch_target_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(target_zero),
            ValueRef::Parameter(block_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        branch_target_check,
        function,
        Vec::new(),
        vec![branch_target_valid],
        inventory_cond(
            operation_value(branch_target_valid),
            branch_arguments_call,
            Vec::new(),
            local_error,
            Vec::new(),
        ),
    );

    let branch_arguments = assembler.operation(
        branch_arguments_call,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(arguments_zero),
            ValueRef::Parameter(register_count),
        ],
        unit_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: validator,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        branch_arguments_call,
        function,
        Vec::new(),
        vec![branch_arguments],
        inventory_switch(
            operation_value(branch_arguments),
            vec![
                (BuiltinCase::Ok, emit_branch, Vec::new()),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let cond_register_valid = assembler.operation(
        cond_register_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(primary),
            ValueRef::Parameter(register_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        cond_register_check,
        function,
        Vec::new(),
        vec![cond_register_valid],
        inventory_cond(
            operation_value(cond_register_valid),
            cond_target_zero_check,
            Vec::new(),
            local_error,
            Vec::new(),
        ),
    );
    for (block, target, success) in [
        (cond_target_zero_check, target_zero, cond_target_one_check),
        (cond_target_one_check, target_one, cond_arguments_zero_call),
    ] {
        let valid = assembler.operation(
            block,
            Opcode::LessThan,
            vec![
                ValueRef::Parameter(target),
                ValueRef::Parameter(block_count),
            ],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![valid],
            inventory_cond(
                operation_value(valid),
                success,
                Vec::new(),
                local_error,
                Vec::new(),
            ),
        );
    }

    for (block, arguments, success) in [
        (
            cond_arguments_zero_call,
            arguments_zero,
            cond_arguments_one_call,
        ),
        (cond_arguments_one_call, arguments_one, emit_cond),
    ] {
        let call = assembler.operation(
            block,
            Opcode::CallDirect,
            vec![
                ValueRef::Parameter(arguments),
                ValueRef::Parameter(register_count),
            ],
            unit_lower_result_type(),
            Immediate::Function(FunctionRefValue {
                function: validator,
                type_arguments: Vec::new(),
            }),
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![call],
            inventory_switch(
                operation_value(call),
                vec![
                    (BuiltinCase::Ok, success, Vec::new()),
                    (
                        BuiltinCase::Err,
                        forward_error,
                        vec![SwitchArgument::CasePayload],
                    ),
                ],
            ),
        );
    }

    let trap_zero = assembler.constant_ref(trap_code_low_check, zero, u32_type());
    let trap_above_zero = assembler.operation(
        trap_code_low_check,
        Opcode::LessThan,
        vec![operation_value(trap_zero), ValueRef::Parameter(primary)],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        trap_code_low_check,
        function,
        Vec::new(),
        vec![trap_zero, trap_above_zero],
        inventory_cond(
            operation_value(trap_above_zero),
            trap_code_high_check,
            Vec::new(),
            local_error,
            Vec::new(),
        ),
    );
    let trap_five = assembler.constant_ref(trap_code_high_check, trap_tag, u32_type());
    let trap_below_five = assembler.operation(
        trap_code_high_check,
        Opcode::LessThan,
        vec![ValueRef::Parameter(primary), operation_value(trap_five)],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        trap_code_high_check,
        function,
        Vec::new(),
        vec![trap_five, trap_below_five],
        inventory_cond(
            operation_value(trap_below_five),
            trap_payload_switch,
            Vec::new(),
            local_error,
            Vec::new(),
        ),
    );
    assembler.push_block(
        trap_payload_switch,
        function,
        Vec::new(),
        Vec::new(),
        inventory_switch(
            ValueRef::Parameter(payload),
            vec![
                (BuiltinCase::None, emit_trap_none, Vec::new()),
                (
                    BuiltinCase::Some,
                    trap_payload_check,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );
    let trap_payload_value =
        assembler.parameter(trap_payload_check, ParameterRole::Block, 0, u32_type());
    let trap_payload_valid = assembler.operation(
        trap_payload_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(trap_payload_value),
            ValueRef::Parameter(register_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        trap_payload_check,
        function,
        vec![trap_payload_value],
        vec![trap_payload_valid],
        inventory_cond(
            operation_value(trap_payload_valid),
            emit_trap_some,
            vec![ValueRef::Parameter(trap_payload_value)],
            local_error,
            Vec::new(),
        ),
    );

    let emit_without_payload = |assembler: &mut InventoryAssembler,
                                block: EntityId,
                                tag: EntityId,
                                primary_value: ValueRef,
                                target_zero_value: ValueRef,
                                arguments_zero_value: ValueRef,
                                target_one_value: ValueRef,
                                arguments_one_value: ValueRef,
                                mut operations: Vec<EntityId>| {
        let tag_value = assembler.constant_ref(block, tag, u32_type());
        let none = assembler.operation(
            block,
            Opcode::OptionNone,
            Vec::new(),
            optional_u32_type(),
            Immediate::None,
        );
        operations.extend([tag_value, none]);
        append_terminator_success_block(
            assembler,
            function,
            block,
            Vec::new(),
            operations,
            &[
                operation_value(tag_value),
                primary_value,
                target_zero_value,
                arguments_zero_value,
                target_one_value,
                arguments_one_value,
                operation_value(none),
            ],
        );
    };

    let return_zero = assembler.constant_ref(emit_return, zero, u32_type());
    let return_args_zero = assembler.operation(
        emit_return,
        Opcode::VectorNew,
        Vec::new(),
        u32vec_type(),
        Immediate::None,
    );
    let return_args_one = assembler.operation(
        emit_return,
        Opcode::VectorNew,
        Vec::new(),
        u32vec_type(),
        Immediate::None,
    );
    emit_without_payload(
        &mut assembler,
        emit_return,
        return_tag,
        ValueRef::Parameter(primary),
        operation_value(return_zero),
        operation_value(return_args_zero),
        operation_value(return_zero),
        operation_value(return_args_one),
        vec![return_zero, return_args_zero, return_args_one],
    );

    let branch_zero = assembler.constant_ref(emit_branch, zero, u32_type());
    let branch_args_one = assembler.operation(
        emit_branch,
        Opcode::VectorNew,
        Vec::new(),
        u32vec_type(),
        Immediate::None,
    );
    emit_without_payload(
        &mut assembler,
        emit_branch,
        branch_tag,
        operation_value(branch_zero),
        ValueRef::Parameter(target_zero),
        ValueRef::Parameter(arguments_zero),
        operation_value(branch_zero),
        operation_value(branch_args_one),
        vec![branch_zero, branch_args_one],
    );

    emit_without_payload(
        &mut assembler,
        emit_cond,
        cond_tag,
        ValueRef::Parameter(primary),
        ValueRef::Parameter(target_zero),
        ValueRef::Parameter(arguments_zero),
        ValueRef::Parameter(target_one),
        ValueRef::Parameter(arguments_one),
        Vec::new(),
    );

    let trap_none_zero = assembler.constant_ref(emit_trap_none, zero, u32_type());
    let trap_none_args_zero = assembler.operation(
        emit_trap_none,
        Opcode::VectorNew,
        Vec::new(),
        u32vec_type(),
        Immediate::None,
    );
    let trap_none_args_one = assembler.operation(
        emit_trap_none,
        Opcode::VectorNew,
        Vec::new(),
        u32vec_type(),
        Immediate::None,
    );
    emit_without_payload(
        &mut assembler,
        emit_trap_none,
        trap_tag,
        ValueRef::Parameter(primary),
        operation_value(trap_none_zero),
        operation_value(trap_none_args_zero),
        operation_value(trap_none_zero),
        operation_value(trap_none_args_one),
        vec![trap_none_zero, trap_none_args_zero, trap_none_args_one],
    );

    let trap_some_value = assembler.parameter(emit_trap_some, ParameterRole::Block, 0, u32_type());
    let trap_some_zero = assembler.constant_ref(emit_trap_some, zero, u32_type());
    let trap_some_args_zero = assembler.operation(
        emit_trap_some,
        Opcode::VectorNew,
        Vec::new(),
        u32vec_type(),
        Immediate::None,
    );
    let trap_some_args_one = assembler.operation(
        emit_trap_some,
        Opcode::VectorNew,
        Vec::new(),
        u32vec_type(),
        Immediate::None,
    );
    let trap_some_payload = assembler.operation(
        emit_trap_some,
        Opcode::OptionSome,
        vec![ValueRef::Parameter(trap_some_value)],
        optional_u32_type(),
        Immediate::None,
    );
    let trap_some_tag = assembler.constant_ref(emit_trap_some, trap_tag, u32_type());
    append_terminator_success_block(
        &mut assembler,
        function,
        emit_trap_some,
        vec![trap_some_value],
        vec![
            trap_some_zero,
            trap_some_args_zero,
            trap_some_args_one,
            trap_some_payload,
            trap_some_tag,
        ],
        &[
            operation_value(trap_some_tag),
            ValueRef::Parameter(primary),
            operation_value(trap_some_zero),
            operation_value(trap_some_args_zero),
            operation_value(trap_some_zero),
            operation_value(trap_some_args_one),
            operation_value(trap_some_payload),
        ],
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        terminator_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    for (block, code) in [
        (unsupported_error, unsupported_code),
        (local_error, local_code),
    ] {
        let code_value = assembler.constant_ref(block, code, u32_type());
        let failure = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![operation_value(code_value)],
            terminator_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![code_value, failure],
            Terminator::Return(ReturnTerminator {
                value: operation_value(failure),
            }),
        );
    }

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            kind,
            primary,
            target_zero,
            arguments_zero,
            target_one,
            arguments_one,
            payload,
            register_count,
            block_count,
        ],
        result_type: terminator_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph, validator_graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: Vec::new(),
    }
}

#[allow(clippy::too_many_lines)]
fn build_switch_argument_validator(
    assembler: &mut InventoryAssembler,
    function: EntityId,
) -> FunctionGraph {
    let arguments = assembler.parameter(
        function,
        ParameterRole::Function,
        0,
        switch_argument_facts_type(),
    );
    let register_count = assembler.parameter(function, ParameterRole::Function, 1, u32_type());

    let entry = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let unpack = assembler.block_id();
    let value_tag = assembler.block_id();
    let payload_tag = assembler.block_id();
    let value_reference = assembler.block_id();
    let advance = assembler.block_id();
    let done = assembler.block_id();
    let signature_error = assembler.block_id();
    let local_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();

    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let value_tag_constant = assembler.constant(u32_value(1));
    let payload_tag_constant = assembler.constant(u32_value(2));
    let unit = assembler.constant(ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    });
    let signature_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
    )));
    let local_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    )));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let zero = assembler.constant_ref(entry, zero_u64, u64_type());
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![zero],
        inventory_branch(check, vec![operation_value(zero)]),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let length = assembler.operation(
        check,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(arguments)],
        u64_type(),
        Immediate::None,
    );
    let has_argument = assembler.operation(
        check,
        Opcode::LessThan,
        vec![ValueRef::Parameter(check_index), operation_value(length)],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        vec![check_index],
        vec![length, has_argument],
        inventory_cond(
            operation_value(has_argument),
            get,
            vec![ValueRef::Parameter(check_index)],
            done,
            Vec::new(),
        ),
    );

    let get_index = assembler.parameter(get, ParameterRole::Block, 0, u64_type());
    let argument = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(arguments),
            ValueRef::Parameter(get_index),
        ],
        TypeExpr::Option(Box::new(switch_argument_fact_type())),
        Immediate::None,
    );
    assembler.push_block(
        get,
        function,
        vec![get_index],
        vec![argument],
        inventory_switch(
            operation_value(argument),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                    ],
                ),
            ],
        ),
    );

    let unpack_argument =
        assembler.parameter(unpack, ParameterRole::Block, 0, switch_argument_fact_type());
    let unpack_index = assembler.parameter(unpack, ParameterRole::Block, 1, u64_type());
    let tag = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(unpack_argument)],
        u32_type(),
        Immediate::Index(0),
    );
    let value = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(unpack_argument)],
        u32_type(),
        Immediate::Index(1),
    );
    assembler.push_block(
        unpack,
        function,
        vec![unpack_argument, unpack_index],
        vec![tag, value],
        inventory_branch(
            value_tag,
            vec![
                operation_value(tag),
                operation_value(value),
                ValueRef::Parameter(unpack_index),
            ],
        ),
    );

    let tag_block = |assembler: &mut InventoryAssembler,
                     block: EntityId,
                     expected: EntityId,
                     matched: EntityId,
                     unmatched: EntityId| {
        let found_tag = assembler.parameter(block, ParameterRole::Block, 0, u32_type());
        let found_value = assembler.parameter(block, ParameterRole::Block, 1, u32_type());
        let found_index = assembler.parameter(block, ParameterRole::Block, 2, u64_type());
        let expected_value = assembler.constant_ref(block, expected, u32_type());
        let equal = assembler.operation(
            block,
            Opcode::Equal,
            vec![
                ValueRef::Parameter(found_tag),
                operation_value(expected_value),
            ],
            TypeExpr::Bool,
            Immediate::None,
        );
        let next_values = vec![
            ValueRef::Parameter(found_value),
            ValueRef::Parameter(found_index),
        ];
        assembler.push_block(
            block,
            function,
            vec![found_tag, found_value, found_index],
            vec![expected_value, equal],
            inventory_cond(
                operation_value(equal),
                matched,
                next_values,
                unmatched,
                if unmatched == signature_error {
                    Vec::new()
                } else {
                    vec![
                        ValueRef::Parameter(found_tag),
                        ValueRef::Parameter(found_value),
                        ValueRef::Parameter(found_index),
                    ]
                },
            ),
        );
    };
    tag_block(
        assembler,
        value_tag,
        value_tag_constant,
        value_reference,
        payload_tag,
    );
    tag_block(
        assembler,
        payload_tag,
        payload_tag_constant,
        advance,
        signature_error,
    );

    let reference = assembler.parameter(value_reference, ParameterRole::Block, 0, u32_type());
    let reference_index = assembler.parameter(value_reference, ParameterRole::Block, 1, u64_type());
    let reference_valid = assembler.operation(
        value_reference,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(reference),
            ValueRef::Parameter(register_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        value_reference,
        function,
        vec![reference, reference_index],
        vec![reference_valid],
        inventory_cond(
            operation_value(reference_valid),
            advance,
            vec![
                ValueRef::Parameter(reference),
                ValueRef::Parameter(reference_index),
            ],
            local_error,
            Vec::new(),
        ),
    );

    let advance_value = assembler.parameter(advance, ParameterRole::Block, 0, u32_type());
    let advance_index = assembler.parameter(advance, ParameterRole::Block, 1, u64_type());
    let one = assembler.constant_ref(advance, one_u64, u64_type());
    let next_index = assembler.operation(
        advance,
        Opcode::IntAddChecked,
        vec![ValueRef::Parameter(advance_index), operation_value(one)],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance,
        function,
        vec![advance_value, advance_index],
        vec![one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (BuiltinCase::Ok, check, vec![SwitchArgument::CasePayload]),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let unit_value = assembler.constant_ref(done, unit, TypeExpr::Unit);
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![operation_value(unit_value)],
        unit_lower_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        Vec::new(),
        vec![unit_value, success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );
    for (block, code) in [
        (signature_error, signature_code),
        (local_error, local_code),
        (resource_error, resource_code),
    ] {
        let code_value = assembler.constant_ref(block, code, u32_type());
        let failure = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![operation_value(code_value)],
            unit_lower_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![code_value, failure],
            Terminator::Return(ReturnTerminator {
                value: operation_value(failure),
            }),
        );
    }
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );

    FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![arguments, register_count],
        result_type: unit_lower_result_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

/// Lowers the complete runtime case inventory for a built-in variant switch.
/// Case order and payload markers are preserved, while every selector,
/// target, and ordinary argument is checked against its dense inventory.
#[allow(clippy::too_many_lines)]
fn builtin_variant_switch_lowerer() -> LowerScaffold {
    let function = inventory_id(5, 5);
    let argument_validator = inventory_id(5, 6);
    let mut assembler = InventoryAssembler::new();
    let validator_graph = build_switch_argument_validator(&mut assembler, argument_validator);

    let selector = assembler.parameter(function, ParameterRole::Function, 0, u32_type());
    let cases = assembler.parameter(
        function,
        ParameterRole::Function,
        1,
        builtin_switch_case_facts_type(),
    );
    let register_count = assembler.parameter(function, ParameterRole::Function, 2, u32_type());
    let block_count = assembler.parameter(function, ParameterRole::Function, 3, u32_type());

    let selector_check = assembler.block_id();
    let start = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let unpack = assembler.block_id();
    let key_low_check = assembler.block_id();
    let key_high_check = assembler.block_id();
    let target_check = assembler.block_id();
    let arguments_call = assembler.block_id();
    let advance = assembler.block_id();
    let done = assembler.block_id();
    let forward_error = assembler.block_id();
    let signature_error = assembler.block_id();
    let local_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();

    let zero_u64 = assembler.constant(u64_value(0));
    let one_u64 = assembler.constant(u64_value(1));
    let zero_u32 = assembler.constant(u32_value(0));
    let five_u32 = assembler.constant(u32_value(5));
    let signature_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
    )));
    let local_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    )));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    )));

    let selector_valid = assembler.operation(
        selector_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(selector),
            ValueRef::Parameter(register_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        selector_check,
        function,
        Vec::new(),
        vec![selector_valid],
        inventory_cond(
            operation_value(selector_valid),
            start,
            Vec::new(),
            local_error,
            Vec::new(),
        ),
    );

    let zero = assembler.constant_ref(start, zero_u64, u64_type());
    assembler.push_block(
        start,
        function,
        Vec::new(),
        vec![zero],
        inventory_branch(check, vec![operation_value(zero)]),
    );

    let check_index = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let length = assembler.operation(
        check,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(cases)],
        u64_type(),
        Immediate::None,
    );
    let has_case = assembler.operation(
        check,
        Opcode::LessThan,
        vec![ValueRef::Parameter(check_index), operation_value(length)],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        vec![check_index],
        vec![length, has_case],
        inventory_cond(
            operation_value(has_case),
            get,
            vec![ValueRef::Parameter(check_index)],
            done,
            Vec::new(),
        ),
    );

    let get_index = assembler.parameter(get, ParameterRole::Block, 0, u64_type());
    let case = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![ValueRef::Parameter(cases), ValueRef::Parameter(get_index)],
        TypeExpr::Option(Box::new(builtin_switch_case_fact_type())),
        Immediate::None,
    );
    assembler.push_block(
        get,
        function,
        vec![get_index],
        vec![case],
        inventory_switch(
            operation_value(case),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                    ],
                ),
            ],
        ),
    );

    let unpack_case = assembler.parameter(
        unpack,
        ParameterRole::Block,
        0,
        builtin_switch_case_fact_type(),
    );
    let unpack_index = assembler.parameter(unpack, ParameterRole::Block, 1, u64_type());
    let key = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(unpack_case)],
        u32_type(),
        Immediate::Index(0),
    );
    let target = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(unpack_case)],
        u32_type(),
        Immediate::Index(1),
    );
    let arguments = assembler.operation(
        unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(unpack_case)],
        switch_argument_facts_type(),
        Immediate::Index(2),
    );
    assembler.push_block(
        unpack,
        function,
        vec![unpack_case, unpack_index],
        vec![key, target, arguments],
        inventory_branch(
            key_low_check,
            vec![
                operation_value(key),
                operation_value(target),
                operation_value(arguments),
                ValueRef::Parameter(unpack_index),
            ],
        ),
    );

    let validation_block =
        |assembler: &mut InventoryAssembler, block: EntityId, upper: bool, success: EntityId| {
            let found_key = assembler.parameter(block, ParameterRole::Block, 0, u32_type());
            let found_target = assembler.parameter(block, ParameterRole::Block, 1, u32_type());
            let found_arguments =
                assembler.parameter(block, ParameterRole::Block, 2, switch_argument_facts_type());
            let found_index = assembler.parameter(block, ParameterRole::Block, 3, u64_type());
            let boundary = if upper { five_u32 } else { zero_u32 };
            let boundary_value = assembler.constant_ref(block, boundary, u32_type());
            let comparison = assembler.operation(
                block,
                Opcode::LessThan,
                if upper {
                    vec![
                        ValueRef::Parameter(found_key),
                        operation_value(boundary_value),
                    ]
                } else {
                    vec![
                        operation_value(boundary_value),
                        ValueRef::Parameter(found_key),
                    ]
                },
                TypeExpr::Bool,
                Immediate::None,
            );
            assembler.push_block(
                block,
                function,
                vec![found_key, found_target, found_arguments, found_index],
                vec![boundary_value, comparison],
                inventory_cond(
                    operation_value(comparison),
                    success,
                    vec![
                        ValueRef::Parameter(found_key),
                        ValueRef::Parameter(found_target),
                        ValueRef::Parameter(found_arguments),
                        ValueRef::Parameter(found_index),
                    ],
                    signature_error,
                    Vec::new(),
                ),
            );
        };
    validation_block(&mut assembler, key_low_check, false, key_high_check);
    validation_block(&mut assembler, key_high_check, true, target_check);

    let target_key = assembler.parameter(target_check, ParameterRole::Block, 0, u32_type());
    let target_value = assembler.parameter(target_check, ParameterRole::Block, 1, u32_type());
    let target_arguments = assembler.parameter(
        target_check,
        ParameterRole::Block,
        2,
        switch_argument_facts_type(),
    );
    let target_index = assembler.parameter(target_check, ParameterRole::Block, 3, u64_type());
    let target_valid = assembler.operation(
        target_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(target_value),
            ValueRef::Parameter(block_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        target_check,
        function,
        vec![target_key, target_value, target_arguments, target_index],
        vec![target_valid],
        inventory_cond(
            operation_value(target_valid),
            arguments_call,
            vec![
                ValueRef::Parameter(target_arguments),
                ValueRef::Parameter(target_index),
            ],
            local_error,
            Vec::new(),
        ),
    );

    let call_arguments = assembler.parameter(
        arguments_call,
        ParameterRole::Block,
        0,
        switch_argument_facts_type(),
    );
    let call_index = assembler.parameter(arguments_call, ParameterRole::Block, 1, u64_type());
    let validated = assembler.operation(
        arguments_call,
        Opcode::CallDirect,
        vec![
            ValueRef::Parameter(call_arguments),
            ValueRef::Parameter(register_count),
        ],
        unit_lower_result_type(),
        Immediate::Function(FunctionRefValue {
            function: argument_validator,
            type_arguments: Vec::new(),
        }),
    );
    assembler.push_block(
        arguments_call,
        function,
        vec![call_arguments, call_index],
        vec![validated],
        inventory_switch(
            operation_value(validated),
            vec![
                (
                    BuiltinCase::Ok,
                    advance,
                    vec![SwitchArgument::Value(ValueRef::Parameter(call_index))],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
    );

    let advance_index = assembler.parameter(advance, ParameterRole::Block, 0, u64_type());
    let one = assembler.constant_ref(advance, one_u64, u64_type());
    let next_index = assembler.operation(
        advance,
        Opcode::IntAddChecked,
        vec![ValueRef::Parameter(advance_index), operation_value(one)],
        arithmetic_result_type(u64_type()),
        Immediate::None,
    );
    assembler.push_block(
        advance,
        function,
        vec![advance_index],
        vec![one, next_index],
        inventory_switch(
            operation_value(next_index),
            vec![
                (BuiltinCase::Ok, check, vec![SwitchArgument::CasePayload]),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let model = assembler.operation(
        done,
        Opcode::TupleNew,
        vec![ValueRef::Parameter(selector), ValueRef::Parameter(cases)],
        builtin_switch_model_type(),
        Immediate::None,
    );
    let success = assembler.operation(
        done,
        Opcode::ResultOk,
        vec![operation_value(model)],
        builtin_switch_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        done,
        function,
        Vec::new(),
        vec![model, success],
        Terminator::Return(ReturnTerminator {
            value: operation_value(success),
        }),
    );

    let forwarded = assembler.parameter(forward_error, ParameterRole::Block, 0, u32_type());
    let forwarded_failure = assembler.operation(
        forward_error,
        Opcode::ResultErr,
        vec![ValueRef::Parameter(forwarded)],
        builtin_switch_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        forward_error,
        function,
        vec![forwarded],
        vec![forwarded_failure],
        Terminator::Return(ReturnTerminator {
            value: operation_value(forwarded_failure),
        }),
    );
    for (block, code) in [
        (signature_error, signature_code),
        (local_error, local_code),
        (resource_error, resource_code),
    ] {
        let code_value = assembler.constant_ref(block, code, u32_type());
        let failure = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![operation_value(code_value)],
            builtin_switch_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![code_value, failure],
            Terminator::Return(ReturnTerminator {
                value: operation_value(failure),
            }),
        );
    }
    assembler.push_block(
        invariant_trap,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![selector, cases, register_count, block_count],
        result_type: builtin_switch_result_type(),
        effects: Vec::new(),
        entry_block: selector_check,
        blocks: assembler
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    LowerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph, validator_graph],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        constants: assembler.constants,
        adapters: Vec::new(),
    }
}

fn generous_limits() -> sley_vm::ExecutionLimits {
    sley_vm::ExecutionLimits {
        max_instructions: 100_000,
        max_fuel: 1_000_000,
        max_value_units: 100_000_000,
        max_output_units: 100_000,
        cancel_at_fuel: None,
    }
}

fn admitted_scaffold() -> (sley_vm::ExecutionPackage, sley_vm::ApprovedExecutionPackage) {
    admit_lower_program(&lower_scaffold())
}

fn admit_lower_program(
    scaffold: &LowerScaffold,
) -> (sley_vm::ExecutionPackage, sley_vm::ApprovedExecutionPackage) {
    use sley_vm::{
        ExecutionPackage, V2Closure, approve_package_v2, bootstrap::BootstrapProfileInput,
        bootstrap::BootstrapProfileVersion,
    };
    let lowered = sley_vm::lower_function(sley_vm::LoweringInput {
        types: &scaffold.types,
        function: &scaffold.entry,
        parameters: &scaffold.parameters,
        blocks: &scaffold.blocks,
        operations: &scaffold.operations,
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &scaffold.constants,
        globals: &[],
        functions: &scaffold.functions,
        contracts: &[],
        adapters: &scaffold.adapters,
    })
    .expect("scaffold lowers under the reference lowerer");
    let gate = sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &scaffold.types,
        schema_epoch: epoch(),
        entry: &scaffold.entry,
        presented_image_bytes: &lowered.bytes,
        functions: &scaffold.functions,
        parameters: &scaffold.parameters,
        blocks: &scaffold.blocks,
        operations: &scaffold.operations,
        adapters: &scaffold.adapters,
        constants: &scaffold.constants,
        profile_version: BootstrapProfileVersion::V2,
    })
    .expect("scaffold admits under V2");
    let limits = generous_limits();
    let package = ExecutionPackage {
        image_bytes: lowered.bytes.clone(),
        constants: scaffold.constants.clone(),
        type_definitions: Vec::new(),
        imports: scaffold.adapters.clone(),
        globals: Vec::new(),
        contracts: Vec::new(),
        entry: scaffold.entry.entity_id,
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        admitted_limits: limits,
        gate_operation_count: gate.operation_count(),
        gate_bridge_uses: gate.bridge_uses(),
        gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
    };
    // C0 seed minting route (declared): the staged authority judges plus
    // reference re-lowers before minting; excluded from clean stages.
    let closure = V2Closure {
        types: &scaffold.types,
        schema_epoch: epoch(),
        state_root: root(),
        entry: scaffold.entry.entity_id,
        functions: &scaffold.functions,
        parameters: &scaffold.parameters,
        blocks: &scaffold.blocks,
        operations: &scaffold.operations,
        adapters: &scaffold.adapters,
        constants: &scaffold.constants,
        globals: &[],
        contracts: &[],
    };
    let (digests, receipt, report) =
        sley_vm::admit_v2_package(&closure, &package).expect("authority admits scaffold");
    assert_eq!(
        receipt.profile_digest(),
        &sley_vm::BOOTSTRAP_PROFILE_2_DIGEST,
        "scaffold receipt binds the successor profile"
    );
    let approved =
        approve_package_v2(&package, &digests, receipt, &report).expect("v2 approves scaffold");
    (package, approved)
}

fn execute_scaffold(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    marker: u128,
    witness: &[u8],
) -> sley_vm::ExecutionTermination {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![u32_value(marker), bytes_value(witness)],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes scaffold")
    .termination
}

fn execute_single_bool(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    opcode: u32,
    parameter_count: u32,
    operand_zero: u32,
    operand_one: u32,
    next_register: u32,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                u32_value(u128::from(opcode)),
                u32_value(u128::from(parameter_count)),
                u32_value(u128::from(operand_zero)),
                u32_value(u128::from(operand_one)),
                u32_value(u128::from(next_register)),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes single-operation lowerer")
}

fn execute_bool_inventory(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    rows: &[(u32, u32, u32, u32)],
    first_register: u32,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                bool_inventory_value(rows),
                u32_value(u128::from(first_register)),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes ordered Boolean inventory lowerer")
}

fn execute_immediate_free_operation(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    opcode: Opcode,
    operands: &[u32],
    next_register: u32,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                u32_value(u128::from(opcode.tag())),
                u32vec_value(operands),
                u32_value(u128::from(next_register)),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes immediate-free operation lowerer")
}

#[allow(clippy::too_many_arguments)]
fn execute_immediate_operation(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    opcode: Opcode,
    operands: &[u32],
    immediate_tag: u32,
    primary: u64,
    secondary: u64,
    immediate_bytes: &[u8],
    next_register: u32,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                u32_value(u128::from(opcode.tag())),
                u32vec_value(operands),
                u32_value(u128::from(immediate_tag)),
                u64_value(u128::from(primary)),
                u64_value(u128::from(secondary)),
                bytes_value(immediate_bytes),
                u32_value(u128::from(next_register)),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes bootstrap immediate lowerer")
}

fn execute_immediate_inventory(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    rows: &[ImmediateInventoryFact],
    first_register: u32,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                immediate_inventory_value(rows),
                u32_value(u128::from(first_register)),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes ordered immediate inventory lowerer")
}

#[allow(clippy::too_many_arguments)]
fn execute_simple_terminator(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    kind: u32,
    primary: u32,
    target_zero: u32,
    arguments_zero: &[u32],
    target_one: u32,
    arguments_one: &[u32],
    payload: Option<u32>,
    register_count: u32,
    block_count: u32,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                u32_value(u128::from(kind)),
                u32_value(u128::from(primary)),
                u32_value(u128::from(target_zero)),
                u32vec_value(arguments_zero),
                u32_value(u128::from(target_one)),
                u32vec_value(arguments_one),
                optional_u32_value(payload),
                u32_value(u128::from(register_count)),
                u32_value(u128::from(block_count)),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes simple terminator lowerer")
}

#[allow(clippy::too_many_arguments)]
fn execute_simple_block(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    rows: &[ImmediateInventoryFact],
    first_register: u32,
    slot: u32,
    parameter_registers: &[u32],
    terminator: &sley_vm::BytecodeTerminator,
    block_count: u32,
    reachability: u32,
) -> sley_vm::ExecutionOutcome {
    let (kind, primary, target_zero, arguments_zero, target_one, arguments_one, payload) =
        simple_terminator_fact(terminator);
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                immediate_inventory_value(rows),
                u32_value(u128::from(first_register)),
                u32_value(u128::from(slot)),
                u32vec_value(parameter_registers),
                u32_value(u128::from(kind)),
                u32_value(u128::from(primary)),
                u32_value(u128::from(target_zero)),
                u32vec_value(&arguments_zero),
                u32_value(u128::from(target_one)),
                u32vec_value(&arguments_one),
                optional_u32_value(payload),
                u32_value(u128::from(block_count)),
                u32_value(u128::from(reachability)),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes simple block lowerer")
}

#[allow(clippy::too_many_arguments)]
fn execute_complete_block(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    rows: &[ImmediateInventoryFact],
    first_register: u32,
    slot: u32,
    parameter_registers: &[u32],
    terminator: &sley_vm::BytecodeTerminator,
    block_count: u32,
    reachability: u32,
) -> sley_vm::ExecutionOutcome {
    let (kind, primary, target_zero, arguments_zero, target_one, arguments_one, payload, cases) =
        if let sley_vm::BytecodeTerminator::VariantSwitch { .. } = terminator {
            let (selector, cases) = builtin_switch_facts(terminator);
            (4, selector, 0, Vec::new(), 0, Vec::new(), None, cases)
        } else {
            let (kind, primary, target_zero, arguments_zero, target_one, arguments_one, payload) =
                simple_terminator_fact(terminator);
            (
                kind,
                primary,
                target_zero,
                arguments_zero,
                target_one,
                arguments_one,
                payload,
                Vec::new(),
            )
        };
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                immediate_inventory_value(rows),
                u32_value(u128::from(first_register)),
                u32_value(u128::from(slot)),
                u32vec_value(parameter_registers),
                u32_value(u128::from(kind)),
                u32_value(u128::from(primary)),
                u32_value(u128::from(target_zero)),
                u32vec_value(&arguments_zero),
                u32_value(u128::from(target_one)),
                u32vec_value(&arguments_one),
                optional_u32_value(payload),
                u32_value(u128::from(block_count)),
                u32_value(u128::from(reachability)),
                builtin_switch_facts_value(&cases),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes complete block lowerer")
}

fn complete_block_facts(function: &sley_vm::BytecodeFunction) -> Vec<CompleteBlockFact> {
    function
        .blocks
        .iter()
        .map(|block| CompleteBlockFact {
            slot: block.slot,
            parameter_registers: block.parameter_registers.clone(),
            instructions: block
                .instructions
                .iter()
                .map(immediate_inventory_fact)
                .collect(),
            terminator: block.terminator.clone(),
            reachability: block.reachability,
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn execute_complete_function(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    function_identity: &[u8],
    function_parameters: &[u32],
    register_types: &[Vec<u8>],
    result_type: &[u8],
    entry_slot: u32,
    block_count: u32,
    blocks: &[CompleteBlockFact],
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                bytes_value(function_identity),
                u32vec_value(function_parameters),
                bytesvec_value(register_types),
                bytes_value(result_type),
                u32_value(u128::from(entry_slot)),
                u32_value(u128::from(block_count)),
                complete_block_facts_value(blocks),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes complete function lowerer")
}

fn execute_complete_image(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    root: &CompleteFunctionFact,
    callees: &[CompleteFunctionFact],
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                bytes_value(&root.identity),
                u32vec_value(&root.parameter_registers),
                bytesvec_value(&root.register_types),
                bytes_value(&root.result_type),
                u32_value(u128::from(root.entry_slot)),
                u32_value(u128::from(root.block_count)),
                complete_block_facts_value(&root.blocks),
                complete_function_facts_value(callees),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes complete function-image lowerer")
}

fn execute_package_dependency_sections(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    expected: &sley_vm::ExecutionPackage,
    global_rows: &[Vec<u8>],
    contract_rows: &[Vec<u8>],
) -> sley_vm::ExecutionOutcome {
    let fingerprint_bytes = expected
        .gate_closure_fingerprints
        .iter()
        .map(|fingerprint| fingerprint.as_bytes().to_vec())
        .collect::<Vec<_>>();
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                bytes_value(expected.entry.as_bytes()),
                bytes_value(expected.schema_epoch.as_bytes()),
                bytes_value(expected.state_root.as_bytes()),
                u32_value(u128::from(expected.gate_operation_count)),
                u32_value(u128::from(expected.gate_bridge_uses)),
                bytesvec_value(&fingerprint_bytes),
                u64_value(u128::from(expected.admitted_limits.max_instructions)),
                u64_value(u128::from(expected.admitted_limits.max_fuel)),
                u64_value(u128::from(expected.admitted_limits.max_value_units)),
                u64_value(u128::from(expected.admitted_limits.max_output_units)),
                bytesvec_value(global_rows),
                bytesvec_value(contract_rows),
                bool_value(expected.admitted_limits.cancel_at_fuel.is_some()),
                u64_value(u128::from(
                    expected.admitted_limits.cancel_at_fuel.unwrap_or_default(),
                )),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes package-dependency-section encoder")
}

fn execute_package_inventory_section(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    rows: &[Vec<u8>],
    frame_rows: bool,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![bytesvec_value(rows), bool_value(frame_rows)],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes package-inventory-section encoder")
}

fn execute_package_envelope_composer(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    expected: &sley_vm::ExecutionPackage,
    sections: &[Vec<u8>; 4],
    digests: &sley_vm::PackageDigests,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                bytes_value(&expected.image_bytes),
                bytes_value(&sections[0]),
                bytes_value(&sections[1]),
                bytes_value(&sections[2]),
                bytes_value(&sections[3]),
                bytes_value(&digests.image_digest),
                bytes_value(&digests.constants_digest),
                bytes_value(&digests.layouts_digest),
                bytes_value(&digests.imports_digest),
                bytes_value(&digests.dependency_digest),
                bytes_value(expected.entry.as_bytes()),
                bytes_value(expected.schema_epoch.as_bytes()),
                bytes_value(expected.state_root.as_bytes()),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes package-envelope composer")
}

fn execute_package_builder(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    expected: &sley_vm::ExecutionPackage,
    digests: &sley_vm::PackageDigests,
) -> sley_vm::ExecutionOutcome {
    let fingerprint_bytes = expected
        .gate_closure_fingerprints
        .iter()
        .map(|fingerprint| fingerprint.as_bytes().to_vec())
        .collect::<Vec<_>>();
    let (global_rows, contract_rows) = canonical_dependency_rows(expected);
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                bytes_value(&expected.image_bytes),
                bytesvec_value(&canonical_constant_rows(&expected.constants)),
                bytesvec_value(&canonical_layout_rows(&expected.type_definitions)),
                bytesvec_value(&canonical_import_rows(&expected.imports)),
                bytes_value(expected.entry.as_bytes()),
                bytes_value(expected.schema_epoch.as_bytes()),
                bytes_value(expected.state_root.as_bytes()),
                u32_value(u128::from(expected.gate_operation_count)),
                u32_value(u128::from(expected.gate_bridge_uses)),
                bytesvec_value(&fingerprint_bytes),
                u64_value(u128::from(expected.admitted_limits.max_instructions)),
                u64_value(u128::from(expected.admitted_limits.max_fuel)),
                u64_value(u128::from(expected.admitted_limits.max_value_units)),
                u64_value(u128::from(expected.admitted_limits.max_output_units)),
                bytesvec_value(&global_rows),
                bytesvec_value(&contract_rows),
                bool_value(expected.admitted_limits.cancel_at_fuel.is_some()),
                u64_value(u128::from(
                    expected.admitted_limits.cancel_at_fuel.unwrap_or_default(),
                )),
                bytes_value(&digests.image_digest),
                bytes_value(&digests.constants_digest),
                bytes_value(&digests.layouts_digest),
                bytes_value(&digests.imports_digest),
                bytes_value(&digests.dependency_digest),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes composed package builder")
}

fn successful_package_sections(outcome: &sley_vm::ExecutionOutcome) -> [Vec<u8>; 4] {
    use sley_ssmc::ResultConst;

    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "package-section encoder must terminate with a value, got {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(sections)) = &value.data else {
        panic!(
            "package-section encoder must return Ok, got {:?}",
            value.data
        )
    };
    let ConstData::Sequence(sections) = &sections.data else {
        panic!("package-section encoder result must be a tuple")
    };
    sections
        .iter()
        .map(|section| match &section.data {
            ConstData::Bytes(bytes) => bytes.clone(),
            other => panic!("package section must be Bytes, got {other:?}"),
        })
        .collect::<Vec<_>>()
        .try_into()
        .expect("package-section tuple has four fields")
}

fn successful_bytes_result(outcome: &sley_vm::ExecutionOutcome, subject: &str) -> Vec<u8> {
    use sley_ssmc::ResultConst;

    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "{subject} must terminate with a value, got {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
        panic!("{subject} must return Ok, got {:?}", value.data)
    };
    let ConstData::Bytes(encoded) = &encoded.data else {
        panic!("{subject} result must be Bytes")
    };
    encoded.clone()
}

fn package_inventory_fixture(image_bytes: Vec<u8>, entry: EntityId) -> sley_vm::ExecutionPackage {
    use sley_id::SemanticFingerprint;

    let constants = vec![
        ConstantDefinition {
            entity_id: id(0xa1),
            value: bool_value(true),
        },
        ConstantDefinition {
            entity_id: id(0xa2),
            value: bool_value(false),
        },
    ];
    let type_definitions = vec![
        TypeDefinition {
            entity_id: id(0xb1),
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(Vec::new()),
            invariants: Vec::new(),
            visibility: Visibility::Package,
        },
        TypeDefinition {
            entity_id: id(0xb2),
            type_parameters: Vec::new(),
            form: TypeDefForm::Variant(Vec::new()),
            invariants: Vec::new(),
            visibility: Visibility::Exported,
        },
    ];
    let imports = vec![
        AdapterImport {
            entity_id: id(0xc1),
            adapter_id: [0xc2; 32],
            abi_version: 2,
            request_type: TypeExpr::Bytes,
            response_type: TypeExpr::Bytes,
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
            effects: vec![id(0xc3)],
        },
        AdapterImport {
            entity_id: id(0xc4),
            adapter_id: [0xc5; 32],
            abi_version: 3,
            request_type: TypeExpr::Bool,
            response_type: TypeExpr::Bool,
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
            effects: Vec::new(),
        },
    ];
    let globals = vec![GlobalValueDefinition {
        entity_id: id(0xd1),
        value_type: TypeExpr::Bool,
        initializer: constants[0].entity_id,
        visibility: Visibility::Workspace,
    }];
    let contracts = vec![ContractDefinition {
        entity_id: id(0xe1),
        target: entry,
        contract_kind: ContractKind::Postcondition,
        predicate: id(0xe2),
        bindings: Vec::new(),
        resource_limits: None,
    }];
    let mut admitted_limits = generous_limits();
    admitted_limits.cancel_at_fuel = Some(1_500);
    sley_vm::ExecutionPackage {
        image_bytes,
        constants,
        type_definitions,
        imports,
        globals,
        contracts,
        entry,
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        admitted_limits,
        gate_operation_count: 91,
        gate_bridge_uses: 4,
        gate_closure_fingerprints: vec![
            SemanticFingerprint::from_bytes([0xc3; 32]),
            SemanticFingerprint::from_bytes([0xd4; 32]),
        ],
    }
}

fn canonical_constant_rows(constants: &[ConstantDefinition]) -> Vec<Vec<u8>> {
    constants
        .iter()
        .map(|constant| {
            let encoded =
                sley_vm::exec_package::encode_constants_section(std::slice::from_ref(constant))
                    .expect("single constant row encodes");
            assert_eq!(&encoded[..8], &1_u64.to_be_bytes());
            encoded[8..].to_vec()
        })
        .collect()
}

fn canonical_framed_row(encoded: &[u8], subject: &str) -> Vec<u8> {
    assert_eq!(&encoded[..8], &1_u64.to_be_bytes());
    let row_len = u64::from_be_bytes(encoded[8..16].try_into().unwrap());
    let row_len = usize::try_from(row_len).expect("row length fits usize");
    assert_eq!(encoded.len(), 16 + row_len, "{subject} row frame is exact");
    encoded[16..].to_vec()
}

fn canonical_layout_rows(definitions: &[TypeDefinition]) -> Vec<Vec<u8>> {
    definitions
        .iter()
        .map(|definition| {
            let encoded =
                sley_vm::exec_package::encode_layouts_section(std::slice::from_ref(definition))
                    .expect("single layout row encodes");
            canonical_framed_row(&encoded, "layout")
        })
        .collect()
}

fn canonical_import_rows(imports: &[AdapterImport]) -> Vec<Vec<u8>> {
    imports
        .iter()
        .map(|import| {
            let encoded =
                sley_vm::exec_package::encode_imports_section(std::slice::from_ref(import))
                    .expect("single import row encodes");
            canonical_framed_row(&encoded, "import")
        })
        .collect()
}

fn canonical_dependency_rows(package: &sley_vm::ExecutionPackage) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut empty = package.clone();
    empty.globals.clear();
    empty.contracts.clear();
    let empty_encoded = sley_vm::exec_package::encode_dependency_section(&empty)
        .expect("empty dependency inventory encodes");
    let prefix_len = empty_encoded
        .len()
        .checked_sub(16)
        .expect("dependency contains both inventory counts");

    let globals = package
        .globals
        .iter()
        .map(|global| {
            let mut single = empty.clone();
            single.globals.push(global.clone());
            let encoded = sley_vm::exec_package::encode_dependency_section(&single)
                .expect("single global row encodes");
            assert_eq!(&encoded[prefix_len..prefix_len + 8], &1_u64.to_be_bytes());
            assert_eq!(&encoded[encoded.len() - 8..], &0_u64.to_be_bytes());
            encoded[prefix_len + 8..encoded.len() - 8].to_vec()
        })
        .collect();
    let contracts = package
        .contracts
        .iter()
        .map(|contract| {
            let mut single = empty.clone();
            single.contracts.push(contract.clone());
            let encoded = sley_vm::exec_package::encode_dependency_section(&single)
                .expect("single contract row encodes");
            assert_eq!(&encoded[prefix_len..prefix_len + 8], &0_u64.to_be_bytes());
            assert_eq!(
                &encoded[prefix_len + 8..prefix_len + 16],
                &1_u64.to_be_bytes()
            );
            encoded[prefix_len + 16..].to_vec()
        })
        .collect();
    (globals, contracts)
}

fn execute_fixed_width_encoder(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    prefix: &[u8],
    value_u32: u32,
    value_u64: u64,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                bytes_value(prefix),
                u32_value(u128::from(value_u32)),
                u64_value(u128::from(value_u64)),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes fixed-width byte encoder")
}

fn execute_byte_chunk_encoder(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    prefix: &[u8],
    chunk: &[u8],
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![bytes_value(prefix), bytes_value(chunk)],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes byte-chunk encoder")
}

fn execute_instruction_byte_encoder(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    instruction: &sley_vm::Instruction,
    prefix: &[u8],
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                immediate_instruction_value(instruction),
                u8vec_value(prefix),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes instruction byte encoder")
}

fn execute_instruction_map_byte_encoder(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    instructions: &[sley_vm::Instruction],
    prefix: &[u8],
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                immediate_inventory_model_value(instructions),
                u64_value(u128::try_from(instructions.len()).expect("instruction count fits u128")),
                u8vec_value(prefix),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes instruction-map byte encoder")
}

fn execute_simple_terminator_byte_encoder(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    terminator: &sley_vm::BytecodeTerminator,
    prefix: &[u8],
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![simple_terminator_value(terminator), u8vec_value(prefix)],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes simple-terminator byte encoder")
}

fn execute_builtin_switch_terminator_byte_encoder(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    terminator: &sley_vm::BytecodeTerminator,
    prefix: &[u8],
) -> sley_vm::ExecutionOutcome {
    let (selector, cases) = builtin_switch_facts(terminator);
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                builtin_switch_model_value(selector, &cases),
                u8vec_value(prefix),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes built-in-switch terminator byte encoder")
}

fn execute_complete_terminator_byte_encoder(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    terminator: &sley_vm::BytecodeTerminator,
    prefix: &[u8],
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                complete_terminator_model_value(terminator),
                u8vec_value(prefix),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes complete-terminator byte encoder")
}

fn execute_complete_block_byte_encoder(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    block: &sley_vm::BytecodeBlock,
    prefix: &[u8],
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                complete_block_model_value(block),
                u64_value(
                    u128::try_from(block.instructions.len()).expect("instruction count fits u128"),
                ),
                u8vec_value(prefix),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes complete-block byte encoder")
}

fn execute_complete_block_map_byte_encoder(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    function: &sley_vm::BytecodeFunction,
    prefix: &[u8],
) -> sley_vm::ExecutionOutcome {
    let facts = complete_block_facts(function);
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                complete_block_map_value(&function.blocks),
                complete_block_facts_value(&facts),
                u8vec_value(prefix),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes complete-block-map byte encoder")
}

fn execute_builtin_switch(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    selector: u32,
    cases: &[BuiltinSwitchFact],
    register_count: u32,
    block_count: u32,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                u32_value(u128::from(selector)),
                builtin_switch_facts_value(cases),
                u32_value(u128::from(register_count)),
                u32_value(u128::from(block_count)),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes built-in variant-switch lowerer")
}

fn assert_builtin_switch_model(
    outcome: &sley_vm::ExecutionOutcome,
    expected_selector: u32,
    expected_cases: &[BuiltinSwitchFact],
) {
    use sley_ssmc::ResultConst;
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("variant-switch lowering must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(model)) = &value.data else {
        panic!(
            "variant-switch lowering must return Ok, got {:?}",
            value.data
        )
    };
    let expected = ConstData::Sequence(vec![
        u32_value(u128::from(expected_selector)),
        builtin_switch_facts_value(expected_cases),
    ]);
    assert_eq!(model.data, expected);
}

type SimpleTerminatorFact = (u32, u32, u32, Vec<u32>, u32, Vec<u32>, Option<u32>);

fn simple_terminator_fact(value: &sley_vm::BytecodeTerminator) -> SimpleTerminatorFact {
    match value {
        sley_vm::BytecodeTerminator::Return(register) => {
            (1, *register, 0, Vec::new(), 0, Vec::new(), None)
        }
        sley_vm::BytecodeTerminator::Branch(edge) => (
            2,
            0,
            edge.target,
            edge.arguments.clone(),
            0,
            Vec::new(),
            None,
        ),
        sley_vm::BytecodeTerminator::CondBranch {
            condition,
            if_true,
            if_false,
        } => (
            3,
            *condition,
            if_true.target,
            if_true.arguments.clone(),
            if_false.target,
            if_false.arguments.clone(),
            None,
        ),
        sley_vm::BytecodeTerminator::Trap { code, payload } => {
            (5, *code, 0, Vec::new(), 0, Vec::new(), *payload)
        }
        sley_vm::BytecodeTerminator::VariantSwitch { .. } => {
            panic!("variant switch is outside the simple terminator slice")
        }
    }
}

fn simple_terminator_value(value: &sley_vm::BytecodeTerminator) -> ConstValue {
    let (kind, primary, target_zero, arguments_zero, target_one, arguments_one, payload) =
        simple_terminator_fact(value);
    ConstValue {
        value_type: terminator_model_type(),
        data: ConstData::Sequence(vec![
            u32_value(u128::from(kind)),
            u32_value(u128::from(primary)),
            u32_value(u128::from(target_zero)),
            u32vec_value(&arguments_zero),
            u32_value(u128::from(target_one)),
            u32vec_value(&arguments_one),
            optional_u32_value(payload),
        ]),
    }
}

fn assert_terminator_model(
    outcome: &sley_vm::ExecutionOutcome,
    expected: &sley_vm::BytecodeTerminator,
) {
    use sley_ssmc::ResultConst;
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("terminator lowering must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(model)) = &value.data else {
        panic!("terminator lowering must return Ok, got {:?}", value.data)
    };
    let ConstData::Sequence(fields) = &model.data else {
        panic!("terminator model must be a tuple, got {:?}", model.data)
    };
    assert_eq!(fields.len(), 7);
    let uint = |value: &ConstValue| match value.data {
        ConstData::UInt(found) => u32::try_from(found).expect("terminator scalar fits u32"),
        ref other => panic!("terminator scalar must be UInt32, got {other:?}"),
    };
    let registers = |value: &ConstValue| match &value.data {
        ConstData::Sequence(found) => found.iter().map(uint).collect::<Vec<_>>(),
        other => panic!("terminator arguments must be Vector, got {other:?}"),
    };
    let payload = match &fields[6].data {
        ConstData::Option(None) => None,
        ConstData::Option(Some(value)) => Some(uint(value)),
        other => panic!("terminator payload must be Option, got {other:?}"),
    };
    let found = (
        uint(&fields[0]),
        uint(&fields[1]),
        uint(&fields[2]),
        registers(&fields[3]),
        uint(&fields[4]),
        registers(&fields[5]),
        payload,
    );
    let normalized = simple_terminator_fact(expected);
    assert_eq!(found, normalized);
}

fn assert_terminator_error(outcome: &sley_vm::ExecutionOutcome, expected: u32) {
    use sley_ssmc::ResultConst;
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("terminator error must terminate with a value")
    };
    let ConstData::Result(ResultConst::Err(code)) = &value.data else {
        panic!("terminator lowering must return Err, got {:?}", value.data)
    };
    assert_eq!(code.data, ConstData::UInt(u128::from(expected)));
}

fn assert_inventory_summary(
    outcome: &sley_vm::ExecutionOutcome,
    expected_instructions: &[sley_vm::Instruction],
    expected_next_register: u32,
) {
    use sley_ssmc::ResultConst;
    let registers = |value: &ConstValue| match &value.data {
        ConstData::Sequence(found) => found
            .iter()
            .map(|register| match register.data {
                ConstData::UInt(value) => u32::try_from(value).expect("lowered register fits u32"),
                ref other => panic!("lowered register must be UInt32, got {other:?}"),
            })
            .collect::<Vec<_>>(),
        other => panic!("lowered register list must be Vector, got {other:?}"),
    };
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("inventory lowering must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(summary)) = &value.data else {
        panic!("inventory lowering must return Ok, got {:?}", value.data)
    };
    let ConstData::Sequence(fields) = &summary.data else {
        panic!("inventory summary must be a tuple, got {:?}", summary.data)
    };
    assert_eq!(fields.len(), 3, "inventory summary has three fields");
    let ConstData::Sequence(instructions) = &fields[0].data else {
        panic!("inventory model must be a vector, got {:?}", fields[0].data)
    };
    assert_eq!(instructions.len(), expected_instructions.len());
    for (found, expected) in instructions.iter().zip(expected_instructions) {
        let ConstData::Sequence(instruction_fields) = &found.data else {
            panic!("lowered instruction must be a tuple, got {:?}", found.data)
        };
        assert_eq!(instruction_fields.len(), 3);
        assert_eq!(
            instruction_fields[0].data,
            ConstData::UInt(u128::from(expected.opcode))
        );
        assert_eq!(registers(&instruction_fields[1]), expected.operands);
        assert_eq!(registers(&instruction_fields[2]), expected.results);
    }
    assert_eq!(
        fields[1].data,
        ConstData::UInt(u128::try_from(expected_instructions.len()).expect("count fits u128"))
    );
    assert_eq!(
        fields[2].data,
        ConstData::UInt(u128::from(expected_next_register))
    );
}

fn assert_inventory_error(outcome: &sley_vm::ExecutionOutcome, expected: u32) {
    use sley_ssmc::ResultConst;
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("inventory error must terminate with a value")
    };
    let ConstData::Result(ResultConst::Err(code)) = &value.data else {
        panic!("inventory lowering must return Err, got {:?}", value.data)
    };
    assert_eq!(code.data, ConstData::UInt(u128::from(expected)));
}

fn assert_immediate_free_summary(
    outcome: &sley_vm::ExecutionOutcome,
    expected: &sley_vm::Instruction,
    expected_frontier: u32,
) {
    use sley_ssmc::ResultConst;
    let registers = |value: &ConstValue| match &value.data {
        ConstData::Sequence(found) => found
            .iter()
            .map(|register| match register.data {
                ConstData::UInt(value) => u32::try_from(value).expect("register fits u32"),
                ref other => panic!("register must be UInt32, got {other:?}"),
            })
            .collect::<Vec<_>>(),
        other => panic!("register list must be Vector, got {other:?}"),
    };
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("immediate-free lowering must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(summary)) = &value.data else {
        panic!(
            "immediate-free lowering must return Ok, got {:?}",
            value.data
        )
    };
    let ConstData::Sequence(fields) = &summary.data else {
        panic!("immediate-free summary must be a tuple")
    };
    let ConstData::Sequence(instruction) = &fields[0].data else {
        panic!("immediate-free instruction must be a tuple")
    };
    assert_eq!(
        instruction[0].data,
        ConstData::UInt(u128::from(expected.opcode))
    );
    assert_eq!(registers(&instruction[1]), expected.operands);
    assert_eq!(registers(&instruction[2]), expected.results);
    assert_eq!(
        fields[1].data,
        ConstData::UInt(u128::from(expected_frontier))
    );
}

fn compact_identity(bytes: &[u8; 32]) -> u64 {
    u64::from_be_bytes(
        bytes[24..]
            .try_into()
            .expect("identity suffix is eight bytes"),
    )
}

fn immediate_projection(immediate: &Immediate) -> (u32, u64, u64) {
    match immediate {
        Immediate::None => (immediate.tag(), 0, 0),
        Immediate::Entity(entity) => (immediate.tag(), compact_identity(entity.as_bytes()), 0),
        Immediate::Index(index) => (immediate.tag(), u64::from(*index), 0),
        Immediate::Field(member) => (immediate.tag(), compact_identity(member.as_bytes()), 0),
        Immediate::Variant(variant) => (
            immediate.tag(),
            compact_identity(variant.definition.as_bytes()),
            compact_identity(variant.member_id.as_bytes()),
        ),
        Immediate::Function(reference) => {
            assert!(reference.type_arguments.is_empty());
            (
                immediate.tag(),
                compact_identity(reference.function.as_bytes()),
                0,
            )
        }
        other @ Immediate::Observation(_) => {
            panic!("immediate projection does not support {other:?}")
        }
    }
}

fn immediate_inventory_fact(instruction: &sley_vm::Instruction) -> ImmediateInventoryFact {
    let (immediate_tag, primary, secondary) = immediate_projection(&instruction.immediate);
    ImmediateInventoryFact {
        opcode: instruction.opcode,
        operands: instruction.operands.clone(),
        immediate_tag,
        primary,
        secondary,
        immediate_bytes: encoded_immediate(&instruction.immediate),
    }
}

fn immediate_instruction_value(instruction: &sley_vm::Instruction) -> ConstValue {
    let (immediate_tag, primary, secondary) = immediate_projection(&instruction.immediate);
    ConstValue {
        value_type: immediate_instruction_type(),
        data: ConstData::Sequence(vec![
            u32_value(u128::from(instruction.opcode)),
            u32vec_value(&instruction.operands),
            u32vec_value(&instruction.results),
            u32_value(u128::from(immediate_tag)),
            u64_value(u128::from(primary)),
            u64_value(u128::from(secondary)),
            bytes_value(&encoded_immediate(&instruction.immediate)),
        ]),
    }
}

fn immediate_inventory_model_value(instructions: &[sley_vm::Instruction]) -> ConstValue {
    ConstValue {
        value_type: immediate_inventory_model_type(),
        data: ConstData::Map(
            instructions
                .iter()
                .enumerate()
                .map(|(index, instruction)| sley_ssmc::MapEntryConst {
                    key: u64_value(u128::try_from(index).expect("instruction index fits u128")),
                    value: immediate_instruction_value(instruction),
                })
                .collect(),
        ),
    }
}

fn assert_immediate_summary(
    outcome: &sley_vm::ExecutionOutcome,
    expected: &sley_vm::Instruction,
    expected_frontier: u32,
) {
    use sley_ssmc::ResultConst;
    let registers = |value: &ConstValue| match &value.data {
        ConstData::Sequence(found) => found
            .iter()
            .map(|register| match register.data {
                ConstData::UInt(value) => u32::try_from(value).expect("register fits u32"),
                ref other => panic!("register must be UInt32, got {other:?}"),
            })
            .collect::<Vec<_>>(),
        other => panic!("register list must be Vector, got {other:?}"),
    };
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("immediate lowering must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(summary)) = &value.data else {
        panic!("immediate lowering must return Ok, got {:?}", value.data)
    };
    let ConstData::Sequence(fields) = &summary.data else {
        panic!("immediate summary must be a tuple")
    };
    let ConstData::Sequence(instruction) = &fields[0].data else {
        panic!("immediate instruction must be a tuple")
    };
    let (tag, primary, secondary) = immediate_projection(&expected.immediate);
    assert_eq!(
        instruction[0].data,
        ConstData::UInt(u128::from(expected.opcode))
    );
    assert_eq!(registers(&instruction[1]), expected.operands);
    assert_eq!(registers(&instruction[2]), expected.results);
    assert_eq!(instruction[3].data, ConstData::UInt(u128::from(tag)));
    assert_eq!(instruction[4].data, ConstData::UInt(u128::from(primary)));
    assert_eq!(instruction[5].data, ConstData::UInt(u128::from(secondary)));
    assert_eq!(
        instruction[6],
        bytes_value(&encoded_immediate(&expected.immediate))
    );
    assert_eq!(
        fields[1].data,
        ConstData::UInt(u128::from(expected_frontier))
    );
}

fn assert_immediate_inventory_summary(
    outcome: &sley_vm::ExecutionOutcome,
    expected: &[sley_vm::Instruction],
    expected_frontier: u32,
) {
    use sley_ssmc::ResultConst;
    let registers = |value: &ConstValue| match &value.data {
        ConstData::Sequence(found) => found
            .iter()
            .map(|register| match register.data {
                ConstData::UInt(value) => u32::try_from(value).expect("register fits u32"),
                ref other => panic!("register must be UInt32, got {other:?}"),
            })
            .collect::<Vec<_>>(),
        other => panic!("register list must be Vector, got {other:?}"),
    };
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("immediate inventory lowering must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(summary)) = &value.data else {
        panic!(
            "immediate inventory lowering must return Ok, got {:?}",
            value.data
        )
    };
    let ConstData::Sequence(fields) = &summary.data else {
        panic!("immediate inventory summary must be a tuple")
    };
    let ConstData::Map(instructions) = &fields[0].data else {
        panic!("immediate inventory model must be an ordered map")
    };
    assert_eq!(instructions.len(), expected.len());
    for (index, (instruction, expected)) in instructions.iter().zip(expected).enumerate() {
        assert_eq!(
            instruction.key,
            u64_value(u128::try_from(index).expect("instruction index fits u128"))
        );
        let ConstData::Sequence(parts) = &instruction.value.data else {
            panic!("immediate instruction must be a tuple")
        };
        let (tag, primary, secondary) = immediate_projection(&expected.immediate);
        assert_eq!(parts[0].data, ConstData::UInt(u128::from(expected.opcode)));
        assert_eq!(registers(&parts[1]), expected.operands);
        assert_eq!(registers(&parts[2]), expected.results);
        assert_eq!(parts[3].data, ConstData::UInt(u128::from(tag)));
        assert_eq!(parts[4].data, ConstData::UInt(u128::from(primary)));
        assert_eq!(parts[5].data, ConstData::UInt(u128::from(secondary)));
        assert_eq!(
            parts[6],
            bytes_value(&encoded_immediate(&expected.immediate))
        );
    }
    assert_eq!(
        fields[1].data,
        ConstData::UInt(u128::try_from(expected.len()).expect("instruction count fits u128"))
    );
    assert_eq!(
        fields[2].data,
        ConstData::UInt(u128::from(expected_frontier))
    );
}

fn assert_simple_block_summary(
    outcome: &sley_vm::ExecutionOutcome,
    expected_slot: u32,
    expected_parameters: &[u32],
    expected_instructions: &[sley_vm::Instruction],
    expected_terminator: &sley_vm::BytecodeTerminator,
    expected_reachability: u32,
    expected_frontier: u32,
) {
    use sley_ssmc::ResultConst;
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("simple block lowering must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(summary)) = &value.data else {
        panic!("simple block lowering must return Ok, got {:?}", value.data)
    };
    let ConstData::Sequence(summary_fields) = &summary.data else {
        panic!("simple block summary must be a tuple")
    };
    let ConstData::Sequence(block_fields) = &summary_fields[0].data else {
        panic!("simple block model must be a tuple")
    };
    assert_eq!(
        block_fields[0],
        u32_value(u128::from(expected_slot)),
        "block slot"
    );
    assert_eq!(block_fields[1], u32vec_value(expected_parameters));
    assert_eq!(
        block_fields[2],
        immediate_inventory_model_value(expected_instructions)
    );
    assert_eq!(
        block_fields[3],
        simple_terminator_value(expected_terminator)
    );
    assert_eq!(
        block_fields[4],
        u32_value(u128::from(expected_reachability))
    );
    assert_eq!(summary_fields[1], u32_value(u128::from(expected_frontier)));
}

#[allow(clippy::too_many_lines)]
fn native_single_scalar(opcode: Opcode) -> sley_vm::Instruction {
    let function_id = id(1);
    let block_id = id(2);
    let operation_id = id(3);
    let signed32 = TypeExpr::SInt(IntegerWidth::from_bits(32));
    let (parameter_count, operand_type, result_type) = match opcode {
        Opcode::BoolNot => (1, TypeExpr::Bool, TypeExpr::Bool),
        Opcode::BoolAnd | Opcode::BoolOr | Opcode::Equal | Opcode::NotEqual => {
            (2, TypeExpr::Bool, TypeExpr::Bool)
        }
        Opcode::LessThan | Opcode::LessEqual | Opcode::GreaterThan | Opcode::GreaterEqual => {
            (2, u32_type(), TypeExpr::Bool)
        }
        Opcode::IntAddChecked
        | Opcode::IntSubChecked
        | Opcode::IntMulChecked
        | Opcode::IntDivChecked
        | Opcode::IntRemChecked
        | Opcode::IntShlChecked
        | Opcode::IntShrChecked => (2, u32_type(), arithmetic_result_type(u32_type())),
        Opcode::IntNegChecked => (1, signed32.clone(), arithmetic_result_type(signed32)),
        Opcode::FloatAdd | Opcode::FloatSub | Opcode::FloatMul | Opcode::FloatDiv => {
            (2, TypeExpr::F64, TypeExpr::F64)
        }
        Opcode::FloatNeg => (1, TypeExpr::F64, TypeExpr::F64),
        Opcode::OptionSome => (
            1,
            TypeExpr::Bool,
            TypeExpr::Option(Box::new(TypeExpr::Bool)),
        ),
        Opcode::OptionNone => (
            0,
            TypeExpr::Unit,
            TypeExpr::Option(Box::new(TypeExpr::Bool)),
        ),
        Opcode::ResultOk => (
            1,
            TypeExpr::Bool,
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Bool),
                error: Box::new(TypeExpr::Unit),
            },
        ),
        Opcode::ResultErr => (
            1,
            TypeExpr::Bool,
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Unit),
                error: Box::new(TypeExpr::Bool),
            },
        ),
        Opcode::CellNew => (
            1,
            TypeExpr::Bool,
            TypeExpr::LocalCell(Box::new(TypeExpr::Bool)),
        ),
        Opcode::CellGet => (
            1,
            TypeExpr::LocalCell(Box::new(TypeExpr::Bool)),
            TypeExpr::Bool,
        ),
        Opcode::ValueHash => (1, TypeExpr::Bool, TypeExpr::Bytes),
        other => panic!("scalar reference fixture does not support {other:?}"),
    };
    let parameter_ids: Vec<EntityId> = (0..parameter_count)
        .map(|index| id(10 + u8::try_from(index).expect("small parameter index")))
        .collect();
    let function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: parameter_ids.clone(),
        result_type: result_type.clone(),
        effects: Vec::new(),
        entry_block: block_id,
        blocks: vec![block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters: Vec<Parameter> = parameter_ids
        .iter()
        .enumerate()
        .map(|(ordinal, entity_id)| Parameter {
            entity_id: *entity_id,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: u32::try_from(ordinal).expect("small parameter ordinal"),
            value_type: operand_type.clone(),
        })
        .collect();
    let operation = Operation {
        entity_id: operation_id,
        block: block_id,
        ordinal: 0,
        opcode,
        operands: parameter_ids
            .iter()
            .copied()
            .map(ValueRef::Parameter)
            .collect(),
        result_types: vec![result_type],
        immediate: Immediate::None,
    };
    let block = Block {
        entity_id: block_id,
        function: function_id,
        parameters: Vec::new(),
        operations: vec![operation_id],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: operation_id,
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    };
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    let lowered = sley_vm::lower_function(sley_vm::LoweringInput {
        types: &types,
        function: &function,
        parameters: &parameters,
        blocks: &[block],
        operations: &[operation],
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: std::slice::from_ref(&function),
        contracts: &[],
        adapters: &[],
    })
    .unwrap_or_else(|error| panic!("native reference lowers {opcode:?}: {error:?}"));
    lowered.bytecode.blocks[0].instructions[0].clone()
}

#[allow(clippy::too_many_lines)]
fn native_variadic_instruction(opcode: Opcode) -> sley_vm::Instruction {
    let vector = TypeExpr::Vector(Box::new(TypeExpr::Bool));
    let map = TypeExpr::OrderedMap {
        key: Box::new(TypeExpr::Bool),
        value: Box::new(u32_type()),
    };
    let (parameter_types, result_type) = match opcode {
        Opcode::FloatFma => (vec![TypeExpr::F64; 3], TypeExpr::F64),
        Opcode::TupleNew => (
            vec![TypeExpr::Bool, u32_type()],
            TypeExpr::Tuple(vec![TypeExpr::Bool, u32_type()]),
        ),
        Opcode::VectorNew => (vec![TypeExpr::Bool, TypeExpr::Bool], vector.clone()),
        Opcode::VectorLen => (vec![vector.clone()], u64_type()),
        Opcode::VectorGet => (
            vec![vector.clone(), u64_type()],
            TypeExpr::Option(Box::new(TypeExpr::Bool)),
        ),
        Opcode::VectorSet => (
            vec![vector.clone(), u64_type(), TypeExpr::Bool],
            TypeExpr::Result {
                ok: Box::new(vector.clone()),
                error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
            },
        ),
        Opcode::MapNew => (
            vec![TypeExpr::Bool, u32_type(), TypeExpr::Bool, u32_type()],
            TypeExpr::Result {
                ok: Box::new(map.clone()),
                error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
            },
        ),
        Opcode::MapGet => (
            vec![map.clone(), TypeExpr::Bool],
            TypeExpr::Option(Box::new(u32_type())),
        ),
        Opcode::MapContains => (vec![map.clone(), TypeExpr::Bool], TypeExpr::Bool),
        Opcode::MapInsert => (vec![map.clone(), TypeExpr::Bool, u32_type()], map.clone()),
        Opcode::MapRemove => (vec![map.clone(), TypeExpr::Bool], map),
        other => panic!("variadic reference fixture does not support {other:?}"),
    };
    let function_id = id(1);
    let block_id = id(2);
    let operation_id = id(3);
    let parameter_ids = (0..parameter_types.len())
        .map(|index| id(10 + u8::try_from(index).expect("small parameter index")))
        .collect::<Vec<_>>();
    let function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: parameter_ids.clone(),
        result_type: result_type.clone(),
        effects: Vec::new(),
        entry_block: block_id,
        blocks: vec![block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = parameter_ids
        .iter()
        .zip(parameter_types)
        .enumerate()
        .map(|(ordinal, (entity_id, value_type))| Parameter {
            entity_id: *entity_id,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: u32::try_from(ordinal).expect("small parameter ordinal"),
            value_type,
        })
        .collect::<Vec<_>>();
    let operation = Operation {
        entity_id: operation_id,
        block: block_id,
        ordinal: 0,
        opcode,
        operands: parameter_ids
            .iter()
            .copied()
            .map(ValueRef::Parameter)
            .collect(),
        result_types: vec![result_type],
        immediate: Immediate::None,
    };
    let block = Block {
        entity_id: block_id,
        function: function_id,
        parameters: Vec::new(),
        operations: vec![operation_id],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: operation_id,
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    };
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    sley_vm::lower_function(sley_vm::LoweringInput {
        types: &types,
        function: &function,
        parameters: &parameters,
        blocks: &[block],
        operations: &[operation],
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: std::slice::from_ref(&function),
        contracts: &[],
        adapters: &[],
    })
    .unwrap_or_else(|error| panic!("native reference lowers {opcode:?}: {error:?}"))
    .bytecode
    .blocks[0]
        .instructions[0]
        .clone()
}

/// One native lowering image that covers all seven immediate-bearing entries
/// in the frozen bootstrap opcode table plus the separately admitted bridge
/// operation. The results deliberately form one dense register sequence so
/// the Sley algorithm must preserve both prior-result references and the
/// result frontier for every immediate family.
#[allow(clippy::too_many_lines)]
fn native_bootstrap_immediate_instructions() -> Vec<sley_vm::Instruction> {
    let function_id = id(1);
    let block_id = id(2);
    let operation_ids = (3_u8..=10).map(id).collect::<Vec<_>>();
    let parameter_ids = (11_u8..=15).map(id).collect::<Vec<_>>();
    let callee_id = id(20);
    let callee_block_id = id(21);
    let callee_parameter_id = id(22);
    let constant_id = id(60);
    let record_id = id(50);
    let variant_id = id(51);
    let record_field_a = MemberId::from_bytes([0xA1; 32]);
    let record_field_b = MemberId::from_bytes([0xB2; 32]);
    let variant_case = MemberId::from_bytes([0xC1; 32]);
    let octet = TypeExpr::UInt(IntegerWidth::from_bits(8));
    let byte_vector = TypeExpr::Vector(Box::new(octet));
    let bridge_result = TypeExpr::Result {
        ok: Box::new(byte_vector.clone()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
    };
    let pair_type = TypeExpr::Named(NamedType {
        definition: record_id,
        arguments: Vec::new(),
    });
    let shape_type = TypeExpr::Named(NamedType {
        definition: variant_id,
        arguments: Vec::new(),
    });
    let definitions = vec![
        TypeDefinition {
            entity_id: record_id,
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(vec![
                RecordField {
                    member_id: record_field_a,
                    value_type: u64_type(),
                    visibility: Visibility::Private,
                },
                RecordField {
                    member_id: record_field_b,
                    value_type: TypeExpr::Text,
                    visibility: Visibility::Private,
                },
            ]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
        TypeDefinition {
            entity_id: variant_id,
            type_parameters: Vec::new(),
            form: TypeDefForm::Variant(vec![VariantCase {
                member_id: variant_case,
                payload_type: Some(u64_type()),
            }]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
    ];
    let function_result = bridge_result.clone();
    let function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: parameter_ids.clone(),
        result_type: function_result,
        effects: Vec::new(),
        entry_block: block_id,
        blocks: vec![block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let callee = FunctionGraph {
        entity_id: callee_id,
        type_parameters: Vec::new(),
        parameters: vec![callee_parameter_id],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: callee_block_id,
        blocks: vec![callee_block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameter_types = [
        TypeExpr::Tuple(vec![TypeExpr::Bool, u32_type()]),
        u64_type(),
        TypeExpr::Text,
        TypeExpr::Unit,
        TypeExpr::Bytes,
    ];
    let mut parameters = parameter_ids
        .iter()
        .zip(parameter_types)
        .enumerate()
        .map(|(ordinal, (entity_id, value_type))| Parameter {
            entity_id: *entity_id,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: u32::try_from(ordinal).expect("small parameter ordinal"),
            value_type,
        })
        .collect::<Vec<_>>();
    parameters.push(Parameter {
        entity_id: callee_parameter_id,
        owner: callee_id,
        role: ParameterRole::Function,
        ordinal: 0,
        value_type: TypeExpr::Bool,
    });

    let result = |operation: EntityId| {
        ValueRef::OperationResult(OperationResultRef {
            operation,
            result_index: 0,
        })
    };
    let operations = vec![
        Operation {
            entity_id: operation_ids[0],
            block: block_id,
            ordinal: 0,
            opcode: Opcode::ConstantRef,
            operands: Vec::new(),
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::Entity(constant_id),
        },
        Operation {
            entity_id: operation_ids[1],
            block: block_id,
            ordinal: 1,
            opcode: Opcode::TupleGet,
            operands: vec![ValueRef::Parameter(parameter_ids[0])],
            result_types: vec![u32_type()],
            immediate: Immediate::Index(1),
        },
        Operation {
            entity_id: operation_ids[2],
            block: block_id,
            ordinal: 2,
            opcode: Opcode::RecordNew,
            operands: vec![
                ValueRef::Parameter(parameter_ids[1]),
                ValueRef::Parameter(parameter_ids[2]),
            ],
            result_types: vec![pair_type.clone()],
            immediate: Immediate::Entity(record_id),
        },
        Operation {
            entity_id: operation_ids[3],
            block: block_id,
            ordinal: 3,
            opcode: Opcode::RecordGet,
            operands: vec![result(operation_ids[2])],
            result_types: vec![TypeExpr::Text],
            immediate: Immediate::Field(record_field_b),
        },
        Operation {
            entity_id: operation_ids[4],
            block: block_id,
            ordinal: 4,
            opcode: Opcode::VariantNew,
            operands: vec![ValueRef::Parameter(parameter_ids[1])],
            result_types: vec![shape_type],
            immediate: Immediate::Variant(VariantImmediate {
                definition: variant_id,
                member_id: variant_case,
            }),
        },
        Operation {
            entity_id: operation_ids[5],
            block: block_id,
            ordinal: 5,
            opcode: Opcode::VariantGet,
            operands: vec![result(operation_ids[4])],
            result_types: vec![TypeExpr::Option(Box::new(u64_type()))],
            immediate: Immediate::Variant(VariantImmediate {
                definition: variant_id,
                member_id: variant_case,
            }),
        },
        Operation {
            entity_id: operation_ids[6],
            block: block_id,
            ordinal: 6,
            opcode: Opcode::CallDirect,
            operands: vec![result(operation_ids[0])],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::Function(FunctionRefValue {
                function: callee_id,
                type_arguments: Vec::new(),
            }),
        },
        Operation {
            entity_id: operation_ids[7],
            block: block_id,
            ordinal: 7,
            opcode: Opcode::AdapterInvoke,
            operands: vec![
                ValueRef::Parameter(parameter_ids[3]),
                ValueRef::Parameter(parameter_ids[4]),
            ],
            result_types: vec![bridge_result],
            immediate: Immediate::Entity(EntityId::from_bytes(sley_vm::host_abi::bridge_identity(
                sley_vm::host_abi::BRIDGE_CODE_B2V1,
            ))),
        },
    ];
    let block = Block {
        entity_id: block_id,
        function: function_id,
        parameters: Vec::new(),
        operations: operation_ids.clone(),
        terminator: Terminator::Return(ReturnTerminator {
            value: result(operation_ids[7]),
        }),
        reachability: Reachability::Required,
    };
    let callee_block = Block {
        entity_id: callee_block_id,
        function: callee_id,
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::Parameter(callee_parameter_id),
        }),
        reachability: Reachability::Required,
    };
    let constants = vec![ConstantDefinition {
        entity_id: constant_id,
        value: bool_value(true),
    }];
    let adapter_identity = sley_vm::host_abi::bridge_identity(sley_vm::host_abi::BRIDGE_CODE_B2V1);
    let adapters = vec![AdapterImport {
        entity_id: EntityId::from_bytes(adapter_identity),
        adapter_id: adapter_identity,
        abi_version: sley_vm::host_abi::BRIDGE_ABI_VERSION,
        request_type: TypeExpr::Bytes,
        response_type: byte_vector,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    }];
    let types = sley_check::TypeEnvironment::new(definitions).unwrap();
    sley_vm::lower_function(sley_vm::LoweringInput {
        types: &types,
        function: &function,
        parameters: &parameters,
        blocks: &[block, callee_block],
        operations: &operations,
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &constants,
        globals: &[],
        functions: &[function.clone(), callee],
        contracts: &[],
        adapters: &adapters,
    })
    .expect("native reference lowers the bootstrap immediate family")
    .bytecode
    .blocks
    .remove(0)
    .instructions
}

fn native_bool_chain() -> Vec<sley_vm::Instruction> {
    let function_id = id(1);
    let block_id = id(2);
    let left = id(10);
    let right = id(11);
    let first_id = id(3);
    let second_id = id(4);
    let function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: vec![left, right],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: block_id,
        blocks: vec![block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = vec![
        Parameter {
            entity_id: left,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: right,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: TypeExpr::Bool,
        },
    ];
    let operations = vec![
        Operation {
            entity_id: first_id,
            block: block_id,
            ordinal: 0,
            opcode: Opcode::BoolAnd,
            operands: vec![ValueRef::Parameter(left), ValueRef::Parameter(right)],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        },
        Operation {
            entity_id: second_id,
            block: block_id,
            ordinal: 1,
            opcode: Opcode::BoolNot,
            operands: vec![ValueRef::OperationResult(OperationResultRef {
                operation: first_id,
                result_index: 0,
            })],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        },
    ];
    let block = Block {
        entity_id: block_id,
        function: function_id,
        parameters: Vec::new(),
        operations: vec![first_id, second_id],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: second_id,
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    };
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    sley_vm::lower_function(sley_vm::LoweringInput {
        types: &types,
        function: &function,
        parameters: &parameters,
        blocks: &[block],
        operations: &operations,
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: std::slice::from_ref(&function),
        contracts: &[],
        adapters: &[],
    })
    .expect("native reference lowers the Boolean chain")
    .bytecode
    .blocks
    .remove(0)
    .instructions
}

#[allow(clippy::too_many_lines)]
fn native_cell_chain() -> Vec<sley_vm::Instruction> {
    let function_id = id(1);
    let block_id = id(2);
    let value = id(10);
    let replacement = id(11);
    let new_id = id(3);
    let set_id = id(4);
    let get_id = id(5);
    let function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: vec![value, replacement],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: block_id,
        blocks: vec![block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = vec![
        Parameter {
            entity_id: value,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: replacement,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: TypeExpr::Bool,
        },
    ];
    let operations = vec![
        Operation {
            entity_id: new_id,
            block: block_id,
            ordinal: 0,
            opcode: Opcode::CellNew,
            operands: vec![ValueRef::Parameter(value)],
            result_types: vec![TypeExpr::LocalCell(Box::new(TypeExpr::Bool))],
            immediate: Immediate::None,
        },
        Operation {
            entity_id: set_id,
            block: block_id,
            ordinal: 1,
            opcode: Opcode::CellSet,
            operands: vec![
                ValueRef::OperationResult(OperationResultRef {
                    operation: new_id,
                    result_index: 0,
                }),
                ValueRef::Parameter(replacement),
            ],
            result_types: vec![TypeExpr::Unit],
            immediate: Immediate::None,
        },
        Operation {
            entity_id: get_id,
            block: block_id,
            ordinal: 2,
            opcode: Opcode::CellGet,
            operands: vec![ValueRef::OperationResult(OperationResultRef {
                operation: new_id,
                result_index: 0,
            })],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        },
    ];
    let block = Block {
        entity_id: block_id,
        function: function_id,
        parameters: Vec::new(),
        operations: vec![new_id, set_id, get_id],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: get_id,
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    };
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    sley_vm::lower_function(sley_vm::LoweringInput {
        types: &types,
        function: &function,
        parameters: &parameters,
        blocks: &[block],
        operations: &operations,
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: std::slice::from_ref(&function),
        contracts: &[],
        adapters: &[],
    })
    .expect("native reference lowers local-cell construction and read")
    .bytecode
    .blocks
    .remove(0)
    .instructions
}

#[allow(clippy::too_many_lines)]
fn native_simple_terminator(kind: u32) -> (sley_vm::BytecodeTerminator, u32, u32) {
    let function_id = id(1);
    let entry_id = id(2);
    let operation_id = id(3);
    let target_id = id(4);
    let left = id(10);
    let right = id(11);
    let target_parameter = id(12);
    let result = ValueRef::OperationResult(OperationResultRef {
        operation: operation_id,
        result_index: 0,
    });
    let mut function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: vec![left, right],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: entry_id,
        blocks: vec![entry_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let mut parameters = vec![
        Parameter {
            entity_id: left,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: right,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: TypeExpr::Bool,
        },
    ];
    let operation = Operation {
        entity_id: operation_id,
        block: entry_id,
        ordinal: 0,
        opcode: Opcode::BoolAnd,
        operands: vec![ValueRef::Parameter(left), ValueRef::Parameter(right)],
        result_types: vec![TypeExpr::Bool],
        immediate: Immediate::None,
    };
    let terminator = match kind {
        1 => Terminator::Return(ReturnTerminator { value: result }),
        2 => Terminator::Branch(BranchTerminator {
            edge: TargetEdge {
                target: target_id,
                arguments: vec![result],
            },
        }),
        3 => Terminator::CondBranch(CondBranchTerminator {
            condition: result,
            if_true: TargetEdge {
                target: target_id,
                arguments: vec![result],
            },
            if_false: TargetEdge {
                target: target_id,
                arguments: vec![ValueRef::Parameter(left)],
            },
        }),
        5 => Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: Some(result),
        }),
        _ => panic!("unsupported native simple terminator fixture"),
    };
    let mut blocks = vec![Block {
        entity_id: entry_id,
        function: function_id,
        parameters: Vec::new(),
        operations: vec![operation_id],
        terminator,
        reachability: Reachability::Required,
    }];
    if matches!(kind, 2 | 3) {
        function.blocks.push(target_id);
        parameters.push(Parameter {
            entity_id: target_parameter,
            owner: target_id,
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        });
        blocks.push(Block {
            entity_id: target_id,
            function: function_id,
            parameters: vec![target_parameter],
            operations: Vec::new(),
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::Parameter(target_parameter),
            }),
            reachability: Reachability::Required,
        });
    }
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    let lowered = sley_vm::lower_function(sley_vm::LoweringInput {
        types: &types,
        function: &function,
        parameters: &parameters,
        blocks: &blocks,
        operations: &[operation],
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: std::slice::from_ref(&function),
        contracts: &[],
        adapters: &[],
    })
    .expect("native reference lowers simple terminator");
    (
        lowered.bytecode.blocks[0].terminator.clone(),
        u32::try_from(lowered.bytecode.register_types.len()).expect("register count fits u32"),
        u32::try_from(lowered.bytecode.blocks.len()).expect("block count fits u32"),
    )
}

#[allow(clippy::too_many_lines)]
fn native_builtin_switch_terminator() -> (sley_vm::BytecodeTerminator, u32, u32) {
    let function_id = id(1);
    let entry_id = id(2);
    let operation_id = id(3);
    let none_target = id(4);
    let some_target = id(5);
    let left = id(10);
    let right = id(11);
    let none_parameter = id(12);
    let some_payload_parameter = id(13);
    let some_value_parameter = id(14);
    let result = ValueRef::OperationResult(OperationResultRef {
        operation: operation_id,
        result_index: 0,
    });
    let function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: vec![left, right],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: entry_id,
        blocks: vec![entry_id, none_target, some_target],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = vec![
        Parameter {
            entity_id: left,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: right,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: none_parameter,
            owner: none_target,
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: some_payload_parameter,
            owner: some_target,
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: some_value_parameter,
            owner: some_target,
            role: ParameterRole::Block,
            ordinal: 1,
            value_type: TypeExpr::Bool,
        },
    ];
    let operation = Operation {
        entity_id: operation_id,
        block: entry_id,
        ordinal: 0,
        opcode: Opcode::OptionSome,
        operands: vec![ValueRef::Parameter(left)],
        result_types: vec![TypeExpr::Option(Box::new(TypeExpr::Bool))],
        immediate: Immediate::None,
    };
    let blocks = vec![
        Block {
            entity_id: entry_id,
            function: function_id,
            parameters: Vec::new(),
            operations: vec![operation_id],
            terminator: Terminator::VariantSwitch(VariantSwitchTerminator {
                value: result,
                cases: vec![
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::None),
                        edge: SwitchEdge {
                            target: none_target,
                            arguments: vec![SwitchArgument::Value(ValueRef::Parameter(right))],
                        },
                    },
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Some),
                        edge: SwitchEdge {
                            target: some_target,
                            arguments: vec![
                                SwitchArgument::CasePayload,
                                SwitchArgument::Value(ValueRef::Parameter(left)),
                            ],
                        },
                    },
                ],
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: none_target,
            function: function_id,
            parameters: vec![none_parameter],
            operations: Vec::new(),
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::Parameter(none_parameter),
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: some_target,
            function: function_id,
            parameters: vec![some_payload_parameter, some_value_parameter],
            operations: Vec::new(),
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::Parameter(some_value_parameter),
            }),
            reachability: Reachability::Required,
        },
    ];
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    let lowered = sley_vm::lower_function(sley_vm::LoweringInput {
        types: &types,
        function: &function,
        parameters: &parameters,
        blocks: &blocks,
        operations: &[operation],
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: std::slice::from_ref(&function),
        contracts: &[],
        adapters: &[],
    })
    .expect("native reference lowers the built-in variant switch");
    (
        lowered.bytecode.blocks[0].terminator.clone(),
        u32::try_from(lowered.bytecode.register_types.len()).expect("register count fits u32"),
        u32::try_from(lowered.bytecode.blocks.len()).expect("block count fits u32"),
    )
}

#[allow(clippy::too_many_lines)]
fn native_complete_lowered() -> sley_vm::LoweredFunction {
    let function_id = id(1);
    let block_ids = [id(2), id(3), id(4)];
    let operation_ids = [id(5), id(6), id(7)];
    let function_parameters = [id(10), id(11)];
    let none_parameter = id(12);
    let some_parameters = [id(13), id(14)];
    let option_result = ValueRef::OperationResult(OperationResultRef {
        operation: operation_ids[0],
        result_index: 0,
    });
    let none_result = ValueRef::OperationResult(OperationResultRef {
        operation: operation_ids[1],
        result_index: 0,
    });
    let some_result = ValueRef::OperationResult(OperationResultRef {
        operation: operation_ids[2],
        result_index: 0,
    });
    let function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: function_parameters.to_vec(),
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: block_ids[0],
        blocks: block_ids.to_vec(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = vec![
        Parameter {
            entity_id: function_parameters[0],
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: function_parameters[1],
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: none_parameter,
            owner: block_ids[1],
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: some_parameters[0],
            owner: block_ids[2],
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: some_parameters[1],
            owner: block_ids[2],
            role: ParameterRole::Block,
            ordinal: 1,
            value_type: TypeExpr::Bool,
        },
    ];
    let operations = vec![
        Operation {
            entity_id: operation_ids[0],
            block: block_ids[0],
            ordinal: 0,
            opcode: Opcode::OptionSome,
            operands: vec![ValueRef::Parameter(function_parameters[0])],
            result_types: vec![TypeExpr::Option(Box::new(TypeExpr::Bool))],
            immediate: Immediate::None,
        },
        Operation {
            entity_id: operation_ids[1],
            block: block_ids[1],
            ordinal: 0,
            opcode: Opcode::BoolNot,
            operands: vec![ValueRef::Parameter(none_parameter)],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        },
        Operation {
            entity_id: operation_ids[2],
            block: block_ids[2],
            ordinal: 0,
            opcode: Opcode::BoolAnd,
            operands: vec![
                ValueRef::Parameter(some_parameters[0]),
                ValueRef::Parameter(some_parameters[1]),
            ],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        },
    ];
    let blocks = vec![
        Block {
            entity_id: block_ids[0],
            function: function_id,
            parameters: Vec::new(),
            operations: vec![operation_ids[0]],
            terminator: Terminator::VariantSwitch(VariantSwitchTerminator {
                value: option_result,
                cases: vec![
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::None),
                        edge: SwitchEdge {
                            target: block_ids[1],
                            arguments: vec![SwitchArgument::Value(ValueRef::Parameter(
                                function_parameters[1],
                            ))],
                        },
                    },
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Some),
                        edge: SwitchEdge {
                            target: block_ids[2],
                            arguments: vec![
                                SwitchArgument::CasePayload,
                                SwitchArgument::Value(ValueRef::Parameter(function_parameters[0])),
                            ],
                        },
                    },
                ],
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: block_ids[1],
            function: function_id,
            parameters: vec![none_parameter],
            operations: vec![operation_ids[1]],
            terminator: Terminator::Return(ReturnTerminator { value: none_result }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: block_ids[2],
            function: function_id,
            parameters: some_parameters.to_vec(),
            operations: vec![operation_ids[2]],
            terminator: Terminator::Return(ReturnTerminator { value: some_result }),
            reachability: Reachability::Required,
        },
    ];
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    sley_vm::lower_function(sley_vm::LoweringInput {
        types: &types,
        function: &function,
        parameters: &parameters,
        blocks: &blocks,
        operations: &operations,
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: std::slice::from_ref(&function),
        contracts: &[],
        adapters: &[],
    })
    .expect("native reference lowers complete multi-block function")
}

fn native_complete_function() -> sley_vm::BytecodeFunction {
    native_complete_lowered().bytecode
}

#[allow(clippy::too_many_lines)]
fn native_direct_call_lowered() -> sley_vm::LoweredFunction {
    let root_id = id(1);
    let root_block_id = id(2);
    let root_operation_id = id(3);
    let root_parameter_id = id(4);
    let callee_id = id(20);
    let callee_block_id = id(21);
    let callee_operation_id = id(22);
    let callee_parameter_id = id(23);
    let leaf_id = id(10);
    let leaf_block_id = id(11);
    let leaf_operation_id = id(12);
    let leaf_parameter_id = id(13);
    let root_graph = FunctionGraph {
        entity_id: root_id,
        type_parameters: Vec::new(),
        parameters: vec![root_parameter_id],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: root_block_id,
        blocks: vec![root_block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let callee = FunctionGraph {
        entity_id: callee_id,
        type_parameters: Vec::new(),
        parameters: vec![callee_parameter_id],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: callee_block_id,
        blocks: vec![callee_block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let leaf = FunctionGraph {
        entity_id: leaf_id,
        type_parameters: Vec::new(),
        parameters: vec![leaf_parameter_id],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: leaf_block_id,
        blocks: vec![leaf_block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = vec![
        Parameter {
            entity_id: root_parameter_id,
            owner: root_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: callee_parameter_id,
            owner: callee_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
        Parameter {
            entity_id: leaf_parameter_id,
            owner: leaf_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
    ];
    let root_operation = Operation {
        entity_id: root_operation_id,
        block: root_block_id,
        ordinal: 0,
        opcode: Opcode::CallDirect,
        operands: vec![ValueRef::Parameter(root_parameter_id)],
        result_types: vec![TypeExpr::Bool],
        immediate: Immediate::Function(FunctionRefValue {
            function: callee_id,
            type_arguments: Vec::new(),
        }),
    };
    let callee_operation = Operation {
        entity_id: callee_operation_id,
        block: callee_block_id,
        ordinal: 0,
        opcode: Opcode::CallDirect,
        operands: vec![ValueRef::Parameter(callee_parameter_id)],
        result_types: vec![TypeExpr::Bool],
        immediate: Immediate::Function(FunctionRefValue {
            function: leaf_id,
            type_arguments: Vec::new(),
        }),
    };
    let leaf_operation = Operation {
        entity_id: leaf_operation_id,
        block: leaf_block_id,
        ordinal: 0,
        opcode: Opcode::BoolNot,
        operands: vec![ValueRef::Parameter(leaf_parameter_id)],
        result_types: vec![TypeExpr::Bool],
        immediate: Immediate::None,
    };
    let result = |operation| {
        ValueRef::OperationResult(OperationResultRef {
            operation,
            result_index: 0,
        })
    };
    let blocks = vec![
        Block {
            entity_id: root_block_id,
            function: root_id,
            parameters: Vec::new(),
            operations: vec![root_operation_id],
            terminator: Terminator::Return(ReturnTerminator {
                value: result(root_operation_id),
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: callee_block_id,
            function: callee_id,
            parameters: Vec::new(),
            operations: vec![callee_operation_id],
            terminator: Terminator::Return(ReturnTerminator {
                value: result(callee_operation_id),
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: leaf_block_id,
            function: leaf_id,
            parameters: Vec::new(),
            operations: vec![leaf_operation_id],
            terminator: Terminator::Return(ReturnTerminator {
                value: result(leaf_operation_id),
            }),
            reachability: Reachability::Required,
        },
    ];
    let functions = [root_graph.clone(), callee, leaf];
    let operations = [root_operation, callee_operation, leaf_operation];
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    sley_vm::lower_function(sley_vm::LoweringInput {
        types: &types,
        function: &root_graph,
        parameters: &parameters,
        blocks: &blocks,
        operations: &operations,
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: &functions,
        contracts: &[],
        adapters: &[],
    })
    .expect("native reference lowers root and its transitive callees")
}

fn builtin_switch_facts(terminator: &sley_vm::BytecodeTerminator) -> (u32, Vec<BuiltinSwitchFact>) {
    let sley_vm::BytecodeTerminator::VariantSwitch { value, cases } = terminator else {
        panic!("native reference must be a variant switch")
    };
    let facts = cases
        .iter()
        .map(|case| {
            let CaseKey::Builtin(key) = case.case_key else {
                panic!("bounded slice accepts built-in case keys")
            };
            let arguments = case
                .edge
                .arguments
                .iter()
                .map(|argument| match argument {
                    sley_vm::BytecodeSwitchArgument::Value(register) => (1, *register),
                    sley_vm::BytecodeSwitchArgument::CasePayload => (2, 0),
                })
                .collect();
            (key.tag(), case.edge.target, arguments)
        })
        .collect();
    (*value, facts)
}

fn assert_single_lowered(outcome: &sley_vm::ExecutionOutcome, expected: &sley_vm::Instruction) {
    use sley_ssmc::ResultConst;
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(value) => match &value.data {
            ConstData::Result(ResultConst::Ok(model)) => match &model.data {
                ConstData::Sequence(fields) if fields.len() == 3 => {
                    let uint = |value: &ConstValue| match value.data {
                        ConstData::UInt(found) => {
                            u32::try_from(found).expect("lowered register fits u32")
                        }
                        ref other => panic!("lowered scalar must be UInt32, got {other:?}"),
                    };
                    let registers = |value: &ConstValue| match &value.data {
                        ConstData::Sequence(found) => found.iter().map(uint).collect::<Vec<_>>(),
                        other => panic!("lowered register list must be Vector, got {other:?}"),
                    };
                    assert_eq!(uint(&fields[0]), expected.opcode, "opcode tag parity");
                    assert_eq!(
                        registers(&fields[1]),
                        expected.operands,
                        "operand-register parity"
                    );
                    assert_eq!(
                        registers(&fields[2]),
                        expected.results,
                        "result-register parity"
                    );
                }
                other => panic!("lowered Ok must carry a 3-tuple, got {other:?}"),
            },
            other => panic!("single-operation lowering must return Ok, got {other:?}"),
        },
        other => panic!("single-operation lowering must succeed, got {other:?}"),
    }
}

fn assert_single_lower_error(outcome: &sley_vm::ExecutionOutcome, expected: u32) {
    use sley_ssmc::ResultConst;
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(value) => match &value.data {
            ConstData::Result(ResultConst::Err(code)) => match code.data {
                ConstData::UInt(found) => assert_eq!(
                    u32::try_from(found).expect("lowering error fits u32"),
                    expected
                ),
                ref other => panic!("lowering Err must carry UInt32, got {other:?}"),
            },
            other => panic!("single-operation lowering must return Err, got {other:?}"),
        },
        other => panic!("single-operation lowering must terminate with a value, got {other:?}"),
    }
}

#[test]
fn lower_single_boolean_operations_match_native_dense_registers() {
    let (package, approved) = admit_lower_program(&single_bool_lowerer());
    for opcode in [Opcode::BoolNot, Opcode::BoolAnd, Opcode::BoolOr] {
        let reference = native_single_scalar(opcode);
        let parameter_count = u32::try_from(reference.operands.len()).expect("small arity");
        let operand_zero = reference.operands[0];
        let operand_one = reference.operands.get(1).copied().unwrap_or(0);
        let next_register = reference.results[0];
        let first = execute_single_bool(
            &package,
            &approved,
            opcode.tag(),
            parameter_count,
            operand_zero,
            operand_one,
            next_register,
        );
        let second = execute_single_bool(
            &package,
            &approved,
            opcode.tag(),
            parameter_count,
            operand_zero,
            operand_one,
            next_register,
        );
        assert_single_lowered(&first, &reference);
        assert_single_lowered(&second, &reference);
        assert_eq!(
            first.termination, second.termination,
            "lowering is deterministic"
        );
    }
}

#[test]
fn lower_composes_over_a_prior_operation_result() {
    let (package, approved) = admit_lower_program(&single_bool_lowerer());
    let native = native_bool_chain();
    assert_eq!(native.len(), 2, "reference chain has two instructions");
    let first = execute_single_bool(&package, &approved, Opcode::BoolAnd.tag(), 2, 0, 1, 2);
    let second = execute_single_bool(&package, &approved, Opcode::BoolNot.tag(), 1, 2, 0, 3);
    assert_single_lowered(&first, &native[0]);
    assert_single_lowered(&second, &native[1]);
}

#[test]
fn lower_single_boolean_operations_return_frozen_errors() {
    let (package, approved) = admit_lower_program(&single_bool_lowerer());
    for (opcode, count) in [
        (Opcode::BoolNot.tag(), 0),
        (Opcode::BoolNot.tag(), 2),
        (Opcode::BoolAnd.tag(), 1),
        (Opcode::BoolOr.tag(), 3),
    ] {
        assert_single_lower_error(
            &execute_single_bool(&package, &approved, opcode, count, 0, 1, 2),
            sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
        );
    }
    for opcode in [0, Opcode::Equal.tag(), u32::MAX] {
        assert_single_lower_error(
            &execute_single_bool(&package, &approved, opcode, 2, 0, 1, 2),
            sley_vm::LowerErrorCode::OpcodeUnsupported.numeric(),
        );
    }
    for (opcode, count, operand_zero, operand_one, next_register) in [
        (Opcode::BoolNot.tag(), 1, 1, 0, 1),
        (Opcode::BoolAnd.tag(), 2, 0, 2, 2),
        (Opcode::BoolOr.tag(), 2, u32::MAX, 1, 2),
    ] {
        assert_single_lower_error(
            &execute_single_bool(
                &package,
                &approved,
                opcode,
                count,
                operand_zero,
                operand_one,
                next_register,
            ),
            sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
        );
    }
}

#[test]
fn lower_ordered_scalar_inventory_matches_native_model_and_frontier() {
    let (package, approved) = admit_lower_program(&ordered_scalar_inventory_lowerer());
    let rows = [
        (Opcode::BoolAnd.tag(), 2, 0, 1),
        (Opcode::BoolNot.tag(), 1, 2, 0),
    ];
    let native = native_bool_chain();
    let first = execute_bool_inventory(&package, &approved, &rows, 2);
    let second = execute_bool_inventory(&package, &approved, &rows, 2);
    assert_inventory_summary(
        &first,
        &native,
        native
            .iter()
            .flat_map(|instruction| instruction.results.iter().copied())
            .max()
            .map_or(2, |register| register + 1),
    );
    assert_eq!(
        first.termination, second.termination,
        "inventory loop is deterministic"
    );

    assert_inventory_summary(&execute_bool_inventory(&package, &approved, &[], 2), &[], 2);

    for (opcode, arity) in [
        (Opcode::Equal, 2),
        (Opcode::NotEqual, 2),
        (Opcode::LessThan, 2),
        (Opcode::LessEqual, 2),
        (Opcode::GreaterThan, 2),
        (Opcode::GreaterEqual, 2),
        (Opcode::IntAddChecked, 2),
        (Opcode::IntSubChecked, 2),
        (Opcode::IntMulChecked, 2),
        (Opcode::IntDivChecked, 2),
        (Opcode::IntRemChecked, 2),
        (Opcode::IntNegChecked, 1),
        (Opcode::IntShlChecked, 2),
        (Opcode::IntShrChecked, 2),
        (Opcode::FloatAdd, 2),
        (Opcode::FloatSub, 2),
        (Opcode::FloatMul, 2),
        (Opcode::FloatDiv, 2),
        (Opcode::FloatNeg, 1),
        (Opcode::OptionSome, 1),
        (Opcode::OptionNone, 0),
        (Opcode::ResultOk, 1),
        (Opcode::ResultErr, 1),
        (Opcode::ValueHash, 1),
    ] {
        let expected = native_single_scalar(opcode);
        assert_inventory_summary(
            &execute_bool_inventory(&package, &approved, &[(opcode.tag(), arity, 0, 1)], arity),
            std::slice::from_ref(&expected),
            arity + 1,
        );
    }

    let cell_native = native_cell_chain();
    assert_inventory_summary(
        &execute_bool_inventory(
            &package,
            &approved,
            &[
                (Opcode::CellNew.tag(), 1, 0, 0),
                (Opcode::CellSet.tag(), 2, 2, 1),
                (Opcode::CellGet.tag(), 1, 2, 0),
            ],
            2,
        ),
        &cell_native,
        5,
    );
}

fn empty_simple_terminator_value() -> ConstValue {
    ConstValue {
        value_type: terminator_model_type(),
        data: ConstData::Sequence(vec![
            u32_value(0),
            u32_value(0),
            u32_value(0),
            u32vec_value(&[]),
            u32_value(0),
            u32vec_value(&[]),
            optional_u32_value(None),
        ]),
    }
}

fn builtin_switch_model_value(selector: u32, cases: &[BuiltinSwitchFact]) -> ConstValue {
    ConstValue {
        value_type: builtin_switch_model_type(),
        data: ConstData::Sequence(vec![
            u32_value(u128::from(selector)),
            builtin_switch_facts_value(cases),
        ]),
    }
}

fn complete_terminator_model_value(terminator: &sley_vm::BytecodeTerminator) -> ConstValue {
    let (kind, simple, builtin) =
        if let sley_vm::BytecodeTerminator::VariantSwitch { .. } = terminator {
            let (selector, cases) = builtin_switch_facts(terminator);
            (
                4,
                empty_simple_terminator_value(),
                builtin_switch_model_value(selector, &cases),
            )
        } else {
            let (kind, ..) = simple_terminator_fact(terminator);
            (
                kind,
                simple_terminator_value(terminator),
                builtin_switch_model_value(0, &[]),
            )
        };
    ConstValue {
        value_type: complete_terminator_model_type(),
        data: ConstData::Sequence(vec![u32_value(u128::from(kind)), simple, builtin]),
    }
}

fn complete_block_model_value(block: &sley_vm::BytecodeBlock) -> ConstValue {
    ConstValue {
        value_type: complete_block_model_type(),
        data: ConstData::Sequence(vec![
            u32_value(u128::from(block.slot)),
            u32vec_value(&block.parameter_registers),
            immediate_inventory_model_value(&block.instructions),
            complete_terminator_model_value(&block.terminator),
            u32_value(u128::from(block.reachability)),
        ]),
    }
}

fn assert_complete_function_model(
    outcome: &sley_vm::ExecutionOutcome,
    expected: &sley_vm::BytecodeFunction,
) {
    use sley_ssmc::ResultConst;
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("complete function lowering must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(model)) = &value.data else {
        panic!(
            "complete function lowering must return Ok, got {:?}",
            value.data
        )
    };
    let expected_map = ConstValue {
        value_type: complete_block_map_type(),
        data: ConstData::Map(
            expected
                .blocks
                .iter()
                .map(|block| sley_ssmc::MapEntryConst {
                    key: u32_value(u128::from(block.slot)),
                    value: complete_block_model_value(block),
                })
                .collect(),
        ),
    };
    assert_eq!(
        model.as_ref(),
        &ConstValue {
            value_type: complete_function_model_type(),
            data: ConstData::Sequence(vec![
                bytes_value(expected.function.as_bytes()),
                u32vec_value(&expected.parameter_registers),
                bytesvec_value(
                    &expected
                        .register_types
                        .iter()
                        .map(encoded_type)
                        .collect::<Vec<_>>(),
                ),
                bytes_value(&encoded_type(&expected.result_type)),
                u32_value(u128::from(expected.entry_block)),
                expected_map,
                u32_value(
                    u128::try_from(expected.register_types.len())
                        .expect("register count fits u128"),
                ),
            ]),
        }
    );
}

fn assert_complete_block_summary(
    outcome: &sley_vm::ExecutionOutcome,
    expected_slot: u32,
    expected_parameters: &[u32],
    expected_instructions: &[sley_vm::Instruction],
    expected_terminator: &sley_vm::BytecodeTerminator,
    expected_reachability: u32,
    expected_frontier: u32,
) {
    use sley_ssmc::ResultConst;
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("complete block lowering must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(summary)) = &value.data else {
        panic!(
            "complete block lowering must return Ok, got {:?}",
            value.data
        )
    };
    let ConstData::Sequence(summary_fields) = &summary.data else {
        panic!("complete block summary must be a tuple")
    };
    let ConstData::Sequence(block_fields) = &summary_fields[0].data else {
        panic!("complete block model must be a tuple")
    };
    assert_eq!(block_fields[0], u32_value(u128::from(expected_slot)));
    assert_eq!(block_fields[1], u32vec_value(expected_parameters));
    assert_eq!(
        block_fields[2],
        immediate_inventory_model_value(expected_instructions)
    );
    let ConstData::Sequence(terminator_fields) = &block_fields[3].data else {
        panic!("complete terminator must be a tuple")
    };
    if let sley_vm::BytecodeTerminator::VariantSwitch { .. } = expected_terminator {
        let (selector, cases) = builtin_switch_facts(expected_terminator);
        assert_eq!(terminator_fields[0], u32_value(4));
        assert_eq!(terminator_fields[1], empty_simple_terminator_value());
        assert_eq!(
            terminator_fields[2],
            builtin_switch_model_value(selector, &cases)
        );
    } else {
        let (kind, ..) = simple_terminator_fact(expected_terminator);
        assert_eq!(terminator_fields[0], u32_value(u128::from(kind)));
        assert_eq!(
            terminator_fields[1],
            simple_terminator_value(expected_terminator)
        );
        assert_eq!(terminator_fields[2], builtin_switch_model_value(0, &[]));
    }
    assert_eq!(
        block_fields[4],
        u32_value(u128::from(expected_reachability))
    );
    assert_eq!(summary_fields[1], u32_value(u128::from(expected_frontier)));
}

#[test]
fn lower_ordered_scalar_inventory_checks_every_row_in_order() {
    let (package, approved) = admit_lower_program(&ordered_scalar_inventory_lowerer());
    let valid = (Opcode::BoolAnd.tag(), 2, 0, 1);
    assert_inventory_error(
        &execute_bool_inventory(
            &package,
            &approved,
            &[valid, (Opcode::TupleNew.tag(), 2, 0, 1)],
            2,
        ),
        sley_vm::LowerErrorCode::OpcodeUnsupported.numeric(),
    );
    assert_inventory_error(
        &execute_bool_inventory(
            &package,
            &approved,
            &[valid, (Opcode::BoolNot.tag(), 2, 2, 0)],
            2,
        ),
        sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
    );
    assert_inventory_error(
        &execute_bool_inventory(
            &package,
            &approved,
            &[valid, (Opcode::BoolNot.tag(), 1, 3, 0)],
            2,
        ),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );
    assert_inventory_error(
        &execute_bool_inventory(
            &package,
            &approved,
            &[(Opcode::BoolNot.tag(), 1, 0, 0)],
            u32::MAX,
        ),
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    );
}

#[test]
fn lower_immediate_free_operation_families_match_native_dense_models() {
    let (package, approved) = admit_lower_program(&immediate_free_operation_lowerer());
    for opcode in [
        Opcode::BoolNot,
        Opcode::BoolAnd,
        Opcode::BoolOr,
        Opcode::Equal,
        Opcode::NotEqual,
        Opcode::LessThan,
        Opcode::LessEqual,
        Opcode::GreaterThan,
        Opcode::GreaterEqual,
        Opcode::IntAddChecked,
        Opcode::IntSubChecked,
        Opcode::IntMulChecked,
        Opcode::IntDivChecked,
        Opcode::IntRemChecked,
        Opcode::IntNegChecked,
        Opcode::IntShlChecked,
        Opcode::IntShrChecked,
        Opcode::FloatAdd,
        Opcode::FloatSub,
        Opcode::FloatMul,
        Opcode::FloatDiv,
        Opcode::FloatNeg,
        Opcode::OptionSome,
        Opcode::OptionNone,
        Opcode::ResultOk,
        Opcode::ResultErr,
        Opcode::ValueHash,
    ] {
        let expected = native_single_scalar(opcode);
        let next_register = expected.results[0];
        let first = execute_immediate_free_operation(
            &package,
            &approved,
            opcode,
            &expected.operands,
            next_register,
        );
        let second = execute_immediate_free_operation(
            &package,
            &approved,
            opcode,
            &expected.operands,
            next_register,
        );
        assert_immediate_free_summary(&first, &expected, next_register + 1);
        assert_eq!(first.termination, second.termination);
    }
    for cell_instruction in native_cell_chain() {
        let opcode = Opcode::from_tag(cell_instruction.opcode).expect("cell opcode is frozen");
        let next_register = cell_instruction.results[0];
        let outcome = execute_immediate_free_operation(
            &package,
            &approved,
            opcode,
            &cell_instruction.operands,
            next_register,
        );
        assert_immediate_free_summary(&outcome, &cell_instruction, next_register + 1);
    }

    for opcode in [
        Opcode::FloatFma,
        Opcode::TupleNew,
        Opcode::VectorNew,
        Opcode::VectorLen,
        Opcode::VectorGet,
        Opcode::VectorSet,
        Opcode::MapNew,
        Opcode::MapGet,
        Opcode::MapContains,
        Opcode::MapInsert,
        Opcode::MapRemove,
    ] {
        let expected = native_variadic_instruction(opcode);
        let next_register =
            u32::try_from(expected.operands.len()).expect("small variadic fixture arity");
        let first = execute_immediate_free_operation(
            &package,
            &approved,
            opcode,
            &expected.operands,
            next_register,
        );
        let second = execute_immediate_free_operation(
            &package,
            &approved,
            opcode,
            &expected.operands,
            next_register,
        );
        assert_immediate_free_summary(&first, &expected, next_register + 1);
        assert_eq!(first.termination, second.termination);
    }
}

#[test]
fn lower_immediate_free_operation_families_preserve_failure_order() {
    let (package, approved) = admit_lower_program(&immediate_free_operation_lowerer());
    for (outcome, expected) in [
        (
            execute_immediate_free_operation(&package, &approved, Opcode::ConstantRef, &[0, 1], 2),
            sley_vm::LowerErrorCode::OpcodeUnsupported.numeric(),
        ),
        (
            execute_immediate_free_operation(&package, &approved, Opcode::VectorLen, &[0, 1], 2),
            sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
        ),
        (
            execute_immediate_free_operation(&package, &approved, Opcode::MapNew, &[0, 1, 2], 3),
            sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
        ),
        (
            execute_immediate_free_operation(&package, &approved, Opcode::FloatFma, &[0, 1, 3], 3),
            sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
        ),
        (
            execute_immediate_free_operation(&package, &approved, Opcode::TupleNew, &[], u32::MAX),
            sley_vm::LowerErrorCode::ResourceLimit.numeric(),
        ),
    ] {
        assert_inventory_error(&outcome, expected);
    }
}

#[test]
fn lower_bootstrap_immediates_match_native_dense_models() {
    let expected = native_bootstrap_immediate_instructions();
    assert_eq!(expected.len(), 8, "the native image covers every family");
    let (package, approved) = admit_lower_program(&bootstrap_immediate_lowerer());
    for instruction in expected {
        let opcode = Opcode::from_tag(instruction.opcode).expect("native opcode is frozen");
        let (tag, primary, secondary) = immediate_projection(&instruction.immediate);
        let next_register = instruction.results[0];
        let first = execute_immediate_operation(
            &package,
            &approved,
            opcode,
            &instruction.operands,
            tag,
            primary,
            secondary,
            &encoded_immediate(&instruction.immediate),
            next_register,
        );
        let second = execute_immediate_operation(
            &package,
            &approved,
            opcode,
            &instruction.operands,
            tag,
            primary,
            secondary,
            &encoded_immediate(&instruction.immediate),
            next_register,
        );
        assert_immediate_summary(&first, &instruction, next_register + 1);
        assert_eq!(first.termination, second.termination);
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn lower_bootstrap_immediates_preserve_failure_order() {
    let (package, approved) = admit_lower_program(&bootstrap_immediate_lowerer());
    let entity_tag = Immediate::Entity(id(0)).tag();
    let index_tag = Immediate::Index(0).tag();
    let variant_tag = Immediate::Variant(VariantImmediate {
        definition: id(0),
        member_id: MemberId::from_bytes([0; 32]),
    })
    .tag();
    let function_tag = Immediate::Function(FunctionRefValue {
        function: id(0),
        type_arguments: Vec::new(),
    })
    .tag();
    for (outcome, expected) in [
        (
            execute_immediate_operation(
                &package,
                &approved,
                Opcode::BoolAnd,
                &[0, 1],
                entity_tag,
                0,
                0,
                b"",
                2,
            ),
            sley_vm::LowerErrorCode::OpcodeUnsupported.numeric(),
        ),
        (
            execute_immediate_operation(
                &package,
                &approved,
                Opcode::ConstantRef,
                &[0],
                index_tag,
                0,
                0,
                b"",
                1,
            ),
            sley_vm::LowerErrorCode::ImmediateMismatch.numeric(),
        ),
        (
            execute_immediate_operation(
                &package,
                &approved,
                Opcode::TupleGet,
                &[],
                index_tag,
                0,
                0,
                b"",
                0,
            ),
            sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
        ),
        (
            execute_immediate_operation(
                &package,
                &approved,
                Opcode::VariantNew,
                &[0, 1],
                variant_tag,
                0,
                0,
                b"",
                2,
            ),
            sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
        ),
        (
            execute_immediate_operation(
                &package,
                &approved,
                Opcode::CallDirect,
                &[2],
                function_tag,
                0,
                0,
                b"",
                2,
            ),
            sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
        ),
        (
            execute_immediate_operation(
                &package,
                &approved,
                Opcode::TupleGet,
                &[0],
                index_tag,
                0,
                0,
                b"",
                u32::MAX,
            ),
            sley_vm::LowerErrorCode::ResourceLimit.numeric(),
        ),
    ] {
        assert_inventory_error(&outcome, expected);
    }
}

#[test]
fn lower_bootstrap_immediates_preserve_full_identity_bytes() {
    let mut left_bytes = [0x31; 32];
    let mut right_bytes = left_bytes;
    left_bytes[0] = 0xA1;
    right_bytes[0] = 0xB2;
    let left_immediate = Immediate::Entity(EntityId::from_bytes(left_bytes));
    let right_immediate = Immediate::Entity(EntityId::from_bytes(right_bytes));
    let (left_tag, left_primary, left_secondary) = immediate_projection(&left_immediate);
    let (right_tag, right_primary, right_secondary) = immediate_projection(&right_immediate);
    assert_eq!(left_tag, right_tag);
    assert_eq!(
        left_primary, right_primary,
        "compact suffixes deliberately collide"
    );
    assert_eq!(left_secondary, right_secondary);

    let (package, approved) = admit_lower_program(&bootstrap_immediate_lowerer());
    let execute = |immediate: &Immediate| {
        execute_immediate_operation(
            &package,
            &approved,
            Opcode::ConstantRef,
            &[],
            immediate.tag(),
            left_primary,
            0,
            &encoded_immediate(immediate),
            0,
        )
    };
    let left = execute(&left_immediate);
    let right = execute(&right_immediate);
    for (outcome, immediate) in [(&left, left_immediate), (&right, right_immediate)] {
        assert_immediate_summary(
            outcome,
            &sley_vm::Instruction {
                opcode: Opcode::ConstantRef.tag(),
                operands: Vec::new(),
                results: vec![0],
                immediate,
            },
            1,
        );
    }
    assert_ne!(left.termination, right.termination);
}

#[test]
fn lower_ordered_immediate_inventory_matches_native_dense_sequence() {
    let expected = native_bootstrap_immediate_instructions();
    let rows = expected
        .iter()
        .map(immediate_inventory_fact)
        .collect::<Vec<_>>();
    let first_register = expected[0].results[0];
    let expected_frontier = expected.last().expect("nonempty fixture").results[0] + 1;
    let (package, approved) = admit_lower_program(&ordered_immediate_inventory_lowerer());
    let first = execute_immediate_inventory(&package, &approved, &rows, first_register);
    let second = execute_immediate_inventory(&package, &approved, &rows, first_register);
    assert_immediate_inventory_summary(&first, &expected, expected_frontier);
    assert_eq!(first.termination, second.termination);

    let empty = execute_immediate_inventory(&package, &approved, &[], first_register);
    assert_immediate_inventory_summary(&empty, &[], first_register);
}

#[test]
fn lower_ordered_immediate_inventory_returns_first_late_failure() {
    let expected = native_bootstrap_immediate_instructions();
    let rows = expected
        .iter()
        .map(immediate_inventory_fact)
        .collect::<Vec<_>>();
    let first_register = expected[0].results[0];
    let (package, approved) = admit_lower_program(&ordered_immediate_inventory_lowerer());

    let mut wrong_immediate = rows.clone();
    wrong_immediate[5].immediate_tag = Immediate::Index(0).tag();
    assert_inventory_error(
        &execute_immediate_inventory(&package, &approved, &wrong_immediate, first_register),
        sley_vm::LowerErrorCode::ImmediateMismatch.numeric(),
    );

    let mut invalid_late_reference = rows;
    invalid_late_reference[6].operands[0] += 100;
    assert_inventory_error(
        &execute_immediate_inventory(&package, &approved, &invalid_late_reference, first_register),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );

    let constant = immediate_inventory_fact(&expected[0]);
    assert_inventory_error(
        &execute_immediate_inventory(&package, &approved, &[constant], u32::MAX),
        sley_vm::LowerErrorCode::ResourceLimit.numeric(),
    );
}

#[test]
fn lower_mixed_operation_inventory_composes_both_opcode_families() {
    let immediate = native_bootstrap_immediate_instructions();
    let constant = immediate[0].clone();
    let mut bool_not = native_single_scalar(Opcode::BoolNot);
    bool_not.operands = vec![5];
    bool_not.results = vec![6];
    let mut tuple = native_variadic_instruction(Opcode::TupleNew);
    tuple.operands = vec![0, 1];
    tuple.results = vec![7];
    let mut call = immediate[6].clone();
    call.operands = vec![6];
    call.results = vec![8];
    let mut adapter = immediate[7].clone();
    adapter.results = vec![9];
    let expected = vec![constant, bool_not, tuple, call, adapter];
    let rows = expected
        .iter()
        .map(immediate_inventory_fact)
        .collect::<Vec<_>>();
    let (package, approved) = admit_lower_program(&mixed_operation_inventory_lowerer());
    let first = execute_immediate_inventory(&package, &approved, &rows, 5);
    let second = execute_immediate_inventory(&package, &approved, &rows, 5);
    assert_immediate_inventory_summary(&first, &expected, 10);
    assert_eq!(first.termination, second.termination);
}

#[test]
fn lower_mixed_operation_inventory_preserves_cross_family_failure_order() {
    let immediate = native_bootstrap_immediate_instructions();
    let mut bool_not = native_single_scalar(Opcode::BoolNot);
    bool_not.operands = vec![5];
    bool_not.results = vec![6];
    let expected = [immediate[0].clone(), bool_not, immediate[6].clone()];
    let rows = expected
        .iter()
        .map(immediate_inventory_fact)
        .collect::<Vec<_>>();
    let (package, approved) = admit_lower_program(&mixed_operation_inventory_lowerer());

    let mut wrong_none_tag = rows.clone();
    wrong_none_tag[1].immediate_tag = Immediate::Entity(id(0)).tag();
    wrong_none_tag[1].operands.clear();
    assert_inventory_error(
        &execute_immediate_inventory(&package, &approved, &wrong_none_tag, 5),
        sley_vm::LowerErrorCode::ImmediateMismatch.numeric(),
    );

    let mut nonzero_none_payload = rows.clone();
    nonzero_none_payload[1].primary = 1;
    assert_inventory_error(
        &execute_immediate_inventory(&package, &approved, &nonzero_none_payload, 5),
        sley_vm::LowerErrorCode::ImmediateMismatch.numeric(),
    );

    let mut wrong_none_bytes = rows.clone();
    wrong_none_bytes[1].immediate_bytes = encoded_immediate(&Immediate::Entity(id(1)));
    assert_inventory_error(
        &execute_immediate_inventory(&package, &approved, &wrong_none_bytes, 5),
        sley_vm::LowerErrorCode::ImmediateMismatch.numeric(),
    );

    let mut invalid_late_reference = rows;
    invalid_late_reference[2].operands[0] = 8;
    assert_inventory_error(
        &execute_immediate_inventory(&package, &approved, &invalid_late_reference, 5),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );

    let unsupported = ImmediateInventoryFact {
        opcode: Opcode::ContractAssert.tag(),
        operands: vec![0],
        immediate_tag: Immediate::Entity(id(1)).tag(),
        primary: 1,
        secondary: 0,
        immediate_bytes: encoded_immediate(&Immediate::Entity(id(1))),
    };
    assert_inventory_error(
        &execute_immediate_inventory(&package, &approved, &[unsupported], 5),
        sley_vm::LowerErrorCode::OpcodeUnsupported.numeric(),
    );
}

#[test]
fn lower_simple_block_composes_operations_and_native_terminators() {
    let instruction = native_single_scalar(Opcode::BoolAnd);
    let rows = [immediate_inventory_fact(&instruction)];
    let (package, approved) = admit_lower_program(&simple_block_lowerer());
    for kind in [1, 2, 3, 5] {
        let (terminator, _register_count, block_count) = native_simple_terminator(kind);
        let first = execute_simple_block(
            &package,
            &approved,
            &rows,
            2,
            0,
            &[],
            &terminator,
            block_count,
            Reachability::Required.tag(),
        );
        let second = execute_simple_block(
            &package,
            &approved,
            &rows,
            2,
            0,
            &[],
            &terminator,
            block_count,
            Reachability::Required.tag(),
        );
        assert_simple_block_summary(
            &first,
            0,
            &[],
            std::slice::from_ref(&instruction),
            &terminator,
            Reachability::Required.tag(),
            3,
        );
        assert_eq!(first.termination, second.termination);
    }
}

#[test]
fn lower_simple_block_preserves_operation_before_terminator_failures() {
    let instruction = native_single_scalar(Opcode::BoolAnd);
    let rows = [immediate_inventory_fact(&instruction)];
    let (_, _, block_count) = native_simple_terminator(1);
    let (package, approved) = admit_lower_program(&simple_block_lowerer());

    let mut invalid_operation = rows.clone();
    invalid_operation[0].operands[1] = 2;
    assert_inventory_error(
        &execute_simple_block(
            &package,
            &approved,
            &invalid_operation,
            2,
            0,
            &[],
            &sley_vm::BytecodeTerminator::Return(3),
            block_count,
            Reachability::Required.tag(),
        ),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );

    assert_inventory_error(
        &execute_simple_block(
            &package,
            &approved,
            &rows,
            2,
            0,
            &[],
            &sley_vm::BytecodeTerminator::Return(3),
            block_count,
            Reachability::Required.tag(),
        ),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );

    let empty = execute_simple_block(
        &package,
        &approved,
        &[],
        2,
        0,
        &[0, 1],
        &sley_vm::BytecodeTerminator::Return(1),
        1,
        Reachability::ExplicitlyUnreachable.tag(),
    );
    assert_simple_block_summary(
        &empty,
        0,
        &[0, 1],
        &[],
        &sley_vm::BytecodeTerminator::Return(1),
        Reachability::ExplicitlyUnreachable.tag(),
        2,
    );
}

#[test]
fn lower_complete_block_normalizes_every_terminator_family() {
    let bool_instruction = native_single_scalar(Opcode::BoolAnd);
    let bool_rows = [immediate_inventory_fact(&bool_instruction)];
    let (package, approved) = admit_lower_program(&complete_block_lowerer());
    for kind in [1, 2, 3, 5] {
        let (terminator, _, block_count) = native_simple_terminator(kind);
        let outcome = execute_complete_block(
            &package,
            &approved,
            &bool_rows,
            2,
            0,
            &[],
            &terminator,
            block_count,
            Reachability::Required.tag(),
        );
        assert_complete_block_summary(
            &outcome,
            0,
            &[],
            std::slice::from_ref(&bool_instruction),
            &terminator,
            Reachability::Required.tag(),
            3,
        );
    }

    let (switch, _, block_count) = native_builtin_switch_terminator();
    let mut option_instruction = native_single_scalar(Opcode::OptionSome);
    option_instruction.operands = vec![0];
    option_instruction.results = vec![2];
    let switch_rows = [immediate_inventory_fact(&option_instruction)];
    let first = execute_complete_block(
        &package,
        &approved,
        &switch_rows,
        2,
        0,
        &[],
        &switch,
        block_count,
        Reachability::Required.tag(),
    );
    let second = execute_complete_block(
        &package,
        &approved,
        &switch_rows,
        2,
        0,
        &[],
        &switch,
        block_count,
        Reachability::Required.tag(),
    );
    assert_complete_block_summary(
        &first,
        0,
        &[],
        std::slice::from_ref(&option_instruction),
        &switch,
        Reachability::Required.tag(),
        3,
    );
    assert_eq!(first.termination, second.termination);
}

#[test]
fn lower_complete_block_preserves_operation_before_switch_failures() {
    let (mut switch, _, block_count) = native_builtin_switch_terminator();
    let mut option_instruction = native_single_scalar(Opcode::OptionSome);
    option_instruction.operands = vec![0];
    option_instruction.results = vec![2];
    let valid_rows = [immediate_inventory_fact(&option_instruction)];
    let (package, approved) = admit_lower_program(&complete_block_lowerer());

    let sley_vm::BytecodeTerminator::VariantSwitch { cases, .. } = &mut switch else {
        panic!("native fixture is a variant switch")
    };
    cases[1].edge.arguments[1] = sley_vm::BytecodeSwitchArgument::Value(3);

    let mut invalid_operation = valid_rows.clone();
    invalid_operation[0].operands[0] = 2;
    assert_inventory_error(
        &execute_complete_block(
            &package,
            &approved,
            &invalid_operation,
            2,
            0,
            &[],
            &switch,
            block_count,
            Reachability::Required.tag(),
        ),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );
    assert_inventory_error(
        &execute_complete_block(
            &package,
            &approved,
            &valid_rows,
            2,
            0,
            &[],
            &switch,
            block_count,
            Reachability::Required.tag(),
        ),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );
}

#[test]
fn complete_block_map_byte_encoder_walks_aligned_function_blocks() {
    use sley_ssmc::ResultConst;

    let native = native_complete_function();
    let scaffold = complete_block_map_byte_encoder();
    let (package, approved) = admit_lower_program(&scaffold);
    let prefix = vec![0xA0, 0xA1, 0xA2];
    let outcome = execute_complete_block_map_byte_encoder(&package, &approved, &native, &prefix);
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "complete-block-map encoder must terminate with a value, got {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
        panic!(
            "complete-block-map encoder must return Ok, got {:?}",
            value.data
        )
    };
    let ConstData::Sequence(found) = &encoded.data else {
        panic!("complete-block-map encoder result must be an octet vector")
    };
    let found = found
        .iter()
        .map(|octet| match octet.data {
            ConstData::UInt(value) => u8::try_from(value).expect("encoded octet fits u8"),
            ref other => panic!("encoded octet must be UInt8, got {other:?}"),
        })
        .collect::<Vec<_>>();
    let mut expected = prefix;
    for block in &native.blocks {
        expected.extend_from_slice(&encoded_block(block));
    }
    assert_eq!(found, expected);
}

#[test]
fn complete_block_byte_encoder_matches_native_records() {
    use sley_ssmc::ResultConst;

    let native = native_complete_function();
    let scaffold = complete_block_byte_encoder();
    let (package, approved) = admit_lower_program(&scaffold);
    for (index, block) in native.blocks.iter().enumerate() {
        let prefix = vec![0xB0, u8::try_from(index).expect("fixture index fits u8")];
        let outcome = execute_complete_block_byte_encoder(&package, &approved, block, &prefix);
        let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
            panic!(
                "complete-block encoder must terminate with a value, got {:?}",
                outcome.termination
            )
        };
        let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
            panic!(
                "complete-block encoder must return Ok, got {:?}",
                value.data
            )
        };
        let ConstData::Sequence(found) = &encoded.data else {
            panic!("complete-block encoder result must be an octet vector")
        };
        let found = found
            .iter()
            .map(|octet| match octet.data {
                ConstData::UInt(value) => u8::try_from(value).expect("encoded octet fits u8"),
                ref other => panic!("encoded octet must be UInt8, got {other:?}"),
            })
            .collect::<Vec<_>>();
        let mut expected = prefix;
        expected.extend_from_slice(&encoded_block(block));
        assert_eq!(found, expected);
    }
}

#[test]
fn complete_terminator_byte_encoder_dispatches_every_family() {
    use sley_ssmc::ResultConst;

    let mut terminators = [1, 2, 3, 5]
        .into_iter()
        .map(|kind| native_simple_terminator(kind).0)
        .collect::<Vec<_>>();
    terminators.push(native_builtin_switch_terminator().0);
    let scaffold = complete_terminator_byte_encoder();
    let (package, approved) = admit_lower_program(&scaffold);
    for (index, terminator) in terminators.iter().enumerate() {
        let prefix = vec![0xC0, u8::try_from(index).expect("fixture index fits u8")];
        let outcome =
            execute_complete_terminator_byte_encoder(&package, &approved, terminator, &prefix);
        let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
            panic!(
                "complete-terminator encoder must terminate with a value, got {:?}",
                outcome.termination
            )
        };
        let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
            panic!(
                "complete-terminator encoder must return Ok, got {:?}",
                value.data
            )
        };
        let ConstData::Sequence(found) = &encoded.data else {
            panic!("complete-terminator encoder result must be an octet vector")
        };
        let found = found
            .iter()
            .map(|octet| match octet.data {
                ConstData::UInt(value) => u8::try_from(value).expect("encoded octet fits u8"),
                ref other => panic!("encoded octet must be UInt8, got {other:?}"),
            })
            .collect::<Vec<_>>();
        let mut expected = prefix;
        expected.extend_from_slice(&encoded_terminator(terminator));
        assert_eq!(found, expected);
    }
}

#[test]
fn builtin_switch_terminator_byte_encoder_matches_native_layout() {
    use sley_ssmc::ResultConst;

    let (terminator, _, _) = native_builtin_switch_terminator();
    let scaffold = builtin_switch_terminator_byte_encoder();
    let (package, approved) = admit_lower_program(&scaffold);
    let prefix = vec![0xE1, 0xE2, 0xE3];
    let outcome =
        execute_builtin_switch_terminator_byte_encoder(&package, &approved, &terminator, &prefix);
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "switch-terminator encoder must terminate with a value, got {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
        panic!(
            "switch-terminator encoder must return Ok, got {:?}",
            value.data
        )
    };
    let ConstData::Sequence(found) = &encoded.data else {
        panic!("switch-terminator encoder result must be an octet vector")
    };
    let found = found
        .iter()
        .map(|octet| match octet.data {
            ConstData::UInt(value) => u8::try_from(value).expect("encoded octet fits u8"),
            ref other => panic!("encoded octet must be UInt8, got {other:?}"),
        })
        .collect::<Vec<_>>();
    let mut expected = prefix;
    expected.extend_from_slice(&encoded_terminator(&terminator));
    assert_eq!(found, expected);
}

#[test]
fn simple_terminator_byte_encoder_matches_native_layouts() {
    use sley_ssmc::ResultConst;

    let scaffold = simple_terminator_byte_encoder();
    let (package, approved) = admit_lower_program(&scaffold);
    for kind in [1, 2, 3, 5] {
        let (terminator, _, _) = native_simple_terminator(kind);
        let prefix = vec![0xD0, u8::try_from(kind).expect("terminator kind fits u8")];
        let outcome =
            execute_simple_terminator_byte_encoder(&package, &approved, &terminator, &prefix);
        let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
            panic!(
                "simple-terminator encoder must terminate with a value, got {:?}",
                outcome.termination
            )
        };
        let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
            panic!(
                "simple-terminator encoder must return Ok, got {:?}",
                value.data
            )
        };
        let ConstData::Sequence(found) = &encoded.data else {
            panic!("simple-terminator encoder result must be an octet vector")
        };
        let found = found
            .iter()
            .map(|octet| match octet.data {
                ConstData::UInt(value) => u8::try_from(value).expect("encoded octet fits u8"),
                ref other => panic!("encoded octet must be UInt8, got {other:?}"),
            })
            .collect::<Vec<_>>();
        let mut expected = prefix;
        expected.extend_from_slice(&encoded_terminator(&terminator));
        assert_eq!(found, expected);
    }
}

#[test]
fn instruction_map_byte_encoder_walks_every_ordinal_key() {
    use sley_ssmc::ResultConst;

    let instructions = native_bootstrap_immediate_instructions();
    let scaffold = instruction_map_byte_encoder();
    let (package, approved) = admit_lower_program(&scaffold);
    for (selected, prefix) in [
        (&instructions[..0], Vec::new()),
        (&instructions[..1], vec![0x11]),
        (instructions.as_slice(), vec![0x22, 0x33]),
    ] {
        let outcome = execute_instruction_map_byte_encoder(&package, &approved, selected, &prefix);
        let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
            panic!(
                "instruction-map encoder for {} rows must terminate with a value, got {:?}",
                selected.len(),
                outcome.termination
            )
        };
        let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
            panic!(
                "instruction-map encoder must return Ok, got {:?}",
                value.data
            )
        };
        let ConstData::Sequence(found) = &encoded.data else {
            panic!("instruction-map encoder result must be an octet vector")
        };
        let found = found
            .iter()
            .map(|octet| match octet.data {
                ConstData::UInt(value) => u8::try_from(value).expect("encoded octet fits u8"),
                ref other => panic!("encoded octet must be UInt8, got {other:?}"),
            })
            .collect::<Vec<_>>();
        let mut expected = prefix;
        expected.extend_from_slice(
            &u64::try_from(selected.len())
                .expect("instruction count fits u64")
                .to_be_bytes(),
        );
        for instruction in selected {
            expected.extend_from_slice(&encoded_instruction(instruction));
        }
        assert_eq!(found, expected);
    }
}

#[test]
fn instruction_byte_encoder_matches_native_extended_layout() {
    use sley_ssmc::ResultConst;

    let scaffold = instruction_byte_encoder();
    let (package, approved) = admit_lower_program(&scaffold);
    for (index, instruction) in native_bootstrap_immediate_instructions().iter().enumerate() {
        let prefix = vec![0xA5, u8::try_from(index).expect("fixture index fits u8")];
        let outcome = execute_instruction_byte_encoder(&package, &approved, instruction, &prefix);
        let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
            panic!("instruction byte encoder must terminate with a value")
        };
        let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
            panic!(
                "instruction byte encoder must return Ok, got {:?}",
                value.data
            )
        };
        let ConstData::Sequence(found) = &encoded.data else {
            panic!("instruction encoder result must be an octet vector")
        };
        let found = found
            .iter()
            .map(|octet| match octet.data {
                ConstData::UInt(value) => u8::try_from(value).expect("encoded octet fits u8"),
                ref other => panic!("encoded octet must be UInt8, got {other:?}"),
            })
            .collect::<Vec<_>>();
        let mut expected = prefix;
        expected.extend_from_slice(&encoded_instruction(instruction));
        assert_eq!(found, expected);
    }
}

#[test]
fn byte_chunk_encoder_preserves_exact_order_and_boundaries() {
    use sley_ssmc::ResultConst;

    let scaffold = byte_chunk_encoder();
    let (package, approved) = admit_lower_program(&scaffold);
    for (prefix, chunk) in [
        (Vec::new(), Vec::new()),
        (b"SLEYBC02".to_vec(), Vec::new()),
        (Vec::new(), vec![0, 1, 127, 128, 254, 255]),
        (vec![255, 0, 17], (0_u8..=u8::MAX).collect::<Vec<_>>()),
    ] {
        let outcome = execute_byte_chunk_encoder(&package, &approved, &prefix, &chunk);
        let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
            panic!("byte-chunk encoder must terminate with a value")
        };
        let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
            panic!("byte-chunk encoder must return Ok, got {:?}", value.data)
        };
        let mut expected = prefix;
        expected.extend_from_slice(&chunk);
        assert_eq!(encoded.data, ConstData::Bytes(expected));
    }
}

#[test]
fn fixed_width_byte_encoder_matches_native_big_endian_bytes() {
    use sley_ssmc::ResultConst;

    let scaffold = fixed_width_byte_encoder();
    let adapter_ids = scaffold
        .adapters
        .iter()
        .map(|adapter| adapter.adapter_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(adapter_ids.len(), 3);
    for bridge in [
        sley_vm::host_abi::BRIDGE_CODE_B2V1,
        sley_vm::host_abi::BRIDGE_CODE_PSH1,
        sley_vm::host_abi::BRIDGE_CODE_V2B1,
    ] {
        assert!(adapter_ids.contains(&sley_vm::host_abi::bridge_identity(bridge)));
    }
    let (package, approved) = admit_lower_program(&scaffold);
    for (prefix, value_u32, value_u64) in [
        (Vec::new(), 0, 0),
        (b"SLEYBC02".to_vec(), 1, 3),
        (vec![0, 255, 17], 0x0102_03fe, 0x0102_0304_0506_07fe),
        (vec![42], u32::MAX, u64::MAX),
    ] {
        let outcome =
            execute_fixed_width_encoder(&package, &approved, &prefix, value_u32, value_u64);
        let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
            panic!("fixed-width encoder must terminate with a value")
        };
        let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
            panic!("fixed-width encoder must return Ok, got {:?}", value.data)
        };
        let mut expected = prefix;
        expected.extend_from_slice(&value_u32.to_be_bytes());
        expected.extend_from_slice(&value_u64.to_be_bytes());
        assert_eq!(encoded.data, ConstData::Bytes(expected));
    }
}

#[test]
fn complete_function_image_encoder_matches_native_transitive_callee_table() {
    use sley_ssmc::ResultConst;

    let native = native_direct_call_lowered();
    assert_eq!(
        native.callees.len(),
        2,
        "fixture has one direct and one transitive callee"
    );
    assert!(
        native
            .callees
            .windows(2)
            .all(|pair| pair[0].function < pair[1].function),
        "native callee inventory is in canonical identity order"
    );
    assert_eq!(
        native
            .callees
            .iter()
            .map(|callee| callee.function)
            .collect::<Vec<_>>(),
        vec![id(10), id(20)],
        "native output sorts the transitively discovered leaf before its caller"
    );
    let root = complete_function_fact(&native.bytecode);
    let callees = native
        .callees
        .iter()
        .map(complete_function_fact)
        .collect::<Vec<_>>();
    let scaffold = complete_function_image_encoder();
    let (package, approved) = admit_lower_program(&scaffold);
    let outcome = execute_complete_image(&package, &approved, &root, &callees);
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "complete image encoder must terminate with a value, got {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
        panic!(
            "complete image encoder must return Ok, got {:?}",
            value.data
        )
    };
    assert_eq!(encoded.data, ConstData::Bytes(native.bytes));
}

#[test]
fn package_section_encoders_match_nonempty_native_inventories_and_dependency() {
    let expected = package_inventory_fixture(Vec::new(), id(0x31));

    let inventory_scaffold = package_inventory_section_encoder();
    let (inventory_package, inventory_approved) = admit_lower_program(&inventory_scaffold);
    let constants = successful_bytes_result(
        &execute_package_inventory_section(
            &inventory_package,
            &inventory_approved,
            &canonical_constant_rows(&expected.constants),
            false,
        ),
        "constant-section encoder",
    );
    let layouts = successful_bytes_result(
        &execute_package_inventory_section(
            &inventory_package,
            &inventory_approved,
            &canonical_layout_rows(&expected.type_definitions),
            true,
        ),
        "layout-section encoder",
    );
    let imports = successful_bytes_result(
        &execute_package_inventory_section(
            &inventory_package,
            &inventory_approved,
            &canonical_import_rows(&expected.imports),
            true,
        ),
        "import-section encoder",
    );
    assert_eq!(
        constants,
        sley_vm::exec_package::encode_constants_section(&expected.constants).unwrap()
    );
    assert_eq!(
        layouts,
        sley_vm::exec_package::encode_layouts_section(&expected.type_definitions).unwrap()
    );
    assert_eq!(
        imports,
        sley_vm::exec_package::encode_imports_section(&expected.imports).unwrap()
    );

    let (global_rows, contract_rows) = canonical_dependency_rows(&expected);
    let dependency_scaffold = package_dependency_sections_encoder();
    let (dependency_package, dependency_approved) = admit_lower_program(&dependency_scaffold);
    let dependency_outcome = execute_package_dependency_sections(
        &dependency_package,
        &dependency_approved,
        &expected,
        &global_rows,
        &contract_rows,
    );
    let encoded = successful_package_sections(&dependency_outcome);
    assert_eq!(
        encoded[3],
        sley_vm::exec_package::encode_dependency_section(&expected).unwrap()
    );
    let decoded = sley_vm::decode_dependency_section(&encoded[3]).unwrap();
    assert_eq!(decoded.entry, expected.entry);
    assert_eq!(
        decoded.gate_closure_fingerprints,
        expected.gate_closure_fingerprints
    );
    assert_eq!(decoded.admitted_limits, expected.admitted_limits);
    assert_eq!(decoded.globals, expected.globals);
    assert_eq!(decoded.contracts, expected.contracts);
}

#[test]
fn package_envelope_composer_matches_native_bytes_and_hydrates() {
    let native = native_complete_lowered();
    let expected = package_inventory_fixture(native.bytes, native.bytecode.function);

    let inventory_scaffold = package_inventory_section_encoder();
    let (inventory_package, inventory_approved) = admit_lower_program(&inventory_scaffold);
    let constants = successful_bytes_result(
        &execute_package_inventory_section(
            &inventory_package,
            &inventory_approved,
            &canonical_constant_rows(&expected.constants),
            false,
        ),
        "constant-section encoder",
    );
    let layouts = successful_bytes_result(
        &execute_package_inventory_section(
            &inventory_package,
            &inventory_approved,
            &canonical_layout_rows(&expected.type_definitions),
            true,
        ),
        "layout-section encoder",
    );
    let imports = successful_bytes_result(
        &execute_package_inventory_section(
            &inventory_package,
            &inventory_approved,
            &canonical_import_rows(&expected.imports),
            true,
        ),
        "import-section encoder",
    );
    let (global_rows, contract_rows) = canonical_dependency_rows(&expected);
    let dependency_scaffold = package_dependency_sections_encoder();
    let (dependency_package, dependency_approved) = admit_lower_program(&dependency_scaffold);
    let dependency_outcome = execute_package_dependency_sections(
        &dependency_package,
        &dependency_approved,
        &expected,
        &global_rows,
        &contract_rows,
    );
    let dependency = successful_package_sections(&dependency_outcome)[3].clone();
    let sections = [constants, layouts, imports, dependency];
    let digests = sley_vm::package_digests_v2(&expected).unwrap();

    let composer_scaffold = package_envelope_composer();
    let (composer_package, composer_approved) = admit_lower_program(&composer_scaffold);
    let outcome = execute_package_envelope_composer(
        &composer_package,
        &composer_approved,
        &expected,
        &sections,
        &digests,
    );
    let envelope = successful_bytes_result(&outcome, "package-envelope composer");
    assert_eq!(
        envelope,
        sley_vm::encode_package_envelope_v2(&expected).unwrap()
    );
    let hydrated = sley_vm::hydrate_package_envelope_v2(&envelope).unwrap();
    assert_eq!(hydrated.package, expected);
    assert_eq!(hydrated.digests, digests);
}

#[test]
fn package_builder_emits_complete_populated_envelope_in_one_invocation() {
    let native = native_complete_lowered();
    let expected = package_inventory_fixture(native.bytes, native.bytecode.function);
    let digests = sley_vm::package_digests_v2(&expected).unwrap();
    let scaffold = package_builder();
    let (package, approved) = admit_lower_program(&scaffold);
    let outcome = execute_package_builder(&package, &approved, &expected, &digests);
    let envelope = successful_bytes_result(&outcome, "composed package builder");
    assert_eq!(
        envelope,
        sley_vm::encode_package_envelope_v2(&expected).unwrap()
    );
    let hydrated = sley_vm::hydrate_package_envelope_v2(&envelope).unwrap();
    assert_eq!(hydrated.package, expected);
    assert_eq!(hydrated.digests, digests);

    let mut changed = hydrated.package;
    changed.constants[1].value = bool_value(true);
    let changed_digests = sley_vm::package_digests_v2(&changed).unwrap();
    let changed_outcome = execute_package_builder(&package, &approved, &changed, &changed_digests);
    let changed_envelope = successful_bytes_result(&changed_outcome, "mutated package builder");
    assert_ne!(changed_envelope, envelope);
    assert_eq!(
        changed_envelope,
        sley_vm::encode_package_envelope_v2(&changed).unwrap()
    );
    assert_eq!(
        sley_vm::hydrate_package_envelope_v2(&changed_envelope)
            .unwrap()
            .package,
        changed
    );
}

#[test]
fn root_function_image_encoder_matches_native_sleybc02_image() {
    use sley_ssmc::ResultConst;

    let native = native_complete_lowered();
    assert!(
        native.callees.is_empty(),
        "bounded root-image slice covers the empty callee table"
    );
    let function = &native.bytecode;
    let facts = complete_block_facts(function);
    let register_types = function
        .register_types
        .iter()
        .map(encoded_type)
        .collect::<Vec<_>>();
    let result_type = encoded_type(&function.result_type);
    let block_count = u32::try_from(function.blocks.len()).expect("fixture block count fits u32");
    let scaffold = root_function_image_encoder();
    let (package, approved) = admit_lower_program(&scaffold);
    let outcome = execute_complete_function(
        &package,
        &approved,
        function.function.as_bytes(),
        &function.parameter_registers,
        &register_types,
        &result_type,
        function.entry_block,
        block_count,
        &facts,
    );
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "root image encoder must terminate with a value, got {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
        panic!("root image encoder must return Ok, got {:?}", value.data)
    };
    assert_eq!(encoded.data, ConstData::Bytes(native.bytes));
}

#[test]
fn complete_function_body_encoder_matches_native_sleybc02_body() {
    use sley_ssmc::ResultConst;

    let native = native_complete_function();
    let facts = complete_block_facts(&native);
    let register_types = native
        .register_types
        .iter()
        .map(encoded_type)
        .collect::<Vec<_>>();
    let result_type = encoded_type(&native.result_type);
    let block_count = u32::try_from(native.blocks.len()).expect("fixture block count fits u32");
    let scaffold = complete_function_body_encoder();
    let (package, approved) = admit_lower_program(&scaffold);
    let outcome = execute_complete_function(
        &package,
        &approved,
        native.function.as_bytes(),
        &native.parameter_registers,
        &register_types,
        &result_type,
        native.entry_block,
        block_count,
        &facts,
    );
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!(
            "function body encoder must terminate with a value, got {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
        panic!("function body encoder must return Ok, got {:?}", value.data)
    };
    assert_eq!(
        encoded.data,
        ConstData::Bytes(encoded_function_body(&native))
    );
}

#[test]
fn complete_function_header_encoder_matches_native_sleybc02_prefix() {
    use sley_ssmc::ResultConst;

    let native = native_complete_function();
    let facts = complete_block_facts(&native);
    let register_types = native
        .register_types
        .iter()
        .map(encoded_type)
        .collect::<Vec<_>>();
    let result_type = encoded_type(&native.result_type);
    let block_count = u32::try_from(native.blocks.len()).expect("fixture block count fits u32");
    let scaffold = complete_function_header_encoder();
    let adapter_ids = scaffold
        .adapters
        .iter()
        .map(|adapter| adapter.adapter_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(adapter_ids.len(), 3);
    let (package, approved) = admit_lower_program(&scaffold);
    let outcome = execute_complete_function(
        &package,
        &approved,
        native.function.as_bytes(),
        &native.parameter_registers,
        &register_types,
        &result_type,
        native.entry_block,
        block_count,
        &facts,
    );
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("function header encoder must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(encoded)) = &value.data else {
        panic!(
            "function header encoder must return Ok, got {:?}",
            value.data
        )
    };
    assert_eq!(
        encoded.data,
        ConstData::Bytes(encoded_function_header(&native))
    );
    assert_inventory_error(
        &execute_complete_function(
            &package,
            &approved,
            &native.function.as_bytes()[..31],
            &native.parameter_registers,
            &register_types,
            &result_type,
            native.entry_block,
            block_count,
            &facts,
        ),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );
}

#[test]
fn lower_complete_function_matches_native_block_and_register_order() {
    let native = native_complete_function();
    let facts = complete_block_facts(&native);
    let register_types = native
        .register_types
        .iter()
        .map(encoded_type)
        .collect::<Vec<_>>();
    let result_type = encoded_type(&native.result_type);
    let block_count = u32::try_from(native.blocks.len()).expect("fixture block count fits u32");
    let scaffold = complete_function_lowerer();
    assert!(scaffold.adapters.iter().all(|adapter| {
        adapter.adapter_id
            != sley_vm::host_abi::bridge_identity(sley_vm::host_abi::BRIDGE_CODE_PSH1)
    }));
    let (package, approved) = admit_lower_program(&scaffold);
    let first = execute_complete_function(
        &package,
        &approved,
        native.function.as_bytes(),
        &native.parameter_registers,
        &register_types,
        &result_type,
        native.entry_block,
        block_count,
        &facts,
    );
    let second = execute_complete_function(
        &package,
        &approved,
        native.function.as_bytes(),
        &native.parameter_registers,
        &register_types,
        &result_type,
        native.entry_block,
        block_count,
        &facts,
    );
    assert_complete_function_model(&first, &native);
    assert_eq!(first.termination, second.termination);
}

#[test]
fn lower_complete_function_rejects_first_invalid_dense_fact() {
    let native = native_complete_function();
    let facts = complete_block_facts(&native);
    let register_types = native
        .register_types
        .iter()
        .map(encoded_type)
        .collect::<Vec<_>>();
    let result_type = encoded_type(&native.result_type);
    let block_count = u32::try_from(native.blocks.len()).expect("fixture block count fits u32");
    let (package, approved) = admit_lower_program(&complete_function_lowerer());

    let mut wrong_slot = facts.clone();
    wrong_slot[1].slot = 2;
    assert_inventory_error(
        &execute_complete_function(
            &package,
            &approved,
            native.function.as_bytes(),
            &native.parameter_registers,
            &register_types,
            &result_type,
            native.entry_block,
            block_count,
            &wrong_slot,
        ),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );

    let mut wrong_parameters = facts.clone();
    wrong_parameters[2].parameter_registers[1] = 7;
    assert_inventory_error(
        &execute_complete_function(
            &package,
            &approved,
            native.function.as_bytes(),
            &native.parameter_registers,
            &register_types,
            &result_type,
            native.entry_block,
            block_count,
            &wrong_parameters,
        ),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );

    let mut earlier_operation = wrong_parameters;
    earlier_operation[0].instructions[0].immediate_tag = Immediate::Entity(id(0)).tag();
    assert_inventory_error(
        &execute_complete_function(
            &package,
            &approved,
            native.function.as_bytes(),
            &native.parameter_registers,
            &register_types,
            &result_type,
            native.entry_block,
            block_count,
            &earlier_operation,
        ),
        sley_vm::LowerErrorCode::ImmediateMismatch.numeric(),
    );

    assert_inventory_error(
        &execute_complete_function(
            &package,
            &approved,
            native.function.as_bytes(),
            &native.parameter_registers,
            &register_types,
            &result_type,
            native.entry_block,
            block_count + 1,
            &facts,
        ),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );
    assert_inventory_error(
        &execute_complete_function(
            &package,
            &approved,
            native.function.as_bytes(),
            &native.parameter_registers,
            &register_types,
            &result_type,
            block_count,
            block_count,
            &facts,
        ),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );
}

#[test]
fn lower_complete_function_binds_identity_and_register_type_cardinality() {
    let native = native_complete_function();
    let facts = complete_block_facts(&native);
    let register_types = native
        .register_types
        .iter()
        .map(encoded_type)
        .collect::<Vec<_>>();
    let result_type = encoded_type(&native.result_type);
    let block_count = u32::try_from(native.blocks.len()).expect("fixture block count fits u32");
    let (package, approved) = admit_lower_program(&complete_function_lowerer());

    assert_inventory_error(
        &execute_complete_function(
            &package,
            &approved,
            &native.function.as_bytes()[..31],
            &native.parameter_registers,
            &register_types,
            &result_type,
            native.entry_block,
            block_count,
            &facts,
        ),
        sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
    );

    for wrong_types in [
        register_types[..register_types.len() - 1].to_vec(),
        register_types
            .iter()
            .cloned()
            .chain(std::iter::once(encoded_type(&TypeExpr::Unit)))
            .collect(),
    ] {
        assert_inventory_error(
            &execute_complete_function(
                &package,
                &approved,
                native.function.as_bytes(),
                &native.parameter_registers,
                &wrong_types,
                &result_type,
                native.entry_block,
                block_count,
                &facts,
            ),
            sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric(),
        );
    }
}

#[test]
fn lower_simple_terminators_match_native_models() {
    let (package, approved) = admit_lower_program(&simple_terminator_lowerer());
    for kind in [1, 2, 3, 5] {
        let (native, register_count, block_count) = native_simple_terminator(kind);
        let (primary, target_zero, arguments_zero, target_one, arguments_one, payload) =
            match &native {
                sley_vm::BytecodeTerminator::Return(value) => {
                    (*value, 0, Vec::new(), 0, Vec::new(), None)
                }
                sley_vm::BytecodeTerminator::Branch(edge) => {
                    (0, edge.target, edge.arguments.clone(), 0, Vec::new(), None)
                }
                sley_vm::BytecodeTerminator::CondBranch {
                    condition,
                    if_true,
                    if_false,
                } => (
                    *condition,
                    if_true.target,
                    if_true.arguments.clone(),
                    if_false.target,
                    if_false.arguments.clone(),
                    None,
                ),
                sley_vm::BytecodeTerminator::Trap { code, payload } => {
                    (*code, 0, Vec::new(), 0, Vec::new(), *payload)
                }
                sley_vm::BytecodeTerminator::VariantSwitch { .. } => unreachable!(),
            };
        let first = execute_simple_terminator(
            &package,
            &approved,
            kind,
            primary,
            target_zero,
            &arguments_zero,
            target_one,
            &arguments_one,
            payload,
            register_count,
            block_count,
        );
        let second = execute_simple_terminator(
            &package,
            &approved,
            kind,
            primary,
            target_zero,
            &arguments_zero,
            target_one,
            &arguments_one,
            payload,
            register_count,
            block_count,
        );
        assert_terminator_model(&first, &native);
        assert_eq!(first.termination, second.termination);
    }
}

#[test]
fn lower_simple_terminators_reject_invalid_dense_references() {
    let (package, approved) = admit_lower_program(&simple_terminator_lowerer());
    let local = sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric();
    let unsupported = sley_vm::LowerErrorCode::OpcodeUnsupported.numeric();
    for outcome in [
        execute_simple_terminator(&package, &approved, 1, 3, 0, &[], 0, &[], None, 3, 1),
        execute_simple_terminator(&package, &approved, 2, 0, 2, &[0], 0, &[], None, 3, 2),
        execute_simple_terminator(&package, &approved, 2, 0, 1, &[0, 3], 0, &[], None, 3, 2),
        execute_simple_terminator(&package, &approved, 3, 2, 1, &[0], 1, &[1, 3], None, 3, 2),
        execute_simple_terminator(&package, &approved, 5, 0, 0, &[], 0, &[], None, 3, 1),
        execute_simple_terminator(
            &package,
            &approved,
            5,
            TrapCode::InternalInvariant.tag(),
            0,
            &[],
            0,
            &[],
            Some(3),
            3,
            1,
        ),
    ] {
        assert_terminator_error(&outcome, local);
    }
    assert_terminator_error(
        &execute_simple_terminator(&package, &approved, 4, 0, 0, &[], 0, &[], None, 3, 1),
        unsupported,
    );
}

#[test]
fn lower_builtin_variant_switch_matches_native_case_inventory() {
    let (native, register_count, block_count) = native_builtin_switch_terminator();
    let (selector, cases) = builtin_switch_facts(&native);
    let (package, approved) = admit_lower_program(&builtin_variant_switch_lowerer());
    let first = execute_builtin_switch(
        &package,
        &approved,
        selector,
        &cases,
        register_count,
        block_count,
    );
    let second = execute_builtin_switch(
        &package,
        &approved,
        selector,
        &cases,
        register_count,
        block_count,
    );
    assert_builtin_switch_model(&first, selector, &cases);
    assert_eq!(first.termination, second.termination);
}

#[test]
fn lower_builtin_variant_switch_checks_every_case_and_argument() {
    let (native, register_count, block_count) = native_builtin_switch_terminator();
    let (selector, cases) = builtin_switch_facts(&native);
    let (package, approved) = admit_lower_program(&builtin_variant_switch_lowerer());
    let local = sley_vm::LowerErrorCode::LocalReferenceInvalid.numeric();
    let signature = sley_vm::LowerErrorCode::SignatureMismatch.numeric();

    assert_terminator_error(
        &execute_builtin_switch(
            &package,
            &approved,
            register_count,
            &cases,
            register_count,
            block_count,
        ),
        local,
    );

    let mut invalid_target = cases.clone();
    invalid_target[1].1 = block_count;
    assert_terminator_error(
        &execute_builtin_switch(
            &package,
            &approved,
            selector,
            &invalid_target,
            register_count,
            block_count,
        ),
        local,
    );

    let mut invalid_late_reference = cases.clone();
    invalid_late_reference[1].2[1] = (1, register_count);
    assert_terminator_error(
        &execute_builtin_switch(
            &package,
            &approved,
            selector,
            &invalid_late_reference,
            register_count,
            block_count,
        ),
        local,
    );

    for invalid_key in [0, 5] {
        let mut invalid = cases.clone();
        invalid[1].0 = invalid_key;
        assert_terminator_error(
            &execute_builtin_switch(
                &package,
                &approved,
                selector,
                &invalid,
                register_count,
                block_count,
            ),
            signature,
        );
    }

    let mut invalid_argument_tag = cases.clone();
    invalid_argument_tag[1].2[1].0 = 3;
    assert_terminator_error(
        &execute_builtin_switch(
            &package,
            &approved,
            selector,
            &invalid_argument_tag,
            register_count,
            block_count,
        ),
        signature,
    );
}

#[test]
fn lower_scaffold_lowering_legs_trap_per_index() {
    use sley_vm::ExecutionTermination;
    let (package, approved) = admitted_scaffold();
    // Every lowering leg traps Unreachable carrying its own leg index —
    // dispatch is observable per leg — and distinct witness bytes trap
    // identically, proving the scaffold lowers nothing.
    for leg in 1..=LEG_COUNT {
        for witness in [b"".as_slice(), b"\x00lower-witness\xff".as_slice()] {
            match execute_scaffold(&package, &approved, leg, witness) {
                ExecutionTermination::Trap { trap_tag, payload } => {
                    assert_eq!(
                        trap_tag,
                        sley_ssmc::TrapCode::Unreachable.tag(),
                        "leg {leg} traps Unreachable"
                    );
                    match payload {
                        Some(value) => assert_eq!(
                            value.data,
                            ConstData::UInt(leg),
                            "leg {leg} trap carries its index"
                        ),
                        None => panic!("leg {leg} trap must carry its index"),
                    }
                }
                other => panic!("leg {leg} must trap, got {other:?}"),
            }
        }
    }
}

#[test]
fn lower_scaffold_trivial_accept_and_unknown_marker() {
    use sley_vm::ExecutionTermination;
    let (package, approved) = admitted_scaffold();
    // Marker 0 is the admitted trivial closure: accepted with the empty
    // model marker regardless of witness bytes. Outside 0..=7 the
    // scaffold returns the typed PROFILE_UNSUPPORTED vocabulary value;
    // no leg traps.
    for witness in [b"".as_slice(), b"\x00lower-witness\xff".as_slice()] {
        match execute_scaffold(&package, &approved, 0, witness) {
            ExecutionTermination::Success(value) => assert_eq!(
                value.data,
                ConstData::UInt(LOWER_OK_EMPTY),
                "marker 0 is the trivial accept"
            ),
            other => panic!("marker 0 must accept, got {other:?}"),
        }
    }
    for marker in [8_u128, 100_u128, 4_294_967_295_u128] {
        match execute_scaffold(&package, &approved, marker, b"") {
            ExecutionTermination::Success(value) => assert_eq!(
                value.data,
                ConstData::UInt(PROFILE_UNSUPPORTED),
                "unknown marker {marker} returns PROFILE_UNSUPPORTED"
            ),
            other => {
                panic!("unknown marker {marker} must return PROFILE_UNSUPPORTED, got {other:?}")
            }
        }
    }
}

#[test]
fn lower_scaffold_fixture_identities_are_within_image_disjoint() {
    // Fixture hygiene: every identity that must be unique within this
    // assembled image is unique. Derived from the assembled fixture
    // itself, not from a hand-maintained list (`entry` is the same
    // graph object as `functions[0]`, so it is counted once).
    // Independent scaffold images admit separately, so this asserts
    // nothing about numeric reuse across images.
    let scaffold = lower_scaffold();
    let mut ids: Vec<EntityId> = Vec::new();
    ids.extend(scaffold.functions.iter().map(|f| f.entity_id));
    ids.extend(scaffold.parameters.iter().map(|p| p.entity_id));
    ids.extend(scaffold.blocks.iter().map(|b| b.entity_id));
    ids.extend(scaffold.operations.iter().map(|o| o.entity_id));
    ids.extend(scaffold.constants.iter().map(|c| c.entity_id));
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        ids.len(),
        sorted.len(),
        "fixture-local identities must be pairwise disjoint within this image"
    );
}

#[test]
fn lower_scaffold_vocabulary_matches_frozen_native() {
    // The scaffold's leg order and vocabulary constant are the frozen
    // S20-260 `LowerErrorCode` order and numerics — asserted here
    // against the native enum so drift in either source fails loudly.
    // Symbols: VM_LOWER_PROFILE_UNSUPPORTED through
    // VM_LOWER_RESOURCE_LIMIT (ERROR_CODES_V1.md: S20-260 freezes
    // 26000..=26006).
    let native = [
        sley_vm::LowerErrorCode::ProfileUnsupported,
        sley_vm::LowerErrorCode::OpcodeUnsupported,
        sley_vm::LowerErrorCode::SignatureMismatch,
        sley_vm::LowerErrorCode::ImmediateMismatch,
        sley_vm::LowerErrorCode::LocalReferenceInvalid,
        sley_vm::LowerErrorCode::CacheKeyUnsupported,
        sley_vm::LowerErrorCode::ResourceLimit,
    ];
    let symbols = [
        "VM_LOWER_PROFILE_UNSUPPORTED",
        "VM_LOWER_OPCODE_UNSUPPORTED",
        "VM_LOWER_SIGNATURE_MISMATCH",
        "VM_LOWER_IMMEDIATE_MISMATCH",
        "VM_LOWER_LOCAL_REFERENCE_INVALID",
        "VM_LOWER_CACHE_KEY_UNSUPPORTED",
        "VM_LOWER_RESOURCE_LIMIT",
    ];
    assert_eq!(
        native.len() as u128,
        LEG_COUNT,
        "seven frozen lowering legs"
    );
    for (index, (code, symbol)) in native.iter().zip(symbols.iter()).enumerate() {
        let leg = index as u128 + 1;
        // Seven legs; the expect documents the bound.
        let want = 26_000 + u32::try_from(index).expect("leg index fits u32");
        assert_eq!(code.numeric(), want, "leg {leg} numeric is frozen");
        assert_eq!(code.as_str(), *symbol, "leg {leg} symbol is frozen");
    }
    assert_eq!(
        PROFILE_UNSUPPORTED,
        u128::from(sley_vm::LowerErrorCode::ProfileUnsupported.numeric()),
        "scaffold vocabulary constant is the frozen leg-1 numeric"
    );
}
