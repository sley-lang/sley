//! RW-080 checker scaffold (§1.2): entry plus error vocabulary.
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
//! scaffold performs no type, CFG, or effect judgment. Construction
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

    fn const_ref(&mut self, block: EntityId, ordinal: u32, target: EntityId) -> EntityId {
        let op = self.take_op();
        self.operations.push(Operation {
            entity_id: op,
            block,
            ordinal,
            opcode: Opcode::ConstantRef,
            operands: Vec::new(),
            result_types: vec![u8_type()],
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
    use sley_vm::{
        ExecutionPackage, V2Closure, approve_package_v2, bootstrap::BootstrapProfileInput,
        bootstrap::BootstrapProfileVersion,
    };
    let scaffold = checker_scaffold();
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
