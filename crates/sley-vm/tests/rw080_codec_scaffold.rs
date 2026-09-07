//! RW-080 codec scaffold (§1.1): entry dispatch plus error vocabulary.
//!
//! PROVISIONAL C0 SEED SCAFFOLD — explicitly not accepted runtime
//! authority. Seed-authored (this file is the seed-assembler record)
//! under the operator development override; it exercises the v2
//! admit/approve/execute path for the first RW-080 toolchain module
//! without implementing any compiler algorithm. Per contract §1.1,
//! a scaffold is entry dispatch plus the error vocabulary with
//! `unimplemented` traps on real inputs: all four operation legs trap
//! (`TrapCode::Unreachable` with the leg index as payload, proving
//! dispatch reached the leg), and only the unknown-operation path
//! returns a typed `CodecError` value (VERSION). No framing is parsed,
//! no bytes are produced, no judgment is performed. Construction
//! provenance: machineresearch/sley-2.0/reweave/rw-080-codec-scaffold.md.

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

// Scaffold CodecError vocabulary (§1.1): single-UInt discriminants,
// documented in the manifest. TRUNCATED=1 TRAILING=2 MALFORMED_TAG=3
// SHAPE=4 COUNT=5 VERSION=6 LIMIT=7. The real CodecError enum arrives
// with the functioning decoder (RW-100 gate); the scaffold only needs
// one returnable code to prove the error path returns values.
const VERSION_ERROR: u128 = 6;

// Fixture-namespace entity identities (recorded in the manifest;
// collision-free within this closure by construction).
const FUNCTION: u8 = 200;
const OP_PARAM: u8 = 210;
const INPUT_PARAM: u8 = 211;
const ENTRY_BLOCK: u8 = 220;
const CHAIN_BLOCK_1: u8 = 226;
const CHAIN_BLOCK_2: u8 = 227;
const CHAIN_BLOCK_3: u8 = 228;
const LEG_BLOCK_0: u8 = 221;
const LEG_BLOCK_1: u8 = 222;
const LEG_BLOCK_2: u8 = 223;
const LEG_BLOCK_3: u8 = 224;
const UNKNOWN_BLOCK: u8 = 225;

struct CodecScaffold {
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
    op_param: EntityId,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    next_op: u8,
}

impl ScaffoldBuilder {
    fn new(function: EntityId, op_param: EntityId) -> Self {
        Self {
            function,
            op_param,
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
            result_types: vec![u8_type()],
            immediate: Immediate::Entity(target),
        });
        op
    }

    // Chain block: compare the selector against one constant, branch to
    // the leg on equality else to the next chain block.
    fn chain_block(&mut self, block_id: u8, selector_const: EntityId, leg: u8, next: u8) {
        let block = id(block_id);
        let const_op = self.const_ref(block, 0, selector_const);
        let eq_op = self.take_op();
        self.operations.push(Operation {
            entity_id: eq_op,
            block,
            ordinal: 1,
            opcode: Opcode::Equal,
            operands: vec![
                ValueRef::Parameter(self.op_param),
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
                    target: id(leg),
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

    // Operation leg: unimplemented trap carrying the leg index.
    fn leg_block(&mut self, leg: u8, payload_const: EntityId) {
        let block = id(LEG_BLOCK_0 + leg);
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

    // Unknown operation: typed VERSION error value (the vocabulary
    // return path; the only value-returning exit in the scaffold).
    fn unknown_block(&mut self, version_const: EntityId) {
        let unknown = id(UNKNOWN_BLOCK);
        let version_op = self.const_ref(unknown, 0, version_const);
        self.blocks.push(Block {
            entity_id: unknown,
            function: self.function,
            parameters: Vec::new(),
            operations: vec![version_op],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: version_op,
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

fn codec_scaffold() -> CodecScaffold {
    let function = id(FUNCTION);
    let op_param = id(OP_PARAM);
    let input_param = id(INPUT_PARAM);
    // Operation-selector constants K0..K3 double as the per-leg trap
    // payloads, so reaching a leg is observable: leg k traps carrying k.
    let const_id = |k: u8| id(230 + k);
    let version_const = id(234);
    let mut constants: Vec<ConstantDefinition> = (0..4)
        .map(|k| ConstantDefinition {
            entity_id: const_id(k),
            value: u8_value(u128::from(k)),
        })
        .collect();
    constants.push(ConstantDefinition {
        entity_id: version_const,
        value: u8_value(VERSION_ERROR),
    });

    // Operation identities run on their own sequential namespace (100+);
    // block identities stay in the 220s. Never mixed, never reused.
    let mut builder = ScaffoldBuilder::new(function, op_param);
    // Chain blocks test selectors 0..=2 in order; the final else covers
    // selector 3 plus everything unknown, so block 3 both dispatches
    // leg 3 and returns VERSION for anything else.
    let chain = [
        (ENTRY_BLOCK, 0, LEG_BLOCK_0, CHAIN_BLOCK_1),
        (CHAIN_BLOCK_1, 1, LEG_BLOCK_1, CHAIN_BLOCK_2),
        (CHAIN_BLOCK_2, 2, LEG_BLOCK_2, CHAIN_BLOCK_3),
        (CHAIN_BLOCK_3, 3, LEG_BLOCK_3, UNKNOWN_BLOCK),
    ];
    for (block_id, selector, leg, next) in chain {
        builder.chain_block(block_id, const_id(selector), leg, next);
    }
    // Operation legs: unimplemented traps carrying the leg index.
    for leg in 0..4 {
        builder.leg_block(leg, const_id(leg));
    }
    builder.unknown_block(version_const);
    let (blocks, operations) = builder.finish();

    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![op_param, input_param],
        result_type: u8_type(),
        effects: Vec::new(),
        entry_block: id(ENTRY_BLOCK),
        blocks: blocks.iter().map(|block| block.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    CodecScaffold {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: vec![
            Parameter {
                entity_id: op_param,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: u8_type(),
            },
            Parameter {
                entity_id: input_param,
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
    let scaffold = codec_scaffold();
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
    op: u128,
    input: &[u8],
) -> sley_vm::ExecutionTermination {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs: vec![u8_value(op), bytes_value(input)],
            limits: generous_limits(),
        },
    )
    .expect("v2 executes scaffold")
    .termination
}

#[test]
fn codec_scaffold_dispatch_traps_per_leg() {
    use sley_vm::ExecutionTermination;
    let (package, approved) = admitted_scaffold();
    // Every operation leg traps Unreachable carrying its own index —
    // dispatch is observable per leg — and distinct input bytes trap
    // identically, proving the scaffold reads no framing.
    for leg in 0..4_u128 {
        for input in [b"".as_slice(), b"\x00\x01\x02scaffold-input".as_slice()] {
            match execute_scaffold(&package, &approved, leg, input) {
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
fn codec_scaffold_unknown_operation_returns_version_error() {
    use sley_vm::ExecutionTermination;
    let (package, approved) = admitted_scaffold();
    // Outside 0..=3 the scaffold returns the typed vocabulary value —
    // the only value-returning exit; no leg traps.
    for op in [4_u128, 7_u128, 255_u128] {
        match execute_scaffold(&package, &approved, op, b"") {
            ExecutionTermination::Success(value) => assert_eq!(
                value.data,
                ConstData::UInt(VERSION_ERROR),
                "unknown op {op} returns VERSION"
            ),
            other => panic!("unknown op {op} must return VERSION, got {other:?}"),
        }
    }
}
