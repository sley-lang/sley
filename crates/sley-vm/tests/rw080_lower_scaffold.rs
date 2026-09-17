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
//! machineresearch/sley-2.0/reweave/rw-080-lower-terminators.md.

use std::collections::{BTreeMap, BTreeSet};

use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, BranchTerminator, BuiltinCase, BuiltinFailureKind, CaseKey,
    CondBranchTerminator, ConstData, ConstValue, ConstantDefinition, FunctionGraph,
    FunctionRefValue, Immediate, IntegerWidth, MemberId, NamedType, Opcode, Operation,
    OperationResultRef, Parameter, ParameterRole, Reachability, RecordField, ReturnTerminator,
    SwitchArgument, SwitchCase, SwitchEdge, TargetEdge, Terminator, TrapCode, TrapTerminator,
    TypeDefForm, TypeDefinition, TypeExpr, ValueRef, VariantCase, VariantImmediate,
    VariantSwitchTerminator, Visibility,
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
        max_instructions: 10_000,
        max_fuel: 100_000,
        max_value_units: 1_000_000,
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
fn native_complete_function() -> sley_vm::BytecodeFunction {
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
    .bytecode
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
