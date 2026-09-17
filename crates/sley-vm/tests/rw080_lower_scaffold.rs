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
//! Construction provenance:
//! machineresearch/sley-2.0/reweave/rw-080-lower-scaffold.md and
//! machineresearch/sley-2.0/reweave/rw-080-lower-single-op.md.

use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    Block, CondBranchTerminator, ConstData, ConstValue, ConstantDefinition, FunctionGraph,
    Immediate, IntegerWidth, Opcode, Operation, OperationResultRef, Parameter, ParameterRole,
    Reachability, ReturnTerminator, TargetEdge, Terminator, TrapCode, TrapTerminator, TypeExpr,
    ValueRef, Visibility,
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

fn u32_value(n: u128) -> ConstValue {
    ConstValue {
        value_type: u32_type(),
        data: ConstData::UInt(n),
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
    }
}

/// Real bounded §1.3 algorithm: lower one checked Boolean operation to
/// `(opcode, operand_registers, result_registers)`. Function parameters own
/// dense registers from zero and the single result follows them. The input is
/// intentionally supplied after admission, so the image cannot contain the
/// answer being compared with the native reference lowerer.
#[allow(clippy::too_many_lines)]
fn single_bool_lowerer() -> LowerScaffold {
    const FUNCTION_ID: u8 = 1;
    const OPCODE_PARAM: u8 = 10;
    const PARAM_COUNT_PARAM: u8 = 11;
    const ENTRY: u8 = 20;
    const AND_CHECK: u8 = 21;
    const OR_CHECK: u8 = 22;
    const UNARY_COUNT: u8 = 23;
    const BINARY_COUNT: u8 = 24;
    const UNARY_EMIT: u8 = 25;
    const BINARY_EMIT: u8 = 26;
    const OPCODE_ERROR: u8 = 27;
    const SIGNATURE_ERROR: u8 = 28;
    const K_NOT: u8 = 60;
    const K_AND: u8 = 61;
    const K_OR: u8 = 62;
    const K_ONE: u8 = 63;
    const K_TWO: u8 = 64;
    const K_ZERO: u8 = 65;
    const K_OPCODE_ERROR: u8 = 66;
    const K_SIGNATURE_ERROR: u8 = 67;

    let function = id(FUNCTION_ID);
    let opcode_param = id(OPCODE_PARAM);
    let parameter_count_param = id(PARAM_COUNT_PARAM);
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
            entity_id: id(K_ZERO),
            value: u32_value(0),
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
    count_block(UNARY_COUNT, K_ONE, UNARY_EMIT);
    count_block(BINARY_COUNT, K_TWO, BINARY_EMIT);

    let mut emit_block = |block_id: u8, operand_constants: &[u8], result_constant: u8| {
        let block = id(block_id);
        let mut operation_ids = Vec::new();
        let mut operand_values = Vec::new();
        for constant in operand_constants {
            let operation = builder.const_ref(
                block,
                u32::try_from(operation_ids.len()).expect("small operation inventory"),
                id(*constant),
            );
            operation_ids.push(operation);
            operand_values.push(ValueRef::OperationResult(OperationResultRef {
                operation,
                result_index: 0,
            }));
        }
        let result_ref = builder.const_ref(
            block,
            u32::try_from(operation_ids.len()).expect("small operation inventory"),
            id(result_constant),
        );
        operation_ids.push(result_ref);
        let operands = builder.take_op();
        builder.operations.push(Operation {
            entity_id: operands,
            block,
            ordinal: u32::try_from(operation_ids.len()).expect("small operation inventory"),
            opcode: Opcode::VectorNew,
            operands: operand_values,
            result_types: vec![u32vec_type()],
            immediate: Immediate::None,
        });
        operation_ids.push(operands);
        let results = builder.take_op();
        builder.operations.push(Operation {
            entity_id: results,
            block,
            ordinal: u32::try_from(operation_ids.len()).expect("small operation inventory"),
            opcode: Opcode::VectorNew,
            operands: vec![ValueRef::OperationResult(OperationResultRef {
                operation: result_ref,
                result_index: 0,
            })],
            result_types: vec![u32vec_type()],
            immediate: Immediate::None,
        });
        operation_ids.push(results);
        let tuple = builder.take_op();
        builder.operations.push(Operation {
            entity_id: tuple,
            block,
            ordinal: u32::try_from(operation_ids.len()).expect("small operation inventory"),
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
            ordinal: u32::try_from(operation_ids.len()).expect("small operation inventory"),
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
    emit_block(UNARY_EMIT, &[K_ZERO], K_ONE);
    emit_block(BINARY_EMIT, &[K_ZERO, K_ONE], K_TWO);

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
    let (blocks, operations) = builder.finish();
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![opcode_param, parameter_count_param],
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
        functions: &[],
        contracts: &[],
        adapters: &[],
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
        adapters: &[],
        constants: &scaffold.constants,
        profile_version: BootstrapProfileVersion::V2,
    })
    .expect("scaffold admits under V2");
    let limits = generous_limits();
    let package = ExecutionPackage {
        image_bytes: lowered.bytes.clone(),
        constants: scaffold.constants.clone(),
        type_definitions: Vec::new(),
        imports: Vec::new(),
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
        adapters: &[],
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
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                u32_value(u128::from(opcode)),
                u32_value(u128::from(parameter_count)),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes single-operation lowerer")
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
        let first = execute_single_bool(&package, &approved, opcode.tag(), parameter_count);
        let second = execute_single_bool(&package, &approved, opcode.tag(), parameter_count);
        assert_single_lowered(&first, &reference);
        assert_single_lowered(&second, &reference);
        assert_eq!(
            first.termination, second.termination,
            "lowering is deterministic"
        );
    }
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
            &execute_single_bool(&package, &approved, opcode, count),
            sley_vm::LowerErrorCode::SignatureMismatch.numeric(),
        );
    }
    for opcode in [0, Opcode::Equal.tag(), u32::MAX] {
        assert_single_lower_error(
            &execute_single_bool(&package, &approved, opcode, 2),
            sley_vm::LowerErrorCode::OpcodeUnsupported.numeric(),
        );
    }
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
