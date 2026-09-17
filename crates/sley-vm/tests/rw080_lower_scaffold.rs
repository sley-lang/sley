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
//! model, and preserves frozen late-row failures.
//! The simple-terminator slice lowers return, branch, conditional branch, and
//! trap models and walks every edge-argument register in Sley.
//! Construction provenance:
//! machineresearch/sley-2.0/reweave/rw-080-lower-scaffold.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-single-op.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-inventory.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-terminators.md.

use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, BranchTerminator, BuiltinCase, BuiltinFailureKind, CaseKey,
    CondBranchTerminator, ConstData, ConstValue, ConstantDefinition, FunctionGraph,
    FunctionRefValue, Immediate, IntegerWidth, Opcode, Operation, OperationResultRef, Parameter,
    ParameterRole, Reachability, ReturnTerminator, SwitchArgument, SwitchCase, SwitchEdge,
    TargetEdge, Terminator, TrapCode, TrapTerminator, TypeExpr, ValueRef, VariantSwitchTerminator,
    Visibility,
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

fn u32_value(n: u128) -> ConstValue {
    ConstValue {
        value_type: u32_type(),
        data: ConstData::UInt(n),
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

fn optional_u32_value(value: Option<u32>) -> ConstValue {
    ConstValue {
        value_type: optional_u32_type(),
        data: ConstData::Option(value.map(|value| Box::new(u32_value(u128::from(value))))),
    }
}

fn bytes_value(bytes: &[u8]) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(bytes.to_vec()),
    }
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

/// Real bounded §1.3 inventory algorithm. The inventory is supplied at
/// execution time, walked by a CFG backedge, and validated row by row before
/// the dense register frontier advances.
#[allow(clippy::too_many_lines)]
fn ordered_bool_inventory_lowerer() -> LowerScaffold {
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
    let unary_count = assembler.block_id();
    let binary_count = assembler.block_id();
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
    let one_u32 = assembler.constant(u32_value(1));
    let two_u32 = assembler.constant(u32_value(2));
    let not_tag = assembler.constant(u32_value(u128::from(Opcode::BoolNot.tag())));
    let and_tag = assembler.constant(u32_value(u128::from(Opcode::BoolAnd.tag())));
    let or_tag = assembler.constant(u32_value(u128::from(Opcode::BoolOr.tag())));
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

    let emit_block = |assembler: &mut InventoryAssembler, block: EntityId, binary: bool| {
        let parameters = inventory_row_parameters(assembler, block);
        let mut operand_values = vec![ValueRef::Parameter(parameters.operand_zero)];
        if binary {
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
    emit_block(&mut assembler, emit_unary, false);
    emit_block(&mut assembler, emit_binary, true);

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
    let normalized = match expected {
        sley_vm::BytecodeTerminator::Return(value) => {
            (1, *value, 0, Vec::new(), 0, Vec::new(), None)
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
    };
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

fn native_single_bool(opcode: Opcode) -> sley_vm::Instruction {
    let function_id = id(1);
    let block_id = id(2);
    let operation_id = id(3);
    let parameter_count = if opcode == Opcode::BoolNot { 1 } else { 2 };
    let parameter_ids: Vec<EntityId> = (0..parameter_count)
        .map(|index| id(10 + u8::try_from(index).expect("small parameter index")))
        .collect();
    let function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: parameter_ids.clone(),
        result_type: TypeExpr::Bool,
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
            value_type: TypeExpr::Bool,
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
        result_types: vec![TypeExpr::Bool],
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
    .expect("native reference lowers the Boolean operation");
    lowered.bytecode.blocks[0].instructions[0].clone()
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
        let reference = native_single_bool(opcode);
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
fn lower_ordered_boolean_inventory_matches_native_model_and_frontier() {
    let (package, approved) = admit_lower_program(&ordered_bool_inventory_lowerer());
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
}

#[test]
fn lower_ordered_boolean_inventory_checks_every_row_in_order() {
    let (package, approved) = admit_lower_program(&ordered_bool_inventory_lowerer());
    let valid = (Opcode::BoolAnd.tag(), 2, 0, 1);
    assert_inventory_error(
        &execute_bool_inventory(
            &package,
            &approved,
            &[valid, (Opcode::Equal.tag(), 2, 0, 1)],
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
