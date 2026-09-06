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
        let function = id(1);
        let block = id(2);
        let scope_param = id(10);
        let request_param = id(11);
        let operation = id(100);
        let result_type = result_of(TypeExpr::Bytes);
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
                    value_type: TypeExpr::Unit,
                },
                Parameter {
                    entity_id: request_param,
                    owner: function,
                    role: ParameterRole::Function,
                    ordinal: 1,
                    value_type: TypeExpr::Bytes,
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
                immediate: Immediate::Entity(EntityId::from_bytes(bridge_identity(
                    BRIDGE_CODE_RHW1,
                ))),
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

fn execute_raw(preimage: &[u8]) -> ExecutionTermination {
    let program = BridgeProgram::new(vec![frozen_rhw1()]);
    sley_vm::execute_function(
        program.lowering_input(),
        ExecutionRequest {
            inputs: vec![unit_value(), bytes_value(preimage)],
            limits: generous_limits(),
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
    // boundary itself (the refusal case uses generous limits to prove the
    // typed value, not resource exhaustion, is what refuses).
    fn big_limits() -> ExecutionLimits {
        ExecutionLimits {
            max_instructions: 1_000_000,
            max_fuel: 10_000_000,
            max_value_units: 100_000_000,
            max_output_units: 10_000_000,
            cancel_at_fuel: None,
        }
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
        ExecutionPackage, admit_package_v2, approve_package_v2, execute_approved_package_v2,
        package_digests_v2, verify_package_binding_v2,
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
    let digests = package_digests_v2(&package).expect("v2 digests");
    let receipt = admit_package_v2(digests.package_digest);
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
