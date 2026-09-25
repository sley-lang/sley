//! S3 G3 S2B-PERF-001: nested comparison scan vs ordered-map membership.
//!
//! Two `EXTENDED_V1` implementations over one fixed large input (40 haystack
//! plus 40 query bytes from a fixed xorshift64 seed, scalarized as `u8`
//! parameters so both strategies read identical data): `build_before` is the
//! quadratic nested scan (`Equal` plus `BoolOr` chains), `build_after` builds an
//! ordered map once (`MapNew` over deduplicated keys) and probes it
//! (`MapContains`). Both record `instruction_count` plus `fuel_used` off
//! `ExecutionOutcome`; outputs must agree exactly.
//!
//! Naming: `s3_g3_perf` is the file slug; `s3_g3_perf_neg_faster_but_wrong`
//! is the single negative. The `s3_g3_effect_refusal_pin` /
//! `s3_g3_cap_refusal_pin` tests pin the S2B-EFFECT-001 / S2B-CAP-001
//! `VM_LOWER_OPCODE_UNSUPPORTED` refusals through the real restricted
//! lowering path for those tasks' oracle.py drivers (no execution fixtures
//! exist for EFFECT/CAP by contract).
//!
//! Public `sley-vm` surface only (`lower_function`, `execute_function`),
//! plus public `sley-check` / `sley-id` / `sley-ssmc` value types — the same
//! boundary the existing `rw070_*` integration tests use. No crate
//! internals, no dev-dependencies beyond the crate's own dependencies.

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    Block, BuiltinCase, BuiltinFailureKind, CaseKey, ConstData, ConstValue, FunctionGraph,
    Immediate, IntegerWidth, Opcode, Operation, OperationResultRef, Parameter, ParameterRole,
    Reachability, ReturnTerminator, SwitchArgument, SwitchCase, SwitchEdge, Terminator, TrapCode,
    TrapTerminator, TypeExpr, ValueRef, VariantSwitchTerminator, Visibility,
};
use sley_vm::{
    CacheProfile, ExecutionLimits, ExecutionOutcome, ExecutionRequest, ExecutionTermination,
    LowerErrorCode, LoweringError, LoweringInput, execute_function, lower_function,
};

// ── Fixed input ──────────────────────────────────────────────────────────

/// Fixed seed: every run generates the identical 80-byte input.
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;
/// Haystack length (before-side nested scan width).
const HAYSTACK_LEN: usize = 40;
/// Query count (scan repetitions / map probes).
const QUERY_LEN: usize = 40;

fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// The one fixed large input: 40 haystack bytes then 40 query bytes.
fn fixed_input() -> ([u8; HAYSTACK_LEN], [u8; QUERY_LEN]) {
    let mut state = SEED;
    // Discard the first output: the raw seed itself must not appear verbatim
    // in the input (keeps seed-vs-data provenance clean).
    xorshift64(&mut state);
    let mut haystack = [0_u8; HAYSTACK_LEN];
    let mut queries = [0_u8; QUERY_LEN];
    for slot in &mut haystack {
        *slot = u8::try_from((xorshift64(&mut state) >> 11) & 0xff).unwrap();
    }
    for slot in &mut queries {
        *slot = u8::try_from((xorshift64(&mut state) >> 11) & 0xff).unwrap();
    }
    (haystack, queries)
}

// ── Small builders (same shapes as the crate's own closure workloads) ────

/// Unique entity id from a category tag and a counter.
fn eid(tag: u8, n: u32) -> EntityId {
    let mut bytes = [0x5Eu8; 32];
    bytes[0] = tag;
    bytes[1..5].copy_from_slice(&n.to_be_bytes());
    EntityId::from_bytes(bytes)
}

fn u8_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(8))
}

fn boolvec_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(TypeExpr::Bool))
}

fn u8_map_type() -> TypeExpr {
    TypeExpr::OrderedMap {
        key: Box::new(u8_type()),
        value: Box::new(u8_type()),
    }
}

fn map_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(u8_map_type()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    }
}

fn octet(value: u8) -> ConstValue {
    ConstValue {
        value_type: u8_type(),
        data: ConstData::UInt(u128::from(value)),
    }
}

fn param(entity: EntityId, owner: EntityId, ordinal: u32, value_type: TypeExpr) -> Parameter {
    Parameter {
        entity_id: entity,
        owner,
        role: ParameterRole::Function,
        ordinal,
        value_type,
    }
}

fn bparam(entity: EntityId, owner: EntityId, ordinal: u32, value_type: TypeExpr) -> Parameter {
    Parameter {
        entity_id: entity,
        owner,
        role: ParameterRole::Block,
        ordinal,
        value_type,
    }
}

fn oper(
    entity: EntityId,
    block: EntityId,
    ordinal: u32,
    opcode: Opcode,
    operands: Vec<ValueRef>,
    result: TypeExpr,
) -> Operation {
    Operation {
        entity_id: entity,
        block,
        ordinal,
        opcode,
        operands,
        result_types: vec![result],
        immediate: Immediate::None,
    }
}

fn p(entity: EntityId) -> ValueRef {
    ValueRef::Parameter(entity)
}

fn r(operation: EntityId) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation,
        result_index: 0,
    })
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02x}").unwrap();
    }
    out
}

fn generous_limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: 100_000,
        max_fuel: 100_000_000,
        max_value_units: 10_000_000,
        max_output_units: 1_000_000,
        cancel_at_fuel: None,
    }
}

/// Owned single-function program.
struct Program {
    types: TypeEnvironment,
    entry: FunctionGraph,
    functions: Vec<FunctionGraph>,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}

impl Program {
    fn lowering_input(&self) -> LoweringInput<'_> {
        LoweringInput {
            types: &self.types,
            function: &self.entry,
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &self.operations,
            schema_epoch: SchemaEpochId::from_bytes([8; 32]),
            state_root: StateRoot::from_bytes([9; 32]),
            profile: CacheProfile::EXTENDED_V1,
            constants: &[],
            globals: &[],
            functions: &self.functions,
            contracts: &[],
            adapters: &[],
        }
    }

    fn run(&self, inputs: Vec<ConstValue>) -> ExecutionOutcome {
        execute_function(
            self.lowering_input(),
            ExecutionRequest {
                inputs,
                limits: generous_limits(),
            },
        )
        .expect("program is well formed and resourced")
    }
}

/// Scalar u8 inputs in parameter order: haystack then queries.
fn scalar_inputs(haystack: &[u8; HAYSTACK_LEN], queries: &[u8; QUERY_LEN]) -> Vec<ConstValue> {
    haystack
        .iter()
        .chain(queries.iter())
        .map(|byte| octet(*byte))
        .collect()
}

/// BEFORE: quadratic nested comparison scan, one block.
/// For each query: `Equal(h0,q)` then `BoolOr(acc, Equal(hi,q))` chains.
/// Output: 40-element `Vector(Bool)`.
struct ScalarSet {
    parameters: Vec<Parameter>,
    haystack: Vec<EntityId>,
    queries: Vec<EntityId>,
}

fn scalar_set(tag_hay: u8, tag_query: u8, function: EntityId) -> ScalarSet {
    let mut parameters = Vec::with_capacity(HAYSTACK_LEN + QUERY_LEN + 1);
    let mut haystack = Vec::with_capacity(HAYSTACK_LEN);
    let mut queries = Vec::with_capacity(QUERY_LEN);
    for (i, _) in (0..HAYSTACK_LEN).enumerate() {
        let id = eid(tag_hay, u32::try_from(i).unwrap());
        haystack.push(id);
        parameters.push(param(id, function, u32::try_from(i).unwrap(), u8_type()));
    }
    for (j, _) in (0..QUERY_LEN).enumerate() {
        let id = eid(tag_query, u32::try_from(j).unwrap());
        queries.push(id);
        parameters.push(param(
            id,
            function,
            u32::try_from(HAYSTACK_LEN + j).unwrap(),
            u8_type(),
        ));
    }
    ScalarSet {
        parameters,
        haystack,
        queries,
    }
}

fn distinct_bytes(haystack: &[u8; HAYSTACK_LEN]) -> Vec<u8> {
    let mut distinct: Vec<u8> = Vec::new();
    for byte in *haystack {
        if !distinct.contains(&byte) {
            distinct.push(byte);
        }
    }
    distinct
}

fn key_params_for(
    haystack: &[u8; HAYSTACK_LEN],
    haystack_params: &[EntityId],
    distinct: &[u8],
) -> Vec<EntityId> {
    let mut keys: Vec<EntityId> = Vec::with_capacity(distinct.len());
    for byte in distinct {
        let index = haystack
            .iter()
            .position(|candidate| candidate == byte)
            .expect("distinct byte comes from the haystack");
        keys.push(haystack_params[index]);
    }
    keys
}

fn map_operands(key_params: &[EntityId]) -> Vec<ValueRef> {
    let mut operands: Vec<ValueRef> = Vec::with_capacity(key_params.len() * 2);
    for key in key_params {
        operands.push(p(*key));
        operands.push(p(*key));
    }
    operands
}

fn before_scan_ops(
    block: EntityId,
    haystack_params: &[EntityId],
    query_params: &[EntityId],
) -> (Vec<Operation>, EntityId) {
    let mut operations = Vec::new();
    let mut results = Vec::with_capacity(QUERY_LEN);
    let mut counter: u32 = 0;
    let mut ordinal: u32 = 0;
    for query in query_params {
        let first = eid(3, counter);
        counter += 1;
        operations.push(oper(
            first,
            block,
            ordinal,
            Opcode::Equal,
            vec![p(haystack_params[0]), p(*query)],
            TypeExpr::Bool,
        ));
        ordinal += 1;
        let mut acc = first;
        for needle in haystack_params.iter().skip(1) {
            let eq = eid(3, counter);
            counter += 1;
            operations.push(oper(
                eq,
                block,
                ordinal,
                Opcode::Equal,
                vec![p(*needle), p(*query)],
                TypeExpr::Bool,
            ));
            ordinal += 1;
            let or = eid(3, counter);
            counter += 1;
            operations.push(oper(
                or,
                block,
                ordinal,
                Opcode::BoolOr,
                vec![r(acc), r(eq)],
                TypeExpr::Bool,
            ));
            ordinal += 1;
            acc = or;
        }
        results.push(r(acc));
    }
    let gather = eid(3, counter);
    operations.push(oper(
        gather,
        block,
        ordinal,
        Opcode::VectorNew,
        results,
        boolvec_type(),
    ));
    (operations, gather)
}

fn plain_probes(
    work: EntityId,
    table: EntityId,
    query_params: &[EntityId],
    base_counter: u32,
) -> (Vec<Operation>, EntityId) {
    let mut work_ops = Vec::new();
    let mut probes = Vec::with_capacity(QUERY_LEN);
    let mut counter: u32 = base_counter;
    let mut ordinal: u32 = 0;
    for query in query_params {
        let probe = eid(3, counter);
        counter += 1;
        work_ops.push(oper(
            probe,
            work,
            ordinal,
            Opcode::MapContains,
            vec![p(table), p(*query)],
            TypeExpr::Bool,
        ));
        ordinal += 1;
        probes.push(r(probe));
    }
    let gather = eid(3, counter);
    work_ops.push(oper(
        gather,
        work,
        ordinal,
        Opcode::VectorNew,
        probes,
        boolvec_type(),
    ));
    (work_ops, gather)
}

fn flipped_probes(
    work: EntityId,
    table: EntityId,
    query_params: &[EntityId],
    base_counter: u32,
) -> (Vec<Operation>, EntityId) {
    let mut work_ops = Vec::new();
    let mut probes = Vec::with_capacity(QUERY_LEN);
    let mut counter: u32 = base_counter;
    let mut ordinal: u32 = 0;
    for (index, query) in query_params.iter().enumerate() {
        let probe = eid(3, counter);
        counter += 1;
        work_ops.push(oper(
            probe,
            work,
            ordinal,
            Opcode::MapContains,
            vec![p(table), p(*query)],
            TypeExpr::Bool,
        ));
        ordinal += 1;
        if index == 0 {
            let flipped = eid(3, counter);
            counter += 1;
            work_ops.push(oper(
                flipped,
                work,
                ordinal,
                Opcode::BoolNot,
                vec![r(probe)],
                TypeExpr::Bool,
            ));
            ordinal += 1;
            probes.push(r(flipped));
        } else {
            probes.push(r(probe));
        }
    }
    let gather = eid(3, counter);
    work_ops.push(oper(
        gather,
        work,
        ordinal,
        Opcode::VectorNew,
        probes,
        boolvec_type(),
    ));
    (work_ops, gather)
}

fn build_before() -> Program {
    let function = eid(1, 1);
    let block = eid(2, 1);
    let scalar = scalar_set(10, 11, function);
    // Ordinals are per-block positions (graph validation pins this order).
    let (operations, gather) = before_scan_ops(block, &scalar.haystack, &scalar.queries);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: scalar
            .haystack
            .iter()
            .chain(scalar.queries.iter())
            .copied()
            .collect(),
        result_type: boolvec_type(),
        effects: Vec::new(),
        entry_block: block,
        blocks: vec![block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: scalar.parameters,
        blocks: vec![Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: operations.iter().map(|op| op.entity_id).collect(),
            terminator: Terminator::Return(ReturnTerminator { value: r(gather) }),
            reachability: Reachability::Required,
        }],
        operations,
    }
}

/// AFTER: ordered-map strategy. `MapNew` over deduplicated haystack keys in
/// `entry`, `Ok` unwrapped by an exhaustive switch into `work` (which probes
/// `MapContains` per query and gathers the bool vector); `Err` (statically
/// unreachable: keys are deduplicated by construction) traps in `lost`.
fn build_after() -> Program {
    let (haystack, _) = fixed_input();
    let distinct = distinct_bytes(&haystack);
    let function = eid(1, 2);
    let entry = eid(2, 2);
    let work = eid(2, 3);
    let lost = eid(2, 4);
    let table = eid(12, 1);
    let mut scalar = scalar_set(20, 21, function);
    scalar
        .parameters
        .push(bparam(table, work, 0, u8_map_type()));
    // MapNew operands reference haystack params by deduplicated value: the
    // first param carrying each distinct byte serves as both key and value.
    let key_params = key_params_for(&haystack, &scalar.haystack, &distinct);
    let build = eid(3, 100_000);
    let operands = map_operands(&key_params);
    let (work_ops, gather) = plain_probes(work, table, &scalar.queries, 200_000);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: scalar
            .haystack
            .iter()
            .chain(scalar.queries.iter())
            .copied()
            .collect(),
        result_type: boolvec_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: vec![entry, work, lost],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: scalar.parameters,
        blocks: vec![
            Block {
                entity_id: entry,
                function,
                parameters: Vec::new(),
                operations: vec![build],
                terminator: Terminator::VariantSwitch(VariantSwitchTerminator {
                    value: r(build),
                    cases: vec![
                        SwitchCase {
                            case_key: CaseKey::Builtin(BuiltinCase::Ok),
                            edge: SwitchEdge {
                                target: work,
                                arguments: vec![SwitchArgument::CasePayload],
                            },
                        },
                        SwitchCase {
                            case_key: CaseKey::Builtin(BuiltinCase::Err),
                            edge: SwitchEdge {
                                target: lost,
                                arguments: Vec::new(),
                            },
                        },
                    ],
                }),
                reachability: Reachability::Required,
            },
            Block {
                entity_id: work,
                function,
                parameters: vec![table],
                operations: work_ops.iter().map(|op| op.entity_id).collect(),
                terminator: Terminator::Return(ReturnTerminator { value: r(gather) }),
                reachability: Reachability::Required,
            },
            Block {
                entity_id: lost,
                function,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Trap(TrapTerminator {
                    code: TrapCode::Unreachable,
                    payload: None,
                }),
                reachability: Reachability::Required,
            },
        ],
        operations: std::iter::once(oper(
            build,
            entry,
            0,
            Opcode::MapNew,
            operands,
            map_result_type(),
        ))
        .chain(work_ops)
        .collect(),
    }
}

/// AFTER + one `BoolNot` on the first probe: strictly fewer instructions
/// than BEFORE, but the output digest changes. The negative oracle.
fn build_faster_but_wrong() -> Program {
    build_wrong_variant()
}

fn build_wrong_variant() -> Program {
    let (haystack, _) = fixed_input();
    let distinct = distinct_bytes(&haystack);
    let function = eid(1, 3);
    let entry = eid(2, 5);
    let work = eid(2, 6);
    let lost = eid(2, 7);
    let table = eid(12, 2);
    let mut scalar = scalar_set(30, 31, function);
    scalar
        .parameters
        .push(bparam(table, work, 0, u8_map_type()));
    let key_params = key_params_for(&haystack, &scalar.haystack, &distinct);
    let build = eid(3, 300_000);
    let operands = map_operands(&key_params);
    let (work_ops, gather) = flipped_probes(work, table, &scalar.queries, 400_000);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: scalar
            .haystack
            .iter()
            .chain(scalar.queries.iter())
            .copied()
            .collect(),
        result_type: boolvec_type(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: vec![entry, work, lost],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: scalar.parameters,
        blocks: vec![
            Block {
                entity_id: entry,
                function,
                parameters: Vec::new(),
                operations: vec![build],
                terminator: Terminator::VariantSwitch(VariantSwitchTerminator {
                    value: r(build),
                    cases: vec![
                        SwitchCase {
                            case_key: CaseKey::Builtin(BuiltinCase::Ok),
                            edge: SwitchEdge {
                                target: work,
                                arguments: vec![SwitchArgument::CasePayload],
                            },
                        },
                        SwitchCase {
                            case_key: CaseKey::Builtin(BuiltinCase::Err),
                            edge: SwitchEdge {
                                target: lost,
                                arguments: Vec::new(),
                            },
                        },
                    ],
                }),
                reachability: Reachability::Required,
            },
            Block {
                entity_id: work,
                function,
                parameters: vec![table],
                operations: work_ops.iter().map(|op| op.entity_id).collect(),
                terminator: Terminator::Return(ReturnTerminator { value: r(gather) }),
                reachability: Reachability::Required,
            },
            Block {
                entity_id: lost,
                function,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Trap(TrapTerminator {
                    code: TrapCode::Unreachable,
                    payload: None,
                }),
                reachability: Reachability::Required,
            },
        ],
        operations: std::iter::once(oper(
            build,
            entry,
            0,
            Opcode::MapNew,
            operands,
            map_result_type(),
        ))
        .chain(work_ops)
        .collect(),
    }
}

fn success_value(outcome: &ExecutionOutcome) -> ConstValue {
    match &outcome.termination {
        ExecutionTermination::Success(value) => value.clone(),
        other => panic!("expected Success, got {other:?}"),
    }
}

fn output_digest(value: &ConstValue) -> String {
    use sley_ssmc::fingerprint::hash_validated_value;
    let digest = hash_validated_value(SchemaEpochId::from_bytes([8; 32]), value)
        .expect("output value is fingerprintable");
    hex(digest.as_bytes())
}

// ── Pinned measurements (from the engine; emitter prints the same lines) ──

const PIN_BEFORE_INSTRUCTIONS: u64 = 3161;
const PIN_BEFORE_FUEL: u64 = 3162;
const PIN_AFTER_INSTRUCTIONS: u64 = 42;
const PIN_AFTER_FUEL: u64 = 47;
const PIN_OUTPUT_DIGEST: &str = "01f5c799bd1c4d8f3ab1853a9366f3669fdbf6c8d6771f810a2425fdd41881eb";

// ── Conformance ──────────────────────────────────────────────────────────

#[test]
fn s3_g3_perf_fixture_conformance() {
    let (haystack, queries) = fixed_input();
    let inputs = scalar_inputs(&haystack, &queries);
    let before = build_before();
    let after = build_after();
    assert!(
        before.entry.effects.is_empty() && after.entry.effects.is_empty(),
        "effect set unchanged: both implementations are pure"
    );
    let out_before = before.run(inputs.clone());
    let out_after = after.run(inputs);
    // No memory (or any resource) ceiling breach: both terminate Success.
    assert_eq!(
        out_before.termination, out_after.termination,
        "terminations agree"
    );
    let value_before = success_value(&out_before);
    let value_after = success_value(&out_after);
    assert_eq!(value_before, value_after, "identical outputs");
    let digest = output_digest(&value_before);
    assert_eq!(digest, output_digest(&value_after), "digest match");
    assert!(
        out_after.instruction_count < out_before.instruction_count,
        "after uses fewer instructions"
    );
    let reduction_pct = f64::from(
        u32::try_from(out_before.instruction_count - out_after.instruction_count).unwrap(),
    ) / f64::from(u32::try_from(out_before.instruction_count).unwrap())
        * 100.0;
    assert!(
        reduction_pct >= 30.0,
        "reduction {reduction_pct:.2}% meets the 30% bar"
    );
    // Pinned engine facts (kept in sync with fixture/fixture.json).
    assert_eq!(out_before.instruction_count, PIN_BEFORE_INSTRUCTIONS);
    assert_eq!(out_before.fuel_used, PIN_BEFORE_FUEL);
    assert_eq!(out_after.instruction_count, PIN_AFTER_INSTRUCTIONS);
    assert_eq!(out_after.fuel_used, PIN_AFTER_FUEL);
    assert_eq!(digest, PIN_OUTPUT_DIGEST);
    println!(
        "PIN task=S2B-PERF-001 before_instructions={} before_fuel={} after_instructions={} after_fuel={} output_digest={digest} reduction_pct={reduction_pct:.2}",
        out_before.instruction_count,
        out_before.fuel_used,
        out_after.instruction_count,
        out_after.fuel_used,
    );
}

// ── Negative: faster but wrong ───────────────────────────────────────────

#[test]
fn s3_g3_perf_neg_faster_but_wrong() {
    // Oracle code for this negative: ORACLE_OUTPUT_MISMATCH.
    let (haystack, queries) = fixed_input();
    let inputs = scalar_inputs(&haystack, &queries);
    let before = build_before();
    let wrong = build_faster_but_wrong();
    let out_before = before.run(inputs.clone());
    let out_wrong = wrong.run(inputs);
    let digest_before = output_digest(&success_value(&out_before));
    let digest_wrong = output_digest(&success_value(&out_wrong));
    assert!(
        out_wrong.instruction_count < out_before.instruction_count,
        "the wrong variant is still faster (fewer instructions)"
    );
    assert_ne!(
        digest_before, digest_wrong,
        "ORACLE_OUTPUT_MISMATCH: fewer instructions but changed output digest"
    );
    println!(
        "PIN task=S2B-PERF-001 negative=faster_but_wrong code=ORACLE_OUTPUT_MISMATCH wrong_digest={digest_wrong}"
    );
}

// ── Ignored emitter (regen driver input) ─────────────────────────────────

#[test]
#[ignore = "fixture emitter; run explicitly for refresh"]
fn emit_s3_g3_perf_fixture() {
    let (haystack, queries) = fixed_input();
    let inputs = scalar_inputs(&haystack, &queries);
    let before = build_before();
    let after = build_after();
    let out_before = before.run(inputs.clone());
    let out_after = after.run(inputs);
    let digest = output_digest(&success_value(&out_before));
    println!("FIXTURE task=S2B-PERF-001 arm=sley2 profile=EXTENDED_V1");
    println!("FIXTURE seed=0x{SEED:016x} haystack_len={HAYSTACK_LEN} query_len={QUERY_LEN}");
    println!(
        "FIXTURE before_instructions={} before_fuel={}",
        out_before.instruction_count, out_before.fuel_used
    );
    println!(
        "FIXTURE after_instructions={} after_fuel={}",
        out_after.instruction_count, out_after.fuel_used
    );
    println!("FIXTURE output_digest={digest}");
}

// ── EFFECT / CAP refusal pins (oracle.py drivers for the excluded tasks) ──

/// One restricted-profile function carrying a single non-bool operation.
fn restricted_probe(
    opcode: Opcode,
    result: TypeExpr,
    immediate: Immediate,
) -> LoweringInput<'static> {
    // Leaked on purpose: the probe outlives the builder frame and the test
    // process only. (No external crate owns these inventories.)
    let types: &'static TypeEnvironment =
        Box::leak(Box::new(TypeEnvironment::new(Vec::new()).unwrap()));
    let function_id = EntityId::from_bytes([0xE0; 32]);
    let block_id = EntityId::from_bytes([0xE1; 32]);
    let op_id = EntityId::from_bytes([0xE2; 32]);
    let graph: &'static FunctionGraph = Box::leak(Box::new(FunctionGraph {
        entity_id: function_id,
        type_parameters: Vec::new(),
        parameters: Vec::new(),
        result_type: result.clone(),
        effects: Vec::new(),
        entry_block: block_id,
        blocks: vec![block_id],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }));
    let blocks: &'static Vec<Block> = Box::leak(Box::new(vec![Block {
        entity_id: block_id,
        function: function_id,
        parameters: Vec::new(),
        operations: vec![op_id],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: op_id,
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    }]));
    let operations: &'static Vec<Operation> = Box::leak(Box::new(vec![Operation {
        entity_id: op_id,
        block: block_id,
        ordinal: 0,
        opcode,
        operands: Vec::new(),
        result_types: vec![result],
        immediate,
    }]));
    let parameters: &'static Vec<Parameter> = Box::leak(Box::new(Vec::new()));
    LoweringInput {
        types,
        function: graph,
        parameters,
        blocks,
        operations,
        schema_epoch: SchemaEpochId::from_bytes([8; 32]),
        state_root: StateRoot::from_bytes([9; 32]),
        profile: CacheProfile::RESTRICTED_V1,
        constants: &[],
        globals: &[],
        functions: &[],
        contracts: &[],
        adapters: &[],
    }
}

#[test]
fn s3_g3_effect_refusal_pin() {
    // S2B-EFFECT-001: attempting the effect-request operation through the
    // real restricted lowering path refuses with VM_LOWER_OPCODE_UNSUPPORTED.
    let input = restricted_probe(Opcode::EffectRequest, TypeExpr::Unit, Immediate::None);
    match lower_function(input).unwrap_err() {
        LoweringError::Lower(error) => {
            assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported);
            assert_eq!(error.code().as_str(), "VM_LOWER_OPCODE_UNSUPPORTED");
        }
        LoweringError::Cfg(failure) => panic!("no CFG judgment expected: {failure}"),
    }
    println!("PIN task=S2B-EFFECT-001 refusal_code=VM_LOWER_OPCODE_UNSUPPORTED");
}

#[test]
fn s3_g3_cap_refusal_pin() {
    // S2B-CAP-001: attempting the adapter-invoke (capability-gated) operation
    // through the real restricted lowering path refuses identically.
    let input = restricted_probe(
        Opcode::AdapterInvoke,
        TypeExpr::Unit,
        Immediate::Entity(EntityId::from_bytes([0xC0; 32])),
    );
    match lower_function(input).unwrap_err() {
        LoweringError::Lower(error) => {
            assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported);
            assert_eq!(error.code().as_str(), "VM_LOWER_OPCODE_UNSUPPORTED");
        }
        LoweringError::Cfg(failure) => panic!("no CFG judgment expected: {failure}"),
    }
    println!("PIN task=S2B-CAP-001 refusal_code=VM_LOWER_OPCODE_UNSUPPORTED");
}
