//! RW-080 checker construction (§1.2): scaffold plus one-block CFG slice.
//!
//! PROVISIONAL C0 SEED SCAFFOLD — explicitly not accepted runtime
//! authority. Seed-authored (this file is the seed-assembler record)
//! under the operator development override; it exercises the v2
//! admit/approve/execute path for the second RW-080 toolchain module
//! without implementing any semantic judgment. Per contract §1.2,
//! a scaffold is the entry plus the error vocabulary plus one admitted
//! trivial program: marker 0 returns the trivial-accept value (the one
//! value-returning success exit), markers 1..=3 trap
//! (`TrapCode::Unreachable` with the judgment-phase index as payload,
//! proving dispatch reached the leg), and any other marker returns a
//! typed `SemanticError`-family value. The witness bytes are unread by
//! design (proven: distinct witness bytes behave identically), so the
//! scaffold performs no type, CFG, or effect judgment. The later
//! `single_bool_cfg_checker` is the first real bounded algorithm. Runtime
//! inputs describe a two-block, one-operation Boolean CFG projection; Sley
//! checks inventory, entry, ordinal, reachability, operand resolution,
//! result index, and return type in native first-failure order. It returns
//! either the native CFG code or a compact deterministic report. Construction
//! provenance: machineresearch/sley-2.0/reweave/rw-080-checker-scaffold.md.

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

fn u8_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(8))
}

fn u8_value(n: u128) -> ConstValue {
    ConstValue {
        value_type: u8_type(),
        data: ConstData::UInt(n),
    }
}

fn u32_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(32))
}

fn u64_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(64))
}

fn u32_value(n: u128) -> ConstValue {
    ConstValue {
        value_type: u32_type(),
        data: ConstData::UInt(n),
    }
}

fn check_plan_type() -> TypeExpr {
    TypeExpr::Tuple(vec![u32_type(), u32_type(), u64_type()])
}

fn check_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(check_plan_type()),
        error: Box::new(u32_type()),
    }
}

fn bytes_value(bytes: &[u8]) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(bytes.to_vec()),
    }
}

// Scaffold vocabulary (§1.2), documented in the manifest.
// ACCEPT_EMPTY_PLAN=0 is scaffold-local: the trivial program is accepted
// with an empty test plan. It is NOT a frozen code; the real TestPlan
// arrives with the RW-100 corpus. TYPE=1/CFG=2/EFFECT=3 name the owning
// judgment families (frozen S20-210 21xxx / S20-220 22xxx / S20-230 23xxx
// codes arrive with the real checker); TYPE doubles as the single
// returnable error code, the same role VERSION=6 plays in the §1.1 codec
// scaffold. Leg indexes double as trap payloads, so reaching a leg is
// observable: leg k traps carrying k.
const ACCEPT_EMPTY_PLAN: u128 = 0;
const TYPE_ERROR: u128 = 1;

// Fixture-namespace entity identities (recorded in the manifest;
// collision-free within this closure by construction; disjoint from the
// §1.1 codec scaffold ranges by choice, although each image admits
// independently).
const FUNCTION: u8 = 201;
const MARKER_PARAM: u8 = 212;
const WITNESS_PARAM: u8 = 213;
const ENTRY_BLOCK: u8 = 240;
const LEG_BLOCK_1: u8 = 241;
const LEG_BLOCK_2: u8 = 242;
const LEG_BLOCK_3: u8 = 243;
const ACCEPT_BLOCK: u8 = 244;
const UNKNOWN_BLOCK: u8 = 245;
const CHAIN_BLOCK_1: u8 = 246;
const CHAIN_BLOCK_2: u8 = 247;
const CHAIN_BLOCK_3: u8 = 248;

struct CheckerScaffold {
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
            next_op: 120,
        }
    }

    fn take_op(&mut self) -> EntityId {
        let op = id(self.next_op);
        self.next_op += 1;
        op
    }

    fn const_ref_as(
        &mut self,
        block: EntityId,
        ordinal: u32,
        target: EntityId,
        result_type: TypeExpr,
    ) -> EntityId {
        let op = self.take_op();
        self.operations.push(Operation {
            entity_id: op,
            block,
            ordinal,
            opcode: Opcode::ConstantRef,
            operands: Vec::new(),
            result_types: vec![result_type],
            immediate: Immediate::Entity(target),
        });
        op
    }

    fn const_ref(&mut self, block: EntityId, ordinal: u32, target: EntityId) -> EntityId {
        self.const_ref_as(block, ordinal, target, u8_type())
    }

    fn equal_branch(
        &mut self,
        block_id: u8,
        parameter: EntityId,
        expected: EntityId,
        if_true: u8,
        if_false: u8,
    ) {
        let block = id(block_id);
        let expected_op = self.const_ref_as(block, 0, expected, u32_type());
        let equal_op = self.take_op();
        self.operations.push(Operation {
            entity_id: equal_op,
            block,
            ordinal: 1,
            opcode: Opcode::Equal,
            operands: vec![
                ValueRef::Parameter(parameter),
                ValueRef::OperationResult(OperationResultRef {
                    operation: expected_op,
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
            operations: vec![expected_op, equal_op],
            terminator: Terminator::CondBranch(CondBranchTerminator {
                condition: ValueRef::OperationResult(OperationResultRef {
                    operation: equal_op,
                    result_index: 0,
                }),
                if_true: TargetEdge {
                    target: id(if_true),
                    arguments: Vec::new(),
                },
                if_false: TargetEdge {
                    target: id(if_false),
                    arguments: Vec::new(),
                },
            }),
            reachability: Reachability::Required,
        });
    }

    fn bool_branch(&mut self, block_id: u8, parameter: EntityId, if_true: u8, if_false: u8) {
        self.blocks.push(Block {
            entity_id: id(block_id),
            function: self.function,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::CondBranch(CondBranchTerminator {
                condition: ValueRef::Parameter(parameter),
                if_true: TargetEdge {
                    target: id(if_true),
                    arguments: Vec::new(),
                },
                if_false: TargetEdge {
                    target: id(if_false),
                    arguments: Vec::new(),
                },
            }),
            reachability: Reachability::Required,
        });
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

    // Judgment leg: unimplemented trap carrying the phase index.
    fn leg_block(&mut self, leg: u8, payload_const: EntityId) {
        let block = id(LEG_BLOCK_1 + leg - 1);
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

fn checker_scaffold() -> CheckerScaffold {
    let function = id(FUNCTION);
    let marker_param = id(MARKER_PARAM);
    let witness_param = id(WITNESS_PARAM);
    // Marker constants K0..K3; K1..K3 double as the per-leg trap
    // payloads, so reaching a leg is observable: leg k traps carrying k.
    let const_id = |k: u8| id(250 + k);
    let accept_const = id(254);
    let type_const = id(255);
    let mut constants: Vec<ConstantDefinition> = (0..4)
        .map(|k| ConstantDefinition {
            entity_id: const_id(k),
            value: u8_value(u128::from(k)),
        })
        .collect();
    constants.push(ConstantDefinition {
        entity_id: accept_const,
        value: u8_value(ACCEPT_EMPTY_PLAN),
    });
    constants.push(ConstantDefinition {
        entity_id: type_const,
        value: u8_value(TYPE_ERROR),
    });

    // Operation identities run on their own sequential namespace (120+);
    // block identities stay in the 240s. Never mixed, never reused.
    let mut builder = ScaffoldBuilder::new(function, marker_param);
    // Entry tests marker 0 (trivial accept); the chain then tests 1..=3
    // in order; the final else covers everything unknown, so block 3
    // both dispatches leg 3 and returns TYPE for anything else.
    builder.chain_block(ENTRY_BLOCK, const_id(0), ACCEPT_BLOCK, CHAIN_BLOCK_1);
    let chain = [
        (CHAIN_BLOCK_1, 1, LEG_BLOCK_1, CHAIN_BLOCK_2),
        (CHAIN_BLOCK_2, 2, LEG_BLOCK_2, CHAIN_BLOCK_3),
        (CHAIN_BLOCK_3, 3, LEG_BLOCK_3, UNKNOWN_BLOCK),
    ];
    for (block_id, marker, leg, next) in chain {
        builder.chain_block(block_id, const_id(marker), leg, next);
    }
    // Judgment legs: unimplemented traps carrying the phase index.
    for leg in 1..4 {
        builder.leg_block(leg, const_id(leg));
    }
    // Trivial accept: the admitted trivial program with an empty plan.
    builder.value_block(ACCEPT_BLOCK, accept_const);
    // Unknown marker: typed TYPE error value (the vocabulary return
    // path; the only value-returning error exit in the scaffold).
    builder.value_block(UNKNOWN_BLOCK, type_const);
    let (blocks, operations) = builder.finish();

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![marker_param, witness_param],
        result_type: u8_type(),
        effects: Vec::new(),
        entry_block: id(ENTRY_BLOCK),
        blocks: blocks.iter().map(|block| block.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    CheckerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: vec![
            Parameter {
                entity_id: marker_param,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: u8_type(),
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

/// Real bounded §1.2 algorithm for a two-block Boolean CFG projection.
///
/// Every fact is supplied after admission. The image owns the validation
/// order and the mapping to frozen S20-220 codes; successful output is
/// `(reachable_block_count, edge_count, dominator_word_operations)`.
#[allow(clippy::too_many_lines)]
fn single_bool_cfg_checker() -> CheckerScaffold {
    const FUNCTION_ID: u8 = 1;
    const BLOCK_COUNT_PARAM: u8 = 10;
    const ENTRY_MATCHES_PARAM: u8 = 11;
    const OPERATION_COUNT_PARAM: u8 = 12;
    const OPERATION_ORDINAL_PARAM: u8 = 13;
    const REACHABILITY_MATCHES_PARAM: u8 = 14;
    const OPERAND_RESOLVES_PARAM: u8 = 15;
    const RETURN_RESULT_INDEX_PARAM: u8 = 16;
    const RESULT_IS_BOOL_PARAM: u8 = 17;

    const BLOCK_COUNT_CHECK: u8 = 20;
    const ENTRY_CHECK: u8 = 21;
    const OPERATION_COUNT_CHECK: u8 = 22;
    const OPERATION_ORDINAL_CHECK: u8 = 23;
    const REACHABILITY_CHECK: u8 = 24;
    const OPERAND_CHECK: u8 = 25;
    const RESULT_INDEX_CHECK: u8 = 26;
    const RETURN_TYPE_CHECK: u8 = 27;
    const SUCCESS: u8 = 28;
    const INVENTORY_ERROR: u8 = 29;
    const ENTRY_ERROR: u8 = 30;
    const ORDINAL_ERROR: u8 = 31;
    const REACHABILITY_ERROR: u8 = 32;
    const VALUE_ERROR: u8 = 33;
    const RESULT_INDEX_ERROR: u8 = 34;
    const RETURN_TYPE_ERROR: u8 = 35;

    const K_ZERO_U32: u8 = 60;
    const K_ONE_U32: u8 = 61;
    const K_ZERO_U64: u8 = 62;
    const K_INVENTORY_ERROR: u8 = 63;
    const K_ENTRY_ERROR: u8 = 64;
    const K_ORDINAL_ERROR: u8 = 65;
    const K_REACHABILITY_ERROR: u8 = 66;
    const K_VALUE_ERROR: u8 = 67;
    const K_RESULT_INDEX_ERROR: u8 = 68;
    const K_RETURN_TYPE_ERROR: u8 = 69;
    const K_TWO_U32: u8 = 70;

    let function = id(FUNCTION_ID);
    let block_count = id(BLOCK_COUNT_PARAM);
    let entry_matches = id(ENTRY_MATCHES_PARAM);
    let operation_count = id(OPERATION_COUNT_PARAM);
    let operation_ordinal = id(OPERATION_ORDINAL_PARAM);
    let reachability_matches = id(REACHABILITY_MATCHES_PARAM);
    let operand_resolves = id(OPERAND_RESOLVES_PARAM);
    let return_result_index = id(RETURN_RESULT_INDEX_PARAM);
    let result_is_bool = id(RESULT_IS_BOOL_PARAM);

    let constants = vec![
        ConstantDefinition {
            entity_id: id(K_ZERO_U32),
            value: u32_value(0),
        },
        ConstantDefinition {
            entity_id: id(K_ONE_U32),
            value: u32_value(1),
        },
        ConstantDefinition {
            entity_id: id(K_ZERO_U64),
            value: ConstValue {
                value_type: u64_type(),
                data: ConstData::UInt(0),
            },
        },
        ConstantDefinition {
            entity_id: id(K_INVENTORY_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::GraphInventoryMismatch.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_ENTRY_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::EntryInvalid.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_ORDINAL_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::GraphOrdinalMismatch.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_REACHABILITY_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::Reachability.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_VALUE_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::ValueUnresolved.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_RESULT_INDEX_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::ResultIndex.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_RETURN_TYPE_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::ReturnType.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_TWO_U32),
            value: u32_value(2),
        },
    ];

    let mut builder = ScaffoldBuilder::new(function, block_count);
    builder.equal_branch(
        BLOCK_COUNT_CHECK,
        block_count,
        id(K_TWO_U32),
        ENTRY_CHECK,
        INVENTORY_ERROR,
    );
    builder.bool_branch(
        ENTRY_CHECK,
        entry_matches,
        OPERATION_COUNT_CHECK,
        ENTRY_ERROR,
    );
    builder.equal_branch(
        OPERATION_COUNT_CHECK,
        operation_count,
        id(K_ONE_U32),
        OPERATION_ORDINAL_CHECK,
        INVENTORY_ERROR,
    );
    builder.equal_branch(
        OPERATION_ORDINAL_CHECK,
        operation_ordinal,
        id(K_ZERO_U32),
        REACHABILITY_CHECK,
        ORDINAL_ERROR,
    );
    builder.bool_branch(
        REACHABILITY_CHECK,
        reachability_matches,
        OPERAND_CHECK,
        REACHABILITY_ERROR,
    );
    builder.bool_branch(
        OPERAND_CHECK,
        operand_resolves,
        RESULT_INDEX_CHECK,
        VALUE_ERROR,
    );
    builder.equal_branch(
        RESULT_INDEX_CHECK,
        return_result_index,
        id(K_ZERO_U32),
        RETURN_TYPE_CHECK,
        RESULT_INDEX_ERROR,
    );
    builder.bool_branch(
        RETURN_TYPE_CHECK,
        result_is_bool,
        SUCCESS,
        RETURN_TYPE_ERROR,
    );

    let success_block = id(SUCCESS);
    let reachable = builder.const_ref_as(success_block, 0, id(K_ONE_U32), u32_type());
    let edges = builder.const_ref_as(success_block, 1, id(K_ZERO_U32), u32_type());
    let dominator_work = builder.const_ref_as(success_block, 2, id(K_ZERO_U64), u64_type());
    let plan = builder.take_op();
    builder.operations.push(Operation {
        entity_id: plan,
        block: success_block,
        ordinal: 3,
        opcode: Opcode::TupleNew,
        operands: vec![
            ValueRef::OperationResult(OperationResultRef {
                operation: reachable,
                result_index: 0,
            }),
            ValueRef::OperationResult(OperationResultRef {
                operation: edges,
                result_index: 0,
            }),
            ValueRef::OperationResult(OperationResultRef {
                operation: dominator_work,
                result_index: 0,
            }),
        ],
        result_types: vec![check_plan_type()],
        immediate: Immediate::None,
    });
    let ok = builder.take_op();
    builder.operations.push(Operation {
        entity_id: ok,
        block: success_block,
        ordinal: 4,
        opcode: Opcode::ResultOk,
        operands: vec![ValueRef::OperationResult(OperationResultRef {
            operation: plan,
            result_index: 0,
        })],
        result_types: vec![check_result_type()],
        immediate: Immediate::None,
    });
    builder.blocks.push(Block {
        entity_id: success_block,
        function,
        parameters: Vec::new(),
        operations: vec![reachable, edges, dominator_work, plan, ok],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: ok,
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    });

    for (block_id, constant_id) in [
        (INVENTORY_ERROR, K_INVENTORY_ERROR),
        (ENTRY_ERROR, K_ENTRY_ERROR),
        (ORDINAL_ERROR, K_ORDINAL_ERROR),
        (REACHABILITY_ERROR, K_REACHABILITY_ERROR),
        (VALUE_ERROR, K_VALUE_ERROR),
        (RESULT_INDEX_ERROR, K_RESULT_INDEX_ERROR),
        (RETURN_TYPE_ERROR, K_RETURN_TYPE_ERROR),
    ] {
        let block = id(block_id);
        let code = builder.const_ref_as(block, 0, id(constant_id), u32_type());
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
            result_types: vec![check_result_type()],
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
    }

    let (blocks, operations) = builder.finish();
    let parameter_specs = [
        (block_count, u32_type()),
        (entry_matches, TypeExpr::Bool),
        (operation_count, u32_type()),
        (operation_ordinal, u32_type()),
        (reachability_matches, TypeExpr::Bool),
        (operand_resolves, TypeExpr::Bool),
        (return_result_index, u32_type()),
        (result_is_bool, TypeExpr::Bool),
    ];
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: parameter_specs.iter().map(|(entity, _)| *entity).collect(),
        result_type: check_result_type(),
        effects: Vec::new(),
        entry_block: id(BLOCK_COUNT_CHECK),
        blocks: blocks.iter().map(|block| block.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = parameter_specs
        .into_iter()
        .enumerate()
        .map(|(ordinal, (entity_id, value_type))| Parameter {
            entity_id,
            owner: function,
            role: ParameterRole::Function,
            ordinal: u32::try_from(ordinal).expect("bounded checker parameter ordinal"),
            value_type,
        })
        .collect();
    CheckerScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters,
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
    admit_checker_program(&checker_scaffold())
}

fn admit_checker_program(
    scaffold: &CheckerScaffold,
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
            inputs: vec![u8_value(marker), bytes_value(witness)],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes scaffold")
    .termination
}

#[derive(Clone, Copy, Debug)]
enum FactState {
    Valid,
    Invalid,
}

impl FactState {
    const fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }
}

#[derive(Clone, Copy, Debug)]
struct SingleCfgFacts {
    block_count: u32,
    entry_matches: FactState,
    operation_count: u32,
    operation_ordinal: u32,
    reachability_matches: FactState,
    operand_resolves: FactState,
    return_result_index: u32,
    result_is_bool: FactState,
}

impl SingleCfgFacts {
    const VALID: Self = Self {
        block_count: 2,
        entry_matches: FactState::Valid,
        operation_count: 1,
        operation_ordinal: 0,
        reachability_matches: FactState::Valid,
        operand_resolves: FactState::Valid,
        return_result_index: 0,
        result_is_bool: FactState::Valid,
    };
}

fn execute_single_cfg(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    facts: SingleCfgFacts,
) -> sley_vm::ExecutionOutcome {
    let bool_value = |value| ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    };
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                u32_value(u128::from(facts.block_count)),
                bool_value(facts.entry_matches.is_valid()),
                u32_value(u128::from(facts.operation_count)),
                u32_value(u128::from(facts.operation_ordinal)),
                bool_value(facts.reachability_matches.is_valid()),
                bool_value(facts.operand_resolves.is_valid()),
                u32_value(u128::from(facts.return_result_index)),
                bool_value(facts.result_is_bool.is_valid()),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes single-CFG checker")
}

fn native_single_cfg(
    facts: SingleCfgFacts,
) -> Result<sley_check::cfg::CfgReport, sley_check::cfg::CfgErrorCode> {
    use sley_check::cfg::{CfgValidationError, validate_function_graph};

    let function_id = id(1);
    let block_id = id(2);
    let dead_block_id = id(5);
    let operation_id = id(3);
    let parameter_id = id(4);
    let function_blocks = match facts.block_count {
        2 => vec![block_id, dead_block_id],
        0 => Vec::new(),
        _ => vec![block_id],
    };
    let block_operations = match facts.operation_count {
        1 => vec![operation_id],
        0 => Vec::new(),
        _ => vec![operation_id, id(6)],
    };
    let function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: vec![parameter_id],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: if facts.entry_matches.is_valid() {
            block_id
        } else {
            id(99)
        },
        blocks: function_blocks,
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameter = Parameter {
        entity_id: parameter_id,
        owner: function_id,
        role: ParameterRole::Function,
        ordinal: 0,
        value_type: TypeExpr::Bool,
    };
    let operation = Operation {
        entity_id: operation_id,
        block: block_id,
        ordinal: facts.operation_ordinal,
        opcode: Opcode::BoolNot,
        operands: vec![ValueRef::Parameter(if facts.operand_resolves.is_valid() {
            parameter_id
        } else {
            id(98)
        })],
        result_types: vec![if facts.result_is_bool.is_valid() {
            TypeExpr::Bool
        } else {
            u32_type()
        }],
        immediate: Immediate::None,
    };
    let block = Block {
        entity_id: block_id,
        function: function_id,
        parameters: Vec::new(),
        operations: block_operations,
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: operation_id,
                result_index: facts.return_result_index,
            }),
        }),
        reachability: Reachability::Required,
    };
    let dead_block = Block {
        entity_id: dead_block_id,
        function: function_id,
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Trap(TrapTerminator {
            code: TrapCode::Unreachable,
            payload: None,
        }),
        reachability: if facts.reachability_matches.is_valid() {
            Reachability::ExplicitlyUnreachable
        } else {
            Reachability::Required
        },
    };
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    validate_function_graph(
        &types,
        &function,
        &[parameter],
        &[block, dead_block],
        &[operation],
    )
    .map_err(|error| match error {
        CfgValidationError::Cfg(error) => error.code(),
        CfgValidationError::Type(error) => {
            panic!("single-CFG reference must not reach type error: {error}")
        }
    })
}

fn assert_cfg_ok(outcome: &sley_vm::ExecutionOutcome, report: &sley_check::cfg::CfgReport) {
    use sley_ssmc::ResultConst;
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(value) => match &value.data {
            ConstData::Result(ResultConst::Ok(plan)) => match &plan.data {
                ConstData::Sequence(fields) if fields.len() == 3 => {
                    let number = |value: &ConstValue| match value.data {
                        ConstData::UInt(found) => found,
                        ref other => panic!("check plan field must be UInt, got {other:?}"),
                    };
                    assert_eq!(number(&fields[0]), report.reachable_blocks.len() as u128);
                    assert_eq!(number(&fields[1]), u128::from(report.edges));
                    assert_eq!(
                        number(&fields[2]),
                        u128::from(report.dominator_word_operations)
                    );
                }
                other => panic!("checker Ok must carry a 3-tuple, got {other:?}"),
            },
            other => panic!("single-CFG checker must return Ok, got {other:?}"),
        },
        other => panic!("single-CFG checker must succeed, got {other:?}"),
    }
}

fn assert_cfg_error(outcome: &sley_vm::ExecutionOutcome, code: sley_check::cfg::CfgErrorCode) {
    use sley_ssmc::ResultConst;
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(value) => match &value.data {
            ConstData::Result(ResultConst::Err(error)) => match error.data {
                ConstData::UInt(found) => assert_eq!(found, u128::from(code.numeric())),
                ref other => panic!("checker Err must carry UInt32, got {other:?}"),
            },
            other => panic!("single-CFG checker must return Err, got {other:?}"),
        },
        other => panic!("single-CFG checker must terminate with a value, got {other:?}"),
    }
}

#[test]
fn checker_single_boolean_cfg_matches_native_report() {
    let (package, approved) = admit_checker_program(&single_bool_cfg_checker());
    let native = native_single_cfg(SingleCfgFacts::VALID).expect("native checker accepts");
    let first = execute_single_cfg(&package, &approved, SingleCfgFacts::VALID);
    let second = execute_single_cfg(&package, &approved, SingleCfgFacts::VALID);
    assert_cfg_ok(&first, &native);
    assert_cfg_ok(&second, &native);
    assert_eq!(
        first.termination, second.termination,
        "checker is deterministic"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn checker_single_boolean_cfg_preserves_native_first_failure_codes() {
    use sley_check::cfg::CfgErrorCode;

    let (package, approved) = admit_checker_program(&single_bool_cfg_checker());
    let cases = [
        (
            SingleCfgFacts {
                block_count: 0,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::GraphInventoryMismatch,
        ),
        (
            SingleCfgFacts {
                entry_matches: FactState::Invalid,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::EntryInvalid,
        ),
        (
            SingleCfgFacts {
                operation_count: 0,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::GraphInventoryMismatch,
        ),
        (
            SingleCfgFacts {
                operation_ordinal: 1,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::GraphOrdinalMismatch,
        ),
        (
            SingleCfgFacts {
                reachability_matches: FactState::Invalid,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::Reachability,
        ),
        (
            SingleCfgFacts {
                operand_resolves: FactState::Invalid,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::ValueUnresolved,
        ),
        (
            SingleCfgFacts {
                return_result_index: 1,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::ResultIndex,
        ),
        (
            SingleCfgFacts {
                result_is_bool: FactState::Invalid,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::ReturnType,
        ),
        (
            SingleCfgFacts {
                entry_matches: FactState::Invalid,
                operation_count: 0,
                operation_ordinal: 1,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::EntryInvalid,
        ),
        (
            SingleCfgFacts {
                operation_count: 0,
                operation_ordinal: 1,
                reachability_matches: FactState::Invalid,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::GraphInventoryMismatch,
        ),
        (
            SingleCfgFacts {
                operation_ordinal: 1,
                reachability_matches: FactState::Invalid,
                operand_resolves: FactState::Invalid,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::GraphOrdinalMismatch,
        ),
        (
            SingleCfgFacts {
                reachability_matches: FactState::Invalid,
                operand_resolves: FactState::Invalid,
                return_result_index: 1,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::Reachability,
        ),
        (
            SingleCfgFacts {
                operand_resolves: FactState::Invalid,
                return_result_index: 1,
                result_is_bool: FactState::Invalid,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::ValueUnresolved,
        ),
        (
            SingleCfgFacts {
                return_result_index: 1,
                result_is_bool: FactState::Invalid,
                ..SingleCfgFacts::VALID
            },
            CfgErrorCode::ResultIndex,
        ),
        (
            SingleCfgFacts {
                block_count: 0,
                entry_matches: FactState::Invalid,
                operation_count: 0,
                operation_ordinal: 1,
                reachability_matches: FactState::Invalid,
                operand_resolves: FactState::Invalid,
                return_result_index: 1,
                result_is_bool: FactState::Invalid,
            },
            CfgErrorCode::GraphInventoryMismatch,
        ),
    ];
    for (facts, expected) in cases {
        assert_eq!(native_single_cfg(facts), Err(expected), "native oracle");
        assert_cfg_error(&execute_single_cfg(&package, &approved, facts), expected);
    }
}

#[test]
fn checker_scaffold_phase_legs_trap_per_index() {
    use sley_vm::ExecutionTermination;
    let (package, approved) = admitted_scaffold();
    // Every judgment leg traps Unreachable carrying its own phase index —
    // dispatch is observable per leg — and distinct witness bytes trap
    // identically, proving the scaffold performs no judgment.
    for leg in 1..4_u128 {
        for witness in [b"".as_slice(), b"\x00\x01\x02checker-witness".as_slice()] {
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
fn checker_scaffold_trivial_accept_and_unknown_marker() {
    use sley_vm::ExecutionTermination;
    let (package, approved) = admitted_scaffold();
    // Marker 0 is the admitted trivial program: accepted with the empty
    // plan marker regardless of witness bytes. Outside 0..=3 the
    // scaffold returns the typed TYPE vocabulary value; no leg traps.
    for witness in [b"".as_slice(), b"\x00\x01\x02checker-witness".as_slice()] {
        match execute_scaffold(&package, &approved, 0, witness) {
            ExecutionTermination::Success(value) => assert_eq!(
                value.data,
                ConstData::UInt(ACCEPT_EMPTY_PLAN),
                "marker 0 is the trivial accept"
            ),
            other => panic!("marker 0 must accept, got {other:?}"),
        }
    }
    for marker in [4_u128, 7_u128, 255_u128] {
        match execute_scaffold(&package, &approved, marker, b"") {
            ExecutionTermination::Success(value) => assert_eq!(
                value.data,
                ConstData::UInt(TYPE_ERROR),
                "unknown marker {marker} returns TYPE"
            ),
            other => panic!("unknown marker {marker} must return TYPE, got {other:?}"),
        }
    }
}
