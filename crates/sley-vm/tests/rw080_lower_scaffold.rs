//! RW-080 lowerer scaffold (§1.3): entry plus frozen lowering vocabulary.
//!
//! PROVISIONAL C0 SEED SCAFFOLD — explicitly not accepted runtime
//! authority. Seed-authored (this file is the seed-assembler record)
//! under the operator development override; it exercises the v2
//! admit/approve/execute path for the third RW-080 toolchain module
//! without implementing any lowering. Per contract §1.3, a scaffold is
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
//! Construction provenance:
//! machineresearch/sley-2.0/reweave/rw-080-lower-scaffold.md.

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

// Fixture-namespace entity identities (recorded in the manifest;
// collision-free within this closure by construction; disjoint from
// the §1.1 codec and §1.2 checker scaffold ranges by choice,
// although each image admits independently).
const FUNCTION: u8 = 202;
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
    // block identities stay in the 230s/240s; constants in the 200s.
    // Never mixed, never reused.
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
    let scaffold = lower_scaffold();
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
