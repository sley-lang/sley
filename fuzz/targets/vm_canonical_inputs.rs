#![allow(unsafe_code)]
#![no_main]

use core::slice;

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, BuiltinFailureKind, ConstData, ConstValue, ConstantDefinition, Immediate,
    IntegerWidth, Opcode, Operation, OperationResultRef, Parameter, ParameterRole, Reachability,
    ResultConst, ReturnTerminator, Terminator, TypeExpr, ValueRef, Visibility,
};
use sley_vm::{
    CacheProfile, ExecutionError, ExecutionLimits, ExecutionRequest, ExecutionTermination,
    LowerErrorCode, LoweringError, LoweringInput, ResourceKind, derive_observation_id,
    execute_function, judge_function_operations, lower_function, validated_execution_input_hashes,
};

const MAX_FUZZ_INPUT_BYTES: usize = 4096;
const MAX_RAW_INPUTS: usize = 4;
const MAX_COLLECTION_ITEMS: usize = 4;
const MAX_PAYLOAD_BYTES: usize = 32;
const FIXTURE_COUNT: u8 = 9;
/// Extended-profile fixtures, one per landed opcode family beyond E1.
const EXTENDED_FIXTURE_COUNT: u8 = 9;

#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn LLVMFuzzerTestOneInput(data: *const u8, len: usize) -> i32 {
    if len == 0 {
        return 0;
    }
    let input = unsafe { slice::from_raw_parts(data, len) };
    fuzz_one(input);
    0
}

fn fuzz_one(input: &[u8]) {
    if input.len() > MAX_FUZZ_INPUT_BYTES {
        return;
    }

    let mut cursor = Cursor::new(input);
    let fixture = vm_fixture(cursor.byte() % FIXTURE_COUNT);
    // Fixed-position lane header: every lane decision is consumed before any
    // variable-length value construction, so a seed byte selects the lane it
    // names regardless of how many bytes the outer fixture's canonical value
    // consumes. A header read after the inputs would shift by the fixture's
    // arity and collapse most families onto the filler default.
    let family_gate = cursor.byte();
    let family_selector = cursor.byte() % EXTENDED_FIXTURE_COUNT;
    let extended_lane = cursor.byte() % 2 == 1;
    let map_order_lane_flag = cursor.byte().is_multiple_of(2);
    let canonical_inputs = cursor.byte().is_multiple_of(4);
    let inputs = if canonical_inputs {
        fixture
            .expected_input_types
            .iter()
            .map(|value_type| canonical_value(value_type, &mut cursor))
            .collect()
    } else {
        (0..cursor.bounded(MAX_RAW_INPUTS))
            .map(|_| raw_value(&mut cursor))
            .collect()
    };
    let limits_profile = cursor.byte() % 6;
    let request = ExecutionRequest {
        inputs,
        limits: generated_limits(limits_profile, &mut cursor),
    };
    // The extended profile lane (E1): the same Boolean fixtures lower and
    // execute under `EXTENDED_V1`, and every termination must equal the
    // restricted profile's while the cache keys differ.
    let types = TypeEnvironment::new(Vec::new()).expect("empty type environment is valid");
    let lowering = fixture.lowering_input(&types, CacheProfile::RESTRICTED_V1);
    if extended_lane {
        let extended = fixture.lowering_input(&types, CacheProfile::EXTENDED_V1);
        let restricted_outcome = execute_function(lowering, request.clone());
        let extended_outcome = execute_function(extended, request.clone());
        match (&restricted_outcome, &extended_outcome) {
            (Ok(left), Ok(right)) => {
                assert_eq!(
                    left.termination, right.termination,
                    "cross-profile termination drifted"
                );
                assert_ne!(
                    left.cache_key, right.cache_key,
                    "profiles shared a cache key"
                );
            }
            (Err(left), Err(right)) => assert_eq!(
                left.to_string(),
                right.to_string(),
                "cross-profile failure drifted"
            ),
            _ => panic!("cross-profile acceptance drifted"),
        }
    }

    // The extended family lane (E2 through E6 plus E1 under the extended
    // profile, E7a, and E8): fixtures whose opcodes exist only under
    // `EXTENDED_V1` execute deterministically there, retain their observation
    // identity, and are refused by the restricted profile.
    if family_gate.is_multiple_of(3) {
        extended_family_lane(family_selector, &mut cursor);
        // The bridge adversarial subl lane tampers rows, shapes, and
        // budgets around the frozen path the shared lane just covered.
        if family_selector == 8 {
            bridge_sublane(&mut cursor);
        }
    }

    // The map-order lane (E4): a supplied ordered map must arrive in the
    // order of its keys' canonical bytes, because `equal` and `value_hash`
    // read that order structurally.
    if map_order_lane_flag {
        map_order_lane(&mut cursor);
    }

    let first_hashes = validated_execution_input_hashes(lowering, &request);
    let second_hashes = validated_execution_input_hashes(lowering, &request);
    assert_eq!(
        first_hashes, second_hashes,
        "VM canonical-input hash judgment was not deterministic"
    );

    let first = execute_function(lowering, request.clone());
    let second = execute_function(lowering, request.clone());
    assert_eq!(first, second, "VM execution judgment was not deterministic");

    if canonical_inputs && limits_profile == 0 {
        assert!(
            first_hashes.is_ok(),
            "a canonical fixture input under normal limits was rejected"
        );
        // Completion oracle: `is_ok` admits InternalInvariant, Trap, and
        // ResourceLimit outcomes, exactly the engine-defect class this
        // slice exists to surface. The contract requires a completed
        // termination, so a canonical fixture must Success-terminate, and
        // the Success payload must be result-canonical (encodeable).
        match &first {
            Ok(outcome) => {
                let ExecutionTermination::Success(value) = &outcome.termination
                else {
                    panic!(
                        "a valid fixed VM fixture under normal limits did not succeed: {:?}",
                        outcome.termination
                    )
                };
                assert!(
                    sley_mutate::encode_const_value(value).is_ok(),
                    "a succeeded VM fixture value is not result-canonical"
                );
            }
            Err(error) => panic!(
                "a valid fixed VM fixture under normal limits was rejected: {error:?}"
            ),
        }
    }

    if !canonical_inputs {
        // Must-reject oracle: input validation refuses a count mismatch
        // unconditionally and first, then a type mismatch before
        // canonicality. A raw draw that disagrees with the fixture shape
        // must therefore fail; a fail-open regression that lets it through
        // would otherwise stay green on determinism alone.
        let expected = &fixture.expected_input_types;
        if request.inputs.len() != expected.len() {
            assert_eq!(
                first.as_ref().err(),
                Some(
                    &sley_vm::ExecutionError::Exec(
                        sley_vm::ExecutionErrorCode::InputCountMismatch
                    )
                ),
                "a raw VM input with the wrong arity was not refused"
            );
        } else if request
            .inputs
            .iter()
            .zip(expected.iter())
            .any(|(input, value_type)| &input.value_type != value_type)
        {
            assert!(
                first.is_err(),
                "a mistyped raw VM input was accepted"
            );
        }
    }

    if let (Ok(hashes), Ok(outcome)) = (first_hashes, first) {
        assert_eq!(hashes.len(), request.inputs.len());
        assert_eq!(outcome.schema_epoch, lowering.schema_epoch);
        assert_eq!(outcome.state_root, lowering.state_root);
        assert_eq!(outcome.function, lowering.function.entity_id);
        assert_eq!(
            derive_observation_id(
                lowering,
                request.limits,
                outcome.cache_key,
                &hashes,
                &outcome.termination,
                outcome.instruction_count,
                outcome.fuel_used,
                outcome.peak_value_units,
            )
            .expect("a completed restricted outcome must retain a valid observation"),
            outcome.observation_id,
            "VM observation identity drifted"
        );
    }
}

/// Runs one extended-family fixture under both profiles.
/// E4: two representations of one semantic ordered map must never carry two
/// identities.
///
/// S20-210 accepts a map in any entry order: `TYPE_SYSTEM_V1.md` section 5
/// reserves the byte ordering to the SCB encoder/decoder and forbids the
/// checker from reimplementing it or silently sorting a decoded constant. The
/// extended profile reads entry order structurally, so a map supplied to the
/// VM out of canonical order must be refused rather than compared or hashed.
/// This lane builds one map, permutes its entries, and requires that the
/// canonical order is equal to itself and hashes stably while every other
/// order is refused.
fn map_order_lane(cursor: &mut Cursor<'_>) {
    let key_type = TypeExpr::UInt(IntegerWidth::from_bits(64));
    let map_type = TypeExpr::OrderedMap {
        key: Box::new(key_type),
        value: Box::new(TypeExpr::Text),
    };
    let Some(canonical) = canonical_map(&map_type, cursor) else {
        return;
    };
    let ConstData::Map(entries) = &canonical.data else {
        unreachable!("canonical_map builds a map");
    };
    // One entry has only one order, so there is nothing to permute.
    if entries.len() < 2 {
        return;
    }
    let mut permuted = entries.clone();
    let rotation = 1 + usize::from(cursor.byte()) % (permuted.len() - 1);
    permuted.rotate_left(rotation);
    let permuted = ConstValue {
        value_type: map_type.clone(),
        data: ConstData::Map(permuted),
    };
    assert_ne!(
        canonical.data, permuted.data,
        "a rotation must change the entry order"
    );

    let types = TypeEnvironment::new(Vec::new()).expect("empty type environment is valid");
    let equality = operation_fixture(
        900,
        Opcode::Equal,
        Immediate::None,
        vec![map_type.clone(), map_type.clone()],
        TypeExpr::Bool,
    );
    let hashing = operation_fixture(
        920,
        Opcode::ValueHash,
        Immediate::None,
        vec![map_type],
        TypeExpr::Bytes,
    );
    let run = |fixture: &VmFixture, inputs: Vec<ConstValue>| {
        execute_function(
            fixture.lowering_input(&types, CacheProfile::EXTENDED_V1),
            ExecutionRequest {
                inputs,
                limits: generous_limits(),
            },
        )
    };

    // The canonical order is equal to itself and hashes the same every time.
    let equal = run(&equality, vec![canonical.clone(), canonical.clone()])
        .expect("a canonical map pair is accepted");
    assert_eq!(
        equal.termination,
        sley_vm::ExecutionTermination::Success(ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(true),
        }),
        "a canonical map is not equal to itself"
    );
    assert_eq!(
        run(&hashing, vec![canonical.clone()]).expect("a canonical map is hashable"),
        run(&hashing, vec![canonical.clone()]).expect("a canonical map is hashable"),
        "one canonical map hashed to two values"
    );

    // Every other order is refused before it can be compared or hashed.
    for inputs in [
        vec![canonical.clone(), permuted.clone()],
        vec![permuted.clone(), canonical.clone()],
        vec![permuted.clone(), permuted.clone()],
    ] {
        assert_eq!(
            run(&equality, inputs).expect_err("a permuted map input must be refused"),
            sley_vm::ExecutionError::Exec(sley_vm::ExecutionErrorCode::InputNotCanonical),
            "a permuted map input reached equality"
        );
    }
    assert_eq!(
        run(&hashing, vec![permuted]).expect_err("a permuted map input must be refused"),
        sley_vm::ExecutionError::Exec(sley_vm::ExecutionErrorCode::InputNotCanonical),
        "a permuted map input reached value hashing"
    );
}

/// Builds an ordered map whose entries carry distinct keys in the order of
/// their canonical bytes, or `None` when the cursor yields no usable entry.
fn canonical_map(map_type: &TypeExpr, cursor: &mut Cursor<'_>) -> Option<ConstValue> {
    let TypeExpr::OrderedMap { key, value } = map_type else {
        return None;
    };
    let mut entries: Vec<sley_ssmc::MapEntryConst> = Vec::new();
    for _ in 0..cursor.bounded(MAX_COLLECTION_ITEMS) {
        let entry = sley_ssmc::MapEntryConst {
            key: canonical_value(key, cursor),
            value: canonical_value(value, cursor),
        };
        if entries.iter().all(|existing| existing.key != entry.key) {
            entries.push(entry);
        }
    }
    if entries.is_empty() {
        return None;
    }
    // The canonical order is the order of the keys' S20-350 bytes; a key the
    // codec cannot encode has no place in the order at all.
    let mut keyed = Vec::with_capacity(entries.len());
    for entry in entries {
        keyed.push((sley_mutate::encode_const_value(&entry.key).ok()?, entry));
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Some(ConstValue {
        value_type: map_type.clone(),
        data: ConstData::Map(keyed.into_iter().map(|(_, entry)| entry).collect()),
    })
}

fn extended_family_lane(selector: u8, cursor: &mut Cursor<'_>) {
    let fixture = extended_fixture(selector, cursor.byte());
    let types = TypeEnvironment::new(fixture.definitions.clone())
        .expect("the extended fixture type environment is valid");
    // The lane builds the request from the fixture's own parameter types, not
    // from the outer restricted request: a restricted-shaped input dies in
    // validation before any extended opcode runs, which would leave the lane
    // comparing two identical input errors and calling it determinism.
    let request = ExecutionRequest {
        inputs: fixture
            .fixture
            .expected_input_types
            .iter()
            .map(|value_type| canonical_value(value_type, cursor))
            .collect(),
        limits: generous_limits(),
    };
    let extended = fixture.fixture.lowering_input_with(
        &types,
        CacheProfile::EXTENDED_V1,
        &fixture.constants,
        &fixture.functions,
        &fixture.contracts,
        &fixture.adapters,
    );
    let first = execute_function(extended, request.clone());
    let second = execute_function(extended, request.clone());
    assert_eq!(
        first, second,
        "extended-family execution judgment was not deterministic"
    );
    // A family lane that never reaches execution is a lowering-judgment lane
    // wearing a determinism assertion: the first draw must Success-terminate
    // under generous limits. Every family's failure path (E2 Arithmetic, E4
    // DuplicateKey, E7a ContractViolation) is a Success(Result::Err) value,
    // so Success-termination is uniform; `is_ok` would additionally admit
    // InternalInvariant, Trap, and ResourceLimit outcomes.
    match &first {
        Ok(outcome) => {
            let ExecutionTermination::Success(value) = &outcome.termination
            else {
                panic!(
                    "a family fixture under its own canonical inputs did not succeed: {:?}",
                    outcome.termination
                )
            };
            assert!(
                sley_mutate::encode_const_value(value).is_ok(),
                "a succeeded family fixture value is not result-canonical"
            );
        }
        Err(error) => panic!(
            "a family fixture under its own canonical inputs failed to execute: {error:?}"
        ),
    }

    // Every extended family program is outside the restricted profile, and the
    // refusal must be the lowering profile's: a bare error would stay green
    // if a malformed fixture failed input validation instead.
    let restricted = fixture.fixture.lowering_input_with(
        &types,
        CacheProfile::RESTRICTED_V1,
        &fixture.constants,
        &fixture.functions,
        &fixture.contracts,
        &fixture.adapters,
    );
    let refusal = execute_function(restricted, request.clone());
    match refusal {
        Err(ExecutionError::Lowering(LoweringError::Lower(code))) => assert_eq!(
            code.code(),
            LowerErrorCode::OpcodeUnsupported,
            "the restricted refusal was not the opcode judgment"
        ),
        // The multi-function programs (E6 nested callees, E7a predicate) never
        // reach the opcode check: narrowing to owned inventory is an
        // extended-profile step, so under restricted the shared flat inventory
        // fails the single-graph rule first. The refusal is still the lowering
        // profile, never an input error, and the `is_ok` assertion above keeps
        // a malformed fixture from hiding behind it.
        Err(ExecutionError::Lowering(_)) => assert!(
            matches!(selector, 6 | 7),
            "unexpected non-opcode lowering refusal for family {selector}"
        ),
        ok_or_input => panic!(
            "the restricted profile did not refuse the family program with a lowering error: {ok_or_input:?}"
        ),
    }

    if let (Ok(hashes), Ok(outcome)) = (validated_execution_input_hashes(extended, &request), first)
    {
        assert_eq!(
            derive_observation_id(
                extended,
                request.limits,
                outcome.cache_key,
                &hashes,
                &outcome.termination,
                outcome.instruction_count,
                outcome.fuel_used,
                outcome.peak_value_units,
            )
            .expect("a completed extended outcome must retain a valid observation"),
            outcome.observation_id,
            "extended observation identity drifted"
        );
    }

    // Float-canonicality sub-draw (E3): the codec refuses -0.0 and
    // non-canonical NaN before execution, but the family lane only ever
    // feeds pre-canonicalised bits. Feed raw u64 bits with the F64 type
    // and require refusal exactly when the bits are non-canonical.
    if selector == 2 {
        let arity = fixture.fixture.expected_input_types.len();
        let raws: Vec<u64> = (0..arity).map(|_| cursor.u64()).collect();
        let non_canonical = raws.iter().any(|raw| canonical_f64_bits(*raw) != *raw);
        let probe = ExecutionRequest {
            inputs: raws
                .into_iter()
                .map(|raw| ConstValue {
                    value_type: TypeExpr::F64,
                    data: ConstData::F64Bits(raw),
                })
                .collect(),
            limits: generous_limits(),
        };
        let outcome = execute_function(extended, probe);
        // Refusal must come from exactly one of the two canonicality
        // layers: the type layer rejects non-canonical bits in an already
        // constructed value (TYPE_FLOAT_NON_CANONICAL) before the
        // execution layer's InputNotCanonical check is reached (that code
        // fires on the codec path, e.g. the map-order lane). Either layer
        // proves the bits never execute; anything else — or success on
        // non-canonical bits — fails loudly.
        match &outcome {
            Err(sley_vm::ExecutionError::Exec(
                sley_vm::ExecutionErrorCode::InputNotCanonical,
            )) => assert!(
                non_canonical,
                "a canonical float input was refused as non-canonical"
            ),
            Err(sley_vm::ExecutionError::Type(error))
                if error.code() == sley_check::TypeErrorCode::FloatNonCanonical =>
            {
                assert!(
                    non_canonical,
                    "a canonical float input was refused as non-canonical"
                )
            }
            Err(other) => panic!(
                "a raw float input failed with the wrong error: {other:?}"
            ),
            Ok(outcome) => {
                assert!(
                    !non_canonical,
                    "a non-canonical float input executed"
                );
                assert!(
                    matches!(
                        outcome.termination,
                        ExecutionTermination::Success(_)
                    ),
                    "a canonical float input did not succeed: {:?}",
                    outcome.termination
                );
            }
        }
    }
}

struct ExtendedFixture {
    fixture: VmFixture,
    constants: Vec<ConstantDefinition>,
    definitions: Vec<sley_ssmc::TypeDefinition>,
    /// Contract inventory for the assertion family (slice E7a).
    contracts: Vec<sley_ssmc::ContractDefinition>,
    /// Callee inventory for the direct-call family; the callee's parameters,
    /// blocks, and operations live in the fixture's own inventories, which the
    /// lowerer narrows per function.
    functions: Vec<sley_ssmc::FunctionGraph>,
    /// Adapter inventory for the bridge family (slice E8): the carried
    /// import rows the `adapter_invoke` immediates resolve against.
    adapters: Vec<AdapterImport>,
}

fn extended_fixture(selector: u8, entry_index: u8) -> ExtendedFixture {
    match selector {
        // E2: a checked signed addition over two 32-bit integers.
        0 => arithmetic_fixture(0, Opcode::IntAddChecked, IntegerWidth::from_bits(32)),
        // E2: a checked division, whose zero divisor is a value failure.
        1 => arithmetic_fixture(1, Opcode::IntDivChecked, IntegerWidth::from_bits(64)),
        // E3: a deterministic float addition.
        2 => float_fixture(2, Opcode::FloatAdd),
        // E4: an ordered map built from two key/value pairs.
        3 => map_fixture(3),
        // E5: a per-execution cell written and read back.
        4 => cell_fixture(4),
        // E1: a constant reference under the extended profile.
        5 => constant_fixture(5),
        // E6: a direct call to a zero-parameter callee.
        6 => call_fixture(6),
        // E7a: one contract assertion over a Bool predicate.
        7 => contract_fixture(7),
        // E8: one bridge `adapter_invoke` over the frozen rows; the entry
        // varies per draw so every conversion and push reaches the lane.
        8 => bridge_fixture(entry_index),
        _ => unreachable!(),
    }
}

fn arithmetic_fixture(selector: u8, opcode: Opcode, width: IntegerWidth) -> ExtendedFixture {
    let value_type = TypeExpr::SInt(width);
    let result_type = TypeExpr::Result {
        ok: Box::new(value_type.clone()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    };
    ExtendedFixture {
        fixture: operation_fixture(
            300 + u32::from(selector) * 10,
            opcode,
            Immediate::None,
            vec![value_type.clone(), value_type],
            result_type,
        ),
        constants: Vec::new(),
        definitions: Vec::new(),
        functions: Vec::new(),
        contracts: Vec::new(),
        adapters: Vec::new(),
    }
}

fn float_fixture(selector: u8, opcode: Opcode) -> ExtendedFixture {
    ExtendedFixture {
        fixture: operation_fixture(
            300 + u32::from(selector) * 10,
            opcode,
            Immediate::None,
            vec![TypeExpr::F64, TypeExpr::F64],
            TypeExpr::F64,
        ),
        constants: Vec::new(),
        definitions: Vec::new(),
        functions: Vec::new(),
        contracts: Vec::new(),
        adapters: Vec::new(),
    }
}

/// E5: `cell_new` then `cell_get`, so the cell never escapes the execution.
fn cell_fixture(selector: u8) -> ExtendedFixture {
    let base = 300 + u32::from(selector) * 10;
    let function = id(base);
    let block = id(base + 1);
    let parameter = id(base + 2);
    let new_cell = id(base + 3);
    let read_cell = id(base + 4);
    let cell_type = TypeExpr::LocalCell(Box::new(TypeExpr::Bool));
    ExtendedFixture {
        fixture: VmFixture {
            function: function_body(function, vec![parameter], TypeExpr::Bool, block),
            parameters: vec![function_parameter(parameter, function, 0, TypeExpr::Bool)],
            blocks: vec![Block {
                entity_id: block,
                function,
                parameters: Vec::new(),
                operations: vec![new_cell, read_cell],
                terminator: Terminator::Return(ReturnTerminator {
                    value: ValueRef::OperationResult(OperationResultRef {
                        operation: read_cell,
                        result_index: 0,
                    }),
                }),
                reachability: Reachability::Required,
            }],
            operations: vec![
                Operation {
                    entity_id: new_cell,
                    block,
                    ordinal: 0,
                    opcode: Opcode::CellNew,
                    operands: vec![ValueRef::Parameter(parameter)],
                    result_types: vec![cell_type],
                    immediate: Immediate::None,
                },
                Operation {
                    entity_id: read_cell,
                    block,
                    ordinal: 1,
                    opcode: Opcode::CellGet,
                    operands: vec![ValueRef::OperationResult(OperationResultRef {
                        operation: new_cell,
                        result_index: 0,
                    })],
                    result_types: vec![TypeExpr::Bool],
                    immediate: Immediate::None,
                },
            ],
            expected_input_types: vec![TypeExpr::Bool],
        },
        constants: Vec::new(),
        definitions: Vec::new(),
        functions: Vec::new(),
        contracts: Vec::new(),
        adapters: Vec::new(),
    }
}

/// E4: `map_new` over two key/value pairs, which yields a duplicate-key
/// failure as a value when the fuzzer supplies equal keys.
fn map_fixture(selector: u8) -> ExtendedFixture {
    let key = TypeExpr::UInt(IntegerWidth::from_bits(64));
    let value = TypeExpr::Text;
    ExtendedFixture {
        fixture: operation_fixture(
            300 + u32::from(selector) * 10,
            Opcode::MapNew,
            Immediate::None,
            vec![key.clone(), value.clone(), key.clone(), value.clone()],
            TypeExpr::Result {
                ok: Box::new(TypeExpr::OrderedMap {
                    key: Box::new(key),
                    value: Box::new(value),
                }),
                error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
            },
        ),
        constants: Vec::new(),
        definitions: Vec::new(),
        functions: Vec::new(),
        contracts: Vec::new(),
        adapters: Vec::new(),
    }
}

/// E1 under the extended profile: a constant reference.
fn constant_fixture(selector: u8) -> ExtendedFixture {
    let base = 300 + u32::from(selector) * 10;
    let constant = id(base + 5);
    ExtendedFixture {
        fixture: operation_fixture(
            base,
            Opcode::ConstantRef,
            Immediate::Entity(constant),
            Vec::new(),
            TypeExpr::Bool,
        ),
        constants: vec![ConstantDefinition {
            entity_id: constant,
            value: ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(true),
            },
        }],
        definitions: Vec::new(),
        functions: Vec::new(),
        contracts: Vec::new(),
        adapters: Vec::new(),
    }
}

/// E6: one `call_direct` to a callee taking the entry's Bool, so the call
/// boundary copies an argument; the callee itself calls a second
/// zero-parameter callee, so the multi-entry callee table and per-function
/// narrowing run past the single-callee case.
fn call_fixture(selector: u8) -> ExtendedFixture {
    let base = 300 + u32::from(selector) * 10;
    let callee = id(base + 20);
    let callee_block = id(base + 21);
    let call_inner = id(base + 22);
    let callee_parameter = id(base + 23);
    let inner = id(base + 24);
    let inner_block = id(base + 25);
    let inner_operation = id(base + 26);
    let constant = id(base + 27);
    let mut fixture = operation_fixture(
        base,
        Opcode::CallDirect,
        Immediate::Function(sley_ssmc::FunctionRefValue {
            function: callee,
            type_arguments: Vec::new(),
        }),
        vec![TypeExpr::Bool],
        TypeExpr::Bool,
    );
    let callee_graph = function_body(callee, vec![callee_parameter], TypeExpr::Bool, callee_block);
    fixture.parameters.push(function_parameter(
        callee_parameter,
        callee,
        0,
        TypeExpr::Bool,
    ));
    fixture.blocks.push(Block {
        entity_id: callee_block,
        function: callee,
        parameters: Vec::new(),
        operations: vec![call_inner],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: call_inner,
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    });
    fixture.operations.push(Operation {
        entity_id: call_inner,
        block: callee_block,
        ordinal: 0,
        opcode: Opcode::CallDirect,
        operands: Vec::new(),
        result_types: vec![TypeExpr::Bool],
        immediate: Immediate::Function(sley_ssmc::FunctionRefValue {
            function: inner,
            type_arguments: Vec::new(),
        }),
    });
    let inner_graph = function_body(inner, Vec::new(), TypeExpr::Bool, inner_block);
    fixture.blocks.push(Block {
        entity_id: inner_block,
        function: inner,
        parameters: Vec::new(),
        operations: vec![inner_operation],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: inner_operation,
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    });
    fixture.operations.push(Operation {
        entity_id: inner_operation,
        block: inner_block,
        ordinal: 0,
        opcode: Opcode::ConstantRef,
        operands: Vec::new(),
        result_types: vec![TypeExpr::Bool],
        immediate: Immediate::Entity(constant),
    });
    ExtendedFixture {
        fixture,
        constants: vec![ConstantDefinition {
            entity_id: constant,
            value: ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(true),
            },
        }],
        definitions: Vec::new(),
        functions: vec![callee_graph, inner_graph],
        contracts: Vec::new(),
        adapters: Vec::new(),
    }
}

/// E7a: one `contract_assert` whose predicate answers the fuzzer's own Bool,
/// so both the held and the violated arm are reachable from one byte.
fn contract_fixture(selector: u8) -> ExtendedFixture {
    let base = 300 + u32::from(selector) * 10;
    let predicate = id(base + 20);
    let predicate_block = id(base + 21);
    let predicate_operation = id(base + 22);
    let predicate_parameter = id(base + 23);
    let contract = id(base + 24);
    let mut fixture = operation_fixture(
        base,
        Opcode::ContractAssert,
        Immediate::Entity(contract),
        vec![TypeExpr::Bool],
        TypeExpr::Result {
            ok: Box::new(TypeExpr::Unit),
            error: Box::new(TypeExpr::BuiltinFailure(
                BuiltinFailureKind::ContractViolation,
            )),
        },
    );
    let target = fixture.function.entity_id;
    let predicate_graph = function_body(
        predicate,
        vec![predicate_parameter],
        TypeExpr::Bool,
        predicate_block,
    );
    fixture.parameters.push(Parameter {
        entity_id: predicate_parameter,
        owner: predicate,
        role: ParameterRole::Function,
        ordinal: 0,
        value_type: TypeExpr::Bool,
    });
    fixture.blocks.push(Block {
        entity_id: predicate_block,
        function: predicate,
        parameters: Vec::new(),
        operations: vec![predicate_operation],
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::OperationResult(OperationResultRef {
                operation: predicate_operation,
                result_index: 0,
            }),
        }),
        reachability: Reachability::Required,
    });
    fixture.operations.push(Operation {
        entity_id: predicate_operation,
        block: predicate_block,
        ordinal: 0,
        opcode: Opcode::BoolAnd,
        operands: vec![
            ValueRef::Parameter(predicate_parameter),
            ValueRef::Parameter(predicate_parameter),
        ],
        result_types: vec![TypeExpr::Bool],
        immediate: Immediate::None,
    });
    ExtendedFixture {
        fixture,
        constants: Vec::new(),
        definitions: Vec::new(),
        functions: vec![predicate_graph],
        contracts: vec![sley_ssmc::ContractDefinition {
            entity_id: contract,
            target,
            contract_kind: sley_ssmc::ContractKind::Precondition,
            predicate,
            bindings: vec![sley_ssmc::ContractBinding {
                predicate_parameter: 0,
                source: sley_ssmc::ContractSource::Parameter(id(base + 1)),
            }],
            resource_limits: None,
        }],
        adapters: Vec::new(),
    }
}

/// E8: one `adapter_invoke` over the frozen bridge rows. The entry index
/// varies per draw (unlike the family selector, which always names the
/// bridge lane); the carried inventory always holds the three frozen rows,
/// so the shared family lane asserts the frozen path (determinism,
/// completion, restricted refusal, observation identity) and
/// `bridge_sublane` below adversarially tampers rows, shapes, and budgets.
fn bridge_fixture(entry_index: u8) -> ExtendedFixture {
    let base = 380_u32;
    let entry = match entry_index % 3 {
        0 => *b"B2V1",
        1 => *b"V2B1",
        _ => *b"PSH1",
    };
    let (scope_type, request_type, response_type) = match entry {
        e if e == *b"B2V1" => (TypeExpr::Unit, TypeExpr::Bytes, u8vec_type()),
        e if e == *b"V2B1" => (TypeExpr::Unit, u8vec_type(), TypeExpr::Bytes),
        _ => (u8vec_type(), u8_type(), u8vec_type()),
    };
    let fixture = operation_fixture(
        base,
        Opcode::AdapterInvoke,
        Immediate::Entity(bridge_id(entry)),
        vec![scope_type, request_type],
        TypeExpr::Result {
            ok: Box::new(response_type),
            error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
        },
    );
    ExtendedFixture {
        fixture,
        constants: Vec::new(),
        definitions: Vec::new(),
        functions: Vec::new(),
        contracts: Vec::new(),
        adapters: frozen_bridge_rows(),
    }
}

/// The frozen `Entity` identity of one bridge entry: twelve ASCII bytes
/// `SLY1/BRIDGE/`, the four-byte entry code, zero padding to 32 bytes.
/// Input construction only: the production authority is
/// `sley_vm::extended::bridge_entry_id`, and the drift checker pins the
/// prefix, the codes, and the padding width.
fn bridge_id(code: [u8; 4]) -> EntityId {
    let mut bytes = [0_u8; 32];
    bytes[..12].copy_from_slice(b"SLY1/BRIDGE/");
    bytes[12..16].copy_from_slice(&code);
    EntityId::from_bytes(bytes)
}

fn u8_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(8))
}

fn u8vec_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(u8_type()))
}

/// The three frozen bridge rows: genuine epoch-1 import values (identity,
/// adapter identity, ABI version 1, exact schemas, `Index` failure type,
/// empty effect list). Tampered copies are built per draw in the subl lane.
fn frozen_bridge_rows() -> Vec<AdapterImport> {
    [
        (*b"B2V1", TypeExpr::Bytes, u8vec_type()),
        (*b"V2B1", u8vec_type(), TypeExpr::Bytes),
        (*b"PSH1", u8_type(), u8vec_type()),
    ]
    .map(|(code, request, response)| AdapterImport {
        entity_id: bridge_id(code),
        adapter_id: *bridge_id(code).as_bytes(),
        abi_version: 1,
        request_type: request,
        response_type: response,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    })
    .to_vec()
}

/// E8 adversarial subl lane: tampered rows, tampered shapes, and fuel
/// budgets around the measured charge, all judged by production code.
/// Runs when the family selector names the bridge; the frozen path already
/// passed through the shared lane above.
fn bridge_sublane(cursor: &mut Cursor<'_>) {
    let entry = match cursor.byte() % 4 {
        0 => *b"B2V1",
        1 => *b"V2B1",
        2 => *b"PSH1",
        _ => *b"XXXX",
    };
    let tamper = cursor.byte() % 12;
    let shape = cursor.byte() % 8;
    let types = TypeEnvironment::new(Vec::new()).expect("empty type environment is valid");
    let frozen = frozen_bridge_rows();
    let mut row = match entry {
        e if e == *b"B2V1" => frozen[0].clone(),
        e if e == *b"V2B1" => frozen[1].clone(),
        e if e == *b"PSH1" => frozen[2].clone(),
        _ => AdapterImport {
            entity_id: bridge_id(*b"XXXX"),
            adapter_id: *bridge_id(*b"XXXX").as_bytes(),
            abi_version: 1,
            request_type: TypeExpr::Bytes,
            response_type: u8vec_type(),
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
            effects: Vec::new(),
        },
    };
    // Single-field row tampering: every mutant must refuse unsupported at
    // resolution, before shape is even considered.
    match tamper {
        0 => {}
        1 => row.adapter_id = [9_u8; 32],
        2 => row.abi_version = 2,
        3 => {
            row.failure_type = TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic);
        }
        4 => row.effects.push(id(cursor.u32())),
        5 => {
            let request = row.request_type.clone();
            row.request_type = row.response_type.clone();
            row.response_type = request;
        }
        6 => {
            row.request_type =
                TypeExpr::Vector(Box::new(TypeExpr::UInt(IntegerWidth::from_bits(16))));
        }
        7 => {
            row.request_type = u8_type();
            row.response_type = TypeExpr::Vector(Box::new(TypeExpr::Bool));
        }
        8 => row.abi_version = 0,
        9 => {
            row.failure_type = TypeExpr::BuiltinFailure(BuiltinFailureKind::Capability);
        }
        10 => row.request_type = TypeExpr::Text,
        // Response-family confusion, entry-aware: the replacement must
        // always differ from the frozen response (a no-op tamper once let
        // a frozen V2B1 row through as "tampered" — regression seed
        // `20 45 6b` pins the class).
        _ => {
            row.response_type = if row.response_type == TypeExpr::Bytes {
                u8vec_type()
            } else {
                TypeExpr::Bytes
            };
        }
    }
    let tampered = tamper != 0 || entry == *b"XXXX";
    // A tampered draw must actually differ from every frozen row: a no-op
    // mutant would assert a refusal the production code must not produce.
    if tampered && entry != *b"XXXX" {
        assert!(
            !frozen_bridge_rows().contains(&row),
            "a tamper class left the row frozen"
        );
    }
    let (scope_type, request_type) = match entry {
        e if e == *b"PSH1" => (row.response_type.clone(), row.request_type.clone()),
        _ => (TypeExpr::Unit, row.request_type.clone()),
    };
    let (scope_type, request_type) = match shape {
        // Swapped operands, wrong scope, wrong request.
        1 => (request_type, scope_type),
        2 => (TypeExpr::Bool, request_type),
        3 => (scope_type, TypeExpr::Bool),
        _ => (scope_type, request_type),
    };
    let result_type = TypeExpr::Result {
        ok: Box::new(if shape == 4 {
            TypeExpr::Bool
        } else {
            row.response_type.clone()
        }),
        error: Box::new(if shape == 5 {
            TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)
        } else {
            row.failure_type.clone()
        }),
    };
    let immediate = match shape {
        6 => Immediate::Index(0),
        7 => Immediate::Entity(id(77)),
        _ => Immediate::Entity(row.entity_id),
    };
    let base = 700_u32;
    let function = id(base);
    let block = id(base + 1);
    let operation = id(base + 2);
    let scope_param = id(base + 3);
    let request_param = id(base + 4);
    let function_graph = sley_ssmc::FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![scope_param, request_param],
        result_type: result_type.clone(),
        effects: Vec::new(),
        entry_block: block,
        blocks: vec![block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let parameters = vec![
        function_parameter(scope_param, function, 0, scope_type.clone()),
        function_parameter(request_param, function, 1, request_type.clone()),
    ];
    let operations = vec![Operation {
        entity_id: operation,
        block,
        ordinal: 0,
        opcode: Opcode::AdapterInvoke,
        operands: vec![
            ValueRef::Parameter(scope_param),
            ValueRef::Parameter(request_param),
        ],
        result_types: vec![result_type],
        immediate,
    }];
    let blocks = vec![Block {
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
    }];
    let adapters = [row];
    let input = LoweringInput {
        types: &types,
        function: &function_graph,
        parameters: &parameters,
        blocks: &blocks,
        operations: &operations,
        schema_epoch: SchemaEpochId::from_bytes([8; 32]),
        state_root: StateRoot::from_bytes([9; 32]),
        profile: CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: &[],
        contracts: &[],
        adapters: &adapters,
    };
    let lowering = lower_function(input);
    let judged = judge_function_operations(input);
    // The expected refusal follows the production order: the non-Entity
    // immediate refuses first, then resolution (any tamper or the unlanded
    // identity), then signature.
    let expected = if shape == 6 {
        Some(LowerErrorCode::ImmediateMismatch)
    } else if tampered {
        Some(LowerErrorCode::OpcodeUnsupported)
    } else if shape == 7 {
        Some(LowerErrorCode::OpcodeUnsupported)
    } else if shape != 0 {
        Some(LowerErrorCode::SignatureMismatch)
    } else {
        None
    };
    match expected {
        Some(code) => {
            let lowering_code = match lowering.expect_err("tampered lane draw must not lower") {
                LoweringError::Lower(error) => error.code(),
                LoweringError::Cfg(failure) => panic!("no CFG failure in lane programs: {failure}"),
            };
            assert_eq!(lowering_code, code, "bridge lane lowering refusal drifted");
            let judged_code = match judged.expect_err("tampered lane draw must not judge") {
                LoweringError::Lower(error) => error.code(),
                LoweringError::Cfg(failure) => panic!("no CFG failure in lane programs: {failure}"),
            };
            assert_eq!(judged_code, code, "bridge lane judgment refusal drifted");
        }
        None => {
            lowering.expect("frozen lane draw must lower");
            judged.expect("frozen lane draw must judge");
            // Small frozen inputs under generous limits: determinism plus
            // fuel exactness around the measured charge.
            let request = ExecutionRequest {
                inputs: vec![
                    bridge_scope_input(&scope_type, cursor),
                    bridge_request_input(&request_type, cursor),
                ],
                limits: generous_limits(),
            };
            let measured =
                execute_function(input, request.clone()).expect("frozen lane draw executes");
            let rerun =
                execute_function(input, request.clone()).expect("frozen lane draw re-executes");
            assert_eq!(
                measured, rerun,
                "bridge lane execution was not deterministic"
            );
            assert!(
                matches!(measured.termination, ExecutionTermination::Success(_)),
                "small frozen lane draw must succeed"
            );
            let fuel = measured.fuel_used;
            let exact = execute_function(
                input,
                ExecutionRequest {
                    inputs: request.inputs.clone(),
                    limits: ExecutionLimits {
                        max_fuel: fuel,
                        ..generous_limits()
                    },
                },
            )
            .expect("exact-charge lane draw executes");
            assert_eq!(
                exact.termination, measured.termination,
                "exact fuel charge changed the lane termination"
            );
            let short = execute_function(
                input,
                ExecutionRequest {
                    inputs: request.inputs,
                    limits: ExecutionLimits {
                        max_fuel: fuel.saturating_sub(1),
                        ..generous_limits()
                    },
                },
            )
            .expect("short-charge lane draw executes");
            assert_eq!(
                short.termination,
                ExecutionTermination::ResourceLimit(ResourceKind::Fuel),
                "one fuel short of measured must terminate on fuel"
            );
        }
    }
}

/// Small scope values for the frozen lane draws.
fn bridge_scope_input(scope_type: &TypeExpr, cursor: &mut Cursor<'_>) -> ConstValue {
    match scope_type {
        TypeExpr::Unit => ConstValue {
            value_type: TypeExpr::Unit,
            data: ConstData::Unit,
        },
        TypeExpr::Vector(inner) => ConstValue {
            value_type: scope_type.clone(),
            data: ConstData::Sequence(
                (0..cursor.bounded(3))
                    .map(|_| ConstValue {
                        value_type: inner.as_ref().clone(),
                        data: ConstData::UInt(u128::from(cursor.byte() % 251)),
                    })
                    .collect(),
            ),
        },
        _ => ConstValue {
            value_type: scope_type.clone(),
            data: ConstData::Bool(true),
        },
    }
}

/// Small request values for the frozen lane draws.
fn bridge_request_input(request_type: &TypeExpr, cursor: &mut Cursor<'_>) -> ConstValue {
    match request_type {
        TypeExpr::Bytes => ConstValue {
            value_type: TypeExpr::Bytes,
            data: ConstData::Bytes((0..cursor.bounded(8)).map(|_| cursor.byte()).collect()),
        },
        TypeExpr::Vector(inner) => ConstValue {
            value_type: request_type.clone(),
            data: ConstData::Sequence(
                (0..cursor.bounded(4))
                    .map(|_| ConstValue {
                        value_type: inner.as_ref().clone(),
                        data: ConstData::UInt(u128::from(cursor.byte() % 251)),
                    })
                    .collect(),
            ),
        },
        _ => ConstValue {
            value_type: request_type.clone(),
            data: ConstData::UInt(u128::from(cursor.byte())),
        },
    }
}
/// One function whose single block runs one operation over its parameters.
fn operation_fixture(
    base: u32,
    opcode: Opcode,
    immediate: Immediate,
    parameter_types: Vec<TypeExpr>,
    result_type: TypeExpr,
) -> VmFixture {
    let function = id(base);
    let block = id(base + 1);
    let operation = id(base + 2);
    let parameter_ids = (0..parameter_types.len())
        .map(|offset| id(base + 3 + u32::try_from(offset).unwrap_or(u32::MAX)))
        .collect::<Vec<_>>();
    let parameters = parameter_ids
        .iter()
        .zip(&parameter_types)
        .enumerate()
        .map(|(ordinal, (entity_id, value_type))| {
            function_parameter(
                *entity_id,
                function,
                u32::try_from(ordinal).unwrap_or(u32::MAX),
                value_type.clone(),
            )
        })
        .collect();
    VmFixture {
        function: function_body(function, parameter_ids.clone(), result_type.clone(), block),
        parameters,
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
            opcode,
            operands: parameter_ids
                .iter()
                .copied()
                .map(ValueRef::Parameter)
                .collect(),
            result_types: vec![result_type],
            immediate,
        }],
        expected_input_types: parameter_types,
    }
}

struct VmFixture {
    function: sley_ssmc::FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    expected_input_types: Vec<TypeExpr>,
}

impl VmFixture {
    fn lowering_input<'a>(
        &'a self,
        types: &'a TypeEnvironment,
        profile: CacheProfile,
    ) -> LoweringInput<'a> {
        LoweringInput {
            types,
            function: &self.function,
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &self.operations,
            schema_epoch: SchemaEpochId::from_bytes([8; 32]),
            state_root: StateRoot::from_bytes([9; 32]),
            profile,
            constants: &[],
            globals: &[],
            functions: &[],
            contracts: &[],
            adapters: &[],
        }
    }

    fn lowering_input_with<'a>(
        &'a self,
        types: &'a TypeEnvironment,
        profile: CacheProfile,
        constants: &'a [ConstantDefinition],
        functions: &'a [sley_ssmc::FunctionGraph],
        contracts: &'a [sley_ssmc::ContractDefinition],
        adapters: &'a [AdapterImport],
    ) -> LoweringInput<'a> {
        LoweringInput {
            constants,
            functions,
            contracts,
            adapters,
            ..self.lowering_input(types, profile)
        }
    }
}

fn vm_fixture(selector: u8) -> VmFixture {
    match selector {
        0 => identity_fixture(0, TypeExpr::Unit),
        1 => identity_fixture(1, TypeExpr::Bool),
        2 => identity_fixture(2, TypeExpr::Bytes),
        3 => identity_fixture(3, TypeExpr::Text),
        4 => identity_fixture(4, TypeExpr::Option(Box::new(TypeExpr::Bool))),
        5 => identity_fixture(
            5,
            TypeExpr::Result {
                ok: Box::new(TypeExpr::Bool),
                error: Box::new(TypeExpr::Unit),
            },
        ),
        6 => boolean_fixture(6, Opcode::BoolNot, 1),
        7 => boolean_fixture(7, Opcode::BoolAnd, 2),
        8 => boolean_fixture(8, Opcode::BoolOr, 2),
        _ => unreachable!(),
    }
}

fn identity_fixture(selector: u8, value_type: TypeExpr) -> VmFixture {
    let base = 100_u32 + (u32::from(selector) * 10);
    let function = id(base);
    let parameter = id(base + 1);
    let block = id(base + 2);
    VmFixture {
        function: function_body(function, vec![parameter], value_type.clone(), block),
        parameters: vec![function_parameter(
            parameter,
            function,
            0,
            value_type.clone(),
        )],
        blocks: vec![Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::Parameter(parameter),
            }),
            reachability: Reachability::Required,
        }],
        operations: Vec::new(),
        expected_input_types: vec![value_type],
    }
}

fn boolean_fixture(selector: u8, opcode: Opcode, arity: usize) -> VmFixture {
    let base = 200_u32 + (u32::from(selector) * 10);
    let function = id(base);
    let block = id(base + 1);
    let operation = id(base + 2);
    let parameter_ids = (0..arity)
        .map(|offset| id(base + 3 + u32::try_from(offset).unwrap_or(u32::MAX)))
        .collect::<Vec<_>>();
    let parameters = parameter_ids
        .iter()
        .enumerate()
        .map(|(ordinal, entity_id)| {
            function_parameter(
                *entity_id,
                function,
                u32::try_from(ordinal).unwrap_or(u32::MAX),
                TypeExpr::Bool,
            )
        })
        .collect();
    VmFixture {
        function: function_body(function, parameter_ids.clone(), TypeExpr::Bool, block),
        parameters,
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
            opcode,
            operands: parameter_ids
                .iter()
                .map(|entity_id| ValueRef::Parameter(*entity_id))
                .collect(),
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::None,
        }],
        expected_input_types: vec![TypeExpr::Bool; arity],
    }
}

fn function_body(
    entity_id: EntityId,
    parameters: Vec<EntityId>,
    result_type: TypeExpr,
    entry_block: EntityId,
) -> sley_ssmc::FunctionGraph {
    sley_ssmc::FunctionGraph {
        entity_id,
        type_parameters: Vec::new(),
        parameters,
        result_type,
        effects: Vec::new(),
        entry_block,
        blocks: vec![entry_block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn function_parameter(
    entity_id: EntityId,
    owner: EntityId,
    ordinal: u32,
    value_type: TypeExpr,
) -> Parameter {
    Parameter {
        entity_id,
        owner,
        role: ParameterRole::Function,
        ordinal,
        value_type,
    }
}

fn canonical_value(value_type: &TypeExpr, cursor: &mut Cursor<'_>) -> ConstValue {
    let data = match value_type {
        TypeExpr::Unit => ConstData::Unit,
        TypeExpr::Bool => ConstData::Bool(cursor.byte().is_multiple_of(2)),
        TypeExpr::Bytes => ConstData::Bytes(payload_bytes(cursor)),
        TypeExpr::Text => ConstData::Text(payload_text(cursor)),
        TypeExpr::UInt(width) => ConstData::UInt(in_width_uint(*width, cursor)),
        TypeExpr::SInt(width) => ConstData::SInt(in_width_sint(*width, cursor)),
        // Canonical floats admit one quiet NaN and no negative zero
        // (contract E3), so the constructor applies the same rule the VM
        // applies to results; anything else is stored exactly.
        TypeExpr::F32 => ConstData::F32Bits(canonical_f32_bits(cursor.u32())),
        TypeExpr::F64 => ConstData::F64Bits(canonical_f64_bits(cursor.u64())),
        TypeExpr::Option(inner) => {
            if cursor.byte().is_multiple_of(2) {
                ConstData::Option(None)
            } else {
                ConstData::Option(Some(Box::new(canonical_value(inner, cursor))))
            }
        }
        TypeExpr::Result { ok, error } => {
            if cursor.byte().is_multiple_of(2) {
                ConstData::Result(ResultConst::Ok(Box::new(canonical_value(ok, cursor))))
            } else {
                ConstData::Result(ResultConst::Err(Box::new(canonical_value(error, cursor))))
            }
        }
        // Canonical small vectors: only the bridge fixtures (slice E8) use
        // vector parameter types, so this arm serves that lane alone.
        TypeExpr::Vector(inner) => ConstData::Sequence(
            (0..cursor.bounded(MAX_COLLECTION_ITEMS))
                .map(|_| canonical_value(inner, cursor))
                .collect(),
        ),
        _ => unreachable!("fixed VM fixtures use only supported canonical input types"),
    };
    ConstValue {
        value_type: value_type.clone(),
        data,
    }
}

/// An unsigned value inside an epoch-1 width, so S20-210 accepts it.
fn in_width_uint(width: IntegerWidth, cursor: &mut Cursor<'_>) -> u128 {
    let bits = u32::from(width.bits());
    let raw = cursor.u128();
    if bits == 0 || bits > 128 {
        return raw;
    }
    if bits == 128 {
        raw
    } else {
        raw % (1_u128 << bits)
    }
}

/// A signed value inside an epoch-1 width, so S20-210 accepts it.
fn in_width_sint(width: IntegerWidth, cursor: &mut Cursor<'_>) -> i128 {
    let bits = u32::from(width.bits());
    let raw = cursor.i128();
    if bits == 0 || bits >= 128 {
        return raw;
    }
    let span = 1_i128 << (bits - 1);
    raw.rem_euclid(span << 1) - span
}

/// The canonical quiet NaN bit patterns (contract E3).
const CANONICAL_NAN_F32: u32 = 0x7fc0_0000;
const CANONICAL_NAN_F64: u64 = 0x7ff8_0000_0000_0000;

/// A canonical 32-bit float input from raw bits.
fn canonical_f32_bits(raw: u32) -> u32 {
    let value = f32::from_bits(raw);
    if value.is_nan() {
        CANONICAL_NAN_F32
    } else if value == 0.0 {
        0
    } else {
        raw
    }
}

/// A canonical 64-bit float input from raw bits.
fn canonical_f64_bits(raw: u64) -> u64 {
    let value = f64::from_bits(raw);
    if value.is_nan() {
        CANONICAL_NAN_F64
    } else if value == 0.0 {
        0
    } else {
        raw
    }
}

fn raw_value(cursor: &mut Cursor<'_>) -> ConstValue {
    ConstValue {
        value_type: raw_type(cursor),
        data: raw_data(cursor),
    }
}

fn raw_type(cursor: &mut Cursor<'_>) -> TypeExpr {
    match cursor.byte() % 14 {
        0 => TypeExpr::Unit,
        1 => TypeExpr::Bool,
        2 => TypeExpr::SInt(integer_width(cursor)),
        3 => TypeExpr::UInt(integer_width(cursor)),
        4 => TypeExpr::F32,
        5 => TypeExpr::F64,
        6 => TypeExpr::Bytes,
        7 => TypeExpr::Text,
        8 => TypeExpr::Option(Box::new(TypeExpr::Bool)),
        9 => TypeExpr::Result {
            ok: Box::new(TypeExpr::Bool),
            error: Box::new(TypeExpr::Unit),
        },
        10 => TypeExpr::Vector(Box::new(TypeExpr::Bool)),
        11 => TypeExpr::LocalCell(Box::new(TypeExpr::Bool)),
        12 => TypeExpr::TypeParameter(u32::from(cursor.byte() % 4)),
        13 => TypeExpr::AdapterHandle(id(cursor.u32())),
        _ => unreachable!(),
    }
}

fn raw_data(cursor: &mut Cursor<'_>) -> ConstData {
    match cursor.byte() % 11 {
        0 => ConstData::Unit,
        1 => ConstData::Bool(cursor.byte().is_multiple_of(2)),
        2 => ConstData::SInt(cursor.i128()),
        3 => ConstData::UInt(cursor.u128()),
        4 => ConstData::F32Bits(cursor.u32()),
        5 => ConstData::F64Bits(cursor.u64()),
        6 => ConstData::Bytes(payload_bytes(cursor)),
        7 => ConstData::Text(payload_text(cursor)),
        8 => ConstData::Sequence(
            (0..cursor.bounded(MAX_COLLECTION_ITEMS))
                .map(|_| ConstValue {
                    value_type: TypeExpr::Bool,
                    data: ConstData::Bool(cursor.byte().is_multiple_of(2)),
                })
                .collect(),
        ),
        9 => ConstData::Option((!cursor.byte().is_multiple_of(2)).then(|| {
            Box::new(ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(cursor.byte().is_multiple_of(2)),
            })
        })),
        10 => {
            if cursor.byte().is_multiple_of(2) {
                ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
                    value_type: TypeExpr::Bool,
                    data: ConstData::Bool(cursor.byte().is_multiple_of(2)),
                })))
            } else {
                ConstData::Result(ResultConst::Err(Box::new(ConstValue {
                    value_type: TypeExpr::Unit,
                    data: ConstData::Unit,
                })))
            }
        }
        _ => unreachable!(),
    }
}

/// Limits large enough that a one-operation fixture always runs to a result,
/// so this lane observes the input judgment rather than a ceiling.
fn generous_limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: 1_000,
        max_fuel: 100_000,
        max_value_units: 10_000_000,
        max_output_units: 10_000_000,
        cancel_at_fuel: None,
    }
}

fn generated_limits(profile: u8, cursor: &mut Cursor<'_>) -> ExecutionLimits {
    match profile {
        0 => ExecutionLimits {
            max_instructions: 100,
            max_fuel: 100,
            max_value_units: 10_000,
            max_output_units: 10_000,
            cancel_at_fuel: None,
        },
        1 => ExecutionLimits {
            max_instructions: 0,
            max_fuel: 0,
            max_value_units: 0,
            max_output_units: 0,
            cancel_at_fuel: Some(0),
        },
        2 => ExecutionLimits {
            max_instructions: 1,
            max_fuel: 1,
            max_value_units: 1,
            max_output_units: 1,
            cancel_at_fuel: Some(1),
        },
        3 => ExecutionLimits {
            max_instructions: u64::MAX,
            max_fuel: u64::MAX,
            max_value_units: u64::MAX,
            max_output_units: u64::MAX,
            cancel_at_fuel: Some(u64::MAX),
        },
        4 => ExecutionLimits {
            max_instructions: cursor.u64(),
            max_fuel: cursor.u64(),
            max_value_units: cursor.u64(),
            max_output_units: cursor.u64(),
            cancel_at_fuel: None,
        },
        5 => ExecutionLimits {
            max_instructions: cursor.u64(),
            max_fuel: cursor.u64(),
            max_value_units: cursor.u64(),
            max_output_units: cursor.u64(),
            cancel_at_fuel: Some(cursor.u64()),
        },
        _ => unreachable!(),
    }
}

fn integer_width(cursor: &mut Cursor<'_>) -> IntegerWidth {
    let bits = match cursor.byte() % 8 {
        0 => 8,
        1 => 16,
        2 => 32,
        3 => 64,
        4 => 128,
        _ => cursor.u16(),
    };
    IntegerWidth::from_bits(bits)
}

fn payload_bytes(cursor: &mut Cursor<'_>) -> Vec<u8> {
    (0..cursor.bounded(MAX_PAYLOAD_BYTES))
        .map(|_| cursor.byte())
        .collect()
}

fn payload_text(cursor: &mut Cursor<'_>) -> String {
    (0..cursor.bounded(MAX_PAYLOAD_BYTES))
        .map(|_| char::from(0x20 + (cursor.byte() % 0x5f)))
        .collect()
}

fn id(value: u32) -> EntityId {
    let mut bytes = [0_u8; 32];
    bytes[..4].copy_from_slice(&value.to_be_bytes());
    EntityId::from_bytes(bytes)
}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn byte(&mut self) -> u8 {
        let value = self.input[self.offset % self.input.len()];
        self.offset = self.offset.wrapping_add(1);
        value
    }

    fn u16(&mut self) -> u16 {
        u16::from_be_bytes([self.byte(), self.byte()])
    }

    fn u32(&mut self) -> u32 {
        u32::from_be_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }

    fn u64(&mut self) -> u64 {
        u64::from_be_bytes([
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
        ])
    }

    fn u128(&mut self) -> u128 {
        u128::from_be_bytes([
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
            self.byte(),
        ])
    }

    fn i128(&mut self) -> i128 {
        i128::from_be_bytes(self.u128().to_be_bytes())
    }

    fn bounded(&mut self, maximum: usize) -> usize {
        usize::from(self.byte()) % (maximum + 1)
    }
}
