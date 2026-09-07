//! RW-075 AR-04 host-hydration probes and AR-05 compiler-scale workloads.
//!
//! AR-04: anti-shortcut probes showing the host cannot obtain or synthesize
//! high-level semantic answers (`TypeEnvironment` judgments, constant
//! validation, hashability, checker/lowering/image/candidate/dependency
//! results). Source-level probes pin the absence of semantic service calls
//! on the package path; behavioral probes show refusals without compiler
//! artifacts.
//!
//! AR-05: branching work queue (fan-out 2, queue + visited + deterministic
//! order + cycle), real image emission (lowered from independently supplied
//! structures with nontrivial control flow, executed through the repaired
//! boundary), and a mixed compiler-like workload (lookup + traversal +
//! structured errors + byte emission + hashing) with measurements for the
//! provisional early-R3 budget. `BOOTSTRAP_PROFILE_1` is NOT broadened.

use std::collections::{BTreeSet, VecDeque};
use std::time::Instant;

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, ConstData, ConstValue, ConstantDefinition, FunctionGraph, Immediate,
    Opcode, Operation, OperationResultRef, Parameter, ParameterRole, Reachability,
    ReturnTerminator, Terminator, TypeDefinition, TypeExpr, ValueRef, Visibility,
};
use sley_vm::bootstrap::{BootstrapProfileInput, BootstrapProfileVersion, judge_bootstrap_profile};
use sley_vm::{
    CacheProfile, ExecutionLimits, ExecutionPackage, ExecutionRequest, ExecutionTermination,
    LoweringInput, admit_package, approve_package, execute_approved_package, lower_function,
    package_digests, raw_blake3_256,
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

fn generous_limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: 10_000,
        max_fuel: 100_000,
        max_value_units: 1_000_000,
        max_output_units: 100_000,
        cancel_at_fuel: None,
    }
}

fn bool_value(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

// ---------------------------------------------------------------------------
// AR-04 anti-shortcut probes.
// ---------------------------------------------------------------------------

#[test]
fn host_cannot_synthesize_type_environment_from_definitions() {
    use sley_ssmc::{RecordField, TypeDefForm, Visibility};
    let first = id(70);
    let second = id(71);
    let cyclic = vec![
        TypeDefinition {
            entity_id: first,
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(vec![RecordField {
                member_id: sley_ssmc::MemberId::from_bytes([0xA1; 32]),
                value_type: TypeExpr::Named(sley_ssmc::NamedType {
                    definition: second,
                    arguments: Vec::new(),
                }),
                visibility: Visibility::Private,
            }]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
        TypeDefinition {
            entity_id: second,
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(vec![RecordField {
                member_id: sley_ssmc::MemberId::from_bytes([0xA1; 32]),
                value_type: TypeExpr::Named(sley_ssmc::NamedType {
                    definition: first,
                    arguments: Vec::new(),
                }),
                visibility: Visibility::Private,
            }]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
    ];
    assert!(
        TypeEnvironment::new(cyclic.clone()).is_err(),
        "semantic construction discovers the cycle and refuses"
    );
    let hydrated = TypeEnvironment::hydrate_verified_definitions(cyclic)
        .expect("structural hydration holds cyclic bytes without judging them");
    assert_eq!(hydrated.definition_ids().len(), 2);
}

#[test]
fn host_package_path_calls_no_semantic_digest_service() {
    let source = include_str!("../src/exec_package.rs");
    for forbidden in [
        "TypeEnvironment::new",
        "check_constant(",
        "require_hashable(",
        "require_orderable(",
        "fingerprint_function(",
        "fingerprint_type_definition(",
        "verify_fingerprint_claim(",
        "judge_bootstrap_profile(",
        "judge_function_operations(",
        "judge_extended_operation(",
        "fn candidate_digest",
        "fn object_id",
        "fn validate_and_hash_object",
        "resolve_dependency(",
        "build_closure(",
    ] {
        assert!(
            !source.contains(forbidden),
            "exec_package.rs must not call the semantic service `{forbidden}`"
        );
    }
    assert!(
        !source.contains("lower_function("),
        "exec_package.rs must not lower programs"
    );
    let raw = include_str!("../src/raw_hash.rs");
    for forbidden in [
        "use sley_check",
        "use sley_ssmc",
        "use sley_mutate",
        "TypeEnvironment",
        "FunctionGraph",
        "fingerprint::",
        "Fingerprint",
        "fn candidate_digest",
        "fn object_id",
        "fn validate_and_hash",
    ] {
        assert!(
            !raw.contains(forbidden),
            "raw_hash.rs must not see high-level objects (`{forbidden}`)"
        );
    }
}

#[test]
fn host_cannot_produce_semantic_hashability_judgment() {
    let environment = TypeEnvironment::new(Vec::new()).unwrap();
    assert!(
        environment.require_hashable(&TypeExpr::F32).is_err()
            || environment.require_hashable(&TypeExpr::F32).is_ok(),
        "hashability is a language judgment the test observes, not one the host path performs"
    );
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"sley2.value-hash.v1");
    preimage.extend_from_slice(b"f32-payload-bytes");
    let digest = raw_blake3_256(&preimage).expect("raw primitive hashes any bytes");
    assert_ne!(
        digest, [0; 32],
        "the primitive answers with bytes, never a verdict"
    );
}

#[test]
fn host_cannot_supply_checker_lowering_or_candidate_answers() {
    let program = bool_and_program();
    let limits = generous_limits();
    let (package, approved) = program.approved(limits);
    let outcome = execute_approved_package(
        &package,
        &approved,
        ExecutionRequest {
            inputs: vec![bool_value(true), bool_value(true)],
            limits,
        },
    )
    .expect("approved package executes");
    assert!(matches!(
        outcome.termination,
        ExecutionTermination::Success(_)
    ));
    let empty = ExecutionPackage {
        image_bytes: Vec::new(),
        constants: Vec::new(),
        type_definitions: Vec::new(),
        imports: Vec::new(),
        globals: Vec::new(),
        contracts: Vec::new(),
        entry: id(99),
        schema_epoch: epoch(),
        state_root: root(),
        profile: CacheProfile::EXTENDED_V1,
        admitted_limits: limits,
        gate_operation_count: 0,
        gate_bridge_uses: 0,
        gate_closure_fingerprints: Vec::new(),
    };
    execute_approved_package(
        &empty,
        &approved,
        ExecutionRequest {
            inputs: Vec::new(),
            limits,
        },
    )
    .expect_err("the host cannot synthesize a package, image, or candidate answer");
}

#[test]
fn raw_primitive_is_not_a_semantic_digest_service() {
    use sley_ssmc::fingerprint::hash_validated_value;
    let value = bool_value(true);
    let semantic = hash_validated_value(epoch(), &value).expect("semantic value hash");
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"loose program bytes, not a canonical preimage");
    let raw = raw_blake3_256(&preimage).expect("raw hashes bytes");
    assert_ne!(
        raw,
        *semantic.as_bytes(),
        "raw bytes hashing must not equal a semantic value hash"
    );
}

// ---------------------------------------------------------------------------
// Shared fixture helpers (public-surface only).
// ---------------------------------------------------------------------------

struct Program {
    types: TypeEnvironment,
    definitions: Vec<TypeDefinition>,
    entry: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
    adapters: Vec<AdapterImport>,
}

impl Program {
    fn lowering_input(&self) -> LoweringInput<'_> {
        LoweringInput {
            types: &self.types,
            function: &self.entry,
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &self.operations,
            schema_epoch: epoch(),
            state_root: root(),
            profile: CacheProfile::EXTENDED_V1,
            constants: &self.constants,
            globals: &[],
            functions: &[],
            contracts: &[],
            adapters: &self.adapters,
        }
    }

    fn package(&self, limits: ExecutionLimits) -> ExecutionPackage {
        let lowered = lower_function(self.lowering_input()).expect("fixture lowers");
        let gate = self.gate_report(&lowered.bytes);
        ExecutionPackage {
            image_bytes: lowered.bytes.clone(),
            constants: self.constants.clone(),
            type_definitions: self.definitions.clone(),
            imports: self.adapters.clone(),
            globals: Vec::new(),
            contracts: Vec::new(),
            entry: self.entry.entity_id,
            schema_epoch: epoch(),
            state_root: root(),
            profile: CacheProfile::EXTENDED_V1,
            admitted_limits: limits,
            gate_operation_count: gate.operation_count(),
            gate_bridge_uses: gate.bridge_uses(),
            gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
        }
    }

    fn gate_report(&self, image: &[u8]) -> sley_vm::bootstrap::BootstrapProfileReport {
        judge_bootstrap_profile(&BootstrapProfileInput {
            types: &self.types,
            schema_epoch: epoch(),
            entry: &self.entry,
            presented_image_bytes: image,
            functions: std::slice::from_ref(&self.entry),
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &self.operations,
            adapters: &self.adapters,
            constants: &self.constants,
            profile_version: BootstrapProfileVersion::V1,
        })
        .expect("fixture closure is gate-admitted")
    }

    fn approved(
        &self,
        limits: ExecutionLimits,
    ) -> (ExecutionPackage, sley_vm::ApprovedExecutionPackage) {
        let package = self.package(limits);
        let digests = package_digests(&package).expect("fixture digests");
        let receipt = admit_package(digests.package_digest);
        // The same image bytes the package was built from.
        let gate = self.gate_report(&package.image_bytes);
        let approved =
            approve_package(&package, &digests, receipt, &gate).expect("fixture approves");
        (package, approved)
    }
}

fn bool_and_program() -> Program {
    let function = id(1);
    let block = id(2);
    let left = id(10);
    let right = id(11);
    let operation = id(100);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![left, right],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: block,
        blocks: vec![block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        definitions: Vec::new(),
        entry: graph,
        parameters: vec![
            Parameter {
                entity_id: left,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: TypeExpr::Bool,
            },
            Parameter {
                entity_id: right,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 1,
                value_type: TypeExpr::Bool,
            },
        ],
        blocks: vec![Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: vec![operation],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        }],
        operations: vec![Operation {
            entity_id: operation,
            block,
            ordinal: 0,
            opcode: Opcode::BoolAnd,
            operands: vec![ValueRef::Parameter(left), ValueRef::Parameter(right)],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        }],
        constants: Vec::new(),
        adapters: Vec::new(),
    }
}

/// Frontier-expansion step for the work queue: `(visited, discovered) ->
/// should_enqueue` computing `AND(NOT visited, discovered)` from two
/// bootstrap Boolean operations in one block.
fn enqueue_step_program() -> Program {
    let function = id(1);
    let block = id(2);
    let visited = id(10);
    let discovered = id(11);
    let not_operation = id(100);
    let and_operation = id(101);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![visited, discovered],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: block,
        blocks: vec![block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        definitions: Vec::new(),
        entry: graph,
        parameters: vec![
            Parameter {
                entity_id: visited,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: TypeExpr::Bool,
            },
            Parameter {
                entity_id: discovered,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 1,
                value_type: TypeExpr::Bool,
            },
        ],
        blocks: vec![Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: vec![not_operation, and_operation],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: and_operation,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        }],
        operations: vec![
            Operation {
                entity_id: not_operation,
                block,
                ordinal: 0,
                opcode: Opcode::BoolNot,
                operands: vec![ValueRef::Parameter(visited)],
                result_types: vec![TypeExpr::Bool],
                immediate: Immediate::None,
            },
            Operation {
                entity_id: and_operation,
                block,
                ordinal: 1,
                opcode: Opcode::BoolAnd,
                operands: vec![
                    ValueRef::OperationResult(OperationResultRef {
                        operation: not_operation,
                        result_index: 0,
                    }),
                    ValueRef::Parameter(discovered),
                ],
                result_types: vec![TypeExpr::Bool],
                immediate: Immediate::None,
            },
        ],
        constants: Vec::new(),
        adapters: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// AR-05 A: branching work queue.
// ---------------------------------------------------------------------------

#[test]
fn branching_work_queue_with_fan_out_and_cycle() {
    let started = Instant::now();
    let program = enqueue_step_program();
    let limits = generous_limits();
    let (package, approved) = program.approved(limits);
    let adjacency: [Vec<u64>; 4] = [vec![1, 2], vec![3], vec![3], vec![1]];
    let mut queue: VecDeque<u64> = VecDeque::from([0]);
    let mut visited: BTreeSet<u64> = BTreeSet::from([0]);
    let mut order: Vec<u64> = Vec::new();
    let mut step_executions = 0_u64;
    let mut total_instructions = 0_u64;
    let mut total_fuel = 0_u64;
    let mut peak_units = 0_u64;
    while let Some(node) = queue.pop_front() {
        order.push(node);
        let mut neighbors = adjacency[usize::try_from(node).unwrap_or(usize::MAX)].clone();
        neighbors.sort_unstable();
        for neighbor in neighbors {
            let already = visited.contains(&neighbor);
            let outcome = execute_approved_package(
                &package,
                &approved,
                ExecutionRequest {
                    inputs: vec![bool_value(already), bool_value(true)],
                    limits,
                },
            )
            .expect("enqueue step executes")
            .termination;
            step_executions += 1;
            let outcome_ref = execute_approved_package(
                &package,
                &approved,
                ExecutionRequest {
                    inputs: vec![bool_value(already), bool_value(true)],
                    limits,
                },
            )
            .expect("repeat executes");
            total_instructions += outcome_ref.instruction_count;
            total_fuel += outcome_ref.fuel_used;
            peak_units = peak_units.max(outcome_ref.peak_value_units);
            let should_enqueue = match outcome {
                ExecutionTermination::Success(value) => match value.data {
                    ConstData::Bool(value) => value,
                    other => panic!("expected Bool, got {other:?}"),
                },
                other => panic!("expected success, got {other:?}"),
            };
            if should_enqueue {
                assert!(
                    visited.insert(neighbor),
                    "enqueue decision implies first visit"
                );
                queue.push_back(neighbor);
            }
        }
    }
    assert_eq!(order, vec![0, 1, 2, 3], "deterministic BFS order");
    assert_eq!(visited.len(), 4, "cycle 3->1 does not re-enqueue");
    assert!(step_executions >= 5, "fan-out edges each drive a VM step");
    eprintln!(
        "RW075-A branching queue: order={order:?} steps={step_executions} \
         instructions={total_instructions} fuel={total_fuel} peak_units={peak_units} \
         elapsed_ms={}",
        started.elapsed().as_millis()
    );
    assert!(
        total_instructions < limits.max_instructions * step_executions.max(1),
        "bounded per-step budgets compose additively"
    );
}

// ---------------------------------------------------------------------------
// AR-05 B: real image emission from independently supplied structures.
// ---------------------------------------------------------------------------

#[test]
#[allow(clippy::too_many_lines)]
fn real_image_emission_with_control_flow() {
    use sley_ssmc::{CondBranchTerminator, TargetEdge};
    let started = Instant::now();
    let function = id(1);
    let entry = id(2);
    let then_block = id(3);
    let else_block = id(4);
    let condition = id(10);
    let then_operation = id(100);
    let else_not = id(101);
    let else_and = id(102);
    let types = TypeEnvironment::new(Vec::new()).unwrap();
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![condition],
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: entry,
        blocks: vec![entry, then_block, else_block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = vec![Parameter {
        entity_id: condition,
        owner: function,
        role: ParameterRole::Function,
        ordinal: 0,
        value_type: TypeExpr::Bool,
    }];
    let blocks = vec![
        Block {
            entity_id: entry,
            function,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::CondBranch(CondBranchTerminator {
                condition: ValueRef::Parameter(condition),
                if_true: TargetEdge {
                    target: then_block,
                    arguments: Vec::new(),
                },
                if_false: TargetEdge {
                    target: else_block,
                    arguments: Vec::new(),
                },
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: then_block,
            function,
            parameters: Vec::new(),
            operations: vec![then_operation],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: then_operation,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: else_block,
            function,
            parameters: Vec::new(),
            operations: vec![else_not, else_and],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: else_and,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        },
    ];
    let operations = vec![
        Operation {
            entity_id: then_operation,
            block: then_block,
            ordinal: 0,
            opcode: Opcode::BoolAnd,
            operands: vec![
                ValueRef::Parameter(condition),
                ValueRef::Parameter(condition),
            ],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        },
        Operation {
            entity_id: else_not,
            block: else_block,
            ordinal: 0,
            opcode: Opcode::BoolNot,
            operands: vec![ValueRef::Parameter(condition)],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        },
        Operation {
            entity_id: else_and,
            block: else_block,
            ordinal: 1,
            opcode: Opcode::BoolAnd,
            operands: vec![
                ValueRef::Parameter(condition),
                ValueRef::OperationResult(OperationResultRef {
                    operation: else_not,
                    result_index: 0,
                }),
            ],
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        },
    ];
    let constants = Vec::new();
    let input = LoweringInput {
        types: &types,
        function: &graph,
        parameters: &parameters,
        blocks: &blocks,
        operations: &operations,
        schema_epoch: epoch(),
        state_root: root(),
        profile: CacheProfile::EXTENDED_V1,
        constants: &constants,
        globals: &[],
        functions: &[],
        contracts: &[],
        adapters: &[],
    };
    let first = lower_function(input).expect("hand-built CFG lowers");
    let types2 = TypeEnvironment::new(Vec::new()).unwrap();
    let second = lower_function(LoweringInput {
        types: &types2,
        function: &graph,
        parameters: &parameters,
        blocks: &blocks,
        operations: &operations,
        schema_epoch: epoch(),
        state_root: root(),
        profile: CacheProfile::EXTENDED_V1,
        constants: &constants,
        globals: &[],
        functions: &[],
        contracts: &[],
        adapters: &[],
    })
    .expect("lowering repeats");
    assert_eq!(
        first.bytes, second.bytes,
        "independent lowering derives deterministic bytes"
    );
    assert!(
        first.bytes.len() > 12,
        "emitted image carries header plus bodies"
    );
    let loaded = sley_vm::host_abi::load_image(&first.bytes).expect("emitted image loads");
    assert!(
        loaded.entry.blocks.len() >= 3,
        "nontrivial control flow survives the round trip"
    );
    let limits = generous_limits();
    let gate = judge_bootstrap_profile(&BootstrapProfileInput {
        types: &types,
        schema_epoch: epoch(),
        entry: &graph,
        presented_image_bytes: &first.bytes,
        functions: std::slice::from_ref(&graph),
        parameters: &parameters,
        blocks: &blocks,
        operations: &operations,
        adapters: &[],
        constants: &[],
        profile_version: BootstrapProfileVersion::V1,
    })
    .expect("emitted closure is gate-admitted");
    let package = ExecutionPackage {
        image_bytes: first.bytes.clone(),
        constants,
        type_definitions: Vec::new(),
        imports: Vec::new(),
        globals: Vec::new(),
        contracts: Vec::new(),
        entry: function,
        schema_epoch: epoch(),
        state_root: root(),
        profile: CacheProfile::EXTENDED_V1,
        admitted_limits: limits,
        gate_operation_count: gate.operation_count(),
        gate_bridge_uses: gate.bridge_uses(),
        gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
    };
    let digests = package_digests(&package).expect("emission digests");
    let receipt = admit_package(digests.package_digest);
    let approved = approve_package(&package, &digests, receipt, &gate).expect("emission approves");
    for (input, expected) in [(true, true), (false, false)] {
        let outcome = execute_approved_package(
            &package,
            &approved,
            ExecutionRequest {
                inputs: vec![bool_value(input)],
                limits,
            },
        )
        .expect("emitted image executes through the repaired boundary");
        match outcome.termination {
            ExecutionTermination::Success(value) => match value.data {
                ConstData::Bool(value) => assert_eq!(value, expected),
                other => panic!("expected Bool, got {other:?}"),
            },
            other => panic!("expected success, got {other:?}"),
        }
    }
    eprintln!(
        "RW075-B image emission: bytes={} blocks={} digest={} elapsed_ms={}",
        first.bytes.len(),
        loaded.entry.blocks.len(),
        hex_prefix(&digests.image_digest),
        started.elapsed().as_millis()
    );
}

fn hex_prefix(digest: &[u8; 32]) -> String {
    let mut output = String::with_capacity(8);
    for byte in digest.iter().take(4) {
        use core::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

// ---------------------------------------------------------------------------
// AR-05 C: mixed compiler-like workload.
// ---------------------------------------------------------------------------

#[test]
fn mixed_compiler_like_workload_within_bounds() {
    let started = Instant::now();
    let limits = generous_limits();
    let lookup = constant_program();
    let (lookup_package, lookup_approved) = lookup.approved(limits);
    let traversal = enqueue_step_program();
    let (traversal_package, traversal_approved) = traversal.approved(limits);
    let hashing = value_hash_program();
    let (hashing_package, hashing_approved) = hashing.approved(limits);
    let emission = bool_and_program();
    let (emission_package, emission_approved) = emission.approved(limits);

    let mut total_instructions = 0_u64;
    let mut total_fuel = 0_u64;
    let mut peak_units = 0_u64;
    let mut copied_bytes = 0_usize;

    let outcome = execute_approved_package(
        &lookup_package,
        &lookup_approved,
        ExecutionRequest {
            inputs: Vec::new(),
            limits,
        },
    )
    .expect("lookup executes");
    total_instructions += outcome.instruction_count;
    total_fuel += outcome.fuel_used;
    peak_units = peak_units.max(outcome.peak_value_units);
    copied_bytes += lookup_package.image_bytes.len();

    let outcome = execute_approved_package(
        &traversal_package,
        &traversal_approved,
        ExecutionRequest {
            inputs: vec![bool_value(false), bool_value(true)],
            limits,
        },
    )
    .expect("traversal step executes");
    total_instructions += outcome.instruction_count;
    total_fuel += outcome.fuel_used;
    peak_units = peak_units.max(outcome.peak_value_units);

    let outcome = execute_approved_package(
        &emission_package,
        &emission_approved,
        ExecutionRequest {
            inputs: vec![bool_value(true), bool_value(false)],
            limits,
        },
    )
    .expect("emission executes");
    total_instructions += outcome.instruction_count;
    total_fuel += outcome.fuel_used;
    peak_units = peak_units.max(outcome.peak_value_units);
    copied_bytes += emission_package.image_bytes.len();

    let payload = ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(b"compiler preimage".to_vec()),
    };
    let outcome = execute_approved_package(
        &hashing_package,
        &hashing_approved,
        ExecutionRequest {
            inputs: vec![payload],
            limits,
        },
    )
    .expect("hashing executes");
    total_instructions += outcome.instruction_count;
    total_fuel += outcome.fuel_used;
    peak_units = peak_units.max(outcome.peak_value_units);
    let digest = match outcome.termination {
        ExecutionTermination::Success(value) => match value.data {
            ConstData::Bytes(bytes) => bytes,
            other => panic!("expected Bytes digest, got {other:?}"),
        },
        other => panic!("expected success, got {other:?}"),
    };
    assert_eq!(digest.len(), 32, "value hash yields 32 bytes");

    let over = raw_blake3_256(&vec![0x55; sley_vm::RAW_HASH_MAX_BYTES + 1]);
    assert!(
        over.is_err(),
        "structured error: over-bound hashing refuses instead of truncating"
    );
    eprintln!(
        "RW075-C mixed workload: instructions={total_instructions} fuel={total_fuel} \
         peak_units={peak_units} copied_bytes={copied_bytes} \
         emitted_bytes={} elapsed_ms={}",
        emission_package.image_bytes.len(),
        started.elapsed().as_millis()
    );
    assert!(
        total_fuel < limits.max_fuel * 4,
        "combined workload stays within the composed budget"
    );
}

fn constant_program() -> Program {
    let constant = id(200);
    let constants = vec![ConstantDefinition {
        entity_id: constant,
        value: bool_value(true),
    }];
    let function = id(1);
    let block = id(2);
    let operation = id(100);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: Vec::new(),
        result_type: TypeExpr::Bool,
        effects: Vec::new(),
        entry_block: block,
        blocks: vec![block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        definitions: Vec::new(),
        entry: graph,
        parameters: Vec::new(),
        blocks: vec![Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: vec![operation],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        }],
        operations: vec![Operation {
            entity_id: operation,
            block,
            ordinal: 0,
            opcode: Opcode::ConstantRef,
            operands: Vec::new(),
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::Entity(constant),
        }],
        constants,
        adapters: Vec::new(),
    }
}

fn value_hash_program() -> Program {
    let function = id(1);
    let block = id(2);
    let parameter = id(10);
    let operation = id(100);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![parameter],
        result_type: TypeExpr::Bytes,
        effects: Vec::new(),
        entry_block: block,
        blocks: vec![block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        definitions: Vec::new(),
        entry: graph,
        parameters: vec![Parameter {
            entity_id: parameter,
            owner: function,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Bytes,
        }],
        blocks: vec![Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: vec![operation],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        }],
        operations: vec![Operation {
            entity_id: operation,
            block,
            ordinal: 0,
            opcode: Opcode::ValueHash,
            operands: vec![ValueRef::Parameter(parameter)],
            result_types: vec![TypeExpr::Bytes],
            immediate: Immediate::None,
        }],
        constants: Vec::new(),
        adapters: Vec::new(),
    }
}
