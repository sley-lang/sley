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
//! either the native CFG code or a compact deterministic report. The later
//! `option_switch_cfg_checker` covers the switch-specific target, selector,
//! case, payload, and argument judgments for an `Option<Bool>` projection.
//! The bounded `single_effect_closure_checker` resolves one optional declared
//! and requested effect identity before comparing the computed closure.
//! `effect_set_inventory_checker` advances that phase over runtime vectors
//! with Sley-owned strict-order and nested membership walks.
//! `two_function_effect_closure_checker` adds bounded direct-call propagation
//! and native-equivalent closure-work accounting.
//! `composed::bounded_checker_program` rebases all seven executable slices
//! into one collision-free closure, dispatches them through direct calls, and
//! normalizes their reports while forwarding frozen numeric errors exactly.
//! Construction provenance:
//! machineresearch/sley-2.0/reweave/rw-080-checker-scaffold.md,
//! machineresearch/sley-2.0/reweave/rw-080-checker-single-cfg.md,
//! machineresearch/sley-2.0/reweave/rw-080-checker-option-switch.md,
//! machineresearch/sley-2.0/reweave/rw-080-checker-operation-inventory.md,
//! machineresearch/sley-2.0/reweave/rw-080-checker-type-chain.md,
//! machineresearch/sley-2.0/reweave/rw-080-checker-single-effect.md,
//! machineresearch/sley-2.0/reweave/rw-080-checker-effect-inventory.md, and
//! machineresearch/sley-2.0/reweave/rw-080-checker-effect-propagation.md.

use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    Block, BuiltinCase, BuiltinFailureKind, CaseKey, CondBranchTerminator, ConstData, ConstValue,
    ConstantDefinition, EffectDefinition, EffectKind, FunctionGraph, FunctionRefValue, Immediate,
    IntegerWidth, MAX_TYPE_DEPTH, Opcode, Operation, OperationResultRef, Parameter, ParameterRole,
    Reachability, ReturnTerminator, SwitchArgument, SwitchCase, SwitchEdge, TargetEdge, Terminator,
    TrapCode, TrapTerminator, TypeExpr, ValueRef, VariantSwitchTerminator, Visibility,
};

#[path = "rw080_checker_program/canonical.rs"]
mod canonical;
#[path = "rw080_checker_program/composed.rs"]
mod composed;

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

fn u64vec_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(u64_type()))
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

fn cfg_inventory_row_type() -> TypeExpr {
    TypeExpr::Tuple(vec![u64_type(), u64_type(), u64_type()])
}

fn cfg_inventory_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(cfg_inventory_row_type()))
}

fn arithmetic_u64_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(u64_type()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    }
}

fn type_chain_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(u64_type()),
        error: Box::new(u32_type()),
    }
}

fn effect_summary_type() -> TypeExpr {
    TypeExpr::Tuple(vec![u64_type(), u32_type(), u32_type(), u64_type()])
}

fn effect_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(effect_summary_type()),
        error: Box::new(u32_type()),
    }
}

fn bytes_value(bytes: &[u8]) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(bytes.to_vec()),
    }
}

type CfgInventoryRow = (u64, u64, u64);

fn cfg_inventory_value(rows: &[CfgInventoryRow]) -> ConstValue {
    let value = |number| ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::from(number)),
    };
    ConstValue {
        value_type: cfg_inventory_type(),
        data: ConstData::Sequence(
            rows.iter()
                .map(|(ordinal, reference_kind, reference_index)| ConstValue {
                    value_type: cfg_inventory_row_type(),
                    data: ConstData::Sequence(vec![
                        value(*ordinal),
                        value(*reference_kind),
                        value(*reference_index),
                    ]),
                })
                .collect(),
        ),
    }
}

fn u64vec_value(values: &[u64]) -> ConstValue {
    ConstValue {
        value_type: u64vec_type(),
        data: ConstData::Sequence(
            values
                .iter()
                .map(|value| ConstValue {
                    value_type: u64_type(),
                    data: ConstData::UInt(u128::from(*value)),
                })
                .collect(),
        ),
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

fn checker_inventory_id(namespace: u8, index: u16) -> EntityId {
    let mut bytes = [0_u8; 32];
    bytes[0] = namespace;
    bytes[1..3].copy_from_slice(&index.to_be_bytes());
    EntityId::from_bytes(bytes)
}

struct InventoryCheckAssembler {
    next_block: u16,
    next_parameter: u16,
    next_operation: u16,
    next_constant: u16,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
}

impl InventoryCheckAssembler {
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
        let id = checker_inventory_id(1, self.next_block);
        self.next_block += 1;
        id
    }

    fn parameter(
        &mut self,
        owner: EntityId,
        role: ParameterRole,
        ordinal: u32,
        value_type: TypeExpr,
    ) -> EntityId {
        let id = checker_inventory_id(2, self.next_parameter);
        self.next_parameter += 1;
        self.parameters.push(Parameter {
            entity_id: id,
            owner,
            role,
            ordinal,
            value_type,
        });
        id
    }

    fn operation(
        &mut self,
        block: EntityId,
        opcode: Opcode,
        operands: Vec<ValueRef>,
        result_type: TypeExpr,
        immediate: Immediate,
    ) -> EntityId {
        let id = checker_inventory_id(3, self.next_operation);
        self.next_operation += 1;
        let ordinal = u32::try_from(
            self.operations
                .iter()
                .filter(|operation| operation.block == block)
                .count(),
        )
        .expect("bounded inventory operation ordinal");
        self.operations.push(Operation {
            entity_id: id,
            block,
            ordinal,
            opcode,
            operands,
            result_types: vec![result_type],
            immediate,
        });
        id
    }

    fn constant(&mut self, value: ConstValue) -> EntityId {
        let id = checker_inventory_id(4, self.next_constant);
        self.next_constant += 1;
        self.constants.push(ConstantDefinition {
            entity_id: id,
            value,
        });
        id
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

fn inventory_operation_value(operation: EntityId) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation,
        result_index: 0,
    })
}

fn inventory_branch(target: EntityId, arguments: Vec<ValueRef>) -> Terminator {
    Terminator::Branch(sley_ssmc::BranchTerminator {
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

/// Appends a real Sley loop that requires a runtime `Vector<UInt(64)>` to
/// be strictly increasing. The returned block is the loop entry.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn append_sorted_u64_vector_check(
    assembler: &mut InventoryCheckAssembler,
    function: EntityId,
    values: EntityId,
    zero: EntityId,
    one: EntityId,
    success: EntityId,
    noncanonical: EntityId,
    resource_error: EntityId,
    invariant_trap: EntityId,
) -> EntityId {
    let entry = assembler.block_id();
    let first_get = assembler.block_id();
    let first_unpack = assembler.block_id();
    let check = assembler.block_id();
    let get = assembler.block_id();
    let unpack = assembler.block_id();
    let compare = assembler.block_id();
    let advance = assembler.block_id();

    let start = assembler.constant_ref(entry, zero, u64_type());
    let length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(values)],
        u64_type(),
        Immediate::None,
    );
    let has_first = assembler.operation(
        entry,
        Opcode::LessThan,
        vec![
            inventory_operation_value(start),
            inventory_operation_value(length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![start, length, has_first],
        inventory_cond(
            inventory_operation_value(has_first),
            first_get,
            vec![inventory_operation_value(start)],
            success,
            Vec::new(),
        ),
    );

    let first_index = assembler.parameter(first_get, ParameterRole::Block, 0, u64_type());
    let first = assembler.operation(
        first_get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(values),
            ValueRef::Parameter(first_index),
        ],
        TypeExpr::Option(Box::new(u64_type())),
        Immediate::None,
    );
    assembler.push_block(
        first_get,
        function,
        vec![first_index],
        vec![first],
        inventory_switch(
            inventory_operation_value(first),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    first_unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(first_index)),
                    ],
                ),
            ],
        ),
    );

    let first_value = assembler.parameter(first_unpack, ParameterRole::Block, 0, u64_type());
    let unpacked_first_index =
        assembler.parameter(first_unpack, ParameterRole::Block, 1, u64_type());
    let first_one = assembler.constant_ref(first_unpack, one, u64_type());
    let after_first = assembler.operation(
        first_unpack,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(unpacked_first_index),
            inventory_operation_value(first_one),
        ],
        arithmetic_u64_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        first_unpack,
        function,
        vec![first_value, unpacked_first_index],
        vec![first_one, after_first],
        inventory_switch(
            inventory_operation_value(after_first),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(first_value)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let previous = assembler.parameter(check, ParameterRole::Block, 0, u64_type());
    let index = assembler.parameter(check, ParameterRole::Block, 1, u64_type());
    let check_length = assembler.operation(
        check,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(values)],
        u64_type(),
        Immediate::None,
    );
    let has_value = assembler.operation(
        check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(index),
            inventory_operation_value(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        check,
        function,
        vec![previous, index],
        vec![check_length, has_value],
        inventory_cond(
            inventory_operation_value(has_value),
            get,
            vec![ValueRef::Parameter(previous), ValueRef::Parameter(index)],
            success,
            Vec::new(),
        ),
    );

    let get_previous = assembler.parameter(get, ParameterRole::Block, 0, u64_type());
    let get_index = assembler.parameter(get, ParameterRole::Block, 1, u64_type());
    let value = assembler.operation(
        get,
        Opcode::VectorGet,
        vec![ValueRef::Parameter(values), ValueRef::Parameter(get_index)],
        TypeExpr::Option(Box::new(u64_type())),
        Immediate::None,
    );
    assembler.push_block(
        get,
        function,
        vec![get_previous, get_index],
        vec![value],
        inventory_switch(
            inventory_operation_value(value),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_previous)),
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                    ],
                ),
            ],
        ),
    );

    let current = assembler.parameter(unpack, ParameterRole::Block, 0, u64_type());
    let unpack_previous = assembler.parameter(unpack, ParameterRole::Block, 1, u64_type());
    let unpack_index = assembler.parameter(unpack, ParameterRole::Block, 2, u64_type());
    assembler.push_block(
        unpack,
        function,
        vec![current, unpack_previous, unpack_index],
        Vec::new(),
        inventory_branch(
            compare,
            vec![
                ValueRef::Parameter(unpack_previous),
                ValueRef::Parameter(current),
                ValueRef::Parameter(unpack_index),
            ],
        ),
    );

    let compare_previous = assembler.parameter(compare, ParameterRole::Block, 0, u64_type());
    let compare_current = assembler.parameter(compare, ParameterRole::Block, 1, u64_type());
    let compare_index = assembler.parameter(compare, ParameterRole::Block, 2, u64_type());
    let ordered = assembler.operation(
        compare,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(compare_previous),
            ValueRef::Parameter(compare_current),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        compare,
        function,
        vec![compare_previous, compare_current, compare_index],
        vec![ordered],
        inventory_cond(
            inventory_operation_value(ordered),
            advance,
            vec![
                ValueRef::Parameter(compare_current),
                ValueRef::Parameter(compare_index),
            ],
            noncanonical,
            Vec::new(),
        ),
    );

    let advance_current = assembler.parameter(advance, ParameterRole::Block, 0, u64_type());
    let advance_index = assembler.parameter(advance, ParameterRole::Block, 1, u64_type());
    let advance_one = assembler.constant_ref(advance, one, u64_type());
    let next = assembler.operation(
        advance,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_index),
            inventory_operation_value(advance_one),
        ],
        arithmetic_u64_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        advance,
        function,
        vec![advance_current, advance_index],
        vec![advance_one, next],
        inventory_switch(
            inventory_operation_value(next),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(advance_current)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );
    entry
}

/// Appends a nested Sley walk that resolves every needle against the runtime
/// haystack. The returned block is the outer-loop entry.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn append_u64_membership_check(
    assembler: &mut InventoryCheckAssembler,
    function: EntityId,
    needles: EntityId,
    haystack: EntityId,
    zero: EntityId,
    one: EntityId,
    success: EntityId,
    unresolved: EntityId,
    resource_error: EntityId,
    invariant_trap: EntityId,
) -> EntityId {
    let entry = assembler.block_id();
    let needle_check = assembler.block_id();
    let needle_get = assembler.block_id();
    let needle_unpack = assembler.block_id();
    let search_check = assembler.block_id();
    let search_get = assembler.block_id();
    let search_unpack = assembler.block_id();
    let compare = assembler.block_id();
    let needle_advance = assembler.block_id();
    let search_advance = assembler.block_id();

    let start = assembler.constant_ref(entry, zero, u64_type());
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![start],
        inventory_branch(needle_check, vec![inventory_operation_value(start)]),
    );

    let needle_index = assembler.parameter(needle_check, ParameterRole::Block, 0, u64_type());
    let needle_length = assembler.operation(
        needle_check,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(needles)],
        u64_type(),
        Immediate::None,
    );
    let has_needle = assembler.operation(
        needle_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(needle_index),
            inventory_operation_value(needle_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        needle_check,
        function,
        vec![needle_index],
        vec![needle_length, has_needle],
        inventory_cond(
            inventory_operation_value(has_needle),
            needle_get,
            vec![ValueRef::Parameter(needle_index)],
            success,
            Vec::new(),
        ),
    );

    let get_needle_index = assembler.parameter(needle_get, ParameterRole::Block, 0, u64_type());
    let needle = assembler.operation(
        needle_get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(needles),
            ValueRef::Parameter(get_needle_index),
        ],
        TypeExpr::Option(Box::new(u64_type())),
        Immediate::None,
    );
    assembler.push_block(
        needle_get,
        function,
        vec![get_needle_index],
        vec![needle],
        inventory_switch(
            inventory_operation_value(needle),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    needle_unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_needle_index)),
                    ],
                ),
            ],
        ),
    );

    let needle_value = assembler.parameter(needle_unpack, ParameterRole::Block, 0, u64_type());
    let unpack_needle_index =
        assembler.parameter(needle_unpack, ParameterRole::Block, 1, u64_type());
    let search_zero = assembler.constant_ref(needle_unpack, zero, u64_type());
    assembler.push_block(
        needle_unpack,
        function,
        vec![needle_value, unpack_needle_index],
        vec![search_zero],
        inventory_branch(
            search_check,
            vec![
                ValueRef::Parameter(needle_value),
                ValueRef::Parameter(unpack_needle_index),
                inventory_operation_value(search_zero),
            ],
        ),
    );

    let search_needle = assembler.parameter(search_check, ParameterRole::Block, 0, u64_type());
    let search_needle_index =
        assembler.parameter(search_check, ParameterRole::Block, 1, u64_type());
    let search_index = assembler.parameter(search_check, ParameterRole::Block, 2, u64_type());
    let haystack_length = assembler.operation(
        search_check,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(haystack)],
        u64_type(),
        Immediate::None,
    );
    let has_candidate = assembler.operation(
        search_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(search_index),
            inventory_operation_value(haystack_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        search_check,
        function,
        vec![search_needle, search_needle_index, search_index],
        vec![haystack_length, has_candidate],
        inventory_cond(
            inventory_operation_value(has_candidate),
            search_get,
            vec![
                ValueRef::Parameter(search_needle),
                ValueRef::Parameter(search_needle_index),
                ValueRef::Parameter(search_index),
            ],
            unresolved,
            Vec::new(),
        ),
    );

    let get_search_needle = assembler.parameter(search_get, ParameterRole::Block, 0, u64_type());
    let get_search_needle_index =
        assembler.parameter(search_get, ParameterRole::Block, 1, u64_type());
    let get_search_index = assembler.parameter(search_get, ParameterRole::Block, 2, u64_type());
    let candidate = assembler.operation(
        search_get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(haystack),
            ValueRef::Parameter(get_search_index),
        ],
        TypeExpr::Option(Box::new(u64_type())),
        Immediate::None,
    );
    assembler.push_block(
        search_get,
        function,
        vec![get_search_needle, get_search_needle_index, get_search_index],
        vec![candidate],
        inventory_switch(
            inventory_operation_value(candidate),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    search_unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_search_needle)),
                        SwitchArgument::Value(ValueRef::Parameter(get_search_needle_index)),
                        SwitchArgument::Value(ValueRef::Parameter(get_search_index)),
                    ],
                ),
            ],
        ),
    );

    let unpack_candidate = assembler.parameter(search_unpack, ParameterRole::Block, 0, u64_type());
    let unpack_search_needle =
        assembler.parameter(search_unpack, ParameterRole::Block, 1, u64_type());
    let unpack_search_needle_index =
        assembler.parameter(search_unpack, ParameterRole::Block, 2, u64_type());
    let unpack_search_index =
        assembler.parameter(search_unpack, ParameterRole::Block, 3, u64_type());
    assembler.push_block(
        search_unpack,
        function,
        vec![
            unpack_candidate,
            unpack_search_needle,
            unpack_search_needle_index,
            unpack_search_index,
        ],
        Vec::new(),
        inventory_branch(
            compare,
            vec![
                ValueRef::Parameter(unpack_candidate),
                ValueRef::Parameter(unpack_search_needle),
                ValueRef::Parameter(unpack_search_needle_index),
                ValueRef::Parameter(unpack_search_index),
            ],
        ),
    );

    let compare_candidate = assembler.parameter(compare, ParameterRole::Block, 0, u64_type());
    let compare_needle = assembler.parameter(compare, ParameterRole::Block, 1, u64_type());
    let compare_needle_index = assembler.parameter(compare, ParameterRole::Block, 2, u64_type());
    let compare_search_index = assembler.parameter(compare, ParameterRole::Block, 3, u64_type());
    let matches = assembler.operation(
        compare,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(compare_candidate),
            ValueRef::Parameter(compare_needle),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        compare,
        function,
        vec![
            compare_candidate,
            compare_needle,
            compare_needle_index,
            compare_search_index,
        ],
        vec![matches],
        inventory_cond(
            inventory_operation_value(matches),
            needle_advance,
            vec![ValueRef::Parameter(compare_needle_index)],
            search_advance,
            vec![
                ValueRef::Parameter(compare_needle),
                ValueRef::Parameter(compare_needle_index),
                ValueRef::Parameter(compare_search_index),
            ],
        ),
    );

    let advance_needle_index =
        assembler.parameter(needle_advance, ParameterRole::Block, 0, u64_type());
    let needle_one = assembler.constant_ref(needle_advance, one, u64_type());
    let next_needle = assembler.operation(
        needle_advance,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_needle_index),
            inventory_operation_value(needle_one),
        ],
        arithmetic_u64_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        needle_advance,
        function,
        vec![advance_needle_index],
        vec![needle_one, next_needle],
        inventory_switch(
            inventory_operation_value(next_needle),
            vec![
                (
                    BuiltinCase::Ok,
                    needle_check,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let advance_search_needle =
        assembler.parameter(search_advance, ParameterRole::Block, 0, u64_type());
    let advance_search_needle_index =
        assembler.parameter(search_advance, ParameterRole::Block, 1, u64_type());
    let advance_search_index =
        assembler.parameter(search_advance, ParameterRole::Block, 2, u64_type());
    let search_one = assembler.constant_ref(search_advance, one, u64_type());
    let next_search = assembler.operation(
        search_advance,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_search_index),
            inventory_operation_value(search_one),
        ],
        arithmetic_u64_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        search_advance,
        function,
        vec![
            advance_search_needle,
            advance_search_needle_index,
            advance_search_index,
        ],
        vec![search_one, next_search],
        inventory_switch(
            inventory_operation_value(next_search),
            vec![
                (
                    BuiltinCase::Ok,
                    search_check,
                    vec![
                        SwitchArgument::Value(ValueRef::Parameter(advance_search_needle)),
                        SwitchArgument::Value(ValueRef::Parameter(advance_search_needle_index)),
                        SwitchArgument::CasePayload,
                    ],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );
    entry
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

/// Real bounded §1.2 judgment for the switch-specific CFG rules of an
/// `Option<Bool>` selector. Runtime facts select the native failure surface;
/// the Sley image owns the frozen first-failure ordering and report.
#[allow(clippy::too_many_lines)]
fn option_switch_cfg_checker() -> CheckerScaffold {
    const FUNCTION_ID: u8 = 101;
    const TARGET_VALID_PARAM: u8 = 102;
    const SELECTOR_TYPE_PARAM: u8 = 103;
    const CASE_COUNT_PARAM: u8 = 104;
    const FIRST_KEY_PARAM: u8 = 105;
    const SECOND_KEY_PARAM: u8 = 106;
    const NONE_PAYLOAD_PARAM: u8 = 107;
    const ARGUMENT_TYPES_PARAM: u8 = 108;

    const TARGET_CHECK: u8 = 150;
    const SELECTOR_CHECK: u8 = 151;
    const CASE_COUNT_CHECK: u8 = 152;
    const FIRST_KEY_CHECK: u8 = 153;
    const SECOND_KEY_CHECK: u8 = 154;
    const NONE_PAYLOAD_CHECK: u8 = 155;
    const ARGUMENT_TYPES_CHECK: u8 = 156;
    const SUCCESS: u8 = 157;
    const TARGET_ERROR: u8 = 158;
    const SWITCH_TYPE_ERROR: u8 = 159;
    const SWITCH_CASES_ERROR: u8 = 160;
    const SWITCH_PAYLOAD_ERROR: u8 = 161;
    const TARGET_ARGUMENTS_ERROR: u8 = 162;

    const K_OPTION_TAG: u8 = 170;
    const K_TWO_U32: u8 = 171;
    const K_NONE_TAG: u8 = 172;
    const K_SOME_TAG: u8 = 173;
    const K_THREE_U32: u8 = 174;
    const K_TWO_EDGES: u8 = 175;
    const K_EIGHT_WORK: u8 = 176;
    const K_TARGET_ERROR: u8 = 177;
    const K_SWITCH_TYPE_ERROR: u8 = 178;
    const K_SWITCH_CASES_ERROR: u8 = 179;
    const K_SWITCH_PAYLOAD_ERROR: u8 = 180;
    const K_TARGET_ARGUMENTS_ERROR: u8 = 181;

    let function = id(FUNCTION_ID);
    let target_valid = id(TARGET_VALID_PARAM);
    let selector_type = id(SELECTOR_TYPE_PARAM);
    let case_count = id(CASE_COUNT_PARAM);
    let first_key = id(FIRST_KEY_PARAM);
    let second_key = id(SECOND_KEY_PARAM);
    let none_payload = id(NONE_PAYLOAD_PARAM);
    let argument_types = id(ARGUMENT_TYPES_PARAM);
    let constants = vec![
        ConstantDefinition {
            entity_id: id(K_OPTION_TAG),
            value: u32_value(u128::from(TypeExpr::Option(Box::new(TypeExpr::Bool)).tag())),
        },
        ConstantDefinition {
            entity_id: id(K_TWO_U32),
            value: u32_value(2),
        },
        ConstantDefinition {
            entity_id: id(K_NONE_TAG),
            value: u32_value(u128::from(BuiltinCase::None.tag())),
        },
        ConstantDefinition {
            entity_id: id(K_SOME_TAG),
            value: u32_value(u128::from(BuiltinCase::Some.tag())),
        },
        ConstantDefinition {
            entity_id: id(K_THREE_U32),
            value: u32_value(3),
        },
        ConstantDefinition {
            entity_id: id(K_TWO_EDGES),
            value: u32_value(2),
        },
        ConstantDefinition {
            entity_id: id(K_EIGHT_WORK),
            value: ConstValue {
                value_type: u64_type(),
                data: ConstData::UInt(8),
            },
        },
        ConstantDefinition {
            entity_id: id(K_TARGET_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::TargetInvalid.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_SWITCH_TYPE_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::SwitchType.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_SWITCH_CASES_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::SwitchCases.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_SWITCH_PAYLOAD_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::SwitchPayload.numeric(),
            )),
        },
        ConstantDefinition {
            entity_id: id(K_TARGET_ARGUMENTS_ERROR),
            value: u32_value(u128::from(
                sley_check::cfg::CfgErrorCode::TargetArguments.numeric(),
            )),
        },
    ];

    let mut builder = ScaffoldBuilder::new(function, selector_type);
    builder.bool_branch(TARGET_CHECK, target_valid, SELECTOR_CHECK, TARGET_ERROR);
    builder.equal_branch(
        SELECTOR_CHECK,
        selector_type,
        id(K_OPTION_TAG),
        CASE_COUNT_CHECK,
        SWITCH_TYPE_ERROR,
    );
    builder.equal_branch(
        CASE_COUNT_CHECK,
        case_count,
        id(K_TWO_U32),
        FIRST_KEY_CHECK,
        SWITCH_CASES_ERROR,
    );
    builder.equal_branch(
        FIRST_KEY_CHECK,
        first_key,
        id(K_NONE_TAG),
        SECOND_KEY_CHECK,
        SWITCH_CASES_ERROR,
    );
    builder.equal_branch(
        SECOND_KEY_CHECK,
        second_key,
        id(K_SOME_TAG),
        NONE_PAYLOAD_CHECK,
        SWITCH_CASES_ERROR,
    );
    builder.bool_branch(
        NONE_PAYLOAD_CHECK,
        none_payload,
        SWITCH_PAYLOAD_ERROR,
        ARGUMENT_TYPES_CHECK,
    );
    builder.bool_branch(
        ARGUMENT_TYPES_CHECK,
        argument_types,
        SUCCESS,
        TARGET_ARGUMENTS_ERROR,
    );

    let success_block = id(SUCCESS);
    let reachable = builder.const_ref_as(success_block, 0, id(K_THREE_U32), u32_type());
    let edges = builder.const_ref_as(success_block, 1, id(K_TWO_EDGES), u32_type());
    let dominator_work = builder.const_ref_as(success_block, 2, id(K_EIGHT_WORK), u64_type());
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
        (TARGET_ERROR, K_TARGET_ERROR),
        (SWITCH_TYPE_ERROR, K_SWITCH_TYPE_ERROR),
        (SWITCH_CASES_ERROR, K_SWITCH_CASES_ERROR),
        (SWITCH_PAYLOAD_ERROR, K_SWITCH_PAYLOAD_ERROR),
        (TARGET_ARGUMENTS_ERROR, K_TARGET_ARGUMENTS_ERROR),
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
        (target_valid, TypeExpr::Bool),
        (selector_type, u32_type()),
        (case_count, u32_type()),
        (first_key, u32_type()),
        (second_key, u32_type()),
        (none_payload, TypeExpr::Bool),
        (argument_types, TypeExpr::Bool),
    ];
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: parameter_specs.iter().map(|(entity, _)| *entity).collect(),
        result_type: check_result_type(),
        effects: Vec::new(),
        entry_block: id(TARGET_CHECK),
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

/// Walks an arbitrary runtime operation inventory twice. The first pass owns
/// global ordinal precedence; only after it succeeds may the second pass
/// resolve parameter and operation-result uses in program order.
#[allow(clippy::too_many_lines)]
fn ordered_operation_inventory_checker() -> CheckerScaffold {
    let function = checker_inventory_id(5, 1);
    let mut assembler = InventoryCheckAssembler::new();
    let rows = assembler.parameter(function, ParameterRole::Function, 0, cfg_inventory_type());
    let parameter_count = assembler.parameter(function, ParameterRole::Function, 1, u64_type());

    let entry = assembler.block_id();
    let ordinal_check = assembler.block_id();
    let ordinal_get = assembler.block_id();
    let ordinal_unpack = assembler.block_id();
    let ordinal_compare = assembler.block_id();
    let ordinal_advance = assembler.block_id();
    let reference_start = assembler.block_id();
    let reference_check = assembler.block_id();
    let reference_get = assembler.block_id();
    let reference_unpack = assembler.block_id();
    let reference_kind_parameter = assembler.block_id();
    let reference_kind_operation = assembler.block_id();
    let parameter_reference_check = assembler.block_id();
    let operation_reference_exists = assembler.block_id();
    let operation_reference_prior = assembler.block_id();
    let reference_advance = assembler.block_id();
    let success = assembler.block_id();
    let ordinal_error = assembler.block_id();
    let value_error = assembler.block_id();
    let use_before_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();

    let zero = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(0),
    });
    let one = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(1),
    });
    let parameter_kind = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(1),
    });
    let operation_kind = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(2),
    });
    let ordinal_code = assembler.constant(u32_value(u128::from(
        sley_check::cfg::CfgErrorCode::GraphOrdinalMismatch.numeric(),
    )));
    let value_code = assembler.constant(u32_value(u128::from(
        sley_check::cfg::CfgErrorCode::ValueUnresolved.numeric(),
    )));
    let use_before_code = assembler.constant(u32_value(u128::from(
        sley_check::cfg::CfgErrorCode::UseBeforeDefinition.numeric(),
    )));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_check::cfg::CfgErrorCode::ResourceLimit.numeric(),
    )));
    let reachable_one = assembler.constant(u32_value(1));
    let edges_zero = assembler.constant(u32_value(0));
    let work_zero = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(0),
    });

    let entry_zero = assembler.constant_ref(entry, zero, u64_type());
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![entry_zero],
        inventory_branch(ordinal_check, vec![inventory_operation_value(entry_zero)]),
    );

    let ordinal_check_index =
        assembler.parameter(ordinal_check, ParameterRole::Block, 0, u64_type());
    let ordinal_length = assembler.operation(
        ordinal_check,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(rows)],
        u64_type(),
        Immediate::None,
    );
    let ordinal_has_row = assembler.operation(
        ordinal_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(ordinal_check_index),
            inventory_operation_value(ordinal_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        ordinal_check,
        function,
        vec![ordinal_check_index],
        vec![ordinal_length, ordinal_has_row],
        inventory_cond(
            inventory_operation_value(ordinal_has_row),
            ordinal_get,
            vec![ValueRef::Parameter(ordinal_check_index)],
            reference_start,
            Vec::new(),
        ),
    );

    let ordinal_get_index = assembler.parameter(ordinal_get, ParameterRole::Block, 0, u64_type());
    let ordinal_row = assembler.operation(
        ordinal_get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(rows),
            ValueRef::Parameter(ordinal_get_index),
        ],
        TypeExpr::Option(Box::new(cfg_inventory_row_type())),
        Immediate::None,
    );
    assembler.push_block(
        ordinal_get,
        function,
        vec![ordinal_get_index],
        vec![ordinal_row],
        inventory_switch(
            inventory_operation_value(ordinal_row),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    ordinal_unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(ordinal_get_index)),
                    ],
                ),
            ],
        ),
    );

    let ordinal_unpack_row = assembler.parameter(
        ordinal_unpack,
        ParameterRole::Block,
        0,
        cfg_inventory_row_type(),
    );
    let ordinal_unpack_index =
        assembler.parameter(ordinal_unpack, ParameterRole::Block, 1, u64_type());
    let found_ordinal = assembler.operation(
        ordinal_unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(ordinal_unpack_row)],
        u64_type(),
        Immediate::Index(0),
    );
    assembler.push_block(
        ordinal_unpack,
        function,
        vec![ordinal_unpack_row, ordinal_unpack_index],
        vec![found_ordinal],
        inventory_branch(
            ordinal_compare,
            vec![
                inventory_operation_value(found_ordinal),
                ValueRef::Parameter(ordinal_unpack_index),
            ],
        ),
    );

    let ordinal_compare_value =
        assembler.parameter(ordinal_compare, ParameterRole::Block, 0, u64_type());
    let ordinal_compare_index =
        assembler.parameter(ordinal_compare, ParameterRole::Block, 1, u64_type());
    let ordinal_matches = assembler.operation(
        ordinal_compare,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(ordinal_compare_value),
            ValueRef::Parameter(ordinal_compare_index),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        ordinal_compare,
        function,
        vec![ordinal_compare_value, ordinal_compare_index],
        vec![ordinal_matches],
        inventory_cond(
            inventory_operation_value(ordinal_matches),
            ordinal_advance,
            vec![ValueRef::Parameter(ordinal_compare_index)],
            ordinal_error,
            Vec::new(),
        ),
    );

    let ordinal_advance_index =
        assembler.parameter(ordinal_advance, ParameterRole::Block, 0, u64_type());
    let ordinal_one = assembler.constant_ref(ordinal_advance, one, u64_type());
    let next_ordinal_index = assembler.operation(
        ordinal_advance,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(ordinal_advance_index),
            inventory_operation_value(ordinal_one),
        ],
        arithmetic_u64_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        ordinal_advance,
        function,
        vec![ordinal_advance_index],
        vec![ordinal_one, next_ordinal_index],
        inventory_switch(
            inventory_operation_value(next_ordinal_index),
            vec![
                (
                    BuiltinCase::Ok,
                    ordinal_check,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let reference_zero = assembler.constant_ref(reference_start, zero, u64_type());
    assembler.push_block(
        reference_start,
        function,
        Vec::new(),
        vec![reference_zero],
        inventory_branch(
            reference_check,
            vec![inventory_operation_value(reference_zero)],
        ),
    );

    let reference_check_index =
        assembler.parameter(reference_check, ParameterRole::Block, 0, u64_type());
    let reference_length = assembler.operation(
        reference_check,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(rows)],
        u64_type(),
        Immediate::None,
    );
    let reference_has_row = assembler.operation(
        reference_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(reference_check_index),
            inventory_operation_value(reference_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        reference_check,
        function,
        vec![reference_check_index],
        vec![reference_length, reference_has_row],
        inventory_cond(
            inventory_operation_value(reference_has_row),
            reference_get,
            vec![ValueRef::Parameter(reference_check_index)],
            success,
            Vec::new(),
        ),
    );

    let reference_get_index =
        assembler.parameter(reference_get, ParameterRole::Block, 0, u64_type());
    let reference_row = assembler.operation(
        reference_get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(rows),
            ValueRef::Parameter(reference_get_index),
        ],
        TypeExpr::Option(Box::new(cfg_inventory_row_type())),
        Immediate::None,
    );
    assembler.push_block(
        reference_get,
        function,
        vec![reference_get_index],
        vec![reference_row],
        inventory_switch(
            inventory_operation_value(reference_row),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    reference_unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(reference_get_index)),
                    ],
                ),
            ],
        ),
    );

    let reference_unpack_row = assembler.parameter(
        reference_unpack,
        ParameterRole::Block,
        0,
        cfg_inventory_row_type(),
    );
    let reference_unpack_index =
        assembler.parameter(reference_unpack, ParameterRole::Block, 1, u64_type());
    let reference_kind = assembler.operation(
        reference_unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(reference_unpack_row)],
        u64_type(),
        Immediate::Index(1),
    );
    let reference_index = assembler.operation(
        reference_unpack,
        Opcode::TupleGet,
        vec![ValueRef::Parameter(reference_unpack_row)],
        u64_type(),
        Immediate::Index(2),
    );
    assembler.push_block(
        reference_unpack,
        function,
        vec![reference_unpack_row, reference_unpack_index],
        vec![reference_kind, reference_index],
        inventory_branch(
            reference_kind_parameter,
            vec![
                inventory_operation_value(reference_kind),
                inventory_operation_value(reference_index),
                ValueRef::Parameter(reference_unpack_index),
            ],
        ),
    );

    let kind_block = |assembler: &mut InventoryCheckAssembler,
                      block: EntityId,
                      expected: EntityId,
                      matched: EntityId,
                      unmatched: EntityId| {
        let kind = assembler.parameter(block, ParameterRole::Block, 0, u64_type());
        let referenced = assembler.parameter(block, ParameterRole::Block, 1, u64_type());
        let current = assembler.parameter(block, ParameterRole::Block, 2, u64_type());
        let expected_value = assembler.constant_ref(block, expected, u64_type());
        let matches = assembler.operation(
            block,
            Opcode::Equal,
            vec![
                ValueRef::Parameter(kind),
                inventory_operation_value(expected_value),
            ],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            vec![kind, referenced, current],
            vec![expected_value, matches],
            inventory_cond(
                inventory_operation_value(matches),
                matched,
                vec![
                    ValueRef::Parameter(referenced),
                    ValueRef::Parameter(current),
                ],
                unmatched,
                if unmatched == value_error {
                    Vec::new()
                } else {
                    vec![
                        ValueRef::Parameter(kind),
                        ValueRef::Parameter(referenced),
                        ValueRef::Parameter(current),
                    ]
                },
            ),
        );
    };
    kind_block(
        &mut assembler,
        reference_kind_parameter,
        parameter_kind,
        parameter_reference_check,
        reference_kind_operation,
    );
    kind_block(
        &mut assembler,
        reference_kind_operation,
        operation_kind,
        operation_reference_exists,
        value_error,
    );

    let parameter_reference = assembler.parameter(
        parameter_reference_check,
        ParameterRole::Block,
        0,
        u64_type(),
    );
    let parameter_current = assembler.parameter(
        parameter_reference_check,
        ParameterRole::Block,
        1,
        u64_type(),
    );
    let parameter_valid = assembler.operation(
        parameter_reference_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(parameter_reference),
            ValueRef::Parameter(parameter_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        parameter_reference_check,
        function,
        vec![parameter_reference, parameter_current],
        vec![parameter_valid],
        inventory_cond(
            inventory_operation_value(parameter_valid),
            reference_advance,
            vec![ValueRef::Parameter(parameter_current)],
            value_error,
            Vec::new(),
        ),
    );

    let exists_reference = assembler.parameter(
        operation_reference_exists,
        ParameterRole::Block,
        0,
        u64_type(),
    );
    let exists_current = assembler.parameter(
        operation_reference_exists,
        ParameterRole::Block,
        1,
        u64_type(),
    );
    let operation_count = assembler.operation(
        operation_reference_exists,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(rows)],
        u64_type(),
        Immediate::None,
    );
    let operation_exists = assembler.operation(
        operation_reference_exists,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(exists_reference),
            inventory_operation_value(operation_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        operation_reference_exists,
        function,
        vec![exists_reference, exists_current],
        vec![operation_count, operation_exists],
        inventory_cond(
            inventory_operation_value(operation_exists),
            operation_reference_prior,
            vec![
                ValueRef::Parameter(exists_reference),
                ValueRef::Parameter(exists_current),
            ],
            value_error,
            Vec::new(),
        ),
    );

    let prior_reference = assembler.parameter(
        operation_reference_prior,
        ParameterRole::Block,
        0,
        u64_type(),
    );
    let prior_current = assembler.parameter(
        operation_reference_prior,
        ParameterRole::Block,
        1,
        u64_type(),
    );
    let operation_is_prior = assembler.operation(
        operation_reference_prior,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(prior_reference),
            ValueRef::Parameter(prior_current),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        operation_reference_prior,
        function,
        vec![prior_reference, prior_current],
        vec![operation_is_prior],
        inventory_cond(
            inventory_operation_value(operation_is_prior),
            reference_advance,
            vec![ValueRef::Parameter(prior_current)],
            use_before_error,
            Vec::new(),
        ),
    );

    let reference_advance_index =
        assembler.parameter(reference_advance, ParameterRole::Block, 0, u64_type());
    let reference_one = assembler.constant_ref(reference_advance, one, u64_type());
    let next_reference_index = assembler.operation(
        reference_advance,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(reference_advance_index),
            inventory_operation_value(reference_one),
        ],
        arithmetic_u64_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        reference_advance,
        function,
        vec![reference_advance_index],
        vec![reference_one, next_reference_index],
        inventory_switch(
            inventory_operation_value(next_reference_index),
            vec![
                (
                    BuiltinCase::Ok,
                    reference_check,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let success_reachable = assembler.constant_ref(success, reachable_one, u32_type());
    let success_edges = assembler.constant_ref(success, edges_zero, u32_type());
    let success_work = assembler.constant_ref(success, work_zero, u64_type());
    let plan = assembler.operation(
        success,
        Opcode::TupleNew,
        vec![
            inventory_operation_value(success_reachable),
            inventory_operation_value(success_edges),
            inventory_operation_value(success_work),
        ],
        check_plan_type(),
        Immediate::None,
    );
    let accepted = assembler.operation(
        success,
        Opcode::ResultOk,
        vec![inventory_operation_value(plan)],
        check_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success,
        function,
        Vec::new(),
        vec![
            success_reachable,
            success_edges,
            success_work,
            plan,
            accepted,
        ],
        Terminator::Return(ReturnTerminator {
            value: inventory_operation_value(accepted),
        }),
    );

    for (block, code) in [
        (ordinal_error, ordinal_code),
        (value_error, value_code),
        (use_before_error, use_before_code),
        (resource_error, resource_code),
    ] {
        let code_value = assembler.constant_ref(block, code, u32_type());
        let rejected = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![inventory_operation_value(code_value)],
            check_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![code_value, rejected],
            Terminator::Return(ReturnTerminator {
                value: inventory_operation_value(rejected),
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
        parameters: vec![rows, parameter_count],
        result_type: check_result_type(),
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

/// Validates a runtime unary type-expression chain. Wrapper nodes are walked
/// with a CFG backedge; the leaf owns width or type-parameter judgments.
#[allow(clippy::too_many_lines)]
fn unary_type_chain_checker() -> CheckerScaffold {
    let function = checker_inventory_id(5, 2);
    let mut assembler = InventoryCheckAssembler::new();
    let wrappers = assembler.parameter(function, ParameterRole::Function, 0, u64vec_type());
    let leaf_tag = assembler.parameter(function, ParameterRole::Function, 1, u64_type());
    let leaf_payload = assembler.parameter(function, ParameterRole::Function, 2, u64_type());
    let parameter_count = assembler.parameter(function, ParameterRole::Function, 3, u64_type());

    let entry = assembler.block_id();
    let wrapper_check = assembler.block_id();
    let wrapper_get = assembler.block_id();
    let wrapper_unpack = assembler.block_id();
    let wrapper_vector = assembler.block_id();
    let wrapper_option = assembler.block_id();
    let wrapper_cell = assembler.block_id();
    let wrapper_advance = assembler.block_id();
    let leaf_bool = assembler.block_id();
    let leaf_uint = assembler.block_id();
    let leaf_parameter = assembler.block_id();
    let width_8 = assembler.block_id();
    let width_16 = assembler.block_id();
    let width_32 = assembler.block_id();
    let width_64 = assembler.block_id();
    let width_128 = assembler.block_id();
    let parameter_scope = assembler.block_id();
    let success_add = assembler.block_id();
    let success_emit = assembler.block_id();
    let depth_error = assembler.block_id();
    let width_error = assembler.block_id();
    let parameter_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();

    let zero = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(0),
    });
    let one = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(1),
    });
    let max_depth = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::try_from(MAX_TYPE_DEPTH).expect("type depth fits u128")),
    });
    let vector_tag = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::from(TypeExpr::Vector(Box::new(TypeExpr::Bool)).tag())),
    });
    let option_tag = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::from(TypeExpr::Option(Box::new(TypeExpr::Bool)).tag())),
    });
    let cell_tag = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::from(
            TypeExpr::LocalCell(Box::new(TypeExpr::Bool)).tag(),
        )),
    });
    let bool_tag = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::from(TypeExpr::Bool.tag())),
    });
    let uint_tag = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::from(u32_type().tag())),
    });
    let parameter_tag = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::from(TypeExpr::TypeParameter(0).tag())),
    });
    let width_constants = [8_u64, 16, 32, 64, 128].map(|width| {
        assembler.constant(ConstValue {
            value_type: u64_type(),
            data: ConstData::UInt(u128::from(width)),
        })
    });
    let depth_code = assembler.constant(u32_value(u128::from(
        sley_check::TypeErrorCode::DepthLimit.numeric(),
    )));
    let width_code = assembler.constant(u32_value(u128::from(
        sley_check::TypeErrorCode::WidthInvalid.numeric(),
    )));
    let parameter_code = assembler.constant(u32_value(u128::from(
        sley_check::TypeErrorCode::ParameterOutOfScope.numeric(),
    )));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_check::TypeErrorCode::ResourceLimit.numeric(),
    )));

    let length = assembler.operation(
        entry,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(wrappers)],
        u64_type(),
        Immediate::None,
    );
    let maximum = assembler.constant_ref(entry, max_depth, u64_type());
    let depth_valid = assembler.operation(
        entry,
        Opcode::LessThan,
        vec![
            inventory_operation_value(length),
            inventory_operation_value(maximum),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    let start = assembler.constant_ref(entry, zero, u64_type());
    assembler.push_block(
        entry,
        function,
        Vec::new(),
        vec![length, maximum, depth_valid, start],
        inventory_cond(
            inventory_operation_value(depth_valid),
            wrapper_check,
            vec![inventory_operation_value(start)],
            depth_error,
            Vec::new(),
        ),
    );

    let check_index = assembler.parameter(wrapper_check, ParameterRole::Block, 0, u64_type());
    let check_length = assembler.operation(
        wrapper_check,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(wrappers)],
        u64_type(),
        Immediate::None,
    );
    let has_wrapper = assembler.operation(
        wrapper_check,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(check_index),
            inventory_operation_value(check_length),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        wrapper_check,
        function,
        vec![check_index],
        vec![check_length, has_wrapper],
        inventory_cond(
            inventory_operation_value(has_wrapper),
            wrapper_get,
            vec![ValueRef::Parameter(check_index)],
            leaf_bool,
            vec![inventory_operation_value(check_length)],
        ),
    );

    let get_index = assembler.parameter(wrapper_get, ParameterRole::Block, 0, u64_type());
    let wrapper = assembler.operation(
        wrapper_get,
        Opcode::VectorGet,
        vec![
            ValueRef::Parameter(wrappers),
            ValueRef::Parameter(get_index),
        ],
        TypeExpr::Option(Box::new(u64_type())),
        Immediate::None,
    );
    assembler.push_block(
        wrapper_get,
        function,
        vec![get_index],
        vec![wrapper],
        inventory_switch(
            inventory_operation_value(wrapper),
            vec![
                (BuiltinCase::None, invariant_trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    wrapper_unpack,
                    vec![
                        SwitchArgument::CasePayload,
                        SwitchArgument::Value(ValueRef::Parameter(get_index)),
                    ],
                ),
            ],
        ),
    );

    let found_wrapper = assembler.parameter(wrapper_unpack, ParameterRole::Block, 0, u64_type());
    let found_index = assembler.parameter(wrapper_unpack, ParameterRole::Block, 1, u64_type());
    assembler.push_block(
        wrapper_unpack,
        function,
        vec![found_wrapper, found_index],
        Vec::new(),
        inventory_branch(
            wrapper_vector,
            vec![
                ValueRef::Parameter(found_wrapper),
                ValueRef::Parameter(found_index),
            ],
        ),
    );

    let wrapper_tag_block = |assembler: &mut InventoryCheckAssembler,
                             block: EntityId,
                             expected: EntityId,
                             unmatched: EntityId| {
        let found = assembler.parameter(block, ParameterRole::Block, 0, u64_type());
        let index = assembler.parameter(block, ParameterRole::Block, 1, u64_type());
        let expected_value = assembler.constant_ref(block, expected, u64_type());
        let matches = assembler.operation(
            block,
            Opcode::Equal,
            vec![
                ValueRef::Parameter(found),
                inventory_operation_value(expected_value),
            ],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            vec![found, index],
            vec![expected_value, matches],
            inventory_cond(
                inventory_operation_value(matches),
                wrapper_advance,
                vec![ValueRef::Parameter(index)],
                unmatched,
                if unmatched == invariant_trap {
                    Vec::new()
                } else {
                    vec![ValueRef::Parameter(found), ValueRef::Parameter(index)]
                },
            ),
        );
    };
    wrapper_tag_block(&mut assembler, wrapper_vector, vector_tag, wrapper_option);
    wrapper_tag_block(&mut assembler, wrapper_option, option_tag, wrapper_cell);
    wrapper_tag_block(&mut assembler, wrapper_cell, cell_tag, invariant_trap);

    let advance_index = assembler.parameter(wrapper_advance, ParameterRole::Block, 0, u64_type());
    let advance_one = assembler.constant_ref(wrapper_advance, one, u64_type());
    let next_index = assembler.operation(
        wrapper_advance,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(advance_index),
            inventory_operation_value(advance_one),
        ],
        arithmetic_u64_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        wrapper_advance,
        function,
        vec![advance_index],
        vec![advance_one, next_index],
        inventory_switch(
            inventory_operation_value(next_index),
            vec![
                (
                    BuiltinCase::Ok,
                    wrapper_check,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );

    let leaf_tag_block = |assembler: &mut InventoryCheckAssembler,
                          block: EntityId,
                          expected: EntityId,
                          matched: EntityId,
                          unmatched: EntityId| {
        let wrappers_length = assembler.parameter(block, ParameterRole::Block, 0, u64_type());
        let expected_value = assembler.constant_ref(block, expected, u64_type());
        let matches = assembler.operation(
            block,
            Opcode::Equal,
            vec![
                ValueRef::Parameter(leaf_tag),
                inventory_operation_value(expected_value),
            ],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            vec![wrappers_length],
            vec![expected_value, matches],
            inventory_cond(
                inventory_operation_value(matches),
                matched,
                vec![ValueRef::Parameter(wrappers_length)],
                unmatched,
                if unmatched == invariant_trap {
                    Vec::new()
                } else {
                    vec![ValueRef::Parameter(wrappers_length)]
                },
            ),
        );
    };
    leaf_tag_block(&mut assembler, leaf_bool, bool_tag, success_add, leaf_uint);
    leaf_tag_block(&mut assembler, leaf_uint, uint_tag, width_8, leaf_parameter);
    leaf_tag_block(
        &mut assembler,
        leaf_parameter,
        parameter_tag,
        parameter_scope,
        invariant_trap,
    );

    let width_block = |assembler: &mut InventoryCheckAssembler,
                       block: EntityId,
                       expected: EntityId,
                       unmatched: EntityId| {
        let wrappers_length = assembler.parameter(block, ParameterRole::Block, 0, u64_type());
        let expected_value = assembler.constant_ref(block, expected, u64_type());
        let matches = assembler.operation(
            block,
            Opcode::Equal,
            vec![
                ValueRef::Parameter(leaf_payload),
                inventory_operation_value(expected_value),
            ],
            TypeExpr::Bool,
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            vec![wrappers_length],
            vec![expected_value, matches],
            inventory_cond(
                inventory_operation_value(matches),
                success_add,
                vec![ValueRef::Parameter(wrappers_length)],
                unmatched,
                if unmatched == width_error {
                    Vec::new()
                } else {
                    vec![ValueRef::Parameter(wrappers_length)]
                },
            ),
        );
    };
    width_block(&mut assembler, width_8, width_constants[0], width_16);
    width_block(&mut assembler, width_16, width_constants[1], width_32);
    width_block(&mut assembler, width_32, width_constants[2], width_64);
    width_block(&mut assembler, width_64, width_constants[3], width_128);
    width_block(&mut assembler, width_128, width_constants[4], width_error);

    let scope_length = assembler.parameter(parameter_scope, ParameterRole::Block, 0, u64_type());
    let parameter_in_scope = assembler.operation(
        parameter_scope,
        Opcode::LessThan,
        vec![
            ValueRef::Parameter(leaf_payload),
            ValueRef::Parameter(parameter_count),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        parameter_scope,
        function,
        vec![scope_length],
        vec![parameter_in_scope],
        inventory_cond(
            inventory_operation_value(parameter_in_scope),
            success_add,
            vec![ValueRef::Parameter(scope_length)],
            parameter_error,
            Vec::new(),
        ),
    );

    let success_length = assembler.parameter(success_add, ParameterRole::Block, 0, u64_type());
    let success_one = assembler.constant_ref(success_add, one, u64_type());
    let node_count = assembler.operation(
        success_add,
        Opcode::IntAddChecked,
        vec![
            ValueRef::Parameter(success_length),
            inventory_operation_value(success_one),
        ],
        arithmetic_u64_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success_add,
        function,
        vec![success_length],
        vec![success_one, node_count],
        inventory_switch(
            inventory_operation_value(node_count),
            vec![
                (
                    BuiltinCase::Ok,
                    success_emit,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource_error, Vec::new()),
            ],
        ),
    );
    let emitted_count = assembler.parameter(success_emit, ParameterRole::Block, 0, u64_type());
    let accepted = assembler.operation(
        success_emit,
        Opcode::ResultOk,
        vec![ValueRef::Parameter(emitted_count)],
        type_chain_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success_emit,
        function,
        vec![emitted_count],
        vec![accepted],
        Terminator::Return(ReturnTerminator {
            value: inventory_operation_value(accepted),
        }),
    );

    for (block, code) in [
        (depth_error, depth_code),
        (width_error, width_code),
        (parameter_error, parameter_code),
        (resource_error, resource_code),
    ] {
        let code_value = assembler.constant_ref(block, code, u32_type());
        let rejected = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![inventory_operation_value(code_value)],
            type_chain_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![code_value, rejected],
            Terminator::Return(ReturnTerminator {
                value: inventory_operation_value(rejected),
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
        parameters: vec![wrappers, leaf_tag, leaf_payload, parameter_count],
        result_type: type_chain_result_type(),
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

/// Bounded one-definition effect-closure judgment. Runtime identities and
/// presence facts determine resolution and the declared/computed closure.
#[allow(clippy::too_many_lines)]
fn single_effect_closure_checker() -> CheckerScaffold {
    let function = checker_inventory_id(5, 3);
    let mut assembler = InventoryCheckAssembler::new();
    let definition_present =
        assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bool);
    let definition_id = assembler.parameter(function, ParameterRole::Function, 1, u64_type());
    let declared_present =
        assembler.parameter(function, ParameterRole::Function, 2, TypeExpr::Bool);
    let declared_id = assembler.parameter(function, ParameterRole::Function, 3, u64_type());
    let request_present = assembler.parameter(function, ParameterRole::Function, 4, TypeExpr::Bool);
    let request_id = assembler.parameter(function, ParameterRole::Function, 5, u64_type());

    let entry = assembler.block_id();
    let declared_definition = assembler.block_id();
    let declared_identity = assembler.block_id();
    let request_presence = assembler.block_id();
    let request_definition = assembler.block_id();
    let request_identity = assembler.block_id();
    let closure_declared = assembler.block_id();
    let closure_request_required = assembler.block_id();
    let closure_request_absent = assembler.block_id();
    let success_empty = assembler.block_id();
    let success_one = assembler.block_id();
    let unresolved_error = assembler.block_id();
    let closure_error = assembler.block_id();

    let unresolved_code = assembler.constant(u32_value(u128::from(
        sley_check::effects::EffectErrorCode::UnresolvedEntity.numeric(),
    )));
    let closure_code = assembler.constant(u32_value(u128::from(
        sley_check::effects::EffectErrorCode::ClosureMismatch.numeric(),
    )));
    let zero_u32 = assembler.constant(u32_value(0));
    let one_u32 = assembler.constant(u32_value(1));
    let zero_u64 = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(0),
    });
    let one_u64 = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(1),
    });

    assembler.push_block(
        entry,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(declared_present),
            declared_definition,
            Vec::new(),
            request_presence,
            Vec::new(),
        ),
    );
    assembler.push_block(
        declared_definition,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(definition_present),
            declared_identity,
            Vec::new(),
            unresolved_error,
            Vec::new(),
        ),
    );
    let declared_matches = assembler.operation(
        declared_identity,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(declared_id),
            ValueRef::Parameter(definition_id),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        declared_identity,
        function,
        Vec::new(),
        vec![declared_matches],
        inventory_cond(
            inventory_operation_value(declared_matches),
            request_presence,
            Vec::new(),
            unresolved_error,
            Vec::new(),
        ),
    );

    assembler.push_block(
        request_presence,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(request_present),
            request_definition,
            Vec::new(),
            closure_declared,
            Vec::new(),
        ),
    );
    assembler.push_block(
        request_definition,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(definition_present),
            request_identity,
            Vec::new(),
            unresolved_error,
            Vec::new(),
        ),
    );
    let request_matches = assembler.operation(
        request_identity,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(request_id),
            ValueRef::Parameter(definition_id),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        request_identity,
        function,
        Vec::new(),
        vec![request_matches],
        inventory_cond(
            inventory_operation_value(request_matches),
            closure_declared,
            Vec::new(),
            unresolved_error,
            Vec::new(),
        ),
    );

    assembler.push_block(
        closure_declared,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(declared_present),
            closure_request_required,
            Vec::new(),
            closure_request_absent,
            Vec::new(),
        ),
    );
    assembler.push_block(
        closure_request_required,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(request_present),
            success_one,
            Vec::new(),
            closure_error,
            Vec::new(),
        ),
    );
    assembler.push_block(
        closure_request_absent,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(request_present),
            closure_error,
            Vec::new(),
            success_empty,
            Vec::new(),
        ),
    );

    let success_block = |assembler: &mut InventoryCheckAssembler,
                         block: EntityId,
                         count: EntityId,
                         work: EntityId| {
        let count_value = assembler.constant_ref(block, count, u64_type());
        let edges = assembler.constant_ref(block, zero_u32, u32_type());
        let rounds = assembler.constant_ref(block, one_u32, u32_type());
        let work_value = assembler.constant_ref(block, work, u64_type());
        let summary = assembler.operation(
            block,
            Opcode::TupleNew,
            vec![
                inventory_operation_value(count_value),
                inventory_operation_value(edges),
                inventory_operation_value(rounds),
                inventory_operation_value(work_value),
            ],
            effect_summary_type(),
            Immediate::None,
        );
        let accepted = assembler.operation(
            block,
            Opcode::ResultOk,
            vec![inventory_operation_value(summary)],
            effect_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![count_value, edges, rounds, work_value, summary, accepted],
            Terminator::Return(ReturnTerminator {
                value: inventory_operation_value(accepted),
            }),
        );
    };
    success_block(&mut assembler, success_empty, zero_u64, zero_u64);
    success_block(&mut assembler, success_one, one_u64, one_u64);

    for (block, code) in [
        (unresolved_error, unresolved_code),
        (closure_error, closure_code),
    ] {
        let code_value = assembler.constant_ref(block, code, u32_type());
        let rejected = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![inventory_operation_value(code_value)],
            effect_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![code_value, rejected],
            Terminator::Return(ReturnTerminator {
                value: inventory_operation_value(rejected),
            }),
        );
    }

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            definition_present,
            definition_id,
            declared_present,
            declared_id,
            request_present,
            request_id,
        ],
        result_type: effect_result_type(),
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

/// Walks arbitrary runtime effect-definition, declaration, and local-closure
/// inventories. The local-closure vector is the sorted/unique projection of
/// effect requests after operation scanning.
#[allow(clippy::too_many_lines)]
fn effect_set_inventory_checker() -> CheckerScaffold {
    let function = checker_inventory_id(5, 4);
    let mut assembler = InventoryCheckAssembler::new();
    let definitions = assembler.parameter(function, ParameterRole::Function, 0, u64vec_type());
    let declared = assembler.parameter(function, ParameterRole::Function, 1, u64vec_type());
    let local_closure = assembler.parameter(function, ParameterRole::Function, 2, u64vec_type());

    let success = assembler.block_id();
    let closure_compare = assembler.block_id();
    let definition_order_error = assembler.block_id();
    let declaration_order_error = assembler.block_id();
    let unresolved_error = assembler.block_id();
    let closure_error = assembler.block_id();
    let resource_error = assembler.block_id();
    let invariant_trap = assembler.block_id();

    let zero_u64 = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(0),
    });
    let one_u64 = assembler.constant(ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(1),
    });
    let zero_u32 = assembler.constant(u32_value(0));
    let one_u32 = assembler.constant(u32_value(1));
    let definition_order_code = assembler.constant(u32_value(u128::from(
        sley_check::effects::EffectErrorCode::SetNotCanonical.numeric(),
    )));
    let declaration_order_code = assembler.constant(u32_value(u128::from(
        sley_check::cfg::CfgErrorCode::GraphInventoryMismatch.numeric(),
    )));
    let unresolved_code = assembler.constant(u32_value(u128::from(
        sley_check::effects::EffectErrorCode::UnresolvedEntity.numeric(),
    )));
    let closure_code = assembler.constant(u32_value(u128::from(
        sley_check::effects::EffectErrorCode::ClosureMismatch.numeric(),
    )));
    let resource_code = assembler.constant(u32_value(u128::from(
        sley_check::effects::EffectErrorCode::ResourceLimit.numeric(),
    )));

    let request_membership = append_u64_membership_check(
        &mut assembler,
        function,
        local_closure,
        definitions,
        zero_u64,
        one_u64,
        closure_compare,
        unresolved_error,
        resource_error,
        invariant_trap,
    );
    let declared_membership = append_u64_membership_check(
        &mut assembler,
        function,
        declared,
        definitions,
        zero_u64,
        one_u64,
        request_membership,
        unresolved_error,
        resource_error,
        invariant_trap,
    );
    let declaration_order = append_sorted_u64_vector_check(
        &mut assembler,
        function,
        declared,
        zero_u64,
        one_u64,
        declared_membership,
        declaration_order_error,
        resource_error,
        invariant_trap,
    );
    let entry = append_sorted_u64_vector_check(
        &mut assembler,
        function,
        definitions,
        zero_u64,
        one_u64,
        declaration_order,
        definition_order_error,
        resource_error,
        invariant_trap,
    );

    let closures_match = assembler.operation(
        closure_compare,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(declared),
            ValueRef::Parameter(local_closure),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        closure_compare,
        function,
        Vec::new(),
        vec![closures_match],
        inventory_cond(
            inventory_operation_value(closures_match),
            success,
            Vec::new(),
            closure_error,
            Vec::new(),
        ),
    );

    let closure_count = assembler.operation(
        success,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(local_closure)],
        u64_type(),
        Immediate::None,
    );
    let edges = assembler.constant_ref(success, zero_u32, u32_type());
    let rounds = assembler.constant_ref(success, one_u32, u32_type());
    let closure_work = assembler.operation(
        success,
        Opcode::VectorLen,
        vec![ValueRef::Parameter(local_closure)],
        u64_type(),
        Immediate::None,
    );
    let summary = assembler.operation(
        success,
        Opcode::TupleNew,
        vec![
            inventory_operation_value(closure_count),
            inventory_operation_value(edges),
            inventory_operation_value(rounds),
            inventory_operation_value(closure_work),
        ],
        effect_summary_type(),
        Immediate::None,
    );
    let accepted = assembler.operation(
        success,
        Opcode::ResultOk,
        vec![inventory_operation_value(summary)],
        effect_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        success,
        function,
        Vec::new(),
        vec![
            closure_count,
            edges,
            rounds,
            closure_work,
            summary,
            accepted,
        ],
        Terminator::Return(ReturnTerminator {
            value: inventory_operation_value(accepted),
        }),
    );

    for (block, code) in [
        (definition_order_error, definition_order_code),
        (declaration_order_error, declaration_order_code),
        (unresolved_error, unresolved_code),
        (closure_error, closure_code),
        (resource_error, resource_code),
    ] {
        let code_value = assembler.constant_ref(block, code, u32_type());
        let rejected = assembler.operation(
            block,
            Opcode::ResultErr,
            vec![inventory_operation_value(code_value)],
            effect_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![code_value, rejected],
            Terminator::Return(ReturnTerminator {
                value: inventory_operation_value(rejected),
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
        parameters: vec![definitions, declared, local_closure],
        result_type: effect_result_type(),
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

/// Computes the least effect closure for a bounded two-function call graph.
/// Runtime facts control both local requests, the caller-to-callee edge, and
/// each declared closure.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn two_function_effect_closure_checker() -> CheckerScaffold {
    let function = checker_inventory_id(5, 5);
    let mut assembler = InventoryCheckAssembler::new();
    let caller_declared = assembler.parameter(function, ParameterRole::Function, 0, TypeExpr::Bool);
    let caller_local = assembler.parameter(function, ParameterRole::Function, 1, TypeExpr::Bool);
    let calls_callee = assembler.parameter(function, ParameterRole::Function, 2, TypeExpr::Bool);
    let callee_declared = assembler.parameter(function, ParameterRole::Function, 3, TypeExpr::Bool);
    let callee_local = assembler.parameter(function, ParameterRole::Function, 4, TypeExpr::Bool);

    let entry = assembler.block_id();
    let called_compute = assembler.block_id();
    let caller_compare = assembler.block_id();
    let callee_compare = assembler.block_id();
    let success_dispatch = assembler.block_id();
    let no_call_caller = assembler.block_id();
    let no_call_caller_false = assembler.block_id();
    let no_call_caller_true = assembler.block_id();
    let call_caller = assembler.block_id();
    let call_caller_false = assembler.block_id();
    let call_caller_true = assembler.block_id();
    let success_000 = assembler.block_id();
    let success_001 = assembler.block_id();
    let success_010 = assembler.block_id();
    let success_011 = assembler.block_id();
    let success_100 = assembler.block_id();
    let success_101 = assembler.block_id();
    let success_110 = assembler.block_id();
    let success_111 = assembler.block_id();
    let closure_error = assembler.block_id();

    let u64_constants = [0_u64, 1, 2, 4, 5].map(|value| {
        assembler.constant(ConstValue {
            value_type: u64_type(),
            data: ConstData::UInt(u128::from(value)),
        })
    });
    let zero_u32 = assembler.constant(u32_value(0));
    let one_u32 = assembler.constant(u32_value(1));
    let closure_code = assembler.constant(u32_value(u128::from(
        sley_check::effects::EffectErrorCode::ClosureMismatch.numeric(),
    )));

    assembler.push_block(
        entry,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(calls_callee),
            called_compute,
            Vec::new(),
            caller_compare,
            vec![ValueRef::Parameter(caller_local)],
        ),
    );
    let propagated = assembler.operation(
        called_compute,
        Opcode::BoolOr,
        vec![
            ValueRef::Parameter(caller_local),
            ValueRef::Parameter(callee_local),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        called_compute,
        function,
        Vec::new(),
        vec![propagated],
        inventory_branch(caller_compare, vec![inventory_operation_value(propagated)]),
    );

    let caller_closure =
        assembler.parameter(caller_compare, ParameterRole::Block, 0, TypeExpr::Bool);
    let caller_matches = assembler.operation(
        caller_compare,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(caller_declared),
            ValueRef::Parameter(caller_closure),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        caller_compare,
        function,
        vec![caller_closure],
        vec![caller_matches],
        inventory_cond(
            inventory_operation_value(caller_matches),
            callee_compare,
            vec![ValueRef::Parameter(caller_closure)],
            closure_error,
            Vec::new(),
        ),
    );

    let checked_caller_closure =
        assembler.parameter(callee_compare, ParameterRole::Block, 0, TypeExpr::Bool);
    let callee_matches = assembler.operation(
        callee_compare,
        Opcode::Equal,
        vec![
            ValueRef::Parameter(callee_declared),
            ValueRef::Parameter(callee_local),
        ],
        TypeExpr::Bool,
        Immediate::None,
    );
    assembler.push_block(
        callee_compare,
        function,
        vec![checked_caller_closure],
        vec![callee_matches],
        inventory_cond(
            inventory_operation_value(callee_matches),
            success_dispatch,
            vec![ValueRef::Parameter(checked_caller_closure)],
            closure_error,
            Vec::new(),
        ),
    );

    let accepted_caller_closure =
        assembler.parameter(success_dispatch, ParameterRole::Block, 0, TypeExpr::Bool);
    assembler.push_block(
        success_dispatch,
        function,
        vec![accepted_caller_closure],
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(calls_callee),
            call_caller,
            Vec::new(),
            no_call_caller,
            vec![ValueRef::Parameter(accepted_caller_closure)],
        ),
    );

    let no_call_closure =
        assembler.parameter(no_call_caller, ParameterRole::Block, 0, TypeExpr::Bool);
    assembler.push_block(
        no_call_caller,
        function,
        vec![no_call_closure],
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(no_call_closure),
            no_call_caller_true,
            Vec::new(),
            no_call_caller_false,
            Vec::new(),
        ),
    );
    assembler.push_block(
        no_call_caller_false,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(callee_local),
            success_001,
            Vec::new(),
            success_000,
            Vec::new(),
        ),
    );
    assembler.push_block(
        no_call_caller_true,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(callee_local),
            success_011,
            Vec::new(),
            success_010,
            Vec::new(),
        ),
    );
    assembler.push_block(
        call_caller,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(caller_local),
            call_caller_true,
            Vec::new(),
            call_caller_false,
            Vec::new(),
        ),
    );
    assembler.push_block(
        call_caller_false,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(callee_local),
            success_101,
            Vec::new(),
            success_100,
            Vec::new(),
        ),
    );
    assembler.push_block(
        call_caller_true,
        function,
        Vec::new(),
        Vec::new(),
        inventory_cond(
            ValueRef::Parameter(callee_local),
            success_111,
            Vec::new(),
            success_110,
            Vec::new(),
        ),
    );

    let mut success_block = |block: EntityId, count: usize, edges: EntityId, work: usize| {
        let count_value = assembler.constant_ref(block, u64_constants[count], u64_type());
        let edge_value = assembler.constant_ref(block, edges, u32_type());
        let round_value = assembler.constant_ref(block, one_u32, u32_type());
        let work_value = assembler.constant_ref(block, u64_constants[work], u64_type());
        let summary = assembler.operation(
            block,
            Opcode::TupleNew,
            vec![
                inventory_operation_value(count_value),
                inventory_operation_value(edge_value),
                inventory_operation_value(round_value),
                inventory_operation_value(work_value),
            ],
            effect_summary_type(),
            Immediate::None,
        );
        let accepted = assembler.operation(
            block,
            Opcode::ResultOk,
            vec![inventory_operation_value(summary)],
            effect_result_type(),
            Immediate::None,
        );
        assembler.push_block(
            block,
            function,
            Vec::new(),
            vec![
                count_value,
                edge_value,
                round_value,
                work_value,
                summary,
                accepted,
            ],
            Terminator::Return(ReturnTerminator {
                value: inventory_operation_value(accepted),
            }),
        );
    };
    success_block(success_000, 0, zero_u32, 0);
    success_block(success_001, 1, zero_u32, 1);
    success_block(success_010, 1, zero_u32, 1);
    success_block(success_011, 2, zero_u32, 2);
    success_block(success_100, 0, one_u32, 1);
    success_block(success_101, 2, one_u32, 4);
    success_block(success_110, 1, one_u32, 2);
    success_block(success_111, 2, one_u32, 3);

    let code_value = assembler.constant_ref(closure_error, closure_code, u32_type());
    let rejected = assembler.operation(
        closure_error,
        Opcode::ResultErr,
        vec![inventory_operation_value(code_value)],
        effect_result_type(),
        Immediate::None,
    );
    assembler.push_block(
        closure_error,
        function,
        Vec::new(),
        vec![code_value, rejected],
        Terminator::Return(ReturnTerminator {
            value: inventory_operation_value(rejected),
        }),
    );

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![
            caller_declared,
            caller_local,
            calls_callee,
            callee_declared,
            callee_local,
        ],
        result_type: effect_result_type(),
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
    admit_checker_program_with_bindings(scaffold, epoch(), root())
}

fn admit_checker_program_with_bindings(
    scaffold: &CheckerScaffold,
    schema_epoch: SchemaEpochId,
    state_root: StateRoot,
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
        schema_epoch,
        state_root,
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &scaffold.constants,
        globals: &[],
        functions: &scaffold.functions,
        contracts: &[],
        adapters: &[],
    })
    .expect("scaffold lowers under the reference lowerer");
    let gate = sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &scaffold.types,
        schema_epoch,
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
        schema_epoch,
        state_root,
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
        schema_epoch,
        state_root,
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

#[derive(Clone, Copy, Debug)]
struct OptionSwitchFacts {
    target_valid: FactState,
    selector_type: u32,
    case_count: u32,
    first_key: u32,
    second_key: u32,
    none_uses_payload: FactState,
    argument_types_match: FactState,
}

impl OptionSwitchFacts {
    fn valid() -> Self {
        Self {
            target_valid: FactState::Valid,
            selector_type: TypeExpr::Option(Box::new(TypeExpr::Bool)).tag(),
            case_count: 2,
            first_key: BuiltinCase::None.tag(),
            second_key: BuiltinCase::Some.tag(),
            none_uses_payload: FactState::Invalid,
            argument_types_match: FactState::Valid,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SingleEffectFacts {
    definition_present: bool,
    definition_id: u64,
    declared_present: bool,
    declared_id: u64,
    request_present: bool,
    request_id: u64,
}

impl SingleEffectFacts {
    const EMPTY: Self = Self {
        definition_present: false,
        definition_id: 20,
        declared_present: false,
        declared_id: 20,
        request_present: false,
        request_id: 20,
    };

    const ONE: Self = Self {
        definition_present: true,
        definition_id: 20,
        declared_present: true,
        declared_id: 20,
        request_present: true,
        request_id: 20,
    };
}

type EffectSetCase<'a> = (&'a [u64], &'a [u64], &'a [u64], u32);

#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_excessive_bools)]
struct TwoFunctionEffectFacts {
    caller_declared: bool,
    caller_local: bool,
    calls_callee: bool,
    callee_declared: bool,
    callee_local: bool,
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

fn execute_option_switch_cfg(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    facts: OptionSwitchFacts,
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
                bool_value(facts.target_valid.is_valid()),
                u32_value(u128::from(facts.selector_type)),
                u32_value(u128::from(facts.case_count)),
                u32_value(u128::from(facts.first_key)),
                u32_value(u128::from(facts.second_key)),
                bool_value(facts.none_uses_payload.is_valid()),
                bool_value(facts.argument_types_match.is_valid()),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes Option-switch CFG checker")
}

fn execute_operation_inventory(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    rows: &[CfgInventoryRow],
    parameter_count: u64,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                cfg_inventory_value(rows),
                ConstValue {
                    value_type: u64_type(),
                    data: ConstData::UInt(u128::from(parameter_count)),
                },
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes ordered operation-inventory checker")
}

fn execute_type_chain(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    wrappers: &[u64],
    leaf_tag: u64,
    leaf_payload: u64,
    parameter_count: u64,
) -> sley_vm::ExecutionOutcome {
    let u64_value = |value| ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::from(value)),
    };
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                u64vec_value(wrappers),
                u64_value(leaf_tag),
                u64_value(leaf_payload),
                u64_value(parameter_count),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes unary type-chain checker")
}

fn execute_single_effect(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    facts: SingleEffectFacts,
) -> sley_vm::ExecutionOutcome {
    let bool_value = |value| ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    };
    let u64_value = |value| ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::from(value)),
    };
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                bool_value(facts.definition_present),
                u64_value(facts.definition_id),
                bool_value(facts.declared_present),
                u64_value(facts.declared_id),
                bool_value(facts.request_present),
                u64_value(facts.request_id),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes single-effect closure checker")
}

fn execute_effect_set_inventory(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    definitions: &[u64],
    declared: &[u64],
    local_closure: &[u64],
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                u64vec_value(definitions),
                u64vec_value(declared),
                u64vec_value(local_closure),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes effect-set inventory checker")
}

fn execute_two_function_effect_closure(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    facts: TwoFunctionEffectFacts,
) -> sley_vm::ExecutionOutcome {
    let value = |value| ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    };
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![
                value(facts.caller_declared),
                value(facts.caller_local),
                value(facts.calls_callee),
                value(facts.callee_declared),
                value(facts.callee_local),
            ],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes two-function effect-closure checker")
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

#[allow(clippy::too_many_lines)]
fn native_option_switch_cfg(
    facts: OptionSwitchFacts,
) -> Result<sley_check::cfg::CfgReport, sley_check::cfg::CfgErrorCode> {
    use sley_check::cfg::{CfgValidationError, validate_function_graph};

    let function_id = id(1);
    let entry_id = id(2);
    let operation_id = id(3);
    let none_target = id(4);
    let some_target = id(5);
    let missing_target = id(99);
    let left = id(10);
    let right = id(11);
    let none_parameter = id(12);
    let some_parameter = id(13);
    let selector_is_option =
        facts.selector_type == TypeExpr::Option(Box::new(TypeExpr::Bool)).tag();
    let selector = ValueRef::OperationResult(OperationResultRef {
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
            value_type: if facts.argument_types_match.is_valid() {
                TypeExpr::Bool
            } else {
                u32_type()
            },
        },
        Parameter {
            entity_id: some_parameter,
            owner: some_target,
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: TypeExpr::Bool,
        },
    ];
    let operation = Operation {
        entity_id: operation_id,
        block: entry_id,
        ordinal: 0,
        opcode: if selector_is_option {
            Opcode::OptionSome
        } else {
            Opcode::BoolNot
        },
        operands: vec![ValueRef::Parameter(left)],
        result_types: vec![if selector_is_option {
            TypeExpr::Option(Box::new(TypeExpr::Bool))
        } else {
            TypeExpr::Bool
        }],
        immediate: Immediate::None,
    };
    let key = |tag| match tag {
        1 => CaseKey::Builtin(BuiltinCase::None),
        2 => CaseKey::Builtin(BuiltinCase::Some),
        3 => CaseKey::Builtin(BuiltinCase::Ok),
        4 => CaseKey::Builtin(BuiltinCase::Err),
        other => panic!("Option-switch fixture key tag must be frozen, got {other}"),
    };
    let mut cases = vec![
        SwitchCase {
            case_key: key(facts.first_key),
            edge: SwitchEdge {
                target: none_target,
                arguments: vec![if facts.none_uses_payload.is_valid() {
                    SwitchArgument::CasePayload
                } else {
                    SwitchArgument::Value(ValueRef::Parameter(right))
                }],
            },
        },
        SwitchCase {
            case_key: key(facts.second_key),
            edge: SwitchEdge {
                target: if facts.target_valid.is_valid() {
                    some_target
                } else {
                    missing_target
                },
                arguments: vec![SwitchArgument::CasePayload],
            },
        },
    ];
    if facts.case_count == 3 {
        cases.push(SwitchCase {
            case_key: CaseKey::Builtin(BuiltinCase::Some),
            edge: SwitchEdge {
                target: some_target,
                arguments: vec![SwitchArgument::CasePayload],
            },
        });
    } else if facts.case_count != 2 {
        panic!("bounded Option-switch fixture supports two or three cases")
    }
    let blocks = vec![
        Block {
            entity_id: entry_id,
            function: function_id,
            parameters: Vec::new(),
            operations: vec![operation_id],
            terminator: Terminator::VariantSwitch(VariantSwitchTerminator {
                value: selector,
                cases,
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
            parameters: vec![some_parameter],
            operations: Vec::new(),
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::Parameter(some_parameter),
            }),
            reachability: Reachability::Required,
        },
    ];
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    validate_function_graph(&types, &function, &parameters, &blocks, &[operation]).map_err(
        |error| match error {
            CfgValidationError::Cfg(error) => error.code(),
            CfgValidationError::Type(error) => {
                panic!("Option-switch reference must not reach type error: {error}")
            }
        },
    )
}

#[allow(clippy::too_many_lines)]
fn native_operation_inventory_cfg(
    rows: &[CfgInventoryRow],
    parameter_count: u64,
) -> Result<sley_check::cfg::CfgReport, sley_check::cfg::CfgErrorCode> {
    use sley_check::cfg::{CfgValidationError, validate_function_graph};

    let function_id = id(1);
    let block_id = id(2);
    let parameter_ids: Vec<_> = (0..parameter_count)
        .map(|index| {
            checker_inventory_id(
                10,
                u16::try_from(index + 1).expect("bounded native parameter index"),
            )
        })
        .collect();
    let operation_ids: Vec<_> = (0..rows.len())
        .map(|index| {
            checker_inventory_id(
                11,
                u16::try_from(index + 1).expect("bounded native operation index"),
            )
        })
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
    let parameters = parameter_ids
        .iter()
        .enumerate()
        .map(|(ordinal, entity_id)| Parameter {
            entity_id: *entity_id,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: u32::try_from(ordinal).expect("bounded native parameter ordinal"),
            value_type: TypeExpr::Bool,
        })
        .collect::<Vec<_>>();
    let operations = rows
        .iter()
        .enumerate()
        .map(|(index, (ordinal, reference_kind, reference_index))| {
            let operand = match *reference_kind {
                1 => parameter_ids
                    .get(usize::try_from(*reference_index).unwrap_or(usize::MAX))
                    .copied()
                    .map_or(ValueRef::Parameter(id(98)), ValueRef::Parameter),
                2 => operation_ids
                    .get(usize::try_from(*reference_index).unwrap_or(usize::MAX))
                    .copied()
                    .map_or(
                        ValueRef::OperationResult(OperationResultRef {
                            operation: id(97),
                            result_index: 0,
                        }),
                        |operation| {
                            ValueRef::OperationResult(OperationResultRef {
                                operation,
                                result_index: 0,
                            })
                        },
                    ),
                _ => ValueRef::Parameter(id(96)),
            };
            Operation {
                entity_id: operation_ids[index],
                block: block_id,
                ordinal: u32::try_from(*ordinal).expect("bounded native operation ordinal"),
                opcode: Opcode::BoolNot,
                operands: vec![operand],
                result_types: vec![TypeExpr::Bool],
                immediate: Immediate::None,
            }
        })
        .collect::<Vec<_>>();
    let return_value = parameter_ids
        .first()
        .copied()
        .map(ValueRef::Parameter)
        .expect("bounded inventory fixture has a parameter");
    let block = Block {
        entity_id: block_id,
        function: function_id,
        parameters: Vec::new(),
        operations: operation_ids,
        terminator: Terminator::Return(ReturnTerminator {
            value: return_value,
        }),
        reachability: Reachability::Required,
    };
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    validate_function_graph(&types, &function, &parameters, &[block], &operations).map_err(
        |error| match error {
            CfgValidationError::Cfg(error) => error.code(),
            CfgValidationError::Type(error) => {
                panic!("operation-inventory reference must not reach type error: {error}")
            }
        },
    )
}

fn native_unary_type_chain(
    wrappers: &[u64],
    leaf_tag: u64,
    leaf_payload: u64,
    parameter_count: u64,
) -> Result<u64, sley_check::TypeErrorCode> {
    let mut value = match u32::try_from(leaf_tag).expect("bounded leaf tag") {
        tag if tag == TypeExpr::Bool.tag() => TypeExpr::Bool,
        tag if tag == u32_type().tag() => TypeExpr::UInt(IntegerWidth::from_bits(
            u16::try_from(leaf_payload).expect("bounded width payload"),
        )),
        tag if tag == TypeExpr::TypeParameter(0).tag() => {
            TypeExpr::TypeParameter(u32::try_from(leaf_payload).expect("bounded parameter payload"))
        }
        other => panic!("bounded type-chain leaf tag is unsupported: {other}"),
    };
    for wrapper in wrappers.iter().rev() {
        value = match u32::try_from(*wrapper).expect("bounded wrapper tag") {
            tag if tag == TypeExpr::Vector(Box::new(TypeExpr::Bool)).tag() => {
                TypeExpr::Vector(Box::new(value))
            }
            tag if tag == TypeExpr::Option(Box::new(TypeExpr::Bool)).tag() => {
                TypeExpr::Option(Box::new(value))
            }
            tag if tag == TypeExpr::LocalCell(Box::new(TypeExpr::Bool)).tag() => {
                TypeExpr::LocalCell(Box::new(value))
            }
            other => panic!("bounded type-chain wrapper tag is unsupported: {other}"),
        };
    }
    let environment = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    environment
        .check_type(
            &value,
            u32::try_from(parameter_count).expect("bounded parameter count"),
        )
        .map(|()| u64::try_from(wrappers.len() + 1).expect("bounded node count"))
        .map_err(|error| error.code())
}

fn assert_type_chain_ok(outcome: &sley_vm::ExecutionOutcome, expected_nodes: u64) {
    use sley_ssmc::ResultConst;
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("type-chain checker must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(nodes)) = &value.data else {
        panic!("type-chain checker must return Ok, got {:?}", value.data)
    };
    assert_eq!(nodes.data, ConstData::UInt(u128::from(expected_nodes)));
}

fn assert_type_chain_error(
    outcome: &sley_vm::ExecutionOutcome,
    expected: sley_check::TypeErrorCode,
) {
    use sley_ssmc::ResultConst;
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("type-chain checker must terminate with a value")
    };
    let ConstData::Result(ResultConst::Err(code)) = &value.data else {
        panic!("type-chain checker must return Err, got {:?}", value.data)
    };
    assert_eq!(code.data, ConstData::UInt(u128::from(expected.numeric())));
}

fn effect_entity_id(value: u64) -> EntityId {
    let mut bytes = [0_u8; 32];
    bytes[0] = 0xee;
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    EntityId::from_bytes(bytes)
}

fn native_single_effect_closure(
    facts: SingleEffectFacts,
) -> Result<sley_check::effects::EffectReport, sley_check::effects::EffectErrorCode> {
    use sley_check::effects::{EffectValidationError, FunctionUnit, validate_effect_program};

    let function_id = id(1);
    let block_id = id(2);
    let scope = id(3);
    let request = id(4);
    let operation_id = id(5);
    let function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: vec![scope, request],
        result_type: TypeExpr::Unit,
        effects: facts
            .declared_present
            .then(|| effect_entity_id(facts.declared_id))
            .into_iter()
            .collect(),
        entry_block: block_id,
        blocks: vec![block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = vec![
        Parameter {
            entity_id: scope,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Unit,
        },
        Parameter {
            entity_id: request,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: TypeExpr::Unit,
        },
    ];
    let operations = facts
        .request_present
        .then(|| Operation {
            entity_id: operation_id,
            block: block_id,
            ordinal: 0,
            opcode: Opcode::EffectRequest,
            operands: vec![ValueRef::Parameter(scope), ValueRef::Parameter(request)],
            result_types: vec![TypeExpr::Result {
                ok: Box::new(TypeExpr::Unit),
                error: Box::new(TypeExpr::Unit),
            }],
            immediate: Immediate::Entity(effect_entity_id(facts.request_id)),
        })
        .into_iter()
        .collect::<Vec<_>>();
    let block = Block {
        entity_id: block_id,
        function: function_id,
        parameters: Vec::new(),
        operations: operations
            .iter()
            .map(|operation| operation.entity_id)
            .collect(),
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::Parameter(scope),
        }),
        reachability: Reachability::Required,
    };
    let effects = facts
        .definition_present
        .then(|| EffectDefinition {
            entity_id: effect_entity_id(facts.definition_id),
            effect_kind: EffectKind::StdoutWrite,
            scope_type: TypeExpr::Unit,
            request_type: TypeExpr::Unit,
            response_type: TypeExpr::Unit,
            failure_type: TypeExpr::Unit,
            visibility: Visibility::Private,
        })
        .into_iter()
        .collect::<Vec<_>>();
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    let unit = FunctionUnit {
        function: &function,
        parameters: &parameters,
        blocks: std::slice::from_ref(&block),
        operations: &operations,
    };
    validate_effect_program(&types, &[unit], &effects, &[], &[], &[]).map_err(|error| match error {
        EffectValidationError::Effect(error) => error.code(),
        earlier => panic!("single-effect reference reached earlier error: {earlier}"),
    })
}

fn native_effect_set_inventory(
    definitions: &[u64],
    declared: &[u64],
    local_closure: &[u64],
) -> Result<sley_check::effects::EffectReport, u32> {
    use sley_check::{
        cfg::CfgValidationError,
        effects::{EffectValidationError, FunctionUnit, validate_effect_program},
    };

    let function_id = id(1);
    let block_id = id(2);
    let scope = id(3);
    let request = id(4);
    let function = FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: vec![scope, request],
        result_type: TypeExpr::Unit,
        effects: declared.iter().copied().map(effect_entity_id).collect(),
        entry_block: block_id,
        blocks: vec![block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = vec![
        Parameter {
            entity_id: scope,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Unit,
        },
        Parameter {
            entity_id: request,
            owner: function_id,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: TypeExpr::Unit,
        },
    ];
    let operations = local_closure
        .iter()
        .copied()
        .enumerate()
        .map(|(ordinal, effect)| Operation {
            entity_id: checker_inventory_id(
                9,
                u16::try_from(ordinal + 1).expect("bounded effect operation identity"),
            ),
            block: block_id,
            ordinal: u32::try_from(ordinal).expect("bounded effect operation ordinal"),
            opcode: Opcode::EffectRequest,
            operands: vec![ValueRef::Parameter(scope), ValueRef::Parameter(request)],
            result_types: vec![TypeExpr::Result {
                ok: Box::new(TypeExpr::Unit),
                error: Box::new(TypeExpr::Unit),
            }],
            immediate: Immediate::Entity(effect_entity_id(effect)),
        })
        .collect::<Vec<_>>();
    let block = Block {
        entity_id: block_id,
        function: function_id,
        parameters: Vec::new(),
        operations: operations
            .iter()
            .map(|operation| operation.entity_id)
            .collect(),
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::Parameter(scope),
        }),
        reachability: Reachability::Required,
    };
    let effects = definitions
        .iter()
        .copied()
        .map(|effect| EffectDefinition {
            entity_id: effect_entity_id(effect),
            effect_kind: EffectKind::StdoutWrite,
            scope_type: TypeExpr::Unit,
            request_type: TypeExpr::Unit,
            response_type: TypeExpr::Unit,
            failure_type: TypeExpr::Unit,
            visibility: Visibility::Private,
        })
        .collect::<Vec<_>>();
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    let unit = FunctionUnit {
        function: &function,
        parameters: &parameters,
        blocks: std::slice::from_ref(&block),
        operations: &operations,
    };
    validate_effect_program(&types, &[unit], &effects, &[], &[], &[]).map_err(|error| match error {
        EffectValidationError::Type(error)
        | EffectValidationError::Cfg(CfgValidationError::Type(error)) => error.code().numeric(),
        EffectValidationError::Cfg(CfgValidationError::Cfg(error)) => error.code().numeric(),
        EffectValidationError::Effect(error) => error.code().numeric(),
    })
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn native_two_function_effect_closure(
    facts: TwoFunctionEffectFacts,
) -> Result<sley_check::effects::EffectReport, u32> {
    use sley_check::{
        cfg::CfgValidationError,
        effects::{EffectValidationError, FunctionUnit, validate_effect_program},
    };

    let effect = effect_entity_id(20);
    let caller_id = id(1);
    let caller_scope = id(2);
    let caller_request = id(3);
    let caller_block_id = id(4);
    let callee_id = id(10);
    let callee_scope = id(11);
    let callee_request = id(12);
    let callee_block_id = id(13);
    let result_type = TypeExpr::Result {
        ok: Box::new(TypeExpr::Unit),
        error: Box::new(TypeExpr::Unit),
    };

    let caller = FunctionGraph {
        entity_id: caller_id,
        type_parameters: Vec::new(),
        parameters: vec![caller_scope, caller_request],
        result_type: TypeExpr::Unit,
        effects: facts
            .caller_declared
            .then_some(effect)
            .into_iter()
            .collect(),
        entry_block: caller_block_id,
        blocks: vec![caller_block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let callee = FunctionGraph {
        entity_id: callee_id,
        type_parameters: Vec::new(),
        parameters: vec![callee_scope, callee_request],
        result_type: TypeExpr::Unit,
        effects: facts
            .callee_declared
            .then_some(effect)
            .into_iter()
            .collect(),
        entry_block: callee_block_id,
        blocks: vec![callee_block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let caller_parameters = vec![
        Parameter {
            entity_id: caller_scope,
            owner: caller_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Unit,
        },
        Parameter {
            entity_id: caller_request,
            owner: caller_id,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: TypeExpr::Unit,
        },
    ];
    let callee_parameters = vec![
        Parameter {
            entity_id: callee_scope,
            owner: callee_id,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Unit,
        },
        Parameter {
            entity_id: callee_request,
            owner: callee_id,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: TypeExpr::Unit,
        },
    ];

    let mut caller_operations = Vec::new();
    if facts.caller_local {
        caller_operations.push(Operation {
            entity_id: id(5),
            block: caller_block_id,
            ordinal: u32::try_from(caller_operations.len()).expect("bounded caller ordinal"),
            opcode: Opcode::EffectRequest,
            operands: vec![
                ValueRef::Parameter(caller_scope),
                ValueRef::Parameter(caller_request),
            ],
            result_types: vec![result_type.clone()],
            immediate: Immediate::Entity(effect),
        });
    }
    if facts.calls_callee {
        caller_operations.push(Operation {
            entity_id: id(6),
            block: caller_block_id,
            ordinal: u32::try_from(caller_operations.len()).expect("bounded caller ordinal"),
            opcode: Opcode::CallDirect,
            operands: vec![
                ValueRef::Parameter(caller_scope),
                ValueRef::Parameter(caller_request),
            ],
            result_types: vec![TypeExpr::Unit],
            immediate: Immediate::Function(FunctionRefValue {
                function: callee_id,
                type_arguments: Vec::new(),
            }),
        });
    }
    let callee_operations = facts
        .callee_local
        .then(|| Operation {
            entity_id: id(14),
            block: callee_block_id,
            ordinal: 0,
            opcode: Opcode::EffectRequest,
            operands: vec![
                ValueRef::Parameter(callee_scope),
                ValueRef::Parameter(callee_request),
            ],
            result_types: vec![result_type],
            immediate: Immediate::Entity(effect),
        })
        .into_iter()
        .collect::<Vec<_>>();
    let caller_block = Block {
        entity_id: caller_block_id,
        function: caller_id,
        parameters: Vec::new(),
        operations: caller_operations
            .iter()
            .map(|operation| operation.entity_id)
            .collect(),
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::Parameter(caller_scope),
        }),
        reachability: Reachability::Required,
    };
    let callee_block = Block {
        entity_id: callee_block_id,
        function: callee_id,
        parameters: Vec::new(),
        operations: callee_operations
            .iter()
            .map(|operation| operation.entity_id)
            .collect(),
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::Parameter(callee_scope),
        }),
        reachability: Reachability::Required,
    };
    let definition = EffectDefinition {
        entity_id: effect,
        effect_kind: EffectKind::StdoutWrite,
        scope_type: TypeExpr::Unit,
        request_type: TypeExpr::Unit,
        response_type: TypeExpr::Unit,
        failure_type: TypeExpr::Unit,
        visibility: Visibility::Private,
    };
    let caller_unit = FunctionUnit {
        function: &caller,
        parameters: &caller_parameters,
        blocks: std::slice::from_ref(&caller_block),
        operations: &caller_operations,
    };
    let callee_unit = FunctionUnit {
        function: &callee,
        parameters: &callee_parameters,
        blocks: std::slice::from_ref(&callee_block),
        operations: &callee_operations,
    };
    let types = sley_check::TypeEnvironment::new(Vec::new()).unwrap();
    validate_effect_program(
        &types,
        &[caller_unit, callee_unit],
        &[definition],
        &[],
        &[],
        &[],
    )
    .map_err(|error| match error {
        EffectValidationError::Type(error)
        | EffectValidationError::Cfg(CfgValidationError::Type(error)) => error.code().numeric(),
        EffectValidationError::Cfg(CfgValidationError::Cfg(error)) => error.code().numeric(),
        EffectValidationError::Effect(error) => error.code().numeric(),
    })
}

fn assert_effect_ok(
    outcome: &sley_vm::ExecutionOutcome,
    report: &sley_check::effects::EffectReport,
) {
    use sley_ssmc::ResultConst;
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("effect checker must terminate with a value")
    };
    let ConstData::Result(ResultConst::Ok(summary)) = &value.data else {
        panic!("effect checker must return Ok, got {:?}", value.data)
    };
    let ConstData::Sequence(fields) = &summary.data else {
        panic!("effect summary must be a tuple")
    };
    let closure_count = report
        .functions
        .iter()
        .map(|function| function.effects.len() as u128)
        .sum();
    assert_eq!(fields[0].data, ConstData::UInt(closure_count));
    assert_eq!(
        fields[1].data,
        ConstData::UInt(u128::from(report.call_edges))
    );
    assert_eq!(
        fields[2].data,
        ConstData::UInt(u128::from(report.closure_rounds))
    );
    assert_eq!(
        fields[3].data,
        ConstData::UInt(u128::from(report.closure_work))
    );
}

fn assert_effect_error(
    outcome: &sley_vm::ExecutionOutcome,
    expected: sley_check::effects::EffectErrorCode,
) {
    assert_effect_numeric_error(outcome, expected.numeric());
}

fn assert_effect_numeric_error(outcome: &sley_vm::ExecutionOutcome, expected: u32) {
    use sley_ssmc::ResultConst;
    let sley_vm::ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("effect checker must terminate with a value")
    };
    let ConstData::Result(ResultConst::Err(code)) = &value.data else {
        panic!("effect checker must return Err, got {:?}", value.data)
    };
    assert_eq!(code.data, ConstData::UInt(u128::from(expected)));
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
fn checker_option_switch_cfg_matches_native_report() {
    let facts = OptionSwitchFacts::valid();
    let (package, approved) = admit_checker_program(&option_switch_cfg_checker());
    let native = native_option_switch_cfg(facts).expect("native checker accepts Option switch");
    let first = execute_option_switch_cfg(&package, &approved, facts);
    let second = execute_option_switch_cfg(&package, &approved, facts);
    assert_cfg_ok(&first, &native);
    assert_eq!(first.termination, second.termination);
}

#[test]
fn checker_option_switch_cfg_preserves_native_first_failure_codes() {
    use sley_check::cfg::CfgErrorCode;

    let valid = OptionSwitchFacts::valid();
    let (package, approved) = admit_checker_program(&option_switch_cfg_checker());
    let cases = [
        (
            OptionSwitchFacts {
                target_valid: FactState::Invalid,
                ..valid
            },
            CfgErrorCode::TargetInvalid,
        ),
        (
            OptionSwitchFacts {
                selector_type: TypeExpr::Bool.tag(),
                ..valid
            },
            CfgErrorCode::SwitchType,
        ),
        (
            OptionSwitchFacts {
                case_count: 3,
                ..valid
            },
            CfgErrorCode::SwitchCases,
        ),
        (
            OptionSwitchFacts {
                first_key: BuiltinCase::Some.tag(),
                ..valid
            },
            CfgErrorCode::SwitchCases,
        ),
        (
            OptionSwitchFacts {
                second_key: BuiltinCase::None.tag(),
                ..valid
            },
            CfgErrorCode::SwitchCases,
        ),
        (
            OptionSwitchFacts {
                none_uses_payload: FactState::Valid,
                ..valid
            },
            CfgErrorCode::SwitchPayload,
        ),
        (
            OptionSwitchFacts {
                argument_types_match: FactState::Invalid,
                ..valid
            },
            CfgErrorCode::TargetArguments,
        ),
        (
            OptionSwitchFacts {
                target_valid: FactState::Invalid,
                selector_type: TypeExpr::Bool.tag(),
                case_count: 3,
                ..valid
            },
            CfgErrorCode::TargetInvalid,
        ),
        (
            OptionSwitchFacts {
                selector_type: TypeExpr::Bool.tag(),
                case_count: 3,
                none_uses_payload: FactState::Valid,
                ..valid
            },
            CfgErrorCode::SwitchType,
        ),
        (
            OptionSwitchFacts {
                case_count: 3,
                none_uses_payload: FactState::Valid,
                argument_types_match: FactState::Invalid,
                ..valid
            },
            CfgErrorCode::SwitchCases,
        ),
        (
            OptionSwitchFacts {
                none_uses_payload: FactState::Valid,
                argument_types_match: FactState::Invalid,
                ..valid
            },
            CfgErrorCode::SwitchPayload,
        ),
    ];
    for (facts, expected) in cases {
        assert_eq!(
            native_option_switch_cfg(facts),
            Err(expected),
            "native Option-switch oracle"
        );
        assert_cfg_error(
            &execute_option_switch_cfg(&package, &approved, facts),
            expected,
        );
    }
}

#[test]
fn checker_operation_inventory_matches_native_report() {
    let rows = [(0, 1, 0), (1, 2, 0), (2, 2, 1)];
    let (package, approved) = admit_checker_program(&ordered_operation_inventory_checker());
    let native = native_operation_inventory_cfg(&rows, 2).expect("native inventory accepts");
    let first = execute_operation_inventory(&package, &approved, &rows, 2);
    let second = execute_operation_inventory(&package, &approved, &rows, 2);
    assert_cfg_ok(&first, &native);
    assert_eq!(first.termination, second.termination);

    let empty_native = native_operation_inventory_cfg(&[], 2).expect("empty inventory accepts");
    assert_cfg_ok(
        &execute_operation_inventory(&package, &approved, &[], 2),
        &empty_native,
    );
}

#[test]
fn checker_operation_inventory_preserves_two_pass_precedence() {
    use sley_check::cfg::CfgErrorCode;

    let (package, approved) = admit_checker_program(&ordered_operation_inventory_checker());
    let cases: &[(&[CfgInventoryRow], CfgErrorCode)] = &[
        (&[(0, 1, 9), (2, 1, 0)], CfgErrorCode::GraphOrdinalMismatch),
        (&[(0, 1, 9)], CfgErrorCode::ValueUnresolved),
        (&[(0, 3, 0)], CfgErrorCode::ValueUnresolved),
        (&[(0, 2, 1), (1, 1, 0)], CfgErrorCode::UseBeforeDefinition),
        (&[(0, 1, 0), (1, 2, 1)], CfgErrorCode::UseBeforeDefinition),
        (&[(0, 2, 3), (1, 1, 0)], CfgErrorCode::ValueUnresolved),
        (
            &[(0, 1, 0), (1, 1, 9), (2, 2, 2)],
            CfgErrorCode::ValueUnresolved,
        ),
    ];
    for (rows, expected) in cases {
        assert_eq!(
            native_operation_inventory_cfg(rows, 2),
            Err(*expected),
            "native operation-inventory oracle"
        );
        assert_cfg_error(
            &execute_operation_inventory(&package, &approved, rows, 2),
            *expected,
        );
    }
}

#[test]
fn checker_unary_type_chains_match_native_type_judgment() {
    let vector_tag = u64::from(TypeExpr::Vector(Box::new(TypeExpr::Bool)).tag());
    let option_tag = u64::from(TypeExpr::Option(Box::new(TypeExpr::Bool)).tag());
    let cell_tag = u64::from(TypeExpr::LocalCell(Box::new(TypeExpr::Bool)).tag());
    let uint_tag = u64::from(u32_type().tag());
    let bool_tag = u64::from(TypeExpr::Bool.tag());
    let parameter_tag = u64::from(TypeExpr::TypeParameter(0).tag());
    let (package, approved) = admit_checker_program(&unary_type_chain_checker());

    for (wrappers, leaf_tag, payload, parameters) in [
        (vec![vector_tag, option_tag, cell_tag], uint_tag, 32, 0),
        (Vec::new(), bool_tag, 0, 0),
        (vec![option_tag], parameter_tag, 1, 2),
    ] {
        let native = native_unary_type_chain(&wrappers, leaf_tag, payload, parameters)
            .expect("native type checker accepts chain");
        let first = execute_type_chain(
            &package, &approved, &wrappers, leaf_tag, payload, parameters,
        );
        let second = execute_type_chain(
            &package, &approved, &wrappers, leaf_tag, payload, parameters,
        );
        assert_type_chain_ok(&first, native);
        assert_eq!(first.termination, second.termination);
    }

    let boundary = vec![vector_tag; MAX_TYPE_DEPTH - 1];
    let native = native_unary_type_chain(&boundary, bool_tag, 0, 0)
        .expect("maximum legal type depth accepts");
    assert_type_chain_ok(
        &execute_type_chain(&package, &approved, &boundary, bool_tag, 0, 0),
        native,
    );
}

#[test]
fn checker_unary_type_chains_preserve_depth_and_leaf_errors() {
    use sley_check::TypeErrorCode;

    let vector_tag = u64::from(TypeExpr::Vector(Box::new(TypeExpr::Bool)).tag());
    let uint_tag = u64::from(u32_type().tag());
    let parameter_tag = u64::from(TypeExpr::TypeParameter(0).tag());
    let (package, approved) = admit_checker_program(&unary_type_chain_checker());
    let cases = [
        (Vec::new(), uint_tag, 24, 0, TypeErrorCode::WidthInvalid),
        (
            Vec::new(),
            parameter_tag,
            2,
            2,
            TypeErrorCode::ParameterOutOfScope,
        ),
        (
            vec![vector_tag; MAX_TYPE_DEPTH],
            uint_tag,
            24,
            0,
            TypeErrorCode::DepthLimit,
        ),
    ];
    for (wrappers, leaf_tag, payload, parameters, expected) in cases {
        assert_eq!(
            native_unary_type_chain(&wrappers, leaf_tag, payload, parameters),
            Err(expected),
            "native unary type-chain oracle"
        );
        assert_type_chain_error(
            &execute_type_chain(
                &package, &approved, &wrappers, leaf_tag, payload, parameters,
            ),
            expected,
        );
    }
}

#[test]
fn checker_single_effect_closures_match_native_report() {
    let (package, approved) = admit_checker_program(&single_effect_closure_checker());

    for facts in [SingleEffectFacts::EMPTY, SingleEffectFacts::ONE] {
        let native =
            native_single_effect_closure(facts).expect("native effect checker accepts fixture");
        let first = execute_single_effect(&package, &approved, facts);
        let second = execute_single_effect(&package, &approved, facts);
        assert_effect_ok(&first, &native);
        assert_eq!(first.termination, second.termination);
    }
}

#[test]
fn checker_single_effect_closures_preserve_resolution_precedence() {
    use sley_check::effects::EffectErrorCode;

    let (package, approved) = admit_checker_program(&single_effect_closure_checker());
    let cases = [
        (
            SingleEffectFacts {
                declared_present: true,
                ..SingleEffectFacts::EMPTY
            },
            EffectErrorCode::UnresolvedEntity,
        ),
        (
            SingleEffectFacts {
                request_present: true,
                ..SingleEffectFacts::EMPTY
            },
            EffectErrorCode::UnresolvedEntity,
        ),
        (
            SingleEffectFacts {
                declared_id: 21,
                ..SingleEffectFacts::ONE
            },
            EffectErrorCode::UnresolvedEntity,
        ),
        (
            SingleEffectFacts {
                request_id: 21,
                ..SingleEffectFacts::ONE
            },
            EffectErrorCode::UnresolvedEntity,
        ),
        (
            SingleEffectFacts {
                request_present: false,
                ..SingleEffectFacts::ONE
            },
            EffectErrorCode::ClosureMismatch,
        ),
        (
            SingleEffectFacts {
                declared_present: false,
                ..SingleEffectFacts::ONE
            },
            EffectErrorCode::ClosureMismatch,
        ),
        (
            SingleEffectFacts {
                declared_id: 21,
                request_id: 22,
                ..SingleEffectFacts::ONE
            },
            EffectErrorCode::UnresolvedEntity,
        ),
    ];
    for (facts, expected) in cases {
        assert_eq!(
            native_single_effect_closure(facts),
            Err(expected),
            "native single-effect oracle"
        );
        assert_effect_error(&execute_single_effect(&package, &approved, facts), expected);
    }
}

#[test]
fn checker_effect_set_inventories_match_native_report() {
    let (package, approved) = admit_checker_program(&effect_set_inventory_checker());
    for (definitions, declared, local_closure) in [
        (Vec::new(), Vec::new(), Vec::new()),
        (vec![10, 20, 30], vec![10, 30], vec![10, 30]),
        (vec![7, 9, 11, 13], vec![9, 11, 13], vec![9, 11, 13]),
    ] {
        let native = native_effect_set_inventory(&definitions, &declared, &local_closure)
            .expect("native effect-set checker accepts fixture");
        let first = execute_effect_set_inventory(
            &package,
            &approved,
            &definitions,
            &declared,
            &local_closure,
        );
        let second = execute_effect_set_inventory(
            &package,
            &approved,
            &definitions,
            &declared,
            &local_closure,
        );
        assert_effect_ok(&first, &native);
        assert_eq!(first.termination, second.termination);
    }
}

#[test]
fn checker_effect_set_inventories_preserve_native_precedence() {
    use sley_check::{cfg::CfgErrorCode, effects::EffectErrorCode};

    let (package, approved) = admit_checker_program(&effect_set_inventory_checker());
    let cases: &[EffectSetCase<'_>] = &[
        (
            &[20, 10],
            &[20, 10],
            &[40],
            EffectErrorCode::SetNotCanonical.numeric(),
        ),
        (
            &[10, 10],
            &[],
            &[],
            EffectErrorCode::SetNotCanonical.numeric(),
        ),
        (
            &[10, 20],
            &[20, 10],
            &[10, 20],
            CfgErrorCode::GraphInventoryMismatch.numeric(),
        ),
        (
            &[10, 20],
            &[10, 40],
            &[50],
            EffectErrorCode::UnresolvedEntity.numeric(),
        ),
        (
            &[10, 20],
            &[10],
            &[40],
            EffectErrorCode::UnresolvedEntity.numeric(),
        ),
        (
            &[10, 20],
            &[10],
            &[],
            EffectErrorCode::ClosureMismatch.numeric(),
        ),
        (
            &[10, 20],
            &[],
            &[20],
            EffectErrorCode::ClosureMismatch.numeric(),
        ),
    ];
    for (definitions, declared, local_closure, expected) in cases {
        assert_eq!(
            native_effect_set_inventory(definitions, declared, local_closure),
            Err(*expected),
            "native effect-set oracle"
        );
        assert_effect_numeric_error(
            &execute_effect_set_inventory(
                &package,
                &approved,
                definitions,
                declared,
                local_closure,
            ),
            *expected,
        );
    }
}

#[test]
fn checker_two_function_effect_closures_match_native_state_space() {
    let (package, approved) = admit_checker_program(&two_function_effect_closure_checker());
    for caller_declared in [false, true] {
        for caller_local in [false, true] {
            for calls_callee in [false, true] {
                for callee_declared in [false, true] {
                    for callee_local in [false, true] {
                        let facts = TwoFunctionEffectFacts {
                            caller_declared,
                            caller_local,
                            calls_callee,
                            callee_declared,
                            callee_local,
                        };
                        let native = native_two_function_effect_closure(facts);
                        let first = execute_two_function_effect_closure(&package, &approved, facts);
                        let second =
                            execute_two_function_effect_closure(&package, &approved, facts);
                        match native {
                            Ok(report) => assert_effect_ok(&first, &report),
                            Err(code) => assert_effect_numeric_error(&first, code),
                        }
                        assert_eq!(
                            first.termination, second.termination,
                            "two-function checker is deterministic for {facts:?}"
                        );
                    }
                }
            }
        }
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
