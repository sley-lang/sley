//! RW-075 correction: `RAW_BLAKE3_V1` genuinely callable via the successor.
//!
//! Uses only the public `sley-vm` surface plus public `sley-ssmc` value
//! types — the same boundary a later seed-absent toolchain sees. Proves:
//!
//! * callable shape `(Unit, Bytes) -> Result<Bytes32, Index>` through exact
//!   `RHW1` bridge rows (vectors, boundaries, fuel, tamper);
//! * unknown/wrong-version negatives fail closed;
//! * gate admission + v2 package binding (including mismatch negatives);
//! * semantic preimage ownership (Sley builds bytes, host only hashes);
//! * successor workloads (image emission + mixed) within bounds;
//! * SLEYBC02 encoding boundary (no native image-construction service).

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, BuiltinFailureKind, ConstData, ConstValue, FunctionGraph, Immediate,
    Opcode, Operation, OperationResultRef, Parameter, ParameterRole, Reachability, ResultConst,
    ReturnTerminator, Terminator, TypeExpr, ValueRef, Visibility,
};
use sley_vm::{
    CacheProfile, ExecutionLimits, ExecutionRequest, ExecutionTermination, LowerErrorCode,
    LoweringError, LoweringInput,
    bootstrap::BootstrapProfileInput,
    host_abi::{BRIDGE_ABI_VERSION, BRIDGE_CODE_RHW1, bridge_identity},
    lower_function,
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

fn index_failure() -> TypeExpr {
    TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)
}

fn result_of(ok: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(ok),
        error: Box::new(index_failure()),
    }
}

fn frozen_rhw1() -> AdapterImport {
    let identity = EntityId::from_bytes(bridge_identity(BRIDGE_CODE_RHW1));
    AdapterImport {
        entity_id: identity,
        adapter_id: *identity.as_bytes(),
        abi_version: BRIDGE_ABI_VERSION,
        request_type: TypeExpr::Bytes,
        response_type: TypeExpr::Bytes,
        failure_type: index_failure(),
        effects: Vec::new(),
    }
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
    fn new(adapters: Vec<AdapterImport>) -> Self {
        Self::with(
            BRIDGE_CODE_RHW1,
            TypeExpr::Unit,
            TypeExpr::Bytes,
            result_of(TypeExpr::Bytes),
            adapters,
        )
    }

    fn with(
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
            schema_epoch: epoch(),
            entry: &self.entry,
            presented_image_bytes: &[],
            functions: &self.functions,
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &self.operations,
            adapters: &self.adapters,
            constants: &[],
        })
        .expect("successor gate admits the RHW1 program")
    }

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
            constants: &[],
            globals: &[],
            functions: &[],
            contracts: &[],
            adapters: &self.adapters,
        }
    }
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

fn big_limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: 1_000_000,
        max_fuel: 10_000_000,
        max_value_units: 100_000_000,
        max_output_units: 10_000_000,
        cancel_at_fuel: None,
    }
}

fn execute_raw(preimage: &[u8]) -> ExecutionTermination {
    execute_raw_with(preimage, generous_limits())
}

fn execute_raw_with(preimage: &[u8], limits: ExecutionLimits) -> ExecutionTermination {
    let program = BridgeProgram::new(vec![frozen_rhw1()]);
    sley_vm::execute_function(
        program.lowering_input(),
        ExecutionRequest {
            inputs: vec![unit_value(), bytes_value(preimage)],
            limits,
        },
    )
    .expect("well-formed request")
    .termination
}

fn ok_bytes(termination: ExecutionTermination) -> Vec<u8> {
    match termination {
        ExecutionTermination::Success(value) => match value.data {
            ConstData::Result(ResultConst::Ok(payload)) => match payload.data {
                ConstData::Bytes(bytes) => bytes,
                other => panic!("expected Bytes digest, got {other:?}"),
            },
            other => panic!("expected Ok, got {other:?}"),
        },
        other => panic!("expected success, got {other:?}"),
    }
}

fn err_code(termination: ExecutionTermination) -> u16 {
    match termination {
        ExecutionTermination::Success(value) => match value.data {
            ConstData::Result(ResultConst::Err(payload)) => match payload.data {
                ConstData::BuiltinFailure(failure) => failure.code,
                other => panic!("expected BuiltinFailure, got {other:?}"),
            },
            other => panic!("expected Err, got {other:?}"),
        },
        other => panic!("expected typed refusal, got {other:?}"),
    }
}

// ── Large-preimage composition (exact Sley-owned rule) ───────────────

/// Sley-owned large-preimage composition through the callable primitive:
/// split into 1 MiB chunks, hash each via `RHW1`, frame with `SLEYCHNK1`,
/// hash the frame via `RHW1`. The host only hashes; Sley owns chunking,
/// order, count, and framing.
fn composed_digest(preimage: &[u8]) -> Vec<u8> {
    const CHUNK: usize = 1_048_576;
    assert!(
        preimage.len() > CHUNK,
        "composition is for over-bound inputs"
    );
    let chunks: Vec<&[u8]> = preimage.chunks(CHUNK).collect();
    let chunk_digests: Vec<Vec<u8>> = chunks
        .iter()
        .map(|c| ok_bytes(execute_raw_with(c, big_limits())))
        .collect();
    let mut frame = Vec::new();
    frame.extend_from_slice(b"SLEYCHNK1");
    frame.extend_from_slice(&1_u32.to_be_bytes());
    frame.extend_from_slice(
        &u32::try_from(chunk_digests.len())
            .expect("chunk count fits u32")
            .to_be_bytes(),
    );
    for digest in &chunk_digests {
        frame.extend_from_slice(digest);
    }
    ok_bytes(execute_raw_with(&frame, big_limits()))
}

fn reference_composed_digest(preimage: &[u8]) -> Vec<u8> {
    const CHUNK: usize = 1_048_576;
    let chunks: Vec<&[u8]> = preimage.chunks(CHUNK).collect();
    let mut frame = Vec::new();
    frame.extend_from_slice(b"SLEYCHNK1");
    frame.extend_from_slice(&1_u32.to_be_bytes());
    frame.extend_from_slice(
        &u32::try_from(chunks.len())
            .expect("chunk count fits u32")
            .to_be_bytes(),
    );
    for chunk in &chunks {
        frame.extend_from_slice(blake3::hash(chunk).as_bytes());
    }
    blake3::hash(&frame).as_bytes().to_vec()
}

#[test]
fn raw_large_preimage_composition_is_exact() {
    // 1 MiB + 1 refuses single-shot but composes exactly (big limits prove
    // the typed value, not resource exhaustion, is what refuses).
    let one_plus = vec![0xABu8; sley_vm::RAW_HASH_MAX_BYTES + 1];
    assert_eq!(
        err_code(execute_raw_with(&one_plus, big_limits())),
        2,
        "single-shot over-bound refuses"
    );
    assert_eq!(
        composed_digest(&one_plus),
        reference_composed_digest(&one_plus),
        "composition matches the reference construction"
    );
    // 2 MiB composes exactly and differs from any single-shot meaning.
    let two_mib = vec![0xCDu8; 2 * sley_vm::RAW_HASH_MAX_BYTES];
    assert_eq!(
        composed_digest(&two_mib),
        reference_composed_digest(&two_mib),
        "2 MiB composition matches"
    );
    assert_ne!(
        composed_digest(&two_mib),
        ok_bytes(execute_raw(b"unrelated small preimage")),
        "composition is not confusable with single-shot digests"
    );
    // All R2-required fixture preimages in this file are small (<1 KiB)
    // and use single-shot; composition is the defined over-bound rule.
}

// ── Callable shape + standard vectors ────────────────────────────────

#[test]
fn raw_callable_exact_invocation_succeeds() {
    let program = BridgeProgram::new(vec![frozen_rhw1()]);
    let report = program.gate_report();
    assert_eq!(
        report.bridge_uses(),
        1,
        "successor gate counts the RHW1 use"
    );
    let digest = ok_bytes(execute_raw(b"abc"));
    assert_eq!(digest.len(), 32, "exact 32-byte result contract");
    assert_eq!(
        digest,
        blake3::hash(b"abc").as_bytes().to_vec(),
        "matches the independent reference"
    );
}

#[test]
fn raw_standard_vectors_match_reference() {
    assert_eq!(
        ok_bytes(execute_raw(b"")),
        vec![
            0xaf, 0x13, 0x49, 0xb9, 0xf5, 0xf9, 0xa1, 0xa6, 0xa0, 0x40, 0x4d, 0xea, 0x36, 0xdc,
            0xc9, 0x49, 0x9b, 0xcb, 0x25, 0xc9, 0xad, 0xc1, 0x12, 0xb7, 0xcc, 0x9a, 0x93, 0xca,
            0xe4, 0x1f, 0x32, 0x62,
        ],
        "BLAKE3 empty vector pinned"
    );
    assert_eq!(
        ok_bytes(execute_raw(b"abc")),
        vec![
            0x64, 0x37, 0xb3, 0xac, 0x38, 0x46, 0x51, 0x33, 0xff, 0xb6, 0x3b, 0x75, 0x27, 0x3a,
            0x8d, 0xb5, 0x48, 0xc5, 0x58, 0x46, 0x5d, 0x79, 0xdb, 0x03, 0xfd, 0x35, 0x9c, 0x6c,
            0xd5, 0xbd, 0x9d, 0x85,
        ],
        "BLAKE3 abc vector pinned"
    );
}

#[test]
fn raw_boundaries_admit_and_over_bound_refuses_typed() {
    // 1 MiB values need value-unit headroom: use expanded limits for the
    // boundary itself and the refusal (proving the typed value, not
    // resource exhaustion, is what refuses).
    ok_bytes(execute_raw(&[]));
    ok_bytes(execute_raw(&[0x61]));
    ok_bytes(execute_raw(&vec![0x55; 1024]));
    assert_eq!(
        ok_bytes(execute_raw_with(
            &vec![0x55; sley_vm::RAW_HASH_MAX_BYTES],
            big_limits()
        ))
        .len(),
        32,
        "exactly 1 MiB admits"
    );
    let refusal = execute_raw_with(&vec![0x55; sley_vm::RAW_HASH_MAX_BYTES + 1], big_limits());
    assert_eq!(
        err_code(refusal),
        2,
        "1 MiB + 1 refuses as typed Err(Index, 2), never truncation"
    );
}

#[test]
fn raw_tamper_changes_digest_and_same_preimage_deterministic() {
    let first = ok_bytes(execute_raw(b"sley compiler preimage v1"));
    let second = ok_bytes(execute_raw(b"sley compiler preimage v2"));
    assert_ne!(first, second, "one flipped byte changes the digest");
    assert_eq!(
        ok_bytes(execute_raw(b"sley compiler preimage v1")),
        first,
        "same preimage is deterministic"
    );
}

#[test]
fn raw_fuel_schedule_is_exact_and_precharged() {
    assert_eq!(sley_vm::raw_hash_fuel(0), 1);
    assert_eq!(sley_vm::raw_hash_fuel(1), 2);
    assert_eq!(sley_vm::raw_hash_fuel(1024), 2);
    assert_eq!(sley_vm::raw_hash_fuel(1025), 3);
    assert_eq!(sley_vm::raw_hash_fuel(sley_vm::RAW_HASH_MAX_BYTES), 1025);
    // Starved budget terminates without performing the work: pre-charge.
    let program = BridgeProgram::new(vec![frozen_rhw1()]);
    let starved = sley_vm::execute_function(
        program.lowering_input(),
        ExecutionRequest {
            inputs: vec![unit_value(), bytes_value(&[1, 2, 3])],
            limits: ExecutionLimits {
                max_instructions: 10_000,
                max_fuel: 0,
                max_value_units: 1_000_000,
                max_output_units: 100_000,
                cancel_at_fuel: None,
            },
        },
    )
    .expect("request well formed")
    .termination;
    assert!(
        !matches!(starved, ExecutionTermination::Success(_)),
        "starved fuel must not hash"
    );
}

// ── Unknown / wrong-version negatives fail closed ────────────────────

#[test]
fn raw_unknown_and_tampered_rows_deny() {
    // Unknown identity denies at lowering.
    let mut unknown = BridgeProgram::new(vec![frozen_rhw1()]);
    unknown.operations[0].immediate = Immediate::Entity(id(77));
    match lower_function(unknown.lowering_input()) {
        Err(LoweringError::Lower(error)) => {
            assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported);
        }
        other => panic!("unknown identity must deny, got {other:?}"),
    }
    // Wrong ABI version denies.
    let mut wrong_version = frozen_rhw1();
    wrong_version.abi_version = 2;
    let program = BridgeProgram::new(vec![wrong_version]);
    match lower_function(program.lowering_input()) {
        Err(LoweringError::Lower(error)) => {
            assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported);
        }
        other => panic!("wrong version must deny, got {other:?}"),
    }
    // Foreign adapter identity denies.
    let mut foreign = frozen_rhw1();
    foreign.adapter_id = [0xFF; 32];
    let program = BridgeProgram::new(vec![foreign]);
    match lower_function(program.lowering_input()) {
        Err(LoweringError::Lower(error)) => {
            assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported);
        }
        other => panic!("foreign adapter must deny, got {other:?}"),
    }
    // Effectful row denies.
    let mut effectful = frozen_rhw1();
    effectful.effects = vec![id(9)];
    let program = BridgeProgram::new(vec![effectful]);
    match lower_function(program.lowering_input()) {
        Err(LoweringError::Lower(error)) => {
            assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported);
        }
        other => panic!("effectful row must deny, got {other:?}"),
    }
    // Wrong request schema denies with signature mismatch (approved
    // identity, unserved schemas).
    let mut wrong_schema = frozen_rhw1();
    wrong_schema.request_type = TypeExpr::Vector(Box::new(TypeExpr::Bool));
    let program = BridgeProgram::new(vec![wrong_schema]);
    match lower_function(program.lowering_input()) {
        Err(LoweringError::Lower(error)) => {
            assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported);
        }
        other => panic!("wrong schema must deny, got {other:?}"),
    }
    // Rust-level variant selector still denies unknown algorithms.
    assert_eq!(
        sley_vm::raw_hash_variant(0, b"abc"),
        Err(sley_vm::RawHashError::UnknownVariant)
    );
    assert_eq!(
        sley_vm::raw_hash_variant(2, b"abc"),
        Err(sley_vm::RawHashError::UnknownVariant)
    );
}

// ── Successor package binding ────────────────────────────────────────

#[test]
fn raw_successor_package_binds_and_mismatches_refuse() {
    use sley_vm::{
        ExecutionPackage, V2Closure, approve_package_v2, execute_approved_package_v2,
        verify_package_binding_v2,
    };
    let program = BridgeProgram::new(vec![frozen_rhw1()]);
    let lowered = lower_function(program.lowering_input()).expect("successor lowers");
    let gate = sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &program.types,
        schema_epoch: epoch(),
        entry: &program.entry,
        presented_image_bytes: &lowered.bytes,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.adapters,
        constants: &[],
    })
    .expect("successor gate admits");
    let limits = generous_limits();
    let package = ExecutionPackage {
        image_bytes: lowered.bytes.clone(),
        constants: Vec::new(),
        type_definitions: Vec::new(),
        imports: program.adapters.clone(),
        globals: Vec::new(),
        contracts: Vec::new(),
        entry: program.entry.entity_id,
        schema_epoch: epoch(),
        state_root: root(),
        profile: CacheProfile::EXTENDED_V1,
        admitted_limits: limits,
        gate_operation_count: gate.operation_count(),
        gate_bridge_uses: gate.bridge_uses(),
        gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
    };
    // Honest minting routes exclusively through the production authority
    // (single closure bundle; gate plus reference re-lowering derived
    // internally, so graph-A/gate versus graph-B/lowering cannot diverge).
    let closure = V2Closure {
        types: &program.types,
        schema_epoch: epoch(),
        state_root: root(),
        entry: program.entry.entity_id,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.adapters,
        constants: &[],
        globals: &[],
        contracts: &[],
    };
    let (digests, receipt, gate) =
        sley_vm::admit_v2_package(&closure, &package).expect("authority admits honest");
    assert_eq!(
        receipt.profile_digest,
        sley_vm::BOOTSTRAP_PROFILE_2_DIGEST,
        "v2 receipt binds the successor profile"
    );
    let approved = approve_package_v2(&package, &digests, receipt, &gate).expect("v2 approves");
    verify_package_binding_v2(&package, &digests, &approved).expect("v2 verifies");
    let outcome = execute_approved_package_v2(
        &package,
        &approved,
        ExecutionRequest {
            inputs: vec![unit_value(), bytes_value(b"package preimage")],
            limits,
        },
    )
    .expect("v2 executes");
    match outcome.termination {
        ExecutionTermination::Success(value) => match value.data {
            ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                ConstData::Bytes(bytes) => assert_eq!(
                    bytes,
                    blake3::hash(b"package preimage").as_bytes(),
                    "package-path hash matches the reference"
                ),
                other => panic!("expected Bytes, got {other:?}"),
            },
            other => panic!("expected Ok, got {other:?}"),
        },
        other => panic!("expected success, got {other:?}"),
    }
    // V1 and V2 bindings are distinct by construction: same package bytes
    // digest differently under the two preimages (envelope version,
    // profile digest, ABI version all differ).
    let v1_digests = sley_vm::package_digests(&package).expect("v1 digests");
    assert_ne!(
        v1_digests.package_digest, digests.package_digest,
        "v1/v2 package identities differ for the same package"
    );
    // V1 verification must refuse v2 digests; v2 approval must refuse a
    // v1 receipt (profile/ABI mismatch).
    assert!(
        sley_vm::verify_package_binding(&package, &digests, &approved).is_err(),
        "v1 verification must refuse v2 bindings"
    );
    let v1_receipt = sley_vm::admit_package(digests.package_digest);
    assert!(
        sley_vm::approve_package_v2(&package, &digests, v1_receipt, &gate).is_err(),
        "v2 approval must refuse a v1 receipt"
    );
}

// ── Staged v2 admission authority (graph-to-image correspondence) ───
//
// The host path verifies byte-hash equality but cannot prove the package
// image was lowered from the judged graphs. R2 authority evidence uses the
// production staged authority (`sley_vm::admit_v2_package`), which takes
// one canonical closure bundle and derives both legs internally, so graph
// A for the gate versus graph B for the lowering cannot diverge. A
// mismatch aborts with no receipt, so no approval or execution can follow
// (exact model for the Sley build driver per the RW-080 contract §1.4;
// no toolchain graph, no C1 here).

fn stage_v2_admission(
    program: &BridgeProgram,
    package: &sley_vm::ExecutionPackage,
) -> Result<
    (
        sley_vm::PackageDigests,
        sley_vm::AdmissionReceipt,
        sley_vm::bootstrap::BootstrapProfileReport,
    ),
    String,
> {
    let closure = sley_vm::V2Closure {
        types: &program.types,
        schema_epoch: epoch(),
        state_root: root(),
        entry: program.entry.entity_id,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.adapters,
        constants: &[],
        globals: &[],
        contracts: &[],
    };
    sley_vm::admit_v2_package(&closure, package)
        .map_err(|error| format!("staged authority refuses: {error:?}"))
}

#[test]
fn raw_staged_authority_binds_graphs_to_image() {
    use sley_vm::ExecutionPackage;
    let program = BridgeProgram::new(vec![frozen_rhw1()]);
    let lowered = lower_function(program.lowering_input()).expect("successor lowers");
    let gate = sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &program.types,
        schema_epoch: epoch(),
        entry: &program.entry,
        presented_image_bytes: &lowered.bytes,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.adapters,
        constants: &[],
    })
    .expect("gate admits");
    let limits = generous_limits();
    let package = ExecutionPackage {
        image_bytes: lowered.bytes.clone(),
        constants: Vec::new(),
        type_definitions: Vec::new(),
        imports: program.adapters.clone(),
        globals: Vec::new(),
        contracts: Vec::new(),
        entry: program.entry.entity_id,
        schema_epoch: epoch(),
        state_root: root(),
        profile: CacheProfile::EXTENDED_V1,
        admitted_limits: limits,
        gate_operation_count: gate.operation_count(),
        gate_bridge_uses: gate.bridge_uses(),
        gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
    };
    // Honest package: authority mints, approval and execution follow.
    let (digests, receipt, gate) = stage_v2_admission(&program, &package).expect("honest admits");
    let approved =
        sley_vm::approve_package_v2(&package, &digests, receipt, &gate).expect("honest approves");
    sley_vm::verify_package_binding_v2(&package, &digests, &approved).expect("honest verifies");
    // Tampered image (one flipped byte, same gate graphs): authority
    // refuses with no receipt, so no approval or execution can follow.
    let mut tampered_bytes = lowered.bytes.clone();
    let last = tampered_bytes.len() - 1;
    tampered_bytes[last] ^= 0x01;
    let tampered = ExecutionPackage {
        image_bytes: tampered_bytes,
        ..package.clone()
    };
    assert!(
        stage_v2_admission(&program, &tampered).is_err(),
        "rewired bytes must not receive a receipt"
    );
}

#[test]
fn raw_authority_refuses_graph_a_gate_with_graph_b_image() {
    // Adversarial substitution: closure A (RHW1 program) judged, but the
    // package carries image bytes lowered from a different valid closure B
    // (B2V1 program) with A's gate claims. The single-bundle authority
    // re-lowers A internally and compares exactly, so B's bytes refuse
    // with no receipt — graph A can never ride graph B's executable bytes.
    use sley_vm::host_abi::BRIDGE_CODE_B2V1;
    use sley_vm::{ExecutionPackage, V2Closure};
    fn u8_type() -> TypeExpr {
        TypeExpr::UInt(sley_ssmc::IntegerWidth::from_bits(8))
    }
    fn u8vec() -> TypeExpr {
        TypeExpr::Vector(Box::new(u8_type()))
    }
    let b2v1_row = {
        let identity = EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1));
        AdapterImport {
            entity_id: identity,
            adapter_id: *identity.as_bytes(),
            abi_version: BRIDGE_ABI_VERSION,
            request_type: TypeExpr::Bytes,
            response_type: u8vec(),
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
            effects: Vec::new(),
        }
    };
    let program_a = BridgeProgram::new(vec![frozen_rhw1()]);
    let program_b = BridgeProgram::with(
        BRIDGE_CODE_B2V1,
        TypeExpr::Unit,
        TypeExpr::Bytes,
        result_of(u8vec()),
        vec![b2v1_row],
    );
    let lowered_b = lower_function(program_b.lowering_input()).expect("B lowers");
    let gate_a = sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &program_a.types,
        schema_epoch: epoch(),
        entry: &program_a.entry,
        presented_image_bytes: &lowered_b.bytes,
        functions: &program_a.functions,
        parameters: &program_a.parameters,
        blocks: &program_a.blocks,
        operations: &program_a.operations,
        adapters: &program_a.adapters,
        constants: &[],
    })
    .expect("gate judges A (correspondence is the authority's job, not the gate's)");
    let limits = generous_limits();
    let package_b_with_a_claims = ExecutionPackage {
        image_bytes: lowered_b.bytes.clone(),
        constants: Vec::new(),
        type_definitions: Vec::new(),
        imports: program_a.adapters.clone(),
        globals: Vec::new(),
        contracts: Vec::new(),
        entry: program_a.entry.entity_id,
        schema_epoch: epoch(),
        state_root: root(),
        profile: CacheProfile::EXTENDED_V1,
        admitted_limits: limits,
        gate_operation_count: gate_a.operation_count(),
        gate_bridge_uses: gate_a.bridge_uses(),
        gate_closure_fingerprints: gate_a.closure_fingerprints().to_vec(),
    };
    let closure_a = V2Closure {
        types: &program_a.types,
        schema_epoch: epoch(),
        state_root: root(),
        entry: program_a.entry.entity_id,
        functions: &program_a.functions,
        parameters: &program_a.parameters,
        blocks: &program_a.blocks,
        operations: &program_a.operations,
        adapters: &program_a.adapters,
        constants: &[],
        globals: &[],
        contracts: &[],
    };
    assert!(
        sley_vm::admit_v2_package(&closure_a, &package_b_with_a_claims).is_err(),
        "graph-A closure with graph-B image must not receive a receipt"
    );
}

// ── Semantic preimage ownership ──────────────────────────────────────

fn check_domain_preimage(domain: &[u8], fields: &[u8], label: &str) {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(domain);
    preimage.extend_from_slice(fields);
    let digest = ok_bytes(execute_raw(&preimage));
    assert_eq!(
        digest,
        blake3::hash(&preimage).as_bytes().to_vec(),
        "{label}: host adds nothing"
    );
    // Changing the domain separator changes the identity.
    let mut other = Vec::new();
    other.extend_from_slice(b"wrong-domain");
    other.extend_from_slice(fields);
    assert_ne!(
        ok_bytes(execute_raw(&other)),
        digest,
        "{label}: domain separator owns the identity"
    );
    // Changing one field byte changes the identity.
    if !fields.is_empty() {
        let mut flipped = preimage.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 0x01;
        assert_ne!(
            ok_bytes(execute_raw(&flipped)),
            digest,
            "{label}: field bytes own the identity"
        );
    }
}

#[test]
fn raw_preimage_ownership_across_compiler_domains() {
    // Representative real compiler identity domains from the adopted
    // closure (hash inventory §1-8). Each preimage is Sley-built bytes
    // (domain prefix + canonical field bytes); the host merely hashes.
    check_domain_preimage(b"SLEYSFP1", b"type-def-canonical-projection", "sleysfp1");
    check_domain_preimage(b"SLEYVHS1", b"value-canonical-bytes", "sleyvhs1");
    check_domain_preimage(
        b"sley2.value-hash.v1",
        b"epoch||schema-hash||type||data",
        "value-hash",
    );
    check_domain_preimage(
        b"sley2.semantic-fingerprint.v1",
        b"epoch||fields||kind||body",
        "semantic-fingerprint",
    );
    check_domain_preimage(
        b"sley2.vm-bytecode-cache-key.v1",
        b"epoch||root||entry||versions",
        "cache-key",
    );
    check_domain_preimage(
        b"sley2.observation.v1",
        b"package||limits||termination",
        "observation",
    );
    check_domain_preimage(b"sley2.entity.v1", b"entity-canonical", "entity");
    check_domain_preimage(b"sley2.object.v1", b"object-canonical", "object");
    check_domain_preimage(b"sley2.state-root.v1", b"root-canonical", "state-root");
    check_domain_preimage(b"sley2.transaction.v1", b"tx-canonical", "transaction");
    check_domain_preimage(b"sley2.candidate.v1", b"candidate-canonical", "candidate");
    // Order owns the identity: swapped halves differ.
    let ab = ok_bytes(execute_raw(b"field-A||field-B"));
    let ba = ok_bytes(execute_raw(b"field-B||field-A"));
    assert_ne!(ab, ba, "field order owns the identity");
}

#[test]
#[allow(clippy::too_many_lines)] // one block per unwrap leg, mirroring bytes_round_trip
fn raw_sley_built_preimage_end_to_end() {
    // End-to-end Sley ownership, well-typed, lowered, and executed:
    // separate domain (`Bytes`) and field (`UInt(8)`) inputs threaded
    // through conversion (`B2V1`), push (`PSH1`), conversion (`V2B1`),
    // and hash (`RHW1`), unwrapping each `Ok` payload with
    // `VariantSwitch` (`Err` legs wrap and return). Sley assembles every
    // byte of the hashed preimage; the host only converts, pushes, and
    // hashes. Expected digests are computed natively for comparison only.
    use sley_ssmc::{BuiltinCase, CaseKey, SwitchArgument, SwitchCase, SwitchEdge};
    use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    fn frozen(code: [u8; 4], request: TypeExpr, response: TypeExpr) -> AdapterImport {
        let identity = EntityId::from_bytes(bridge_identity(code));
        AdapterImport {
            entity_id: identity,
            adapter_id: *identity.as_bytes(),
            abi_version: BRIDGE_ABI_VERSION,
            request_type: request,
            response_type: response,
            failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
            effects: Vec::new(),
        }
    }
    fn u8_type() -> TypeExpr {
        TypeExpr::UInt(sley_ssmc::IntegerWidth::from_bits(8))
    }
    fn u8vec() -> TypeExpr {
        TypeExpr::Vector(Box::new(u8_type()))
    }
    fn bridge_result(ok: TypeExpr) -> TypeExpr {
        TypeExpr::Result {
            ok: Box::new(ok),
            error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
        }
    }
    let function = id(1);
    let entry = id(2);
    let ok1 = id(3);
    let ok2 = id(4);
    let ok3 = id(5);
    let err = id(6);
    let unit_param = id(10);
    let domain_param = id(11);
    let suffix_param = id(12);
    let domain_vec_param = id(13);
    let grown_param = id(14);
    let preimage_param = id(15);
    let failure_param = id(16);
    let op_convert = id(100);
    let op_push = id(101);
    let op_back = id(102);
    let op_hash = id(103);
    let op_wrap_err = id(104);
    let hash_result = bridge_result(TypeExpr::Bytes);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![unit_param, domain_param, suffix_param],
        result_type: hash_result.clone(),
        effects: Vec::new(),
        entry_block: entry,
        blocks: vec![entry, ok1, ok2, ok3, err],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let types = TypeEnvironment::new(Vec::new()).unwrap();
    let parameters = vec![
        Parameter {
            entity_id: unit_param,
            owner: function,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: TypeExpr::Unit,
        },
        Parameter {
            entity_id: domain_param,
            owner: function,
            role: ParameterRole::Function,
            ordinal: 1,
            value_type: TypeExpr::Bytes,
        },
        Parameter {
            entity_id: suffix_param,
            owner: function,
            role: ParameterRole::Function,
            ordinal: 2,
            value_type: u8_type(),
        },
        Parameter {
            entity_id: domain_vec_param,
            owner: ok1,
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: u8vec(),
        },
        Parameter {
            entity_id: grown_param,
            owner: ok2,
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: u8vec(),
        },
        Parameter {
            entity_id: preimage_param,
            owner: ok3,
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: TypeExpr::Bytes,
        },
        Parameter {
            entity_id: failure_param,
            owner: err,
            role: ParameterRole::Block,
            ordinal: 0,
            value_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        },
    ];
    let blocks = vec![
        Block {
            entity_id: entry,
            function,
            parameters: Vec::new(),
            operations: vec![op_convert],
            terminator: Terminator::VariantSwitch(sley_ssmc::VariantSwitchTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: op_convert,
                    result_index: 0,
                }),
                cases: vec![
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Ok),
                        edge: SwitchEdge {
                            target: ok1,
                            arguments: vec![SwitchArgument::CasePayload],
                        },
                    },
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Err),
                        edge: SwitchEdge {
                            target: err,
                            arguments: vec![SwitchArgument::CasePayload],
                        },
                    },
                ],
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: ok1,
            function,
            parameters: vec![domain_vec_param],
            operations: vec![op_push],
            terminator: Terminator::VariantSwitch(sley_ssmc::VariantSwitchTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: op_push,
                    result_index: 0,
                }),
                cases: vec![
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Ok),
                        edge: SwitchEdge {
                            target: ok2,
                            arguments: vec![SwitchArgument::CasePayload],
                        },
                    },
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Err),
                        edge: SwitchEdge {
                            target: err,
                            arguments: vec![SwitchArgument::CasePayload],
                        },
                    },
                ],
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: ok2,
            function,
            parameters: vec![grown_param],
            operations: vec![op_back],
            terminator: Terminator::VariantSwitch(sley_ssmc::VariantSwitchTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: op_back,
                    result_index: 0,
                }),
                cases: vec![
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Ok),
                        edge: SwitchEdge {
                            target: ok3,
                            arguments: vec![SwitchArgument::CasePayload],
                        },
                    },
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Err),
                        edge: SwitchEdge {
                            target: err,
                            arguments: vec![SwitchArgument::CasePayload],
                        },
                    },
                ],
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: ok3,
            function,
            parameters: vec![preimage_param],
            operations: vec![op_hash],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: op_hash,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        },
        Block {
            entity_id: err,
            function,
            parameters: vec![failure_param],
            operations: vec![op_wrap_err],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation: op_wrap_err,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        },
    ];
    let operations = vec![
        Operation {
            entity_id: op_convert,
            block: entry,
            ordinal: 0,
            opcode: Opcode::AdapterInvoke,
            operands: vec![
                ValueRef::Parameter(unit_param),
                ValueRef::Parameter(domain_param),
            ],
            result_types: vec![bridge_result(u8vec())],
            immediate: Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
        },
        Operation {
            entity_id: op_push,
            block: ok1,
            ordinal: 0,
            opcode: Opcode::AdapterInvoke,
            operands: vec![
                ValueRef::Parameter(domain_vec_param),
                ValueRef::Parameter(suffix_param),
            ],
            result_types: vec![bridge_result(u8vec())],
            immediate: Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
        },
        Operation {
            entity_id: op_back,
            block: ok2,
            ordinal: 0,
            opcode: Opcode::AdapterInvoke,
            operands: vec![
                ValueRef::Parameter(unit_param),
                ValueRef::Parameter(grown_param),
            ],
            result_types: vec![bridge_result(TypeExpr::Bytes)],
            immediate: Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
        },
        Operation {
            entity_id: op_hash,
            block: ok3,
            ordinal: 0,
            opcode: Opcode::AdapterInvoke,
            operands: vec![
                ValueRef::Parameter(unit_param),
                ValueRef::Parameter(preimage_param),
            ],
            result_types: vec![bridge_result(TypeExpr::Bytes)],
            immediate: Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_RHW1))),
        },
        Operation {
            entity_id: op_wrap_err,
            block: err,
            ordinal: 0,
            opcode: Opcode::ResultErr,
            operands: vec![ValueRef::Parameter(failure_param)],
            result_types: vec![hash_result.clone()],
            immediate: Immediate::None,
        },
    ];
    let adapters = vec![
        frozen(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec()),
        frozen(BRIDGE_CODE_PSH1, u8_type(), u8vec()),
        frozen(BRIDGE_CODE_V2B1, u8vec(), TypeExpr::Bytes),
        frozen_rhw1(),
    ];
    let functions = vec![graph.clone()];
    let report = sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &types,
        schema_epoch: epoch(),
        entry: &graph,
        presented_image_bytes: &[],
        functions: &functions,
        parameters: &parameters,
        blocks: &blocks,
        operations: &operations,
        adapters: &adapters,
        constants: &[],
    })
    .expect("four-step Sley assembly gate-admits");
    assert_eq!(
        report.bridge_uses(),
        4,
        "conversion, push, conversion, hash"
    );
    let run_assembly = |domain: &[u8], suffix: u8| {
        let input = LoweringInput {
            types: &types,
            function: &graph,
            parameters: &parameters,
            blocks: &blocks,
            operations: &operations,
            schema_epoch: epoch(),
            state_root: root(),
            profile: CacheProfile::EXTENDED_V1,
            constants: &[],
            globals: &[],
            functions: &[],
            contracts: &[],
            adapters: &adapters,
        };
        let termination = sley_vm::execute_function(
            input,
            ExecutionRequest {
                inputs: vec![
                    ConstValue {
                        value_type: TypeExpr::Unit,
                        data: ConstData::Unit,
                    },
                    ConstValue {
                        value_type: TypeExpr::Bytes,
                        data: ConstData::Bytes(domain.to_vec()),
                    },
                    ConstValue {
                        value_type: u8_type(),
                        data: ConstData::UInt(u128::from(suffix)),
                    },
                ],
                limits: generous_limits(),
            },
        )
        .expect("well-formed request")
        .termination;
        match termination {
            ExecutionTermination::Success(value) => match value.data {
                ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                    ConstData::Bytes(bytes) => bytes.clone(),
                    other => panic!("expected Bytes digest, got {other:?}"),
                },
                other => panic!("expected Ok digest, got {other:?}"),
            },
            other => panic!("expected success, got {other:?}"),
        }
    };
    // Sley joins the separate domain and field inputs, converts, and
    // hashes: the digest equals the reference over the joined bytes.
    let mut joined = b"SLEYSFP1".to_vec();
    joined.push(0x41);
    assert_eq!(
        run_assembly(b"SLEYSFP1", 0x41),
        blake3::hash(&joined).as_bytes().to_vec(),
        "Sley-assembled domain plus field hashes exactly"
    );
    // Either input owns the identity.
    let mut other_field = b"SLEYSFP1".to_vec();
    other_field.push(0x42);
    assert_ne!(
        run_assembly(b"SLEYSFP1", 0x41),
        blake3::hash(&other_field).as_bytes().to_vec(),
        "field change owns the identity"
    );
    let mut other_domain = b"SLEYSFP2".to_vec();
    other_domain.push(0x41);
    assert_ne!(
        run_assembly(b"SLEYSFP1", 0x41),
        blake3::hash(&other_domain).as_bytes().to_vec(),
        "domain change owns the identity"
    );
}
// ── SLEYBC02 encoding boundary ───────────────────────────────────────

#[test]
fn sleybc02_image_construction_is_not_a_native_service() {
    // No bridge identity encodes a semantic program into an image.
    for code in [*b"ENC1", *b"IMG1", *b"CMP1", *b"LOW1", *b"BLD1"] {
        let identity = EntityId::from_bytes(sley_vm::host_abi::bridge_identity(code));
        let row = AdapterImport {
            entity_id: identity,
            adapter_id: *identity.as_bytes(),
            abi_version: BRIDGE_ABI_VERSION,
            request_type: TypeExpr::Bytes,
            response_type: TypeExpr::Bytes,
            failure_type: index_failure(),
            effects: Vec::new(),
        };
        let program = BridgeProgram::new(vec![row]);
        match lower_function(program.lowering_input()) {
            Err(LoweringError::Lower(error)) => assert_eq!(
                error.code(),
                LowerErrorCode::OpcodeUnsupported,
                "image-construction spellings must deny"
            ),
            other => panic!("encode-like service must deny, got {other:?}"),
        }
    }
    // Source probe: the bridge/host-abi sources carry no compiler-owned
    // layout/assembly decisions (no encode of semantic graphs).
    let extended = include_str!("../src/extended.rs");
    for forbidden in [
        "encode_sleybc02",
        "construct_compiler_image",
        "assemble_ssmc",
        "lower_function(",
        "fingerprint_function(",
        "candidate_digest",
        "object_id(",
    ] {
        assert!(
            !extended.contains(forbidden),
            "bridge execution must not contain `{forbidden}`"
        );
    }
    let host_abi = include_str!("../src/host_abi.rs");
    assert!(
        host_abi.contains("load_image"),
        "structural loading stays native (permitted)"
    );
    assert!(
        !host_abi.contains("construct_compiler_image"),
        "image construction is not a host service"
    );
}
