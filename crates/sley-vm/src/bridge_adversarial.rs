//! Repository-native bounded adversarial lane for the slice-E8 host bridge
//! (RW-050 slice 2, §1).
//!
//! A seeded generator builds `adapter_invoke` programs across the full
//! adversarial dimension space — entries, carried-row tampering, invocation
//! shapes, input values, and fuel budgets — and production authorities judge
//! them: `lower_function` and `judge_function_operations` for lowering
//! judgment (with acceptance parity asserted on every draw), and
//! `execute_function` for execution, fuel accounting, and result validation.
//! The lane asserts determinism (every accepted program runs twice to one
//! outcome), exact refusal codes from a decision table over the draw
//! dimensions, fuel monotonicity around the measured charge, and
//! decode/re-encode round-trip identity across chained executions.
//!
//! Malformed input must fail deterministically: no panic, no hang, no
//! unbounded allocation, no host effect, no partial externally visible
//! state. Any panic or hang IS the finding.

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, BuiltinCase, BuiltinFailureKind, CaseKey, ConstData, ConstValue,
    FunctionGraph, Immediate, IntegerWidth, Opcode, Operation, OperationResultRef, Parameter,
    ParameterRole, Reachability, ResultConst, ReturnTerminator, SwitchArgument, SwitchCase,
    SwitchEdge, Terminator, TypeExpr, ValueRef, VariantSwitchTerminator, Visibility,
};

use crate::extended::{BRIDGE_MAX_ITEMS, bridge_entry_id, bridge_test_imports};
use crate::{
    CacheProfile, ExecutionLimits, ExecutionOutcome, ExecutionRequest, ExecutionTermination,
    LowerErrorCode, LoweringError, LoweringInput, ResourceKind, execute_function,
    judge_function_operations, lower_function,
};

/// Bounded draw count per seed: every draw builds and judges one program,
/// one to four executions. Three seeds cover the dimension space.
const DRAWS_PER_SEED: usize = 128;
const SEEDS: [u64; 3] = [
    0x9E37_79B9_7F4A_7C15,
    0xBF58_476D_1CE4_E5B9,
    0x94D0_49BB_1331_11EB,
];

/// Small payload ceiling: lane inputs stay tiny (true 2^20 boundaries live
/// in the fixed tests below, which already build megabyte inputs).
const MAX_LANE_BYTES: usize = 8;
const MAX_LANE_ELEMENTS: usize = 4;

/// Minimal deterministic generator: the lane stays dependency-free and
/// exactly reproducible, so no external RNG crate.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64star, nonzero seeds only (see SEEDS).
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % bound as u64).expect("lane bounds fit the host word")
    }

    fn byte(&mut self) -> u8 {
        u8::try_from(self.below(256)).expect("a byte fits in a byte")
    }
}

fn id(byte: u8) -> EntityId {
    EntityId::from_bytes([byte; 32])
}

fn u8_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(8))
}

fn u8vec_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(u8_type()))
}

fn index_error() -> TypeExpr {
    TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)
}

fn bridge_result(ok: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(ok),
        error: Box::new(index_error()),
    }
}

fn unit_value() -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    }
}

fn octet(value: u8) -> ConstValue {
    ConstValue {
        value_type: u8_type(),
        data: ConstData::UInt(u128::from(value)),
    }
}

/// One generated adversarial draw.
#[derive(Clone, Copy)]
struct Draw {
    /// 0 = B2V1, 1 = V2B1, 2 = PSH1, 3 = unlanded fourth identity.
    entry: usize,
    /// 0 = frozen row; 1..=11 = single-field tamper classes (`carried_row`).
    row: usize,
    /// 0 = correct shape; 1..=10 = invocation-shape tamper classes.
    shape: usize,
    /// 0 = canonical small, 1 = empty, 2/3 = mistyped data at the boundary.
    values: usize,
    /// 0 = generous, 1 = exact measured, 2 = measured-1, 3 = starved,
    /// 4 = zero fuel, 5 = zero instructions.
    limits: usize,
}

fn draw(rng: &mut Rng) -> Draw {
    Draw {
        entry: rng.below(4),
        row: rng.below(12),
        shape: rng.below(11),
        values: rng.below(4),
        limits: rng.below(6),
    }
}

fn frozen_draw(entry: usize) -> Draw {
    Draw {
        entry,
        row: 0,
        shape: 0,
        values: 0,
        limits: 0,
    }
}

/// The carried row for a draw: the frozen row for the entry (the
/// representative `UInt(8)` push instantiation), the B2V shape under the
/// unlanded fourth identity, or one tampered single-field mutant.
fn carried_row(draw: &Draw) -> AdapterImport {
    let frozen = bridge_test_imports();
    let base = match draw.entry {
        0 => frozen[0].clone(),
        1 => frozen[1].clone(),
        2 => frozen[2].clone(),
        _ => AdapterImport {
            entity_id: bridge_entry_id(*b"XXXX"),
            adapter_id: *bridge_entry_id(*b"XXXX").as_bytes(),
            abi_version: 1,
            request_type: TypeExpr::Bytes,
            response_type: u8vec_type(),
            failure_type: index_error(),
            effects: Vec::new(),
        },
    };
    match draw.row {
        0 => base,
        // Wrong external adapter identity bytes.
        1 => AdapterImport {
            adapter_id: [9_u8; 32],
            ..base
        },
        // Wrong ABI version (one above; zero is class 9).
        2 => AdapterImport {
            abi_version: 2,
            ..base
        },
        // Wrong failure-code family: arithmetic.
        3 => AdapterImport {
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic),
            ..base
        },
        // Wrong failure-code family: capability.
        4 => AdapterImport {
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Capability),
            ..base
        },
        // Effect-bearing row masquerading as pure: a non-empty effect list
        // is not a landed entry however pure the schemas look.
        5 => AdapterImport {
            effects: vec![id(9)],
            ..base
        },
        // Conversion schemas swapped: a well-formed but unlanded shape.
        6 => AdapterImport {
            request_type: base.response_type.clone(),
            response_type: base.request_type.clone(),
            ..base
        },
        // Schema confusion across similar rows: 16-bit octet vector.
        7 => AdapterImport {
            request_type: TypeExpr::Vector(Box::new(TypeExpr::UInt(IntegerWidth::from_bits(16)))),
            response_type: TypeExpr::Bytes,
            ..base
        },
        // Push relationship pin broken: response is not Vector<request>.
        8 => AdapterImport {
            request_type: u8_type(),
            response_type: TypeExpr::Vector(Box::new(TypeExpr::Bool)),
            ..base
        },
        // Zero ABI version.
        9 => AdapterImport {
            abi_version: 0,
            ..base
        },
        // Duplicate-key failure family: well-formed, unlanded.
        10 => AdapterImport {
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey),
            ..base
        },
        // Bytes/Text confusion: text where bytes belong.
        11 => AdapterImport {
            request_type: TypeExpr::Text,
            response_type: TypeExpr::Vector(Box::new(u8_type())),
            ..base
        },
        _ => unreachable!("row dimension is bounded"),
    }
}

/// Parameter (scope, request) types for a draw: derived from the carried
/// row when the shape is correct, mutated per shape-tamper class.
fn parameter_types(draw: &Draw, row: &AdapterImport) -> (TypeExpr, TypeExpr) {
    let (scope, request) = (row.response_type.clone(), row.request_type.clone());
    // Conversions pin Unit scope; push pins (response, request).
    let (scope, request) = match draw.entry {
        2 => (scope, request),
        _ => (TypeExpr::Unit, request),
    };
    match draw.shape {
        // Swapped scope/request operand order.
        4 => (request, scope),
        // Wrong scope type.
        5 => (TypeExpr::Bool, request),
        // Wrong request type.
        6 => (scope, TypeExpr::Bool),
        _ => (scope, request),
    }
}

fn result_type(draw: &Draw, row: &AdapterImport) -> TypeExpr {
    let expected = bridge_result(row.response_type.clone());
    match draw.shape {
        // Wrong result ok type.
        7 => bridge_result(TypeExpr::Bool),
        // Wrong result error family.
        8 => TypeExpr::Result {
            ok: Box::new(row.response_type.clone()),
            error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
        },
        // Result is not a Result at all.
        9 => row.response_type.clone(),
        _ => expected,
    }
}

/// The exact lowering refusal a draw must produce, or `None` when lowering
/// must accept. Resolution precedes arity: any row tamper (or the unlanded
/// fourth identity) refuses unsupported before shape is even considered;
/// the non-Entity immediate refuses before resolution.
fn expected_lowering(draw: &Draw) -> Option<LowerErrorCode> {
    if draw.shape == 3 {
        return Some(LowerErrorCode::ImmediateMismatch);
    }
    if draw.entry == 3 || draw.row != 0 {
        return Some(LowerErrorCode::OpcodeUnsupported);
    }
    match draw.shape {
        0 => None,
        // Unknown carried identity: resolution fails after the well-formed
        // Entity immediate passes the immediate check.
        10 => Some(LowerErrorCode::OpcodeUnsupported),
        _ => Some(LowerErrorCode::SignatureMismatch),
    }
}

/// One draw's owned program: every inventory lives here so lowering and
/// execution borrow locally.
struct LaneProgram {
    draw: Draw,
    row: AdapterImport,
    adapters: [AdapterImport; 1],
    types: TypeEnvironment,
    function: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}

impl LaneProgram {
    fn build(draw: Draw) -> Self {
        let row = carried_row(&draw);
        let adapters = [row.clone()];
        let types = TypeEnvironment::new(Vec::new()).expect("empty type environment is valid");
        let (scope_type, request_type) = parameter_types(&draw, &row);
        let function = id(1);
        let block = id(2);
        let scope_param = id(10);
        let request_param = id(11);
        let operation = id(100);
        let immediate = match draw.shape {
            3 => Immediate::Index(0),
            10 => Immediate::Entity(id(77)),
            _ => Immediate::Entity(row.entity_id),
        };
        let mut operands = vec![
            ValueRef::Parameter(scope_param),
            ValueRef::Parameter(request_param),
        ];
        match draw.shape {
            // One operand: under-arity.
            1 => {
                operands.pop();
            }
            // Three operands: over-arity.
            2 => {
                operands.push(ValueRef::Parameter(request_param));
            }
            _ => {}
        }
        let result = result_type(&draw, &row);
        let operation = Operation {
            entity_id: operation,
            block,
            ordinal: 0,
            opcode: Opcode::AdapterInvoke,
            operands,
            result_types: vec![result.clone()],
            immediate,
        };
        let function_graph = FunctionGraph {
            entity_id: function,
            type_parameters: Vec::new(),
            parameters: vec![scope_param, request_param],
            result_type: result,
            effects: Vec::new(),
            entry_block: block,
            blocks: vec![block],
            contracts: Vec::new(),
            visibility: Visibility::Private,
        };
        let parameters = vec![
            Parameter {
                entity_id: scope_param,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: scope_type,
            },
            Parameter {
                entity_id: request_param,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 1,
                value_type: request_type,
            },
        ];
        let blocks = vec![Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: vec![operation.entity_id],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: operation.entity_id,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        }];
        Self {
            draw,
            row,
            adapters,
            types,
            function: function_graph,
            parameters,
            blocks,
            operations: vec![operation],
        }
    }

    fn input(&self) -> LoweringInput<'_> {
        LoweringInput {
            types: &self.types,
            function: &self.function,
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &self.operations,
            schema_epoch: SchemaEpochId::from_bytes([8; 32]),
            state_root: StateRoot::from_bytes([9; 32]),
            profile: CacheProfile::EXTENDED_V1,
            constants: &[],
            globals: &[],
            functions: &[],
            contracts: &[],
            adapters: &self.adapters,
        }
    }

    fn run(
        &self,
        inputs: &[ConstValue],
        limits: ExecutionLimits,
    ) -> Result<ExecutionOutcome, crate::ExecutionError> {
        execute_function(
            self.input(),
            ExecutionRequest {
                inputs: inputs.to_vec(),
                limits,
            },
        )
    }

    fn lowering_code(error: &LoweringError) -> LowerErrorCode {
        match error {
            LoweringError::Lower(code) => code.code(),
            LoweringError::Cfg(failure) => panic!("no CFG failure in lane programs: {failure}"),
        }
    }

    /// Runs a refused draw to its verdict: lowering, the §3.1 judgment,
    /// and execution refuse with the identical exact code.
    fn run_refused(&self, code: LowerErrorCode, rng: &mut Rng) {
        let context = format!(
            "draw entry={} row={} shape={}",
            self.draw.entry, self.draw.row, self.draw.shape
        );
        // Judgment parity: the §3.1 entry refuses exactly what lowering
        // refuses, so neither static path can silently diverge.
        assert_eq!(
            Self::lowering_code(
                &lower_function(self.input()).expect_err("tampered draw must not lower")
            ),
            code,
            "lowering refusal drifted for {context}"
        );
        assert_eq!(
            Self::lowering_code(
                &judge_function_operations(self.input()).expect_err("tampered draw must not judge")
            ),
            code,
            "judgment refusal drifted for {context}"
        );
        let inputs = canonical_inputs(&self.draw, rng, &self.row);
        match self.run(&inputs, generous_limits()) {
            Err(crate::ExecutionError::Lowering(error)) => assert_eq!(
                Self::lowering_code(&error),
                code,
                "execution refusal drifted for {context}"
            ),
            other => panic!("tampered draw must fail at lowering, got {other:?} for {context}"),
        }
    }

    /// Runs an accepted draw to its verdict: determinism, exact success
    /// values, and fuel monotonicity around the measured charge.
    fn run_accepted(&self, rng: &mut Rng) -> Option<Chain> {
        lower_function(self.input()).expect("frozen well-shaped draw must lower");
        judge_function_operations(self.input()).expect("frozen well-shaped draw must judge");
        if self.draw.values >= 2 {
            // Mistyped data at the boundary: input judgment refuses
            // deterministically; the bridge arm never sees it.
            let inputs = mistyped_inputs(&self.draw, &self.row);
            let first = self.run(&inputs, generous_limits());
            let second = self.run(&inputs, generous_limits());
            assert_eq!(
                first.is_ok(),
                second.is_ok(),
                "boundary refusal nondeterministic"
            );
            assert!(
                !is_success(&first),
                "mistyped boundary data must never succeed"
            );
            return None;
        }
        let inputs = canonical_inputs(&self.draw, rng, &self.row);
        // Measure under generous limits, then assert determinism and fuel
        // monotonicity around the measured charge.
        let measured = self
            .run(&inputs, generous_limits())
            .expect("frozen canonical draw executes");
        let rerun = self
            .run(&inputs, generous_limits())
            .expect("frozen canonical draw re-executes");
        assert_eq!(measured, rerun, "bridge execution was not deterministic");
        let ExecutionTermination::Success(value) = &measured.termination else {
            panic!("small frozen draw must succeed: {:?}", measured.termination);
        };
        assert_eq!(
            value,
            &expected_success(&self.draw, &inputs, &self.row),
            "bridge representation mapping drifted"
        );
        let fuel = measured.fuel_used;
        assert!(fuel > 0, "every execution charges fuel");
        self.run_budgeted(&inputs, &measured, fuel);
        chain_payload(&self.draw, &inputs, value)
    }

    /// Runs the fuel-budget dimension around the measured charge: exact
    /// reruns identically, short/starved/zero budgets terminate on fuel,
    /// zero instructions terminate on instructions.
    fn run_budgeted(&self, inputs: &[ConstValue], measured: &ExecutionOutcome, fuel: u64) {
        let budgeted = |max_fuel: u64| {
            self.run(
                inputs,
                ExecutionLimits {
                    max_fuel,
                    ..generous_limits()
                },
            )
            .expect("budgeted draw executes")
        };
        match self.draw.limits {
            0 => {}
            // Exact measured charge re-executes identically modulo the
            // observation, which binds the request limits by design.
            1 => {
                let exact = budgeted(fuel);
                assert_eq!(
                    outcome_core(&exact),
                    outcome_core(measured),
                    "exact fuel charge changed the outcome"
                );
                assert_eq!(
                    exact.cache_key, measured.cache_key,
                    "exact fuel charge changed the cache key"
                );
            }
            // One fuel below measured terminates on fuel: pre-charge
            // atomicity — no partial success, no invariant fault.
            2 => {
                assert_eq!(
                    budgeted(fuel.saturating_sub(1)).termination,
                    ExecutionTermination::ResourceLimit(ResourceKind::Fuel),
                    "one fuel short of measured must terminate on fuel"
                );
            }
            // Starved budget: terminates on fuel exactly when the measured
            // charge exceeds it, else the identical success.
            3 => {
                let starved = budgeted(3);
                if fuel > 3 {
                    assert_eq!(
                        starved.termination,
                        ExecutionTermination::ResourceLimit(ResourceKind::Fuel),
                        "starved budget must terminate on fuel"
                    );
                } else {
                    assert_eq!(
                        outcome_core(&starved),
                        outcome_core(measured),
                        "affordable starved budget must succeed identically"
                    );
                }
            }
            // Zero fuel: the first charge terminates, before any arm
            // allocates or converts.
            4 => {
                assert_eq!(
                    budgeted(0).termination,
                    ExecutionTermination::ResourceLimit(ResourceKind::Fuel),
                    "zero fuel must terminate on fuel"
                );
            }
            // Zero instructions: the first dispatch terminates.
            _ => {
                let zero = self
                    .run(
                        inputs,
                        ExecutionLimits {
                            max_instructions: 0,
                            ..generous_limits()
                        },
                    )
                    .expect("zero-instruction draw executes");
                assert_eq!(
                    zero.termination,
                    ExecutionTermination::ResourceLimit(ResourceKind::Instruction),
                    "zero instructions must terminate on instructions"
                );
            }
        }
    }
}

/// A successful conversion draw that can chain: the original input payload
/// plus the answered value, for decode/re-encode identity across executions.
enum Chain {
    /// B2V1 success: original bytes plus the answered octet vector.
    Decode {
        bytes: Vec<u8>,
        vector: Vec<ConstValue>,
    },
    /// V2B1 success: original octet vector plus the answered bytes.
    Encode {
        vector: Vec<ConstValue>,
        bytes: Vec<u8>,
    },
}

fn chain_payload(draw: &Draw, inputs: &[ConstValue], value: &ConstValue) -> Option<Chain> {
    let ConstData::Result(result) = &value.data else {
        panic!("bridge answers results");
    };
    let ResultConst::Ok(answer) = result else {
        panic!("small conversion succeeds");
    };
    match draw.entry {
        0 => {
            let ConstData::Bytes(bytes) = &inputs[1].data else {
                panic!("lane builds bytes requests for B2V1");
            };
            let ConstData::Sequence(items) = &answer.data else {
                panic!("B2V1 answers vectors");
            };
            Some(Chain::Decode {
                bytes: bytes.clone(),
                vector: items.clone(),
            })
        }
        1 => {
            let ConstData::Sequence(items) = &inputs[1].data else {
                panic!("lane builds vector requests for V2B1");
            };
            let ConstData::Bytes(bytes) = &answer.data else {
                panic!("V2B1 answers bytes");
            };
            Some(Chain::Encode {
                vector: items.clone(),
                bytes: bytes.clone(),
            })
        }
        _ => None,
    }
}

/// Canonical small inputs for a draw: correct value types, lengths from the
/// generator. Empty when `values == 1`.
fn canonical_inputs(draw: &Draw, rng: &mut Rng, row: &AdapterImport) -> Vec<ConstValue> {
    let (scope_type, request_type) = parameter_types(draw, row);
    let empty = draw.values == 1;
    let scope = match &scope_type {
        TypeExpr::Unit => unit_value(),
        TypeExpr::Vector(inner) => {
            let len = if empty {
                0
            } else {
                rng.below(MAX_LANE_ELEMENTS + 1)
            };
            ConstValue {
                value_type: scope_type.clone(),
                data: ConstData::Sequence(
                    (0..len)
                        .map(|_| ConstValue {
                            value_type: inner.as_ref().clone(),
                            data: element_data(rng, inner.as_ref()),
                        })
                        .collect(),
                ),
            }
        }
        _ => ConstValue {
            value_type: scope_type.clone(),
            data: ConstData::Bool(true),
        },
    };
    let request = match &request_type {
        TypeExpr::Bytes => {
            let len = if empty {
                0
            } else {
                rng.below(MAX_LANE_BYTES + 1)
            };
            ConstValue {
                value_type: TypeExpr::Bytes,
                data: ConstData::Bytes((0..len).map(|_| rng.byte()).collect()),
            }
        }
        TypeExpr::Vector(inner) => {
            let len = if empty {
                0
            } else {
                rng.below(MAX_LANE_ELEMENTS + 1)
            };
            ConstValue {
                value_type: request_type.clone(),
                data: ConstData::Sequence(
                    (0..len)
                        .map(|_| ConstValue {
                            value_type: inner.as_ref().clone(),
                            data: element_data(rng, inner.as_ref()),
                        })
                        .collect(),
                ),
            }
        }
        _ => ConstValue {
            value_type: request_type.clone(),
            data: element_data(rng, &request_type),
        },
    };
    vec![scope, request]
}

fn element_data(rng: &mut Rng, value_type: &TypeExpr) -> ConstData {
    match value_type {
        TypeExpr::Bool => ConstData::Bool(rng.below(2) == 0),
        TypeExpr::UInt(_) => ConstData::UInt(u128::from(rng.byte())),
        TypeExpr::Unit => ConstData::Unit,
        _ => ConstData::Bool(true),
    }
}

/// Mistyped data at the input boundary: correct value types, wrong data
/// shapes. These must fail input judgment deterministically — never reach
/// the bridge arm as mistyped values, never panic.
fn mistyped_inputs(draw: &Draw, row: &AdapterImport) -> Vec<ConstValue> {
    let (scope_type, request_type) = parameter_types(draw, row);
    let scope = match scope_type {
        TypeExpr::Unit => unit_value(),
        other => ConstValue {
            value_type: other,
            data: ConstData::Unit,
        },
    };
    let request = match draw.entry {
        // Bytes-typed request carrying text data.
        0 => ConstValue {
            value_type: request_type,
            data: ConstData::Text("not-bytes".to_string()),
        },
        // Octet-vector request carrying a 16-bit element value.
        1 => ConstValue {
            value_type: request_type,
            data: ConstData::Sequence(vec![ConstValue {
                value_type: u8_type(),
                data: ConstData::UInt(300),
            }]),
        },
        _ => ConstValue {
            value_type: request_type,
            data: ConstData::Bytes(vec![1, 2, 3]),
        },
    };
    vec![scope, request]
}

fn generous_limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: 1_000,
        max_fuel: 100_000,
        max_value_units: 10_000_000,
        max_output_units: 10_000_000,
        cancel_at_fuel: None,
    }
}

/// The exact expected success value for frozen, well-shaped, small draws:
/// the E8 representation mapping, computed independently from the draw.
fn expected_success(draw: &Draw, inputs: &[ConstValue], row: &AdapterImport) -> ConstValue {
    let ok_data = match draw.entry {
        0 => {
            let ConstData::Bytes(bytes) = &inputs[1].data else {
                panic!("lane builds bytes requests for B2V1");
            };
            ConstData::Sequence(bytes.iter().map(|byte| octet(*byte)).collect())
        }
        1 => {
            let ConstData::Sequence(items) = &inputs[1].data else {
                panic!("lane builds vector requests for V2B1");
            };
            ConstData::Bytes(
                items
                    .iter()
                    .map(|item| {
                        let ConstData::UInt(byte) = item.data else {
                            panic!("lane builds octet elements");
                        };
                        u8::try_from(byte).expect("lane octets fit in a byte")
                    })
                    .collect(),
            )
        }
        _ => {
            let ConstData::Sequence(items) = &inputs[0].data else {
                panic!("lane builds vector scopes for PSH1");
            };
            let mut grown = items.clone();
            grown.push(inputs[1].clone());
            ConstData::Sequence(grown)
        }
    };
    ConstValue {
        value_type: bridge_result(row.response_type.clone()),
        data: ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
            value_type: row.response_type.clone(),
            data: ok_data,
        }))),
    }
}

/// Outcome identity modulo the observation: the observation preimage binds
/// the request limits, so reruns under different budgets share termination,
/// charges, and cache key but never the observation id.
fn outcome_core(outcome: &ExecutionOutcome) -> (&ExecutionTermination, u64, u64, u64) {
    (
        &outcome.termination,
        outcome.fuel_used,
        outcome.instruction_count,
        outcome.peak_value_units,
    )
}

fn is_success(outcome: &Result<ExecutionOutcome, crate::ExecutionError>) -> bool {
    matches!(
        outcome,
        Ok(result) if matches!(result.termination, ExecutionTermination::Success(_))
    )
}

/// The repository-native E8 adversarial lane: seeded, bounded, fully
/// deterministic. Every draw is judged by production code; the lane only
/// generates inputs and classifies verdicts from the draw dimensions.
#[test]
fn bridge_adversarial_lane_is_deterministic_and_fail_closed() {
    let mut round_trips = 0_usize;
    for seed in SEEDS {
        let mut rng = Rng(seed);
        for _ in 0..DRAWS_PER_SEED {
            let program = LaneProgram::build(draw(&mut rng));
            let next = match expected_lowering(&program.draw) {
                Some(code) => {
                    program.run_refused(code, &mut rng);
                    None
                }
                None => program.run_accepted(&mut rng),
            };
            match next {
                // Decode chains: the answered vector re-encodes to the
                // identical original bytes through a fresh V2B1 program.
                Some(Chain::Decode { bytes, vector }) => {
                    let back = LaneProgram::build(frozen_draw(1));
                    let outcome = back
                        .run(
                            &[
                                unit_value(),
                                ConstValue {
                                    value_type: u8vec_type(),
                                    data: ConstData::Sequence(vector),
                                },
                            ],
                            generous_limits(),
                        )
                        .expect("round-trip re-encode executes");
                    let ExecutionTermination::Success(value) = &outcome.termination else {
                        panic!("round-trip re-encode succeeds: {:?}", outcome.termination);
                    };
                    let ConstData::Result(result) = &value.data else {
                        panic!("bridge answers results");
                    };
                    let ResultConst::Ok(answer) = result else {
                        panic!("small re-encode succeeds");
                    };
                    assert_eq!(
                        answer.data,
                        ConstData::Bytes(bytes),
                        "decode/re-encode is the identity on bytes"
                    );
                    round_trips += 1;
                }
                // Encode chains: the answered bytes re-decode to the
                // identical original vector through a fresh B2V1 program.
                Some(Chain::Encode { vector, bytes }) => {
                    let back = LaneProgram::build(frozen_draw(0));
                    let outcome = back
                        .run(
                            &[
                                unit_value(),
                                ConstValue {
                                    value_type: TypeExpr::Bytes,
                                    data: ConstData::Bytes(bytes),
                                },
                            ],
                            generous_limits(),
                        )
                        .expect("round-trip re-decode executes");
                    let ExecutionTermination::Success(value) = &outcome.termination else {
                        panic!("round-trip re-decode succeeds: {:?}", outcome.termination);
                    };
                    let ConstData::Result(result) = &value.data else {
                        panic!("bridge answers results");
                    };
                    let ResultConst::Ok(answer) = result else {
                        panic!("small re-decode succeeds");
                    };
                    assert_eq!(
                        answer.data,
                        ConstData::Sequence(vector),
                        "encode/re-decode is the identity on octet vectors"
                    );
                    round_trips += 1;
                }
                None => {}
            }
        }
    }
    assert!(round_trips > 0, "the lane must exercise round-trips");
}

/// Zero-length inputs cross as empty values, never as refusals.
#[test]
fn bridge_empty_inputs_cross_as_empty() {
    for entry in [0, 1, 2] {
        let program = LaneProgram::build(Draw {
            entry,
            row: 0,
            shape: 0,
            values: 1,
            limits: 0,
        });
        let inputs = canonical_inputs(&program.draw, &mut Rng(SEEDS[0]), &program.row);
        let outcome = program
            .run(&inputs, generous_limits())
            .expect("empty-input draw executes");
        let ExecutionTermination::Success(value) = &outcome.termination else {
            panic!("empty inputs cross: {:?}", outcome.termination);
        };
        let ConstData::Result(result) = &value.data else {
            panic!("bridge answers results");
        };
        assert!(
            matches!(result, ResultConst::Ok(_)),
            "empty inputs answer Ok, never capacity refusal"
        );
    }
}

/// The asymmetric capacity boundary, pinned at exactly 2^20
/// (`BRIDGE_MAX_ITEMS`) against the S20-210 constant ceiling
/// (`MAX_CONSTANT_ELEMENTS` = 1,000,000 < 2^20):
///
/// - B2V1's boundary is directly reachable (Bytes payloads to 16 MiB): past
///   the cap answers `Err(Index, 2)`, at the cap succeeds (covered by
///   `e8_bridge_capacity_refusal_is_a_typed_index_value`; not duplicated).
/// - Cap-sized vectors can never re-enter as inputs (1,048,576 elements >
///   the 1,000,000-element constant ceiling), so V2B1-at-cap and
///   PSH1-at-max are reached through single-execution chains: B2V1 decodes
///   cap-sized bytes, a `VariantSwitch` deconstructs the answered `Result`,
///   and the payload feeds V2B1/PSH1 in the taken arm. Past-cap vectors are
///   unconstructible (inputs capped, B2V1 caps its output, push refuses at
///   the cap), so V2B1's over-cap arm is unreachable defense in depth —
///   recorded, not tested through an impossible input.
/// - Oversized direct inputs fail first at input judgment with the S20-210
///   resource code, deterministically, before any bridge arm runs.
#[test]
fn bridge_capacity_boundary_is_exact_at_two_to_twenty() {
    let roomy = ExecutionLimits {
        max_instructions: 100_000,
        max_fuel: 100_000_000,
        max_value_units: 300_000_000,
        max_output_units: 300_000_000,
        cancel_at_fuel: None,
    };
    // V2B1 of a cap-sized vector succeeds to the identical megabyte.
    let back = run_chained(
        *b"V2B1",
        vec![
            unit_value(),
            ConstValue {
                value_type: TypeExpr::Bytes,
                data: ConstData::Bytes(vec![7_u8; BRIDGE_MAX_ITEMS]),
            },
        ],
        roomy,
    );
    let ExecutionTermination::Success(back_value) = &back.termination else {
        panic!("V2B1 at exactly the cap succeeds: {:?}", back.termination);
    };
    let ConstData::Result(back_result) = &back_value.data else {
        panic!("bridge answers results");
    };
    let ResultConst::Ok(back_bytes) = back_result else {
        panic!("cap-sized V2B1 answers Ok");
    };
    assert_eq!(
        back_bytes.data,
        ConstData::Bytes(vec![7_u8; BRIDGE_MAX_ITEMS]),
        "cap-sized chain is lossless"
    );
    // PSH1 onto the cap-sized vector refuses with Index code 2 without
    // doing per-element work: the total charge is the B2V1 length charge
    // plus dispatch, never a second pass.
    let refused = run_chained(
        *b"PSH1",
        vec![
            unit_value(),
            ConstValue {
                value_type: TypeExpr::Bytes,
                data: ConstData::Bytes(vec![7_u8; BRIDGE_MAX_ITEMS]),
            },
            octet(8),
        ],
        roomy,
    );
    assert_capacity_refusal(&refused.termination, &u8vec_type());
    assert!(
        u64::try_from(BRIDGE_MAX_ITEMS).is_ok_and(|cap| refused.fuel_used < cap + 1_000),
        "at-max push refusal does no per-element work: {}",
        refused.fuel_used
    );
    // A past-cap direct vector input fails first at input judgment with
    // the S20-210 resource code — deterministically, twice equal, before
    // any bridge arm runs.
    let direct = LaneProgram::build(frozen_draw(1));
    let over: Vec<ConstValue> = (0..=BRIDGE_MAX_ITEMS).map(|_| octet(7)).collect();
    let oversized = [
        unit_value(),
        ConstValue {
            value_type: u8vec_type(),
            data: ConstData::Sequence(over),
        },
    ];
    for _ in 0..2 {
        match direct.run(&oversized, roomy) {
            Err(crate::ExecutionError::Type(error)) => assert_eq!(
                error.code(),
                sley_check::TypeErrorCode::ResourceLimit,
                "oversized inputs fail at input judgment with the resource code"
            ),
            other => panic!("oversized inputs must fail at input judgment: {other:?}"),
        }
    }
}

/// One chained program's owned inventories: B2V1 decodes, a
/// `VariantSwitch` deconstructs the answered `Result`, and the taken arm
/// feeds the payload to the second entry.
struct ChainedProgram {
    types: TypeEnvironment,
    function: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    adapters: [AdapterImport; 2],
}

fn chained_program(entry: [u8; 4]) -> ChainedProgram {
    let frozen = bridge_test_imports();
    let second_row = if entry == *b"V2B1" {
        frozen[1].clone()
    } else {
        frozen[2].clone()
    };
    let adapters = [frozen[0].clone(), second_row];
    let push_chain = entry == *b"PSH1";
    let second_result = if push_chain {
        bridge_result(u8vec_type())
    } else {
        bridge_result(TypeExpr::Bytes)
    };
    let (function_params, parameters) = chained_parameters(push_chain);
    let function = id(1);
    ChainedProgram {
        types: TypeEnvironment::new(Vec::new()).expect("empty type environment is valid"),
        function: FunctionGraph {
            entity_id: function,
            type_parameters: Vec::new(),
            parameters: function_params,
            result_type: second_result.clone(),
            effects: Vec::new(),
            entry_block: id(2),
            blocks: vec![id(2), id(3), id(4)],
            contracts: Vec::new(),
            visibility: Visibility::Private,
        },
        parameters,
        blocks: chained_blocks(),
        operations: chained_operations(push_chain, &second_result),
        adapters,
    }
}

/// Function/block parameter inventories for a chained program. Entity ids
/// are recomputed by every builder from the same constants, so no id table
/// crosses function boundaries.
fn chained_parameters(push_chain: bool) -> (Vec<EntityId>, Vec<Parameter>) {
    let function = id(1);
    let mut function_params = vec![id(10), id(11)];
    let mut parameters = vec![
        Parameter {
            entity_id: id(10),
            owner: function,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Unit,
        },
        Parameter {
            entity_id: id(11),
            owner: function,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: TypeExpr::Bytes,
        },
        Parameter {
            entity_id: id(110),
            owner: id(3),
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: u8vec_type(),
        },
        Parameter {
            entity_id: id(111),
            owner: id(4),
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: index_error(),
        },
    ];
    if push_chain {
        function_params.push(id(12));
        parameters.push(Parameter {
            entity_id: id(12),
            owner: function,
            role: ParameterRole::Function,
            ordinal: 2,
            value_type: u8_type(),
        });
    }
    (function_params, parameters)
}

fn chained_operations(push_chain: bool, second_result: &TypeExpr) -> Vec<Operation> {
    let second_operands = if push_chain {
        vec![ValueRef::Parameter(id(110)), ValueRef::Parameter(id(12))]
    } else {
        vec![ValueRef::Parameter(id(10)), ValueRef::Parameter(id(110))]
    };
    let second_immediate = if push_chain {
        bridge_entry_id(*b"PSH1")
    } else {
        bridge_entry_id(*b"V2B1")
    };
    vec![
        Operation {
            entity_id: id(100),
            block: id(2),
            ordinal: 0,
            opcode: Opcode::AdapterInvoke,
            operands: vec![ValueRef::Parameter(id(10)), ValueRef::Parameter(id(11))],
            result_types: vec![bridge_result(u8vec_type())],
            immediate: Immediate::Entity(bridge_entry_id(*b"B2V1")),
        },
        Operation {
            entity_id: id(101),
            block: id(3),
            ordinal: 0,
            opcode: Opcode::AdapterInvoke,
            operands: second_operands,
            result_types: vec![second_result.clone()],
            immediate: Immediate::Entity(second_immediate),
        },
        Operation {
            entity_id: id(102),
            block: id(4),
            ordinal: 0,
            opcode: Opcode::ResultErr,
            operands: vec![ValueRef::Parameter(id(111))],
            result_types: vec![second_result.clone()],
            immediate: Immediate::None,
        },
    ]
}

fn chained_blocks() -> Vec<Block> {
    let function = id(1);
    vec![
        Block {
            entity_id: id(2),
            function,
            parameters: Vec::new(),
            operations: vec![id(100)],
            terminator: Terminator::VariantSwitch(VariantSwitchTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: id(100),
                    result_index: 0,
                }),
                cases: vec![
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Ok),
                        edge: SwitchEdge {
                            target: id(3),
                            arguments: vec![SwitchArgument::CasePayload],
                        },
                    },
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Err),
                        edge: SwitchEdge {
                            target: id(4),
                            arguments: vec![SwitchArgument::CasePayload],
                        },
                    },
                ],
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: id(3),
            function,
            parameters: vec![id(110)],
            operations: vec![id(101)],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: id(101),
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: id(4),
            function,
            parameters: vec![id(111)],
            operations: vec![id(102)],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: id(102),
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        },
    ]
}

/// Runs a single-execution chain: B2V1 decodes the bytes input, a
/// `VariantSwitch` deconstructs the answered `Result`, and the taken arm
/// feeds the payload to the second entry — V2B1 with the Unit scope, PSH1
/// with the octet third input. The Err arm wraps the failure back into the
/// shared result type, so both arms return one type.
fn run_chained(
    entry: [u8; 4],
    inputs: Vec<ConstValue>,
    limits: ExecutionLimits,
) -> ExecutionOutcome {
    let program = chained_program(entry);
    execute_function(
        LoweringInput {
            types: &program.types,
            function: &program.function,
            parameters: &program.parameters,
            blocks: &program.blocks,
            operations: &program.operations,
            schema_epoch: SchemaEpochId::from_bytes([8; 32]),
            state_root: StateRoot::from_bytes([9; 32]),
            profile: CacheProfile::EXTENDED_V1,
            constants: &[],
            globals: &[],
            functions: &[],
            contracts: &[],
            adapters: &program.adapters,
        },
        ExecutionRequest { inputs, limits },
    )
    .expect("chained cap program executes")
}

fn assert_capacity_refusal(termination: &ExecutionTermination, ok_type: &TypeExpr) {
    let ExecutionTermination::Success(value) = termination else {
        panic!("capacity refusal answers, never terminates: {termination:?}");
    };
    assert_eq!(value.value_type, bridge_result(ok_type.clone()));
    let ConstData::Result(result) = &value.data else {
        panic!("bridge answers results");
    };
    let ResultConst::Err(failure) = result else {
        panic!("past-cap answers Err");
    };
    assert_eq!(
        failure.data,
        ConstData::BuiltinFailure(sley_ssmc::BuiltinFailureValue {
            kind: BuiltinFailureKind::Index,
            code: crate::extended::BRIDGE_CAPACITY_CODE,
        }),
        "capacity refusal carries Index code 2"
    );
}
