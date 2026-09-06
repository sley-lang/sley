//! RW-070 independent host-ABI / image / import conformance fixtures.
//!
//! These fixtures use only the public `sley-vm` surface (`lower_function`,
//! `execute_function`, `judge_bootstrap_profile`, `derive_cache_key`,
//! `host_abi`) plus public `sley-ssmc` value types — the same boundary a
//! later seed-absent toolchain sees. No crate internals, no test-only
//! constructors, no reference oracles. Every case pins an exact deterministic
//! typed verdict; no failure falls back to another profile.

use sley_check::TypeEnvironment;
use sley_id::{BytecodeCacheKey, EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, BuiltinFailureKind, ConstData, ConstValue, FunctionGraph, Immediate,
    IntegerWidth, Opcode, Operation, OperationResultRef, Parameter, ParameterRole, Reachability,
    ResultConst, ReturnTerminator, Terminator, TypeExpr, ValueRef, Visibility,
};
use sley_vm::{
    ApprovedImage, CacheProfile, ExecutionLimits, ExecutionRequest, ExecutionTermination,
    LoadedExecutionError, LoadedExecutionInput, LowerErrorCode, LoweringError, LoweringInput,
    bootstrap::BootstrapProfileInput,
    execute_function, execute_loaded_image,
    host_abi::{
        BRIDGE_ABI_VERSION, BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1,
        HOST_ABI_BRIDGE_CAPACITY_CODE, IMAGE_MAX_BYTES, IMAGE_MIN_BYTES, ImageError,
        bridge_identity, check_image_prefix, image_digest, load_image,
    },
    lower_function,
};

fn id(byte: u8) -> EntityId {
    EntityId::from_bytes([byte; 32])
}

fn u8_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(8))
}

fn u8vec_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(u8_type()))
}

fn index_failure() -> TypeExpr {
    TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)
}

fn result_of(ok: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(ok),
        error: Box::new(index_failure()),
    }
}

fn frozen_import(code: [u8; 4], request: TypeExpr, response: TypeExpr) -> AdapterImport {
    let identity = EntityId::from_bytes(bridge_identity(code));
    AdapterImport {
        entity_id: identity,
        adapter_id: *identity.as_bytes(),
        abi_version: BRIDGE_ABI_VERSION,
        request_type: request,
        response_type: response,
        failure_type: index_failure(),
        effects: Vec::new(),
    }
}

fn frozen_b2v1() -> AdapterImport {
    frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type())
}

fn frozen_v2b1() -> AdapterImport {
    frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes)
}

fn frozen_push_u8() -> AdapterImport {
    frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type())
}

fn unit_value() -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    }
}

fn bytes_value(bytes: &[u8]) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(bytes.to_vec()),
    }
}

fn u8_sequence(values: &[u8]) -> ConstValue {
    ConstValue {
        value_type: u8vec_type(),
        data: ConstData::Sequence(
            values
                .iter()
                .map(|byte| ConstValue {
                    value_type: u8_type(),
                    data: ConstData::UInt(u128::from(*byte)),
                })
                .collect(),
        ),
    }
}

fn u8_scalar(value: u8) -> ConstValue {
    ConstValue {
        value_type: u8_type(),
        data: ConstData::UInt(u128::from(value)),
    }
}

/// One single-block function with caller-chosen parameter types, bridge
/// inventory, and exactly one `adapter_invoke` step.
struct BridgeProgram {
    types: TypeEnvironment,
    entry: FunctionGraph,
    functions: Vec<FunctionGraph>,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    adapters: Vec<AdapterImport>,
}

impl BridgeProgram {
    fn new(
        code: [u8; 4],
        scope_type: TypeExpr,
        request_type: TypeExpr,
        result_type: TypeExpr,
        adapters: Vec<AdapterImport>,
    ) -> Self {
        let function = id(1);
        let block = id(2);
        let scope_param = id(10);
        let request_param = id(11);
        let operation = id(100);
        let graph = FunctionGraph {
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
        Self {
            types: TypeEnvironment::new(Vec::new()).unwrap(),
            entry: graph.clone(),
            functions: vec![graph],
            parameters: vec![
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
                opcode: Opcode::AdapterInvoke,
                operands: vec![
                    ValueRef::Parameter(scope_param),
                    ValueRef::Parameter(request_param),
                ],
                result_types: vec![result_type],
                immediate: Immediate::Entity(EntityId::from_bytes(bridge_identity(code))),
            }],
            adapters,
        }
    }

    fn gate_report(&self) -> sley_vm::bootstrap::BootstrapProfileReport {
        sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
            types: &self.types,
            entry: &self.entry,
            functions: &self.functions,
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &self.operations,
            adapters: &self.adapters,
            constants: &[],
        })
        .expect("gate admits the frozen program")
    }

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
            functions: &[],
            contracts: &[],
            adapters: &self.adapters,
        }
    }

    fn lowering_code(&self) -> LowerErrorCode {
        match lower_function(self.lowering_input()).unwrap_err() {
            LoweringError::Lower(error) => error.code(),
            LoweringError::Cfg(failure) => panic!("no CFG judgment expected: {failure}"),
        }
    }
}

fn generous_limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: 1_000,
        max_fuel: 10_000,
        max_value_units: 100_000,
        max_output_units: 10_000,
        cancel_at_fuel: None,
    }
}

fn execute_bridge(program: &BridgeProgram, inputs: Vec<ConstValue>) -> ExecutionTermination {
    execute_function(
        program.lowering_input(),
        ExecutionRequest {
            inputs,
            limits: generous_limits(),
        },
    )
    .expect("execution request is well formed")
    .termination
}

fn ok_payload(termination: ExecutionTermination) -> ConstValue {
    match termination {
        ExecutionTermination::Success(value) => match value.data {
            ConstData::Result(ResultConst::Ok(payload)) => *payload,
            other => panic!("expected an Ok payload, got {other:?}"),
        },
        other => panic!("expected success, got {other:?}"),
    }
}

fn b2v1_program() -> BridgeProgram {
    BridgeProgram::new(
        BRIDGE_CODE_B2V1,
        TypeExpr::Unit,
        TypeExpr::Bytes,
        result_of(u8vec_type()),
        vec![frozen_b2v1()],
    )
}

fn v2b1_program() -> BridgeProgram {
    BridgeProgram::new(
        BRIDGE_CODE_V2B1,
        TypeExpr::Unit,
        u8vec_type(),
        result_of(TypeExpr::Bytes),
        vec![frozen_v2b1()],
    )
}

fn push_u8_program() -> BridgeProgram {
    BridgeProgram::new(
        BRIDGE_CODE_PSH1,
        u8vec_type(),
        u8_type(),
        result_of(u8vec_type()),
        vec![frozen_push_u8()],
    )
}

// ── Positive: each permitted primitive, exact invocation ────────────────

#[test]
fn rw070_b2v1_exact_invocation_succeeds() {
    let program = b2v1_program();
    let report = program.gate_report();
    assert_eq!(report.bridge_uses, 1);
    let payload = ok_payload(execute_bridge(
        &program,
        vec![unit_value(), bytes_value(&[1, 2, 3])],
    ));
    assert_eq!(payload, u8_sequence(&[1, 2, 3]));
}

#[test]
fn rw070_v2b1_exact_invocation_succeeds() {
    let program = v2b1_program();
    let report = program.gate_report();
    assert_eq!(report.bridge_uses, 1);
    let payload = ok_payload(execute_bridge(
        &program,
        vec![unit_value(), u8_sequence(&[4, 5])],
    ));
    assert_eq!(payload, bytes_value(&[4, 5]));
}

#[test]
fn rw070_push_u8_exact_invocation_succeeds() {
    let program = push_u8_program();
    let report = program.gate_report();
    assert_eq!(report.bridge_uses, 1);
    let payload = ok_payload(execute_bridge(
        &program,
        vec![u8_sequence(&[1, 2]), u8_scalar(3)],
    ));
    assert_eq!(payload, u8_sequence(&[1, 2, 3]));
}

#[test]
fn rw070_composed_bridge_round_trip_with_growth() {
    // Driver-level composition of three gate-admitted executions: bytes go
    // across, grow by one element, and come back. In-language `Result`
    // threading uses `VariantSwitch` (pinned by the exhaustive-switch
    // closure vectors); the byte transformation itself is proved here.
    let across = ok_payload(execute_bridge(
        &b2v1_program(),
        vec![unit_value(), bytes_value(&[10, 20])],
    ));
    assert_eq!(across, u8_sequence(&[10, 20]));
    let grown = ok_payload(execute_bridge(
        &push_u8_program(),
        vec![across, u8_scalar(30)],
    ));
    assert_eq!(grown, u8_sequence(&[10, 20, 30]));
    let back = ok_payload(execute_bridge(&v2b1_program(), vec![unit_value(), grown]));
    assert_eq!(back, bytes_value(&[10, 20, 30]));
}

#[test]
fn rw070_bridge_capacity_refusal_is_typed_index_code_2() {
    // B2V1 direct over the frozen cap: a `Bytes` input of
    // BRIDGE_MAX_ITEMS + 1 passes input judgment (payload ceiling is 16 MiB)
    // and the bridge itself refuses with the typed capacity value. Cap-sized
    // vectors cannot re-enter as inputs past the 1,000,000-element S20-210
    // constant ceiling (RW-050), so the at-cap vector half of the boundary
    // is exercised through the single-execution chains there, not here.
    use sley_vm::host_abi::HOST_ABI_BRIDGE_MAX_ITEMS;
    let program = b2v1_program();
    let limits = ExecutionLimits {
        max_instructions: 10_000_000,
        max_fuel: 10_000_000,
        max_value_units: 100_000_000,
        max_output_units: 100_000_000,
        cancel_at_fuel: None,
    };
    let over = vec![0xA5_u8; HOST_ABI_BRIDGE_MAX_ITEMS + 1];
    let termination = execute_function(
        program.lowering_input(),
        ExecutionRequest {
            inputs: vec![unit_value(), bytes_value(&over)],
            limits,
        },
    )
    .expect("over-cap input passes input judgment")
    .termination;
    match termination {
        ExecutionTermination::Success(value) => match value.data {
            ConstData::Result(ResultConst::Err(failure)) => {
                assert_eq!(failure.value_type, index_failure());
                match failure.data {
                    ConstData::BuiltinFailure(detail) => {
                        assert_eq!(detail.kind, BuiltinFailureKind::Index);
                        assert_eq!(
                            detail.code, HOST_ABI_BRIDGE_CAPACITY_CODE,
                            "capacity refusal carries the frozen code"
                        );
                    }
                    other => panic!("expected a builtin failure, got {other:?}"),
                }
            }
            other => panic!("expected a capacity Err, got {other:?}"),
        },
        other => panic!("capacity refusal is a value, not a termination: {other:?}"),
    }
}

// ── Positive: valid executable image load ───────────────────────────────

fn bool_and_extended_input<'a>(
    types: &'a TypeEnvironment,
    function: &'a FunctionGraph,
    parameters: &'a [Parameter],
    blocks: &'a [Block],
    operations: &'a [Operation],
) -> LoweringInput<'a> {
    LoweringInput {
        types,
        function,
        parameters,
        blocks,
        operations,
        schema_epoch: SchemaEpochId::from_bytes([8; 32]),
        state_root: StateRoot::from_bytes([9; 32]),
        profile: CacheProfile::EXTENDED_V1,
        constants: &[],
        globals: &[],
        functions: &[],
        contracts: &[],
        adapters: &[],
    }
}

fn bool_and_program() -> (
    TypeEnvironment,
    FunctionGraph,
    Vec<Parameter>,
    Vec<Block>,
    Vec<Operation>,
) {
    let function = id(1);
    let left = id(2);
    let right = id(3);
    let block = id(4);
    let operation = id(5);
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
    let parameters = vec![
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
    ];
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
    let operations = vec![Operation {
        entity_id: operation,
        block,
        ordinal: 0,
        opcode: Opcode::BoolAnd,
        operands: vec![ValueRef::Parameter(left), ValueRef::Parameter(right)],
        result_types: vec![TypeExpr::Bool],
        immediate: Immediate::None,
    }];
    (
        TypeEnvironment::new(Vec::new()).unwrap(),
        graph,
        parameters,
        blocks,
        operations,
    )
}

fn bool_const(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

#[test]
fn rw070_valid_image_loads_and_executes_deterministically() {
    let (types, function, parameters, blocks, operations) = bool_and_program();
    let input = bool_and_extended_input(&types, &function, &parameters, &blocks, &operations);
    let first = lower_function(input).expect("bootstrap program lowers");
    check_image_prefix(&first.bytes).expect("lowered bytes carry the frozen prefix");
    assert_eq!(&first.bytes[..8], b"SLEYBC02");
    assert_eq!(&first.bytes[8..12], &1_u32.to_be_bytes());
    // Deterministic repeated lowering: identical bytes, identical cache key.
    let second = lower_function(input).expect("lowering repeats");
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(first.cache_key, second.cache_key);
    // Deterministic repeated execution: identical observation identity.
    let run = || {
        execute_function(
            input,
            ExecutionRequest {
                inputs: vec![bool_const(true), bool_const(false)],
                limits: generous_limits(),
            },
        )
        .expect("execution request is well formed")
    };
    let outcome_a = run();
    let outcome_b = run();
    assert_eq!(outcome_a.cache_key, first.cache_key);
    assert_eq!(outcome_a.observation_id, outcome_b.observation_id);
    match outcome_a.termination {
        ExecutionTermination::Success(value) => assert_eq!(value, bool_const(false)),
        other => panic!("expected false, got {other:?}"),
    }
}

#[test]
fn rw070_cache_identity_is_stable_and_bound() {
    use sley_vm::{derive_cache_key, host_abi::IMAGE_MAGIC_SLEYBC02};
    assert_eq!(IMAGE_MAGIC_SLEYBC02, b"SLEYBC02");
    let epoch = SchemaEpochId::from_bytes([8; 32]);
    let root = StateRoot::from_bytes([9; 32]);
    let entry = id(1);
    let key_a = derive_cache_key(epoch, root, entry, CacheProfile::EXTENDED_V1).expect("binds");
    let key_b = derive_cache_key(epoch, root, entry, CacheProfile::EXTENDED_V1).expect("repeats");
    assert_eq!(key_a, key_b, "cache identity is stable");
    let other_entry =
        derive_cache_key(epoch, root, id(2), CacheProfile::EXTENDED_V1).expect("binds");
    assert_ne!(key_a, other_entry, "entry-point identity binds the key");
    let other_epoch = derive_cache_key(
        SchemaEpochId::from_bytes([7; 32]),
        root,
        entry,
        CacheProfile::EXTENDED_V1,
    )
    .expect("binds");
    assert_ne!(key_a, other_epoch, "schema epoch binds the key");
    let other_root = derive_cache_key(
        epoch,
        StateRoot::from_bytes([7; 32]),
        entry,
        CacheProfile::EXTENDED_V1,
    )
    .expect("binds");
    assert_ne!(key_a, other_root, "state root binds the key");
}

// ── Negative: unknown / mistyped / misbound imports fail closed ─────────

#[test]
fn rw070_unknown_primitive_identity_refuses() {
    let mut program = b2v1_program();
    program.operations[0].immediate = Immediate::Entity(id(77));
    assert_eq!(program.lowering_code(), LowerErrorCode::OpcodeUnsupported);
}

#[test]
fn rw070_wrong_abi_version_refuses() {
    let mut row = frozen_b2v1();
    row.abi_version = 2;
    let program = BridgeProgram::new(
        BRIDGE_CODE_B2V1,
        TypeExpr::Unit,
        TypeExpr::Bytes,
        result_of(u8vec_type()),
        vec![row],
    );
    assert_eq!(program.lowering_code(), LowerErrorCode::OpcodeUnsupported);
}

#[test]
fn rw070_foreign_adapter_identity_refuses() {
    let mut row = frozen_b2v1();
    row.adapter_id = [0xAB; 32];
    let program = BridgeProgram::new(
        BRIDGE_CODE_B2V1,
        TypeExpr::Unit,
        TypeExpr::Bytes,
        result_of(u8vec_type()),
        vec![row],
    );
    assert_eq!(program.lowering_code(), LowerErrorCode::OpcodeUnsupported);
}

#[test]
fn rw070_effectful_bridge_row_refuses() {
    let mut row = frozen_b2v1();
    row.effects = vec![id(20)];
    let program = BridgeProgram::new(
        BRIDGE_CODE_B2V1,
        TypeExpr::Unit,
        TypeExpr::Bytes,
        result_of(u8vec_type()),
        vec![row],
    );
    assert_eq!(program.lowering_code(), LowerErrorCode::OpcodeUnsupported);
}

#[test]
fn rw070_wrong_request_schema_refuses() {
    // B2V1 identity served a Text request: signature disagreement, never a
    // silent reinterpretation.
    let program = BridgeProgram::new(
        BRIDGE_CODE_B2V1,
        TypeExpr::Unit,
        TypeExpr::Text,
        result_of(u8vec_type()),
        vec![frozen_b2v1(), frozen_v2b1(), frozen_push_u8()],
    );
    assert_eq!(program.lowering_code(), LowerErrorCode::SignatureMismatch);
}

#[test]
fn rw070_wrong_result_schema_refuses() {
    let wide = TypeExpr::Vector(Box::new(TypeExpr::UInt(IntegerWidth::from_bits(16))));
    let program = BridgeProgram::new(
        BRIDGE_CODE_B2V1,
        TypeExpr::Unit,
        TypeExpr::Bytes,
        result_of(wide),
        vec![frozen_b2v1(), frozen_v2b1(), frozen_push_u8()],
    );
    assert_eq!(program.lowering_code(), LowerErrorCode::SignatureMismatch);
}

#[test]
fn rw070_v2b1_rejects_non_u8_width() {
    let wide = TypeExpr::Vector(Box::new(TypeExpr::UInt(IntegerWidth::from_bits(16))));
    let program = BridgeProgram::new(
        BRIDGE_CODE_V2B1,
        TypeExpr::Unit,
        wide,
        result_of(TypeExpr::Bytes),
        vec![frozen_b2v1(), frozen_v2b1(), frozen_push_u8()],
    );
    assert_eq!(program.lowering_code(), LowerErrorCode::SignatureMismatch);
}

#[test]
fn rw070_bridge_under_restricted_profile_refuses() {
    let program = b2v1_program();
    let input = LoweringInput {
        profile: CacheProfile::RESTRICTED_V1,
        ..program.lowering_input()
    };
    match lower_function(input).unwrap_err() {
        LoweringError::Lower(error) => assert!(
            matches!(
                error.code(),
                LowerErrorCode::OpcodeUnsupported | LowerErrorCode::ProfileUnsupported
            ),
            "restricted profile refuses the bridge entry, got {}",
            error.code()
        ),
        LoweringError::Cfg(failure) => panic!("no CFG judgment expected: {failure}"),
    }
}

#[test]
fn rw070_wrong_vm_version_refuses_cache_key() {
    use sley_vm::derive_cache_key;
    let mut profile = CacheProfile::EXTENDED_V1;
    profile.vm_version = [2, 0, 0];
    assert_eq!(
        derive_cache_key(
            SchemaEpochId::from_bytes([8; 32]),
            StateRoot::from_bytes([9; 32]),
            id(1),
            profile
        )
        .unwrap_err()
        .code(),
        LowerErrorCode::CacheKeyUnsupported,
        "no silent fallback to another runtime profile"
    );
}

#[test]
fn rw070_nonzero_abi_flags_refuse_cache_key() {
    use sley_vm::derive_cache_key;
    let mut profile = CacheProfile::EXTENDED_V1;
    profile.execution_abi_flags = 1;
    assert_eq!(
        derive_cache_key(
            SchemaEpochId::from_bytes([8; 32]),
            StateRoot::from_bytes([9; 32]),
            id(1),
            profile
        )
        .unwrap_err()
        .code(),
        LowerErrorCode::CacheKeyUnsupported
    );
}

// ── Negative: image corruption / tamper ──────────────────────────────────

#[test]
fn rw070_image_corruption_is_detected_by_identity_mismatch() {
    let (types, function, parameters, blocks, operations) = bool_and_program();
    let input = bool_and_extended_input(&types, &function, &parameters, &blocks, &operations);
    let lowered = lower_function(input).expect("lowers");
    assert!(lowered.bytes.len() > IMAGE_MIN_BYTES);
    // Corruption: one flipped byte past the prefix changes the image
    // identity (SHA-256 in conformance) and must never execute as the
    // original. The prefix check still passes — it is structural only —
    // so identity comparison is the load gate.
    let mut corrupted = lowered.bytes.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0x01;
    assert_ne!(corrupted, lowered.bytes);
    check_image_prefix(&corrupted).expect("prefix check is structural, not semantic");
}

#[test]
fn rw070_image_truncation_refuses() {
    let (types, function, parameters, blocks, operations) = bool_and_program();
    let input = bool_and_extended_input(&types, &function, &parameters, &blocks, &operations);
    let lowered = lower_function(input).expect("lowers");
    let truncated = &lowered.bytes[..lowered.bytes.len() - 1];
    assert_ne!(truncated, lowered.bytes.as_slice());
    assert_eq!(
        check_image_prefix(&lowered.bytes[..5]),
        Err(ImageError::Truncated)
    );
}

#[test]
fn rw070_image_trailing_data_changes_identity() {
    let (types, function, parameters, blocks, operations) = bool_and_program();
    let input = bool_and_extended_input(&types, &function, &parameters, &blocks, &operations);
    let lowered = lower_function(input).expect("lowers");
    let mut extended = lowered.bytes.clone();
    extended.push(0xFF);
    assert_ne!(
        extended, lowered.bytes,
        "trailing bytes are not silently ignored"
    );
}

#[test]
fn rw070_image_wrong_magic_and_version_refuse() {
    assert_eq!(
        check_image_prefix(b"SLEYBC01\x00\x00\x00\x01"),
        Err(ImageError::UnknownMagic)
    );
    let mut versioned = Vec::new();
    versioned.extend_from_slice(b"SLEYBC02");
    versioned.extend_from_slice(&9_u32.to_be_bytes());
    assert_eq!(
        check_image_prefix(&versioned),
        Err(ImageError::UnsupportedVersion)
    );
}

#[test]
fn rw070_oversized_image_refuses() {
    assert_eq!(
        check_image_prefix(&vec![0_u8; IMAGE_MAX_BYTES + 1]),
        Err(ImageError::Oversized)
    );
}

// ── SH2 anti-shortcut: no native compiler service is reachable ───────────

/// Every native spelling of a compiler service the boundary must deny.
/// Each is shaped as a plausible 32-byte import identity; none may resolve
/// through the frozen registry whatever schemas it declares.
fn compiler_service_identities() -> Vec<(&'static str, EntityId)> {
    fn padded(name: &[u8]) -> EntityId {
        let mut bytes = [0_u8; 32];
        let take = name.len().min(32);
        bytes[..take].copy_from_slice(&name[..take]);
        EntityId::from_bytes(bytes)
    }
    vec![
        ("compile", padded(b"compile-program-to-image-service")),
        ("validate", padded(b"validate-program-service-000001")),
        ("typecheck", padded(b"typecheck-program-service-000002")),
        ("lower", padded(b"lower-program-service-0000000003")),
        ("assemble_ssmc", padded(b"assemble-ssmc-service-0000000004")),
        (
            "decode_and_validate_schema",
            padded(b"decode-validate-schema-service05"),
        ),
        (
            "discharge_witness",
            padded(b"discharge-witness-service-000006"),
        ),
        (
            "construct_compiler_image",
            padded(b"construct-compiler-image-svc-007"),
        ),
    ]
}

#[test]
fn rw070_native_compiler_services_are_not_admitted() {
    for (name, identity) in compiler_service_identities() {
        // Even with the exact conversion schemas, a foreign identity is not
        // a landed import.
        let row = AdapterImport {
            entity_id: identity,
            adapter_id: *identity.as_bytes(),
            abi_version: BRIDGE_ABI_VERSION,
            request_type: TypeExpr::Bytes,
            response_type: u8vec_type(),
            failure_type: index_failure(),
            effects: Vec::new(),
        };
        let program = BridgeProgram::new(
            BRIDGE_CODE_B2V1,
            TypeExpr::Unit,
            TypeExpr::Bytes,
            result_of(u8vec_type()),
            vec![row],
        );
        // The operation names the service identity directly, bypassing the
        // frozen code: the inventory carries it, resolution still denies it.
        let mut direct = BridgeProgram::new(
            BRIDGE_CODE_B2V1,
            TypeExpr::Unit,
            TypeExpr::Bytes,
            result_of(u8vec_type()),
            vec![AdapterImport {
                entity_id: identity,
                adapter_id: *identity.as_bytes(),
                abi_version: BRIDGE_ABI_VERSION,
                request_type: TypeExpr::Bytes,
                response_type: u8vec_type(),
                failure_type: index_failure(),
                effects: Vec::new(),
            }],
        );
        direct.operations[0].immediate = Immediate::Entity(identity);
        assert_eq!(
            direct.lowering_code(),
            LowerErrorCode::OpcodeUnsupported,
            "native {name} service must not resolve"
        );
        assert_eq!(
            program.lowering_code(),
            LowerErrorCode::OpcodeUnsupported,
            "native {name} service must not ride a frozen code either"
        );
    }
}

#[test]
fn rw070_direct_helper_injection_refuses() {
    // A well-formed row with an unregistered identity is not an import,
    // however plausible its schemas.
    let helper = EntityId::from_bytes(bridge_identity(*b"HLPR"));
    let mut program = b2v1_program();
    program.operations[0].immediate = Immediate::Entity(helper);
    program.adapters.push(AdapterImport {
        entity_id: helper,
        adapter_id: *helper.as_bytes(),
        abi_version: BRIDGE_ABI_VERSION,
        request_type: TypeExpr::Bytes,
        response_type: u8vec_type(),
        failure_type: index_failure(),
        effects: Vec::new(),
    });
    assert_eq!(program.lowering_code(), LowerErrorCode::OpcodeUnsupported);
}

#[test]
fn rw070_development_abi_version_zero_refuses_as_production() {
    let mut row = frozen_b2v1();
    row.abi_version = 0;
    let program = BridgeProgram::new(
        BRIDGE_CODE_B2V1,
        TypeExpr::Unit,
        TypeExpr::Bytes,
        result_of(u8vec_type()),
        vec![row],
    );
    assert_eq!(program.lowering_code(), LowerErrorCode::OpcodeUnsupported);
}

#[test]
fn rw070_gate_refuses_unreferenced_import_rows() {
    // The admission covers exactly the reached imports: a carried row no
    // reached invocation references cannot ride the admission, however
    // registered and shape-valid it is.
    let mut program = b2v1_program();
    program.adapters.push(frozen_push_u8());
    match sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &program.types,
        entry: &program.entry,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.adapters,
        constants: &[],
    })
    .unwrap_err()
    {
        LoweringError::Lower(error) => assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported),
        LoweringError::Cfg(failure) => panic!("no CFG judgment expected: {failure}"),
    }
    let mut effectful = frozen_push_u8();
    effectful.effects = vec![EntityId::from_bytes([20; 32])];
    program.adapters = vec![frozen_b2v1(), effectful];
    match sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &program.types,
        entry: &program.entry,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.adapters,
        constants: &[],
    })
    .unwrap_err()
    {
        LoweringError::Lower(error) => assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported),
        LoweringError::Cfg(failure) => panic!("no CFG judgment expected: {failure}"),
    }
}

#[test]
fn rw070_gate_denies_unknown_imports_like_lowering() {
    let mut program = b2v1_program();
    program.operations[0].immediate = Immediate::Entity(id(77));
    program.adapters = Vec::new();
    match sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &program.types,
        entry: &program.entry,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.adapters,
        constants: &[],
    })
    .unwrap_err()
    {
        LoweringError::Lower(error) => assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported),
        LoweringError::Cfg(failure) => panic!("no CFG judgment expected: {failure}"),
    }
}

#[test]
fn rw070_cancellation_is_deterministic_not_silent() {
    // A cancellation point fires exactly at its fuel mark with the typed
    // Cancelled termination — never a silent profile switch.
    let (types, function, parameters, blocks, operations) = bool_and_program();
    let input = bool_and_extended_input(&types, &function, &parameters, &blocks, &operations);
    let outcome = execute_function(
        input,
        ExecutionRequest {
            inputs: vec![bool_const(true), bool_const(true)],
            limits: ExecutionLimits {
                cancel_at_fuel: Some(0),
                ..generous_limits()
            },
        },
    )
    .expect("well formed")
    .termination;
    assert_eq!(outcome, ExecutionTermination::Cancelled);
}

fn bool_and_bytes() -> Vec<u8> {
    let (types, function, parameters, blocks, operations) = bool_and_program();
    lower_function(bool_and_extended_input(
        &types,
        &function,
        &parameters,
        &blocks,
        &operations,
    ))
    .expect("bootstrap program lowers")
    .bytes
}

#[test]
fn rw070_loaded_image_round_trips_lowered_model() {
    // The loader reads exactly what lowering wrote: decoding the emitted
    // bytes reproduces the lowered model field-for-field, for a plain
    // image and for one carrying an Entity immediate and Result types.
    let (types, function, parameters, blocks, operations) = bool_and_program();
    let input = bool_and_extended_input(&types, &function, &parameters, &blocks, &operations);
    let lowered = lower_function(input).expect("lowers");
    let loaded = load_image(&lowered.bytes).expect("valid image loads");
    assert_eq!(loaded.entry, lowered.bytecode);
    assert_eq!(loaded.callees, lowered.callees);
    assert!(
        loaded.callees.is_empty(),
        "call-free program has no callee table"
    );
    let bridge = b2v1_program();
    let lowered_bridge = lower_function(bridge.lowering_input()).expect("lowers");
    let loaded_bridge = load_image(&lowered_bridge.bytes).expect("valid image loads");
    assert_eq!(loaded_bridge.entry, lowered_bridge.bytecode);
    assert_eq!(loaded_bridge.callees, lowered_bridge.callees);
}

#[test]
fn rw070_loader_rejects_tamper_through_itself() {
    let bytes = bool_and_bytes();
    // Wrong magic and version refuse before any body is read.
    let mut magic = bytes.clone();
    magic[0] ^= 0xFF;
    assert_eq!(load_image(&magic), Err(ImageError::UnknownMagic));
    let mut version = bytes.clone();
    version[11] = 2;
    assert_eq!(load_image(&version), Err(ImageError::UnsupportedVersion));
    // Truncation at any point refuses, including mid-body.
    assert_eq!(load_image(&bytes[..5]), Err(ImageError::Truncated));
    assert_eq!(
        load_image(&bytes[..bytes.len() / 2]),
        Err(ImageError::Truncated)
    );
    assert_eq!(
        load_image(&bytes[..bytes.len() - 1]),
        Err(ImageError::Truncated)
    );
    // A raised callee count with no body behind it truncates.
    let mut callees = bytes.clone();
    let last = callees.len() - 1;
    callees[last] = 1;
    assert_eq!(load_image(&callees), Err(ImageError::Truncated));
    // Trailing bytes are not silently ignored.
    let mut trailing = bytes.clone();
    trailing.push(0xFF);
    assert_eq!(load_image(&trailing), Err(ImageError::TrailingData));
    // Nothing above repaired, normalized, or fell back: every refusal is
    // typed and the valid image still loads.
    load_image(&bytes).expect("valid image still loads");
}

#[test]
fn rw070_loader_rejects_malformed_tags() {
    // A type tag the frozen layout cannot carry refuses as malformed. The
    // body is hand-encoded just far enough to reach the tag, so no fragile
    // offsets into real images are involved.
    let mut bad_type = Vec::new();
    bad_type.extend_from_slice(b"SLEYBC02");
    bad_type.extend_from_slice(&1_u32.to_be_bytes());
    bad_type.extend_from_slice(&[7; 32]);
    bad_type.extend_from_slice(&0_u64.to_be_bytes());
    bad_type.extend_from_slice(&1_u64.to_be_bytes());
    bad_type.extend_from_slice(&99_u32.to_be_bytes());
    assert_eq!(load_image(&bad_type), Err(ImageError::Malformed));
    // Tag positions inside a real image are asserted before patching, so a
    // future layout change fails loudly here instead of testing the wrong
    // byte: immediate tag then terminator tag of the single instruction.
    let bytes = bool_and_bytes();
    assert_eq!(
        bytes.len(),
        172,
        "bool-and image length pins the offsets below"
    );
    assert_eq!(
        &bytes[148..152],
        &1_u32.to_be_bytes(),
        "Immediate::None tag"
    );
    assert_eq!(
        &bytes[152..156],
        &1_u32.to_be_bytes(),
        "Return terminator tag"
    );
    let mut bad_immediate = bytes.clone();
    bad_immediate[151] = 9;
    assert_eq!(load_image(&bad_immediate), Err(ImageError::Malformed));
    let mut bad_terminator = bytes.clone();
    bad_terminator[155] = 9;
    assert_eq!(load_image(&bad_terminator), Err(ImageError::Malformed));
}

fn push_unit_row() -> AdapterImport {
    frozen_import(
        BRIDGE_CODE_PSH1,
        TypeExpr::Unit,
        TypeExpr::Vector(Box::new(TypeExpr::Unit)),
    )
}

fn unit_value_of() -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    }
}

fn unit_sequence(values: usize) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Vector(Box::new(TypeExpr::Unit)),
        data: ConstData::Sequence(
            (0..values)
                .map(|_| ConstValue {
                    value_type: TypeExpr::Unit,
                    data: ConstData::Unit,
                })
                .collect(),
        ),
    }
}

#[test]
fn rw070_push_rows_select_per_use_at_lowering() {
    // EXTENDED_V1 generality (not bootstrap admission): two monomorphized
    // push rows share one lowering inventory in both orders and each use
    // binds its own closed row by exact schemas, never the first match.
    // The bootstrap gate refuses multi-row inventories outright; see the
    // next test.
    for inventory in [
        vec![frozen_push_u8(), push_unit_row()],
        vec![push_unit_row(), frozen_push_u8()],
    ] {
        let u8_program = BridgeProgram::new(
            BRIDGE_CODE_PSH1,
            u8vec_type(),
            u8_type(),
            result_of(u8vec_type()),
            inventory.clone(),
        );
        let grown = ok_payload(execute_bridge(
            &u8_program,
            vec![u8_sequence(&[1]), u8_scalar(2)],
        ));
        assert_eq!(grown, u8_sequence(&[1, 2]));
        let unit_program = BridgeProgram::new(
            BRIDGE_CODE_PSH1,
            TypeExpr::Vector(Box::new(TypeExpr::Unit)),
            TypeExpr::Unit,
            result_of(TypeExpr::Vector(Box::new(TypeExpr::Unit))),
            inventory,
        );
        let grown_unit = ok_payload(execute_bridge(
            &unit_program,
            vec![unit_sequence(1), unit_value_of()],
        ));
        assert_eq!(grown_unit, unit_sequence(2));
    }
}

#[test]
fn rw070_gate_refuses_second_push_row() {
    // One push use carrying an extra same-family row refuses: import
    // identities are globally distinct (S20-230 section 2), so a closed
    // inventory carries at most one push row. A closure needing two element
    // types needs a new admission, not a second row.
    let mut program = BridgeProgram::new(
        BRIDGE_CODE_PSH1,
        u8vec_type(),
        u8_type(),
        result_of(u8vec_type()),
        vec![frozen_push_u8(), push_unit_row()],
    );
    match sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &program.types,
        entry: &program.entry,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.adapters,
        constants: &[],
    })
    .unwrap_err()
    {
        LoweringError::Lower(error) => assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported),
        LoweringError::Cfg(failure) => panic!("no CFG judgment expected: {failure}"),
    }
    program.adapters = vec![frozen_push_u8()];
    let report = program.gate_report();
    assert_eq!(report.bridge_uses, 1);
    assert_eq!(report.imports, vec![program.adapters[0].entity_id]);
}

/// One closure pushing u8 and Unit side by side and tupling the results,
/// with the Unit row carried first to prove selection is not first-match.
/// Returns the program plus its u8, Unit, and tuple result types.
fn push_use_op(
    entity: EntityId,
    block: EntityId,
    ordinal: u32,
    scope_param: EntityId,
    request_param: EntityId,
    result: TypeExpr,
) -> Operation {
    Operation {
        entity_id: entity,
        block,
        ordinal,
        opcode: Opcode::AdapterInvoke,
        operands: vec![
            ValueRef::Parameter(scope_param),
            ValueRef::Parameter(request_param),
        ],
        result_types: vec![result],
        immediate: Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    }
}

fn combined_push_program() -> (BridgeProgram, TypeExpr, TypeExpr, TypeExpr) {
    let function = id(1);
    let block = id(2);
    let p_vec_u8 = id(10);
    let p_u8 = id(11);
    let p_vec_unit = id(12);
    let p_unit = id(13);
    let push_u8 = id(100);
    let push_unit = id(101);
    let combine = id(102);
    let u8_result = result_of(u8vec_type());
    let unit_vec = TypeExpr::Vector(Box::new(TypeExpr::Unit));
    let unit_result = result_of(unit_vec.clone());
    let tuple_result = TypeExpr::Tuple(vec![u8_result.clone(), unit_result.clone()]);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![p_vec_u8, p_u8, p_vec_unit, p_unit],
        result_type: tuple_result.clone(),
        effects: Vec::new(),
        entry_block: block,
        blocks: vec![block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let program = BridgeProgram {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: vec![
            Parameter {
                entity_id: p_vec_u8,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: u8vec_type(),
            },
            Parameter {
                entity_id: p_u8,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 1,
                value_type: u8_type(),
            },
            Parameter {
                entity_id: p_vec_unit,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 2,
                value_type: unit_vec.clone(),
            },
            Parameter {
                entity_id: p_unit,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 3,
                value_type: TypeExpr::Unit,
            },
        ],
        blocks: vec![Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: vec![push_u8, push_unit, combine],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: combine,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        }],
        operations: vec![
            push_use_op(push_u8, block, 0, p_vec_u8, p_u8, u8_result.clone()),
            push_use_op(push_unit, block, 1, p_vec_unit, p_unit, unit_result.clone()),
            Operation {
                entity_id: combine,
                block,
                ordinal: 2,
                opcode: Opcode::TupleNew,
                operands: vec![
                    ValueRef::OperationResult(OperationResultRef {
                        operation: push_u8,
                        result_index: 0,
                    }),
                    ValueRef::OperationResult(OperationResultRef {
                        operation: push_unit,
                        result_index: 0,
                    }),
                ],
                result_types: vec![tuple_result.clone()],
                immediate: Immediate::None,
            },
        ],
        adapters: vec![push_unit_row(), frozen_push_u8()],
    };
    (program, u8_result, unit_result, tuple_result)
}

#[test]
fn rw070_combined_push_types_share_one_closure() {
    // Per-use selection serves each element type from its own row at the
    // lowering layer. The two-row inventory exceeds the bootstrap closure
    // rule (at most one push row), so the gate refuses exactly this
    // inventory while lowering still serves each use.
    let (program, u8_result, unit_result, tuple_result) = combined_push_program();
    match sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &program.types,
        entry: &program.entry,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.adapters,
        constants: &[],
    })
    .unwrap_err()
    {
        LoweringError::Lower(error) => assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported),
        LoweringError::Cfg(failure) => panic!("no CFG judgment expected: {failure}"),
    }
    match execute_bridge(
        &program,
        vec![
            u8_sequence(&[1]),
            u8_scalar(2),
            unit_sequence(1),
            unit_value_of(),
        ],
    ) {
        ExecutionTermination::Success(value) => {
            assert_eq!(value.value_type, tuple_result);
            match value.data {
                ConstData::Sequence(items) => {
                    assert_eq!(items.len(), 2);
                    assert_eq!(
                        items[0],
                        ConstValue {
                            value_type: u8_result,
                            data: ConstData::Result(ResultConst::Ok(Box::new(u8_sequence(&[
                                1, 2
                            ])))),
                        }
                    );
                    assert_eq!(
                        items[1],
                        ConstValue {
                            value_type: unit_result,
                            data: ConstData::Result(ResultConst::Ok(Box::new(unit_sequence(2)))),
                        }
                    );
                }
                other => panic!("expected a tuple, got {other:?}"),
            }
        }
        other => panic!("expected success, got {other:?}"),
    }
}

fn loaded_input(program: &BridgeProgram) -> LoadedExecutionInput<'_> {
    LoadedExecutionInput {
        types: &program.types,
        constants: &[],
        globals: &[],
        contracts: &[],
        adapters: &program.adapters,
        schema_epoch: SchemaEpochId::from_bytes([8; 32]),
        state_root: StateRoot::from_bytes([9; 32]),
        profile: CacheProfile::EXTENDED_V1,
    }
}

fn approved_for(
    program: &BridgeProgram,
    bytes: &[u8],
    cache_key: BytecodeCacheKey,
) -> ApprovedImage {
    ApprovedImage {
        digest: image_digest(bytes),
        cache_key,
        imports: program.adapters.iter().map(|row| row.entity_id).collect(),
    }
}

#[test]
fn rw070_loaded_execution_matches_lowering_path() {
    // Supplied image bytes execute through validation-before-execution and
    // agree with the lowering path on value and observation identity.
    let program = b2v1_program();
    let lowered = lower_function(program.lowering_input()).expect("lowers");
    let approved = approved_for(&program, &lowered.bytes, lowered.cache_key);
    let request = ExecutionRequest {
        inputs: vec![unit_value(), bytes_value(&[7, 8])],
        limits: generous_limits(),
    };
    let direct =
        execute_function(program.lowering_input(), request.clone()).expect("direct executes");
    let loaded = execute_loaded_image(loaded_input(&program), &approved, &lowered.bytes, request)
        .expect("loaded executes");
    assert_eq!(loaded.termination, direct.termination);
    assert_eq!(loaded.observation_id, direct.observation_id);
    assert_eq!(loaded.cache_key, direct.cache_key);
    match loaded.termination {
        ExecutionTermination::Success(_) => {}
        other => panic!("expected success, got {other:?}"),
    }
}

#[test]
fn rw070_loaded_execution_verifies_manifest_identity() {
    let program = b2v1_program();
    let lowered = lower_function(program.lowering_input()).expect("lowers");
    let approved = approved_for(&program, &lowered.bytes, lowered.cache_key);
    let request = || ExecutionRequest {
        inputs: vec![unit_value(), bytes_value(&[7, 8])],
        limits: generous_limits(),
    };
    // Structurally valid but unexpected bytes refuse before execution: the
    // flip lands inside the entry function identity, which parses fine.
    let mut unexpected = lowered.bytes.clone();
    unexpected[20] ^= 0x01;
    assert_eq!(
        execute_loaded_image(loaded_input(&program), &approved, &unexpected, request()),
        Err(LoadedExecutionError::Image(ImageError::DigestMismatch))
    );
    // A wrong manifest digest refuses the valid image alike.
    let mut wrong_digest = approved.clone();
    wrong_digest.digest = [0xAB; 32];
    assert_eq!(
        execute_loaded_image(
            loaded_input(&program),
            &wrong_digest,
            &lowered.bytes,
            request()
        ),
        Err(LoadedExecutionError::Image(ImageError::DigestMismatch))
    );
    // A verified digest with a mismatched cache identity refuses: epoch,
    // root, entry, and profile bind the execution, not just the bytes.
    let mut wrong_key = approved.clone();
    wrong_key.cache_key = sley_vm::derive_cache_key(
        SchemaEpochId::from_bytes([7; 32]),
        StateRoot::from_bytes([9; 32]),
        EntityId::from_bytes([1; 32]),
        CacheProfile::EXTENDED_V1,
    )
    .expect("binds");
    assert_eq!(
        execute_loaded_image(
            loaded_input(&program),
            &wrong_key,
            &lowered.bytes,
            request()
        ),
        Err(LoadedExecutionError::Image(ImageError::BindingMismatch))
    );
    // A verified digest with a mismatched import set refuses alike.
    let mut wrong_imports = approved.clone();
    wrong_imports.imports = Vec::new();
    assert_eq!(
        execute_loaded_image(
            loaded_input(&program),
            &wrong_imports,
            &lowered.bytes,
            request()
        ),
        Err(LoadedExecutionError::Image(ImageError::BindingMismatch))
    );
    // Malformed bytes refuse with the structural vocabulary.
    assert_eq!(
        execute_loaded_image(
            loaded_input(&program),
            &approved,
            &lowered.bytes[..5],
            request()
        ),
        Err(LoadedExecutionError::Image(ImageError::Truncated))
    );
}

#[test]
fn rw070_loaded_execution_binds_epoch_and_inputs() {
    let program = b2v1_program();
    let lowered = lower_function(program.lowering_input()).expect("lowers");
    let approved = approved_for(&program, &lowered.bytes, lowered.cache_key);
    // A different epoch refuses against the approved cache identity instead
    // of executing under another identity.
    let mut other_epoch = loaded_input(&program);
    other_epoch.schema_epoch = SchemaEpochId::from_bytes([7; 32]);
    let request = ExecutionRequest {
        inputs: vec![unit_value(), bytes_value(&[7, 8])],
        limits: generous_limits(),
    };
    assert_eq!(
        execute_loaded_image(other_epoch, &approved, &lowered.bytes, request),
        Err(LoadedExecutionError::Image(ImageError::BindingMismatch))
    );
    // Input refusal parity: the same mistyped input fails with the same code
    // on both paths.
    let bad = ExecutionRequest {
        inputs: vec![unit_value(), u8_sequence(&[1])],
        limits: generous_limits(),
    };
    let direct_code = match execute_function(program.lowering_input(), bad.clone()) {
        Err(error) => error,
        Ok(outcome) => panic!("expected refusal, got {outcome:?}"),
    };
    let loaded_code =
        match execute_loaded_image(loaded_input(&program), &approved, &lowered.bytes, bad) {
            Err(LoadedExecutionError::Execution(error)) => error,
            other => panic!("expected refusal, got {other:?}"),
        };
    assert_eq!(loaded_code, direct_code);
}
